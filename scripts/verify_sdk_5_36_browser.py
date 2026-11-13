"""Exercise authoritative basic followup and settlement in the delivered H5."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
from urllib.parse import urlsplit

from playwright.sync_api import sync_playwright


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
        peer_input = "/api/input?" + urlsplit(links[1]["url"]).query
        move = request.post(peer_input, data={"kind": "move", "direction": [1, 0]})
        assert move.ok, move.text()
        peer.wait_for_function("x => window.__lastSnapshot.self.position[0] > x", arg=peer_x)
        assert request.post(peer_input, data={"kind": "stop"}).ok
        actor.get_by_role("button", name="普攻", exact=True).click()
        actor.wait_for_function("window.__basicCommits === 1 && window.__lastSnapshot.self.basic_ready_at_ms > 0")
        actor.get_by_role("button", name="排队普攻", exact=True).click()
        actor.wait_for_function("!!window.__lastSnapshot.self.basic_followup")
        queued = actor.evaluate("window.__lastSnapshot.self.basic_followup")
        assert queued["target"]["generation"] >= 1
        assert actor.locator("#extract").is_disabled()
        if scenario == "four":
            actor.wait_for_function("window.__basicCommits === 2 && !window.__lastSnapshot.self.basic_followup")
            outcome, commits = "committed", 2
        else:
            actor.get_by_role("button", name="取消排队普攻").click()
            actor.wait_for_function("end => window.__lastSnapshot.logical_ms >= end && !window.__lastSnapshot.self.basic_followup",
                                    arg=queued["expires_at_ms"] + 150)
            assert actor.evaluate("window.__basicCommits") == 1
            outcome, commits = "cancelled", 1
        assert peer.evaluate("window.__lastSnapshot.self.basic_followup ?? null") is None
        costs = actor.evaluate("window.__hpCostStats")
        assert costs["total"] == 0 and costs["commits"] == 0
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
        return {"scenario": scenario, "pages": seats, "outcome": outcome, "hpCost": 0,
                "commits": commits, "peerContinued": True, "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("--port", type=int, default=4244)
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
                assert response.json()["sdk"]["basicFollowup"]["minimumMechanismVersion"] == 53
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
