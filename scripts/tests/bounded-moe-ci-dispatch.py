#!/usr/bin/env python3
"""Regression checks for the bounded MoE generic-CI dispatch contract."""

from __future__ import annotations

import importlib.util
import subprocess
from pathlib import Path
from types import ModuleType


ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts/test-bounded-moe-docs.py"
CI_LOCAL = ROOT / "scripts/ci-local.sh"
DOCS_COMMAND = (
    "  run_step bounded-moe-docs \\\n"
    "    python3 scripts/test-bounded-moe-docs.py"
)
HARNESS = r'''
set -Eeuo pipefail
source "$1"

run_step() {
  printf '%s' "$1"
  shift
  printf '\t%s' "$@"
  printf '\n'
}
run_shard_policy() { :; }
run_parity_matrix_checks() { :; }
run_format() { :; }
run_check() { :; }
run_backend_build() { :; }
run_cpu_tests() { :; }
run_rustc_codegen_lib_tests() { :; }
run_auxiliary_tests() { :; }
run_all_rustc_codegen_shards() { :; }

"$2"
'''


def load_checker() -> ModuleType:
    spec = importlib.util.spec_from_file_location("bounded_moe_docs", CHECKER)
    if spec is None or spec.loader is None:
        raise SystemExit("cannot load bounded MoE documentation checker")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def replace_once(source: str, old: str, new: str) -> str:
    if source.count(old) != 1:
        raise SystemExit(f"non-unique mutation anchor: {old!r}")
    return source.replace(old, new, 1)


def expect_rejected(checker: ModuleType, label: str, source: str) -> None:
    try:
        checker.validate_ci_dispatch(source)
    except SystemExit:
        return
    raise SystemExit(f"dispatch mutation unexpectedly passed: {label}")


def observed_steps(function: str) -> list[str]:
    result = subprocess.run(
        ["bash", "-c", HARNESS, "bash", str(CI_LOCAL), function],
        cwd=ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode != 0:
        raise SystemExit(
            f"instrumented {function} failed with {result.returncode}:\n{result.stderr}"
        )
    return result.stdout.splitlines()


def main() -> None:
    checker = load_checker()
    production = CI_LOCAL.read_text(encoding="utf-8")
    checker.validate_ci_dispatch(production)

    expect_rejected(
        checker,
        "missing command",
        replace_once(production, f"{DOCS_COMMAND}\n", ""),
    )
    expect_rejected(
        checker,
        "duplicate command",
        replace_once(production, DOCS_COMMAND, f"{DOCS_COMMAND}\n{DOCS_COMMAND}"),
    )
    expect_rejected(
        checker,
        "command moved to generic",
        replace_once(
            replace_once(production, f"{DOCS_COMMAND}\n", ""),
            "run_generic() {\n",
            f"run_generic() {{\n{DOCS_COMMAND}\n",
        ),
    )
    expect_rejected(
        checker,
        "generic delegation removed",
        replace_once(production, "run_generic() {\n  run_generic_core\n", "run_generic() {\n"),
    )
    expect_rejected(
        checker,
        "generic delegation duplicated",
        replace_once(
            production,
            "run_generic() {\n  run_generic_core\n",
            "run_generic() {\n  run_generic_core\n  run_generic_core\n",
        ),
    )
    intervening_owner = replace_once(production, f"{DOCS_COMMAND}\n", "")
    intervening_owner = replace_once(
        intervening_owner,
        "run_generic_core() {\n",
        "run_generic_core() {\n  run_step bounded-moe-docs true\n",
    )
    intervening_owner = replace_once(
        intervening_owner,
        "\n}\n\nrun_generic() {",
        f"\n}}\n\naudit_intervening_owner() {{\n{DOCS_COMMAND}\n}}\n\nrun_generic() {{",
    )
    expect_rejected(checker, "intervening command owner", intervening_owner)

    intervening_delegate = replace_once(
        production,
        "run_generic() {\n  run_generic_core\n",
        "run_generic() {\n  run_step bounded-moe-docs true\n",
    )
    intervening_delegate = replace_once(
        intervening_delegate,
        "\n}\n\nrun_rocm_compile() {",
        "\n}\n\naudit_intervening_delegate() {\n"
        "  run_generic_core\n}\n\nrun_rocm_compile() {",
    )
    expect_rejected(checker, "intervening generic delegate", intervening_delegate)

    expected = "bounded-moe-docs\tpython3\tscripts/test-bounded-moe-docs.py"
    for function in ["run_generic_core", "run_generic"]:
        steps = observed_steps(function)
        count = steps.count(expected)
        if count != 1:
            raise SystemExit(
                f"{function} dispatched the exact bounded-MoE command {count} times: {steps}"
            )
        wrong = [step for step in steps if step.startswith("bounded-moe-docs\t") and step != expected]
        if wrong:
            raise SystemExit(f"{function} dispatched drifted bounded-MoE commands: {wrong}")

    print("bounded MoE generic-CI dispatch regression passed")


if __name__ == "__main__":
    main()
