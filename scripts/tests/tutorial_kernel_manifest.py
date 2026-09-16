#!/usr/bin/env python3
"""Source-contract and matrix enumeration tests; never production receipts."""

from __future__ import annotations

import copy
from collections import Counter
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts/validate-tutorial-kernel-manifest.py"
MANIFEST = ROOT / "config/tutorial-kernel-manifest-v1.json"


def load_validator():
    specification = importlib.util.spec_from_file_location("tutorial_contract", CHECKER)
    assert specification is not None and specification.loader is not None
    module = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(module)
    return module


def recovered_scope(manifest):
    fixtures = []
    for original in manifest["compilerFixtures"]:
        if original["fixtureId"] == "gfx942-scalar-gemm":
            continue
        fixture = {key: original[key] for key in ("fixtureId", "testId", "testPath", "target", "matrix")}
        fixture["compilerInput"] = {
            key: original["compilerInput"][key]
            for key in (
                "packageManifest", "sourcePaths", "cargoTarget", "defaultFeatures",
                "features", "kernelSymbols", "cargoLockPath",
            )
        }
        fixture["simulation"] = {key: value for key, value in original["simulation"].items() if key != "status"}
        fixtures.append(fixture)
    entries = copy.deepcopy(manifest["entries"])
    for entry in entries:
        entry["compilerFixtureIds"] = [
            value for value in entry["compilerFixtureIds"] if value != "gfx942-scalar-gemm"
        ]
    return {
        "productionContract": manifest["productionContract"],
        "fixtures": fixtures,
        "entries": entries,
        "suites": [
            {key: suite[key] for key in ("suiteId", "gate", "command", "coverage")}
            for suite in manifest["qualification"]["suites"]
            if suite["suiteId"] not in {"cpu-reference-scalar-gemm", "semantic-simulation-scalar-gemm"}
        ],
    }


class TutorialKernelSourceContractTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.validator = load_validator()
        cls.original = cls.validator.load_manifest(MANIFEST)

    def setUp(self):
        self.manifest = copy.deepcopy(self.original)

    def reject(self, pattern):
        with self.assertRaisesRegex(SystemExit, pattern):
            self.validator.validate_manifest(ROOT, self.manifest)

    def test_all_expected_source_contracts_match_and_are_not_receipts(self):
        with mock.patch.object(
            self.validator, "package_rust_sources", wraps=self.validator.package_rust_sources
        ) as walk, mock.patch.object(
            self.validator, "_validate_package_source_path", wraps=self.validator._validate_package_source_path
        ) as source_path, mock.patch.object(
            self.validator, "source_contains_ordinary_attributed_kernel", wraps=self.validator.source_contains_ordinary_attributed_kernel
        ) as source_scan:
            fixtures = self.validator.validate_manifest(ROOT, self.manifest)
        lessons = {entry["lessonId"]: len(entry["sourcePaths"]) for entry in self.manifest["entries"]}
        self.assertEqual(
            Counter(call.args[3] for call in source_path.call_args_list if call.args[3] in lessons),
            Counter(lessons),
        )
        self.assertEqual(source_scan.call_count, sum(lessons.values()))
        expected_packages = {
            f["compilerInput"]["packageManifest"] for f in self.manifest["compilerFixtures"]
        } | {entry["packageManifest"] for entry in self.manifest["entries"]}
        expected_packages.update(
            tab["sourceItem"]["compilerInput"]["packageManifest"]
            for lesson in self.manifest["curriculum"]["lessons"] for tab in lesson["codeTabs"]
            if tab["sourceItem"] is not None
        )
        self.assertEqual(
            Counter(call.args[1] for call in walk.call_args_list),
            Counter({package: 1 for package in expected_packages}),
        )
        self.assertEqual(len(fixtures), 48)
        self.assertEqual(sum(f["target"] == "gfx942" for f in fixtures.values()), 11)
        self.assertEqual(sum(f["target"] == "gfx950" for f in fixtures.values()), 37)
        self.assertEqual(len(self.manifest["entries"]), 25)
        self.assertEqual(len(self.manifest["qualification"]["suites"]), 60)
        self.assertTrue(all(e["classification"] == "compiler-produced" for e in self.manifest["entries"]))
        self.assertTrue(all(s["availability"] == "pending" for s in self.manifest["qualification"]["suites"]))

    def test_original_47_source_feature_symbol_and_semantic_obligations_are_preserved(self):
        payload = json.dumps(
            recovered_scope(self.manifest), sort_keys=True, separators=(",", ":"), ensure_ascii=True
        ).encode("ascii")
        self.assertEqual(hashlib.sha256(payload).hexdigest(), "1df009c40fff2d9e667fecfb4081cde24ce6aba228f44c8d0b05a3f645655e9f")

    def test_full_runtime_curriculum_snapshot_preserves_pending_obligations(self):
        curriculum = self.manifest["curriculum"]
        lessons = curriculum["lessons"]
        self.assertEqual(len(lessons), 56)
        self.assertEqual(Counter(lesson["role"] for lesson in lessons), {"executable": 46, "conceptual": 10})
        self.assertEqual(sum(len(lesson["codeTabs"]) for lesson in lessons), 306)
        self.assertEqual(
            [lesson["lessonId"] for lesson in lessons if any(v["kind"] == "mixed" for v in lesson["variants"])],
            ["reductions-scans", "gemm-tiling", "softmax-invariant"],
        )
        payload = json.dumps(curriculum, sort_keys=True, separators=(",", ":"), ensure_ascii=True).encode("ascii")
        self.assertEqual(hashlib.sha256(payload).hexdigest(), "81f46d6ad50a534979939a0b20b567253ceea23a1ab2eee0265b88e96c8dbe17")

    def test_legacy_manifests_remain_accepted_but_required_curriculum_cannot_be_omitted(self):
        del self.manifest["curriculum"]
        self.assertEqual(len(self.validator.validate_manifest(ROOT, self.manifest)), 48)
        with tempfile.TemporaryDirectory(prefix="fe2o3-curriculum-") as temporary:
            path = Path(temporary) / "legacy.json"
            path.write_text(json.dumps(self.manifest), encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(CHECKER), "--manifest", str(path), "--require-curriculum"],
                text=True, capture_output=True, check=False,
            )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("exhaustive curriculum extension is required", result.stderr)

    def validate_curriculum(self):
        self.validator.validate_curriculum(
            self.manifest["curriculum"],
            {entry["lessonId"]: entry for entry in self.manifest["entries"]},
            {fixture["fixtureId"]: fixture for fixture in self.manifest["compilerFixtures"]},
            ROOT,
        )

    def curriculum_lesson(self, lesson_id="first-fill"):
        return next(lesson for lesson in self.manifest["curriculum"]["lessons"] if lesson["lessonId"] == lesson_id)

    def test_curriculum_rejects_duplicate_unknown_and_detached_source_entries(self):
        for ids, pattern in ((["first-fill", "first-fill"], "duplicate"), (["absent"], "unknown"), (["typed-vecadd"], "detach")):
            with self.subTest(ids=ids):
                self.manifest = copy.deepcopy(self.original)
                self.curriculum_lesson()["sourceEntryIds"] = ids
                with self.assertRaisesRegex(SystemExit, pattern):
                    self.validate_curriculum()
        self.manifest = copy.deepcopy(self.original)
        self.manifest["curriculum"]["lessons"].append(copy.deepcopy(self.curriculum_lesson()))
        with self.assertRaisesRegex(SystemExit, "duplicate or invalid curriculum lessonId"):
            self.validate_curriculum()

    def test_conceptual_lessons_cannot_hide_existing_or_carry_new_runnable_obligations(self):
        self.curriculum_lesson()["role"] = "conceptual"
        with self.assertRaisesRegex(SystemExit, "cannot downgrade"):
            self.validate_curriculum()
        self.manifest = copy.deepcopy(self.original)
        self.curriculum_lesson("compiler-checks")["variants"] = copy.deepcopy(self.curriculum_lesson()["variants"])
        with self.assertRaisesRegex(SystemExit, "conceptual lesson cannot carry runnable obligations"):
            self.validate_curriculum()

    def test_variants_require_both_programming_modes_and_cannot_claim_implementation(self):
        for mutate, pattern in (
            (lambda variants: variants.pop(), "ordered SIMT/tile"),
            (lambda variants: variants.reverse(), "ordered SIMT/tile"),
            (lambda variants: variants[0].update(status="qualified"), "pending obligations"),
            (lambda variants: variants[0].update(sourceItems=[{"symbol": "invented"}]), "pending obligations"),
            (lambda variants: variants[0].pop("sourceItems"), "keys differ"),
        ):
            self.manifest = copy.deepcopy(self.original)
            mutate(self.curriculum_lesson()["variants"])
            with self.assertRaisesRegex(SystemExit, pattern):
                self.validate_curriculum()
        self.manifest = copy.deepcopy(self.original)
        for lesson in self.manifest["curriculum"]["lessons"]:
            lesson["variants"] = [variant for variant in lesson["variants"] if variant["kind"] != "mixed"]
        with self.assertRaisesRegex(SystemExit, "mixed SIMT/tile showcase"):
            self.validate_curriculum()

    def test_missing_new_source_association_remains_an_explicit_unqualified_gap(self):
        lesson = self.curriculum_lesson("gemm-autoresearch")
        lesson["sourceEntryIds"] = []
        with self.assertRaisesRegex(SystemExit, "sourceBindingGap"):
            self.validate_curriculum()
        lesson["sourceBindingGap"] = "An actual source fixture must be supplied; this is not implementation."
        self.validate_curriculum()

    def test_source_item_obligations_cannot_be_manufactured_or_dropped(self):
        for key, value in (("sourceItem", {"symbol": "fill"}), ("sourceItemStatus", "not-applicable")):
            self.manifest = copy.deepcopy(self.original)
            self.curriculum_lesson()["codeTabs"][0][key] = value
            with self.assertRaisesRegex(SystemExit, "source-item obligation"):
                self.validate_curriculum()

    def test_whole_file_metadata_and_tab_ordinals_remain_required(self):
        tab = next(tab for lesson in self.manifest["curriculum"]["lessons"] for tab in lesson["codeTabs"] if tab["sourceDigestScope"] == "file")
        tab["sourceCommit"] = None
        with self.assertRaisesRegex(SystemExit, "incomplete whole-file"):
            self.validate_curriculum()
        self.manifest = copy.deepcopy(self.original)
        self.curriculum_lesson()["codeTabs"][0]["ordinal"] = True
        with self.assertRaisesRegex(SystemExit, "ordered ordinal"):
            self.validate_curriculum()

    def test_malformed_enum_values_use_controlled_diagnostics(self):
        for key, value in (("kind", []), ("language", {}), ("sourceDigestScope", [])):
            self.manifest = copy.deepcopy(self.original)
            self.curriculum_lesson()["codeTabs"][0][key] = value
            with self.assertRaisesRegex(SystemExit, "must be a nonempty string"):
                self.validate_curriculum()
        self.manifest = copy.deepcopy(self.original)
        self.curriculum_lesson()["role"] = {}
        with self.assertRaisesRegex(SystemExit, "must be a nonempty string"):
            self.validate_curriculum()

    def test_new_aliases_require_selected_source_path_overlap_even_with_gap_text(self):
        for gap in (None, "A claimed gap must not authorize unrelated source entries."):
            self.manifest = copy.deepcopy(self.original)
            lesson = self.curriculum_lesson("gfx950-fp4-gemm-performance-lab")
            lesson["sourceEntryIds"] = ["typed-vecadd"]
            lesson["sourceBindingGap"] = gap
            with self.assertRaisesRegex(SystemExit, "no matching pinned kernel source path"):
                self.validate_curriculum()

    def test_selected_fixture_paths_support_aliases_without_package_widening(self):
        self.validate_curriculum()
        entry = next(entry for entry in self.manifest["entries"] if entry["lessonId"] == "gemm-tiling")
        self.assertNotIn("examples/gemm_autoresearch_v1/src/kernel.rs", entry["sourcePaths"])
        self.assertIn("gfx942-gemm-autoresearch", entry["compilerFixtureIds"])
        lesson = self.curriculum_lesson("gfx950-fp4-gemm-performance-lab")
        lesson["codeTabs"][0]["sourcePath"] = "examples/gfx950_low_precision/src/unselected.rs"
        with self.assertRaisesRegex(SystemExit, "no matching pinned kernel source path"):
            self.validate_curriculum()

    def test_existing_and_partially_matched_sources_report_exact_pending_paths(self):
        gaps = {}
        self.validator.validate_manifest(ROOT, self.manifest, curriculum_gaps=gaps)
        self.assertEqual(gaps, {
            "gemm-proof-plan": ["examples/tiled_gemm_v1/src/kernel.rs"],
            "gfx950-gpt-oss-120b-megakernel": [
                "examples/gfx950_gpt_oss_decode/src/kernel_pipelined_attention.rs",
                "examples/gfx950_gpt_oss_decode/src/kernel_scalar_attention.rs",
            ],
        })
        tab = self.curriculum_lesson("cpu-semantic-simulation")["codeTabs"][0]
        tab.update(sourceItem=None, sourceItemStatus="pending")
        with self.assertRaisesRegex(SystemExit, "sourceBindingGap.*production-ranked-bounds-device"):
            self.validate_curriculum()

    def test_unjustified_source_gap_claims_are_rejected(self):
        self.curriculum_lesson("gemm-autoresearch")["sourceBindingGap"] = "Not a real gap."
        with self.assertRaisesRegex(SystemExit, "fully linked paths"):
            self.validate_curriculum()

    def test_site_inventory_rejects_non_utf8_display_or_source_fragments(self):
        for key, value in (("displayedCode", "\ud800"), ("sourceFragments", ["\ud800"])):
            curriculum, inventory = self.site_inventory_fixture()
            inventory["lessons"][0]["codeTabs"][0][key] = value
            with self.assertRaisesRegex(SystemExit, "not valid UTF-8"):
                self.validator.validate_site_inventory(curriculum, inventory)

    def site_inventory_fixture(self):
        # Independent tiny rendered strings exercise hashing without retaining the full site.
        curriculum = copy.deepcopy(self.original["curriculum"])
        inventory = {"schema": self.validator.SITE_INVENTORY_SCHEMA, "site": copy.deepcopy(curriculum["site"]), "lessons": []}
        for lesson in curriculum["lessons"]:
            actual = {"id": lesson["lessonId"], "codeTabs": []}
            for tab in lesson["codeTabs"]:
                code = f"{lesson['lessonId']}:{tab['ordinal']}\n"
                tab["displayedUtf8Bytes"] = len(code.encode("utf-8"))
                tab["displayedSha256"] = hashlib.sha256(code.encode("utf-8")).hexdigest()
                tab["sourceFragmentsSha256"] = None
                if tab["sourceItem"] is not None:
                    tab.update(sourceItem=None, sourceItemStatus="pending")
                    lesson["sourceBindingGap"] = "Synthetic inventory has no source-driver contract."
                projected = {key: tab[key] for key in self.validator.CURRICULUM_TAB_FIELDS if key != "sourceFragmentsSha256"}
                projected.update(displayedCode=code, sourceFragments=None)
                actual["codeTabs"].append(projected)
            inventory["lessons"].append(actual)
        return curriculum, inventory

    def test_site_only_commits_do_not_create_a_circular_pin_dependency(self):
        curriculum, inventory = self.site_inventory_fixture()
        inventory["site"].update(commit="a" * 40, tree="b" * 40)
        self.validator.validate_site_inventory(curriculum, inventory)

    def test_site_inventory_rejects_missing_new_or_reordered_lessons(self):
        for mutate in (lambda rows: rows.pop(), lambda rows: rows.append(copy.deepcopy(rows[0])), lambda rows: rows.reverse()):
            curriculum, inventory = self.site_inventory_fixture()
            mutate(inventory["lessons"])
            with self.assertRaisesRegex(SystemExit, "ordered lesson coverage"):
                self.validator.validate_site_inventory(curriculum, inventory)

    def test_site_inventory_rejects_missing_new_and_reordered_tabs(self):
        for mutate in (lambda rows: rows.pop(), lambda rows: rows.append(copy.deepcopy(rows[0])), lambda rows: rows.reverse()):
            curriculum, inventory = self.site_inventory_fixture()
            mutate(inventory["lessons"][0]["codeTabs"])
            with self.assertRaisesRegex(SystemExit, "code-tab coverage|source/display binding"):
                self.validator.validate_site_inventory(curriculum, inventory)

    def test_site_inventory_checks_actual_rendered_bytes_and_exact_metadata(self):
        for key, value, pattern in (
            ("displayedCode", "changed\n", "displayed bytes do not match"),
            ("displayedSha256", "0" * 64, "displayed bytes do not match"),
            ("label", "renamed", "source/display binding"),
            ("sourceCommit", "a" * 40, "source/display binding"),
            ("sourcePath", "examples/absent.rs", "source/display binding"),
            ("sourceFragments", ["new fragment"], "source/display binding"),
            ("ordinal", False, "source/display binding"),
        ):
            curriculum, inventory = self.site_inventory_fixture()
            inventory["lessons"][0]["codeTabs"][0][key] = value
            with self.assertRaisesRegex(SystemExit, pattern):
                self.validator.validate_site_inventory(curriculum, inventory)

    def test_scalar_gemm_adds_compile_and_pending_oracle_obligations(self):
        fixture = next(f for f in self.manifest["compilerFixtures"] if f["fixtureId"] == "gfx942-scalar-gemm")
        self.assertEqual(fixture["compilerInput"]["kernelSymbols"], ["scalar_gemm_v1"])
        self.assertEqual(fixture["simulation"]["status"], "pending-design")
        self.assertIsNone(fixture["simulation"]["requestSha256"])
        for lesson in ("gemm-proof-plan", "gemm-tiling"):
            entry = next(e for e in self.manifest["entries"] if e["lessonId"] == lesson)
            self.assertIn(fixture["fixtureId"], entry["compilerFixtureIds"])

    def test_six_added_gfx942_library_symbol_and_feature_contracts(self):
        expected = {
            "gfx942-fill-simulation": ("fill", "fill", "fe2o3_fill", "fill", True, []),
            "gfx942-moe-top2": (
                "moe-top2", "moe_top2_v1", "fe2o3_moe_top2_v1",
                "moe_top2_route_f32_t8_e4_k2_c4_v1", True, [],
            ),
            "gfx942-scalar-gemm": (
                "scalar-gemm", "scalar_gemm_v1", "fe2o3_scalar_gemm_v1",
                "scalar_gemm_v1", True, [],
            ),
            "gfx942-typed-vecadd-source": (
                "typed-vecadd", "vecadd", "fe2o3_vecadd", "vecadd", True, [],
            ),
            "gfx942-wave64-collectives": (
                "wave64-collectives", "wave64_collectives_v1", "fe2o3_wave64_collectives_v1",
                "wave64_collectives_v1", True, [],
            ),
            "gfx942-workgroup-collectives": (
                "workgroup-collectives", "workgroup_sync_v1", "fe2o3_workgroup_sync_v1",
                "lds_publish_read_reduce_i32_v1", False, ["lds-kernel"],
            ),
        }
        fixtures = {fixture["fixtureId"]: fixture for fixture in self.manifest["compilerFixtures"]}
        for fixture_id, (case, package, crate, symbol, defaults, features) in expected.items():
            with self.subTest(fixture_id=fixture_id):
                fixture = fixtures[fixture_id]
                self.assertEqual(fixture["target"], "gfx942")
                self.assertEqual(fixture["testId"], f"kernel-compile-matrix/gfx942/{case}")
                self.assertEqual(fixture["matrix"], {
                    "caseId": case,
                    "runnerPath": f"examples/{package}/run-gfx942.sh",
                    "artifactName": f"{package}.hsaco",
                    "runnerArguments": [],
                    "environment": [],
                })
                inputs = fixture["compilerInput"]
                self.assertEqual(inputs["packageManifest"], f"examples/{package}/Cargo.toml")
                self.assertEqual(inputs["cargoTarget"], {
                    "kind": "lib", "name": crate, "sourcePath": "src/lib.rs",
                })
                self.assertEqual(inputs["kernelSymbols"], [symbol])
                self.assertEqual(inputs["defaultFeatures"], defaults)
                self.assertEqual(inputs["features"], features)


    def test_matrix_records_are_stable_and_do_not_execute(self):
        for target, count in (("gfx942", 11), ("gfx950", 37)):
            result = subprocess.run(
                [sys.executable, str(CHECKER), "--emit-matrix", target],
                check=True, text=True, capture_output=True,
            )
            records = [line.split("|") for line in result.stdout.splitlines()]
            self.assertEqual(len(records), count)
            self.assertTrue(all(len(record) == 6 for record in records))
            self.assertEqual([r[0] for r in records], sorted(r[0] for r in records))
            expected = sorted(
                (f for f in self.manifest["compilerFixtures"] if f["target"] == target),
                key=lambda f: f["matrix"]["caseId"],
            )
            self.assertEqual(result.stdout.splitlines(), [self.validator.matrix_record(f) for f in expected])

    def test_require_qualified_always_fails_closed(self):
        result = subprocess.run(
            [sys.executable, str(CHECKER), "--require-qualified"],
            text=True, capture_output=True, check=False,
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("qualification receipts and policy/final-graph evidence are not implemented", result.stderr)

    def test_release_schema_and_accepted_baseline_are_rejected(self):
        self.manifest["schema"] = "fe2o3-tutorial-kernel-manifest-v1"
        self.reject("source-contract schema")
        self.manifest = copy.deepcopy(self.original)
        self.manifest["baseline"]["compilerCommit"] = "a" * 40
        self.reject("accepted compiler baseline")

    def test_production_contract_cannot_allow_fallback_or_skip_final_verification(self):
        for key, value in (
            ("allowsFallback", True), ("allowsPipelineSelection", True),
            ("requiresFinalOptimizedGraphVerification", False), ("requiredPolicyVersion", 3),
        ):
            with self.subTest(key=key):
                self.manifest = copy.deepcopy(self.original)
                self.manifest["productionContract"][key] = value
                self.reject("production pipeline")

    def test_duplicates_dangling_scope_and_missing_simulation_are_rejected(self):
        self.manifest["compilerFixtures"].append(copy.deepcopy(self.manifest["compilerFixtures"][0]))
        self.reject("duplicate or invalid fixtureId")
        self.manifest = copy.deepcopy(self.original)
        self.manifest["entries"][0]["compilerFixtureIds"] = ["absent"]
        self.reject("no fixture|unknown fixture")
        self.manifest = copy.deepcopy(self.original)
        self.manifest["qualification"]["suites"] = [
            s for s in self.manifest["qualification"]["suites"] if s["suiteId"] != "semantic-simulation-fill"
        ]
        self.reject("missing pending semantic-simulation")

    def test_runnable_and_ordinary_source_entries_cannot_be_relabelled(self):
        self.manifest["entries"][0]["classification"] = "design-only"
        self.reject("cannot be downgraded")
        self.manifest = copy.deepcopy(self.original)
        entry = next(e for e in self.manifest["entries"] if e["lessonId"] == "gemm-proof-plan")
        entry["classification"] = "simulator-only"
        self.reject("ordinary kernel cannot be downgraded")

    def test_input_identity_and_feature_contracts_fail_closed(self):
        for key, value, pattern in (
            ("packageManifestSha256", "0" * 64, "packageManifestSha256 is stale"),
            ("cargoLockSha256", "0" * 64, "cargoLockSha256 is stale"),
            ("sourceClosureSha256", "0" * 64, "sourceClosureSha256 is stale"),
            ("kernelSymbols", ["not_an_attributed_kernel"], "not attributed"),
            ("features", ["absent-feature"], "unknown Cargo feature"),
            ("contractSha256", "0" * 64, "contractSha256 is stale"),
        ):
            with self.subTest(key=key):
                fixture = copy.deepcopy(self.original["compilerFixtures"][0])
                fixture["compilerInput"][key] = value
                with self.assertRaisesRegex(SystemExit, pattern):
                    self.validator.validate_compiler_input(ROOT, fixture, "fixture")

    def test_runner_arguments_and_environment_cannot_escape_matrix_contract(self):
        for key, value in (
            ("runnerArguments", ["x", "y"]),
            ("runnerArguments", ["x|y"]),
            ("environment", ["FE2O3_OTHER_PIPELINE=1"]),
            ("artifactName", "../input.hsaco"),
            ("runnerPath", "../outside.sh"),
        ):
            with self.subTest(key=key):
                fixture = copy.deepcopy(self.original["compilerFixtures"][0])
                fixture["matrix"][key] = value
                with self.assertRaises(SystemExit):
                    self.validator.validate_matrix(ROOT, fixture, "fixture")

    def test_pending_simulation_and_suite_cannot_claim_success(self):
        self.manifest["compilerFixtures"][0]["simulation"]["status"] = "passed"
        self.reject("explicitly pending")
        self.manifest = copy.deepcopy(self.original)
        self.manifest["qualification"]["suites"][0]["availability"] = "available"
        self.reject("not an available or passed adapter")
        self.manifest = copy.deepcopy(self.original)
        self.manifest["compilerFixtures"][0]["simulation"]["canonicalKirVersion"] = 10
        self.reject("Bundle V7 / KIR V12")

    def test_source_closure_tracks_nested_sources_but_not_target(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / "example"
            (package / "src").mkdir(parents=True)
            (package / "target").mkdir()
            (package / "Cargo.toml").write_text('[package]\nname = "test"\nversion = "0.1.0"\n', encoding="utf-8")
            source = package / "src/lib.rs"
            source.write_text("#[kernel] fn example() {}\n", encoding="utf-8")
            paths = self.validator.package_rust_sources(root, "example/Cargo.toml", "test")
            before = self.validator.package_source_closure_sha256(root, paths)
            (package / "target/generated.rs").write_text("not an input\n", encoding="utf-8")
            self.assertEqual(before, self.validator.package_source_closure_sha256(
                root, self.validator.package_rust_sources(root, "example/Cargo.toml", "test")
            ))
            source.write_text("#[kernel] fn changed() {}\n", encoding="utf-8")
            self.assertNotEqual(before, self.validator.package_source_closure_sha256(
                root, self.validator.package_rust_sources(root, "example/Cargo.toml", "test")
            ))
            (package / "src/link.rs").symlink_to(source)
            with self.assertRaisesRegex(SystemExit, "symlink"):
                self.validator.package_rust_sources(root, "example/Cargo.toml", "test")

    def test_standalone_workspace_does_not_inherit_unrelated_parent_lock(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "Cargo.lock").write_text("parent", encoding="utf-8")
            package = root / "example"
            package.mkdir()
            (package / "Cargo.toml").write_text("[workspace]\n", encoding="utf-8")
            with self.assertRaisesRegex(SystemExit, "workspace root has no Cargo.lock"):
                self.validator.effective_cargo_lock(root, package, "test")

    def test_package_presence_reuses_exact_cached_attributions_including_empty(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / "example"
            (package / "src").mkdir(parents=True)
            (package / "Cargo.toml").write_text('[package]\nname = "test"\nversion = "0.1.0"\n', encoding="utf-8")
            (package / "src/a.rs").write_text('const TEXT: &str = "#[kernel] fn fake() {}";\n', encoding="utf-8")
            (package / "src/z.rs").write_text("#[kernel] fn real() {}\n", encoding="utf-8")
            sources = self.validator.package_rust_sources(root, "example/Cargo.toml", "fixture")
            names = {
                path.relative_to(package).as_posix(): self.validator.ordinary_attributed_kernel_names(text)
                for path, text in sources
            }
            self.assertEqual(names, {"src/a.rs": [], "src/z.rs": ["real"]})
            compiler_cache = {"example/Cargo.toml": {
                "packageSources": sources, "attributedNames": names,
            }}
            package_kernels = {}
            with mock.patch.object(
                self.validator, "package_rust_sources", wraps=self.validator.package_rust_sources
            ) as walk, mock.patch.object(
                self.validator, "ordinary_attributed_kernel_names", wraps=self.validator.ordinary_attributed_kernel_names
            ) as scan:
                for lesson in ("first", "second"):
                    self.assertTrue(self.validator._package_has_ordinary_kernel(
                        root, "example/Cargo.toml", lesson, compiler_cache, package_kernels
                    ))
                walk.assert_not_called()
                scan.assert_not_called()
            self.assertEqual(package_kernels, {"example/Cargo.toml": True})

    def test_fresh_package_cache_observes_changed_off_entry_source(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / "example"
            (package / "src").mkdir(parents=True)
            (package / "Cargo.toml").write_text('[package]\nname = "test"\nversion = "0.1.0"\n', encoding="utf-8")
            (package / "src/lib.rs").write_text("fn ordinary() {}\n", encoding="utf-8")
            hidden = package / "src/hidden.rs"
            hidden.write_text("fn hidden() {}\n", encoding="utf-8")
            cache = {}
            with mock.patch.object(
                self.validator, "package_rust_sources", wraps=self.validator.package_rust_sources
            ) as walk:
                for lesson in ("first", "second"):
                    self.assertFalse(self.validator._package_has_ordinary_kernel(
                        root, "example/Cargo.toml", lesson, {}, cache
                    ))
                self.assertEqual(walk.call_count, 1)
            # validate_manifest owns fresh dictionaries on every invocation.
            # This helper-level test does not fabricate a qualified manifest.
            hidden.write_text("#[kernel] fn newly_added() {}\n", encoding="utf-8")
            with mock.patch.object(
                self.validator, "package_rust_sources", wraps=self.validator.package_rust_sources
            ) as walk:
                self.assertTrue(self.validator._package_has_ordinary_kernel(
                    root, "example/Cargo.toml", "new invocation", {}, {}
                ))
                self.assertEqual(walk.call_count, 1)

    def test_duplicate_json_keys_and_non_finite_values_are_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "manifest.json"
            for text, pattern in (
                ('{"schema": 1, "schema": 2}', "duplicate JSON key"),
                ('{"schema": NaN}', "non-finite JSON constant"),
            ):
                path.write_text(text, encoding="utf-8")
                with self.assertRaisesRegex(SystemExit, pattern):
                    self.validator.load_manifest(path)

    def test_ordinary_kernel_scanner_ignores_comments_literals_and_macro_templates(self):
        scanner = self.validator.source_contains_ordinary_attributed_kernel
        self.assertFalse(scanner(
            "// #[kernel] fn fake() {}\n"
            'const TEXT: &str = r#"#[kernel] fn fake_two() {}"#;\n'
            "/* outer #[kernel] /* nested */ fn fake_three() {} */\n"
            "macro_rules! outer { () => { #[kernel] fn fake_four() {} }; }\n"
        ))
        self.assertTrue(scanner(
            '#[kernel(typed)] #[allow(dead_code)] pub unsafe extern "C" fn real() {}\n'
        ))
        self.assertTrue(scanner('#[cfg_attr(feature = "kernel", kernel)] fn real() {}\n'))
        with self.assertRaisesRegex(SystemExit, "unterminated .*string literal"):
            scanner('const BROKEN: &str = "unterminated')

    def test_v1_remains_strict_and_retains_its_unbound_source_gap(self):
        self.manifest["curriculum"]["schema"] = self.validator.CURRICULUM_SCHEMA
        with self.assertRaisesRegex(SystemExit, "source-item obligation"):
            self.validate_curriculum()
        lesson = self.curriculum_lesson("cpu-semantic-simulation")
        lesson["codeTabs"][0].update(sourceItem=None, sourceItemStatus="pending")
        lesson["sourceBindingGap"] = "Legacy source driver is not yet contract-bound."
        self.validate_curriculum()
        self.manifest["curriculum"]["schema"] = "unknown"
        with self.assertRaisesRegex(SystemExit, "pending obligation schema"):
            self.validate_curriculum()

    def test_source_driver_has_complete_ordered_displayed_cases_without_qualification(self):
        tab = self.curriculum_lesson("cpu-semantic-simulation")["codeTabs"][0]
        item = tab["sourceItem"]
        self.assertEqual(len(item["cases"]), 13)
        self.assertEqual(sum(row["expectation"]["kind"] == "rejected" for row in item["cases"]), 3)
        self.assertGreater(item["sourceRanges"][0]["byteOffset"], item["sourceRanges"][1]["byteOffset"])
        self.assertEqual(self.validator.validate_source_item(ROOT, "cpu-semantic-simulation", tab, {}), tab["sourcePath"])
        self.assertEqual(tab["sourceItemStatus"], "contract-bound")
        self.assertEqual(self.manifest["curriculum"]["status"], "pending")

    def test_rehashed_source_contracts_reject_malformed_or_incomplete_coverage(self):
        mutations = [
            lambda item: item["cases"].pop(),
            lambda item: item["cases"].append(copy.deepcopy(item["cases"][0])),
            lambda item: item["cases"].reverse(),
            lambda item: item["cases"][0].update(kernelSymbol="aggregate_pair_struct"),
            lambda item: item["cases"][0].update(displayedFragmentOrdinal=1),
            lambda item: item["cases"][0].update(displayedFragmentOrdinal=True),
            lambda item: item["cases"][0].update(features=["unavailable_feature"]),
            lambda item: item["cases"][0].update(target="gfx000"),
            lambda item: item["cases"][0].update(testFunction="missing_driver_test"),
            lambda item: item["cases"][0]["expectation"].update(bundleVersion=7),
            lambda item: item["cases"][0]["expectation"].update(bundleVersion=True),
            lambda item: item["cases"][0]["expectation"].update(kind="qualified"),
            lambda item: item["cases"][6]["expectation"].update(outputArtifact="present"),
            lambda item: item["sourceRanges"][0].update(byteOffset=-1),
            lambda item: item["sourceRanges"][0].update(byteLength=True),
            lambda item: item["sourceRanges"][0].update(byteLength=2**32),
            lambda item: item["sourceRanges"].append(copy.deepcopy(item["sourceRanges"][0])),
            lambda item: item["sourceRanges"].__setitem__(1, copy.deepcopy(item["sourceRanges"][0])),
            lambda item: item["compilerInput"].update(cargoLockSha256="0" * 64),
            lambda item: item["compilerInput"].update(sourceClosureSha256="0" * 64),
            lambda item: item["compilerInput"].update(packageManifestSha256="0" * 64),
            lambda item: item["driver"].update(path="other.rs"),
            lambda item: item.update(qualified=True),
        ]
        for index, mutate in enumerate(mutations):
            with self.subTest(index=index):
                tab = copy.deepcopy(self.curriculum_lesson("cpu-semantic-simulation")["codeTabs"][0])
                mutate(tab["sourceItem"])
                tab["sourceItem"]["contractSha256"] = self.validator.source_item_contract_sha256("cpu-semantic-simulation", tab)
                with self.assertRaises(SystemExit):
                    self.validator.validate_source_item(ROOT, "cpu-semantic-simulation", tab, {})

    def test_source_contract_digest_binds_independent_driver_expectations(self):
        original = self.curriculum_lesson("cpu-semantic-simulation")["codeTabs"][0]
        for key, value in (
            ("features", ["aggregate_pair_struct"]),
            ("testFunction", "ordinary_rust_struct_argument_exports_exact_v4_components"),
            ("target", "gfx950"),
            ("expectation", {"kind": "verified-bundle-export", "bundleVersion": 4}),
        ):
            tab = copy.deepcopy(original)
            tab["sourceItem"]["cases"][0][key] = value
            self.assertNotEqual(
                original["sourceItem"]["contractSha256"],
                self.validator.source_item_contract_sha256("cpu-semantic-simulation", tab),
            )
        self.assertNotEqual(original["sourceItem"]["contractSha256"], self.validator.source_item_contract_sha256("other-lesson", original))

    def test_feature_scoped_includes_require_a_literal_false_top_level_module(self):
        source = '#[cfg(feature = "generated")] mod selected { include!(env!("SOURCE")); }'
        validate = self.validator.validate_rust_source_includes
        args = (ROOT / "source.rs", ROOT, "fixture")
        validate(source, *args, enabled_features=frozenset())
        validate(source.replace("mod selected", "#[allow(dead_code)] pub(crate) mod selected"), *args, enabled_features=frozenset())
        for candidate, features in (
            (source, None), (source, frozenset({"generated"})),
            (source.replace('feature = "generated"', 'unknown'), frozenset()),
            (source.replace('feature = "generated"', 'not(feature = "generated")'), frozenset()),
            ("macro_rules! outer { () => {" + source + "}; }", frozenset()),
            ("other! { " + source + " }", frozenset()),
            (source.replace("#[cfg", "#![cfg"), frozenset()),
            (source + ' include!(env!("OTHER"));', frozenset()),
        ):
            with self.subTest(candidate=candidate, features=features), self.assertRaisesRegex(SystemExit, "non-literal include"):
                validate(candidate, *args, enabled_features=features)

    def test_build_scripts_cannot_inject_a_supposedly_disabled_feature(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / "fixture"
            (package / "src").mkdir(parents=True)
            source = package / "src/lib.rs"
            source.write_text('#[cfg(feature = "generated")] mod selected { include!(env!("SOURCE")); }\n#[kernel] fn selected() {}\n')
            lock = package / "Cargo.lock"
            lock.write_text("version = 4\n")
            manifest = package / "Cargo.toml"
            for build, implicit, allowed in (("", False, True), ("", True, False), ('build = "builder.rs"\n', False, False), ("build = true\n", False, False), ("build = false\n", True, True)):
                manifest.write_text('[package]\nname = "fixture"\nversion = "0.1.0"\n' + build + '[workspace]\n[features]\ngenerated = []\n')
                build_path = package / "build.rs"
                build_path.unlink(missing_ok=True)
                if implicit:
                    build_path.write_text('fn main() { println!("cargo::rustc-cfg=feature=\\\"generated\\\""); }\n')
                if 'builder.rs' in build:
                    (package / "builder.rs").write_text("fn main() {}\n")
                sources = self.validator.package_rust_sources(root, "fixture/Cargo.toml", "test")
                item = {
                    "packageManifest": "fixture/Cargo.toml", "packageManifestSha256": hashlib.sha256(manifest.read_bytes()).hexdigest(),
                    "cargoLockPath": "fixture/Cargo.lock", "cargoLockSha256": hashlib.sha256(lock.read_bytes()).hexdigest(),
                    "sourcePaths": ["fixture/src/lib.rs"], "sourceClosureSha256": self.validator.package_source_closure_sha256(root, sources),
                    "cargoTarget": {"kind": "lib", "name": "fixture", "sourcePath": "src/lib.rs"},
                    "defaultFeatures": True, "features": [], "kernelSymbols": ["selected"],
                }
                with self.subTest(build=build, implicit=implicit):
                    if allowed:
                        self.validator.validate_compiler_input_data(root, item, "test", feature_scoped_includes=True)
                    else:
                        with self.assertRaisesRegex(SystemExit, "non-literal include"):
                            self.validator.validate_compiler_input_data(root, item, "test", feature_scoped_includes=True)


if __name__ == "__main__":
    unittest.main()
