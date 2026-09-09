import argparse
import importlib.util
import json
import pathlib
import tempfile
import unittest
from unittest import mock

spec = importlib.util.spec_from_file_location("r61_owner_test_runner", pathlib.Path(__file__).with_name("run-r61-owner-mi300x.py"))
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class OwnerRunnerTests(unittest.TestCase):
    def test_final_link_profile_is_explicit(self):
        good = 'LC_ALL="C" PATH="/usr/bin" "/usr/bin/cc" "-fuse-ld=bfd" "-static-pie"\n'
        runner.validate_link_command(good)
        for value in ("", good * 2, good.replace("/usr/bin/cc", "cc"),
                      good.replace("-fuse-ld=bfd", "-fuse-ld=lld"), good.replace("-static-pie", "-shared"),
                      *(good.rstrip() + " " + flag for flag in ("-fuse-ld=lld", "-shared", "-no-pie", "-static-pie"))):
            with self.assertRaises(runner.base.RunError):
                runner.validate_link_command(value)

    def test_static_symbols_reject_hidden_forbidden_and_missing_tables(self):
        policy = json.loads((pathlib.Path(__file__).resolve().parents[2] / "scripts/runtime-pure-rust-policy.json").read_text())
        good = "main T 100 10\npthread_create W 200 10\n"
        self.assertEqual(runner.validate_static_symbols(good, policy), 2)
        for value in ("", "nm: no symbols\n", "main T 100 10\n", good + "valid T not-hex 10\n",
                      good + "unknown U\n", good + "unknown w\n", good + "unknown v\n"):
            with self.assertRaises(runner.base.RunError):
                runner.validate_static_symbols(value, policy)
        for name in (*policy["forbidden_dynamic_symbols"], "__dlsym", "dlsym@GLIBC_2.34",
                     "__pthread_get_minstack", *(p + "probe" for p in policy["forbidden_dynamic_symbol_prefixes"])):
            for kind in ("T", "t", "W", "w", "U"):
                with self.assertRaises(runner.base.RunError):
                    runner.validate_static_symbols(good + f"{name} {kind} 300 10\n", policy)

    def test_static_headers_reject_loader_and_missing_evidence(self):
        good = "Program Headers:\n LOAD 0x0000 0x0000 R E\n"
        runner.validate_static_headers(good)
        for value in ("", "There are no program headers.\n", good + " INTERP 0x0\n",
                      good + " 0x0000000000000001 (NEEDED) Shared library: [libc.so.6]\n"):
            with self.assertRaises(runner.base.RunError):
                runner.validate_static_headers(value)

    def test_exact_output(self):
        runner.validate_output(runner.PASS)
        for value in ("", runner.PASS[:-1], runner.PASS * 2, runner.PASS.replace("complete", "partial")):
            with self.assertRaises(runner.base.RunError):
                runner.validate_output(value)

    def test_selected_telemetry_fails_closed(self):
        with tempfile.TemporaryDirectory() as tmp:
            instance = runner.OwnerRunner(argparse.Namespace(rocm_path=pathlib.Path("/opt/rocm")), pathlib.Path(tmp))
            good = {"Unique ID": runner.base.UNIQUE_ID, "PCI Bus": "0000:26:00.0", "GPU use (%)": "0", "GPU Memory Allocated (VRAM%)": "0"}
            def observe(card):
                with mock.patch.object(instance, "run", return_value=json.dumps({"card0": {"busy": True}, "card1": card})):
                    instance.telemetry("test")
            observe(good)
            for key, value in (("Unique ID", "0x123"), ("PCI Bus", "0000:05:00.0"), ("GPU use (%)", "6"), ("GPU Memory Allocated (VRAM%)", "1")):
                with self.assertRaises(runner.base.RunError):
                    observe(dict(good, **{key: value}))

    def test_constructor_failure_cleans_only_owned_stage(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            sentinel = root / "unrelated"
            sentinel.touch()
            args = argparse.Namespace(staging_parent=root, output_dir=root)
            with mock.patch.object(runner.base, "parse_args", return_value=args), mock.patch.object(runner, "OwnerRunner", side_effect=RuntimeError("constructor failed")):
                self.assertEqual(runner.main(), 2)
            self.assertEqual(list(root.iterdir()), [sentinel])

    def test_cleanup_failure_withdraws_published_evidence(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            args = argparse.Namespace(staging_parent=root, output_dir=root)
            class Fake(runner.OwnerRunner):
                def snapshot(self): pass
                def build(self): pass
                def qualify_and_measure(self): pass
                def publish(self):
                    dest = root / "r61-owner-fake"
                    dest.mkdir()
                    return dest
            original = runner.base.cleanup_tree
            failed = False
            def cleanup(path):
                nonlocal failed
                if path.name.startswith("fe2o3-r61-owner.") and not failed:
                    failed = True
                    raise RuntimeError("cleanup failed")
                original(path)
            with mock.patch.object(runner.base, "parse_args", return_value=args), mock.patch.object(runner, "OwnerRunner", Fake), mock.patch.object(runner.base, "cleanup_tree", side_effect=cleanup):
                self.assertEqual(runner.main(), 2)
            self.assertFalse(list(root.glob("r61-owner-*")))
            self.assertFalse(list(root.glob("fe2o3-r61-owner.*")))
            rejected = list(root.glob("r61-rejected-*"))
            self.assertEqual(len(rejected), 1)
            self.assertTrue((rejected[0] / "commands.json").is_file())
            self.assertTrue((rejected[0] / "rejection.json").is_file())

    def test_corrupt_publication_never_enters_final_namespace(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            stage = root / "stage"
            output = root / "output"
            stage.mkdir()
            output.mkdir()
            instance = runner.OwnerRunner(argparse.Namespace(output_dir=output), stage)
            instance.commit = "1" * 40
            instance.snapshot_input_hashes = instance.tool_hashes = instance.binary_hashes = instance.topology = {}
            binary = stage / "binary"
            binary.touch()
            instance.binaries = {"kfd": binary}
            (instance.evidence / "source.tar").touch()
            original = runner.shutil.copytree
            def corrupt(source, destination, **kwargs):
                result = original(source, destination, **kwargs)
                (destination / "unexpected").touch()
                return result
            with mock.patch.object(runner.shutil, "copytree", side_effect=corrupt):
                with self.assertRaises(runner.base.RunError): instance.publish()
            self.assertEqual(list(output.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
