"""Verify source-terrain rotating sites and the full Meteor Wasteland settlement route."""

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
from verify_reconnect import hashes


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "artifacts/b3-16"
SCENARIO = "b3-16"


def scenario(run, prepared):
    begin_route(run, prepared, SCENARIO)
    sites = run.config["pve"]["timed_position"]["position"]["sites"]
    run.arrive(sites[1]["position"])
    first = warning(run)
    assert mark_key(first) == (1, 1) and first["site"] == sites[0], first
    duration, backlash = (4700, 22) if prepared else (3200, 40)
    assert first["resolves_at_ms"] - first["started_at_ms"] == duration
    assert first["failure_raw_damage"] == backlash
    failure = result(run, first)
    assert failure["site_id"] == sites[0]["id"] and failure["target_id"] is None
    assert failure["damage"] == backlash, failure
    trace = [failure]
    marks = [first]
    last = mark_key(first)

    if prepared:
        pending = warning(run, last)
        assert pending["site"] == sites[1] and mark_key(pending) == (1, 2)
        run.retreat("sanctum")
        assert run.encounter("sanctum")["reset"]["round"] == 2
        assert run.snapshot()["pve"]["timed_position"]["warning"] is None
        history = run.current()["events"]
        assert any(e["kind"] == "timed_position_cancelled" and mark_key(e) == mark_key(pending)
                   for e in history)
        assert not any(e["kind"] == "timed_position_resolved" and mark_key(e) == mark_key(pending)
                       for e in history)
        last = mark_key(pending)
        steps = [(0, "sanctum-2", 82), (1, "sanctum-3", 82),
                 (2, "sanctum-1", 350), (0, "sanctum-1", None)]
    else:
        steps = [(1, "sanctum-3", 82), (2, "sanctum-1", 350),
                 (0, "sanctum-2", 82), (1, "sanctum-1", None)]

    for step, (index, target, amount) in enumerate(steps):
        run.arrive(sites[index]["position"])
        mark = warning(run, last)
        assert mark["site"] == sites[index], mark
        assert mark["resolves_at_ms"] - mark["started_at_ms"] == duration
        assert mark["failure_raw_damage"] == backlash
        if prepared:
            assert mark_key(mark) == (2, step + 1)
        else:
            assert mark_key(mark) == (1, step + 2)
        resolved = result(run, mark)
        assert resolved["site_id"] == sites[index]["id"] and resolved["target_id"] == target, resolved
        if amount is None:
            assert 0 < resolved["damage"] <= 350
        else:
            assert resolved["damage"] == amount, resolved
        trace.append(resolved)
        marks.append(mark)
        last = mark_key(mark)

    assert run.encounter("sanctum")["phase"] == "completed"
    assert run.snapshot()["pve"]["timed_position"]["warning"] is None
    run.collect()
    run.open_reward_and_extract()
    value = run.done()
    verify_settlement(run, value, prepared, "scroll-caozi-jianjue", 2)
    restored = run.restart()
    assert settlement(restored) == settlement(value)
    assert restored["ledger"] == value["ledger"]
    verify_settlement(run, restored, prepared, "scroll-caozi-jianjue", 2)
    return {"warnings": marks, "resolutions": trace, "ledger": value["ledger"],
            "settlementRequest": settlement(value)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=["direct", "prepared"])
    args = parser.parse_args()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = hashes()
    subprocess.run(["python3", "scripts/export_b3_16.py", "--check"], cwd=ROOT, check=True)
    subprocess.run(["cargo", "build", "--locked", "--features", "replay", "--bin", "kernel-host"],
                   cwd=ROOT, check=True)
    binary = ROOT / "target/debug/kernel-host"
    binary_sha = hashlib.sha256(binary.read_bytes()).hexdigest()
    results, errors = {}, []
    with sync_playwright() as playwright:
        browser = playwright.chromium.launch()
        for name in ("direct", "prepared"):
            if args.case and name != args.case:
                continue
            with tempfile.TemporaryDirectory(prefix="meteor-sites-") as directory:
                host = Host(directory, package(directory, SCENARIO))
                context = browser.new_context(viewport={"width": 1440, "height": 1100})
                page = context.new_page()
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
                    context.close()
                    host.stop()
                    host.error_log.close()
        browser.close()
    assert not errors, errors
    assert sources == hashes()
    assert hashlib.sha256(binary.read_bytes()).hexdigest() == binary_sha
    filename = "verification-development.json" if args.case else "verification.json"
    report = {"sourceHashes": sources, "binarySha256": binary_sha, "pageErrors": errors, "results": results}
    (OUTPUT / filename).write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n")


if __name__ == "__main__":
    main()
