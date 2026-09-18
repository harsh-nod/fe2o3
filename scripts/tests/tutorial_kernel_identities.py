#!/usr/bin/env python3
"""Pure identity-contract tests; shared Rust scanner tests live in the parent suite."""

import copy
import hashlib
import importlib.util
from pathlib import Path
import re
import unittest
from unittest import mock


PATH = Path(__file__).resolve().parents[1] / "tutorial_kernel_identities.py"
SPEC = importlib.util.spec_from_file_location("tutorial_kernel_identities", PATH)
IDENTITIES = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IDENTITIES)


def scan(source):
    # This fixture scanner supplies the frozen callback contract, not a second
    # production parser. Its inputs are only the small literal sources below.
    return [{"kernelSymbol": match["name"].removeprefix("r#"),
             "functionUtf8Offset": len(source[:match.start("name")].encode()),
             "attributedKernel": bool(match["attr"])}
            for match in re.finditer(r"(?P<attr>#\[kernel\]\s*)?fn\s+(?P<name>r#\w+|\w+)", source)]


def case_ref(index, tab=0):
    return {"kind": "source-driver-case", "lessonId": "lesson", "tabOrdinal": tab, "caseOrdinal": index}


def variants():
    return [{"kind": kind, "status": "pending", "source": None,
             "blocker": {"owner": "issue275-integration",
                         "issue": "https://github.com/harsh-nod/fe2o3/issues/275",
                         "reason": "Variant implementation is pending."}}
            for kind in ("simt", "tile")]


class KernelIdentitiesTests(unittest.TestCase):
    def setUp(self):
        self.source = "#[kernel] fn good() {}\n#[kernel] fn bad() {}\nfn helper() {}\n"
        inputs = {"packageManifest": "example/Cargo.toml", "packageManifestSha256": "1" * 64,
                  "cargoLockPath": "Cargo.lock", "cargoLockSha256": "2" * 64,
                  "sourcePaths": ["example/src/lib.rs"], "sourceClosureSha256": "3" * 64,
                  "cargoTarget": {"kind": "lib", "name": "example", "sourcePath": "src/lib.rs"},
                  "defaultFeatures": False, "features": ["good"]}
        self.fixture_ref = {"kind": "fixture", "fixtureId": "first", "kernelSymbol": "good"}
        cases = [{"kernelSymbol": name, "features": [name], "target": "gfx942",
                  "displayedFragmentOrdinal": 0,
                  "expectation": {"kind": kind}}
                 for name, kind in (("good", "verified-bundle-export"), ("bad", "rejected"))]
        tab = self.tab(self.source)
        tab["sourceItem"] = {"compilerInput": {key: value for key, value in inputs.items() if key != "features"},
                             "cases": cases}
        display = []
        for item in scan(self.source):
            symbol = item["kernelSymbol"]
            display.append({"lessonId": "lesson", "tabOrdinal": 0,
                            "functionUtf8Offset": item["functionUtf8Offset"], "kernelSymbol": symbol,
                            "classification": {"good": "kernel", "bad": "required-negative", "helper": "helper"}[symbol],
                            "kernelIds": ["good"] if symbol == "good" else [],
                            "negativeCases": [case_ref(1)] if symbol == "bad" else [],
                            "bindingStatus": "not-applicable" if symbol == "helper" else "source-driver-contract",
                            "reason": "Exact fixture classification."})
        self.manifest = {
            "compilerFixtures": [{"fixtureId": "first", "target": "gfx942",
                                  "compilerInput": {**inputs, "kernelSymbols": ["good"]}}],
            "curriculum": {"schema": "fe2o3-tutorial-curriculum-obligations-v2",
                           "lessons": [{"lessonId": "lesson", "role": "executable", "codeTabs": [tab]}]},
            "kernelInventory": {"schema": IDENTITIES.SCHEMA,
                                "kernels": [{"kernelId": "good", "selections": [self.fixture_ref, case_ref(0)],
                                             "variants": variants()}],
                                "negativeCases": [case_ref(1)], "displayItems": display},
        }
        self.runtime = {"lessons": [{"id": "lesson", "codeTabs": [self.live(self.source)]}]}

    @staticmethod
    def tab(source, kind="kernel"):
        encoded = source.encode()
        return {"language": "rust", "kind": kind, "displayedUtf8Bytes": len(encoded),
                "displayedSha256": hashlib.sha256(encoded).hexdigest(), "sourceItem": None,
                "sourceDigestScope": "displayed", "sourceFragmentsSha256": [hashlib.sha256(encoded).hexdigest()]}

    @staticmethod
    def live(source):
        return {"displayedCode": source, "sourceFragments": [source]}

    def validate(self, runtime=True, **kwargs):
        return IDENTITIES.validate_kernel_inventory(self.manifest, self.runtime if runtime else None, scan, **kwargs)

    def inventory(self):
        return self.manifest["kernelInventory"]

    def assert_refused(self, pattern, runtime=True):
        with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, pattern):
            self.validate(runtime)

    def add_tab(self, source, kind="host", classification="kernel"):
        ordinal = len(self.manifest["curriculum"]["lessons"][0]["codeTabs"])
        self.manifest["curriculum"]["lessons"][0]["codeTabs"].append(self.tab(source, kind))
        self.runtime["lessons"][0]["codeTabs"].append(self.live(source))
        for item in scan(source):
            if not item["attributedKernel"] and kind != "kernel":
                continue
            self.inventory()["displayItems"].append({
                "lessonId": "lesson", "tabOrdinal": ordinal,
                "functionUtf8Offset": item["functionUtf8Offset"], "kernelSymbol": item["kernelSymbol"],
                "classification": classification, "kernelIds": [], "negativeCases": [],
                "bindingStatus": "pending" if classification == "kernel" else "not-applicable",
                "reason": "Source correspondence is pending."})
        return ordinal

    def test_complete_partition_is_not_variant_qualification_and_does_not_mutate(self):
        before = copy.deepcopy((self.manifest, self.runtime))
        result = self.validate()
        self.assertTrue(result["inventoryComplete"])
        self.assertEqual(result["requiredPairCount"], 1)
        self.assertEqual(result["knownKernelIdentityCount"], 1)
        self.assertEqual(result["negativeCaseCount"], 1)
        self.assertEqual(result["unresolvedBindings"], [])
        self.assertEqual(result["kernelIdentities"][0]["variants"], variants())
        self.assertEqual((self.manifest, self.runtime), before)
        result["kernelIdentities"][0]["variants"][0]["blocker"]["owner"] = "changed"
        self.assertEqual((self.manifest, self.runtime), before)

    def test_without_runtime_never_claims_complete_or_hides_missing_joins(self):
        result = self.validate(False)
        self.assertFalse(result["runtimeCensusValidated"])
        self.assertFalse(result["inventoryComplete"])
        self.assertIsNone(result["requiredPairCount"])
        self.assertTrue(any("selection" in row for row in result["unresolvedBindings"]))

    def test_selection_partition_missing_duplicate_negative_and_foreign(self):
        original = copy.deepcopy(self.manifest)
        for mutation, pattern in (
            (lambda: self.inventory()["kernels"][0]["selections"].pop(), "missing positive"),
            (lambda: self.inventory()["kernels"][0]["selections"].append(copy.deepcopy(self.fixture_ref)), "duplicate positive"),
            (lambda: self.inventory()["kernels"][0]["selections"].append(case_ref(1)), "negative"),
            (lambda: self.inventory()["kernels"][0]["selections"].append(case_ref(99)), "unknown"),
            (lambda: self.inventory()["negativeCases"].clear(), "missing required-negative"),
            (lambda: self.inventory()["negativeCases"].append(case_ref(1)), "duplicate required-negative"),
            (lambda: self.inventory()["negativeCases"].append(case_ref(0)), "positive"),
        ):
            self.manifest = copy.deepcopy(original)
            mutation()
            with self.subTest(pattern=pattern):
                self.assert_refused(pattern)

    def test_features_defaults_source_hashes_and_cargo_target_do_not_merge(self):
        original = copy.deepcopy(self.manifest)
        for key, changed in (("features", ["other"]), ("defaultFeatures", True),
                             ("sourceClosureSha256", "9" * 64), ("cargoLockSha256", "8" * 64),
                             ("sourcePaths", ["example/src/other.rs"]),
                             ("cargoTarget", {"kind": "lib", "name": "other", "sourcePath": "src/lib.rs"})):
            self.manifest = copy.deepcopy(original)
            self.manifest["compilerFixtures"][0]["compilerInput"][key] = changed
            with self.subTest(key=key):
                self.assert_refused("cannot merge different source selections")

    def test_cross_target_identical_source_shares_one_id_not_two(self):
        fixture = copy.deepcopy(self.manifest["compilerFixtures"][0])
        fixture.update(fixtureId="other-target", target="gfx950")
        self.manifest["compilerFixtures"].append(fixture)
        ref = {"kind": "fixture", "fixtureId": "other-target", "kernelSymbol": "good"}
        self.inventory()["kernels"][0]["selections"].append(ref)
        self.assertEqual(self.validate()["requiredPairCount"], 1)
        self.inventory()["kernels"][0]["selections"].pop()
        self.inventory()["kernels"].append({"kernelId": "other", "selections": [ref], "variants": variants()})
        self.assert_refused("identical source selections must share")

    def test_unbound_positive_fixture_keeps_count_unknown(self):
        fixture = copy.deepcopy(self.manifest["compilerFixtures"][0])
        fixture.update(fixtureId="different-feature")
        fixture["compilerInput"]["features"] = ["other"]
        self.manifest["compilerFixtures"].append(fixture)
        ref = {"kind": "fixture", "fixtureId": "different-feature", "kernelSymbol": "good"}
        self.inventory()["kernels"].append({"kernelId": "other", "selections": [ref], "variants": variants()})
        result = self.validate()
        self.assertFalse(result["inventoryComplete"])
        self.assertIsNone(result["requiredPairCount"])
        self.assertIn(ref, [row.get("selection") for row in result["unresolvedBindings"]])

    def test_all_tab_kinds_and_bare_kernel_functions_need_occurrences(self):
        self.add_tab("#[kernel] fn first() {}\n#[kernel] fn first() {}\n", "comparison")
        self.add_tab("fn unannotated_entry() {}\n", "kernel")
        result = self.validate()
        self.assertEqual(result["displayItemCount"], 6)
        self.assertEqual(result["pendingDisplayItemCount"], 3)
        self.inventory()["displayItems"].pop()
        self.assert_refused("missing live function census")

    def test_raw_unicode_name_tokens_and_non_kernel_helpers(self):
        self.add_tab("// retained UTF-8: \u00e9\n#[kernel] fn r#type() {}\nfn \u03bb() {}\n", "kernel")
        self.inventory()["displayItems"][-1].update(classification="helper", bindingStatus="not-applicable")
        self.add_tab("fn \u03bb() {}\n", "host")
        result = self.validate()
        self.assertEqual(result["pendingDisplayItemCount"], 1)
        self.assertEqual(result["displayItemCount"], 5)
        self.inventory()["displayItems"][-2]["functionUtf8Offset"] += 2
        self.assert_refused("missing from the live function census")

    def test_no_attributed_helper_or_executable_conceptual_downgrade(self):
        row = self.inventory()["displayItems"][0]
        row.update(kernelIds=[], bindingStatus="not-applicable", classification="conceptual")
        self.assert_refused("cannot detach a source case")
        self.add_tab("#[kernel] fn other() {}\n", "kernel")
        row.update(kernelIds=["good"], bindingStatus="source-driver-contract", classification="kernel")
        self.inventory()["displayItems"][-1].update(classification="helper", bindingStatus="not-applicable")
        self.assert_refused("attributed kernel cannot be classified as a helper")

    def test_negative_cannot_be_reclassified_or_detached(self):
        row = self.inventory()["displayItems"][1]
        row.update(classification="kernel", kernelIds=["good"], negativeCases=[])
        self.assert_refused("negative cannot be converted")

    def test_same_name_in_distinct_positive_negative_fragments(self):
        parts = ["#[kernel] fn good() {}", "#[kernel] fn good() {}"]
        source = "\n\n".join(parts)
        tab = self.manifest["curriculum"]["lessons"][0]["codeTabs"][0]
        item = tab["sourceItem"]
        tab.update(self.tab(source))
        tab["sourceItem"] = item
        tab["sourceFragmentsSha256"] = [hashlib.sha256(part.encode()).hexdigest() for part in parts]
        item["cases"][1].update(kernelSymbol="good", displayedFragmentOrdinal=1)
        self.runtime["lessons"][0]["codeTabs"][0] = {"displayedCode": source, "sourceFragments": parts}
        self.inventory()["displayItems"].pop()
        for row, function in zip(self.inventory()["displayItems"], scan(source), strict=True):
            row.update(kernelSymbol="good", functionUtf8Offset=function["functionUtf8Offset"])
        self.assertFalse(self.validate(False)["inventoryComplete"])
        self.assertTrue(self.validate()["inventoryComplete"])
        self.inventory()["displayItems"][1]["functionUtf8Offset"] -= 1
        self.assert_refused("missing from the live function census")

    def test_case_join_is_not_transferred_to_another_display_occurrence(self):
        tab = copy.deepcopy(self.manifest["curriculum"]["lessons"][0]["codeTabs"][0])
        source = "#[kernel] fn good() {}\n"
        item = tab["sourceItem"]
        item["cases"] = [item["cases"][0]]
        tab.update(self.tab(source))
        tab["sourceItem"] = item
        self.manifest["curriculum"]["lessons"][0]["codeTabs"].append(tab)
        self.runtime["lessons"][0]["codeTabs"].append(self.live(source))
        self.inventory()["kernels"][0]["selections"].append(case_ref(0, 1))
        self.inventory()["displayItems"].append({**copy.deepcopy(self.inventory()["displayItems"][0]),
                                               "tabOrdinal": 1, "bindingStatus": "pending", "kernelIds": [],
                                               "functionUtf8Offset": scan(source)[0]["functionUtf8Offset"]})
        result = self.validate()
        self.assertIn(case_ref(0, 1), [row.get("selection") for row in result["unresolvedBindings"]])
        self.assertFalse(result["inventoryComplete"])

    def test_same_name_helper_in_another_fragment_defers_only_the_occurrence_join(self):
        parts = ["#[kernel] fn good() {}\n#[kernel] fn bad() {}", "mod local { fn good() {} }"]
        source = "\n\n".join(parts)
        tab = self.manifest["curriculum"]["lessons"][0]["codeTabs"][0]
        item = tab["sourceItem"]
        tab.update(self.tab(source))
        tab["sourceItem"] = item
        tab["sourceFragmentsSha256"] = [hashlib.sha256(part.encode()).hexdigest() for part in parts]
        self.runtime["lessons"][0]["codeTabs"][0] = {"displayedCode": source, "sourceFragments": parts}
        for row, function in zip(self.inventory()["displayItems"], scan(source), strict=True):
            row.update(kernelSymbol=function["kernelSymbol"],
                       functionUtf8Offset=function["functionUtf8Offset"])
        self.assertFalse(self.validate(False)["inventoryComplete"])
        self.assertTrue(self.validate()["inventoryComplete"])
        self.inventory()["displayItems"][0].update(
            classification="helper", bindingStatus="not-applicable", kernelIds=[],
        )
        self.assert_refused("cannot detach a source case")

    def test_stale_fragments_bytes_missing_duplicate_and_malformed_fields(self):
        manifest, runtime = copy.deepcopy(self.manifest), copy.deepcopy(self.runtime)
        mutations = [
            (lambda: self.runtime["lessons"][0]["codeTabs"][0].update(displayedCode=self.source + "x"), "displayed bytes"),
            (lambda: self.runtime["lessons"][0]["codeTabs"][0].update(sourceFragments=[self.source + "x"]), "reconstruct"),
            (lambda: self.inventory()["displayItems"].append(copy.deepcopy(self.inventory()["displayItems"][0])), "duplicate display"),
            (lambda: self.inventory()["displayItems"][0].update(classification={}), "must be strings"),
            (lambda: self.inventory()["displayItems"][0].update(bindingStatus=[]), "must be strings"),
            (lambda: self.inventory()["displayItems"][0].update(functionUtf8Offset=True), "nonnegative integer"),
            (lambda: self.inventory()["kernels"][0]["variants"][0].update(status="passed"), "remain pending"),
            (lambda: self.manifest.pop("curriculum"), "existing V2 curriculum"),
            (lambda: self.manifest["curriculum"].update(schema="other"), "existing V2 curriculum"),
        ]
        for mutation, pattern in mutations:
            self.manifest, self.runtime = copy.deepcopy(manifest), copy.deepcopy(runtime)
            mutation()
            with self.subTest(pattern=pattern):
                self.assert_refused(pattern)

    def test_exact_record_and_streamed_identity_byte_limits(self):
        minimum = next(limit for limit in range(1, 100)
                       if self.succeeds_with_limit(limit))
        self.assertTrue(self.validate(max_records=minimum)["inventoryComplete"])
        with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "record bound"):
            self.validate(max_records=minimum - 1)
        with mock.patch.object(IDENTITIES, "MAX_IDENTITY_BYTES", 1):
            self.assert_refused("identity bytes exceed")
        with mock.patch.object(IDENTITIES, "MAX_RUNTIME_BYTES", 1):
            self.assert_refused("aggregate byte bound")

    def succeeds_with_limit(self, limit):
        try:
            self.validate(max_records=limit)
            return True
        except IDENTITIES.KernelInventoryError:
            return False


class FixtureDisplayTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = PATH.with_name("validate-tutorial-kernel-manifest.py")
        spec = importlib.util.spec_from_file_location("fixture_scanner", path)
        cls.scanner = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.scanner)

    def setUp(self):
        self.library = "example/src/lib.rs"
        self.path = "example/src/left.rs"
        self.sources = {
            self.library: '#![cfg_attr(target_arch = "amdgpu", no_std)]\n'
                          '#[cfg(feature = "left")] mod left;\n#[cfg(feature = "right")] mod right;\n',
            self.path: '// UTF-8: \u03bb\n#[cfg(any(not(target_arch = "amdgpu"), feature = "left"))]\n#[kernel] fn same() {}\n',
            "example/src/right.rs": '#[cfg(feature = "right")]\n#[kernel] fn same() {}\n',
        }
        source = self.sources[self.path]
        digest = hashlib.sha256(source.encode()).hexdigest()
        self.tab = {**KernelIdentitiesTests.tab(source), "sourcePath": self.path,
                    "sourceDigestScope": "file", "sourceSha256": digest,
                    "sourceFragmentsSha256": None}
        inputs = {"packageManifest": "example/Cargo.toml", "packageManifestSha256": "1" * 64,
                  "cargoLockPath": "Cargo.lock", "cargoLockSha256": "2" * 64,
                  "sourcePaths": [self.path], "sourceClosureSha256": "3" * 64,
                  "cargoTarget": {"kind": "lib", "name": "example", "sourcePath": "src/lib.rs"},
                  "defaultFeatures": False, "features": ["left"], "kernelSymbols": ["same"]}
        self.fixture = {"fixtureId": "left", "target": "gfx950", "compilerInput": inputs}
        function = self.scanner.ordinary_rust_function_items(source)[0]
        self.row = {"lessonId": "lesson", "tabOrdinal": 0, "kernelSymbol": "same",
                    "functionUtf8Offset": function["functionUtf8Offset"], "classification": "kernel",
                    "kernelIds": ["left"], "negativeCases": [], "bindingStatus": "fixture-source-contract",
                    "reason": "Expected fixture/source contract only."}
        self.manifest = {
            "compilerFixtures": [self.fixture],
            "curriculum": {"schema": "fe2o3-tutorial-curriculum-obligations-v2",
                           "lessons": [{"lessonId": "lesson", "role": "executable", "codeTabs": [self.tab]}]},
            "kernelInventory": {"schema": IDENTITIES.SCHEMA,
                                "kernels": [{"kernelId": "left", "variants": variants(), "selections": [
                                    {"kind": "fixture", "fixtureId": "left", "kernelSymbol": "same"}]}],
                                "negativeCases": [], "displayItems": [self.row]},
        }
        self.runtime = {"lessons": [{"id": "lesson", "codeTabs": [
            {"displayedCode": source, "sourceFragments": None}]}]}

    def validate(self, runtime=True, **kwargs):
        def syntax(source):
            code = self.scanner._rust_code_without_comments_and_literals(source)
            return code, self.scanner._rust_delimiters(code)

        return IDENTITIES.validate_kernel_inventory(
            self.manifest, self.runtime if runtime else None, self.scanner.ordinary_rust_function_items,
            load_fixture_sources=kwargs.pop("load_fixture_sources", lambda fixture: (
                self.library, self.sources, fixture["compilerInput"]["features"])),
            rust_syntax=syntax, **kwargs,
        )

    def test_whole_file_contract_requires_live_census_and_keeps_variants_pending(self):
        before = copy.deepcopy((self.manifest, self.runtime, self.sources))
        result = self.validate()
        self.assertEqual(result["unresolvedBindings"], [])
        self.assertEqual(result["kernelIdentities"][0]["variants"], variants())
        self.assertEqual((self.manifest, self.runtime, self.sources), before)
        without_runtime = self.validate(False)
        self.assertFalse(without_runtime["inventoryComplete"])
        self.assertIsNone(without_runtime["requiredPairCount"])
        self.assertTrue(any("selection" in row for row in without_runtime["unresolvedBindings"]))
        with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "physical source validation"):
            IDENTITIES.validate_kernel_inventory(self.manifest, None, self.scanner.ordinary_rust_function_items)

    def test_wrong_file_feature_bytes_and_occurrence_reject(self):
        for field, value in (("sourcePath", "example/src/right.rs"), ("sourceSha256", "0" * 64),
                             ("displayedSha256", "0" * 64), ("displayedUtf8Bytes", 1),
                             ("sourceDigestScope", "displayed"), ("sourceFragmentsSha256", [])):
            self.setUp()
            self.tab[field] = value
            with self.subTest(field=field), self.assertRaises(IDENTITIES.KernelInventoryError):
                self.validate(False)
        self.setUp()
        self.fixture["compilerInput"]["features"] = ["right"]
        with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "exact current source occurrence"):
            self.validate(False)
        self.setUp()
        self.row["functionUtf8Offset"] += 1
        with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "exact current source occurrence"):
            self.validate(False)
        self.setUp()
        self.sources[self.path] += "// changed\n"
        with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "exact current source occurrence"):
            self.validate(False)

    def test_package_membership_does_not_prove_module_reachability(self):
        cases = [
            ('#[cfg(feature = "right")] mod left;', "roster differs"),
            ('mod left; mod left;', "ambiguous"),
            ('#[path = "left.rs"] mod renamed;', "selection attribute"),
            ('mod left { #[kernel] fn same() {} }', "nested fixture kernel"),
            ('mod r#left;', "unsupported fixture item"),
            ('include!("left.rs");', "unsupported fixture item"),
            ('#[cfg_attr(feature = "left", path = "left.rs")] mod left;', "selection attribute"),
            ('#[unknown_attribute] mod left;', "selection attribute"),
            ('#[cfg(all(feature = "left", unknown))] mod left;', "cfg predicate"),
        ]
        for library, message in cases:
            self.setUp()
            self.sources[self.library] = library
            with self.subTest(library=library), self.assertRaisesRegex(IDENTITIES.KernelInventoryError, message):
                self.validate(False)
        self.setUp()
        self.sources["example/src/left/mod.rs"] = self.sources[self.path]
        with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "ambiguous"):
            self.validate(False)

    def test_duplicate_selected_symbol_and_unknown_cfg_stay_unresolved(self):
        self.fixture["compilerInput"]["features"] = ["left", "right"]
        with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "ambiguous feature-selected"):
            self.validate(False)
        self.setUp()
        self.sources[self.path] = '#[cfg(unknown)] #[kernel] fn same() {}'
        with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "cfg predicate"):
            self.validate(False)
        self.row.update(kernelIds=[], bindingStatus="pending")
        result = self.validate(False)
        self.assertEqual(result["pendingDisplayItemCount"], 1)
        self.assertFalse(result["inventoryComplete"])

    def test_fixture_source_work_and_cfg_bounds(self):
        successes = []
        for limit in range(1, 100):
            try:
                self.validate(False, max_records=limit)
                successes.append(limit)
                break
            except IDENTITIES.KernelInventoryError:
                pass
        self.assertTrue(successes)
        with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "record bound"):
            self.validate(False, max_records=successes[0] - 1)
        source_bytes = sum(len(self.sources[path].encode()) for path in (self.library, self.path))
        with mock.patch.object(IDENTITIES, "MAX_RUNTIME_BYTES", source_bytes):
            self.validate(False)
        with mock.patch.object(IDENTITIES, "MAX_RUNTIME_BYTES", source_bytes - 1):
            with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "aggregate byte bound"):
                self.validate(False)
        for cfg in ("not(" * 34 + 'feature="left"' + ")" * 34,
                    "all(" + ','.join(['feature="left"'] * 70) + ")", " " * 8193):
            with self.assertRaisesRegex(IDENTITIES.KernelInventoryError, "bound"):
                IDENTITIES._fixture_cfg(cfg, {"left"})

    def test_repeated_displays_use_one_fixture_selection_cache(self):
        self.manifest["curriculum"]["lessons"][0]["codeTabs"].append(copy.deepcopy(self.tab))
        self.manifest["kernelInventory"]["displayItems"].append({**self.row, "tabOrdinal": 1})
        self.runtime["lessons"][0]["codeTabs"].append(copy.deepcopy(self.runtime["lessons"][0]["codeTabs"][0]))
        loader = mock.Mock(return_value=(self.library, self.sources, ["left"]))
        self.assertEqual(self.validate(load_fixture_sources=loader)["unresolvedBindings"], [])
        loader.assert_called_once_with(self.fixture)


if __name__ == "__main__":
    unittest.main()
