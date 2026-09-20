#!/usr/bin/env python3
"""Exercise the unchanged controller with this campaign's bindings."""

import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("aggregate_campaign", Path(__file__).with_name("campaign.py"))
C = importlib.util.module_from_spec(spec)
spec.loader.exec_module(C)
path = C.ROOT / "docs/evidence/dev-xgmi-peer-hot-mi300x-2026-09-19/test_campaign.py"
controller = path.with_name("campaign.py")
if C.sha(controller) != "6d62e802ef177540741915c7e4177b2ad5348e746de7c439b74329c74d4fd869":
    raise RuntimeError("unauthenticated test initialization dependency")
if C.sha(path) != "3f17e140f67df8da0cccdc62bfd6e8acebd04427f1c460517bcb2860b09ca614":
    raise RuntimeError("unauthenticated controller tests")
T = C.load(path, "aggregate_controller_tests")
T.C = C


class AttributionControlsTests(unittest.TestCase):
    def test_native_authentication_precedes_loader(self):
        with tempfile.TemporaryDirectory(prefix="aggregate-loader-test-") as folder:
            root = Path(folder)
            (root / "native.py").write_text("raise RuntimeError('must not execute')\n")
            with patch.object(C, "HERE", root), patch.object(C, "load") as loader:
                with self.assertRaisesRegex(RuntimeError, "authenticate native"):
                    C.native_module()
                loader.assert_not_called()
            (root / "native.py").unlink()
            (root / "native.py").symlink_to(C.HERE / "native.py")
            with patch.object(C, "HERE", root), patch.object(C, "load") as loader:
                with self.assertRaisesRegex(RuntimeError, "ordinary native"):
                    C.native_module()
                loader.assert_not_called()
        with patch.object(C, "load", return_value=object()) as loader:
            C.native_module()
            loader.assert_called_once_with(C.HERE / "native.py", "aggregate_campaign_native")

    def test_outer_timeout_exceeds_all_nested_success_bounds(self):
        owned = Path("/owned")
        build = sum(seconds for _, _, seconds in C.N.build_specs(owned))
        final = sum(seconds for _, _, seconds in C.N.final_specs(owned))
        trial = 6 * 100 + 300 + C.N.PLAN["settled_seconds"] + C.N.PLAN["delayed_seconds"]
        self.assertGreater(C.NATIVE_TIMEOUT, build + final + len(C.N.PLAN["order"]) * trial)
        self.assertEqual(C.NATIVE_TIMEOUT, 6000)


T.AttributionControlsTests = AttributionControlsTests


if __name__ == "__main__":
    unittest.main(module=T)
