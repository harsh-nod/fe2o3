#!/usr/bin/env python3
"""Verify full CPU harnesses and exact source/executable qualification identity."""

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys

sys.dont_write_bytecode = True
ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
HELPER = ARCHIVE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"
assert (
    hashlib.sha256(HELPER.read_bytes()).hexdigest()
    == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb"
)
spec = importlib.util.spec_from_file_location("whole_harness", HELPER)
helper = importlib.util.module_from_spec(spec)
spec.loader.exec_module(helper)
ENV = [
    "env",
    "CARGO_INCREMENTAL=0",
    "CARGO_PROFILE_DEV_DEBUG=0",
    "CARGO_PROFILE_TEST_DEBUG=0",
    "CARGO_BUILD_JOBS=2",
    "CARGO_TERM_COLOR=never",
    "RUST_TEST_THREADS=1",
]
BASE = "8058fd785e9d73f80d704a9cf726250a11865264"
MUSL_HIP_WARNING = (
    "warning: fe2o3-hip-sys@0.1.0: HIP headers not found; "
    "device-property discovery will return HIP_ERROR_NOT_SUPPORTED\n"
)


def receipt(name, command, code=0):
    paths = {
        suffix: ARCHIVE / f"raw/{name}.{suffix}"
        for suffix in ("command", "started", "finished", "exit", "log")
    }
    assert all(path.is_file() and not path.is_symlink() for path in paths.values()), (
        name
    )
    assert paths["exit"].read_text() == f"{code}\n", name
    assert shlex.split(paths["command"].read_text()) == command, name
    times = [paths[key].read_text().strip() for key in ("started", "finished")]
    assert all(
        re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", value)
        for value in times
    )
    assert times[0] <= times[1]
    return times


def docs(text):
    lines, index, crate = text.splitlines(), 0, None
    rosters, counts = {}, {}
    while index < len(lines):
        line = lines[index]
        index += 1
        if not line.strip():
            continue
        header = re.fullmatch(r"   Doc-tests (fe2o3_host|fe2o3_runtime)", line)
        if header:
            crate = header[1]
            assert crate not in rosters
            rosters[crate], counts[crate] = {}, []
            continue
        running = re.fullmatch(r"running ([0-9]+) tests?", line)
        if running:
            assert crate is not None
            count = int(running[1])
            assert count > 0
            counts[crate].append(count)
            for _ in range(count):
                row = re.fullmatch(
                    r"test (crates/[^ ]+ - .+ \(line [0-9]+\)) - (compile|compile fail) \.\.\. ok",
                    lines[index],
                )
                index += 1
                assert row and row[1] not in rosters[crate]
                rosters[crate][row[1]] = row[2]
            assert lines[index] == ""
            index += 1
            assert re.fullmatch(
                rf"test result: ok\. {count} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in [0-9.]+s",
                lines[index],
            )
            index += 1
            continue
        assert re.fullmatch(
            r"   Compiling .+|    Finished `test` profile .+|all doctests ran in [0-9.]+s; merged doctests compilation took [0-9.]+s",
            line,
        ), line
    assert counts == {"fe2o3_host": [4, 21], "fe2o3_runtime": [1, 41]}, counts
    bundle = [
        kind
        for name, kind in rosters["fe2o3_host"].items()
        if "/bundle_completion.rs " in name
    ]
    assert sorted(bundle) == ["compile"] + ["compile fail"] * 5
    return rosters


def verify():
    commands = {
        "rustc": ["rustc", "--version", "--verbose"],
        "cargo": ["cargo", "--version", "--verbose"],
        "source-before": ["python3", "-I", str(ARCHIVE / "source.py")],
    }
    tests = {}
    for target in ("gnu", "musl"):
        prefix = ENV + (["FE2O3_HIP_SYS_DISABLE=1"] if target == "musl" else [])
        flags = ["--target", "x86_64-unknown-linux-musl"] if target == "musl" else []
        for crate in ("host", "runtime"):
            name = target + "-" + crate
            tests[name] = (
                prefix
                + [
                    "cargo",
                    "test",
                    "--frozen",
                    "-p",
                    "fe2o3-" + crate,
                    "--all-features",
                    "--lib",
                ]
                + flags
            )
            commands[name + "-build"] = tests[name] + ["--no-run"]
    commands["binaries-before"] = ["python3", "-I", str(ARCHIVE / "binaries.py")]
    commands.update(tests)
    commands.update(
        {
            "binaries-after": ["python3", "-I", str(ARCHIVE / "binaries.py")],
            "doctests": ENV
            + [
                "cargo",
                "test",
                "--frozen",
                "-p",
                "fe2o3-host",
                "-p",
                "fe2o3-runtime",
                "--all-features",
                "--doc",
            ],
            "no-default": ENV
            + [
                "cargo",
                "check",
                "--frozen",
                "-p",
                "fe2o3-host",
                "-p",
                "fe2o3-runtime",
                "--no-default-features",
            ],
            "clippy": ENV
            + [
                "cargo",
                "clippy",
                "--frozen",
                "-p",
                "fe2o3-host",
                "-p",
                "fe2o3-runtime",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
            "unsafe-source": ENV
            + [
                "cargo",
                "test",
                "--frozen",
                "-p",
                "cargo-fe2o3",
                "--test",
                "unsafe_source_policy",
            ],
            "fmt": ENV + ["cargo", "fmt", "--all", "--", "--check"],
            "source-after": ["python3", "-I", str(ARCHIVE / "source.py")],
        }
    )
    times = {name: receipt(name, command) for name, command in commands.items()}
    for before, after in zip(list(commands), list(commands)[1:]):
        assert times[before][1] <= times[after][0], (before, after)
    rosters = {}
    for target in ("gnu", "musl"):
        for crate, passed, ignored in (("host", 282, 4), ("runtime", 1067, 17)):
            name = target + "-" + crate
            transcript = (ARCHIVE / f"raw/{name}.log").read_text()
            if name == "musl-host":
                assert transcript.startswith(MUSL_HIP_WARNING)
                transcript = transcript[len(MUSL_HIP_WARNING) :]
            rosters[name] = helper.parse(transcript, passed, ignored)
    for crate in ("host", "runtime"):
        assert rosters["gnu-" + crate] == rosters["musl-" + crate]
    bundle_names = {name for name in rosters["gnu-host"] if "::bundle_tests::" in name}
    assert len(bundle_names) == 11 and all(
        rosters["gnu-host"][name] == "ok" for name in bundle_names
    )
    assert any("maximum_abi_tuple" in name for name in bundle_names)
    baselines = {
        "host": (
            "dev-c6-owned-stop-2026-09-17/raw/gnu-host.log",
            "8b8e6607c9ccf0928e8cfa636f69e50bac3cb0e1fa43f25a2a5422696ac9dfea",
            271,
            4,
        ),
        "runtime": (
            "dev-kfd-native-wait-cpu-2026-09-18/raw/gnu-runtime-final.log",
            "dcf214320a8ca00ca11518f6769a6ebf1969333e97975dbfc2cc04c84ed23f19",
            1067,
            17,
        ),
    }
    for crate, (path, digest, passed, ignored) in baselines.items():
        raw = (ARCHIVE.parent / path).read_bytes()
        assert hashlib.sha256(raw).hexdigest() == digest
        previous = helper.parse(raw.decode(), passed, ignored)
        current_roster = rosters["gnu-" + crate]
        added = bundle_names if crate == "host" else set()
        assert current_roster.keys() - previous.keys() == added
        assert {k: v for k, v in current_roster.items() if k not in added} == previous
    unsafe = helper.parse((ARCHIVE / "raw/unsafe-source.log").read_text(), 5, 1)
    doc_rosters = docs((ARCHIVE / "raw/doctests.log").read_text())
    before = json.loads((ARCHIVE / "raw/source-before.log").read_text())
    after = json.loads((ARCHIVE / "raw/source-after.log").read_text())
    assert before == after and before["base"] == BASE
    current = json.loads(
        subprocess.check_output(
            ["python3", "-I", str(ARCHIVE / "source.py")], text=True
        )
    )
    assert current["files"] == after["files"] and len(after["files"]) == 5534
    binaries = json.loads((ARCHIVE / "raw/binaries-before.log").read_text())
    assert binaries == json.loads((ARCHIVE / "raw/binaries-after.log").read_text())
    assert set(binaries) == set(tests)
    for name, data in binaries.items():
        assert set(data) == {"path", "sha256"}
        assert (
            hashlib.sha256((ROOT / data["path"]).read_bytes()).hexdigest()
            == data["sha256"]
        )
        paths = re.findall(
            r"^     Running .+ \((target/[^)]+)\)$",
            (ARCHIVE / f"raw/{name}.log").read_text(),
            re.MULTILINE,
        )
        assert paths == [data["path"]], name
        built = re.findall(
            r"^  Executable .+ \((target/[^)]+)\)$",
            (ARCHIVE / f"raw/{name}-build.log").read_text(),
            re.MULTILINE,
        )
        assert built == paths
    exploratory = {
        "exploratory-check": (
            ENV[:3] + ENV[4:6] + ["cargo", "check", "--frozen", "-p", "fe2o3-host"],
            101,
        ),
        "exploratory-bundle": (
            ENV
            + [
                "cargo",
                "test",
                "--frozen",
                "-p",
                "fe2o3-host",
                "--lib",
                "bundle_completion_",
            ],
            101,
        ),
        "exploratory-bundle-fixed": (
            ENV
            + [
                "cargo",
                "test",
                "--frozen",
                "-p",
                "fe2o3-host",
                "--lib",
                "bundle_completion_",
            ],
            0,
        ),
        "exploratory-clippy": (
            ENV[:-1]
            + [
                "cargo",
                "clippy",
                "--frozen",
                "-p",
                "fe2o3-host",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ],
            101,
        ),
    }
    for name, (command, code) in exploratory.items():
        receipt(name, command, code)
    helper.parse((ARCHIVE / "raw/exploratory-bundle-fixed.log").read_text(), 11, 0, 154)
    assert "error[E0220]" in (ARCHIVE / "raw/exploratory-check.log").read_text()
    assert "10 passed; 1 failed" in (ARCHIVE / "raw/exploratory-bundle.log").read_text()
    assert "clippy::err-expect" in (ARCHIVE / "raw/exploratory-clippy.log").read_text()
    relative = str(ARCHIVE.relative_to(ROOT))
    shellcheck = ["shellcheck"] + [
        f"{relative}/{name}.sh" for name in ("record", "qualify", "seal")
    ]
    verifier = ["python3", "-I", str(ARCHIVE / "verify.py")]
    evidence_commands = {
        "verify": (verifier, 0),
        "python-lint": (["ruff", "check", relative], 0),
        "python-format": (["ruff", "format", "--check", relative], 0),
        "shellcheck": (shellcheck, 1),
        "shellcheck-final": (shellcheck, 0),
        "python-lint-final": (["ruff", "check", relative], 0),
        "python-format-final": (["ruff", "format", "--check", relative], 0),
        "shellcheck-seal-final": (shellcheck, 0),
    }
    for name, (command, code) in evidence_commands.items():
        assert times["source-after"][1] <= receipt(name, command, code)[0]
    assert "SC2094" in (ARCHIVE / "raw/shellcheck.log").read_text()
    suffixes = {"command", "started", "finished", "exit", "log"}
    expected = {
        f"{name}.{suffix}"
        for name in commands.keys() | exploratory.keys() | evidence_commands.keys()
        for suffix in suffixes
    }
    actual = {path.name for path in (ARCHIVE / "raw").iterdir()}
    final = {f"verify-final.{suffix}" for suffix in suffixes}
    assert actual - final == expected
    if (ARCHIVE / "raw/verify-final.exit").exists():
        receipt("verify-final", verifier)
    mutations = []
    for name, passed, ignored in (("gnu-host", 282, 4), ("gnu-runtime", 1067, 17)):
        text = (ARCHIVE / f"raw/{name}.log").read_text()
        mutations.extend(
            (helper.parse, altered, (passed, ignored))
            for altered in (
                text + "unexpected trailing payload\n",
                text + "running 0 tests\n",
                text.replace(" ... ok\n", " ... FAILED\n", 1),
                text.replace(" ... ok\n", " ... ok, unparsed payload\n", 1),
                text.replace("test result: ok.", "test result: FAILED."),
                text[: text.index("test result:")],
            )
        )
    text = (ARCHIVE / "raw/doctests.log").read_text()
    mutations.extend(
        (docs, altered, ())
        for altered in (
            text + "unexpected trailing payload\n",
            text.replace(" ... ok\n", " ... FAILED\n", 1),
            text.replace("running 4 tests", "running 3 tests", 1),
            text[: text.index("test result:")],
        )
    )
    for parser, mutated, arguments in mutations:
        try:
            parser(mutated, *arguments)
        except (AssertionError, IndexError, KeyError):
            pass
        else:
            raise AssertionError("corrupt harness accepted")
    return {
        "scope": "CPU compositional qualification, not protected/native execution, performance, or formal refinement",
        "source_base": BASE,
        "source_files": len(after["files"]),
        "test_executables": binaries,
        "host_per_target": {"passed": 282, "ignored": 4},
        "runtime_per_target": {"passed": 1067, "ignored": 17},
        "new_bundle_tests": sorted(bundle_names),
        "doctests": sum(map(len, doc_rosters.values())),
        "new_bundle_doctests": 6,
        "unsafe_policy": {
            "passed": sum(status == "ok" for status in unsafe.values()),
            "ignored": 1,
        },
        "rejected_transcript_mutations": len(mutations),
        "preserved_exploratory_failures": 3,
        "preserved_host_baseline_tests": 275,
        "preserved_runtime_baseline_tests": 1084,
        "preserved_evidence_lint_failures": 1,
    }


if __name__ == "__main__":
    print(json.dumps(verify(), indent=2))
