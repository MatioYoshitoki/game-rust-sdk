"""Verify the exact packaged source and ordinary runtime from outside the checkout."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

from package_sdk import ROOT, digest, extract_crate

OUT = ROOT / "artifacts/sdk-5-29"
DELIVERY = OUT / "delivery"


def run(name, command, cwd, env=None):
    result = subprocess.run(command, cwd=cwd, env={**os.environ, **(env or {})},
                            capture_output=True, text=True, timeout=1200)
    (DELIVERY / f"{name}.log").write_text(result.stdout + result.stderr)
    assert result.returncode == 0, (name, result.stderr[-3000:], result.stdout[-1500:])
    print(f"{name}: passed", flush=True)
    return result.stdout


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--browser-port", type=int, default=4223)
    parser.add_argument("--reuse-non-browser", action="store_true")
    args = parser.parse_args()
    package = json.loads((DELIVERY / "package.json").read_text())
    assert digest(package["archive"]) == package["sha256"]
    with tempfile.TemporaryDirectory(prefix="sdk-5-29-consume-") as directory:
        work = Path(directory)
        bundle = work / "bundle"
        extract_crate(package["archive"], bundle, package["rootName"])
        run("integrity", ["python3", "verify_bundle.py"], bundle)
        binary = bundle / "bin/kernel-host"
        assert digest(binary) == package["binarySha256"]
        sources = [*ROOT.glob("src/**/*.rs"), *ROOT.glob("src/**/*.json")]
        for source in sources:
            assert digest(source) == digest(bundle / "sdk" / source.relative_to(ROOT)), source

        if args.reuse_non_browser:
            stage = json.loads((DELIVERY / "non-browser-stage.json").read_text())
            assert stage["archiveSha256"] == package["sha256"] and stage["binarySha256"] == package["binarySha256"]
            for name, expected in stage["logs"].items():
                assert digest(DELIVERY / f"{name}.log") == expected
            names = stage["offlineTests"]
            assert json.loads((OUT / "public/report.json").read_text())["binarySha256"] == package["binarySha256"]
            print("Prior source and Node stages verified against the same archive", flush=True)
        else:
            cargo_home = work / "empty-cargo-home"
            cargo_home.mkdir()
            offline = {"CARGO_HOME": str(cargo_home), "CARGO_NET_OFFLINE": "true",
                       "CARGO_TARGET_DIR": str(work / "target")}
            source_log = run("offline-movement-tests", [
                "cargo", "test", "--offline", "--locked", "--manifest-path", "sdk/Cargo.toml", "cast_movement",
            ], bundle, offline)
            names = sorted(re.findall(r"^test (\S+) \.\.\. ok$", source_log, re.M))
            assert len(names) == 7 and all("cast_movement" in name for name in names)
            run("offline-rust-consumer", ["cargo", "build", "--offline", "--locked",
                                        "--manifest-path", "consumer/Cargo.toml"], bundle, offline)

        external = work / "external"
        external.mkdir()
        for profile in ["cast-movement", "skill-hp-cost"]:
            shutil.copytree(bundle / "samples" / profile, external / profile)
        hidden = []
        for path in [bundle / "sdk/config", bundle / "sdk/examples", bundle / "samples"]:
            destination = work / ("hidden-" + path.name)
            path.rename(destination)
            hidden.append((path, destination))
        runtime_env = {"MATCH_CONFIG_MANIFEST": str(external / "absent-default.json")}
        if args.reuse_non_browser:
            legacy = json.loads((DELIVERY / "legacy-mechanism-45.log").read_text().splitlines()[0])
        else:
            support = json.loads(run("ordinary-info", [str(binary), "--sdk-info"], external, runtime_env))
            assert support["crateVersion"] == "0.31.0" and support["mechanismVersions"]["max"] == 46
            assert support["castMovement"]["policies"] == ["free", "slow", "locked"]
            assert not support["clocks"]["replay"]
            run("public-movement", [
                "node", str(ROOT / "scripts/verify_sdk_5_29_public.mjs"), str(binary),
                str(bundle / "hosts/node/two-member-host.mjs"), str(external / "cast-movement"), str(OUT / "public"),
            ], external, runtime_env)
            legacy = json.loads(run("legacy-mechanism-45", [
                "node", str(bundle / "hosts/node/skill-hp-cost-host.mjs"), "--binary", str(binary),
                "--configs", str(external / "skill-hp-cost/catalog.json"),
                "--accounts", str(external / "skill-hp-cost/accounts-b.json"),
                "--scenario", "eight", "--reverse", "--database", str(OUT / "legacy-mechanism-45.sqlite"),
            ], external, runtime_env))
        assert legacy["phase"] == "closed" and legacy["pending"] == 0 and legacy["hpCostsValidated"]
        assert len(legacy["hpCostStats"]) == 8
        assert all(len(member["bySkill"]) == 8 and member["total"] > 0 for member in legacy["hpCostStats"])
        if not args.reuse_non_browser:
            stage_names = ["offline-movement-tests", "offline-rust-consumer", "ordinary-info",
                           "public-movement", "legacy-mechanism-45"]
            stage = {"archiveSha256": package["sha256"], "binarySha256": package["binarySha256"],
                     "offlineTests": names,
                     "logs": {name: digest(DELIVERY / f"{name}.log") for name in stage_names}}
            (DELIVERY / "non-browser-stage.json").write_text(json.dumps(stage, indent=2) + "\n")
        run("browser", [
            "python3", str(ROOT / "scripts/verify_sdk_5_29_browser.py"),
            "--server", str(bundle / "hosts/h5/skill-hp-cost-server.mjs"), "--binary", str(binary),
            "--configs", str(external / "cast-movement"), "--out", str(OUT / "browser"),
            "--cwd", str(external), "--port", str(args.browser_port),
        ], external, runtime_env)
        for path, destination in hidden:
            destination.rename(path)
        run("final-integrity", ["python3", "verify_bundle.py"], bundle)

    extract_crate(package["archive"], OUT / "candidate", package["rootName"])
    report = {
        "passed": True, "archiveSha256": package["sha256"], "binarySha256": package["binarySha256"],
        "sourceCrateSha256": package["sourceCrateSha256"], "outsideCheckout": True, "sourceFixturesHidden": True,
        "emptyCargoHome": True, "offline": True, "offlineTests": names, "rustConsumerBuilt": True,
        "reusedNonBrowserStage": args.reuse_non_browser,
        "legacy45": {"members": 8, "allSkillsCommitted": True, "phase": legacy["phase"], "pending": legacy["pending"]},
        "logs": {name: digest(DELIVERY / f"{name}.log") for name in [
            "integrity", "offline-movement-tests", "offline-rust-consumer", "ordinary-info",
            "public-movement", "legacy-mechanism-45", "browser", "final-integrity",
        ]},
    }
    (DELIVERY / "independent-verification.json").write_text(json.dumps(report, indent=2) + "\n")
    print("Packaged source, mechanism 46 and mechanism 45 consumers passed", flush=True)


if __name__ == "__main__":
    main()
