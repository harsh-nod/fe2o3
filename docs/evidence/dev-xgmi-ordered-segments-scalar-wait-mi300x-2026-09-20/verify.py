#!/usr/bin/env python3
"""Offline receipt replay. Authenticity relies on the signed archive commit."""

import sys
if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shlex
import subprocess

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SOURCE = "e563ea48c4d9de251052feac58fb5db2252020b4"
EXECUTION_ROOT = "/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917"
LOCAL_OUTPUT = "/home/harsh/.codex-tmp/fe2o3-xgmi-segments-results-u59o762l"
TARGET = "/dev/shm/fe2o3-link-parser-build-20260920.2LJm2Q0Q/target"
SOURCE_SELECTORS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples",
                    "benchmarks/runtime_gfx942", "scripts/unsafe-source-baseline.json", "docs/runtime-primary-queue-release-v1.md"]
DEVICES = [[1, "0000:26:00.0", "0xab83d2ffef0d3cdf"], [2, "0000:46:00.0", "0xd2e26fef80cf5c33"]]
SIGNERS = ROOT / "docs/evidence/dev-combined-sdma-release-native-gpu2-2026-09-18/raw/native/collected/allowed-signers"


def need(value, message):
    if not value:
        raise RuntimeError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary file: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load(relative, digest, name):
    path = ROOT / relative
    need(sha(path) == digest, "pinned helper: " + relative)
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


need(sha(ROOT / "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/native.py") == "820ad87e74a1f9915c2eb7d2d7c7c6c1c451da4cecf29fcc381229324d98473b", "prior native helper before import")
need(sha(ROOT / "benchmarks/runtime_gfx942/xgmi_peer_segments_native.py") == "d47da25e4f1a542a10b1c15cd86371c83593efbf70ebc1286b3453eca7632149", "native helper before import")
OLD = load("docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/campaign.py", "6d62e802ef177540741915c7e4177b2ad5348e746de7c439b74329c74d4fd869", "segments_receipt_shape")
C = load("benchmarks/runtime_gfx942/xgmi_peer_segments_campaign.py", "7b41f1f70eff44dcd591cb1ee23e4e5de1e5694054db5153318a5bfa99194e28", "segments_campaign_replay")
N = C.N
need(sha(Path(N.__file__)) == "d47da25e4f1a542a10b1c15cd86371c83593efbf70ebc1286b3453eca7632149", "native runner identity")
R = load("benchmarks/runtime_gfx942/xgmi_peer_segments_results.py", "64a15ee95a07a427665fbc853069abe648fe63e8470e3028b9ac245f30313515", "segments_result_replay")
H, B = N.H, N.B


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
    expected = {"README.md", "verify.py", "test_verify.py", "qualification", "local", "remote", "binding.json", "owner.json", "collection.json", "remote-inventory.json"}
    if (folder / "inventory.json").exists():
        expected.add("inventory.json")
    need({p.name for p in folder.iterdir()} == expected, "exact packet closure")
    if sealed:
        need(read(folder / "inventory.json") == inventory(folder), "archive seal")
    binding, marker = read(folder / "binding.json"), read(folder / "owner.json")
    need(set(binding) == {"commit", "controls", "devices", "features", "git_tree", "local_build_environment", "local_source_files",
         "order", "payload", "profile", "source_archive_scope", "source_files", "target"}, "exact binding schema")
    need(binding["source_archive_scope"] == "remote comparator and observer inputs; KFD uses signed full source checkpoint", "source archive scope")
    need(marker["commit"] == SOURCE and marker["binding_sha256"] == sha(folder / "binding.json"), "bound source")
    owned = B.owned_path(marker, exists=False)
    need(binding["commit"] == SOURCE and binding["devices"] == DEVICES
         and binding["controls"] == N.CONTROLS and binding["order"] == list(N.ORDER), "fixed campaign")
    need(binding["target"] == "x86_64-unknown-linux-musl" and binding["features"] == "default"
         and binding["profile"] == "release default opt-level=3", "KFD build scope")
    need(sha(SIGNERS) == C.SIGNERS_SHA, "trusted signer")
    signature = subprocess.run(["git", "-c", "gpg.ssh.allowedSignersFile=" + str(SIGNERS), "verify-commit", SOURCE],
                               cwd=ROOT, check=True, capture_output=True)
    tree = subprocess.check_output(["git", "rev-parse", SOURCE + "^{tree}"], cwd=ROOT, text=True).strip()
    need(binding["git_tree"] == tree, "signed source tree")
    source_names = subprocess.check_output(["git", "ls-tree", "-r", "--name-only", SOURCE, "--", *SOURCE_SELECTORS], cwd=ROOT, text=True).splitlines()
    need(set(source_names) == set(binding["local_source_files"]), "complete signed source roster")
    subprocess.run(["git", "diff", "--exit-code", SOURCE, "--", *binding["local_source_files"]],
                   cwd=ROOT, check=True, capture_output=True)
    need({name: sha(ROOT / name) for name in binding["local_source_files"]} == binding["local_source_files"], "local source inventory")
    need(binding["source_files"] == {name: sha(ROOT / name) for name in C.SOURCE_FILES}, "remote comparator inputs")
    need(set(binding["payload"]) == N.PAYLOAD, "payload roster")
    for name, path in {"native.py": Path(N.__file__), "results.py": Path(R.__file__),
                       "hot.py": Path(H.__file__), "base.py": Path(B.__file__)}.items():
        need(binding["payload"][name] == sha(path), "payload code identity")
    need(read(folder / "collection.json") == {"source_commit": SOURCE, "failures": [], "owned_cleanup": True,
         "exclusive_reservation": False, "performance_acceptance": False}, "successful scoped collection")
    remote = folder / "remote"
    need(B.inventory(remote) == read(folder / "remote-inventory.json"), "remote collection identity")
    for name in ("source-before.json", "source-after.json"):
        need(read(remote / name) == binding["source_files"], "remote source identity")
    binaries = read(remote / "binaries.json")
    need(set(binaries) == set(N.BINARIES) and all(re.fullmatch(r"[0-9a-f]{64}", value) for value in binaries.values()), "three binary identities")
    need(binaries == read(remote / "binaries-after.json") and binaries["kfd"] == binding["payload"]["kfd-segments"], "unchanged matching binaries")
    need(read(remote / "finished.json") == {"commit": SOURCE, "failures": [], "native_execution": True,
         "exclusive_reservation": False, "performance_acceptance": False, "formal_refinement": False}, "bounded native completion")

    trials = N.trial_specs(owned, DEVICES)
    specs = N.IDENTITIES + N.build_specs(owned)
    for name, _, command, env in trials:
        for phase in ("before", "workload", "settled", "delayed"):
            if phase == "workload":
                specs.append((name, command, 180))
            else:
                for index, bdf, uid in DEVICES:
                    label = f"{name}-{phase}-gpu{index}"
                    specs.append((label, H.observe_spec(label, index, bdf, uid), 100))
    specs += [("after-" + name, command, seconds) for name, command, seconds in N.IDENTITIES]
    rec = records(remote, [name for name, _, _ in specs])
    need({p.name for p in remote.iterdir()} == set(rec) | {"source-before.json", "source-after.json", "binaries.json",
         "binaries-after.json", "validated-results.json", "finished.json"}, "exact remote result closure")
    environments = {name: env for name, _, _, env in trials}
    for name, command, seconds in specs:
        row = rec[name]
        need(row["command"] == command and row["timeout_seconds"] == seconds and row["cwd"] == str(owned / "source")
             and row["environment"] == environments.get(name, H.environment(owned)) and row["stdin_sha256"] is None, "exact remote command: " + name)
        need((remote / name / "stderr").read_bytes() == b"", "empty remote stderr")
    parsed, previous_endpoint = [], 0
    for name, backend, _, _ in trials:
        for phase in ("before", "settled", "delayed"):
            for index, bdf, uid in DEVICES:
                observation = H.parse_endpoint((remote / f"{name}-{phase}-gpu{index}" / "stdout").read_bytes(), index, bdf, uid)
                need(previous_endpoint <= H.stamp(observation["started"]), "endpoint chronology")
                previous_endpoint = H.stamp(observation["finished"])
        need(rec[name + "-settled-gpu1"]["started_ns"] - rec[name]["finished_ns"] >= 2 * 10**9, "settled delay")
        need(rec[name + "-delayed-gpu1"]["started_ns"] - rec[name + "-settled-gpu2"]["finished_ns"] >= 20 * 10**9, "delayed postflight")
        parsed.append({"trial": name, "result": R.parse_receipt((remote / name / "stdout").read_bytes(),
            backend=backend, unique_ids=[int(d[2], 16) for d in DEVICES], **N.CONTROLS)})
    need(H.same_json(parsed, read(remote / "validated-results.json")), "independent result replay")
    for name, _, _ in N.IDENTITIES:
        for stream in ("stdout", "stderr"):
            need(sha(remote / name / stream) == sha(remote / ("after-" + name) / stream), "unchanged remote toolchain")

    local = folder / "local"
    order = ["source-signature", "rustc", "cargo", "build-kfd", "after-rustc", "after-cargo", "rust-tests",
             "test_xgmi_peer_segments.py", "test_xgmi_peer_segments_results.py", "test_xgmi_peer_segments_campaign.py",
             "create", "upload", "native", "inventory", "collect", "cleanup", "absence"]
    local_rec = records(local, order)
    need({p.name for p in local.iterdir()} == set(order), "exact local result closure")
    need(all(row["cwd"] == EXECUTION_ROOT for row in local_rec.values()), "local execution root")
    env = {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
           "LANG": "C", "LC_ALL": "C", "CARGO_TARGET_DIR": TARGET, "CARGO_INCREMENTAL": "0",
           "CARGO_BUILD_JOBS": "2", "CARGO_TERM_COLOR": "never", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03"}
    need(binding["local_build_environment"] == env, "canonical local build environment")
    binary = TARGET + "/x86_64-unknown-linux-musl/release/examples/gfx942-runtime-xgmi-segments-benchmark"
    commands = {
        "source-signature": (["git", "-c", "gpg.ssh.allowedSignersFile=" + C.SIGNERS, "verify-commit", SOURCE], 30, None),
        "rustc": (["rustc", "-vV"], 1200, env), "cargo": (["cargo", "-V"], 1200, env),
        "after-rustc": (["rustc", "-vV"], 30, env), "after-cargo": (["cargo", "-V"], 30, env),
    }
    for name in ("rustc", "cargo"):
        for stream in ("stdout", "stderr"):
            need(sha(local / name / stream) == sha(local / ("after-" + name) / stream), "unchanged local tools")
    base = ["cargo", "build", "--locked", "--release", "-q", "-p", "fe2o3-runtime", "--target", "x86_64-unknown-linux-musl", "--example", "gfx942-runtime-xgmi-segments-benchmark"]
    commands["build-kfd"] = (base.copy(), 1200, env)
    need(local_rec["build-kfd"]["command"] == base and local_rec["build-kfd"]["environment"] == binding["local_build_environment"], "qualified release build")
    base[1] = "test"
    commands["rust-tests"] = (base.copy(), 1200, env)
    need(local_rec["rust-tests"]["command"] == base, "qualified release tests")
    need(re.search(rb"test result: ok\. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;", (local / "rust-tests/stdout").read_bytes()), "Rust test outcome")
    for name, count in (("test_xgmi_peer_segments.py", 2), ("test_xgmi_peer_segments_results.py", 4), ("test_xgmi_peer_segments_campaign.py", 3)):
        commands[name] = (["/usr/bin/python3", "-I", "-B", EXECUTION_ROOT + "/benchmarks/runtime_gfx942/" + name],
                          120, {**env, "FE2O3_SEGMENTS_RUST_BINARY": binary})
        need(local_rec[name]["command"] == ["/usr/bin/python3", "-I", "-B", EXECUTION_ROOT + "/benchmarks/runtime_gfx942/" + name], "CPU replay command")
        outcome = (local / name / "stderr").read_text()
        need(re.fullmatch(r"\." * count + r"\n-+\nRan " + str(count) + r" tests in [0-9.]+s\n\nOK\n", outcome), "CPU replay without skips")
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    for mode in ("create", "inventory", "cleanup", "absence"):
        commands[mode] = (["ssh", "-T", *C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", mode, serialized])], 120, None)
        need(local_rec[mode]["command"] == ["ssh", "-T", *C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", mode, serialized])]
             and local_rec[mode]["stdin_sha256"] == hashlib.sha256(C.control_bytes()).hexdigest(), "exact ownership control")
    commands["upload"] = (["scp", "-q", *C.SSH, "--", *(LOCAL_OUTPUT + "/payload/" + name for name in sorted(N.PAYLOAD)),
                           LOCAL_OUTPUT + "/binding.json", "mi300x:" + str(owned) + "/"], 120, None)
    commands["native"] = (["ssh", "-T", *C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", str(owned / "native.py"), "run", serialized])], 6000, None)
    commands["collect"] = (["scp", "-q", "-r", *C.SSH, "--", "mi300x:" + str(owned / "results"), LOCAL_OUTPUT + "/remote"], 120, None)
    need(set(commands) == set(order), "complete reconstructed local commands")
    for name, (command, seconds, environment) in commands.items():
        row = local_rec[name]
        need(row["command"] == command and row["timeout_seconds"] == seconds and row["environment"] == environment, "exact local command: " + name)
        if name not in ("create", "inventory", "cleanup", "absence"):
            need(row["stdin_sha256"] is None, "no unrecorded local input")
        if name not in ("source-signature", "test_xgmi_peer_segments.py", "test_xgmi_peer_segments_results.py", "test_xgmi_peer_segments_campaign.py"):
            need((local / name / "stderr").read_bytes() == b"", "empty local stderr: " + name)
    need((local / "source-signature/stdout").read_bytes() == signature.stdout
         and (local / "source-signature/stderr").read_bytes() == signature.stderr, "source signature receipt")
    need((local / "rustc/stdout").read_text().splitlines() == [
        "rustc 1.96.0-nightly (55e86c996 2026-04-02)", "binary: rustc",
        "commit-hash: 55e86c996809902e8bbad512cfb4d2c18be446d9", "commit-date: 2026-04-02",
        "host: x86_64-unknown-linux-gnu", "release: 1.96.0-nightly", "LLVM version: 22.1.2"], "pinned compiler identity")
    need((local / "cargo/stdout").read_bytes() == b"cargo 1.96.0-nightly (888f67534 2026-03-30)\n", "pinned Cargo identity")
    need(read(local / "create/stdout") == marker and read(local / "inventory/stdout") == read(folder / "remote-inventory.json"), "remote owner and collection roster")
    need(read(local / "cleanup/stdout") == {"removed": str(owned)}
         and read(local / "absence/stdout") == {"path_absent": True, "processes_absent": True}, "owned cleanup and process absence")
    qualification = folder / "qualification"
    focused = records(qualification, ["scalar-wait", "ordered-runtime", "source-before",
        "scalar-wait-bracketed", "ordered-runtime-bracketed", "source-after"])
    need({p.name for p in qualification.iterdir()} == set(focused), "exact focused result closure")
    test_env = {**env, "RUST_TEST_THREADS": "2",
        "CARGO_PROFILE_DEV_OPT_LEVEL": "1", "CARGO_PROFILE_DEV_DEBUG": "0",
        "CARGO_PROFILE_DEV_DEBUG_ASSERTIONS": "true", "CARGO_PROFILE_DEV_OVERFLOW_CHECKS": "true",
        "CARGO_PROFILE_TEST_OPT_LEVEL": "1", "CARGO_PROFILE_TEST_DEBUG": "0",
        "CARGO_PROFILE_TEST_DEBUG_ASSERTIONS": "true", "CARGO_PROFILE_TEST_OVERFLOW_CHECKS": "true"}
    for name, package, selection, passed, filtered in (
        ("scalar-wait", "fe2o3-kfd", "sdma::tests::xgmi_single_wait", 9, 1524),
        ("ordered-runtime", "fe2o3-runtime", "kfd_backend::xgmi_segments::tests", 6, 1205),
        ("scalar-wait-bracketed", "fe2o3-kfd", "sdma::tests::xgmi_single_wait", 9, 1524),
        ("ordered-runtime-bracketed", "fe2o3-runtime", "kfd_backend::xgmi_segments::tests", 6, 1205),
    ):
        row = focused[name]
        need(row["command"] == ["cargo", "test", "--locked", "-q", "-p", package, "--all-features", "--lib", selection]
             and row["cwd"] == EXECUTION_ROOT and row["environment"] == test_env
             and row["timeout_seconds"] == 1200 and row["stdin_sha256"] is None, "exact focused command: " + name)
        need((qualification / name / "stderr").read_bytes() == b"", "empty focused stderr")
        outcome = (qualification / name / "stdout").read_text()
        need(re.fullmatch(r"\nrunning " + str(passed) + r" tests\n" + r"\." * passed
             + r"\ntest result: ok\. " + str(passed)
             + r" passed; 0 failed; 0 ignored; 0 measured; " + str(filtered)
             + r" filtered out; finished in [0-9.]+s\n\n", outcome), "focused test roster and result: " + name)
        need(row["started_ns"] >= local_rec["source-signature"]["finished_ns"], "focused source checkpoint chronology")
    for name in ("source-before", "source-after"):
        row = focused[name]
        need(row["command"] == ["git", "diff", "--exit-code", SOURCE, "--", *SOURCE_SELECTORS]
             and row["cwd"] == EXECUTION_ROOT and row["environment"] == test_env
             and row["timeout_seconds"] == 30 and row["stdin_sha256"] is None, "focused signed source identity: " + name)
        need((qualification / name / "stdout").read_bytes() == b""
             and (qualification / name / "stderr").read_bytes() == b"", "unchanged selected source around focused tests")
    return {"source_commit": SOURCE, "trials": len(parsed), "timed_samples": 120, "endpoint_observations": 36,
            "owned_cleanup": True, "performance_acceptance": False, "results": parsed}


if __name__ == "__main__":
    need(sys.argv[1:] in ([], ["--seal"]), "usage: verify.py [--seal]")
    if sys.argv[1:] == ["--seal"]:
        verify(sealed=False)
        need(not (HERE / "inventory.json").exists(), "already sealed")
        (HERE / "inventory.json").write_text(json.dumps(inventory(HERE), indent=2) + "\n", encoding="ascii")
    result = verify()
    print(json.dumps({key: value for key, value in result.items() if key != "results"}, sort_keys=True))
