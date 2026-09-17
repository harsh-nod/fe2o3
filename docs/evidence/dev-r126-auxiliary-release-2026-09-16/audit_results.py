#!/usr/bin/env python3
"""Audit the completed development campaign in its original live checkout."""

import hashlib
import json
from pathlib import Path
import re
import shlex


ARCHIVE = Path(__file__).resolve().parent
ROOT = ARCHIVE.parents[2]
FINAL = ARCHIVE / "final"
TARGETS = ("gnu", "musl")
CRATES = ("kfd", "runtime")
FEATURES = {
    "kfd": ["default", "live-validation"],
    "runtime": ["default", "hardware-diagnostic", "hardware-qualification"],
}
CPU_TEST = (
    "kfd_backend::retained_release_tests::"
    "runtime_auxiliary_teardown_errors_and_panics_seal_reentry_without_destroyed_events"
)
NATIVE_TEST = (
    "kfd_backend::retained_release_tests::"
    "native_runtime_auxiliary_shutdown_retries_after_primary_capacity_rejection"
)


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def read(name):
    return (FINAL / name).read_text()


def command(name, expected):
    require(shlex.split(read(name + ".command")) == expected, f"{name}: command mismatch")


def roster(name):
    text = read(name + ".log")
    names = re.findall(r"^(\S+): test$", text, re.MULTILINE)
    require(len(names) == len(set(names)), f"{name}: duplicate test")
    summaries = re.findall(r"^(\d+) tests?, (\d+) benchmarks?$", text, re.MULTILINE)
    require(summaries == [(str(len(names)), "0")], f"{name}: invalid roster summary")
    return set(names)


expected_records = {
    "source-before", "rustc", "cargo", "python", "clippy", "formatting",
    "no-default", "unsafe-policy", "source-after",
}
for target in TARGETS:
    expected_records.add(target + "-build")
    expected_records.add(target + "-runtime-ignored")
    for crate in CRATES:
        expected_records.update({f"{target}-{crate}-roster", f"{target}-{crate}-full"})
require(len(expected_records) == 21, "invalid audit command inventory")
require({path.stem for path in FINAL.glob("*.exit")} == expected_records,
        "incomplete or extra command status records")
for name in sorted(expected_records):
    for suffix in ("command", "log", "started", "finished", "exit"):
        require((FINAL / f"{name}.{suffix}").is_file(), f"{name}: missing {suffix}")
    require(read(name + ".exit").strip() == "0", f"{name}: nonzero exit")
    start, finish = (read(name + "." + suffix).strip() for suffix in ("started", "finished"))
    require(bool(re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{9}Z", start))
            and bool(re.fullmatch(r"\d{4}-\d\d-\d\dT\d\d:\d\d:\d\d\.\d{9}Z", finish))
            and start <= finish, f"{name}: invalid timestamps")
require("host: x86_64-unknown-linux-gnu\n" in read("rustc.log"), "unexpected GNU host")
other_commands = {
    "source-before": ["sha256sum", "--check", str(ARCHIVE / "source-files.sha256")],
    "source-after": ["sha256sum", "--check", str(ARCHIVE / "source-files.sha256")],
    "rustc": ["rustc", "-vV"],
    "cargo": ["cargo", "-V"],
    "python": ["python3", "--version"],
    "clippy": ["cargo", "clippy", "--locked", "--offline", "-p", "fe2o3-kfd", "-p",
               "fe2o3-runtime", "--all-features", "--all-targets", "--", "-D", "warnings"],
    "formatting": ["cargo", "fmt", "--all", "--", "--check"],
    "no-default": ["cargo", "check", "--locked", "--offline", "-p", "fe2o3-runtime",
                   "--no-default-features"],
    "unsafe-policy": ["cargo", "test", "--locked", "--offline", "-p", "cargo-fe2o3",
                      "--test", "unsafe_source_policy"],
}
for name, expected in other_commands.items():
    command(name, expected)
source_names = [line.split("  ", 1)[1]
                for line in (ARCHIVE / "source-files.sha256").read_text().splitlines()]
require(len(source_names) == 18 and len(set(source_names)) == 18, "wrong source inventory")
for name in ("source-before", "source-after"):
    require(read(name + ".log").splitlines() == [path + ": OK" for path in source_names],
            f"{name}: source-check log mismatch")

for target in TARGETS:
    build = ["cargo", "test", "--locked", "--offline", "-p", "fe2o3-kfd", "-p",
             "fe2o3-runtime", "--all-features", "--lib"]
    target_dir = ROOT / "target"
    if target == "musl":
        build += ["--target", "x86_64-unknown-linux-musl"]
        target_dir /= "x86_64-unknown-linux-musl"
    command(target + "-build", build + ["--no-run", "--message-format=json"])
    artifacts = []
    for line in read(target + "-build.log").splitlines():
        try:
            item = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(item, dict) and item.get("reason") == "compiler-artifact":
            artifacts.append(item)
    for crate in CRATES:
        name = f"{target}-{crate}"
        matches = [item for item in artifacts
                   if item["target"]["name"] == f"fe2o3_{crate}" and item["profile"]["test"]]
        require(len(matches) == 1, f"{name}: ambiguous or missing compiler artifact")
        item = matches[0]
        crate_dir = ROOT / "crates" / f"fe2o3-{crate}"
        require(item["manifest_path"] == str(crate_dir / "Cargo.toml")
                and item["package_id"].startswith("path+" + crate_dir.as_uri() + "#")
                and item["target"]["src_path"] == str(crate_dir / "src/lib.rs")
                and item["target"]["kind"] == ["lib"]
                and item["target"]["crate_types"] == ["lib"]
                and item["target"]["test"] is True
                and item["features"] == FEATURES[crate], f"{name}: wrong artifact identity")
        executable = Path(item["executable"])
        require(executable.parent == target_dir / "debug/deps"
                and str(executable) in item["filenames"], f"{name}: wrong target executable")
        receipt = read(name + "-binary.sha256").splitlines()
        require(len(receipt) == 1, f"{name}: invalid binary receipt")
        digest, path = receipt[0].split("  ", 1)
        require(path == str(executable), f"{name}: binary receipt path mismatch")
        with executable.open("rb") as binary:
            actual = hashlib.file_digest(binary, "sha256").hexdigest()
        require(digest == actual, f"{name}: binary changed after execution")
        command(name + "-roster", [str(executable), "--list"])
        command(name + "-full", ["prlimit", "--core=0:0", "--", str(executable), "--test-threads=4"])
        names = roster(name + "-roster")
        results = read(name + "-full.log")
        rows = re.findall(r"^test (\S+) \.\.\. (ok|ignored)(?:, [^\n]*)?$", results, re.MULTILINE)
        status_lines = re.findall(r"^test \S+ \.\.\..*$", results, re.MULTILINE)
        require(len(status_lines) == len(rows), f"{name}: unrecognized test status")
        require("... FAILED" not in results, f"{name}: failed test row")
        require(len(rows) == len(names) and {row[0] for row in rows} == names,
                f"{name}: missing, duplicate or unexpected result")
        statuses = dict(rows)
        passed = sum(status == "ok" for status in statuses.values())
        ignored = len(statuses) - passed
        summaries = re.findall(r"^test result:.*$", results, re.MULTILINE)
        require(len(summaries) == 1 and re.fullmatch(
            rf"test result: ok\. {passed} passed; 0 failed; {ignored} ignored; "
            r"0 measured; 0 filtered out; finished in \d+(?:\.\d+)?s", summaries[0])
            and passed > 0, f"{name}: summary/result mismatch")
        if crate == "kfd":
            new = {test for test in names if "::auxiliary_release::tests::" in test
                   or "::auxiliary_cases::release_cases::" in test}
            require(len(new) == 17 and ignored == 0
                    and all(statuses[test] == "ok" for test in new), f"{name}: new-test mismatch")
        else:
            command(name + "-ignored", [str(executable), "--ignored", "--list"])
            ignored_names = roster(name + "-ignored")
            require(ignored_names == {test for test, status in rows if status == "ignored"}
                    and NATIVE_TEST in ignored_names and statuses.get(CPU_TEST) == "ok",
                    f"{name}: runtime coverage mismatch")
        print(f"{name}: {passed} passed, {ignored} ignored; artifact, roster and results match")
print("All 21 command records are complete and successful.")
