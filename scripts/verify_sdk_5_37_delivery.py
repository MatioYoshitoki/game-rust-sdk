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

OUT = Path(os.environ.get("SDK_5_37_EVIDENCE_DIR", ROOT / "artifacts/sdk-5-37")).resolve()
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
    parser.add_argument("--browser-port", type=int, default=4247)
    args = parser.parse_args()
    package = json.loads((DELIVERY / "package.json").read_text())
    assert digest(package["archive"]) == package["sha256"]
    with tempfile.TemporaryDirectory(prefix="sdk-5-37-consume-") as directory:
        work = Path(directory)
        bundle = work / "bundle"
        extract_crate(package["archive"], bundle, package["rootName"])
        run("integrity", ["python3", "verify_bundle.py"], bundle)
        binary = bundle / "bin/kernel-host"
        assert digest(binary) == package["binarySha256"]
        sources = [*ROOT.glob("src/**/*.rs"), *ROOT.glob("src/**/*.json")]
        for source in sources:
            assert digest(source) == digest(bundle / "sdk" / source.relative_to(ROOT)), source

        cargo_home = work / "empty-cargo-home"
        cargo_home.mkdir()
        offline = {"CARGO_HOME": str(cargo_home), "CARGO_NET_OFFLINE": "true",
                   "CARGO_TARGET_DIR": str(work / "target"),
                   "CARGO_PROFILE_TEST_DEBUG": "0", "CARGO_PROFILE_DEV_DEBUG": "0"}
        source_log = run("offline-locked-basic-tests", [
            "cargo", "test", "--offline", "--locked", "--manifest-path", "sdk/Cargo.toml", "locked_basic",
        ], bundle, offline)
        names = sorted(name for name in re.findall(r"^test (\S+) \.\.\. ok$", source_log, re.M)
                       if "::locked_basic_target::" in name or "::locked_basic_tests::" in name)
        assert len(names) == 6
        run("offline-rust-consumer", ["cargo", "build", "--offline", "--locked",
                                    "--manifest-path", "consumer/Cargo.toml"], bundle, offline)

        external = work / "external"
        external.mkdir()
        for profile in ["locked-basic-target", "basic-followup"]:
            shutil.copytree(bundle / "samples" / profile, external / profile)
        hidden = []
        for path in [bundle / "sdk/config", bundle / "sdk/examples", bundle / "samples"]:
            destination = work / ("hidden-" + path.name)
            path.rename(destination)
            hidden.append((path, destination))
        runtime_env = {"MATCH_CONFIG_MANIFEST": str(external / "absent-default.json")}
        support = json.loads(run("ordinary-info", [str(binary), "--sdk-info"], external, runtime_env))
        assert support["crateVersion"] == "0.39.0" and support["mechanismVersions"]["max"] == 54
        assert support["lockedBasicTarget"]["minimumMechanismVersion"] == 54
        assert not support["clocks"]["replay"]
        run("public-locked-basic", [
            "node", str(ROOT / "scripts/verify_sdk_5_37_public.mjs"), str(binary),
            str(bundle / "hosts/node/two-member-host.mjs"), str(external / "locked-basic-target"), str(OUT / "public"),
        ], external, runtime_env)
        run("legacy-mechanism-53", [
            "node", str(ROOT / "scripts/verify_sdk_5_36_public.mjs"), str(binary),
            str(bundle / "hosts/node/two-member-host.mjs"), str(external / "basic-followup"), str(OUT / "legacy53"),
        ], external, runtime_env)
        legacy = json.loads((OUT / "legacy53/report.json").read_text())
        assert legacy["passed"] and legacy["binarySha256"] == package["binarySha256"]
        assert [row["members"] for row in legacy["cases"]] == [4, 8]
        assert all(row["phase"] == "closed" and row["pending"] == 0 for row in legacy["cases"])
        run("browser", [
            "python3", str(ROOT / "scripts/verify_sdk_5_37_browser.py"),
            "--server", str(bundle / "hosts/h5/skill-hp-cost-server.mjs"), "--binary", str(binary),
            "--configs", str(external / "locked-basic-target"), "--out", str(OUT / "browser"),
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
        "legacy53": {"cases": legacy["cases"], "reportSha256": digest(OUT / "legacy53/report.json")},
        "logs": {name: digest(DELIVERY / f"{name}.log") for name in [
            "integrity", "offline-locked-basic-tests", "offline-rust-consumer", "ordinary-info",
            "public-locked-basic", "legacy-mechanism-53", "browser", "final-integrity",
        ]},
    }
    (DELIVERY / "independent-verification.json").write_text(json.dumps(report, indent=2) + "\n")
    print("Packaged source, mechanism 54 and mechanism 53 consumers passed", flush=True)


if __name__ == "__main__":
    main()
