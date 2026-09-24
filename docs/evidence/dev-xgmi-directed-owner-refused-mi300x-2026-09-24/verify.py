#!/usr/bin/env python3
"""Replay one rejected native attempt; never upgrades it to graph qualification."""

import sys
if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import importlib.util
import io
import json
from functools import lru_cache
from datetime import datetime, timezone
from pathlib import Path
import re
import shlex
import subprocess
import tarfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SOURCE = "1d1992d07a0de94a683c9f6a1d8ddcd184262ea5"
EXECUTION_ROOT = "/home/harsh/.codex-tmp/fe2o3-directed-owner-20260924-mamEIcU0/source"
LOCAL_OUTPUT = "/home/harsh/.codex-tmp/fe2o3-xgmi-directed-owner-results-sl1xoibi"
TARGET = "/home/harsh/.codex-tmp/fe2o3-directed-owner-20260924-mamEIcU0/target"
EXAMPLE = "gfx942-runtime-xgmi-directed-owner-smoke"
ELF_SHA = "6b187b3fbae8d0dc7441064cb0a81b67b7978c20692afbc40adf40a328b65368"
WORKLOAD_ERROR_SHA = "f4349ea7d90378eb3d4fd440fcaff2962dff5a8b3db024f210cedbbbf0db702b"
NATIVE_ERROR_SHA = "1a933c7ff2b2267edaa320703d9810368f804b60d8330fe6d909b94a52087969"
DEVICES = [[5, "0000:a6:00.0", "0xb7baafd0fb173d8e"], [6, "0000:c6:00.0", "0x10a254ce4987e716"]]
SELECTORS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples",
             "benchmarks/runtime_gfx942", "scripts/unsafe-source-baseline.json", "docs/runtime-primary-queue-release-v1.md"]
SIGNERS = ROOT / "docs/evidence/dev-combined-sdma-release-native-gpu2-2026-09-18/raw/native/collected/allowed-signers"


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def blob(relative):
    return subprocess.check_output(["git", "cat-file", "blob", SOURCE + ":" + relative], cwd=ROOT)


def load(relative, name):
    path = ROOT / relative
    need(sha(path) == hashlib.sha256(blob(relative)).hexdigest(), "signed helper: " + relative)
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


need(sha(SIGNERS) == "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b", "trusted signer")
SIGNATURE = subprocess.run(["git", "-c", "gpg.ssh.allowedSignersFile=" + str(SIGNERS), "verify-commit", SOURCE],
                           cwd=ROOT, check=True, capture_output=True)
# Authenticate the transitive import set before executing any helper.
for relative in (
    "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py",
    "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/native.py",
    "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/campaign.py",
    "benchmarks/runtime_gfx942/xgmi_peer_segments_native.py",
    "benchmarks/runtime_gfx942/xgmi_peer_segments_campaign.py",
    "benchmarks/runtime_gfx942/xgmi_directed_owner_native.py",
    "benchmarks/runtime_gfx942/xgmi_directed_owner_results.py",
):
    need(sha(ROOT / relative) == hashlib.sha256(blob(relative)).hexdigest(), "signed import: " + relative)
OLD = load("docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/campaign.py", "owner_receipts")
C = load("benchmarks/runtime_gfx942/xgmi_peer_segments_campaign.py", "owner_controls")
N = load("benchmarks/runtime_gfx942/xgmi_directed_owner_native.py", "directed_owner_native")
R = load("benchmarks/runtime_gfx942/xgmi_directed_owner_results.py", "directed_owner_results")
H, B = N.H, N.B


@lru_cache(maxsize=1)
def signed_sources():
    names = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", SOURCE, "--", *SELECTORS], cwd=ROOT, text=True).splitlines()
    selectors = [s for s in SELECTORS if any(n == s or n.startswith(s + "/") for n in names)]
    archive = subprocess.check_output(["git", "archive", "--format=tar", SOURCE, "--", *selectors], cwd=ROOT)
    result = {}
    with tarfile.open(fileobj=io.BytesIO(archive), mode="r:") as source:
        for member in source:
            if member.isdir():
                continue
            need(member.isfile() and member.name not in result, "ordinary unique source")
            result[member.name] = hashlib.sha256(source.extractfile(member).read()).hexdigest()
    need(set(result) == set(names), "complete signed source roster")
    return result


def read(path):
    return H.parse_json(path.read_bytes())


def inventory(folder):
    return {name: digest for name, digest in B.inventory(folder).items() if name != "inventory.json"}


def failed_receipt(folder):
    need(folder.is_dir() and not folder.is_symlink(), "ordinary failed receipt directory")
    children = list(folder.iterdir())
    need(len(children) == 3 and {p.name for p in children} == {"receipt.json", "stdout", "stderr"}
         and all(p.is_file() and not p.is_symlink() for p in children), "exact failed receipt files")
    row = read(folder / "receipt.json")
    need(type(row) is dict and set(row) == OLD.RECEIPT_KEYS, "failed receipt schema")
    need(type(row["command"]) is list and bool(row["command"])
         and all(type(arg) is str and arg for arg in row["command"])
         and type(row["cwd"]) is str and bool(row["cwd"])
         and type(row["timeout_seconds"]) is int and row["timeout_seconds"] > 0, "typed failed command")
    need(type(row["exit"]) is int and row["exit"] == 1 and row["error"] is None
         and row["group_absent"] is True and type(row["pid"]) is int and row["pid"] > 0,
         "failed command exited and reaped")
    need(type(row["started_ns"]) is int and type(row["finished_ns"]) is int
         and 0 < row["started_ns"] <= row["finished_ns"], "failed command times")
    env = row["environment"]
    need(env is None or type(env) is dict and all(type(k) is str and type(v) is str for k, v in env.items()),
         "failed command environment")
    digest = row["stdin_sha256"]
    need(digest is None or type(digest) is str and OLD.LOWER_SHA256.fullmatch(digest), "failed command stdin")
    for stream in ("stdout", "stderr"):
        digest = row[stream + "_sha256"]
        need(type(digest) is str and OLD.LOWER_SHA256.fullmatch(digest)
             and digest == sha(folder / stream), "failed command output digest")
    return row


def records(folder, names, failed=()):
    need({p.name for p in folder.iterdir() if p.is_dir()} == set(names), "exact command roster")
    result, previous = {}, 0
    for name in names:
        row = failed_receipt(folder / name) if name in failed else OLD.verify_receipt(folder / name)
        need(row["started_ns"] >= previous, "command chronology: " + name)
        previous = row["finished_ns"]
        result[name] = row
    return result


def utc_ns(value):
    need(type(value) is str, "UTC timestamp string")
    match = re.fullmatch(r"([0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2})\.([0-9]{9})Z", value)
    need(match is not None, "nanosecond UTC timestamp")
    parsed = datetime.strptime(match[1], "%Y-%m-%dT%H:%M:%S").replace(tzinfo=timezone.utc)
    elapsed = parsed - datetime(1970, 1, 1, tzinfo=timezone.utc)
    return (elapsed.days * 86400 + elapsed.seconds) * 10**9 + int(match[2])


def verify(folder=HERE, sealed=True):
    expected = {"README.md", "verify.py", "test_verify.py", "local", "remote", "payload", "qualification",
                "binding.json", "owner.json", "collection.json", "remote-inventory.json"}
    if (folder / "inventory.json").exists():
        expected.add("inventory.json")
    need({p.name for p in folder.iterdir()} == expected, "exact packet closure")
    if sealed:
        need(read(folder / "inventory.json") == inventory(folder), "archive seal")
    binding, marker = read(folder / "binding.json"), read(folder / "owner.json")
    need(type(binding) is dict and set(binding) == {"commit", "controls", "devices", "features", "git_tree", "local_build_environment",
         "local_source_files", "order", "payload", "profile", "source_archive_scope", "source_files", "target"}, "exact binding schema")
    need(binding["source_archive_scope"] == "remote observer input; KFD uses signed full source checkpoint", "source archive scope")
    need(marker["commit"] == SOURCE and marker["binding_sha256"] == sha(folder / "binding.json"), "bound source")
    owned = B.owned_path(marker, exists=False)
    need(binding["commit"] == SOURCE and H.same_json(binding["devices"], DEVICES)
         and H.same_json(binding["controls"], N.CONTROLS) and binding["order"] == list(N.ORDER), "fixed campaign")
    need(binding["target"] == "x86_64-unknown-linux-musl" and binding["features"] == "default"
         and binding["profile"] == "release default opt-level=3", "release build scope")
    tree = subprocess.check_output(["git", "rev-parse", SOURCE + "^{tree}"], cwd=ROOT, text=True).strip()
    need(binding["git_tree"] == tree and binding["local_source_files"] == signed_sources(), "complete signed source inventory")
    observer = "benchmarks/runtime_gfx942/copy-host-observe.py"
    need(binding["source_files"] == {observer: hashlib.sha256(blob(observer)).hexdigest()}, "signed observer")
    need(set(binding["payload"]) == N.PAYLOAD
         and all(type(v) is str and re.fullmatch(r"[0-9a-f]{64}", v) for v in binding["payload"].values()), "payload roster and hashes")
    for name, path in {"native.py": Path(N.__file__), "results.py": Path(R.__file__), "hot.py": Path(H.__file__), "base.py": Path(B.__file__)}.items():
        need(binding["payload"][name] == sha(path), "payload code identity")
    need(B.inventory(folder / "payload") == binding["payload"], "retained payload bytes")
    need(binding["payload"]["kfd-owner"] == ELF_SHA, "exact failed-attempt ELF")
    with tarfile.open(folder / "payload/source.tar.gz", "r:gz") as archive:
        B.validate_members(archive.getmembers(), binding["source_files"])
        for member in archive:
            need(hashlib.sha256(archive.extractfile(member).read()).hexdigest() == binding["source_files"][member.name],
                 "signed observer archive bytes")
    need(H.same_json(read(folder / "collection.json"), {"source_commit": SOURCE, "failures": ["RuntimeError('native failed')"], "owned_cleanup": True,
         "exclusive_reservation": False, "performance_acceptance": False, "formal_refinement": False}), "failed attempt with complete collection/cleanup")
    remote = folder / "remote"
    need(B.inventory(remote) == read(folder / "remote-inventory.json"), "byte-exact collection")
    for name in ("source-before.json", "source-after.json"):
        need(read(remote / name) == binding["source_files"], "remote source identity")
    binaries = {"kfd": binding["payload"]["kfd-owner"]}
    need(read(remote / "binaries.json") == binaries == read(remote / "binaries-after.json"), "matching unchanged ELF")
    need(H.same_json(read(remote / "finished.json"), {"commit": SOURCE, "failures": ["RuntimeError('directed-owner failed')"], "native_execution": False,
         "exclusive_reservation": False, "performance_acceptance": False, "formal_refinement": False}), "native campaign remains unqualified")

    trials = N.trial_specs(owned, DEVICES)
    specs = N.IDENTITIES.copy()
    for name, command, _ in trials:
        for phase in ("before", "workload", "settled", "delayed"):
            if phase == "workload":
                specs.append((name, command, 300))
            else:
                for index, bdf, uid in DEVICES:
                    label = f"{name}-{phase}-gpu{index}"
                    specs.append((label, H.observe_spec(label, index, bdf, uid), 100))
    specs += [("after-" + name, command, seconds) for name, command, seconds in N.IDENTITIES]
    rec = records(remote, [name for name, _, _ in specs], failed=("directed-owner",))
    need({p.name for p in remote.iterdir()} == set(rec) | {"source-before.json", "source-after.json", "binaries.json",
         "binaries-after.json", "validated-results.json", "finished.json"}, "exact remote closure")
    for name, command, seconds in specs:
        row = rec[name]
        need(row["command"] == command and row["timeout_seconds"] == seconds and row["cwd"] == str(owned / "source")
             and row["environment"] == H.environment(owned) and row["stdin_sha256"] is None, "exact remote command: " + name)
        if name != "directed-owner":
            need((remote / name / "stderr").read_bytes() == b"", "empty remote stderr")
    previous_endpoint = 0
    for phase in ("before", "settled", "delayed"):
        for index, bdf, uid in DEVICES:
            observation = H.parse_endpoint((remote / f"directed-owner-{phase}-gpu{index}" / "stdout").read_bytes(), index, bdf, uid)
            enclosing = rec[f"directed-owner-{phase}-gpu{index}"]
            need(enclosing["started_ns"] <= utc_ns(observation["started"]["utc"])
                 <= utc_ns(observation["finished"]["utc"]) <= enclosing["finished_ns"], "fresh endpoint in matching command")
            need(previous_endpoint <= H.stamp(observation["started"]), "endpoint chronology")
            previous_endpoint = H.stamp(observation["finished"])
    first, last = DEVICES[0][0], DEVICES[-1][0]
    need(rec[f"directed-owner-settled-gpu{first}"]["started_ns"] - rec["directed-owner"]["finished_ns"] >= 2 * 10**9, "settled delay")
    need(rec[f"directed-owner-delayed-gpu{first}"]["started_ns"] - rec[f"directed-owner-settled-gpu{last}"]["finished_ns"] >= 20 * 10**9, "delayed postflight")
    need((remote / "directed-owner/stdout").read_bytes() == b""
         and sha(remote / "directed-owner/stderr") == WORKLOAD_ERROR_SHA, "exact original workload refusal and retention")
    need(H.same_json([], read(remote / "validated-results.json")), "no passing workload receipt")
    for name, _, _ in N.IDENTITIES:
        for stream in ("stdout", "stderr"):
            need(sha(remote / name / stream) == sha(remote / ("after-" + name) / stream), "host continuity")

    local = folder / "local"
    tests = {"test_xgmi_directed_owner_results.py": 5, "test_xgmi_directed_owner_campaign.py": 4,
             "test_xgmi_directed_owner_native.py": 5}
    order = ["source-signature", "rustc", "cargo", "build-kfd", "rust-tests", *tests,
             "after-rustc", "after-cargo", "create", "upload", "native", "inventory", "collect", "cleanup", "absence"]
    local_rec = records(local, order, failed=("native",))
    need({p.name for p in local.iterdir()} == set(order), "exact local closure")
    env = {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin", "LANG": "C", "LC_ALL": "C",
           "CARGO_TARGET_DIR": TARGET, "CARGO_INCREMENTAL": "0", "CARGO_BUILD_JOBS": "2", "CARGO_TERM_COLOR": "never",
           "RUSTUP_TOOLCHAIN": "nightly-2026-04-03"}
    need(binding["local_build_environment"] == env, "canonical build environment")
    commands = {"source-signature": (["git", "-c", "gpg.ssh.allowedSignersFile=" + C.SIGNERS, "verify-commit", SOURCE], 30, None)}
    for name, command in (("rustc", ["rustc", "-vV"]), ("cargo", ["cargo", "-V"])):
        commands[name] = (command, 1200, env)
        commands["after-" + name] = (command, 30, env)
        for stream in ("stdout", "stderr"):
            need(sha(local / name / stream) == sha(local / ("after-" + name) / stream), "local tool continuity")
    base = ["cargo", "build", "--locked", "--release", "-q", "-p", "fe2o3-runtime", "--target", "x86_64-unknown-linux-musl", "--example", EXAMPLE]
    commands["build-kfd"] = (base.copy(), 1200, env)
    base[1] = "test"
    commands["rust-tests"] = (base.copy(), 1200, env)
    need(re.findall(rb"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;",
                    (local / "rust-tests/stdout").read_bytes()) == [(b"6", b"0", b"0", b"0", b"0")], "release example tests")
    binary = TARGET + "/x86_64-unknown-linux-musl/release/examples/" + EXAMPLE
    for name in tests:
        commands[name] = (["/usr/bin/python3", "-I", "-B", EXECUTION_ROOT + "/benchmarks/runtime_gfx942/" + name], 120,
                          {**env, "FE2O3_DIRECTED_OWNER_RUST_BINARY": binary})
        count = tests[name]
        need(re.fullmatch(r"\." * count + r"\n-+\nRan " + str(count) + r" tests in [0-9.]+s\n\nOK\n",
                         (local / name / "stderr").read_text()), "CPU parser/controller checks without skips")
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    for mode in ("create", "inventory", "cleanup", "absence"):
        commands[mode] = (["ssh", "-T", *C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", mode, serialized])], 120, None)
        need(local_rec[mode]["stdin_sha256"] == hashlib.sha256(C.control_bytes(N.PREFIX)).hexdigest(), "ownership controller stdin")
    commands["upload"] = (["scp", "-q", *C.SSH, "--", *(LOCAL_OUTPUT + "/payload/" + name for name in sorted(N.PAYLOAD)),
                           LOCAL_OUTPUT + "/binding.json", "mi300x:" + str(owned) + "/"], 120, None)
    commands["native"] = (["ssh", "-T", *C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", str(owned / "native.py"), "run", serialized])], 1200, None)
    commands["collect"] = (["scp", "-q", "-r", *C.SSH, "--", "mi300x:" + str(owned / "results"), LOCAL_OUTPUT + "/remote"], 120, None)
    need(set(commands) == set(order), "complete local commands")
    for name, (command, seconds, environment) in commands.items():
        row = local_rec[name]
        need(row["command"] == command and row["timeout_seconds"] == seconds and row["environment"] == environment
             and row["cwd"] == EXECUTION_ROOT, "exact local command: " + name)
        if name not in ("create", "inventory", "cleanup", "absence"):
            need(row["stdin_sha256"] is None, "no hidden local input")
        if name not in ("source-signature", "native", *tests):
            need((local / name / "stderr").read_bytes() == b"", "empty local stderr")
    need(sha(local / "native/stderr") == NATIVE_ERROR_SHA, "exact failed native controller traceback")
    need((local / "source-signature/stdout").read_bytes() == SIGNATURE.stdout
         and (local / "source-signature/stderr").read_bytes() == SIGNATURE.stderr, "signature receipt")
    need((local / "rustc/stdout").read_text().splitlines() == [
        "rustc 1.96.0-nightly (55e86c996 2026-04-02)", "binary: rustc", "commit-hash: 55e86c996809902e8bbad512cfb4d2c18be446d9",
        "commit-date: 2026-04-02", "host: x86_64-unknown-linux-gnu", "release: 1.96.0-nightly", "LLVM version: 22.1.2"], "pinned compiler")
    need((local / "cargo/stdout").read_bytes() == b"cargo 1.96.0-nightly (888f67534 2026-03-30)\n", "pinned Cargo")
    need(read(local / "create/stdout") == marker and read(local / "inventory/stdout") == read(folder / "remote-inventory.json"), "owner and collection roster")
    need(read(local / "cleanup/stdout") == {"removed": str(owned)}
         and H.same_json(read(local / "absence/stdout"), {"path_absent": True, "processes_absent": True}), "cleanup and process absence")
    verify_cpu_archive(folder / "qualification", local, local_rec["source-signature"]["started_ns"])
    return {"source_commit": SOURCE, "native_process_executed": True, "native_graph_qualified": False,
            "planned_directed_copies": 4, "endpoint_observations": 6,
            "example_tests": 6, "python_tests": 14, "failure": "unordered shared-source admission",
            "owner_disposition": "retained-until-process-exit", "owned_cleanup": True,
            "exclusive_reservation": False, "performance_acceptance": False, "formal_refinement": False}


def verify_cpu_archive(archive, local, campaign_started):
    relative = "docs/evidence/dev-directed-owner-witness-cpu-2026-09-24"
    names = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", SOURCE, "--", relative],
                                    cwd=ROOT, text=True).splitlines()
    expected = {name.removeprefix(relative + "/"): hashlib.sha256(blob(name)).hexdigest() for name in names}
    need(bool(expected) and B.inventory(archive) == expected, "signed CPU archive")
    verify_cpu_contents(archive, local, campaign_started)


def verify_cpu_contents(archive, local, campaign_started):
    before, after = read(archive / "inputs-before.json"), read(archive / "inputs-after.json")
    need(H.same_json(before, after) and set(before) == {"runner", "source"}, "unchanged CPU inputs")
    roots = ("crates/", ".cargo/", "benchmarks/runtime_gfx942/")
    single = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml"}
    sources = {name: digest for name, digest in signed_sources().items()
               if (name in single or name.startswith(roots))
               and (name.endswith((".rs", ".toml", ".lock", ".json", ".py")) or "/fixtures/" in name)}
    need(before["source"] == sources and before["runner"] == sha(archive / "runner.py"), "exact signed CPU inputs")
    cargo = ["cargo", "--locked"]
    commands = {
        "rustc": ["rustc", "-vV"], "cargo": ["cargo", "-V"],
        "format": ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"],
        "gnu": cargo + ["test", "-p", "fe2o3-runtime", "--example", EXAMPLE],
        "musl": cargo + ["test", "-p", "fe2o3-runtime", "--example", EXAMPLE, "--target", "x86_64-unknown-linux-musl"],
        "build-cli": cargo + ["build", "-p", "fe2o3-runtime", "--example", EXAMPLE, "--target", "x86_64-unknown-linux-musl"],
        "clippy": cargo + ["clippy", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"],
        "results-tests": ["/usr/bin/python3", "-I", "-B", "/home/harsh/.codex-tmp/fe2o3-r61-execution/benchmarks/runtime_gfx942/test_xgmi_directed_owner_results.py"],
        "campaign-tests": ["/usr/bin/python3", "-I", "-B", "/home/harsh/.codex-tmp/fe2o3-r61-execution/benchmarks/runtime_gfx942/test_xgmi_directed_owner_campaign.py"],
        "native-runner-tests": ["/usr/bin/python3", "-I", "-B", "/home/harsh/.codex-tmp/fe2o3-r61-execution/benchmarks/runtime_gfx942/test_xgmi_directed_owner_native.py"],
        "after-rustc": ["rustc", "-vV"], "after-cargo": ["cargo", "-V"],
    }
    need({p.name for p in archive.iterdir()} == set(commands) | {"README.md", "runner.py", "inputs-before.json", "inputs-after.json"}, "CPU archive closure")
    previous = 0
    for name, command in commands.items():
        folder = archive / name
        need({p.name for p in folder.iterdir()} == {"record.json", "stdout.log", "stderr.log"}, "CPU command closure")
        record = read(folder / "record.json")
        need(set(record) == {"command", "started_ns", "process_group", "group_absent", "finished_ns", "status"}
             and record["command"] == command and type(record["status"]) is int and record["status"] == 0
             and record["group_absent"] is True, "successful exact CPU command")
        need(all(type(record[key]) is int and record[key] > 0 for key in ("started_ns", "finished_ns", "process_group"))
             and previous <= record["started_ns"] <= record["finished_ns"], "CPU chronology")
        previous = record["finished_ns"]
        if name in ("gnu", "musl"):
            counts = re.findall(rb"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", (folder / "stdout.log").read_bytes())
            need(counts == [(b"6", b"0", b"0", b"0", b"0")], "CPU example tests")
            tests = re.findall(rb"^test (tests::[a-z_]+) \.\.\. ok$", (folder / "stdout.log").read_bytes(), re.MULTILINE)
            need(len(tests) == 6 and set(tests) == {
                b"tests::arguments_reject_wrong_count_zero_duplicate_and_overflow",
                b"tests::plan_has_exact_routes_ranges_and_byte_accounting",
                b"tests::oracle_detects_payload_guard_and_source_boundary_mutations",
                b"tests::expired_deadline_rejects_even_ready_observation",
                b"tests::dropped_gate_sender_unblocks_receiver",
                b"tests::cleanup_failure_does_not_replace_original_workload_error"}, "named CPU test roster")
        if name.endswith("-tests"):
            count = {"results-tests": 5, "campaign-tests": 4, "native-runner-tests": 5}[name]
            need(re.fullmatch(r"\." * count + r"\n-+\nRan " + str(count) + r" tests in [0-9.]+s\n\nOK\n",
                             (folder / "stderr.log").read_text()), "CPU Python tests without skips")
    for tool in ("rustc", "cargo"):
        for stream in ("stdout.log", "stderr.log"):
            need(sha(archive / tool / stream) == sha(archive / ("after-" + tool) / stream)
                 == sha(local / tool / stream.removesuffix(".log")), "CPU and native tool continuity")
    need(previous <= campaign_started, "CPU qualification precedes native campaign")


if __name__ == "__main__":
    need(sys.argv[1:] in ([], ["--seal"]), "usage: verify.py [--seal]")
    if sys.argv[1:] == ["--seal"]:
        verify(sealed=False)
        need(not (HERE / "inventory.json").exists(), "already sealed")
        (HERE / "inventory.json").write_text(json.dumps(inventory(HERE), indent=2) + "\n", encoding="ascii")
    print(json.dumps(verify(), sort_keys=True))
