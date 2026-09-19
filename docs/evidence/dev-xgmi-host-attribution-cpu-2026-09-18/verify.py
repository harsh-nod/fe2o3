#!/usr/bin/env python3
"""Verify recorded CPU commands, test closures, unchanged source, and archive seal."""

import argparse
import json
import re
import subprocess

import qualify as Q

N, HERE, ROOT = Q.N, Q.HERE, Q.ROOT


def verify(live):
    paths = (Q.RUNNER, Q.SELECTOR, HERE / "qualify.py", HERE / "verify.py")
    N.need(
        json.loads((HERE / "tools.json").read_text())
        == {str(path.relative_to(ROOT)): N.sha(path) for path in paths},
        "exact qualification tools",
    )
    expected = Q.commands()
    N.need(
        {p.name for p in (HERE / "raw").iterdir()} == {row[0] for row in expected},
        "exact command roster",
    )
    previous = 0
    for name, command, seconds in expected:
        folder = HERE / "raw" / name
        N.need(
            {p.name for p in folder.iterdir()} == {"receipt.json", "stdout", "stderr"},
            "exact receipt files",
        )
        row = json.loads((folder / "receipt.json").read_text())
        N.need(
            row["command"] == command
            and row["cwd"] == str(ROOT)
            and row["timeout_seconds"] == seconds
            and row["environment"] is None
            and row["stdin_sha256"] is None
            and row["exit"] == 0
            and row["error"] is None
            and row["group_absent"] is True
            and type(row["pid"]) is int
            and row["pid"] > 1
            and previous <= row["started_ns"] < row["finished_ns"],
            name + " successful exact receipt",
        )
        for stream in ("stdout", "stderr"):
            N.need(
                N.sha(folder / stream) == row[stream + "_sha256"],
                name + " output digest",
            )
        previous = row["finished_ns"]
        count = (
            34
            if name.endswith("-kfd")
            else 25
            if name.endswith("-runtime")
            else 4
            if name in ("gnu-example", "musl-example", "example-feature-off")
            else None
        )
        if count is not None:
            output = (folder / "stdout").read_text()
            closures = re.findall(
                r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;",
                output,
                re.MULTILINE,
            )
            N.need(
                closures and closures[-1] == (str(count), "0", "0"),
                name + " exact passing test closure",
            )
        if name.endswith("-roster"):
            package = name.split("-")[1]
            count = 34 if package == "kfd" else 25
            output = (folder / "stdout").read_text()
            N.need(
                len(re.findall(r"^.+: test$", output, re.MULTILINE)) == count
                and f"{count} tests, 0 benchmarks" in output,
                name + " exact test count",
            )
    before = (HERE / "raw/source-before/stdout").read_bytes()
    N.need(
        before == (HERE / "raw/source-after/stdout").read_bytes(),
        "unchanged qualified source",
    )
    snapshot = json.loads(before)
    N.need(
        len(snapshot["files"]) == 5564,
        "source cohort including both new diagnostic modules",
    )
    if live:
        current = json.loads(
            subprocess.check_output(["python3", "-I", str(Q.SELECTOR)], cwd=ROOT)
        )
        N.need(current["files"] == snapshot["files"], "live source equality")
    policy = (HERE / "raw/unsafe-source/stdout").read_text()
    N.need(
        "test result: ok. 5 passed; 0 failed; 1 ignored;" in policy,
        "unsafe policy: five passed, one maintenance test ignored",
    )
    return {
        "cpu_qualification": True,
        "source_base": snapshot["base"],
        "source_files": len(snapshot["files"]),
        "commands": len(expected),
        "native_execution": False,
        "formal_refinement": False,
        "performance_acceptance": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group()
    mode.add_argument("--seal", action="store_true")
    mode.add_argument("--allow-unsealed", action="store_true")
    parser.add_argument("--live", action="store_true")
    args = parser.parse_args()
    result = verify(args.live)
    inventory = N.inventory(HERE)
    inventory.pop("SHA256SUMS", None)
    manifest = "".join(
        digest + "  " + name + "\n" for name, digest in inventory.items()
    )
    if args.seal:
        with (HERE / "SHA256SUMS").open("x") as output:
            output.write(manifest)
    elif not args.allow_unsealed or (HERE / "SHA256SUMS").exists():
        N.need((HERE / "SHA256SUMS").read_text() == manifest, "complete archive seal")
    print(json.dumps(result, sort_keys=True))


if __name__ == "__main__":
    main()
