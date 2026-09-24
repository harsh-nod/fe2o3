#!/usr/bin/env python3
"""CPU-only fixed-controls and collection-before-cleanup calibration."""

import importlib.util
import hashlib
import json
import os
from pathlib import Path
import shlex
import tempfile
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
SPEC = importlib.util.spec_from_file_location("backing_budget_campaign", HERE / "xgmi_backing_budget_campaign.py")
C = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(C)
VALUES = ["5,0000:a6:00.0,0xb7baafd0fb173d8e", "6,0000:c6:00.0,0x10a254ce4987e716"]


class OwnerCampaign(unittest.TestCase):
    def test_qualified_production_rejects_unqualified_workspace_examples(self):
        permitted = ("crates/fe2o3-runtime/examples/" + C.EXAMPLE + ".rs\n").encode()
        for changed, accepted in [(b"", True), (permitted, True),
                                  (b"examples/atomic/Cargo.toml\n", False),
                                  (b"crates/fe2o3-kfd/src/lib.rs\n", False),
                                  (permitted + b"Cargo.lock\n", False)]:
            with self.subTest(changed=changed), mock.patch.object(C, "git", side_effect=[b"", changed]) as git:
                if accepted:
                    C.qualified_production("1" * 40)
                else:
                    with self.assertRaisesRegex(RuntimeError, "unchanged qualified production"):
                        C.qualified_production("1" * 40)
                self.assertEqual(git.call_args_list[0], mock.call("merge-base", "--is-ancestor", C.QUALIFIED, "1" * 40))
                self.assertIn("examples", git.call_args_list[1].args)

    def test_helpers_reject_tampered_or_symlink_bytes_before_execution(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "helper.py"
            raw = b"raise AssertionError('helper executed')\n"
            path.write_bytes(raw)
            digest = hashlib.sha256(raw).hexdigest()
            alias = Path(temp) / "alias.py"
            alias.symlink_to(path)
            for candidate, expected in [(path, "0" * 64), (alias, digest)]:
                with self.subTest(path=candidate), mock.patch("builtins.compile") as compiler:
                    with self.assertRaises(RuntimeError):
                        C.module("refused_helper", candidate, expected)
                    compiler.assert_not_called()
            with self.assertRaisesRegex(AssertionError, "helper executed"):
                C.module("authenticated_helper", path, digest)

    def test_git_authority_ignores_ambient_redirects(self):
        hostile = {"GIT_DIR": "/other", "GIT_WORK_TREE": "/other", "GIT_INDEX_FILE": "/other",
                   "GIT_OBJECT_DIRECTORY": "/other", "GIT_REPLACE_REF_BASE": "refs/other",
                   "GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "core.worktree",
                   "GIT_CONFIG_VALUE_0": "/other", "PATH": "/other"}
        with mock.patch.dict(os.environ, hostile), mock.patch.object(C.subprocess, "check_output", return_value=b"ok") as run:
            self.assertEqual(C.git("rev-parse", "HEAD"), b"ok")
        run.assert_called_once_with([*C.GIT, "rev-parse", "HEAD"], cwd=C.ROOT, env=C.GIT_ENV)
        self.assertEqual(C.GIT[0:2], ["/usr/bin/git", "--no-replace-objects"])
        self.assertFalse(set(hostile).difference({"PATH"}).intersection(C.GIT_ENV))
        self.assertEqual(C.GIT_ENV["GIT_CONFIG_GLOBAL"], "/dev/null")
        self.assertEqual(C.GIT_ENV["GIT_CONFIG_SYSTEM"], "/dev/null")

    def test_remote_bootstrap_authenticates_before_entrypoint(self):
        bootstrap = compile(C.BOOTSTRAP, "bootstrap-test", "exec")
        with tempfile.TemporaryDirectory() as temp:
            owned = Path(temp)
            native = owned / "native.py"
            raw = (b"assert __name__ == '__main__'\nimport sys\n"
                   b"assert sys.argv[1] == 'run'\nraise RuntimeError('entered authenticated native')\n")
            native.write_bytes(raw)
            binding = {"payload": {"native.py": hashlib.sha256(raw).hexdigest()}}
            binding_path = owned / "binding.json"
            binding_path.write_text(json.dumps(binding))
            marker = {"path": temp, "binding_sha256": C.H.sha(binding_path)}

            def run(value):
                serialized = json.dumps(value)
                command = C.native_command(serialized)
                self.assertEqual(command[:-1], ["ssh", "-T", *C.C.SSH, "mi300x"])
                self.assertEqual(shlex.split(command[-1]), ["/usr/bin/python3", "-I", "-B", "-c", C.BOOTSTRAP, serialized])
                with mock.patch.object(C.sys, "argv", ["bootstrap", serialized]):
                    exec(bootstrap, {})

            with self.assertRaisesRegex(RuntimeError, "entered authenticated native"):
                run(marker)
            with self.assertRaisesRegex(RuntimeError, "bootstrap binding digest"):
                run({**marker, "binding_sha256": "0" * 64})
            native.write_bytes(raw + b"# changed\n")
            with self.assertRaisesRegex(RuntimeError, "bootstrap native digest"):
                run(marker)
            native.unlink()
            alias = owned / "alias.py"
            alias.write_bytes(raw)
            native.symlink_to(alias)
            with self.assertRaisesRegex(RuntimeError, "ordinary bootstrap input"):
                run(marker)

    def test_device_controls_are_exact_and_distinct(self):
        self.assertEqual(C.devices_from_args(VALUES), [[5, "0000:a6:00.0", "0xb7baafd0fb173d8e"], [6, "0000:c6:00.0", "0x10a254ce4987e716"]])
        for values in [[], VALUES[:1], VALUES + VALUES[:1], [VALUES[0]] * 2,
                       [VALUES[0], VALUES[1].replace("6,", "5,")],
                       [VALUES[0], VALUES[1].replace("c6", "a6")],
                       [VALUES[0], VALUES[1].replace("0x10a254ce4987e716", "0xb7baafd0fb173d8e")],
                       [VALUES[0], VALUES[1].replace("0x10a254ce4987e716", "0x0000000000000000")],
                       [VALUES[0].replace("a6", "A6"), VALUES[1]],
                       [VALUES[0].replace("5,", "8,"), VALUES[1]]]:
            with self.subTest(values=values), self.assertRaises((RuntimeError, ValueError)):
                C.devices_from_args(values)

    def test_outer_campaign_collects_before_cleanup_and_retains_failed_collection(self):
        for failure in (None, "native", "collect"):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / "source"
                root.mkdir()
                observer = root / "benchmarks/runtime_gfx942/copy-host-observe.py"
                observer.parent.mkdir(parents=True)
                observer.write_bytes(b"inert observer; never execute\n")
                target = Path(temp) / "target"
                binary = target / "x86_64-unknown-linux-musl/release/examples" / C.EXAMPLE
                binary.parent.mkdir(parents=True)
                binary.write_bytes(b"inert ELF; never execute\n")
                output = Path(temp) / "results"
                output.mkdir()
                calls, commands, git_calls = [], {}, []

                def git(*args):
                    git_calls.append(args)
                    if args == ("rev-parse", "--show-toplevel"):
                        return str(root).encode() + b"\n"
                    if args == ("rev-parse", "HEAD"):
                        return b"1" * 40 + b"\n"
                    if args == ("rev-parse", "1" * 40 + "^{tree}"):
                        return b"2" * 40 + b"\n"
                    if args[:1] in (("status",), ("diff",), ("merge-base",)):
                        return b""
                    raise AssertionError(args)

                class Recorder:
                    def __init__(self, folder, cwd):
                        self.output = folder
                        folder.mkdir()

                    def run(self, name, command, seconds, **options):
                        calls.append(name)
                        commands[name] = command
                        folder = self.output / name
                        folder.mkdir()
                        (folder / "stdout").write_bytes(b"{}\n")
                        (folder / "stderr").write_bytes(b"")
                        if name == failure:
                            raise RuntimeError("injected " + name)
                        if name == "collect":
                            (output / "remote").mkdir()
                        return folder

                argv = ["campaign", "--target-dir", str(target)]
                for value in VALUES:
                    argv.extend(["--device", value])
                with (
                    mock.patch.object(C, "ROOT", root),
                    mock.patch.object(C, "git", side_effect=git),
                    mock.patch.object(C, "source_identity", return_value={"fixture": "0" * 64}),
                    mock.patch.object(C.sys, "argv", argv),
                    mock.patch.object(C.tempfile, "mkdtemp", return_value=str(output)),
                    mock.patch.object(C.B, "Recorder", Recorder),
                    mock.patch.object(C.signal, "signal"),
                    mock.patch.object(C.subprocess, "Popen", side_effect=AssertionError("CPU test spawned process")),
                    mock.patch("builtins.print"),
                ):
                    if failure:
                        with self.assertRaisesRegex(RuntimeError, "campaign did not qualify"):
                            C.main()
                    else:
                        C.main()
                self.assertIn(("merge-base", "--is-ancestor", C.QUALIFIED, "1" * 40), git_calls)
                self.assertTrue(any(args[:2] == ("diff", "--name-only") for args in git_calls))
                self.assertEqual(calls.count("native"), 1)
                remote = shlex.split(commands["native"][-1])
                self.assertEqual(remote[:5], ["/usr/bin/python3", "-I", "-B", "-c", C.BOOTSTRAP])
                result = json.loads((output / "collection.json").read_bytes())
                if failure == "collect":
                    self.assertNotIn("cleanup", calls)
                    self.assertNotIn("absence", calls)
                    self.assertFalse(result["owned_cleanup"])
                else:
                    self.assertLess(calls.index("collect"), calls.index("cleanup"))
                    self.assertLess(calls.index("cleanup"), calls.index("absence"))
                    self.assertTrue(result["owned_cleanup"])
                self.assertEqual(bool(result["failures"]), failure is not None)

    def test_one_copy_only_trial_and_private_controller_prefix(self):
        owned = Path(C.N.PREFIX + "0123456789abcdef")
        devices = C.devices_from_args(VALUES)
        self.assertEqual(C.N.ORDER, ("backing-budget",))
        self.assertEqual(C.N.CONTROLS, dict(allocation_bytes=4097, padded_bytes=8192, allocations=3, streams=2,
                directed_copies=2, copy_bytes=1024, checked_bytes=36875, homes=[0, 1, 1],
                device_limits=[[8192, 3], [24576, 2]], coherent_limits=[[4096, 1], [8192, 2]],
                request_bytes=1048576, request_records=16, journal_allocations=16, journal_writers=16,
                pressure_bytes=1, capacity_rejections=2, retries=2, fresh_identity="context",
                pressure_dimensions="inferred-byte-record", offsets=[[32, 17], [64, 2048]],
                observation_timeout_seconds=60))
        self.assertEqual(C.N.trial_specs(owned, devices), [("backing-budget", [str(owned / "kfd-owner"), devices[0][2], devices[1][2]], C.H.environment(owned))])
        self.assertEqual(C.N.PAYLOAD, {"native.py", "results.py", "hot.py", "base.py", "source.tar.gz", "kfd-owner"})
        control = C.C.control_bytes(C.N.PREFIX)
        self.assertIn(("scope['PREFIX'] = " + repr(C.N.PREFIX)).encode(), control)
        self.assertNotIn(b"LD_PRELOAD", str(C.H.environment(owned)).encode())

    def test_controls_reject_bool_for_integer(self):
        for key in ("pressure_bytes", "streams"):
            changed = {**C.N.CONTROLS, key: True}
            self.assertFalse(C.H.same_json(changed, C.N.CONTROLS))

    def test_native_attempt_requires_collection_before_cleanup(self):
        calls = []
        self.assertEqual(C.C.settle_remote(calls.append, collected=False, native_attempted=True), (False, ["remote receipts retained for recovery"]))
        self.assertEqual(calls, [])
        self.assertEqual(C.C.settle_remote(calls.append, collected=True, native_attempted=True), (True, []))
        self.assertEqual(calls, ["cleanup", "absence"])
        calls.clear()

        def fail_cleanup(name):
            calls.append(name)
            if name == "cleanup":
                raise RuntimeError("injected cleanup refusal")

        cleaned, failures = C.C.settle_remote(fail_cleanup, collected=True, native_attempted=True)
        self.assertFalse(cleaned)
        self.assertEqual(calls, ["cleanup", "absence"])
        self.assertEqual(len(failures), 1)


if __name__ == "__main__":
    unittest.main()
