// Pure checks for one source-backed smoke scenario, not a new wire decoder or authority.
import assert from "node:assert/strict";

function integer(value, minimum = 0) {
  assert.ok(Number.isSafeInteger(value) && value >= minimum, "exact nonnegative integer");
  return value;
}
function session(value) {
  assert.ok(value && typeof value === "object" && !Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(), ["backend", "configuration_identity", "cursor", "execution_kind",
    "hardware_observed", "performance_prediction", "revision", "simulated", "state"].sort());
  assert.equal(value.backend, "cpu_kir_simulator");
  assert.equal(value.execution_kind, "cpu_kir_simulation");
  assert.equal(value.state, "stopped");
  assert.equal(value.simulated, true);
  assert.equal(value.hardware_observed, false);
  assert.equal(value.performance_prediction, false);
  assert.match(value.configuration_identity, /^[0-9a-f]{64}$/u);
  assert.notEqual(value.configuration_identity, "0".repeat(64));
  integer(value.revision); integer(value.cursor.event_sequence);
  assert.deepEqual(value.cursor, {
    configuration_identity: value.configuration_identity,
    event_sequence: value.cursor.event_sequence,
    state_revision: value.revision,
  });
  return value;
}
function exchange(pair, operation, previous, mutation) {
  const { request, response } = pair;
  assert.equal(request.schema, "fe2o3-debug-request-v1");
  assert.equal(response.schema, "fe2o3-debug-response-v1");
  assert.equal(request.operation, operation); assert.equal(response.operation, operation);
  integer(request.request_id, 1); assert.equal(response.request_id, request.request_id);
  assert.equal(request.expected_revision, previous.revision);
  assert.equal(response.status, "ok");
  const next = session(response.session);
  assert.equal(next.configuration_identity, previous.configuration_identity);
  assert.equal(next.revision, integer(previous.revision + (mutation ? 1 : 0)));
  if (!mutation) assert.deepEqual(next, previous);
  return next;
}
function capturedCheckpoint(response) {
  const result = response.result;
  assert.equal(result.result, "control");
  assert.deepEqual(result.stop, { reason: "step", outcome: "active", exact: true });
  assert.equal(result.snapshot.status, "captured");
  const snapshot = result.snapshot.snapshot;
  assert.deepEqual(snapshot.stop, result.stop);
  assert.deepEqual(snapshot.anchor.cursor, response.session.cursor);
  assert.equal(snapshot.anchor.scope.level, "lane");
  assert.equal(snapshot.anchor.scope.interpretation, "logical_visualization");
  assert.equal(snapshot.anchor.site.source.status, "resolved");
  assert.equal(snapshot.anchor.site.source.location.provenance, "compiler_bundle_bound");
  return snapshot.anchor;
}

/**
 * Inputs are paired replies from one current bounded public JSONL session.
 * Allocation comes from its actual inventory; watchpoint ID comes from its list.
 * No historical event, revision, allocation ordinal, source coordinate or ID is reused.
 * The existing caller separately checks all output bytes, initialization and canaries.
 */
export function verifySourceWatchpointObservation({
  initialSession, allocation, registration, listing, continuation, checkpoint, memory,
}) {
  const initial = session(initialSession);
  assert.deepEqual(allocation, { ordinal: integer(allocation.ordinal, 1), generation: 0 });
  const spec = { client_label: "source-first-write", enabled: true, allocation,
    byte_offset: 0, byte_len: 4, access: "write", timing: "after_commit" };
  assert.deepEqual(registration.request.watchpoints, [spec]);
  const registered = exchange(registration, "set_watchpoints", initial, true);
  assert.equal(registered.cursor.event_sequence, initial.cursor.event_sequence);
  assert.deepEqual(registration.response.result, { result: "acknowledged", accepted: 1 });

  assert.deepEqual(listing.request.page, { limit: 16 });
  const listed = exchange(listing, "list_watchpoints", registered, false);
  const list = listing.response.result;
  assert.deepEqual(Object.keys(list).sort(), ["result", "watchpoints"]);
  assert.equal(list.result, "watchpoints");
  assert.ok(Array.isArray(list.watchpoints) && list.watchpoints.length === 1);
  const row = list.watchpoints[0];
  assert.deepEqual(Object.keys(row).sort(), ["hit_count", "spec", "watchpoint_id"]);
  const watchpointId = integer(row.watchpoint_id, 1);
  assert.deepEqual(row.spec, spec);
  assert.equal(row.hit_count, 0, "the new first-write watchpoint has no earlier hit");

  assert.equal(continuation.request.max_events, 65536);
  const stopped = exchange(continuation, "continue", listed, true);
  const stop = continuation.response.result;
  assert.equal(stop.result, "control");
  assert.deepEqual(stop.stop, { reason: "watchpoint", watchpoint_id: watchpointId, outcome: "active", exact: true });
  assert.deepEqual(stop.snapshot, { status: "unavailable", reason: "not_captured" },
    "the committed-write stop is not a captured operation checkpoint");
  const advanced = integer(stopped.cursor.event_sequence - listed.cursor.event_sequence, 1);
  assert.ok(advanced <= continuation.request.max_events);
  assert.equal(stop.events_advanced, advanced);

  assert.equal(checkpoint.request.direction, "forward");
  assert.equal(checkpoint.request.granularity, "operation");
  assert.equal(checkpoint.request.count, 1);
  const captured = exchange(checkpoint, "step", stopped, true);
  assert.equal(captured.cursor.event_sequence, integer(stopped.cursor.event_sequence + 1));
  assert.equal(checkpoint.response.result.events_advanced, 1);
  const anchor = capturedCheckpoint(checkpoint.response);

  assert.deepEqual(memory.request.allocation, allocation);
  assert.equal(memory.request.byte_offset, 0); assert.equal(memory.request.byte_len, 24);
  exchange(memory, "read_memory", captured, false);
  const memoryResult = memory.response.result;
  assert.equal(memoryResult.result, "memory");
  assert.deepEqual(memoryResult.snapshot, anchor, "memory belongs to the independent later checkpoint");
  assert.deepEqual(memoryResult.memory.allocation, allocation);
  assert.equal(memoryResult.memory.byte_offset, 0);
  assert.equal(memoryResult.memory.requested_bytes, 24);
  assert.equal(memoryResult.memory.returned_bytes, 24);
  assert.equal(memoryResult.memory.availability.status, "captured");
  assert.equal(memoryResult.memory.availability.truncated, false);
  const pairs = [registration, listing, continuation, checkpoint, memory];
  for (let i = 1; i < pairs.length; i++) {
    assert.ok(pairs[i].request.request_id > pairs[i - 1].request.request_id, "current monotone request IDs");
  }
  return structuredClone({
    watchpoint_id: watchpointId,
    registration_request_id: registration.request.request_id,
    listing_request_id: listing.request.request_id,
    stop_request_id: continuation.request.request_id,
    stop_cursor: stopped.cursor,
    stop_snapshot: stop.snapshot,
    checkpoint_request_id: checkpoint.request.request_id,
    checkpoint_anchor: anchor,
    memory_request_id: memory.request.request_id,
    source_association: "compiler_bundle_bound_not_source_authentication",
    hardware_observed: false,
    performance_prediction: false,
  });
}
