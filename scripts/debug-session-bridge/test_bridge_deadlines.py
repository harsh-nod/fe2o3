"""Injected-clock transport controls only; no subprocess or socket is started."""
import json
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import bridge_http
import bridge_session
import fe2o3_debug_bridge
from bridge_session import BridgeError, CPUProcess
from debug_console_protocol import LineFramer, ProtocolError, encode
from test_bridge_session import (HOST, ORIGIN, TOKEN, Pins, Process, command,
                                 connect_request, fixture)


class Selector:
    """A finite synthetic readiness sequence, not an operating-system selector."""
    def __init__(self):
        self.round = 0
        self.closed = False

    def register(self, *_args):
        pass

    def unregister(self, *_args):
        pass

    def select(self, _timeout):
        self.round += 1
        if self.round == 1:
            return []
        if self.round == 2:
            return [(SimpleNamespace(data="request", fd=10), 0)]
        if self.round == 3:
            return [(SimpleNamespace(data="stdout", fd=11), 0)]
        raise AssertionError("unexpected retry/read")

    def close(self):
        self.closed = True


class ChildDeadlines(unittest.TestCase):
    def exchange(self, end, during_decode):
        now = [100.0]
        owner = CPUProcess(clock=lambda: now[0])
        owner.child = SimpleNamespace(poll=lambda: None, stdin=object(),
                                      stdout=object(), stderr=object())
        selector = Selector()
        reply = {"schema": "fe2o3-debug-response-v1", "request_id": 1}
        raw = encode(reply) + b"\n"
        actual_framer = LineFramer()
        if during_decode:
            def feed(chunk, receive):
                actual_framer.feed(chunk, receive)
                now[0] = end
            owner.framer = SimpleNamespace(feed=feed, partial=False)

        def read(_fd, _length):
            if not during_decode:
                now[0] = end
            return raw

        with patch.object(bridge_session.selectors, "DefaultSelector", return_value=selector), \
                patch.object(bridge_session.os, "write", side_effect=lambda _fd, raw: len(raw)), \
                patch.object(bridge_session.os, "read", side_effect=read):
            try:
                return owner.exchange({"operation": "discover_capabilities"}, 105.0)
            finally:
                self.assertTrue(selector.closed)
                self.assertEqual(selector.round, 3)

    def test_final_read_before_deadline_succeeds(self):
        self.assertEqual(self.exchange(104.999, False)["request_id"], 1)

    def test_final_read_exact_and_late_refuse(self):
        for end in (105.0, 105.001):
            with self.subTest(end=end), self.assertRaises(ProtocolError):
                self.exchange(end, False)

    def test_final_decode_before_deadline_succeeds(self):
        self.assertEqual(self.exchange(104.999, True)["request_id"], 1)

    def test_final_decode_exact_and_late_refuse(self):
        for end in (105.0, 106.0):
            with self.subTest(end=end), self.assertRaises(ProtocolError):
                self.exchange(end, True)


class SessionDeadlines(unittest.TestCase):
    def refused(self, owner, child, request, closed=True):
        before = len(child.requests)
        with self.assertRaises(BridgeError) as caught:
            owner.handle_request(request)
        self.assertEqual((caught.exception.code, caught.exception.outcome, caught.exception.closed),
                         ("backend_failed", "unknown", closed))
        self.assertTrue(owner.poisoned)
        self.assertEqual(child.closes, 1)
        self.assertEqual(len(child.requests), before + 1)
        with self.assertRaises(BridgeError) as again:
            owner.handle_request(request)
        self.assertEqual(again.exception.outcome, "not_sent")
        self.assertEqual(len(child.requests), before + 1)

    def test_reply_before_deadline_preserves_response(self):
        owner, child, _pins, now, _ = fixture()
        child.change = lambda _reply: now.__setitem__(0, 129.999)
        result = owner.handle_request(command(owner))
        self.assertEqual(result["status"], "ok")
        self.assertEqual(result["sequence"], "1")
        self.assertFalse(owner.poisoned)

    def test_late_reply_exact_and_after_deadline_is_unknown_no_retry(self):
        for end in (130.0, 131.0):
            with self.subTest(end=end):
                owner, child, _pins, now, _ = fixture()
                child.change = lambda _reply: now.__setitem__(0, end)
                self.refused(owner, child, command(owner))

    def test_post_reply_custody_work_is_included(self):
        owner, child, pins, now, _ = fixture()
        original = pins.check
        checks = [0]
        def check(full=False):
            original(full)
            checks[0] += 1
            if checks[0] == 2:
                now[0] = 130.0
        pins.check = check
        self.refused(owner, child, command(owner))

    def test_final_projection_work_is_included(self):
        owner, child, _pins, now, _ = fixture()
        original = bridge_session.projected_session
        def project(view):
            result = original(view)
            now[0] = 130.0
            return result
        with patch.object(bridge_session, "projected_session", side_effect=project):
            self.refused(owner, child, command(owner))

    def test_absolute_session_deadline_does_not_gain_thirty_seconds(self):
        owner, child, _pins, now, _ = fixture()
        now[0] = 999.0  # Original start 100; absolute expiry 1000.
        child.change = lambda _reply: now.__setitem__(0, 1000.0)
        self.refused(owner, child, command(owner))

    def test_failed_reap_retains_owned_handle_after_late_reply(self):
        owner, child, _pins, now, _ = fixture()
        child.reaped = False
        child.change = lambda _reply: now.__setitem__(0, 130.0)
        self.refused(owner, child, command(owner), closed=False)
        self.assertIs(owner.process, child)

    def test_late_connect_does_not_publish_discovery(self):
        pins, child, now = Pins(), Process(), [100.0]
        child.change = lambda _reply: now.__setitem__(0, 130.0)
        owner = bridge_session.BridgeSession(("/fixed/cli",), pins, lambda: child, lambda: now[0])
        self.refused(owner, child, connect_request())


def request_wire(preflight=False):
    if preflight:
        lines = ["OPTIONS /v1/connect HTTP/1.1", "Host: " + HOST, "Origin: " + ORIGIN,
                 "Access-Control-Request-Method: POST",
                 "Access-Control-Request-Headers: content-type,x-fe2o3-bridge-token"]
        body = b""
    else:
        body = json.dumps(connect_request(), separators=(",", ":")).encode("ascii")
        lines = ["POST /v1/connect HTTP/1.1", "Host: " + HOST, "Origin: " + ORIGIN,
                 "Content-Type: application/json", "X-Fe2o3-Bridge-Token: " + TOKEN,
                 "Content-Length: " + str(len(body))]
    return ("\r\n".join(lines) + "\r\n\r\n").encode("ascii") + body


class HTTPDeadlines(unittest.TestCase):
    def read(self, end, preflight=False, decode=False):
        now = [100.0]
        def recv(_length):
            if not decode:
                now[0] = end
            return request_wire(preflight)
        peer = SimpleNamespace(recv=recv)
        original = bridge_http.decode_body
        def decoded(*args):
            value = original(*args)
            now[0] = end
            return value
        with patch.object(bridge_http, "decode_body", side_effect=decoded):
            return bridge_http.read_request(peer, HOST, ORIGIN, TOKEN, clock=lambda: now[0])

    def test_final_authenticated_read_and_preflight_before_deadline(self):
        self.assertEqual(self.read(104.999), connect_request())
        self.assertIsNone(self.read(104.999, preflight=True))

    def test_final_authenticated_read_and_preflight_exact_or_late_refuse(self):
        for preflight in (False, True):
            for end in (105.0, 106.0):
                with self.subTest(preflight=preflight, end=end), self.assertRaises(BridgeError):
                    self.read(end, preflight=preflight)

    def test_decode_work_is_included(self):
        self.assertEqual(self.read(104.999, decode=True), connect_request())
        for end in (105.0, 106.0):
            with self.subTest(end=end), self.assertRaises(BridgeError):
                self.read(end, decode=True)

    def test_final_send_before_deadline_succeeds(self):
        now = [100.0]
        def send(raw):
            now[0] = 104.999
            return len(raw)
        bridge_http.write_response(SimpleNamespace(send=send), b"response", clock=lambda: now[0])

    def test_final_send_exact_or_late_reports_unknown_transport(self):
        for end in (105.0, 106.0):
            now, sent = [100.0], []
            def send(raw):
                sent.append(raw)
                now[0] = end
                return len(raw)
            with self.subTest(end=end), self.assertRaises(OSError):
                bridge_http.write_response(SimpleNamespace(send=send), b"response", clock=lambda: now[0])
            self.assertEqual(sent, [b"response"])  # No claim of rollback or unsent bytes.

    def test_final_send_expiry_poisons_dispatched_session_without_retry(self):
        owner, child, _pins, _now, _ = fixture()
        now = [100.0]
        def send(raw):
            now[0] = 105.0
            return len(raw)
        peer = SimpleNamespace(send=send)
        def write(peer, wire):
            return bridge_http.write_response(peer, wire, clock=lambda: now[0])
        with patch.object(fe2o3_debug_bridge, "read_request", return_value=command(owner)), \
                patch.object(fe2o3_debug_bridge, "write_response", side_effect=write):
            self.assertTrue(fe2o3_debug_bridge.serve_peer(peer, owner, Mock(value=TOKEN), HOST, ORIGIN))
        self.assertTrue(owner.closed)
        self.assertTrue(owner.poisoned)
        self.assertEqual(child.closes, 1)
        self.assertEqual(len(child.requests), 2)


if __name__ == "__main__":
    unittest.main()
