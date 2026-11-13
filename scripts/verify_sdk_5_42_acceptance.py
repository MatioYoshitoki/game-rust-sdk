"""Bind IC01–IC06 to the same release archive, workspace tests and preview."""
import json
import os
from pathlib import Path
from urllib.request import urlopen

from package_sdk import ROOT, digest

OUT = Path(os.environ.get("SDK_5_42_EVIDENCE_DIR", ROOT / "artifacts/sdk-5-42")).resolve()
PREVIEW = "http://127.0.0.1:4261/"


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
    assert native["rustTests"] == 1580 and native["retainedRustTests"] == 1576
    assert native["nodeTests"] == 113 and native["sameDefaultAndAllFeatures"]
    assert sorted(native["newTests"]) == delivery["offlineTests"]
    assert all(digest(ROOT / file) == expected for file, expected in native["sourceHashes"].items())
    assert all(digest(OUT / f"{name}.log") == expected for name, expected in native["logs"].items())
    assert all(digest(OUT / "delivery" / f"{name}.log") == expected for name, expected in delivery["logs"].items())
    assert package["version"] == "0.44.0" and digest(package["archive"]) == package["sha256"] == delivery["archiveSha256"]
    assert package["sourceCrateSha256"] == delivery["sourceCrateSha256"]
    binary = digest(OUT / "candidate/bin/kernel-host")
    assert all(report["binarySha256"] == binary for report in [package, public, browser, delivery])
    legacy = load("legacy58/report.json")
    assert legacy["passed"] and legacy["binarySha256"] == binary
    assert [case["members"] for case in legacy["cases"]] == [4, 8]
    assert delivery["legacy58"]["reportSha256"] == digest(OUT / "legacy58/report.json")
    assert all(delivery[key] for key in ["outsideCheckout", "sourceFixturesHidden", "emptyCargoHome", "offline", "rustConsumerBuilt"])
    for cases, capacities in [(public["cases"], "members"), (browser["cases"], "pages")]:
        assert [case[capacities] for case in cases] == [4, 8]
        assert all(case["settled"] == case[capacities] for case in cases)
    for case in public["cases"]:
        assert case["phase"] == "closed" and case["pending"] == 0 and case["full"] > 0 and case["delta"] > 0
    for cases in [public["cases"], browser["cases"]]:
        assert [(case["usedBy"], case["transferred"], case["restored"]) for case in cases] == [
            (["P00", "P00"], False, [40, 20]), (["P00", "P01"], True, [40, 40]),
        ]
        assert all((case["consumed"], case["remaining"]) == (2, 1) for case in cases)
    assert all(case["earlyRejected"] for case in public["cases"])
    assert all(case["earlyDisabled"] for case in browser["cases"])
    assert all(not case["errors"] for case in browser["cases"])
    for path in ["web/replication.mjs", "web/skill-hp-cost.js", "web/skill-hp-cost.html"]:
        assert digest(ROOT / path) == digest(OUT / "candidate" / path), path
    for path in (ROOT / "examples/member-item-cooldown-host").glob("*.json"):
        assert digest(path) == digest(OUT / "candidate/samples/member-item-cooldown" / path.name)
    assert digest(ROOT / "adapters/h5/two-member-server.mjs") == digest(OUT / "candidate/hosts/h5/two-member-server.mjs")
    baseline = load("baseline.json")
    missing_historical = []
    for path, expected in baseline.items():
        file = ROOT / path
        if path.startswith("artifacts/") and not file.is_file():
            missing_historical.append(path)
            continue
        assert file.is_file() and digest(file) == expected, path
    assert not missing_historical or os.environ.get("SDK_5_42_ALLOW_CLEANED_HISTORY") == "1"
    verified_files = len(baseline) - len(missing_historical)

    with urlopen(PREVIEW + "api/config", timeout=5) as response:
        preview = json.load(response)
    assert preview["sdk"]["crateVersion"] == "0.44.0" and preview["sdk"]["memberItemCooldown"]["minimumMechanismVersion"] == 59
    assert {scenario["id"] for scenario in preview["scenarios"]} == {"four", "eight"}

    with urlopen(PREVIEW, timeout=5) as response:
        assert response.status == 200 and b"<html" in response.read()

    criteria = {
        "IC01": "Cooldown capability, fixed step, duration bounds and compatibility",
        "IC02": "Commit starts cooldown, early rejection is atomic and the deadline is inclusive for reuse",
        "IC03": "Item and member isolation, transfer, pickup and reconnect",
        "IC04": "Idempotent input, private replica validation and net settlement",
        "IC05": "Four-member expiry and eight-member transfer during cooldown in Node and H5",
        "IC06": "Regression, offline delivery and mechanism 58 consumption",
    }
    paths = ["native-verification.json", "public/report.json", "browser/report.json",
             "delivery/independent-verification.json", "delivery/package.json", "legacy58/report.json", "baseline.json"]
    docs = ["docs/batches/SDK-5-42-member-item-cooldown.md", "docs/interfaces/member-item-cooldown-v1.md"]
    evidence = {
        "passed": True, "batch": "SDK-5-42", "crate": "0.44.0", "protocol": 6, "schema": 7,
        "mechanism": 59, "criteria": {key: {"status": "passed", "scope": value} for key, value in criteria.items()},
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
