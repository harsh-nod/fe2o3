#!/usr/bin/env python3
"""Extraction/runner controls only: no Rust compiler, native owner, or GPU."""
import importlib.util
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

sys.dont_write_bytecode = True  # Dynamic import must not write into the checkout.
SCRIPT = Path(__file__).resolve().parents[1] / "one_stop_completion_cpu.py"
SPEC = importlib.util.spec_from_file_location("completion_cpu", SCRIPT)
cpu = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(cpu)


class ExtractionControls(unittest.TestCase):
    def test_current_checkout_extracts_all_required_fragments(self):
        source, _ = cpu.snapshot()
        out = cpu.extract(source)
        self.assertEqual(set(out), {"completion.rs", "completion_poll.rs", "deadline.rs", "observe-expression.rs", "constants.rs", "public-tests.rs"})
        self.assertIn(out["completion.rs"].rstrip(), source["resources"])
        self.assertIn(out["completion_poll.rs"].rstrip(), source["resources"])
        self.assertIn(out["observe-expression.rs"].rstrip(), source["native"])
        for name in cpu.PUBLIC_TESTS:
            self.assertIn(cpu.function(source["public_tests"], name, test=True), out["public-tests.rs"])

    def test_string_braces_do_not_truncate_function(self):
        source = 'pub(super) fn f() { let s = "} {"; if true { foo(); } }\nfn g() {}'
        self.assertEqual(cpu.function(source, "f"), source.split('\n')[0])

    def test_nested_comments_do_not_truncate(self):
        source = 'fn f() { /* } /* { */ } */ done(); }'
        self.assertEqual(cpu.function(source, "f"), source)

    def test_raw_string_braces_and_quotes_are_masked(self):
        source = 'fn f() { let s = r##"} \\" {"##; done(); }'
        self.assertEqual(cpu.function(source, "f"), source)

    def test_escaped_quotes_and_char_literals(self):
        source = 'fn f() { let x = "\\\"}"; let c = \'}\'; }'
        self.assertEqual(cpu.function(source, "f"), source)

    def test_lifetime_is_not_a_char_literal(self):
        source = "fn f<'a>(x: &'a str) { let _ = x; }"
        # The closed extractor deliberately refuses unsupported generic signatures.
        with self.assertRaisesRegex(ValueError, "missing or duplicated"):
            cpu.function(source, "f")
        self.assertIn("&'a str", cpu.rust_mask(source))

    def test_function_missing_fails(self):
        with self.assertRaisesRegex(ValueError, "missing or duplicated"):
            cpu.function("fn other() {}", "f")

    def test_function_duplicated_fails(self):
        with self.assertRaisesRegex(ValueError, "duplicated"):
            cpu.function("fn f() {}\nfn f() {}", "f")

    def test_function_name_in_comment_is_not_definition(self):
        self.assertEqual(cpu.function("// fn f() {}\nfn f() { ok(); }", "f"), "fn f() { ok(); }")

    def test_unclosed_comment_fails(self):
        with self.assertRaisesRegex(ValueError, "unclosed"):
            cpu.function("fn f() { /* comment", "f")

    def test_unclosed_body_fails(self):
        with self.assertRaisesRegex(ValueError, "unclosed"):
            cpu.function("fn f() {", "f")

    def test_arm_missing_fails(self):
        with self.assertRaisesRegex(ValueError, "missing"):
            cpu.arm("S::Other => {}", "ObserveCompletion")

    def test_arm_duplicated_fails(self):
        with self.assertRaisesRegex(ValueError, "duplicated"):
            cpu.arm("S::ObserveCompletion => {}\nS::ObserveCompletion => {}", "ObserveCompletion")

    def test_constant_duplicate_fails(self):
        with self.assertRaisesRegex(ValueError, "duplicated"):
            cpu.constant("pub const X: u32 = 1;\npub const X: u32 = 1;", "X")

    def test_public_test_requires_real_test_attribute(self):
        with self.assertRaisesRegex(ValueError, "missing"):
            cpu.function("fn example() {}", "example", test=True)

    def test_all_eleven_named_tests_required(self):
        listing = '\n'.join(name + ': test' for name in sorted(cpu.EXPECTED_TESTS))
        cpu.verify_test_list(listing)
        with self.assertRaisesRegex(ValueError, "eleven"):
            cpu.verify_test_list(listing.split('\n', 1)[1])

    def test_duplicate_or_renamed_test_cannot_replace_missing_one(self):
        names = sorted(cpu.EXPECTED_TESTS)
        names[0] = names[1]
        with self.assertRaisesRegex(ValueError, "eleven"):
            cpu.verify_test_list('\n'.join(n + ': test' for n in names))

    def test_channel_is_repository_pinned_not_default(self):
        self.assertEqual(cpu.channel_from('[toolchain]\nchannel = "nightly-2026-04-03"\n'), 'nightly-2026-04-03')
        with self.assertRaisesRegex(ValueError, "pinned"):
            cpu.channel_from('channel = "nightly"\n')

    def test_strict_destroy_queue_is_required(self):
        source, _ = cpu.snapshot()
        destroyed = cpu.arm(source['native'], 'DestroyQueue')
        self.assertIn('resources::completion(', destroyed)
        source['native'] = source['native'].replace(destroyed, destroyed.replace('resources::completion(', 'resources::completion_poll('))
        with self.assertRaisesRegex(ValueError, "strict retirement"):
            cpu.extract(source)

    def test_original_deadline_is_not_reset_or_replaced(self):
        source, _ = cpu.snapshot()
        self.assertIn('Duration::from_secs(60)', source['owner'])
        source['owner'] = source['owner'].replace('Duration::from_secs(60)', 'Duration::from_secs(61)')
        with self.assertRaisesRegex(ValueError, "60-second"):
            cpu.extract(source)

    def test_child_success_retains_bounded_both_channels(self):
        with tempfile.TemporaryDirectory(prefix="completion-control-") as name:
            output = Path(name)
            result = cpu.run_child([sys.executable, "-c", "import sys; print('cpu only'); print('note', file=sys.stderr)"], "child", output, {"PATH": "/usr/bin:/bin"}, 5)
            self.assertEqual(result, {"stdout": "cpu only\n", "stderr": "note\n"})
            self.assertEqual((output / "child.stdout").read_text(), "cpu only\n")

    def test_child_timeout_is_terminal_and_retains_logs(self):
        with tempfile.TemporaryDirectory(prefix="completion-control-") as name:
            output = Path(name)
            with self.assertRaisesRegex(ValueError, "deadline"):
                cpu.run_child([sys.executable, "-c", "import time; time.sleep(5)"], "timeout", output, {"PATH": "/usr/bin:/bin"}, 0.05)
            self.assertTrue((output / "timeout.stdout").is_file())
            self.assertTrue((output / "timeout.stderr").is_file())

    def test_child_output_overflow_is_terminal(self):
        with tempfile.TemporaryDirectory(prefix="completion-control-") as name:
            output = Path(name)
            with self.assertRaisesRegex(ValueError, "output cap"):
                cpu.run_child([sys.executable, "-c", "import sys; sys.stdout.buffer.write(b'x' * (1024 * 1024 + 1))"], "overflow", output, {"PATH": "/usr/bin:/bin"}, 5)
            self.assertLessEqual((output / "overflow.stdout").stat().st_size, cpu.PIPE_CAP)

    def setup_failure(self, register):
        created = []
        original = subprocess.Popen
        def spawn(*args, **kwargs):
            child = original(*args, **kwargs)
            created.append(child)
            return child
        with tempfile.TemporaryDirectory(prefix="completion-control-") as name:
            output = Path(name)
            if register:
                selector = cpu.selectors.DefaultSelector()
                setup = mock.patch.object(selector, "register", side_effect=OSError("injected register failure"))
                factory = mock.patch.object(cpu.selectors, "DefaultSelector", return_value=selector)
            else:
                setup = mock.patch.object(cpu.selectors, "DefaultSelector", side_effect=OSError("injected selector failure"))
                factory = mock.patch.object(cpu.subprocess, "Popen", side_effect=spawn)
            with setup, factory:
                if register:
                    with mock.patch.object(cpu.subprocess, "Popen", side_effect=spawn):
                        with self.assertRaisesRegex(OSError, "injected register"):
                            cpu.run_child([sys.executable, "-c", "import time; time.sleep(1)"], "setup", output, {"PATH": "/usr/bin:/bin"}, 2)
                else:
                    with self.assertRaisesRegex(OSError, "injected selector"):
                        cpu.run_child([sys.executable, "-c", "import time; time.sleep(1)"], "setup", output, {"PATH": "/usr/bin:/bin"}, 2)
            self.assertEqual(len(created), 1)
            self.assertIsNotNone(created[0].returncode)
            self.assertTrue((output / "setup.stdout").is_file())
            self.assertTrue((output / "setup.stderr").is_file())
            self.assertTrue(json.loads((output / "setup.cleanup.json").read_text())["direct_child_reaped"])

    def test_selector_construction_failure_cleans_spawned_group(self):
        self.setup_failure(register=False)

    def test_selector_registration_failure_cleans_spawned_group(self):
        self.setup_failure(register=True)

    def signal_cancellation(self, signum):
        previous = {s: signal.getsignal(s) for s in (signal.SIGTERM, signal.SIGINT)}
        with tempfile.TemporaryDirectory(prefix="completion-control-") as name:
            output = Path(name)
            # IntEnum stringification is symbolic on Python 3.10.
            numeric_signal = str(int(signum))
            self.assertTrue(numeric_signal.isdecimal())
            script = "import os,time; os.kill(os.getppid(), " + numeric_signal + "); time.sleep(1)"
            with self.assertRaisesRegex(RuntimeError, "interrupted by signal " + numeric_signal):
                cpu.run_child([sys.executable, "-c", script], "signal", output, {"PATH": "/usr/bin:/bin"}, 2)
            record = json.loads((output / "signal.cleanup.json").read_text())
            self.assertEqual(record["cancelled_signal"], signum)
            self.assertTrue(record["direct_child_reaped"])
            self.assertEqual(record["errors"], [])
        self.assertEqual({s: signal.getsignal(s) for s in previous}, previous)

    def test_sigterm_to_parent_enters_cleanup_and_restores_handlers(self):
        self.signal_cancellation(signal.SIGTERM)

    def test_sigint_to_parent_enters_cleanup_and_restores_handlers(self):
        self.signal_cancellation(signal.SIGINT)

    def test_exited_direct_child_with_held_stdout_still_hits_deadline(self):
        with tempfile.TemporaryDirectory(prefix="completion-control-") as name:
            output = Path(name)
            # Grandchild stays in the selected group and holds both output
            # pipes after its direct parent exits. Its own sleep is bounded.
            script = "import subprocess,sys; subprocess.Popen([sys.executable,'-c','import time; time.sleep(1)']); print('held', flush=True)"
            with self.assertRaisesRegex(ValueError, "deadline"):
                cpu.run_child([sys.executable, "-c", script], "held", output, {"PATH": "/usr/bin:/bin"}, 0.15)
            self.assertIn("held", (output / "held.stdout").read_text())
            self.assertTrue(json.loads((output / "held.cleanup.json").read_text())["direct_child_reaped"])

    def test_wait_failure_preserves_original_error_and_attempts_all_logs(self):
        created, real_waits = [], []
        original = subprocess.Popen
        def spawn(*args, **kwargs):
            child = original(*args, **kwargs)
            created.append(child)
            real_waits.append(child.wait)
            child.wait = mock.Mock(side_effect=subprocess.TimeoutExpired("injected wait", 5))
            return child
        with tempfile.TemporaryDirectory(prefix="completion-control-") as name:
            output = Path(name)
            try:
                with mock.patch.object(cpu.subprocess, "Popen", side_effect=spawn):
                    with self.assertRaisesRegex(ValueError, "deadline"):
                        cpu.run_child([sys.executable, "-c", "import time; time.sleep(1)"], "wait", output, {"PATH": "/usr/bin:/bin"}, 0.05)
            finally:
                # Failure injection bypassed only the wrapper wait; reap the
                # already-killed test child with its real method on all paths.
                for wait in real_waits:
                    wait(timeout=2)
            self.assertTrue((output / "wait.stdout").is_file())
            self.assertTrue((output / "wait.stderr").is_file())
            record = json.loads((output / "wait.cleanup.json").read_text())
            self.assertFalse(record["direct_child_reaped"])
            self.assertIn({"operation": "reap-direct-child", "error_type": "TimeoutExpired"}, record["errors"])

    def test_signal_during_cleanup_cannot_return_success(self):
        original = subprocess.Popen
        previous = signal.getsignal(signal.SIGTERM)
        def spawn(*args, **kwargs):
            child = original(*args, **kwargs)
            real_wait = child.wait
            def wait(*args, **kwargs):
                os.kill(os.getpid(), signal.SIGTERM)
                return real_wait(*args, **kwargs)
            child.wait = wait
            return child
        with tempfile.TemporaryDirectory(prefix="completion-control-") as name:
            output = Path(name)
            with mock.patch.object(cpu.subprocess, "Popen", side_effect=spawn):
                with self.assertRaisesRegex(RuntimeError, "interrupted by signal"):
                    cpu.run_child([sys.executable, "-c", "print('done')"], "late-signal", output, {"PATH": "/usr/bin:/bin"}, 2)
            self.assertTrue(json.loads((output / "late-signal.cleanup.json").read_text())["direct_child_reaped"])
        self.assertEqual(signal.getsignal(signal.SIGTERM), previous)


    def test_compile_command_bounds_lld_workers_and_rejects_other_hosts(self):
        compiler, output = Path("/compiler/bin/rustc"), Path("/scratch")
        host = "host: x86_64-unknown-linux-gnu\n"
        self.assertEqual(cpu.compile_command(compiler, output, "rustc test\n" + host),
                         [str(compiler), "--edition=2024", "--test", "-C", "debuginfo=0",
                          "-C", "codegen-units=1", "-C", "link-arg=-Wl,--threads=1",
                          "/scratch/cpu.rs", "-o", "/scratch/tests"])
        for version in ("", "host: aarch64-unknown-linux-gnu\n", host + host):
            with self.assertRaisesRegex(ValueError, "one x86_64-unknown-linux-gnu host"):
                cpu.compile_command(compiler, output, version)


if __name__ == "__main__":
    unittest.main()
