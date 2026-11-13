"""Drive the delivered H5 through its normal page and HTTP entry points."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time
from urllib.parse import urlsplit

from playwright.sync_api import sync_playwright


def verify(server_path, binary, config_dir, output, port, cwd):
    output.mkdir(parents=True, exist_ok=True)
    address = f"http://127.0.0.1:{port}"
    command = [
        "node", str(server_path), "--binary", str(binary),
        "--configs", str(config_dir / "catalog.json"),
        "--host-config", str(config_dir / "host.json"),
        "--database", str(output / "rooms.sqlite"), "--port", str(port),
    ]
    cases = []
    with (output / "server.log").open("w") as log:
        server = subprocess.Popen(command, cwd=cwd, stdout=log, stderr=subprocess.STDOUT)
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
                assert response.json()["sdk"]["castMovement"]["minimumMechanismVersion"] == 46
                browser = playwright.chromium.launch()
                for scenario, seats in [("four", 4), ("eight", 8)]:
                    cases.append(room_journey(browser, request, address, output, scenario, seats))
                browser.close()
                request.dispose()
        finally:
            server.terminate()
            try:
                server.wait(timeout=10)
            except subprocess.TimeoutExpired:
                server.kill()
                server.wait()
    report = {"passed": True, "binarySha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "cases": cases}
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def room_journey(browser, request, address, output, scenario, seats):
    response = request.post("/api/rooms", data={"scenario": scenario})
    assert response.status == 201, response.text()
    links = response.json()["links"]
    assert len(links) == seats
    contexts = [browser.new_context(viewport={"width": 1100, "height": 950}) for _ in links]
    pages = [context.new_page() for context in contexts]
    errors = []
    try:
        for page, link in zip(pages, links):
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.on("console", lambda message: errors.append(message.text) if message.type == "error" else None)
            page.add_init_script("""
                window.movementSamples = [];
                setInterval(() => {
                    const snapshot = window.__lastSnapshot;
                    if (snapshot && snapshot.logical_ms > (window.movementSamples.at(-1)?.time ?? -1)) {
                        window.movementSamples.push({
                            time: snapshot.logical_ms, position: snapshot.self.position,
                            pending: snapshot.self.pending_action !== null,
                            recovery: snapshot.self.recovery_until_ms
                        });
                    }
                }, 15);
            """)
            page.goto(address + link["url"], wait_until="domcontentloaded")
        for page in pages:
            page.wait_for_function("window.__lastSnapshot?.phase === 'active' && window.__lastSnapshot.self.ready")
        actor, peer = pages[:2]
        peer_start = peer.evaluate("window.__lastSnapshot.self.position[0]")
        actor.locator("#cast").click()
        actor.wait_for_function("window.__lastSnapshot?.self.pending_action !== null && !!window.__lastSnapshot")
        for link in links[:2]:
            query = urlsplit(link["url"]).query
            move = request.post("/api/input?" + query, data={"kind": "move", "direction": [1, 0]})
            assert move.ok, move.text()
        actor.wait_for_function("""
            window.__hpCostStats?.commits === 1 &&
            window.__lastSnapshot.self.pending_action === null &&
            window.__lastSnapshot.self.recovery_until_ms > 0 &&
            window.__lastSnapshot.logical_ms >= window.__lastSnapshot.self.recovery_until_ms + 180
        """)
        samples = actor.evaluate("window.movementSamples")
        locked = [sample for sample in samples if sample["pending"]] if scenario == "four" else [
            sample for sample in samples if not sample["pending"] and sample["recovery"] > sample["time"]
        ]
        assert len(locked) >= 2 and all(sample["position"] == locked[0]["position"] for sample in locked)
        assert actor.evaluate("window.__lastSnapshot.self.position[0]") > locked[-1]["position"][0]
        assert peer.evaluate("window.__lastSnapshot.self.position[0]") > peer_start
        assert actor.evaluate("window.__hpCostStats.total") == 3
        assert actor.locator("#map").is_visible()
        actor.screenshot(path=str(output / f"{scenario}-active.png"), full_page=True)
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
        return {"scenario": scenario, "pages": seats, "lockedSamples": len(locked),
                "hpCost": 3, "peerContinued": True, "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    for argument in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + argument, type=Path, required=True)
    parser.add_argument("--port", type=int, default=4223)
    args = parser.parse_args()
    print(json.dumps(verify(args.server.resolve(), args.binary.resolve(), args.configs.resolve(),
                            args.out.resolve(), args.port, args.cwd.resolve())))
