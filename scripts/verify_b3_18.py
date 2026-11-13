"""Verify the Ancient Glacier routes through H5, Rust, and SQLite restart."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from playwright.sync_api import sync_playwright

from verify_b3_13 import wait
from verify_b3_14 import ForestRun, settlement
from verify_b3_15 import begin_route, mark_key, package, verify_settlement, warning
from verify_durability import Host
from verify_reconnect import hashes


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "artifacts/b3-18"
SCENARIO = "b3-18"
CORE = "sanctum-1"


def events_for(run, kind, mark):
    return [event for event in run.current()["events"]
            if event["kind"] == kind and mark_key(event) == mark_key(mark)]


def check_window(mark, prepared):
    assert mark["resolves_at_ms"] - mark["started_at_ms"] == (3700 if prepared else 2200), mark
    assert mark["miss_heal"] == (28 if prepared else 80) and not mark["hit_confirmed"], mark


def scenario(run, prepared):
    begin_route(run, prepared, SCENARIO)
    if prepared:
        run.approach("sanctum")
    core = run.monster(CORE)
    firing_point = [core["position"][0] - 110, core["position"][1]]
    run.arrive(firing_point)
    assert run.snapshot()["pve"]["timed_position"]["contact_target_id"] == CORE
    assert run.snapshot()["pve"]["timed_position"]["warning"] is None
    run.send("attack")
    wait(lambda: run.monster(CORE)["hp"] == core["max_hp"] - 100)
    first = warning(run)
    assert mark_key(first) == (1, 1)
    check_window(first, prepared)
    missed = wait(lambda: next(iter(events_for(run, "timed_hit_missed", first)), None))
    assert missed["target_id"] == CORE and missed["healed"] == first["miss_heal"], missed
    assert run.monster(CORE)["hp"] == core["max_hp"] - 100 + missed["healed"]
    previous = first

    if prepared:
        previous = warning(run, mark_key(first))
        run.retreat("sanctum")
        assert run.encounter("sanctum")["reset"]["round"] == 2
        assert events_for(run, "timed_position_cancelled", previous)
        assert not events_for(run, "timed_hit_missed", previous)
        run.arrive(firing_point)

    mark = warning(run, mark_key(previous))
    check_window(mark, prepared)
    assert mark_key(mark) == ((2, 1) if prepared else (1, 2)), mark
    if prepared:
        position = run.snapshot()["position"]
        direction = [core["position"][i] - position[i] for i in range(2)]
        run.send("cast", skill_id=run.config["skill"]["id"], direction=direction)
    else:
        run.send("attack")
    hit = wait(lambda: next(iter(events_for(run, "timed_hit_triggered", mark)), None))
    assert hit["target_id"] == CORE and hit["damage"] == 160, hit
    wait(lambda: run.snapshot()["logical_ms"] > mark["resolves_at_ms"])
    assert events_for(run, "timed_hit_triggered", mark) == [hit]
    assert not events_for(run, "timed_hit_missed", mark)
    assert "破冰成功" in run.page.locator("#event-log").inner_text()

    run.finish("sanctum")
    assert run.snapshot()["pve"]["timed_position"]["warning"] is None
    run.collect()
    run.open_reward_and_extract()
    value = run.done()
    verify_settlement(run, value, prepared, "jiu-tian-bing", 3)
    assert value["ledger"]["items"]["jiu-tian-bing"] >= 2
    assert settlement(value)["body"]["map_progress"]["consumed"] == [{"item_id": "xuan-bing", "quantity": 2}]
    restored = run.restart()
    assert settlement(restored) == settlement(value) and restored["ledger"] == value["ledger"]
    verify_settlement(run, restored, prepared, "jiu-tian-bing", 3)
    return {"missWindow": first, "miss": missed, "hitWindow": mark, "hit": hit,
            "ledger": value["ledger"], "settlementRequest": settlement(value)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=["direct", "prepared"])
    args = parser.parse_args()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = hashes()
    subprocess.run(["python3", "scripts/export_b3_18.py", "--check"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "--locked", "--features", "replay", "--bin", "kernel-host"],
                   cwd=ROOT, check=True)
    binary = ROOT / "target/debug/kernel-host"
    binary_sha = hashlib.sha256(binary.read_bytes()).hexdigest()
    results, errors = {}, []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        for name in ([args.case] if args.case else ["direct", "prepared"]):
            with tempfile.TemporaryDirectory(prefix="ancient-glacier-") as directory:
                host = Host(directory, package(directory, SCENARIO))
                page = browser.new_page(viewport={"width": 1440, "height": 1100})
                page.on("pageerror", lambda error: errors.append(str(error)))
                run = None
                try:
                    host.start()
                    assert page.goto(host.url).ok
                    run = ForestRun(page, host, scenario=SCENARIO)
                    results[name] = scenario(run, prepared=name == "prepared")
                    print(f"{name}: passed", flush=True)
                except Exception:
                    if run is not None and host.process.poll() is None:
                        (OUTPUT / f"failure-{name}.json").write_text(
                            json.dumps(run.current(), ensure_ascii=False, indent=2)
                        )
                    raise
                finally:
                    page.context.close()
                    host.stop()
                    host.error_log.close()
        browser.close()
    assert not errors, errors
    assert sources == hashes() and hashlib.sha256(binary.read_bytes()).hexdigest() == binary_sha
    filename = "verification-development.json" if args.case else "verification.json"
    report = {"sourceHashes": sources, "binarySha256": binary_sha, "pageErrors": errors, "results": results}
    (OUTPUT / filename).write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
