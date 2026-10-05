"""Neutral item-span cache controls sharing the physical fixture harness."""

import copy
from unittest import mock


def make_loader(fixture_tests, identities, previous=None):
    class ItemCacheTests(fixture_tests):
        def syntax(self, source):
            code = self.scanner._rust_code_without_comments_and_literals(source)
            return code, self.scanner._rust_delimiters(code)

        def declarations(self, source, budget, cache, functions, **kwargs):
            return identities._fixture_declarations(
                source, kwargs.pop("features", set()), self.scanner.ordinary_rust_function_items,
                kwargs.pop("rust_syntax", self.syntax), budget,
                function_cache=functions, item_cache=cache, **kwargs)

        def test_item_cache_reuses_actual_utf8_raw_identifier_body_spans(self):
            source = "// \u03bb\n#[kernel]\nfn r#type() {}\n"
            cache, functions, budget = {}, {}, identities._Budget(20)
            syntax = mock.Mock(wraps=self.syntax)
            with mock.patch.object(identities, "_fixture_item_span", wraps=identities._fixture_item_span) as parse:
                first = self.declarations(source, budget, cache, functions, rust_syntax=syntax)
                retained = cache[source]
                second = self.declarations(source, budget, cache, functions, rust_syntax=syntax)
                self.assertEqual(parse.call_count, 1)
            self.assertEqual(first, second)
            self.assertEqual(first[0][0]["functionUtf8Offset"], source.encode().index(b"r#type"))
            start, boundary, end = source.index("fn "), source.index("{"), source.index("}") + 1
            body = (start, boundary, end, len(source[:start].encode()),
                    len(source[:boundary].encode()), len(source[:end].encode()))
            self.assertEqual(retained, ((((False, source.index("["), source.index("]") + 1),), body),))
            self.assertIs(cache[source], retained)
            self.assertEqual(syntax.call_args_list, [mock.call(source), mock.call(source)])
            self.assertEqual((budget.used, budget.source_item_visits, budget.attribute_visits), (2, 2, 2))

        def test_empty_and_terminal_attribute_cache_entries_are_replayed(self):
            for source in ("", "// empty item roster\n", "#![no_std]\n"):
                with self.subTest(source=source):
                    cache, functions, budget = {}, {}, identities._Budget(10)
                    syntax = mock.Mock(wraps=self.syntax)
                    with mock.patch.object(identities, "_fixture_item_span", wraps=identities._fixture_item_span) as parse:
                        for _ in range(2):
                            self.assertEqual(self.declarations(source, budget, cache, functions,
                                                               rust_syntax=syntax), ([], []))
                        parse.assert_not_called()
                    self.assertIsInstance(cache[source], tuple)
                    self.assertEqual(len(cache[source]), int(source.startswith("#!")))
                    if source.startswith("#!"):
                        self.assertEqual(cache[source][0], (((True, 2, source.index("]") + 1),), None))
                        self.assertEqual((budget.used, budget.source_item_visits, budget.attribute_visits), (1, 2, 2))
                    else:
                        self.assertEqual((budget.used, budget.source_item_visits, budget.attribute_visits), (0, 0, 0))
                    self.assertEqual(syntax.call_count, 2)

        def test_cached_spans_do_not_cache_feature_eligibility(self):
            source = '#[cfg(feature = "left")] #[kernel] fn same() {}\n' \
                     '#[cfg(feature = "right")] #[kernel] fn same() {}\n'
            cache, functions, budget = {}, {}, identities._Budget(20)
            with mock.patch.object(identities, "_fixture_item_span", wraps=identities._fixture_item_span) as parse:
                left = self.declarations(source, budget, cache, functions, features={"left"})
                right = self.declarations(source, budget, cache, functions, features={"right"})
                self.assertEqual(parse.call_count, 2)
            self.assertEqual([item["functionUtf8Offset"] for item in left[0]], [source.encode().index(b"same")])
            self.assertEqual([item["functionUtf8Offset"] for item in right[0]], [source.encode().rindex(b"same")])
            self.assertEqual((budget.used, budget.source_item_visits, budget.attribute_visits), (4, 4, 8))

        def test_changed_source_bytes_receive_fresh_spans_and_coordinates(self):
            original = "#[kernel] fn same() {}\n"
            changed = "// \u03bb shifts physical bytes\n" + original
            cache, functions, budget = {}, {}, identities._Budget(20)
            with mock.patch.object(identities, "_fixture_item_span", wraps=identities._fixture_item_span) as parse:
                first = self.declarations(original, budget, cache, functions)
                second = self.declarations(changed, budget, cache, functions)
                self.assertEqual([call.args[0] for call in parse.call_args_list], [original, changed])
            self.assertEqual(set(cache), {original, changed})
            self.assertNotEqual(cache[original], cache[changed])
            self.assertEqual(first[0][0]["functionUtf8Offset"], original.encode().index(b"same"))
            self.assertEqual(second[0][0]["functionUtf8Offset"], changed.encode().index(b"same"))

        def test_failed_traversal_never_publishes_partial_spans_even_for_inactive_items(self):
            source = 'fn helper() {}\n#[cfg(feature = "left")] #[cfg(unknown)] fn other() {}\n'
            cache, functions, budget = {}, {}, identities._Budget(100)
            with mock.patch.object(identities, "_fixture_item_span", wraps=identities._fixture_item_span) as parse:
                for _ in range(2):
                    with self.assertRaisesRegex(identities.KernelInventoryError, "cfg predicate"):
                        self.declarations(source, budget, cache, functions, features={"right"})
                    self.assertNotIn(source, cache)
                self.assertEqual(parse.call_count, 2)

        def test_new_item_span_records_have_exact_and_one_short_bounds(self):
            source = "fn helper() {}\n"
            for limit in (2, 1):
                with self.subTest(limit=limit):
                    cache, functions, budget = {}, {}, identities._Budget(limit)
                    if limit == 2:
                        self.assertEqual(self.declarations(source, budget, cache, functions), ([], []))
                        self.assertEqual((budget.used, budget.source_item_visits), (2, 1))
                        self.assertIn(source, cache)
                    else:
                        with self.assertRaisesRegex(identities.KernelInventoryError, "fixture source items.*record bound"):
                            self.declarations(source, budget, cache, functions)
                        self.assertNotIn(source, cache)
                        self.assertEqual(budget.used, 1)

        def test_replayed_item_visits_have_separate_cumulative_exact_and_short_bounds(self):
            source = "fn helper() {}\n"
            for limit in (3, 2):
                with self.subTest(limit=limit):
                    cache, functions, budget = {}, {}, identities._Budget(limit)
                    for _ in range(2):
                        self.assertEqual(self.declarations(source, budget, cache, functions), ([], []))
                    if limit == 3:
                        self.assertEqual(self.declarations(source, budget, cache, functions), ([], []))
                    else:
                        with self.assertRaisesRegex(identities.KernelInventoryError, "aggregate visit bound"):
                            self.declarations(source, budget, cache, functions)
                    self.assertEqual((budget.used, budget.source_item_visits), (2, limit))

        def test_cached_spans_still_apply_macros_only_context(self):
            for source in ("const VALUE: u32 = 0;\n", "#![no_std]\n"):
                with self.subTest(source=source):
                    cache, functions, budget = {}, {}, identities._Budget(20)
                    self.declarations(source, budget, cache, functions)
                    retained = cache[source]
                    with self.assertRaisesRegex(identities.KernelInventoryError, "included fixture source"):
                        self.declarations(source, budget, cache, functions, macros_only=True)
                    self.assertIs(cache[source], retained)

        def test_cached_spans_refuse_newly_active_inline_module(self):
            source = '#[cfg(feature = "left")] mod nested {}\n'
            cache, functions, budget = {}, {}, identities._Budget(20)
            self.assertEqual(self.declarations(source, budget, cache, functions, features={"right"}), ([], []))
            self.assertIn(source, cache)
            with mock.patch.object(identities, "_fixture_item_span", wraps=identities._fixture_item_span) as parse:
                with self.assertRaisesRegex(identities.KernelInventoryError, "inline fixture modules"):
                    self.declarations(source, budget, cache, functions, features={"left"})
                parse.assert_not_called()

        def test_cached_item_selection_retains_source_byte_and_attribute_limits(self):
            exact_bytes = 2 * sum(len(self.sources[path].encode()) for path in (self.library, self.path))
            for byte_limit, record_limit, error in ((exact_bytes, 20, None),
                                                     (exact_bytes - 1, 20, "aggregate byte bound"),
                                                     (exact_bytes, 9, "aggregate visit bound")):
                with self.subTest(byte_limit=byte_limit, record_limit=record_limit):
                    cache, functions, budget = {}, {}, identities._Budget(record_limit)
                    loader = mock.Mock(return_value=(self.library, self.sources, ["left"]))

                    def select():
                        return identities._fixture_selection(
                            self.fixture, loader, self.scanner.ordinary_rust_function_items, self.syntax,
                            budget, function_cache=functions, item_cache=cache)

                    with mock.patch.object(identities, "MAX_RUNTIME_BYTES", byte_limit):
                        select()
                        with mock.patch.object(identities, "_fixture_item_span", wraps=identities._fixture_item_span) as parse:
                            if error is None:
                                select()
                            else:
                                with self.assertRaisesRegex(identities.KernelInventoryError, error):
                                    select()
                            parse.assert_not_called()
                    self.assertEqual(budget.source_bytes, exact_bytes)
                    if record_limit == 9:
                        self.assertEqual((budget.used, budget.source_item_visits, budget.attribute_visits), (4, 6, 9))
                    self.assertEqual(loader.call_args_list, [mock.call(self.fixture)] * 2)

        def test_cached_includes_preserve_current_closure_and_scope_shadowing_refusals(self):
            self.sources[self.library] = 'include!("macros.rs");\nmod left;\n'
            macros = "example/src/macros.rs"
            self.sources[macros] = "macro_rules! inert { () => {} }\n"
            cache, functions, budget = {}, {}, identities._Budget(100)
            loader = mock.Mock(side_effect=lambda fixture: (
                self.library, self.sources, fixture["compilerInput"]["features"]))

            def select():
                return identities._fixture_selection(
                    self.fixture, loader, self.scanner.ordinary_rust_function_items, self.syntax,
                    budget, function_cache=functions, item_cache=cache)

            select()
            shadow = "macro_rules! include { () => {} }\n"
            self.declarations(shadow, budget, cache, functions)
            self.sources[macros] = shadow
            with mock.patch.object(identities, "_fixture_item_span", wraps=identities._fixture_item_span) as parse:
                with self.assertRaisesRegex(identities.KernelInventoryError, "ambiguous macro or import scope"):
                    select()
                parse.assert_not_called()
            del self.sources[macros]
            with self.assertRaisesRegex(identities.KernelInventoryError, "ambiguous or missing fixture module"):
                select()
            self.assertEqual(loader.call_args_list, [mock.call(self.fixture)] * 3)

        def test_public_validation_item_cache_is_local_and_still_reloads_selections(self):
            self.add_right()
            self.bind()
            self.bind("tile", "right")
            before = copy.deepcopy((self.manifest, self.sources))
            loader = mock.Mock(side_effect=lambda fixture: (
                self.library, self.sources, fixture["compilerInput"]["features"]))
            with mock.patch.object(identities, "_fixture_item_span", wraps=identities._fixture_item_span) as parse:
                for ordinal in (1, 2):
                    result = self.validate(False, load_fixture_sources=loader)
                    self.assertEqual(result["sourceBoundVariantCount"], 2)
                    self.assertFalse(result["inventoryComplete"])
                    root_calls = [call for call in parse.call_args_list if call.args[0] == self.sources[self.library]]
                    self.assertEqual(len(root_calls), 2 * ordinal)
            self.assertEqual(loader.call_args_list,
                             [mock.call(fixture) for fixture in self.manifest["compilerFixtures"]] * 2)
            self.assertEqual((self.manifest, self.sources), before)

    def load_tests(loader, tests, pattern):
        if previous is not None:
            tests = previous(loader, tests, pattern)
        for name in sorted(ItemCacheTests.__dict__):
            if name.startswith("test_"):
                tests.addTest(ItemCacheTests(name))
        return tests

    return load_tests
