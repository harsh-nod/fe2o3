#!/usr/bin/env python3
"""Consume complete test harnesses; interrupted or failed attempts are not passes."""

import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess

ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
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
            r"   Compiling .+|    Finished `test` profile .+|     Running (?:unittests|tests/).+",
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
        ordinary = re.fullmatch(r"test (\S+) \.\.\. (ok|ignored)(, .+)?", line)
        if ordinary:
            name, status, reason = ordinary.groups()
            assert status == "ignored" or reason is None
            assert name not in ABORTS, "abort parent must retain exact child transcripts"
        else:
            parent = re.fullmatch(r"test (\S+) \.\.\. ", line)
            assert parent and parent[1] in ABORTS, ("unexpected outcome", line)
            name, status = parent[1], "ok"
            for child in range(ABORTS[name]):
                assert lines[cursor] == "running 1 test"
                cursor += 1
                assert lines[cursor] == f"test {name} ... " + ("ok" if child + 1 == ABORTS[name] else "")
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
    assert all(not line.strip() for line in lines[cursor + 1:]), "trailing payload"
    assert sum(value == "ok" for value in rows.values()) == passed
    assert sum(value == "ignored" for value in rows.values()) == ignored
    return rows


def receipt(name, command=None, code=0):
    paths = {suffix: ARCHIVE / f"raw/{name}.{suffix}" for suffix in ("command", "started", "finished", "exit", "log")}
    assert all(path.is_file() and not path.is_symlink() for path in paths.values()), name
    assert paths["exit"].read_text() == f"{code}\n", name
    if command is not None:
        assert shlex.split(paths["command"].read_text()) == command, name
    times = [paths[key].read_text().strip() for key in ("started", "finished")]
    assert all(re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", value) for value in times), name
    assert times[0] <= times[1], name
    return times


def main():
    example = ["cargo", "test", "--frozen", "-p", "fe2o3-runtime", "--features", "hardware-diagnostic", "--example", "gfx942-runtime-directional-window-benchmark"]
    runtime = ["cargo", "test", "--frozen", "-p", "fe2o3-runtime", "--all-features", "--lib"]
    kfd = ["cargo", "test", "--frozen", "-p", "fe2o3-kfd", "--features", "hardware-diagnostic", "--lib"]
    musl = ["env", "FE2O3_HIP_SYS_DISABLE=1"]
    target = ["--target", "x86_64-unknown-linux-musl"]
    specs = {
        "gnu-example-qualified": (17, 0, 0),
        "gnu-example-default": (14, 0, 0),
        "gnu-kfd-sdma-final": (151, 0, 1258),
        "gnu-kfd-wait-final": (26, 0, 1383),
        "gnu-runtime-final": (1067, 17, 0),
        "musl-example": (17, 0, 0),
        "musl-runtime": (1067, 17, 0),
        "model-r39": (6, 0, 792),
        "unsafe-source": (5, 1, 0),
    }
    commands = {
        "gnu-example-qualified": example,
        "gnu-example-default": example[:5] + example[7:],
        "gnu-kfd-sdma-final": kfd + ["sdma::"],
        "gnu-kfd-wait-final": kfd + ["wait::"],
        "gnu-runtime-final": runtime,
        "gnu-runtime-build": runtime + ["--no-run"],
        "musl-example": musl + example + target,
        "musl-runtime": musl + runtime + target,
        "model-r39": ["cargo", "test", "--frozen", "-p", "fe2o3-runtime-model", "--lib", "r39_scoped_persistent_sdma_wait_policy"],
        "unsafe-source": ["cargo", "test", "--frozen", "-p", "cargo-fe2o3", "--test", "unsafe_source_policy"],
        "clippy-final": ["cargo", "clippy", "--frozen", "-p", "fe2o3-kfd", "-p", "fe2o3-runtime", "--all-targets", "--all-features", "--", "-D", "warnings"],
        "fmt": ["cargo", "fmt", "--all", "--", "--check"],
        "source-qualified-before-v2": ["python3", "-I", str(ARCHIVE / "source.py")],
        "source-after": ["python3", "-I", str(ARCHIVE / "source.py")],
        "binaries-before": ["python3", "-I", str(ARCHIVE / "binaries.py")],
        "binaries-after": ["python3", "-I", str(ARCHIVE / "binaries.py")],
    }
    times = {name: receipt(name, command) for name, command in commands.items()}
    chain = ["source-qualified-before-v2", "clippy-final", "fmt", "gnu-example-qualified", "musl-example", "musl-runtime", "gnu-runtime-build", "gnu-kfd-sdma-final", "gnu-kfd-wait-final", "binaries-before", "gnu-runtime-final", "binaries-after", "source-after"]
    for left, right in zip(chain, chain[1:]):
        assert times[left][1] <= times[right][0], (left, right)
    rosters = {}
    for name, counts in specs.items():
        assert (ARCHIVE / f"raw/{name}.exit").read_text() == "0\n"
        rosters[name] = parse((ARCHIVE / f"raw/{name}.log").read_text(), *counts)
    assert rosters["gnu-example-qualified"] == rosters["musl-example"]
    assert rosters["gnu-runtime-final"] == rosters["musl-runtime"]
    for name, code in (("gnu-example", 101), ("gnu-runtime", 101), ("mixed-settlement-debug", 134), ("clippy", 101), ("clippy-fixed", 101)):
        receipt(name, code=code)
    for name in ("gnu-kfd", "gnu-kfd-serial"):
        assert not (ARCHIVE / f"raw/{name}.exit").exists()
        assert "test result:" not in (ARCHIVE / f"raw/{name}.log").read_text()
    before = json.loads((ARCHIVE / "raw/source-qualified-before-v2.log").read_text())
    after = json.loads((ARCHIVE / "raw/source-after.log").read_text())
    assert before == after
    assert before["base"] == "c6310ac6369a95e2e836444f19b347f26054193a"
    current = json.loads(subprocess.check_output(["python3", "-I", str(ARCHIVE / "source.py")], text=True))
    assert after["files"] == current["files"] and len(after["files"]) == 5532
    for name, digest in after["files"].items():
        assert hashlib.sha256((ROOT / name).read_bytes()).hexdigest() == digest, name
    binaries = json.loads((ARCHIVE / "raw/binaries-before.log").read_text())
    assert set(binaries) == {"gnu-kfd-sdma-final", "gnu-example-default", "gnu-example-qualified", "gnu-runtime-fixed", "musl-example", "musl-runtime", "model-r39", "unsafe-source"}
    assert binaries == json.loads((ARCHIVE / "raw/binaries-after.log").read_text())
    for name, data in binaries.items():
        assert set(data) == {"path", "sha256"}
        paths = re.findall(r"^     Running .+ \((target/[^)]+)\)$", (ARCHIVE / f"raw/{name}.log").read_text(), re.MULTILINE)
        assert paths == [data["path"]], name
        assert hashlib.sha256((ROOT / data["path"]).read_bytes()).hexdigest() == data["sha256"]
    for name, hashed in (("gnu-runtime-final", "gnu-runtime-fixed"), ("gnu-kfd-wait-final", "gnu-kfd-sdma-final")):
        paths = re.findall(r"^     Running .+ \((target/[^)]+)\)$", (ARCHIVE / f"raw/{name}.log").read_text(), re.MULTILINE)
        assert paths == [binaries[hashed]["path"]], name
    builds = re.findall(r"^  Executable .+ \((target/[^)]+)\)$", (ARCHIVE / "raw/gnu-runtime-build.log").read_text(), re.MULTILINE)
    assert builds == [binaries["gnu-runtime-fixed"]["path"]]
    text = (ARCHIVE / "raw/gnu-runtime-final.log").read_text()
    parent = next(iter(ABORTS))
    mutations = [
        text + "unexpected trailing payload\n",
        text + "running 0 tests\n",
        text.replace(" ... ok\n", " ... FAILED\n", 1),
        text.replace(" ... ok\n", " ... ok, unparsed payload\n", 1),
        text.replace("\nrunning 1 test\n", "\nrunning 2 tests\n", 1),
        text.replace("\nrunning 1 test\n", "", 1),
        text.replace("\nrunning 1 test\n", "\nrunning 1 test\n\nrunning 1 test\n", 1),
        text.replace(f"test {parent} ... \n", "test foreign ... \n", 1),
        text.replace(f"test {parent} ... ok", f"test {parent} ... FAILED"),
        text.replace("test result: ok.", "test result: FAILED."),
        text[:text.index("test result:")],
    ]
    for mutated in mutations:
        assert mutated != text
        try:
            parse(mutated, 1067, 17)
        except (AssertionError, IndexError):
            pass
        else:
            raise AssertionError("corrupt harness accepted")
    print(json.dumps({
        "scope": "CPU qualification only; no new native, performance, or formal-proof claim",
        "complete_harnesses": len(specs), "source_files": len(after["files"]),
        "binaries": len(binaries), "rejected_harness_mutations": len(mutations),
        "kfd_distinct_tests": len(rosters["gnu-kfd-sdma-final"].keys() | rosters["gnu-kfd-wait-final"].keys()),
        "runtime_per_target": {"passed": 1067, "ignored": 17},
        "new_runtime_tests": sum("directional_wait_diagnostic" in name for name in rosters["gnu-runtime-final"]),
        "incomplete_full_kfd_attempts": 2,
    }, indent=2))


if __name__ == "__main__":
    main()
