"""Pure/mock bridge controls. No socket listener, subprocess, compiler or GPU is started."""
import copy
import hashlib
import json
from pathlib import Path
import stat
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import bridge_http
import bridge_inputs
import bridge_session
import fe2o3_debug_bridge
from bridge_http import decode_body, parse_headers, response_bytes, validate_headers
from bridge_inputs import CustodyError, FilePin, HashBudget, TokenFile, metadata
from bridge_session import (BridgeError, BridgeSession, CPUProcess, REQUEST_SCHEMA,
                            RESPONSE_SCHEMA, decimal, projected_session)

CID = "1" * 64
OTHER = "2" * 64
TOKEN = "3" * 64
HOST = "127.0.0.1:8741"
ORIGIN = "http://127.0.0.1:5173"


def view(revision=0, event=0):
    return {"backend": "cpu_kir_simulator", "execution_kind": "cpu_kir_simulation",
            "state": "stopped", "revision": revision, "configuration_identity": "a" * 64,
            "cursor": {"configuration_identity": "a" * 64, "event_sequence": event,
                       "state_revision": revision},
            "simulated": True, "hardware_observed": False, "performance_prediction": False}


class Pins:
    def __init__(self):
        self.calls = []
        self.failure = False

    def check(self, full=False):
        self.calls.append(full)
        if self.failure:
            raise CustodyError("synthetic selected-byte change")


class Process:
    def __init__(self):
        self.requests = []
        self.current = view()
        self.started = None
        self.reaped = True
        self.closes = 0
        self.failure = None
        self.refusal = None
        self.change = None

    def start(self, argv):
        self.started = list(argv)

    def exchange(self, request, _deadline, _peer):
        self.requests.append(copy.deepcopy(request))
        if self.failure:
            raise self.failure
        op = request["operation"]
        if self.refusal:
            status, payload = self.refusal
            return {"schema": "fe2o3-debug-response-v1", "request_id": request["request_id"],
                    "operation": op, "status": status, "session": copy.deepcopy(self.current),
                    status: copy.deepcopy(payload)}
        if op == "discover_capabilities":
            result = {"result": "capabilities", "capabilities": []}
        elif op == "get_state":
            result = {"result": "state", "snapshot": {"status": "unavailable", "reason": "not_captured"}}
        elif op in ("step", "continue"):
            delta = -1 if request.get("direction") == "reverse" else 1
            self.current = view(self.current["revision"] + 1,
                                self.current["cursor"]["event_sequence"] + delta)
            result = {"result": "control", "events_advanced": 1,
                      "snapshot": {"status": "unavailable", "reason": "not_captured"}}
        elif op.startswith("set_") or op.startswith("remove_"):
            self.current = view(self.current["revision"] + 1,
                                self.current["cursor"]["event_sequence"])
            result = {"result": "acknowledged", "accepted": 1}
        elif op in ("list_breakpoints", "list_watchpoints"):
            tag = op.removeprefix("list_")
            result = {"result": tag, tag: []}
        elif op == "inspect_stack":
            result = {"result": "stack", "frames": [],
                      "snapshot": {"cursor": copy.deepcopy(self.current["cursor"])}}
        elif op == "resolve_source":
            result = {"result": "source", "site": {"kir": copy.deepcopy(request["site"]),
                      "source": {"status": "unavailable", "reason": "synthetic"}}}
        elif op == "read_memory":
            result = {"result": "memory", "snapshot": {"cursor": copy.deepcopy(self.current["cursor"])},
                      "memory": {"allocation": copy.deepcopy(request["allocation"]),
                                 "byte_offset": request["byte_offset"], "requested_bytes": request["byte_len"],
                                 "returned_bytes": 0,
                                 "availability": {"status": "unavailable", "reason": "synthetic"}}}
        else:
            raise AssertionError("unsupported synthetic operation")
        reply = {"schema": "fe2o3-debug-response-v1", "request_id": request["request_id"],
                 "operation": op, "status": "ok", "session": copy.deepcopy(self.current), "result": result}
        if self.change:
            self.change(reply)
        return reply

    def close(self):
        self.closes += 1
        return self.reaped


def connect_request(cid=CID):
    return {"schema": REQUEST_SCHEMA, "action": "connect", "connection_id": cid}


def disconnected_request(cid=CID):
    return {"schema": REQUEST_SCHEMA, "action": "disconnect", "connection_id": cid}


def fixture():
    pins, child = Pins(), Process()
    now = [100.0]
    owner = BridgeSession(("/trusted/fe2o3-debug", "sim"), pins, lambda: child, lambda: now[0])
    reply = owner.handle_request(connect_request())
    return owner, child, pins, now, reply


def command(owner, text="state", **changes):
    request = {"schema": REQUEST_SCHEMA, "action": "command",
               "bridge_session": owner.bridge_session, "sequence": str(owner.protocol.sent),
               "expected_revision": str(owner.protocol.view["revision"]), "command": text}
    request.update(changes)
    return request


def headers():
    return {"host": HOST, "origin": ORIGIN, "content-type": "application/json",
            "content-length": "123", "x-fe2o3-bridge-token": TOKEN}


class SessionControls(unittest.TestCase):
    def assert_code(self, code, callback):
        with self.assertRaises(BridgeError) as caught:
            callback()
        self.assertEqual(caught.exception.code, code)
        return caught.exception

    def test_connect_exact_initial_correlation_and_fixed_argv(self):
        owner, child, pins, _now, reply = fixture()
        self.assertEqual(set(reply), {"schema", "status", "connection_id", "bridge_session",
                                     "sequence", "session", "response_json", "closed"})
        self.assertEqual(reply["schema"], RESPONSE_SCHEMA)
        self.assertEqual(reply["sequence"], "0")
        self.assertEqual(reply["connection_id"], CID)
        self.assertNotEqual(reply["bridge_session"], CID)
        self.assertEqual(child.started, ["/trusted/fe2o3-debug", "sim"])
        self.assertEqual(child.requests[0]["operation"], "discover_capabilities")
        self.assertEqual(child.requests[0]["request_id"], 1)
        self.assertFalse(reply["closed"])
        self.assertIn(True, pins.calls)
        self.assertNotIn("\n", reply["response_json"])

    def test_reconnect_never_replaces_live_child(self):
        owner, child, _pins, _now, _reply = fixture()
        error = self.assert_code("session_exists", lambda: owner.handle_request(connect_request(OTHER)))
        self.assertEqual(child.closes, 0)
        self.assertEqual(len(child.requests), 1)
        self.assertFalse(error.closed)

    def test_three_stale_dimensions_never_dispatch_or_mutate(self):
        for field, value, code in (("bridge_session", OTHER, "stale_session"),
                                   ("sequence", "0", "stale_sequence"),
                                   ("expected_revision", "1", "stale_revision")):
            owner, child, _pins, _now, _reply = fixture()
            self.assert_code(code, lambda: owner.handle_request(command(owner, **{field: value})))
            self.assertEqual(len(child.requests), 1)
            self.assertEqual(child.closes, 0)
            self.assertEqual(owner.protocol.sent, 1)

    def test_sequence_consumes_backend_refusal_but_revision_does_not(self):
        owner, child, _pins, _now, _reply = fixture()
        for status in ("unavailable", "error"):
            child.refusal = (status, {"state_changed": False, "reason": "synthetic"})
            previous = owner.protocol.sent
            reply = owner.handle_request(command(owner, "stack"))
            self.assertEqual(reply["status"], "ok")
            self.assertEqual(json.loads(reply["response_json"])["status"], status)
            self.assertEqual(reply["sequence"], str(previous))
            self.assertEqual(reply["session"]["revision"], "0")

    def test_existing_command_roster_reaches_unchanged_owner(self):
        owner, child, _pins, _now, _reply = fixture()
        for text in ("state", "step", "reverse", "continue 1", "break add 0 0 0",
                     "break list", "break remove 1", "watch add 1 0 0 4 write",
                     "watch list", "watch remove 1", "source 0 0 0", "stack", "memory 1 0 0 4"):
            result = owner.handle_request(command(owner, text))
            self.assertEqual(result["status"], "ok")
        self.assertEqual(len(child.requests), 14)

    def test_help_quit_empty_and_browser_injection_refused(self):
        for text in ("help", "quit", "", "shell /bin/sh", "state\nstep", "memory 1 0 0 4097"):
            owner, child, _pins, _now, _reply = fixture()
            self.assert_code("command_refused", lambda: owner.handle_request(command(owner, text)))
            self.assertEqual(len(child.requests), 1)
        owner, child, _pins, _now, _reply = fixture()
        self.assert_code("invalid_request", lambda: owner.handle_request(command(owner, argv=["bad"])))
        self.assertEqual(len(child.requests), 1)

    def test_exact_decimal_u64_and_boolean_refusals(self):
        self.assertEqual(decimal(str((1 << 64) - 1)), (1 << 64) - 1)
        for value in ("01", "-1", "1.0", "18446744073709551616", 1, True, None):
            self.assert_code("invalid_request", lambda: decimal(value))

    def test_high_u64_projection_and_lossless_reencoded_reply(self):
        owner, child, _pins, _now, _reply = fixture()
        large = (1 << 63) + 97
        child.current = view(large, large + 1)
        owner.protocol.view = copy.deepcopy(child.current)  # Explicit synthetic retained prior state.
        result = owner.handle_request(command(owner))
        self.assertEqual(result["session"]["revision"], str(large))
        self.assertEqual(result["session"]["cursor"]["event_sequence"], str(large + 1))
        self.assertEqual(result["session"]["cursor"]["state_revision"], str(large))
        self.assertEqual(json.loads(result["response_json"])["session"]["revision"], large)

    def test_disconnect_uses_original_connection_not_revision(self):
        owner, child, _pins, _now, _reply = fixture()
        owner.handle_request(command(owner, "step"))
        self.assert_code("stale_session", lambda: owner.handle_request(disconnected_request(OTHER)))
        self.assertEqual(child.closes, 0)
        result = owner.handle_request(disconnected_request())
        self.assertEqual(result, {"schema": RESPONSE_SCHEMA, "status": "disconnected",
                                  "connection_id": CID, "closed": True})
        self.assertTrue(owner.closed)
        self.assertEqual(len(child.requests), 2)  # No fabricated protocol terminate.
        self.assertEqual(owner.handle_request(disconnected_request()), result)

    def test_old_disconnect_cannot_close_replacement_and_nonce_cannot_repeat(self):
        owner, child, _pins, _now, _reply = fixture()
        owner.handle_request(disconnected_request())
        self.assert_code("stale_session", lambda: owner.handle_request(connect_request()))
        child.current = view()
        owner.handle_request(connect_request(OTHER))
        previous = child.closes
        self.assert_code("stale_session", lambda: owner.handle_request(disconnected_request()))
        self.assertEqual(child.closes, previous)
        self.assertFalse(owner.closed)

    def test_cleanup_failure_never_claims_closed_or_drops_handle(self):
        owner, child, _pins, _now, _reply = fixture()
        child.reaped = False
        error = self.assert_code("cleanup_failed", lambda: owner.handle_request(disconnected_request()))
        self.assertFalse(error.closed)
        self.assertIs(owner.process, child)
        self.assert_code("session_exists", lambda: owner.handle_request(connect_request(OTHER)))
        child.reaped = True
        self.assertTrue(owner.handle_request(disconnected_request())["closed"])

    def test_timeout_poison_unknown_no_retry_and_explicit_cleanup(self):
        owner, child, _pins, _now, _reply = fixture()
        child.failure = TimeoutError("synthetic")
        error = self.assert_code("backend_failed", lambda: owner.handle_request(command(owner, "step")))
        self.assertEqual(error.outcome, "unknown")
        self.assertTrue(error.closed)
        self.assertEqual(len(child.requests), 2)
        self.assert_code("session_unavailable", lambda: owner.handle_request(command(owner, "step")))
        self.assertEqual(len(child.requests), 2)
        self.assertTrue(owner.handle_request(disconnected_request())["closed"])

    def test_idle_deadline_reaps_without_new_command(self):
        owner, child, _pins, now, _reply = fixture()
        now[0] += 900
        owner.expire()
        self.assertTrue(owner.closed)
        self.assertEqual(child.closes, 1)
        self.assertEqual(len(child.requests), 1)

    def test_invalid_reply_identity_and_hardware_flag_poison(self):
        for mutate in (lambda r: r.update(request_id=9),
                       lambda r: r["session"].update(hardware_observed=True),
                       lambda r: r["session"]["cursor"].update(state_revision=1)):
            owner, child, _pins, _now, _reply = fixture()
            child.change = mutate
            self.assert_code("backend_failed", lambda: owner.handle_request(command(owner)))
            self.assertTrue(owner.closed)

    def test_reencoded_output_cap_and_no_secret_or_stderr_error_echo(self):
        owner, child, _pins, _now, _reply = fixture()
        child.change = lambda r: r["result"].update(detail="x" * (256 * 1024))
        error = self.assert_code("backend_failed", lambda: owner.handle_request(command(owner)))
        self.assertEqual(set(error.envelope()), {"schema", "status", "code", "outcome", "closed"})
        self.assertNotIn(TOKEN, json.dumps(error.envelope()))

    def test_input_mutation_prevents_dispatch_and_future_reconnect(self):
        owner, child, pins, _now, _reply = fixture()
        pins.failure = True
        error = self.assert_code("backend_failed", lambda: owner.handle_request(command(owner, "step")))
        self.assertEqual(error.outcome, "not_sent")
        self.assertEqual(len(child.requests), 1)
        self.assertTrue(owner.closed)
        pins.failure = False
        self.assert_code("backend_failed", lambda: owner.handle_request(connect_request(OTHER)))

    def test_connection_and_protocol_caps_are_closed(self):
        owner, child, _pins, _now, _reply = fixture()
        owner.protocol.sent = 255
        self.assert_code("resource_limit", lambda: owner.handle_request(command(owner)))
        self.assertTrue(owner.closed)
        self.assertEqual(len(child.requests), 1)
        owner.used_connections = {str(i) * 64 for i in range(1, 5)}
        self.assert_code("resource_limit", lambda: owner.handle_request(connect_request("5" * 64)))

    def test_process_cleanup_reuses_existing_owner(self):
        process = CPUProcess()
        process.child = Mock()
        for pipe in (process.child.stdin, process.child.stdout, process.child.stderr):
            pipe.closed = False
        with patch("bridge_session.cleanup", return_value=False) as old:
            self.assertFalse(process.close())
            old.assert_called_once_with(process.child)
            process.child.stdin.close.assert_not_called()
        with patch("bridge_session.cleanup", return_value=True):
            self.assertTrue(process.close())
            process.child.stdin.close.assert_called_once()


class HTTPControls(unittest.TestCase):
    def test_exact_header_profile(self):
        raw = ("POST /v1/connect HTTP/1.1\r\nHost: " + HOST +
               "\r\nOrigin: " + ORIGIN + "\r\nContent-Type: application/json\r\n" +
               "Content-Length: 123\r\nX-Fe2o3-Bridge-Token: " + TOKEN + "\r\n\r\n").encode("ascii")
        method, path, parsed = parse_headers(raw)
        self.assertEqual(path, "/v1/connect")
        self.assertEqual(validate_headers(method, parsed, HOST, ORIGIN, TOKEN), 123)

    def test_duplicate_host_origin_token_length_and_noncanonical_paths(self):
        prefix = b"POST /v1/connect HTTP/1.1\r\n"
        for name in ("Host", "Origin", "X-Fe2o3-Bridge-Token", "Content-Length"):
            with self.subTest(name=name), self.assertRaises(BridgeError):
                parse_headers(prefix + (name + ": a\r\n" + name.lower() + ": a\r\n\r\n").encode("ascii"))
        for path in ("/v1/connect?token=x", "/v1/connect#x", "http://127.0.0.1/v1/connect", "/v1/%63onnect"):
            with self.assertRaises(BridgeError):
                parse_headers(("POST " + path + " HTTP/1.1\r\n\r\n").encode("ascii"))

    def test_request_line_headers_and_folding_caps(self):
        for raw in (b"GET /v1/connect HTTP/1.1\r\n\r\n",
                    b"POST /v1/connect HTTP/1.0\r\n\r\n",
                    b"POST /v1/connect HTTP/1.1\r\n X: y\r\n\r\n",
                    b"POST /v1/connect HTTP/1.1\r\nX: a\tb\r\n\r\n",
                    b"x" * 8193):
            with self.assertRaises(BridgeError):
                parse_headers(raw)

    def test_auth_origin_host_transfer_expect_cookie_refusals(self):
        for key, value in (("host", "localhost:8741"), ("origin", "http://evil.example"),
                           ("x-fe2o3-bridge-token", "4" * 64), ("transfer-encoding", "chunked"),
                           ("expect", "100-continue"), ("content-encoding", "gzip"),
                           ("cookie", "session=x"), ("authorization", "Bearer x")):
            current = headers()
            current[key] = value
            with self.subTest(key=key), self.assertRaises(BridgeError):
                validate_headers("POST", current, HOST, ORIGIN, TOKEN)

    def test_content_type_and_length_are_closed_before_body_read(self):
        for length in ("0", "1", "01", "-1", "4097", "999999999999", ""):
            current = headers()
            current["content-length"] = length
            with self.assertRaises(BridgeError):
                validate_headers("POST", current, HOST, ORIGIN, TOKEN)
        for content_type in ("text/plain", "application/json; charset=utf-8"):
            current = headers()
            current["content-type"] = content_type
            with self.assertRaises(BridgeError):
                validate_headers("POST", current, HOST, ORIGIN, TOKEN)

    def test_preflight_is_no_token_exact_allowlist_and_zero_body(self):
        current = {"host": HOST, "origin": ORIGIN, "access-control-request-method": "POST",
                   "access-control-request-headers": "x-fe2o3-bridge-token, content-type"}
        self.assertEqual(validate_headers("OPTIONS", current, HOST, ORIGIN, TOKEN), 0)
        for change in ({"access-control-request-method": "GET"},
                       {"access-control-request-headers": "content-type"},
                       {"content-length": "1"}, {"x-fe2o3-bridge-token": TOKEN}):
            with self.assertRaises(BridgeError):
                validate_headers("OPTIONS", {**current, **change}, HOST, ORIGIN, TOKEN)

    def test_body_duplicate_number_nested_null_surrogate_and_route_refusals(self):
        valid = json.dumps(connect_request()).encode()
        self.assertEqual(decode_body(valid, "connect"), connect_request())
        for raw in (b'{"action":"connect","action":"connect"}',
                    b'{"action":"connect","sequence":9007199254740993}',
                    b'{"action":"connect","x":true}', b'{"action":"connect","x":null}',
                    b'{"action":"connect","x":{}}', b'{"action":"connect","x":"\\ud800"}',
                    b'{"action":"command"}', b"x" * 4097):
            with self.assertRaises(BridgeError):
                decode_body(raw, "connect")

    def test_loopback_origin_only_and_no_cors_credentials(self):
        self.assertEqual(bridge_http.origin(ORIGIN), ORIGIN)
        for value in ("http://localhost:5173", "https://127.0.0.1:5173", "http://127.0.0.1:0",
                      "http://127.0.0.1:5173/", "http://127.0.0.1:65536", "http://127.0.0.1:05173"):
            with self.assertRaises(ValueError):
                bridge_http.origin(value)
        wire = response_bytes(None, ORIGIN, preflight=True)
        self.assertIn(b"Vary: Origin", wire)
        self.assertNotIn(b"Allow-Credentials", wire)
        self.assertNotIn(TOKEN.encode(), wire)

    def test_bad_auth_and_unauthenticated_socket_reset_do_not_poison(self):
        owner, child, _pins, _now, _reply = fixture()
        token = Mock(value=TOKEN)
        for failure in (BridgeError("authentication_failed"), ConnectionResetError("synthetic")):
            with patch("fe2o3_debug_bridge.read_request", side_effect=failure), \
                    patch("fe2o3_debug_bridge.write_response"):
                self.assertTrue(fe2o3_debug_bridge.serve_peer(Mock(), owner, token, HOST, ORIGIN))
            self.assertFalse(owner.closed)
            self.assertEqual(child.closes, 0)
            self.assertEqual(len(child.requests), 1)

    def test_failed_response_write_after_dispatch_poison_without_retry(self):
        owner, child, _pins, _now, _reply = fixture()
        token = Mock(value=TOKEN)
        with patch("fe2o3_debug_bridge.read_request", return_value=command(owner, "step")), \
                patch("fe2o3_debug_bridge.write_response", side_effect=BrokenPipeError):
            fe2o3_debug_bridge.serve_peer(Mock(), owner, token, HOST, ORIGIN)
        self.assertTrue(owner.closed)
        self.assertEqual(len(child.requests), 2)

    def test_changed_token_stops_service_and_reaps(self):
        owner, child, _pins, _now, _reply = fixture()
        token = Mock(value=TOKEN)
        token.check.side_effect = CustodyError("synthetic")
        with patch("fe2o3_debug_bridge.read_request") as reader:
            self.assertFalse(fe2o3_debug_bridge.serve_peer(Mock(), owner, token, HOST, ORIGIN))
            reader.assert_not_called()
        self.assertTrue(owner.closed)
        self.assertTrue(owner.custody_failed)
        self.assertEqual(child.closes, 1)


class CustodyControls(unittest.TestCase):
    @staticmethod
    def info(size=3, mode=stat.S_IFREG | 0o600, **changes):
        fields = dict(st_dev=1, st_ino=2, st_mode=mode, st_uid=7, st_gid=7,
                      st_nlink=1, st_size=size, st_mtime_ns=10, st_ctime_ns=11)
        fields.update(changes)
        return SimpleNamespace(**fields)

    def test_fixed_metadata_includes_replacement_size_time_and_link_count(self):
        baseline = self.info()
        for field in ("st_ino", "st_size", "st_mtime_ns", "st_ctime_ns", "st_nlink"):
            changed = self.info(**{field: getattr(baseline, field) + 1})
            self.assertNotEqual(metadata(baseline), metadata(changed))

    def test_exact_file_full_hash_and_same_size_changed_bytes_refusal(self):
        info = self.info()
        pin = FilePin.__new__(FilePin)
        pin.fd, pin.path, pin.size = 9, "/owned/input", 3
        pin.initial, pin.sha256, pin.budget = metadata(info), hashlib.sha256(b"abc").hexdigest(), HashBudget()
        with patch("bridge_inputs.Path.resolve", return_value=Path(pin.path)), \
                patch("bridge_inputs.os.fstat", return_value=info), \
                patch("bridge_inputs.os.stat", return_value=info), \
                patch("bridge_inputs.os.pread", side_effect=[b"abc", b""]):
            pin.check(full=True)
        with patch("bridge_inputs.Path.resolve", return_value=Path(pin.path)), \
                patch("bridge_inputs.os.fstat", return_value=info), \
                patch("bridge_inputs.os.stat", return_value=info), \
                patch("bridge_inputs.os.pread", side_effect=[b"abd", b""]):
            with self.assertRaises(CustodyError):
                pin.check(full=True)

    def test_hash_work_cap_is_charged_before_reads(self):
        budget = HashBudget()
        budget.total = bridge_inputs.HASH_WORK_CAP
        with self.assertRaises(CustodyError):
            budget.charge(1)
        self.assertEqual(budget.total, bridge_inputs.HASH_WORK_CAP)

    def test_token_owner_permissions_link_count_and_zero_secret_refuse(self):
        for info, raw in ((self.info(64, st_uid=8), TOKEN.encode()),
                          (self.info(64, mode=stat.S_IFREG | 0o644), TOKEN.encode()),
                          (self.info(64, st_nlink=2), TOKEN.encode()),
                          (self.info(64), b"0" * 64)):
            with patch("bridge_inputs.Path.resolve", return_value=Path("/owned/token")), \
                    patch("bridge_inputs.os.open", return_value=9), \
                    patch("bridge_inputs.os.fstat", return_value=info), \
                    patch("bridge_inputs.os.stat", return_value=info), \
                    patch("bridge_inputs.os.pread", return_value=raw), \
                    patch("bridge_inputs.os.getuid", return_value=7), \
                    patch("bridge_inputs.os.close") as close:
                with self.assertRaises(CustodyError):
                    TokenFile("/owned/token")
                close.assert_called_once_with(9)


if __name__ == "__main__":
    unittest.main()
