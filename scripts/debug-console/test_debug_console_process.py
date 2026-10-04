"""Pure bounded I/O-state controls. All OS writes/signals/waits are mocked."""
import signal
import subprocess
import unittest
from unittest.mock import Mock, patch

from debug_console_process import (ConsoleInput, Output, OUTPUT_QUEUE, OUTPUT_TOTAL,
                                   check_deadlines, cleanup)
from debug_console_protocol import ProtocolError
from debug_console_process import run_console

class DescriptorAliasTests(unittest.TestCase):
    def check_restored(self, failure, expected_status):
        # Different descriptor numbers share one open-file description, as on
        # a terminal. All state changes are mocked; no child or PTY is opened.
        modes = {0: True, 111: True, 112: True, 113: True}
        aliases = {101: 0, 102: 0, 2: 0, 111: 111, 112: 112, 113: 113}
        events = []
        def get_mode(fd):
            events.append(("get", fd))
            return modes[aliases[fd]]
        def set_mode(fd, blocking):
            events.append(("set", fd))
            modes[aliases[fd]] = blocking
        child = Mock(pid=42)
        child.poll.return_value = None
        child.wait.return_value = 0
        for name, fd in (("stdin", 111), ("stdout", 112), ("stderr", 113)):
            getattr(child, name).fileno.return_value = fd
            getattr(child, name).closed = False
        selector = Mock()
        selector.select.side_effect = failure
        with (patch("debug_console_process.selectors.DefaultSelector", return_value=selector),
              patch("debug_console_process.subprocess.Popen", return_value=child),
              patch("debug_console_process.os.get_blocking", side_effect=get_mode),
              patch("debug_console_process.os.set_blocking", side_effect=set_mode),
              patch("debug_console_process.os.killpg") as kill,
              patch("debug_console_process.os.write", return_value=1)):
            self.assertEqual(run_console(["synthetic-debugger"], 101, 102), expected_status)
        self.assertEqual(events[:2], [("get", 101), ("get", 102)])
        self.assertTrue(modes[0], "caller terminal and aliased stderr must stay blocking")
        selector.close.assert_called_once_with()
        kill.assert_called_once_with(42, signal.SIGTERM)
        child.wait.assert_called_once_with(timeout=1)
        for pipe in (child.stdin, child.stdout, child.stderr):
            pipe.close.assert_called_once_with()

    def test_error_restores_aliased_console_descriptors(self):
        self.check_restored(OSError("synthetic selector failure"), 1)

    def test_interrupt_restores_aliased_console_descriptors(self):
        self.check_restored(KeyboardInterrupt(), 130)

class InputTests(unittest.TestCase):
    def test_split_crlf_and_eof(self):
        stream = ConsoleInput()
        stream.feed(b"sta")
        stream.feed(b"te\r\nquit\n")
        self.assertEqual(list(stream.lines), ["state", "quit"])
        stream.eof()
        self.assertTrue(stream.ended)

    def test_partial_utf8_and_invalid_utf8(self):
        stream = ConsoleInput()
        stream.feed(b"\xc3")
        stream.feed(b"\xa9\n")
        self.assertEqual(list(stream.lines), ["\u00e9"])
        with self.assertRaises(UnicodeError):
            ConsoleInput().feed(b"\xff\n")

    def test_queue_line_total_commands_and_chunk_caps(self):
        with self.assertRaises(ProtocolError):
            ConsoleInput().feed(b"\n" * 9)
        with self.assertRaises(ProtocolError):
            ConsoleInput().feed(b"x" * 1025 + b"\n")
        with self.assertRaises(ProtocolError):
            ConsoleInput().feed(b"x" * 4097)
        for field, value in (("total", 65536), ("commands", 256)):
            stream = ConsoleInput()
            setattr(stream, field, value)
            with self.assertRaises(ProtocolError):
                stream.feed(b"\n")
        stream = ConsoleInput()
        stream.feed(b"state")
        with self.assertRaises(ProtocolError):
            stream.eof()

class OutputTests(unittest.TestCase):
    def test_partial_write_and_stalled_write(self):
        output = Output()
        with patch("debug_console_process.time.monotonic", return_value=10):
            output.append(b"abc")
        self.assertEqual(output.since, 10)
        with patch("debug_console_process.os.write", return_value=1):
            output.write(99)
        self.assertEqual(output.pending, b"bc")
        with patch("debug_console_process.os.write", side_effect=BlockingIOError):
            output.write(99)
        self.assertEqual(output.pending, b"bc")
        with patch("debug_console_process.os.write", return_value=2):
            output.write(99)
        self.assertEqual(output.pending, b"")
        self.assertIsNone(output.since)

    def test_queue_cumulative_and_closed_write(self):
        output = Output()
        with self.assertRaises(ProtocolError):
            output.append(b"x" * (OUTPUT_QUEUE + 1))
        self.assertEqual(output.total, 0)
        output.total = OUTPUT_TOTAL
        with self.assertRaises(ProtocolError):
            output.append(b"x")
        output = Output()
        output.append(b"x")
        with patch("debug_console_process.os.write", return_value=0), self.assertRaises(ProtocolError):
            output.write(99)

    def test_request_session_and_both_drain_deadlines(self):
        check_deadlines(1, 0, 0, 0, None)
        for args in ((900, 0, None, None, None), (30, 0, 0, None, None),
                     (5, 0, None, 0, None), (2, 0, None, None, 0)):
            with self.subTest(args=args), self.assertRaises(ProtocolError):
                check_deadlines(*args)

class CleanupTests(unittest.TestCase):
    def child(self):
        child = Mock()
        child.pid = 42
        child.poll.return_value = None
        return child

    def test_term_then_kill_bounded_wait(self):
        child = self.child()
        child.wait.side_effect = [subprocess.TimeoutExpired("synthetic", 1), 0]
        with patch("debug_console_process.os.killpg") as kill:
            self.assertTrue(cleanup(child))
        self.assertEqual(kill.call_args_list[0].args, (42, signal.SIGTERM))
        self.assertEqual(kill.call_args_list[1].args, (42, signal.SIGKILL))
        self.assertEqual(child.wait.call_count, 2)
        self.assertEqual(child.wait.call_args.kwargs, {"timeout": 1})

    def test_cannot_confirm_reap_is_not_success(self):
        child = self.child()
        child.wait.side_effect = subprocess.TimeoutExpired("synthetic", 1)
        with patch("debug_console_process.os.killpg"):
            self.assertFalse(cleanup(child))

    def test_already_reaped_leader_is_not_signalled(self):
        child = self.child()
        child.poll.return_value = 0
        with patch("debug_console_process.os.killpg") as kill:
            self.assertTrue(cleanup(child))
            kill.assert_not_called()

    def test_signal_failure_is_not_reap_success(self):
        child = self.child()
        with patch("debug_console_process.os.killpg", side_effect=PermissionError):
            self.assertFalse(cleanup(child))

if __name__ == "__main__":
    unittest.main()
