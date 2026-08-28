#!/usr/bin/env python3

from __future__ import annotations

import copy
import importlib.util
import json
from pathlib import Path
import sys
import unittest


sys.dont_write_bytecode = True
CHECKER_PATH = Path(__file__).resolve().parents[1] / "workspace_dependency_policy.py"
SPEC = importlib.util.spec_from_file_location("workspace_dependency_policy", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)
POLICY_PATH = Path(__file__).resolve().parents[1] / "workspace-dependency-policy.json"
COMPILER_CLOSURE_CAPABILITY = "fe2o3-compiler-closure-capability"
CANONICAL_CONTRACTS_LAYER = "canonical-contracts"


def checked_in_policy() -> dict:
    return json.loads(POLICY_PATH.read_text(encoding="utf-8"))


def package_owners(candidate: dict, package_name: str) -> list[str]:
    return [
        layer["name"]
        for layer in candidate["layers"]
        if package_name in layer["packages"]
    ]


def require_compiler_closure_capability_class(candidate: dict) -> None:
    owners = package_owners(candidate, COMPILER_CLOSURE_CAPABILITY)
    if owners != [CANONICAL_CONTRACTS_LAYER]:
        raise AssertionError(
            f"{COMPILER_CLOSURE_CAPABILITY} must be owned exactly by "
            f"{CANONICAL_CONTRACTS_LAYER}, found {owners!r}"
        )


def policy() -> dict:
    return {
        "schema_version": 1,
        "layers": [
            {"name": "contract", "packages": ["contract"]},
            {"name": "runtime", "packages": ["runtime"]},
            {"name": "fixture", "packages": []},
        ],
        "path_layers": [{"prefix": "examples/", "layer": "fixture"}],
        "allowed_dependency_edges": [],
        "forbidden_dependency_directions": [
            {"from": "contract", "to": ["runtime", "fixture"]},
            {"from": "runtime", "to": ["fixture"]},
        ],
    }


def package(name: str, relative_dir: str, dependencies: list[dict] | None = None) -> dict:
    return {
        "id": f"{name} 0.1.0 (path+file:///workspace/{relative_dir})",
        "name": name,
        "manifest_path": f"/workspace/{relative_dir}/Cargo.toml",
        "dependencies": dependencies or [],
    }


def dependency(name: str, relative_dir: str, kind: str | None = None) -> dict:
    return {
        "name": name,
        "path": f"/workspace/{relative_dir}",
        "kind": kind,
        "target": None,
    }


def metadata(packages: list[dict]) -> dict:
    return {
        "workspace_root": "/workspace",
        "workspace_members": [entry["id"] for entry in packages],
        "packages": packages,
    }


class WorkspaceDependencyPolicyTests(unittest.TestCase):
    def test_checked_in_compiler_closure_capability_is_canonical_contract(self) -> None:
        require_compiler_closure_capability_class(checked_in_policy())

    def test_rejects_removed_compiler_closure_capability_classification(self) -> None:
        hostile = copy.deepcopy(checked_in_policy())
        canonical = next(
            layer
            for layer in hostile["layers"]
            if layer["name"] == CANONICAL_CONTRACTS_LAYER
        )
        canonical["packages"].remove(COMPILER_CLOSURE_CAPABILITY)

        with self.assertRaisesRegex(AssertionError, "must be owned exactly"):
            require_compiler_closure_capability_class(hostile)
        violations, _ = CHECKER.check_policy(
            metadata(
                [
                    package(
                        COMPILER_CLOSURE_CAPABILITY,
                        "crates/fe2o3-compiler-closure-capability",
                    )
                ]
            ),
            hostile,
        )
        self.assertEqual(
            [
                "unclassified workspace member: "
                "fe2o3-compiler-closure-capability "
                "(crates/fe2o3-compiler-closure-capability/Cargo.toml)"
            ],
            violations,
        )

    def test_rejects_every_noncanonical_compiler_closure_capability_class(self) -> None:
        baseline = checked_in_policy()
        noncanonical_layers = [
            layer["name"]
            for layer in baseline["layers"]
            if layer["name"] != CANONICAL_CONTRACTS_LAYER
        ]
        self.assertEqual(7, len(noncanonical_layers))

        for hostile_layer in noncanonical_layers:
            with self.subTest(layer=hostile_layer):
                hostile = copy.deepcopy(baseline)
                for layer in hostile["layers"]:
                    if layer["name"] == CANONICAL_CONTRACTS_LAYER:
                        layer["packages"].remove(COMPILER_CLOSURE_CAPABILITY)
                    elif layer["name"] == hostile_layer:
                        layer["packages"].append(COMPILER_CLOSURE_CAPABILITY)
                with self.assertRaisesRegex(AssertionError, "must be owned exactly"):
                    require_compiler_closure_capability_class(hostile)

    def test_rejects_duplicate_compiler_closure_capability_ownership(self) -> None:
        hostile = copy.deepcopy(checked_in_policy())
        next(
            layer for layer in hostile["layers"] if layer["name"] == "host-runtime"
        )["packages"].append(COMPILER_CLOSURE_CAPABILITY)

        with self.assertRaisesRegex(
            CHECKER.PolicyConfigurationError, "assigned to both"
        ):
            CHECKER.check_policy(metadata([]), hostile)

    def test_allows_dependencies_toward_contracts(self) -> None:
        packages = [
            package("contract", "crates/contract"),
            package(
                "runtime",
                "crates/runtime",
                [dependency("contract", "crates/contract")],
            ),
        ]
        violations, stats = CHECKER.check_policy(metadata(packages), policy())
        self.assertEqual([], violations)
        self.assertEqual(2, stats["workspace_members"])
        self.assertEqual(1, stats["internal_dependencies"])

    def test_rejects_forbidden_direction_for_every_dependency_kind(self) -> None:
        packages = [
            package(
                "contract",
                "crates/contract",
                [
                    dependency("runtime", "crates/runtime"),
                    dependency("runtime", "crates/runtime", "build"),
                    dependency("runtime", "crates/runtime", "dev"),
                ],
            ),
            package("runtime", "crates/runtime"),
        ]
        violations, _ = CHECKER.check_policy(metadata(packages), policy())
        self.assertEqual(3, len(violations))
        self.assertEqual(sorted(violations), violations)
        self.assertTrue(any("(normal;" in violation for violation in violations))
        self.assertTrue(any("(build;" in violation for violation in violations))
        self.assertTrue(any("(dev;" in violation for violation in violations))

    def test_classifies_explicit_fixture_path(self) -> None:
        packages = [package("tutorial", "examples/tutorial")]
        violations, _ = CHECKER.check_policy(metadata(packages), policy())
        self.assertEqual([], violations)

    def test_rejects_unclassified_workspace_member(self) -> None:
        packages = [package("unowned", "crates/unowned")]
        violations, _ = CHECKER.check_policy(metadata(packages), policy())
        self.assertEqual(
            ["unclassified workspace member: unowned (crates/unowned/Cargo.toml)"],
            violations,
        )

    def test_allows_only_the_exact_package_and_dependency_kind_exception(self) -> None:
        reviewed = policy()
        reviewed["allowed_dependency_edges"] = [
            {"from": "contract", "to": "runtime", "kinds": ["normal"]}
        ]
        packages = [
            package(
                "contract",
                "crates/contract",
                [
                    dependency("runtime", "crates/runtime"),
                    dependency("runtime", "crates/runtime", "dev"),
                ],
            ),
            package("runtime", "crates/runtime"),
        ]
        violations, _ = CHECKER.check_policy(metadata(packages), reviewed)
        self.assertEqual(1, len(violations))
        self.assertIn("(dev;", violations[0])

    def test_rejects_exception_that_does_not_cross_a_forbidden_direction(self) -> None:
        invalid = policy()
        invalid["allowed_dependency_edges"] = [
            {"from": "runtime", "to": "contract", "kinds": ["normal"]}
        ]
        with self.assertRaisesRegex(
            CHECKER.PolicyConfigurationError, "does not cross a forbidden layer direction"
        ):
            CHECKER.check_policy(metadata([]), invalid)

    def test_rejects_duplicate_package_ownership(self) -> None:
        invalid = policy()
        invalid["layers"][1]["packages"].append("contract")
        with self.assertRaisesRegex(
            CHECKER.PolicyConfigurationError, "assigned to both"
        ):
            CHECKER.check_policy(metadata([]), invalid)

    def test_rejects_unknown_layer_in_forbidden_direction(self) -> None:
        invalid = policy()
        invalid["forbidden_dependency_directions"].append(
            {"from": "missing", "to": ["contract"]}
        )
        with self.assertRaisesRegex(
            CHECKER.PolicyConfigurationError, "unknown source layer"
        ):
            CHECKER.check_policy(metadata([]), invalid)


if __name__ == "__main__":
    unittest.main()
