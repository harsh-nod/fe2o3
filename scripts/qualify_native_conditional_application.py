#!/usr/bin/env python3
"""Qualify the explicit native V5/V3 application route on an installed deployment.

This observer neither provisions services nor grants execution authority. The
real Cargo/application path must admit the fixed deployments, original source,
currentness and retained proof. A legacy result or mere Cargo success is refused.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import stat
import sys


ROOT = Path(__file__).resolve().parents[1]
FIXTURE = Path("crates/cargo-fe2o3/tests/fixtures/conditional-custodian-application")
SCHEMA = "fe2o3.native-conditional-fill.v1"
ROSTER_SCHEMA = "fe2o3.native-conditional-fill-roster.v1"
REPORT_SCHEMA = "fe2o3.native-conditional-application-qualification.v1"
BLOBS = ("source", "final_kernel_ir", "hsaco", "readiness", "analysis_request",
         "analysis_bundle", "analysis_receipt", "generated_source", "obligation", "signed_receipt")
IDENTITIES = ("policy_identity", "carriage_identity", "subject_identity", "proof_identity",
              "proof_session", "registration_identity")
FIELDS = set(BLOBS + IDENTITIES) | {"schema", "device", "target", "outputs",
    "settled_native_launches", "shutdown", "result_credits", "physical_overlap_measured",
    "all_host_devices_qualified"}
ROSTER_FIELDS = (FIELDS - {"device"}) | {"devices", "transport", "settled_directed_copies",
    "native_peer_completions", "selected_roster_complete", "copy_compute_order", "directed_pairs"}
TRANSPORTS = ("native-xgmi", "host-staged")
MAX_FILE = 512 * 1024 * 1024
MAX_SOURCE = 128 * 1024 * 1024
MAX_SOURCE_FILES = 8192
SOURCE_CRATES = ("cargo-fe2o3", "fe2o3-runtime", "fe2o3-runtime-model", "fe2o3-kfd",
                 "fe2o3-kfd-uapi", "fe2o3-aql", "fe2o3-amdhsa-loader", "fe2o3-host")
FIXED_FILES = tuple(Path("/etc/fe2o3/compiler-execution") / name for name in (
    "client-profile-v3", "supervisor-deployment-v3", "anchor-deployment-v3")) + (
    Path("/etc/fe2o3/proof-custodian/application-native-deployment-v1"),
    Path("/etc/fe2o3/proof-custodian/native-manager-deployment-v1"),
    Path("/etc/fe2o3/proof-custodian/native-conditional-root-policy-v1"),
    Path("/usr/libexec/fe2o3/fe2o3-native-application-manager"),
    Path("/usr/libexec/fe2o3/fe2o3-native-application-proof-controller"),
)

def startup_helper():
    spec = importlib.util.spec_from_file_location(
        "native_application_startup", ROOT / "scripts/native_application_startup.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def helper():
    spec = importlib.util.spec_from_file_location(
        "native_qualification_process_runner", ROOT / "scripts/qualify-tutorial-default-cargo.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def digest(value):
    return isinstance(value, str) and re.fullmatch(r"[0-9a-f]{64}", value) and value != "0" * 64


def device(value):
    if not isinstance(value, str) or not re.fullmatch(r"0x[0-9a-f]{16}", value) or int(value, 16) == 0:
        raise ValueError("device must be a nonzero canonical 0x-prefixed 16-digit lowercase UID")
    return value


def selection(selected, transport=None):
    if isinstance(selected, str):
        if transport is not None:
            raise ValueError("single-device qualification does not select a peer transport")
        return device(selected)
    if not isinstance(selected, (list, tuple)) or not 2 <= len(selected) <= 8:
        raise ValueError("native roster requires two through eight explicit devices")
    values = tuple(device(value) for value in selected)
    if len(set(values)) != len(values) or transport not in TRANSPORTS:
        raise ValueError("native roster requires unique UIDs and an explicit supported transport")
    return values


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON member: {key}")
        result[key] = value
    return result


def validate_result(value, selected, transport=None):
    selected = selection(selected, transport)
    if not isinstance(selected, str):
        return validate_roster_result(value, selected, transport)
    if not isinstance(value, dict) or set(value) != FIELDS or value["schema"] != SCHEMA:
        raise ValueError("exact native application result schema required")
    expected = {"device": selected, "target": "gfx942:xnack-", "outputs": [64, 37],
                "settled_native_launches": 2, "shutdown": "released", "result_credits": "refunded",
                "physical_overlap_measured": False, "all_host_devices_qualified": False}
    for key, wanted in expected.items():
        if type(value[key]) is not type(wanted) or value[key] != wanted:
            raise ValueError(f"native result differs: {key}")
    if any(type(count) is not int for count in value["outputs"]):
        raise ValueError("native output lengths must be integers")
    validate_provenance(value)
    return value


def validate_provenance(value):
    for key in BLOBS:
        blob = value[key]
        if (not isinstance(blob, dict) or set(blob) != {"sha256", "bytes"}
                or not digest(blob["sha256"]) or type(blob["bytes"]) is not int
                or not 0 < blob["bytes"] < 2**64):
            raise ValueError(f"invalid native content identity: {key}")
    for key in IDENTITIES:
        if not digest(value[key]):
            raise ValueError(f"invalid native identity: {key}")


def validate_roster_result(value, selected, transport):
    if not isinstance(value, dict) or set(value) != ROSTER_FIELDS or value["schema"] != ROSTER_SCHEMA:
        raise ValueError("exact native roster result schema required")
    count = len(selected)
    pair_count = count * (count - 1)
    expected = {"target": "gfx942:xnack-", "outputs": list(range(32, 32 + count)),
                "transport": transport, "settled_native_launches": count,
                "settled_directed_copies": pair_count,
                "native_peer_completions": pair_count if transport == "native-xgmi" else 0,
                "selected_roster_complete": True, "copy_compute_order": "compute-then-serial-copy",
                "shutdown": "released", "result_credits": "refunded",
                "physical_overlap_measured": False, "all_host_devices_qualified": False}
    for key, wanted in expected.items():
        if type(value[key]) is not type(wanted) or value[key] != wanted:
            raise ValueError(f"native roster result differs: {key}")
    if any(type(length) is not int for length in value["outputs"]):
        raise ValueError("native roster output lengths must be integers")
    observed = value["devices"]
    if not isinstance(observed, list) or len(observed) != count:
        raise ValueError("native admitted roster extent differs")
    minors = set()
    for uid, item in zip(selected, observed):
        if (not isinstance(item, dict) or set(item) != {"uid", "render_minor"}
                or item["uid"] != uid or type(item["render_minor"]) is not int
                or not 128 <= item["render_minor"] <= 255 or item["render_minor"] in minors):
            raise ValueError("native admitted UID/render roster differs or aliases")
        minors.add(item["render_minor"])
    pairs = value["directed_pairs"]
    if not isinstance(pairs, list) or len(pairs) != pair_count:
        raise ValueError("native directed-pair roster extent differs")
    expected_pairs = [(source, destination) for source in selected for destination in selected
                      if source != destination]
    for ordinal, (record, (source, destination)) in enumerate(zip(pairs, expected_pairs), 1):
        wanted = {"source": source, "destination": destination,
                  "payload_bytes": 24 + (32 + selected.index(source)) * 4,
                  "native_peer_completions": ordinal if transport == "native-xgmi" else 0}
        if (not isinstance(record, dict) or set(record) != set(wanted)
                or any(type(record[key]) is not type(item) or record[key] != item
                       for key, item in wanted.items())):
            raise ValueError("native directed pair, extent or mechanism counter differs")
    validate_provenance(value)
    return value


def result_from_log(raw, selected, transport=None):
    found = []
    for line in raw.decode("utf-8", errors="strict").splitlines():
        if not line.lstrip().startswith("{"):
            continue
        try:
            value = json.loads(line, object_pairs_hook=unique_object)
        except ValueError:
            if "fe2o3.native-conditional" in line or "fe2o3.genuine" in line:
                raise ValueError("malformed application result") from None
            continue
        if not isinstance(value, dict):
            continue
        schema = value.get("schema", "")
        if isinstance(schema, str) and schema.startswith("fe2o3.genuine"):
            raise ValueError("legacy application result is not native qualification")
        if isinstance(schema, str) and schema.startswith("fe2o3.native-conditional"):
            found.append(validate_result(value, selected, transport))
    if len(found) != 1:
        raise ValueError("exactly one native application result is required")
    return found[0]


def reconcile_execution(outcome, raw, selected, transport=None):
    if (outcome["status"] != "cargo-completed-unqualified" or type(outcome["exitCode"]) is not int
            or outcome["exitCode"] != 0
            or outcome["logComplete"] is not True or outcome["directChildReaped"] is not True):
        raise ValueError("native Cargo/application did not complete successfully")
    if (type(outcome["logBytes"]) is not int or len(raw) != outcome["logBytes"]
            or hashlib.sha256(raw).hexdigest() != outcome["logSha256"]):
        raise ValueError("captured application log changed")
    return result_from_log(raw, selected, transport)


def measure(path, maximum=MAX_FILE, *, allow_empty=False):
    fd = os.open(path, os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        before = os.fstat(fd)
        if (not stat.S_ISREG(before.st_mode) or not 0 <= before.st_size <= maximum
                or (before.st_size == 0 and not allow_empty)):
            raise ValueError(f"not a bounded nonempty regular input: {path}")
        sha = hashlib.sha256()
        remaining = before.st_size
        while remaining:
            chunk = os.read(fd, min(65536, remaining))
            if not chunk:
                raise ValueError(f"input truncated: {path}")
            sha.update(chunk)
            remaining -= len(chunk)
        def identity(info):
            return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
                    info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)
        if os.read(fd, 1) or identity(os.fstat(fd)) != identity(before):
            raise ValueError(f"input changed during observation: {path}")
        return {"sha256": sha.hexdigest(), "bytes": before.st_size}
    finally:
        os.close(fd)


def source_snapshot(root):
    # This named runtime/host inventory is not the complete compiler build closure.
    paths = {Path("Cargo.toml"), Path("Cargo.lock"), Path("rust-toolchain.toml")}
    trees = []
    for name in SOURCE_CRATES:
        base = Path("crates") / name
        paths.add(base / "Cargo.toml")
        trees.append(base / "src")
    trees.append(FIXTURE / "src")
    def walk_error(error):
        raise error
    for tree in trees:
        if not stat.S_ISDIR((root / tree).stat(follow_symlinks=False).st_mode):
            raise ValueError(f"source inventory root is not an original directory: {tree}")
        for directory, subdirs, files in os.walk(root / tree, followlinks=False, onerror=walk_error):
            if any((Path(directory) / child).is_symlink() for child in subdirs):
                raise ValueError("source inventory contains a directory symlink")
            paths.update((Path(directory) / child).relative_to(root)
                         for child in files if child.endswith(".rs"))
            if len(paths) > MAX_SOURCE_FILES:
                raise ValueError("source inventory exceeds file bound")
    paths.update(FIXTURE / name for name in ("Cargo.toml", "Cargo.lock"))
    paths.update(Path("scripts") / name for name in (
        "qualify_native_conditional_application.py", "qualify-tutorial-default-cargo.py",
        "native_application_startup.py"))
    total = 0
    records = {}
    for path in sorted(paths):
        record = measure(root / path, min(MAX_FILE, MAX_SOURCE - total), allow_empty=True)
        total += record["bytes"]
        records[str(path)] = record
    return records


def command_for(cargo, root, target, producer_source, selected, transport=None):
    selected = selection(selected, transport)
    if (not producer_source or len(producer_source.encode("utf-8")) > 4096
            or "\0" in producer_source or "\n" in producer_source):
        raise ValueError("exact producer source spelling required")
    if isinstance(selected, str):
        binary = "native-conditional-fill"
        application = ["--native-v5", "--producer-source", producer_source, "--device", selected]
    else:
        binary = "native-conditional-fill-roster"
        application = ["--native-v5-roster", "--producer-source", producer_source,
                       "--transport", transport, "--devices", *selected]
    return [str(cargo), "authority", "release", "--native", "run", "--native-application-proof-custodian",
            "--manifest-path", str(root / FIXTURE / "Cargo.toml"), "--target-dir", str(target),
            "--offline", "--frozen", "--bin", binary, "--", *application]


def qualify(args, support):
    transport = args.transport
    selected = selection(args.device if args.device is not None else args.devices, transport)
    roster = not isinstance(selected, str)
    environment = dict(os.environ)
    environment["FE2O3_TARGET"] = "gfx942"
    arguments = command_for(args.cargo_fe2o3, args.repo_root, args.target_dir,
                            args.producer_source, selected, transport)
    args.output.mkdir(mode=0o700, parents=True, exist_ok=False)
    report = {"schema": REPORT_SCHEMA, "arguments": arguments, "cwd": str(args.repo_root / FIXTURE),
              "producerSourceSpelling": args.producer_source,
              "requiredApplicationSchema": ROSTER_SCHEMA if roster else SCHEMA,
              "nativeApplicationQualificationPassed": False,
              "rootServiceCleanupQualified": False,
              "grantsArtifactOrLaunchAuthority": False, "milestoneClosure": False,
              "sourceInventoryScope": "runtime-host-native-application-not-complete-compiler-build-closure",
              "complete": False, "status": "not-attempted"}
    if roster:
        report.update(devices=list(selected), transport=transport,
                      qualificationScope="explicit-selected-device-roster",
                      allHostDevicesQualified=False)
    else:
        report["device"] = selected
    support.write_report(args.output, report)
    try:
        failures = support.prerequisites(args.cargo_fe2o3, environment)
        report["prerequisiteFailures"] = failures
        if failures:
            raise ValueError("protected compiler prerequisites unavailable")
        report["startupObservation"] = startup_helper().observe_installed_systemd(support, args.output)
        report["cargoFe2o3"] = measure(args.cargo_fe2o3)
        if report["cargoFe2o3"]["sha256"] != args.cargo_fe2o3_sha256:
            raise ValueError("qualification Cargo executable differs from explicit pin")
        report["installedObservations"] = {str(path): measure(path) for path in FIXED_FILES}
        report["proofRuntimeManifest"] = measure(support.RUNTIME / "FUNCTIONAL_REFINEMENT_RUNTIME_V1.manifest")
        report["compilerPins"] = {key: value for key, value in environment.items()
                                  if key in support.REQUIRED_ENV or key in support.CONFIG_ENV}
        before = source_snapshot(args.repo_root)
        report["sourceBefore"] = before
        report["status"] = "running"
        support.write_report(args.output, report)
        outcome = support.run_command(arguments, args.repo_root / FIXTURE, environment,
                                      args.output / "application.log", args.timeout_seconds,
                                      support.MAX_LOG_BYTES)
        report["execution"] = outcome
        report["sourceAfter"] = source_snapshot(args.repo_root)
        if report["sourceAfter"] != before:
            raise ValueError("runtime/host/application source changed during qualification")
        if ({str(path): measure(path) for path in FIXED_FILES} != report["installedObservations"]
                or measure(args.cargo_fe2o3) != report["cargoFe2o3"]
                or measure(support.RUNTIME / "FUNCTIONAL_REFINEMENT_RUNTIME_V1.manifest")
                != report["proofRuntimeManifest"]):
            raise ValueError("observed tool or deployment changed during qualification")
        raw = (args.output / "application.log").read_bytes()
        report["nativeApplicationResult"] = reconcile_execution(outcome, raw, selected, transport)
        report["status"] = "native-closed-fill-roster-pass" if roster else "native-closed-fill-pass"
        report["nativeApplicationQualificationPassed"] = True
    except (OSError, ValueError, KeyError, TypeError) as error:
        report["status"] = "refused"
        report["error"] = str(error)
    except KeyboardInterrupt:
        report["status"] = "interrupted"
        report["error"] = "qualification interrupted; owned process group cleanup completed"
    finally:
        report["complete"] = True
        support.write_report(args.output, report)
    return report


def argument_parser():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=ROOT)
    parser.add_argument("--cargo-fe2o3", type=Path, required=True)
    parser.add_argument("--cargo-fe2o3-sha256", required=True)
    parser.add_argument("--target-dir", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--producer-source", required=True)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--device")
    group.add_argument("--devices", nargs="+")
    parser.add_argument("--transport", choices=TRANSPORTS)
    parser.add_argument("--timeout-seconds", type=int, default=1500)
    return parser


def main():
    args = argument_parser().parse_args()
    try:
        selection(args.device if args.device is not None else args.devices, args.transport)
        if not digest(args.cargo_fe2o3_sha256) or not 0 < args.timeout_seconds <= 1800:
            raise ValueError("invalid executable pin or qualification deadline")
        for path in (args.repo_root, args.cargo_fe2o3, args.target_dir, args.output):
            if not path.is_absolute():
                raise ValueError("qualification paths must be absolute")
        support = helper()
        with support.cli_interrupt_handlers():
            report = qualify(args, support)
        print(json.dumps({"status": report["status"], "report": str(args.output / "report.json")}))
        return 0 if report["nativeApplicationQualificationPassed"] else 1
    except (OSError, ValueError, KeyboardInterrupt) as error:
        print(f"native application qualification refused: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
