"""Exercise authoritative recovery followup and settlement in the delivered H5."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
from urllib.parse import urlsplit

from playwright.sync_api import sync_playwright


def pick_direction(page, dx, dy):
    canvas = page.locator("#map")
    bounds = canvas.bounding_box()
    page.locator("#selectGround").click()
    canvas.click(position={"x": bounds["width"] / 2 + dx, "y": bounds["height"] / 2 + dy})


def journey(browser, request, address, output, scenario, seats):
    room = request.post("/api/rooms", data={"scenario": scenario})
    assert room.status == 201, room.text()
    links = room.json()["links"]
    assert len(links) == seats
    contexts = [browser.new_context(viewport={"width": 1100, "height": 950}) for _ in links]
    pages = [context.new_page() for context in contexts]
    errors = []
    try:
        for page, link in zip(pages, links):
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.on("console", lambda message: errors.append(message.text) if message.type == "error" else None)
            page.goto(address + link["url"], wait_until="domcontentloaded")
        for page in pages:
            page.wait_for_function("window.__lastSnapshot?.phase === 'active' && window.__lastSnapshot.self.ready")
        actor, peer = pages[:2]
        peer_x = peer.evaluate("window.__lastSnapshot.self.position[0]")
        move = request.post("/api/input?" + urlsplit(links[1]["url"]).query,
                            data={"kind": "move", "direction": [1, 0]})
        assert move.ok, move.text()
        peer.wait_for_function("x => window.__lastSnapshot.self.position[0] > x", arg=peer_x)
        stop = request.post("/api/input?" + urlsplit(links[1]["url"]).query, data={"kind": "stop"})
        assert stop.ok, stop.text()
        initial = actor.locator("#skill").input_value()
        actor.evaluate("""({initial, replace}) => {
            const cast = document.querySelector('#cast');
            const skill = document.querySelector('#skill');
            const cancel = document.querySelector('#cancel');
            const extract = document.querySelector('#extract');
            const probe = window.__followupProbe = {release: null, firstSent: false, secondSent: false, done: false};
            const select = id => {
                skill.value = id;
                skill.dispatchEvent(new Event('change', {bubbles: true}));
            };
            // React to the same authoritative window without an extra browser-driver round trip.
            const observer = new MutationObserver(() => {
                const followup = window.__lastSnapshot?.self.followup;
                if (!followup || followup.source_skill_id !== initial) return;
                probe.release ??= followup.promotes_at_ms;
                if (followup.promotes_at_ms !== probe.release) return;
                const queued = followup.queued?.skill_id ?? null;
                if (!probe.firstSent) {
                    if (skill.value !== 'followup-first') select('followup-first');
                    if (!cast.disabled && cast.textContent === '排队跟进') {
                        probe.firstSent = true;
                        probe.firstAt = window.__lastSnapshot.logical_ms;
                        cast.click();
                    }
                } else if (!probe.secondSent && queued === 'followup-first') {
                    probe.extractDisabled = extract.disabled;
                    probe.secondSent = true;
                    probe.secondAt = window.__lastSnapshot.logical_ms;
                    if (replace) {
                        select('followup-latest');
                        if (!cast.disabled && cast.textContent === '排队跟进') cast.click();
                    } else if (!cancel.disabled && cancel.textContent === '取消跟进') {
                        cancel.click();
                    }
                } else if (probe.secondSent && queued === (replace ? 'followup-latest' : null)) {
                    probe.done = true;
                    observer.disconnect();
                }
            });
            observer.observe(document.querySelector('#own'), {childList: true, subtree: true});
        }""", {"initial": initial, "replace": scenario == "four"})
        pick_direction(actor, 80, 40)
        actor.locator("#cast").click()
        try:
            actor.wait_for_function("window.__followupProbe.done", timeout=10000)
        except Exception:
            print(scenario, actor.evaluate("window.__followupProbe"), actor.locator("#problem").inner_text(), flush=True)
            raise
        probe = actor.evaluate("window.__followupProbe")
        assert probe["extractDisabled"]
        if scenario == "four":
            actor.wait_for_function("window.__hpCostStats.bySkill['followup-latest']?.total === 2")
            outcome, latest, hp_cost, commits = "replaced", "followup-latest", 5, 2
        else:
            actor.wait_for_function("end => window.__lastSnapshot.logical_ms >= end", arg=probe["release"] + 300)
            outcome, latest, hp_cost, commits = "cancelled", None, 3, 1
        assert peer.evaluate("window.__lastSnapshot.self.position[0]") > peer_x
        assert peer.evaluate("window.__lastSnapshot.self.followup ?? null") is None
        costs = actor.evaluate("window.__hpCostStats")
        assert costs["total"] == hp_cost and costs["commits"] == commits, (scenario, costs, hp_cost, commits)
        assert "followup-first" not in costs["bySkill"]
        assert actor.locator("#cancel").inner_text() != "取消跟进"
        actor.screenshot(path=str(output / f"{scenario}-{outcome}.png"), full_page=True)
        for page in pages:
            page.locator("#leave").click()
        deadline = time.monotonic() + 15
        while True:
            ledgers = [request.get("/api/ledger?" + urlsplit(link["url"]).query).json() for link in links]
            if all(row["phase"] == "closed" and not row["host_live"] for row in ledgers):
                break
            assert time.monotonic() < deadline
            actor.wait_for_timeout(40)
        assert all(row["ledger"]["admissions"] == row["ledger"]["settlements"] == 1 for row in ledgers)
        assert not errors and all(not row["errors"] for row in ledgers)
        return {"scenario": scenario, "pages": seats, "outcome": outcome, "followupSkill": latest,
                "hpCost": hp_cost, "commits": commits, "peerContinued": True, "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("--port", type=int, default=4238)
    args = parser.parse_args()
    output = args.out
    output.mkdir(parents=True, exist_ok=True)
    address = f"http://127.0.0.1:{args.port}"
    command = [
        "node", str(args.server), "--binary", str(args.binary),
        "--configs", str(args.configs / "catalog.json"), "--host-config", str(args.configs / "host.json"),
        "--database", str(output / "rooms.sqlite"), "--port", str(args.port),
    ]
    with (output / "server.log").open("w") as log:
        server = subprocess.Popen(command, cwd=args.cwd, stdout=log, stderr=subprocess.STDOUT)
        try:
            with sync_playwright() as playwright:
                request = playwright.request.new_context(base_url=address)
                deadline = time.monotonic() + 15
                while True:
                    try:
                        response = request.get("/api/config", timeout=1000)
                    except Exception:
                        response = None
                    if response and response.ok:
                        break
                    assert server.poll() is None and time.monotonic() < deadline
                    time.sleep(0.1)
                assert response.json()["sdk"]["skillFollowup"]["minimumMechanismVersion"] == 51
                browser = playwright.chromium.launch()
                cases = [journey(browser, request, address, output, scenario, seats)
                         for scenario, seats in [("four", 4), ("eight", 8)]]
                browser.close()
                request.dispose()
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
    report = {"passed": True, "binarySha256": hashlib.sha256(args.binary.read_bytes()).hexdigest(), "cases": cases}
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    print(json.dumps(report))


if __name__ == "__main__":
    main()
