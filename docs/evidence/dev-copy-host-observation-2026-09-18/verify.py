#!/usr/bin/env python3
"""Read-only audit of CPU qualification and the six recorded quiet endpoints."""

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
BASE = "b87f30d1b87b2dca29e9f03e8b00f99a65b04391"
FIXTURE = "kfd_backend::retained_release_tests::copy_accounting::native_runtime_directional_256_mib_copy_shutdown_refunds_exact_backing"


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


def raw(name):
    return (ARCHIVE / f"raw/{name}.log").read_text()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def python_tests(name, count):
    text = raw(name)
    tests = re.findall(r"^test_[^\n]+ \.\.\. ok$", text, re.MULTILINE)
    assert len(tests) == count and len(set(tests)) == count
    assert re.search(rf"\nRan {count} tests in [0-9.]+s\n\nOK\n$", text)
    return tests


def verify():
    helper_path = ARCHIVE.parent / "dev-kfd-native-wait-cpu-2026-09-18/verify.py"
    assert (
        digest(helper_path)
        == "13b8c2fc57cb013cd19df6621efb0cd22a79b8ce84c9539ba4258b95a8c22ddb"
    )
    helper = load("copy_harness_parser", helper_path)
    before = json.loads(raw("source-qualified-before"))
    assert before == json.loads(raw("source-qualified-after"))
    assert before["base"] == BASE and len(before["files"]) == 5543
    current = json.loads(
        subprocess.check_output(
            ["python3", "-I", str(ARCHIVE / "source.py")], text=True
        )
    )
    assert current["files"] == before["files"]
    previous_archive = ARCHIVE.parent / "dev-async-observer-registration-2026-09-18"
    previous = json.loads((previous_archive / "raw/source-after.log").read_text())[
        "files"
    ]
    added = {
        "benchmarks/runtime_gfx942/copy-host-observe.py",
        "benchmarks/runtime_gfx942/test_copy_host_observe.py",
        "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests/copy_accounting.rs",
    }
    assert before["files"].keys() - previous.keys() == added
    assert previous.keys() - before["files"].keys() == set()
    assert {p for p in previous if previous[p] != before["files"][p]} == {
        "crates/fe2o3-runtime/src/kfd_backend/retained_release_tests.rs"
    }
    binaries = json.loads(raw("binaries-before"))
    assert binaries == json.loads(raw("binaries-after")) and set(binaries) == {
        "gnu",
        "musl",
    }
    prior_log = previous_archive / "raw/gnu-runtime.log"
    prior_roster = helper.parse(prior_log.read_text(), 1101, 17)
    rosters = {}
    for target in ("gnu", "musl"):
        transcript = raw(target + "-tests")
        environment = [
            "env",
            "CARGO_INCREMENTAL=0",
            "CARGO_PROFILE_DEV_DEBUG=0",
            "CARGO_PROFILE_TEST_DEBUG=0",
            "CARGO_BUILD_JOBS=2",
            "CARGO_TERM_COLOR=never",
        ]
        target_env = ["FE2O3_HIP_SYS_DISABLE=1"] if target == "musl" else []
        target_flags = (
            ["--target", "x86_64-unknown-linux-musl"] if target == "musl" else []
        )
        cargo = [
            "cargo",
            "test",
            "--frozen",
            "-p",
            "fe2o3-runtime",
            "--all-features",
            "--lib",
        ]
        assert shlex.split(
            (ARCHIVE / f"raw/{target}-build.command").read_text()
        ) == environment + target_env + cargo + target_flags + ["--no-run"]
        assert (
            shlex.split((ARCHIVE / f"raw/{target}-tests.command").read_text())
            == environment + ["RUST_TEST_THREADS=1"] + target_env + cargo + target_flags
        )
        rosters[target] = helper.parse(transcript, 1101, 18)
        assert rosters[target].keys() - prior_roster.keys() == {FIXTURE}
        assert rosters[target][FIXTURE].startswith("ignored")
        assert {
            k: v for k, v in rosters[target].items() if k != FIXTURE
        } == prior_roster
        built = re.findall(
            r"^  Executable .+ \((target/[^)]+)\)$",
            raw(target + "-build"),
            re.MULTILINE,
        )
        executed = re.findall(
            r"^     Running .+ \((target/[^)]+)\)$", transcript, re.MULTILINE
        )
        assert built == executed == [binaries[target]["path"]]
        assert digest(ROOT / binaries[target]["path"]) == binaries[target]["sha256"]
    assert rosters["gnu"] == rosters["musl"]
    assert "INTERP" not in raw("musl-elf") and "(NEEDED)" not in raw("musl-elf")
    assert raw("musl-binary-before").split() == [
        binaries["musl"]["sha256"],
        binaries["musl"]["path"],
    ]
    assert len(helper.parse(raw("unsafe-source"), 5, 1)) == 6
    assert python_tests("observer-tests", 19) == python_tests(
        "observer-tests-final", 19
    )
    python_tests("host-guard-regression", 75)
    assert raw("observer-source-before") == raw("observer-source-after")
    for line in raw("observer-source-before").splitlines():
        expected, path = line.split()
        assert expected == before["files"][path]

    observer = load(
        "copy_endpoint_parser", ROOT / "benchmarks/runtime_gfx942/copy-host-observe.py"
    )
    records = [json.loads(line) for line in raw("live-observer").splitlines()]
    assert len(records) == 7
    last = records[-1]
    assert last == {
        "schema": observer.SCHEMA,
        "record": "complete",
        "observations": 6,
        "refused": 0,
        "all_endpoints_admitted": True,
        "performance_accepted": False,
    }
    previous_end = 0
    for index, row in enumerate(records[:-1]):
        assert row["schema"] == observer.SCHEMA and row["record"] == "observation"
        assert row["index"] == index and row["gpu_index"] == 4
        assert (
            row["pci_bdf"] == "0000:85:00.0"
            and row["unique_id"] == "0x54f88318ca05093d"
        )
        assert row["vram_limit_exclusive"] == observer.VRAM_LIMIT
        assert len(row["sysfs"]) == 3
        prefix = [
            "/usr/bin/timeout",
            "--kill-after=5s",
            "20s",
            "/opt/rocm/bin/rocm-smi",
        ]
        assert row["status"]["command"] == prefix + [
            "--showuse",
            "--showmeminfo",
            "vram",
            "--showuniqueid",
            "--showbus",
            "--json",
        ]
        assert row["pids"]["command"] == prefix + ["--showpidgpus"]
        assert (
            row["endpoint_admitted"]
            and row["reasons"] == []
            and row["selected_pids"] == []
        )
        assert row["started"]["monotonic_ns"] > previous_end
        previous_end = row["finished"]["monotonic_ns"]
        points = [row["started"]]
        for sysfs, label in zip(row["sysfs"], ("status", "pids", None)):
            assert (
                not sysfs["errors"]
                and sysfs["values"]["unique_id"] == row["unique_id"][2:]
            )
            assert sysfs["path"] == "/sys/bus/pci/devices/" + row["pci_bdf"]
            assert sysfs["values"]["gpu_busy_percent"] == "0"
            assert sysfs["values"]["mem_info_vram_used"] == "298647552"
            points.extend([sysfs["started"], sysfs["finished"]])
            if label:
                capture = row[label]
                assert (
                    capture["exit"] == 0
                    and capture["error"] is None
                    and capture["stderr"] == ""
                )
                points.extend([capture["started"], capture["finished"]])
        points.append(row["finished"])
        assert all(
            a["monotonic_ns"] <= b["monotonic_ns"] for a, b in zip(points, points[1:])
        )
        status = observer.parse_status(
            row["status"]["stdout"], 4, row["pci_bdf"], row["unique_id"]
        )
        assert status == {"busy_percent": 0, "vram_bytes": 298647552}
        assert not any(
            4 in devices
            for devices in observer.parse_pids(row["pids"]["stdout"]).values()
        )

    suffixes = {"command", "started", "finished", "exit", "log"}
    names = {path.stem for path in (ARCHIVE / "raw").iterdir()}
    actual = {path.name for path in (ARCHIVE / "raw").iterdir()}
    expected = {f"{name}.{suffix}" for name in names for suffix in suffixes}
    pending = (
        "verify-final" in names and not (ARCHIVE / "raw/verify-final.exit").exists()
    )
    if pending:
        expected -= {"verify-final.exit", "verify-final.finished"}
    assert actual == expected
    for name in names:
        if name == "verify-final" and pending:
            continue
        assert (ARCHIVE / f"raw/{name}.exit").read_text() == "0\n"
        assert shlex.split((ARCHIVE / f"raw/{name}.command").read_text())
        times = [
            (ARCHIVE / f"raw/{name}.{suffix}").read_text().strip()
            for suffix in ("started", "finished")
        ]
        assert all(
            re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{9}Z", t)
            for t in times
        )
        assert times[0] <= times[1]
    print(
        json.dumps(
            {
                "audit": "PASS",
                "source_files": 5543,
                "runtime_each": {"passed": 1101, "ignored": 18},
                "new_native_fixture": "ignored-in-CPU-campaign",
                "observer_tests": 19,
                "host_guard_tests": 75,
                "quiet_endpoint_samples": 6,
                "unchanged_test_binaries": 2,
                "closed_receipts": len(names),
                "performance_accepted": False,
            },
            indent=2,
        )
    )


if __name__ == "__main__":
    verify()
