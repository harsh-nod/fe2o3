#!/usr/bin/env python3
"""Supplement the sealed CPU verifier with complete serial-harness consumption."""

import hashlib
import json
from pathlib import Path
import re

ARCHIVE = Path(__file__).resolve().parent
CPU = ARCHIVE.parent / "dev-kfd-copy-progress-cpu-2026-09-18"
ABORTS = {
    "kfd_backend::tests::runtime_compute_pipeline_drop_aborts_for_every_live_logical_phase": 5,
    "kfd_backend::tests::scripted_sdma_drop_still_aborts_with_live_or_terminal_custody": 2,
}


def parse(text, passed, ignored, filtered=0):
    lines = text.splitlines()
    cursor = 0
    while cursor < len(lines) and not lines[cursor].startswith("running "):
        line = lines[cursor]
        assert not line.strip() or re.fullmatch(
            r"   Compiling .+|    Finished `test` profile .+|     Running unittests .+",
            line,
        ), ("unexpected prelude", line)
        cursor += 1
    count = passed + ignored
    assert lines[cursor] == f"running {count} test{'s' if count != 1 else ''}"
    cursor += 1
    rows = {}
    for _ in range(count):
        line = lines[cursor]
        cursor += 1
        ordinary = re.fullmatch(r"test (\S+) \.\.\. (ok|ignored)(?:, .+)?", line)
        if ordinary:
            name, status = ordinary.groups()
            assert name not in ABORTS, (
                "abort parent must retain its exact child banners"
            )
        else:
            parent = re.fullmatch(r"test (\S+) \.\.\. ", line)
            assert parent and parent[1] in ABORTS, ("unexpected outcome", line)
            name, status = parent[1], "ok"
            for child in range(ABORTS[name]):
                if child:
                    assert lines[cursor] == ""
                    cursor += 1
                assert lines[cursor] == "running 1 test"
                cursor += 1
            assert lines[cursor] == "ok"
            cursor += 1
        assert name not in rows
        rows[name] = status
    assert lines[cursor] == ""
    cursor += 1
    assert re.fullmatch(
        rf"test result: ok\. {passed} passed; 0 failed; {ignored} ignored; "
        rf"0 measured; {filtered} filtered out; finished in [0-9]+\.[0-9]+s",
        lines[cursor],
    )
    assert all(not line.strip() for line in lines[cursor + 1 :]), "trailing payload"
    assert sum(value == "ok" for value in rows.values()) == passed
    assert sum(value == "ignored" for value in rows.values()) == ignored
    return rows


def main():
    manifest = CPU / "SHA256SUMS"
    assert hashlib.sha256(manifest.read_bytes()).hexdigest() == (
        "044d073bd0bd4ad51e19f9f267e2c14f22cdab5d24e9ea8b5c6fe1d6ef58e87a"
    )
    members = set()
    for line in manifest.read_text().splitlines():
        digest, name = line.split("  ", 1)
        assert name.startswith("./") and ".." not in Path(name).parts
        assert name not in members
        members.add(name)
        assert hashlib.sha256((CPU / name).read_bytes()).hexdigest() == digest
    assert members == {
        "./" + str(path.relative_to(CPU))
        for path in CPU.rglob("*")
        if path.is_file() and path != manifest
    }
    assert not any(path.is_symlink() for path in CPU.rglob("*"))
    rosters = {}
    for name, passed, ignored, filtered in (
        ("gnu-example", 12, 0, 0),
        ("gnu-example-final", 13, 0, 0),
        ("musl-example", 13, 0, 0),
        ("gnu-runtime", 1057, 17, 0),
        ("musl-runtime", 1057, 17, 0),
        ("gnu-frontier", 1, 0, 1073),
    ):
        assert (CPU / f"raw/{name}.exit").read_text() == "0\n"
        rosters[name] = parse(
            (CPU / f"raw/{name}.log").read_text(), passed, ignored, filtered
        )
    assert rosters["gnu-example-final"] == rosters["musl-example"]
    assert rosters["gnu-runtime"] == rosters["musl-runtime"]
    text = (CPU / "raw/gnu-runtime.log").read_text()
    first_parent = next(iter(ABORTS))
    mutations = (
        text + "test unexpected ... FAILED\n",
        text + "test result: FAILED.\n",
        text + "running 0 tests\n",
        text + "unparsed payload\n",
        text.replace("test " + first_parent, "test unexpected_parent"),
        text.replace("\nrunning 1 test\n", "\nrunning 2 tests\n", 1),
        text.replace("\nrunning 1 test\n", "", 1),
        text.replace("\nrunning 1 test\n", "\nrunning 1 test\n\nrunning 1 test\n", 1),
        text.replace("\nok\n", "\nFAILED\n", 1),
        text.replace("\nok\n", "\n", 1),
        text.replace(" ... ok\n", " ... FAILED\n", 1),
        text.replace("test result: ok.", "test result: FAILED.", 1),
        text[: text.index("test result:")],
    )
    for mutated in mutations:
        assert mutated != text
        try:
            parse(mutated, 1057, 17)
        except (AssertionError, IndexError):
            pass
        else:
            raise AssertionError("corrupt harness accepted")
    print(
        json.dumps(
            {
                "scope": "strict supplemental CPU receipt audit, no test rerun",
                "sealed_manifest_sha256": hashlib.sha256(
                    manifest.read_bytes()
                ).hexdigest(),
                "sealed_members": len(members),
                "logs_consumed": len(rosters),
                "rejected_harness_mutations": len(mutations),
                "example_passed_per_target": 13,
                "runtime_passed_per_target": 1057,
                "runtime_ignored_per_target": 17,
                "abort_parent_child_banners": ABORTS,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
