#!/usr/bin/env python3
"""Bounded, observation-only preflight for the installed joined native manager.

These records cannot establish root custody, readiness, or cleanup. The original
compiler and application protocols remain the admission boundary. Nothing here
starts, stops, connects to, or polls a service.
"""
from pathlib import Path
import hashlib
import re
import stat


MANAGER = "fe2o3-native-application-manager.service"
STANDALONE = ("fe2o3-compiler-execution.service", "fe2o3-compiler-execution-v1.service")
UNITS = (MANAGER, *STANDALONE)
PROPERTIES = ("Id", "LoadState", "ActiveState", "SubState", "MainPID", "InvocationID", "Job")
COMPILER_SOCKET = Path("/run/fe2o3/compiler-execution-supervisor.sock")
APPLICATION_SOCKET = Path("/run/fe2o3/native-application-supervisor.sock")
MAX_LOG = 16384
TIMEOUT = 5


def phase_plan():
    """Inert plan usable by private-cgroup runners without host systemd."""
    return {
        "mode": "joined-native-manager-v3",
        "compilerRequests": 1,
        "standaloneCoordinatorAllowed": False,
        "applicationListenerRequiredBeforeCompiler": False,
        "successfulCompilerCompletionGate": "original-retirement-then-actual-application-supervisor-ready",
        "unsuccessfulCompilerCompletionGate": "original-retirement-without-application-startup",
        "cleanupController": "same-original-controller-final-shutdown-once",
        "grantsAdmissionAuthority": False,
    }


def parse_units(raw):
    if not isinstance(raw, bytes) or not 0 < len(raw) <= MAX_LOG:
        raise ValueError("bounded systemd startup observation required")
    records = {}
    for block in raw.decode("ascii", errors="strict").strip().split("\n\n"):
        record = {}
        for line in block.splitlines():
            key, separator, value = line.partition("=")
            if not separator or key not in PROPERTIES or key in record:
                raise ValueError("unexpected or duplicate systemd property")
            record[key] = value
        if set(record) != set(PROPERTIES) or record["Id"] in records:
            raise ValueError("incomplete or duplicate systemd unit")
        records[record["Id"]] = record
    if set(records) != set(UNITS):
        raise ValueError("exact native manager and standalone compiler units required")
    manager = records[MANAGER]
    if (manager["LoadState"] != "loaded" or manager["ActiveState"] != "active"
            or manager["SubState"] != "running" or manager["Job"]
            or not re.fullmatch(r"[1-9][0-9]{0,9}", manager["MainPID"])
            or not 1 <= int(manager["MainPID"]) <= 2**31 - 1
            or not re.fullmatch(r"[0-9a-f]{32}", manager["InvocationID"])
            or manager["InvocationID"] == "0" * 32):
        raise ValueError("joined native manager is not observably active and running")
    for name in STANDALONE:
        other = records[name]
        if (other["LoadState"] not in ("loaded", "not-found") or other["Job"]
                or (other["ActiveState"], other["SubState"]) not in
                (("inactive", "dead"), ("failed", "failed")) or other["MainPID"] != "0"):
            raise ValueError("conflicting standalone compiler startup")
    return records


def socket_phase(compiler=COMPILER_SOCKET, application=APPLICATION_SOCKET):
    if not stat.S_ISSOCK(compiler.stat(follow_symlinks=False).st_mode):
        raise ValueError("fixed compiler listener is absent or substituted")
    try:
        application.stat(follow_symlinks=False)
    except FileNotFoundError:
        pass
    else:
        raise ValueError("application listener must be absent before the original compiler request")
    return {"compilerSocket": str(compiler), "applicationSocket": str(application),
            "compilerSocketObserved": True, "applicationPathAbsent": True,
            "socketCustodyAuthenticated": False}


def observe_installed_systemd(support, output):
    arguments = ["/usr/bin/systemctl", "--no-pager", "--all", "show",
                 "--property=" + ",".join(PROPERTIES), "--", *UNITS]
    log = output / "startup-systemd.log"
    outcome = support.run_command(arguments, output, {"PATH": "/usr/bin:/bin", "LC_ALL": "C"},
                                  log, TIMEOUT, MAX_LOG)
    if (outcome["status"] != "cargo-completed-unqualified"
            or type(outcome["exitCode"]) is not int or outcome["exitCode"] != 0
            or outcome["logComplete"] is not True or outcome["directChildReaped"] is not True
            or type(outcome["logBytes"]) is not int or not 0 < outcome["logBytes"] <= MAX_LOG):
        raise ValueError("bounded systemd startup observation did not complete")
    with log.open("rb") as file:
        raw = file.read(MAX_LOG + 1)
    if len(raw) != outcome["logBytes"] or hashlib.sha256(raw).hexdigest() != outcome["logSha256"]:
        raise ValueError("systemd startup observation changed")
    units = parse_units(raw)
    paths = socket_phase()
    return {"plan": phase_plan(), "scope": "systemd-observation-only-not-admission",
            "arguments": arguments, "execution": outcome, "units": units, "paths": paths}
