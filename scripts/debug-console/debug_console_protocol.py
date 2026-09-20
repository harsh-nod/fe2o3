"""Bounded lossless framing/correlation. This is not the full Rust wire validator."""
import copy
import json
import re

LINE_BYTES = 256 * 1024
MAX_COMMANDS = 256
U64 = (1 << 64) - 1
READ_ONLY = {"discover_capabilities", "get_state", "list_breakpoints",
             "list_watchpoints", "resolve_source", "inspect_stack", "read_memory"}
MUTATING = {"set_breakpoints", "remove_breakpoints", "set_watchpoints",
            "remove_watchpoints", "terminate"}
RESULTS = {"discover_capabilities": "capabilities", "get_state": "state",
           "list_breakpoints": "breakpoints", "list_watchpoints": "watchpoints",
           "resolve_source": "source", "inspect_stack": "stack", "read_memory": "memory",
           "step": "control", "continue": "control", "terminate": "terminated",
           "set_breakpoints": "acknowledged", "remove_breakpoints": "acknowledged",
           "set_watchpoints": "acknowledged", "remove_watchpoints": "acknowledged"}

class ProtocolError(ValueError):
    pass

def require(condition, message):
    if not condition:
        raise ProtocolError(message)

def uint(value):
    require(type(value) is int and 0 <= value <= U64, "exact u64 required")
    return value

def keys(value, expected):
    require(type(value) is dict and set(value) == set(expected), "closed object shape required")

def exact(left, right):
    """JSON equality without Python bool/int aliasing; inputs are wire-bounded."""
    if type(left) is not type(right):
        return False
    if type(left) is dict:
        return left.keys() == right.keys() and all(exact(left[k], right[k]) for k in left)
    if type(left) is list:
        return len(left) == len(right) and all(exact(a, b) for a, b in zip(left, right))
    return left == right

def identity(value):
    require(type(value) is str and re.fullmatch(r"[0-9a-f]{64}", value)
            and value != "0" * 64, "nonzero opaque identity required")

def _pairs(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, "duplicate JSON key")
        result[key] = value
    return result

def _not_integer(_text):
    raise ProtocolError("wire numbers must be unsigned integers")

def decode_line(raw):
    require(type(raw) is bytes and 0 < len(raw) <= LINE_BYTES, "response line cap")
    require(b"\r" not in raw and b"\n" not in raw, "embedded framing byte")
    try:
        result = json.loads(raw.decode("utf-8", "strict"), object_pairs_hook=_pairs,
                            parse_float=_not_integer, parse_constant=_not_integer)
    except (UnicodeError, ValueError, RecursionError) as error:
        raise ProtocolError("invalid bounded JSON: " + str(error)[:160]) from error
    queue, nodes = [(result, 0)], 0
    while queue:
        value, depth = queue.pop()
        nodes += 1
        require(nodes <= 32768 and depth <= 32, "JSON graph cap")
        if type(value) is int:
            uint(value)
        elif type(value) in (dict, list):
            require(len(value) <= 4096, "JSON collection cap")
            children = value.values() if type(value) is dict else value
            queue.extend((child, depth + 1) for child in children)
        else:
            require(type(value) in (str, bool), "null/unsupported JSON value")
    return result

def encode(value):
    return json.dumps(value, ensure_ascii=True, separators=(",", ":"), allow_nan=False).encode("ascii")

class LineFramer:
    """One owned partial line; no incoming backing views are retained."""
    def __init__(self):
        self.partial = bytearray()
        self.total = 0

    def feed(self, chunk, receive):
        require(type(chunk) is bytes and len(chunk) <= 4096, "read chunk cap")
        require(self.total + len(chunk) <= 8 * 1024 * 1024, "cumulative stdout cap")
        self.total += len(chunk)
        parts = chunk.split(b"\n")
        for index, part in enumerate(parts):
            require(b"\r" not in part, "CR framing refused")
            if index < len(parts) - 1:
                require(len(self.partial) + len(part) <= LINE_BYTES, "response line cap")
                self.partial.extend(part)
                require(self.partial, "empty response line")
                receive(decode_line(bytes(self.partial)))
                self.partial.clear()
            else:
                require(len(self.partial) + len(part) <= LINE_BYTES, "response line cap")
                self.partial.extend(part)

    def eof(self):
        require(not self.partial, "unterminated response line")

def session_shape(session):
    keys(session, ("backend", "execution_kind", "state", "revision", "configuration_identity",
                   "cursor", "simulated", "hardware_observed", "performance_prediction"))
    require(session["backend"] == "cpu_kir_simulator"
            and session["execution_kind"] == "cpu_kir_simulation"
            and session["simulated"] is True and session["hardware_observed"] is False
            and session["performance_prediction"] is False, "CPU observation classification")
    require(session["state"] in ("created", "running", "stopped", "terminated"), "session state")
    identity(session["configuration_identity"])
    uint(session["revision"])
    cursor = session["cursor"]
    keys(cursor, ("configuration_identity", "event_sequence", "state_revision"))
    uint(cursor["event_sequence"])
    uint(cursor["state_revision"])
    require(cursor["configuration_identity"] == session["configuration_identity"]
            and cursor["state_revision"] == session["revision"], "session cursor binding")

def result_bindings(request, response):
    result, session = response["result"], response["session"]
    require(type(result) is dict and result.get("result") == RESULTS[request["operation"]],
            "operation/result correlation")
    tag = result["result"]
    anchor = None
    if tag in ("state", "control"):
        snapshot = result.get("snapshot")
        require(type(snapshot) is dict and snapshot.get("status") in ("captured", "unavailable"), "snapshot availability")
        if snapshot["status"] == "captured":
            require(type(snapshot.get("snapshot")) is dict, "captured snapshot")
            anchor = snapshot["snapshot"].get("anchor")
            require(type(anchor) is dict, "captured snapshot anchor required")
    elif tag in ("stack", "memory"):
        anchor = result.get("snapshot")
    if anchor is not None:
        require(type(anchor) is dict and exact(anchor.get("cursor"), session["cursor"]), "same-stop snapshot cursor")
    elif tag in ("stack", "memory"):
        raise ProtocolError("snapshot anchor required")
    if tag == "acknowledged":
        require(uint(result.get("accepted")) == 1, "exact single-filter acknowledgement")
    if tag == "control":
        uint(result.get("events_advanced"))
    if tag == "source":
        require(type(result.get("site")) is dict and exact(result["site"].get("kir"), request["site"]), "source site substitution")
    if tag in ("breakpoints", "watchpoints", "stack"):
        field = "frames" if tag == "stack" else tag
        require(type(result.get(field)) is list and len(result[field]) <= request["page"]["limit"], "page bound")
    if tag == "memory":
        memory = result.get("memory")
        require(type(memory) is dict and exact(memory.get("allocation"), request["allocation"])
                and exact(memory.get("byte_offset"), request["byte_offset"])
                and exact(memory.get("requested_bytes"), request["byte_len"]), "memory request substitution")
        count = uint(memory.get("returned_bytes"))
        require(count <= request["byte_len"], "memory extent")
        availability = memory.get("availability")
        require(type(availability) is dict and availability.get("status") in ("captured", "unavailable", "redacted"), "memory availability")
        if availability["status"] == "captured":
            for field, size in (("bytes", count), ("initialized", (count + 7) // 8)):
                value = availability.get(field)
                require(type(value) is str and re.fullmatch(r"0x[0-9a-f]*", value)
                        and len(value) == 2 + 2 * size, "memory bytes/init extent")
            if count % 8:
                require(int(availability["initialized"][-2:], 16) >> (count % 8) == 0, "initialization padding bits")
        else:
            require(count == 0, "unavailable/redacted memory carries bytes")

class Session:
    def __init__(self):
        self.sent = 0
        self.pending = None
        self.view = None

    def begin(self, body):
        require(self.pending is None, "one request may be pending")
        require(type(body) is dict and body.get("operation") in RESULTS, "unsupported operation")
        require(not {"schema", "request_id", "expected_revision"}.intersection(body), "caller header refused")
        require(self.sent < MAX_COMMANDS and
                (self.sent < MAX_COMMANDS - 1 or body["operation"] == "terminate"), "command cap reserves quit")
        require(self.view is not None or body["operation"] == "discover_capabilities", "handshake first")
        require(self.view is None or self.view["state"] != "terminated", "session terminated")
        request = {"schema": "fe2o3-debug-request-v1", "request_id": self.sent + 1,
                   "expected_revision": self.view["revision"] if self.view else 0, **copy.deepcopy(body)}
        require(len(encode(request)) + 1 <= 4096, "request byte cap")
        self.sent += 1
        self.pending = request
        return copy.deepcopy(request)

    def accept(self, response):
        require(self.pending is not None, "unsolicited/duplicate response")
        request = self.pending
        require(type(response) is dict and response.get("status") in ("ok", "unavailable", "error"), "response status")
        status = response["status"]
        payload = {"ok": "result", "unavailable": "unavailable", "error": "error"}[status]
        keys(response, ("status", "schema", "request_id", "operation", "session", payload))
        require(response["schema"] == "fe2o3-debug-response-v1"
                and response["request_id"] == request["request_id"]
                and response["operation"] == request["operation"], "response correlation")
        uint(response["request_id"])
        current = response["session"]
        session_shape(current)
        if self.view is not None:
            require(current["configuration_identity"] == self.view["configuration_identity"], "configuration changed")
        expected = request["expected_revision"]
        if status != "ok":
            require(type(response[payload]) is dict and response[payload].get("state_changed") is False,
                    "error/unavailable changed state")
            require(self.view is not None and current == self.view, "refusal changed session")
        else:
            operation = request["operation"]
            if operation in READ_ONLY:
                require(current["revision"] == expected, "read-only revision changed")
                if self.view is not None:
                    require(current == self.view, "read-only session changed")
            elif operation in MUTATING:
                require(expected < U64 and current["revision"] == expected + 1, "mutation revision")
                if self.view is not None:
                    require(current["cursor"]["event_sequence"] == self.view["cursor"]["event_sequence"]
                            and (operation == "terminate" or current["state"] == self.view["state"]),
                            "filter or termination moved cursor/state")
            else:
                require(current["revision"] in (expected, expected + 1), "control revision jump")
                if self.view and current["revision"] == expected:
                    require(current == self.view, "state changed without revision")
                if self.view and current["cursor"]["event_sequence"] != self.view["cursor"]["event_sequence"]:
                    require(current["revision"] == expected + 1, "cursor changed without revision")
            result_bindings(request, response)
            if operation in ("step", "continue") and self.view is not None:
                movement = current["cursor"]["event_sequence"] - self.view["cursor"]["event_sequence"]
                reverse = operation == "step" and request["direction"] == "reverse"
                require(movement <= 0 if reverse else movement >= 0, "control moved against requested direction")
                delta = abs(movement)
                require(response["result"]["events_advanced"] == delta, "control event delta")
            if operation == "terminate":
                require(current["state"] == "terminated", "termination state")
        # Commit only after all checks. Protocol rejection is fatal to this client.
        self.view = copy.deepcopy(current)
        self.pending = None
        return response
