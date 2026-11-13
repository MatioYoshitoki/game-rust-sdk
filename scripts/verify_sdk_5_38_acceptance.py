"""Bind BP01–BP06 to the same release archive, workspace tests and preview."""
import json
import os
from pathlib import Path
from urllib.request import urlopen

from package_sdk import ROOT, digest

OUT = Path(os.environ.get("SDK_5_38_EVIDENCE_DIR", ROOT / "artifacts/sdk-5-38")).resolve()
PREVIEW = "http://127.0.0.1:4249/"


def load(relative):
    return json.loads((OUT / relative).read_text())


def verify_schema():
    relative = Path("docs/interfaces/publications-v6.schema.json")
    source = ROOT / relative
    assert digest(source) == digest(OUT / "candidate" / relative)
    schema = json.loads(source.read_text())

    def visit(value):
        if isinstance(value, dict):
            for name, item in value.items():
                if name == "$ref" and item.startswith("#/"):
                    target = schema
                    for token in item[2:].split("/"):
                        target = target[token.replace("~1", "/").replace("~0", "~")]
                else:
                    visit(item)
        elif isinstance(value, list):
            for item in value:
                visit(item)

    visit(schema)
    for name in ("basicFollowup", "recoveryCancel", "basicPursuit"):
        assert name in schema["$defs"]
    return digest(source)


def main():
    native = load("native-verification.json")
    public = load("public/report.json")
    browser = load("browser/report.json")
    delivery = load("delivery/independent-verification.json")
    package = load("delivery/package.json")
    assert all(report["passed"] for report in [native, public, browser, delivery])
    assert native["rustTests"] == 1561 and native["retainedRustTests"] == 1555
    assert native["nodeTests"] == 112 and native["sameDefaultAndAllFeatures"]
    assert sorted(native["newTests"]) == delivery["offlineTests"]
    assert all(digest(ROOT / file) == expected for file, expected in native["sourceHashes"].items())
    assert all(digest(OUT / f"{name}.log") == expected for name, expected in native["logs"].items())
    assert all(digest(OUT / "delivery" / f"{name}.log") == expected for name, expected in delivery["logs"].items())
    assert package["version"] == "0.40.0" and digest(package["archive"]) == package["sha256"] == delivery["archiveSha256"]
    assert package["sourceCrateSha256"] == delivery["sourceCrateSha256"]
    binary = digest(OUT / "candidate/bin/kernel-host")
    assert all(report["binarySha256"] == binary for report in [package, public, browser, delivery])
    legacy = load("legacy54/report.json")
    assert legacy["passed"] and legacy["binarySha256"] == binary
    assert [case["members"] for case in legacy["cases"]] == [4, 8]
    assert delivery["legacy54"]["reportSha256"] == digest(OUT / "legacy54/report.json")
    assert all(delivery[key] for key in ["outsideCheckout", "sourceFixturesHidden", "emptyCargoHome", "offline", "rustConsumerBuilt"])
    for cases, capacities in [(public["cases"], "members"), (browser["cases"], "pages")]:
        assert [case[capacities] for case in cases] == [4, 8]
        assert all(case["settled"] == case[capacities] and case["peerContinued"] for case in cases)
    for case in public["cases"]:
        assert case["phase"] == "closed" and case["pending"] == 0 and case["full"] > 0 and case["delta"] > 0
    for cases in [public["cases"], browser["cases"]]:
        assert [(case["outcome"], case["hpCost"], case["commits"]) for case in cases] == [
            ("committed", 0, 1), ("cancelled", 0, 0),
        ]
        assert all(case["target"]["generation"] >= 1 and case["travelled"] > 0 for case in cases)
    assert [case["projectiles"] for case in public["cases"]] == [1, 0]
    assert all(not case["errors"] for case in browser["cases"])
    for path in ["web/replication.mjs", "web/skill-hp-cost.js", "web/skill-hp-cost.html"]:
        assert digest(ROOT / path) == digest(OUT / "candidate" / path), path
    for path in (ROOT / "examples/basic-pursuit-host").glob("*.json"):
        assert digest(path) == digest(OUT / "candidate/samples/basic-pursuit" / path.name)
    baseline = load("baseline.json")
    missing_historical = []
    for path, expected in baseline.items():
        file = ROOT / path
        if path.startswith("artifacts/") and not file.is_file():
            missing_historical.append(path)
            continue
        assert file.is_file() and digest(file) == expected, path
    assert not missing_historical or os.environ.get("SDK_5_38_ALLOW_CLEANED_HISTORY") == "1"
    verified_files = len(baseline) - len(missing_historical)

    with urlopen(PREVIEW + "api/config", timeout=5) as response:
        preview = json.load(response)
    assert preview["sdk"]["crateVersion"] == "0.40.0" and preview["sdk"]["basicPursuit"]["minimumMechanismVersion"] == 55
    assert {scenario["id"] for scenario in preview["scenarios"]} == {"four", "eight"}

    with urlopen(PREVIEW, timeout=5) as response:
        assert response.status == 200 and b"<html" in response.read()

    criteria = {
        "BP01": "Opt-in config, range and deadline validation, and mechanism 54 compatibility",
        "BP02": "Movement to range, original action identity, projectile, hit and cooldown",
        "BP03": "Blocked, stale, out-of-range and deadline failures without costs",
        "BP04": "Cancel, idempotency, reconnect, member isolation and private replica",
        "BP05": "Four-member commit and eight-member cancel through ordinary Node and H5",
        "BP06": "Full regression, offline delivery and mechanism 54 consumption",
    }
    paths = ["native-verification.json", "public/report.json", "browser/report.json",
             "delivery/independent-verification.json", "delivery/package.json", "legacy54/report.json", "baseline.json"]
    docs = ["docs/batches/SDK-5-38-basic-pursuit.md", "docs/interfaces/basic-pursuit-v1.md"]
    evidence = {
        "passed": True, "batch": "SDK-5-38", "crate": "0.40.0", "protocol": 6, "schema": 7,
        "mechanism": 55, "criteria": {key: {"status": "passed", "scope": value} for key, value in criteria.items()},
        "rustTests": native["rustTests"], "nodeTests": native["nodeTests"], "protectedFiles": verified_files,
        "historicalArtifactsMissing": len(missing_historical),
        "publicationSchemaSha256": verify_schema(),
        "archiveSha256": package["sha256"], "binarySha256": binary,
        "preview": {"url": PREVIEW, "method": "GET", "scenarios": ["four", "eight"]},
        "reports": {path: digest(OUT / path) for path in paths},
        "docs": {path: digest(ROOT / path) for path in docs},
    }
    (OUT / "acceptance.json").write_text(json.dumps(evidence, indent=2) + "\n")
    print(json.dumps({key: evidence[key] for key in ["passed", "batch", "rustTests", "nodeTests", "protectedFiles"]}))


if __name__ == "__main__":
    main()
