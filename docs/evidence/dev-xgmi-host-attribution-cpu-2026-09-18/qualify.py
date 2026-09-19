#!/usr/bin/env python3
"""Record one non-overwriting CPU qualification of XGMI host attribution."""

import importlib.util
from pathlib import Path
import signal
import sys

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[2]
RUNNER = ROOT / "docs/evidence/dev-xgmi-settled-mi300x-2026-09-18/native.py"
SELECTOR = ROOT / "docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py"
spec = importlib.util.spec_from_file_location("xgmi_recorder", RUNNER)
N = importlib.util.module_from_spec(spec)
spec.loader.exec_module(N)
ENV = [
    "env",
    "CARGO_INCREMENTAL=0",
    "CARGO_PROFILE_DEV_DEBUG=0",
    "CARGO_PROFILE_TEST_DEBUG=0",
    "CARGO_BUILD_JOBS=2",
    "CARGO_TERM_COLOR=never",
    "RUST_TEST_THREADS=1",
    "CARGO_PROFILE_TEST_OPT_LEVEL=1",
    "CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true",
    "CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true",
]
KFD_FILTERS = [
    "sdma::xgmi_diagnostic::tests::",
    "sdma::xgmi_creation::tests::",
    "sdma::tests::xgmi_",
    "sdma::tests::creation_guards_cover_the_first_memory_operation_and_xgmi_route_scope",
    "sdma::tests::sdma_copy_manifest_digest_is_frozen",
    "queue_linux::tests::terminal_creation_arm_poisons_on_drop_or_unwind_and_disarms_only_on_success",
    "queue::live::construction_primary::integration_tests::release_cases::sdma_creation_cases::",
]
RUNTIME_FILTERS = [
    "kfd_backend::tests::native_xgmi_",
    "kfd_backend::xgmi_diagnostic::tests::",
]


def commands():
    result = [
        ("source-before", ["python3", "-I", str(SELECTOR)], 60),
        ("rustc", ["rustc", "-vV"], 30),
        ("cargo", ["cargo", "-V"], 30),
        (
            "clippy",
            ENV
            + [
                "cargo",
                "clippy",
                "--frozen",
                "-p",
                "fe2o3-kfd",
                "-p",
                "fe2o3-runtime",
                "--all-features",
                "--all-targets",
                "--",
                "-D",
                "warnings",
            ],
            1200,
        ),
        (
            "no-default",
            ENV
            + [
                "cargo",
                "check",
                "--frozen",
                "-p",
                "fe2o3-kfd",
                "-p",
                "fe2o3-runtime",
                "--no-default-features",
                "--all-targets",
            ],
            1200,
        ),
    ]
    for target in ("gnu", "musl"):
        target_args = (
            [] if target == "gnu" else ["--target", "x86_64-unknown-linux-musl"]
        )
        for package, filters in [("kfd", KFD_FILTERS), ("runtime", RUNTIME_FILTERS)]:
            base = ENV + [
                "cargo",
                "test",
                "--frozen",
                "-p",
                "fe2o3-" + package,
                "--all-features",
                *target_args,
                "--lib",
                "--",
            ]
            result.append(
                (target + "-" + package + "-roster", base + ["--list", *filters], 1800)
            )
            result.append((target + "-" + package, base + filters, 1800))
        result.append(
            (
                target + "-example",
                ENV
                + [
                    "cargo",
                    "test",
                    "--frozen",
                    "-p",
                    "fe2o3-runtime",
                    "--all-features",
                    *target_args,
                    "--example",
                    "gfx942-runtime-xgmi-peer-benchmark",
                ],
                1200,
            )
        )
    result.extend(
        [
            (
                "example-feature-off",
                ENV
                + [
                    "cargo",
                    "test",
                    "--frozen",
                    "-p",
                    "fe2o3-runtime",
                    "--no-default-features",
                    "--example",
                    "gfx942-runtime-xgmi-peer-benchmark",
                ],
                1200,
            ),
            (
                "unsafe-source",
                ENV
                + [
                    "cargo",
                    "test",
                    "--frozen",
                    "-p",
                    "cargo-fe2o3",
                    "--test",
                    "unsafe_source_policy",
                ],
                1200,
            ),
            ("fmt", ["cargo", "fmt", "--all", "--check"], 180),
            ("diff", ["git", "diff", "--check"], 60),
            ("source-after", ["python3", "-I", str(SELECTOR)], 60),
            (
                "source-unchanged",
                [
                    "cmp",
                    str(HERE / "raw/source-before/stdout"),
                    str(HERE / "raw/source-after/stdout"),
                ],
                30,
            ),
        ]
    )
    return result


def main():
    for number in N.MANAGED:
        signal.signal(number, N.interrupted)
    recorder = N.Recorder(HERE / "raw", ROOT)
    N.write_json(
        HERE / "tools.json",
        {
            str(path.relative_to(ROOT)): N.sha(path)
            for path in (RUNNER, SELECTOR, HERE / "qualify.py", HERE / "verify.py")
        },
    )
    for name, command, seconds in commands():
        recorder.run(name, command, seconds)


if __name__ == "__main__":
    main()
