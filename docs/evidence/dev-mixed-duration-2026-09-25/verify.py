#!/usr/bin/env python3
"""Offline replay of the scoped mixed-duration CPU evidence, not hardware proof."""
import gzip
import hashlib
import io
import json
from pathlib import Path
import re
import sys
import tarfile

HERE = Path(__file__).resolve().parent


def need(value, message):
    if not value:
        raise ValueError(message)


def read(path):
    return json.loads(path.read_text())


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
    archive = root / "source.tar.gz"
    need(hashlib.sha256(archive.read_bytes()).hexdigest() == read(root / "archive.json")["sha256"], "archive identity")
    with tarfile.open(archive, "r:gz") as packed:
        members = {member.name: member for member in packed.getmembers() if member.isfile()}
        need(members.keys() == before["inputs"].keys(), "complete signed source archive")
        for name, facts in before["inputs"].items():
            raw = packed.extractfile(members[name]).read()
            need(hashlib.sha256(raw).hexdigest() == facts["sha256"], "archive sha256: " + name)
            need(hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == facts["git_blob"], "archive blob: " + name)
    results = read(root / "results.json")
    expected_phases = {"runner-tests", "fixture-rebuild", "short-disassembly", "long-disassembly", "rustc",
                       "gnu-build", "gnu", "musl-build", "musl", "doctests"}
    need(results.keys() == expected_phases and all(row["status"] == 0 and row["passed"] == (name != "doctests")
         for name, row in results.items()), "original CPU phase outcomes, including false rejection")
    records = {path.parent.name: read(path) for path in root.glob("*/record.json")}
    need(records.keys() == expected_phases | {"signature", "source-archive", "gnu-roster", "musl-roster"}, "exact command roster")
    need(all(row["status"] == 0 and row["group_absent"] for row in records.values()), "successful reaped commands")
    for platform in ("gnu", "musl"):
        metadata = read(root / (platform + "-runtime-tests.json"))
        compressed = (root / (platform + "-runtime-tests.gz")).read_bytes()
        check_elf(compressed, metadata, results[platform], records[platform])
        need(results[platform]["counts"] == [[1423, 0, 24, 0, 0]], "exact CPU counts")
        stdout = (root / platform / "stdout.log").read_text()
        need(re.findall(r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", stdout, re.M)
             == [("1423", "0", "24", "0", "0")], "actual test summary")
    need(results["doctests"]["counts"] == [[4, 0, 0, 0, 0], [42, 0, 0, 0, 0]], "original passing split doctests")
    continuation = root.parent / "cpu-completion-v1"
    opening = read(continuation / "inputs-before.json")
    need(opening == read(continuation / "inputs-after.json"), "continuation archive/helper continuity")
    need(opening["source_commit"] == before["commit"] and opening["source_inputs"] == before["inputs"], "same runtime source archive")
    continued = read(continuation / "results.json")
    expected = {"runner-tests", "replay-tests", "doctests", "default", "clippy", "format"}
    need(continued.keys() == expected and all(row["passed"] and row["status"] == 0 for row in continued.values()), "all continuation phases")
    continued_records = {path.parent.name: read(path) for path in continuation.glob("*/record.json")}
    need(continued_records.keys() == expected | {"signature"}, "exact continuation command roster")
    need(all(row["status"] == 0 and row["group_absent"] for row in continued_records.values()), "continuation commands reaped")
    need(continued["doctests"]["counts"] == [[4, 0, 0, 0, 0], [42, 0, 0, 0, 0]], "rerun split doctests")
    return dict(commit=before["commit"], source_inputs=len(before["inputs"]), command_count=len(records),
                continuation_command_count=len(continued_records), helper_commit=opening["helper_commit"],
                gnu_passed=1423, musl_passed=1423, hardware_ignored_per_platform=24, native_executed=False)


if __name__ == "__main__":
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    root = Path(sys.argv[1]) if len(sys.argv) == 2 else HERE / "retained/signed-cpu-v2"
    print(json.dumps(verify(root), sort_keys=True))
