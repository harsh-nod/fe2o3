#!/usr/bin/env python3
"""Offline execution/source association, never general performance acceptance."""

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
CAMPAIGN_SHA = "1516b7638fc6745db165b412c3f5ff2142824e436a4efb7f1264654226fd0042"
CALIBRATION_TESTS = 27


def authenticated_module(path, digest, name):
    if (
        path.is_symlink()
        or not path.is_file()
        or hashlib.sha256(path.read_bytes()).hexdigest() != digest
    ):
        raise RuntimeError("authenticate helper before import")
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


C = authenticated_module(
    HERE / "campaign.py", CAMPAIGN_SHA, "verified_comparison_campaign"
)
P, B = C.P, C.B
need, read, sha = P.need, P.read, P.sha
OLD = P.load_pinned(
    P.ROOT / (P.PRIOR + "verify.py"), C.OLD_VERIFIER_SHA, "comparison_receipt_parser"
)
receipt, endpoint, postflight_timing, exact_tree = (
    OLD.receipt,
    OLD.endpoint,
    OLD.postflight_timing,
    OLD.exact_tree,
)


def verify_signature(commit):
    need(sha(P.SIGNERS) == P.SIGNERS_SHA, "pinned signing trust")
    subprocess.run(
        [
            "git",
            "-c",
            "gpg.ssh.allowedSignersFile=" + P.SIGNERS,
            "verify-commit",
            commit,
        ],
        cwd=P.ROOT,
        capture_output=True,
        check=True,
        timeout=30,
    )


def calibration(folder, count):
    need((folder / "stdout").read_bytes() == b"", "empty calibration stdout")
    need(
        re.fullmatch(
            r"\.{"
            + str(count)
            + r"}\n-+\nRan "
            + str(count)
            + r" tests in [0-9.]+s\n\nOK\n",
            (folder / "stderr").read_text(),
        ),
        "exact complete calibration closure",
    )


def facade_comparison(parsed):
    result = {}
    for mode in ("on", "off"):
        for measurement in ("remap-per-round", "persistent-hot"):
            for direction in ("forward", "reverse"):
                for metric in ("p50", "p95"):
                    values = {}
                    for cohort in P.COHORTS:
                        values[cohort] = [
                            int(
                                next(
                                    row
                                    for row in parsed[f"{cohort}-{mode}{replicate}"][
                                        "aggregates"
                                    ]
                                    if row["measurement"] == measurement
                                )[direction + "_" + metric + "_ns"]
                            )
                            for replicate in (1, 2)
                        ]
                    result[f"{mode}/{measurement}/{direction}/{metric}"] = {
                        "baseline_ns_by_process": values["baseline"],
                        "candidate_ns_by_process": values["candidate"],
                        "candidate_over_baseline_by_replicate_index": [
                            c / b
                            for c, b in zip(values["candidate"], values["baseline"])
                        ],
                    }
    return result


def verify(root=HERE):
    binding, marker = read(root / "binding.json"), read(root / "owner.json")
    P.validate_binding(binding)
    need(
        binding["commit"] == marker["commit"]
        and marker["binding_sha256"] == sha(root / "binding.json"),
        "exact ownership association",
    )
    owned = B.owned_path(marker, exists=False)
    selected = binding["devices"]
    need(
        binding["tools"]
        == {name: sha(root / name) for name in P.STATIC_INPUTS}
        == C.committed_tools(binding["commit"]),
        "unchanged signed launch tools",
    )
    verify_signature(binding["commit"])
    for cohort, spec in P.COHORTS.items():
        verify_signature(spec["commit"])
        expected = P.cpu_source(cohort)
        value, _ = P.materialize(P.ROOT, spec["commit"], expected)
        need(
            P.same_json(
                binding["cohorts"][cohort],
                {"cohort": cohort, "prerequisite": spec, **value},
            ),
            "independent signed-tree CPU/archive association",
        )
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
        and all(state[k] is True for k in flags)
        and state["failure"] is None
        and state["native_failure"] is None
        and state["secondary_failures"] == [],
        "complete run and cleanup state",
    )
    payload = Path(state["local_payload"])
    need(
        re.fullmatch(
            r"/home/harsh/\.codex-tmp/" + re.escape(P.LOCAL_PREFIX) + r"[A-Za-z0-9_]+",
            str(payload),
        ),
        "owned local payload shape",
    )
    commands = C.local_specs(payload, marker, execution_root=EXECUTION_ROOT)
    last = 0
    for name in C.LOCAL_ORDER:
        command, seconds, stdin = commands[name]
        row = receipt(
            root / "local" / name, command, seconds, EXECUTION_ROOT, last, stdin=stdin
        )
        last = row["finished_ns"]
    for name, count in (
        ("calibration", CALIBRATION_TESTS),
        ("parser-calibration", 21),
        ("observer-tests", 19),
    ):
        need(count > 0, "frozen calibration count")
        calibration(root / "local" / name, count)
    need(
        (root / "local/source-clean/stdout").read_bytes() == b"",
        "clean selected compiler inputs",
    )
    need(
        read(root / "local/source/stdout")
        == {
            "base": binding["commit"],
            "files": binding["cohorts"]["candidate"]["source_files"],
        },
        "current selected source equality",
    )
    for remote in ("origin", "upstream"):
        need(
            (root / ("local/ref-" + remote) / "stdout").read_text()
            == binding["commit"] + "\t" + C.BRANCH + "\n",
            "both recorded published tooling refs",
        )
    for cohort, spec in P.COHORTS.items():
        report = read(root / ("local/cpu-" + cohort) / "stdout")
        need(
            report["cpu_qualification"] is True
            and type(report["source_files"]) is int
            and report["source_files"] == spec["files"]
            and report["source_snapshot_sha256"] == spec["source_snapshot"]
            and all(
                report[k] is False
                for k in (
                    "native_execution",
                    "formal_refinement",
                    "performance_acceptance",
                )
            ),
            "CPU-only prerequisite report",
        )
        need(
            P.same_json(
                read(root / ("local/pack-" + cohort) / "stdout"),
                binding["cohorts"][cohort],
            ),
            "recorded pack result",
        )
    manifest = read(root / "remote-inventory.json")
    need(B.inventory(root / "remote") == manifest, "remote collection digest equality")
    need(
        P.same_json(read(root / "local/create/stdout"), marker)
        and read(root / "local/remote-inventory/stdout") == manifest
        and read(root / "local/cleanup/stdout") == {"removed": str(owned)}
        and P.same_json(
            read(root / "local/absence/stdout"),
            {"path_absent": True, "processes_absent": True},
        ),
        "owned collection/cleanup outcomes",
    )
    remote, last, receipts = root / "remote", 0, {}
    specs = P.remote_specs(owned, selected)
    need(
        commands["native"][1]
        == P.native_timeout()
        > sum(s[2] for s in specs) + len(P.PHASES) * 22,
        "outer bound exceeds nested command/settlement bounds",
    )
    for name, command, seconds, cwd, env in specs:
        row = receipt(remote / name, command, seconds, cwd, last, environment=env)
        last, receipts[name] = row["finished_ns"], row
    for name in ("rustc", "cargo"):
        outputs = [
            (P.ROOT / spec["cpu"] / "raw" / name / "stdout").read_bytes()
            for spec in P.COHORTS.values()
        ]
        need(
            outputs[0] == outputs[1] == (remote / name / "stdout").read_bytes(),
            "same qualified reported compiler toolchain",
        )
    expected_sources = {
        n: {k: v[k] for k in ("source_files", "source_modes")}
        for n, v in binding["cohorts"].items()
    }
    need(
        read(remote / "source-before.json")
        == expected_sources
        == read(remote / "source-after.json"),
        "unchanged separate remote source cohorts",
    )
    binaries = read(remote / "binaries.json")
    need(
        set(binaries) == set(P.COHORTS)
        and all(
            type(v) is str and re.fullmatch(r"[0-9a-f]{64}", v)
            for v in binaries.values()
        ),
        "separate binary identities",
    )
    parsed = {}
    for name, _, enabled in P.PHASES:
        folder = remote / name
        need((folder / "stderr").read_bytes() == b"", "empty workload stderr")
        parsed[name] = P.parse_transcript(
            (folder / "stdout").read_bytes(), selected, enabled
        )
        for suffix in ("before", "settled", "delayed"):
            for device in selected:
                endpoint(
                    remote / (name + "-" + suffix + "-gpu" + str(device[0])), device
                )
        postflight_timing(receipts, name, selected)
    need(
        P.same_json(parsed, read(remote / "parsed.json")),
        "independent complete transcript replay",
    )
    need(
        P.same_json(
            read(remote / "finished.json"),
            {
                "commit": binding["commit"],
                "native_execution": True,
                "formal_refinement": False,
                "performance_acceptance": False,
                "failure": None,
                "secondary_failures": [],
            },
        ),
        "bounded native execution verdict",
    )
    files = set(P.STATIC_INPUTS) | {
        "README.md",
        "binding.json",
        "owner.json",
        "remote-inventory.json",
        "controller-state.json",
    }
    files.update(
        "local/" + n + "/" + item
        for n in C.LOCAL_ORDER
        for item in ("receipt.json", "stdout", "stderr")
    )
    files.update(
        "remote/" + n + "/" + item
        for n, *_ in specs
        for item in ("receipt.json", "stdout", "stderr")
    )
    files.update(
        "remote/" + n
        for n in (
            "source-before.json",
            "source-after.json",
            "binaries.json",
            "parsed.json",
            "finished.json",
        )
    )
    exact_tree(root, files)
    return {
        "native_execution": True,
        "source_to_source_comparison": True,
        "formal_refinement": False,
        "performance_acceptance": False,
        "tooling_commit": binding["commit"],
        "source_commits": {n: s["commit"] for n, s in P.COHORTS.items()},
        "binary_sha256": binaries,
        "binary_digests_distinct": len(set(binaries.values())) == 2,
        "devices": selected,
        "local_commands": len(C.LOCAL_ORDER),
        "remote_commands": len(specs),
        "endpoint_observations": len(P.PHASES) * 6,
        "facade": {n: v["aggregates"] for n, v in parsed.items()},
        "descriptive_facade_ratios": facade_comparison(parsed),
        "stages": {n: P.summarize(parsed[n]) for n, _, enabled in P.PHASES if enabled},
    }


def seal(root, create=False):
    manifest = "".join(
        digest + "  " + name + "\n"
        for name, digest in B.inventory(root).items()
        if name != "SHA256SUMS"
    )
    path = root / "SHA256SUMS"
    if create:
        with path.open("x") as output:
            output.write(manifest)
    else:
        need(path.read_text() == manifest, "whole archive seal")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--summary", action="store_true")
    args = parser.parse_args()
    report = verify()
    if args.seal or not args.allow_unsealed or (HERE / "SHA256SUMS").exists():
        seal(HERE, args.seal)
    if not args.summary:
        for name in ("facade", "descriptive_facade_ratios", "stages"):
            report.pop(name)
    print(json.dumps(report, sort_keys=True))


if __name__ == "__main__":
    main()
