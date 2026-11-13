"""Exercise authoritative cancelable recovery and settlement in the delivered H5."""
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
        peer_input = "/api/input?" + urlsplit(links[1]["url"]).query
        move = request.post(peer_input, data={"kind": "move", "direction": [1, 0]})
        assert move.ok, move.text()
        peer.wait_for_function("x => window.__lastSnapshot.self.position[0] > x", arg=peer_x)
        assert request.post(peer_input, data={"kind": "stop"}).ok
        pick_direction(actor, 80, 40)
        actor.locator("#cast").click()
        actor.wait_for_function("!!window.__lastSnapshot?.self.recovery_cancel")
        opening = actor.evaluate("window.__lastSnapshot.self")
        original_end = opening["recovery_until_ms"]
        assert original_end - opening["recovery_cancel"]["cancel_at_ms"] == 1800
        if scenario == "four":
            actor.locator("#skill").select_option("next-bolt")
            actor.get_by_role("button", name="接管施法").click()
            actor.wait_for_function("window.__lastSnapshot.self.pending_action?.skill_id === 'next-bolt'")
            accepted = actor.evaluate("window.__lastSnapshot.self")
            assert accepted["pending_action"]["commits_at_ms"] == accepted["recovery_until_ms"] + 300
            actor.wait_for_function("window.__hpCostStats.bySkill['next-bolt']?.total === 2")
            outcome, replacement, hp_cost, commits = "replaced", "next-bolt", 5, 2
        else:
            own_input = "/api/input?" + urlsplit(links[0]["url"]).query
            assert request.post(own_input, data={"kind": "move", "direction": [1, 0]}).ok
            actor.get_by_role("button", name="取消后摇").click()
            actor.wait_for_function("!window.__lastSnapshot.self.recovery_cancel")
            accepted = actor.evaluate("window.__lastSnapshot.self")
            actor.wait_for_function("x => window.__lastSnapshot.self.position[0] > x",
                                    arg=opening["position"][0])
            assert request.post(own_input, data={"kind": "stop"}).ok
            outcome, replacement, hp_cost, commits = "cancelled", None, 3, 1
        earlier = original_end - accepted["recovery_until_ms"]
        assert 0 < earlier <= 1800 and not accepted.get("recovery_cancel")
        assert peer.evaluate("window.__lastSnapshot.self.recovery_cancel ?? null") is None
        costs = actor.evaluate("window.__hpCostStats")
        assert costs["total"] == hp_cost and costs["commits"] == commits
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
        return {"scenario": scenario, "pages": seats, "outcome": outcome, "replacementSkill": replacement,
                "hpCost": hp_cost, "commits": commits, "earlierByMs": earlier, "peerContinued": True,
                "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("--port", type=int, default=4241)
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
                assert response.json()["sdk"]["recoveryCancel"]["minimumMechanismVersion"] == 52
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
