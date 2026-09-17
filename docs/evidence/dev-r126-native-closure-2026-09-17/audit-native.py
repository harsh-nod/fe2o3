#!/usr/bin/env python3
"""Audit the recorded eight-probe prefix, guard stop and exact scratch cleanup."""

import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import sys


ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
SOURCE = "967a62dfffb94b4ca679ba3d4ae5ed60fc82eba1"
DIGEST = "9204ac3f18b35d6ce4bcc29492c7c971f9187a647dd03ee0f936c73ba33a1f00"
REMOTE = "/tmp/fe2o3-r126-native-967a62df.CoT8qwY4"
TESTS = [
    "cold_allocation::native_runtime_cold_device_capacity_refunds_context_and_retries",
    "cold_allocation::native_runtime_cold_host_capacity_refunds_context_and_retries",
    "native_runtime_auxiliary_shutdown_retries_after_primary_capacity_rejection",
    "native_runtime_allocates_while_primary_compute_is_pending",
    "native_runtime_allocates_while_primary_and_auxiliary_compute_are_pending",
    "native_runtime_allocation_shutdown_selects_retained_directional_release",
    "native_runtime_device_promotion_roundtrip_and_retained_shutdown",
    "native_runtime_zero_capacity_recycle_disposes_before_trim",
]


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def read(group, name, suffix="log"):
    return (ARCHIVE / group / f"{name}.{suffix}").read_text()


def command(group, name, expected):
    require(shlex.split(read(group, name, "command")) == expected,
            f"{group}/{name}: command mismatch")


def availability_records(prefix):
    return [f"{prefix}-{suffix}" for suffix in ("use", "identity", "pids", "admit")]


native_order = ["source", "binary-before", *availability_records("initial"),
                "create-scratch", "upload", "remote-hash-before", "remote-tools"]
for index in range(9):
    native_order.extend(availability_records(f"probe-{index:02d}-before"))
    if index < len(TESTS):
        native_order.append(f"probe-{index:02d}")
closure_order = [
    "remote-hash", "scratch-owner", "scratch-members", "process-absent", "remove-binary",
    "remove-directory", "scratch-absent", "scratch-link-absent", "final-use",
    "final-identity", "final-pids", "binary-after", "source-after", "source-clean",
    "source-index-clean",
]
native, closure = set(native_order), set(closure_order)
require(len(native) == 54 and len(closure) == 15, "invalid record inventory")
for group, names in (("native", native), ("closure", closure)):
    expected_files = {f"{name}.{suffix}" for name in names
                      for suffix in ("command", "log", "started", "finished", "exit")}
    require({path.name for path in (ARCHIVE / group).iterdir()} == expected_files,
            f"{group}: incomplete or extra command records")
    for name in names:
        expected_exit = "1" if name in {"probe-08-before-admit", "process-absent"} else "0"
        require(read(group, name, "exit").strip() == expected_exit, f"{group}/{name}: exit mismatch")
        start, finish = (read(group, name, suffix).strip() for suffix in ("started", "finished"))
        require(all(re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{9}Z", value)
                    for value in (start, finish)) and start <= finish,
                f"{group}/{name}: invalid timestamps")

previous_finish = None
for group, names in (("native", native_order), ("closure", closure_order)):
    require(len(names) == len(set(names)), f"{group}: duplicated ordered record")
    for name in names:
        start = read(group, name, "started").strip()
        require(previous_finish is None or previous_finish <= start,
                f"{group}/{name}: record order mismatch")
        previous_finish = read(group, name, "finished").strip()

for index, test in enumerate(TESTS):
    name = f"probe-{index:02d}"
    full = "kfd_backend::retained_release_tests::" + test
    command("native", name, [
        "ssh", "mi300x", "env", "FE2O3_TEST_NATIVE_UNIQUE_ID=0xab83d2ffef0d3cdf",
        "FE2O3_TEST_NATIVE_ISOLATED=1", "FE2O3_TEST_NATIVE_INITIALIZED_PREFIX=2",
        "prlimit", "--core=0:0", "--", "timeout", "--signal=TERM", "--kill-after=10s", "180s",
        REMOTE + "/runtime-tests", full, "--exact", "--ignored", "--nocapture", "--test-threads=1",
    ])
    log = read("native", name)
    require(re.findall(r"^test (\S+) \.\.\.", log, re.MULTILINE) == [full],
            f"{name}: wrong test identity")
    summaries = re.findall(r"^test result:.*$", log, re.MULTILINE)
    require(len(summaries) == 1 and re.fullmatch(
        r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
        r"852 filtered out; finished in \d+(?:\.\d+)?s", summaries[0])
        and "FAILED" not in log and log.count("\nrunning 1 test\n") == 1,
        f"{name}: invalid result")
    print(f"{name}: {test}: PASS")

for prefix in ["initial"] + [f"probe-{index:02d}-before" for index in range(9)]:
    for suffix, arguments in (("use", ["--showuse", "--showmeminfo", "vram", "--json"]),
                              ("identity", ["--showuniqueid", "--showbus", "--json"]),
                              ("pids", ["--showpidgpus"])):
        command("native", prefix + "-" + suffix, ["ssh", "mi300x", "/opt/rocm/bin/rocm-smi", *arguments])
    paths = [str(ARCHIVE / "native" / f"{prefix}-{suffix}.log")
             for suffix in ("use", "identity", "pids")]
    command("native", prefix + "-admit", ["python3", str(ARCHIVE / "check-availability.py"), *paths])
    result = subprocess.run([sys.executable, str(ARCHIVE / "check-availability.py"), *paths],
                            capture_output=True, text=True, check=False)
    stopped = prefix == "probe-08-before"
    require(result.returncode == int(stopped), f"{prefix}: guard replay mismatch")
    require(result.stdout + result.stderr == read("native", prefix + "-admit"),
            f"{prefix}: guard output mismatch")
require(read("native", "probe-08-before-admit").strip() == "another process owns GPU 1",
        "wrong stop reason")

require(read("native", "source").strip() == read("closure", "source-after").strip() == SOURCE,
        "source endpoint mismatch")
require(read("native", "create-scratch").strip() == REMOTE, "scratch identity mismatch")
expected_hash = DIGEST + "  " + REMOTE + "/runtime-tests\n"
require(read("native", "remote-hash-before") == read("closure", "remote-hash") == expected_hash,
        "remote binary mismatch")
receipt = ROOT / "docs/evidence/dev-r126-auxiliary-release-2026-09-16/final/musl-runtime-binary.sha256"
digest, binary = receipt.read_text().strip().split("  ", 1)
command("native", "source", ["git", "rev-parse", "HEAD"])
command("closure", "source-after", ["git", "rev-parse", "HEAD"])
for group, name in (("native", "binary-before"), ("closure", "binary-after")):
    command(group, name, ["sha256sum", "--check", str(receipt)])
command("native", "create-scratch", [
    "ssh", "mi300x", "mktemp", "-d", "/tmp/fe2o3-r126-native-967a62df.XXXXXXXX",
])
command("native", "upload", ["scp", binary, "mi300x:" + REMOTE + "/runtime-tests"])
require(read("native", "upload") == "", "unexpected upload output")
for group, name in (("native", "remote-hash-before"), ("closure", "remote-hash")):
    command(group, name, ["ssh", "mi300x", "sha256sum", REMOTE + "/runtime-tests"])
command("native", "remote-tools", [
    "ssh", "mi300x", "bash", "-c", "'command -v timeout && command -v prlimit && command -v fuser'",
])
require(read("native", "remote-tools") == "/usr/bin/timeout\n/usr/bin/prlimit\n/usr/bin/fuser\n",
        "unexpected remote tool paths")
with Path(binary).open("rb") as executable:
    require(hashlib.file_digest(executable, "sha256").hexdigest() == digest == DIGEST,
            "local binary identity mismatch")
require(read("native", "binary-before") == read("closure", "binary-after") == binary + ": OK\n",
        "local binary-check output mismatch")
command("closure", "process-absent", ["ssh", "mi300x", "fuser", REMOTE + "/runtime-tests"])
require(read("closure", "process-absent") == "", "ambiguous process-closure output")
command("closure", "scratch-owner", ["ssh", "mi300x", "stat", "-c", "'%U %a'", REMOTE])
command("closure", "scratch-members", [
    "ssh", "mi300x", "find", REMOTE, "-mindepth", "1", "-maxdepth", "1", "-printf", "'%y %u %f\\n'",
])
require(read("closure", "scratch-owner").strip() == "harsh 700"
        and read("closure", "scratch-members").strip() == "f harsh runtime-tests", "unexpected scratch ownership")
for name, arguments in (
    ("remove-binary", ["rm", "--", REMOTE + "/runtime-tests"]),
    ("remove-directory", ["rmdir", "--", REMOTE]),
    ("scratch-absent", ["test", "!", "-e", REMOTE]),
    ("scratch-link-absent", ["test", "!", "-L", REMOTE]),
):
    command("closure", name, ["ssh", "mi300x", *arguments])
    require(read("closure", name) == "", f"{name}: unexpected output")
require(read("closure", "source-clean") == read("closure", "source-index-clean") == "",
        "Rust source changed")
for name, arguments in (("source-clean", ["git", "diff", "--exit-code", "--", "crates"]),
                        ("source-index-clean", ["git", "diff", "--cached", "--exit-code", "--", "crates"])):
    command("closure", name, arguments)
for suffix, arguments in (("use", ["--showuse", "--showmeminfo", "vram", "--json"]),
                          ("identity", ["--showuniqueid", "--showbus", "--json"]),
                          ("pids", ["--showpidgpus"])):
    command("closure", "final-" + suffix, ["ssh", "mi300x", "/opt/rocm/bin/rocm-smi", *arguments])
require(json.loads(read("closure", "final-identity"))["card1"]
        == json.loads(read("native", "initial-identity"))["card1"], "closing device identity changed")
print("Eight native tests passed; ninth admission rejected; five planned test invocations did not run.")
print("All 54 native and 15 closure records match in serial order; remote binary and private directory removed.")
