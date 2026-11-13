"""Bind RA01–RA06 to the same release archive, workspace tests and preview."""
import json
import math
from pathlib import Path
from urllib.request import urlopen

from package_sdk import ROOT, digest

OUT = ROOT / "artifacts/sdk-5-31"
PREVIEW = "http://127.0.0.1:4228/"


def load(relative):
    return json.loads((OUT / relative).read_text())


def main():
    native = load("native-verification.json")
    public = load("public/report.json")
    browser = load("browser/report.json")
    delivery = load("delivery/independent-verification.json")
    package = load("delivery/package.json")
    assert all(report["passed"] for report in [native, public, browser, delivery])
    assert native["rustTests"] == 1517 and native["retainedRustTests"] == 1509
    assert native["nodeTests"] == 98 and native["sameDefaultAndAllFeatures"]
    assert sorted(native["newTests"]) == delivery["offlineTests"]
    assert all(digest(ROOT / file) == expected for file, expected in native["sourceHashes"].items())
    assert all(digest(OUT / f"{name}.log") == expected for name, expected in native["logs"].items())
    assert all(digest(OUT / "delivery" / f"{name}.log") == expected for name, expected in delivery["logs"].items())
    assert package["version"] == "0.33.0" and digest(package["archive"]) == package["sha256"] == delivery["archiveSha256"]
    assert package["sourceCrateSha256"] == delivery["sourceCrateSha256"]
    binary = digest(OUT / "candidate/bin/kernel-host")
    assert all(report["binarySha256"] == binary for report in [package, public, browser, delivery])
    legacy = load("legacy47/report.json")
    assert legacy["passed"] and legacy["binarySha256"] == binary
    assert [case["members"] for case in legacy["cases"]] == [4, 8]
    assert delivery["legacy47"]["reportSha256"] == digest(OUT / "legacy47/report.json")
    assert all(delivery[key] for key in ["outsideCheckout", "sourceFixturesHidden", "emptyCargoHome", "offline", "rustConsumerBuilt"])
    for cases, capacities in [(public["cases"], "members"), (browser["cases"], "pages")]:
        assert [case[capacities] for case in cases] == [4, 8]
        assert all(case["settled"] == case[capacities] and case["peerContinued"] and case["hpCost"] == 3 for case in cases)
    for case in public["cases"]:
        assert case["phase"] == "closed" and case["pending"] == 0 and case["full"] > 0 and case["delta"] > 0
    first, second = public["cases"]
    assert not first["automatic"] and not first["piercing"]
    assert all(case["sameDirectionDedup"] for case in public["cases"])
    assert second["automatic"] and second["piercing"]
    for cases in [public["cases"], browser["cases"]]:
        for case, suffix in zip(cases, ["a", "b"]):
            profile = json.loads((ROOT / "examples/charge-reaim-host" / f"arena-{suffix}.json").read_text())
            charge = profile["skill"]["charge"]
            minimum, maximum = charge["min_ms"], charge["max_ms"]
            held, floor = case["heldMs"], charge["min_damage_bps"]
            assert minimum <= held <= maximum
            assert (held == maximum) == (suffix == "b")
            assert case["factorBps"] == floor + (10000 - floor) * (held - minimum) // (maximum - minimum)
            accepted, final = case["acceptedDirection"], case["finalDirection"]
            assert math.dist(accepted, [math.sqrt(0.5)] * 2) < 0.025
            assert (math.dist(final, [0, 1]) < 0.025) if suffix == "a" else final == accepted
    assert all(not case["errors"] for case in browser["cases"])
    for path in ["web/replication.mjs", "web/skill-hp-cost.js", "web/skill-hp-cost.html"]:
        assert digest(ROOT / path) == digest(OUT / "candidate" / path), path
    for path in (ROOT / "examples/charge-reaim-host").glob("*.json"):
        assert digest(path) == digest(OUT / "candidate/samples/charge-reaim" / path.name)
    baseline = load("baseline.json")
    for path, expected in baseline.items():
        assert (ROOT / path).is_file() and digest(ROOT / path) == expected, path

    with urlopen(PREVIEW + "api/config", timeout=5) as response:
        preview = json.load(response)
    assert preview["sdk"]["crateVersion"] == "0.33.0" and preview["sdk"]["chargeReaim"]["minimumMechanismVersion"] == 48
    assert {scenario["id"] for scenario in preview["scenarios"]} == {"four", "eight"}

    criteria = {
        "RA01": "Strict editable charge configuration and legacy lock compatibility",
        "RA02": "Normalized direction, deduplication and rate and angle boundaries",
        "RA03": "Manual final aim, automatic accepted aim and atomic projectile commit",
        "RA04": "Private aim projection, atomic decoding and identity fencing",
        "RA05": "Four and eight members through the ordinary Node and H5 paths",
        "RA06": "Full regression, offline delivery and mechanism 47 consumption",
    }
    paths = ["native-verification.json", "public/report.json", "browser/report.json",
             "delivery/independent-verification.json", "delivery/package.json", "legacy47/report.json", "baseline.json"]
    docs = ["docs/batches/SDK-5-31-charge-reaim.md", "docs/interfaces/charge-reaim-v1.md"]
    evidence = {
        "passed": True, "batch": "SDK-5-31", "crate": "0.33.0", "protocol": 6, "schema": 7,
        "mechanism": 48, "criteria": {key: {"status": "passed", "scope": value} for key, value in criteria.items()},
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
