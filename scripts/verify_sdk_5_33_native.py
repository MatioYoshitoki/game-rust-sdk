"""Verify the workspace before verifying the delivered ordinary runtime."""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(os.environ.get("SDK_5_33_EVIDENCE_DIR", ROOT / "artifacts/sdk-5-33")).resolve()
PREVIOUS = Path(os.environ.get("SDK_5_32_REFERENCE_DIR", ROOT / "artifacts/sdk-5-32")).resolve()


def tests(log):
    return set(re.findall(r"^test (\S+) \.\.\. ok$", log.read_text(), re.M))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    sources = [*ROOT.glob("src/**/*.rs"), *ROOT.glob("src/**/*.json"), ROOT / "Cargo.toml", ROOT / "Cargo.lock"]
    source_hashes = {str(path.relative_to(ROOT)): digest(path) for path in sources}
    checks = {
        "default": ["cargo", "test", "--offline", "--locked"],
        "all-features": ["cargo", "test", "--offline", "--locked", "--all-features"],
        "clippy": ["cargo", "clippy", "--offline", "--locked", "--all-targets", "--all-features", "--", "-D", "warnings"],
        "fmt": ["cargo", "fmt", "--check"],
        "node-tests": ["node", "--test", *map(str, sorted(ROOT.glob("adapters/*/*.test.mjs")))],
    }
    for name, command in checks.items():
        with (OUT / f"{name}.log").open("w") as log:
            subprocess.run(command, cwd=ROOT, stdout=log, stderr=subprocess.STDOUT,
                           env={**os.environ, "RUST_TEST_THREADS": os.environ.get("RUST_TEST_THREADS", "6")}, timeout=1200, check=True)
        print(f"{name}: passed", flush=True)
    old = tests(PREVIOUS / "default.log")
    current = tests(OUT / "default.log")
    assert len(old) == 1525 and old <= current and current == tests(OUT / "all-features.log")
    added = current - old
    assert len(added) == 6 and all("::projectile_recast::" in name or "::recast_tests::" in name for name in added)
    node_names = lambda log: set(re.findall(r"^# Subtest: (.+)$", log.read_text(), re.M))
    old_node = node_names(PREVIOUS / "node-tests.log")
    node = node_names(OUT / "node-tests.log")
    assert len(old_node) == 100 and old_node <= node and len(node) == 102
    assert all(digest(ROOT / path) == sha for path, sha in source_hashes.items())
    report = {
        "passed": True, "rustTests": len(current), "retainedRustTests": len(old),
        "newTests": sorted(added), "nodeTests": len(node), "sameDefaultAndAllFeatures": True,
        "sourceHashes": source_hashes,
        "logs": {name: digest(OUT / f"{name}.log") for name in checks},
    }
    (OUT / "native-verification.json").write_text(json.dumps(report, indent=2) + "\n")


if __name__ == "__main__":
    main()
