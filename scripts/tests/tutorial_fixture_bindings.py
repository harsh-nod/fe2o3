#!/usr/bin/env python3
"""Exercise registered fixture excerpts through the real source-binding parent."""

from __future__ import annotations

import copy
import hashlib
import importlib.util
from pathlib import Path
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
        self.assertEqual(result["displayItemCount"], 8)
        self.assertEqual(result["knownKernelIdentityCount"], 4)
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
        row = document["kernelInventory"]["displayItems"][0]
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
        with self.assertRaisesRegex(SystemExit, "differs from its physical source"):
            self.parent.validate_kernel_inventory(document, runtime, repo_root=ROOT)


if __name__ == "__main__":
    unittest.main()
