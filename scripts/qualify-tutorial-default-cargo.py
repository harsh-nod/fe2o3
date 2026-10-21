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
import stat
import subprocess
import sys
import time


SCHEMA = "fe2o3-tutorial-default-cargo-census-v1"
# Observation of the fixed production requirement, never an alternate runtime.
RUNTIME = Path("/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5")
MAX_LOG_BYTES = 16 * 1024 * 1024
MAX_TIMEOUT_SECONDS = 3600
SELECTED_REQUEST_SCHEMA = "fe2o3-tutorial-selected-case-request-v1"
SELECTED_RESULT_SCHEMA = "fe2o3-tutorial-selected-case-result-v1"
MAX_SELECTED_REQUEST_BYTES = 64 * 1024
MAX_SELECTED_SECONDS = 1800
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
               isolate_negative_outputs=False, invocation_ids=None, expected_source_sha256=None):
    environment = dict(os.environ if environment is None else environment)
    validator, census, manifest, manifest_sha, invocations = load_inputs(root, manifest_path)
    production_module = load_module("tutorial_production_census_v91",
        root / "scripts/tutorial_production_census_v91.py") if production else None
    output.mkdir(parents=True, exist_ok=False)
    selected = [(key, row) for key, row in sorted(invocations.items()) if row[0]["target"] in targets]
    if invocation_ids is not None:
        if (not isinstance(invocation_ids, tuple) or len(invocation_ids) != 1
                or invocation_ids[0] not in dict(selected)):
            raise ValueError("selection must name one registered invocation on its declared target")
        selected = [(key, row) for key, row in selected if key in invocation_ids]
    elif expected_source_sha256 is not None:
        raise ValueError("a bound source snapshot requires an explicit invocation selection")
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
            if expected_source_sha256 is not None and snapshot_sha256(before) != expected_source_sha256:
                raise ValueError("selected source snapshot changed before compilation")
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


def snapshot_sha256(snapshot):
    return hashlib.sha256(json.dumps(snapshot, sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=True, allow_nan=False).encode("ascii")).hexdigest()


def read_selected_request(path):
    absolute = path.absolute()
    if absolute.resolve(strict=True) != absolute:
        raise ValueError("selected request must have a canonical nonsymlink path")
    flags = os.O_RDONLY | os.O_CLOEXEC | os.O_NOFOLLOW | os.O_NONBLOCK
    with os.fdopen(os.open(absolute, flags), "rb") as stream:
        before = os.fstat(stream.fileno())
        if not stat.S_ISREG(before.st_mode) or before.st_size > MAX_SELECTED_REQUEST_BYTES:
            raise ValueError("selected request must be a bounded regular file")
        raw = stream.read(MAX_SELECTED_REQUEST_BYTES + 1)
        after = os.fstat(stream.fileno())
    fields = ("st_dev", "st_ino", "st_size", "st_mtime_ns", "st_ctime_ns")
    if len(raw) > MAX_SELECTED_REQUEST_BYTES or any(getattr(before, key) != getattr(after, key) for key in fields):
        raise ValueError("selected request changed while reading")
    return raw


def decode_selected_request(raw):
    if not isinstance(raw, bytes) or len(raw) > MAX_SELECTED_REQUEST_BYTES:
        raise ValueError("selected request byte limit")
    def unique(pairs):
        result = {}
        for key, value in pairs:
            if key in result:
                raise ValueError("duplicate selected request key")
            result[key] = value
        return result
    def invalid_number(value):
        raise ValueError("selected request numbers must be finite integers")
    value = json.loads(raw, object_pairs_hook=unique, parse_float=invalid_number,
                       parse_constant=invalid_number)
    required = {"schema", "manifestSha256", "invocationId", "sourceSnapshotSha256", "limits"}
    if not isinstance(value, dict) or set(value) != required or value["schema"] != SELECTED_REQUEST_SCHEMA:
        raise ValueError("closed selected request schema required")
    for key in ("manifestSha256", "sourceSnapshotSha256"):
        digest = value[key]
        if not isinstance(digest, str) or len(digest) != 64 or any(c not in "0123456789abcdef" for c in digest):
            raise ValueError("selected request requires lowercase SHA256 identities")
    if not isinstance(value["invocationId"], str) or not 1 <= len(value["invocationId"]) <= 256:
        raise ValueError("invalid selected invocation identity")
    limits = value["limits"]
    maxima = {"compileSeconds": 600, "simulationSeconds": 600,
              "totalSeconds": MAX_SELECTED_SECONDS, "logBytes": MAX_LOG_BYTES}
    if not isinstance(limits, dict) or set(limits) != set(maxima):
        raise ValueError("closed selected limits required")
    if any(type(limits[key]) is not int or not 1 <= limits[key] <= maximum
           for key, maximum in maxima.items()):
        raise ValueError("selected limit exceeds the finite envelope")
    return value


class SelectedCaseDeadlineExceeded(ValueError):
    pass


def run_selected_case(root, manifest_path, cargo_fe2o3, cache, output, request_bytes,
                      reference, simulator, *, runner=run_command, environment=None,
                      clock=time.monotonic):
    """Observe one registered positive; no native provider is accepted or invented here."""
    output.mkdir(mode=0o700, parents=False, exist_ok=False)
    report = {"schema": SELECTED_RESULT_SCHEMA, "complete": False, "qualified": False,
              "coversAllRegisteredInvocations": False, "hardwareExecuted": False,
              "grantsArtifactOrLaunchAuthority": False, "status": "request-refused",
              "phase": "request", "commands": [], "kernels": [],
              "application": {"status": "blocked", "attempted": False,
                              "reason": "manifest-bound-generated-consumer-and-native-provider-unavailable"}}
    environment = dict(os.environ if environment is None else environment)
    try:
        request = decode_selected_request(request_bytes)
        report["request"] = request
        validator, census, manifest, manifest_sha, roster = load_inputs(root, manifest_path)
        key = request["invocationId"]
        if request["manifestSha256"] != manifest_sha or key not in roster:
            raise ValueError("selected request does not match the actual manifest invocation")
        fixture, references, negative = roster[key]
        if negative:
            raise ValueError("selected end-to-end mode requires a positive; registered negatives retain their existing corpus gate")
        symbols = fixture["compilerInput"]["kernelSymbols"]
        if not symbols or len(symbols) > 64:
            raise ValueError("selected invocation root count exceeds the finite envelope")
        before = census.input_snapshot(validator, root, manifest, manifest_sha, fixture)
        if snapshot_sha256(before) != request["sourceSnapshotSha256"]:
            raise ValueError("selected request does not match the actual source snapshot")
        report.update(invocationId=key, target=fixture["target"], sourceBefore=before)
        limits = request["limits"]
        deadline = clock() + limits["totalSeconds"]
        def guard():
            if clock() >= deadline:
                raise SelectedCaseDeadlineExceeded("original selected attempt deadline exceeded")
            current = census.input_snapshot(validator, root, manifest, manifest_sha, fixture)
            if (current != before
                    or validator.load_manifest(manifest_path, with_sha256=True)[1] != manifest_sha):
                raise ValueError("selected source or manifest changed")
        def bounded_runner(arguments, cwd, child_environment, log, timeout, max_log_bytes):
            guard()
            remaining = deadline - clock()
            if remaining <= 0:
                raise SelectedCaseDeadlineExceeded("original selected attempt deadline exceeded")
            row = {"phase": report["phase"], "arguments": arguments,
                   "timeoutSeconds": min(timeout, remaining),
                   "maxLogBytes": min(max_log_bytes, limits["logBytes"])}
            report["commands"].append(row)
            outcome = runner(arguments, cwd, child_environment, log,
                             row["timeoutSeconds"], row["maxLogBytes"])
            row["outcome"] = outcome
            guard()
            return outcome
        report["phase"] = "compile"
        compiled = output / "compilation"
        compilation = run_census(root, manifest_path, cargo_fe2o3, cache, compiled,
            (fixture["target"],), limits["compileSeconds"], limits["logBytes"],
            runner=bounded_runner, environment=environment, production=True, capture_graphs=True,
            isolate_negative_outputs=True, invocation_ids=(key,),
            expected_source_sha256=request["sourceSnapshotSha256"])
        report["compilation"] = compilation
        if compilation["prerequisiteFailures"]:
            report["status"] = "blocked-prerequisite"
        else:
            if len(compilation["cases"]) != 1:
                raise ValueError("selected compilation changed its case roster")
            case = compilation["cases"][0]
            if (case["id"] != key or case["fixture"] != fixture
                    or case["references"] != [list(row) for row in references]
                    or case["expectedNegative"] is not False
                    or case["status"] != "production-compile-census-pass"
                    or case.get("sourceBefore") != before or case.get("sourceAfter") != before):
                raise ValueError("selected production compilation refused or changed its input")
            guard()
            simulation = load_module("tutorial_selected_simulation_v92", root / "scripts/tutorial_simulation_v92.py")
            tools = (reference, simulator)
            pins = [simulation.pin_executable(path) for path in tools]
            report["tools"] = [dict(path=str(path), **pin) for path, pin in zip(tools, pins)]
            report["phase"] = "cpu-oracle"
            directory = output / "simulation"
            directory.mkdir(mode=0o700)
            for ordinal, kernel in enumerate(symbols):
                guard()
                request_path = directory / f"{ordinal:04d}.request.json"
                generated = bounded_runner([str(reference), "generate", kernel], root,
                    environment, request_path, min(120, limits["simulationSeconds"]), limits["logBytes"])
                if not simulation.command_passed(generated):
                    raise ValueError("selected independent request generation failed")
                value = simulation.strict_json(simulation.read_file(request_path)[0])
                if value["kernel"] != kernel:
                    raise ValueError("selected independent request substituted the kernel")
                observation = simulation.run_case(case, compiled / case["capturedGraphsV92"],
                    request_path, reference, simulator, directory / f"{ordinal:04d}", bounded_runner,
                    environment=environment, cwd=root, timeout=limits["simulationSeconds"])
                report["kernels"].append({"kernel": kernel, "generation": generated,
                                           "observation": observation})
                guard()
            if [simulation.pin_executable(path) for path in tools] != pins:
                raise ValueError("selected CPU tools changed")
            report["sourceAfter"] = census.input_snapshot(validator, root, manifest, manifest_sha, fixture)
            guard()
            report["phase"] = "application"
            report["status"] = "blocked-native-prerequisite"
    except SelectedCaseDeadlineExceeded as error:
        report.update(status="deadline-exceeded", error=str(error))
    except KeyboardInterrupt:
        report.update(status="interrupted", error="selected attempt interrupted")
        raise
    except (OSError, ValueError, KeyError, TypeError, SystemExit) as error:
        report.update(status="request-refused" if report["phase"] == "request" else "phase-refused",
                      error=str(error)[:4096])
    except Exception as error:
        report.update(status="internal-error", error=f"{type(error).__name__}: {error}"[:4096])
        raise
    finally:
        report["complete"] = True
        try:
            payload = json.dumps(report, indent=2, sort_keys=True, allow_nan=False) + "\n"
            if len(payload.encode()) > MAX_LOG_BYTES:
                raise ValueError("selected terminal receipt exceeds the finite envelope")
        except (TypeError, ValueError):
            report = {"schema": SELECTED_RESULT_SCHEMA, "complete": True,
                      "status": "receipt-refused", "priorStatus": report["status"],
                      "qualified": False, "hardwareExecuted": False,
                      "grantsArtifactOrLaunchAuthority": False,
                      "phase": report["phase"], "application": report["application"],
                      "error": "invalid or oversized terminal data; inspect retained phase files"}
            payload = json.dumps(report, sort_keys=True) + "\n"
        with (output / "terminal.json").open("x", encoding="utf-8") as stream:
            stream.write(payload)
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
    parser.add_argument("--selected-request", type=Path, help="closed single-invocation request; never whole-corpus or native qualification")
    parser.add_argument("--reference", type=Path, help="existing reviewed reference executable for selected mode")
    parser.add_argument("--simulator", type=Path, help="existing reviewed current simulator executable for selected mode")
    args = parser.parse_args(argv)
    if not 1 <= args.timeout_seconds <= MAX_TIMEOUT_SECONDS or not 1 <= args.max_log_bytes <= MAX_LOG_BYTES:
        parser.error("timeout or log limit exceeds the finite harness envelope")
    if not args.cargo_fe2o3.is_absolute():
        parser.error("--cargo-fe2o3 must be absolute")
    root = args.repo_root.resolve(strict=True)
    manifest = args.manifest if args.manifest.is_absolute() else root / args.manifest
    targets = ("gfx942", "gfx950") if args.target == "all" else (args.target,)
    if args.selected_request is not None:
        if (args.legacy_compile_census or args.target != "all" or args.reference is None
                or args.simulator is None or not args.reference.is_absolute()
                or not args.simulator.is_absolute() or args.timeout_seconds != 600
                or args.max_log_bytes != MAX_LOG_BYTES):
            parser.error("selected mode requires reviewed absolute CPU tools and request-derived target/limits, without legacy or target/limit overrides")
        try:
            report = run_selected_case(root, manifest, args.cargo_fe2o3, args.target_dir.resolve(),
                args.output.resolve(), read_selected_request(args.selected_request), args.reference, args.simulator)
        except (OSError, ValueError, SystemExit) as error:
            print(f"tutorial selected case refused: {error}", file=sys.stderr)
            return 2
        print(json.dumps({"receipt": str(args.output.resolve() / "terminal.json"),
                          "status": report["status"], "qualified": False}, sort_keys=True))
        return 1
    if args.reference is not None or args.simulator is not None:
        parser.error("CPU tool paths require selected mode")
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
