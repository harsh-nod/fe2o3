"""Synthetic exact-correlation controls, not actual-source qualification."""
import copy
import unittest

from debug_console_commands import parse_command
from debug_console_protocol import MAX_COMMANDS, ProtocolError, Session

IDENTITY = "a" * 64

def view(revision=0, sequence=0, state="stopped"):
    return {"backend": "cpu_kir_simulator", "execution_kind": "cpu_kir_simulation",
            "state": state, "revision": revision, "configuration_identity": IDENTITY,
            "cursor": {"configuration_identity": IDENTITY, "event_sequence": sequence,
                       "state_revision": revision},
            "simulated": True, "hardware_observed": False, "performance_prediction": False}

def response(request, current, result):
    return {"status": "ok", "schema": "fe2o3-debug-response-v1",
            "request_id": request["request_id"], "operation": request["operation"],
            "session": copy.deepcopy(current), "result": result}

def unavailable_snapshot():
    return {"status": "unavailable", "reason": "no_current_event"}

def handshake():
    session = Session()
    request = session.begin({"operation": "discover_capabilities"})
    session.accept(response(request, view(), {"result": "capabilities", "capabilities": []}))
    return session

def anchor(current):
    return {"cursor": copy.deepcopy(current["cursor"]), "scope": {"level": "dispatch"}}

class SessionTests(unittest.TestCase):
    def test_handshake_headers_and_pending_ownership(self):
        session = Session()
        with self.assertRaises(ProtocolError):
            session.begin(parse_command("state"))
        request = session.begin({"operation": "discover_capabilities"})
        self.assertEqual(request["request_id"], 1)
        self.assertEqual(request["expected_revision"], 0)
        request["request_id"] = 99
        self.assertEqual(session.pending["request_id"], 1)
        with self.assertRaises(ProtocolError):
            session.begin(parse_command("state"))
        with self.assertRaises(ProtocolError):
            Session().accept({})

    def test_readonly_and_duplicate(self):
        session = handshake()
        request = session.begin(parse_command("state"))
        reply = response(request, view(), {"result": "state", "snapshot": unavailable_snapshot()})
        self.assertEqual(session.accept(reply), reply)
        with self.assertRaises(ProtocolError):
            session.accept(reply)

    def test_correlation_and_truth_refusals_do_not_commit(self):
        changes = [
            lambda r: r.update(request_id=9),
            lambda r: r.update(request_id=True),
            lambda r: r.update(operation="step"),
            lambda r: r.update(schema="other"),
            lambda r: r["session"].update(configuration_identity="b" * 64),
            lambda r: r["session"].update(hardware_observed=True),
            lambda r: r["session"]["cursor"].update(state_revision=1),
            lambda r: r["session"].update(revision=True),
            lambda r: r["result"].update(result="control"),
            lambda r: r.update(extra=True),
        ]
        for change in changes:
            session = handshake()
            request = session.begin(parse_command("state"))
            reply = response(request, view(), {"result": "state", "snapshot": unavailable_snapshot()})
            change(reply)
            with self.subTest(change=change), self.assertRaises(ProtocolError):
                session.accept(reply)
            self.assertEqual(session.view, view())
            self.assertIsNotNone(session.pending)

    def test_readonly_cannot_bump_or_move(self):
        for changed in (view(1), view(0, 1)):
            session = handshake()
            request = session.begin(parse_command("state"))
            with self.assertRaises(ProtocolError):
                session.accept(response(request, changed, {"result": "state", "snapshot": unavailable_snapshot()}))

    def test_filter_mutation_exactly_one_revision_and_no_cursor_move(self):
        for current, valid in ((view(1), True), (view(), False), (view(2), False), (view(1, 1), False)):
            session = handshake()
            request = session.begin(parse_command("break add 0 2 1"))
            reply = response(request, current, {"result": "acknowledged", "accepted": 1})
            if valid:
                session.accept(reply)
                self.assertEqual(session.begin(parse_command("state"))["expected_revision"], 1)
            else:
                with self.assertRaises(ProtocolError):
                    session.accept(reply)

    def test_control_noop_movement_reverse_and_delta(self):
        session = handshake()
        for text, current, advanced in (("step", view(1, 9), 9), ("reverse", view(2, 3), 6),
                                        ("step", view(2, 3), 0)):
            request = session.begin(parse_command(text))
            session.accept(response(request, current, {"result": "control",
                "snapshot": unavailable_snapshot(), "events_advanced": advanced}))
        for current, advanced in ((view(2, 4), 1), (view(4, 3), 0), (view(3, 4), 0)):
            request = session.begin(parse_command("step"))
            with self.assertRaises(ProtocolError):
                session.accept(response(request, current, {"result": "control",
                    "snapshot": unavailable_snapshot(), "events_advanced": advanced}))
            session.pending = None  # Independent synthetic case only.

    def test_refusal_must_preserve_exact_session(self):
        for status, payload in (("error", {"state_changed": False, "message": "synthetic refusal"}),
                                ("unavailable", {"state_changed": False, "detail": "synthetic refusal"})):
            session = handshake()
            request = session.begin(parse_command("stack"))
            reply = response(request, view(), {})
            reply.pop("result")
            reply["status"], reply[status] = status, payload
            session.accept(reply)
            request = session.begin(parse_command("stack"))
            reply["request_id"] = request["request_id"]
            reply["session"] = view(1)
            with self.assertRaises(ProtocolError):
                session.accept(reply)

    def test_same_stop_snapshot_and_page_bound(self):
        for cursor, frames, valid in ((view()["cursor"], [], True), (view(1)["cursor"], [], False),
                                      (view()["cursor"], [{}] * 17, False)):
            session = handshake()
            request = session.begin(parse_command("stack"))
            reply = response(request, view(), {"result": "stack",
                "snapshot": {"cursor": cursor, "scope": {"level": "dispatch"}}, "frames": frames})
            if valid:
                session.accept(reply)
            else:
                with self.assertRaises(ProtocolError):
                    session.accept(reply)
        session = handshake()
        request = session.begin(parse_command("state"))
        reply = response(request, view(), {"result": "state",
            "snapshot": {"status": "captured", "snapshot": {}}})
        with self.assertRaises(ProtocolError):
            session.accept(reply)

    def test_source_site_and_boolean_alias_refused(self):
        for ordinal, valid in ((7, True), (9, False), (True, False)):
            session = handshake()
            request = session.begin(parse_command("source 1 7 2"))
            site = copy.deepcopy(request["site"])
            site["block_ordinal"] = ordinal
            reply = response(request, view(), {"result": "source",
                "site": {"kir": site, "source": {"status": "unavailable", "reason": "synthetic"}}})
            if valid:
                session.accept(reply)
            else:
                with self.assertRaises(ProtocolError):
                    session.accept(reply)
        # Explicit bool==int trap on an ordinal whose requested value is one.
        session = handshake()
        request = session.begin(parse_command("source 1 7 2"))
        site = copy.deepcopy(request["site"])
        site["function_ordinal"] = True
        with self.assertRaises(ProtocolError):
            session.accept(response(request, view(), {"result": "source", "site": {"kir": site}}))

    def test_memory_exact_extent_identity_and_initialization(self):
        base = {"allocation": {"ordinal": 1, "generation": 3}, "byte_offset": 0,
                "requested_bytes": 1, "returned_bytes": 1,
                "availability": {"status": "captured", "address_space": "global",
                                 "bytes": "0x42", "initialized": "0x01", "truncated": False}}
        changes = [None, lambda m: m["allocation"].update(generation=0),
                   lambda m: m["allocation"].update(ordinal=True),
                   lambda m: m.update(byte_offset=True),
                   lambda m: m.update(requested_bytes=True),
                   lambda m: m.update(returned_bytes=2),
                   lambda m: m["availability"].update(bytes="0x"),
                   lambda m: m["availability"].update(initialized="0x80"),
                   lambda m: m.update(availability={"status": "unavailable", "reason": "synthetic"})]
        for change in changes:
            session = handshake()
            request = session.begin(parse_command("memory 1 3 0 1"))
            memory = copy.deepcopy(base)
            if change:
                change(memory)
            reply = response(request, view(), {"result": "memory", "snapshot": anchor(view()), "memory": memory})
            if change is None:
                session.accept(reply)
            else:
                with self.assertRaises(ProtocolError):
                    session.accept(reply)

    def test_request_cap_reserves_quit_and_termination_revision(self):
        session = handshake()
        session.sent = MAX_COMMANDS - 1
        with self.assertRaises(ProtocolError):
            session.begin(parse_command("state"))
        request = session.begin(parse_command("quit"))
        session.accept(response(request, view(1, 0, "terminated"), {"result": "terminated"}))
        with self.assertRaises(ProtocolError):
            session.begin(parse_command("quit"))

class ReviewedCorrelationTests(unittest.TestCase):
    def test_zero_acknowledgement_refused_without_commit(self):
        for command in ("break add 0 2 1", "break remove 1",
                        "watch add 1 0 0 4 write", "watch remove 1"):
            with self.subTest(command=command):
                session = handshake()
                request = session.begin(parse_command(command))
                reply = response(request, view(1), {"result": "acknowledged", "accepted": 0})
                with self.assertRaises(ProtocolError):
                    session.accept(reply)
                self.assertEqual(session.view, view())
                self.assertEqual(session.pending, request)

    def test_control_direction_noop_and_nonunit_delta(self):
        cases = (("step", 9, False), ("reverse", 11, False),
                 ("continue 100", 9, False), ("step", 17, True),
                 ("reverse", 3, True), ("continue 100", 17, True),
                 ("step", 10, True), ("reverse", 10, True),
                 ("continue 100", 10, True))
        for command, sequence, valid in cases:
            with self.subTest(command=command, sequence=sequence):
                session = handshake()
                session.view = view(4, 10)  # Independent synthetic prior stop.
                request = session.begin(parse_command(command))
                current = view(4 if sequence == 10 else 5, sequence)
                reply = response(request, current, {"result": "control",
                    "snapshot": {"status": "unavailable", "reason": "not_captured"},
                    "events_advanced": abs(sequence - 10)})
                if valid:
                    session.accept(reply)
                    self.assertEqual(session.view, current)
                    self.assertIsNone(session.pending)
                else:
                    with self.assertRaises(ProtocolError):
                        session.accept(reply)
                    self.assertEqual(session.view, view(4, 10))
                    self.assertEqual(session.pending, request)

    def test_terminate_preserves_cursor_but_changes_state(self):
        for sequence, valid in ((9, False), (11, False), (10, True)):
            with self.subTest(sequence=sequence):
                session = handshake()
                session.view = view(4, 10)
                request = session.begin(parse_command("quit"))
                current = view(5, sequence, "terminated")
                reply = response(request, current, {"result": "terminated"})
                if valid:
                    session.accept(reply)
                    self.assertEqual(session.view, current)
                    self.assertIsNone(session.pending)
                else:
                    with self.assertRaises(ProtocolError):
                        session.accept(reply)
                    self.assertEqual(session.view, view(4, 10))
                    self.assertEqual(session.pending, request)

if __name__ == "__main__":
    unittest.main()
