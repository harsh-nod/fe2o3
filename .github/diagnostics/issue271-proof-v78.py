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

HEAD = "dde4eaee5daf7cde3a20f1a1a475eb0944ff1f9e"
TREE = "4a69d0b7b7db2ba0288c61fc6c1a3e973199b70a"
ROOT = Path(os.environ["GITHUB_WORKSPACE"]).resolve(strict=True)
SOURCE = ROOT / "source"
CONTROL = ROOT / "control"
TEMP = Path(os.environ["RUNNER_TEMP"]).resolve(strict=True)
LOGS = TEMP / "issue271-proof-v78-logs"
ROSTER = CONTROL / ".github/diagnostics/issue271-proof-roster-v78.json"
os.umask(0o077)
assert TEMP != ROOT and ROOT not in TEMP.parents
LOGS.mkdir(mode=0o700)
LANE = Path(tempfile.mkdtemp(prefix="issue271-proof-v78-", dir=TEMP))
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


def select_artifact(log):
    records = [json.loads(line) for line in log.splitlines() if line.startswith("{")]
    finish = [row for row in records if row.get("reason") == "build-finished"]
    assert len(finish) == 1 and finish[0]["success"] is True
    rows = [row for row in records if row.get("reason") == "compiler-artifact"
            and row.get("executable") is not None]
    assert len(rows) == 1
    row = rows[0]
    assert row["target"]["name"] == "fe2o3_verifier"
    assert row["target"]["kind"] == ["lib"] and row["profile"]["test"] is True
    assert Path(row["manifest_path"]).resolve(strict=True) == SOURCE / "crates/fe2o3-verifier/Cargo.toml"
    assert Path(row["target"]["src_path"]).resolve(strict=True) == SOURCE / "crates/fe2o3-verifier/src/lib.rs"
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


def export_tested_artifacts(binary, pins):
    destination = LOGS / "artifacts"
    destination.mkdir(mode=0o700)
    assert shutil.disk_usage(LOGS).free >= 2 * 1024**3
    rows = []
    for role, original in [("verifier_binary", binary),
                           ("verifier_depfile", binary.with_suffix(".d"))]:
        before = artifact_pin(original)
        assert before == pins[role]
        bound = 1024**3 if role == "verifier_binary" else 16 * 1024**2
        assert before["identity"][0] <= bound
        copied = destination / original.name
        total = 0
        with original.open("rb") as source, copied.open("xb") as output:
            while chunk := source.read(65536):
                total += len(chunk)
                assert total <= bound
                output.write(chunk)
            output.flush()
            os.fsync(output.fileno())
        os.chmod(copied, 0o500 if role == "verifier_binary" else 0o400)
        after = artifact_pin(original)
        copied_pin = artifact_pin(copied)
        assert after == before
        assert total == copied_pin["identity"][0] == before["identity"][0]
        assert copied_pin["sha256"] == before["sha256"]
        rows.append({"role": role, "original": before, "copy": copied_pin})
    save("artifact-export.json", {"head": HEAD, "tree": TREE, "source": source_pin(),
                                  "artifacts": rows, "protected_proof_executed": False})


def parse_coverage(log, filters):
    roster_log = re.split(r"^failures:$", log, maxsplit=1, flags=re.M)[0]
    rows = re.findall(r"^test (\S+) \.\.\. (ok|FAILED|ignored)(?:,.*)?$", roster_log, re.M)
    assert len(rows) == 185 and len({name for name, _ in rows}) == 185
    assert sorted(name.rsplit("::", 1)[-1] for name, _ in rows) == sorted(filters)
    assert all(result == "ok" for _, result in rows)
    assert re.findall(r"^running (\d+) tests?$", roster_log, re.M) == ["185"]
    assert len(re.findall(r"^test result:", log, re.M)) == 1
    summary = re.search(r"test result: ok\. 185 passed; 0 failed; 0 ignored; 0 measured; (\d+) filtered out; finished in [0-9.]+s\s*\Z", log)
    assert summary is not None
    return {"passed": 185, "failed": 0, "ignored": 0, "filtered": int(summary.group(1)), "rows": rows,
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
    assert before["roster_sha256"] == "12566d5cc5c9862f497f9ef238280154e868b6182fb23674815605afbddcc602"
    roster = json.loads(ROSTER.read_text())
    assert roster["head"] == HEAD and roster["package"] == "fe2o3-verifier"
    filters = roster["filters"]
    assert len(filters) == len(set(filters)) == 185
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
    artifact_args = [cargo, "test", "--offline", "--locked", "--no-default-features", "-j", "2", "-p",
                     roster["package"], "--lib", "--no-run", "--message-format=json"]
    run("artifact-build", artifact_args, environment, 120, 32 * 1024**2)
    binary = select_artifact((LOGS / "artifact-build.log").read_text())
    artifact_before = {"verifier_binary": artifact_pin(binary),
                       "verifier_depfile": artifact_pin(binary.with_suffix(".d"))}
    save("artifact-before.json", artifact_before)
    direct_args = [str(binary), "--test-threads=8", *filters]
    for name in roster["ignored_not_selected"]:
        direct_args += ["--skip", name]
    run("artifact-ordinary", direct_args, environment, 120, 64 * 1024**2)
    direct_coverage = parse_coverage((LOGS / "artifact-ordinary.log").read_text(), filters)
    assert direct_coverage["rows"] == coverage["rows"] or dict(direct_coverage["rows"]) == dict(coverage["rows"])
    artifact_after = {"verifier_binary": artifact_pin(binary),
                      "verifier_depfile": artifact_pin(binary.with_suffix(".d"))}
    assert artifact_after == artifact_before
    save("artifact-after.json", artifact_after)
    save("artifact-coverage.json", direct_coverage)
    assert source_pin() == before
    export_tested_artifacts(binary, artifact_before)
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
