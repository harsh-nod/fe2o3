#!/usr/bin/env python3
"""Execute one packaged manager's nonroot refusal, not a protected success path."""
import argparse
import errno
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat


ROOT = Path(__file__).resolve().parents[1]
MAX_IMAGE = 512 * 1024 * 1024
MAX_LOG = 4096
EXPECTED = (b"native application manager: root native compiler deployment rejected: "
            b'Filesystem("root native deployment requires exact stable root credentials")\n')
SOURCES = (
    "crates/fe2o3-proof-custodian/src/bin/native_application_manager.rs",
    "crates/fe2o3-proof-custodian/src/native_manager_entrypoint.rs",
    "crates/fe2o3-compiler-execution-coordinator/src/native_compiler_phase.rs",
    "crates/fe2o3-compiler-execution-coordinator/src/native_entrypoint.rs",
    "crates/fe2o3-protected-service-spawn/src/process_reaper_native.rs",
    "crates/fe2o3-protected-service-spawn/src/creator_scope.rs",
    "scripts/qualify_native_manager_cpu.py",
)


def support():
    spec = importlib.util.spec_from_file_location("manager_cpu_capture", ROOT / "scripts/qualify-tutorial-default-cargo.py")
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def nonroot():
    if any(uid == 0 for uid in os.getresuid()):
        raise ValueError("packaged manager refusal requires original nonroot credentials")


def source_identities():
    spec = importlib.util.spec_from_file_location("manager_cpu_source_observer",
                                                ROOT / "scripts/qualify_native_conditional_application.py")
    observer = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(observer)
    return {path: observer.measure(ROOT / path, 1024 * 1024) for path in SOURCES}


def identity(info):
    return (info.st_dev, info.st_ino, info.st_mode, info.st_uid, info.st_gid,
            info.st_nlink, info.st_size, info.st_mtime_ns, info.st_ctime_ns)


def measure_fd(fd):
    before = os.fstat(fd)
    if (not stat.S_ISREG(before.st_mode) or before.st_mode & 0o6022
            or not before.st_mode & 0o111 or not 64 <= before.st_size <= MAX_IMAGE):
        raise ValueError("bounded non-setid executable without group/other write access required")
    try:
        capabilities = os.getxattr(fd, "security.capability")
    except OSError as error:
        if error.errno != errno.ENODATA:
            raise
        capabilities = b""
    if capabilities:
        raise ValueError("packaged executable must not elevate file capabilities")
    if os.pread(fd, 4, 0) != b"\x7fELF":
        raise ValueError("actual packaged ELF image required")
    sha = hashlib.sha256()
    offset = 0
    while offset < before.st_size:
        data = os.pread(fd, min(65536, before.st_size - offset), offset)
        if not data:
            raise ValueError("packaged executable truncated")
        sha.update(data)
        offset += len(data)
    if os.pread(fd, 1, offset) or identity(os.fstat(fd)) != identity(before):
        raise ValueError("packaged executable changed during measurement")
    return {"sha256": sha.hexdigest(), "bytes": before.st_size}, identity(before)


def validate_capture(outcome, raw):
    if (outcome["status"] != "cargo-failed" or type(outcome["exitCode"]) is not int
            or outcome["exitCode"] != 98 or outcome["logComplete"] is not True
            or outcome["directChildReaped"] is not True or raw != EXPECTED
            or type(outcome["logBytes"]) is not int or outcome["logBytes"] != len(raw)
            or outcome["logSha256"] != hashlib.sha256(raw).hexdigest()):
        raise ValueError("exact packaged manager nonroot refusal was not observed")


def qualify(image, pin, output, capture):
    nonroot()
    if (not image.is_absolute() or not output.is_absolute() or len(pin) != 64
            or any(c not in "0123456789abcdef" for c in pin) or pin == "0" * 64):
        raise ValueError("absolute image/output and explicit canonical image pin required")
    output.mkdir(mode=0o700, parents=True, exist_ok=False)
    report = {"schema": "fe2o3.native-manager-packaged-refusal.v1", "complete": False,
              "packagedEntryRefusalPassed": False, "joinedProtectedPhaseQualified": False,
              "rootServiceCleanupQualified": False, "gpuExecution": False,
              "sourceInventoryCompleteBuildClosure": False,
              "grantsAdmissionAuthority": False, "imagePath": str(image)}
    capture.write_report(output, report)
    try:
        report["sourceBefore"] = source_identities()
        fd = os.open(image, os.O_RDONLY | os.O_NOFOLLOW | os.O_CLOEXEC | os.O_NONBLOCK)
        try:
            measured, original = measure_fd(fd)
            if measured["sha256"] != pin or identity(image.stat(follow_symlinks=False)) != original:
                raise ValueError("packaged image differs from explicit pin or original path")
            report["image"] = measured
            nonroot()
            # The static secure-start ABI requires one nonempty argv and no
            # environment, before libc or Rust can inspect inherited custody.
            result = capture.run_command([str(image)], output, {},
                                         output / "manager.log", 5, MAX_LOG, executable_fd=fd)
            report["execution"] = result
            with (output / "manager.log").open("rb") as log:
                raw = log.read(MAX_LOG + 1)
            validate_capture(result, raw)
            after, current = measure_fd(fd)
            if (after != measured or current != original
                    or identity(image.stat(follow_symlinks=False)) != original):
                raise ValueError("packaged image or original path changed during refusal")
            report["sourceAfter"] = source_identities()
            if report["sourceAfter"] != report["sourceBefore"]:
                raise ValueError("observed joined-phase source changed during refusal")
            report["packagedEntryRefusalPassed"] = True
        finally:
            os.close(fd)
    except (OSError, ValueError, KeyError, TypeError) as error:
        report["error"] = str(error)
    finally:
        report["complete"] = True
        capture.write_report(output, report)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--image", type=Path, required=True)
    parser.add_argument("--image-sha256", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    runner = support()
    with runner.cli_interrupt_handlers():
        result = qualify(args.image, args.image_sha256, args.output, runner)
    print(json.dumps({"packagedEntryRefusalPassed": result["packagedEntryRefusalPassed"],
                      "joinedProtectedPhaseQualified": False, "rootServiceCleanupQualified": False}))
    return 0 if result["packagedEntryRefusalPassed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
