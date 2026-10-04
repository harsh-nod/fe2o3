"""Bridge-local first-page query adapter sharing the unchanged V1 Session ledger.

Only actual validated stopped operation-step responses select an unframed anchor.
No caller-provided anchor, raw JSON, pagination token or dynamic activation ID.
"""
import copy
import re

from debug_console_commands import CommandError, number, parse_command
from debug_console_protocol import (MAX_COMMANDS, MUTATING, ProtocolError, Session, encode,
                                    exact, keys, require, session_shape)
from bridge_live_query_values import (RESOURCE_REQUEST, RESOURCE_RESPONSE, VARIABLE_REQUEST,
    VARIABLE_RESPONSE, VALUE_UNAVAILABLE, allocation, anchor, bounded, enum,
    resource_response, variable_response)

SCHEMAS = {RESOURCE_REQUEST: RESOURCE_RESPONSE, VARIABLE_REQUEST: VARIABLE_RESPONSE}


class LiveQuerySession(Session):
    def __init__(self):
        super().__init__()
        self.clear_selection()

    def clear_selection(self):
        self._anchor = None
        self._control_request = None
        self._control_response = None
        self._frame = None
        self._inventory = {}

    @property
    def selected_control(self):
        """Independent copies for local diagnostics, never request/admission authority."""
        if self._anchor is None:
            return None
        return {"request": copy.deepcopy(self._control_request),
                "response": copy.deepcopy(self._control_response)}

    def _selected(self):
        require(self.pending is None and self._anchor is not None and self.view is not None and
                self.view["state"] == "stopped" and
                exact(self.view, self._control_response["session"]) and
                exact(self._anchor["cursor"], self.view["cursor"]), "current selected operation checkpoint")
        return self._anchor

    def _body(self, command, ordinal=None):
        selected = self._selected()
        if command == "allocations":
            return {"schema": RESOURCE_REQUEST, "operation": "query_allocations",
                    "expected_snapshot": copy.deepcopy(selected), "address_space": "global",
                    "page": {"max_items": 16, "max_scanned": 64}}
        if command == "accesses":
            require((ordinal, 0) in self._inventory, "allocation from current inventory")
            return {"schema": RESOURCE_REQUEST, "operation": "query_memory_accesses",
                    "expected_snapshot": copy.deepcopy(selected),
                    "filter": {"scope": {"level": "dispatch"},
                               "allocation": {"ordinal": ordinal, "generation": 0},
                               "address_space": "global"},
                    "page": {"max_items": 16, "max_scanned": 64}}
        require(command == "variables" and self._frame is not None, "current complete single-frame stack")
        return {"schema": VARIABLE_REQUEST, "operation": "inspect_source_variables",
                "scope": {"level": "dispatch"}, "frame": 1, "selector": {"selector": "all"},
                "page": {"limit": 16}}

    def parse_command(self, line):
        # Legacy commands retain their exact original parser and behavior.
        if not isinstance(line, str):
            return parse_command(line)
        words = line.split()
        if not words or words[0] not in ("allocations", "accesses", "variables"):
            return parse_command(line)
        try:
            require(self.pending is None, "one query pending")
            if line == "allocations":
                return self._body("allocations")
            if line == "variables 1":
                return self._body("variables")
            match = re.fullmatch(r"accesses (0|[1-9][0-9]{0,19}) 0", line)
            require(match is not None, "closed live query command")
            return self._body("accesses", number(match.group(1), 1))
        except ProtocolError:
            raise CommandError("live query requires the current supported checkpoint inputs") from None

    def begin(self, body):
        if type(body) is not dict or body.get("schema") not in SCHEMAS:
            request = super().begin(body)
            operation = request["operation"]
            if operation in MUTATING or operation in ("step", "continue"):
                self.clear_selection()
            elif operation == "inspect_stack":
                self._frame = None
            return request
        require(self.pending is None and self.sent < MAX_COMMANDS-1, "shared query command budget")
        operation = body.get("operation")
        if operation == "query_allocations":
            expected = self._body("allocations")
        elif operation == "query_memory_accesses":
            filtered = body.get("filter")
            require(type(filtered) is dict and type(filtered.get("allocation")) is dict, "query allocation selector")
            ordinal, _generation = allocation(filtered["allocation"])
            expected = self._body("accesses", ordinal)
        else:
            require(operation == "inspect_source_variables", "supported live query")
            expected = self._body("variables")
        require(exact(body, expected), "caller query fields or anchor refused")
        request = {"request_id": self.sent+1, "expected_revision": self.view["revision"], **copy.deepcopy(body)}
        require(len(encode(request))+1 <= 4096, "shared request byte cap")
        if operation == "query_allocations":
            self._inventory = {}
        self.sent += 1
        self.pending = request
        return copy.deepcopy(request)

    def _retain_step(self, request, response):
        if (request["operation"] != "step" or request.get("granularity") != "operation" or
                response["status"] != "ok" or self.view["state"] != "stopped"):
            return
        result = response["result"]
        if result.get("stop") != {"reason": "step", "outcome": "active", "exact": True}:
            return
        available = result.get("snapshot")
        if type(available) is not dict or available.get("status") != "captured":
            return
        # Valid but unsupported V1 snapshots remain V1 responses; they do not
        # create query authority. The original Session is deliberately unchanged.
        try:
            keys(result, ("result", "stop", "events_advanced", "snapshot"))
            require(result["result"] == "control" and type(result["stop"]["exact"]) is bool,
                    "active exact operation step")
            keys(available, ("status", "snapshot"))
            snapshot = available["snapshot"]
            keys(snapshot, ("anchor", "stop", "values"))
            require(exact(snapshot["stop"], result["stop"]) and type(snapshot["values"]) is list,
                    "same captured stop")
            anchor(snapshot["anchor"], self.view)
        except (ProtocolError, KeyError, TypeError):
            return
        self._anchor = copy.deepcopy(snapshot["anchor"])
        self._control_request = copy.deepcopy(request)
        self._control_response = copy.deepcopy(response)

    def _retain_stack(self, request, response):
        if request["operation"] != "inspect_stack" or self._anchor is None or response["status"] != "ok":
            return
        result = response["result"]
        try:
            require(exact(request["scope"], {"level": "dispatch"}) and exact(request["page"], {"limit": 16}),
                    "closed stack request")
            keys(result, ("result", "snapshot", "frames"))
            require(result["result"] == "stack" and exact(result["snapshot"], self._anchor) and
                    type(result["frames"]) is list and len(result["frames"]) == 1, "complete one-frame stack")
            frame = result["frames"][0]
            keys(frame, ("frame", "function_ordinal", "block_ordinal", "next_operation", "values"))
            require(type(frame["frame"]) is int and frame["frame"] == 1, "legacy top frame")
            site = self._anchor["site"]["kir"]
            for field in ("function_ordinal", "block_ordinal"):
                bounded(frame[field])
                require(frame[field] == site[field], "stack selected site")
            # After-operation next_operation need not equal the recorded site.
            bounded(frame["next_operation"])
            values = frame["values"]
            require(type(values) is dict, "stack values availability")
            if values.get("status") == "captured":
                keys(values, ("status", "value_count"))
                bounded(values["value_count"])
            else:
                keys(values, ("status", "reason"))
                require(values["status"] == "unavailable", "stack availability status")
                enum(values["reason"], VALUE_UNAVAILABLE)
        except (ProtocolError, KeyError, TypeError):
            return
        self._frame = copy.deepcopy(frame)

    def accept(self, response):
        try:
            require(self.pending is not None, "unsolicited query response")
            request = copy.deepcopy(self.pending)
            if request["schema"] not in SCHEMAS:
                result = super().accept(response)
                self._retain_step(request, response)
                self._retain_stack(request, response)
                return result
            require(type(response) is dict and response.get("schema") == SCHEMAS[request["schema"]] and
                    type(response.get("request_id")) is int and response["request_id"] == request["request_id"] and
                    response.get("operation") == request["operation"] and
                    response.get("status") in ("ok", "unavailable", "error"), "live query response correlation")
            session_shape(response.get("session"))
            require(exact(response["session"], self.view), "read-only query changed full session")
            require(self._anchor is not None and exact(self._anchor["cursor"], self.view["cursor"]),
                    "live query lost selected anchor")
            if request["schema"] == RESOURCE_REQUEST:
                rows = resource_response(response, request, self._anchor, self._inventory)
                if request["operation"] == "query_allocations" and rows is not None:
                    self._inventory = {allocation(row["allocation"]): copy.deepcopy(row) for row in rows}
            else:
                require(self._frame is not None, "source query lost current frame")
                variable_response(response, self._anchor, self._frame)
            # Refusals consume exactly the one dispatched ID; no schema rewriting.
            self.view = copy.deepcopy(response["session"])
            self.pending = None
            return response
        except Exception:
            self.clear_selection()
            raise
