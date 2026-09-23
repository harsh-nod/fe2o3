"""Pure synthetic observed-query controls: no subprocess, socket, or compiler."""
import copy
import json
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import bridge_session
import fe2o3_debug_bridge
from bridge_observed_queries import ObservedQuerySession
from bridge_runtime_values import RESOURCE_REQUEST, RESOURCE_RESPONSE, RUNTIME_REQUEST, RUNTIME_RESPONSE
from debug_console_commands import CommandError, parse_command
from debug_console_protocol import LineFramer, ProtocolError, encode
from test_bridge_live_queries import view, selected, step_response, v1
from test_bridge_session import Pins, Process, command, connect_request

OWNER = {"backend_session": "9007199254740993", "capture_instance": "18446744073709551615"}
IDENTITY = {"allocation": "3", "storage_slot": "2", "generation": "2"}
INVOCATION = {"global": ["0", "0", "0"], "workgroup": ["0", "0", "0"], "local": [0, 0, 0],
              "workgroup_size": [4, 1, 1], "workgroup_count": ["1", "1", "1"], "launch_extent": ["4", "1", "1"]}
SITE = {"function_ordinal": "0", "block": 99, "operation": 3}


def runtime_reply(request, current, status="ok", memory_stop=False):
    reply = {"schema": RUNTIME_RESPONSE, "status": status, "request_id": request["request_id"],
             "session": copy.deepcopy(current)}
    if status == "error":
        reply["error"] = {"stage": "session", "code": "invalid_cursor",
                          "message": "synthetic refusal", "state_changed": False}
        return reply
    reply.update(binding={"owner": copy.deepcopy(OWNER), "cursor": copy.deepcopy(current["cursor"])},
                 completeness={"status": "complete"})
    if status == "unavailable":
        reply["reason"] = "no_selected_record"
        return reply
    reply.update(invocation=copy.deepcopy(INVOCATION),
        origin={"availability": "available", "identity": {"activation": "1", "attempt": "1", "site": copy.deepcopy(SITE)}},
        frames={"availability": "captured", "frames": [{"legacy_depth": 0, "function_ordinal": "0", "block": 99,
            "next_operation": 4, "activation": "1", "operation": {"state": "active_operation",
                "attempt": "1", "site": copy.deepcopy(SITE)}, "parent": {"parent": "root"}}]},
        allocation_watermark={"availability": "available", "through_sequence": "4"},
        origin_coverage={"coverage": "complete"}, frame_coverage={"coverage": "complete"},
        lifecycle_coverage={"coverage": "complete"})
    if memory_stop:
        reply["frames"] = {"availability": "unavailable", "reason": "not_checkpoint"}
    return reply


def desc(identity=None, private=True):
    result = {"identity": copy.deepcopy(IDENTITY if identity is None else identity),
        "address_space": "private" if private else "global", "access": "read_write",
        "alignment": 4, "byte_len": "16",
        "owning_scope": {"scope": "invocation", "invocation": copy.deepcopy(INVOCATION)} if private else {"scope": "dispatch"}}
    if private:
        result["creation_site"] = copy.deepcopy(SITE)
    return result


def inventory_row():
    return {"descriptor": desc(), "snapshot_bytes_available": True, "initialization_available": True}


def lifecycle_rows():
    first = desc({"allocation": "1", "storage_slot": "1", "generation": "1"}, False)
    private = desc({"allocation": "2", "storage_slot": "2", "generation": "1"})
    return [
        {"sequence": "1", "descriptor": first, "kind": {"transition": "preexisting"}},
        {"sequence": "2", "descriptor": private, "kind": {"transition": "create"}},
        {"sequence": "3", "descriptor": copy.deepcopy(private), "kind": {"transition": "release"}},
        {"sequence": "4", "descriptor": desc(), "kind": {"transition": "create", "previous_allocation": "2"}},
    ]


def resource_reply(request, current):
    operation = request["operation"]
    reply = {"schema": RESOURCE_RESPONSE, "status": "ok", "request_id": request["request_id"],
        "operation": operation, "session": copy.deepcopy(current), "binding": copy.deepcopy(request["expected_binding"]),
        "through_sequence": "4", "completeness": {"status": "complete"}}
    if operation == "read_allocation_memory":
        count = int(request["range"]["byte_len"])
        bits = bytearray((count+7)//8)
        for index in range(count):
            bits[index//8] |= 1 << (index % 8)
        reply["result"] = {"result": "allocation_memory", "memory": {
            "allocation": copy.deepcopy(request["allocation"]), "range": copy.deepcopy(request["range"]),
            "address_space": "private", "bytes": "0x"+"a5"*count, "initialized": "0x"+bits.hex()}}
        return reply
    if operation == "query_allocations":
        tag, field, rows, total, scanned = "allocations", "allocations", [inventory_row()], 1, 1
    elif operation == "query_allocation_lifecycle":
        tag, field, rows, total, scanned = "allocation_lifecycle", "transitions", lifecycle_rows(), 4, 4
    else:
        tag, field, total, scanned = "memory_accesses", "accesses", current["cursor"]["event_sequence"], current["cursor"]["event_sequence"]
        rows = [{"occurrence": {"record_ordinal": 1, "event_sequence": 2,
            "scope": selected(current)["scope"], "site": selected(current)["site"]["kir"],
            "schedule": {"identity": "workgroup_major_local_zyx_cooperative_v1", "decision_ordinal": 0}},
            "invocation": copy.deepcopy(INVOCATION), "allocation": copy.deepcopy(request["allocation"]),
            "range": {"byte_offset": "0", "byte_len": "4"}, "address_space": "private",
            "access": "write_committed", "origin": {"availability": "available",
                "identity": {"activation": "1", "attempt": "1", "site": copy.deepcopy(SITE)}}}]
    reply["page"] = {"source_count": str(total), "source_start": "0", "scanned": scanned}
    reply["result"] = {"result": tag, field: rows}
    return reply


def ready(events=3):
    session = ObservedQuerySession()
    request = session.begin({"operation": "discover_capabilities"})
    session.accept(v1(request, view(), {"result": "capabilities", "capabilities": []}))
    for _ in range(events):
        request = session.begin(session.parse_command("step 1"))
        session.accept(step_response(request, session.view))
    if events == 0:
        state(session)
    return session


def state(session):
    request = session.begin(session.parse_command("state"))
    session.accept(v1(request, session.view, {"result": "state",
        "snapshot": {"status": "unavailable", "reason": "not_captured"}}))


def discover(session, mutate=None):
    request = session.begin(session.parse_command("runtime"))
    reply = runtime_reply(request, session.view)
    if mutate:
        mutate(reply)
    session.accept(reply)
    return request, reply


def storage(session):
    request = session.begin(session.parse_command("storage"))
    reply = resource_reply(request, session.view)
    session.accept(reply)
    return request, reply


class ObservedAdapterTests(unittest.TestCase):
    def test_closed_commands_require_state_runtime_and_inventory(self):
        fresh = ObservedQuerySession()
        for text in ("runtime", "storage", "lifecycle", "storageaccess 3 2 2", "storagememory 3 2 2 0 4"):
            with self.assertRaises(CommandError):
                fresh.parse_command(text)
        session = ready()
        for text in ("storage", "lifecycle", "storageaccess 3 2 2", "storagememory 3 2 2 0 4"):
            with self.assertRaises(CommandError):
                session.parse_command(text)
        discover(session)
        with self.assertRaises(CommandError):
            session.parse_command("storageaccess 3 2 2")
        storage(session)
        before = session.sent
        for text in (" runtime", "runtime\n", "runtime {}", "runtime 1", "storage next",
                     "storageaccess 03 2 2", "storageaccess 0 2 2", "storageaccess 3 2 0",
                     "storageaccess 3 2 1", "storageaccess 3 2 2 token", "storagememory 3 2 2 0 0",
                     "storagememory 3 2 2 0 4097", "storagememory 3 2 2 15 2",
                     "storagememory 3 2 2 18446744073709551615 1", "storageaccess 3 2 18446744073709551616"):
            with self.subTest(text=text), self.assertRaises(CommandError):
                session.parse_command(text)
            self.assertEqual(session.sent, before)

    def test_original_commands_and_schemas_keep_shared_ledger(self):
        session = ready()
        first = session.sent
        for text in ("state", "stack", "step 1", "reverse 1", "continue 2",
                     "break add 0 2 3 after", "watch add 1 0 0 4 write"):
            self.assertEqual(session.parse_command(text), parse_command(text))
        request, response = discover(session)
        self.assertNotIn("expected_owner", request)
        self.assertEqual(request["request_id"], first+1)
        self.assertEqual(request["expected_cursor"], session.view["cursor"])
        ar, reply = storage(session)
        self.assertEqual(ar["request_id"], first+2)
        self.assertEqual(ar["page"], {"max_items": 16, "max_scanned": 64})
        self.assertEqual(ar["expected_binding"], response["binding"])
        self.assertEqual(reply["schema"], RESOURCE_RESPONSE)
        again = session.begin(session.parse_command("runtime"))
        self.assertEqual(again["expected_owner"], OWNER)
        session.accept(runtime_reply(again, session.view))
        with self.assertRaises(CommandError):
            session.parse_command("storageaccess 3 2 2")

    def test_initial_capabilities_are_not_current_record_selection(self):
        session = ObservedQuerySession()
        request = session.begin({"operation": "discover_capabilities"})
        session.accept(v1(request, view(), {"result": "capabilities", "capabilities": []}))
        with self.assertRaises(CommandError):
            session.parse_command("runtime")
        state(session)
        request = session.begin(session.parse_command("runtime"))
        session.accept(runtime_reply(request, session.view, "unavailable"))
        self.assertEqual(session._observation_owner, OWNER)
        with self.assertRaises(CommandError):
            session.parse_command("storage")
        state(session)
        self.assertEqual(session.parse_command("runtime")["expected_owner"], OWNER)

    def test_owner_discovery_cannot_repeat_without_owner_after_refusal(self):
        session = ready()
        request = session.begin(session.parse_command("runtime"))
        session.accept(runtime_reply(request, session.view, "error"))
        state(session)
        with self.assertRaises(CommandError):
            session.parse_command("runtime")
        self.assertIsNone(session._runtime_record)

    def test_caller_headers_bindings_ranges_and_page_tokens_never_forward(self):
        for text, mutate in (
            ("runtime", lambda body: body.update(expected_owner=copy.deepcopy(OWNER))),
            ("runtime", lambda body: body.update(request_id=77)),
            ("storage", lambda body: body["expected_binding"]["owner"].update(capture_instance="7")),
            ("storage", lambda body: body["page"].update(token="runtime.forged")),
            ("storage", lambda body: body["page"].update(max_items=17)),
            ("lifecycle", lambda body: body.update(address_space="private")),
            ("storagememory 3 2 2 0 4", lambda body: body["range"].update(byte_len="4097")),
        ):
            session = ready()
            if text != "runtime":
                discover(session); storage(session)
            body = session.parse_command(text)
            mutate(body)
            before = session.sent
            with self.subTest(text=text), self.assertRaises(ProtocolError):
                session.begin(body)
            self.assertEqual(session.sent, before)
            self.assertIsNone(session.pending)

    def test_runtime_failure_table_closes_owner_and_all_derived_state(self):
        changes = [
            lambda reply: reply.update(operation="inspect_current_record"),
            lambda reply: reply.update(request_id=True),
            lambda reply: reply["session"].update(hardware_observed=True),
            lambda reply: reply["binding"]["cursor"].update(event_sequence=99),
            lambda reply: reply["binding"]["owner"].update(backend_session=1),
            lambda reply: reply["binding"]["owner"].update(backend_session="01"),
            lambda reply: reply["binding"]["owner"].update(capture_instance="0"),
            lambda reply: reply["invocation"]["global"].__setitem__(0, "1"),
            lambda reply: reply["invocation"]["workgroup_size"].__setitem__(0, True),
            lambda reply: reply["origin"]["identity"].update(activation="0"),
            lambda reply: reply["origin"]["identity"].update(attempt="2"),
            lambda reply: reply["frames"]["frames"][0].update(legacy_depth=1),
            lambda reply: reply["frames"]["frames"][0].update(next_operation=None),
            lambda reply: reply["frames"]["frames"][0].update(parent={"parent": "caller", "activation": "1",
                "attempt": "1", "call_site": copy.deepcopy(SITE)}),
            lambda reply: reply.update(frame_coverage={"coverage": "disabled"}),
            lambda reply: reply.update(origin_coverage={"coverage": "prefix_truncated",
                "retained_records": "2", "reason": "row_limit"}),
            lambda reply: reply.update(lifecycle_coverage={"coverage": "invalid_join"}),
            lambda reply: reply.update(completeness={"status": "truncated", "reason": "event_limit", "emitted_events": 2}),
            lambda reply: reply["allocation_watermark"].update(physical_address="0x1234"),
        ]
        for change in changes:
            session = ready()
            with self.subTest(change=change), self.assertRaises(ProtocolError):
                discover(session, change)
            self.assertFalse(session._observation_connection_open)
            self.assertIsNone(session._observation_owner)
            self.assertIsNone(session.selected_runtime)

    def test_exact_suspended_parent_roster_and_new_work_cutoff(self):
        session = ready()
        def nested(reply):
            current = reply["frames"]["frames"][0]
            current["activation"] = "2"
            current["parent"] = {"parent": "caller", "activation": "1", "attempt": "1", "call_site": copy.deepcopy(SITE)}
            caller = copy.deepcopy(current)
            caller.update(legacy_depth=0, activation="1", parent={"parent": "root"},
                          operation={"state": "suspended", "attempt": "1", "site": copy.deepcopy(SITE)})
            current["legacy_depth"] = 1
            reply["frames"]["frames"].insert(0, caller)
            reply["origin"]["identity"]["activation"] = "2"
            reply["frame_coverage"] = {"coverage": "prefix_truncated", "retained_records": "3",
                                       "reason": "validation_work_limit"}
        discover(session, nested)
        self.assertEqual(len(session.selected_runtime["frames"]["frames"]), 2)
        retained = session.selected_runtime
        retained["frames"]["frames"][0]["activation"] = "99"
        self.assertEqual(session.selected_runtime["frames"]["frames"][0]["activation"], "1")

    def test_memory_watch_stop_queries_actual_origin_without_prior_frames(self):
        session = ready()
        discover(session); storage(session)
        request = session.begin(session.parse_command("continue 3"))
        current = view(session.view["revision"]+1, session.view["cursor"]["event_sequence"]+1)
        session.accept(v1(request, current, {"result": "control", "events_advanced": 1,
            "snapshot": {"status": "unavailable", "reason": "not_captured"}}))
        self.assertIsNone(session._anchor)
        request = session.begin(session.parse_command("runtime"))
        self.assertEqual(request["expected_owner"], OWNER)
        session.accept(runtime_reply(request, session.view, memory_stop=True))
        self.assertEqual(session.selected_runtime["frames"], {"availability": "unavailable", "reason": "not_checkpoint"})
        self.assertEqual(session._storage_inventory, {})

    def test_control_filter_and_close_fences(self):
        for text in ("step 1", "reverse 1", "continue 1", "break add 0 2 3 after", "watch add 1 0 0 4 write"):
            session = ready()
            discover(session); storage(session)
            session.begin(session.parse_command(text))
            self.assertIsNone(session.selected_runtime)
            self.assertEqual(session._storage_inventory, {})
            self.assertEqual(session._observation_owner, OWNER)
        session = ready()
        discover(session); storage(session)
        session.close_observation_connection()
        self.assertIsNone(session._observation_owner)
        with self.assertRaises(CommandError):
            session.parse_command("runtime")

    def test_storage_lifecycle_access_and_memory_original_payloads(self):
        session = ready()
        discover(session); storage(session)
        for text in ("lifecycle", "storageaccess 3 2 2", "storagememory 3 2 2 0 3"):
            request = session.begin(session.parse_command(text))
            reply = resource_reply(request, session.view)
            self.assertIs(session.accept(reply), reply)
        self.assertEqual(request["allocation"], IDENTITY)
        self.assertEqual(request["range"], {"byte_offset": "0", "byte_len": "3"})
        self.assertEqual(reply["result"]["memory"]["initialized"], "0x07")

    def test_resource_failure_table_drops_state_and_owner(self):
        changes = [
            ("storage", lambda reply: reply["binding"]["owner"].update(capture_instance="1")),
            ("storage", lambda reply: reply.update(through_sequence="5")),
            ("storage", lambda reply: reply["page"].update(source_start="1")),
            ("storage", lambda reply: reply["page"].update(next_token="spare")),
            ("storage", lambda reply: reply["result"]["allocations"].append(inventory_row())),
            ("storage", lambda reply: reply["result"]["allocations"][0]["descriptor"].update(alignment=3)),
            ("storage", lambda reply: reply["result"]["allocations"][0].update(initialization_available=False)),
            ("lifecycle", lambda reply: reply["result"]["transitions"][3]["kind"].update(previous_allocation="1")),
            ("lifecycle", lambda reply: reply["result"]["transitions"][3]["descriptor"]["identity"].update(generation="3")),
            ("lifecycle", lambda reply: reply["result"]["transitions"][2].update(sequence="4")),
            ("lifecycle", lambda reply: reply["result"]["transitions"][2]["descriptor"].update(byte_len="8")),
            ("storageaccess 3 2 2", lambda reply: reply["result"]["accesses"][0]["allocation"].update(generation="1")),
            ("storageaccess 3 2 2", lambda reply: reply["result"]["accesses"][0]["range"].update(byte_offset="16")),
            ("storageaccess 3 2 2", lambda reply: reply["result"]["accesses"][0]["invocation"]["global"].__setitem__(0, "2")),
            ("storageaccess 3 2 2", lambda reply: reply["result"]["accesses"][0]["occurrence"].update(event_sequence=3)),
            ("storagememory 3 2 2 0 3", lambda reply: reply["result"]["memory"].update(initialized="0xff")),
            ("storagememory 3 2 2 0 3", lambda reply: reply["result"]["memory"].update(bytes="0xA5a5a5")),
            ("storagememory 3 2 2 0 3", lambda reply: reply["result"]["memory"]["range"].update(byte_len="2")),
        ]
        for text, change in changes:
            session = ready()
            discover(session); storage(session)
            request = session.begin(session.parse_command(text))
            reply = resource_reply(request, session.view)
            change(reply)
            with self.subTest(text=text, change=change), self.assertRaises(ProtocolError):
                session.accept(reply)
            self.assertIsNone(session._observation_owner)
            self.assertEqual(session._storage_inventory, {})

    def test_valid_first_page_token_is_observation_only_and_no_automatic_request(self):
        session = ready()
        discover(session)
        request = session.begin(session.parse_command("storage"))
        reply = resource_reply(request, session.view)
        reply["page"].update(source_count="17", next_token="runtime.1.1")
        before = session.sent
        session.accept(reply)
        self.assertEqual(session.sent, before)
        with self.assertRaises(CommandError):
            session.parse_command("storage runtime.1.1")
        self.assertNotIn("token", session.parse_command("storage")["page"])

    def test_shared_command_limit_and_one_pending_rule(self):
        session = ready()
        request = session.begin(session.parse_command("runtime"))
        with self.assertRaises(CommandError):
            session.parse_command("runtime")
        session.accept(runtime_reply(request, session.view))
        session.sent = 255
        with self.assertRaises(ProtocolError):
            session.begin(session.parse_command("storage"))

    def test_fragmented_wire_preserves_large_decimal_strings(self):
        session = ready()
        request = session.begin(session.parse_command("runtime"))
        reply = runtime_reply(request, session.view)
        framer, decoded = LineFramer(), []
        wire = encode(reply)+b"\n"
        for index in range(0, len(wire), 7):
            framer.feed(wire[index:index+7], decoded.append)
        session.accept(decoded[0])
        self.assertEqual(session._observation_owner, OWNER)
        self.assertEqual(session.sent, request["request_id"])

    def test_terminal_unavailable_allows_cursor_beyond_truncated_prefix_but_ok_does_not(self):
        session = ready()
        request = session.begin(session.parse_command("runtime"))
        reply = runtime_reply(request, session.view, "unavailable")
        reply["completeness"] = {"status": "truncated", "reason": "event_limit", "emitted_events": 2}
        session.accept(reply)
        self.assertEqual(session._observation_owner, OWNER)
        self.assertIsNone(session.selected_runtime)
        state(session)
        request = session.begin(session.parse_command("runtime"))
        reply = runtime_reply(request, session.view)
        reply["completeness"] = {"status": "truncated", "reason": "event_limit", "emitted_events": 2}
        with self.assertRaises(ProtocolError):
            session.accept(reply)

    def test_resource_terminal_unavailable_validates_shape_without_minting_rows(self):
        from bridge_storage_values import validate_resource
        session = ready()
        discover(session); storage(session)
        request = session.begin(session.parse_command("storage"))
        completeness = {"status": "truncated", "reason": "event_limit", "emitted_events": 2}
        # A standalone typed refusal can carry a terminal cursor. The bridge
        # still cannot create resource authority from a RuntimeUnavailable reply.
        runtime = session.selected_runtime
        runtime["completeness"] = copy.deepcopy(completeness)
        reply = {"schema": RESOURCE_RESPONSE, "status": "unavailable",
            "request_id": request["request_id"], "operation": request["operation"],
            "session": copy.deepcopy(session.view), "binding": copy.deepcopy(request["expected_binding"]),
            "completeness": completeness, "reason": "no_selected_record"}
        self.assertIsNone(validate_resource(reply, request, session.view, runtime, {}))


class ObservedProcess(Process):
    def exchange(self, request, deadline, peer):
        if request["schema"] not in (RUNTIME_REQUEST, RESOURCE_REQUEST):
            return super().exchange(request, deadline, peer)
        self.requests.append(copy.deepcopy(request))
        reply = runtime_reply(request, self.current) if request["schema"] == RUNTIME_REQUEST else resource_reply(request, self.current)
        if self.change:
            self.change(reply)
        return reply


class ObservedBridgeControls(unittest.TestCase):
    def fixture(self):
        process = ObservedProcess()
        owner = bridge_session.BridgeSession(("/trusted/debug", "sim", "--runtime-observations", "v1"),
            Pins(), lambda: process, lambda: 100.0, runtime_observations="v1")
        owner.handle_request(connect_request())
        owner.handle_request(command(owner, "step 1"))
        return owner, process

    def test_owner_flag_only_extends_closed_launch_and_default_unchanged(self):
        for profile, extra in ((None, []), ("v1", ["--runtime-observations", "v1"])):
            original = ["/trusted/debug", "sim", "--protocol", "jsonl"]
            with patch.object(fe2o3_debug_bridge, "launch_arguments", return_value=original.copy()):
                result = fe2o3_debug_bridge.observed_launch_arguments(SimpleNamespace(runtime_observations=profile))
                self.assertEqual(result, original+extra)
        with patch.object(fe2o3_debug_bridge, "launch_arguments", return_value=[]), self.assertRaises(ValueError):
            fe2o3_debug_bridge.observed_launch_arguments(SimpleNamespace(runtime_observations="other"))

    def test_default_bridge_refuses_new_commands_before_dispatch(self):
        process = Process()
        owner = bridge_session.BridgeSession(("/trusted/debug", "sim"), Pins(), lambda: process, lambda: 100.0)
        owner.handle_request(connect_request())
        before = len(process.requests)
        with self.assertRaises(bridge_session.BridgeError) as caught:
            owner.handle_request(command(owner, "runtime"))
        self.assertEqual(caught.exception.code, "command_refused")
        self.assertEqual(len(process.requests), before)

    def test_same_envelope_mixed_schema_shared_sequence_and_backend_close(self):
        owner, process = self.fixture()
        for text in ("runtime", "storage", "lifecycle", "storagememory 3 2 2 0 4"):
            reply = owner.handle_request(command(owner, text))
            self.assertEqual(reply["schema"], bridge_session.RESPONSE_SCHEMA)
            inner = json.loads(reply["response_json"])
            self.assertEqual(inner["schema"], RUNTIME_RESPONSE if text == "runtime" else RESOURCE_RESPONSE)
            self.assertEqual(int(reply["sequence"])+1, inner["request_id"])
        self.assertEqual(process.started, ["/trusted/debug", "sim", "--runtime-observations", "v1"])
        self.assertTrue(owner.close())
        self.assertIsNone(owner.protocol._observation_owner)

    def test_invalid_runtime_reply_reaps_owned_process_no_rebinding(self):
        owner, process = self.fixture()
        process.change = lambda reply: reply["binding"]["cursor"].update(event_sequence=99)
        with self.assertRaises(bridge_session.BridgeError) as caught:
            owner.handle_request(command(owner, "runtime"))
        self.assertEqual(caught.exception.code, "backend_failed")
        self.assertEqual(caught.exception.outcome, "unknown")
        self.assertTrue(owner.closed)
        self.assertGreater(process.closes, 0)
        self.assertIsNone(owner.protocol._observation_owner)


if __name__ == "__main__":
    unittest.main()
