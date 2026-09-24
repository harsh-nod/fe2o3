#!/usr/bin/env python3
"""Replay signed source, all native receipts/results and owned cleanup."""

import hashlib
import json
from datetime import datetime, timezone
from pathlib import Path
import re
import shlex
import subprocess
import tarfile
from types import ModuleType

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[2]
PACKET_PATH = str(ROOT.relative_to(REPO))
PROTOCOL_COMMIT = "f8f7c46f584ce7dfa72e4e106d64b0f71d3f8e5e"
LOCAL = Path("/home/harsh/.codex-tmp/fe2o3-hot-batch-20260924-feYb12D3/native1")
CAMPAIGN_SHA = "a6b2f42beb687b82bc9f8207acc034df6003c598eb7dc8308447024ef421f940"


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary retained file")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def unique(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, "duplicate JSON key")
        result[key] = value
    return result


def read(path):
    return json.loads(path.read_bytes(), object_pairs_hook=unique)


def load_campaign():
    path = ROOT / "campaign.py"
    need(sha(path) == CAMPAIGN_SHA, "pinned campaign")
    value = ModuleType("hot_batch_replay_campaign")
    value.__file__ = str(path)
    exec(compile(path.read_bytes(), str(path), "exec"), value.__dict__)
    return value


C = load_campaign()
N, B, H = C.N, C.B, C.H


def git(*arguments, input=None):
    return subprocess.check_output([*C.K.GIT, *arguments], cwd=REPO, env=C.K.GIT_ENV, timeout=120, input=input)


def signed_files(commit, selectors):
    tree = git("ls-tree", "-rz", commit, "--", *selectors)
    entries = []
    for row in tree.rstrip(b"\0").split(b"\0"):
        header, name = row.split(b"\t")
        mode, kind, oid = header.decode().split()
        need(mode in ("100644", "100755") and kind == "blob", "ordinary signed source")
        entries.append((name.decode(), oid))
    need(len({name for name, _ in entries}) == len(entries), "unique signed sources")
    blobs = git("cat-file", "--batch", input="".join(oid + "\n" for _, oid in entries).encode())
    cursor, result = 0, {}
    for name, expected_oid in entries:
        end = blobs.index(b"\n", cursor)
        oid, kind, size = blobs[cursor:end].decode().split()
        need(oid == expected_oid and kind == "blob" and size.isdecimal(), "exact signed blob")
        length = int(size)
        raw = blobs[end + 1:end + 1 + length]
        need(len(raw) == length and blobs[end + 1 + length:end + 2 + length] == b"\n", "complete signed blob")
        result[name] = hashlib.sha256(raw).hexdigest()
        cursor = end + 2 + length
    need(cursor == len(blobs), "exact signed blob roster")
    return tree, result


def receipt(folder, command, cwd, seconds, environment, stdin=None, expected_exit=0):
    row = read(folder / "receipt.json")
    need(set(row) == {"command", "cwd", "started_ns", "timeout_seconds", "pid", "exit", "error",
                      "group_absent", "environment", "stdin_sha256", "finished_ns", "stdout_sha256", "stderr_sha256"}, "receipt roster")
    need(type(row["exit"]) is int and row["exit"] == expected_exit and row["error"] is None
         and row["group_absent"] is True, "expected reaped command outcome: " + folder.name)
    need(type(row["pid"]) is int and row["pid"] > 0, "positive process identity")
    need(type(row["started_ns"]) is int and type(row["finished_ns"]) is int
         and 0 < row["started_ns"] < row["finished_ns"], "receipt chronology")
    need(row["command"] == command and row["cwd"] == str(cwd), "exact command/cwd: " + folder.name)
    need(type(row["timeout_seconds"]) is int and row["timeout_seconds"] == seconds, "command deadline")
    need(row["environment"] == environment, "command environment: " + folder.name)
    need(row["stdin_sha256"] == (hashlib.sha256(stdin).hexdigest() if stdin is not None else None), "command input")
    for stream in ("stdout", "stderr"):
        need(row[stream + "_sha256"] == sha(folder / stream), "captured stream digest")
    return row


def utc_ns(value):
    need(type(value) is str, "UTC timestamp string")
    match = re.fullmatch(r"([0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2})\.([0-9]{9})Z", value)
    need(match is not None, "nanosecond UTC timestamp")
    parsed = datetime.strptime(match[1], "%Y-%m-%dT%H:%M:%S").replace(tzinfo=timezone.utc)
    elapsed = parsed - datetime(1970, 1, 1, tzinfo=timezone.utc)
    return (elapsed.days * 86400 + elapsed.seconds) * 10**9 + int(match[2])


def endpoint_containment(observation, row):
    opening = utc_ns(observation["started"]["utc"])
    closing = utc_ns(observation["finished"]["utc"])
    need(row["started_ns"] <= opening <= closing <= row["finished_ns"], "fresh endpoint receipt containment")
    previous = opening
    snapshots = observation["sysfs"]
    for captured in (snapshots[0], observation["status"], snapshots[1], observation["pids"], snapshots[2]):
        start, end = utc_ns(captured["started"]["utc"]), utc_ns(captured["finished"]["utc"])
        need(previous <= start <= end <= closing, "fresh nested endpoint captures")
        previous = end


def main(recovered=False):
    raw = ROOT / "raw"
    need(B.inventory(raw) == read(ROOT / "artifacts.json"), "complete artifact roster")
    run = raw / "native1"
    if not recovered:
        need(read(run / "collection.json")["failures"] == [], "strict campaign did not qualify")
    binding, marker = read(run / "binding.json"), read(run / "owner.json")
    need(set(marker) == {"path", "commit", "binding_sha256"}, "owner marker roster")
    need(re.fullmatch(re.escape(N.PREFIX) + r"[0-9a-f]{16}", marker["path"]) is not None, "owned remote path")
    need(marker["binding_sha256"] == sha(run / "binding.json") and marker["commit"] == binding["commit"] == C.SOURCE_COMMIT,
         "exact signed source binding")
    owned = Path(marker["path"])
    devices = C.K.devices_from_args([",".join(map(str, device)) for device in binding["devices"]])
    need(H.same_json(binding["order"], N.ORDER) and H.same_json(binding["controls"], N.CONTROLS), "fixed trial plan")
    need(binding["target"] == "x86_64-unknown-linux-musl" and binding["features"] == "default"
         and binding["profile"] == "release default" and type(binding["outer_native_seconds"]) is int
         and binding["outer_native_seconds"] == N.REMOTE_SECONDS, "fixed build profile/bounds")
    need(H.sha(Path(C.C.SIGNERS)) == C.C.SIGNERS_SHA, "trusted signer list")
    for commit in (C.SOURCE_COMMIT, PROTOCOL_COMMIT):
        git("-c", "gpg.ssh.allowedSignersFile=" + C.C.SIGNERS, "verify-commit", commit)
    tree, sources = signed_files(C.SOURCE_COMMIT, C.K.SELECTORS)
    need(sources == binding["local_source_files"] == read(run / "source.json"), "complete signed source map")
    need((run / "local/source-tree/stdout").read_bytes() == tree, "signed tree receipt")
    _, cpu_files = signed_files(C.SOURCE_COMMIT, [C.CPU, C.PRIOR_CPU])
    need(cpu_files == binding["cpu_qualification"]["evidence_files"], "complete signed CPU evidence map")
    need(binding["cpu_qualification"]["commit"] == C.SOURCE_COMMIT and binding["cpu_qualification"]["packet"] == C.CPU
         and binding["cpu_qualification"]["checker_sha256"] == C.CPU_VERIFY_SHA, "CPU qualification identity")
    need(binding["cpu_qualification"]["input_sha256"] == sha(run / "local/cpu-source-map/stdout"), "CPU source-map receipt")
    need(sha(run / "local/cpu-source-map/stdout") == cpu_files[C.CPU + "/raw/cpu2/inputs-before.json"], "signed CPU map bytes")
    need((run / "local/cpu-packet-tree/stdout").read_bytes() == git("ls-tree", "-rz", C.SOURCE_COMMIT, "--", C.CPU, C.PRIOR_CPU),
         "signed CPU tree receipt")
    protocol_base = PACKET_PATH + "/"
    _, signed_protocol = signed_files(PROTOCOL_COMMIT, [protocol_base + name for name in C.PROTOCOL])
    protocol = {name: signed_protocol[protocol_base + name] for name in C.PROTOCOL}
    need(protocol == read(run / "protocol-before.json") == read(run / "protocol-after.json")
         == binding["protocol_files"] == B.inventory(run / "protocol"), "signed frozen protocol bracket")
    need({name: sha(ROOT / name) for name in C.PROTOCOL} == protocol, "current frozen protocol")
    verify_protocol(protocol)
    need(B.inventory(run / "payload") == binding["payload"] and set(binding["payload"]) == N.PAYLOAD, "payload roster")
    need(binding["payload"]["native.py"] == protocol["native.py"] and binding["payload"]["hot.py"] == N.HOT_SHA
         and binding["payload"]["base.py"] == N.BASE_SHA and binding["payload"]["results.py"] == N.PARSER_SHA, "payload helper identities")
    remote_sources = {name: sources[name] for name in C.C.SOURCE_FILES}
    need(remote_sources == binding["source_files"], "signed remote source map")
    with tarfile.open(run / "payload/source.tar.gz", "r:gz") as archive:
        members = archive.getmembers()
        B.validate_members(members, remote_sources)
        for member in members:
            need(0 <= member.size <= 1024 * 1024, "bounded remote source member")
            need(hashlib.sha256(archive.extractfile(member).read()).hexdigest() == remote_sources[member.name], "remote source archive bytes")
    for tool in ("rustc", "cargo"):
        for stream in ("stdout", "stderr"):
            need(sha(run / "local" / tool / stream) == cpu_files[C.CPU + "/raw/cpu2/" + tool + "/" + stream + ".log"],
                 "CPU/native Rust tool identity")
    result_root = run / "recovery1" if recovered else run
    remote = result_root / "remote"
    inventory_command = result_root / ("commands" if recovered else "local") / "inventory"
    need(B.inventory(remote) == read(result_root / "remote-inventory.json") == H.parse_json((inventory_command / "stdout").read_bytes()), "byte-exact remote collection")
    need(read(remote / "source-before.json") == read(remote / "source-after.json") == remote_sources, "remote source bracket")
    binaries = read(remote / "binaries.json")
    need(set(binaries) == set(N.BINARIES) and binaries == read(remote / "binaries-after.json"), "three ELF bracket")
    need(B.inventory(remote / "artifacts") == {N.BINARIES[key]: value for key, value in binaries.items()}, "retained ELF bytes")
    need(binaries["kfd"] == binding["payload"]["kfd"] == read(run / "built-kfd.json")["sha256"], "standalone KFD build identity")
    env = H.environment(owned)
    expected_remote, previous = [], 0
    def remote_receipt(name, command, seconds, environment=env):
        nonlocal previous
        expected_remote.append(name)
        row = receipt(remote / name, command, owned / "source", seconds, environment)
        need(previous <= row["started_ns"], "remote command ordering")
        previous = row["finished_ns"]
        return row
    for name, command, seconds in N.IDENTITIES + N.builds(owned):
        remote_receipt(name, command, seconds)
    results, parser = [], N.load(run / "payload/results.py", N.PARSER_SHA, "hot_batch_replay_parser")
    for name, backend, depth, command, phase_env in N.trials(owned, devices):
        for suffix, delay in (("-before", 0), ("-settled", 2), ("-delayed", 20)):
            anchor = previous
            for ordinal, (index, bdf, uid) in enumerate(devices):
                label = name + suffix
                row = remote_receipt(label + "-gpu" + str(index), H.observe_spec(label, index, bdf, uid), N.OBSERVE_SECONDS)
                if ordinal == 0:
                    need(row["started_ns"] - anchor >= delay * 10**9, "settling interval")
                folder = remote / (label + "-gpu" + str(index))
                need((folder / "stderr").read_bytes() == b"", "observer stderr")
                observation = H.parse_endpoint((folder / "stdout").read_bytes(), index, bdf, uid)
                endpoint_containment(observation, row)
            if suffix == "-before":
                remote_receipt(name, command, N.TRIAL_SECONDS, phase_env)
                need((remote / name / "stderr").read_bytes() == b"", "workload stderr")
                parsed = parser.parse_result((remote / name / "stdout").read_bytes(), backend=backend,
                                             unique_ids=[d[2] for d in devices], copy_bytes=N.CONTROLS["bytes"], depth=depth,
                                             warmups=N.CONTROLS["warmups"], samples=N.CONTROLS["samples"])
                results.append(dict(trial=name, backend=backend, depth=depth, result=parsed))
    for name, command, seconds in N.IDENTITIES:
        remote_receipt("after-" + name, command, seconds)
        for stream in ("stdout", "stderr"):
            need((remote / name / stream).read_bytes() == (remote / ("after-" + name) / stream).read_bytes(), "remote tool continuity")
    need({p.parent.name for p in remote.glob("*/receipt.json")} == set(expected_remote) and len(expected_remote) == 134, "134 exact remote commands")
    transcript = (run / "local/native/stdout").read_bytes()
    reports = [dict(phase=name, exit=0, error=None, group_absent=True) for name in expected_remote]
    if recovered:
        expected_transcript = "".join(json.dumps(row) + "\n" for row in reports).encode()
        need(transcript and expected_transcript.startswith(transcript), "exact recovered stdout prefix")
        need((run / "local/native/stderr").read_bytes() == b"Timeout, server sharkmi300x-1 not responding.\r\n", "preserved SSH timeout")
    else:
        need(H.same_json([json.loads(line, object_pairs_hook=unique) for line in transcript.splitlines()], reports), "remote phase transcript")
        need((run / "local/native/stderr").read_bytes() == b"", "remote controller stderr")
    need(H.same_json(results, read(remote / "validated-results.json")) and len(results) == 18, "18 replayed trial results")
    need(H.same_json(read(remote / "finished.json"), dict(commit=C.SOURCE_COMMIT, failures=[], native_execution=True,
                                                        exclusive_reservation=False, performance_acceptance=False, formal_refinement=False)), "native closure")
    previous = verify_local(run, binding, marker, sources, recovered=recovered)
    if recovered:
        need(H.same_json(read(run / "collection.json"), dict(failures=["RuntimeError('native failed')",
             "collection: RuntimeError('inventory failed')", "remote receipts retained for recovery"],
             owned_cleanup=False, exclusive_reservation=False, performance_acceptance=False)), "preserved controller rejection")
        verify_recovery(run, marker, previous)
    else:
        need(H.same_json(read(run / "collection.json"), dict(failures=[], owned_cleanup=True, exclusive_reservation=False,
                                                           performance_acceptance=False)), "successful collection closure")
    cleanup = read(raw / "local-cleanup.json")
    need(type(cleanup) is list and len(cleanup) == 3, "local cleanup roster")
    for row, name in zip(cleanup, ("source", "target", "build-source.tar.gz"), strict=True):
        path = LOCAL / name
        need(set(row) == {"path", "allocated_bytes", "absent"} and row["path"] == str(path)
             and type(row["allocated_bytes"]) is int and row["allocated_bytes"] >= 0 and row["absent"] is True,
             "local cleanup record")
        need(not path.exists() and not path.is_symlink(), "local transient remains")
    print(json.dumps({"native_trials": len(results), "remote_commands": len(expected_remote), "endpoints": 108,
                      "local_commands": 20 if recovered else 19, "record_replay": "pass", "recovered": recovered,
                      "campaign_accepted": not recovered, "performance_acceptance": False}))


def verify_protocol(protocol):
    folder = ROOT / "protocol3"
    prefix = PACKET_PATH + "/protocol3/"
    _, signed = signed_files(PROTOCOL_COMMIT, [prefix.rstrip("/")])
    need(B.inventory(folder) == {name.removeprefix(prefix): digest for name, digest in signed.items()},
         "signed protocol qualification records")
    need(read(folder / "inputs-before.json") == read(folder / "inputs-after.json") == protocol
         == B.inventory(folder / "protocol"), "qualified protocol bracket")
    specs = [("test_campaign", ROOT / "test_campaign.py", 15),
             ("test_xgmi_backing_budget_campaign", REPO / "benchmarks/runtime_gfx942/test_xgmi_backing_budget_campaign.py", 9),
             ("test_xgmi_peer_hot_results", REPO / "benchmarks/runtime_gfx942/test_xgmi_peer_hot_results.py", 7)]
    need({p.name for p in (folder / "commands").iterdir()} == {row[0] for row in specs}, "protocol qualification roster")
    previous = 0
    for name, script, count in specs:
        original_script = REPO / PACKET_PATH / script.name if name == "test_campaign" else script
        stage = folder / "commands" / name
        row = receipt(stage, ["/usr/bin/python3", "-I", "-B", str(original_script)], REPO, 180,
                      {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"})
        need(previous <= row["started_ns"], "protocol qualification ordering")
        previous = row["finished_ns"]
        need(re.search(rf"Ran {count} tests in [0-9.]+s\n\nOK\n\Z", (stage / "stderr").read_text()) is not None,
             "protocol qualification results")


def verify_local(run, binding, marker, sources, recovered=False):
    env = binding["local_build_environment"]
    need(env == dict(HOME="/home/harsh", USER="harsh", PATH="/home/harsh/.cargo/bin:/usr/bin:/bin", LANG="C", LC_ALL="C",
                     CARGO_TARGET_DIR=str(LOCAL / "target"), CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2",
                     CARGO_TERM_COLOR="never", RUSTUP_TOOLCHAIN="nightly-2026-04-03"), "cold local build environment")
    selectors = [value for value in C.K.SELECTORS if any(name == value or name.startswith(value + "/") for name in sources)]
    specs = [
        ("source-signature", [*C.K.GIT, "-c", "gpg.ssh.allowedSignersFile=" + C.C.SIGNERS, "verify-commit", C.SOURCE_COMMIT], 30),
        ("source-tree", [*C.K.GIT, "ls-tree", "-rz", C.SOURCE_COMMIT, "--", *C.K.SELECTORS], 120),
        ("source-archive", [*C.K.GIT, "archive", "--format=tar.gz", "--output=" + str(LOCAL / "build-source.tar.gz"), C.SOURCE_COMMIT, *selectors], 120),
        ("cpu-packet-tree", [*C.K.GIT, "ls-tree", "-rz", C.SOURCE_COMMIT, "--", C.CPU, C.PRIOR_CPU], 120),
        ("cpu-source-map", [*C.K.GIT, "show", C.SOURCE_COMMIT + ":" + C.CPU + "/raw/cpu2/inputs-before.json"], 30),
        ("cpu-replay", ["/usr/bin/python3", "-I", "-B", str(REPO / C.CPU / "verify.py")], 120),
    ]
    previous, names = 0, set()
    def check(name, command, seconds, cwd=REPO, environment=C.K.GIT_ENV, stdin=None, expected_exit=0):
        nonlocal previous
        names.add(name)
        row = receipt(run / "local" / name, command, cwd, seconds, environment, stdin, expected_exit)
        need(previous <= row["started_ns"], "local command order")
        previous = row["finished_ns"]
    for name, command, seconds in specs:
        check(name, command, seconds)
    commands = [("rustc", ["rustc", "-vV"], 30), ("cargo", ["cargo", "-V"], 30)]
    for verb in ("build", "test"):
        commands.append((verb + "-kfd", ["cargo", verb, "--offline", "--locked", "--release", "-p", "fe2o3-runtime",
                                        "--target", "x86_64-unknown-linux-musl", "--example", C.EXAMPLE], 1800))
    commands += [("after-rustc", ["rustc", "-vV"], 30), ("after-cargo", ["cargo", "-V"], 30)]
    for name, command, seconds in commands:
        check(name, command, seconds, LOCAL / "source", env)
    need("test result: ok. 13 passed; 0 failed; 0 ignored;" in (run / "local/test-kfd/stdout").read_text(), "release example test results")
    for tool in ("rustc", "cargo"):
        for stream in ("stdout", "stderr"):
            need((run / "local" / tool / stream).read_bytes() == (run / "local" / ("after-" + tool) / stream).read_bytes(), "local tool continuity")
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    def control(name, expected_exit=0):
        check(name, ["ssh", "-T", *C.C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])],
              120, environment=None, stdin=C.C.control_bytes(N.PREFIX), expected_exit=expected_exit)
    control("create")
    need(H.parse_json((run / "local/create/stdout").read_bytes()) == marker, "exact created marker")
    check("upload", ["scp", "-q", *C.C.SSH, "--", *(str(LOCAL / "payload" / name) for name in sorted(N.PAYLOAD)),
                     str(LOCAL / "binding.json"), "mi300x:" + marker["path"] + "/"], 120, environment=None)
    check("native", C.K.native_command(serialized), N.REMOTE_SECONDS, environment=None, expected_exit=255 if recovered else 0)
    control("inventory", expected_exit=255 if recovered else 0)
    if recovered:
        need((run / "local/inventory/stderr").read_bytes() == b"ssh: Could not resolve hostname sharkmi300x-1: Temporary failure in name resolution\r\n",
             "preserved DNS failure")
        need({p.parent.name for p in (run / "local").glob("*/receipt.json")} == names and len(names) == 16, "16 exact original local commands")
        return previous
    check("collect", ["scp", "-q", "-r", *C.C.SSH, "--", "mi300x:" + marker["path"] + "/results", str(LOCAL / "remote")], 120, environment=None)
    control("cleanup")
    control("absence")
    need(H.parse_json((run / "local/cleanup/stdout").read_bytes()) == {"removed": marker["path"]}, "exact remote removal")
    need(H.same_json(H.parse_json((run / "local/absence/stdout").read_bytes()), {"path_absent": True, "processes_absent": True}), "independent remote absence")
    need({p.parent.name for p in (run / "local").glob("*/receipt.json")} == names and len(names) == 19, "19 exact local commands")
    return previous


def verify_recovery(run, marker, previous):
    folder = run / "recovery1"
    need(H.same_json(read(folder / "result.json"), dict(collected=True, owned_cleanup=True, failures=[],
         campaign_accepted=False, performance_acceptance=False)), "explicit recovered-but-rejected classification")
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    for name in ("inventory", "collect", "cleanup", "absence"):
        if name == "collect":
            command = ["scp", "-q", "-r", *C.C.SSH, "--", "mi300x:" + marker["path"] + "/results", str(LOCAL / "recovery1/remote")]
            stdin = None
        else:
            command = ["ssh", "-T", *C.C.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])]
            stdin = C.C.control_bytes(N.PREFIX)
        row = receipt(folder / "commands" / name, command, REPO, 120, None, stdin)
        need(previous <= row["started_ns"], "recovery after original controller and in order")
        previous = row["finished_ns"]
    need({p.name for p in (folder / "commands").iterdir()} == {"inventory", "collect", "cleanup", "absence"}, "four recovery commands")
    need(H.parse_json((folder / "commands/cleanup/stdout").read_bytes()) == {"removed": marker["path"]}, "exact recovered remote removal")
    need(H.same_json(H.parse_json((folder / "commands/absence/stdout").read_bytes()),
                     {"path_absent": True, "processes_absent": True}), "independent recovered remote absence")


if __name__ == "__main__":
    main()
