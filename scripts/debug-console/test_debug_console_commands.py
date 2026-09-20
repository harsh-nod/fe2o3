"""Synthetic parser/transport controls; no compiler/debugger subprocesses."""
import argparse
import unittest
from unittest.mock import patch

from debug_console_commands import CommandError, parse_command
from debug_console_protocol import (LINE_BYTES, LineFramer, ProtocolError,
                                    decode_line, encode)
from fe2o3_debug_console import launch_arguments

class CommandTests(unittest.TestCase):
    def test_exact_state_stack_and_quit(self):
        self.assertEqual(parse_command("state"), {"operation": "get_state"})
        self.assertEqual(parse_command("stack"), {"operation": "inspect_stack",
            "scope": {"level": "dispatch"}, "page": {"limit": 16}})
        self.assertEqual(parse_command("quit"), {"operation": "terminate"})

    def test_explicit_site_and_phase(self):
        site = {"function_ordinal": 2, "block_ordinal": 17,
                "point": {"kind": "operation", "operation_ordinal": 8}}
        self.assertEqual(parse_command("source 2 17 8"),
                         {"operation": "resolve_source", "site": site})
        self.assertEqual(parse_command("break add 2 17 8 after"),
                         {"operation": "set_breakpoints", "breakpoints": [
                             {"enabled": True, "kind": {"kind": "site", "site": site,
                                                       "phase": "after_operation"}}]})

    def test_explicit_watch_generation_and_full_u64(self):
        request = parse_command("watch add 18446744073709551615 9 0 4 atomic")
        self.assertEqual(request, {"operation": "set_watchpoints", "watchpoints": [{
            "enabled": True, "allocation": {"ordinal": (1 << 64) - 1, "generation": 9},
            "byte_offset": 0, "byte_len": 4, "access": "atomic", "timing": "after_commit"}]})
        memory = parse_command("memory 2 0 18446744073709551614 1")
        self.assertEqual(memory["byte_offset"], (1 << 64) - 2)

    def test_exact_lists_removals_and_control(self):
        for name, plural in (("break", "breakpoints"), ("watch", "watchpoints")):
            self.assertEqual(parse_command(name + " list"),
                             {"operation": "list_" + plural, "page": {"limit": 16}})
            request = parse_command(name + " remove 3")
            self.assertEqual(request["operation"], "remove_" + plural)
            self.assertEqual(request[("breakpoint" if name == "break" else "watchpoint") + "_ids"], [3])
        self.assertEqual(parse_command("step"), {"operation": "step", "direction": "forward",
                         "granularity": "operation", "count": 1})
        self.assertEqual(parse_command("reverse 64")["direction"], "reverse")
        self.assertEqual(parse_command("continue 65536"), {"operation": "continue", "max_events": 65536})

    def test_unavailable_commands_and_bounds_refused(self):
        invalid = ["step 0", "step 65", "reverse 2 3", "continue 65537", "step over",
                   "source", "source 0 0 -1", "break add 0 0 0 source", "break remove 0",
                   "watch add 1 0 0 0", "watch add 1 0 0 4097", "watch add 1 0 0 1 bad",
                   "memory 0 0 0 1", "memory 1 0 18446744073709551615 1",
                   "memory 1 18446744073709551616 0 1", "source 00 0 0", "stack 1",
                   "state\t", "state\x1b", "x" * 1025, "eval x", "hardware", "out"]
        for text in invalid:
            with self.subTest(text=text[:40]), self.assertRaises(CommandError):
                parse_command(text)

    def test_help_and_blank_are_local(self):
        self.assertEqual(parse_command("help"), "help")
        self.assertIsNone(parse_command("  "))

    def test_fixed_cpu_launch_only(self):
        args = argparse.Namespace(binary="/bin/debug", input="/input/bundle", request="/input/request",
                                  kind="bundle-v6", wave_width=64)
        with patch("fe2o3_debug_console.regular_path", side_effect=lambda value, *a, **kw: value):
            self.assertEqual(launch_arguments(args), ["/bin/debug", "sim", "--bundle-v6",
                "/input/bundle", "--request", "/input/request", "--wave-width", "64",
                "--protocol", "jsonl"])
            args.kind = "hardware"
            with self.assertRaises(ValueError):
                launch_arguments(args)

class FramingTests(unittest.TestCase):
    def test_lossless_u64_and_owned_partial(self):
        framer, received = LineFramer(), []
        framer.feed(b'{"number":1844674407370955', received.append)
        self.assertEqual(received, [])
        framer.feed(b'1615}\n{"ok":true}\n', received.append)
        self.assertEqual(received, [{"number": (1 << 64) - 1}, {"ok": True}])
        framer.eof()

    def test_malformed_json_refused(self):
        for raw in [b'{"x":1,"x":2}', b'{"x":1.0}', b'{"x":NaN}', b'{"x":null}',
                    b'{"x":-1}', b'{"x":18446744073709551616}', b'{"x":"\xff"}',
                    b'{"x":1}\r', b'{"x":1}\n', b""]:
            with self.subTest(raw=raw), self.assertRaises(ProtocolError):
                decode_line(raw)

    def test_line_chunk_total_and_eof_caps(self):
        with self.assertRaises(ProtocolError):
            LineFramer().feed(b"x" * 4097, lambda _: None)
        framer = LineFramer()
        framer.partial = bytearray(b"x" * LINE_BYTES)
        with self.assertRaises(ProtocolError):
            framer.feed(b"x", lambda _: None)
        framer = LineFramer()
        framer.total = 8 * 1024 * 1024
        with self.assertRaises(ProtocolError):
            framer.feed(b"x", lambda _: None)
        for raw in [b"\n", b"{}\r\n"]:
            with self.assertRaises(ProtocolError):
                LineFramer().feed(raw, lambda _: None)
        framer = LineFramer()
        framer.feed(b"{}", lambda _: None)
        with self.assertRaises(ProtocolError):
            framer.eof()

    def test_graph_depth_and_collection_caps(self):
        with self.assertRaises(ProtocolError):
            decode_line(b"[" * 34 + b"0" + b"]" * 34)
        with self.assertRaises(ProtocolError):
            decode_line(encode([0] * 4097))

if __name__ == "__main__":
    unittest.main()
