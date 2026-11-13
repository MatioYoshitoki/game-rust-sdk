"""Verify the two Immortal Edge routes through H5, Rust, and SQLite restart."""

import argparse
import hashlib
import json
import math
from pathlib import Path
import subprocess
import tempfile

from playwright.sync_api import sync_playwright

from verify_b3_13 import wait
from verify_b3_14 import ForestRun, settlement
from verify_b3_15 import begin_route, package, verify_settlement
from verify_durability import Host
from verify_reconnect import hashes


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "artifacts/b3-19"
SCENARIO = "b3-19"
CORE = "sanctum-1"


def cast_after(run, previous):
    return wait(lambda: cast if (cast := run.monster(CORE)["heavy"]["cast"])
                and cast["cast_id"] > previous else None)


def events_for(run, kind, cast):
    return [event for event in run.current()["events"] if event["kind"] == kind
            and event["cast"]["monster_id"] == CORE and event["cast"]["cast_id"] == cast["cast_id"]]


def scenario(run, prepared):
    begin_route(run, prepared, SCENARIO)
    if prepared:
        run.approach("sanctum")
    core = run.monster(CORE)
    x, y = core["position"]
    run.arrive([x, y - 315])
    run.arrive([x, y - 175])
    cast = cast_after(run, 0)
    contact = None
    if not prepared:
        contact = wait(lambda: next(iter(events_for(run, "heavy_resolved", cast)), None))
        assert contact["contacted"] and contact["damage"] == 48 and not contact["counter_awarded"], contact
        assert run.snapshot()["pve"].get("counter") is None
        cast = cast_after(run, cast["cast_id"])

    dx, dy = (cast["end"][i] - cast["origin"][i] for i in range(2))
    length = math.hypot(dx, dy)
    position = run.snapshot()["position"]
    destination = [position[0] - dy / length * 90, position[1] + dx / length * 90]
    run.arrive(destination)
    dodge = wait(lambda: next(iter(events_for(run, "heavy_resolved", cast)), None))
    assert not dodge["contacted"] and not dodge["blocked"] and dodge["counter_awarded"], dodge
    window = run.snapshot()["pve"]["counter"]
    assert window["cast_id"] == cast["cast_id"] and window["monster_id"] == CORE
    assert window["ends_at_ms"] - window["started_at_ms"] == (3800 if prepared else 2400)
    assert window["raw_damage"] == (270 if prepared else 200)

    if prepared:
        position = run.snapshot()["position"]
        direction = [core["position"][i] - position[i] for i in range(2)]
        run.send("cast", skill_id=run.config["skill"]["id"], direction=direction)
    else:
        run.send("attack")
    strike = wait(lambda: next(iter(events_for(run, "counter_triggered", cast)), None))
    assert strike["damage"] == window["raw_damage"], strike
    assert run.snapshot()["pve"].get("counter") is None
    if not prepared:
        previous_hp = run.monster(CORE)["hp"]
        wait(lambda: (s := run.snapshot())["logical_ms"] >= s["basic_ready_at_ms"])
        run.send("attack")
        wait(lambda: run.monster(CORE)["hp"] == previous_hp - 100)
        assert events_for(run, "counter_triggered", cast) == [strike]
    run.page.wait_for_function("document.querySelector('#event-log').textContent.includes('返剑')")

    run.finish("sanctum")
    run.collect()
    run.open_reward_and_extract()
    value = run.done()
    verify_settlement(run, value, prepared, "hong-meng-jin", 1)
    assert settlement(value)["body"]["map_progress"]["consumed"] == [{"item_id": "hong-meng-jin", "quantity": 1}]
    restored = run.restart()
    assert settlement(restored) == settlement(value) and restored["ledger"] == value["ledger"]
    verify_settlement(run, restored, prepared, "hong-meng-jin", 1)
    return {"contact": contact, "dodge": dodge, "window": window, "strike": strike,
            "ledger": value["ledger"], "settlementRequest": settlement(value)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=["direct", "prepared"])
    args = parser.parse_args()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = hashes()
    subprocess.run(["python3", "scripts/export_b3_19.py", "--check"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "--locked", "--features", "replay", "--bin", "kernel-host"],
                   cwd=ROOT, check=True)
    binary = ROOT / "target/debug/kernel-host"
    binary_sha = hashlib.sha256(binary.read_bytes()).hexdigest()
    results, errors = {}, []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        for name in ([args.case] if args.case else ["direct", "prepared"]):
            with tempfile.TemporaryDirectory(prefix="immortal-edge-") as directory:
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
