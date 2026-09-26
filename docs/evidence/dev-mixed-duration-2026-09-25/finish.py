#!/usr/bin/env python3
"""Finish checks on the unchanged archive after the split-doctest false rejection."""
import hashlib
import json
import os
from pathlib import Path
import runpy
import signal
import sys
import types

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
SCRATCH = Path("/home/harsh/.codex-tmp/fe2o3-mixed-duration-20260925-Fi4yH01q")
BASE = SCRATCH / "signed-cpu-v2"
OUT = SCRATCH / "cpu-completion-v1"
RUN = runpy.run_path(str(HERE / "run.py"))
need, save = RUN["need"], RUN["save"]


def main():
    need(sys.flags.isolated and sys.flags.dont_write_bytecode and not sys.flags.optimize, "use python3 -I -B")
    before = json.loads((BASE / "source-before.json").read_text())
    need(before == json.loads((BASE / "source-after.json").read_text()), "closed original source brackets")
    need(before["commit"] == "57644e3252726637d8e260d7682ff35dc4b4b17b", "exact original source")
    source = Path(json.loads((BASE / "archive.json").read_text())["source"])
    environment = json.loads((BASE / "environment.json").read_text())
    helpers = [HERE / name for name in ("run.py", "test_run.py", "finish.py", "verify.py", "test_verify.py")]
    helper_commit = RUN["git"]("rev-parse", "HEAD").decode().strip()

    def snapshot():
        need(RUN["git"]("rev-parse", "HEAD").decode().strip() == helper_commit, "fixed helper commit")
        need({str(path.relative_to(source)) for path in source.rglob("*") if path.is_file()} == before["inputs"].keys(), "closed archive file set")
        for name, facts in before["inputs"].items():
            path = source / name
            need(path.is_file() and not path.is_symlink(), "regular archive input")
            raw = path.read_bytes()
            need(RUN["bind_blob"](raw, facts["git_blob"]) == facts["sha256"], "unchanged archive input")
            if not name.startswith("docs/evidence/"):
                need((ROOT / name).read_bytes() == raw, "current runtime source unchanged")
        bound = {}
        for path in helpers:
            name = str(path.relative_to(ROOT))
            oid = RUN["git"]("rev-parse", helper_commit + ":" + name).decode().strip()
            bound[name] = RUN["bind_blob"](path.read_bytes(), oid)
        return dict(source_commit=before["commit"], source_inputs=before["inputs"], helper_commit=helper_commit, helpers=bound)

    OUT.mkdir()
    opening = snapshot()
    save(OUT / "inputs-before.json", opening)
    controller_path = RUN["CONTROLLER"]
    raw = controller_path.read_bytes()
    need(hashlib.sha256(raw).hexdigest() == RUN["CONTROLLER_SHA"], "owned controller")
    controller = types.ModuleType("mixed_duration_completion")
    controller.__file__ = str(controller_path)
    sys.modules[controller.__name__] = controller
    exec(compile(raw, str(controller_path), "exec"), controller.__dict__)
    for number in controller.SIGNALS:
        signal.signal(number, controller.interrupted)
    os.chdir(ROOT)
    status, _, _ = controller.run_owned(["/usr/bin/git", "--no-replace-objects", "-c", "gpg.format=ssh",
        "-c", "gpg.ssh.program=/usr/bin/ssh-keygen", "-c", "gpg.ssh.allowedSignersFile=" + str(BASE / "allowed-signers"),
        "verify-commit", helper_commit], 60, OUT / "signature", environment)
    need(status == 0, "signed continuation helpers")
    os.chdir(source)
    cargo = ["cargo", "--locked", "--offline"]
    phases = [
        ("runner-tests", [sys.executable, "-I", "-B", str(HERE / "test_run.py")], None),
        ("replay-tests", [sys.executable, "-I", "-B", str(HERE / "test_verify.py")], None),
        ("doctests", [*cargo, "test", "-p", "fe2o3-runtime", "--all-features", "--doc"], [(4, 0, 0, 0, 0), (42, 0, 0, 0, 0)]),
        ("default", [*cargo, "check", "-p", "fe2o3-runtime"], None),
        ("clippy", [*cargo, "clippy", "-p", "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"], None),
        ("format", ["cargo", "fmt", "-p", "fe2o3-runtime", "--", "--check"], None),
    ]
    results = {}
    try:
        for name, command, expected in phases:
            need(snapshot() == opening, "continuity before " + name)
            print("RUN: " + name, flush=True)
            status, stdout, _ = controller.run_owned(command, 1200, OUT / name, environment)
            passed = RUN["accepted"](status, stdout, expected)
            results[name] = dict(passed=passed, status=status, counts=RUN["counts"](stdout))
            save(OUT / (name + ".json"), results[name])
            need(passed, "failed continuation phase: " + name)
            print("PASS: " + name, flush=True)
    finally:
        closing = snapshot()
        save(OUT / "inputs-after.json", closing)
        save(OUT / "results.json", results)
        need(opening == closing, "closing archive/helper continuity")


if __name__ == "__main__":
    main()
