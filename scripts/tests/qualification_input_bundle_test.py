#!/usr/bin/python3
"""Adversarial controls for inert qualification transport snapshots."""

import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import qualification_input_bundle as bundle


class BundleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="fe2o3-input-bundle-test-")
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.source.mkdir()
        (self.source / "empty").mkdir()
        (self.source / "data").write_bytes(b"a\0b\xff\n")
        (self.source / "empty-file").touch()
        (self.source / "tool").write_bytes(b"#!/bin/sh\nexit 91\n")
        self.source.chmod(0o755)
        (self.source / "empty").chmod(0o755)
        (self.source / "data").chmod(0o644)
        (self.source / "empty-file").chmod(0o644)
        (self.source / "tool").chmod(0o755)
        self.output = self.root / "bundle"

    def tearDown(self):
        for directory, _, _ in os.walk(self.root, followlinks=False):
            os.chmod(directory, 0o700)
        self.temporary.cleanup()

    def prepare(self, policy="exact"):
        return bundle.prepare_tree(self.source, self.output, policy)

    def rewrite_manifest(self, mutate=None, raw=None):
        path = self.output / bundle.MANIFEST
        if raw is None:
            value = json.loads(path.read_bytes())
            mutate(value)
            raw = bundle.canonical_json(value)
        path.chmod(0o644)
        path.write_bytes(raw)
        path.chmod(0o444)
        return hashlib.sha256(raw).hexdigest()

    def test_exact_modes_empty_directories_and_binary_content(self):
        digest = self.prepare()
        self.assertEqual(bundle.verify(self.output, digest)["mode_policy"], "exact")
        for relative in ("", "empty", "data", "empty-file", "tool"):
            self.assertEqual(stat.S_IMODE((self.source / relative).stat().st_mode),
                             stat.S_IMODE((self.output / "data" / relative).stat().st_mode))
        self.assertEqual((self.output / "data/data").read_bytes(), b"a\0b\xff\n")

    def application_metadata(self):
        self.prepare()
        environment = {key: "a" * 64 for key in bundle.PIN_NAMES}
        metadata = {
            "profile": "genuine-application", "source_root": "/home/owner/repo",
            "fixture": bundle.FIXTURE, "pins": environment.copy(),
            "source_excluded_prefix": "docs/evidence/",
            "host_linker": "installed-host-premise-not-in-this-bundle",
        }
        digest = self.rewrite_manifest(lambda value: value.update(metadata=metadata))
        return digest, environment

    def test_application_metadata_binds_caller_pins_and_projection(self):
        digest, environment = self.application_metadata()
        self.assertEqual(bundle.verify_application(self.output, digest, environment),
                         "/home/owner/repo")
        for key in bundle.PIN_NAMES:
            with self.subTest(pin=key):
                wrong = environment.copy()
                wrong[key] = "b" * 64
                with self.assertRaisesRegex(bundle.HardeningError, "pin differs"):
                    bundle.verify_application(self.output, digest, wrong)
                del wrong[key]
                with self.assertRaises(bundle.HardeningError):
                    bundle.verify_application(self.output, digest, wrong)

    def test_application_metadata_rejects_profile_scope_and_path_changes(self):
        _, environment = self.application_metadata()
        original = (self.output / bundle.MANIFEST).read_bytes()
        mutations = (
            lambda m: m.update(extra=True),
            lambda m: m.update(profile="tree"),
            lambda m: m.update(fixture="other"),
            lambda m: m.update(source_excluded_prefix="crates/"),
            lambda m: m.update(host_linker="bundled"),
            lambda m: m.update(source_root="/home/owner/../repo"),
            lambda m: m.update(source_root="/tmp/repo"),
            lambda m: m.update(source_root=["/home/owner/repo"]),
            lambda m: m["pins"].update(extra="a" * 64),
            lambda m: m["pins"].update({bundle.PIN_NAMES[0]: 3}),
        )
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                self.rewrite_manifest(raw=original)
                digest = self.rewrite_manifest(lambda value: mutate(value["metadata"]))
                with self.assertRaises(bundle.HardeningError):
                    bundle.verify_application(self.output, digest, environment)

    def test_private_stage_requires_root_before_reading_inputs(self):
        with patch.object(bundle.os, "geteuid", return_value=1000):
            with self.assertRaisesRegex(bundle.HardeningError, "root credentials"):
                bundle.stage_application(self.output, "a" * 64, self.root / "stage", {})

    def test_shell_verification_ignores_inherited_python_environment(self):
        digest, environment = self.application_metadata()
        hostile = self.root / "hostile"
        hostile.mkdir()
        sentinel = self.root / "unexpected-import"
        (hostile / "sitecustomize.py").write_text(
            f"open({str(sentinel)!r}, 'w').write('untrusted import')\n", encoding="ascii")
        environment.update(PATH="/usr/bin:/bin", PYTHONPATH=str(hostile),
                           PYTHONHOME="/nonexistent", PYTHONUSERBASE=str(hostile),
                           FE2O3_GENUINE_INPUT_BUNDLE=str(self.output),
                           FE2O3_GENUINE_INPUT_SHA256=digest)
        repo = Path(bundle.__file__).resolve().parents[1]
        result = subprocess.run(
            ["/bin/bash", "-c",
             'set -euo pipefail; source "$1"; repo=$2; campaign=genuine; '
             'prepare_application_input_bundle; printf "%s" "$FE2O3_GENUINE_INPUT_SOURCE_ROOT"',
             "qualification-test", str(repo / "scripts/qualify-application-inputs.sh"), str(repo)],
            env=environment, capture_output=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        self.assertEqual(result.stdout, b"/home/owner/repo")
        self.assertFalse(sentinel.exists())

    @unittest.skipUnless(os.geteuid() == 0 and os.getegid() == 0, "real-root staging control")
    def test_private_stage_severs_transport_aliases(self):
        digest, environment = self.application_metadata()
        destination = self.root / "stage"
        self.assertEqual(bundle.stage_application(self.output, digest, destination, environment),
                         "/home/owner/repo")
        copied = destination / "copied"
        original = self.output / "data/data"
        self.assertNotEqual(original.stat().st_ino, (copied / "data/data").stat().st_ino)
        original.write_bytes(b"transport changed later")
        bundle.verify_application(copied, digest, environment)
        self.assertEqual((copied / "data/data").read_bytes(), b"a\0b\xff\n")
        for path in copied.rglob("*"):
            self.assertEqual((path.stat().st_uid, path.stat().st_gid), (0, 0))

    @unittest.skipUnless(os.geteuid() == 0 and os.getegid() == 0, "real-root staging control")
    def test_private_stage_rejects_writable_parent_and_preexisting_destination(self):
        digest, environment = self.application_metadata()
        destination = self.root / "stage"
        self.root.chmod(0o777)
        with self.assertRaisesRegex(bundle.HardeningError, "private stage parent"):
            bundle.stage_application(self.output, digest, destination, environment)
        self.root.chmod(0o700)
        destination.mkdir()
        sentinel = destination / "sentinel"
        sentinel.write_bytes(b"unrelated")
        with self.assertRaisesRegex(bundle.HardeningError, "already exists"):
            bundle.stage_application(self.output, digest, destination, environment)
        self.assertEqual(sentinel.read_bytes(), b"unrelated")

    @unittest.skipUnless(os.geteuid() == 0 and os.getegid() == 0, "real-root staging control")
    def test_private_stage_rejects_capacity_before_creating_destination(self):
        digest, environment = self.application_metadata()
        destination = self.root / "stage"
        capacity = os.statvfs_result((4096, 4096, 100, 0, 0, 100, 0, 0, 0, 255))
        with patch.object(bundle.os, "statvfs", return_value=capacity):
            with self.assertRaisesRegex(bundle.HardeningError, "insufficient space"):
                bundle.stage_application(self.output, digest, destination, environment)
        self.assertFalse(destination.exists())

    @unittest.skipUnless(os.geteuid() == 0 and os.getegid() == 0, "real-root staging control")
    def test_private_stage_rejects_copy_mutation_and_removes_only_its_output(self):
        digest, environment = self.application_metadata()
        destination = self.root / "stage"
        original = bundle.copy_tree

        def corrupt(source, target, policy):
            original(source, target, policy)
            (target / "data/data").write_bytes(b"corrupt copied bytes")

        with patch.object(bundle, "copy_tree", corrupt):
            with self.assertRaisesRegex(bundle.HardeningError, "content/inventory"):
                bundle.stage_application(self.output, digest, destination, environment)
        self.assertFalse(destination.exists())
        bundle.verify_application(self.output, digest, environment)

    def test_private_source_becomes_searchable_without_changing_original(self):
        self.source.chmod(0o700)
        (self.source / "data").chmod(0o600)
        digest = self.prepare("read-only")
        bundle.verify(self.output, digest)
        self.assertEqual(stat.S_IMODE(self.source.stat().st_mode), 0o700)
        self.assertEqual(stat.S_IMODE((self.source / "data").stat().st_mode), 0o600)
        self.assertEqual(stat.S_IMODE((self.output / "data").stat().st_mode), 0o555)
        self.assertEqual(stat.S_IMODE((self.output / "data/data").stat().st_mode), 0o444)
        self.assertEqual(stat.S_IMODE((self.output / "data/tool").stat().st_mode), 0o555)

    def test_transport_changes_identity_not_content(self):
        digest = self.prepare()
        transported = self.root / "transported"
        shutil.copytree(self.output, transported)
        os.utime(transported / "data/data", ns=(1, 2))
        self.assertNotEqual((self.output / "data/data").stat().st_ino,
                            (transported / "data/data").stat().st_ino)
        bundle.verify(transported, digest)

    def test_wrong_independent_digest(self):
        self.prepare()
        with self.assertRaisesRegex(bundle.HardeningError, "SHA256 differs"):
            bundle.verify(self.output, "0" * 64)

    def test_duplicate_json_keys_reject_even_with_matching_digest(self):
        self.prepare()
        raw = (self.output / bundle.MANIFEST).read_bytes()
        raw = raw.replace(b'{"entries":', b'{"schema":"duplicate","entries":', 1)
        digest = self.rewrite_manifest(raw=raw)
        with self.assertRaisesRegex(bundle.HardeningError, "duplicate JSON"):
            bundle.verify(self.output, digest)

    def test_noncanonical_json_rejects(self):
        self.prepare()
        raw = b" " + (self.output / bundle.MANIFEST).read_bytes()
        digest = self.rewrite_manifest(raw=raw)
        with self.assertRaisesRegex(bundle.HardeningError, "noncanonical"):
            bundle.verify(self.output, digest)

    def test_malformed_inventories_reject(self):
        self.prepare()
        original = (self.output / bundle.MANIFEST).read_bytes()
        mutations = (
            lambda v: v["entries"].reverse(),
            lambda v: v["entries"].append(v["entries"][0]),
            lambda v: v["entries"][1].update(path="../data"),
            lambda v: v["entries"][1].update(path="/data"),
            lambda v: v["entries"][1].update(path="a//data"),
            lambda v: v["entries"][1].update(unknown=1),
            lambda v: v.update(unknown=1),
            lambda v: v.update(entry_count=True),
            lambda v: v.update(total_bytes=0),
        )
        for mutate in mutations:
            with self.subTest(mutation=mutate):
                self.rewrite_manifest(raw=original)
                digest = self.rewrite_manifest(mutate)
                with self.assertRaises(bundle.HardeningError):
                    bundle.verify(self.output, digest)

    def test_extra_outer_entry_rejects(self):
        digest = self.prepare()
        self.output.chmod(0o755)
        (self.output / "extra").touch()
        self.output.chmod(0o555)
        with self.assertRaises(bundle.HardeningError):
            bundle.verify(self.output, digest)

    def test_changed_content_rejects(self):
        digest = self.prepare()
        (self.output / "data/data").write_bytes(b"changed")
        with self.assertRaisesRegex(bundle.HardeningError, "content/inventory"):
            bundle.verify(self.output, digest)

    def test_changed_mode_rejects(self):
        digest = self.prepare()
        (self.output / "data/data").chmod(0o444)
        with self.assertRaisesRegex(bundle.HardeningError, "content/inventory"):
            bundle.verify(self.output, digest)

    def test_missing_and_extra_empty_directories_reject(self):
        digest = self.prepare()
        (self.output / "data/empty").rmdir()
        with self.assertRaises(bundle.HardeningError):
            bundle.verify(self.output, digest)
        (self.output / "data/empty").mkdir(mode=0o755)
        (self.output / "data/extra").mkdir()
        with self.assertRaises(bundle.HardeningError):
            bundle.verify(self.output, digest)

    def test_symlink_hardlink_fifo_reject(self):
        digest = self.prepare()
        extra = self.output / "data/extra"
        for make in (lambda: extra.symlink_to("data"),
                     lambda: os.link(self.source / "data", extra),
                     lambda: os.mkfifo(extra)):
            with self.subTest(make=make):
                make()
                with self.assertRaises(bundle.HardeningError):
                    bundle.verify(self.output, digest)
                extra.unlink()

    def test_source_fifo_rejects_without_blocking_or_publication(self):
        os.mkfifo(self.source / "fifo")
        with self.assertRaises(bundle.HardeningError):
            self.prepare()
        self.assertFalse(self.output.exists())

    def test_cache_hardlinks_become_independent_copies(self):
        os.link(self.source / "data", self.source / "alias")
        copied = self.root / "copied"
        bundle.copy_tree(self.source, copied, "read-only", hardlinks=True)
        self.assertEqual((copied / "alias").read_bytes(), (copied / "data").read_bytes())
        self.assertEqual((copied / "alias").stat().st_nlink, 1)
        self.assertNotEqual((copied / "alias").stat().st_ino, (copied / "data").stat().st_ino)
        with self.assertRaises(bundle.HardeningError):
            self.prepare()

    def test_source_mutation_fails_without_publishing(self):
        original = bundle.copy_file

        def changed(*args, **kwargs):
            original(*args, **kwargs)
            (self.source / "data").write_bytes(b"mutated")

        with patch.object(bundle, "copy_file", changed):
            with self.assertRaisesRegex(bundle.HardeningError, "source changed"):
                self.prepare()
        self.assertFalse(self.output.exists())

    def test_preexisting_destination_is_never_removed(self):
        self.output.mkdir()
        sentinel = self.output / "sentinel"
        sentinel.write_bytes(b"unrelated")
        with self.assertRaises(FileExistsError):
            self.prepare()
        self.assertEqual(sentinel.read_bytes(), b"unrelated")

    def test_selected_parent_symlink_rejects(self):
        (self.source / "link").symlink_to(self.source / "empty", target_is_directory=True)
        with self.assertRaises((OSError, bundle.HardeningError)):
            bundle.copy_tree(self.source, self.output, "read-only", selected=["link/file"])
        self.assertFalse(self.output.exists())

    def test_exact_unreadable_modes_and_special_bits_reject(self):
        for mode in (0o600, 0o4644):
            (self.source / "data").chmod(mode)
            with self.assertRaises(bundle.HardeningError):
                self.prepare()
            self.assertFalse(self.output.exists())

    def test_low_descriptor_limit_is_independent_of_file_count(self):
        for number in range(128):
            (self.source / f"f{number}").write_bytes(b"content")
        code = (
            "import resource,sys; from pathlib import Path; "
            "sys.path.insert(0,sys.argv[1]); import qualification_input_bundle as b; "
            "resource.setrlimit(resource.RLIMIT_NOFILE,(32,32)); "
            "d=b.prepare_tree(Path(sys.argv[2]),Path(sys.argv[3]),'read-only'); "
            "b.verify(Path(sys.argv[3]),d)"
        )
        result = subprocess.run([sys.executable, "-B", "-c", code,
                                 str(Path(bundle.__file__).parent), str(self.source), str(self.output)],
                                capture_output=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stderr.decode())

    def test_small_entry_bound_rejects_before_copy(self):
        with patch.object(bundle, "MAX_SNAPSHOT_FILES", 2):
            with self.assertRaisesRegex(bundle.HardeningError, "entry bound"):
                self.prepare()
        self.assertFalse(self.output.exists())

    def test_selected_entry_bound_is_incremental(self):
        with patch.object(bundle, "MAX_SNAPSHOT_FILES", 2):
            with self.assertRaisesRegex(bundle.HardeningError, "entry bound"):
                bundle.copy_tree(self.source, self.output, "read-only", selected=["a/b/c"])
        self.assertFalse(self.output.exists())

    def test_aggregate_budget_rejects_before_second_copy(self):
        budget = bundle.CopyBudget()
        with patch.object(bundle, "MAX_CLOSURE_TOTAL_BYTES", 40):
            bundle.copy_tree(self.source, self.root / "first", "read-only", budget=budget)
            with self.assertRaisesRegex(bundle.HardeningError, "aggregate copy budget"):
                bundle.copy_tree(self.source, self.output, "read-only", budget=budget)
        self.assertFalse(self.output.exists())

    def test_cleanup_rejects_substituted_root_and_does_not_follow_children(self):
        self.output.mkdir()
        identity = bundle.stat_identity(self.output.stat())[:2]
        moved = self.root / "moved"
        self.output.rename(moved)
        self.output.symlink_to(self.source, target_is_directory=True)
        with self.assertRaises(OSError):
            bundle.remove_owned_tree(self.output, identity)
        self.assertEqual(stat.S_IMODE(self.source.stat().st_mode), 0o755)
        self.output.unlink()
        moved.rename(self.output)
        (self.output / "link").symlink_to(self.source, target_is_directory=True)
        bundle.remove_owned_tree(self.output, identity)
        self.assertTrue((self.source / "data").exists())
        self.assertEqual(stat.S_IMODE(self.source.stat().st_mode), 0o755)

    def test_cleanup_rejects_replaced_directory_identity(self):
        self.output.mkdir()
        identity = bundle.stat_identity(self.output.stat())[:2]
        self.output.rename(self.root / "moved")
        self.output.mkdir()
        (self.output / "sentinel").touch()
        with self.assertRaisesRegex(bundle.HardeningError, "root was replaced"):
            bundle.remove_owned_tree(self.output, identity)
        self.assertTrue((self.output / "sentinel").exists())

    def test_tracked_inventory_ignores_inherited_git_environment(self):
        repo = self.root / "repo"
        repo.mkdir()
        subprocess.run(["/usr/bin/git", "init", "-q", str(repo)], check=True)
        (repo / "Cargo.toml").write_bytes(b"manifest")
        subprocess.run(["/usr/bin/git", "-C", str(repo), "add", "Cargo.toml"], check=True)
        with patch.dict(os.environ, {"GIT_INDEX_FILE": "/nonexistent", "GIT_DIR": "/nonexistent",
                                     "PATH": "/nonexistent"}):
            self.assertEqual(bundle.tracked_source_paths(repo), ["Cargo.toml"])

    def test_git_reader_rejects_errors_and_reaps_children(self):
        original = subprocess.Popen
        cases = (
            ("import os; os.write(1,b'Cargo.toml\\0'); raise SystemExit(17)", 32 * 1024 * 1024, 5),
            ("import os,time; os.write(1,b'x'*1024); time.sleep(60)", 8, 5),
            ("import time; time.sleep(60)", 32 * 1024 * 1024, 0.05),
            ("import os,time; os.close(1); time.sleep(60)", 32 * 1024 * 1024, 0.05),
        )
        for code, limit, deadline in cases:
            processes = []

            def start(_command, **kwargs):
                child = original([sys.executable, "-c", code], **kwargs)
                processes.append(child)
                return child

            with self.subTest(code=code), patch.object(bundle.subprocess, "Popen", start), \
                    patch.object(bundle, "MAX_MANIFEST_BYTES", limit), \
                    patch.object(bundle, "GIT_TIMEOUT_SECONDS", deadline):
                with self.assertRaises((bundle.HardeningError, subprocess.TimeoutExpired)):
                    bundle.tracked_source_paths(self.source)
            self.assertEqual(len(processes), 1)
            self.assertIsNotNone(processes[0].poll())
            self.assertTrue(processes[0].stdout.closed)

    def test_wrong_manifest_hash_rejects_before_json_parser(self):
        self.prepare()
        with patch.object(bundle.json, "loads", side_effect=AssertionError("parsed unapproved bytes")):
            with self.assertRaisesRegex(bundle.HardeningError, "SHA256 differs"):
                bundle.verify(self.output, "0" * 64)


if __name__ == "__main__":
    unittest.main()
