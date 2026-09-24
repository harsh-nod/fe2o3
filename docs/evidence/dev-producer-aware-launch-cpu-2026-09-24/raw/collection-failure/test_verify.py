#!/usr/bin/env python3
"""Negative checks for CPU receipt and named-test evidence parsing."""

import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
from types import ModuleType
import unittest
from unittest.mock import patch


def load(path, digest, name):
    if not stat.S_ISREG(path.lstat().st_mode):
        raise RuntimeError("ordinary test helper")
    raw = path.read_bytes()
    if hashlib.sha256(raw).hexdigest() != digest:
        raise RuntimeError("test helper identity")
    module = ModuleType(name)
    module.__file__ = str(path)
    exec(compile(raw, str(path), "exec"), module.__dict__)
    return module


V = load(Path(__file__).with_name("verify.py"),
         "eb855d4cddba181c3edbe926e344a2b5dc671d2d9baf4a1542f4150c1d3e1d24", "producer_launch_verify")
C = load(Path(__file__).with_name("collect.py"),
         "3be22b5ad7ed86cc9e5d7ed76063edee16b41b39c4f71a7c7d6245730cd80f24", "producer_launch_collect")


class EvidenceTests(unittest.TestCase):
    def test_duplicate_json_keys(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "record.json"
            for text in ('{"exit":101,"exit":0}', '{"absent":false,"absent":true}',
                         '{"source":{"a":"old","a":"new"}}', '{"value":NaN}'):
                path.write_text(text)
                with self.subTest(text=text), self.assertRaises(RuntimeError):
                    V.read(path)

    def test_helper_authentication_precedes_compile(self):
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "helper.py"
            raw = b"VALUE = 3\n"
            path.write_bytes(raw)
            digest = hashlib.sha256(raw).hexdigest()
            self.assertEqual(V.authenticated_module(path, digest, "positive").VALUE, 3)
            link = path.with_name("link.py")
            link.symlink_to(path)
            for candidate, expected in ((path, "0" * 64), (link, digest)):
                with self.subTest(path=candidate), patch("builtins.compile") as compiler:
                    with self.assertRaises(RuntimeError):
                        V.authenticated_module(candidate, expected, "negative")
                    compiler.assert_not_called()

    def test_bootstrap_rejects_substituted_helpers_before_execution(self):
        root = Path(__file__).resolve().parent
        for script, helper in (("verify.py", "runner.py"), ("collect.py", "verify.py"), ("audit.py", "verify.py")):
            with self.subTest(script=script), tempfile.TemporaryDirectory() as temporary:
                folder = Path(temporary)
                shutil.copy2(root / script, folder / script)
                marker = folder / "executed"
                (folder / helper).write_text("from pathlib import Path\nPath(" + repr(str(marker)) + ").touch()\n")
                result = subprocess.run([sys.executable, "-I", "-B", str(folder / script)],
                                        capture_output=True, text=True, timeout=30, check=False)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("identity mismatch", result.stderr)
                self.assertFalse(marker.exists())
                self.assertFalse((folder / "__pycache__").exists())

    def test_collector_cache_refuses_symlinks_and_special_entries(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            regular = folder / "ordinary"
            regular.write_bytes(b"cache")
            self.assertGreater(C.cache_bytes(folder), 0)
            uid = os.getuid()
            with patch("os.getuid", return_value=uid + 1), self.assertRaises(RuntimeError):
                C.cache_bytes(folder)
            link = folder / "link"
            link.symlink_to(regular)
            with self.assertRaises(RuntimeError):
                C.cache_bytes(folder)
            link.unlink()
            fifo = folder / "fifo"
            os.mkfifo(fifo)
            with self.assertRaises(RuntimeError):
                C.cache_bytes(folder)

    def test_retained_iterations_reject_before_collection_or_deletion(self):
        root = Path(__file__).resolve().parent
        for mutation in ("extra", "symlink", "fifo", "changed", "missing", "root-alias"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                folder = Path(temporary)
                raw = folder / "raw"
                iterations = raw / "iterations"
                shutil.copytree(root / "raw/iterations", iterations)
                self.assertEqual(C.owned_iterations(raw), V.iteration_inventory(raw))
                entry = iterations / "check-initial.log"
                if mutation == "extra":
                    (iterations / "unrecorded.log").write_text("unexpected")
                elif mutation == "symlink":
                    entry.unlink()
                    entry.symlink_to(iterations / "focused-initial.log")
                elif mutation == "fifo":
                    entry.unlink()
                    os.mkfifo(entry)
                elif mutation == "changed":
                    entry.write_text("substituted")
                elif mutation == "missing":
                    entry.unlink()
                else:
                    raw.rename(folder / "original")
                    raw.symlink_to(folder / "original", target_is_directory=True)
                with patch.object(C, "HERE", folder), patch.object(C.R, "helpers") as helpers, \
                        patch.object(C.shutil, "rmtree") as remove:
                    with self.assertRaises(RuntimeError):
                        C.main()
                    helpers.assert_not_called()
                    remove.assert_not_called()

    def test_full_campaign_rejection_precedes_cache_deletion(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            evidence = folder / "evidence"
            shutil.copytree(Path(__file__).resolve().parent / "raw/iterations", evidence / "raw/iterations")
            private = folder / "private"
            source = private / "campaign1"
            source.mkdir(parents=True)
            target = private / "target"
            target.mkdir()
            marker = target / "retained"
            marker.write_text("cache")
            for name in ("inputs-before.json", "inputs-after.json"):
                (source / name).write_text(json.dumps({"source": {}}))
            with patch.object(C, "HERE", evidence), patch.object(C.R, "PRIVATE", private), \
                    patch.object(C.R, "helpers"), patch.object(C.R, "inputs", return_value={"source": {}}), \
                    patch.object(C.V, "validate_campaign", side_effect=RuntimeError("rejected roster")) as validate, \
                    patch.object(C.shutil, "rmtree") as remove:
                with self.assertRaisesRegex(RuntimeError, "rejected roster"):
                    C.main()
                validate.assert_called_once_with(source, retained=False)
                remove.assert_not_called()
                self.assertTrue(marker.is_file())

    def test_cleanup_preserves_primary_failure_and_records_success(self):
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            target = folder / "target"
            target.mkdir()
            (target / "file").write_text("cache")
            records = []
            def write(_path, row):
                records.append(dict(row))
            result = C.remove_cache(target, folder / "receipt", 4096, write)
            self.assertTrue(result["absent"])
            self.assertEqual([row["absent"] for row in records], [False, True])
            self.assertFalse(target.exists())
            target.mkdir()
            primary = OSError("deletion failed")
            secondary = RuntimeError("receipt failed")
            records.clear()
            def failing_write(path, row):
                write(path, row)
                if len(records) == 2:
                    raise secondary
            with patch.object(C.shutil, "rmtree", side_effect=primary):
                with self.assertRaises(OSError) as caught:
                    C.remove_cache(target, folder / "receipt", 4096, failing_write)
            self.assertIs(caught.exception, primary)
            self.assertIs(caught.exception.__cause__, secondary)
            self.assertEqual([row["absent"] for row in records], [False, False])
            with patch.object(C.shutil, "rmtree") as remove:
                def initial_failure(_path, _row):
                    raise secondary
                with self.assertRaises(RuntimeError) as caught:
                    C.remove_cache(target, folder / "receipt", 4096, initial_failure)
                self.assertIs(caught.exception, secondary)
                remove.assert_not_called()
            self.assertTrue(target.is_dir())

    def test_named_roster_and_counts(self):
        valid = "test a ... ok\ntest b ... ignored, hardware\n" + (
            "test result: ok. 1 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.1s\n"
        )
        self.assertEqual(V.test_roster(valid, [1, 0, 1, 0, 0])[0], {"a": "ok", "b": "ignored"})
        mutations = (
            valid.replace("test b", "test a"), valid.replace("test a ... ok\n", ""),
            valid.replace("test a ... ok", "test a ... FAILED"), valid + valid,
            valid.replace("0 filtered", "1 filtered"), valid.replace("1 passed", "0 passed"),
            valid.replace("0 failed", "1 failed"), valid.replace("0 measured", "1 measured"),
            valid.replace("test result: ok.", "test result: FAILED."),
        )
        for mutant in mutations:
            with self.subTest(mutant=mutant), self.assertRaises(RuntimeError):
                V.test_roster(mutant, [1, 0, 1, 0, 0])

    def test_command_disposition_and_controls(self):
        command = ["cargo", "test"]
        valid = {"command": command, "cwd": str(V.R.REPO), "started_ns": 1, "timeout_seconds": 30,
                 "pid": 1, "exit": 0, "error": None, "group_absent": True, "environment": V.environment(),
                 "stdin_sha256": None, "finished_ns": 2, "stdout_sha256": "a", "stderr_sha256": "b"}
        self.assertEqual(V.receipt(valid, command, 30, 0), 2)
        mutations = (("command", ["cargo", "check"]), ("cwd", "/tmp"), ("timeout_seconds", 31),
                     ("pid", 0), ("exit", False), ("exit", 101), ("error", "timeout"), ("group_absent", False),
                     ("environment", {}), ("stdin_sha256", "injected"), ("finished_ns", 1),
                     ("finished_ns", 46 * 10**9), ("started_ns", True))
        for key, value in mutations:
            mutant = copy.deepcopy(valid)
            mutant[key] = value
            with self.subTest(key=key, value=value), self.assertRaises(RuntimeError):
                V.receipt(mutant, command, 30, 0)
        with self.assertRaises(RuntimeError):
            V.receipt(valid, command, 30, 2)
        with self.assertRaises(RuntimeError):
            V.receipt(valid | {"unrecorded": True}, command, 30, 0)


if __name__ == "__main__":
    unittest.main()
