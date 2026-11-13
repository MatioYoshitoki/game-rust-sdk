"""Exercise charge controls and settlement in the delivered H5."""
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
        assert actor.locator("#cast").inner_text() == "开始蓄力"
        peer_x = peer.evaluate("window.__lastSnapshot.self.position[0]")
        actor.locator("#cast").click()
        actor.wait_for_function("!!window.__lastSnapshot?.self.pending_action?.charge")
        charge = actor.evaluate("window.__lastSnapshot.self.pending_action.charge")
        assert actor.locator("#releaseCharge").is_visible()
        assert peer.evaluate("window.__lastSnapshot.self.pending_action") is None
        move = request.post("/api/input?" + urlsplit(links[1]["url"]).query,
                            data={"kind": "move", "direction": [1, 0]})
        assert move.ok, move.text()
        if scenario == "eight":
            actor.screenshot(path=str(output / f"{scenario}-charging.png"), full_page=True)
        if scenario == "four":
            actor.wait_for_function("""
                !!window.__lastSnapshot.self.pending_action?.charge &&
                window.__lastSnapshot.logical_ms >= window.__lastSnapshot.self.pending_action.charge.min_release_at_ms
            """)
            actor.locator("#releaseCharge").click()
        actor.wait_for_function("""
            !!window.__lastChargeCommit && window.__lastSnapshot.self.pending_action === null &&
            window.__lastSnapshot.self.recovery_until_ms > 0
        """)
        committed = actor.evaluate("window.__lastChargeCommit")
        assert committed["action_id"] == charge["action_id"] and committed["hp_cost"] == 3
        held = committed["charge"]["held_ms"]
        factor = committed["charge"]["damage_factor_bps"]
        if scenario == "four":
            assert 450 <= held < 1500 and 4000 <= factor < 10000
        else:
            assert held == 1800 and factor == 10000
        assert peer.evaluate("window.__lastSnapshot.self.position[0]") > peer_x
        assert peer.evaluate("window.__lastChargeCommit ?? null") is None
        assert actor.locator("#releaseCharge").is_hidden()
        actor.wait_for_function("document.querySelector('#own').textContent.includes('上次蓄力')")
        actor.screenshot(path=str(output / f"{scenario}-committed.png"), full_page=True)
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
        return {"scenario": scenario, "pages": seats, "heldMs": held, "factorBps": factor,
                "hpCost": 3, "peerContinued": True, "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("--port", type=int, default=4226)
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
                assert response.json()["sdk"]["directionCharge"]["minimumMechanismVersion"] == 47
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
