"""Real-source component controls for exact display joins, never site receipts."""

import copy
import hashlib


FRAGMENT_RANGES = {
    "gfx950-attnres-gr-mhc": ((66007, 1998), (69829, 1509), (72893, 2994),),
    "gfx950-attnres-performance-lab": ((66007, 1998),),
    "gfx950-compressed-hybrid-attention": ((54841, 7552),),
    "gfx950-compressed-hybrid-performance-lab": ((54841, 7552),),
    "gfx950-content-sparse-performance-lab": ((23449, 15899),),
    "gfx950-deepseek-sparse-attention": ((42022, 9649),),
    "gfx950-deepseek-sparse-performance-lab": ((42022, 9649),),
    "gfx950-four-branch-residual-performance-lab": ((69829, 1509),),
    "gfx950-indexed-sparse-attention": ((23449, 15899),),
    "gfx950-kda-decode-performance-lab": ((10217, 3456),),
    "gfx950-kda-gdn-linear-attention": ((6052, 3963), (10217, 3456), (15019, 4232),),
    "gfx950-kda-prefill-performance-lab": ((15019, 4232),),
    "gfx950-mhc-performance-lab": ((72893, 2994),),
    "softmax-invariant": ((0, 3979),),
}
FIXTURES = {
    "gfx950-attnres-aggregate": ("gfx950_attnres_aggregate", 66151),
    "gfx950-four-branch-residual": ("gfx950_four_branch_residual", 69970),
    "gfx950-mhc-sinkhorn-mix": ("gfx950_mhc_sinkhorn_mix", 73034),
    "gfx950-compressed-hybrid-attention-division-baseline": ("gfx950_compressed_hybrid_attention", 55423),
    "gfx950-compressed-hybrid-attention": ("gfx950_compressed_hybrid_attention", 55423),
    "gfx950-content-sparse-attention-reciprocal-reuse": ("gfx950_content_sparse_attention", 24024),
    "gfx950-content-sparse-attention": ("gfx950_content_sparse_attention", 24024),
    "gfx950-deepseek-sparse-attention": ("gfx950_deepseek_sparse_attention", 42129),
    "gfx950-kda-decode": ("gfx950_kda_decode", 10324),
    "gfx950-kda-prefill": ("gfx950_kda_chunkwise_prefill", 15126),
    "gfx942-row-softmax": ("row_softmax_general_v1", 1121),
}
SOURCE_HASHES = {
    "examples/gfx950_advanced_attention/src/kernel.rs":
        "b37d2717079a596f0efac2de38bf670e96364f931b01622fc59b5e2506f11240",
    "examples/row_softmax_general_v1/src/kernel.rs":
        "b7f65c16395ea89c590aaee800167a9dceea620e55765044f423eadf9360e04d",
}


def make_loader(fixture_tests, root):
    class ExactDisplayJoinTests(fixture_tests):
        def document(self):
            """A 19-occurrence component, not the complete website census."""
            document = copy.deepcopy(self.original)
            ids = {f"fixture:{name}:{symbol}" for name, (symbol, _) in FIXTURES.items()}
            document["compilerFixtures"] = [
                row for row in document["compilerFixtures"] if row["fixtureId"] in FIXTURES
            ]
            inventory = document["kernelInventory"]
            inventory["kernels"] = [row for row in inventory["kernels"] if row["kernelId"] in ids]
            inventory["negativeCases"] = []
            inventory["displayItems"] = [
                row for row in inventory["displayItems"]
                if row["lessonId"] in FRAGMENT_RANGES and row["tabOrdinal"] == 0
            ]
            self.assertEqual(len(inventory["kernels"]), 11)
            self.assertEqual(len(inventory["displayItems"]), 19)
            self.assertEqual(sum(row["classification"] == "helper" for row in inventory["displayItems"]), 2)
            joined = [row for row in inventory["displayItems"] if row["classification"] == "kernel"]
            self.assertEqual(len(joined), 17)
            self.assertTrue(all(row["bindingStatus"] == "fixture-source-contract" for row in joined))
            self.assertEqual(sum(len(row["kernelIds"]) for row in joined), 21)
            document["curriculum"]["lessons"] = [
                row for row in document["curriculum"]["lessons"] if row["lessonId"] in FRAGMENT_RANGES
            ]
            runtime = {"schema": self.parent.SITE_INVENTORY_SCHEMA,
                       "site": document["curriculum"]["site"], "lessons": []}
            for lesson in document["curriculum"]["lessons"]:
                lesson["codeTabs"] = lesson["codeTabs"][:1]
                tab = lesson["codeTabs"][0]
                raw = (root / tab["sourcePath"]).read_bytes()
                self.assertEqual(hashlib.sha256(raw).hexdigest(), SOURCE_HASHES[tab["sourcePath"]])
                ranges = FRAGMENT_RANGES[lesson["lessonId"]]
                fragments = [raw[start:start + size].decode("utf-8") for start, size in ranges]
                displayed = "\n\n".join(fragments)
                self.assertEqual(len(displayed.encode("utf-8")), tab["displayedUtf8Bytes"])
                self.assertEqual(hashlib.sha256(displayed.encode("utf-8")).hexdigest(),
                                 tab["displayedSha256"])
                cursor = 0
                for (start, size), fragment in zip(ranges, fragments, strict=True):
                    for row in joined:
                        if row["lessonId"] != lesson["lessonId"]:
                            continue
                        relative = row["functionUtf8Offset"] - cursor
                        if not 0 <= relative < size:
                            continue
                        for identity in row["kernelIds"]:
                            fixture = identity.split(":")[1]
                            symbol, offset = FIXTURES[fixture]
                            self.assertEqual((row["kernelSymbol"], start + relative), (symbol, offset))
                            self.assertTrue(fragment.encode("utf-8")[relative:].startswith(symbol.encode("utf-8")))
                    cursor += size + 2
                runtime["lessons"].append({"id": lesson["lessonId"], "codeTabs": [{
                    **tab, "displayedCode": displayed,
                    "sourceFragments": fragments if tab["sourceDigestScope"] == "displayed" else None,
                }]})
            return document, runtime

        def test_exact_display_component_preserves_variant_obligations(self):
            document, runtime = self.document()
            before = copy.deepcopy((document, runtime))
            result = self.check(document, runtime)
            self.assertEqual(result["unresolvedBindings"], [])
            self.assertEqual(result["knownKernelIdentityCount"], 11)
            self.assertEqual(result["displayItemCount"], 19)
            self.assertEqual(result["sourceBoundVariantCount"], 10)
            self.assertEqual(result["pendingVariantCount"], 12)
            self.assertEqual(result["sourceBoundPairCount"], 0)
            self.assertEqual(result["pendingDisplayItemCount"], 0)
            self.assertEqual(result["unregisteredDisplayItemCount"], 0)
            self.assertTrue(result["runtimeCensusValidated"])
            self.assertEqual(
                [(row["kernelId"], row["variants"]) for row in result["kernelIdentities"]],
                [(row["kernelId"], row["variants"]) for row in document["kernelInventory"]["kernels"]],
            )
            self.assertEqual((document, runtime), before)

        def test_exact_display_component_still_requires_runtime_census(self):
            document, _ = self.document()
            result = self.parent.validate_kernel_inventory(document, None, repo_root=root)
            self.assertFalse(result["inventoryComplete"])
            self.assertFalse(result["runtimeCensusValidated"])
            self.assertIsNone(result["requiredPairCount"])
            self.assertEqual(sum("selection" in row for row in result["unresolvedBindings"]), 11)
            self.assertEqual(result["sourceBoundPairCount"], 0)

        def test_exact_display_census_rejects_missing_helper_duplicate_and_shift(self):
            for mutation in ("missing helper", "duplicate", "shifted"):
                document, runtime = self.document()
                rows = document["kernelInventory"]["displayItems"]
                if mutation == "missing helper":
                    rows.remove(next(row for row in rows if row["classification"] == "helper"))
                elif mutation == "duplicate":
                    rows.append(copy.deepcopy(rows[0]))
                else:
                    rows[0]["functionUtf8Offset"] += 1
                with self.subTest(mutation=mutation), self.assertRaisesRegex(
                        SystemExit, "function census|duplicate display occurrence"):
                    self.check(document, runtime)

        def test_exact_display_joins_reject_same_symbol_other_source(self):
            for lesson, fixture in (
                ("gfx950-attnres-performance-lab", "gfx950-attnres-aggregate-explicit-reuse"),
                ("gfx950-kda-decode-performance-lab", "gfx950-kda-decode-baseline"),
                ("gfx950-mhc-performance-lab", "gfx950-mhc-sinkhorn-mix-scalar"),
            ):
                document = copy.deepcopy(self.original)
                row = next(row for row in document["kernelInventory"]["displayItems"]
                           if row["lessonId"] == lesson and row["tabOrdinal"] == 0)
                row["kernelIds"] = [f"fixture:{fixture}:{row['kernelSymbol']}"]
                with self.subTest(fixture=fixture), self.assertRaisesRegex(
                        SystemExit, "exact selected source|exact current source occurrence"):
                    self.parent.validate_kernel_inventory(document, None, repo_root=root)

        def test_exact_display_component_rechecks_active_source_feature(self):
            document, runtime = self.document()
            fixture_id = "gfx950-kda-decode"
            fixture = next(row for row in document["compilerFixtures"] if row["fixtureId"] == fixture_id)
            baseline = next(row for row in self.original["compilerFixtures"]
                            if row["fixtureId"] == "gfx950-kda-decode-baseline")
            self.assertEqual(fixture["compilerInput"]["kernelSymbols"], ["gfx950_kda_decode"])
            self.assertEqual(baseline["compilerInput"]["kernelSymbols"], ["gfx950_kda_decode"])
            self.assertEqual(fixture["compilerInput"]["sourcePaths"],
                             ["examples/gfx950_advanced_attention/src/kernel.rs"])
            self.assertEqual(baseline["compilerInput"]["sourcePaths"],
                             ["examples/gfx950_advanced_attention/src/kda_baseline.rs"])
            fixture["compilerInput"]["features"] = ["kernel-kda-decode-baseline-v1"]
            fixture["compilerInput"]["contractSha256"] = self.parent.fixture_input_contract_sha256(fixture)
            kernel = next(row for row in document["kernelInventory"]["kernels"]
                          if row["kernelId"] == f"fixture:{fixture_id}:gfx950_kda_decode")
            for variant in kernel["variants"]:
                variant.update(status="pending", source=None)
            with self.assertRaisesRegex(
                    SystemExit, "fixture display differs from the exact current source occurrence"):
                self.check(document, runtime)

        def test_exact_display_component_rejects_changed_runtime_bytes(self):
            document, runtime = self.document()
            runtime["lessons"][0]["codeTabs"][0]["displayedCode"] += " "
            with self.assertRaisesRegex(SystemExit, "displayed bytes do not match"):
                self.check(document, runtime)

    def load_tests(loader, tests, pattern):
        tests.addTests(ExactDisplayJoinTests(name) for name in ExactDisplayJoinTests.__dict__
                       if name.startswith("test_"))
        return tests

    return load_tests
