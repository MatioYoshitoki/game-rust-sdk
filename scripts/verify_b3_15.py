"""Verify the reusable timed-position flow on Thunder Valley source terrain."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

from playwright.sync_api import sync_playwright

from verify_b3_13 import wait
from verify_b3_14 import ForestRun, settlement
from verify_durability import Host
from verify_h5 import active
from verify_reconnect import hashes


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "artifacts/b3-15"


def package(directory, scenario="b3-15"):
    path = Path(directory) / "config"
    path.mkdir()
    for filename in ["b1.json", f"{scenario}.json"]:
        shutil.copyfile(ROOT / "config" / filename, path / filename)
    manifest = path / "catalog.json"
    manifest.write_text(json.dumps({
        "schema_version": 1,
        "active": {"b1": "b1.json", scenario: f"{scenario}.json"},
        "archives": [],
    }))
    return manifest


def mark_key(value):
    return value["round"], value["cycle"]


def warning(run, after=(0, 0)):
    return wait(lambda: mark if (mark := run.snapshot()["pve"]["timed_position"]["warning"])
                and mark_key(mark) > after else None)


def result(run, mark):
    def resolved():
        return next((event for event in run.current()["events"]
                     if event["kind"] == "timed_position_resolved" and mark_key(event) == mark_key(mark)), None)
    return wait(resolved)


def verify_settlement(run, value, prepared, reward_marker="scroll-leidi-baoshu", reward_max=1):
    body = settlement(value)["body"]
    progress = body["map_progress"]
    ledger = value["ledger"]
    assert body["outcome"] == "extraction"
    assert progress["completed_encounter_ids"] == (
        ["ruins", "sanctum", "wetland"] if prepared else ["sanctum"]
    )
    assert progress["attuned"] and progress["reward_opened"]
    assert progress["consumed"] == [run.config["pve"]["map_objective"]["cost"]]
    assert body["scoring"]["kills"] == body["reset_progress"]["actual_kills"] == (7 if prepared else 3)
    assert body["scoring"]["kill_score"] == (160 if prepared else 120)
    assert 1 <= ledger["items"][reward_marker] <= reward_max
    assert ledger["warehouse"] == 100 - body["carried"] + body["kept"]
    assert ledger["cultivation"] == body["cultivation"]
    assert ledger["settlements"] == ledger["admissions"] == 1
    assert not ledger["pending"] and not ledger["manual"]
    consumed = {row["item_id"]: row["quantity"] for row in progress["consumed"]}
    for item in body["items"]:
        assert item["looted"] == item["kept"] + item["dropped"] + consumed.get(item["item_id"], 0)
    assert ledger["items"] == {item["item_id"]: item["kept"] for item in body["items"]}


def begin_route(run, prepared, scenario="b3-15"):
    page = run.page
    page.locator("#scenario").select_option(scenario)
    page.locator("#clock").select_option("fast")
    page.locator("#start").click()
    active(page)
    assert page.locator("#arena").get_attribute("data-projection") == "isometric"
    if prepared:
        page.locator("#navigate-encounter").click()
        run.stop_at_encounter("ruins")
        run.finish("ruins")
        run.collect()
        run.approach("wetland")
        run.finish("wetland")
        run.collect()
    else:
        page.locator("#navigate-required-encounter").click()
        run.stop_at_encounter("sanctum")


def scenario(run, prepared):
    page = run.page
    begin_route(run, prepared)
    center = run.encounter_config("sanctum")["trigger"]["center"]
    far = [center[0], center[1] + 128]
    near = [center[0], center[1] + 56]
    run.arrive(far)
    check = run.snapshot()["pve"]["timed_position"]
    assert check["warning_bonus_active"] == check["failure_scale_active"] == prepared
    label = "反噬已减轻" if prepared else "基础反噬"
    page.wait_for_function(
        "(label) => document.querySelector('#timed-position-status').textContent.includes(label)",
        arg=label,
    )
    first = warning(run)
    assert first["resolves_at_ms"] - first["started_at_ms"] == (4500 if prepared else 3000)
    assert first["failure_raw_damage"] == (23 if prepared else 42)
    failure = result(run, first)
    assert failure["target_id"] is None and failure["damage"] == first["failure_raw_damage"], failure
    trace = [failure]

    after = mark_key(first)
    if prepared:
        pending = warning(run, after)
        run.retreat("sanctum")
        assert run.encounter("sanctum")["reset"]["round"] == 2
        assert run.snapshot()["pve"]["timed_position"]["warning"] is None
        history = run.current()["events"]
        assert any(e["kind"] == "timed_position_cancelled" and mark_key(e) == mark_key(pending)
                   for e in history)
        assert not any(e["kind"] == "timed_position_resolved" and mark_key(e) == mark_key(pending)
                       for e in history)
        run.arrive(far)
        after = mark_key(pending)

    # Move inside the live check radius with an ordinary navigation input.
    second = warning(run, after)
    assert second["resolves_at_ms"] - second["started_at_ms"] == (4500 if prepared else 3000)
    run.arrive(near)
    success = result(run, second)
    assert success["target_id"] == "sanctum-1" and success["damage"] == 300, success
    trace.append(success)
    next_mark = warning(run, mark_key(second))
    finishing_strike = result(run, next_mark)
    assert finishing_strike["target_id"] == "sanctum-1" and finishing_strike["damage"] > 0
    assert run.monster("sanctum-1")["hp"] == 0
    trace.append(finishing_strike)
    page.screenshot(path=str(OUTPUT / ("prepared.png" if prepared else "direct.png")), full_page=True)
    run.finish("sanctum")
    run.collect()
    run.open_reward_and_extract()
    value = run.done()
    verify_settlement(run, value, prepared)
    restored = run.restart()
    assert settlement(restored) == settlement(value)
    assert restored["ledger"] == value["ledger"]
    verify_settlement(run, restored, prepared)
    return {"warning": first, "resolutions": trace, "ledger": value["ledger"],
            "settlementRequest": settlement(value)}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--case", choices=["direct", "prepared"])
    args = parser.parse_args()
    OUTPUT.mkdir(parents=True, exist_ok=True)
    sources = hashes()
    subprocess.run(["python3", "scripts/export_b3_15.py", "--check"], cwd=ROOT, check=True)
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
            with tempfile.TemporaryDirectory(prefix="thunder-position-") as directory:
                host = Host(directory, package(directory))
                context = browser.new_context(viewport={"width": 1440, "height": 1100})
                page = context.new_page()
                page.on("pageerror", lambda error: errors.append(str(error)))
                try:
                    host.start()
                    assert page.goto(host.url).ok
                    run = ForestRun(page, host, scenario="b3-15")
                    results[name] = scenario(run, prepared=name == "prepared")
                    print(f"{name}: passed", flush=True)
                except Exception:
                    if host.process.poll() is None and page.url == host.url + "/":
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
