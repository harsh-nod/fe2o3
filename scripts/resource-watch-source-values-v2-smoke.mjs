#!/usr/bin/env node
// Closed qualification of existing public queries. Importing starts no work.
// Original response anchors/lines are retained; this is not a protocol authority.
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { spawn, spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { verifySourceWatchpointObservation } from "./resource-query-v6-watchpoint.mjs";

const V1 = "fe2o3-debug-request-v1", V2 = "fe2o3-debug-source-variable-request-v2", RESOURCE = "fe2o3-debug-resource-request-v1";
const responseSchema = schema => schema === V2 ? "fe2o3-debug-source-variable-response-v2"
  : schema === RESOURCE ? "fe2o3-debug-resource-response-v1" : "fe2o3-debug-response-v1";
const sha = bytes => createHash("sha256").update(bytes).digest("hex"), copy = value => structuredClone(value);
const MiB = 1024 ** 2;
export const LIMITS = Object.freeze({ source: 256 * 1024, json: MiB, bundle: 8 * MiB, tool: 512 * MiB,
  inputBytes: 2 * 1024 ** 3, line: 65536, requests: 256 * 1024, responses: 4 * MiB, stderr: 65536,
  excerpt: 256 * 1024, retained: 8 * MiB, failureReserve: 65536, commands: 128, pages: 32, values: 64,
  stageMs: 30000, debuggerMs: 120000, replyMs: 15000, drainMs: 10000, totalMs: 180000 });
export const BEFORE_BYTES = "0x" + "a5a5a5a5".repeat(4) + "deadbeefcafebabe";
const result = Number((((((0xfffffff0n + 0x25n) & 0xffffffffn) - 0x25n) & 0xffffffffn) ^ 0x25n) & 255n | 256n);
assert.equal(result, 469);
const word = Buffer.alloc(4); word.writeUInt32LE(result);
export const AFTER_FIRST_BYTES = "0x" + word.toString("hex") + "a5a5a5a5".repeat(3) + "deadbeefcafebabe";
export const FINAL_BYTES = "0x" + word.toString("hex").repeat(4) + "deadbeefcafebabe";
function integer(value, min = 0, max = Number.MAX_SAFE_INTEGER) {
  assert.ok(Number.isSafeInteger(value) && value >= min && value <= max, "bounded exact integer"); return value;
}
function exact(value, keys) {
  assert.ok(value && typeof value === "object" && !Array.isArray(value), "object required");
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), "closed observation shape"); return value;
}
function digest(value) { assert.equal(typeof value, "string"); assert.match(value, /^[a-f0-9]{64}$/u); assert.notEqual(value, "0".repeat(64)); return value; }
function session(value) {
  exact(value, ["backend", "execution_kind", "state", "revision", "configuration_identity", "cursor", "simulated", "hardware_observed", "performance_prediction"]);
  assert.equal(value.backend, "cpu_kir_simulator"); assert.equal(value.execution_kind, "cpu_kir_simulation");
  assert.equal(value.state, "stopped"); assert.equal(value.simulated, true);
  assert.equal(value.hardware_observed, false); assert.equal(value.performance_prediction, false);
  integer(value.revision); digest(value.configuration_identity);
  assert.deepEqual(value.cursor, { configuration_identity: value.configuration_identity,
    event_sequence: integer(value.cursor.event_sequence), state_revision: value.revision }); return value;
}
function paired(pair, operation, schema = V1) {
  exact(pair, ["request", "response"]); const { request, response } = pair;
  integer(request.request_id, 1); integer(request.expected_revision);
  assert.equal(request.schema, schema); assert.equal(response.schema, responseSchema(schema));
  assert.equal(request.operation, operation); assert.equal(response.operation, operation);
  assert.equal(response.request_id, request.request_id); assert.equal(response.status, "ok"); session(response.session);
}
function unchanged(pair, operation, stopped, schema = V1) {
  paired(pair, operation, schema); assert.equal(pair.request.expected_revision, stopped.revision);
  assert.deepEqual(pair.response.session, stopped, "query belongs to this exact stopped session");
}
const unavailable = new Set(["not_represented", "not_captured", "optimized_out", "outside_capture_scope", "not_in_scope",
  "not_live", "uninitialized", "truncated", "unsupported_by_backend", "requires_authenticated_map"]);
function availability(value) {
  if (value.status === "unavailable") {
    exact(value, ["status", "reason"]); assert.ok(unavailable.has(value.reason)); return;
  }
  exact(value, ["status", "value_type", "value", "provenance"]);
  assert.equal(value.status, "captured"); assert.equal(value.provenance, "simulated_observation");
  const type = value.value_type, retained = value.value;
  if (type.kind === "pointer") {
    exact(type, ["kind", "address_space"]);
    assert.ok(["private", "workgroup", "global", "constant", "generic"].includes(type.address_space));
    exact(retained, ["encoding", "allocation", "byte_offset"]);
    assert.equal(retained.encoding, "allocation_relative_pointer");
    assert.deepEqual(retained.allocation, { ordinal: integer(retained.allocation.ordinal, 1), generation: 0 });
    integer(retained.byte_offset); return;
  }
  let width;
  if (type.kind === "bool") { exact(type, ["kind"]); width = 1; }
  else if (type.kind === "integer") {
    exact(type, ["kind", "signed", "bits"]); assert.equal(typeof type.signed, "boolean"); width = integer(type.bits, 1, 64);
  } else {
    exact(type, ["kind", "bits"]); assert.ok(["float", "index"].includes(type.kind));
    assert.ok((type.kind === "float" ? [16, 32, 64] : [32, 64]).includes(type.bits)); width = type.bits;
  }
  exact(retained, ["encoding", "bits"]); assert.equal(retained.encoding, "bits");
  assert.equal(typeof retained.bits, "string"); assert.match(retained.bits, /^0x[0-9a-f]+$/u);
  assert.equal(retained.bits.length, 2 + Math.ceil(width / 4)); assert.ok(BigInt(retained.bits) < (1n << BigInt(width)));
}
function checkpoint(control, count = 1, lane = 0) {
  integer(count, 1, 2); integer(lane, 0, 1);
  paired(control, "step");
  exact(control.request, ["schema", "request_id", "expected_revision", "operation", "direction", "granularity", "count"]);
  assert.ok(["forward", "reverse"].includes(control.request.direction));
  assert.equal(control.request.granularity, "operation"); assert.equal(control.request.count, count);
  assert.equal(control.response.session.revision, control.request.expected_revision + 1);
  exact(control.response, ["status", "schema", "request_id", "operation", "session", "result"]);
  const r = exact(control.response.result, ["result", "stop", "snapshot", "events_advanced"]);
  assert.equal(r.result, "control"); integer(r.events_advanced, 1, 65536);
  assert.deepEqual(r.stop, { reason: "step", outcome: "active", exact: true });
  exact(r.snapshot, ["status", "snapshot"]); assert.equal(r.snapshot.status, "captured");
  const captured = exact(r.snapshot.snapshot, ["anchor", "stop", "values"]); assert.deepEqual(captured.stop, r.stop);
  const anchor = exact(captured.anchor, ["cursor", "scope", "site"]); assert.deepEqual(anchor.cursor, control.response.session.cursor);
  assert.deepEqual(anchor.scope, { level: "lane", workgroup: [0, 0, 0], wave: 0, lane,
    logical_workitem: [lane, 0, 0], active_mask: 15, wave_width: 32, interpretation: "logical_visualization" });
  exact(anchor.site, ["kir", "source"]); const kir = exact(anchor.site.kir, ["function_ordinal", "block_ordinal", "point"]);
  assert.equal(kir.function_ordinal, 0); integer(kir.block_ordinal);
  exact(kir.point, ["kind", "operation_ordinal"]); assert.equal(kir.point.kind, "operation"); integer(kir.point.operation_ordinal);
  exact(anchor.site.source, ["status", "location"]); assert.equal(anchor.site.source.status, "resolved");
  const location = exact(anchor.site.source.location, ["map_identity", "file_identity", "provenance", "byte_start", "byte_end"]);
  digest(location.map_identity); digest(location.file_identity); assert.equal(location.provenance, "compiler_bundle_bound");
  integer(location.byte_start); integer(location.byte_end, location.byte_start + 1);
  assert.ok(Array.isArray(captured.values) && captured.values.length > 0 && captured.values.length <= LIMITS.values);
  const seen = new Set();
  for (const row of captured.values) {
    exact(row, ["path", "availability"]); exact(row.path, ["root", "components"]); assert.deepEqual(row.path.components, []);
    const identity = exact(row.path.root, ["kind", "function_ordinal", "frame", "value_ordinal"]);
    assert.equal(identity.kind, "ssa"); assert.equal(identity.function_ordinal, 0); assert.equal(identity.frame, 1);
    integer(identity.value_ordinal); assert.ok(!seen.has(identity.value_ordinal)); seen.add(identity.value_ordinal); availability(row.availability);
  }
  return captured;
}
function stackAt(stack, stopped, captured, sourceQueryable) {
  const anchor = captured.anchor;
  unchanged(stack, "inspect_stack", stopped);
  exact(stack.request, ["schema", "request_id", "expected_revision", "operation", "scope", "page"]);
  assert.deepEqual(stack.request.scope, { level: "dispatch" }); assert.deepEqual(stack.request.page, { limit: 16 });
  exact(stack.response, ["status", "schema", "request_id", "operation", "session", "result"]);
  const sr = exact(stack.response.result, ["result", "snapshot", "frames"]);
  assert.equal(sr.result, "stack"); assert.deepEqual(sr.snapshot, anchor);
  assert.ok(Array.isArray(sr.frames) && sr.frames.length === 1, "real complete root frame required");
  const frame = exact(sr.frames[0], ["frame", "function_ordinal", "block_ordinal", "values", ...(sourceQueryable ? ["next_operation"] : [])]);
  assert.equal(frame.frame, 1); assert.equal(frame.function_ordinal, 0); assert.equal(frame.block_ordinal, anchor.site.kir.block_ordinal);
  if (sourceQueryable) integer(frame.next_operation); assert.deepEqual(frame.values, { status: "captured", value_count: captured.values.length });
  return frame;
}
function memoryAt(memory, stopped, anchor, expectedBytes) {
  unchanged(memory, "read_memory", stopped);
  exact(memory.request, ["schema", "request_id", "expected_revision", "operation", "allocation", "byte_offset", "byte_len"]);
  const allocation = memory.request.allocation;
  assert.deepEqual(allocation, { ordinal: integer(allocation.ordinal, 1), generation: 0 });
  assert.equal(memory.request.byte_offset, 0); assert.equal(memory.request.byte_len, 24);
  exact(memory.response, ["status", "schema", "request_id", "operation", "session", "result"]);
  assert.deepEqual(memory.response.result, { result: "memory", snapshot: anchor, memory: {
    allocation, byte_offset: 0, requested_bytes: 24, returned_bytes: 24,
    availability: { status: "captured", address_space: "global", bytes: expectedBytes, initialized: "0xffffff", truncated: false } } });
  return memory.response.result.memory;
}
export function verifyWatchSourceCheckpoint(group, expectedBytes, count, lane) {
  exact(group, ["control", "stack", "sourcePages", "memory"]);
  const { control, stack, sourcePages, memory } = group, captured = checkpoint(control, count, lane), anchor = captured.anchor, stopped = control.response.session;
  const frame = stackAt(stack, stopped, captured, true);
  assert.ok(Array.isArray(sourcePages) && sourcePages.length >= 2 && sourcePages.length <= LIMITS.pages);
  const values = [], seen = new Set(); let cursor, sourceAnchor, queryIdentity;
  for (const [index, pair] of sourcePages.entries()) {
    unchanged(pair, "inspect_source_variables", stopped, V2);
    exact(pair.request, ["schema", "request_id", "expected_revision", "operation", "scope", "frame", "selector", "page"]);
    assert.deepEqual(pair.request.scope, { level: "dispatch" }); assert.equal(pair.request.frame, 1);
    assert.deepEqual(pair.request.selector, { selector: "all" }); assert.deepEqual(pair.request.page, { limit: 2, ...(cursor ? { cursor } : {}) });
    const response = pair.response;
    exact(response, ["status", "schema", "request_id", "operation", "session", "snapshot", "values",
      ...(Object.hasOwn(response, "next_cursor") ? ["next_cursor"] : [])]);
    assert.deepEqual(response.snapshot, { ...anchor, frame: 1, occurrence: 1 }, "explicit frame refinement, not checkpoint equality");
    sourceAnchor ??= response.snapshot; assert.deepEqual(response.snapshot, sourceAnchor);
    assert.ok(Array.isArray(response.values) && response.values.length > 0 && response.values.length <= 2);
    for (const row of response.values) {
      exact(row, ["variable_identity", "name", "function_ordinal", "scope_identity", "scope_depth", "generation", "availability"]);
      digest(row.variable_identity); digest(row.scope_identity); assert.equal(row.function_ordinal, 0);
      integer(row.scope_depth, 0, 4096); integer(row.generation);
      assert.ok(typeof row.name === "string" && row.name.length > 0 && Buffer.byteLength(row.name) <= 4096 && !/[\p{Cc}\p{Cs}]/u.test(row.name));
      assert.ok(!seen.has(row.variable_identity)); seen.add(row.variable_identity);
      exact(row.availability, ["status", "value"]); assert.equal(row.availability.status, "value"); availability(row.availability.value);
      if (row.availability.value.status === "captured") assert.ok(row.generation > 0);
      values.push(row); assert.ok(values.length <= LIMITS.values);
    }
    cursor = response.next_cursor;
    if (cursor !== undefined) {
      exact(cursor, ["query_identity", "position"]); digest(cursor.query_identity); assert.equal(cursor.position, values.length);
      queryIdentity ??= cursor.query_identity; assert.equal(cursor.query_identity, queryIdentity); assert.equal(response.values.length, 2);
    }
    assert.equal(cursor === undefined, index === sourcePages.length - 1, "complete source page chain");
  }
  for (const [name, bits] of [["a", "0xfffffff0"], ["b", "0x00000025"]]) {
    const matches = values.filter(v => v.name === name); assert.equal(matches.length, 1);
    assert.equal(matches[0].generation, 1);
    assert.deepEqual(matches[0].availability, { status: "value", value: { status: "captured",
      value_type: { kind: "integer", signed: false, bits: 32 }, value: { encoding: "bits", bits }, provenance: "simulated_observation" } });
  }
  // Other variables retain their actual availability; no source-to-SSA guess.
  for (const name of ["out", "result"]) assert.equal(values.filter(v => v.name === name).length, 1);
  memoryAt(memory, stopped, anchor, expectedBytes);
  const pairs = [control, stack, ...sourcePages, memory];
  pairs.slice(1).forEach((p, i) => assert.ok(p.request.request_id > pairs[i].request.request_id, "monotone original query IDs"));
  return copy({ control_request_id: control.request.request_id, stack_request_id: stack.request.request_id,
    source_request_ids: sourcePages.map(p => p.request.request_id), memory_request_id: memory.request.request_id,
    checkpoint_anchor: anchor, source_anchor: sourceAnchor, stack: frame, source_values: values,
    ssa_values: captured.values, memory: memory.response.result.memory });
}
export function verifyImmediateWatchCheckpoint(group) {
  exact(group, ["control", "stack", "sourceRefusal", "memory"]);
  const { control, stack, sourceRefusal, memory } = group, captured = checkpoint(control, 1, 0);
  assert.equal(control.request.direction, "forward");
  const frame = stackAt(stack, control.response.session, captured, false);
  verifyWatchSourceRefusal(sourceRefusal, control.response.session, "unavailable", "checkpoint_not_captured");
  memoryAt(memory, control.response.session, captured.anchor, AFTER_FIRST_BYTES);
  const pairs = [control, stack, sourceRefusal, memory];
  pairs.slice(1).forEach((p, i) => assert.ok(p.request.request_id > pairs[i].request.request_id));
  return copy({ control_request_id: control.request.request_id, stack_request_id: stack.request.request_id,
    source_refusal_request_id: sourceRefusal.request.request_id, memory_request_id: memory.request.request_id,
    checkpoint_anchor: captured.anchor, stack: frame, ssa_values: captured.values,
    source_values: "unavailable_checkpoint_not_captured", memory: memory.response.result.memory });
}
export function verifyWatchSourceReplay(input) {
  exact(input, ["initial", "inventory", "registration", "listing", "continuation", "watchSourceRefusal", "immediate", "groups"]);
  const initial = checkpoint(input.initial); assert.equal(input.initial.request.direction, "forward");
  const immediate = verifyImmediateWatchCheckpoint(input.immediate);
  verifyWatchSourceRefusal(input.watchSourceRefusal, input.continuation.response.session, "unavailable", "checkpoint_not_captured");
  assert.ok(Array.isArray(input.groups) && input.groups.length === 3);
  const counts = [1, 2, 2], lanes = [1, 0, 1];
  const observations = input.groups.map((g, i) => verifyWatchSourceCheckpoint(g,
    i === 1 ? BEFORE_BYTES : AFTER_FIRST_BYTES, counts[i], lanes[i]));
  const [post, reverse, repeat] = observations;
  assert.deepEqual(input.groups.map(g => g.control.request.direction), ["forward", "reverse", "forward"]);
  const inv = input.inventory;
  unchanged(inv, "query_allocations", input.initial.response.session, RESOURCE);
  exact(inv.request, ["schema", "request_id", "expected_revision", "operation", "expected_snapshot", "page"]);
  assert.deepEqual(inv.request.expected_snapshot, initial.anchor); assert.deepEqual(inv.request.page, { max_items: 16, max_scanned: 16 });
  exact(inv.response, ["status", "schema", "request_id", "operation", "session", "snapshot", "page", "result", "physical_registers"]);
  assert.deepEqual(inv.response.page, { source_count: 1, scanned: 1, completeness: { status: "complete" } });
  exact(inv.response.result, ["result", "allocations"]);
  assert.deepEqual(inv.response.snapshot, initial.anchor); assert.equal(inv.response.physical_registers, "not_represented");
  assert.equal(inv.response.result.result, "allocations"); assert.equal(inv.response.result.allocations.length, 1);
  assert.equal(inv.response.page.completeness.status, "complete"); assert.equal(inv.response.page.next_token, undefined);
  const allocation = immediate.memory.allocation, row = inv.response.result.allocations[0];
  exact(row, ["allocation", "address_space", "access", "alignment", "capacity_bytes", "snapshot_bytes_available",
    "initialization_available", "owning_scope", "lifetime", "physical_base"]);
  assert.equal(row.alignment, 4); assert.equal(row.snapshot_bytes_available, true); assert.equal(row.initialization_available, true);
  assert.deepEqual(row.allocation, allocation); assert.equal(row.capacity_bytes, "24"); assert.equal(row.address_space, "global");
  assert.equal(row.access, "read_write"); assert.equal(row.owning_scope, "not_represented");
  assert.equal(row.lifetime, "not_represented"); assert.equal(row.physical_base, "not_represented");
  for (const [key, operation, field] of [["registration", "set_watchpoints", "watchpoints"],
    ["listing", "list_watchpoints", "page"], ["continuation", "continue", "max_events"]]) {
    paired(input[key], operation);
    exact(input[key].request, ["schema", "request_id", "expected_revision", "operation", field]);
    exact(input[key].response, ["status", "schema", "request_id", "operation", "session", "result"]);
  }
  exact(input.continuation.response.result, ["result", "stop", "snapshot", "events_advanced"]);
  // The legacy seven pairs still refer to the immediate lane-0 checkpoint.
  // They never substitute the later lane-1 source-queryable checkpoint.
  const watchpoint = verifySourceWatchpointObservation({ initialSession: input.initial.response.session, allocation,
    registration: input.registration, listing: input.listing, continuation: input.continuation,
    checkpoint: input.immediate.control, memory: input.immediate.memory });
  assert.equal(post.checkpoint_anchor.cursor.state_revision, immediate.checkpoint_anchor.cursor.state_revision + 1);
  assert.equal(input.groups[0].control.request.expected_revision, immediate.checkpoint_anchor.cursor.state_revision);
  assert.equal(post.checkpoint_anchor.cursor.event_sequence - immediate.checkpoint_anchor.cursor.event_sequence,
    input.groups[0].control.response.result.events_advanced);
  assert.deepEqual(post.checkpoint_anchor.site.kir, { function_ordinal: 0, block_ordinal: 0,
    point: { kind: "operation", operation_ordinal: 0 } });
  assert.equal(post.stack.next_operation, 0, "lane 1 is before its first operation");
  assert.deepEqual(post.checkpoint_anchor.site, initial.anchor.site, "same static first operation, distinct invocation");
  const immediateSource = immediate.checkpoint_anchor.site.source.location;
  for (const field of ["map_identity", "file_identity", "provenance"])
    assert.equal(immediateSource[field], initial.anchor.site.source.location[field]);
  assert.notDeepEqual(post.checkpoint_anchor, immediate.checkpoint_anchor, "distinct original checkpoints");
  for (const [index, observation] of observations.entries()) {
    assert.equal(observation.checkpoint_anchor.cursor.configuration_identity, initial.anchor.cursor.configuration_identity);
    const source = observation.checkpoint_anchor.site.source.location, initialSource = initial.anchor.site.source.location;
    for (const field of ["map_identity", "file_identity", "provenance"]) assert.equal(source[field], initialSource[field]);
    assert.deepEqual(observation.memory.allocation, allocation);
    if (index > 0) {
      assert.equal(observation.checkpoint_anchor.cursor.state_revision, observations[index - 1].checkpoint_anchor.cursor.state_revision + 1);
      assert.equal(input.groups[index].control.request.expected_revision, observations[index - 1].checkpoint_anchor.cursor.state_revision);
    }
  }
  assert.ok(reverse.checkpoint_anchor.cursor.event_sequence < watchpoint.stop_cursor.event_sequence,
    "reverse reaches the separately captured lane-0 prewrite state");
  assert.deepEqual(reverse.checkpoint_anchor.site, immediate.checkpoint_anchor.site, "same static store before versus after");
  assert.equal(reverse.stack.next_operation, reverse.checkpoint_anchor.site.kir.point.operation_ordinal);
  assert.equal(post.checkpoint_anchor.cursor.event_sequence - reverse.checkpoint_anchor.cursor.event_sequence,
    input.groups[1].control.response.result.events_advanced);
  assert.equal(repeat.checkpoint_anchor.cursor.event_sequence - reverse.checkpoint_anchor.cursor.event_sequence,
    input.groups[2].control.response.result.events_advanced);
  assert.equal(repeat.checkpoint_anchor.cursor.event_sequence, post.checkpoint_anchor.cursor.event_sequence);
  for (const key of ["source_values", "ssa_values", "stack", "memory"]) assert.deepEqual(repeat[key], post[key], "repeat restores actual retained state");
  assert.deepEqual(repeat.checkpoint_anchor.site, post.checkpoint_anchor.site);
  assert.deepEqual(repeat.checkpoint_anchor.scope, post.checkpoint_anchor.scope);
  assert.deepEqual(reverse.checkpoint_anchor.scope, immediate.checkpoint_anchor.scope);
  assert.notDeepEqual(reverse.checkpoint_anchor.scope, post.checkpoint_anchor.scope, "explicit cross-invocation replay");
  assert.notDeepEqual(reverse.memory.availability.bytes, post.memory.availability.bytes);
  const all = [input.initial, inv, input.registration, input.listing, input.continuation, input.watchSourceRefusal,
    input.immediate.control, input.immediate.stack, input.immediate.sourceRefusal, input.immediate.memory,
    ...input.groups.flatMap(g => [g.control, g.stack, ...g.sourcePages, g.memory])];
  all.slice(1).forEach((p, i) => assert.ok(p.request.request_id > all[i].request.request_id, "monotone whole selected interaction"));
  return copy({ watchpoint, immediate_watch_checkpoint: immediate, observations,
    watchpoint_source_refusal: input.watchSourceRefusal, watchpoint_snapshot: "unavailable_not_captured",
    immediate_postwrite_source_values: "unavailable_checkpoint_not_captured",
    source_and_memory_belong_to: "distinct_cross_invocation_operation_checkpoints_only",
    source_checkpoint_lanes: lanes, source_control_counts: counts, source_to_ssa_mapping: "not_supplied",
    frame_identity: "static_stack_depth_not_dynamic_activation",
    allocation_reuse: "not_represented", dynamic_helper_activation: "not_represented",
    source_authentication: false, hardware_observed: false, performance_prediction: false });
}
export function verifyWatchSourceRefusal(pair, previousSession, status, reason) {
  session(previousSession); exact(pair, ["request", "response"]);
  const { request, response } = pair; assert.equal(request.schema, V2);
  exact(request, ["schema", "request_id", "expected_revision", "operation", "scope", "frame", "selector", "page"]);
  integer(request.expected_revision); assert.deepEqual(request.scope, { level: "dispatch" }); assert.equal(request.frame, 1);
  assert.ok(request.selector.selector === "all" || request.selector.selector === "name");
  assert.deepEqual(request.selector, request.selector.selector === "all" ? { selector: "all" } : { selector: "name", name: "a" });
  exact(request.page, ["limit", ...(Object.hasOwn(request.page, "cursor") ? ["cursor"] : [])]); assert.equal(request.page.limit, 2);
  if (request.page.cursor !== undefined) {
    exact(request.page.cursor, ["query_identity", "position"]); digest(request.page.cursor.query_identity); integer(request.page.cursor.position, 1, LIMITS.values);
  }
  assert.equal(request.operation, "inspect_source_variables"); integer(request.request_id, 1);
  assert.equal(response.request_id, request.request_id); assert.equal(response.operation, request.operation);
  assert.equal(response.schema, responseSchema(V2)); assert.equal(response.status, status);
  assert.deepEqual(response.session, previousSession, "refusal cannot change any stopped-session field");
  if (status === "error") {
    assert.ok(["stale_revision", "invalid_cursor"].includes(reason));
    if (reason === "stale_revision") assert.ok(request.expected_revision < previousSession.revision);
    else { assert.equal(request.expected_revision, previousSession.revision); assert.ok(request.page.cursor); }
    exact(response, ["status", "schema", "request_id", "operation", "session", "error"]);
    exact(response.error, ["stage", "code", "message", "state_changed"]); assert.equal(response.error.stage, "session");
    assert.equal(response.error.code, reason); assert.equal(response.error.state_changed, false);
    assert.ok(typeof response.error.message === "string" && Buffer.byteLength(response.error.message) <= 4096);
  } else {
    assert.equal(reason, "checkpoint_not_captured"); assert.equal(request.expected_revision, previousSession.revision);
    assert.deepEqual(request.selector, { selector: "all" }); assert.deepEqual(request.page, { limit: 2 });
    assert.equal(status, "unavailable"); exact(response, ["status", "schema", "request_id", "operation", "session", "reason"]);
    assert.equal(response.reason, reason);
  }
  return copy(pair);
}
export function verifyIndependentWatchResult(value, kirDigest) {
  assert.equal(value.schema, "fe2o3-simulation-result-v1"); assert.equal(value.status, "ok");
  assert.equal(value.authority, "observation_only"); assert.equal(value.simulated, true);
  for (const flag of ["hardware_observed", "hardware_validation", "performance_prediction"]) assert.equal(value[flag], false);
  assert.equal(value.kir.sha256, kirDigest); assert.equal(value.counts.invocations_executed, 4);
  assert.equal(value.arguments.length, 3); assert.equal(value.shared_buffers.length, 0);
  assert.deepEqual(value.arguments.slice(1), [{ kind: "scalar", type: "u32", bits: "0xfffffff0" }, { kind: "scalar", type: "u32", bits: "0x00000025" }]);
  const first = value.arguments[0]; assert.equal(first.kind, "buffer");
  assert.deepEqual(first.value, { element: "u32", access: "read_write", alignment: 4, bytes: FINAL_BYTES, initialized: "0xffffff" });
  return { expected_u32: result, output_words: 4, tail_canary_unchanged: true, hardware_observed: false };
}
export function originalExcerpt(ids, pairs, requests, responses) {
  assert.equal(pairs.length, requests.length); assert.equal(pairs.length, responses.length);
  assert.ok(pairs.length > 0 && pairs.length <= LIMITS.commands);
  pairs.forEach((pair, index) => { integer(pair.request.request_id, 1, LIMITS.commands);
    assert.equal(pair.response.request_id, pair.request.request_id);
    if (index > 0) assert.ok(pair.request.request_id > pairs[index - 1].request.request_id, "unique ordered full transcript IDs");
  });
  assert.ok(Array.isArray(ids) && ids.length > 0 && ids.length <= LIMITS.commands && new Set(ids).size === ids.length);
  let prior = 0;
  const indices = ids.map(id => { integer(id, 1); assert.ok(id > prior); prior = id;
    const index = pairs.findIndex(p => p.request.request_id === id); assert.ok(index >= 0);
    for (const [line, object] of [[requests[index], pairs[index].request], [responses[index], pairs[index].response]]) {
      assert.ok(line.endsWith("\n") && !line.includes("\r") && line.charCodeAt(0) !== 0xfeff && Buffer.byteLength(line) <= LIMITS.line);
      assert.deepEqual(JSON.parse(line), object);
    }
    assert.equal(pairs[index].response.request_id, id); assert.equal(pairs[index].response.status, "ok"); return index;
  });
  const requestText = indices.map(i => requests[i]).join(""), responseText = indices.map(i => responses[i]).join("");
  assert.ok(Buffer.byteLength(requestText) <= LIMITS.excerpt && Buffer.byteLength(responseText) <= LIMITS.excerpt);
  return { requestText, responseText };
}

const statIdentity = s => ["dev", "ino", "size", "mode", "mtimeNs", "ctimeNs", "nlink"].map(k => String(s[k]));
async function run(repoArgument, sourceArgument, outputArgument) {
  const root = path.resolve(repoArgument), baseline = path.resolve(sourceArgument), output = path.resolve(outputArgument);
  for (const dir of [root, baseline, path.dirname(output)]) assert.ok(fs.lstatSync(dir).isDirectory() && fs.realpathSync(dir) === dir);
  for (const protectedInput of [root, baseline]) assert.ok(output !== protectedInput && !output.startsWith(protectedInput + path.sep), "output cannot modify an input tree");
  assert.ok(!fs.existsSync(output)); fs.mkdirSync(output, { mode: 0o700 });
  const started = Date.now(), deadline = started + LIMITS.totalMs, pins = []; let inputBytes = 0, retainedBytes = 0;
  const check = () => assert.ok(Date.now() < deadline, "total capture wall bound");
  function measure(file, cap, read = false) {
    check(); assert.equal(fs.realpathSync(file), file);
    const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
    try {
      const stat = fs.fstatSync(fd, { bigint: true }); assert.ok(stat.isFile() && stat.size > 0n && stat.size <= BigInt(cap));
      const h = createHash("sha256"), chunks = [], block = Buffer.alloc(65536); let bytes = 0;
      while (bytes < Number(stat.size)) { check(); const n = fs.readSync(fd, block, 0, Math.min(block.length, Number(stat.size) - bytes), bytes);
        assert.ok(n > 0); bytes += n; inputBytes += n; assert.ok(inputBytes <= LIMITS.inputBytes, "cumulative input reads");
        h.update(block.subarray(0, n)); if (read) chunks.push(Buffer.from(block.subarray(0, n)));
      }
      assert.deepEqual(statIdentity(fs.fstatSync(fd, { bigint: true })), statIdentity(stat));
      assert.deepEqual(statIdentity(fs.lstatSync(file, { bigint: true })), statIdentity(stat));
      return { pin: { path: file, bytes, sha256: h.digest("hex"), identity: statIdentity(stat) }, bytes: read ? Buffer.concat(chunks) : null };
    } finally { fs.closeSync(fd); }
  }
  function input(file, cap, read = true) { const value = measure(file, cap, read); pins.push({ ...value.pin, cap }); return value.bytes; }
  function save(name, value, failure = false) {
    assert.match(name, /^[a-zA-Z0-9][a-zA-Z0-9.-]*$/u);
    const bytes = Buffer.isBuffer(value) ? value : Buffer.from(typeof value === "string" ? value : JSON.stringify(value, null, 2) + "\n");
    const ceiling = LIMITS.retained - (failure ? 0 : LIMITS.failureReserve);
    assert.ok(retainedBytes + bytes.length <= ceiling, "retained file budget");
    fs.writeFileSync(path.join(output, name), bytes, { flag: "wx", mode: 0o600 }); retainedBytes += bytes.length;
    return { path: name, bytes: bytes.length, sha256: sha(bytes) };
  }
  const pairs = [], requests = [], responses = [], refusals = [], stages = [];
  let child, pending, timer, drainTimer, exited, exitValue, closed = false, closing = false, failure;
  let revision = 0, id = 1, buffered = "", stderr = Buffer.alloc(0), totalResponse = 0, totalRequest = 0;
  const rawFiles = {}, decoder = new TextDecoder("utf-8", { fatal: true });
  function append(which, bytes) {
    assert.ok(retainedBytes + bytes.length <= LIMITS.retained - LIMITS.failureReserve, "raw retained budget");
    let offset = 0; while (offset < bytes.length) { const n = fs.writeSync(rawFiles[which], bytes, offset, bytes.length - offset); assert.ok(n > 0); offset += n; }
    retainedBytes += bytes.length;
  }
  function fail(error) {
    failure ??= error; pending?.reject(error); pending = undefined;
    if (child && !closed) child.kill("SIGKILL");
  }
  async function ask(command) {
    check(); if (failure) throw failure; assert.equal(pending, undefined); integer(id, 1, LIMITS.commands);
    const request = { schema: V1, request_id: id++, expected_revision: revision, ...command }, line = JSON.stringify(request) + "\n";
    const bytes = Buffer.from(line); totalRequest += bytes.length; assert.ok(bytes.length <= LIMITS.line && totalRequest <= LIMITS.requests);
    append("requests", bytes); requests.push(line);
    const response = await new Promise((resolve, reject) => {
      const timeout = setTimeout(() => fail(new Error("debugger response deadline")), Math.min(LIMITS.replyMs, deadline - Date.now()));
      pending = { resolve(value) { clearTimeout(timeout); resolve(value); }, reject(error) { clearTimeout(timeout); reject(error); } };
      child.stdin.write(line, error => { if (error) fail(error); });
    });
    assert.equal(response.request_id, request.request_id); assert.equal(response.operation, request.operation);
    assert.equal(response.schema, responseSchema(request.schema)); revision = integer(response.session.revision);
    const pair = { request, response }; pairs.push(pair); return pair;
  }
  try {
    const sourcePath = "crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30/src/lib.rs";
    const source = input(path.join(root, sourcePath), LIMITS.source);
    const receiptBytes = input(path.join(baseline, "receipt.json"), LIMITS.json), receipt = JSON.parse(receiptBytes);
    assert.equal(receipt.schema, "fe2o3-assembly-authoring-source-smoke-v30"); assert.equal(receipt.hardware_observed, false);
    assert.equal(receipt.source, sourcePath); assert.equal(receipt.source_sha256, sha(source));
    for (const name of ["Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
      "crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30/Cargo.toml"]) input(path.join(root, name), LIMITS.json, false);
    input(fileURLToPath(import.meta.url), LIMITS.source, false);
    const dependency = fileURLToPath(new URL("./resource-query-v6-watchpoint.mjs", import.meta.url));
    const dependencyBytes = input(dependency, LIMITS.source);
    assert.deepEqual(dependencyBytes, input(path.join(root, "scripts/resource-query-v6-watchpoint.mjs"), LIMITS.source), "current unchanged watchpoint verifier");
    const bundlePath = path.join(baseline, "base-v6.fe2sim"), requestPath = path.join(baseline, "base-request.json");
    const bundle = input(bundlePath, LIMITS.bundle), requestBytes = input(requestPath, LIMITS.line), request = JSON.parse(requestBytes);
    assert.equal(sha(bundle), receipt.base.bundle_sha256);
    assert.deepEqual(request, { schema: "fe2o3-simulation-request-v1", kernel: "assembly_chain", grid: [4, 1, 1], workgroup: [64, 1, 1],
      arguments: [{ kind: "buffer", element: "u32", access: "read_write", alignment: 4, bytes: BEFORE_BYTES },
        { kind: "scalar", type: "u32", bits: "0xfffffff0" }, { kind: "scalar", type: "u32", bits: "0x00000025" }] });
    const bin = path.join(path.resolve(process.env.CARGO_TARGET_DIR ?? path.join(root, "target")), "debug");
    const tools = ["fe2o3-author", "fe2o3-kir-sim", "fe2o3-debug"].map(name => { const file = path.join(bin, name); input(file, LIMITS.tool, false); return file; });
    const stage = (name, index, args, stdin) => {
      check(); const result = spawnSync(tools[index], args, { cwd: root, input: stdin,
        timeout: Math.max(1, Math.min(LIMITS.stageMs, deadline - Date.now())), maxBuffer: LIMITS.json, killSignal: "SIGKILL" });
      const stdout = save(name + ".stdout", result.stdout ?? ""), stderr = save(name + ".stderr", result.stderr ?? "");
      stages.push({ name, executable: tools[index], args, code: result.status, signal: result.signal, error: result.error?.code ?? null, stdout, stderr });
      assert.equal(result.error, undefined); assert.equal(result.status, 0); assert.equal(result.signal, null);
      return JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(result.stdout));
    };
    const summary = stage("inspection", 0, ["inspect"], bundle);
    assert.equal(summary.bundle_identity, receipt.base.summary.bundle_identity);
    assert.equal(summary.canonical_kir_digest, receipt.base.summary.canonical_kir_digest); assert.equal(summary.canonical_kir_version, 11);
    assert.equal(summary.authority.grants_production_resume, false); assert.equal(summary.authority.source_authenticated, false);
    const independent = verifyIndependentWatchResult(stage("simulation", 1, ["--bundle-v6", bundlePath, "--request", requestPath]), summary.canonical_kir_digest);
    const independentPin = save("independent-result.json", independent);
    for (const name of ["requests", "responses"]) rawFiles[name] = fs.openSync(path.join(output, "debug-" + name + ".jsonl"), "wx", 0o600);
    child = spawn(tools[2], ["sim", "--bundle-v6", bundlePath, "--request", requestPath, "--wave-width", "32"], { cwd: root, stdio: ["pipe", "pipe", "pipe"] });
    exited = new Promise(resolve => child.once("close", (code, signal) => {
      closed = true; exitValue = { code, signal }; clearTimeout(drainTimer);
      try { assert.equal(decoder.decode(), ""); if (pending) fail(new Error("closed before pending response drained")); } catch (e) { fail(e); }
      resolve(exitValue);
    }));
    timer = setTimeout(() => fail(new Error("debugger process deadline")), Math.max(1, Math.min(LIMITS.debuggerMs, deadline - Date.now())));
    child.once("error", fail); child.stdin.on("error", fail);
    child.stderr.on("data", chunk => { try { assert.ok(stderr.length + chunk.length <= LIMITS.stderr); stderr = Buffer.concat([stderr, chunk]); } catch (e) { fail(e); } });
    child.stdout.on("data", chunk => {
      try {
        totalResponse += chunk.length; assert.ok(totalResponse <= LIMITS.responses);
        append("responses", chunk); buffered += decoder.decode(chunk, { stream: true });
        assert.ok(Buffer.byteLength(buffered) <= LIMITS.line); const end = buffered.indexOf("\n"); if (end < 0) return;
        assert.ok(pending); const line = buffered.slice(0, end + 1); buffered = buffered.slice(end + 1);
        assert.equal(buffered, ""); assert.ok(!line.includes("\r") && line.charCodeAt(0) !== 0xfeff);
        const response = JSON.parse(line); responses.push(line); const current = pending; pending = undefined; current.resolve(response);
      } catch (e) { fail(e); }
    });
    child.once("exit", (code, signal) => {
      if (!closing || code !== 0 || signal !== null) fail(new Error("unexpected debugger exit"));
      drainTimer = setTimeout(() => { fail(new Error("debugger stream drain deadline")); child.stdout.destroy(); child.stderr.destroy(); child.unref(); }, LIMITS.drainMs);
    });
    const step = (direction, count = 1) => ask({ operation: "step", direction, granularity: "operation", count });
    const query = { schema: V2, operation: "inspect_source_variables", scope: { level: "dispatch" }, frame: 1, selector: { selector: "all" }, page: { limit: 2 } };
    const refuse = async (command, status, reason) => {
      const before = copy(pairs.at(-1).response.session), pair = await ask(command);
      verifyWatchSourceRefusal(pair, before, status, reason); refusals.push({ request_id: pair.request.request_id, status, reason }); return pair;
    };
    const initial = await step("forward"), first = checkpoint(initial);
    const inventory = await ask({ schema: RESOURCE, operation: "query_allocations", expected_snapshot: first.anchor, page: { max_items: 16, max_scanned: 16 } });
    assert.equal(inventory.response.status, "ok"); assert.equal(inventory.response.result.allocations.length, 1);
    const allocation = inventory.response.result.allocations[0].allocation;
    assert.deepEqual(allocation, { ordinal: integer(allocation.ordinal, 1), generation: 0 });
    const registration = await ask({ operation: "set_watchpoints", watchpoints: [{ client_label: "source-first-write", enabled: true,
      allocation, byte_offset: 0, byte_len: 4, access: "write", timing: "after_commit" }] });
    const listing = await ask({ operation: "list_watchpoints", page: { limit: 16 } });
    const continuation = await ask({ operation: "continue", max_events: 65536 });
    const watchSourceRefusal = await refuse(query, "unavailable", "checkpoint_not_captured");
    const immediateControl = await step("forward");
    const immediateStack = await ask({ operation: "inspect_stack", scope: { level: "dispatch" }, page: { limit: 16 } });
    const immediateSourceRefusal = await refuse(query, "unavailable", "checkpoint_not_captured");
    const immediateMemory = await ask({ operation: "read_memory", allocation, byte_offset: 0, byte_len: 24 });
    const immediate = { control: immediateControl, stack: immediateStack, sourceRefusal: immediateSourceRefusal, memory: immediateMemory };
    verifyImmediateWatchCheckpoint(immediate);
    const collect = async (control, count, lane) => {
      checkpoint(control, count, lane);
      const stack = await ask({ operation: "inspect_stack", scope: { level: "dispatch" }, page: { limit: 16 } }), sourcePages = [];
      let cursor;
      for (let page = 0; page < LIMITS.pages; page++) {
        const pair = await ask({ ...query, page: { limit: 2, ...(cursor ? { cursor } : {}) } });
        assert.equal(pair.response.status, "ok", "producer limitation: source values unavailable at actual post-watch checkpoint");
        sourcePages.push(pair); cursor = pair.response.next_cursor; if (cursor === undefined) break;
      }
      assert.equal(cursor, undefined, "complete source page chain within bound");
      const memory = await ask({ operation: "read_memory", allocation, byte_offset: 0, byte_len: 24 });
      return { control, stack, sourcePages, memory };
    };
    const post = await collect(await step("forward", 1), 1, 1); verifyWatchSourceCheckpoint(post, AFTER_FIRST_BYTES, 1, 1);
    const oldCursor = post.sourcePages[0].response.next_cursor; assert.ok(oldCursor);
    await refuse({ ...query, selector: { selector: "name", name: "a" }, page: { limit: 2, cursor: oldCursor } }, "error", "invalid_cursor");
    const reverse = await collect(await step("reverse", 2), 2, 0); verifyWatchSourceCheckpoint(reverse, BEFORE_BYTES, 2, 0);
    await refuse({ ...query, expected_revision: post.control.response.session.revision }, "error", "stale_revision");
    await refuse({ ...query, page: { limit: 2, cursor: oldCursor } }, "error", "invalid_cursor");
    const repeat = await collect(await step("forward", 2), 2, 1);
    const groups = [post, reverse, repeat], observed = verifyWatchSourceReplay({ initial, inventory, registration, listing, continuation, watchSourceRefusal, immediate, groups });
    closing = true; assert.equal((await ask({ operation: "terminate" })).response.status, "ok"); child.stdin.end();
    assert.deepEqual(await exited, { code: 0, signal: null }); clearTimeout(timer); if (failure) throw failure;
    assert.equal(buffered, ""); assert.equal(requests.length, responses.length);
    const watchIds = [initial, inventory, registration, listing, continuation, immediate.control, immediate.memory].map(p => p.request.request_id);
    const sourceIds = groups.flatMap(g => [g.control, g.stack, ...g.sourcePages, g.memory]).map(p => p.request.request_id);
    const excerpts = [];
    for (const [prefix, ids] of [["watchpoint", watchIds], ["resource-watch-source-values", sourceIds]]) {
      const raw = originalExcerpt(ids, pairs, requests, responses);
      excerpts.push(save(prefix + ".requests.jsonl", raw.requestText), save(prefix + ".responses.jsonl", raw.responseText));
    }
    const stderrPin = save("debug-stderr.txt", stderr), observationsPin = save("observations.json", observed);
    const rawPins = ["requests", "responses"].map(name => {
      const filename = "debug-" + name + ".jsonl", pin = measure(path.join(output, filename), LIMITS[name]).pin;
      return { path: filename, bytes: pin.bytes, sha256: pin.sha256 };
    });
    for (const observation of observed.observations) assert.ok(observation.checkpoint_anchor.site.source.location.byte_end <= source.length);
    for (const pin of pins) assert.deepEqual(measure(pin.path, pin.cap).pin, { path: pin.path, bytes: pin.bytes, sha256: pin.sha256, identity: pin.identity }, "input drift");
    save("receipt.json", { status: "passed", purpose: "existing-public-watchpoint-source-variable-resource-qualification",
      source: sourcePath, source_sha256: sha(source), source_receipt: { bytes: receiptBytes.length, sha256: sha(receiptBytes), path: path.join(baseline, "receipt.json") },
      bundle_sha256: sha(bundle), bundle_identity: summary.bundle_identity, canonical_kir_digest: summary.canonical_kir_digest,
      request_sha256: sha(requestBytes), independent_result: independent, stages, selected_input_pins: pins, limits: LIMITS,
      full_pairs: pairs.length, watchpoint_request_ids: watchIds, source_request_ids: sourceIds, excerpts, refusals,
      watchpoint_source_query_request_id: watchSourceRefusal.request.request_id,
      immediate_checkpoint_source_query_request_id: immediateSourceRefusal.request.request_id,
      immediate_checkpoint_anchor: observed.immediate_watch_checkpoint.checkpoint_anchor,
      raw_transcripts: rawPins, observations: observationsPin, independent_result_file: independentPin, debugger_stderr: stderrPin,
      checkpoints: observed.observations.map(v => ({ control_request_id: v.control_request_id, checkpoint_anchor: v.checkpoint_anchor,
        source_anchor: v.source_anchor, source_request_ids: v.source_request_ids, ssa_count: v.ssa_values.length, source_count: v.source_values.length })),
      raw_line_preservation: true, watchpoint_snapshot: "unavailable_not_captured", values_at_watchpoint_stop: "not_supplied",
      immediate_postwrite_source_values: "unavailable_checkpoint_not_captured",
      source_and_memory_belong_to: "distinct_cross_invocation_operation_checkpoints_only",
      source_checkpoint_lanes: [1, 0, 1], source_control_counts: [1, 2, 2],
      frame_identity: "static_stack_depth_not_dynamic_activation", source_to_ssa_mapping: "not_supplied",
      allocation_reuse: "not_represented", dynamic_helper_activation: "not_represented", source_authentication: false,
      hardware_observed: false, performance_prediction: false, elapsed_ms: Date.now() - started });
    process.stdout.write("Actual watchpoint/source replay capture passed: " + output + "\n");
  } catch (error) {
    fail(error); clearTimeout(timer);
    if (exited && !closed) {
      let boundedTimer;
      try { await Promise.race([exited, new Promise(resolve => { boundedTimer = setTimeout(resolve, LIMITS.drainMs); })]); }
      finally { clearTimeout(boundedTimer); }
      if (!closed) { child.stdout.destroy(); child.stderr.destroy(); child.unref(); }
    }
    clearTimeout(drainTimer);
    save("failure.json", { status: "failed", detail: String(error).slice(0, 4096), debugger_exit: exitValue ?? null,
      debugger_stderr_prefix_base64: stderr.subarray(0, 8192).toString("base64"), debugger_stderr_prefix_may_be_truncated: stderr.length > 8192,
      raw_transcripts_may_be_partial: true, successful_fallback: false }, true);
    throw error;
  } finally {
    clearTimeout(timer); clearTimeout(drainTimer);
    for (const fd of Object.values(rawFiles)) fs.closeSync(fd);
  }
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  if (process.argv.length !== 5) throw new Error("usage: node resource-watch-source-values-v2-smoke.mjs REPOSITORY_DIRECTORY SOURCE_ASSEMBLY_SMOKE_DIRECTORY NEW_OUTPUT_DIRECTORY");
  await run(...process.argv.slice(2));
}
