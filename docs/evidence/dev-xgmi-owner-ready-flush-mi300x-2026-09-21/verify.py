#!/usr/bin/env python3
"""Offline correctness replay; authenticity relies on the signed archive commit."""

import sys
if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import importlib.util
import io
import json
from functools import lru_cache
from pathlib import Path
import re
import shlex
import subprocess
import tarfile

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SOURCE = "c44625e12a1567204e1cb186b9f4da867244f686"
EXECUTION_ROOT = "/home/harsh/.codex-tmp/fe2o3-r61-execution"
LOCAL_OUTPUT = "/home/harsh/.codex-tmp/fe2o3-xgmi-owner-results-f6gc9i5x"
TARGET = "/dev/shm/fe2o3-link-parser-build-20260920.2LJm2Q0Q/target"
EXAMPLE = "gfx942-runtime-xgmi-segments-owner-smoke"
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
    "benchmarks/runtime_gfx942/xgmi_segments_owner_native.py",
    "benchmarks/runtime_gfx942/xgmi_segments_owner_results.py",
):
    need(sha(ROOT / relative) == hashlib.sha256(blob(relative)).hexdigest(), "signed import: " + relative)
OLD = load("docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/campaign.py", "owner_receipts")
C = load("benchmarks/runtime_gfx942/xgmi_peer_segments_campaign.py", "owner_controls")
N = load("benchmarks/runtime_gfx942/xgmi_segments_owner_native.py", "owner_native")
R = load("benchmarks/runtime_gfx942/xgmi_segments_owner_results.py", "owner_results")
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


def records(folder, names):
    need({p.name for p in folder.iterdir() if p.is_dir()} == set(names), "exact command roster")
    result, previous = {}, 0
    for name in names:
        row = OLD.verify_receipt(folder / name)
        need(row["started_ns"] >= previous, "command chronology: " + name)
        previous = row["finished_ns"]
        result[name] = row
    return result


def verify(folder=HERE, sealed=True):
    expected = {"README.md", "verify.py", "test_verify.py", "qualification", "qualification-finished.json", "local", "remote",
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
    need(H.same_json(read(folder / "collection.json"), {"source_commit": SOURCE, "failures": [], "owned_cleanup": True,
         "exclusive_reservation": False, "performance_acceptance": False, "formal_refinement": False}), "successful scoped collection")
    remote = folder / "remote"
    need(B.inventory(remote) == read(folder / "remote-inventory.json"), "byte-exact collection")
    for name in ("source-before.json", "source-after.json"):
        need(read(remote / name) == binding["source_files"], "remote source identity")
    binaries = {"kfd": binding["payload"]["kfd-owner"]}
    need(read(remote / "binaries.json") == binaries == read(remote / "binaries-after.json"), "matching unchanged ELF")
    need(H.same_json(read(remote / "finished.json"), {"commit": SOURCE, "failures": [], "native_execution": True,
         "exclusive_reservation": False, "performance_acceptance": False, "formal_refinement": False}), "qualified native campaign")

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
    rec = records(remote, [name for name, _, _ in specs])
    need({p.name for p in remote.iterdir()} == set(rec) | {"source-before.json", "source-after.json", "binaries.json",
         "binaries-after.json", "validated-results.json", "finished.json"}, "exact remote closure")
    for name, command, seconds in specs:
        row = rec[name]
        need(row["command"] == command and row["timeout_seconds"] == seconds and row["cwd"] == str(owned / "source")
             and row["environment"] == H.environment(owned) and row["stdin_sha256"] is None, "exact remote command: " + name)
        need((remote / name / "stderr").read_bytes() == b"", "empty remote stderr")
    previous_endpoint = 0
    for phase in ("before", "settled", "delayed"):
        for index, bdf, uid in DEVICES:
            observation = H.parse_endpoint((remote / f"owner-{phase}-gpu{index}" / "stdout").read_bytes(), index, bdf, uid)
            need(previous_endpoint <= H.stamp(observation["started"]), "endpoint chronology")
            previous_endpoint = H.stamp(observation["finished"])
    need(rec["owner-settled-gpu5"]["started_ns"] - rec["owner"]["finished_ns"] >= 2 * 10**9, "settled delay")
    need(rec["owner-delayed-gpu5"]["started_ns"] - rec["owner-settled-gpu6"]["finished_ns"] >= 20 * 10**9, "delayed postflight")
    parsed = [{"trial": "owner", "result": R.parse_receipt((remote / "owner/stdout").read_bytes(), unique_ids=[int(d[2], 16) for d in DEVICES])}]
    need(H.same_json(parsed, read(remote / "validated-results.json")), "independent result replay")
    for name, _, _ in N.IDENTITIES:
        for stream in ("stdout", "stderr"):
            need(sha(remote / name / stream) == sha(remote / ("after-" + name) / stream), "host continuity")

    local = folder / "local"
    tests = ["test_xgmi_segments_owner_results.py", "test_xgmi_segments_owner_campaign.py"]
    order = ["source-signature", "rustc", "cargo", "build-kfd", "rust-tests", *tests,
             "after-rustc", "after-cargo", "create", "upload", "native", "inventory", "collect", "cleanup", "absence"]
    local_rec = records(local, order)
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
    need(re.search(rb"test result: ok\. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;", (local / "rust-tests/stdout").read_bytes()), "release example tests")
    binary = TARGET + "/x86_64-unknown-linux-musl/release/examples/" + EXAMPLE
    for name in tests:
        commands[name] = (["/usr/bin/python3", "-I", "-B", EXECUTION_ROOT + "/benchmarks/runtime_gfx942/" + name], 120,
                          {**env, "FE2O3_OWNER_RUST_BINARY": binary})
        need(re.fullmatch(r"\.\.\.\n-+\nRan 3 tests in [0-9.]+s\n\nOK\n", (local / name / "stderr").read_text()), "CPU parser/controller checks without skips")
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
        if name not in ("source-signature", *tests):
            need((local / name / "stderr").read_bytes() == b"", "empty local stderr")
    need((local / "source-signature/stdout").read_bytes() == SIGNATURE.stdout
         and (local / "source-signature/stderr").read_bytes() == SIGNATURE.stderr, "signature receipt")
    need((local / "rustc/stdout").read_text().splitlines() == [
        "rustc 1.96.0-nightly (55e86c996 2026-04-02)", "binary: rustc", "commit-hash: 55e86c996809902e8bbad512cfb4d2c18be446d9",
        "commit-date: 2026-04-02", "host: x86_64-unknown-linux-gnu", "release: 1.96.0-nightly", "LLVM version: 22.1.2"], "pinned compiler")
    need((local / "cargo/stdout").read_bytes() == b"cargo 1.96.0-nightly (888f67534 2026-03-30)\n", "pinned Cargo")
    need(read(local / "create/stdout") == marker and read(local / "inventory/stdout") == read(folder / "remote-inventory.json"), "owner and collection roster")
    need(read(local / "cleanup/stdout") == {"removed": str(owned)}
         and H.same_json(read(local / "absence/stdout"), {"path_absent": True, "processes_absent": True}), "cleanup and process absence")
    qualified = verify_qualification(folder, env)
    need(qualified["source-after"]["finished_ns"] <= local_rec["source-signature"]["started_ns"], "CPU qualification precedes campaign")
    return {"source_commit": SOURCE, "trials": 1, "ordered_lists": 2, "endpoint_observations": 6, "runtime_passed_per_target": 1221,
            "pending_dataflow": "refused_before_submission", "owned_cleanup": True, "performance_acceptance": False, "formal_refinement": False}


def verify_qualification(folder, env):
    qualification = folder / "qualification"
    order = ["source-before", "signature", "x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl", "no-default-features",
             "clippy", "r74-model", "rustfmt", "diff-check", "source-after"]
    qualified = records(qualification, order)
    need({p.name for p in qualification.iterdir()} == set(order), "exact qualification closure")
    need(H.same_json(read(folder / "qualification-finished.json"), {"source_commit": SOURCE, "qualified": True,
         "native_execution": False, "performance_acceptance": False, "formal_refinement": False}), "finished qualification")
    test_env = {**env, "RUST_TEST_THREADS": "8", "CARGO_PROFILE_DEV_OPT_LEVEL": "1", "CARGO_PROFILE_DEV_DEBUG": "0",
        "CARGO_PROFILE_DEV_DEBUG_ASSERTIONS": "true", "CARGO_PROFILE_DEV_OVERFLOW_CHECKS": "true", "CARGO_PROFILE_TEST_OPT_LEVEL": "1",
        "CARGO_PROFILE_TEST_DEBUG": "0", "CARGO_PROFILE_TEST_DEBUG_ASSERTIONS": "true", "CARGO_PROFILE_TEST_OVERFLOW_CHECKS": "true"}
    commands = {
        "source-before": (["git", "diff", "--exit-code", SOURCE], 30),
        "source-after": (["git", "diff", "--exit-code", SOURCE], 30),
        "signature": (["git", "-c", "gpg.ssh.allowedSignersFile=" + C.SIGNERS, "verify-commit", SOURCE], 30),
        "no-default-features": (["cargo", "check", "--locked", "-q", "-p", "fe2o3-runtime", "--no-default-features", "--target", "x86_64-unknown-linux-gnu", "--lib"], 1200),
        "clippy": (["cargo", "clippy", "--locked", "-q", "-p", "fe2o3-runtime", "--all-features", "--target", "x86_64-unknown-linux-gnu",
                    "--lib", "--tests", "--example", EXAMPLE, "--", "-D", "warnings"], 1200),
        "r74-model": (["cargo", "test", "--locked", "-q", "-p", "fe2o3-runtime-model", "--target", "x86_64-unknown-linux-gnu", "--lib", "r74_ordered_peer_copy"], 1200),
        "rustfmt": (["rustfmt", "--edition", "2024", "--check", "crates/fe2o3-runtime/src/kfd_backend/xgmi_segments.rs",
            "crates/fe2o3-runtime/src/kfd_backend/xgmi_segments_diagnostic/tests.rs", "crates/fe2o3-runtime/src/context/tests/peer_segments_tests.rs",
            "crates/fe2o3-runtime/src/async_engine/tests/owned_tests/snapshot_tests.rs", "crates/fe2o3-runtime/examples/" + EXAMPLE + ".rs"], 120),
        "diff-check": (["git", "diff", "--check", "161b295d56b86dcf4176a206e778bc1f9d9c5b0c", SOURCE], 30),
    }
    for target in ("x86_64-unknown-linux-gnu", "x86_64-unknown-linux-musl"):
        commands[target] = (["cargo", "test", "--locked", "-q", "-p", "fe2o3-runtime", "--all-features", "--target", target,
                            "--lib", "--example", EXAMPLE], 2400)
    for name, (command, seconds) in commands.items():
        row = qualified[name]
        need(row["command"] == command and row["cwd"] == EXECUTION_ROOT and row["environment"] == test_env
             and row["timeout_seconds"] == seconds and row["stdin_sha256"] is None, "exact CPU command: " + name)
        raw, error = (qualification / name / "stdout").read_bytes(), (qualification / name / "stderr").read_bytes()
        if name.startswith("x86_64-"):
            counts = re.findall(rb"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; (\d+) filtered out;", raw)
            need(counts == [(b"1221", b"20", b"0"), (b"3", b"0", b"0")] and error == b"", "full runtime and example results")
        elif name == "r74-model":
            need(re.search(rb"test result: ok\. 3 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out;", raw)
                 and error == b"", "R74 model regression")
        elif name == "signature":
            need(raw == SIGNATURE.stdout and error == SIGNATURE.stderr, "CPU signed source")
        else:
            need(raw == b"" and error == b"", "clean CPU/source check: " + name)
    return qualified


if __name__ == "__main__":
    need(sys.argv[1:] in ([], ["--seal"]), "usage: verify.py [--seal]")
    if sys.argv[1:] == ["--seal"]:
        verify(sealed=False)
        need(not (HERE / "inventory.json").exists(), "already sealed")
        (HERE / "inventory.json").write_text(json.dumps(inventory(HERE), indent=2) + "\n", encoding="ascii")
    print(json.dumps(verify(), sort_keys=True))
