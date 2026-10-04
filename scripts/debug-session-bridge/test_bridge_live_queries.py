"""Pure synthetic adapter/bridge controls; no subprocess, listener or compiler.

Synthetic wire objects exercise client guards only, not producer qualification.
Run after staging the two modules and minimal bridge patch beside original files.
"""
import copy
import unittest

# This existing entry imports the unchanged repository-local V1 console owner.
import bridge_session
from bridge_live_queries import LiveQuerySession
from bridge_live_query_values import (RESOURCE_REQUEST, RESOURCE_RESPONSE, VARIABLE_REQUEST,
    VARIABLE_RESPONSE)
from debug_console_commands import CommandError, parse_command
from debug_console_protocol import LineFramer, ProtocolError, encode

CFG = "a"*64
SOURCE = {"status": "unavailable", "reason": "requires_authenticated_map"}
STOP = {"reason": "step", "outcome": "active", "exact": True}


def view(revision=0, event=0):
    return {"backend": "cpu_kir_simulator", "execution_kind": "cpu_kir_simulation",
            "state": "stopped", "revision": revision, "configuration_identity": CFG,
            "cursor": {"configuration_identity": CFG, "event_sequence": event, "state_revision": revision},
            "simulated": True, "hardware_observed": False, "performance_prediction": False}


def selected(session):
    return {"cursor": copy.deepcopy(session["cursor"]),
            "scope": {"level": "lane", "workgroup": [0, 0, 0], "wave": 0, "lane": 0,
                      "logical_workitem": [0, 0, 0], "active_mask": 15, "wave_width": 32,
                      "interpretation": "logical_visualization"},
            "site": {"kir": {"function_ordinal": 0, "block_ordinal": 2,
                            "point": {"kind": "operation", "operation_ordinal": 3}},
                     "source": copy.deepcopy(SOURCE)}}


def v1(request, current, result, status="ok"):
    return {"schema": "fe2o3-debug-response-v1", "status": status, "request_id": request["request_id"],
            "operation": request["operation"], "session": copy.deepcopy(current), "result": result}


def step_response(request, previous):
    current = view(previous["revision"]+1, previous["cursor"]["event_sequence"]+1)
    return v1(request, current, {"result": "control", "stop": copy.deepcopy(STOP), "events_advanced": 1,
        "snapshot": {"status": "captured", "snapshot": {"anchor": selected(current),
                     "stop": copy.deepcopy(STOP), "values": []}}})


def ready():
    session = LiveQuerySession()
    request = session.begin(parse_command("state") if session.view else {"operation": "discover_capabilities"})
    session.accept(v1(request, view(), {"result": "capabilities", "capabilities": []}))
    request = session.begin(session.parse_command("step 1"))
    session.accept(step_response(request, session.view))
    return session


def allocation_row(ordinal=1):
    return {"allocation": {"ordinal": ordinal, "generation": 0}, "address_space": "global",
            "access": "read_write", "alignment": 4, "capacity_bytes": "24",
            "snapshot_bytes_available": True, "initialization_available": True,
            "owning_scope": "not_represented", "lifetime": "not_represented", "physical_base": "not_represented"}


def resource(request, current, rows=None, status="ok"):
    response = {"schema": RESOURCE_RESPONSE, "status": status, "request_id": request["request_id"],
                "operation": request["operation"], "session": copy.deepcopy(current)}
    if status == "error":
        response["error"] = {"stage": "session", "code": "invalid_cursor", "message": "synthetic refusal",
                             "state_changed": False}
    elif status == "unavailable":
        response.update(reason="not_captured", required="1", completeness={"status": "complete"})
    else:
        allocations = request["operation"] == "query_allocations"
        rows = [allocation_row()] if rows is None and allocations else [] if rows is None else rows
        response.update(snapshot=copy.deepcopy(request["expected_snapshot"]),
            page={"source_count": len(rows), "scanned": len(rows), "completeness": {"status": "complete"}},
            result={"result": "allocations" if allocations else "memory_accesses",
                    "allocations" if allocations else "accesses": copy.deepcopy(rows)},
            physical_registers="not_represented")
    return response


def inventory(session, ordinal=1):
    request = session.begin(session.parse_command("allocations"))
    response = resource(request, session.view, [allocation_row(ordinal)])
    session.accept(response)
    return request, response


def frame_row():
    return {"frame": 1, "function_ordinal": 0, "block_ordinal": 2, "next_operation": 4,
            "values": {"status": "captured", "value_count": 0}}


def stack(session, mutate=None):
    request = session.begin(session.parse_command("stack"))
    result = {"result": "stack", "snapshot": copy.deepcopy(session._anchor), "frames": [frame_row()]}
    if mutate:
        mutate(result)
    response = v1(request, session.view, result)
    session.accept(response)
    return request, response


def source_value(index=0):
    return {"variable_identity": format(index+1, "064x"), "name": "a", "function_ordinal": 0,
            "scope_identity": "b"*64, "scope_depth": 0, "generation": 1,
            "availability": {"status": "value", "value": {"status": "captured",
                "value_type": {"kind": "integer", "signed": False, "bits": 32},
                "value": {"encoding": "bits", "bits": "0xfffffff0"}, "provenance": "simulated_observation"}}}


def variables(request, session, rows=None, status="ok"):
    response = {"schema": VARIABLE_RESPONSE, "status": status, "request_id": request["request_id"],
                "operation": request["operation"], "session": copy.deepcopy(session.view)}
    if status == "unavailable":
        response["reason"] = "source_map_v2_required"
    elif status == "error":
        response["error"] = {"stage": "session", "code": "invalid_cursor", "message": "synthetic refusal",
                             "state_changed": False}
    else:
        response.update(snapshot={**copy.deepcopy(session._anchor), "frame": 1, "occurrence": 1},
                        values=[source_value()] if rows is None else copy.deepcopy(rows))
    return response


class QueryAdapterTests(unittest.TestCase):
    def test_old_parser_unchanged_and_new_commands_are_closed(self):
        session = ready()
        for line in ("state", "stack", "step 1", "reverse 1", "continue 10", "memory 1 0 0 1",
                     "source 0 2 3", "break add 0 2 3 after", "watch add 1 0 0 4 write"):
            self.assertEqual(session.parse_command(line), parse_command(line))
        before = session.sent
        for line in ("allocations 1", " allocations", "allocations\n", "allocations\t",
                     "accesses 01 0", "accesses 0 0", "accesses 1 1", "accesses 18446744073709551616 0",
                     "variables 0", "variables 2", "variables 1 cursor", "variables  1",
                     "allocations {}", "accesses 1 0 token", "variables 1\n"):
            with self.subTest(line=line), self.assertRaises((CommandError, ProtocolError)):
                session.parse_command(line)
            self.assertEqual(session.sent, before)

    def test_queries_need_real_selected_step_stack_and_inventory_before_dispatch(self):
        session = LiveQuerySession()
        for line in ("allocations", "accesses 1 0", "variables 1"):
            with self.assertRaises(CommandError):
                session.parse_command(line)
        self.assertEqual((session.sent, session.pending), (0, None))
        session = ready()
        for line in ("accesses 1 0", "variables 1"):
            with self.assertRaises(CommandError):
                session.parse_command(line)
        self.assertEqual(session.sent, 2)

    def test_exact_generated_shapes_shared_ids_and_original_response_schemas(self):
        session = ready()
        self.assertEqual(session.selected_control["request"]["request_id"], 2)
        ar, av = inventory(session)
        self.assertEqual(ar, {"schema": RESOURCE_REQUEST, "request_id": 3, "expected_revision": 1,
            "operation": "query_allocations", "expected_snapshot": selected(view(1, 1)),
            "address_space": "global", "page": {"max_items": 16, "max_scanned": 64}})
        sr, _ = stack(session)
        self.assertEqual(sr["request_id"], 4)
        request = session.begin(session.parse_command("variables 1"))
        self.assertEqual(request, {"schema": VARIABLE_REQUEST, "request_id": 5, "expected_revision": 1,
            "operation": "inspect_source_variables", "scope": {"level": "dispatch"}, "frame": 1,
            "selector": {"selector": "all"}, "page": {"limit": 16}})
        reply = variables(request, session)
        self.assertIs(session.accept(reply), reply)
        request = session.begin(session.parse_command("accesses 1 0"))
        self.assertEqual(request["request_id"], 6)
        self.assertEqual(request["filter"], {"scope": {"level": "dispatch"},
            "allocation": {"ordinal": 1, "generation": 0}, "address_space": "global"})
        session.accept(resource(request, session.view))
        self.assertEqual(session.sent, 6)
        self.assertEqual(session.view, view(1, 1))
        self.assertEqual(av["schema"], RESOURCE_RESPONSE)
        self.assertEqual(reply["schema"], VARIABLE_RESPONSE)

    def test_retained_step_pair_and_anchor_are_independent_owned_copies(self):
        session = ready()
        retained = session.selected_control
        retained["request"]["count"] = 64
        retained["response"]["result"]["snapshot"]["snapshot"]["anchor"]["scope"]["lane"] = 7
        self.assertEqual(session.selected_control["request"]["count"], 1)
        self.assertEqual(session._anchor["scope"]["lane"], 0)
        body = session.parse_command("allocations")
        body["expected_snapshot"]["scope"]["lane"] = 7
        with self.assertRaises(ProtocolError):
            session.begin(body)
        self.assertEqual(session.sent, 2)
        self.assertIsNone(session.pending)

    def test_caller_headers_filters_anchors_or_pagination_cannot_be_forwarded(self):
        for mutation in (
            lambda b: b.update(request_id=77), lambda b: b.update(expected_revision=77),
            lambda b: b.update(address_space="private"), lambda b: b["page"].update(token="opaque"),
            lambda b: b["page"].update(max_items=17),
            lambda b: b["expected_snapshot"]["cursor"].update(event_sequence=77),
        ):
            session = ready()
            body = session.parse_command("allocations")
            mutation(body)
            with self.subTest(mutation=mutation), self.assertRaises(ProtocolError):
                session.begin(body)
            self.assertEqual(session.sent, 2)

    def test_only_supported_active_step_can_select_not_state_or_continue(self):
        for command in ("state", "continue 1"):
            session = ready()
            session.clear_selection()
            request = session.begin(session.parse_command(command))
            if command == "state":
                result = {"result": "state", "snapshot": {"status": "captured",
                    "snapshot": {"anchor": selected(session.view), "stop": copy.deepcopy(STOP), "values": []}}}
                session.accept(v1(request, session.view, result))
            else:
                session.accept(step_response(request, session.view))
            self.assertIsNone(session.selected_control)
        for mutation in (
            lambda r: r["result"]["stop"].update(outcome="completed"),
            lambda r: r["result"]["stop"].update(exact=False),
            lambda r: r["result"]["snapshot"]["snapshot"]["anchor"].update(frame=1, occurrence=1),
            lambda r: r["result"]["snapshot"]["snapshot"]["anchor"].pop("site"),
            lambda r: r["result"]["snapshot"]["snapshot"]["anchor"]["scope"].update(lane=32),
            lambda r: r["result"]["snapshot"]["snapshot"]["anchor"]["scope"].update(interpretation="hardware_observed"),
        ):
            session = ready()
            request = session.begin(session.parse_command("step 1"))
            reply = step_response(request, session.view)
            mutation(reply)
            session.accept(reply)
            self.assertIsNone(session.selected_control)

    def test_terminal_and_uncaptured_stops_do_not_reuse_old_selection(self):
        session = ready()
        inventory(session); stack(session)
        request = session.begin(session.parse_command("continue 1"))
        result = {"result": "control", "stop": {"reason": "completed", "outcome": "completed", "exact": True},
                  "events_advanced": 1, "snapshot": {"status": "unavailable", "reason": "not_captured"}}
        session.accept(v1(request, view(2, 2), result))
        self.assertIsNone(session.selected_control)
        self.assertIsNone(session._frame)
        self.assertEqual(session._inventory, {})

    def test_clear_precedes_every_control_or_filter_dispatch(self):
        for command in ("step 1", "reverse 1", "continue 1", "break add 0 2 3", "break remove 1",
                        "watch add 1 0 0 4 write", "watch remove 1"):
            session = ready(); inventory(session); stack(session)
            session.begin(session.parse_command(command))
            self.assertIsNone(session.selected_control, command)
            self.assertIsNone(session._frame)
            self.assertEqual(session._inventory, {})

    def test_one_complete_frame_required_and_next_operation_is_not_site_operation(self):
        session = ready()
        stack(session)
        self.assertEqual(session._frame["next_operation"], 4)
        self.assertEqual(session._anchor["site"]["kir"]["point"]["operation_ordinal"], 3)
        self.assertEqual(session.parse_command("variables 1")["frame"], 1)
        for mutation in (
            lambda r: r["frames"].append({**frame_row(), "frame": 2}),
            lambda r: r.update(next_cursor={"query_identity": "c"*64, "position": 1}),
            lambda r: r["frames"][0].pop("next_operation"),
            lambda r: r["frames"][0].update(frame=2),
            lambda r: r["frames"][0].update(function_ordinal=1),
            lambda r: r["frames"][0].update(block_ordinal=3),
        ):
            current = ready(); stack(current, mutation)
            with self.subTest(mutation=mutation), self.assertRaises(CommandError):
                current.parse_command("variables 1")

    def test_resource_envelope_and_whole_session_mismatches_poison_selection(self):
        for mutation in (
            lambda r: r.update(schema="fe2o3-debug-response-v1"),
            lambda r: r.update(request_id=True), lambda r: r.update(request_id=99),
            lambda r: r.update(operation="query_memory_accesses"),
            lambda r: r["session"].update(state="running"),
            lambda r: r["session"].update(hardware_observed=True),
            lambda r: r.update(proof_authority=True),
            lambda r: r["snapshot"]["scope"].update(lane=1),
        ):
            session = ready()
            request = session.begin(session.parse_command("allocations"))
            reply = resource(request, session.view)
            mutation(reply)
            with self.subTest(mutation=mutation), self.assertRaises((ProtocolError, KeyError, TypeError)):
                session.accept(reply)
            self.assertIsNone(session.selected_control)

    def test_resource_rows_pages_and_literal_unavailable_facts(self):
        for mutation in (
            lambda r: r.update(physical_registers={"status": "captured"}),
            lambda r: r["result"]["allocations"][0].update(lifetime="live"),
            lambda r: r["result"]["allocations"][0].update(physical_base="0x1000"),
            lambda r: r["result"]["allocations"][0].update(owning_scope={}),
            lambda r: r["result"]["allocations"][0].update(alignment=3),
            lambda r: r["result"]["allocations"][0].update(capacity_bytes="024"),
            lambda r: r["result"]["allocations"][0].update(address_space="workgroup"),
            lambda r: r["result"]["allocations"][0]["allocation"].update(generation=1),
            lambda r: r["result"]["allocations"][0].update(initialization_available=False),
            lambda r: r["page"].update(scanned=65, source_count=65),
            lambda r: r["page"].update(next_token=None),
            lambda r: (r["result"]["allocations"].append(allocation_row()),
                       r["page"].update(scanned=2, source_count=2)),
        ):
            session = ready()
            request = session.begin(session.parse_command("allocations"))
            reply = resource(request, session.view)
            mutation(reply)
            with self.subTest(mutation=mutation), self.assertRaises(ProtocolError):
                session.accept(reply)

    def test_partial_inventory_observed_rows_allow_access_without_forwarding_token(self):
        session = ready()
        request = session.begin(session.parse_command("allocations"))
        reply = resource(request, session.view)
        reply["page"].update(source_count=20, scanned=16, next_token="opaque.1")
        session.accept(reply)
        self.assertEqual(reply["page"]["next_token"], "opaque.1")
        self.assertNotIn("token", session.parse_command("accesses 1 0")["page"])
        with self.assertRaises(CommandError):
            session.parse_command("accesses 2 0")

    def test_allocation_ordinal_preserves_u64_above_safe_js_range(self):
        session = ready()
        ordinal = (1 << 64)-1
        inventory(session, ordinal)
        request = session.begin(session.parse_command("accesses " + str(ordinal) + " 0"))
        self.assertEqual(request["filter"]["allocation"]["ordinal"], ordinal)

    def test_access_occurrence_range_filter_and_no_authority(self):
        def fixture():
            session = ready(); inventory(session)
            request = session.begin(session.parse_command("accesses 1 0"))
            row = {"occurrence": {"record_ordinal": 0, "event_sequence": 1,
                   "scope": copy.deepcopy(session._anchor["scope"]),
                   "site": copy.deepcopy(session._anchor["site"]["kir"]),
                   "schedule": {"identity": "workgroup_major_local_zyx_cooperative_v1", "decision_ordinal": 0}},
                   "allocation": {"ordinal": 1, "generation": 0}, "range": {"byte_offset": "0", "byte_len": "4"},
                   "address_space": "global", "access": "write_committed", "call_frame": "not_represented",
                   "operation_occurrence": "not_represented", "source_association": "not_represented"}
            return session, resource(request, session.view, [row])
        session, reply = fixture()
        session.accept(reply)
        for mutation in (
            lambda r: r["result"]["accesses"][0]["allocation"].update(ordinal=2),
            lambda r: r["result"]["accesses"][0]["range"].update(byte_len="0"),
            lambda r: r["result"]["accesses"][0]["range"].update(byte_offset=str((1 << 64)-1)),
            lambda r: r["result"]["accesses"][0]["occurrence"].update(record_ordinal=1),
            lambda r: r["result"]["accesses"][0]["occurrence"].update(event_sequence=2),
            lambda r: r["result"]["accesses"][0].update(access="write"),
            lambda r: r["result"]["accesses"][0].update(source_association="matched"),
            lambda r: r["result"]["accesses"][0]["occurrence"]["scope"].update(wave_width=64),
        ):
            session, reply = fixture()
            mutation(reply)
            with self.subTest(mutation=mutation), self.assertRaises(ProtocolError):
                session.accept(reply)

    def test_source_framed_refinement_rows_and_original_next_cursor(self):
        session = ready(); stack(session)
        request = session.begin(session.parse_command("variables 1"))
        reply = variables(request, session, [source_value(n) for n in range(16)])
        reply["next_cursor"] = {"query_identity": "c"*64, "position": 16}
        session.accept(reply)
        self.assertEqual(reply["snapshot"]["occurrence"], 1)
        self.assertNotIn("cursor", session.parse_command("variables 1")["page"])
        for mutation in (
            lambda r: r["snapshot"].update(occurrence=2),
            lambda r: r["snapshot"].update(frame=2),
            lambda r: r["snapshot"]["site"]["kir"].update(block_ordinal=3),
            lambda r: r["values"][0].update(function_ordinal=1),
            lambda r: r["values"][0].update(generation=0),
            lambda r: r["values"][0]["availability"]["value"].update(provenance="reconstructed"),
            lambda r: r["values"][0]["availability"]["value"]["value"].update(bits="0xff"),
            lambda r: r.update(next_cursor=None),
            lambda r: r.update(next_cursor={"query_identity": "c"*64, "position": 1}),
            lambda r: r["values"].append(source_value()),
        ):
            current = ready(); stack(current)
            req = current.begin(current.parse_command("variables 1"))
            bad = variables(req, current)
            mutation(bad)
            with self.subTest(mutation=mutation), self.assertRaises(ProtocolError):
                current.accept(bad)

    def test_source_ambiguous_and_unavailable_are_not_promoted_to_ssa(self):
        session = ready(); stack(session)
        rows = [source_value(0), source_value(1)]
        rows[0].update(generation=0, availability={"status": "ambiguous"})
        rows[1].update(generation=0, availability={"status": "value",
                                                 "value": {"status": "unavailable", "reason": "not_represented"}})
        request = session.begin(session.parse_command("variables 1"))
        reply = variables(request, session, rows)
        session.accept(reply)
        self.assertEqual(reply["values"], rows)

    def test_real_schema_refusals_consume_one_id_and_keep_exact_session(self):
        for status in ("unavailable", "error"):
            session = ready(); before = copy.deepcopy(session.view)
            req = session.begin(session.parse_command("allocations"))
            reply = resource(req, session.view, status=status)
            session.accept(reply)
            self.assertEqual(session.sent, 3)
            self.assertEqual(session.view, before)
            self.assertIsNone(session.pending)
            self.assertEqual(session._inventory, {})
            self.assertIsNotNone(session.selected_control)
            stack(session)
            req = session.begin(session.parse_command("variables 1"))
            reply = variables(req, session, status=status)
            session.accept(reply)
            self.assertEqual(session.sent, 5)
            self.assertEqual(session.view, before)
            self.assertNotIn("unavailable", reply)

    def test_shared_254_post_handshake_budget_no_second_query_ledger(self):
        session = ready()
        session.sent = 254  # Synthetic near-budget state; no subprocess requests.
        req = session.begin(session.parse_command("allocations"))
        self.assertEqual(req["request_id"], 255)
        session.accept(resource(req, session.view))
        with self.assertRaises(ProtocolError):
            session.begin(session.parse_command("allocations"))
        self.assertEqual(session.sent, 255)
        self.assertIsNone(session.pending)

    def test_existing_generic_framer_keeps_both_separately_versioned_schemas(self):
        session = ready(); stack(session)
        req = session.begin(session.parse_command("variables 1"))
        reply = variables(req, session)
        data = encode(reply)+b"\n"
        framer, received = LineFramer(), []
        for position in range(0, len(data), 23):
            framer.feed(data[position:position+23], received.append)
        self.assertEqual(received, [reply])
        self.assertEqual(received[0]["schema"], VARIABLE_RESPONSE)

    def test_v1_readonly_does_not_create_or_destroy_a_valid_selection(self):
        session = ready()
        before = session.selected_control
        req = session.begin(session.parse_command("state"))
        session.accept(v1(req, session.view, {"result": "state",
            "snapshot": {"status": "unavailable", "reason": "not_captured"}}))
        self.assertEqual(session.selected_control, before)


class BridgeIntegrationTests(unittest.TestCase):
    def test_bridge_uses_original_mixed_response_schemas_and_one_sequence(self):
        class Pins:
            def check(self, full=False):
                pass
        class Process:
            def __init__(self):
                self.current = view()
                self.replies = []
            def start(self, argv):
                pass
            def close(self):
                return True
            def exchange(self, request, deadline, peer=None):
                operation = request["operation"]
                if operation == "discover_capabilities":
                    reply = v1(request, self.current, {"result": "capabilities", "capabilities": []})
                elif operation == "step":
                    reply = step_response(request, self.current)
                    self.current = copy.deepcopy(reply["session"])
                elif operation == "query_allocations":
                    reply = resource(request, self.current)
                else:
                    raise AssertionError("unexpected synthetic operation")
                self.replies.append(copy.deepcopy(reply))
                return reply
        process = Process()
        owner = bridge_session.BridgeSession(("/unused/debugger",), Pins(), lambda: process, lambda: 0)
        connected = owner.connect("1"*64, None)
        self.assertEqual(connected["sequence"], "0")
        def command(line, sequence, revision):
            return owner.handle_request({"schema": bridge_session.REQUEST_SCHEMA, "action": "command",
                "bridge_session": owner.bridge_session, "sequence": str(sequence),
                "expected_revision": str(revision), "command": line})
        stepped = command("step 1", 1, 0)
        self.assertEqual(stepped["sequence"], "1")
        observed = command("allocations", 2, 1)
        self.assertEqual(observed["sequence"], "2")
        self.assertEqual(observed["response_json"], encode(process.replies[-1]).decode("ascii"))
        self.assertEqual(process.replies[-1]["schema"], RESOURCE_RESPONSE)
        self.assertEqual(observed["session"]["revision"], "1")
        self.assertEqual(observed["session"]["cursor"]["event_sequence"], "1")
        self.assertEqual(owner.protocol.sent, 3)
        self.assertEqual(len(process.replies), 3)
        self.assertTrue(owner.close())

    def test_predispatch_missing_selection_does_not_send_and_close_clears_selection(self):
        class Pins:
            def check(self, full=False):
                pass
        class Process:
            def __init__(self):
                self.requests = []
            def start(self, argv):
                pass
            def close(self):
                return True
            def exchange(self, request, deadline, peer=None):
                self.requests.append(copy.deepcopy(request))
                return v1(request, view(), {"result": "capabilities", "capabilities": []})
        process = Process()
        owner = bridge_session.BridgeSession(("/unused/debugger",), Pins(), lambda: process, lambda: 0)
        owner.connect("1"*64, None)
        request = {"schema": bridge_session.REQUEST_SCHEMA, "action": "command",
                   "bridge_session": owner.bridge_session, "sequence": "1",
                   "expected_revision": "0", "command": "allocations"}
        with self.assertRaises(bridge_session.BridgeError) as refused:
            owner.handle_request(request)
        self.assertEqual(refused.exception.code, "command_refused")
        self.assertEqual(len(process.requests), 1)
        self.assertEqual(owner.protocol.sent, 1)
        owner.protocol = ready()
        inventory(owner.protocol); stack(owner.protocol)
        self.assertTrue(owner.close())
        self.assertIsNone(owner.protocol.selected_control)
        self.assertEqual(owner.protocol._inventory, {})
        self.assertIsNone(owner.protocol._frame)


if __name__ == "__main__":
    unittest.main()
