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
    contexts = [browser.new_context(viewport={"width": 1100, "height": 950}) for _ in links]
    pages = [context.new_page() for context in contexts]
    errors = []
    health, spirit = "pill_healing", "pill_ningqi"
    transferred = seats == 8
    donor, peer = pages[:2]
    actor = peer if transferred else donor
    cost = 6 if transferred else 8

    def held(item):
        return f"(window.__lastSnapshot.self.inventory.find(i => i.item_id === '{item}')?.quantity ?? 0)"

    def cooldown(item):
        return f"window.__lastSnapshot.self.item_cooldowns?.find(i => i.item_id === '{item}')?.ready_at_ms"

    def spend(page):
        page.locator("#selfCast").click()
        page.wait_for_function(f"window.__lastSnapshot.self.spirit === window.__lastSnapshot.self.max_spirit - {cost}")

    try:
        assert len(pages) == seats
        for page, link in zip(pages, links):
            page.on("pageerror", lambda error: errors.append(str(error)))
            page.on("console", lambda message: errors.append(message.text) if message.type == "error" else None)
            page.goto(address + link["url"], wait_until="domcontentloaded")
        for page in pages:
            page.wait_for_function("window.__lastSnapshot?.phase === 'active' && window.__lastSnapshot.self.ready")
        donor.get_by_role("button", name="普攻", exact=True).click()
        donor.wait_for_function("window.__lastSnapshot.resources.some(r => r.item_id === 'pill_healing') && window.__lastSnapshot.resources.some(r => r.item_id === 'pill_ningqi')")
        for _ in range(4):
            if donor.evaluate(f"{held(health)} === 2 && {held(spirit)} === 2"):
                break
            revision = donor.evaluate("window.__lastSnapshot.self.inventory_revision")
            donor.locator("#pickup").click()
            donor.wait_for_function("rev => window.__lastSnapshot.self.inventory_revision > rev", arg=revision)
        assert donor.evaluate(f"{held(health)} === 2 && {held(spirit)} === 2")
        spend(donor)
        donor.locator("#useItemChoice").select_option(health)
        donor.locator("#useItem").click()
        donor.wait_for_function(f"{held(health)} === 1 && !!({cooldown(spirit)}) && window.__lastItemUsed?.hp_restored === 40")
        deadline = donor.evaluate(cooldown(spirit))
        assert donor.evaluate(cooldown(health)) == deadline
        donor.locator("#useItemChoice").select_option(spirit)
        assert donor.locator("#useItem").is_disabled()
        assert donor.locator("#itemCooldown").inner_text().startswith("冷却 ")
        assert peer.evaluate(f"({cooldown(spirit)}) ?? null") is None

        if transferred:
            donor.locator("#dropItem").select_option(spirit)
            donor.locator("#dropQuantity").fill("1")
            donor.locator("#dropLoot").click()
            peer.wait_for_function("window.__lastSnapshot.resources.some(r => r.id.startsWith('player-drop-'))")
            peer.locator("#pickup").click()
            peer.wait_for_function(f"{held(spirit)} === 1")
            donor.wait_for_function(f"{held(spirit)} === 1")
            spend(peer)
        else:
            donor.wait_for_function(f"end => window.__lastSnapshot.logical_ms >= end && !({cooldown(spirit)})", arg=deadline)

        revision = actor.evaluate("window.__lastSnapshot.self.inventory_revision")
        actor.locator("#useItemChoice").select_option(spirit)
        assert actor.locator("#useItem").is_enabled()
        actor.locator("#useItem").click()
        actor.wait_for_function(f"window.__lastItemUsed?.spirit_restored === {cost} && ({cooldown(spirit)}) > {deadline}")
        assert actor.evaluate("window.__lastSnapshot.self.inventory_revision") == revision + 1
        assert actor.evaluate(cooldown(health)) == actor.evaluate(cooldown(spirit))
        assert (actor.evaluate("window.__lastSnapshot.logical_ms") < deadline) == transferred
        assert all(page.evaluate("window.__lastItemUsed ?? null") is None for page in pages[2:])
        actor.screenshot(path=str(output / f"{scenario}-group-cooldown.png"), full_page=True)

        for page in pages:
            page.locator("#leave").click()
        limit = time.monotonic() + 15
        while True:
            # Playwright retries only ECONNRESET; ledger reads are safe to repeat.
            ledgers = [request.get("/api/ledger?" + urlsplit(link["url"]).query,
                                   max_retries=2, timeout=3000).json() for link in links]
            if all(row["phase"] == "closed" and not row["host_live"] for row in ledgers):
                break
            assert time.monotonic() < limit
            donor.wait_for_timeout(40)
        assert all(row["ledger"]["admissions"] == row["ledger"]["settlements"] == 1 for row in ledgers)
        credits = [sum(row["ledger"]["balances"].get(item, 0) for row in ledgers) for item in (health, spirit)]
        assert credits == [1, 1]
        assert not errors and all(not row["errors"] for row in ledgers)
        return {"scenario": scenario, "pages": seats,
                "usedBy": [links[0]["participant_id"], links[1 if transferred else 0]["participant_id"]],
                "transferred": transferred, "earlyDisabled": True, "restored": [40, cost],
                "consumed": 2, "remaining": credits, "settled": seats, "errors": errors}
    finally:
        for context in contexts:
            context.close()


def main():
    parser = argparse.ArgumentParser()
    for name in ("server", "binary", "configs", "out", "cwd"):
        parser.add_argument("--" + name, type=lambda value: Path(value).resolve(), required=True)
    parser.add_argument("--port", type=int, default=4268)
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
                assert response.json()["sdk"]["memberItemCooldownGroup"]["minimumMechanismVersion"] == 61
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
