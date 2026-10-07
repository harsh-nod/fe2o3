#!/usr/bin/env python3
"""Run every registered tutorial input through the protected default Cargo entry.

The default records the complete live V89/V90 managed-publication census.
It does not grant authority or replace the manifest's CPU-reference and semantic
simulation obligations. Cargo success without the exact census is refused.
"""

from __future__ import annotations

import argparse
from collections import Counter
from contextlib import contextmanager, nullcontext
from contextvars import ContextVar
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import selectors
import signal
import subprocess
import sys
import time


SCHEMA = "fe2o3-tutorial-default-cargo-census-v1"
# Observation of the fixed production requirement, never an alternate runtime.
RUNTIME = Path("/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5")
MAX_LOG_BYTES = 16 * 1024 * 1024
MAX_TIMEOUT_SECONDS = 3600
REQUIRED_ENV = (
    "CARGO", "FE2O3_BACKEND", "FE2O3_AUTHORITY_BACKEND_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_SHA256_V1", "FE2O3_AUTHORITY_RUSTC_PATH_V1",
    "FE2O3_AUTHORITY_RUSTC_SHA256_V1", "FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1",
    "FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1",
)
CONFIG_ENV = ("FE2O3_PRODUCTION_BUILD_CONFIG_V1", "FE2O3_PRODUCTION_BUILD_CONFIG_V2")
INTERRUPT_SIGNALS = (signal.SIGINT, signal.SIGTERM, signal.SIGHUP)
_CLI_INTERRUPTION = ContextVar("tutorial_cli_interruption", default=None)


class CliInterruption:
    def __init__(self):
        self.deferred = 0
        self.pending = False

    def __call__(self, signum=None, frame=None):
        if self.deferred:
            self.pending = True
            return
        # A second interruption must not interrupt process-group cleanup.
        for number in INTERRUPT_SIGNALS:
            signal.signal(number, signal.SIG_IGN)
        raise KeyboardInterrupt

    @contextmanager
    def defer(self):
        self.deferred += 1
        try:
            yield
        finally:
            self.deferred -= 1
            if self.pending and not self.deferred:
                self()


@contextmanager
def cli_interrupt_handlers():
    previous = {}
    interrupt = CliInterruption()
    token = _CLI_INTERRUPTION.set(interrupt)
    try:
        for number in INTERRUPT_SIGNALS:
            previous[number] = signal.signal(number, interrupt)
        yield
    finally:
        for number, handler in previous.items():
            signal.signal(number, handler)
        _CLI_INTERRUPTION.reset(token)


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def load_inputs(root, manifest_path):
    validator = load_module("tutorial_default_validator", root / "scripts/validate-tutorial-kernel-manifest.py")
    census = load_module("tutorial_default_census", root / "scripts/tutorial_source_census.py")
    manifest, digest = validator.load_manifest(manifest_path, with_sha256=True)
    validator.validate_manifest(root, manifest)
    invocations = census.registered_invocations(manifest)
    if not invocations:
        raise ValueError("the validated tutorial invocation roster is empty")
    return validator, census, manifest, digest, invocations


def command_for(root, cargo_fe2o3, cache, fixture):
    inputs = fixture["compilerInput"]
    # The existing validator authenticates the unique library target and closure.
    # No kernel-name filter or direct LLVM path is accepted by this harness.
    if inputs["cargoTarget"]["kind"] != "lib":
        raise ValueError("default tutorial census currently requires a registered library target")
    args = [str(cargo_fe2o3), "authority", "release", "build", "--locked", "--offline",
            "--release", "--manifest-path", str(root / inputs["packageManifest"]),
            "--lib", "--target-dir", str(cache / fixture["target"])]
    if not inputs["defaultFeatures"]:
        args.append("--no-default-features")
    if inputs["features"]:
        args.extend(["--features", ",".join(inputs["features"])])
    return args


def prerequisites(cargo_fe2o3, environment):
    """Cheap absence checks only; the production entry still authenticates all inputs."""
    failures = []
    if not RUNTIME.is_dir():
        failures.append({"kind": "missing-required-verus-runtime", "path": str(RUNTIME)})
    if (not cargo_fe2o3.is_absolute() or not cargo_fe2o3.is_file()
            or not os.access(cargo_fe2o3, os.X_OK)):
        failures.append({"kind": "missing-cargo-fe2o3-executable", "path": str(cargo_fe2o3)})
    for name in REQUIRED_ENV:
        if not environment.get(name):
            failures.append({"kind": "missing-protected-environment", "name": name})
    configs = [name for name in CONFIG_ENV if environment.get(name)]
    if len(configs) != 1:
        failures.append({"kind": "requires-exactly-one-production-build-config", "names": configs})
    else:
        path = Path(environment[configs[0]])
        if not path.is_absolute() or not path.is_file():
            failures.append({"kind": "unavailable-production-build-config", "name": configs[0]})
    return failures


def stop_group(process):
    # Retain the unreaped session leader until group cleanup: its PID owns the PGID.
    if process.returncode is None:
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
    process.wait()


def run_command(arguments, cwd, environment, log_path, timeout, max_log_bytes, *, executable_fd=None):
    """Bounded combined log, process-group deadline, and reaped direct child."""
    digest = hashlib.sha256()
    size = 0
    reason = None
    started = time.monotonic()
    with log_path.open("xb") as log:
        process = None
        interruption = _CLI_INTERRUPTION.get()
        try:
            try:
                # Defer CLI interruption until the returned child has an owner;
                # do not mask signals, which would change the child's mask too.
                with interruption.defer() if interruption is not None else nullcontext():
                    executable = {} if executable_fd is None else {
                        "executable": f"/proc/self/fd/{executable_fd}", "pass_fds": (executable_fd,)}
                    process = subprocess.Popen(arguments, cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                                               stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                               start_new_session=True, **executable)
            except OSError as error:
                return {"status": "launch-error", "error": str(error), "exitCode": None,
                        "logSha256": digest.hexdigest(), "logBytes": 0, "logComplete": True,
                        "directChildReaped": True}
            with selectors.DefaultSelector() as selector:
                selector.register(process.stdout, selectors.EVENT_READ)
                while selector.get_map():
                    remaining = timeout - (time.monotonic() - started)
                    if remaining <= 0:
                        reason = "timeout"
                        break
                    for key, _ in selector.select(min(remaining, 0.25)):
                        chunk = os.read(key.fd, 65536)
                        if not chunk:
                            selector.unregister(key.fileobj)
                            continue
                        retained = chunk[:max_log_bytes - size]
                        log.write(retained)
                        digest.update(retained)
                        size += len(retained)
                        if len(retained) != len(chunk):
                            reason = "log-limit"
                            break
                    if reason is not None:
                        break
                if reason is None:
                    while os.waitid(os.P_PID, process.pid,
                                    os.WEXITED | os.WNOHANG | os.WNOWAIT) is None:
                        remaining = timeout - (time.monotonic() - started)
                        if remaining <= 0:
                            reason = "timeout"
                            break
                        time.sleep(min(remaining, 0.01))
        finally:
            # Also kills any descendant retaining the pipe after the leader exits.
            if process is not None:
                stop_group(process)
                process.stdout.close()
        return {"status": reason or ("cargo-completed-unqualified" if process.returncode == 0 else "cargo-failed"),
                "exitCode": process.returncode, "logSha256": digest.hexdigest(), "logBytes": size,
                "logComplete": reason is None, "directChildReaped": True}


def write_report(output, report):
    temporary = output / "report.json.tmp"
    with temporary.open("x", encoding="utf-8") as stream:
        json.dump(report, stream, indent=2, sort_keys=True, allow_nan=False)
        stream.write("\n")
    temporary.replace(output / "report.json")


def run_census(root, manifest_path, cargo_fe2o3, cache, output, targets, timeout, max_log_bytes,
               *, runner=run_command, environment=None, production=False, capture_graphs=False,
               isolate_negative_outputs=False):
    environment = dict(os.environ if environment is None else environment)
    validator, census, manifest, manifest_sha, invocations = load_inputs(root, manifest_path)
    production_module = load_module("tutorial_production_census_v91",
        root / "scripts/tutorial_production_census_v91.py") if production else None
    output.mkdir(parents=True, exist_ok=False)
    selected = [(key, row) for key, row in sorted(invocations.items()) if row[0]["target"] in targets]
    report = {
        "schema": "fe2o3-tutorial-production-census-v91" if production else SCHEMA,
        "manifest": str(manifest_path), "manifestSha256": manifest_sha,
        "registeredInvocations": len(invocations), "selectedInvocations": len(selected),
        "coversAllRegisteredInvocations": len(selected) == len(invocations),
        "commandFamily": "cargo-fe2o3 authority release build",
        "requiredDescriptorSchema": 89 if production else 53,
        "requiredTypedLineageSchema": 90 if production else 50,
        "runtimePath": str(RUNTIME), "runtimePresenceAuthenticatesNothing": True,
        "qualified": False, "authenticatesCompilerExecution": False,
        "grantsArtifactOrLaunchAuthority": False, "defaultPipelineQualificationPassed": False,
        "pendingEvidence": (["all registered live V89/V90 compilation censuses",
                             "same-input CPU reference execution",
                             "declared BundleV7/KIR12 simulation adapter is absent",
                             "exact negative diagnostics and absent artifacts"] if production else
                            ["same-invocation complete V53 kernel publication census",
                             "authenticated executed V50 proof and native refinement", "full tutorial qualification"]),
        "limits": {"timeoutSecondsPerInvocation": timeout, "logBytesPerInvocation": max_log_bytes},
        "prerequisiteFailures": prerequisites(cargo_fe2o3, environment), "complete": False,
        "cases": [{"id": key, "fixture": row[0], "references": [list(reference) for reference in row[1]],
                   "expectedNegative": row[2], "expectedNegativeMatched": None,
                   "kernelSelection": "manifest features and library closure; symbols are expected, not a filter",
                   "arguments": command_for(root, cargo_fe2o3, cache, row[0]),
                   "targetEnvironment": {"FE2O3_TARGET": row[0]["target"]},
                   "status": "not-attempted", "qualified": False}
                  for key, row in selected],
    }
    write_report(output, report)
    for ordinal, case in enumerate(report["cases"]):
        if isolate_negative_outputs and case["expectedNegative"]:
            isolated = output / f"{ordinal:04d}-negative-target"
            isolated.mkdir(mode=0o700)
            position = case["arguments"].index("--target-dir") + 1
            case["arguments"][position] = str(isolated)
            case["negativeArtifactDirectoryV92"] = isolated.name
        try:
            before = census.input_snapshot(validator, root, manifest, manifest_sha, case["fixture"])
            case["sourceBefore"] = before
        except (OSError, ValueError, SystemExit) as error:
            case.update(status="invalid-source-input", sourceError=str(error))
            write_report(output, report)
            continue
        if report["prerequisiteFailures"]:
            case["status"] = "blocked-prerequisite"
        else:
            child_environment = dict(environment)
            child_environment["FE2O3_TARGET"] = case["fixture"]["target"]
            if production:
                try:
                    child_environment, expected = production_module.prepare(root, case["fixture"],
                        cargo_fe2o3, child_environment, output, ordinal)
                    case["productionExpected"] = expected
                    if capture_graphs:
                        directory = output / f"{ordinal:04d}-graphs-v92"
                        directory.mkdir(mode=0o700)
                        child_environment["FE2O3_TUTORIAL_GRAPH_CAPTURE_V92"] = str(directory.resolve())
                        case["capturedGraphsV92"] = directory.name
                except (OSError, ValueError, KeyError, TypeError) as error:
                    case.update(status="production-input-refused", productionError=str(error))
                    write_report(output, report)
                    continue
            log_name = f"{ordinal:04d}.log"
            case["log"] = log_name
            outcome = runner(case["arguments"], root, child_environment, output / log_name,
                             timeout, max_log_bytes)
            case["execution"] = outcome
            case["status"] = outcome["status"]
            try:
                after = census.input_snapshot(validator, root, manifest, manifest_sha, case["fixture"])
                case["sourceAfter"] = after
                _, current_sha = validator.load_manifest(manifest_path, with_sha256=True)
                if before != after or current_sha != manifest_sha:
                    case["status"] = "source-changed"
                elif production:
                    try:
                        observed = production_module.reconcile(output / log_name, outcome, expected)
                        case["productionCensus"] = observed
                        case["status"] = observed["status"] if not case["expectedNegative"] else "unexpected-negative-success"
                    except (OSError, ValueError, KeyError, TypeError) as error:
                        case["productionError"] = str(error)
                        if outcome["exitCode"] == 0:
                            case["status"] = "production-census-refused"
            except (OSError, ValueError, SystemExit) as error:
                case.update(status="source-changed", sourceError=str(error))
        write_report(output, report)
    report["counts"] = dict(sorted(Counter(case["status"] for case in report["cases"]).items()))
    report["complete"] = True
    if production:
        report["positiveCompilationCensusPassed"] = all(case["status"] == "production-compile-census-pass"
            for case in report["cases"] if not case["expectedNegative"])
        report["referenceExecutionPassed"] = False
        report["semanticSimulationPassed"] = False
    write_report(output, report)
    return report


def census_main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo-root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--manifest", type=Path, default=Path("config/tutorial-kernel-manifest-v1.json"))
    parser.add_argument("--cargo-fe2o3", type=Path, required=True, help="absolute existing protected CLI executable")
    parser.add_argument("--target-dir", type=Path, required=True, help="persistent cache; no artifacts are counted as proof")
    parser.add_argument("--output", type=Path, required=True, help="new report/log directory; never overwritten")
    parser.add_argument("--target", choices=("all", "gfx942", "gfx950"), default="all")
    parser.add_argument("--timeout-seconds", type=int, default=600)
    parser.add_argument("--max-log-bytes", type=int, default=MAX_LOG_BYTES)
    parser.add_argument("--legacy-compile-census", action="store_true",
                        help="record only the historical compile/refusal diagnostic, never qualification")
    args = parser.parse_args(argv)
    if not 1 <= args.timeout_seconds <= MAX_TIMEOUT_SECONDS or not 1 <= args.max_log_bytes <= MAX_LOG_BYTES:
        parser.error("timeout or log limit exceeds the finite harness envelope")
    if not args.cargo_fe2o3.is_absolute():
        parser.error("--cargo-fe2o3 must be absolute")
    root = args.repo_root.resolve(strict=True)
    manifest = args.manifest if args.manifest.is_absolute() else root / args.manifest
    targets = ("gfx942", "gfx950") if args.target == "all" else (args.target,)
    try:
        report = run_census(root, manifest, args.cargo_fe2o3, args.target_dir.resolve(), args.output.resolve(),
                            targets, args.timeout_seconds, args.max_log_bytes,
                            production=not args.legacy_compile_census)
    except (OSError, ValueError, SystemExit) as error:
        print(f"tutorial default Cargo census refused: {error}", file=sys.stderr)
        return 2
    print(json.dumps({"report": str(args.output.resolve() / "report.json"), "counts": report["counts"],
                      "qualified": False}, sort_keys=True))
    # Completion of the observation is not successful qualification, including
    # expected-negative cases and zero-exit cached Cargo invocations.
    return 1


def main(argv=None):
    try:
        with cli_interrupt_handlers():
            return census_main(argv)
    except KeyboardInterrupt:
        print("tutorial default Cargo census interrupted; last checkpoint retained",
              file=sys.stderr)
        return 130


if __name__ == "__main__":
    raise SystemExit(main())
