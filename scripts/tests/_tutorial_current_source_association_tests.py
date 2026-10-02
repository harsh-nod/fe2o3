"""Exact current-source associations, not compiler or hardware qualification."""

import copy
import hashlib
import json

CASES = (
    ("moe-routing", 5, "gfx942-moe-top2", "moe_top2_route_f32_t8_e4_k2_c4_v1", 3605,
     "8b8b3477b7d9670b7a0356b05ff2aaab2aaec9f0b919bf26c4c46215d1a81eb4", False),
    ("reductions-scans", 5, "gfx942-wave64-collectives", "wave64_collectives_v1", 842,
     "3f7064730fdb52aa815cace2bcfd9a666628302506b14771c05487c95922eb4d", True),
    ("gemm-tiling", 7, "gfx942-scalar-gemm", "scalar_gemm_v1", 673,
     "b3a21a1fdd7f6fbede2437551500cabddb4500d4a07371f821a2c9a8ab620b21", True),
)


def make_loader(fixture_tests, root, previous_loader):
    class CurrentSourceAssociationTests(fixture_tests):
        def component(self, case):
            """An isolated real-source component, not the full runtime census."""
            lesson_id, ordinal, fixture_id, symbol, offset, digest, associated = case
            document = copy.deepcopy(self.original)
            document["compilerFixtures"] = [row for row in document["compilerFixtures"]
                                            if row["fixtureId"] == fixture_id]
            kernel_id = f"fixture:{fixture_id}:{symbol}"
            inventory = document["kernelInventory"]
            inventory["kernels"] = [row for row in inventory["kernels"] if row["kernelId"] == kernel_id]
            inventory["negativeCases"] = []
            inventory["displayItems"] = [row for row in inventory["displayItems"]
                                        if row["lessonId"] == lesson_id and row["tabOrdinal"] == ordinal]
            for row in inventory["displayItems"]:
                row["tabOrdinal"] = 0
            lesson = next(row for row in document["curriculum"]["lessons"] if row["lessonId"] == lesson_id)
            tab = lesson["codeTabs"][ordinal]
            tab["ordinal"] = 0
            lesson["codeTabs"] = [tab]
            document["curriculum"]["lessons"] = [lesson]
            source = (root / tab["sourcePath"]).read_bytes()
            self.assertEqual(hashlib.sha256(source).hexdigest(), digest)
            self.assertEqual(tab["displayedSha256"], digest)
            self.assertEqual(tab["sourceSha256"], digest)
            self.assertEqual(tab["sourceDigestScope"], "file")
            self.assertIsNone(tab["evidenceId"])
            self.assertIsNone(tab["sourceItem"])
            self.assertEqual(source[offset:offset + len(symbol)], symbol.encode("ascii"))
            runtime = {"schema": self.parent.SITE_INVENTORY_SCHEMA,
                       "site": document["curriculum"]["site"], "lessons": [{
                           "id": lesson_id, "codeTabs": [{
                               **tab, "displayedCode": source.decode("utf-8"), "sourceFragments": None,
                           }],
                       }]}
            return document, runtime, inventory["kernels"][0]

        def test_current_wave_and_scalar_sources_bind_without_qualifying_pairs(self):
            for case in CASES[1:]:
                with self.subTest(lesson=case[0]):
                    document, runtime, kernel = self.component(case)
                    before = copy.deepcopy((document, runtime))
                    report = self.check(document, runtime)
                    self.assertEqual(report["unresolvedBindings"], [])
                    self.assertEqual(report["sourceBoundVariantCount"], 1)
                    self.assertEqual(report["sourceBoundPairCount"], 0)
                    self.assertEqual([(row["kind"], row["status"]) for row in kernel["variants"]],
                                     [("simt", "source-bound"), ("tile", "pending")])
                    self.assertIsNone(kernel["variants"][1]["source"])
                    source = kernel["variants"][0]["source"]
                    self.assertEqual(source["selection"], kernel["selections"][0])
                    self.assertEqual(source["selectionSha256"], report["kernelIdentities"][0]["selectionSha256"])
                    self.assertEqual(source["functionUtf8Offset"], case[4])
                    self.assertEqual(source["sourceSha256"], case[5])
                    self.assertEqual(source["implementationKernelId"], kernel["kernelId"])
                    self.assertNotIn("qualified", report)
                    self.assertEqual((document, runtime), before)

        def test_current_source_associations_reject_forged_bindings_and_qualification(self):
            mutations = (
                ("sourceSha256", "0" * 64), ("selectionSha256", "0" * 64),
                ("sourcePath", "examples/fill/src/lib.rs"), ("functionUtf8Offset", 0),
            )
            for case in CASES[1:]:
                for field, value in mutations:
                    with self.subTest(lesson=case[0], field=field):
                        document, runtime, kernel = self.component(case)
                        kernel["variants"][0]["source"][field] = value
                        with self.assertRaises(SystemExit):
                            self.check(document, runtime)
                for mutation in ("qualified-status", "qualified-field", "display-bytes", "missing-display"):
                    with self.subTest(lesson=case[0], mutation=mutation):
                        document, runtime, kernel = self.component(case)
                        if mutation == "qualified-status":
                            kernel["variants"][0]["status"] = "qualified"
                        elif mutation == "qualified-field":
                            kernel["variants"][0]["qualified"] = True
                        elif mutation == "display-bytes":
                            runtime["lessons"][0]["codeTabs"][0]["displayedCode"] += " "
                        else:
                            document["kernelInventory"]["displayItems"] = []
                        with self.assertRaises(SystemExit):
                            self.check(document, runtime)

        def test_current_moe_source_stays_pending_at_the_real_selector_boundary(self):
            document, runtime, kernel = self.component(CASES[0])
            before = copy.deepcopy((document, runtime))
            report = self.check(document, runtime)
            self.assertEqual(report["sourceBoundVariantCount"], 0)
            self.assertEqual(report["pendingDisplayItemCount"], 1)
            self.assertEqual(report["sourceBoundPairCount"], 0)
            self.assertFalse(report["inventoryComplete"])
            self.assertIsNone(report["requiredPairCount"])
            self.assertTrue(all(row["status"] == "pending" and row["source"] is None
                                for row in kernel["variants"]))
            rows = document["kernelInventory"]["displayItems"]
            self.assertEqual([(row["kernelSymbol"], row["functionUtf8Offset"]) for row in rows], [
                ("candidate_precedes_v1", 1217), ("select_top2_v1", 1493),
                ("logits_are_finite_v1", 2262), ("write_value_v1", 2491),
                ("moe_top2_route_f32_t8_e4_k2_c4_v1", 3605),
            ])
            self.assertTrue(all(row["classification"] == "helper" for row in rows[:4]))
            self.assertEqual(rows[-1]["bindingStatus"], "pending")
            self.assertEqual(rows[-1]["kernelIds"], [])
            self.assertEqual((document, runtime), before)
            tab = document["curriculum"]["lessons"][0]["codeTabs"][0]
            kernel["variants"][0].update(status="source-bound", source={
                "implementationKernelId": kernel["kernelId"],
                "selection": copy.deepcopy(kernel["selections"][0]),
                "selectionSha256": report["kernelIdentities"][0]["selectionSha256"],
                "sourcePath": tab["sourcePath"], "sourceSha256": tab["sourceSha256"],
                "functionUtf8Offset": 3605,
            })
            # This is the real metadata selector's refusal, not rustc execution.
            with self.assertRaisesRegex(SystemExit, "unsupported fixture selection attribute"):
                self.check(document, runtime)

        def test_current_source_append_retains_all_historical_obligations(self):
            document = copy.deepcopy(self.original)
            for lesson_id, ordinal, _, _, _, _, _ in CASES:
                lesson = next(row for row in document["curriculum"]["lessons"]
                              if row["lessonId"] == lesson_id)
                self.assertEqual(len(lesson["codeTabs"]), ordinal + 1)
                lesson["codeTabs"].pop()
            document["curriculum"]["site"] = {
                "repository": "harsh-nod/fe2o3-kernels",
                "commit": "ee6c785fff697e6e376b68456cf02014642ed587",
                "tree": "53240b49721d1a0ecdc1cd77fb26f7dc0e8df957",
            }
            inventory = document["kernelInventory"]
            self.assertEqual(len(inventory["displayItems"]), 133)
            inventory["displayItems"] = inventory["displayItems"][:126]
            self.assertEqual(sum(row["bindingStatus"] == "pending" for row in inventory["displayItems"]), 7)
            for _, _, fixture_id, symbol, _, _, _ in CASES[1:]:
                kernel = next(row for row in inventory["kernels"]
                              if row["kernelId"] == f"fixture:{fixture_id}:{symbol}")
                variant = kernel["variants"][0]
                variant.update(status="pending", source=None)
                variant["blocker"]["reason"] = (
                    "Exact simt source variant and required per-target compile, simulation, "
                    "and hardware evidence remain pending."
                )
            for key, digest in (
                ("curriculum", "7d072ca9b6d92d05cb7835e2e052688ec193cd46c8b6b44cc04b656ba66f45a8"),
                ("kernelInventory", "c609780dc7a66d2835a0c854fe2a23b518efb344d785eed7383d1b95843c86fd"),
            ):
                payload = json.dumps(document[key], sort_keys=True, separators=(",", ":"),
                                     ensure_ascii=True).encode("ascii")
                self.assertEqual(hashlib.sha256(payload).hexdigest(), digest)

    def load_tests(loader, tests, pattern):
        tests = previous_loader(loader, tests, pattern)
        tests.addTests(CurrentSourceAssociationTests(name) for name in CurrentSourceAssociationTests.__dict__
                       if name.startswith("test_"))
        return tests

    return load_tests
