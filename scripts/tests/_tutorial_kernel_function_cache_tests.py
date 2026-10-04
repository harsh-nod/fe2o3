"""Additional neutral-census cache controls, sharing the established fixture."""

import copy
from unittest import mock


def make_loader(fixture_tests, identities):
    class FunctionCacheTests(fixture_tests):
        def shared_feature_source(self):
            self.sources = {
                self.library: "mod left;\n",
                self.path: '#[cfg(feature = "left")]\n#[kernel] fn same() {}\n'
                           '#[cfg(feature = "right")]\n#[kernel] fn same() {}\n',
            }
            self.add_right()
            self.manifest["compilerFixtures"][1]["compilerInput"]["sourcePaths"] = [self.path]
            self.row.update(kernelIds=[], bindingStatus="pending")
            self.bind()
            right = self.bind("tile", "right")
            right["functionUtf8Offset"] = self.sources[self.path].encode().rindex(b"same")
            return self.manifest["compilerFixtures"]

        def syntax(self, source):
            code = self.scanner._rust_code_without_comments_and_literals(source)
            return code, self.scanner._rust_delimiters(code)

        def source_loader(self):
            return mock.Mock(side_effect=lambda fixture: (
                self.library, self.sources, fixture["compilerInput"]["features"]))

        def select(self, fixture, loader, scanner, budget, cache):
            return identities._fixture_selection(
                fixture, loader, scanner, self.syntax, budget, function_cache=cache)

        def test_feature_selection_reuses_only_neutral_roster_including_empty_root(self):
            fixtures = self.shared_feature_source()
            loader = self.source_loader()
            scanner = mock.Mock(wraps=self.scanner.ordinary_rust_function_items)
            cache, budget = {}, identities._Budget(100)
            before = copy.deepcopy((self.manifest, self.sources))
            left, right = [self.select(fixture, loader, scanner, budget, cache) for fixture in fixtures]
            source = self.sources[self.path]
            self.assertEqual(left, {"same": (self.path, source.encode().index(b"same"), source)})
            self.assertEqual(right, {"same": (self.path, source.encode().rindex(b"same"), source)})
            self.assertNotEqual(left, right)
            self.assertEqual(scanner.call_args_list, [mock.call(self.sources[self.library]), mock.call(source)])
            self.assertEqual(cache[self.sources[self.library]], [])
            self.assertEqual(len(cache[source]), 2)
            self.assertEqual(loader.call_args_list, [mock.call(fixture) for fixture in fixtures])
            self.assertEqual((self.manifest, self.sources), before)

        def test_public_validation_reloads_each_selection_and_has_no_global_scan_cache(self):
            fixtures = self.shared_feature_source()
            loader = self.source_loader()
            with mock.patch.object(self.scanner, "ordinary_rust_function_items",
                                   wraps=self.scanner.ordinary_rust_function_items) as scanner:
                for _ in range(2):
                    result = self.validate(False, load_fixture_sources=loader)
                    self.assertEqual(result["sourceBoundVariantCount"], 2)
                    self.assertEqual(result["sourceBoundPairCount"], 1)
                    self.assertFalse(result["inventoryComplete"])
                self.assertEqual(scanner.call_args_list,
                                 [mock.call(self.sources[self.library]), mock.call(self.sources[self.path])] * 2)
            self.assertEqual(loader.call_args_list, [mock.call(fixture) for fixture in fixtures] * 2)

        def test_changed_bytes_same_path_are_rescanned_then_stale_binding_refuses(self):
            fixtures = self.shared_feature_source()
            original = self.sources[self.path]
            changed = "// changed physical offset\n" + original

            def load(fixture):
                sources = dict(self.sources)
                if fixture["fixtureId"] == "right":
                    sources[self.path] = changed
                return self.library, sources, fixture["compilerInput"]["features"]

            loader = mock.Mock(side_effect=load)
            before = copy.deepcopy((self.manifest, self.sources))
            with mock.patch.object(self.scanner, "ordinary_rust_function_items",
                                   wraps=self.scanner.ordinary_rust_function_items) as scanner:
                with self.assertRaisesRegex(identities.KernelInventoryError, "exact current source occurrence"):
                    self.validate(False, load_fixture_sources=loader)
                self.assertEqual(scanner.call_args_list,
                                 [mock.call(self.sources[self.library]), mock.call(original), mock.call(changed)])
            self.assertEqual(loader.call_args_list, [mock.call(fixture) for fixture in fixtures])
            self.assertEqual((self.manifest, self.sources), before)

        def test_cached_neutral_roster_exact_record_and_one_short_limits(self):
            fixtures = self.shared_feature_source()
            # Two neutral function rows once, plus one module and two function
            # items on each selection. Four attributes per selection remain paid.
            exact = 2 + 2 * (1 + 2)
            self.assertEqual(exact, 8)
            for limit in (exact, exact - 1):
                with self.subTest(limit=limit):
                    budget, cache = identities._Budget(limit), {}
                    loader = self.source_loader()
                    self.select(fixtures[0], loader, self.scanner.ordinary_rust_function_items, budget, cache)
                    if limit == exact:
                        self.select(fixtures[1], loader, self.scanner.ordinary_rust_function_items, budget, cache)
                        self.assertEqual((budget.used, budget.attribute_visits), (8, 8))
                    else:
                        with self.assertRaisesRegex(identities.KernelInventoryError, "record bound"):
                            self.select(fixtures[1], loader, self.scanner.ordinary_rust_function_items, budget, cache)
                        self.assertEqual((budget.used, budget.attribute_visits), (7, 6))
                    self.assertEqual(loader.call_args_list, [mock.call(fixture) for fixture in fixtures])

        def test_unadmitted_function_roster_is_not_cached(self):
            self.shared_feature_source()
            source = self.sources[self.path]
            scanner = mock.Mock(wraps=self.scanner.ordinary_rust_function_items)
            cache, short = {}, identities._Budget(1)
            with self.assertRaisesRegex(identities.KernelInventoryError, "fixture function items.*record bound"):
                identities._fixture_declarations(
                    source, {"left"}, scanner, self.syntax, short, function_cache=cache)
            self.assertEqual(cache, {})
            self.assertEqual((short.used, short.attribute_visits), (0, 0))
            exact = identities._Budget(4)
            selected, modules = identities._fixture_declarations(
                source, {"right"}, scanner, self.syntax, exact, function_cache=cache)
            self.assertEqual(selected[0]["functionUtf8Offset"], source.encode().rindex(b"same"))
            self.assertEqual(modules, [])
            self.assertEqual((exact.used, exact.attribute_visits), (4, 4))
            self.assertEqual(scanner.call_args_list, [mock.call(source), mock.call(source)])

        def test_cached_selection_still_charges_all_physical_source_bytes(self):
            fixtures = self.shared_feature_source()
            exact = 2 * sum(len(source.encode()) for source in self.sources.values())
            for limit in (exact, exact - 1):
                with self.subTest(limit=limit), mock.patch.object(identities, "MAX_RUNTIME_BYTES", limit):
                    budget, cache = identities._Budget(100), {}
                    loader = self.source_loader()
                    self.select(fixtures[0], loader, self.scanner.ordinary_rust_function_items, budget, cache)
                    if limit == exact:
                        self.select(fixtures[1], loader, self.scanner.ordinary_rust_function_items, budget, cache)
                    else:
                        with self.assertRaisesRegex(identities.KernelInventoryError, "aggregate byte bound"):
                            self.select(fixtures[1], loader, self.scanner.ordinary_rust_function_items, budget, cache)
                    self.assertEqual(budget.source_bytes, exact)
                    self.assertEqual(loader.call_args_list, [mock.call(fixture) for fixture in fixtures])

        def test_cached_function_roster_does_not_bypass_attribute_cfg_or_text_limits(self):
            self.shared_feature_source()
            source, cache = self.sources[self.path], {}
            scanner = mock.Mock(wraps=self.scanner.ordinary_rust_function_items)
            identities._fixture_declarations(
                source, {"left"}, scanner, self.syntax, identities._Budget(10), function_cache=cache)
            budget = identities._Budget(4)
            budget.visit_attribute()
            with self.assertRaisesRegex(identities.KernelInventoryError, "aggregate visit bound"):
                identities._fixture_declarations(
                    source, {"right"}, scanner, self.syntax, budget, function_cache=cache)
            self.assertEqual((budget.used, budget.attribute_visits), (2, 4))
            for constant, limit in (("MAX_CFG_TOKENS", 1), ("MAX_TEXT_BYTES", len(source.encode()) - 1)):
                with self.subTest(constant=constant), mock.patch.object(identities, constant, limit):
                    with self.assertRaisesRegex(identities.KernelInventoryError, "bound"):
                        identities._fixture_declarations(
                            source, {"right"}, scanner, self.syntax, identities._Budget(100), function_cache=cache)
            scanner.assert_called_once_with(source)

        def test_cached_selection_still_enforces_include_depth_and_loader_errors(self):
            fixtures = self.shared_feature_source()
            self.sources[self.library] = 'include!("macros.rs");\nmod left;\n'
            self.sources["example/src/macros.rs"] = "macro_rules! inert { () => {} }\n"
            cache, budget = {}, identities._Budget(100)
            loader = self.source_loader()
            scanner = mock.Mock(wraps=self.scanner.ordinary_rust_function_items)
            self.select(fixtures[0], loader, scanner, budget, cache)
            initial_scans = scanner.call_count
            with mock.patch.object(identities, "MAX_INCLUDE_DEPTH", 0):
                with self.assertRaisesRegex(identities.KernelInventoryError, "include exceeds its nesting bound"):
                    self.select(fixtures[1], loader, scanner, budget, cache)
            self.assertEqual(scanner.call_count, initial_scans)
            refused = mock.Mock(side_effect=identities.KernelInventoryError("current loader refused"))
            with self.assertRaisesRegex(identities.KernelInventoryError, "current loader refused"):
                self.select(fixtures[1], refused, scanner, budget, cache)
            refused.assert_called_once_with(fixtures[1])
            self.assertEqual(scanner.call_count, initial_scans)

    def load_tests(loader, tests, pattern):
        # The parent suite already contains inherited tests; register only these
        # new methods while sharing its setup, physical parser and fixture tools.
        for name in sorted(FunctionCacheTests.__dict__):
            if name.startswith("test_"):
                tests.addTest(FunctionCacheTests(name))
        return tests

    return load_tests
