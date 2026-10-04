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

HEAD = "9ddbe5eae020494d661e9f9247cbea14de693912"
TREE = "2020d9fbde02f567edcdd46475a0688352db3780"
ROOT = Path(os.environ["GITHUB_WORKSPACE"]).resolve(strict=True)
SOURCE = ROOT / "source"
CONTROL = ROOT / "control"
TEMP = Path(os.environ["RUNNER_TEMP"]).resolve(strict=True)
LOGS = TEMP / "issue271-global-guard-v85-logs"
ROSTER = CONTROL / ".github/diagnostics/issue271-proof-roster-v78.json"
os.umask(0o077)
assert TEMP != ROOT and ROOT not in TEMP.parents
LOGS.mkdir(mode=0o700)
LANE = Path(tempfile.mkdtemp(prefix="issue271-global-guard-v85-", dir=TEMP))
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


def parse_coverage(log, filters):
    roster_log = re.split(r"^failures:$", log, maxsplit=1, flags=re.M)[0]
    rows = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)(?:,.*)?$", roster_log, re.M)
    assert len(rows) == 241 and len({name for name, _ in rows}) == 241
    assert sorted(name.rsplit("::", 1)[-1] for name, _ in rows) == sorted(filters)
    assert all(result == "ok" for _, result in rows)
    assert re.findall(r"^running (\d+) tests?$", roster_log, re.M) == ["241"]
    assert len(re.findall(r"^test result:", log, re.M)) == 1
    summary = re.search(r"test result: ok\. 241 passed; 0 failed; 0 ignored; 0 measured; (\d+) filtered out; finished in [0-9.]+s\s*\Z", log)
    assert summary is not None
    return {"passed": 241, "failed": 0, "ignored": 0, "filtered": int(summary.group(1)), "rows": rows,
            "ignored_owning_parents_executed": False}


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
    assert before["roster_sha256"] == "ccfcfc90b454eedd7d106b53e2461ca80f959d43bd13fced5f2f98c8594d0541"
    roster = json.loads(ROSTER.read_text())
    assert roster["head"] == HEAD and roster["package"] == "fe2o3-lower-mir-kernel"
    filters = roster["filters"]
    assert len(filters) == len(set(filters)) == 241
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
    args = [cargo, "test", "--offline", "--locked", "--no-default-features", "-j", "2", "-p",
            roster["package"], "--lib", "--", "--test-threads=8", *filters]
    for name in roster["ignored_not_selected"]:
        args += ["--skip", name]
    run("ordinary", args, environment, 1800, 64 * 1024**2)
    log = (LOGS / "ordinary.log").read_text()
    coverage = parse_coverage(log, filters)
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
