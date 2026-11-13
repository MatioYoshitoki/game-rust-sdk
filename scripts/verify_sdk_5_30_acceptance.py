"""Bind CH01–CH06 to the same release archive, workspace tests and preview."""
import json
from pathlib import Path
from urllib.request import urlopen

from package_sdk import ROOT, digest

OUT = ROOT / "artifacts/sdk-5-30"
PREVIEW = "http://127.0.0.1:4225/"


def load(relative):
    return json.loads((OUT / relative).read_text())


def main():
    native = load("native-verification.json")
    public = load("public/report.json")
    browser = load("browser/report.json")
    delivery = load("delivery/independent-verification.json")
    package = load("delivery/package.json")
    assert all(report["passed"] for report in [native, public, browser, delivery])
    assert native["rustTests"] == 1509 and native["retainedRustTests"] == 1501
    assert native["nodeTests"] == 96 and native["sameDefaultAndAllFeatures"]
    assert sorted(native["newTests"]) == delivery["offlineTests"]
    assert all(digest(ROOT / file) == expected for file, expected in native["sourceHashes"].items())
    assert all(digest(OUT / f"{name}.log") == expected for name, expected in native["logs"].items())
    assert all(digest(OUT / "delivery" / f"{name}.log") == expected for name, expected in delivery["logs"].items())
    assert package["version"] == "0.32.0" and digest(package["archive"]) == package["sha256"] == delivery["archiveSha256"]
    assert package["sourceCrateSha256"] == delivery["sourceCrateSha256"]
    binary = digest(OUT / "candidate/bin/kernel-host")
    assert all(report["binarySha256"] == binary for report in [package, public, browser, delivery])
    legacy = load("legacy46/report.json")
    assert legacy["passed"] and legacy["binarySha256"] == binary
    assert [case["members"] for case in legacy["cases"]] == [4, 8]
    assert delivery["legacy46"]["reportSha256"] == digest(OUT / "legacy46/report.json")
    assert all(delivery[key] for key in ["outsideCheckout", "sourceFixturesHidden", "emptyCargoHome", "offline", "rustConsumerBuilt"])
    for cases, capacities in [(public["cases"], "members"), (browser["cases"], "pages")]:
        assert [case[capacities] for case in cases] == [4, 8]
        assert all(case["settled"] == case[capacities] and case["peerContinued"] and case["hpCost"] == 3 for case in cases)
    for case in public["cases"]:
        assert case["phase"] == "closed" and case["pending"] == 0 and case["full"] > 0 and case["delta"] > 0
    first, second = public["cases"]
    assert first["earlyReleaseChecked"] and not first["automatic"] and not first["piercing"]
    assert second["automatic"] and second["piercing"]
    for cases in [public["cases"], browser["cases"]]:
        for case, suffix in zip(cases, ["a", "b"]):
            profile = json.loads((ROOT / "examples/direction-charge-host" / f"arena-{suffix}.json").read_text())
            charge = profile["skill"]["charge"]
            minimum, maximum = charge["min_ms"], charge["max_ms"]
            held, floor = case["heldMs"], charge["min_damage_bps"]
            assert minimum <= held <= maximum
            assert (held == maximum) == (suffix == "b")
            assert case["factorBps"] == floor + (10000 - floor) * (held - minimum) // (maximum - minimum)
    assert all(not case["errors"] for case in browser["cases"])
    for path in ["web/replication.mjs", "web/skill-hp-cost.js", "web/skill-hp-cost.html"]:
        assert digest(ROOT / path) == digest(OUT / "candidate" / path), path
    for path in (ROOT / "examples/direction-charge-host").glob("*.json"):
        assert digest(path) == digest(OUT / "candidate/samples/direction-charge" / path.name)
    baseline = load("baseline.json")
    for path, expected in baseline.items():
        assert (ROOT / path).is_file() and digest(ROOT / path) == expected, path

    with urlopen(PREVIEW + "api/config", timeout=5) as response:
        preview = json.load(response)
    assert preview["sdk"]["crateVersion"] == "0.32.0" and preview["sdk"]["directionCharge"]["minimumMechanismVersion"] == 47
    assert {scenario["id"] for scenario in preview["scenarios"]} == {"four", "eight"}

    criteria = {
        "CH01": "Strict charge configuration and typed input gate",
        "CH02": "Early release, minimum, linear charge power, maximum and movement",
        "CH03": "Projectile variants, commit revalidation and atomic costs",
        "CH04": "Private charge views, member isolation, retry and reconnect",
        "CH05": "Ordinary Node and H5 manual and automatic charge with settlement",
        "CH06": "Full regression and offline delivery, including mechanism 46",
    }
    paths = ["native-verification.json", "public/report.json", "browser/report.json",
             "delivery/independent-verification.json", "delivery/package.json", "legacy46/report.json", "baseline.json"]
    docs = ["docs/batches/SDK-5-30-direction-charge.md", "docs/interfaces/direction-charge-v1.md"]
    evidence = {
        "passed": True, "batch": "SDK-5-30", "crate": "0.32.0", "protocol": 6, "schema": 7,
        "mechanism": 47, "criteria": {key: {"status": "passed", "scope": value} for key, value in criteria.items()},
        "rustTests": native["rustTests"], "nodeTests": native["nodeTests"], "protectedFiles": len(baseline),
        "archiveSha256": package["sha256"], "binarySha256": binary,
        "preview": {"url": PREVIEW, "method": "GET", "scenarios": ["four", "eight"]},
        "reports": {path: digest(OUT / path) for path in paths},
        "docs": {path: digest(ROOT / path) for path in docs},
    }
    (OUT / "acceptance.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps({key: evidence[key] for key in ["passed", "batch", "rustTests", "nodeTests", "protectedFiles"]}))


if __name__ == "__main__":
    main()
