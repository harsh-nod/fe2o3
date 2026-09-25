#!/usr/bin/env python3
"""Replay the two native producer witnesses and their source/process custody."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
import json
from pathlib import Path
import re
import shlex
import subprocess
import tarfile
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
ORIGINAL_REPO = Path("/home/harsh/.codex-tmp/fe2o3-r61-execution")
ORIGINAL_HERE = ORIGINAL_REPO / "docs/evidence/dev-native-producer-mi300x-2026-09-24"
PRIVATE = Path("/home/harsh/.codex-tmp/fe2o3-native-producer-20260924-BSu4wHKM")


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    need(path.is_file() and not path.is_symlink(), "ordinary artifact: " + str(path))
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    def unique(items):
        result = {}
        for key, value in items:
            need(key not in result, "duplicate artifact key")
            result[key] = value
        return result
    return json.loads(path.read_bytes(), object_pairs_hook=unique,
                      parse_constant=lambda _: need(False, "nonfinite artifact"))


def module(path, digest, name):
    raw = path.read_bytes()
    need(not path.is_symlink() and hashlib.sha256(raw).hexdigest() == digest, "captured replay input")
    value = ModuleType(name)
    value.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), value.__dict__)
    return value


def integer(value):
    return type(value) is int and value >= 0


def inventory(root):
    need(root.is_dir() and not root.is_symlink(), "ordinary inventory root")
    result = {}
    for path in sorted(root.rglob("*")):
        need(not path.is_symlink() and (path.is_dir() or path.is_file()), "ordinary inventory member")
        if path.is_file():
            result[path.relative_to(root).as_posix()] = sha(path)
    return result


def command(folder, argv, cwd, env, seconds, stdin=None, status=0):
    row = read(folder / "receipt.json")
    need(set(row) == {"command", "cwd", "started_ns", "timeout_seconds", "pid", "exit", "error", "group_absent",
                      "environment", "stdin_sha256", "finished_ns", "stdout_sha256", "stderr_sha256"}, "local receipt fields")
    need(row["command"] == argv and row["cwd"] == str(cwd) and row["environment"] == env
         and type(row["timeout_seconds"]) is int and row["timeout_seconds"] == seconds
         and row["stdin_sha256"] == (hashlib.sha256(stdin).hexdigest() if stdin is not None else None), "exact local command")
    need(type(row["exit"]) is int and row["exit"] == status and row["error"] is None and row["group_absent"] is True,
         "successful closed local command")
    need(integer(row["pid"]) and row["pid"] > 0 and integer(row["started_ns"]) and integer(row["finished_ns"])
         and 0 <= row["finished_ns"] - row["started_ns"] <= (seconds + 15) * 10**9, "bounded local command")
    for stream in ("stdout", "stderr"):
        need(sha(folder / stream) == row[stream + "_sha256"], "local output identity")
    return row


def signed_source(build, p, m):
    signers = build / "allowed-signers"
    if not signers.exists():
        signers = Path(m.SIGNERS)
    need(sha(signers) == m.SIGNERS_SHA, "pinned expected source signer")
    def git(*args, stdin=None):
        result = subprocess.run([*m.GIT, *args], cwd=REPO, env=m.GIT_ENV, input=stdin, capture_output=True, timeout=120, check=True)
        return result.stdout
    git("-c", "gpg.ssh.allowedSignersFile=" + str(signers), "verify-commit", p.COMMIT)
    raw = git("ls-tree", "-rz", p.COMMIT, "--", *m.SELECTORS)
    need(raw == (build / "commands/source-tree/stdout").read_bytes(), "signed Git source tree")
    entries, modes = [], {}
    for record in raw.rstrip(b"\0").split(b"\0"):
        header, name = record.split(b"\t")
        mode, kind, oid = header.decode().split()
        need(mode in ("100644", "100755") and kind == "blob", "ordinary signed source")
        entries.append((name.decode(), oid))
        modes[name.decode()] = mode
    data = git("cat-file", "--batch", stdin="".join(oid + "\n" for _, oid in entries).encode())
    offset, files = 0, {}
    for name, oid in entries:
        end = data.index(b"\n", offset)
        observed, kind, size = data[offset:end].decode().split()
        size = int(size)
        need(observed == oid and kind == "blob" and size >= 0 and name not in files, "exact batched source object")
        start, offset = end + 1, end + 1 + size
        content = data[start:offset]
        need(len(content) == size and data[offset:offset + 1] == b"\n", "complete Git blob")
        need(hashlib.sha1(b"blob " + str(size).encode() + b"\0" + content).hexdigest() == oid, "source Git object identity")
        offset += 1
        files[name] = hashlib.sha256(content).hexdigest()
    need(offset == len(data), "no extra source objects")
    need(files == read(build / "source.json") == read(build / "source-after.json"), "entire signed source identity map")
    archive_path = build / "build-source.tar.gz"
    need(read(build / "source-archive.json") == {"sha256": sha(archive_path), "bytes": archive_path.stat().st_size}, "retained build archive")
    with tarfile.open(archive_path, "r:gz") as archive:
        observed = {}
        directories = {str(parent) for name in files for parent in Path(name).parents if str(parent) != "."}
        seen_directories = set()
        for member in archive.getmembers():
            need(member.isfile() or member.isdir(), "ordinary retained archive members")
            if member.isdir():
                name = member.name.rstrip("/")
                need(name in directories and name not in seen_directories, "exact archive directory")
                seen_directories.add(name)
                continue
            need(member.name in files and member.name not in observed, "exact retained source member")
            need(bool(member.mode & 0o111) == (modes[member.name] == "100755"), "signed source executable mode")
            content = archive.extractfile(member).read()
            observed[member.name] = hashlib.sha256(content).hexdigest()
        need(observed == files, "retained source archive equals signed Git tree")
    if (build / "source").exists():
        observed = {}
        for path in (build / "source").rglob("*"):
            need(not path.is_symlink() and (path.is_file() or path.is_dir()), "ordinary extracted source nodes")
            if path.is_file():
                name = path.relative_to(build / "source").as_posix()
                need(name in modes and bool(path.stat().st_mode & 0o111) == (modes[name] == "100755"), "extracted source mode")
                observed[name] = sha(path)
        need(observed == files, "extracted source equals signed tree")
    for name, path in (("cpu-source", m.CPU + "/raw/cpu4/inputs-before.json"),
                       ("cpu-roster", m.CPU + "/raw/cpu4/commands/musl-runtime/stdout")):
        need(git("show", p.COMMIT + ":" + path) == (build / "commands" / name / "stdout").read_bytes(), "signed prior CPU evidence")
    return files


def verify_build(build, campaign, p, m):
    original = PRIVATE / "build"
    value = read(build / "build.json")
    need(value["commit"] == p.COMMIT and value["cpu_source_inputs"] == 3961, "qualified current build")
    need(sha(build / "runtime-tests") == value["binary_sha256"]
         and (build / "runtime-tests").read_bytes()[:4] == b"\x7fELF", "retained actual CPU-tested build ELF")
    files = signed_source(build, p, m)
    need(files == value["source_files"], "build source identity")
    roots = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "benchmarks/runtime_gfx942"]
    inputs = {name: digest for name, digest in files.items()
              if any(name == root or name.startswith(root + "/") for root in roots)
              and (name.endswith((".rs", ".toml", ".lock", ".json", ".py", ".cpp", ".hpp")) or "/fixtures/" in name)}
    need(read(build / "commands/cpu-source/stdout")["source"] == inputs and len(inputs) == 3961, "complete prior CPU source map")
    need(read(build / "runner-before.json") == read(build / "runner-after.json") == {"sha256": sha(build / "prepare.py")}
         == {"sha256": sha(campaign / "protocol/prepare.py")}, "frozen build runner")
    env = {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin", "LANG": "C", "LC_ALL": "C",
           "CARGO_TARGET_DIR": str(original / "target"), "CARGO_INCREMENTAL": "0", "CARGO_BUILD_JOBS": "2",
           "CARGO_TERM_COLOR": "never", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03", "CARGO_PROFILE_TEST_OPT_LEVEL": "1",
           "CARGO_PROFILE_TEST_DEBUG": "0", "CARGO_PROFILE_DEV_DEBUG": "0"}
    need(env == value["environment"], "exact cold build environment")
    selectors = [root for root in m.SELECTORS if any(name == root or name.startswith(root + "/") for name in files)]
    rows = [json.loads(line) for line in (build / "commands/build/stdout").read_bytes().splitlines()]
    artifacts = [row for row in rows if row.get("reason") == "compiler-artifact"
                 and row.get("target", {}).get("name") == "fe2o3_runtime" and row.get("target", {}).get("kind") == ["lib"]
                 and row.get("profile", {}).get("test") is True and row.get("executable")]
    need(len(artifacts) == 1, "one Cargo-reported test executable")
    binary = Path(artifacts[0]["executable"])
    need(binary.parent == original / "target/x86_64-unknown-linux-musl/debug/deps", "cold-target artifact")
    specs = [
        ("signature", [*m.GIT, "-c", "gpg.ssh.allowedSignersFile=" + m.SIGNERS, "verify-commit", p.COMMIT], REPO, m.GIT_ENV, 30),
        ("source-tree", [*m.GIT, "ls-tree", "-rz", p.COMMIT, "--", *m.SELECTORS], REPO, m.GIT_ENV, 120),
        ("source-archive", [*m.GIT, "archive", "--format=tar.gz", "--output=" + str(original / "build-source.tar.gz"), p.COMMIT, *selectors], REPO, m.GIT_ENV, 120),
        ("cpu-source", [*m.GIT, "show", p.COMMIT + ":" + m.CPU + "/raw/cpu4/inputs-before.json"], REPO, m.GIT_ENV, 30),
        ("cpu-roster", [*m.GIT, "show", p.COMMIT + ":" + m.CPU + "/raw/cpu4/commands/musl-runtime/stdout"], REPO, m.GIT_ENV, 30),
        ("rustc", ["rustc", "-vV"], original / "source", env, 30),
        ("cargo", ["cargo", "-V"], original / "source", env, 30),
        ("build", ["cargo", "test", "--offline", "--locked", "-p", "fe2o3-runtime", "--all-features", "--lib", "--target",
                   "x86_64-unknown-linux-musl", "--no-run", "--message-format=json"], original / "source", env, 7200),
        ("runtime-cpu", [str(binary), "--test-threads=4", "--color=never"], original / "source", env, 7200),
        ("after-rustc", ["rustc", "-vV"], original / "source", env, 30),
        ("after-cargo", ["cargo", "-V"], original / "source", env, 30),
    ]
    need({path.name for path in (build / "commands").iterdir()} == {row[0] for row in specs}, "exact build command roster")
    previous = 0
    for name, argv, cwd, environment, seconds in specs:
        row = command(build / "commands" / name, argv, ORIGINAL_REPO if cwd == REPO else cwd, environment, seconds)
        need(previous <= row["started_ns"], "ordered build commands")
        previous = row["finished_ns"]
    for tool in ("rustc", "cargo"):
        for stream in ("stdout", "stderr"):
            need(sha(build / "commands" / tool / stream) == sha(build / "commands" / ("after-" + tool) / stream), "tool continuity")
    need(m.roster((build / "commands/runtime-cpu/stdout").read_text()) == value["runtime_roster"]
         == m.roster((build / "commands/cpu-roster/stdout").read_text()), "exact actual-ELF CPU roster")
    need({name for name, outcome in value["runtime_roster"].items() if outcome == "ignored"} == set(p.TESTS.values()), "exact native roster")
    return value


def native_command(folder, argv, root, env, bound, status=0):
    row = read(folder / "record.json")
    need(set(row) == {"name", "command", "cwd", "environment", "started", "spawned", "status", "error", "process_group",
                      "group_absent", "t0", "outer_bound_seconds", "finished", "stdout_sha256", "stderr_sha256"}, "native receipt fields")
    need(row["name"] == folder.name and row["command"] == argv and row["cwd"] == str(root) and row["environment"] == env
         and type(row["outer_bound_seconds"]) is int and row["outer_bound_seconds"] == bound, "exact native command")
    need(type(row["status"]) is int and row["status"] == status and row["error"] is None and row["group_absent"] is True
         and integer(row["process_group"]) and row["process_group"] > 0, "closed successful native command")
    stamps = [row[key] for key in ("started", "spawned", "t0", "finished")]
    need(all(set(value) == {"realtime_ns", "monotonic_ns"} and all(integer(item) for item in value.values()) for value in stamps),
         "complete native command stamps")
    need(all(left["monotonic_ns"] <= right["monotonic_ns"] for left, right in zip(stamps, stamps[1:])), "native command clock order")
    need(stamps[-1]["monotonic_ns"] - stamps[0]["monotonic_ns"] <= (bound + 15) * 10**9, "native command time bound")
    for stream in ("stdout", "stderr"):
        need(sha(folder / (stream + ".log")) == row[stream + "_sha256"], "native output identity")
    return row


def busy_refusal(data, p, observer, t0):
    need(type(data) is bytes and len(data) <= 4 * 1024 * 1024 and data.endswith(b"\n")
         and b"\r" not in data and b"\0" not in data, "bounded complete refused endpoint")
    lines = data.decode("ascii").splitlines()
    need(len(lines) == 2, "complete refused endpoint")
    value, complete = map(p.parse, lines)
    need(p.same(complete, {"schema": "fe2o3.copy-host-observation.v1", "record": "complete", "observations": 1,
                          "refused": 1, "all_endpoints_admitted": False, "performance_accepted": False}), "refusal completion")
    clocks, snapshots, captures = iter((value["started"], value["finished"])), iter(value["sysfs"]), iter((value["status"], value["pids"]))
    def capture_sysfs(root, bdf):
        need(str(root / bdf) == "/sys/bus/pci/devices/" + p.BDF, "original sysfs request")
        snapshot = next(snapshots)
        need(set(snapshot) == {"started", "finished", "path", "values", "errors"}
             and snapshot["path"] == str(root / bdf) and snapshot["errors"] == {}
             and set(snapshot["values"]) == set(observer.METRICS), "complete original sysfs snapshot")
        return snapshot
    def capture(command):
        row = next(captures)
        need(set(row) == {"command", "started", "finished", "exit", "error", "stdout", "stderr"}
             and row["command"] == command and type(row["exit"]) is int and row["exit"] == 0
             and row["error"] is None and row["stderr"] == "", "complete original SMI capture")
        return row
    old_stamp, old_capture = observer.stamp, observer.capture_sysfs
    try:
        observer.stamp = lambda: next(clocks)
        observer.capture_sysfs = capture_sysfs
        reconstructed = observer.observe(p.GPU, p.BDF, p.UID, Path("/opt/rocm/bin/rocm-smi"), run=capture)
    finally:
        observer.stamp, observer.capture_sysfs = old_stamp, old_capture
    need(next(clocks, None) is None and next(snapshots, None) is None and next(captures, None) is None, "exact exhausted capture replay")
    need(type(value["index"]) is int and value["index"] == 0
         and p.same(reconstructed, {key: item for key, item in value.items() if key != "index"}), "independently replayed original refusal")
    need(value["endpoint_admitted"] is False and value["reasons"] == ["sysfs-before-busy"] and value["selected_pids"] == []
         and [row["values"]["gpu_busy_percent"] for row in value["sysfs"]] == ["1", "0", "0"], "exact one-percent-only refusal")
    timeline = [value["started"]]
    for row in (value["sysfs"][0], value["status"], value["sysfs"][1], value["pids"], value["sysfs"][2]):
        timeline.extend((row["started"], row["finished"]))
    timeline.append(value["finished"])
    ticks = [p.stamp(row) for row in timeline]
    need(ticks == sorted(ticks) and 0 <= ticks[0] - t0 <= 10**9, "original refusal clock and immediate window")
    for row in value["sysfs"]:
        need(row["values"]["mem_busy_percent"] == "0", "memory engine idle even in refused observation")
    return value


def verify_native(campaign, p, n, b, rejected):
    binding, marker = read(campaign / "binding.json"), read(campaign / "owner.json")
    need(set(binding) == {"commit", "payload", "order", "outer_seconds", "protocol", "build_sha256", "source_files", "device",
                          "target", "features", "build_environment"}
         and set(marker) == {"path", "commit", "binding_sha256"}, "exact binding and owner fields")
    root = Path(marker["path"])
    need(re.fullmatch(re.escape(p.PREFIX) + r"[0-9a-f]{16}", str(root)) and marker["commit"] == p.COMMIT
         and marker["binding_sha256"] == sha(campaign / "binding.json"), "bound exact remote owner")
    remote = campaign / "remote"
    need(b.inventory(remote) == read(campaign / "remote-inventory.json"), "byte-exact remote inventory")
    need(b.inventory(remote / "artifacts") == binding["payload"], "retained whole executable and helper bytes")
    need(inventory(campaign / "payload") == binding["payload"], "uploaded payload continuity")
    pins = {"base.py": p.BASE_SHA, "recorder.py": p.RECORDER_SHA,
            "observer.py": p.OBSERVER_SHA, "topology.py": n.TOPOLOGY_SHA,
            **{name: sha(campaign / "protocol" / name) for name in ("native.py", "protocol.py")}}
    need(all(binding["payload"][name] == digest for name, digest in pins.items()), "frozen executable payload pins")
    need(binding["commit"] == p.COMMIT, "native source commit")
    need(set(binding["payload"]) == n.PAYLOAD and binding["order"] == [list(row) for row in p.CASE_NAMES]
         and type(binding["outer_seconds"]) is int and binding["outer_seconds"] == n.REMOTE_SECONDS, "fixed complete native matrix")
    observer = p.load_module(remote / "artifacts/observer.py", p.OBSERVER_SHA, "matrix_replay_observer")
    ordered = []
    def native(name, argv, env, bound, status=0):
        row = native_command(remote / name, argv, root, env, bound, status)
        if ordered:
            need(ordered[-1]["finished"]["monotonic_ns"] <= row["started"]["monotonic_ns"], "serialized native commands")
        ordered.append(row)
        return row
    def observe(name, t0=None, offset=None):
        refused = rejected and name == "06-primary-panic-immediate"
        row = native(name, ["/usr/bin/python3", "-I", "-B", str(root / "observer.py"), "--gpu-index", str(p.GPU),
                            "--pci-bdf", p.BDF, "--unique-id", p.UID], p.environment(root), n.OBSERVE_SECONDS, 1 if refused else 0)
        need((remote / name / "stderr.log").read_bytes() == b"", "empty observer stderr")
        data = (remote / name / "stdout.log").read_bytes()
        value = busy_refusal(data, p, observer, t0) if refused else p.endpoint(data, observer, t0=t0, offset=offset)
        need(row["spawned"]["monotonic_ns"] <= p.stamp(value["started"]) <= p.stamp(value["finished"])
             <= row["t0"]["monotonic_ns"], "observer enclosed by receipt")
        return value
    native("topology", ["/usr/bin/python3", "-I", "-B", str(root / "topology.py"), "topology", "--gpu-index", str(p.GPU),
                        "--pci-bdf", p.BDF, "--unique-id", p.UID], p.environment(root), 100)
    need((remote / "topology/stderr.log").read_bytes() == b"", "empty topology stderr")
    n.topology((remote / "topology/stdout.log").read_text(), p)
    native("placement", ["/usr/bin/numactl", "--physcpubind=0-47", "--membind=0", "/usr/bin/numactl", "--show"], p.environment(root), 30)
    need((remote / "placement/stderr.log").read_bytes() == b"", "empty placement stderr")
    n.placement((remote / "placement/stdout.log").read_text(), p)
    cases = []
    executed = p.CASE_NAMES[:6] if rejected else p.CASE_NAMES
    for ordinal, (case, _) in enumerate(executed, 1):
        name = f"{ordinal:02}-{case}"
        resources = read(remote / (name + "-resources.json"))
        need(set(resources) == {"global", "node0", "disk_free"} and type(resources["global"]) is str
             and type(resources["node0"]) is str, "complete resource observation")
        for key, label in (("global", "MemAvailable"), ("node0", "MemFree")):
            values = re.findall(r"(?:^| )" + label + r":\s+([0-9]+) kB$", resources[key], re.MULTILINE)
            need(len(values) == 1 and int(values[0]) * 1024 >= 4 * 1024**3, "memory headroom")
        need(integer(resources["disk_free"]) and resources["disk_free"] >= 1024**3, "disk headroom")
        before = observe(name + "-before")
        row = native(name + "-test", p.command(root, case), p.environment(root, case), n.TEST_SECONDS)
        need(0 <= row["spawned"]["monotonic_ns"] - p.stamp(before["finished"]) <= 10**9, "fresh launch window")
        t0 = row["t0"]["monotonic_ns"]
        observe(name + "-immediate", t0, 0)
        observe(name + "-delayed", t0, 20)
        result = p.transcript(case, (remote / (name + "-test/stdout.log")).read_text(), (remote / (name + "-test/stderr.log")).read_text())
        refused = rejected and case == "primary-panic"
        cases.append({"case": case, "ordinal": ordinal,
                      "failures": ["immediate: ValueError('successful strict observer command')"] if refused else [], "transcript": result,
                      "post_observations": {"immediate": "refused" if refused else "strict_pass", "delayed": "strict_pass"}})
    count = 2 + 4 * len(executed)
    need(len(ordered) == count and len({row["process_group"] for row in ordered}) == count, "distinct closed command groups")
    for index, row in enumerate(ordered, 1):
        active = read(remote / f"active-{index:03}/receipt.json")
        need(set(active) == {"pid", "command", "started", "purpose"} and type(active["pid"]) is int and active["pid"] == row["process_group"]
             and active["command"] == row["command"] and active["purpose"] == "record group before unblocking managed signals",
             "active-to-terminal command bijection")
        need(set(active["started"]) == {"realtime_ns", "monotonic_ns"}
             and all(integer(value) for value in active["started"].values()), "active custody timestamp")
        need(row["started"]["monotonic_ns"] <= active["started"]["monotonic_ns"] <= row["spawned"]["monotonic_ns"], "immediate active custody")
    expected = {"artifacts", "outer", "finished.json"} | {row["name"] for row in ordered}
    expected |= {f"active-{index:03}" for index in range(1, count + 1)}
    expected |= {f"{index:02}-{case}-resources.json" for index, (case, _) in enumerate(executed, 1)}
    need({path.name for path in remote.iterdir()} == expected, "exact remote artifact roster")
    failures = [repr(ValueError("case rejected; no subsequent case: " + repr(cases[-1])))] if rejected else []
    need(p.same(read(remote / "finished.json"), {"commit": p.COMMIT, "cases": cases, "failures": failures, "spawn_failures": [],
                                               "complete_matrix": not rejected, "exclusive_reservation": False,
                                               "performance_acceptance": False, "formal_refinement": False}), "exact native matrix disposition")
    outer = read(remote / "outer/receipt.json")
    need(set(outer) == {"pid", "controller_pid", "purpose"} and integer(outer["pid"]) and outer["pid"] > 0
         and integer(outer["controller_pid"]) and outer["controller_pid"] > 0
         and outer["purpose"] == "remote timeout process group", "outer process custody")
    return binding, marker


def verify_local(campaign, binding, marker, p, m, n, c, rejected):
    original = PRIVATE / campaign.name
    serialized = json.dumps(marker, sort_keys=True, separators=(",", ":"))
    wire = (campaign / "control.py").read_bytes()
    need(wire == c.control_bytes(p, campaign / "remote/artifacts/base.py"), "exact control helper bytes")
    remote = ["/usr/bin/timeout", "--signal=TERM", "--kill-after=15s", str(n.REMOTE_SECONDS) + "s",
              "/usr/bin/python3", "-I", "-B", "-c", c.BOOTSTRAP, serialized]
    specs = [
        ("protocol-tests", ["/usr/bin/python3", "-I", "-B", str(ORIGINAL_HERE / "test_protocol.py")], 120, None),
        ("create", ["ssh", "-T", *c.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", "create", serialized])], 120, wire),
        ("upload", ["scp", "-q", *c.SSH, "--", *(str(original / "payload" / name) for name in sorted(n.PAYLOAD)),
                    str(original / "binding.json"), "mi300x:" + marker["path"] + "/"], 120, None),
        ("native", ["ssh", "-T", *c.SSH, "mi300x", shlex.join(remote)], n.REMOTE_SECONDS + 120, None),
        ("inventory", ["ssh", "-T", *c.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", "inventory", serialized])], 120, wire),
        ("collect", ["scp", "-q", "-r", *c.SSH, "--", "mi300x:" + marker["path"] + "/results", str(original / "remote")], 180, None),
        *[(name, ["ssh", "-T", *c.SSH, "mi300x", shlex.join(["/usr/bin/python3", "-I", "-B", "-", name, serialized])], 120, wire)
          for name in ("cleanup", "absence")],
    ]
    need({path.name for path in (campaign / "commands").iterdir()} == {row[0] for row in specs}, "exact controller command roster")
    previous = 0
    for name, argv, bound, stdin in specs:
        row = command(campaign / "commands" / name, argv, ORIGINAL_REPO, m.GIT_ENV, bound, stdin,
                      1 if rejected and name == "native" else 0)
        need(previous <= row["started_ns"], "collection precedes cleanup and independent absence")
        previous = row["finished_ns"]
    need(read(campaign / "commands/create/stdout") == marker, "exact remote create")
    need(read(campaign / "commands/inventory/stdout") == read(campaign / "remote-inventory.json"), "exact collected inventory")
    need(read(campaign / "commands/cleanup/stdout") == {"removed": marker["path"]}, "owned cleanup")
    need(p.same(read(campaign / "commands/absence/stdout"), {"path_absent": True, "processes_absent": True}), "independent absence")
    need(p.same(read(campaign / "collection.json"), {"failures": ["RuntimeError('native failed')"] if rejected else [],
                                                    "owned_cleanup": True, "collected": True, "exclusive_reservation": False,
                                                    "performance_acceptance": False}), "exact collection disposition")


def verify_campaign(root, name, rejected):
    build, campaign = root / "build", root / name
    before = read(campaign / "protocol-before.json")
    need(before == read(campaign / "protocol-after.json"), "unchanged campaign protocol")
    for name, digest in before.items():
        need(sha(campaign / "protocol" / name) == digest, "frozen campaign protocol")
    p, m, n, c = [module(campaign / "protocol" / (name + ".py"), before[name + ".py"], "matrix_replay_" + name)
                   for name in ("protocol", "prepare", "native", "campaign")]
    need(set(before) == set(c.PROTOCOL), "complete protocol roster")
    b = p.load_module(campaign / "remote/artifacts/base.py", p.BASE_SHA, "matrix_replay_inventory")
    qualified = verify_build(build, campaign, p, m)
    roster = list(p.CASE_NAMES)
    if not rejected:
        need(n.CASE_START == 22 and n.EXPECTED_CASES == 2, "separate producer-only suffix")
        p.CASE_NAMES = n.selected_cases(p)
    binding, marker = verify_native(campaign, p, n, b, rejected)
    need(binding["build_sha256"] == sha(build / "build.json") and binding["source_files"] == qualified["source_files"]
         and binding["build_environment"] == qualified["environment"] and binding["payload"]["runtime-tests"] == qualified["binary_sha256"],
         "exact signed-source built/executed/retained ELF association")
    need(binding["protocol"] == before and binding["device"] == [p.GPU, p.BDF, p.UID]
         and binding["target"] == "x86_64-unknown-linux-musl" and binding["features"] == "all-features", "native build profile")
    verify_local(campaign, binding, marker, p, m, n, c, rejected)
    return {"protocol": {name: before[name] for name in ("protocol.py", "prepare.py")},
            "roster": roster, "tests": p.TESTS,
            "identity": [getattr(p, name) for name in ("COMMIT", "GPU", "BDF", "UID", "PREFIX", "BASE_SHA", "RECORDER_SHA", "OBSERVER_SHA")]
                        + [n.TOPOLOGY_SHA],
            "build": {name: binding[name] for name in ("build_sha256", "source_files", "target", "features", "build_environment")},
            "payload": {name: digest for name, digest in binding["payload"].items() if name != "native.py"}}


def verify(root):
    result = verify_campaign(root, "campaign1", False)
    need(result["tests"] == dict(result["roster"]), "case roster matches test-name map")
    need(len(result["roster"]) == 24 and len(set(name for name, _ in result["roster"])) == 24
         and len(set(test for _, test in result["roster"])) == 22, "complete ignored-test name calibration")
    need([name for name, _ in result["roster"][-2:]] == ["queued-producer", "published-producer"],
         "only producer witnesses selected")
    ordered = [root / "build/commands/after-cargo", root / "campaign1/commands/protocol-tests",
               root / "campaign1/commands/absence"]
    receipts = [read(path / "receipt.json") for path in ordered]
    need(all(left["finished_ns"] <= right["started_ns"] for left, right in zip(receipts, receipts[1:])),
         "cold build and guarded producer campaign are sequential")
    return {"native_commands": 2, "harness_passes": 2, "endpoint_observations": 6, "strict_admitted": 6, "remote_commands": 10,
            "local_build_commands": 11, "local_campaign_commands": 8,
            "cpu_tests": 1400, "cpu_hardware_ignores": 22,
            "whole_elf_retained": True, "record_replay": "pass", "producer_witnesses_accepted": True,
            "refused_observations": 0, "formal_refinement": False, "performance_acceptance": False}


def verify_retention(root):
    need(inventory(root) == read(root.parent / "artifacts.json"), "complete retained artifact inventory")
    row = read(root / "retention.json")
    need(set(row) == {"source", "excluded", "retained_files", "retained_sha256", "allocated_bytes", "absent", "replay",
                      "started_ns", "finished_ns"}, "retention fields")
    data = inventory(root)
    del data["retention.json"]
    del data["retention-before.json"]
    need(row["source"] == str(PRIVATE) and row["excluded"] == ["build/source", "build/target"]
         and type(row["retained_files"]) is int and row["retained_files"] == len(data)
         and row["retained_sha256"] == hashlib.sha256(json.dumps(data, sort_keys=True).encode()).hexdigest(),
         "complete retained records before cleanup")
    need(row["absent"] is True and integer(row["allocated_bytes"]) and row["allocated_bytes"] > 0
         and integer(row["started_ns"]) and integer(row["finished_ns"]) and row["started_ns"] <= row["finished_ns"],
         "owned local cleanup receipt")
    before = read(root / "retention-before.json")
    need(before == {**row, "absent": False, "finished_ns": None}, "immutable cleanup intent matches final result")
    need(sha(root / "build/allowed-signers") == "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b",
         "retained pinned public signer")
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=HERE / "raw")
    args = parser.parse_args()
    result = verify(args.root)
    if args.root == HERE / "raw":
        need(read(args.root / "retention.json")["replay"] == result, "retained semantic replay result")
        verify_retention(args.root)
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
