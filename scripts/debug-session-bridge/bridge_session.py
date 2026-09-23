"""Closed browser adapter over the unchanged CPU console protocol owner."""
import copy
import os
from pathlib import Path
import re
import secrets
import selectors
import subprocess
import sys
import time

# Repository-local, unchanged command/protocol owner. No second KIR/schema decoder.
CONSOLE = Path(__file__).resolve().parent.parent / "debug-console"
sys.path.insert(0, str(CONSOLE))
from debug_console_commands import CommandError, number
from debug_console_process import cleanup
from debug_console_protocol import LineFramer, ProtocolError, encode

from bridge_inputs import CustodyError
from bridge_live_queries import LiveQuerySession
from bridge_observed_queries import ObservedQuerySession
from bridge_target_values import TARGET_BYTES, TARGET_REQUEST

REQUEST_SCHEMA = "fe2o3-cpu-debug-bridge-request-v1"
RESPONSE_SCHEMA = "fe2o3-cpu-debug-bridge-response-v1"
REQUEST_SECONDS = 30
SESSION_SECONDS = 900
MAX_CONNECTIONS = 4
MAX_INNER_BYTES = 256 * 1024
ERROR_CODES = frozenset(("invalid_request", "authentication_failed", "session_exists",
    "session_unavailable", "stale_session", "stale_sequence", "stale_revision",
    "command_refused", "resource_limit", "backend_failed", "cleanup_failed"))


class BridgeError(Exception):
    def __init__(self, code, outcome="not_sent", closed=False):
        if code not in ERROR_CODES or outcome not in ("not_sent", "unknown"):
            raise ValueError("closed error enum required")
        self.code, self.outcome, self.closed = code, outcome, closed
        super().__init__(code)

    def envelope(self):
        return {"schema": RESPONSE_SCHEMA, "status": "error", "code": self.code,
                "outcome": self.outcome, "closed": self.closed}


def exact_keys(value, names):
    if type(value) is not dict or set(value) != set(names):
        raise BridgeError("invalid_request")


def handle(value):
    if type(value) is not str or not re.fullmatch(r"[0-9a-f]{64}", value) or value == "0" * 64:
        raise BridgeError("invalid_request")
    return value


def decimal(value):
    if type(value) is not str:
        raise BridgeError("invalid_request")
    try:
        return number(value)
    except CommandError:
        raise BridgeError("invalid_request") from None


def projected_session(view):
    result = copy.deepcopy(view)
    result["revision"] = str(view["revision"])
    result["cursor"]["event_sequence"] = str(view["cursor"]["event_sequence"])
    result["cursor"]["state_revision"] = str(view["cursor"]["state_revision"])
    return result


class CPUProcess:
    """One owned process; no shell, no browser argv/env, one request at a time."""
    def __init__(self):
        self.child = None
        self.framer = LineFramer()
        self.stderr_bytes = 0

    def start(self, argv):
        # The CLI launcher/environment is trusted; this does not attest dependency closure.
        env = dict(os.environ)
        env.update(HIP_VISIBLE_DEVICES="", ROCR_VISIBLE_DEVICES="", CUDA_VISIBLE_DEVICES="")
        self.child = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, bufsize=0, close_fds=True, start_new_session=True, env=env)
        for pipe in (self.child.stdin, self.child.stdout, self.child.stderr):
            os.set_blocking(pipe.fileno(), False)

    def exchange(self, request, deadline, peer=None):
        if self.child is None or self.child.poll() is not None:
            raise ProtocolError("backend not live")
        outbound = bytearray(encode(request) + b"\n")
        replies = []
        target_bytes = 0
        is_target = request.get("schema") == TARGET_REQUEST
        selector = selectors.DefaultSelector()
        try:
            selector.register(self.child.stdout, selectors.EVENT_READ, "stdout")
            selector.register(self.child.stderr, selectors.EVENT_READ, "stderr")
            if peer is not None:
                selector.register(peer, selectors.EVENT_READ, "peer")
            # Any unsolicited prior stdout is a correlation error, never a new response.
            for key, _events in selector.select(0):
                if key.data in ("stdout", "peer"):
                    raise ProtocolError("unsolicited output or closed caller")
            selector.register(self.child.stdin, selectors.EVENT_WRITE, "request")

            def receive(response):
                if outbound or replies:
                    raise ProtocolError("one fully written request and one reply required")
                replies.append(response)

            while not replies:
                if time.monotonic() >= deadline:
                    raise ProtocolError("backend deadline; outcome unknown")
                if self.child.poll() is not None:
                    raise ProtocolError("unexpected backend exit")
                for key, _events in selector.select(min(0.1, max(0, deadline - time.monotonic()))):
                    if key.data == "peer":
                        # A half-close/abort or unsolicited pipelined bytes is not a new command.
                        raise ProtocolError("caller transport lost; outcome unknown")
                    if key.data == "request":
                        try:
                            count = os.write(key.fd, outbound)
                        except BlockingIOError:
                            continue
                        if count <= 0:
                            raise ProtocolError("backend stdin closed")
                        del outbound[:count]
                        if not outbound:
                            selector.unregister(self.child.stdin)
                        continue
                    try:
                        chunk = os.read(key.fd, 4096)
                    except BlockingIOError:
                        continue
                    if not chunk:
                        raise ProtocolError("unexpected backend pipe EOF")
                    if key.data == "stderr":
                        self.stderr_bytes += len(chunk)
                        if self.stderr_bytes > 65536:
                            raise ProtocolError("backend stderr byte cap")
                    else:
                        if is_target:
                            target_bytes += len(chunk)
                            if target_bytes > TARGET_BYTES:
                                raise ProtocolError("target response byte cap")
                        self.framer.feed(chunk, receive)
            if self.framer.partial:
                raise ProtocolError("trailing incomplete unsolicited response")
            return replies[0]
        finally:
            selector.close()

    def close(self):
        if self.child is None:
            return True
        reaped = cleanup(self.child)
        if reaped:
            for pipe in (self.child.stdin, self.child.stdout, self.child.stderr):
                if pipe is not None and not pipe.closed:
                    try:
                        pipe.close()
                    except OSError:
                        pass
        return reaped


class BridgeSession:
    """V1 correlation plus separately versioned, bridge-local checkpoint queries."""
    def __init__(self, argv, inputs, process_factory=CPUProcess, clock=time.monotonic,
                 runtime_observations=None):
        if runtime_observations is not None and runtime_observations != "v1":
            raise ValueError("closed owner-selected runtime observation profile")
        self.runtime_observations = runtime_observations
        self.argv = tuple(argv)
        self.inputs = inputs
        self.process_factory = process_factory
        self.clock = clock
        self.process = None
        self.protocol = None
        self.connection_id = None
        self.bridge_session = None
        self.started = None
        self.used_connections = set()
        self.poisoned = False
        self.last_dispatched = False
        self.custody_failed = False

    @property
    def closed(self):
        return self.process is None

    def close(self):
        """Only closed=True permits reconnect; failed reaping retains the owned handle."""
        self.poisoned = True
        if self.protocol is not None:
            if self.runtime_observations == "v1":
                self.protocol.close_observation_connection()
            else:
                self.protocol.clear_selection()
        if self.process is None:
            return True
        if not self.process.close():
            return False
        self.process = None
        try:
            self.inputs.check(full=True)
        except (CustodyError, OSError):
            self.custody_failed = True
        return True

    def expire(self):
        if self.process is not None and self.started is not None and (
                self.clock() - self.started >= SESSION_SECONDS):
            self.close()

    def poison(self):
        return self.close()

    def error(self, code, outcome="not_sent"):
        return BridgeError(code, outcome, self.closed)

    def handle_request(self, request, peer=None):
        self.last_dispatched = False
        self.expire()
        if type(request) is not dict or request.get("schema") != REQUEST_SCHEMA:
            raise self.error("invalid_request")
        action = request.get("action")
        if action == "connect":
            exact_keys(request, ("schema", "action", "connection_id"))
            return self.connect(handle(request["connection_id"]), peer)
        if action == "disconnect":
            exact_keys(request, ("schema", "action", "connection_id"))
            connection_id = handle(request["connection_id"])
            if connection_id != self.connection_id:
                raise self.error("stale_session")
            if not self.close():
                raise self.error("cleanup_failed")
            return {"schema": RESPONSE_SCHEMA, "status": "disconnected",
                    "connection_id": connection_id, "closed": True}
        if action != "command":
            raise self.error("invalid_request")
        exact_keys(request, ("schema", "action", "bridge_session", "sequence",
                             "expected_revision", "command"))
        bridge_id = handle(request["bridge_session"])
        sequence = decimal(request["sequence"])
        revision = decimal(request["expected_revision"])
        if self.process is None or self.poisoned:
            raise self.error("session_unavailable")
        if bridge_id != self.bridge_session:
            raise self.error("stale_session")
        if sequence != self.protocol.sent:
            raise self.error("stale_sequence")
        if revision != self.protocol.view["revision"]:
            raise self.error("stale_revision")
        try:
            body = self.protocol.parse_command(request["command"])
        except (CommandError, TypeError):
            raise self.error("command_refused") from None
        if not isinstance(body, dict) or body.get("operation") == "terminate":
            raise self.error("command_refused")
        if self.protocol.sent >= 255:
            self.close()
            raise self.error("resource_limit")
        return self.exchange(body, peer)

    def connect(self, connection_id, peer):
        if self.process is not None:
            raise self.error("session_exists")
        if self.custody_failed:
            raise self.error("backend_failed")
        if connection_id in self.used_connections:
            raise self.error("stale_session")
        if len(self.used_connections) >= MAX_CONNECTIONS:
            raise self.error("resource_limit")
        try:
            self.inputs.check(full=True)
        except (CustodyError, OSError):
            self.custody_failed = True
            raise self.error("backend_failed") from None
        self.connection_id = connection_id
        self.bridge_session = secrets.token_hex(32)
        self.used_connections.add(connection_id)
        self.protocol = ObservedQuerySession() if self.runtime_observations == "v1" else LiveQuerySession()
        self.started = self.clock()
        self.poisoned = False
        self.process = self.process_factory()
        self.last_dispatched = True
        try:
            self.process.start(list(self.argv))
        except Exception:
            self.close()
            raise self.error("backend_failed", "unknown") from None
        return self.exchange({"operation": "discover_capabilities"}, peer)

    def exchange(self, body, peer):
        try:
            self.inputs.check()
            request = self.protocol.begin(body)
            self.last_dispatched = True
            deadline = min(self.started + SESSION_SECONDS, self.clock() + REQUEST_SECONDS)
            response = self.process.exchange(request, deadline, peer)
            self.inputs.check()
            self.protocol.accept(response)
            encoded = encode(response)
            if len(encoded) > MAX_INNER_BYTES:
                raise ProtocolError("re-encoded response byte cap")
            result = {"schema": RESPONSE_SCHEMA, "status": "ok",
                    "connection_id": self.connection_id, "bridge_session": self.bridge_session,
                    "sequence": str(self.protocol.sent - 1),
                    "session": projected_session(self.protocol.view),
                    "response_json": encoded.decode("ascii"), "closed": False}
            if request.get("schema") == TARGET_REQUEST and (
                    len(encoded) + 1 > TARGET_BYTES or len(encode(result)) > TARGET_BYTES):
                raise ProtocolError("target inner or outer response byte cap")
            return result
        except Exception:
            self.close()
            raise self.error("backend_failed", "unknown" if self.last_dispatched else "not_sent") from None
