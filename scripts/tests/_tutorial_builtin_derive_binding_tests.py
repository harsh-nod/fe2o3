"""Builtin-derived data items cannot supply or obscure attributed entry identity."""

import copy
import hashlib


class BuiltinDeriveSelectionTests(LiteralIncludeSelectionTests):
    def select_data(self, data, *, prefix="", suffix="", features=(), caches=None, budget=None):
        sources = {self.library: prefix + "mod data;\n" + self.kernel + suffix,
                   "pkg/src/data.rs": data}

        def syntax(source):
            code = self.parent._rust_code_without_comments_and_literals(source)
            return code, self.parent._rust_delimiters(code)

        function_cache, item_cache = ({}, {}) if caches is None else caches
        return self.identities._fixture_selection(
            {"compilerInput": {"kernelSymbols": ["visible"]}},
            lambda _: (self.library, sources, list(features)),
            self.parent.ordinary_rust_function_items, syntax,
            self.identities._Budget(4096) if budget is None else budget,
            function_cache=function_cache, item_cache=item_cache)

    def test_builtin_derived_data_preserves_only_the_physical_entry(self):
        data = """
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Policy { First, Second }
#[derive(Clone /* comment */, Copy, Debug, Eq, PartialEq,)]
pub struct Profile { pub name: &'static str, pub lanes: [u32; 3], pub policy: Policy }
#[derive(Default)] pub struct Unit;
#[derive(Ord, PartialOrd, Eq, PartialEq, Hash)] pub struct Rank(u32);
"""
        result = self.select_data(data)
        self.assertEqual(set(result), {"visible"})
        path, offset, source = result["visible"]
        self.assertEqual(path, self.library)
        self.assertEqual(source.encode()[offset:offset + 7], b"visible")
        self.assertNotIn("fake", result)

    def test_derive_transformations_and_unsupported_data_fail_closed(self):
        for data in (
            "#[derive(Custom)] struct Value;",
            "#[derive(core::clone::Clone)] struct Value;",
            "#[derive()] struct Value;",
            '#[derive("Clone")] struct Value;',
            "#[derive(Clone, Clone)] struct Value;",
            "#[derive(Clone)] #[derive(Clone)] struct Value;",
            "#![derive(Clone)] struct Value;",
            "#[derive(Clone)] fn ordinary() {}",
            "#[derive(Clone)] struct Value<T> { value: T }",
            "#[derive(Clone)] struct Value { #[unknown] field: u32 }",
            "#[derive(Clone)] struct Value { field: generated!() }",
            "#[derive(Clone)] enum Value { A = emit!() }",
            "#[derive(Clone)] enum Value { A = { #[kernel] fn fake() {} 0 } }",
            "#[kernel] struct Value;",
            "#[derive(Clone)] union Value { field: u32 }",
            "#[derive(Clone)] enum Value;",
            "#[derive(Clone)] enum Value(u32);",
            "#[cfg_attr(feature=\"off\", derive(Custom))] struct Value;",
            "#[cfg_attr(feature=\"off\", unknown)] struct Value;",
        ):
            with self.subTest(data=data), self.assertRaises(self.identities.KernelInventoryError):
                self.select_data(data)

    def test_derive_import_and_textual_shadowing_reject_in_either_traversal_order(self):
        for shadow in (
            "use external::Clone;\n", "use external::*;\n",
            "use external::Custom as Clone;\n", "pub use external::{Copy, Debug};\n",
            "macro_rules! Clone { () => {}; }\n",
        ):
            for before in (False, True):
                with self.subTest(shadow=shadow, before=before), self.assertRaisesRegex(
                        self.identities.KernelInventoryError, "ambiguous macro or import scope"):
                    self.select_data("#[derive(Clone)] struct Value;",
                                     prefix=shadow if before else "", suffix="" if before else shadow)
        self.select_data("use core::primitive::u32;\n#[derive(Clone)] struct Value(u32);")

    def test_cfg_and_immutable_item_cache_replay_do_not_reuse_scope_decisions(self):
        data = '#[cfg_attr(feature="on", derive(Clone))] struct Value;\n'
        caches = ({}, {})
        self.select_data(data, caches=caches)
        self.select_data(data, features=["on"], caches=caches)
        with self.assertRaisesRegex(self.identities.KernelInventoryError, "ambiguous macro or import scope"):
            self.select_data(data, features=["on"], prefix="use other::Clone;\n", caches=caches)
        with self.assertRaisesRegex(self.identities.KernelInventoryError, "builtin derive"):
            self.select_data('#[cfg_attr(feature="off", derive(Unknown))] struct Value;', caches=caches)

    def test_new_data_items_and_attributes_keep_exact_record_and_visit_bounds(self):
        data = "#[derive(Clone)] struct Value;\n#[derive(Copy)] enum Other { A }\n"
        measured = self.identities._Budget(4096)
        self.select_data(data, budget=measured)
        exact = max(measured.used, measured.attribute_visits, measured.source_item_visits)
        self.select_data(data, budget=self.identities._Budget(exact))
        with self.assertRaises(self.identities.KernelInventoryError):
            self.select_data(data, budget=self.identities._Budget(exact - 1))
        caches = ({}, {})
        cumulative = self.identities._Budget(4096)
        self.select_data(data, caches=caches, budget=cumulative)
        cumulative.maximum = cumulative.source_item_visits
        with self.assertRaisesRegex(self.identities.KernelInventoryError, "aggregate visit bound"):
            self.select_data(data, caches=caches, budget=cumulative)


class MoeCurrentBindingTests(FixtureBindingTests):
    path = "examples/moe_top2_v1/src/kernel.rs"
    symbol = "moe_top2_route_f32_t8_e4_k2_c4_v1"
    source_sha = "8b8b3477b7d9670b7a0356b05ff2aaab2aaec9f0b919bf26c4c46215d1a81eb4"

    def component(self):
        original = copy.deepcopy(self.original)
        fixture = next(row for row in original["compilerFixtures"] if row["fixtureId"] == "gfx942-moe-top2")
        kernel_id = "fixture:gfx942-moe-top2:" + self.symbol
        kernel = next(row for row in original["kernelInventory"]["kernels"] if row["kernelId"] == kernel_id)
        lesson = next(row for row in original["curriculum"]["lessons"] if row["lessonId"] == "moe-routing")
        self.assertEqual(len(lesson["codeTabs"]), 5)
        tab = lesson["codeTabs"][0]
        self.assertEqual(tab["evidenceId"], "moe-top2-current-source-v1")
        tab["ordinal"] = 0
        lesson["codeTabs"] = [tab]
        displays = [row for row in original["kernelInventory"]["displayItems"]
                    if row["lessonId"] == "moe-routing" and row["tabOrdinal"] == 0]
        self.assertEqual([(row["kernelSymbol"], row["functionUtf8Offset"]) for row in displays], [
            ("candidate_precedes_v1", 1217), ("select_top2_v1", 1493),
            ("logits_are_finite_v1", 2262), ("write_value_v1", 2491), (self.symbol, 3605),
        ])
        for row in displays:
            row["tabOrdinal"] = 0
        display = displays[-1]
        # Derive only the original identity first; binding then takes the full
        # production physical closure path, including the real contract module.
        display.update(bindingStatus="pending", kernelIds=[])
        kernel["variants"][0].update(status="pending", source=None)
        document = {
            **original, "compilerFixtures": [fixture],
            "curriculum": {**original["curriculum"], "lessons": [lesson]},
            "kernelInventory": {**original["kernelInventory"], "kernels": [kernel],
                                "negativeCases": [], "displayItems": displays},
        }
        identity = self.parent.validate_kernel_inventory(document, None, repo_root=ROOT)
        binding = {
            "implementationKernelId": kernel_id, "selection": kernel["selections"][0],
            "selectionSha256": identity["kernelIdentities"][0]["selectionSha256"],
            "sourcePath": self.path, "sourceSha256": self.source_sha, "functionUtf8Offset": 3605,
        }
        kernel["variants"][0].update(status="source-bound", source=binding)
        display.update(bindingStatus="fixture-source-contract", kernelIds=[kernel_id])
        physical = (ROOT / self.path).read_bytes()
        self.assertEqual(len(physical), 7332)
        self.assertEqual(hashlib.sha256(physical).hexdigest(), self.source_sha)
        runtime = {
            "schema": self.parent.SITE_INVENTORY_SCHEMA, "site": document["curriculum"]["site"],
            "lessons": [{"id": "moe-routing", "codeTabs": [
                {**tab, "displayedCode": physical.decode(), "sourceFragments": None},
            ]}],
        }
        return document, runtime, fixture, kernel, display

    def test_actual_moe_closure_binds_current_source_but_does_not_qualify_a_pair(self):
        document, runtime, _, kernel, _ = self.component()
        before = copy.deepcopy((document, runtime))
        result = self.check(document, runtime)
        self.assertEqual(result["unresolvedBindings"], [])
        self.assertEqual(result["knownKernelIdentityCount"], 1)
        self.assertEqual(result["displayItemCount"], 5)
        self.assertEqual(result["sourceBoundVariantCount"], 1)
        self.assertEqual(result["sourceBoundPairCount"], 0)
        self.assertEqual([(v["kind"], v["status"]) for v in kernel["variants"]],
                         [("simt", "source-bound"), ("tile", "pending")])
        self.assertEqual((document, runtime), before)

    def test_actual_binding_rejects_stale_identity_missing_occurrence_and_false_qualification(self):
        for mutation in ("hash", "offset", "path", "identity", "closure", "feature", "missing", "qualified"):
            document, runtime, fixture, kernel, _ = self.component()
            binding = kernel["variants"][0]["source"]
            if mutation == "hash":
                binding["sourceSha256"] = "0" * 64
            elif mutation == "offset":
                binding["functionUtf8Offset"] += 1
            elif mutation == "path":
                binding["sourcePath"] = "examples/moe_top2_v1/src/contract.rs"
            elif mutation == "identity":
                binding["selectionSha256"] = "0" * 64
            elif mutation in ("closure", "feature"):
                fixture["compilerInput"]["sourceClosureSha256" if mutation == "closure" else "features"] = (
                    "0" * 64 if mutation == "closure" else ["missing-source-feature"])
                fixture["compilerInput"]["contractSha256"] = self.parent.fixture_input_contract_sha256(fixture)
            elif mutation == "missing":
                document["kernelInventory"]["displayItems"].pop()
            else:
                kernel["variants"][0]["status"] = "qualified"
            with self.subTest(mutation=mutation), self.assertRaises(SystemExit):
                self.check(document, runtime)

    def test_real_contract_module_remains_in_the_authenticated_source_closure(self):
        document, _, fixture, _, _ = self.component()
        cache = {}
        self.parent.validate_compiler_input(ROOT, fixture, "derived data closure test", cache)
        package = cache[fixture["compilerInput"]["packageManifest"]]
        sources = package["packageSources"]
        index = next(i for i, (path, _) in enumerate(sources)
                     if path.as_posix().endswith("moe_top2_v1/src/contract.rs"))
        path, source = sources[index]
        self.assertIn("#[derive(Clone, Copy, Debug, Eq, PartialEq)]", source)
        package["packageSources"] = [
            *sources[:index], (path, source.replace("MOE_TOKENS_V1: usize = 8", "MOE_TOKENS_V1: usize = 9")),
            *sources[index + 1:],
        ]
        with self.assertRaisesRegex(SystemExit, "physical source differs from its validated closure"):
            self.parent.validate_kernel_inventory(document, None, repo_root=ROOT, package_cache=cache)

    def test_rehashed_display_is_not_the_selected_physical_source(self):
        document, runtime, _, kernel, _ = self.component()
        tab = document["curriculum"]["lessons"][0]["codeTabs"][0]
        live = runtime["lessons"][0]["codeTabs"][0]
        live["displayedCode"] += "\n// unmatched source bytes\n"
        data = live["displayedCode"].encode()
        digest = hashlib.sha256(data).hexdigest()
        for row in (tab, live):
            row.update(displayedUtf8Bytes=len(data), displayedSha256=digest, sourceSha256=digest)
        kernel["variants"][0]["source"]["sourceSha256"] = digest
        self.parent.validate_site_inventory(document["curriculum"], runtime)
        with self.assertRaisesRegex(SystemExit, "exact current source occurrence"):
            self.parent.validate_kernel_inventory(document, runtime, repo_root=ROOT)
