#!/usr/bin/env python3
"""Offline peer-batch evidence replay without performance acceptance."""

import sys

if not sys.flags.isolated:
    raise RuntimeError("run with python3 -I")

import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
EXECUTION_ROOT = Path("/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917")
CAMPAIGN_SHA256 = "fcca4a2443673d4bb5f6a3e5b1df9497266dc5f63d58bb3eb58a497131598ed7"
RECEIPT_HELPER_SHA256 = (
    "c139fe368604a5deddea5dc66b0713c1e8dab9a4018b6f55fff76545ead6b67f"
)
CALIBRATION_TESTS = 18
ARCHIVE_STATIC = set(
    (
        "README.md",
        "binding.json",
        "owner.json",
        "remote-inventory.json",
        "controller-state.json",
    )
)


def authenticated_module(path, digest, name):
    if (
        path.is_symlink()
        or not path.is_file()
        or hashlib.sha256(path.read_bytes()).hexdigest() != digest
    ):
        raise RuntimeError("authenticate helper before import: " + str(path))
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


C = authenticated_module(HERE / "campaign.py", CAMPAIGN_SHA256, "peer_batch_campaign")
P, B = C.P, C.B
need, read, sha = P.need, P.read, P.sha
receipt_helper = authenticated_module(
    P.ROOT
    / "docs/evidence/dev-topology-prechecked-comparison-mi300x-2026-09-18/verify.py",
    RECEIPT_HELPER_SHA256,
    "peer_batch_receipt_helper",
)
receipt = receipt_helper.receipt
endpoint = receipt_helper.endpoint
postflight_timing = receipt_helper.postflight_timing
exact_tree = receipt_helper.exact_tree


def verify_signature(commit):
    need(sha(P.SIGNERS) == P.SIGNERS_SHA, "pinned signing trust")
    result = subprocess.run(
        [
            "git",
            "-c",
            "gpg.ssh.allowedSignersFile=" + P.SIGNERS,
            "verify-commit",
            commit,
        ],
        cwd=P.ROOT,
        capture_output=True,
        timeout=30,
    )
    need(result.returncode == 0, "signed commit: " + commit)


def calibration(folder):
    need((folder / "stdout").read_bytes() == b"", "empty calibration stdout")
    need(
        re.fullmatch(
            r"\.{"
            + str(CALIBRATION_TESTS)
            + r"}\n-+\nRan "
            + str(CALIBRATION_TESTS)
            + r" tests in [0-9.]+s\n\nOK\n",
            (folder / "stderr").read_text(),
        ),
        "complete calibration transcript",
    )


def cpu_replay():
    binding = P.cpu_binding()
    result = subprocess.run(
        ["python3", "-I", "-B", str(P.ROOT / P.CPU_RELATIVE / "verify.py")],
        cwd=P.ROOT,
        capture_output=True,
        timeout=120,
    )
    need(result.returncode == 0, "sealed CPU prerequisite replay")
    return binding


def verify(root=HERE, *, check_seal=True):
    binding, marker = read(root / "binding.json"), read(root / "owner.json")
    cpu = cpu_replay()
    P.validate_binding(binding, cpu)
    need(
        binding["tooling_commit"] == marker["commit"]
        and marker["binding_sha256"] == sha(root / "binding.json"),
        "exact ownership association",
    )
    owned = B.owned_path(marker, exists=False)
    need(
        binding["tools"]
        == {name: sha(root / name) for name in P.STATIC_INPUTS}
        == C.committed_tools(binding["tooling_commit"]),
        "unchanged signed launch tools",
    )
    verify_signature(binding["tooling_commit"])
    verify_signature(binding["source_commit"])
    source, data = P.source_package()
    source["tar_bytes"] = len(data)
    need(P.same_json(binding["source"], source), "signed-tree CPU source association")

    state = read(root / "controller-state.json")
    flags = {
        "create_attempted",
        "created",
        "native_attempted",
        "native_success",
        "collected",
        "cleaned",
        "absence",
        "local_payload_absent",
    }
    need(
        set(state)
        == flags | {"local_payload", "failure", "native_failure", "secondary_failures"}
        and all(state[name] is True for name in flags)
        and state["failure"] is None
        and state["native_failure"] is None
        and state["secondary_failures"] == [],
        "complete successful controller state",
    )
    payload = Path(state["local_payload"])
    need(
        re.fullmatch(
            r"/home/harsh/\.codex-tmp/" + re.escape(P.LOCAL_PREFIX) + r"[A-Za-z0-9_]+",
            str(payload),
        )
        and C.path_absent(payload),
        "removed private local payload",
    )
    commands = C.local_specs(payload, marker, execution_root=EXECUTION_ROOT)
    last = 0
    for name in C.LOCAL_ORDER:
        command, seconds, stdin = commands[name]
        row = receipt(
            root / "local" / name,
            command,
            seconds,
            EXECUTION_ROOT,
            last,
            stdin=stdin,
        )
        last = row["finished_ns"]
    calibration(root / "local/calibration")
    need((root / "local/source-clean/stdout").read_bytes() == b"", "clean source")
    snapshot = read(root / "local/source/stdout")
    need(
        snapshot == {"base": binding["tooling_commit"], "files": P.cpu_source()},
        "live selected tooling source equality",
    )
    need(
        read(root / "local/pack/stdout") == binding["source"],
        "recorded source packaging result",
    )
    for remote in ("origin", "upstream"):
        need(
            (root / ("local/ref-" + remote) / "stdout").read_text()
            == binding["tooling_commit"] + "\t" + P.TOOLING_BRANCH + "\n",
            "published tooling ref",
        )

    manifest = read(root / "remote-inventory.json")
    need(B.inventory(root / "remote") == manifest, "remote collection digest equality")
    need(
        read(root / "local/create/stdout") == marker
        and read(root / "local/remote-inventory/stdout") == manifest
        and read(root / "local/cleanup/stdout") == {"removed": str(owned)}
        and read(root / "local/absence/stdout")
        == {"path_absent": True, "processes_absent": True},
        "owned collection and cleanup closure",
    )

    specs = P.remote_specs(owned, binding["devices"])
    need(
        commands["native"][1]
        == P.native_timeout()
        > sum(spec[2] for spec in specs) + len(P.PHASES) * 22,
        "outer timeout exceeds nested command and delay bounds",
    )
    remote, last, receipts = root / "remote", 0, {}
    for name, command, seconds, cwd, env in specs:
        row = receipt(
            remote / name,
            command,
            seconds,
            cwd,
            last,
            environment=env,
        )
        last, receipts[name] = row["finished_ns"], row
        if name == "cargo":
            P.validate_toolchain_receipts(remote, binding["cpu"]["toolchain"])
    expected_source = {
        key: binding["source"][key] for key in ("source_files", "source_modes")
    }
    need(
        read(remote / "source-before.json")
        == expected_source
        == read(remote / "source-after.json"),
        "unchanged remote source",
    )
    binaries = read(remote / "binaries.json")
    need(
        set(binaries) == {P.BINARY}
        and re.fullmatch(r"[0-9a-f]{64}", binaries[P.BINARY]),
        "one exact benchmark ELF identity",
    )
    parsed = {}
    for phase in P.PHASES:
        name = phase[0]
        folder = remote / name
        need((folder / "stderr").read_bytes() == b"", "empty workload stderr")
        parsed[name] = P.parse_transcript(
            (folder / "stdout").read_bytes(), binding["devices"], phase
        )
        for suffix in ("before", "settled", "delayed"):
            for device in binding["devices"]:
                endpoint(
                    remote / (name + "-" + suffix + "-gpu" + str(device[0])),
                    device,
                )
        postflight_timing(receipts, name, binding["devices"])
    need(
        parsed == read(remote / "parsed.json"), "independent complete transcript replay"
    )
    need(
        read(remote / "finished.json")
        == {
            "tooling_commit": binding["tooling_commit"],
            "source_commit": binding["source_commit"],
            "native_execution": True,
            "formal_refinement": False,
            "performance_acceptance": False,
            "hip_hsa_parity": False,
            "failure": None,
            "secondary_failures": [],
        },
        "bounded native execution verdict",
    )
    files = set(P.STATIC_INPUTS) | ARCHIVE_STATIC
    files.update(
        "local/" + name + "/" + item
        for name in C.LOCAL_ORDER
        for item in ("receipt.json", "stdout", "stderr")
    )
    files.update(
        "remote/" + name + "/" + item
        for name, *_ in specs
        for item in ("receipt.json", "stdout", "stderr")
    )
    files.update(
        "remote/" + name
        for name in (
            "source-before.json",
            "source-after.json",
            "binaries.json",
            "parsed.json",
            "finished.json",
        )
    )
    exact_tree(root, files)
    if check_seal or (root / "SHA256SUMS").exists():
        manifest_text = "".join(
            digest + "  " + name + "\n"
            for name, digest in B.inventory(root).items()
            if name != "SHA256SUMS"
        )
        need((root / "SHA256SUMS").read_text() == manifest_text, "whole archive seal")
    return {
        "native_execution": True,
        "same_binary": True,
        "ordinary_processes": 4,
        "aggregate_processes": 5,
        "depth63_correctness_processes": 1,
        "endpoint_observations": 54,
        "local_commands": len(C.LOCAL_ORDER),
        "remote_commands": len(specs),
        "binary_sha256": binaries[P.BINARY],
        "formal_refinement": False,
        "performance_acceptance": False,
        "hip_hsa_parity": False,
    }


def seal(create):
    manifest = "".join(
        digest + "  " + name + "\n"
        for name, digest in B.inventory(HERE).items()
        if name != "SHA256SUMS"
    )
    path = HERE / "SHA256SUMS"
    if create:
        with path.open("x") as target:
            target.write(manifest)
    else:
        need(path.read_text() == manifest, "complete archive seal")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--seal", action="store_true")
    args = parser.parse_args()
    need(not (args.allow_unsealed and args.seal), "choose one seal mode")
    result = verify(check_seal=not args.allow_unsealed and not args.seal)
    if args.seal:
        seal(True)
        verify()
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
