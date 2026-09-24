#!/usr/bin/env python3
"""Cold-build the signed runtime harness; no SSH or device access."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import signal
import tarfile
from types import ModuleType, SimpleNamespace

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
COMMIT = "3540325123fa30b5208d57ba950922a707eb07b8"
CPU = "docs/evidence/dev-topology-link-directory-cpu-2026-09-24"
HELPER = "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py"
HELPER_SHA = "df740a80c83c94af02f7e7dfb1203dcf7c1eafbde0010e41219e4aac40b34bb7"
SELECTORS = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "examples",
             "benchmarks/runtime_gfx942", "scripts/unsafe-source-baseline.json",
             "docs/runtime-primary-queue-release-v1.md"]
GIT = ["/usr/bin/git", "--no-replace-objects", "-c", "core.fsmonitor=false", "-c", "core.untrackedCache=false"]
GIT_ENV = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C", "GIT_CONFIG_NOSYSTEM": "1",
           "GIT_CONFIG_SYSTEM": "/dev/null", "GIT_CONFIG_GLOBAL": "/dev/null"}
SIGNERS = "/home/harsh/.codex-tmp/fe2o3-logical-mux-sdma-protocol-v2-20260918/allowed-signers"
SIGNERS_SHA = "ea67b34912b272ccbb46a4a83d8e00eb1a83e87832f089c1c134eed1786b560b"


def load(path, digest, name):
    if path.is_symlink() or not path.is_file():
        raise RuntimeError("ordinary helper required")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != digest:
        raise RuntimeError("helper digest: " + str(path))
    value = ModuleType(name)
    value.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), value.__dict__)
    return value


def helpers():
    base = load(REPO / HELPER, HELPER_SHA, "native_matrix_build_helpers")
    return SimpleNamespace(B=base, H=base, K=SimpleNamespace(GIT=GIT, GIT_ENV=GIT_ENV),
                           C=SimpleNamespace(SIGNERS=SIGNERS, SIGNERS_SHA=SIGNERS_SHA))


def source_archive(rec, output, b):
    folder = rec.run("source-tree", [*GIT, "ls-tree", "-rz", COMMIT, "--", *SELECTORS], 120, env=GIT_ENV)
    entries = {}
    for row in (folder / "stdout").read_bytes().rstrip(b"\0").split(b"\0"):
        header, raw_name = row.split(b"\t")
        mode, kind, oid = header.decode().split()
        name = raw_name.decode()
        b.need(mode in ("100644", "100755") and kind == "blob" and name not in entries, "ordinary unique source")
        entries[name] = (mode, oid)
    selectors = [value for value in SELECTORS if any(name == value or name.startswith(value + "/") for name in entries)]
    snapshot = output / "build-source.tar.gz"
    rec.run("source-archive", [*GIT, "archive", "--format=tar.gz", "--output=" + str(snapshot), COMMIT, *selectors],
            120, env=GIT_ENV)
    checkout = output / "source"
    checkout.mkdir()
    with tarfile.open(snapshot, "r:gz") as archive:
        members = archive.getmembers()
        directories = {str(parent) for name in entries for parent in Path(name).parents if str(parent) != "."}
        observed = set()
        for member in members:
            b.need(member.isfile() or member.isdir(), "ordinary source entries")
            if member.isdir():
                name = member.name.rstrip("/")
                b.need(name in directories and name not in observed, "exact parent directory")
                observed.add(name)
        b.validate_members([member for member in members if member.isfile()], entries)
        archive.extractall(checkout, filter="data")
    files = b.inventory(checkout)
    b.need(set(files) == set(entries), "complete source extraction")
    for name, (mode, oid) in entries.items():
        raw = (checkout / name).read_bytes()
        b.need(hashlib.sha1(b"blob " + str(len(raw)).encode() + b"\0" + raw).hexdigest() == oid, "signed source blob")
        b.need(bool((checkout / name).stat().st_mode & 0o111) == (mode == "100755"), "source mode")
    b.write_json(output / "source.json", files)
    b.write_json(output / "source-archive.json", {"sha256": b.sha(snapshot), "bytes": snapshot.stat().st_size})
    return checkout, files


def build_environment(output):
    target = output / "target"
    target.mkdir()
    return {"HOME": "/home/harsh", "USER": "harsh", "PATH": "/home/harsh/.cargo/bin:/usr/bin:/bin",
            "LANG": "C", "LC_ALL": "C", "CARGO_TARGET_DIR": str(target), "CARGO_INCREMENTAL": "0",
            "CARGO_BUILD_JOBS": "2", "CARGO_TERM_COLOR": "never", "RUSTUP_TOOLCHAIN": "nightly-2026-04-03"}


def roster(text):
    rows = re.findall(r"^test (\S+) \.\.\. (ok|ignored(?:, [^\n]*)?)$", text, re.MULTILINE)
    result = {name: outcome.split(",", 1)[0] for name, outcome in rows}
    if len(rows) != len(result) or len(result) != 1385 or list(result.values()).count("ok") != 1365:
        raise RuntimeError("complete runtime CPU roster")
    summaries = re.findall(r"^test result:.*$", text, re.MULTILINE)
    if len(summaries) != 1 or not re.fullmatch(
        r"test result: ok\. 1365 passed; 0 failed; 20 ignored; 0 measured; 0 filtered out; finished in [0-9.]+s",
        summaries[0],
    ):
        raise RuntimeError("exact runtime CPU summary")
    return result


def executable(data, target):
    candidates = []
    for line in data.splitlines():
        value = json.loads(line)
        if (value.get("reason") == "compiler-artifact" and value.get("target", {}).get("name") == "fe2o3_runtime"
                and value.get("target", {}).get("kind") == ["lib"] and value.get("profile", {}).get("test") is True
                and value.get("executable")):
            candidates.append(Path(value["executable"]))
    if len(candidates) != 1:
        raise RuntimeError("one exact Cargo runtime test artifact")
    path = candidates[0]
    if path.is_symlink() or not path.is_file() or path.resolve().parent != target / "x86_64-unknown-linux-musl/debug/deps":
        raise RuntimeError("cold-target runtime artifact")
    if path.read_bytes()[:4] != b"\x7fELF":
        raise RuntimeError("runtime ELF")
    return path


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output
    if (output.resolve() != output or output.is_symlink() or output.name != "build"
            or not re.fullmatch(r"/home/harsh/\.codex-tmp/fe2o3-runtime-native-20260924-[A-Za-z0-9]{8}", str(output.parent))):
        raise RuntimeError("fresh private build path")
    output.mkdir()
    c = helpers()
    for number in c.B.MANAGED:
        signal.signal(number, c.B.interrupted)
    rec = c.B.Recorder(output / "commands", REPO)
    runner_sha = c.H.sha(Path(__file__))
    shutil.copy2(__file__, output / "prepare.py")
    c.B.write_json(output / "runner-before.json", {"sha256": runner_sha})
    c.H.need(c.H.sha(Path(c.C.SIGNERS)) == c.C.SIGNERS_SHA, "pinned signer")
    rec.run("signature", [*c.K.GIT, "-c", "gpg.ssh.allowedSignersFile=" + c.C.SIGNERS, "verify-commit", COMMIT],
            30, env=c.K.GIT_ENV)
    source, files = source_archive(rec, output, c.B)
    prior = rec.run("cpu-source", [*c.K.GIT, "show", COMMIT + ":" + CPU + "/raw/cpu2/inputs-before.json"],
                    30, env=c.K.GIT_ENV)
    inputs = json.loads((prior / "stdout").read_bytes())["source"]
    roots = ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo", "crates", "benchmarks/runtime_gfx942"]
    expected = {name: digest for name, digest in files.items()
                if any(name == root or name.startswith(root + "/") for root in roots)
                and (name.endswith((".rs", ".toml", ".lock", ".json", ".py", ".cpp", ".hpp")) or "/fixtures/" in name)}
    c.H.need(inputs == expected and len(inputs) == 3956, "exact CPU-qualified source inputs")
    prior = rec.run("cpu-roster", [*c.K.GIT, "show", COMMIT + ":" + CPU + "/raw/cpu2/commands/musl-fe2o3-runtime/stdout"],
                    30, env=c.K.GIT_ENV)
    expected_roster = roster((prior / "stdout").read_text())
    env = build_environment(output)
    env.update(CARGO_PROFILE_TEST_OPT_LEVEL="1", CARGO_PROFILE_TEST_DEBUG="0", CARGO_PROFILE_DEV_DEBUG="0")
    rec.cwd = source
    binary, binary_sha = None, None
    try:
        for tool, arguments in (("rustc", ["rustc", "-vV"]), ("cargo", ["cargo", "-V"])):
            rec.run(tool, arguments, 30, env=env)
        built = rec.run("build", ["cargo", "test", "--offline", "--locked", "-p", "fe2o3-runtime", "--all-features",
                                  "--lib", "--target", "x86_64-unknown-linux-musl", "--no-run", "--message-format=json"],
                        7200, env=env)
        binary = executable((built / "stdout").read_bytes(), Path(env["CARGO_TARGET_DIR"]))
        binary_sha = c.H.sha(binary)
        shutil.copy2(binary, output / "runtime-tests")
        tested = rec.run("runtime-cpu", [str(binary), "--test-threads=4", "--color=never"], 7200, env=env)
        c.H.need(roster((tested / "stdout").read_text()) == expected_roster, "actual native ELF passes exact CPU roster")
        for tool, arguments in (("rustc", ["rustc", "-vV"]), ("cargo", ["cargo", "-V"])):
            rec.run("after-" + tool, arguments, 30, env=env)
            for stream in ("stdout", "stderr"):
                c.H.need(c.H.sha(rec.output / tool / stream) == c.H.sha(rec.output / ("after-" + tool) / stream), "tool continuity")
        c.H.need(c.H.sha(binary) == c.H.sha(output / "runtime-tests") == binary_sha, "unchanged executed and retained ELF")
        c.B.write_json(output / "build.json", {"commit": COMMIT, "binary_sha256": binary_sha, "source_files": files,
                       "environment": env, "runtime_roster": expected_roster, "cpu_source_inputs": len(inputs)})
    finally:
        c.B.write_json(output / "source-after.json", c.B.inventory(source))
        c.B.write_json(output / "runner-after.json", {"sha256": c.H.sha(Path(__file__))})
        c.H.need(c.B.inventory(source) == files, "unchanged signed build inputs")
        c.H.need(c.H.sha(Path(__file__)) == runner_sha, "unchanged build runner")


if __name__ == "__main__":
    main()
