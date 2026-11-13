"""Exercise authoritative item use and settlement in the delivered H5."""
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
    item = "pill_healing"
    held = "(window.__lastSnapshot.self.inventory.find(i => i.item_id === 'pill_healing')?.quantity ?? 0)"
    transferred = seats == 8
    donor, peer = pages[:2]
    actor = peer if transferred else donor

    def console(message, page):
        if message.type != "error":
            return
        if (page == actor and "/api/input?" in message.location.get("url", "") and
                "status of 409" in message.text):
            expected_http_errors.append(message.text)
        else:
            errors.append(message.text)

    def item_input(response):
        return ("/api/input?" in response.url and response.request.method == "POST" and
                response.request.post_data_json.get("kind") == "use_item")

    try:
        for page, link in zip(pages, links):
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.on("console", lambda message, tab=page: console(message, tab))
            page.goto(address + link["url"], wait_until="domcontentloaded")
        for page in pages:
            page.wait_for_function("window.__lastSnapshot?.phase === 'active' && window.__lastSnapshot.self.ready")
        donor.get_by_role("button", name="普攻", exact=True).click()
        donor.wait_for_function("window.__lastSnapshot.resources.some(r => r.item_id === 'pill_healing')")
        donor.locator("#pickup").click()
        donor.wait_for_function(f"{held} === 3")
        if transferred:
            donor.locator("#dropItem").select_option(item)
            donor.locator("#dropQuantity").fill("1")
            donor.locator("#dropLoot").click()
            peer.wait_for_function("window.__lastSnapshot.resources.some(r => r.id.startsWith('player-drop-'))")
            peer.locator("#pickup").click()
            peer.wait_for_function(f"{held} === 1")
            donor.wait_for_function(f"{held} === 2")

        maximum = actor.evaluate("window.__lastSnapshot.self.max_hp")
        revision = actor.evaluate("window.__lastSnapshot.self.inventory_revision")
        count = actor.evaluate(held)
        actor.locator("#useItemChoice").select_option(item)
        with actor.expect_response(item_input) as rejected:
            actor.locator("#useItem").click()
        assert rejected.value.status == 409 and rejected.value.json()["error"] == "INVALID_STATE"
        assert actor.evaluate(held) == count and actor.evaluate("window.__lastSnapshot.self.hp") == maximum
        assert actor.evaluate("window.__lastSnapshot.self.inventory_revision") == revision
        actor.locator("#selfCast").click()
        actor.wait_for_function("window.__lastSnapshot.self.hp === window.__lastSnapshot.self.max_hp - 60")
        with actor.expect_response(item_input) as used:
            actor.locator("#useItem").click()
        assert used.value.ok, used.value.text()
        actor.wait_for_function(f"{held} === {count - 1} && window.__lastSnapshot.self.hp === window.__lastSnapshot.self.max_hp - 20")
        actor.wait_for_function("window.__lastItemUsed?.hp_restored === 40")
        assert actor.evaluate("window.__lastItemUsed.item_id") == item
        assert actor.evaluate("window.__lastSnapshot.self.inventory_revision") == revision + 1
        assert all(page.evaluate("window.__lastItemUsed ?? null") is None for page in pages if page != actor)
        assert actor.locator("#lastItemUse").inner_text() == "恢复生命 40"
        actor.screenshot(path=str(output / f"{scenario}-item-used.png"), full_page=True)

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
        credits = [row["ledger"]["balances"].get(item, 0) for row in ledgers]
        assert credits == [2] + [0] * (seats - 1)
        assert len(expected_http_errors) <= 1 and not errors and all(not row["errors"] for row in ledgers)
        return {"scenario": scenario, "pages": seats, "usedBy": links[1 if transferred else 0]["participant_id"],
                "transferred": transferred, "fullHpRejected": True, "restored": 40, "consumed": 1,
                "remaining": sum(credits), "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("--port", type=int, default=4259)
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
                assert response.json()["sdk"]["memberItemUse"]["minimumMechanismVersion"] == 58
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
