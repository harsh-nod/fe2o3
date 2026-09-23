"""Opt-in runtime/storage commands on the original bridge process and ID ledger.

Only the local launcher enables this adapter. Browser commands cannot supply an
owner, cursor, schema body, executable, profile, continuation token, or source map.
"""
import copy
import re

from debug_console_commands import CommandError
from debug_console_protocol import MAX_COMMANDS, ProtocolError, encode, exact, require
from bridge_live_queries import LiveQuerySession
from bridge_runtime_values import (RESOURCE_REQUEST, RUNTIME_REQUEST, positive, triple,
                                   validate_runtime)
from bridge_storage_values import memory_range, validate_resource

SCHEMAS = frozenset((RUNTIME_REQUEST, RESOURCE_REQUEST))
COMMANDS = frozenset(("runtime", "storage", "lifecycle", "storageaccess", "storagememory"))
DECIMAL = r"(0|[1-9][0-9]{0,19})"


class ObservedQuerySession(LiveQuerySession):
    def __init__(self):
        self._observation_owner = None
        self._owner_discovery_attempted = False
        self._observation_connection_open = True
        super().__init__()

    def clear_selection(self):
        # Overrides the existing hook: every control/filter mutation clears both
        # old and new derived selections before anything is dispatched.
        super().clear_selection()
        self._confirmed_view = None
        self._runtime_record = None
        self._runtime_binding = None
        self._storage_inventory = {}

    def close_observation_connection(self):
        self.clear_selection()
        self._observation_owner = None
        self._owner_discovery_attempted = True
        self._observation_connection_open = False

    @property
    def selected_runtime(self):
        return copy.deepcopy(self._runtime_record)

    def _current(self):
        require(self._observation_connection_open and self.pending is None and
                self.view is not None and self.view["state"] == "stopped" and
                self._confirmed_view is not None and exact(self.view, self._confirmed_view),
                "current accepted legacy state/control cursor")
        return self.view

    def _resources(self):
        view = self._current()
        require(self._runtime_record is not None and self._runtime_record["status"] == "ok" and
                self._runtime_binding is not None and
                exact(self._runtime_binding["cursor"], view["cursor"]) and
                view["cursor"]["event_sequence"] > 0 and
                self._runtime_record["allocation_watermark"]["availability"] == "available",
                "current available runtime allocation prefix")
        return self._runtime_binding

    def _body_observed(self, command, allocation=None, selected_range=None):
        view = self._current()
        if command == "runtime":
            require(self._observation_owner is not None or not self._owner_discovery_attempted,
                    "owner discovery only once on accepted connection")
            body = {"schema": RUNTIME_REQUEST, "operation": "inspect_current_record",
                    "expected_cursor": copy.deepcopy(view["cursor"])}
            if self._observation_owner is not None:
                body["expected_owner"] = copy.deepcopy(self._observation_owner)
            return body
        binding = self._resources()
        operations = {"storage": "query_allocations", "lifecycle": "query_allocation_lifecycle",
                      "storageaccess": "query_memory_accesses", "storagememory": "read_allocation_memory"}
        require(command in operations, "closed runtime resource operation")
        body = {"schema": RESOURCE_REQUEST, "operation": operations[command],
                "expected_binding": copy.deepcopy(binding)}
        if command in ("storageaccess", "storagememory"):
            identity = triple(allocation)
            require(identity in self._storage_inventory, "exact current observed inventory triple")
            body["allocation"] = copy.deepcopy(allocation)
        if command == "storagememory":
            row = self._storage_inventory[identity]
            require(row["snapshot_bytes_available"] is True and row["initialization_available"] is True,
                    "selected allocation has actual captured memory")
            memory_range(selected_range, int(row["descriptor"]["byte_len"]), 4096)
            body["range"] = copy.deepcopy(selected_range)
        else:
            body["page"] = {"max_items": 16, "max_scanned": 64}
        return body

    def parse_command(self, line):
        if not isinstance(line, str) or not line.split() or line.split()[0] not in COMMANDS:
            return super().parse_command(line)
        try:
            if line in ("runtime", "storage", "lifecycle"):
                return self._body_observed(line)
            match = re.fullmatch(r"storageaccess "+DECIMAL+" "+DECIMAL+" "+DECIMAL, line)
            if match:
                identity = dict(zip(("allocation", "storage_slot", "generation"), match.groups()))
                return self._body_observed("storageaccess", identity)
            match = re.fullmatch(r"storagememory "+DECIMAL+" "+DECIMAL+" "+DECIMAL+" "+DECIMAL+" "+DECIMAL, line)
            require(match is not None, "closed observed command")
            fields = match.groups()
            identity = dict(zip(("allocation", "storage_slot", "generation"), fields[:3]))
            return self._body_observed("storagememory", identity,
                                      {"byte_offset": fields[3], "byte_len": fields[4]})
        except ProtocolError:
            raise CommandError("observed query requires current owner-bound inputs") from None

    def begin(self, body):
        if type(body) is not dict or body.get("schema") not in SCHEMAS:
            return super().begin(body)
        require(self.pending is None and self.sent < MAX_COMMANDS-1, "shared observed query budget")
        operation = body.get("operation")
        if body["schema"] == RUNTIME_REQUEST:
            require(operation == "inspect_current_record", "closed runtime operation")
            expected = self._body_observed("runtime")
        else:
            command = {"query_allocations": "storage", "query_allocation_lifecycle": "lifecycle",
                       "query_memory_accesses": "storageaccess", "read_allocation_memory": "storagememory"}.get(operation)
            require(command is not None, "closed storage operation")
            expected = self._body_observed(command, body.get("allocation"), body.get("range"))
        require(exact(body, expected), "caller owner/cursor/query headers refused")
        request = {"request_id": self.sent+1, "expected_revision": self.view["revision"], **copy.deepcopy(body)}
        require(len(encode(request))+1 <= 4096, "shared observed request byte cap")
        if body["schema"] == RUNTIME_REQUEST:
            if self._observation_owner is None:
                self._owner_discovery_attempted = True
            self._runtime_record = None
            self._runtime_binding = None
            self._storage_inventory = {}
        elif operation == "query_allocations":
            self._storage_inventory = {}
        self.sent += 1
        self.pending = request
        return copy.deepcopy(request)

    def accept(self, response):
        try:
            require(self.pending is not None, "unsolicited observed response")
            request = copy.deepcopy(self.pending)
            if request["schema"] not in SCHEMAS:
                result = super().accept(response)
                if response["status"] != "ok":
                    self.clear_selection()
                elif request["operation"] in ("get_state", "step", "continue"):
                    self._confirmed_view = copy.deepcopy(self.view) if self.view["state"] == "stopped" else None
                return result
            if request["schema"] == RUNTIME_REQUEST:
                observed = validate_runtime(response, request, self.view, self._anchor)
                if observed is not None:
                    self._observation_owner = copy.deepcopy(observed["owner"])
                    self._runtime_binding = copy.deepcopy(observed)
                if response["status"] == "ok":
                    self._runtime_record = copy.deepcopy(response)
            else:
                require(self._runtime_record is not None and self._runtime_binding is not None,
                        "resource response lost runtime selection")
                rows = validate_resource(response, request, self.view,
                                         self._runtime_record, self._storage_inventory)
                if request["operation"] == "query_allocations" and rows is not None:
                    self._storage_inventory = {triple(row["descriptor"]["identity"]): copy.deepcopy(row)
                                               for row in rows}
            if response["status"] != "ok":
                # Valid refusal consumes one ID but never retains old inventory.
                self.clear_selection()
            self.pending = None
            return response
        except Exception:
            # No rediscovery/rebinding after uncertain transport or invalid data.
            self.close_observation_connection()
            raise
