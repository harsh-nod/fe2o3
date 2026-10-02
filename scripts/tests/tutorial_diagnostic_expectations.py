#!/usr/bin/env python3
"""Focused diagnostic source-contract controls; no compiler or GPU execution."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
from pathlib import Path
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]


def load_support():
    path = Path(__file__).with_name("tutorial_kernel_manifest.py")
    spec = importlib.util.spec_from_file_location("tutorial_manifest_support", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def diagnostic(orders=None):
    return {"kind": "diagnostic-kir-export-v1", "canonicalKirVersion": 18,
            "diagnosticTileOrders": ["blocked", "striped"] if orders is None else orders,
            "authority": "observation_only"}


class DiagnosticSourceExpectationTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.support = load_support()
        cls.support.TutorialKernelSourceContractTests.setUpClass()
        cls.validator = cls.support.TutorialKernelSourceContractTests.validator

    def setUp(self):
        self.fixture = self.support.TutorialKernelSourceContractTests()
        self.fixture.setUp()

    def validate_expectation(self, value):
        self.validator._expectations.validate_source_expectation(
            value, "case", require_object=self.validator.require_object,
            require_string=self.validator.require_string,
            require_exact_keys=self.validator.require_exact_keys, fail=self.validator.fail)

    def source_fixture(self, root, expectation=None):
        tab, source = self.fixture.whole_file_source_fixture(root)
        tab["sourceItem"]["cases"][0]["expectation"] = diagnostic() if expectation is None else expectation
        self.repin(tab)
        return tab, source

    def repin(self, tab):
        tab["sourceItem"]["contractSha256"] = self.validator.source_item_contract_sha256("whole-file", tab)

    def inventory(self, tab, source):
        reference = {"kind": "source-driver-case", "lessonId": "whole-file", "tabOrdinal": 0,
                     "caseOrdinal": 0}
        variants = copy.deepcopy(self.fixture.curriculum_lesson("cpu-semantic-simulation")["variants"])
        variants = [{"kind": row["kind"], "status": "pending", "source": None,
                     "blocker": {"owner": "test", "issue": "https://github.com/harsh-nod/fe2o3/issues/275",
                                 "reason": "Source association is not execution qualification."}}
                    for row in variants]
        return {
            "entries": [], "compilerFixtures": [],
            "productionContract": copy.deepcopy(self.fixture.original["productionContract"]),
            "curriculum": {"schema": self.validator.CURRICULUM_SCHEMA_V2,
                           "site": copy.deepcopy(self.fixture.original["curriculum"]["site"]),
                           "lessons": [{"lessonId": "whole-file", "role": "executable",
                                        "variants": [{"kind": row["kind"]} for row in variants],
                                        "codeTabs": [tab]}]},
            "kernelInventory": {
                "schema": "fe2o3-tutorial-kernel-identities-v1",
                "kernels": [{"kernelId": "selected", "selections": [reference], "variants": variants}],
                "negativeCases": [],
                "displayItems": [{"lessonId": "whole-file", "tabOrdinal": 0,
                                  "functionUtf8Offset": source.index(b"selected"),
                                  "kernelSymbol": "selected", "classification": "kernel",
                                  "kernelIds": ["selected"], "negativeCases": [],
                                  "bindingStatus": "source-driver-contract",
                                  "reason": "Declared source contract, not an execution receipt."}],
            },
        }

    def test_each_diagnostic_distribution_subset_is_exact_and_nonmutating(self):
        for orders in (["blocked"], ["striped"], ["blocked", "striped"]):
            with self.subTest(orders=orders), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                tab, _ = self.source_fixture(root, diagnostic(orders))
                before = copy.deepcopy(tab)
                self.assertEqual(self.validator.validate_source_item(root, "whole-file", tab, {}),
                                 tab["sourcePath"])
                self.assertEqual(tab, before)

    def test_diagnostic_version_is_not_a_bundle_version(self):
        for version in (None, True, False, 1, 6, 7, 17, 19, 18.0, "18"):
            with self.subTest(version=version):
                value = diagnostic()
                value["canonicalKirVersion"] = version
                with self.assertRaisesRegex(SystemExit, "unsupported diagnostic canonical KIR version"):
                    self.validate_expectation(value)
        for value in ({**diagnostic(), "bundleVersion": 18},
                      {"kind": "diagnostic-kir-export-v1", "bundleVersion": 18}):
            with self.assertRaisesRegex(SystemExit, "keys differ"):
                self.validate_expectation(value)

    def test_distribution_roster_rejects_duplicates_reordering_and_unknowns(self):
        for orders in ([], None, "blocked", ["blocked", "blocked"], ["striped", "blocked"],
                       ["blocked", "striped", "blocked"], ["other"], [True], [18], [[]], [{}]):
            with self.subTest(orders=orders):
                value = diagnostic()
                value["diagnosticTileOrders"] = orders
                with self.assertRaisesRegex(SystemExit, "unique diagnostic tile orders"):
                    self.validate_expectation(value)

    def test_diagnostic_authority_and_exact_fields_cannot_be_promoted(self):
        for authority in (None, True, False, "verified", "source_authenticated", "native", {}):
            with self.subTest(authority=authority):
                value = diagnostic()
                value["authority"] = authority
                with self.assertRaisesRegex(SystemExit, "observation-only diagnostic authority"):
                    self.validate_expectation(value)
        for field in ("sourceAuthentication", "proofAuthority", "nativeAbi", "launchAuthority",
                      "hardwareObserved", "performancePrediction", "qualified", "outputArtifact",
                      "diagnosticContains"):
            with self.subTest(field=field), self.assertRaisesRegex(SystemExit, "keys differ"):
                self.validate_expectation({**diagnostic(), field: True})
        for field in diagnostic():
            value = diagnostic()
            del value[field]
            with self.subTest(missing=field), self.assertRaises(SystemExit):
                self.validate_expectation(value)

    def test_legacy_versions_and_refusals_keep_their_exact_contracts(self):
        for version in range(1, 7):
            for value in ({"kind": "verified-bundle-export", "bundleVersion": version},
                          {"kind": "rejected", "bundleVersion": version,
                           "diagnosticContains": "specific refusal", "outputArtifact": "absent"}):
                with self.subTest(value=value):
                    before = copy.deepcopy(value)
                    self.validate_expectation(value)
                    self.assertEqual(value, before)

    def test_legacy_refusal_diagnostics_are_preserved(self):
        for kind in ("verified-bundle-export", "rejected"):
            for version in (True, 0, 7, 18, "5"):
                value = {"kind": kind, "bundleVersion": version}
                if kind == "rejected":
                    value.update(diagnosticContains="refusal", outputArtifact="absent")
                with self.subTest(value=value), self.assertRaisesRegex(SystemExit, "case has an unsupported bundle version$"):
                    self.validate_expectation(value)
            with self.assertRaisesRegex(SystemExit, "keys differ"):
                self.validate_expectation({"kind": kind, "bundleVersion": 5, "canonicalKirVersion": 18})
        for text, output, refusal in (("", "absent", "case.diagnosticContains must be a nonempty string"),
                                     ("x" * 513, "absent", "case requires an exact refusal and absent artifact"),
                                     ("refusal", "present", "case requires an exact refusal and absent artifact")):
            with self.subTest(text=text, output=output), self.assertRaisesRegex(SystemExit, refusal):
                self.validate_expectation({"kind": "rejected", "bundleVersion": 4,
                                           "diagnosticContains": text, "outputArtifact": output})

    def test_unknown_and_malformed_expectations_fail_closed(self):
        for value in (None, [], {}, {"kind": None}, {"kind": ""}, {"kind": "diagnostic-kir-export"},
                      {"kind": "diagnostic-kir-export-v2"}, {"kind": "accepted"}):
            with self.subTest(value=value), self.assertRaises(SystemExit):
                self.validate_expectation(value)

    def test_diagnostic_distribution_changes_require_a_fresh_contract_digest(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tab, _ = self.source_fixture(root)
            before = tab["sourceItem"]["contractSha256"]
            tab["sourceItem"]["cases"][0]["expectation"] = diagnostic(["blocked"])
            with self.assertRaisesRegex(SystemExit, "contract digest is stale"):
                self.validator.validate_source_item(root, "whole-file", tab, {})
            self.repin(tab)
            self.assertNotEqual(before, tab["sourceItem"]["contractSha256"])
            self.validator.validate_source_item(root, "whole-file", tab, {})

    def test_source_driver_and_physical_binding_checks_still_apply(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            original, _ = self.source_fixture(root)
            mutations = (
                lambda tab: tab["sourceItem"]["cases"][0].update(features=["unknown"]),
                lambda tab: tab["sourceItem"]["cases"][0].update(kernelSymbol="different"),
                lambda tab: tab["sourceItem"]["cases"][0].update(testFunction="different"),
                lambda tab: tab["sourceItem"]["compilerInput"].update(sourceClosureSha256="0" * 64),
                lambda tab: tab["sourceItem"]["compilerInput"].update(cargoLockSha256="0" * 64),
                lambda tab: tab.update(sourceSha256="0" * 64),
            )
            for index, mutate in enumerate(mutations):
                tab = copy.deepcopy(original)
                mutate(tab)
                self.repin(tab)
                with self.subTest(index=index), self.assertRaises(SystemExit):
                    self.validator.validate_source_item(root, "whole-file", tab, {})

    def test_conditional_driver_registration_is_not_admitted_by_diagnostic_kind(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tab, _ = self.source_fixture(root)
            driver = root / tab["sourceItem"]["driver"]["path"]
            original = driver.read_text()
            for prefix in ('#![cfg(target_os = "linux")]\n', '#[cfg(target_os = "linux")]\n'):
                driver.write_text(prefix + original)
                with self.subTest(prefix=prefix), self.assertRaisesRegex(SystemExit, "one existing driver test function"):
                    self.validator.validate_source_item(root, "whole-file", tab, {})

    def test_identity_selection_explicitly_rejects_unknown_kinds(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tab, source = self.source_fixture(root)
            document = self.inventory(tab, source)
            self.validator.validate_kernel_inventory(document, None, repo_root=root)
            for expectation in (None, {}, {"kind": "other"}, {"kind": []}):
                document["curriculum"]["lessons"][0]["codeTabs"][0]["sourceItem"]["cases"][0]["expectation"] = expectation
                with self.subTest(expectation=expectation), self.assertRaisesRegex(SystemExit, "unsupported source-driver expectation kind"):
                    self.validator.validate_kernel_inventory(document, None, repo_root=root)

    def test_actual_cpu_driver_has_one_unconditional_ignored_parent(self):
        target = ROOT / "crates/rustc-codegen-fe2o3/tests/production_scoped_tile_cpu_driver_v1"
        self.assertEqual(self.validator.source_driver_test_names(target.with_suffix(".rs").read_text()),
                         ["ordinary_mixed_tile_source_executes_public_cpu_cli_paths"])
        self.assertEqual(self.validator.source_driver_test_names((target / "linux.rs").read_text()), [])

    def test_diagnostic_source_association_and_report_are_never_qualification(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            tab, source = self.source_fixture(root)
            self.validator.validate_source_item(root, "whole-file", tab, {})
            document = self.inventory(tab, source)
            inventory = self.validator.validate_kernel_inventory(document, None, repo_root=root)
            kernel = document["kernelInventory"]["kernels"][0]
            kernel["variants"][2].update(status="source-bound", source={
                "implementationKernelId": "selected", "selection": kernel["selections"][0],
                "selectionSha256": inventory["kernelIdentities"][0]["selectionSha256"],
                "sourcePath": tab["sourcePath"], "sourceSha256": hashlib.sha256(source).hexdigest(),
                "functionUtf8Offset": source.index(b"selected"),
            })
            before = copy.deepcopy(document)
            report = self.validator._kernel_pair_report(document, {}, {}, None, repo_root=root)
            self.assertEqual(report["sourceDriverCases"][0]["expectation"], diagnostic())
            self.assertFalse(report["qualified"])
            self.assertEqual(report["qualifiedPairCount"], 0)
            self.assertEqual(report["sourceBoundPairCount"], 0)
            self.assertEqual(report["sourceBoundVariantCount"], 1)
            self.assertEqual(report["stageStatus"], "not-evaluated")
            self.assertIn("per-variant-target-evidence", report["missingBindings"])
            self.assertEqual(document, before)
            document["kernelInventory"]["negativeCases"] = kernel["selections"]
            with self.assertRaisesRegex(SystemExit, "positive, or duplicate required-negative"):
                self.validator.validate_kernel_inventory(document, None, repo_root=root)


    def committed_mixed_source(self, document=None):
        document = self.fixture.manifest if document is None else document
        lesson = next(row for row in document["curriculum"]["lessons"]
                      if row["lessonId"] == "cpu-semantic-simulation")
        kernel = next(row for row in document["kernelInventory"]["kernels"]
                      if row["kernelId"] == "source-driver:cpu-semantic-simulation:7:mixed_tile_probe")
        return lesson, lesson["codeTabs"][7], kernel

    def test_registered_mixed_source_binds_actual_file_and_diagnostic_parent(self):
        lesson, tab, kernel = self.committed_mixed_source()
        before = copy.deepcopy(self.fixture.manifest)
        self.validator.validate_source_item(ROOT, lesson["lessonId"], tab, {})
        source = (ROOT / tab["sourcePath"]).read_bytes()
        self.assertEqual(len(source), 1247)
        self.assertEqual(hashlib.sha256(source).hexdigest(), tab["sourceSha256"])
        self.assertEqual(source.index(b"mixed_tile_probe"), 468)
        self.assertEqual(tab["sourceCommit"], "fed6998b1a5eaf2530e94664a1ede650382a8990")
        self.assertEqual(tab["sourceItemStatus"], "contract-bound")
        self.assertIsNone(lesson["sourceBindingGap"])
        item = tab["sourceItem"]
        self.assertEqual(item["sourceRanges"], [{"byteOffset": 0, "byteLength": 1247}])
        self.assertEqual(item["driver"], {
            "package": "rustc-codegen-fe2o3", "target": "production_scoped_tile_cpu_driver_v1",
            "path": "crates/rustc-codegen-fe2o3/tests/production_scoped_tile_cpu_driver_v1.rs",
        })
        self.assertEqual(item["cases"], [{
            "features": ["mixed-tile-u32-kernel"], "kernelSymbol": "mixed_tile_probe",
            "target": "gfx942", "displayedFragmentOrdinal": 0,
            "testFunction": "ordinary_mixed_tile_source_executes_public_cpu_cli_paths",
            "expectation": diagnostic(),
        }])
        self.assertEqual(item["contractSha256"],
                         self.validator.source_item_contract_sha256(lesson["lessonId"], tab))
        self.assertEqual(kernel["selections"], [{
            "kind": "source-driver-case", "lessonId": "cpu-semantic-simulation",
            "tabOrdinal": 7, "caseOrdinal": 0,
        }])
        self.assertEqual(self.fixture.manifest, before)

    def test_registered_mixed_identity_is_separate_and_has_no_qualified_pair(self):
        lesson, tab, kernel = self.committed_mixed_source()
        before = copy.deepcopy(self.fixture.manifest)
        self.assertEqual([(row["kind"], row["status"]) for row in kernel["variants"]],
                         [("simt", "pending"), ("tile", "pending"), ("mixed", "source-bound")])
        self.assertTrue(all(row["status"] == "pending" and row["sourceItems"] == []
                            for row in lesson["variants"]))
        binding = kernel["variants"][2]["source"]
        self.assertEqual(binding["implementationKernelId"], kernel["kernelId"])
        self.assertEqual(binding["selection"], kernel["selections"][0])
        self.assertEqual((binding["sourcePath"], binding["sourceSha256"],
                          binding["functionUtf8Offset"]),
                         (tab["sourcePath"], tab["sourceSha256"], 468))
        row_kernel = next(row for row in self.fixture.manifest["kernelInventory"]["kernels"]
                          if row["kernelId"] == "source-driver:cpu-semantic-simulation:6:row_affine_sum_u32_v1")
        self.assertNotEqual(binding["selectionSha256"], row_kernel["variants"][0]["source"]["selectionSha256"])
        self.assertEqual([(row["kind"], row["status"]) for row in row_kernel["variants"]],
                         [("simt", "source-bound"), ("tile", "pending"), ("mixed", "pending")])
        report = self.fixture.kernel_pair_report()
        cases = [row for row in report["sourceDriverCases"]
                 if row["lessonId"] == lesson["lessonId"] and row["tabOrdinal"] == 7]
        self.assertEqual(len(cases), 1)
        self.assertEqual(cases[0]["expectation"], diagnostic())
        self.assertEqual(report["kernelInventory"]["knownKernelIdentityCount"], 62)
        self.assertEqual(report["knownVariantObligationCount"], 126)
        self.assertEqual(report["pendingVariantCount"], 106)
        self.assertEqual(report["sourceBoundVariantCount"], 20)
        self.assertEqual(report["sourceBoundPairCount"], 0)
        self.assertEqual(report["qualifiedPairCount"], 0)
        self.assertFalse(report["qualified"])
        self.assertFalse(report["inventoryComplete"])
        self.assertIsNone(report["requiredPairCount"])
        self.assertEqual(report["stageStatus"], "not-evaluated")
        self.assertNotIn("ordinarySourceObservations", report)
        self.assertEqual(self.fixture.manifest, before)

    def test_mixed_selection_cannot_be_merged_into_the_distinct_row_identity(self):
        document = copy.deepcopy(self.fixture.manifest)
        _, _, mixed = self.committed_mixed_source(document)
        kernels = document["kernelInventory"]["kernels"]
        row = next(value for value in kernels
                   if value["kernelId"] == "source-driver:cpu-semantic-simulation:6:row_affine_sum_u32_v1")
        row["selections"].extend(copy.deepcopy(mixed["selections"]))
        kernels.remove(mixed)
        with self.assertRaisesRegex(SystemExit, "one kernelId cannot merge different source selections"):
            self.validator.validate_kernel_inventory(document, None, repo_root=ROOT)

    def test_registered_mixed_binding_rejects_stale_physical_and_selection_coordinates(self):
        for field, value, message in (
            ("functionUtf8Offset", 469, "exact current source occurrence"),
            ("sourceSha256", "0" * 64, "exact current source occurrence"),
            ("selectionSha256", "0" * 64, "selection digest is stale"),
            ("implementationKernelId", "source-driver:cpu-semantic-simulation:6:row_affine_sum_u32_v1",
             "variant selection does not belong"),
        ):
            document = copy.deepcopy(self.fixture.manifest)
            _, _, kernel = self.committed_mixed_source(document)
            kernel["variants"][2]["source"][field] = value
            with self.subTest(field=field), self.assertRaisesRegex(SystemExit, message):
                self.validator.validate_kernel_inventory(document, None, repo_root=ROOT)


if __name__ == "__main__":
    unittest.main()
