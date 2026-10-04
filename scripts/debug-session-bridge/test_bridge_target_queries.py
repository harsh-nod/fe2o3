"""Synthetic target adapter controls only; no subprocess, socket or GPU."""
import copy
import unittest
from types import SimpleNamespace
from unittest.mock import Mock, patch

import bridge_session
from bridge_target_values import TARGET_REQUEST, TARGET_RESPONSE, TARGET_BYTES, validate_target
from debug_console_commands import CommandError
from debug_console_protocol import ProtocolError, encode
from test_bridge_observed_queries import ready, discover, storage
from test_bridge_live_queries import step_response


def reply(request, view, target="gfx942:xnack-"):
    return {"schema": TARGET_RESPONSE, "status": "ok", "operation": "inspect_declared_target",
        "request_id": request["request_id"], "session": copy.deepcopy(view),
        "binding": copy.deepcopy(request["expected_binding"]), "logical_wave_width": 32,
        "target": {"availability": "declared", "target": target, "provenance": "verified_simulation_bundle",
            "envelope_version": 5, "envelope_identity": "a"*64, "subject_identity": "b"*64,
            "admitted_module": {"wire_version": 10, "sha256": "c"*64, "canonical_bytes": "245"}}}


class TargetAdapterTests(unittest.TestCase):
    def test_one_explicit_command_preserves_runtime_inventory_and_view(self):
        for gpu in ("gfx942:xnack-", "gfx950:xnack-"):
            session = ready()
            with self.assertRaises(CommandError):
                session.parse_command("target")
            discover(session); storage(session)
            view, inventory, runtime = copy.deepcopy(session.view), copy.deepcopy(session._storage_inventory), session.selected_runtime
            count = session.sent
            request = session.begin(session.parse_command("target"))
            self.assertEqual(request["schema"], TARGET_REQUEST)
            self.assertEqual(request["expected_binding"], runtime["binding"])
            session.accept(reply(request, session.view, gpu))
            self.assertEqual(session.sent, count + 1)
            self.assertEqual(session.view, view)
            self.assertEqual(session._storage_inventory, inventory)
            self.assertEqual(session.selected_runtime, runtime)

    def test_no_override_extra_args_or_forged_headers(self):
        session = ready(); discover(session)
        for text in ("target gfx942", "target ", " target", "target 1", "target\n", "target {}"):
            with self.subTest(text=text), self.assertRaises(CommandError):
                session.parse_command(text)
        for mutate in (
            lambda body: body.update(target="gfx950:xnack-"),
            lambda body: body.update(request_id=4),
            lambda body: body["expected_binding"]["owner"].update(capture_instance="7"),
            lambda body: body["expected_binding"]["cursor"].update(event_sequence=99),
        ):
            body = session.parse_command("target"); mutate(body)
            with self.assertRaises(ProtocolError):
                session.begin(body)

    def test_raw_target_is_unavailable_and_controls_clear_current_binding(self):
        session = ready()
        discover(session)
        request = session.begin(session.parse_command("target"))
        response = reply(request, session.view)
        response["target"] = {"availability": "unavailable", "reason": "raw_input_has_no_declared_gpu_target"}
        session.accept(response)
        request = session.begin(session.parse_command("step 1"))
        session.accept(step_response(request, session.view))
        with self.assertRaises(CommandError):
            session.parse_command("target")

    def test_wrong_target_owner_cursor_truth_envelope_and_unknown_fields_close(self):
        mutations = [
            lambda value: value["binding"]["owner"].update(backend_session="7"),
            lambda value: value["binding"]["cursor"].update(event_sequence=99),
            lambda value: value["session"].update(hardware_observed=True),
            lambda value: value.update(logical_wave_width=True),
            lambda value: value["target"].update(target="gfx942"),
            lambda value: value["target"].update(provenance="hardware_observed"),
            lambda value: value["target"].update(envelope_version=6),
            lambda value: value["target"].update(envelope_identity="0"*64),
            lambda value: value["target"]["admitted_module"].update(canonical_bytes="0"),
            lambda value: value["target"].update(extra="unknown"),
        ]
        for mutate in mutations:
            session = ready(); discover(session); storage(session)
            request = session.begin(session.parse_command("target"))
            response = reply(request, session.view); mutate(response)
            with self.assertRaises(ProtocolError):
                session.accept(response)
            self.assertIsNone(session._observation_owner)
            self.assertIsNone(session.selected_runtime)
            self.assertEqual(session._storage_inventory, {})

    def test_target_reply_cap_and_error_no_change_contract(self):
        session = ready(); discover(session)
        request = session.begin(session.parse_command("target"))
        response = reply(request, session.view)
        self.assertLessEqual(len(encode(response)) + 1, TARGET_BYTES)
        response["target"]["extra"] = "x" * TARGET_BYTES
        with self.assertRaises(ProtocolError):
            validate_target(response, request, session.view)
        error = {"schema": TARGET_RESPONSE, "status": "error", "operation": "inspect_declared_target",
            "request_id": request["request_id"], "session": copy.deepcopy(session.view),
            "error": {"stage": "session", "code": "invalid_cursor", "message": "synthetic refusal", "state_changed": False}}
        session.accept(error)
        self.assertIsNone(session.selected_runtime)
        error["error"]["state_changed"] = True
        with self.assertRaises(ProtocolError):
            validate_target(error, request, session.view)

    def test_target_stdout_cap_precedes_line_framing(self):
        process = bridge_session.CPUProcess()
        process.child = SimpleNamespace(poll=lambda: None, stdout=object(), stderr=object(), stdin=object())
        selector = Mock()
        selector.select.side_effect = [[], [(SimpleNamespace(data="request", fd=1), 0)],
            [(SimpleNamespace(data="stdout", fd=2), 0)], [(SimpleNamespace(data="stdout", fd=2), 0)]]
        with patch.object(bridge_session.selectors, "DefaultSelector", return_value=selector), \
                patch.object(bridge_session.os, "write", side_effect=lambda _fd, data: len(data)), \
                patch.object(bridge_session.os, "read", side_effect=[b" " * TARGET_BYTES, b"\n"]), \
                patch.object(bridge_session.time, "monotonic", return_value=0):
            with self.assertRaisesRegex(ProtocolError, "target response byte cap"):
                process.exchange({"schema": TARGET_REQUEST}, 1)
        self.assertEqual(len(process.framer.partial), TARGET_BYTES)
        selector.close.assert_called_once()
