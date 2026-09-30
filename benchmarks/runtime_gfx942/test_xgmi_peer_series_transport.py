#!/usr/bin/env python3
"""Light transport failure controls; no network, compiler or workload invocation."""

import contextlib
import io
import os
from pathlib import Path
import signal
import subprocess
import sys
import tarfile
import tempfile
import types
import unittest
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parent))
import xgmi_peer_series_transport as transport


class TransportControls(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.prefix = str(self.root / "owned.")
        self.patch = mock.patch.object(transport, "PREFIX", self.prefix)
        self.patch.start()
        self.addCleanup(self.patch.stop)

    def owner(self):
        owned = Path(self.prefix + "1" * 16)
        owned.mkdir(mode=0o700)
        source = owned / "source"
        (source / ".git").mkdir(parents=True)
        commit = "1" * 40
        for name in ("HEAD", "shallow"):
            (source / ".git" / name).write_text(commit + "\n", encoding="ascii")
        (source / ".git/config").write_text(
            "[core]\nrepositoryformatversion = 0\nbare = false\nfilemode = true\n", encoding="ascii")
        for name in (transport.NATIVE_RELATIVE, transport.TRANSPORT_RELATIVE):
            target = source / name
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(b"# synthetic source\n")
        binding = {"schema": "fe2o3.xgmi-series-transport.v1", "commit": commit,
                   "devices": [], "files": {"source/" + key: row for key, row in transport.inventory(source).items()},
                   "payload_bytes": 1, "payload_sha256": "0" * 64, "selected_objects": {},
                   "signer": transport.SIGNER, "signer_fingerprint": transport.FINGERPRINT}
        transport.save(owned / "binding.json", binding)
        marker = {"path": str(owned), "commit": commit, "binding_sha256": transport.sha(owned / "binding.json")}
        transport.save(owned / "owner.json", marker)
        return owned, marker, binding

    def closed_evidence(self, owned, marker):
        campaign = owned / "campaign-1"
        campaign.mkdir()
        namespace = os.readlink("/proc/self/ns/pid")
        transport.save(campaign / "fresh-census.json", {"namespace": namespace, "raw_sha256": {}, "attempt_order": []})
        transport.save(campaign / "finished.json", {"native_execution": False, "failures": ["synthetic rejection"]})
        transport.save(owned / "launch.json", {"marker": marker, "fresh_closure": True,
            "original_foreground_execution_returned": True, "namespace": namespace,
            "fresh_census_sha256": transport.sha(campaign / "fresh-census.json"), "failures": ["synthetic rejection"]})
        (owned / "monitor").mkdir()
        transport.save(owned / "monitor/receipt.json", {"group_absent": True, "namespace": namespace})

    def test_filter_required_before_transfer_and_exact_initial_objects(self):
        good = b"10:10 packet: git< fetch=shallow wait-for-done filter\n"
        transport.require_filter_capability(good)
        for raw in (b"", good.replace(b" filter", b""), good + b"warning: filtering not recognized\n"):
            with self.assertRaises(ValueError):
                transport.require_filter_capability(raw)
        commit = "1" * 40
        objects = {commit: {"type": "commit", "bytes": 100}, "2" * 40: {"type": "tree", "bytes": 100}}
        transport.check_initial_objects(objects, commit)
        for extra in ({"3" * 40: {"type": "blob", "bytes": 1}},
                      {"3" * 40: {"type": "commit", "bytes": 1}},
                      {"3" * 40: {"type": "tree", "bytes": transport.INITIAL_PACK_LIMIT}}):
            with self.assertRaises(ValueError):
                transport.check_initial_objects({**objects, **extra}, commit)
        self.assertIn("uploadpack.allowFilter=true", transport.UPLOAD_PACK)
        self.assertEqual(transport.GIT[:2], ["/usr/bin/git", "--no-replace-objects"])

    def test_object_and_json_rosters_are_strict(self):
        oid = "1" * 40
        self.assertEqual(transport.selected_blobs(f"100644 blob {oid}\ta\0".encode()), {"a": oid})
        for raw in (f"100644 blob {oid}\ta".encode(), f"120000 blob {oid}\ta\0".encode(),
                    f"100644 blob {oid}\t../a\0".encode()):
            with self.assertRaises(ValueError):
                transport.selected_blobs(raw)
        for raw in (b'{"a":1,"a":2}', b'{"a":NaN}'):
            with self.assertRaises(ValueError):
                transport.parse_json(raw)

    def test_archive_exact_roundtrip_rejects_missing_extra_alias_and_corruption(self):
        source = self.root / "files"
        source.mkdir()
        (source / "a").write_bytes(b"payload")
        files = transport.inventory(source)
        archive = self.root / "valid.tar.gz"
        transport.make_archive(archive, source, files)
        destination = self.root / "readback"
        destination.mkdir()
        transport.validate_archive(archive, files, destination)
        self.assertEqual(transport.inventory(destination), files)
        for index, name in enumerate(("../a", "a", "extra", "a")):
            malformed = self.root / f"invalid-{index}.tar.gz"
            with tarfile.open(malformed, "x:gz") as output:
                member = tarfile.TarInfo(name)
                member.mode = files["a"]["mode"]
                member.size = len(b"payload")
                if index == 1:
                    member.type = tarfile.SYMTYPE
                    member.linkname = "a"
                    member.size = 0
                output.addfile(member, io.BytesIO(b"corrupt" if index == 3 else b"payload"))
            with self.assertRaises(ValueError):
                transport.validate_archive(malformed, files)
        with self.assertRaises(ValueError):
            transport.bounded_manifest({"a": {**files["a"], "bytes": transport.ARCHIVE_LIMIT + 1}})

    def test_owned_source_refuses_aliases_extra_files_and_changed_marker(self):
        owned, marker, binding = self.owner()
        self.assertEqual(transport.owned_path(marker), owned)
        transport.verify_source(owned, binding)
        (owned / "source/extra").write_bytes(b"unexpected")
        with self.assertRaises(ValueError):
            transport.verify_source(owned, binding)
        (owned / "source/extra").unlink()
        (owned / "source/.git/refs/replace").mkdir(parents=True)
        with self.assertRaises(ValueError):
            transport.verify_source(owned, binding)
        with self.assertRaises(ValueError):
            transport.owned_path({**marker, "commit": "2" * 40})

    def test_git_sanitizer_refuses_alternates_grafts_and_replacements(self):
        for index, relative in enumerate(("objects/info/alternates", "info/grafts", "refs/replace")):
            directory = self.root / f"git-{index}"
            path = directory / relative
            path.parent.mkdir(parents=True)
            path.write_bytes(b"alias")
            with self.assertRaises(ValueError):
                transport.sanitized_git(directory, "1" * 40)

    def test_owned_readonly_pack_metadata_normalization_roundtrips_without_byte_changes(self):
        owned, marker, _ = self.owner()
        gitdir = owned / "source/.git"
        packdir = gitdir / "objects/pack"
        packdir.mkdir(parents=True)
        members = []
        for extension in ("pack", "idx", "rev", "promisor"):
            path = packdir / ("pack-" + "2" * 40 + "." + extension)
            path.write_bytes(extension.encode("ascii"))
            path.chmod(0o600 if extension == "promisor" else 0o400)
            members.append(path)
        before = {path.name: transport.sha(path) for path in members}
        source_mode = (owned / "source" / transport.NATIVE_RELATIVE).stat().st_mode
        transport.sanitized_git(gitdir, marker["commit"])
        self.assertEqual({path.name: transport.sha(path) for path in members}, before)
        self.assertTrue(all(path.stat().st_mode & 0o777 == 0o600 for path in members))
        self.assertEqual((owned / "source" / transport.NATIVE_RELATIVE).stat().st_mode, source_mode)
        files = transport.inventory(owned / "source")
        archive = self.root / "normalized.tar.gz"
        transport.make_archive(archive, owned / "source", files)
        destination = self.root / "normalized-readback"
        destination.mkdir()
        transport.validate_archive(archive, files, destination)
        self.assertEqual(transport.inventory(destination), files)

    def test_owned_pack_normalization_refuses_aliases_special_names_links_and_modes(self):
        owned, marker, _ = self.owner()
        gitdir = owned / "source/.git"
        packdir = gitdir / "objects/pack"
        packdir.mkdir(parents=True)
        name = "pack-" + "2" * 40 + ".pack"
        path = packdir / name
        outside = self.root / "outside"
        outside.write_bytes(b"external")
        cases = {
            "symlink": lambda: path.symlink_to(outside),
            "hardlink": lambda: os.link(outside, path),
            "fifo": lambda: os.mkfifo(path, 0o600),
            "unexpected-mode": lambda: (path.write_bytes(b"pack"), path.chmod(0o644)),
        }
        for label, create in cases.items():
            with self.subTest(label=label):
                create()
                with self.assertRaises(ValueError):
                    transport.sanitized_git(gitdir, marker["commit"])
                path.unlink()
        unknown = packdir / "unexpected.pack"
        unknown.write_bytes(b"pack")
        unknown.chmod(0o400)
        with self.assertRaises(ValueError):
            transport.sanitized_git(gitdir, marker["commit"])
        self.assertEqual(unknown.stat().st_mode & 0o777, 0o400)
        self.assertEqual(outside.read_bytes(), b"external")

    def test_shared_lock_preserved_on_success_failure_and_replacement(self):
        lock = self.root / "shared.lock"
        lock.write_bytes(b"other owners convention\n")
        before = transport.inventory_row(lock)
        with transport.shared_lock(lock, wait_seconds=0) as identity:
            self.assertEqual(identity["inode"], lock.stat().st_ino)
            with self.assertRaises(ValueError):
                with transport.shared_lock(lock, wait_seconds=0):
                    self.fail("must not acquire competing owner lock")
        with self.assertRaisesRegex(RuntimeError, "body"):
            with transport.shared_lock(lock, wait_seconds=0):
                raise RuntimeError("body")
        self.assertEqual(transport.inventory_row(lock), before)
        with self.assertRaisesRegex(ValueError, "continuity"):
            with transport.shared_lock(lock, wait_seconds=0):
                lock.rename(self.root / "old.lock")
                lock.write_bytes(b"replacement")
                raise RuntimeError("body")
        self.assertEqual((self.root / "old.lock").read_bytes(), b"other owners convention\n")

    def test_cleanup_requires_collected_raw_and_fresh_closure_never_pid_probes(self):
        owned, marker, _ = self.owner()
        self.closed_evidence(owned, marker)
        with contextlib.redirect_stdout(io.StringIO()):
            transport.collect(marker)
        collection = transport.parse_json((owned / "collection.json").read_bytes())
        with mock.patch.object(os, "kill", side_effect=AssertionError("historical PID probe")), \
             mock.patch.object(os, "killpg", side_effect=AssertionError("historical group probe")):
            with self.assertRaises(ValueError):
                transport.cleanup(marker, "0" * 64)
            self.assertTrue(owned.exists())
            with contextlib.redirect_stdout(io.StringIO()):
                transport.cleanup(marker, collection["archive_sha256"])
        self.assertFalse(owned.exists())

    def test_mutated_collected_raw_prevents_cleanup(self):
        owned, marker, _ = self.owner()
        self.closed_evidence(owned, marker)
        with contextlib.redirect_stdout(io.StringIO()):
            transport.collect(marker)
        collection = transport.parse_json((owned / "collection.json").read_bytes())
        (owned / "campaign-1/finished.json").write_bytes(b"changed")
        with self.assertRaises(ValueError):
            transport.cleanup(marker, collection["archive_sha256"])
        self.assertTrue(owned.exists())

    def test_monitor_pipe_eof_and_failure_pidfd_notification(self):
        owned, marker, _ = self.owner()
        stream = io.BytesIO(b"")
        with mock.patch.object(transport.select, "select", return_value=([stream], [], [])), \
             mock.patch.object(transport, "resource_observation") as observe, \
             mock.patch.object(os, "close") as close, \
             mock.patch.object(signal, "pidfd_send_signal") as notify:
            self.assertEqual(transport.resource_monitor(marker, 123, stream), 0)
            observe.assert_not_called()
            notify.assert_not_called()
            close.assert_called_once_with(123)
        with mock.patch.object(transport.select, "select", return_value=([], [], [])), \
             mock.patch.object(transport, "resource_observation", side_effect=ValueError("low disk")), \
             mock.patch.object(os, "close"), mock.patch.object(os, "kill", side_effect=AssertionError("PID signal")), \
             mock.patch.object(signal, "pidfd_send_signal") as notify, contextlib.redirect_stderr(io.StringIO()):
            self.assertEqual(transport.resource_monitor(marker, 123, stream), 1)
            notify.assert_called_once_with(123, signal.SIGTERM)

    def test_monitor_guard_is_thread_free_and_closes_fresh_handle(self):
        import threading
        owned, marker, _ = self.owner()
        process = types.SimpleNamespace(pid=456, returncode=0, stdin=io.BytesIO(),
                                        wait=mock.Mock(return_value=0), poll=mock.Mock(return_value=0))
        base = types.SimpleNamespace(MANAGED=(signal.SIGHUP, signal.SIGINT, signal.SIGTERM),
                                     group_exists=mock.Mock(return_value=False), stop=mock.Mock())
        descriptor = os.open(owned / "owner.json", os.O_RDONLY)
        with mock.patch.object(os, "pidfd_open", return_value=descriptor), \
             mock.patch.object(transport.subprocess, "Popen", return_value=process) as popen, \
             mock.patch.object(threading, "Thread", side_effect=AssertionError("watcher thread")):
            with transport.resource_guard(owned, marker, types.SimpleNamespace(B=base)) as receipt:
                self.assertEqual(receipt["pid"], 456)
            self.assertTrue(popen.call_args.kwargs["start_new_session"])
            self.assertEqual(popen.call_args.kwargs["pass_fds"], (descriptor,))
            self.assertTrue(callable(popen.call_args.kwargs["preexec_fn"]))
        self.assertTrue(receipt["group_absent"])
        self.assertEqual(receipt["exit"], 0)
        self.assertIsNone(receipt["error"])
        self.assertTrue(process.stdin.closed)
        base.stop.assert_not_called()
        with self.assertRaises(OSError):
            os.fstat(descriptor)

    def test_monitor_timeout_stops_only_its_fresh_handle(self):
        owned, marker, _ = self.owner()
        process = types.SimpleNamespace(pid=456, returncode=None, stdin=io.BytesIO())
        process.wait = mock.Mock(side_effect=subprocess.TimeoutExpired("monitor", 30))
        process.poll = lambda: process.returncode
        def stop(value):
            self.assertIs(value, process)
            value.returncode = -15
        base = types.SimpleNamespace(MANAGED=(signal.SIGHUP, signal.SIGINT, signal.SIGTERM),
                                     group_exists=lambda pid: process.returncode is None, stop=mock.Mock(side_effect=stop))
        descriptor = os.open(owned / "owner.json", os.O_RDONLY)
        with mock.patch.object(os, "pidfd_open", return_value=descriptor), \
             mock.patch.object(transport.subprocess, "Popen", return_value=process):
            with self.assertRaises(subprocess.TimeoutExpired):
                with transport.resource_guard(owned, marker, types.SimpleNamespace(B=base)):
                    pass
        base.stop.assert_called_once_with(process)
        receipt = transport.parse_json((owned / "monitor/receipt.json").read_bytes())
        self.assertTrue(receipt["group_absent"])
        self.assertEqual(receipt["exit"], -15)

    def test_remote_final_save_failure_restores_handlers_and_argv(self):
        owned, marker, _ = self.owner()
        managed = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)
        previous = {sig: signal.getsignal(sig) for sig in managed}
        original_argv = sys.argv
        base = types.SimpleNamespace(MANAGED=managed, interrupted=lambda *_: None)
        hot = types.SimpleNamespace(B=base)
        native = types.SimpleNamespace(load_helpers=lambda: (hot, None, None))
        original_save = transport.save
        def save(path, value):
            if path.name == "launch.json":
                raise OSError("disk full")
            original_save(path, value)
        with mock.patch.object(transport, "SCRIPT", owned / "source" / transport.TRANSPORT_RELATIVE), \
             mock.patch.object(transport, "native_module", return_value=native), \
             mock.patch.object(transport, "shared_lock", side_effect=ValueError("synthetic admission stop")), \
             mock.patch.object(transport, "save", side_effect=save):
            with self.assertRaisesRegex(OSError, "disk full"):
                transport.remote_run(marker)
        self.assertIs(sys.argv, original_argv)
        self.assertEqual({sig: signal.getsignal(sig) for sig in managed}, previous)

    def execute_uncertain_terminal(self, *, save_failure=False):
        owned, marker, binding = self.owner()
        payload = owned / "source.tar.gz"
        payload.write_bytes(b"x")
        binding["payload_sha256"] = transport.sha(payload)
        (owned / "binding.json").write_bytes(transport.encoded(binding))
        marker["binding_sha256"] = transport.sha(owned / "binding.json")
        (owned / "owner.json").write_bytes(transport.encoded(marker))
        output = owned / "remote-commands"
        output.mkdir()
        attempted = []
        def run(name, command, seconds, **kwargs):
            attempted.append(name)
            self.assertIn(name, ("receive", "remote-run"))
            folder = output / name
            folder.mkdir()
            transport.save(folder / "receipt.json", {"exit": 0 if name == "receive" else 255,
                "error": None, "group_absent": True})
            if name == "remote-run":
                raise RuntimeError("SSH disconnected")
            return folder
        managed = (signal.SIGHUP, signal.SIGINT, signal.SIGTERM)
        previous = {sig: signal.getsignal(sig) for sig in managed}
        hot = types.SimpleNamespace(B=types.SimpleNamespace(MANAGED=managed, interrupted=lambda *_: None))
        recorder = types.SimpleNamespace(output=output, run=run)
        native = types.SimpleNamespace(load_helpers=lambda: (hot, None, None),
            FreshRecorder=lambda *_: recorder, fresh_census=lambda *_: {"synthetic": True})
        original_save = transport.save
        def save(path, value):
            if save_failure and path.name == "transport-census.json":
                raise OSError("disk full")
            original_save(path, value)
        with mock.patch.object(transport, "SCRIPT", owned / "source" / transport.TRANSPORT_RELATIVE), \
             mock.patch.object(transport, "native_module", return_value=native), \
             mock.patch.object(transport, "save", side_effect=save):
            with self.assertRaises(OSError if save_failure else ValueError):
                transport.execute(types.SimpleNamespace(prepared=owned))
        self.assertEqual(attempted, ["receive", "remote-run"])
        self.assertTrue(owned.exists())
        self.assertEqual({sig: signal.getsignal(sig) for sig in managed}, previous)

    def test_uncertain_original_ssh_terminal_never_collects_or_cleans(self):
        self.execute_uncertain_terminal()

    def test_local_final_save_failure_restores_all_handlers(self):
        self.execute_uncertain_terminal(save_failure=True)

    def test_bootstrap_is_bounded_fixed_host_and_original_script_frame(self):
        _, marker, _ = self.owner()
        command = transport.bootstrap_command(b"pass\n", "collect", marker)
        self.assertEqual(command[:len(transport.SSH)], transport.SSH)
        self.assertIn(" 5 collect ", command[-1])
        self.assertIn("-I -B", command[-1])
        with self.assertRaises(ValueError):
            transport.bootstrap_command(b"x" * (128 * 1024 + 1), "collect", marker)


if __name__ == "__main__":
    unittest.main()
