"""Exercise authoritative projectile recast and settlement in the delivered H5."""
import argparse
import hashlib
import json
import math
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
        pick_direction(actor, 80, 40)
        actor.locator("#cast").click()
        actor.wait_for_function("!!window.__lastSnapshot?.self.projectile_recast")
        window = actor.evaluate("window.__lastSnapshot.self.projectile_recast")
        assert actor.locator("#recast").is_visible() and actor.locator("#recast").is_disabled()
        move = request.post("/api/input?" + urlsplit(links[1]["url"]).query,
                            data={"kind": "move", "direction": [1, 0]})
        assert move.ok, move.text()
        if scenario == "four":
            pick_direction(actor, -80, 40)
            actor.locator("#recast").click()
            actor.wait_for_function("""
                !!window.__lastProjectileRecast && !window.__lastSnapshot.self.projectile_recast
            """)
            fact = actor.evaluate("window.__lastProjectileRecast")
            assert fact["projectile_id"] == window["projectile_id"] and fact["action_id"] == window["action_id"]
            assert fact["spirit_cost"] == 5 and fact["recast_index"] == 1
            assert fact["direction"][1] > 0.9 and abs(math.hypot(*fact["direction"]) - 1) < 1e-9
            actor.wait_for_function("document.querySelector('#own').textContent.includes('弹道已转向')")
            pos = actor.evaluate("id => window.__lastSnapshot.projectiles.find(shot => shot.id === id).position",
                                 window["projectile_id"])
            actor.wait_for_function(
                "([id,y]) => window.__lastSnapshot.projectiles.some(shot => shot.id === id && shot.position[1] > y)",
                arg=[window["projectile_id"], pos[1]])
            outcome, fee = "redirected", 5
        else:
            actor.wait_for_function(
                "end => window.__lastSnapshot.logical_ms >= end && !window.__lastSnapshot.self.projectile_recast",
                arg=window["expires_at_ms"])
            assert actor.evaluate("window.__lastProjectileRecast ?? null") is None
            outcome, fee = "expired", 0
        assert actor.locator("#recast").is_hidden()
        assert peer.evaluate("window.__lastSnapshot.self.position[0]") > peer_x
        assert peer.evaluate("window.__lastProjectileRecast ?? null") is None
        costs = actor.evaluate("window.__hpCostStats")
        assert costs["total"] == 3 and costs["commits"] == 1
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
        return {"scenario": scenario, "pages": seats, "outcome": outcome,
                "projectileId": window["projectile_id"], "redirectSpiritCost": fee, "hpCost": 3,
                "peerContinued": True, "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("--port", type=int, default=4235)
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
                assert response.json()["sdk"]["projectileRecast"]["minimumMechanismVersion"] == 50
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
