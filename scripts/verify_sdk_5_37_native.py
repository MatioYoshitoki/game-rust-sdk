"""Verify the workspace before verifying the delivered ordinary runtime."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(os.environ.get("SDK_5_37_EVIDENCE_DIR", ROOT / "artifacts/sdk-5-37")).resolve()
PREVIOUS = Path(os.environ.get("SDK_5_36_REFERENCE_DIR", ROOT / "artifacts/sdk-5-36")).resolve()


def tests(log):
    return set(re.findall(r"^test (\S+) \.\.\. ok$", log.read_text(), re.M))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--resume-default", action="store_true")
    args = parser.parse_args()
    sources = [*ROOT.glob("src/**/*.rs"), *ROOT.glob("src/**/*.json"), ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    source_hashes = {str(path.relative_to(ROOT)): digest(path) for path in sources}
    old = tests(PREVIOUS / "default.log")
    added = {name for name in tests(OUT / "targeted.log")
             if "::locked_basic_target::" in name or "::locked_basic_tests::" in name}
    assert len(old) == 1549 and len(added) == 6 and not old.intersection(added)
    expected = old | added
    logs = {}
    reruns = {}

    def run(name, command, threads=None, no_debug=False):
        path = OUT / f"{name}.log"
        env = {**os.environ, "RUST_TEST_THREADS": threads or os.environ.get("RUST_TEST_THREADS", "4")}
        if no_debug:
            env.update(CARGO_PROFILE_TEST_DEBUG="0")
        with path.open("w") as log:
            result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                                    env=env, timeout=2400)
        logs[name] = digest(path)
        return result.returncode

    def suite(name, options, resume=False):
        command = ["cargo", "test", "--offline", "--locked", *options]
        status = 0
        if resume:
            logs[name] = digest(OUT / f"{name}.log")
        else:
            # Ordinary-clock stress tests have their own wall-time thresholds.
            isolated = [test for test in sorted(expected) if "hundred" in test]
            skip = [flag for test in isolated for flag in ("--skip", test)]
            status = run(name, [*command, "--", *skip], no_debug=True)
        passed = tests(OUT / f"{name}.log")
        assert passed <= expected
        missing = sorted(expected - passed)
        assert status == 0 or missing
        if missing:
            retry = name + "-retry"
            assert run(retry, [*command, "--lib", "--", "--exact", *missing],
                       threads="1" if len(missing) <= 12 else "3", no_debug=bool(options)) == 0
            assert tests(OUT / f"{retry}.log") == set(missing)
            reruns[name] = missing
            passed.update(missing)
            for target in ("bins", "doc"):
                assert run(f"{name}-{target}", [*command, f"--{target}"], no_debug=bool(options)) == 0
        assert passed == expected
        print(f"{name}: passed ({len(missing)} focused reruns)", flush=True)
        return passed

    current = suite("default", [], resume=args.resume_default)
    assert current == suite("all-features", ["--all-features"])
    checks = {
        "clippy": ["cargo", "clippy", "--offline", "--locked", "--all-targets", "--all-features", "--", "-D", "warnings"],
        "fmt": ["cargo", "fmt", "--check"],
        "node-tests": ["node", "--test", *map(str, sorted(ROOT.glob("adapters/*/*.test.mjs")))],
    }
    for name, command in checks.items():
        assert run(name, command) == 0
        print(f"{name}: passed", flush=True)
    node_names = lambda log: set(re.findall(r"^# Subtest: (.+)$", log.read_text(), re.M))
    old_node = node_names(PREVIOUS / "node-tests.log")
    node = node_names(OUT / "node-tests.log")
    assert len(old_node) == 108 and old_node <= node and len(node) == 110
    assert all(digest(ROOT / path) == sha for path, sha in source_hashes.items())
    report = {
        "passed": True, "rustTests": len(current), "retainedRustTests": len(old),
        "newTests": sorted(added), "nodeTests": len(node), "sameDefaultAndAllFeatures": True,
        "sourceHashes": source_hashes,
        "logs": logs, "focusedReruns": reruns,
    }
    (OUT / "native-verification.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
