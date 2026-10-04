"""Exact-source ordinary diagnostic; no protected, native, or publication authority."""
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import shutil
import signal
import stat
import subprocess
import tempfile
import time
import tomllib

HEAD = "28d2596a93e076fb22a1f604bb9b989d95df816b"
TREE = "961039c9e57293b1381a39aa98812e5bbdae79c4"
ROOT = Path(os.environ["GITHUB_WORKSPACE"]).resolve(strict=True)
SOURCE = ROOT / "source"
CONTROL = ROOT / "control"
TEMP = Path(os.environ["RUNNER_TEMP"]).resolve(strict=True)
LOGS = TEMP / "issue271-predicated-continuation-v90-r4-logs"
ROSTER = CONTROL / ".github/diagnostics/issue271-proof-roster-v78.json"
os.umask(0o077)
assert TEMP != ROOT and ROOT not in TEMP.parents
LOGS.mkdir(mode=0o700)
LANE = Path(tempfile.mkdtemp(prefix="issue271-predicated-continuation-v90-r4-", dir=TEMP))
IDENTITY = (LANE.stat().st_dev, LANE.stat().st_ino, LANE.stat().st_uid)
INTERRUPTED = []
ACTIVE_CHILD = None
for signum in [signal.SIGTERM, signal.SIGINT, signal.SIGHUP]:
    signal.signal(signum, lambda received, _frame: INTERRUPTED.append(received))


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(name, value):
    with (LOGS / name).open("x", encoding="ascii") as output:
        json.dump(value, output, indent=2)
        output.write("\n")


def git(path, *args):
    return subprocess.check_output(["git", *args], cwd=path, timeout=30,
                                   env={**os.environ, "GIT_OPTIONAL_LOCKS": "0"})


def source_pin():
    assert git(SOURCE, "rev-parse", "HEAD").decode().strip() == HEAD
    assert git(SOURCE, "rev-parse", "HEAD^{tree}").decode().strip() == TREE
    assert git(SOURCE, "status", "--porcelain=v1", "--untracked-files=all") == b""
    assert git(CONTROL, "rev-parse", "HEAD").decode().strip() == os.environ["EXPECTED_CONTROL_HEAD"]
    assert git(CONTROL, "status", "--porcelain=v1", "--untracked-files=all") == b""
    return {"head": HEAD, "tree": TREE, "lock_sha256": digest(SOURCE / "Cargo.lock"),
            "control_head": os.environ["EXPECTED_CONTROL_HEAD"], "roster_sha256": digest(ROSTER),
            "toolchain_sha256": digest(SOURCE / "rust-toolchain.toml"), "clean": True}


def group_tasks(pgid):
    tasks = []
    for item in Path("/proc").iterdir():
        if not item.name.isdigit():
            continue
        try:
            fields = (item / "stat").read_text().rsplit(")", 1)[1].split()
            if int(fields[2]) == pgid and fields[0] != "Z":
                tasks.append(int(item.name))
        except (FileNotFoundError, ProcessLookupError):
            pass
        except PermissionError:
            if item.exists() and item.stat().st_uid == os.getuid():
                raise
    return tasks


def select_artifact(log, group):
    records = [json.loads(line) for line in log.splitlines() if line.startswith("{")]
    finish = [row for row in records if row.get("reason") == "build-finished"]
    assert len(finish) == 1 and finish[0]["success"] is True
    rows = [row for row in records if row.get("reason") == "compiler-artifact"
            and row.get("executable") is not None]
    assert len(rows) == 1
    row = rows[0]
    assert row["target"]["name"] == group["target"]
    assert row["target"]["kind"] == ["lib"] and row["profile"]["test"] is True
    assert Path(row["manifest_path"]).resolve(strict=True) == SOURCE / group["manifest"]
    assert Path(row["target"]["src_path"]).resolve(strict=True) == SOURCE / group["library"]
    executable = Path(row["executable"])
    assert executable.resolve(strict=True) == executable
    assert executable.parent == LANE / "target/debug/deps"
    return executable


def artifact_pin(path):
    before = path.lstat()
    assert stat.S_ISREG(before.st_mode) and before.st_uid == os.getuid()
    assert 0 < before.st_size <= 1024**3
    sha = digest(path)
    after = path.lstat()
    identity = lambda row: (row.st_size, row.st_mtime_ns, row.st_dev, row.st_ino, row.st_mode, row.st_uid)
    assert identity(before) == identity(after)
    return {"path": str(path), "sha256": sha, "identity": identity(after)}


def parse_discovery(log, minimum_total, required):
    rows = [json.loads(line) for line in log.splitlines() if line.strip()]
    assert rows[0] == {"type": "suite", "event": "discovery"}
    tests = rows[1:-1]
    expected_total = len(tests)
    assert 0 < minimum_total <= expected_total <= 10000
    assert all(row["type"] == "test" and row["event"] == "discovered"
               and type(row["ignore"]) is bool and isinstance(row["name"], str)
               and re.fullmatch(r"\S+", row["name"]) for row in tests)
    assert len({row["name"] for row in tests}) == expected_total
    ignored = sorted(row["name"] for row in tests if row["ignore"])
    normal = sorted(row["name"] for row in tests if not row["ignore"])
    assert rows[-1] == {"type": "suite", "event": "completed", "tests": expected_total,
                        "benchmarks": 0, "total": expected_total, "ignored": len(ignored)}
    assert normal and all(sum(name.rsplit("::", 1)[-1] == leaf for name in normal) == 1 for leaf in required)
    return {"normal": normal, "ignored": ignored, "total": expected_total,
            "ignored_bodies_executed": False}


def parse_coverage(log, discovered, filtered=0):
    assert type(filtered) is int and 0 <= filtered <= 10000
    assert not re.search(r"^failures:$", log, re.M)
    rows = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)(?:,.*)?$", log, re.M)
    expected = {name: "ok" for name in discovered["normal"]}
    expected.update({name: "ignored" for name in discovered["ignored"]})
    assert len(rows) == len(expected) == discovered["total"]
    assert len({name for name, _ in rows}) == len(rows) and dict(rows) == expected
    assert re.findall(r"^running (\d+) tests?$", log, re.M) == [str(len(expected))]
    assert len(re.findall(r"^test result:", log, re.M)) == 1
    summary = re.search(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored; 0 measured; (\d+) filtered out; finished in [0-9.]+s\s*\Z", log)
    assert summary and (int(summary.group(1)), int(summary.group(2)), int(summary.group(3))) == (len(discovered["normal"]), len(discovered["ignored"]), filtered)
    return {"passed": len(discovered["normal"]), "failed": 0, "ignored": len(discovered["ignored"]),
            "filtered": filtered, "rows": rows, "ignored_owning_parents_executed": False}


def focused_roster(discovered, required):
    assert required and len(required) == len(set(required))
    selected = []
    for leaf in required:
        matches = [name for name in discovered["normal"] if name.rsplit("::", 1)[-1] == leaf]
        assert len(matches) == 1
        selected.append(matches[0])
    assert len(selected) == len(set(selected)) and not set(selected).intersection(discovered["ignored"])
    return {"normal": sorted(selected), "ignored": [], "total": len(selected),
            "ignored_bodies_executed": False}


def run(name, args, environment, seconds, limit):
    global ACTIVE_CHILD
    assert not INTERRUPTED
    child, selector = None, None
    started = time.time()
    deadline = time.monotonic() + seconds
    reason, terminal, status, survivors = None, None, None, None
    eof, total = False, 0
    cleanup_errors = []

    def kill_group():
        try:
            os.killpg(child.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except OSError as error:
            cleanup_errors.append(repr(error))

    try:
        child = subprocess.Popen(args, cwd=SOURCE, env=environment, stdout=subprocess.PIPE,
                                 stderr=subprocess.STDOUT, start_new_session=True)
        ACTIVE_CHILD = child
        selector = selectors.DefaultSelector()
        selector.register(child.stdout, selectors.EVENT_READ)
        save(name + "-started.json", {"command": args, "environment": environment,
                                    "pid": child.pid, "pgid": child.pid, "started": started,
                                    "deadline_seconds": seconds, "log_limit_bytes": limit})
        with (LOGS / (name + ".log")).open("xb") as output:
            while not (terminal is not None and eof):
                if INTERRUPTED or time.monotonic() >= deadline:
                    reason = "interrupted" if INTERRUPTED else "wall-clock deadline"
                    break
                for key, _ in selector.select(0.25):
                    data = os.read(key.fileobj.fileno(), 65536)
                    if not data:
                        selector.unregister(key.fileobj)
                        eof = True
                        continue
                    retained = data[:max(0, limit - total)]
                    output.write(retained)
                    output.flush()
                    total += len(retained)
                    if len(retained) != len(data):
                        reason = "bounded log limit"
                        break
                if reason:
                    break
                terminal = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
                if terminal is not None and group_tasks(child.pid):
                    reason = "owned group survived leader"
                    break
                if terminal is not None and not eof:
                    deadline = min(deadline, time.monotonic() + 10)
            output.flush()
            os.fsync(output.fileno())
    except BaseException as error:
        reason = repr(error)
    finally:
        if child is not None:
            if reason:
                kill_group()
            cleanup_deadline = time.monotonic() + 10
            while time.monotonic() < cleanup_deadline:
                try:
                    terminal = os.waitid(os.P_PID, child.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT)
                    survivors = group_tasks(child.pid)
                    if terminal is not None and not survivors:
                        break
                except OSError as error:
                    cleanup_errors.append(repr(error))
                kill_group()
                time.sleep(0.05)
            if terminal is not None:
                try:
                    status = child.wait(timeout=1)
                except (OSError, subprocess.TimeoutExpired) as error:
                    cleanup_errors.append(repr(error))
            else:
                cleanup_errors.append("no genuine terminal wait; not reported reaped")
            if status is not None and survivors == []:
                ACTIVE_CHILD = None
            child.stdout.close()
        if selector is not None:
            selector.close()
    receipt = {"status": status, "reason": reason, "direct_child_reaped": status is not None,
               "owned_group_survivors": survivors, "output_eof": eof,
               "cleanup_errors": cleanup_errors, "started": started, "finished": time.time(),
               "log_bytes": total, "log_sha256": digest(LOGS / (name + ".log")) if (LOGS / (name + ".log")).exists() else None}
    save(name + "-receipt.json", receipt)
    assert status == 0 and reason is None and eof and survivors == [] and not cleanup_errors, receipt


before, error, coverage, tools, tool_pins = None, None, None, [], None
try:
    before = source_pin()
    assert before["roster_sha256"] == "6dbf1ecb9a1b94d39de3162d15a28b4fb9cb57ffb8e8114977f9d5cecb235d35"
    roster = json.loads(ROSTER.read_text())
    assert roster["head"] == HEAD and roster["tree"] == TREE
    assert [(g["label"], g["package"]) for g in roster["groups"]] == [("lowerer", "fe2o3-lower-mir-kernel"), ("pliron", "fe2o3-pliron")]
    assert [g["minimum_total"] for g in roster["groups"]] == [4644, 2470]
    assert [len(g["required_normal_leaves"]) for g in roster["groups"]] == [258, 38]
    assert [len(g["focused_normal_leaves"]) for g in roster["groups"]] == [12, 5]
    assert all(set(g["focused_normal_leaves"]).issubset(g["required_normal_leaves"]) for g in roster["groups"])
    assert all(len(g["required_normal_leaves"]) == len(set(g["required_normal_leaves"])) for g in roster["groups"])
    for row in roster["source_pins"]:
        assert digest(SOURCE / row["path"]) == row["sha256"]
    nightly = Path.home() / ".rustup/toolchains/nightly-2026-04-03-x86_64-unknown-linux-gnu"
    environment = {"HOME": str(Path.home()), "PATH": f"{nightly}/bin:/usr/bin:/bin",
                   "RUSTUP_HOME": str(Path.home() / ".rustup"), "RUSTUP_TOOLCHAIN": "nightly-2026-04-03",
                   "RUSTC": str(nightly / "bin/rustc"), "LD_LIBRARY_PATH": str(nightly / "lib"),
                   "CARGO_HOME": str(LANE / "cargo-home"), "CARGO_TARGET_DIR": str(LANE / "target"),
                   "TMPDIR": str(LANE / "temporary"), "LANG": "C", "LC_ALL": "C", "TZ": "UTC",
                   "CARGO_BUILD_JOBS": "2", "CARGO_INCREMENTAL": "0", "CARGO_PROFILE_DEV_DEBUG": "0",
                   "CARGO_PROFILE_TEST_DEBUG": "0", "RUSTFLAGS": "-C link-arg=-Wl,--threads=1",
                   "FE2O3_HIP_SYS_DISABLE": "1", "FE2O3_HSA_RUNTIME_DISABLE": "1",
                   "PYTHONDONTWRITEBYTECODE": "1", "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null"}
    for directory in ["cargo-home", "target", "temporary"]:
        (LANE / directory).mkdir(mode=0o700)
    for path in [SOURCE, *SOURCE.parents, LANE / "cargo-home"]:
        for name in ["config", "config.toml"]:
            config = path / name if path == LANE / "cargo-home" else path / ".cargo" / name
            assert not config.exists() and not config.is_symlink(), config
    tools = [nightly / "bin/rustc", nightly / "bin/cargo", nightly / "lib/libLLVM.so.22.1-rust-1.96.0-nightly",
             nightly / "lib/librustc_driver-7bb70639c3ace5a4.so"]
    tool_pins = {str(path): digest(path) for path in tools}
    version = subprocess.check_output([str(nightly / "bin/rustc"), "-vV"], env=environment, timeout=30).decode()
    assert "commit-hash: 55e86c996809902e8bbad512cfb4d2c18be446d9" in version
    save("inputs.json", {"source": before, "tools": tool_pins, "rustc_version": version})
    cargo = str(nightly / "bin/cargo")
    run("fetch", [cargo, "fetch", "--locked", "--manifest-path", str(SOURCE / "Cargo.toml")], environment, 300, 16 * 1024**2)
    environment["CARGO_NET_OFFLINE"] = "true"
    coverage = []
    for group in roster["groups"]:
        package = tomllib.loads((SOURCE / group["manifest"]).read_text())["package"]["name"]
        assert package == group["package"]
        label = group["label"]
        args = [cargo, "test", "--offline", "--locked", "--no-default-features", "-j", "2", "-p",
                package, "--lib", "--no-run", "--message-format=json"]
        run(label + "-build", args, environment, 1800, 64 * 1024**2)
        binary = select_artifact((LOGS / (label + "-build.log")).read_text(), group)
        tested_artifact = artifact_pin(binary)
        save(label + "-artifact-before.json", tested_artifact)
        run(label + "-discovery", [str(binary), "--list", "-Z", "unstable-options", "--format=json"], environment, 60, 16 * 1024**2)
        assert artifact_pin(binary) == tested_artifact
        discovered = parse_discovery((LOGS / (label + "-discovery.log")).read_text(), group["minimum_total"], group["required_normal_leaves"])
        save(label + "-discovered-roster.json", discovered)
        focused = focused_roster(discovered, group["focused_normal_leaves"])
        save(label + "-focused-roster.json", focused)
        run(label + "-focused", [str(binary), "--exact", *focused["normal"], "--test-threads=8"], environment, 120, 64 * 1024**2)
        assert artifact_pin(binary) == tested_artifact
        save(label + "-artifact-after-focused.json", tested_artifact)
        focused_coverage = parse_coverage((LOGS / (label + "-focused.log")).read_text(), focused, discovered["total"] - focused["total"])
        save(label + "-focused-coverage.json", {"scope": "selected preflight, not full ordinary qualification", **focused_coverage})
        run(label + "-ordinary", [str(binary), "--test-threads=8"], environment, 1800, 64 * 1024**2)
        assert artifact_pin(binary) == tested_artifact
        save(label + "-artifact-after.json", tested_artifact)
        parsed = parse_coverage((LOGS / (label + "-ordinary.log")).read_text(), discovered)
        save(label + "-coverage.json", parsed)
        coverage.append({"package": package, **parsed})
    assert len(coverage) == len(roster["groups"]) == 2
    assert {str(path): digest(path) for path in tools} == tool_pins
except BaseException as failure:
    error = repr(failure)
finally:
    stable, tools_stable, cleanup_error = False, False, None
    try:
        stable = before is not None and source_pin() == before
        tools_stable = tool_pins is not None and {str(path): digest(path) for path in tools} == tool_pins
    except BaseException as failure:
        cleanup_error = "source recheck: " + repr(failure)
    try:
        current = LANE.lstat()
        assert stat.S_ISDIR(current.st_mode) and not LANE.is_symlink()
        assert (current.st_dev, current.st_ino, current.st_uid) == IDENTITY
        assert ACTIVE_CHILD is None, "unreaped child or unresolved owned group; preserve temporary tree"
        receipts = [json.loads(path.read_text()) for path in LOGS.glob("*-receipt.json")]
        assert all(r["direct_child_reaped"] and r["owned_group_survivors"] == [] for r in receipts)
        shutil.rmtree(LANE)
    except BaseException as failure:
        cleanup_error = (cleanup_error or "") + " cleanup: " + repr(failure)
    save("result.json", {"head": HEAD, "error": error, "source_stable": stable, "tools_stable": tools_stable,
                         "cleanup_error": cleanup_error, "temporary_absent": not LANE.exists(), "coverage": coverage,
                         "scope": "Ordinary diagnostic only; required CI policy, protected proofs, owning parents, service and publication remain unqualified."})
    print(json.dumps({"head": HEAD, "error": error, "source_stable": stable, "cleanup_error": cleanup_error, "coverage": coverage}))
raise SystemExit(0 if error is None and stable and tools_stable and cleanup_error is None else 1)
