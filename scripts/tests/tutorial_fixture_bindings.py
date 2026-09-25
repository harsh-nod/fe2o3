#!/usr/bin/env python3
"""Exercise registered fixture displays through the real source-binding parent."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
import itertools
from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]
# Independent physical excerpt starts and name coordinates, not a symbol search.
EXCERPTS = {
    "gfx950-fp4-gemm": (1622, 1722, 1791),
    "gfx950-fp8-gemm": (4047, 4147, 4209),
    "gfx950-fp4-attention": (6469, 6569, 6649),
    "gfx950-fp8-attention": (16263, 16363, 16443),
}
SOURCE_SHA256 = "feebb7e80801c6b5323d19bcdb7908b93a5159c26fe2aa8181fb2de623bf6a5d"


class FixtureBindingTests(unittest.TestCase):
    expected_displays = 8
    expected_identities = 4
    changed_source_error = "differs from its physical source"

    @classmethod
    def setUpClass(cls):
        path = ROOT / "scripts/validate-tutorial-kernel-manifest.py"
        spec = importlib.util.spec_from_file_location("fixture_binding_parent", path)
        cls.parent = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.parent)
        cls.original = cls.parent.load_manifest(ROOT / "config/tutorial-kernel-manifest-v1.json")

    def document(self):
        """A real-source eight-display test projection, not the full site census."""
        document = copy.deepcopy(self.original)
        lesson_ids = {name + suffix for name in EXCERPTS for suffix in ("", "-performance-lab")}
        ids = {f"fixture:{name}:{name.replace('-', '_')}_rust" for name in EXCERPTS}
        document["compilerFixtures"] = [
            row for row in document["compilerFixtures"] if row["fixtureId"] in EXCERPTS
        ]
        inventory = document["kernelInventory"]
        inventory["kernels"] = [row for row in inventory["kernels"] if row["kernelId"] in ids]
        inventory["negativeCases"] = []
        inventory["displayItems"] = [
            row for row in inventory["displayItems"] if row["lessonId"] in lesson_ids and row["tabOrdinal"] == 0
        ]
        self.assertEqual(len(inventory["kernels"]), 4)
        self.assertEqual(len(inventory["displayItems"]), 8)
        self.assertTrue(all(row["bindingStatus"] == "fixture-source-contract" for row in inventory["displayItems"]))
        document["curriculum"]["lessons"] = [
            row for row in document["curriculum"]["lessons"] if row["lessonId"] in lesson_ids
        ]
        runtime = {"schema": self.parent.SITE_INVENTORY_SCHEMA,
                   "site": document["curriculum"]["site"], "lessons": []}
        for lesson in document["curriculum"]["lessons"]:
            lesson["codeTabs"] = lesson["codeTabs"][:1]
            tab = lesson["codeTabs"][0]
            physical = (ROOT / tab["sourcePath"]).read_bytes()
            self.assertEqual(hashlib.sha256(physical).hexdigest(), SOURCE_SHA256)
            performance = lesson["lessonId"].endswith("-performance-lab")
            name = lesson["lessonId"].removesuffix("-performance-lab")
            start = EXCERPTS[name][int(performance)]
            excerpt = physical[start:start + tab["displayedUtf8Bytes"]].decode("utf-8")
            row = next(row for row in inventory["displayItems"] if row["lessonId"] == lesson["lessonId"])
            self.assertEqual(start + row["functionUtf8Offset"], EXCERPTS[name][2])
            runtime["lessons"].append({"id": lesson["lessonId"], "codeTabs": [
                {**tab, "displayedCode": excerpt,
                 "sourceFragments": [excerpt] if tab["sourceFragmentsSha256"] is not None else None},
            ]})
        return document, runtime

    def check(self, document, runtime):
        self.parent.validate_site_inventory(document["curriculum"], runtime)
        return self.parent.validate_kernel_inventory(document, runtime, repo_root=ROOT)

    def test_real_excerpts_bind_without_qualifying_variants(self):
        document, runtime = self.document()
        before = copy.deepcopy((document, runtime))
        result = self.check(document, runtime)
        self.assertEqual(result["unresolvedBindings"], [])
        self.assertEqual(result["displayItemCount"], self.expected_displays)
        self.assertEqual(result["knownKernelIdentityCount"], self.expected_identities)
        self.assertEqual(result["sourceBoundVariantCount"], 0)
        self.assertEqual(result["sourceBoundPairCount"], 0)
        self.assertTrue(all(variant["status"] == "pending" and variant["source"] is None
                            for kernel in result["kernelIdentities"] for variant in kernel["variants"]))
        self.assertEqual((document, runtime), before)

    def test_missing_runtime_keeps_required_pair_count_unknown(self):
        document, _ = self.document()
        result = self.parent.validate_kernel_inventory(document, None, repo_root=ROOT)
        self.assertFalse(result["runtimeCensusValidated"])
        self.assertFalse(result["inventoryComplete"])
        self.assertIsNone(result["requiredPairCount"])
        self.assertEqual(result["sourceBoundPairCount"], 0)

    def test_wrong_registered_kernel_identity_rejects(self):
        document, runtime = self.document()
        row = next(row for row in document["kernelInventory"]["displayItems"]
                   if row["classification"] == "kernel")
        row["kernelIds"] = [next(kernel["kernelId"] for kernel in document["kernelInventory"]["kernels"]
                                 if kernel["kernelId"] not in row["kernelIds"])]
        with self.assertRaisesRegex(SystemExit, "different selection"):
            self.check(document, runtime)

    def test_different_compilation_feature_rejects(self):
        document, runtime = self.document()
        fixtures = document["compilerFixtures"]
        fixtures[0]["compilerInput"]["features"] = fixtures[1]["compilerInput"]["features"]
        fixtures[0]["compilerInput"]["contractSha256"] = self.parent.fixture_input_contract_sha256(fixtures[0])
        with self.assertRaisesRegex(SystemExit, "feature-selected fixture kernel roster differs"):
            self.check(document, runtime)

    def test_stale_coordinate_and_missing_occurrence_reject(self):
        for remove in (False, True):
            document, runtime = self.document()
            rows = document["kernelInventory"]["displayItems"]
            if remove:
                rows.pop()
            else:
                rows[0]["functionUtf8Offset"] += 1
            with self.subTest(remove=remove), self.assertRaisesRegex(SystemExit, "function census"):
                self.check(document, runtime)

    def test_changed_runtime_bytes_reject_even_after_display_digests_are_refreshed(self):
        document, runtime = self.document()
        tab = document["curriculum"]["lessons"][0]["codeTabs"][0]
        live = runtime["lessons"][0]["codeTabs"][0]
        live["displayedCode"] += "\n// not present in the selected physical source\n"
        payload = live["displayedCode"].encode("utf-8")
        digest = hashlib.sha256(payload).hexdigest()
        for entry in (tab, live):
            entry.update(displayedUtf8Bytes=len(payload), displayedSha256=digest, sourceSha256=digest)
            if entry["sourceFragmentsSha256"] is not None:
                entry["sourceFragmentsSha256"] = [digest]
        if live["sourceFragments"] is not None:
            live["sourceFragments"] = [live["displayedCode"]]
        self.parent.validate_site_inventory(document["curriculum"], runtime)
        with self.assertRaisesRegex(SystemExit, self.changed_source_error):
            self.parent.validate_kernel_inventory(document, runtime, repo_root=ROOT)


class WholeFileFixtureBindingTests(FixtureBindingTests):
    expected_displays = 13
    changed_source_error = "exact current source occurrence"
    # Independent whole-file sizes, digests and every displayed function offset.
    sources = {
        "flash-attention": (
            "gfx942-flash-attention", "flash_attention_general_v1", 16061,
            "f160e1391e1b354657049b5b11d745428fbbfb8fb6e5e2fceb4143ac93e7b00b",
            [885, 1688, 15154, 15319]),
        "gemm-autoresearch": (
            "gfx942-gemm-autoresearch", "gemm_autoresearch_v1", 4842,
            "3199202452896fa59e0e3f7f5e7f6e656636af039d4bf5b0718febc5edf8f9d0",
            [712, 1380, 4491]),
        "gemm-tiling": (
            "gfx942-tiled-gemm", "tiled_gemm_general_v1", 6703,
            "a898a078ac411b17764d87c0b06681045ddccc83ea8d4314fd2a48b59e8462cd",
            [797, 1530, 6352]),
        "moe-expert-compute": (
            "gfx942-grouped-expert-moe", "moe_grouped_expert_general_v1", 6578,
            "a4af47e5ab3cad6a16d4b0cd2fc9028d0660469a75914474a7fc0d984934bd0a",
            [548, 1209, 6431]),
    }

    def document(self):
        document = copy.deepcopy(self.original)
        fixtures = {source[0] for source in self.sources.values()}
        ids = {f"fixture:{source[0]}:{source[1]}" for source in self.sources.values()}
        document["compilerFixtures"] = [
            row for row in document["compilerFixtures"] if row["fixtureId"] in fixtures
        ]
        self.assertEqual(len(document["compilerFixtures"]), 4)
        for fixture in document["compilerFixtures"]:
            self.assertIs(fixture["compilerInput"]["defaultFeatures"], True)
            self.assertEqual(fixture["compilerInput"]["features"], [])
        inventory = document["kernelInventory"]
        inventory["kernels"] = [row for row in inventory["kernels"] if row["kernelId"] in ids]
        inventory["negativeCases"] = []
        inventory["displayItems"] = [
            row for row in inventory["displayItems"]
            if row["lessonId"] in self.sources and row["tabOrdinal"] == 0
        ]
        self.assertEqual(len(inventory["kernels"]), self.expected_identities)
        self.assertEqual(len(inventory["displayItems"]), self.expected_displays)
        helpers = [row for row in inventory["displayItems"] if row["classification"] == "helper"]
        self.assertEqual(len(helpers), 9)
        self.assertTrue(all(row["bindingStatus"] == "not-applicable" and row["kernelIds"] == []
                            for row in helpers))
        document["curriculum"]["lessons"] = [
            row for row in document["curriculum"]["lessons"] if row["lessonId"] in self.sources
        ]
        runtime = {"schema": self.parent.SITE_INVENTORY_SCHEMA,
                   "site": document["curriculum"]["site"], "lessons": []}
        for lesson in document["curriculum"]["lessons"]:
            fixture, symbol, size, digest, offsets = self.sources[lesson["lessonId"]]
            lesson["codeTabs"] = lesson["codeTabs"][:1]
            tab = lesson["codeTabs"][0]
            self.assertEqual(tab["sourcePath"], f"examples/{symbol}/src/kernel.rs")
            self.assertEqual(tab["sourceDigestScope"], "file")
            self.assertIsNone(tab["sourceFragmentsSha256"])
            physical = (ROOT / tab["sourcePath"]).read_bytes()
            self.assertEqual(len(physical), size)
            self.assertEqual(hashlib.sha256(physical).hexdigest(), digest)
            rows = [row for row in inventory["displayItems"] if row["lessonId"] == lesson["lessonId"]]
            self.assertEqual([row["functionUtf8Offset"] for row in rows], offsets)
            kernels = [row for row in rows if row["classification"] == "kernel"]
            self.assertEqual(len(kernels), 1)
            self.assertEqual(kernels[0]["kernelIds"], [f"fixture:{fixture}:{symbol}"])
            self.assertEqual(kernels[0]["bindingStatus"], "fixture-source-contract")
            runtime["lessons"].append({"id": lesson["lessonId"], "codeTabs": [
                {**tab, "displayedCode": physical.decode("utf-8"), "sourceFragments": None},
            ]})
        return document, runtime

    def test_different_compilation_feature_rejects(self):
        document, runtime = self.document()
        fixture = next(row for row in document["compilerFixtures"]
                       if row["fixtureId"] == "gfx942-tiled-gemm")
        fixture["compilerInput"]["features"] = ["kernel-simt-gemm-general"]
        fixture["compilerInput"]["contractSha256"] = self.parent.fixture_input_contract_sha256(fixture)
        with self.assertRaisesRegex(SystemExit, "feature-selected fixture kernel roster differs"):
            self.check(document, runtime)


class SystemsFixtureBindingTests(FixtureBindingTests):
    expected_displays = 14
    expected_identities = 11
    # Independent physical spans from the current source, not symbol searches.
    spans = {
        "advanced-moe": [(1329, 14012), (14014, 27554), (27556, 29573)],
        "speculative-mtp-verification": [(29575, 37620)],
        "ngram-embedding-gather": [(37622, 42940)],
        "muon-optimizer": [(42942, 45746), (45748, 52096)],
        "moe-route-performance-lab": [(1329, 14012)],
        "moe-expert-performance-lab": [(14014, 27554)],
        "expert-combine-performance-lab": [(27556, 29573)],
        "speculative-performance-lab": [(29575, 37620)],
        "ngram-performance-lab": [(37622, 42940)],
        "gradient-stage-performance-lab": [(42942, 45746)],
        "muon-performance-lab": [(45748, 52096)],
    }
    coordinates = {
        "gfx950_moe_route_fp4_t16_e4_k2_v1": 1541,
        "gfx950_moe_expert_rank_fp4_fp8_v1": 14556,
        "gfx950_combine_expert_ranks_v1": 28051,
        "gfx950_speculative_transaction_v1": 30160,
        "gfx950_qwen_ngram_gather_v1": 38139,
        "gfx950_stage_gradient_shard_v1": 43436,
        "gfx950_muon_update_4x4_v1": 46247,
    }
    fixture_names = {
        "combine-expert-ranks", "moe-expert-rank-expert-serial", "moe-expert-rank",
        "moe-route", "muon-update-broadcast16", "muon-update",
        "qwen-ngram-gather-reverse-probe", "qwen-ngram-gather",
        "speculative-transaction-recompute-prefix", "speculative-transaction",
        "stage-gradient-shard",
    }

    def document(self):
        """A real-source systems projection; the full Vite census is separate."""
        document = copy.deepcopy(self.original)
        lessons = {"gfx950-" + name for name in self.spans}
        fixtures = {"gfx950-" + name for name in self.fixture_names}
        document["compilerFixtures"] = [
            row for row in document["compilerFixtures"] if row["fixtureId"] in fixtures
        ]
        inventory = document["kernelInventory"]
        inventory["kernels"] = [
            row for row in inventory["kernels"] if any(
                ref["kind"] == "fixture" and ref["fixtureId"] in fixtures for ref in row["selections"])
        ]
        inventory["negativeCases"] = []
        inventory["displayItems"] = [
            row for row in inventory["displayItems"] if row["lessonId"] in lessons and row["tabOrdinal"] == 0
        ]
        self.assertEqual(len(inventory["kernels"]), 11)
        self.assertEqual(len(inventory["displayItems"]), 14)
        self.assertTrue(all(row["bindingStatus"] == "fixture-source-contract" for row in inventory["displayItems"]))
        document["curriculum"]["lessons"] = [
            row for row in document["curriculum"]["lessons"] if row["lessonId"] in lessons
        ]
        runtime = {"schema": self.parent.SITE_INVENTORY_SCHEMA,
                   "site": document["curriculum"]["site"], "lessons": []}
        source_path = "examples/gfx950_advanced_systems/src/kernel.rs"
        physical = (ROOT / source_path).read_bytes()
        self.assertEqual(len(physical), 52097)
        self.assertEqual(hashlib.sha256(physical).hexdigest(),
                         "0211e451f562b961eea723fd4d5a6c5b188cfaab83ae2354ff8bcf36dd5056d6")
        for lesson in document["curriculum"]["lessons"]:
            lesson["codeTabs"] = lesson["codeTabs"][:1]
            tab = lesson["codeTabs"][0]
            self.assertEqual(tab["sourcePath"], source_path)
            spans = self.spans[lesson["lessonId"].removeprefix("gfx950-")]
            parts = [physical[start:end].decode("utf-8") for start, end in spans]
            for row in inventory["displayItems"]:
                if row["lessonId"] != lesson["lessonId"]:
                    continue
                offset = row["functionUtf8Offset"]
                physical_offsets = []
                for start, end in spans:
                    if 0 <= offset < end - start:
                        physical_offsets.append(start + offset)
                    offset -= end - start + 2
                self.assertEqual(physical_offsets, [self.coordinates[row["kernelSymbol"]]])
            runtime["lessons"].append({"id": lesson["lessonId"], "codeTabs": [
                {**tab, "displayedCode": "\n\n".join(parts),
                 "sourceFragments": parts if tab["sourceFragmentsSha256"] is not None else None},
            ]})
        return document, runtime

    def test_feature_alternatives_remain_distinct_despite_identical_physical_function(self):
        document, runtime = self.document()
        result = self.check(document, runtime)
        by_symbol = {}
        for kernel in result["kernelIdentities"]:
            symbol = kernel["selections"][0]["kernelSymbol"]
            by_symbol.setdefault(symbol, []).append(kernel["selectionSha256"])
        self.assertEqual(sorted(map(len, by_symbol.values())), [1, 1, 1, 2, 2, 2, 2])
        self.assertTrue(all(len(values) == len(set(values)) for values in by_symbol.values()))


class AttentionCfgSelectionTests(unittest.TestCase):
    """Validate real library selection, without claiming its macro-bearing closure."""

    base_features = (
        "kernel-kda-decode", "kernel-kda-prefill", "kernel-content-sparse-attention",
        "kernel-deepseek-sparse-attention", "kernel-compressed-hybrid-attention",
        "kernel-attnres-aggregate", "kernel-four-branch-residual", "kernel-mhc-sinkhorn-mix",
    )

    @classmethod
    def setUpClass(cls):
        for name, filename in (("parent", "validate-tutorial-kernel-manifest.py"),
                               ("identities", "tutorial_kernel_identities.py")):
            spec = importlib.util.spec_from_file_location(name, ROOT / "scripts" / filename)
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            setattr(cls, name, module)
        package = ROOT / "examples/gfx950_advanced_attention"
        cls.cargo = tomllib.loads((package / "Cargo.toml").read_text())
        cls.library = (package / "src/lib.rs").read_text()
        cls.kernel = (package / "src/kernel.rs").read_text()

    def declarations(self, source, direct):
        enabled = self.parent.cargo_feature_closure(self.cargo, list(direct), False, "attention cfg test")
        code = self.parent._rust_code_without_comments_and_literals(source)
        pairs = self.parent._rust_delimiters(code)
        return self.identities._fixture_declarations(
            source, set(enabled), self.parent.ordinary_rust_function_items,
            lambda _: (code, pairs), self.identities._Budget(4096))

    def test_real_library_selects_each_single_base_feature(self):
        for feature in self.base_features:
            with self.subTest(feature=feature):
                self.assertEqual(self.declarations(self.library, [feature]), ([], ["kernel"]))

    def test_real_library_rejects_zero_and_each_pair_of_base_features(self):
        cases = [(), *itertools.combinations(self.base_features, 2), self.base_features]
        for features in cases:
            with self.subTest(features=features), self.assertRaisesRegex(
                    self.identities.KernelInventoryError, "unsupported fixture item or module selection"):
                self.declarations(self.library, features)

    def test_real_library_uses_transitive_feature_aliases(self):
        aliases = {
            "kernel-kda-decode-baseline-v1": "kda_baseline",
            "kernel-kda-prefill-baseline-v1": "kda_baseline",
            "kernel-content-sparse-attention-reciprocal-reuse-v1": "ablation",
            "kernel-deepseek-sparse-attention-leader-exp-v1": None,
            "kernel-compressed-hybrid-attention-division-baseline-v1": "ablation",
            "kernel-attnres-aggregate-explicit-reuse-v1": "ablation",
            "kernel-four-branch-residual-explicit-v1": "ablation",
            "kernel-mhc-sinkhorn-mix-scalar-v1": "ablation",
        }
        for alias, module in aliases.items():
            expected = [module, "kernel"] if module else ["kernel"]
            base = self.cargo["features"][alias]
            for direct in ([alias], [alias, *base]):
                with self.subTest(direct=direct):
                    self.assertEqual(self.declarations(self.library, direct), ([], expected))
        with self.assertRaisesRegex(self.identities.KernelInventoryError, "unsupported fixture item"):
            self.declarations(self.library, ["kernel-kda-decode-baseline-v1",
                                             "kernel-kda-prefill-baseline-v1"])

    def test_real_kernel_module_macros_still_prevent_source_binding(self):
        for feature in self.base_features:
            with self.subTest(feature=feature), self.assertRaisesRegex(
                    self.identities.KernelInventoryError, "unsupported fixture item or module selection"):
                self.declarations(self.kernel, [feature])


class RowSourceBindingTests(unittest.TestCase):
    lesson_id = "cpu-semantic-simulation"
    kernel_id = "source-driver:cpu-semantic-simulation:6:row_affine_sum_u32_v1"

    @classmethod
    def setUpClass(cls):
        spec = importlib.util.spec_from_file_location(
            "row_binding_parent", ROOT / "scripts/validate-tutorial-kernel-manifest.py")
        cls.parent = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.parent)
        cls.original = cls.parent.load_manifest(ROOT / "config/tutorial-kernel-manifest-v1.json")

    def row(self, document):
        lesson = next(row for row in document["curriculum"]["lessons"]
                      if row["lessonId"] == self.lesson_id)
        kernel = next(row for row in document["kernelInventory"]["kernels"]
                      if row["kernelId"] == self.kernel_id)
        return lesson["codeTabs"][6], kernel

    def test_registered_row_binds_exact_source_but_does_not_qualify_a_pair(self):
        document = copy.deepcopy(self.original)
        before = copy.deepcopy(document)
        tab, kernel = self.row(document)
        self.parent.validate_curriculum_tab(tab, 6, self.lesson_id, "executable", source_items=True)
        self.parent.validate_source_item(ROOT, self.lesson_id, tab, {})
        report = self.parent.validate_kernel_inventory(document, None, repo_root=ROOT)
        self.assertEqual(document, before)
        item = tab["sourceItem"]
        self.assertEqual(item["cases"], [{
            "features": ["row-affine-u32-kernel"], "kernelSymbol": "row_affine_sum_u32_v1",
            "target": "gfx942", "displayedFragmentOrdinal": 0,
            "testFunction": "ordinary_row_affine_source_matches_oracle_and_replay",
            "expectation": {"kind": "verified-bundle-export", "bundleVersion": 5},
        }])
        self.assertFalse(item["compilerInput"]["defaultFeatures"])
        self.assertEqual(item["driver"]["path"],
                         "crates/rustc-codegen-fe2o3/tests/production_neutral_workgroup_reduce_driver_v1.rs")
        self.assertEqual(item["sourceRanges"], [{"byteOffset": 0, "byteLength": 2668}])
        self.assertEqual(tab["sourcePath"], "examples/workgroup_sync_v1/src/kernel_row_affine_u32.rs")
        raw = (ROOT / tab["sourcePath"]).read_bytes()
        self.assertEqual(len(raw), 2668)
        self.assertEqual(hashlib.sha256(raw).hexdigest(),
                         "07adc0c50f24e51cb3d6c6bcb6cc1c8c6ff2a772c45ee2153435601eca2f39df")
        self.assertEqual(raw.index(b"row_affine_sum_u32_v1"), 885)
        self.assertEqual([(row["kind"], row["status"]) for row in kernel["variants"]],
                         [("simt", "source-bound"), ("tile", "pending"), ("mixed", "pending")])
        binding = kernel["variants"][0]["source"]
        self.assertEqual(binding["selection"], kernel["selections"][0])
        self.assertEqual(binding["implementationKernelId"], self.kernel_id)
        self.assertEqual(binding["functionUtf8Offset"], 885)
        self.assertEqual(binding["sourceSha256"], tab["sourceSha256"])
        for variant in kernel["variants"]:
            self.assertIn("gfx942/mi300x and gfx950/mi350", variant["blocker"]["reason"])
        self.assertEqual(report["knownKernelIdentityCount"], 61)
        self.assertEqual(report["sourceBoundVariantCount"], 2)
        self.assertEqual(report["sourceBoundPairCount"], 0)
        self.assertFalse(report["inventoryComplete"])
        self.assertIsNone(report["requiredPairCount"])
        self.assertEqual([(row["target"], row["deterministicSemanticRunnerAvailability"])
                          for row in document["qualification"]["hardwareTargets"]],
                         [("gfx942", "unavailable"), ("gfx950", "unavailable")])

    def component(self):
        """One actual row component, not the full-site runtime census."""
        document = copy.deepcopy(self.original)
        tab, kernel = self.row(document)
        lesson = next(row for row in document["curriculum"]["lessons"]
                      if row["lessonId"] == self.lesson_id)
        lesson["codeTabs"] = [tab]
        tab["ordinal"] = 0
        tab["sourceItem"]["contractSha256"] = self.parent.source_item_contract_sha256(self.lesson_id, tab)
        kernel["selections"][0]["tabOrdinal"] = 0
        kernel["variants"][0]["source"]["selection"]["tabOrdinal"] = 0
        display = next(row for row in document["kernelInventory"]["displayItems"]
                       if row["lessonId"] == self.lesson_id and row["tabOrdinal"] == 6)
        display["tabOrdinal"] = 0
        document["compilerFixtures"] = []
        document["curriculum"]["lessons"] = [lesson]
        document["kernelInventory"].update(kernels=[kernel], negativeCases=[], displayItems=[display])
        binding = kernel["variants"][0]["source"]
        kernel["variants"][0].update(status="pending", source=None)
        report = self.parent.validate_kernel_inventory(document, None, repo_root=ROOT)
        binding["selectionSha256"] = report["kernelIdentities"][0]["selectionSha256"]
        kernel["variants"][0].update(status="source-bound", source=binding)
        runtime = {"schema": self.parent.SITE_INVENTORY_SCHEMA,
                   "site": document["curriculum"]["site"], "lessons": [{
                       "id": self.lesson_id, "codeTabs": [{
                           **tab, "displayedCode": (ROOT / tab["sourcePath"]).read_text(),
                           "sourceFragments": None,
                       }],
                   }]}
        return document, runtime, tab, kernel, display

    def check(self, document, runtime):
        self.parent.validate_site_inventory(document["curriculum"], runtime)
        return self.parent.validate_kernel_inventory(document, runtime, repo_root=ROOT)

    def test_row_component_checks_exact_runtime_bytes_and_association(self):
        document, runtime, _, _, _ = self.component()
        before = copy.deepcopy((document, runtime))
        report = self.check(document, runtime)
        self.assertEqual(report["knownKernelIdentityCount"], 1)
        self.assertTrue(report["runtimeCensusValidated"])
        self.assertEqual(report["displayItemCount"], 1)
        self.assertEqual(report["sourceBoundVariantCount"], 1)
        self.assertEqual(report["sourceBoundPairCount"], 0)
        self.assertEqual((document, runtime), before)

    def test_row_source_and_display_substitutions_fail_closed(self):
        for field, value in (
            ("selectionSha256", "0" * 64), ("sourceSha256", "0" * 64),
            ("sourcePath", "examples/workgroup_sync_v1/src/lib.rs"),
            ("functionUtf8Offset", 886),
        ):
            document, runtime, _, kernel, _ = self.component()
            kernel["variants"][0]["source"][field] = value
            with self.subTest(field=field), self.assertRaises(SystemExit):
                self.check(document, runtime)
        for mutation in ("missing-identity", "duplicate-identity", "missing-display", "wrong-case",
                         "changed-display", "wrong-display-offset", "qualified", "extra-qualification"):
            document, runtime, _, kernel, display = self.component()
            if mutation == "missing-identity":
                document["kernelInventory"]["kernels"] = []
            elif mutation == "duplicate-identity":
                document["kernelInventory"]["kernels"].append(copy.deepcopy(kernel))
            elif mutation == "missing-display":
                document["kernelInventory"]["displayItems"] = []
            elif mutation == "wrong-case":
                kernel["variants"][0]["source"]["selection"]["caseOrdinal"] = 1
            elif mutation == "changed-display":
                runtime["lessons"][0]["codeTabs"][0]["displayedCode"] += "\n"
            elif mutation == "wrong-display-offset":
                display["functionUtf8Offset"] += 1
            elif mutation == "qualified":
                kernel["variants"][0]["status"] = "qualified"
            else:
                kernel["variants"][0]["qualified"] = True
            with self.subTest(mutation=mutation), self.assertRaises(SystemExit):
                self.check(document, runtime)

    def test_rehashed_wrong_feature_and_defaults_reject_physical_binding(self):
        for defaults in (False, True):
            document, _, tab, kernel, _ = self.component()
            item = tab["sourceItem"]
            if defaults:
                item["compilerInput"]["defaultFeatures"] = True
            else:
                item["cases"][0]["features"] = ["lds-kernel"]
            item["contractSha256"] = self.parent.source_item_contract_sha256(self.lesson_id, tab)
            binding = kernel["variants"][0]["source"]
            kernel["variants"][0].update(status="pending", source=None)
            report = self.parent.validate_kernel_inventory(document, None, repo_root=ROOT)
            binding["selectionSha256"] = report["kernelIdentities"][0]["selectionSha256"]
            kernel["variants"][0].update(status="source-bound", source=binding)
            with self.subTest(defaults=defaults), self.assertRaisesRegex(
                    SystemExit, "feature-selected fixture kernel roster differs|selected source"):
                self.parent.validate_kernel_inventory(document, None, repo_root=ROOT)

    def test_rehashed_source_item_still_requires_real_feature_test_and_whole_file(self):
        for mutation in ("feature", "test", "range", "lock", "closure"):
            document = copy.deepcopy(self.original)
            tab, _ = self.row(document)
            item = tab["sourceItem"]
            if mutation == "feature":
                item["cases"][0]["features"] = ["missing-row-feature"]
            elif mutation == "test":
                item["cases"][0]["testFunction"] = "row_affine_source_matches_oracle_and_replay"
            elif mutation == "range":
                item["sourceRanges"][0]["byteLength"] -= 1
            elif mutation == "lock":
                item["compilerInput"]["cargoLockSha256"] = "0" * 64
            else:
                item["compilerInput"]["sourceClosureSha256"] = "0" * 64
            item["contractSha256"] = self.parent.source_item_contract_sha256(self.lesson_id, tab)
            with self.subTest(mutation=mutation), self.assertRaises(SystemExit):
                self.parent.validate_source_item(ROOT, self.lesson_id, tab, {})


if __name__ == "__main__":
    unittest.main()
