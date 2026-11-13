"""Exercise authoritative loot transfer and settlement in the delivered H5."""
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
    expected_http_errors = []
    item = "xue-zhi"
    held = "(window.__lastSnapshot.self.inventory.find(i => i.item_id === 'xue-zhi')?.quantity ?? 0)"
    try:
        def console(message, page):
            if message.type != "error":
                return
            if (scenario == "eight" and page in pages[1:3] and
                    "/api/input?" in message.location.get("url", "") and "status of 409" in message.text):
                expected_http_errors.append(message.text)
            else:
                errors.append(message.text)

        for page, link in zip(pages, links):
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.on("console", lambda message, tab=page: console(message, tab))
            page.goto(address + link["url"], wait_until="domcontentloaded")
        for page in pages:
            page.wait_for_function("window.__lastSnapshot?.phase === 'active' && window.__lastSnapshot.self.ready")
        donor = pages[0]
        donor.get_by_role("button", name="普攻", exact=True).click()
        donor.wait_for_function("window.__lastSnapshot.resources.some(r => r.item_id === 'xue-zhi')")
        donor.locator("#pickup").click()
        donor.wait_for_function(f"{held} === 3")
        donor.locator("#dropItem").select_option(item)
        donor.locator("#dropQuantity").fill("2")
        with donor.expect_response(lambda response: "/api/input?" in response.url
                                   and response.request.post_data_json.get("kind") == "drop_item") as result:
            donor.locator("#dropLoot").click()
        assert result.value.ok, result.value.text()
        donor.wait_for_function(f"{held} === 1 && window.__lastSnapshot.resources.some(r => r.id.startsWith('player-drop-'))")
        resource = donor.evaluate("window.__lastSnapshot.resources.find(r => r.id.startsWith('player-drop-'))")
        assert resource["quantity"] == 2 and resource["position"] == donor.evaluate("window.__lastSnapshot.self.position")
        for page in pages:
            page.wait_for_function("id => window.__lastSnapshot.resources.some(r => r.id === id)", arg=resource["id"])

        rivals = pages[1:3] if seats == 8 else pages[1:2]
        pending = []
        responses = []

        def is_claim(http_request):
            return http_request.method == "POST" and (http_request.post_data_json or {}).get("resource_id") == resource["id"]

        def hold(route):
            if not is_claim(route.request):
                route.continue_()
                return
            pending.append(route)
            if len(pending) == len(rivals):
                for claim in pending:
                    claim.continue_()

        for page in rivals:
            page.route("**/api/input?*", hold)
            page.on("response", lambda response: responses.append(response) if is_claim(response.request) else None)
        for page in rivals:
            page.locator("#pickup").click()
        deadline = time.monotonic() + 15
        while len(responses) < len(rivals):
            assert time.monotonic() < deadline, "missing pickup response"
            donor.wait_for_timeout(40)
        assert len(pending) == len(rivals)
        assert sorted(response.status for response in responses) == ([200, 409] if seats == 8 else [200])
        assert all(response.json()["error"] == "INVALID_TARGET" for response in responses if not response.ok)
        for page in pages:
            page.wait_for_function("id => window.__lastSnapshot.resources.every(r => r.id !== id)", arg=resource["id"])
        credits = [page.evaluate(held) for page in pages]
        assert credits[0] == 1 and sorted(credits[1:]) == [0] * (seats - 2) + [2]
        winner = links[credits.index(2)]["participant_id"]
        donor.screenshot(path=str(output / f"{scenario}-transferred.png"), full_page=True)
        for page in pages:
            page.locator("#leave").click()
        deadline = time.monotonic() + 15
        while True:
            ledgers = [request.get("/api/ledger?" + urlsplit(link["url"]).query).json() for link in links]
            if all(row["phase"] == "closed" and not row["host_live"] for row in ledgers):
                break
            assert time.monotonic() < deadline
            donor.wait_for_timeout(40)
        assert all(row["ledger"]["admissions"] == row["ledger"]["settlements"] == 1 for row in ledgers)
        assert [row["ledger"]["balances"].get(item, 0) for row in ledgers] == credits
        assert len(expected_http_errors) <= int(seats == 8)
        assert not errors and all(not row["errors"] for row in ledgers)
        return {"scenario": scenario, "pages": seats, "drop": resource["id"], "retained": 1, "received": 2,
                "winner": winner, "failedClaims": int(seats == 8), "total": 3, "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("--port", type=int, default=4256)
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
                assert response.json()["sdk"]["memberLootDrop"]["minimumMechanismVersion"] == 57
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
