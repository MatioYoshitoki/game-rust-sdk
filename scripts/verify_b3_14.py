"""Verify both forest routes through the public H5 and Rust APIs."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
import tempfile
import time

from playwright.sync_api import sync_playwright

from verify_b3_13 import wait
from verify_durability import Host
from verify_h5 import active, info
from verify_reconnect import hashes


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "artifacts/b3-14"
SUPPORT_ID = "sanctum-support-1"


def settlement(value):
    return next(r for r in value["io"]["requests"] if r["body"]["kind"] == "commit_settlement")


class ForestRun:
    def __init__(self, page, host, scenario="b3-14"):
        self.page = page
        self.host = host
        self.serial = 0
        response = page.request.get(host.url + f"/api/config?scenario={scenario}")
        assert response.ok, response.text()
        self.config = response.json()

    def current(self):
        return info(self.page, self.host.url)

    def snapshot(self):
        return self.current()["snapshot"]

    def encounter(self, encounter_id):
        return next(e for e in self.snapshot()["pve"]["encounters"] if e["id"] == encounter_id)

    def monster(self, monster_id):
        return next((m for m in self.snapshot()["pve"]["monsters"] if m["id"] == monster_id), None)

    def encounter_config(self, encounter_id):
        return next(e for e in self.config["pve"]["encounters"] if e["id"] == encounter_id)

    def request(self, kind, **args):
        self.serial += 1
        request_id = f"wetland-{self.serial}"
        for _ in range(4):
            value = self.current()
            response = self.page.request.post(
                self.host.url + f"/api/sessions/{value['id']}/input",
                data={
                    "connection": value["connection"],
                    "sequence": value["sequence"] + 1,
                    "requestId": request_id,
                    "action": {"kind": kind, **args},
                },
            )
            if response.ok or response.json().get("error") != "STALE_SEQUENCE":
                return request_id, response
        raise AssertionError(response.text())

    def send(self, kind, **args):
        request_id, response = self.request(kind, **args)
        assert response.ok, (kind, args, response.text())
        return request_id

    def begin(self, first_encounter):
        self.page.locator("#scenario").select_option("b3-14")
        self.page.locator("#clock").select_option("fast")
        self.page.locator("#start").click()
        active(self.page)
        button = "#navigate-required-encounter" if first_encounter == "ruins" else "#navigate-encounter"
        self.page.locator(button).click()
        self.stop_at_encounter(first_encounter)

    def stop_at_encounter(self, encounter_id):
        wait(lambda: self.encounter(encounter_id)["phase"] != "dormant")
        self.send("stop")

    def approach(self, encounter_id):
        self.send("navigate", destination=self.encounter_config(encounter_id)["trigger"]["center"])
        self.stop_at_encounter(encounter_id)

    def arrive(self, point, radius=10):
        self.send("navigate", destination=point)
        wait(lambda: math.dist(self.snapshot()["position"], point) < radius)
        self.send("stop")

    def strike_next(self, encounter_id):
        def living():
            snap = self.snapshot()
            actors = [m for m in snap["pve"]["monsters"] if m["encounter_id"] == encounter_id and m["hp"]]
            return min(actors, key=lambda m: math.dist(m["position"], snap["position"]), default=None)

        target = wait(living)
        # Reach a close firing position before attacking through the forest geometry.
        self.send("navigate", destination=target["position"])
        radius = min(self.config["basic"]["range"] * .8,
                     2 * (self.config["params"]["player_radius"] + target["radius"]))

        def near_target():
            actor = self.monster(target["id"])
            return not actor or not actor["hp"] or math.dist(self.snapshot()["position"], actor["position"]) < radius

        wait(near_target)
        self.send("stop")
        wait(lambda: (snap := self.snapshot())["logical_ms"] >= snap["basic_ready_at_ms"])
        request_id = self.send("attack")
        wait(lambda: any(e["kind"] == "action_committed" and e["action_id"] == request_id
                         for e in self.current()["events"]))
        self.page.wait_for_timeout(100)

    def finish(self, encounter_id):
        for _ in range(100):
            if self.encounter(encounter_id)["phase"] == "completed":
                return
            self.strike_next(encounter_id)
        raise AssertionError(f"Combat budget exceeded for {encounter_id}")

    def collect(self):
        for _ in range(100):
            snap = self.snapshot()
            if not snap["resources"]:
                return
            resource = min(snap["resources"], key=lambda r: math.dist(r["position"], snap["position"]))
            resource_id = resource["id"]
            pickup_range = self.config["params"]["pickup_range"]

            def exists(snapshot):
                return any(r["id"] == resource_id for r in snapshot["resources"])

            if math.dist(snap["position"], resource["position"]) > pickup_range * .8:
                self.send("navigate", destination=resource["position"])
                wait(lambda: not exists(s := self.snapshot())
                     or math.dist(s["position"], resource["position"]) < pickup_range * .8)
                self.send("stop")
            snap = self.snapshot()
            if exists(snap):
                _, response = self.request(
                    "pickup", resource_id=resource_id, expected_revision=snap["inventory_revision"],
                )
                assert response.ok or response.json().get("error") == "NO_RESOURCE", response.text()
                wait(lambda: not exists(self.snapshot()))
        raise AssertionError("Pickup budget exceeded")

    def retreat(self, encounter_id):
        encounter = self.encounter_config(encounter_id)
        center = encounter["trigger"]["center"]
        spawn = self.config["spawn"]
        radius = encounter["reset"]["pursuit_radius"] + 2 * self.config["pve"]["cell_size"]
        angle = math.atan2(spawn[1] - center[1], spawn[0] - center[0])
        old_round = self.encounter(encounter_id)["reset"]["round"]

        def reset_state():
            return self.encounter(encounter_id)["reset"]

        for step in range(8):
            direction = angle + step * math.pi / 4
            destination = [center[0] + radius * math.cos(direction), center[1] + radius * math.sin(direction)]
            _, response = self.request("navigate", destination=destination)
            if not response.ok:
                assert response.json().get("error") == "OUT_OF_RANGE", response.text()
                continue
            try:
                wait(lambda: (r := reset_state())["round"] > old_round or r["reset_at_ms"], timeout=3)
            except AssertionError:
                continue
            self.send("stop")
            wait(lambda: reset_state()["round"] == old_round + 1)
            return
        raise AssertionError(f"No reachable retreat from {encounter_id}")

    def observe_heal(self, weakened):
        support = wait(lambda: self.monster(SUPPORT_ID))
        config = self.config["pve"]["additional_templates"][support["template_id"]]["support"]
        variant = config["on_encounter_completed"] if weakened else config
        assert support["support"]["skill_id"] == variant["id"]
        self.page.wait_for_function(
            "(name) => document.querySelector('#support-status').textContent.includes(name)",
            arg=variant["name"],
        )
        previous = {e["cast_id"] for e in self.current()["events"] if e["kind"] == "support_pulse"}
        for _ in range(8):
            self.strike_next("sanctum")
            value = self.current()
            hp_limits = {m["id"]: m["max_hp"] for m in value["snapshot"]["pve"]["monsters"]}
            for event in value["events"]:
                if event["kind"] != "support_pulse" or event["cast_id"] in previous:
                    continue
                expected = hp_limits[event["target_id"]] * variant["heal_bps"] // 10000
                assert event["monster_id"] == SUPPORT_ID and 0 <= event["amount"] <= expected, event
                if event["amount"] == expected:
                    return {"skillId": variant["id"], "healBps": variant["heal_bps"], "pulse": event}
        raise AssertionError(f"No full-budget pulse from {variant['id']}")

    def enter_final_wave(self):
        self.approach("sanctum")
        while not self.monster(SUPPORT_ID):
            self.strike_next("sanctum")
            # The first wave is dead. Let the second wave start before the next strike.
            if self.encounter("sanctum")["phase"] == "waiting":
                wait(lambda: self.monster(SUPPORT_ID))

    def open_reward_and_extract(self):
        pve = self.config["pve"]
        objective = pve["map_objective"]
        cost = objective["cost"]
        assert dict(self.snapshot()["pve"]["inventory"]).get(cost["item_id"], 0) >= cost["quantity"]
        chest_id = objective["reward_chest_id"]

        def chest():
            return next(ch for ch in self.snapshot()["pve"]["chests"] if ch["id"] == chest_id)

        self.arrive(chest()["position"])
        self.send("attune_objective", objective_id=objective["id"],
                  expected_revision=self.snapshot()["inventory_revision"])
        wait(lambda: self.snapshot()["pve"]["map_objective"]["attuned"])
        generation = chest()["generation"]
        self.send("open_chest", chest_id=chest_id, expected_generation=generation)
        wait(lambda: (seal := chest()["seal"])["completed_layers"] == seal["layers"])
        self.send("open_chest", chest_id=chest_id, expected_generation=generation)
        wait(lambda: self.snapshot()["pve"]["map_objective"]["reward_opened"])
        self.collect()
        wait(lambda: self.snapshot()["extraction_open"])
        self.arrive(self.config["extractionPoint"])
        self.send("extract_start")

    def done(self):
        value = wait(lambda: v if (v := self.current())["phase"] == "closed"
                     and v["settlement"] and not v["ledger"]["pending"] else None, timeout=50)
        self.page.wait_for_function("document.querySelector('#world-live').textContent === '0'")
        return value

    def restart(self):
        self.page.goto("about:blank")
        self.host.kill()
        self.host.start()
        self.page.goto(self.host.url)
        return self.done()


def check_result(value, route, config):
    body = settlement(value)["body"]
    ledger = value["ledger"]
    progress = body["map_progress"]
    assert body["outcome"] == "extraction" and progress["attuned"] and progress["reward_opened"]
    assert progress["completed_encounter_ids"] == (
        ["ruins", "sanctum", "wetland"] if route == "weakened_retry" else ["ruins", "sanctum"]
    )
    assert progress["consumed"] == [config["pve"]["map_objective"]["cost"]]
    expected = {"ordinary": (5, 5, 180), "partial_wetland": (6, 6, 190), "weakened_retry": (8, 7, 200)}
    assert (body["reset_progress"]["actual_kills"], body["scoring"]["kills"],
            body["scoring"]["kill_score"]) == expected[route]
    consumed = {item["item_id"]: item["quantity"] for item in progress["consumed"]}
    assert body["carried"] + body["looted"] == body["kept"] + body["dropped"] + consumed.get(body["item_id"], 0)
    for item in body["items"]:
        assert item["looted"] == item["kept"] + item["dropped"] + consumed.get(item["item_id"], 0)
    assert ledger["items"] == {item["item_id"]: item["kept"] for item in body["items"]}
    assert ledger["items"]["misty-spirit-wood"] > 0
    assert ledger["warehouse"] == 100 - body["carried"] + body["kept"]
    assert ledger["cultivation"] == body["cultivation"]
    assert ledger["admissions"] == ledger["settlements"] == 1 and ledger["manual"] == 0
    assert sum(event["kind"] == "settlement_applied" for event in value["events"]) == 1


def run_route(run, route):
    weakened = route == "weakened_retry"
    run.begin("ruins" if route == "ordinary" else "wetland")
    if weakened:
        run.finish("wetland")
        run.collect()
        run.approach("ruins")
    elif route == "partial_wetland":
        run.strike_next("wetland")
        wait(lambda: run.encounter("wetland")["defeated_required"] == 1)
        run.retreat("wetland")
        assert run.encounter("wetland")["phase"] == "dormant"
        run.approach("ruins")
    run.finish("ruins")
    run.collect()
    run.enter_final_wave()
    heals = [run.observe_heal(weakened)]
    run.page.screenshot(path=str(OUTPUT / f"{route}.png"), full_page=True)
    if weakened:
        run.retreat("sanctum")
        assert run.encounter("wetland")["phase"] == "completed"
        run.enter_final_wave()
        heals.append(run.observe_heal(True))
        assert heals[1]["pulse"]["cast_id"] > heals[0]["pulse"]["cast_id"]
    run.finish("sanctum")
    run.collect()
    run.open_reward_and_extract()
    if route == "ordinary":
        deadline = time.monotonic() + 25
        while run.host.process.poll() is None and time.monotonic() < deadline:
            run.page.wait_for_timeout(20)
        run.host.killed()
        frozen = next(request for _, request in run.host.state()["requests"]
                      if request["body"]["kind"] == "commit_settlement")
        run.page.goto("about:blank")
        run.host.start()
        run.page.goto(run.host.url)
        value = run.done()
        assert settlement(value)["body"] == frozen["body"]
    else:
        value = run.done()
    check_result(value, route, run.config)
    if weakened:
        for _ in range(2):
            restored = run.restart()
            assert restored["ledger"] == value["ledger"]
            assert settlement(restored) == settlement(value)
            check_result(restored, route, run.config)
    return {"heals": heals, "ledger": value["ledger"], "settlementRequest": settlement(value)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=["ordinary", "partial_wetland", "weakened_retry"])
    args = parser.parse_args()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = hashes()
    subprocess.run(["python3", "scripts/export_b3_14.py", "--check"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "--locked", "--features", "replay", "--bin", "kernel-host"],
                   cwd=ROOT, check=True)
    binary = ROOT / "target/debug/kernel-host"
    binary_hash = hashlib.sha256(binary.read_bytes()).hexdigest()
    results = {}
    errors = []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        for route in ["ordinary", "partial_wetland", "weakened_retry"]:
            if args.case and route != args.case:
                continue
            with tempfile.TemporaryDirectory(prefix="wetland-support-") as directory:
                host = Host(directory)
                context = browser.new_context(viewport={"width": 1440, "height": 1100})
                page = context.new_page()
                page.on("pageerror", lambda error: errors.append(str(error)))
                try:
                    if route == "ordinary":
                        host.start("before_storage_commit", "commit_settlement")
                    else:
                        host.start()
                    assert page.goto(host.url).ok
                    run = ForestRun(page, host)
                    results[route] = run_route(run, route)
                    print(f"{route}: passed", flush=True)
                except Exception:
                    page.screenshot(path=str(OUTPUT / f"failure-{route}.png"), full_page=True)
                    if host.process.poll() is None:
                        (OUTPUT / f"failure-{route}.json").write_text(
                            json.dumps(info(page, host.url), ensure_ascii=False, indent=2)
                        )
                    raise
                finally:
                    context.close()
                    host.stop()
                    host.error_log.close()
        browser.close()
    assert not errors, errors
    assert sources == hashes()
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == binary_hash
    filename = "verification-development.json" if args.case else "verification.json"
    report = {"sourceHashes": sources, "binarySha256": binary_hash, "pageErrors": errors, "results": results}
    (OUTPUT / filename).write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
