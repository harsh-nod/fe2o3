#!/usr/bin/env python3
"""Offline replay of the scoped mixed-duration CPU evidence, not hardware proof."""
import gzip
import hashlib
import io
import json
from pathlib import Path, PurePosixPath
import re
import subprocess
import sys
import tarfile

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
PREFIXES = ["crates", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "examples", "tests",
            "docs/evidence/dev-mixed-duration-2026-09-25/run.py", "docs/evidence/dev-mixed-duration-2026-09-25/test_run.py"]


def need(value, message):
    if not value:
        raise ValueError(message)


def read(path):
    return json.loads(path.read_text())


def check_members(entries):
    names, files = set(), {}
    for entry in entries:
        name = entry.name.rstrip("/")
        path = PurePosixPath(name)
        need(name == str(path) and not path.is_absolute() and ".." not in path.parts and name not in ("", "."), "safe archive path")
        need(name not in names and (entry.isdir() or entry.isfile()), "unique ordinary archive entry")
        names.add(name)
        if entry.isfile():
            files[name] = entry
    return files


def check_command(record, expected):
    need(record["command"] == expected, "exact command arguments")


def raw_counts(stdout):
    return [list(map(int, row)) for row in re.findall(
        r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", stdout, re.M)]


def authenticate_source(root, before):
    signer = root / "allowed-signers"
    need(hashlib.sha256(signer.read_bytes()).hexdigest() == "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b", "trusted signer identity")
    need(re.fullmatch(r"[0-9a-f]{40}", before["commit"]), "exact commit identity")
    env = dict(HOME="/home/harsh", PATH="/usr/bin:/bin", LC_ALL="C", GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null")
    base = ["/usr/bin/git", "--no-replace-objects"]
    subprocess.run([*base, "-c", "gpg.format=ssh", "-c", "gpg.ssh.program=/usr/bin/ssh-keygen", "-c",
                    "gpg.ssh.allowedSignersFile=" + str(signer.resolve()), "verify-commit", before["commit"]],
                   cwd=REPO, env=env, check=True, capture_output=True, timeout=30)
    entries = subprocess.check_output([*base, "ls-tree", "-r", "-z", before["commit"], "--", *PREFIXES], cwd=REPO, env=env, timeout=30)
    signed = {}
    for entry in entries.split(b"\0"):
        if entry:
            header, name = entry.split(b"\t", 1)
            mode, kind, oid = header.split()
            need(mode in (b"100644", b"100755") and kind == b"blob", "signed ordinary source")
            signed[name.decode()] = oid.decode()
    need(signed == {name: facts["git_blob"] for name, facts in before["inputs"].items()}, "complete authenticated source tree")


def check_elf(compressed, metadata, result, record):
    need(0 < metadata["bytes"] <= 512 * 1024 * 1024, "bounded retained ELF")
    need(hashlib.sha256(compressed).hexdigest() == metadata["gzip_sha256"], "retained compressed ELF")
    with gzip.GzipFile(fileobj=io.BytesIO(compressed)) as source:
        raw = source.read(metadata["bytes"] + 1)
    digest = hashlib.sha256(raw).hexdigest()
    need(raw.startswith(b"\x7fELF") and len(raw) == metadata["bytes"], "retained ELF extent")
    need(digest == metadata["sha256"] == result["sha256"], "retained and executed ELF are identical")
    need(record["command"] == [metadata["path"], "--test-threads=2"], "direct retained ELF execution")


def verify(root):
    before = read(root / "source-before.json")
    need(before == read(root / "source-after.json"), "signed source continuity")
    authenticate_source(root, before)
    archive = root / "source.tar.gz"
    need(hashlib.sha256(archive.read_bytes()).hexdigest() == read(root / "archive.json")["sha256"], "archive identity")
    with tarfile.open(archive, "r:gz") as packed:
        members = check_members(packed.getmembers())
        need(members.keys() == before["inputs"].keys(), "complete signed source archive")
        for name, facts in before["inputs"].items():
            raw = packed.extractfile(members[name]).read()
            need(hashlib.sha256(raw).hexdigest() == facts["sha256"], "archive sha256: " + name)
            need(hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == facts["git_blob"], "archive blob: " + name)
    results = read(root / "results.json")
    expected_phases = {"runner-tests", "fixture-rebuild", "short-disassembly", "long-disassembly", "rustc",
                       "gnu-build", "gnu", "musl-build", "musl", "doctests", "default", "clippy", "format"}
    need(results.keys() == expected_phases and all(row["status"] == 0 and row["passed"] is True
         for row in results.values()), "complete successful CPU phases")
    records = {path.parent.name: read(path) for path in root.glob("*/record.json")}
    need(records.keys() == expected_phases | {"signature", "source-archive", "gnu-roster", "musl-roster"}, "exact command roster")
    need(all(row["status"] == 0 and row["group_absent"] for row in records.values()), "successful reaped commands")
    for platform in ("gnu", "musl"):
        metadata = read(root / (platform + "-runtime-tests.json"))
        compressed = (root / (platform + "-runtime-tests.gz")).read_bytes()
        check_elf(compressed, metadata, results[platform], records[platform])
        need(results[platform]["counts"] == [[1423, 0, 24, 0, 0]], "exact CPU counts")
        stdout = (root / platform / "stdout.log").read_text()
        need(raw_counts(stdout) == [[1423, 0, 24, 0, 0]], "actual test summary")
    expected_docs = [[4, 0, 0, 0, 0], [42, 0, 0, 0, 0]]
    need(results["doctests"]["counts"] == raw_counts((root / "doctests/stdout.log").read_text()) == expected_docs, "raw split doctests")
    cargo = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-runtime"]
    expected_commands = {
        "gnu-build": [*cargo, "--all-features", "--lib", "--no-run", "--message-format=json"],
        "musl-build": [*cargo, "--all-features", "--lib", "--target", "x86_64-unknown-linux-musl", "--no-run", "--message-format=json"],
        "doctests": [*cargo, "--all-features", "--doc"],
        "default": ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime"],
        "clippy": ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"],
        "format": ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"],
    }
    for name, command in expected_commands.items():
        check_command(records[name], command)
    return dict(commit=before["commit"], source_inputs=len(before["inputs"]), command_count=len(records),
                gnu_passed=1423, musl_passed=1423, hardware_ignored_per_platform=24, native_executed=False)


if __name__ == "__main__":
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    root = Path(sys.argv[1]) if len(sys.argv) == 2 else HERE / "retained/signed-cpu-v3"
    print(json.dumps(verify(root), sort_keys=True))
