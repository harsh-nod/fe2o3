#!/usr/bin/env python3
"""Join the signed-source archive, remote source inventory, Cargo artifact and CPU-tested ELF."""
import gzip
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tarfile

SOURCE = "dda812658d8827e7d508389c9650228844d0f503"
ELF_SHA = "0ac04fa80c5a042670a6aa40d190d4dd7f287c56a2ba7c21d213f01241e76767"
ELF_BYTES = 39105264
REPO = Path(__file__).resolve().parents[3]
BUILD_ROOT = "/home/harsh/.codex-tmp/fe2o3-native-isolated-build-20260928.oNijXsc5"
BUILD_ENV = {"HOME": "/home/harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin", "LANG": "C", "LC_ALL": "C",
             "RUSTUP_TOOLCHAIN": "nightly-2026-04-03", "CARGO_HOME": BUILD_ROOT + "/cargo-home", "TMPDIR": BUILD_ROOT + "/tmp",
             "CARGO_TARGET_DIR": BUILD_ROOT + "/target", "CARGO_BUILD_JOBS": "2", "CARGO_INCREMENTAL": "0",
             "CARGO_TERM_COLOR": "never", "CARGO_PROFILE_DEV_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG": "0",
             "CARGO_PROFILE_TEST_OPT_LEVEL": "1", "CARGO_PROFILE_TEST_DEBUG_ASSERTIONS": "true",
             "CARGO_PROFILE_TEST_OVERFLOW_CHECKS": "true"}


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary build input")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify(raw):
    signers = REPO / "docs/evidence/dev-mixed-observers-2026-09-25/raw/allowed-signers"
    need(sha(signers) == "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b", "expected source signer")
    env = {"PATH": "/usr/bin:/bin", "HOME": "/home/harsh", "LANG": "C", "LC_ALL": "C",
           "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1"}
    git = ["git", "--no-replace-objects", "-c", "gpg.format=ssh", "-c", "gpg.ssh.program=/usr/bin/ssh-keygen"]
    subprocess.run([*git, "-c", "gpg.ssh.allowedSignersFile=" + str(signers), "verify-commit", SOURCE],
                   cwd=REPO, env=env, check=True, capture_output=True, timeout=30)
    archive = subprocess.run([*git, "archive", "--format=tar", SOURCE, "Cargo.toml", "Cargo.lock",
                              "rust-toolchain.toml", "crates", "examples"], cwd=REPO, env=env,
                             check=True, capture_output=True, timeout=120).stdout
    need(hashlib.sha256(archive).hexdigest() == sha(raw / "source.tar"), "archive equals signed Git source")
    need((raw / "source.commit").read_text() == SOURCE + "\n", "exact exported source commit")
    need((raw / "build-remote.status").read_text() == "0\n", "successful remote build and CPU controller")
    inventory = json.loads((raw / "build-inventory.json").read_text())
    source = {}
    with tarfile.open(raw / "source.tar", "r:") as archive:
        for member in archive:
            if member.isdir():
                continue
            need(member.isfile() and member.name not in source, "ordinary unique signed source member")
            source[member.name] = hashlib.sha256(archive.extractfile(member).read()).hexdigest()
    need(source == inventory["source"], "remote source unchanged after build and CPU tests")
    results = raw / "build-results"
    measured = {str(path.relative_to(results)): sha(path) for path in results.rglob("*") if path.is_file()}
    need(measured == inventory["results"], "byte-exact complete build collection")
    need(sha(raw / "source.tar") == (raw / "source.sha256").read_text().split()[0]
         == (results / "input-sha256").read_text().splitlines()[0].split()[0], "local/remote source archive equality")
    for name in ("build", "extract", "elf", "list", "cpu"):
        need((results / (name + ".status")).read_text() == "0\n", "passed build gate: " + name)
    binary = BUILD_ROOT + "/results/runtime-tests"
    commands = {
        "build": ["/usr/bin/timeout", "--signal=TERM", "--kill-after=10s", "900s", "env", "-i",
                  *[key + "=" + value for key, value in BUILD_ENV.items()], "cargo", "test", "--frozen", "-p",
                  "fe2o3-runtime", "--no-default-features", "--features", "scale-qualification", "--lib", "--target",
                  "x86_64-unknown-linux-musl", "--no-run", "--message-format=json"],
        "extract": ["/usr/bin/python3", "-I", "-B", BUILD_ROOT + "/extract.py", BUILD_ROOT],
        "elf": ["/usr/bin/readelf", "-h", "-l", "-d", binary], "list": [binary, "--list"],
        "cpu": ["/usr/bin/timeout", "--signal=TERM", "--kill-after=10s", "600s", "env", "-i", "HOME=/home/harsh",
                "PATH=/usr/bin:/bin", "LANG=C", "LC_ALL=C", "TMPDIR=" + BUILD_ROOT + "/tmp", binary,
                "--test-threads=2", "--color=never"]}
    for name, argv in commands.items():
        need(shlex.split((results / (name + ".command")).read_text()) == argv, "exact build command: " + name)
    artifact = json.loads((results / "executable.json").read_text())
    need(artifact["sha256"] == ELF_SHA and artifact["bytes"] == ELF_BYTES, "fixed executed ELF identity")
    with gzip.open(results / "runtime-tests.gz", "rb") as retained:
        data = retained.read(ELF_BYTES + 1)
    need(len(data) == ELF_BYTES and hashlib.sha256(data).hexdigest() == ELF_SHA, "retained executed ELF bytes")
    records = [json.loads(line) for line in (results / "build.stdout").read_text().splitlines()]
    matches = [row for row in records if row.get("reason") == "compiler-artifact"
               and row["target"]["name"] == "fe2o3_runtime" and row["profile"]["test"] is True
               and row.get("executable") is not None]
    need(matches == [artifact["artifact"]] and records[-1] == {"reason": "build-finished", "success": True},
         "exact Cargo test artifact")
    roster_text = (results / "list.stdout").read_text()
    roster = re.findall(r"^(\S+): test$", roster_text, re.M)
    need(len(roster) == len(set(roster)) == 1792 and roster_text.endswith("1792 tests, 0 benchmarks\n"), "exact ELF roster")
    cpu = (results / "cpu.stdout").read_text()
    outcomes = re.findall(r"^test (\S+) \.\.\. (ok|ignored)(?:,.*)?$", cpu, re.M)
    need(len(outcomes) == len(roster) and {name for name, _ in outcomes} == set(roster), "CPU tests cover exact ELF roster")
    need(sum(state == "ok" for _, state in outcomes) == 1763 and sum(state == "ignored" for _, state in outcomes) == 29,
         "CPU outcomes")
    summaries = re.findall(r"^test result:.*$", cpu, re.M)
    need(len(summaries) == 1 and re.fullmatch(
        r"test result: ok\. 1763 passed; 0 failed; 29 ignored; 0 measured; 0 filtered out; finished in [0-9]+\.[0-9]+s", summaries[0]),
        "successful complete CPU summary")
    return {"commit": SOURCE, "elf_sha256": ELF_SHA, "elf_bytes": ELF_BYTES, "tests": 1792,
            "cpu_passes": 1763, "ignored": [name for name, state in outcomes if state == "ignored"],
            "source_files": len(source), "filtered": 1791, "features": "scale-qualification",
            "target": "x86_64-unknown-linux-musl", "environment": BUILD_ENV}


if __name__ == "__main__":
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    print(json.dumps(verify(Path(sys.argv[1])), sort_keys=True, indent=2))
