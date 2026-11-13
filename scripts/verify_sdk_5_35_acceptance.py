"""Bind CR01–CR06 to the same release archive, workspace tests and preview."""
import json
import os
from pathlib import Path
from urllib.request import urlopen

from package_sdk import ROOT, digest

OUT = Path(os.environ.get("SDK_5_35_EVIDENCE_DIR", ROOT / "artifacts/sdk-5-35")).resolve()
PREVIEW = "http://127.0.0.1:4240/"


def load(relative):
    return json.loads((OUT / relative).read_text())


def main():
    native = load("native-verification.json")
    public = load("public/report.json")
    browser = load("browser/report.json")
    delivery = load("delivery/independent-verification.json")
    package = load("delivery/package.json")
    assert all(report["passed"] for report in [native, public, browser, delivery])
    assert native["rustTests"] == 1543 and native["retainedRustTests"] == 1537
    assert native["nodeTests"] == 106 and native["sameDefaultAndAllFeatures"]
    assert sorted(native["newTests"]) == delivery["offlineTests"]
    assert all(digest(ROOT / file) == expected for file, expected in native["sourceHashes"].items())
    assert all(digest(OUT / f"{name}.log") == expected for name, expected in native["logs"].items())
    assert all(digest(OUT / "delivery" / f"{name}.log") == expected for name, expected in delivery["logs"].items())
    assert package["version"] == "0.37.0" and digest(package["archive"]) == package["sha256"] == delivery["archiveSha256"]
    assert package["sourceCrateSha256"] == delivery["sourceCrateSha256"]
    binary = digest(OUT / "candidate/bin/kernel-host")
    assert all(report["binarySha256"] == binary for report in [package, public, browser, delivery])
    legacy = load("legacy51/report.json")
    assert legacy["passed"] and legacy["binarySha256"] == binary
    assert [case["members"] for case in legacy["cases"]] == [4, 8]
    assert delivery["legacy51"]["reportSha256"] == digest(OUT / "legacy51/report.json")
    assert all(delivery[key] for key in ["outsideCheckout", "sourceFixturesHidden", "emptyCargoHome", "offline", "rustConsumerBuilt"])
    for cases, capacities in [(public["cases"], "members"), (browser["cases"], "pages")]:
        assert [case[capacities] for case in cases] == [4, 8]
        assert all(case["settled"] == case[capacities] and case["peerContinued"] for case in cases)
    for case in public["cases"]:
        assert case["phase"] == "closed" and case["pending"] == 0 and case["full"] > 0 and case["delta"] > 0
    for cases in [public["cases"], browser["cases"]]:
        assert all(0 < case["earlierByMs"] <= 1800 for case in cases)
        assert [(case["outcome"], case["hpCost"], case["replacementSkill"]) for case in cases] == [
            ("replaced", 5, "next-bolt"), ("cancelled", 3, None),
        ]
    assert [(case["projectiles"], case["replacementSpiritCost"]) for case in public["cases"]] == [(2, 7), (1, 0)]
    assert [case["commits"] for case in browser["cases"]] == [2, 1]
    assert all(not case["errors"] for case in browser["cases"])
    for path in ["web/replication.mjs", "web/skill-hp-cost.js", "web/skill-hp-cost.html"]:
        assert digest(ROOT / path) == digest(OUT / "candidate" / path), path
    for path in (ROOT / "examples/cancelable-recovery-host").glob("*.json"):
        assert digest(path) == digest(OUT / "candidate/samples/cancelable-recovery" / path.name)
    baseline = load("baseline.json")
    missing_historical = []
    for path, expected in baseline.items():
        file = ROOT / path
        if path.startswith("artifacts/") and not file.is_file():
            missing_historical.append(path)
            continue
        assert file.is_file() and digest(file) == expected, path
    assert not missing_historical or os.environ.get("SDK_5_35_ALLOW_CLEANED_HISTORY") == "1"
    verified_files = len(baseline) - len(missing_historical)

    with urlopen(PREVIEW + "api/config", timeout=5) as response:
        preview = json.load(response)
    assert preview["sdk"]["crateVersion"] == "0.37.0" and preview["sdk"]["recoveryCancel"]["minimumMechanismVersion"] == 52
    assert {scenario["id"] for scenario in preview["scenarios"]} == {"four", "eight"}

    criteria = {
        "CR01": "Strict per-skill opt-in configuration, bounds and mechanism 51 compatibility",
        "CR02": "Explicit cancellation boundaries and movement with committed effects intact",
        "CR03": "Atomic replacement admission, unchanged state on rejection and actual commit",
        "CR04": "Idempotency, reconnect, terminal cleanup and private replica",
        "CR05": "Four and eight members through ordinary Node and H5",
        "CR06": "Full regression, offline delivery and mechanism 51 consumption",
    }
    paths = ["native-verification.json", "public/report.json", "browser/report.json",
             "delivery/independent-verification.json", "delivery/package.json", "legacy51/report.json", "baseline.json"]
    docs = ["docs/batches/SDK-5-35-cancelable-recovery.md", "docs/interfaces/cancelable-recovery-v1.md"]
    evidence = {
        "passed": True, "batch": "SDK-5-35", "crate": "0.37.0", "protocol": 6, "schema": 7,
        "mechanism": 52, "criteria": {key: {"status": "passed", "scope": value} for key, value in criteria.items()},
        "rustTests": native["rustTests"], "nodeTests": native["nodeTests"], "protectedFiles": verified_files,
        "historicalArtifactsMissing": len(missing_historical),
        "archiveSha256": package["sha256"], "binarySha256": binary,
        "preview": {"url": PREVIEW, "method": "GET", "scenarios": ["four", "eight"]},
        "reports": {path: digest(OUT / path) for path in paths},
        "docs": {path: digest(ROOT / path) for path in docs},
    }
    (OUT / "acceptance.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps({key: evidence[key] for key in ["passed", "batch", "rustTests", "nodeTests", "protectedFiles"]}))


if __name__ == "__main__":
    main()
