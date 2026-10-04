"""Pure V20 bridge controls. Retained CLI rows are not a live bridge qualification."""
import copy
import json
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import bridge_physical_v20 as profile
from bridge_physical_v20_protocol import PhysicalQuerySessionV20
from bridge_physical_v20_values import SCHEMA, query_identity
from bridge_session import BridgeError, BridgeSession, REQUEST_SCHEMA, RESPONSE_SCHEMA
from debug_console_commands import CommandError
from debug_console_protocol import ProtocolError, exact
import fe2o3_debug_bridge
from fe2o3_debug_console import KINDS
from test_bridge_session import CID, HOST, ORIGIN, TOKEN, Pins

FIXTURES = Path(__file__).parent / "fixtures" / "physical-v20"


def retained(name):
    return json.loads((FIXTURES / (name + ".json")).read_text(encoding="ascii"))


def rebase(name, request, previous=None, sequence=None, completed=False):
    """Synthetic transport correlation around exact retained inner field fixtures."""
    value = retained(name)
    value["request_id"] = request["request_id"]
    value["operation"] = request["operation"]
    view = value["session"]
    if previous is not None:
        control = request["operation"] in ("step", "seek")
        revision = previous["revision"] + int(control)
        view = copy.deepcopy(previous)
        view["revision"] = revision
        view["cursor"]["state_revision"] = revision
        if sequence is not None:
            view["cursor"]["event_sequence"] = sequence
        view["state"] = "created" if view["cursor"]["event_sequence"] == 0 else "stopped"
        value["session"] = view
    result = value["result"]
    if result["result"] == "control":
        old = previous["cursor"]["event_sequence"]
        new = view["cursor"]["event_sequence"]
        total = new - 1 if completed else 8192
        result["events_advanced"] = abs(min(new, total) - min(old, total))
    def bind(row):
        if type(row) is dict:
            if set(row) == {"cursor", "scope", "site"}:
                row["cursor"] = copy.deepcopy(view["cursor"])
            for item in row.values():
                bind(item)
        elif type(row) is list:
            for item in row:
                bind(item)
    bind(result)
    return value


def discovered():
    owner = PhysicalQuerySessionV20()
    request = owner.begin({"operation": "discover_capabilities"})
    owner.accept(rebase("capabilities", request))
    return owner


def selected():
    owner = discovered()
    request = owner.begin(owner.parse_command("seek 2181"))
    owner.accept(rebase("checkpoint", request, owner.view, 2181))
    return owner


def control(owner, text, sequence, completed=False):
    request = owner.begin(owner.parse_command(text))
    response = rebase("completed" if completed else "checkpoint", request, owner.view, sequence, completed)
    if not completed:
        response["result"]["stop"] = {"reason": "entry" if sequence == 0 else "step",
                                      "outcome": "active", "exact": True}
        response["result"]["snapshot"] = {"status": "unavailable", "reason": "not_captured"}
    # Once an end is known, the completed sentinel itself does not count as an event.
    total = owner.end - 1 if owner.end is not None else sequence - 1 if completed else 8192
    response["result"]["events_advanced"] = abs(min(sequence, total) -
                                               min(owner.view["cursor"]["event_sequence"], total))
    owner.accept(response)


class PhysicalProtocolControls(unittest.TestCase):
    def test_retained_capabilities_and_checkpoint_exact_u64_mask(self):
        owner = selected()
        self.assertEqual(owner.selected["scope"]["active_mask"], (1 << 64) - 1)
        self.assertEqual(owner.selected["scope"]["lane"], 0)
        self.assertEqual(owner.selected["cursor"], owner.view["cursor"])
        self.assertEqual(owner.view["revision"], 1)

    def test_explicit_event_commands_and_closed_roster(self):
        owner = selected()
        self.assertEqual(owner.parse_command("step 64")["granularity"], "event")
        self.assertEqual(owner.parse_command("reverse 1")["direction"], "reverse")
        for text in ("step", "step 65", "step 0", "seek 8194", "step 01", "state\nstep",
                     "state ", "values 65", "values 0", "values next", "memory 0 257",
                     "memory 18446744073709551615 1", "source 0 0 0", "stack", "target",
                     "runtime", "allocations", "continue 1", "registers", "quit", "{}"):
            with self.subTest(text=text), self.assertRaises(CommandError):
                owner.parse_command(text)

    def test_end_sentinel_reverse_and_forward_have_zero_event_count(self):
        owner = selected()
        control(owner, "seek 2753", 2753, True)
        self.assertEqual(owner.end, 2753)
        control(owner, "reverse 1", 2752)
        control(owner, "step 1", 2753, True)
        control(owner, "step 64", 2753, True)
        control(owner, "seek 0", 0)
        self.assertEqual(owner.view["state"], "created")

    def test_wrong_events_advanced_or_unsolicited_end_rejected(self):
        for change in ("count", "end", "revision"):
            owner = selected()
            request = owner.begin(owner.parse_command("step 1"))
            reply = rebase("checkpoint", request, owner.view, 2182)
            if change == "count":
                reply["result"]["events_advanced"] = 0
            elif change == "end":
                reply["result"]["stop"] = {"reason": "completed", "outcome": "completed", "exact": True}
            else:
                reply["session"]["revision"] += 1
            with self.subTest(change=change), self.assertRaises(ProtocolError):
                owner.accept(reply)
            self.assertIsNone(owner.selected)

    def test_actual_symbolic_rows_never_become_numeric_addresses(self):
        owner = selected()
        request = owner.begin(owner.parse_command("values 64"))
        response = rebase("values", request, owner.view)
        owner.accept(response)
        rows = response["result"]["values"]
        self.assertTrue(any(v["availability"] == {"status": "unavailable", "reason": "not_represented"}
                            for v in rows))
        self.assertTrue(any(v["availability"].get("value_type", {}).get("kind") == "pointer" for v in rows))

    def test_symbolic_unknown_fields_wrong_pointer_and_duplicate_ssa_refuse(self):
        for change in ("symbolic", "pointer", "duplicate", "mask", "source"):
            owner = selected()
            request = owner.begin(owner.parse_command("values 64"))
            response = rebase("values", request, owner.view)
            rows = response["result"]["values"]
            if change == "symbolic":
                next(r for r in rows if r["availability"]["status"] == "unavailable")["availability"]["bits"] = "0x00"
            elif change == "pointer":
                rows[0]["availability"]["value"]["allocation"]["ordinal"] = 2
            elif change == "duplicate":
                rows[1]["path"] = copy.deepcopy(rows[0]["path"])
            elif change == "mask":
                response["result"]["snapshot"]["scope"]["active_mask"] = 1
            else:
                response["result"]["snapshot"]["site"]["source"] = {"status": "captured"}
            with self.subTest(change=change), self.assertRaises(ProtocolError):
                owner.accept(response)
            self.assertIsNone(owner.next_page)

    def page(self, owner):
        request = owner.begin(owner.parse_command("values 1"))
        reply = rebase("values", request, owner.view)
        reply["result"]["values"] = reply["result"]["values"][:1]
        reply["result"]["next_cursor"] = {"query_identity": query_identity(owner.view), "position": 1}
        owner.accept(reply)
        return copy.deepcopy(owner.next_page)

    def test_page_token_only_from_current_response_and_revision(self):
        owner = selected()
        token = self.page(owner)
        body = owner.parse_command("values next")
        self.assertEqual(body["page"]["cursor"], token)
        self.assertEqual(body["scope"], {"level": "lane", "workgroup": [0, 0, 0], "wave": 0, "lane": 0})
        control(owner, "seek 2181", 2181)
        with self.assertRaises(CommandError):
            owner.parse_command("values next")
        with self.assertRaises(ProtocolError):
            owner.begin(body)

    def test_wrong_query_digest_or_nonprogressing_next_page_refuse(self):
        for digest, position in (("b" * 64, 1), (None, 0)):
            owner = selected()
            request = owner.begin(owner.parse_command("values 1"))
            reply = rebase("values", request, owner.view)
            reply["result"]["values"] = reply["result"]["values"][:1]
            reply["result"]["next_cursor"] = {"query_identity": digest or query_identity(owner.view),
                                             "position": position}
            with self.assertRaises(ProtocolError):
                owner.accept(reply)

    def test_actual_memory_preserves_initialization_and_output_identity(self):
        owner = selected()
        request = owner.begin(owner.parse_command("memory 0 4"))
        reply = rebase("memory", request, owner.view)
        owner.accept(reply)
        self.assertEqual(reply["result"]["memory"]["availability"]["initialized"], "0x0f")
        self.assertEqual(request["allocation"], {"ordinal": 1, "generation": 0})

    def test_memory_wrong_identity_padding_bytes_or_anchor_refuse(self):
        for mutation in ("allocation", "generation", "padding", "length", "anchor"):
            owner = selected()
            request = owner.begin(owner.parse_command("memory 0 4"))
            reply = rebase("memory", request, owner.view)
            memory = reply["result"]["memory"]
            if mutation == "allocation":
                memory["allocation"]["ordinal"] = 2
            elif mutation == "generation":
                memory["allocation"]["generation"] = 1
            elif mutation == "padding":
                memory["availability"]["initialized"] = "0xff"
            elif mutation == "length":
                memory["returned_bytes"] = 3
            else:
                reply["result"]["snapshot"]["cursor"]["state_revision"] += 1
            with self.subTest(mutation=mutation), self.assertRaises(ProtocolError):
                owner.accept(reply)

    def test_transactional_unavailable_clears_queries_without_revision_change(self):
        owner = selected()
        self.page(owner)
        previous = copy.deepcopy(owner.view)
        request = owner.begin(owner.parse_command("memory 999999 4"))
        owner.accept({"schema": "fe2o3-debug-response-v1", "request_id": request["request_id"],
                      "operation": "read_memory", "status": "unavailable", "session": previous,
                      "unavailable": {"capability": "allocation_relative_memory",
                          "reason": "outside_capture_scope", "state_changed": False,
                          "detail": "diagnostic physical-entry V20 exposes bounded CPU observations only"}})
        self.assertTrue(exact(owner.view, previous))
        self.assertIsNone(owner.selected)
        self.assertIsNone(owner.next_page)

    def test_foreign_profile_refusal_and_caps_unknown_mutations_refuse(self):
        owner = selected()
        request = owner.begin(owner.parse_command("seek 8193"))
        reply = {"schema": "fe2o3-debug-response-v1", "request_id": request["request_id"],
                 "operation": "seek", "status": "error", "session": copy.deepcopy(owner.view),
                 "error": {"stage": "session", "code": "invalid_cursor",
                           "message": "kir_v21_debug_cursor_out_of_range", "state_changed": False}}
        with self.assertRaises(ProtocolError):
            owner.accept(reply)
        for kind in ("unknown", "available"):
            fresh = PhysicalQuerySessionV20()
            request = fresh.begin({"operation": "discover_capabilities"})
            reply = rebase("capabilities", request)
            reply["result"]["capabilities"][0][kind] = True
            with self.assertRaises(ProtocolError):
                fresh.accept(reply)


class Child:
    def __init__(self):
        self.requests = []
        self.closed = 0
        self.started = None

    def start(self, argv):
        self.started = argv

    def exchange(self, request, _deadline, _peer):
        self.requests.append(copy.deepcopy(request))
        return rebase("capabilities", request)

    def close(self):
        self.closed += 1
        return True


class PhysicalBridgeControls(unittest.TestCase):
    def test_local_profile_selects_new_envelope_old_clients_refuse(self):
        child = Child()
        owner = profile.PhysicalBridgeSessionV20(("/pinned/cli",), Pins(), lambda: child)
        request = {"schema": SCHEMA, "action": "connect", "connection_id": CID}
        reply = owner.handle_request(request)
        self.assertEqual(reply["schema"], SCHEMA)
        self.assertNotEqual(reply["schema"], RESPONSE_SCHEMA)
        with self.assertRaises(BridgeError):
            BridgeSession(("/pinned/cli",), Pins(), lambda: child).handle_request(request)
        with self.assertRaises(BridgeError):
            profile.PhysicalBridgeSessionV20(("/pinned/cli",), Pins(), lambda: child).handle_request(
                {"schema": REQUEST_SCHEMA, "action": "connect", "connection_id": CID})
        self.assertEqual(len(child.requests), 1)

    def test_physical_http_error_uses_local_outer_schema_even_before_dispatch(self):
        owner = profile.PhysicalBridgeSessionV20(("/pinned/cli",), Pins())
        with patch.object(fe2o3_debug_bridge, "read_request", side_effect=BridgeError("invalid_request")), \
                patch.object(fe2o3_debug_bridge, "write_response") as write:
            fe2o3_debug_bridge.serve_peer(object(), owner, Mock(value=TOKEN), HOST, ORIGIN)
        wire = write.call_args.args[1]
        self.assertIn(SCHEMA.encode(), wire)
        self.assertNotIn(RESPONSE_SCHEMA.encode(), wire)
        self.assertTrue(owner.closed)

    def test_fixed_launch_argv_and_no_legacy_kind_widening(self):
        args = SimpleNamespace(kind=profile.KIND, wave_width=64, runtime_observations=None,
                               binary="/fixed/cli", input="/fixed/module", request="/fixed/request")
        self.assertNotIn(profile.KIND, KINDS)
        with patch.object(profile, "regular_path", side_effect=lambda path, *_args, **_kwargs: path) as regular:
            self.assertEqual(profile.launch_arguments_v20(args), ["/fixed/cli", "sim", "--diagnostic-kir-v20",
                "/fixed/module", "--request", "/fixed/request", "--wave-width", "64", "--protocol", "jsonl"])
            self.assertEqual(regular.call_args_list[1].args[1], 128 * 1024)
            self.assertEqual(regular.call_args_list[2].args[1], 16 * 1024)
            for field, wrong in (("kind", "diagnostic-kir-v21"), ("wave_width", 32),
                                 ("runtime_observations", "v1")):
                changed = copy.copy(args)
                setattr(changed, field, wrong)
                with self.assertRaises(ValueError):
                    profile.launch_arguments_v20(changed)

    def test_same_shared_request_budget_and_no_runtime_profile(self):
        owner = selected()
        owner.sent = 255
        with self.assertRaises(ProtocolError):
            owner.begin(owner.parse_command("state"))
        with self.assertRaises(ValueError):
            profile.PhysicalBridgeSessionV20(("/fixed/cli",), Pins(), runtime_observations="v1")


if __name__ == "__main__":
    unittest.main()
