// Synthetic pure controls only: no compiler, debugger, filesystem or capture execution.
import test from "node:test";
import assert from "node:assert/strict";
import { verifySourceWatchpointObservation } from "./resource-query-v6-watchpoint.mjs";

function fixture() {
  const configuration = "1".repeat(64);
  const session = (event, revision) => ({
    backend: "cpu_kir_simulator", execution_kind: "cpu_kir_simulation", state: "stopped", revision,
    configuration_identity: configuration,
    cursor: { configuration_identity: configuration, event_sequence: event, state_revision: revision },
    simulated: true, hardware_observed: false, performance_prediction: false,
  });
  const allocation = { ordinal: 7, generation: 0 };
  const spec = { client_label: "source-first-write", enabled: true, allocation,
    byte_offset: 0, byte_len: 4, access: "write", timing: "after_commit" };
  const initialSession = session(10, 20);
  const registered = session(10, 21), stopped = session(41, 22), captured = session(42, 23);
  const anchor = { cursor: captured.cursor,
    scope: { level: "lane", workgroup: [0, 0, 0], wave: 0, lane: 0, logical_workitem: [0, 0, 0],
      active_mask: 15, wave_width: 32, interpretation: "logical_visualization" },
    site: { kir: { function_ordinal: 0, block_ordinal: 11, point: { kind: "operation", operation_ordinal: 0 } },
      source: { status: "resolved", location: { map_identity: "2".repeat(64), provenance: "compiler_bundle_bound",
        file_identity: "3".repeat(64), byte_start: 931, byte_end: 947 } } } };
  const pair = (id, operation, revision, request, responseSession, result) => ({
    request: structuredClone({ schema: "fe2o3-debug-request-v1", request_id: id, operation, expected_revision: revision, ...request }),
    response: { status: "ok", schema: "fe2o3-debug-response-v1", request_id: id, operation,
      session: structuredClone(responseSession), result: structuredClone(result) },
  });
  return {
    initialSession, allocation,
    registration: pair(101, "set_watchpoints", 20, { watchpoints: [spec] }, registered,
      { result: "acknowledged", accepted: 1 }),
    listing: pair(102, "list_watchpoints", 21, { page: { limit: 16 } }, registered,
      { result: "watchpoints", watchpoints: [{ watchpoint_id: 17, spec, hit_count: 0 }] }),
    continuation: pair(103, "continue", 21, { max_events: 65536 }, stopped,
      { result: "control", stop: { reason: "watchpoint", watchpoint_id: 17, outcome: "active", exact: true },
        snapshot: { status: "unavailable", reason: "not_captured" }, events_advanced: 31 }),
    checkpoint: pair(104, "step", 22, { direction: "forward", granularity: "operation", count: 1 }, captured,
      { result: "control", stop: { reason: "step", outcome: "active", exact: true },
        snapshot: { status: "captured", snapshot: { anchor, stop: { reason: "step", outcome: "active", exact: true }, values: [] } },
        events_advanced: 1 }),
    memory: pair(108, "read_memory", 23, { allocation, byte_offset: 0, byte_len: 24 }, captured,
      { result: "memory", snapshot: anchor, memory: { allocation, byte_offset: 0, requested_bytes: 24, returned_bytes: 24,
        availability: { status: "captured", address_space: "global",
          bytes: "0xd5010000a5a5a5a5a5a5a5a5a5a5a5a5deadbeefcafebabe", initialized: "0xffffff", truncated: false } } }),
  };
}
function rejects(mutations) {
  for (const mutate of mutations) { const x = fixture(); mutate(x); assert.throws(() => verifySourceWatchpointObservation(x)); }
}
test("accepts current dynamic identities and distinguishes stop from memory checkpoint", () => {
  const x = fixture(), original = structuredClone(x), observed = verifySourceWatchpointObservation(x);
  assert.equal(observed.watchpoint_id, 17);
  assert.equal(observed.stop_cursor.event_sequence, 41);
  assert.equal(observed.stop_cursor.state_revision, 22);
  assert.equal(observed.checkpoint_anchor.cursor.event_sequence, 42);
  assert.equal(observed.checkpoint_anchor.cursor.state_revision, 23);
  assert.deepEqual(observed.stop_snapshot, { status: "unavailable", reason: "not_captured" });
  assert.equal(observed.memory_request_id, 108);
  assert.equal(observed.hardware_observed, false);
  assert.deepEqual(x, original);
  observed.checkpoint_anchor.cursor.event_sequence = 999;
  assert.deepEqual(x, original, "returned observations do not alias the input");
});
test("rejects missing or multiple registration and inaccurate acknowledgement", () => {
  rejects([
    x => x.registration.request.watchpoints = [],
    x => x.registration.request.watchpoints.push(structuredClone(x.registration.request.watchpoints[0])),
    x => x.registration.response.result.accepted = 0,
    x => x.registration.response.result.accepted = 2,
    x => x.registration.response.session.cursor.event_sequence++,
    x => x.registration.request.watchpoints[0].timing = "before_commit",
    x => x.registration.request.watchpoints[0].byte_len = 8,
  ]);
});
test("requires one complete matching current list, not an assumed historical ID", () => {
  rejects([
    x => x.listing.response.result.watchpoints = [],
    x => x.listing.response.result.watchpoints.push(structuredClone(x.listing.response.result.watchpoints[0])),
    x => x.listing.response.result.next_cursor = "more",
    x => x.listing.response.result.watchpoints[0].watchpoint_id = 0,
    x => x.listing.response.result.watchpoints[0].watchpoint_id = Number.MAX_SAFE_INTEGER + 1,
    x => x.listing.response.result.watchpoints[0].spec.allocation.ordinal++,
    x => x.listing.response.result.watchpoints[0].hit_count = 1,
    x => x.listing.request.page.limit = 64,
  ]);
});
test("requires an exact active watchpoint stop for the listed watchpoint", () => {
  rejects([
    x => x.continuation.response.result.stop.reason = "step",
    x => x.continuation.response.result.stop.reason = "fault",
    x => x.continuation.response.result.stop.watchpoint_id = 1,
    x => delete x.continuation.response.result.stop.watchpoint_id,
    x => x.continuation.response.result.stop.exact = false,
    x => x.continuation.response.result.stop.outcome = "completed",
    x => x.continuation.response.result.stop.breakpoint_id = 1,
  ]);
});
test("does not manufacture checkpoint bytes at the committed-write stop", () => {
  rejects([
    x => x.continuation.response.result.snapshot = structuredClone(x.checkpoint.response.result.snapshot),
    x => x.continuation.response.result.snapshot.reason = "not_represented",
    x => x.continuation.response.result.snapshot = { status: "unavailable", reason: "not_captured", bytes: "0x00000000" },
  ]);
});
test("requires the separate next forward operation checkpoint and unchanged independent memory anchor", () => {
  rejects([
    x => x.checkpoint.request.direction = "reverse",
    x => x.checkpoint.request.granularity = "source",
    x => x.checkpoint.request.count = 2,
    x => x.checkpoint.response.result.snapshot.status = "unavailable",
    x => x.checkpoint.response.result.snapshot.snapshot.anchor.site.source.location.provenance = "caller_bound",
    x => x.memory.response.result.snapshot.site.kir.block_ordinal++,
    x => x.memory.response.result.snapshot.scope.lane++,
    x => x.memory.response.result.snapshot.cursor = structuredClone(x.continuation.response.session.cursor),
    x => x.checkpoint.response.result.snapshot.snapshot.stop.reason = "watchpoint",
  ]);
});
test("rejects stale revisions, foreign configurations and forged CPU classification at every stage", () => {
  const names = ["registration", "listing", "continuation", "checkpoint", "memory"];
  for (const name of names) rejects([
    x => x[name].request.expected_revision--,
    x => x[name].response.session.revision++,
    x => x[name].response.session.cursor.state_revision++,
    x => { x[name].response.session.configuration_identity = "9".repeat(64); x[name].response.session.cursor.configuration_identity = "9".repeat(64); },
    x => x[name].response.session.hardware_observed = true,
    x => x[name].response.session.simulated = false,
    x => x[name].response.session.state = "terminated",
  ]);
});
test("rejects wrong response correlation, nonmonotone IDs and unavailable/error responses", () => {
  for (const name of ["registration", "listing", "continuation", "checkpoint", "memory"]) rejects([
    x => x[name].response.request_id++,
    x => x[name].response.operation = "get_state",
    x => x[name].response.schema = "another-schema",
    x => x[name].response.status = "unavailable",
    x => x[name].response.status = "error",
  ]);
  rejects([x => { x.memory.request.request_id = 104; x.memory.response.request_id = 104; }]);
});
test("bounds movement and rejects missing, zero, inaccurate and unsafe event arithmetic", () => {
  rejects([
    x => x.continuation.response.result.events_advanced = 0,
    x => x.continuation.response.result.events_advanced = 32,
    x => x.continuation.request.max_events = 1,
    x => x.continuation.response.session.cursor.event_sequence = 10,
    x => x.continuation.response.session.cursor.event_sequence = Number.MAX_SAFE_INTEGER + 1,
    x => x.checkpoint.response.result.events_advanced = 2,
    x => x.checkpoint.response.session.cursor.event_sequence++,
    x => x.initialSession.revision = Number.MAX_SAFE_INTEGER,
  ]);
});
test("retains exact allocation, full requested memory window and generation-zero profile", () => {
  rejects([
    x => x.allocation.generation = 1,
    x => x.memory.request.allocation.ordinal++,
    x => x.memory.request.byte_offset = 4,
    x => x.memory.request.byte_len = 4,
    x => x.memory.response.result.memory.returned_bytes = 4,
    x => x.memory.response.result.memory.allocation.ordinal++,
    x => x.memory.response.result.memory.availability.status = "unavailable",
    x => x.memory.response.result.memory.availability.truncated = true,
  ]);
});
