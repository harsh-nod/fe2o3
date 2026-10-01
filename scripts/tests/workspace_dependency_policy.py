#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import sys
import tomllib
import unittest


sys.dont_write_bytecode = True
CHECKER_PATH = Path(__file__).resolve().parents[1] / "workspace_dependency_policy.py"
SPEC = importlib.util.spec_from_file_location("workspace_dependency_policy", CHECKER_PATH)
assert SPEC is not None and SPEC.loader is not None
CHECKER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CHECKER)


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

    def test_production_simulator_oracle_exception_is_exact_and_dev_only(self) -> None:
        reviewed = json.loads(CHECKER.DEFAULT_POLICY.read_text(encoding="utf-8"))
        for source, target, kind, allowed in [
            ("fe2o3-kernel-opt", "fe2o3-kir-sim", "dev", True),
            ("fe2o3-kernel-opt", "fe2o3-kir-sim", None, False),
            ("fe2o3-kernel-opt", "fe2o3-kir-sim", "build", False),
            ("fe2o3-kernel-opt", "fe2o3-runtime", "dev", False),
            ("fe2o3-lower-mir-kernel", "fe2o3-kir-sim", "dev", True),
            ("fe2o3-lower-mir-kernel", "fe2o3-kir-sim", None, False),
            ("fe2o3-lower-mir-kernel", "fe2o3-kir-sim", "build", False),
            ("fe2o3-lower-mir-kernel", "fe2o3-runtime", "dev", False),
            ("fe2o3-pliron", "fe2o3-kir-sim", "dev", False),
        ]:
            with self.subTest(source=source, target=target, kind=kind):
                packages = [
                    package(
                        source,
                        f"crates/{source}",
                        [dependency(target, f"crates/{target}", kind)],
                    ),
                    package(target, f"crates/{target}"),
                ]
                violations, stats = CHECKER.check_policy(metadata(packages), reviewed)
                expected = [] if allowed else [
                    "forbidden dependency: "
                    f"{source} [pliron-framework] -> {target} [host-runtime] "
                    f"({kind or 'normal'}; crates/{source}/Cargo.toml)"
                ]
                self.assertEqual(expected, violations)
                self.assertEqual(1, stats["internal_dependencies"])

    def test_final_f_verifier_exceptions_are_exact_and_kind_scoped(self) -> None:
        reviewed = json.loads(CHECKER.DEFAULT_POLICY.read_text(encoding="utf-8"))
        edges = ["fe2o3-kernel-opt", "fe2o3-kernel-analysis", "fe2o3-amdgcn-model"]
        cases = [("fe2o3-verifier", target, kind, kind is None or
                  (target == "fe2o3-kernel-opt" and kind == "dev"))
                 for target in edges for kind in (None, "dev", "build")]
        cases += [("fe2o3-verifier", target, None, False) for target in (
            "rustc-codegen-fe2o3", "fe2o3-runtime", "fe2o3-kir-sim")]
        cases += [("fe2o3-mir-model", target, None, False) for target in edges]
        for source, target, kind, allowed in cases:
            with self.subTest(source=source, target=target, kind=kind):
                packages = [package(source, f"crates/{source}", [
                    dependency(target, f"crates/{target}", kind)]),
                    package(target, f"crates/{target}")]
                violations, stats = CHECKER.check_policy(metadata(packages), reviewed)
                self.assertEqual(0 if allowed else 1, len(violations))
                self.assertEqual(1, stats["internal_dependencies"])
                if not allowed:
                    self.assertIn(f"{source} [", violations[0])
                    self.assertIn(f"-> {target} [", violations[0])
                    self.assertIn(f"({kind or 'normal'};", violations[0])

    def test_verifier_optimizer_test_edge_matches_declared_manifest(self) -> None:
        root = CHECKER_PATH.parents[1]
        manifest = tomllib.loads(
            (root / "crates/fe2o3-verifier/Cargo.toml").read_text(encoding="utf-8")
        )
        self.assertEqual(
            {"workspace": True, "features": ["test-support"]},
            manifest["dev-dependencies"]["fe2o3-kernel-opt"],
        )
        reviewed = json.loads(CHECKER.DEFAULT_POLICY.read_text(encoding="utf-8"))
        edges = [row for row in reviewed["allowed_dependency_edges"]
                 if row["from"] == "fe2o3-verifier" and
                 row["to"] == "fe2o3-kernel-opt"]
        self.assertEqual([{"from": "fe2o3-verifier", "to": "fe2o3-kernel-opt",
                           "kinds": ["normal", "dev"]}], edges)

    def test_verifier_helper_exceptions_are_exact_and_normal_only(self) -> None:
        reviewed = json.loads(CHECKER.DEFAULT_POLICY.read_text(encoding="utf-8"))
        condition = 'cfg(all(target_os = "linux", target_arch = "x86_64"))'
        helpers = ("fe2o3-protected-service-spawn", "fe2o3-protected-static-executable")
        cases = [("fe2o3-verifier", target, kind, kind is None)
                 for target in helpers for kind in (None, "dev", "build")]
        cases += [(source, target, kind, False)
                  for source in ("fe2o3-service-verus", "fe2o3-mir-model",
                                 "fe2o3-lower-mir-kernel")
                  for target in helpers for kind in (None, "dev", "build")]
        cases += [("fe2o3-verifier", target, kind, False)
                  for target in ("fe2o3-runtime", "fe2o3-host", "fe2o3-kfd",
                                 "fe2o3-hsa-runtime")
                  for kind in (None, "dev", "build")]
        layers = {name: row["name"] for row in reviewed["layers"]
                  for name in row["packages"]}
        for helper in helpers:
            self.assertEqual("host-runtime", layers[helper])
        for source, target, kind, allowed in cases:
            with self.subTest(source=source, target=target, kind=kind):
                edge = dependency(target, f"crates/{target}", kind)
                edge["target"] = condition
                packages = [package(source, f"crates/{source}", [edge]),
                    package(target, f"crates/{target}")]
                violations, stats = CHECKER.check_policy(metadata(packages), reviewed)
                expected = [] if allowed else [
                    "forbidden dependency: "
                    f"{source} [{layers[source]}] -> {target} [host-runtime] "
                    f"({kind or 'normal'}, target {condition}; crates/{source}/Cargo.toml)"
                ]
                self.assertEqual(expected, violations)
                self.assertEqual(1, stats["internal_dependencies"])

    def test_verifier_helper_edges_match_only_the_linux_x86_64_manifest(self) -> None:
        root = CHECKER_PATH.parents[1]
        manifest = tomllib.loads(
            (root / "crates/fe2o3-verifier/Cargo.toml").read_text(encoding="utf-8")
        )
        helpers = ("fe2o3-protected-service-spawn", "fe2o3-protected-static-executable")
        helper_target = 'cfg(all(target_os = "linux", target_arch = "x86_64"))'
        tables = [(None, manifest), *manifest.get("target", {}).items()]
        for helper in helpers:
            declared = [(target, kind, table[kind][helper])
                        for target, table in tables
                        for kind in ("dependencies", "dev-dependencies", "build-dependencies")
                        if helper in table.get(kind, {})]
            self.assertEqual(
                [(helper_target, "dependencies", {"workspace": True})], declared
            )
        reviewed = json.loads(CHECKER.DEFAULT_POLICY.read_text(encoding="utf-8"))
        host_packages = next(row["packages"] for row in reviewed["layers"]
                             if row["name"] == "host-runtime")
        edges = [row for row in reviewed["allowed_dependency_edges"]
                 if row["from"] == "fe2o3-verifier" and row["to"] in host_packages]
        self.assertEqual(
            [{"from": "fe2o3-verifier", "to": helper, "kinds": ["normal"],
              "target": helper_target}
             for helper in helpers], edges
        )

    def test_static_executable_format_is_a_pure_contract_without_host_edges(self) -> None:
        root = CHECKER_PATH.parents[1]
        name = "fe2o3-static-executable-format"
        manifest = tomllib.loads(
            (root / f"crates/{name}/Cargo.toml").read_text(encoding="utf-8")
        )
        self.assertEqual({"sha2": {"workspace": True}}, manifest["dependencies"])
        for table in ("dev-dependencies", "build-dependencies", "target"):
            self.assertEqual({}, manifest.get(table, {}))
        reviewed = json.loads(CHECKER.DEFAULT_POLICY.read_text(encoding="utf-8"))
        self.assertEqual(["canonical-contracts"], [row["name"]
                         for row in reviewed["layers"] if name in row["packages"]])
        consumers = ("fe2o3-verifier", "fe2o3-protected-static-executable")
        for consumer in consumers:
            packages = [package(consumer, f"crates/{consumer}", [
                dependency(name, f"crates/{name}")]), package(name, f"crates/{name}")]
            violations, _ = CHECKER.check_policy(metadata(packages), reviewed)
            self.assertEqual([], violations)
        for target in ("fe2o3-runtime", "fe2o3-protected-service-spawn",
                       "fe2o3-protected-static-executable"):
            for kind in (None, "dev", "build"):
                with self.subTest(target=target, kind=kind):
                    packages = [package(name, f"crates/{name}", [
                        dependency(target, f"crates/{target}", kind)]),
                        package(target, f"crates/{target}")]
                    violations, _ = CHECKER.check_policy(metadata(packages), reviewed)
                    self.assertEqual([
                        "forbidden dependency: "
                        f"{name} [canonical-contracts] -> {target} [host-runtime] "
                        f"({kind or 'normal'}; crates/{name}/Cargo.toml)"
                    ], violations)

    def test_rejects_exception_that_does_not_cross_a_forbidden_direction(self) -> None:
        invalid = policy()
        invalid["allowed_dependency_edges"] = [
            {"from": "runtime", "to": "contract", "kinds": ["normal"]}
        ]
        with self.assertRaisesRegex(
            CHECKER.PolicyConfigurationError, "does not cross a forbidden layer direction"
        ):
            CHECKER.check_policy(metadata([]), invalid)

    def test_target_scoped_exception_requires_exact_target_and_kind(self) -> None:
        reviewed = policy()
        reviewed["schema_version"] = 2
        condition = 'cfg(all(target_os = "linux", target_arch = "x86_64"))'
        reviewed["allowed_dependency_edges"] = [
            {"from": "contract", "to": "runtime", "kinds": ["normal"],
             "target": condition}
        ]
        for target in (condition, None, 'cfg(target_os = "linux")',
                       'cfg(target_os = "windows")'):
            for kind in (None, "build", "dev"):
                with self.subTest(target=target, kind=kind):
                    edge = dependency("runtime", "crates/runtime", kind)
                    edge["target"] = target
                    packages = [package("contract", "crates/contract", [edge]),
                                package("runtime", "crates/runtime")]
                    violations, _ = CHECKER.check_policy(metadata(packages), reviewed)
                    self.assertEqual(0 if target == condition and kind is None else 1,
                                     len(violations))

    def test_rejects_malformed_or_legacy_target_scoped_exception(self) -> None:
        for target in (None, "", 7, [], {}):
            with self.subTest(target=target):
                reviewed = policy()
                reviewed["schema_version"] = 2
                reviewed["allowed_dependency_edges"] = [
                    {"from": "contract", "to": "runtime", "kinds": ["normal"],
                     "target": target}
                ]
                with self.assertRaises(CHECKER.PolicyConfigurationError):
                    CHECKER.check_policy(metadata([]), reviewed)
        reviewed["schema_version"] = 1
        reviewed["allowed_dependency_edges"][0]["target"] = 'cfg(target_os = "linux")'
        with self.assertRaisesRegex(CHECKER.PolicyConfigurationError, "schema_version 2"):
            CHECKER.check_policy(metadata([]), reviewed)

    def test_unrestricted_target_rules_keep_legacy_semantics(self) -> None:
        for version in (1, 2):
            for target in (None, 'cfg(target_os = "linux")', 'cfg(target_os = "windows")'):
                with self.subTest(version=version, target=target):
                    reviewed = policy()
                    reviewed["schema_version"] = version
                    reviewed["allowed_dependency_edges"] = [
                        {"from": "contract", "to": "runtime", "kinds": ["normal"]}
                    ]
                    edge = dependency("runtime", "crates/runtime")
                    edge["target"] = target
                    packages = [package("contract", "crates/contract", [edge]),
                                package("runtime", "crates/runtime")]
                    violations, _ = CHECKER.check_policy(metadata(packages), reviewed)
                    self.assertEqual([], violations)

    def test_rejects_duplicate_and_misspelled_target_rules(self) -> None:
        reviewed = policy()
        reviewed["schema_version"] = 2
        edge = {"from": "contract", "to": "runtime", "kinds": ["normal"],
                "target": 'cfg(target_os = "linux")'}
        reviewed["allowed_dependency_edges"] = [edge, edge.copy()]
        with self.assertRaisesRegex(CHECKER.PolicyConfigurationError, "duplicate allowed"):
            CHECKER.check_policy(metadata([]), reviewed)
        reviewed["allowed_dependency_edges"] = [dict(edge, targets=edge["target"])]
        with self.assertRaisesRegex(CHECKER.PolicyConfigurationError, "unknown fields"):
            CHECKER.check_policy(metadata([]), reviewed)

    def test_proof_helper_runtime_edges_match_only_guarded_manifest_dependencies(self) -> None:
        reviewed = json.loads(CHECKER.DEFAULT_POLICY.read_text(encoding="utf-8"))
        condition = 'cfg(all(target_os = "linux", target_arch = "x86_64"))'
        targets = ("fe2o3-protected-service-spawn", "fe2o3-protected-static-executable")
        manifest = tomllib.loads(
            (CHECKER_PATH.parents[1] / "crates/fe2o3-verifier/Cargo.toml")
            .read_text(encoding="utf-8")
        )
        for target in targets:
            self.assertEqual({"workspace": True}, manifest["target"][condition]["dependencies"][target])
            self.assertNotIn(target, manifest["dependencies"])
            rules = [row for row in reviewed["allowed_dependency_edges"]
                     if row["from"] == "fe2o3-verifier" and row["to"] == target]
            self.assertEqual([{"from": "fe2o3-verifier", "to": target,
                               "kinds": ["normal"], "target": condition}], rules)
        for source in ("fe2o3-verifier", "fe2o3-kernel-ir"):
            for target in (*targets, "fe2o3-runtime"):
                for guard in (condition, None, 'cfg(target_os = "windows")'):
                    for kind in (None, "dev", "build"):
                        with self.subTest(source=source, target=target, guard=guard, kind=kind):
                            edge = dependency(target, f"crates/{target}", kind)
                            edge["target"] = guard
                            packages = [package(source, f"crates/{source}", [edge]),
                                        package(target, f"crates/{target}")]
                            violations, _ = CHECKER.check_policy(metadata(packages), reviewed)
                            allowed = (source == "fe2o3-verifier" and target in targets
                                       and guard == condition and kind is None)
                            self.assertEqual(0 if allowed else 1, len(violations))

    def test_static_executable_format_is_a_canonical_contract(self) -> None:
        reviewed = json.loads(CHECKER.DEFAULT_POLICY.read_text(encoding="utf-8"))
        packages = [package("fe2o3-static-executable-format",
                            "crates/fe2o3-static-executable-format", [
                                dependency("fe2o3-runtime", "crates/fe2o3-runtime")]),
                    package("fe2o3-runtime", "crates/fe2o3-runtime")]
        violations, _ = CHECKER.check_policy(metadata(packages), reviewed)
        self.assertEqual(1, len(violations))
        self.assertIn("fe2o3-static-executable-format [canonical-contracts]", violations[0])

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
