"""Explicit V20 profile over unchanged inner CLI V1; never selected by capabilities."""
import copy
import re
from debug_console_commands import CommandError, number
from debug_console_protocol import MAX_COMMANDS, ProtocolError, encode, keys, require
from bridge_physical_v20_values import (RECORDS, RESPONSE_BYTES, anchor, bounded, capabilities,
    memory, refusal, same, scope_for, session, snapshot, stop, values)


class PhysicalQuerySessionV20:
    def __init__(self):
        self.sent = 0
        self.pending = None
        self.view = None
        self.end = None
        self.highest = 0
        self.clear_selection()

    def clear_selection(self):
        self.selected = None
        self.next_page = None
        self.page_limit = None

    def parse_command(self, line):
        try:
            require(type(line) is str and len(line) <= 128 and
                    re.fullmatch(r"[a-z0-9]+(?: [a-z0-9]+)*", line), "closed physical command")
            require(self.pending is None and self.view is not None, "physical command state")
            words = line.split(" ")
            if words == ["state"]:
                return {"operation": "get_state"}
            if len(words) == 2 and words[0] in ("step", "reverse"):
                count = number(words[1], 1, 64)
                return {"operation": "step", "direction": "forward" if words[0] == "step" else "reverse",
                        "granularity": "event", "count": count}
            if len(words) == 2 and words[0] == "seek":
                sequence = number(words[1], 0, RECORDS + 1)
                return {"operation": "seek", "cursor": {
                    **copy.deepcopy(self.view["cursor"]), "event_sequence": sequence}}
            require(self.selected is not None, "query needs actual captured checkpoint")
            if len(words) == 2 and words[0] == "values":
                if words[1] == "next":
                    require(self.next_page is not None and self.page_limit is not None,
                            "no current continuation page")
                    page = {"limit": self.page_limit, "cursor": copy.deepcopy(self.next_page)}
                else:
                    page = {"limit": number(words[1], 1, 64)}
                return {"operation": "inspect_values", "scope": scope_for(self.selected),
                        "frame": 1, "selector": {"selector": "all"}, "page": page}
            if len(words) == 3 and words[0] == "memory":
                offset, length = number(words[1]), number(words[2], 1, 256)
                require(offset + length <= (1 << 64) - 1, "physical memory range overflow")
                return {"operation": "read_memory", "allocation": {"ordinal": 1, "generation": 0},
                        "byte_offset": offset, "byte_len": length}
            raise CommandError("unsupported physical command")
        except ProtocolError:
            raise CommandError("physical query requires a current supported checkpoint") from None

    def begin(self, body):
        require(self.pending is None and self.sent < MAX_COMMANDS - 1, "shared command budget")
        require(type(body) is dict, "physical command body")
        operation = body.get("operation")
        if self.view is None:
            same(body, {"operation": "discover_capabilities"})
        elif operation == "get_state":
            same(body, {"operation": "get_state"})
        elif operation == "step":
            keys(body, ("operation", "direction", "granularity", "count"))
            require(body["direction"] in ("forward", "reverse"), "event direction")
            same(body["granularity"], "event")
            require(0 < bounded(body["count"], 64), "event count")
        elif operation == "seek":
            keys(body, ("operation", "cursor"))
            keys(body["cursor"], ("configuration_identity", "event_sequence", "state_revision"))
            expected = copy.deepcopy(self.view["cursor"])
            expected["event_sequence"] = bounded(body["cursor"]["event_sequence"], RECORDS + 1)
            same(body["cursor"], expected)
        elif operation in ("inspect_values", "read_memory"):
            require(self.selected is not None, "current captured physical anchor")
            if operation == "inspect_values":
                page = body.get("page")
                require(type(page) is dict, "physical page")
                limit = bounded(page.get("limit"), 64)
                require(limit > 0, "positive page")
                if "cursor" in page:
                    require(self.next_page is not None, "continuation requires previous response")
                    same(body, self.parse_command("values next"))
                else:
                    same(body, self.parse_command("values " + str(limit)))
            else:
                same(body, self.parse_command("memory " + str(body.get("byte_offset")) +
                                               " " + str(body.get("byte_len"))))
        else:
            raise ProtocolError("closed physical operation")
        request = {"schema": "fe2o3-debug-request-v1", "request_id": self.sent + 1,
                   "expected_revision": self.view["revision"] if self.view else 0, **copy.deepcopy(body)}
        require(len(encode(request)) + 1 <= 4096, "shared request cap")
        self.sent += 1
        self.pending = request
        self.next_page = None
        self.page_limit = None
        if operation in ("step", "seek", "get_state"):
            self.selected = None
        return copy.deepcopy(request)

    def _control(self, result, request, view):
        keys(result, ("result", "stop", "events_advanced", "snapshot"))
        same(result["result"], "control")
        old = self.view["cursor"]["event_sequence"]
        new = view["cursor"]["event_sequence"]
        same(view["revision"], self.view["revision"] + 1)
        end = self.end
        completed = result["stop"] == {"reason": "completed", "outcome": "completed", "exact": True}
        if completed:
            if end is None:
                require(new > self.highest and new > 0, "end follows observed records")
                end = new
            same(new, end)
        stop(result["stop"], new, end)
        if request["operation"] == "seek":
            same(new, request["cursor"]["event_sequence"])
        elif request["direction"] == "reverse":
            same(new, max(0, old - request["count"]))
        elif end is not None:
            same(new, min(old + request["count"], end))
        else:
            same(new, old + request["count"])
        total = end - 1 if end is not None else RECORDS
        same(result["events_advanced"], abs(min(new, total) - min(old, total)))
        selected = snapshot(result["snapshot"], view, end)
        if new == 0 or new == end:
            require(selected is None, "sentinel is not a captured event")
        self.end = end
        self.highest = max(self.highest, new if new != end else 0)
        self.selected = copy.deepcopy(selected)

    def accept(self, response):
        try:
            require(self.pending is not None and len(encode(response)) + 1 <= RESPONSE_BYTES,
                    "physical pending response and byte bound")
            request = self.pending
            require(type(response) is dict and response.get("status") in ("ok", "error", "unavailable"),
                    "physical response status")
            status = response["status"]
            keys(response, ("schema", "request_id", "operation", "session",
                            "status", "result" if status == "ok" else status))
            same(response["schema"], "fe2o3-debug-response-v1")
            same(response["request_id"], request["request_id"])
            same(response["operation"], request["operation"])
            current = response["session"]
            sequence = session(current)
            if self.view is None:
                require(status == "ok", "initial typed physical discovery must succeed")
                same(request["operation"], "discover_capabilities")
                same(current["revision"], 0)
                same(sequence, 0)
                capabilities(response["result"])
            else:
                same(current["configuration_identity"], self.view["configuration_identity"])
                if status != "ok":
                    same(current, self.view)
                    refusal(response, request["operation"])
                    self.clear_selection()
                elif request["operation"] in ("step", "seek"):
                    self._control(response["result"], request, current)
                else:
                    same(current, self.view)
                    result = response["result"]
                    if request["operation"] == "get_state":
                        keys(result, ("result", "snapshot"))
                        same(result["result"], "state")
                        self.selected = copy.deepcopy(snapshot(result["snapshot"], current, self.end))
                    elif request["operation"] == "inspect_values":
                        require(self.selected is not None, "same current physical capture")
                        self.next_page = copy.deepcopy(values(result, request, current, self.selected))
                        self.page_limit = request["page"]["limit"]
                    elif request["operation"] == "read_memory":
                        require(self.selected is not None, "same current physical capture")
                        memory(result, request, current, self.selected)
                    else:
                        raise ProtocolError("unsupported physical successful result")
            self.view = copy.deepcopy(current)
            self.pending = None
            return response
        except Exception:
            self.clear_selection()
            raise
