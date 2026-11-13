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
    item = "pill_healing"
    held = "(window.__lastSnapshot.self.inventory.find(i => i.item_id === 'pill_healing')?.quantity ?? 0)"
    cooldown = "window.__lastSnapshot.self.item_cooldowns?.find(i => i.item_id === 'pill_healing')?.ready_at_ms"
    transferred = seats == 8
    donor, peer = pages[:2]
    actor = peer if transferred else donor

    def spend_hp(page):
        page.locator("#selfCast").click()
        page.wait_for_function("window.__lastSnapshot.self.hp === window.__lastSnapshot.self.max_hp - 60")

    try:
        for page, link in zip(pages, links):
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.on("console", lambda message: errors.append(message.text) if message.type == "error" else None)
            page.goto(address + link["url"], wait_until="domcontentloaded")
        for page in pages:
            page.wait_for_function("window.__lastSnapshot?.phase === 'active' && window.__lastSnapshot.self.ready")
        donor.get_by_role("button", name="普攻", exact=True).click()
        donor.wait_for_function("window.__lastSnapshot.resources.some(r => r.item_id === 'pill_healing')")
        donor.locator("#pickup").click()
        donor.wait_for_function(f"{held} === 3")
        spend_hp(donor)
        donor.locator("#useItemChoice").select_option(item)
        donor.locator("#useItem").click()
        donor.wait_for_function(f"{held} === 2 && !!({cooldown}) && window.__lastItemUsed?.hp_restored === 40")
        assert donor.locator("#useItem").is_disabled()
        assert donor.locator("#itemCooldown").inner_text().startswith("冷却 ")
        deadline = donor.evaluate(cooldown)
        assert peer.evaluate(f"({cooldown}) ?? null") is None
        if transferred:
            donor.locator("#dropItem").select_option(item)
            donor.locator("#dropQuantity").fill("1")
            donor.locator("#dropLoot").click()
            peer.wait_for_function("window.__lastSnapshot.resources.some(r => r.id.startsWith('player-drop-'))")
            peer.locator("#pickup").click()
            peer.wait_for_function(f"{held} === 1")
            donor.wait_for_function(f"{held} === 1")
            spend_hp(peer)
        else:
            donor.wait_for_function(f"end => window.__lastSnapshot.logical_ms >= end && !({cooldown})", arg=deadline)
        count = actor.evaluate(held)
        revision = actor.evaluate("window.__lastSnapshot.self.inventory_revision")
        actor.locator("#useItemChoice").select_option(item)
        assert actor.locator("#useItem").is_enabled()
        actor.locator("#useItem").click()
        restored = 40 if transferred else 20
        actor.wait_for_function(f"{held} === {count - 1} && ({cooldown}) > {deadline} && window.__lastItemUsed?.hp_restored === {restored}")
        assert actor.evaluate("window.__lastSnapshot.self.inventory_revision") == revision + 1
        assert (actor.evaluate("window.__lastSnapshot.logical_ms") < deadline) == transferred
        assert all(page.evaluate("window.__lastItemUsed ?? null") is None for page in pages[2:])
        actor.screenshot(path=str(output / f"{scenario}-cooldown.png"), full_page=True)

        for page in pages:
            page.locator("#leave").click()
        limit = time.monotonic() + 15
        while True:
            ledgers = [request.get("/api/ledger?" + urlsplit(link["url"]).query).json() for link in links]
            if all(row["phase"] == "closed" and not row["host_live"] for row in ledgers):
                break
            assert time.monotonic() < limit
            donor.wait_for_timeout(40)
        assert all(row["ledger"]["admissions"] == row["ledger"]["settlements"] == 1 for row in ledgers)
        credits = [row["ledger"]["balances"].get(item, 0) for row in ledgers]
        assert credits == [1] + [0] * (seats - 1)
        assert not errors and all(not row["errors"] for row in ledgers)
        return {"scenario": scenario, "pages": seats,
                "usedBy": [links[0]["participant_id"], links[1 if transferred else 0]["participant_id"]],
                "transferred": transferred, "earlyDisabled": True, "restored": [40, restored],
                "consumed": 2, "remaining": sum(credits), "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("--port", type=int, default=4262)
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
                assert response.json()["sdk"]["memberItemCooldown"]["minimumMechanismVersion"] == 59
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
