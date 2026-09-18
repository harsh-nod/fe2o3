#!/usr/bin/env python3
"""Validate the bounded CPU receipt, not hardware behavior or formal refinement."""

from datetime import datetime
import hashlib
import json
from pathlib import Path
import re

archive = Path(__file__).resolve().parent
root = archive.parents[2]
names = (
    "gnu-example",
    "source-before",
    "gnu-example-final",
    "musl-example",
    "rustc-version",
    "cargo-version",
    "gnu-frontier",
    "gnu-runtime",
    "musl-runtime",
    "clippy",
    "legacy-isolation",
    "fmt",
    "python-lint",
    "source-after",
    "binaries",
    "source-base",
)


def digest_manifest(text):
    entries = {}
    for line in text.splitlines():
        digest, name = line.split("  ", 1)
        assert re.fullmatch(r"[0-9a-f]{64}", digest) and name not in entries
        entries[name] = digest
    return entries


records = {}
for name in names:
    for suffix in ("command", "started", "finished", "log", "exit"):
        assert (archive / f"raw/{name}.{suffix}").is_file()
    assert (archive / f"raw/{name}.exit").read_text() == "0\n", name
    started = (archive / f"raw/{name}.started").read_text().strip()
    finished = (archive / f"raw/{name}.finished").read_text().strip()
    assert datetime.fromisoformat(started) <= datetime.fromisoformat(finished)
    records[name] = {
        "started": started,
        "finished": finished,
        "command": (archive / f"raw/{name}.command").read_text().strip(),
    }
sources = digest_manifest((archive / "raw/source-before.log").read_text())
assert len(sources) == 11
for path, digest in sources.items():
    assert hashlib.sha256((root / path).read_bytes()).hexdigest() == digest, path
assert (archive / "raw/source-after.log").read_text().splitlines() == [
    f"{path}: OK" for path in sources
]
for name in (
    "gnu-example-final",
    "musl-example",
    "gnu-frontier",
    "gnu-runtime",
    "musl-runtime",
    "clippy",
):
    assert records["source-before"]["finished"] <= records[name]["started"]
    assert records[name]["finished"] <= records["source-after"]["started"]
assert (
    archive / "raw/source-base.log"
).read_text().strip() == "3d2c93a748bfbde7e37093376d121eea67258649"


def parse_rows(text):
    # Serial abort tests leave the parent status after child-harness banners.
    return re.findall(
        r"^test (\S+) \.\.\. (?:\nrunning 1 test\n(?:\nrunning 1 test\n)*)?(ok|ignored)(?:,[^\n]*)?$",
        text,
        re.M,
    )


def test_parse_rows():
    assert parse_rows("test a ... ok\ntest b ... ignored, native only\n") == [
        ("a", "ok"),
        ("b", "ignored"),
    ]
    for children in (1, 2, 5):
        prefix = "test parent ... \nrunning 1 test\n" + "\nrunning 1 test\n" * (children - 1)
        assert parse_rows(prefix + "ok\n") == [("parent", "ok")]
        for tail in ("", "FAILED\n", "unexpected\nok\n", "test nested ... ok\n"):
            assert ("parent", "ok") not in parse_rows(prefix + tail)
    assert parse_rows("test incomplete ... \ntest following ... ok\n") == [
        ("following", "ok")
    ]


test_parse_rows()


def roster(name, passed, ignored):
    text = (archive / f"raw/{name}.log").read_text()
    expected = f"test result: ok. {passed} passed; 0 failed; {ignored} ignored; 0 measured; 0 filtered out;"
    assert len([line for line in text.splitlines() if line.startswith(expected)]) == 1
    rows = parse_rows(text)
    assert len(rows) == passed + ignored and len(dict(rows)) == len(rows)
    assert sum(status == "ok" for _, status in rows) == passed
    return sorted(rows)


roster("gnu-example", 12, 0)
example_roster = roster("gnu-example-final", 13, 0)
assert example_roster == roster("musl-example", 13, 0)
runtime_roster = roster("gnu-runtime", 1057, 17)
assert runtime_roster == roster("musl-runtime", 1057, 17)
frontier = "kfd_backend::tests::scripted_sdma_wait_completion_leaves_continuation_for_explicit_flush"
assert (frontier, "ok") in runtime_roster
assert f"test {frontier} ... ok" in (archive / "raw/gnu-frontier.log").read_text()
assert (
    "1 passed; 0 failed; 0 ignored; 0 measured; 1073 filtered out;"
    in (archive / "raw/gnu-frontier.log").read_text()
)
binaries = digest_manifest((archive / "raw/binaries.log").read_text())
expected_paths = []
for name in ("gnu-example-final", "musl-example", "gnu-runtime", "musl-runtime"):
    paths = re.findall(
        r"Running unittests .* \(([^)]+)\)", (archive / f"raw/{name}.log").read_text()
    )
    assert len(paths) == 1
    expected_paths.extend(paths)
assert list(binaries) == expected_paths
assert records["source-after"]["finished"] <= records["binaries"]["started"]
legacy = (archive / "raw/legacy-isolation.log").read_text()
assert "exact_opt_in_insertions=6 legacy_mutations_rejected=3\n" in legacy
print(
    json.dumps(
        {
            "scope": "CPU qualification only; no native timing or new formal proof",
            "source_files": sources,
            "binary_hash_observations": binaries,
            "example_passed_per_target": 13,
            "runtime_passed_per_target": 1057,
            "runtime_ignored_per_target": 17,
            "parser_regressions": "inline, ignored, child banners, missing/failed/interleaved parent status",
            "records": records,
        },
        indent=2,
    )
)
