#!/usr/bin/env python3
"""Read-only audit of one failed guarded campaign; never execute native work."""

import argparse
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys
import tarfile

sys.dont_write_bytecode = True
PACKET = Path(__file__).resolve().parent
ORIGINAL = "/home/harsh/.codex-tmp/kfd-native-wait-5d70cb0a-20260918.6fSWSPIY"
OWNED = "/tmp/fe2o3-kfd-native-wait-5d70cb0a-20260918.CgOcvGXS"
COMMIT = "5d70cb0a6e16fb265fe224690274fdb0be2b0055"
EXPORT_SHA = "abb3ee3a4be431584a1303bda58828a2c89160d1d1c9461cebad95d64f7eb9b4"
SOURCE_PATHS = [
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "crates",
    "examples",
    "benchmarks/runtime_gfx942",
]
STAMP = r"2026-09-18T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]{9}Z"


def data(name):
    return (PACKET / name).read_bytes()


def text(name):
    return data(name).decode()


def sha(value):
    return hashlib.sha256(value).hexdigest()


def load_protocol():
    helper = PACKET / "vendor/dev-kfd-native-wait-mi300x-2026-09-18/summarize.py"
    assert (
        sha(helper.read_bytes())
        == "b8fd9dac4a81974d3cc2e13c542cf9a3b004aa41c3eb6f9d7d77e9cd5cb693f9"
    )
    spec = importlib.util.spec_from_file_location("pinned_native_protocol", helper)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    old = module.BINARIES
    module.BINARIES = tuple(path.replace(module.OWNED, OWNED) for path in old)
    module.RUNNER_METADATA -= {path + ": OK" for path in old}
    module.RUNNER_METADATA |= {path + ": OK" for path in module.BINARIES}
    module.OWNED = OWNED
    module.COMMIT = COMMIT
    module.CONTEXT = dict(module.CONTEXT, git_commit=COMMIT)
    return module


def verify_records():
    ssh = ["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=10", "mi300x", "bash"]
    commands = {
        "source-export": ["bash", "export.sh"],
        "create": [
            "bash",
            "-c",
            'ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x bash -s < "$1"',
            "--",
            ORIGINAL + "/create.sh",
        ],
        "script-check": [
            "shellcheck",
            "--shell=bash",
            "prepare.sh",
            "run.sh",
            "guard.sh",
            "inspect.sh",
            "cleanup.sh",
            "record.sh",
            "export.sh",
            "create.sh",
        ],
        "upload": [
            "scp",
            "-q",
            "source.tar",
            "source-tree.txt",
            "source.commit",
            "prepare.sh",
            "guard.sh",
            "run.sh",
            "inspect.sh",
            "cleanup.sh",
            "mi300x:" + OWNED + "/",
        ],
        "initial-guard": ssh + [OWNED + "/guard.sh"],
        "collect-build": [
            "scp",
            "-rq",
            "mi300x:" + OWNED + "/results",
            "results-before",
        ],
        "collect": ["scp", "-rq", "mi300x:" + OWNED + "/results", "results"],
        "source-audit": ["python3", "-I", "audit_source.py"],
        "full-summary-refusal": ["python3", "-I", "summarize.py"],
        "interruption-report": ["python3", "-I", "interruption.py"],
        "absence": [
            "bash",
            "-c",
            'ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x bash -s < "$1"',
            "--",
            ORIGINAL + "/absence.sh",
        ],
    }
    for name, script in {
        "inspect-upload": "inspect",
        "prepare": "prepare",
        "inspect-before": "inspect",
        "benchmark": "run",
        "inspect-after": "inspect",
        "cleanup": "cleanup",
    }.items():
        commands[name] = ssh + [OWNED + "/" + script + ".sh", OWNED]
    suffixes = ("command", "started", "finished", "exit", "log")
    assert {p.name for p in (PACKET / "raw").iterdir()} == {
        name + "." + suffix for name in commands for suffix in suffixes
    }
    times = {}
    for name, expected in commands.items():
        assert shlex.split(text("raw/" + name + ".command")) == expected, name
        assert text("raw/" + name + ".exit") == (
            "1\n" if name in ("benchmark", "full-summary-refusal") else "0\n"
        ), name
        start, end = [
            text("raw/" + name + "." + key).strip() for key in ("started", "finished")
        ]
        assert (
            re.fullmatch(STAMP, start) and re.fullmatch(STAMP, end) and start <= end
        ), name
        times[name] = (start, end)
    chain = [
        "source-export",
        "create",
        "script-check",
        "upload",
        "initial-guard",
        "inspect-upload",
        "prepare",
        "inspect-before",
        "collect-build",
        "benchmark",
        "inspect-after",
        "collect",
        "cleanup",
        "absence",
        "interruption-report",
    ]
    for left, right in zip(chain, chain[1:]):
        assert times[left][1] <= times[right][0], (left, right)
    assert (
        times["collect-build"][1]
        <= times["source-audit"][0]
        <= times["source-audit"][1]
        <= times["benchmark"][1]
    )
    assert (
        times["benchmark"][1]
        <= times["full-summary-refusal"][0]
        <= times["interruption-report"][0]
    )
    assert text("raw/create.log") == OWNED + "\n"
    assert (
        text("raw/source-audit.log")
        == json.dumps(
            {
                "commit": COMMIT,
                "source_files": 5538,
                "git_blob_identities": True,
                "remote_build_source_equal": True,
            },
            indent=2,
        )
        + "\n"
    )
    refusal = text("raw/full-summary-refusal.log")
    assert refusal.startswith("Traceback (most recent call last):\n")
    assert ORIGINAL + '/summarize.py", line 25, in <module>' in refusal
    assert 'assert (STAGE / "raw/benchmark.exit").read_text() == "0\\n"' in refusal
    assert refusal.endswith("AssertionError\n")
    return len(commands)


def source_and_binary_identity(repo):
    assert text("source.commit") == COMMIT + "\n"
    tree = subprocess.check_output(
        ["git", "-C", str(repo), "ls-tree", "-r", COMMIT, "--", *SOURCE_PATHS]
    )
    assert tree == data("source-tree.txt")
    exported = subprocess.check_output(
        ["git", "-C", str(repo), "archive", "--format=tar", COMMIT, *SOURCE_PATHS]
    )
    assert sha(exported) == EXPORT_SHA
    expected = {}
    for line in tree.decode().splitlines():
        match = re.fullmatch(r"(100644|100755) blob ([0-9a-f]{40})\t(.+)", line)
        assert match and match[3] not in expected
        expected[match[3]] = (match[1], match[2])
    actual = {}
    with tarfile.open(fileobj=io.BytesIO(exported), mode="r:") as archive:
        assert archive.pax_headers["comment"] == COMMIT
        for entry in archive:
            if entry.isdir():
                continue
            assert entry.isfile() and entry.name not in actual
            mode, blob = expected[entry.name]
            contents = archive.extractfile(entry).read()
            assert (
                hashlib.sha1(
                    b"blob " + str(len(contents)).encode() + b"\0" + contents
                ).hexdigest()
                == blob
            )
            assert bool(entry.mode & 0o111) == (mode == "100755")
            actual[entry.name] = sha(contents)
    assert set(actual) == set(expected) and len(actual) == 5538
    expected_manifest = "".join(
        actual[name] + "  ./" + name + "\n" for name in sorted(actual)
    )
    for directory, names in (
        ("results-before", ("source-files.sha256", "source-build-after.sha256")),
        (
            "results",
            (
                "source-files.sha256",
                "source-build-after.sha256",
                "source-current.sha256",
            ),
        ),
    ):
        for name in names:
            assert text(directory + "/" + name) == expected_manifest, (directory, name)
    expected_checks = "".join("./" + name + ": OK\n" for name in sorted(actual))
    assert (
        text("results/source-before.log")
        == text("results/source-after.log")
        == expected_checks
    )
    assert (
        text("raw/source-export.log")
        == "\n".join(
            (
                EXPORT_SHA + "  source.tar",
                sha(tree) + "  source-tree.txt",
                sha(data("source.commit")) + "  source.commit",
            )
        )
        + "\n"
    )
    binaries = {
        OWNED
        + "/target/release/examples/gfx942-runtime-directional-window-benchmark": "883fd7cf04888bfaa3e3c539d088e94341e92841fe038caa981285930afc46d1",
        OWNED
        + "/hsa-copy-pool-engine": "5d7e0578ea8078bf10066bbd7f36fe78608449af6fb6d647f2ec4503e86d9e33",
    }
    binary_manifest = "".join(
        digest + "  " + name + "\n" for name, digest in binaries.items()
    )
    assert (
        text("results-before/binaries.sha256")
        == text("results/binaries.sha256")
        == binary_manifest
    )
    assert {p.name for p in (PACKET / "results-before").iterdir()} == {
        "binaries.sha256",
        "source-files.sha256",
        "source-build-after.sha256",
    }
    assert {p.name for p in (PACKET / "results").iterdir()} == {
        "binaries.sha256",
        "source-files.sha256",
        "source-build-after.sha256",
        "source-current.sha256",
        "source-before.log",
        "source-after.log",
    }
    inspections = {
        name: text("raw/" + name + ".log")
        for name in ("inspect-upload", "inspect-before", "inspect-after")
    }
    hashes = {
        name: sha(data(name))
        for name in (
            "prepare.sh",
            "guard.sh",
            "run.sh",
            "cleanup.sh",
            "inspect.sh",
            "source.commit",
            "source-tree.txt",
        )
    }
    hashes.update(
        {
            "source.tar": EXPORT_SHA,
            "owner": sha(("fe2o3-native-wait-" + COMMIT + "\n").encode()),
        }
    )
    for name, log in inspections.items():
        rows = re.findall(r"^([0-9a-f]{64})  (\S+)$", log, re.MULTILINE)
        assert (
            len(rows) == len(hashes)
            and dict((file, digest) for digest, file in rows) == hashes
        ), name
        for variable in (
            "HSA_XNACK",
            "HSA_ENABLE_SDMA",
            "HSA_ENABLE_PEER_SDMA",
            "HSA_OVERRIDE_GFX_VERSION",
            "HSA_TOOLS_LIB",
            "LD_PRELOAD",
            "LD_LIBRARY_PATH",
        ):
            assert log.count("inherited_environment " + variable + "=<unset>\n") == 1
    return len(actual), binaries


def benchmark(protocol):
    lines = text("raw/benchmark.log").splitlines()

    def one(prefix):
        found = [i for i, line in enumerate(lines) if line.startswith(prefix)]
        assert len(found) == 1, prefix
        return found[0]

    context, phase, completed, postflight, finished = [
        one(prefix)
        for prefix in ("context ", "phase ", "completed ", "postflight ", "finished ")
    ]
    assert protocol.fields(lines[context]) == protocol.CONTEXT
    assert lines[phase] == "phase repetition=1 cell=A"
    assert lines[completed] == "completed repetition=1 cell=A exit=0"
    assert lines[postflight] == "postflight repetition=1 cell=A exit=1"
    assert (
        lines[finished]
        == "finished exit=1 source_after_exit=0 binaries_after_exit=0 clean_exit=0 occupancy_exit=0"
    )
    cards = [i for i, line in enumerate(lines) if line.startswith('{"card')]
    admitted = [i for i, line in enumerate(lines) if line.startswith("admitted ")]
    assert len(cards) == 3 and len(admitted) == 2
    assert (
        context
        < phase
        < cards[0]
        < admitted[0]
        < completed
        < cards[1]
        < postflight
        < cards[2]
        < admitted[1]
        < finished
    )
    covered = {context, phase, completed, postflight, finished, cards[1]}
    for start, end in ((cards[0], admitted[0]), (cards[2], admitted[1])):
        protocol.base.validate_guards("\n".join(lines[start : end + 1]), 1)
        covered.update(range(start, end + 1))
    payload = lines[admitted[0] + 1 : completed]
    assert len(payload) == 15
    protocol.base.validate_kfd(payload, "A")
    covered.update(range(admitted[0] + 1, completed))
    assert cards[1] == completed + 2 and postflight == completed + 3
    observations = [
        json.loads(lines[i], object_pairs_hook=protocol.base.unique_object)["card4"]
        for i in cards
    ]
    assert [int(row["VRAM Total Used Memory (B)"]) for row in observations] == [
        298647552,
        648634368,
        298754048,
    ]
    for row in observations:
        assert (
            row["Unique ID"] == "0x54f88318ca05093d"
            and row["PCI Bus"] == "0000:85:00.0"
            and row["GPU use (%)"] == "0"
        )
    for i, line in enumerate(lines):
        assert i in covered or protocol.runner_metadata(line), (i, line)
    assert lines.count("post_run_porcelain=") == 1
    assert all(lines.count(binary + ": OK") == 2 for binary in protocol.BINARIES)
    timestamps = [lines[i - 1] for i in cards]
    assert all(
        re.fullmatch(STAMP, value) for value in timestamps
    ) and timestamps == sorted(timestamps)
    protocol.base.validate_guards(text("raw/initial-guard.log"), 1)
    try:
        protocol.parse(text("raw/benchmark.log"))
    except (AssertionError, ValueError, IndexError):
        pass
    else:
        raise AssertionError("complete protocol accepted incomplete campaign")
    expected_report = {
        "source_commit": COMMIT,
        "campaign_exit": 1,
        "completed_processes": [
            {
                "repetition": 1,
                "cell": "A",
                "exit": 0,
                "validated_rounds": 13,
                "measured_rounds": 10,
                "checked_bytes_per_round": 268435456,
                "teardown": "explicit-complete",
            }
        ],
        "not_launched": ["B", "C", "D"],
        "gpu4": [
            {
                "timestamp": timestamp,
                "utilization_percent": 0,
                "vram_bytes": int(row["VRAM Total Used Memory (B)"]),
            }
            for timestamp, row in zip(timestamps, observations)
        ],
        "failed_postflight": "VRAM exceeded 536870912 bytes; guard returned before collecting PID attachments",
        "final_guard": "passed with no reported GPU 4 attachment; does not override prior failure",
        "cause": "unexplained transient usage; no causal or execution-overlap inference",
        "matched_comparison": False,
        "timing_ratios_reported": False,
        "source_after_exit": 0,
        "binaries_after_exit": 0,
        "exact_source_roster_exit": 0,
    }
    assert (
        json.loads(
            text("raw/interruption-report.log"),
            object_pairs_hook=protocol.base.unique_object,
        )
        == expected_report
    )
    return expected_report


def cleanup():
    lines = text("raw/cleanup.log").splitlines()
    assert len(lines) == 4 and all(re.fullmatch(STAMP, lines[i]) for i in (0, 3))
    assert lines[1] == "454M\t" + OWNED
    assert lines[2] == "removed_owned_directory=" + OWNED + " absent=yes"
    absence = text("raw/absence.log").splitlines()
    assert len(absence) == 3 and all(re.fullmatch(STAMP, absence[i]) for i in (0, 2))
    assert absence[1] == "owned_directory_absent=yes owned_process_references=0"
    assert lines[0] <= lines[3] <= absence[0] <= absence[2]
    return absence[2]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--repo",
        type=Path,
        required=True,
        help="read-only Git repository containing the pinned commit",
    )
    args = parser.parse_args()
    assert not any(path.is_symlink() for path in PACKET.rglob("*"))
    count = verify_records()
    sources, binaries = source_and_binary_identity(args.repo)
    report = benchmark(load_protocol())
    absent = cleanup()
    print(
        json.dumps(
            {
                "audit": "PASS",
                "closed_original_records": count,
                "source_files": sources,
                "source_commit": COMMIT,
                "export_sha256": EXPORT_SHA,
                "recorded_binary_hashes": binaries,
                "source_and_binary_checks": "recorded before/after checks, not retained executables",
                "completed_processes": report["completed_processes"],
                "campaign_exit": 1,
                "failure": report["failed_postflight"],
                "matched_comparison": False,
                "owned_absence_scan_finished": absent,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    main()
