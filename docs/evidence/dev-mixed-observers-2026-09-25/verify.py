#!/usr/bin/env python3
"""Replay this failed qualification; successful replay is not a passing suite."""
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import sys
import tarfile

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("observer_runner_replay", HERE / "run.py")
R = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(R)
V = R.load(HERE.parent / "dev-mixed-duration-2026-09-25/verify.py",
           "349db7a83a824b02cf5dea6f7f5f57c8847a011228818b479f8fe18e2c28fe51", "observer_source_replay")
V.PREFIXES += [str((HERE / name).relative_to(R.ROOT)) for name in ("run.py", "test_run.py")]
FAILURES = {
    "authorized_execution::tests::cooperative_debug_telemetry_emits_only_bounded_logical_records",
    "authorized_execution::tests::failed_session_end_is_explicit_and_terminal",
    "authorized_execution::tests::pre_native_telemetry_failure_is_returned_and_poisoned",
}


def runtime_result(stdout, names):
    summaries = re.findall(r"^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;", stdout, re.M)
    R.H.need(len(summaries) == 1, "one terminal suite summary")
    kind, *counts = summaries[0]
    counts = list(map(int, counts))
    rows = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)(?:, [^\n]*)?$", stdout, re.M)
    R.H.need(len(rows) == len(dict(rows)) == len(names) and set(dict(rows)) == set(names), "exact complete test result roster")
    actual = dict(rows)
    observed = [sum(value == state for value in actual.values()) for state in ("ok", "FAILED", "ignored")]
    R.H.need(counts == [*observed, 0, 0], "raw test rows match summary")
    failed = {name for name, status in rows if status == "FAILED"}
    blocks = re.findall(r"^---- (\S+) stdout ----$", stdout, re.M)
    R.H.need(len(blocks) == len(set(blocks)) and set(blocks) == failed, "failure diagnostics agree with test rows")
    failure_lists = re.findall(r"^failures:\n((?:    [^\n]+\n)+)", stdout, re.M)
    R.H.need(len(failure_lists) == int(bool(failed)), "one terminal failure list when failed")
    if failed:
        terminal = failure_lists[0].split()
        R.H.need(len(terminal) == len(set(terminal)) and set(terminal) == failed, "terminal failure identities agree")
    R.H.need((kind == "ok" and counts == [1425, 0, 27, 0, 0]) or
             (kind == "FAILED" and counts == [1422, 3, 27, 0, 0] and failed == FAILURES), "only classified complete suite outcomes")
    for name in failed:
        block = re.search(r"^---- " + re.escape(name) + r" stdout ----\n(.*?)(?=^---- |^failures:|\Z)", stdout, re.M | re.S)
        R.H.need(block is not None and 'InspectSocket(Os { code: 1, kind: PermissionDenied, message: "Operation not permitted" })' in block[1], "socket admission EPERM for " + name)
    for suffix in R.CPU_NAMES:
        R.H.need([status for name, status in rows if name.endswith("::" + suffix)] == ["ok"], "new CPU regression passed")
    for suffix in R.NATIVE_NAMES:
        R.H.need([status for name, status in rows if name.endswith("::" + suffix)] == ["ignored"], "native regression not executed")
    return dict(passed=counts[0], failed=sorted(failed), ignored=counts[2], status=101 if failed else 0)


def phase_records(root, phases, failed_phases=()):
    results = V.read(root / "results.json")
    records = {path.parent.name: V.read(path) for path in root.glob("*/record.json")}
    R.H.need(results.keys() == records.keys() == set(phases), "complete phase and command rosters")
    for name, row in records.items():
        R.H.need(type(row["status"]) is int and row["status"] == results[name]["status"] and row["group_absent"] is True,
                 "terminal reaped command: " + name)
        R.H.need(type(row["started_ns"]) is int and type(row["finished_ns"]) is int and
                 row["finished_ns"] >= row["started_ns"] and type(row["process_group"]) is int and row["process_group"] > 1,
                 "command interval and identity")
        stdout = (root / name / "stdout.log").read_text()
        R.H.need(results[name]["counts"] == V.raw_counts(stdout), "raw successful-summary counts")
        if name in failed_phases:
            R.H.need(row["status"] == 101 and results[name]["passed"] is False, "retained failed prerequisite")
        elif name not in ("gnu", "musl"):
            R.H.need(row["status"] == 0 and results[name]["passed"] is True, "independent check passed: " + name)
    return results, records


def verify(root=HERE):
    original, prior, tail = root / "raw", root / "continuation", root / "validation"
    before = V.read(original / "source-before.json")
    R.H.need(all(V.read(path) == before for path in (original / "source-after.json", prior / "source-before.json", prior / "source-after.json", tail / "source-before.json", tail / "source-after.json")), "signed source continuity across all runs")
    V.authenticate_source(original, before)
    archive = original / "source.tar.gz"
    metadata = V.read(original / "archive.json")
    R.H.need(hashlib.sha256(archive.read_bytes()).hexdigest() == metadata["sha256"], "archive digest")
    with tarfile.open(archive, "r:gz") as packed:
        members = V.check_members(packed.getmembers())
        R.H.need(members.keys() == before["inputs"].keys(), "complete signed archive")
        for name, facts in before["inputs"].items():
            raw = packed.extractfile(members[name]).read()
            R.H.need(R.H.bind_blob(raw, facts["git_blob"]) == facts["sha256"], "signed archive member: " + name)
    for packet, names in ((prior, ("finish.py", "socket_probe.py", "run.py", "test_run.py")),
                          (tail, ("resume.py", "finish.py", "socket_probe.py", "run.py", "test_run.py"))):
        helpers = V.read(packet / "helpers-before.json")
        R.H.need(helpers == V.read(packet / "helpers-after.json") ==
                 {name: hashlib.sha256((HERE / name).read_bytes()).hexdigest() for name in names}, "measured continuation helper continuity")
        for name in ("run.py", "test_run.py"):
            R.H.need(helpers[name] == before["inputs"][str((HERE / name).relative_to(R.ROOT))]["sha256"], "measured helper matches signed source")
    environment = V.read(original / "environment.json")
    R.H.need(environment == V.read(prior / "environment.json") == V.read(tail / "environment.json"), "same base build environment")
    R.H.need(V.read(tail / "musl-environment.json") == {name: value for name, value in environment.items() if name != "RUSTFLAGS"}, "only musl linker flag removed")
    target = Path(metadata["source"]).parent
    R.H.need(environment == dict(HOME="/home/harsh", USER="harsh", PATH="/home/harsh/.cargo/bin:/usr/bin:/bin", LC_ALL="C",
             CARGO_HOME=str(target / "cargo-home"), CARGO_TARGET_DIR=str(target / "build"), CARGO_BUILD_JOBS="2",
             CARGO_INCREMENTAL="0", CARGO_TERM_COLOR="never", CARGO_PROFILE_TEST_DEBUG="0", CARGO_PROFILE_DEV_DEBUG="0",
             RUSTFLAGS="-Clink-arg=-Wl,--threads=1", GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL="/dev/null"), "exact isolated build environment")
    first, first_records = phase_records(original, ("signature", "source-archive", "runner-tests", "rustc", "gnu-build", "gnu-roster", "gnu"))
    rejected, rejected_records = phase_records(prior, ("socket-probe", "musl-build"), ("musl-build",))
    rest, rest_records = phase_records(tail, ("musl-build", "musl-roster", "musl", "doctests", "default", "clippy", "format"))
    cargo = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-runtime"]
    recorded_here = R.ROOT / "docs/evidence/dev-mixed-observers-2026-09-25"
    commands = {
        "signature": ["/usr/bin/git", "--no-replace-objects", "-c", "gpg.format=ssh", "-c", "gpg.ssh.program=/usr/bin/ssh-keygen", "-c", "gpg.ssh.allowedSignersFile=" + str(recorded_here / "raw/allowed-signers"), "verify-commit", before["commit"]],
        "source-archive": ["/usr/bin/git", "--no-replace-objects", "archive", "--format=tar.gz", "--output=" + str(recorded_here / "raw/source.tar.gz"), before["commit"], *[prefix for prefix in R.H.INPUTS if any(name == prefix or name.startswith(prefix + "/") for name in before["inputs"])]],
        "runner-tests": ["/usr/bin/python3", "-I", "-B", str(recorded_here / "test_run.py")],
        "rustc": ["rustc", "-Vv"],
        "socket-probe": ["/usr/bin/python3", "-I", "-B", str(recorded_here / "socket_probe.py")],
        "gnu-build": [*cargo, "--all-features", "--lib", "--no-run", "--message-format=json"],
        "musl-build": [*cargo, "--all-features", "--lib", "--target", "x86_64-unknown-linux-musl", "--no-run", "--message-format=json"],
        "doctests": [*cargo, "--all-features", "--doc"],
        "default": ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime"],
        "clippy": ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"],
        "format": ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"],
    }
    records = {**first_records, **rest_records, "socket-probe": rejected_records["socket-probe"]}
    for name, command in commands.items():
        V.check_command(records[name], command)
    V.check_command(rejected_records["musl-build"], commands["musl-build"])
    failed_build = [json.loads(line) for line in (prior / "musl-build/stdout.log").read_text().splitlines() if line.startswith("{")]
    errors = [row for row in failed_build if row.get("reason") == "compiler-message" and row["message"]["level"] == "error"]
    R.H.need(len(errors) == 1 and errors[0]["message"]["message"] == "linking with `cc` failed: exit status: 1" and
             "/usr/bin/ld: unrecognized option '--threads=1'" in errors[0]["message"]["rendered"] and
             [row for row in failed_build if row.get("reason") == "build-finished"] == [dict(reason="build-finished", success=False)], "preserved musl linker configuration failure")
    rosters, suites = [], {}
    for platform, packet, results in (("gnu", original, first), ("musl", tail, rest)):
        elf = V.read(packet / (platform + "-runtime-tests.json"))
        builds = [json.loads(line) for line in (packet / (platform + "-build/stderr.log")).read_text().splitlines() if line.startswith("{")]
        # Cargo JSON is on stdout; stderr is deliberately not treated as an artifact report.
        R.H.need(not builds, "no ambiguous compiler JSON channel")
        artifacts = [json.loads(line) for line in (packet / (platform + "-build/stdout.log")).read_text().splitlines() if line.startswith("{")]
        artifacts = [row for row in artifacts if row.get("reason") == "compiler-artifact" and
                     row.get("target", {}).get("name") == "fe2o3_runtime" and row.get("profile", {}).get("test") and row.get("executable")]
        R.H.need(len(artifacts) == 1 and artifacts[0]["executable"] == elf["path"] and
                 artifacts[0]["target"]["kind"] == ["lib"] and artifacts[0]["target"]["src_path"] ==
                 str(Path(metadata["source"]) / "crates/fe2o3-runtime/src/lib.rs") and
                 artifacts[0]["package_id"] == "path+file://" + metadata["source"] + "/crates/fe2o3-runtime#0.1.0",
                 "retained ELF joins exact signed library build")
        V.check_elf((packet / (platform + "-runtime-tests.gz")).read_bytes(), elf, results[platform], records[platform])
        if platform == "gnu":
            R.H.need(V.read(tail / "gnu-delayed-digest.json") == dict(sha256=elf["sha256"], immediate_post_execution=False), "delayed, not immediate, GNU digest continuity")
        V.check_command(records[platform + "-roster"], [elf["path"], "--list"])
        roster = R.roster((packet / (platform + "-roster/stdout.log")).read_text())
        rosters.append(roster)
        suites[platform] = runtime_result((packet / platform / "stdout.log").read_text(), roster)
        R.H.need(results[platform]["status"] == suites[platform]["status"] and results[platform]["passed"] is (not suites[platform]["failed"]), "suite result agrees with raw rows")
    R.H.need(rosters[0] == rosters[1] and suites["gnu"]["failed"] == sorted(FAILURES), "same roster; GNU failure remains recorded")
    R.H.need(rest["doctests"]["counts"] == [[4, 0, 0, 0, 0], [42, 0, 0, 0, 0]], "all split doctests")
    probe = json.loads((prior / "socket-probe/stdout.log").read_text())
    R.H.need([row["operation"] for row in probe] == ["SO_DOMAIN", "SO_TYPE", "getpeername", "SO_PEERCRED", "set_SO_PASSCRED", "get_SO_PASSCRED"] and all(row.get("errno") == 1 for row in probe), "environment rejects socket introspection")
    return dict(commit=before["commit"], source_inputs=len(before["inputs"]), commands=len(records) + 1,
                suites=suites, cpu_qualification_passed=False, native_executed=False,
                gnu_immediate_post_execution_digest=False, retained_linker_configuration_failure=True,
                continuation_helpers_signed_before_execution=False)


if __name__ == "__main__":
    R.H.need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    print(json.dumps(verify(Path(sys.argv[1]) if len(sys.argv) == 2 else HERE), sort_keys=True))
