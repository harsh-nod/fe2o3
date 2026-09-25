#!/usr/bin/env python3
"""Record CPU replay and negative tests before exact-owned scratch removal."""

import sys

if not sys.flags.isolated or not sys.flags.dont_write_bytecode:
    raise RuntimeError("use python3 -I -B")

import argparse
import hashlib
import os
from pathlib import Path
import re
import shutil
import signal
import stat
from types import ModuleType

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]


def module(path, digest, name):
    if not stat.S_ISREG(path.lstat().st_mode):
        raise RuntimeError("ordinary authenticated helper")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != digest:
        raise RuntimeError("authenticated helper identity")
    value = ModuleType(name)
    value.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), value.__dict__)
    return value


Q = module(HERE / "qualify.py",
    "682b362a11e17589c2bb3a5f306bc85e66351ad15008996a50abc35599342ecc", "mixed_cpu_finalizer")
COLLECT = REPO / "docs/evidence/dev-native-producer-mi300x-2026-09-24"
C = module(COLLECT / "collect.py",
    "9c9c629aa7ce73e52f565dee51f89810ae96a152ad3b2fb4c9f272f864c3f9cf", "mixed_cpu_owned_cleanup")
DEVELOPMENT = {
    "dev-model.log", "dev-runtime.log", "dev-runtime2.log", "dev-proof1.log",
    "dev-proof2.log", "dev-proof3.log", "dev-proof-whole1.stdout", "dev-proof-whole1.stderr",
    "dev-proof-whole2.stdout", "dev-proof-whole2.stderr",
}
DEVELOPMENT_GROUPS = (471835, 494169, 112704, 112711)
ENV = {"HOME": "/home/harsh", "PATH": "/usr/bin:/bin", "LC_ALL": "C"}
ABSENCE = ["/usr/bin/python3", "-I", "-B", "-c",
           "import os, sys; assert not os.path.lexists(sys.argv[1]), 'owned path still exists'", str(Q.PRIVATE)]


def inputs():
    return {"qualification": Q.Q.inputs(), "packet": {
        name: Q.R.sha(HERE / name) for name in (
            "qualify.py", "test_qualify.py", "finalize.py", "test_finalize.py", "README.md")
    }, "cleanup_tests": Q.R.sha(COLLECT / "test_verify.py")}


def audit_stages():
    yield "replay", ["/usr/bin/python3", "-I", "-B", str(HERE / "qualify.py"), "--output",
                     str(HERE / "raw/cpu1"), "--verify"], 900
    yield "rejections", ["/usr/bin/python3", "-I", "-B", str(HERE / "test_qualify.py")], 900
    yield "cleanup-tests", ["/usr/bin/python3", "-I", "-B", str(COLLECT / "test_verify.py"), "CleanupTests"], 900
    yield "finalizer-tests", ["/usr/bin/python3", "-I", "-B", str(HERE / "test_finalize.py")], 900


def terminal_groups(root, stages, environment, recorder):
    stages = list(stages)
    Q.V.inventory(root)
    Q.need({path.name for path in root.iterdir()} == {name for name, _, _ in stages}, "exact terminal command roster")
    previous = 0
    for name, command, bound in stages:
        folder = root / name
        Q.need({path.name for path in folder.iterdir()} == {"receipt.json", "stdout", "stderr"}, "command artifacts")
        row = Q.V.read(folder / "receipt.json")
        Q.need(set(row) == Q.V.FIELDS and type(row["pid"]) is int and row["pid"] > 0
               and type(row["exit"]) is int and row["exit"] == 0 and row["error"] is None
               and row["group_absent"] is True, "successful terminal command")
        Q.need(row["command"] == command and row["cwd"] == str(REPO) and row["environment"] == environment
               and type(row["timeout_seconds"]) is int and row["timeout_seconds"] == bound
               and row["stdin_sha256"] is None, "exact cleanup command controls")
        Q.need(type(row["started_ns"]) is int and type(row["finished_ns"]) is int
               and previous <= row["started_ns"] < row["finished_ns"]
               and row["finished_ns"] - row["started_ns"] <= (bound + 15) * 10**9,
               "ordered bounded cleanup command")
        previous = row["finished_ns"]
        for stream in ("stdout", "stderr"):
            Q.need(Q.R.sha(folder / stream) == row[stream + "_sha256"], "terminal output identity")
        Q.need(not recorder.group_exists(row["pid"]), "command process group still exists")


def private_users_absent(proc=Path("/proc"), private=Q.PRIVATE):
    prefix = os.fsencode(private)
    report = {"scope": "same-uid-best-effort-not-global-absence", "inspected_processes": 0,
              "observed_private_references": 0, "uninspectable": [], "vanished": []}

    def outside(path):
        return path != prefix and not path.startswith(prefix + b"/")

    for entry in proc.iterdir():
        if not entry.name.isdecimal():
            continue
        try:
            if entry.stat().st_uid != os.getuid():
                continue
        except FileNotFoundError:
            Q.need(not entry.exists(), "incomplete inspection of a live process")
            continue
        identity = {"pid": int(entry.name), "uid": os.getuid(), "start_time_ticks": None}
        report["inspected_processes"] += 1

        def observe(field, read):
            try:
                return read()
            except PermissionError as error:
                report["uninspectable"].append({"process_identity": dict(identity), "field": field, "errno": error.errno})
                return None
            except FileNotFoundError:
                Q.need(not entry.exists(), "incomplete inspection of a live process")
                if identity["pid"] not in report["vanished"]:
                    report["vanished"].append(identity["pid"])
                return None

        def start_time(raw):
            if raw is None:
                return None
            head, separator, tail = raw.rpartition(b") ")
            fields = tail.split()
            Q.need(separator and head.split(b" (", 1)[0] == entry.name.encode()
                   and len(fields) >= 20 and fields[19].isdigit(), "bounded process identity shape")
            return int(fields[19])

        identity["start_time_ticks"] = start_time(observe("stat", lambda: (entry / "stat").read_bytes()))
        arguments = observe("cmdline", lambda: (entry / "cmdline").read_bytes())
        cwd = observe("cwd", lambda: os.readlink(os.fsencode(entry / "cwd")))
        mappings = observe("maps", lambda: (entry / "maps").read_bytes())
        descriptors = observe("fd", lambda: list((entry / "fd").iterdir()))
        Q.need(arguments is None or prefix not in arguments, "live process references owned scratch")
        Q.need(cwd is None or outside(cwd.removesuffix(b" (deleted)")), "live process references owned scratch")
        for mapping in (mappings or b"").splitlines():
            fields = mapping.split(maxsplit=5)
            if len(fields) == 6:
                Q.need(outside(fields[5]), "live mapping references owned scratch")
        for descriptor in descriptors or []:
            try:
                target = os.readlink(os.fsencode(descriptor))
            except PermissionError as error:
                report["uninspectable"].append({"process_identity": dict(identity),
                                              "field": "fd/" + descriptor.name, "errno": error.errno})
                continue
            except FileNotFoundError:
                continue
            Q.need(outside(target), "live descriptor references owned scratch")
        after = start_time(observe("stat-after", lambda: (entry / "stat").read_bytes()))
        Q.need(after is None or identity["start_time_ticks"] is None or after == identity["start_time_ticks"],
               "process identity changed during inspection")
    return report


def cleanup(output, recorder, before):
    raw = HERE / "raw"
    Q.need({path.name for path in raw.iterdir()} == {"cpu1"}, "exact original retained campaign roster")
    Q.verify(raw / "cpu1")
    terminal_groups(raw / "cpu1/commands", Q.stages(), Q.environment(), recorder)
    terminal_groups(output / "commands", audit_stages(), ENV, recorder)
    for group in DEVELOPMENT_GROUPS:
        Q.need(not recorder.group_exists(group), "development process group still exists")
    recorder.write_json(output / "process-visibility-before.json", private_users_absent())
    Q.need({path.name for path in Q.PRIVATE.iterdir()} == DEVELOPMENT | {"target"}, "exact private scratch roster")
    snapshot = C.owned_snapshot(Q.PRIVATE)
    development = raw / "development"
    development.mkdir()
    expected = {name: Q.R.sha(Q.PRIVATE / name) for name in DEVELOPMENT}
    for name in sorted(DEVELOPMENT):
        shutil.copy2(Q.PRIVATE / name, development / name)
    Q.need(Q.V.inventory(development) == expected, "byte-exact development log retention")
    recorder.write_json(raw / "cleanup-source.json", snapshot)
    allocated = sum(path.stat().st_blocks * 512 for path in (Q.PRIVATE, *Q.PRIVATE.rglob("*")))
    row = {"path": str(Q.PRIVATE), "allocated_bytes": allocated, "absent": False,
           "development_files": expected, "source_inventory_sha256": Q.R.sha(raw / "cleanup-source.json"),
           "development_groups": list(DEVELOPMENT_GROUPS), "cpu_commands": 15, "audit_commands": 4,
           "excluded_from_retention": ["target"]}
    recorder.write_json(raw / "cleanup-before.json", row)
    Q.need(inputs() == before, "unchanged inputs before owned removal")
    recorder.write_json(output / "process-visibility-final.json", private_users_absent())
    C.remove_owned(Q.PRIVATE, snapshot)
    Q.need(not Q.PRIVATE.exists() and not Q.PRIVATE.is_symlink(), "owned scratch absent")
    recorder.write_json(raw / "cleanup-after.json", row | {"absent": True})
    print({"removed_allocated_bytes": allocated, "development_logs_retained": len(expected)}, flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cleanup", action="store_true")
    args = parser.parse_args()
    output = args.output.absolute()
    Q.need(output.resolve() == output and output.parent == HERE
           and re.fullmatch(r"audit[1-9][0-9]*", output.name) is not None, "exact new audit directory")
    before = inputs()
    Q.need(before["packet"]["test_qualify.py"] ==
           "515d00434fcc8fab1fbc90fcf545e14c736d07765f62826eb2eeffd4d4214c65"
           and before["packet"]["test_finalize.py"] ==
           "7f8bd80167c753d8b0ca8e6e4c8a1342ae802f30c9158c85cb3b874af2828761"
           and before["cleanup_tests"] ==
           "61971db898f5db1e3c5d56e7339ae1d6425dea265a06eccde6083ca93523feea", "reviewed negative tests")
    output.mkdir()
    b = Q.R.helpers().B
    b.write_json(output / "inputs-before.json", before)
    recorder = b.Recorder(output / "commands", REPO)
    for number in b.MANAGED:
        signal.signal(number, b.interrupted)
    try:
        for name, command, bound in audit_stages():
            folder = recorder.run(name, command, bound, env=ENV)
            count = {"rejections": 8, "cleanup-tests": 5, "finalizer-tests": 8}.get(name)
            if count is not None:
                text = (folder / "stderr").read_text()
                Q.need(re.findall(r"^Ran (\d+) tests? in [0-9.]+s$", text, re.MULTILINE) == [str(count)]
                       and text.endswith("\nOK\n") and "skipped" not in text, "complete passing negative groups")
        if args.cleanup:
            cleanup(output, b, before)
            recorder.run("absence", ABSENCE, 30, env=ENV)
            terminal_groups(output / "commands", [*audit_stages(), ("absence", ABSENCE, 30)], ENV, b)
            Q.need(not Q.PRIVATE.exists() and not Q.PRIVATE.is_symlink(), "final owned path absence")
    finally:
        after = inputs()
        b.write_json(output / "inputs-after.json", after)
        Q.need(before == after, "unchanged audit inputs")
    if args.cleanup:
        b.write_json(HERE / "artifacts.json", Q.V.inventory(HERE / "raw"))


if __name__ == "__main__":
    main()
