"""Verify Spirit Vein Cave through H5, the Rust host, and SQLite restart."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

from playwright.sync_api import sync_playwright

from verify_b3_14 import ForestRun, settlement
from verify_b3_15 import begin_route, mark_key, package, result, verify_settlement, warning
from verify_durability import Host
from verify_h5 import active, wait
from verify_reconnect import hashes


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "artifacts/b3-17"
SCENARIO = "b3-17"


def observe(run, site_id, cycle, *, damage=0, restored=range(13)):
    mark = warning(run, (cycle[0], cycle[1] - 1))
    assert mark_key(mark) == cycle and mark["site"]["id"] == site_id, mark
    assert mark["resolves_at_ms"] - mark["started_at_ms"] == 3000
    assert mark["failure_raw_damage"] == 24
    resolved = result(run, mark)
    assert resolved["site_id"] == site_id and resolved["target_id"] is None, resolved
    assert resolved["damage"] == damage, resolved
    if restored is None:
        assert "spirit_restored" not in resolved, resolved
    else:
        assert resolved["spirit_restored"] in restored, resolved
    return {"warning": mark, "resolution": resolved}


def verify_final_assets(run, value, prepared):
    verify_settlement(run, value, prepared, "xuan-huang", 7)
    body = settlement(value)["body"]
    assert body["map_progress"]["consumed"] == [{"item_id": "xuan-tie", "quantity": 2}]
    assert body["looted"] >= 2
    assert body["carried"] + body["looted"] == body["kept"] + body["dropped"] + 2
    assert 5 <= value["ledger"]["items"]["xuan-huang"] <= 7
    assert value["ledger"]["items"]["huang-yu"] == 6


def scenario(run, prepared):
    page = run.page
    sites = {s["id"]: s["position"] for s in run.config["pve"]["timed_position"]["position"]["sites"]}
    trace = []
    if prepared:
        page.locator("#scenario").select_option(SCENARIO)
        page.locator("#clock").select_option("fast")
        page.locator("#start").click()
        active(page)
        page.locator("#navigate-encounter").click()
        run.stop_at_encounter("ruins")
        run.finish("ruins")
        run.collect()
        run.approach("sanctum")
        run.arrive(sites["furnace"])
        assert run.snapshot()["pve"]["timed_position"]["stabilized_sites"] == 1
        trace.append(observe(run, "furnace", (1, 1)))
        run.arrive(sites["river"])
        trace.append(observe(run, "river", (1, 2)))
        run.retreat("sanctum")
        assert run.encounter("sanctum")["reset"]["round"] == 2
        assert run.snapshot()["pve"]["timed_position"]["warning"] is None
        run.approach("wetland")
        run.finish("wetland")
        run.collect()
        run.approach("sanctum")
        run.arrive(sites["furnace"])
        assert run.snapshot()["pve"]["timed_position"]["stabilized_sites"] == 2
        trace.append(observe(run, "furnace", (2, 1)))
        trace.append(observe(run, "furnace", (2, 2)))
    else:
        begin_route(run, False, SCENARIO)
        run.arrive(sites["crystal"])
        trace.append(observe(run, "furnace", (1, 1), damage=24, restored=None))
        cast = run.send("cast", skill_id=run.config["skill"]["id"], direction=[0, 1])
        wait(lambda: any(e["kind"] == "action_committed" and e["action_id"] == cast
                         for e in run.current()["events"]))
        trace.append(observe(run, "crystal", (1, 2), restored=(12,)))
        run.arrive(sites["river"])
        trace.append(observe(run, "river", (1, 3)))
        run.arrive(sites["furnace"])
        trace.append(observe(run, "furnace", (1, 4)))

    assert page.locator("#arena").get_attribute("data-projection") == "isometric"
    page.wait_for_function("label => document.querySelector('#timed-position-status').textContent.includes(label)",
                           arg=f"已稳定 {2 if prepared else 0} 条灵流")
    assert "镇守成功" in page.locator("#event-log").inner_text()
    assert run.snapshot()["pve"]["kills"] == (4 if prepared else 0)
    run.finish("sanctum")
    assert run.snapshot()["pve"]["timed_position"]["warning"] is None
    run.collect()
    run.open_reward_and_extract()
    value = run.done()
    verify_final_assets(run, value, prepared)
    restored = run.restart()
    assert settlement(restored) == settlement(value) and restored["ledger"] == value["ledger"]
    verify_final_assets(run, restored, prepared)
    return {"trace": trace, "ledger": value["ledger"], "settlementRequest": settlement(value)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=["direct", "prepared"])
    args = parser.parse_args()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = hashes()
    subprocess.run(["python3", "scripts/export_b3_17.py", "--check"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "--locked", "--features", "replay", "--bin", "kernel-host"],
                   cwd=ROOT, check=True)
    binary = ROOT / "target/debug/kernel-host"
    binary_sha = hashlib.sha256(binary.read_bytes()).hexdigest()
    results, errors = {}, []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        for name in ([args.case] if args.case else ["direct", "prepared"]):
            with tempfile.TemporaryDirectory(prefix="spirit-vein-") as directory:
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
