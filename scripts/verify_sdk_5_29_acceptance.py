"""Bind CM01–CM06 to the same release archive, workspace tests and preview."""
import json
from pathlib import Path
from urllib.request import urlopen

from package_sdk import ROOT, digest

OUT = ROOT / "artifacts/sdk-5-29"
PREVIEW = "http://127.0.0.1:4222/"


def load(relative):
    return json.loads((OUT / relative).read_text())


def main():
    native = load("native-verification.json")
    public = load("public/report.json")
    browser = load("browser/report.json")
    delivery = load("delivery/independent-verification.json")
    package = load("delivery/package.json")
    assert all(report["passed"] for report in [native, public, browser, delivery])
    assert native["rustTests"] == 1501 and native["retainedRustTests"] == 1494
    assert native["nodeTests"] == 94 and native["sameDefaultAndAllFeatures"]
    assert sorted(native["newTests"]) == delivery["offlineTests"]
    assert all(digest(ROOT / file) == expected for file, expected in native["sourceHashes"].items())
    assert all(digest(OUT / f"{name}.log") == expected for name, expected in native["logs"].items())
    assert all(digest(OUT / "delivery" / f"{name}.log") == expected for name, expected in delivery["logs"].items())
    assert package["version"] == "0.31.0" and digest(package["archive"]) == package["sha256"] == delivery["archiveSha256"]
    assert package["sourceCrateSha256"] == delivery["sourceCrateSha256"]
    binary = digest(OUT / "candidate/bin/kernel-host")
    assert all(report["binarySha256"] == binary for report in [package, public, browser, delivery])
    assert delivery["legacy45"]["members"] == 8 and delivery["legacy45"]["allSkillsCommitted"]
    assert delivery["legacy45"]["phase"] == "closed" and delivery["legacy45"]["pending"] == 0
    assert all(delivery[key] for key in ["outsideCheckout", "sourceFixturesHidden", "emptyCargoHome", "offline", "rustConsumerBuilt"])
    for cases, capacities in [(public["cases"], "members"), (browser["cases"], "pages")]:
        assert [case[capacities] for case in cases] == [4, 8]
        assert all(case["settled"] == case[capacities] and case["peerContinued"] and case["hpCost"] == 3 for case in cases)
    for case, preparation, recovery in zip(public["cases"], [0, 0.25], [0.5, 0]):
        assert case["phase"] == "closed" and case["pending"] == 0 and case["full"] > 0 and case["delta"] > 0
        assert abs(case["preparationRatio"] - preparation) < 1e-6 and abs(case["recoveryRatio"] - recovery) < 1e-6
    assert all(not case["errors"] and case["lockedSamples"] >= 2 for case in browser["cases"])
    baseline = load("baseline.json")
    for path, expected in baseline.items():
        assert (ROOT / path).is_file() and digest(ROOT / path) == expected, path

    with urlopen(PREVIEW + "api/config", timeout=5) as response:
        preview = json.load(response)
    assert preview["sdk"]["crateVersion"] == "0.31.0" and preview["sdk"]["castMovement"]["minimumMechanismVersion"] == 46
    assert {scenario["id"] for scenario in preview["scenarios"]} == {"four", "eight"}

    criteria = {
        "CM01": "Strict config and mechanism 45 compatibility",
        "CM02": "Phase speeds, latest movement intent and terrain",
        "CM03": "Frozen policy, activation origin, timing and cost",
        "CM04": "Peer isolation, reconnect and settlement",
        "CM05": "Four and eight members via ordinary Node and H5",
        "CM06": "Full regression and offline, outside-checkout delivery",
    }
    paths = ["native-verification.json", "public/report.json", "browser/report.json",
             "delivery/independent-verification.json", "delivery/package.json", "baseline.json"]
    docs = ["docs/batches/SDK-5-29-cast-movement.md", "docs/interfaces/cast-movement-v1.md"]
    evidence = {
        "passed": True, "batch": "SDK-5-29", "crate": "0.31.0", "protocol": 6, "schema": 7,
        "mechanism": 46, "criteria": {key: {"status": "passed", "scope": value} for key, value in criteria.items()},
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
