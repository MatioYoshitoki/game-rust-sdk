"""Exercise authoritative fixed channel and settlement in the delivered H5."""
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
        assert actor.locator("#cast").inner_text() == "开始引导"
        peer_x = peer.evaluate("window.__lastSnapshot.self.position[0]")
        pick_direction(actor, 80, 40)
        actor.locator("#cast").click()
        actor.wait_for_function("!!window.__lastSnapshot?.self.active_channel")
        own = actor.evaluate("window.__lastSnapshot.self")
        channel = own["active_channel"]
        assert math.dist(channel["direction"], [1, 0]) < 0.025
        for link in links[:2]:
            move = request.post("/api/input?" + urlsplit(link["url"]).query,
                                data={"kind": "move", "direction": [1, 0]})
            assert move.ok, move.text()
        actor.wait_for_function("""
            window.__lastChannelPulse?.pulse >= 1 && window.__lastSnapshot.self.active_channel?.pulses >= 1
        """)
        assert actor.evaluate("window.__lastSnapshot.self.position") == own["position"]
        assert actor.locator("#cancel").inner_text() == "取消引导"
        assert actor.locator("#cast").is_disabled() and actor.locator("#extract").is_disabled()
        if scenario == "eight":
            actor.locator("#cancel").click()
        actor.wait_for_function("""
            !!window.__lastChannelEnd && !window.__lastSnapshot.self.active_channel &&
            window.__lastSnapshot.self.recovery_until_ms > 0
        """)
        end = actor.evaluate("window.__lastChannelEnd")
        pulse = actor.evaluate("window.__lastChannelPulse")
        costs = actor.evaluate("window.__hpCostStats")
        assert end["action_id"] == pulse["action_id"] == channel["action_id"]
        assert costs["total"] == 3 and costs["commits"] == 1
        pulses, per_pulse, reason, label = (3, 5, "completed", "自然结束") if scenario == "four" else (
            1, 7, "cancelled", "主动取消")
        assert pulse["pulse"] == pulses and pulse["spirit_cost"] == per_pulse and end["reason"] == reason
        assert peer.evaluate("window.__lastSnapshot.self.position[0]") > peer_x
        assert peer.evaluate("window.__lastChannelPulse ?? null") is None
        assert actor.locator("#cancel").inner_text() == "取消准备"
        actor.wait_for_function(
            "text => document.querySelector('#own').textContent.includes(text)", arg="上次引导 " + label)
        assert f"引导脉冲 {pulses} · 周期灵力 {pulses * per_pulse}" in actor.locator("#own").inner_text()
        actor.screenshot(path=str(output / f"{scenario}-ended.png"), full_page=True)
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
        return {"scenario": scenario, "pages": seats, "pulses": pulses, "endReason": reason,
                "pulseSpiritCost": pulses * per_pulse, "hpCost": 3,
                "locked": True, "peerContinued": True, "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("--port", type=int, default=4232)
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
                assert response.json()["sdk"]["fixedChannel"]["minimumMechanismVersion"] == 49
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
