// Synthetic pure controls only: none of these objects are execution evidence.
import assert from "node:assert/strict";
import test from "node:test";
import { AFTER_FIRST_BYTES, BEFORE_BYTES, FINAL_BYTES, LIMITS, originalExcerpt,
  verifyImmediateWatchCheckpoint, verifyIndependentWatchResult, verifyWatchSourceCheckpoint, verifyWatchSourceRefusal,
  verifyWatchSourceReplay } from "./resource-watch-source-values-v2-smoke.mjs";

const V1 = "fe2o3-debug-request-v1", V2 = "fe2o3-debug-source-variable-request-v2";
const RESOURCE = "fe2o3-debug-resource-request-v1";
const responseSchema = s => s === V2 ? "fe2o3-debug-source-variable-response-v2"
  : s === RESOURCE ? "fe2o3-debug-resource-response-v1" : "fe2o3-debug-response-v1";
const clone = structuredClone, h = n => n.toString(16).padStart(64, "0");
const allocation = { ordinal: 1, generation: 0 };
function stopped(event, revision) {
  return { backend: "cpu_kir_simulator", execution_kind: "cpu_kir_simulation", state: "stopped",
    revision, configuration_identity: h(1), cursor: { configuration_identity: h(1), event_sequence: event,
      state_revision: revision }, simulated: true, hardware_observed: false, performance_prediction: false };
}
function anchor(event, revision, block = 1, lane = 0) {
  return { cursor: stopped(event, revision).cursor, scope: { level: "lane", workgroup: [0, 0, 0], wave: 0, lane,
    logical_workitem: [lane, 0, 0], active_mask: 15, wave_width: 32, interpretation: "logical_visualization" },
    site: { kir: { function_ordinal: 0, block_ordinal: block, point: { kind: "operation", operation_ordinal: 0 } },
      source: { status: "resolved", location: { map_identity: h(2), file_identity: h(3),
        provenance: "compiler_bundle_bound", byte_start: 100, byte_end: 110 } } } };
}
const scalar = bits => ({ status: "captured", value_type: { kind: "integer", signed: false, bits: 32 },
  value: { encoding: "bits", bits }, provenance: "simulated_observation" });
function pair(id, operation, session, fields, result, schema = V1) {
  return { request: { schema, request_id: id, expected_revision: session.revision, operation, ...fields },
    response: { status: "ok", schema: responseSchema(schema), request_id: id, operation, session: clone(session), result } };
}
function control(id, event, revision, direction, advanced, prewrite = false, block = 1, count = 1, lane = 0) {
  const stop = { reason: "step", outcome: "active", exact: true };
  const p = pair(id, "step", stopped(event, revision),
    { expected_revision: revision - 1, direction, granularity: "operation", count },
    { result: "control", stop, snapshot: { status: "captured", snapshot: { anchor: anchor(event, revision, block, lane),
      stop: clone(stop), values: ["0xfffffff0", "0x00000025", prewrite ? "0x00000000" : "0x000001d5"].map((bits, value_ordinal) =>
        ({ path: { root: { kind: "ssa", function_ordinal: 0, frame: 1, value_ordinal }, components: [] },
          availability: scalar(bits) })) } }, events_advanced: advanced });
  return p;
}
function group(id, event, revision, direction, advanced, prewrite = false, count = 1, lane = 0) {
  const block = lane === 1 ? 0 : 1;
  const cp = control(id, event, revision, direction, advanced, prewrite, block, count, lane), session = cp.response.session;
  const snapshot = cp.response.result.snapshot.snapshot.anchor, framed = { ...clone(snapshot), frame: 1, occurrence: 1 };
  const stack = pair(id + 1, "inspect_stack", session, { scope: { level: "dispatch" }, page: { limit: 16 } },
    { result: "stack", snapshot: clone(snapshot), frames: [{ frame: 1, function_ordinal: 0, block_ordinal: block,
      next_operation: prewrite || lane === 1 ? 0 : 1, values: { status: "captured", value_count: 3 } }] });
  const rows = ["out", "a", "b", "result"].map((name, i) => ({ variable_identity: h(10 + i), name, function_ordinal: 0,
    scope_identity: h(20), scope_depth: 0, generation: ["a", "b"].includes(name) ? 1 : 0,
    availability: { status: "value", value: name === "a" ? scalar("0xfffffff0") : name === "b" ? scalar("0x00000025")
      : { status: "unavailable", reason: "not_represented" } } }));
  const cursor = { query_identity: h(100 + revision), position: 2 };
  const sourcePages = [0, 1].map(index => {
    const p = pair(id + 2 + index, "inspect_source_variables", session, { scope: { level: "dispatch" }, frame: 1,
      selector: { selector: "all" }, page: { limit: 2, ...(index ? { cursor: clone(cursor) } : {}) } }, undefined, V2);
    delete p.response.result;
    Object.assign(p.response, { snapshot: clone(framed), values: clone(rows.slice(index * 2, index * 2 + 2)),
      ...(!index ? { next_cursor: clone(cursor) } : {}) });
    return p;
  });
  const memory = pair(id + 4, "read_memory", session, { allocation: clone(allocation), byte_offset: 0, byte_len: 24 },
    { result: "memory", snapshot: clone(snapshot), memory: { allocation: clone(allocation), byte_offset: 0,
      requested_bytes: 24, returned_bytes: 24, availability: { status: "captured", address_space: "global",
        bytes: prewrite ? BEFORE_BYTES : AFTER_FIRST_BYTES, initialized: "0xffffff", truncated: false } } });
  return { control: cp, stack, sourcePages, memory };
}
function unavailablePair(id, session) {
  const p = pair(id, "inspect_source_variables", session, { scope: { level: "dispatch" }, frame: 1,
    selector: { selector: "all" }, page: { limit: 2 } }, undefined, V2);
  delete p.response.result; p.response.status = "unavailable"; p.response.reason = "checkpoint_not_captured"; return p;
}
function fixture() {
  const initial = control(1, 1, 1, "forward", 1, true, 0), s = initial.response.session;
  const inventory = pair(2, "query_allocations", s, { expected_snapshot: clone(initial.response.result.snapshot.snapshot.anchor),
    page: { max_items: 16, max_scanned: 16 } }, { result: "allocations", allocations: [{ allocation: clone(allocation),
      address_space: "global", access: "read_write", alignment: 4, capacity_bytes: "24", snapshot_bytes_available: true,
      initialization_available: true, owning_scope: "not_represented", lifetime: "not_represented", physical_base: "not_represented" }] }, RESOURCE);
  Object.assign(inventory.response, { snapshot: clone(initial.response.result.snapshot.snapshot.anchor),
    page: { source_count: 1, scanned: 1, completeness: { status: "complete" } }, physical_registers: "not_represented" });
  const spec = { client_label: "source-first-write", enabled: true, allocation: clone(allocation),
    byte_offset: 0, byte_len: 4, access: "write", timing: "after_commit" };
  const registration = pair(3, "set_watchpoints", stopped(1, 2), { expected_revision: 1, watchpoints: [spec] },
    { result: "acknowledged", accepted: 1 });
  const listing = pair(4, "list_watchpoints", stopped(1, 2), { page: { limit: 16 } },
    { result: "watchpoints", watchpoints: [{ watchpoint_id: 5, spec: clone(spec), hit_count: 0 }] });
  const continuation = pair(5, "continue", stopped(10, 3), { expected_revision: 2, max_events: 65536 },
    { result: "control", stop: { reason: "watchpoint", watchpoint_id: 5, outcome: "active", exact: true },
      snapshot: { status: "unavailable", reason: "not_captured" }, events_advanced: 9 });
  const terminal = group(7, 11, 4, "forward", 1);
  delete terminal.stack.response.result.frames[0].next_operation;
  terminal.memory.request.request_id = 10; terminal.memory.response.request_id = 10;
  const immediate = { control: terminal.control, stack: terminal.stack,
    sourceRefusal: unavailablePair(9, terminal.control.response.session), memory: terminal.memory };
  return { initial, inventory, registration, listing, continuation,
    watchSourceRefusal: unavailablePair(6, continuation.response.session), immediate,
    groups: [group(11, 12, 5, "forward", 1, false, 1, 1), group(19, 9, 6, "reverse", 3, true, 2, 0),
      group(27, 12, 7, "forward", 3, false, 2, 1)] };
}
const selected = f => [f.initial, f.inventory, f.registration, f.listing, f.continuation, f.watchSourceRefusal,
  f.immediate.control, f.immediate.stack, f.immediate.sourceRefusal, f.immediate.memory,
  ...f.groups.flatMap(g => [g.control, g.stack, ...g.sourcePages, g.memory])];
function refuses(mutate) { const f = fixture(); mutate(f); assert.throws(() => verifyWatchSourceReplay(f)); }
function refusal(status = "error", reason = "stale_revision") {
  const session = status === "unavailable" ? stopped(10, 3) : stopped(9, 5);
  const p = pair(19, "inspect_source_variables", session, { expected_revision: reason === "stale_revision" ? 4 : session.revision,
    scope: { level: "dispatch" }, frame: 1, selector: { selector: "all" },
    page: { limit: 2, ...(reason === "invalid_cursor" ? { cursor: { query_identity: h(555), position: 2 } } : {}) } }, undefined, V2);
  delete p.response.result; p.response.status = status;
  if (status === "error") p.response.error = { stage: "session", code: reason, message: "synthetic refusal only", state_changed: false };
  else p.response.reason = reason;
  return { p, session };
}

test("synthetic explicit lane1/lane0/lane1 replay separates both source-unavailable stops", () => {
  const f = fixture(), before = clone(f), result = verifyWatchSourceReplay(f);
  assert.deepEqual(f, before); assert.equal(result.watchpoint_snapshot, "unavailable_not_captured");
  assert.equal(result.source_and_memory_belong_to, "distinct_cross_invocation_operation_checkpoints_only");
  assert.equal(result.immediate_postwrite_source_values, "unavailable_checkpoint_not_captured");
  assert.deepEqual(result.source_checkpoint_lanes, [1, 0, 1]); assert.deepEqual(result.source_control_counts, [1, 2, 2]);
  assert.notDeepEqual(result.watchpoint.checkpoint_anchor, result.observations[0].checkpoint_anchor);
  assert.equal(result.immediate_watch_checkpoint.source_values, "unavailable_checkpoint_not_captured");
  assert.notEqual(result.watchpoint.stop_cursor.event_sequence, result.observations[0].checkpoint_anchor.cursor.event_sequence);
  assert.equal(result.observations[0].source_anchor.frame, 1);
  assert.equal(Object.hasOwn(result.observations[0].checkpoint_anchor, "frame"), false);
  assert.equal(result.source_to_ssa_mapping, "not_supplied");
  result.observations[0].source_values[0].name = "changed";
  assert.deepEqual(f, before);
});
test("refuses stale session revision and independently changed configuration", () => {
  refuses(f => { f.groups[0].sourcePages[0].request.expected_revision--; });
  refuses(f => { const s = f.groups[0].sourcePages[0].response.session; s.configuration_identity = h(200); s.cursor.configuration_identity = h(200); });
});
test("refuses same-session cross-stop anchors and mixed old source page responses", () => {
  refuses(f => { f.groups[2].sourcePages[0].response.snapshot = clone(f.groups[0].sourcePages[0].response.snapshot); });
  refuses(f => { const old = clone(f.groups[0].sourcePages[0]); old.request.request_id = f.groups[2].sourcePages[0].request.request_id; old.response.request_id = old.request.request_id; f.groups[2].sourcePages[0] = old; });
});
test("refuses fabricated source authentication, authority fields and hardware claims", () => {
  refuses(f => { f.groups[0].sourcePages[0].response.source_authenticated = true; });
  refuses(f => { f.registration.response.authority = "production"; });
  refuses(f => { f.inventory.response.physical_registers = "captured"; });
  refuses(f => { f.groups[0].control.response.result.snapshot.snapshot.anchor.site.source.location.provenance = "authenticated"; });
  refuses(f => { f.groups[0].memory.response.session.hardware_observed = true; });
  refuses(f => { f.groups[0].sourcePages[0].response.values[0].ssa_value_ordinal = 0; });
});
test("requires unmodified frame-refined source anchors and exact represented scalar bits", () => {
  refuses(f => { delete f.groups[0].sourcePages[0].response.snapshot.frame; });
  refuses(f => { f.groups[0].sourcePages[0].response.snapshot.occurrence = 2; });
  refuses(f => { f.groups[0].sourcePages[0].request.frame = 2; });
  refuses(f => { f.groups[0].sourcePages[0].response.values[1].availability.value.value.bits = "0xfffffff1"; });
  refuses(f => { f.groups[0].sourcePages[0].response.values[1].generation = 0; });
});
test("complete opaque page chain refuses missing, duplicate, changed selector and changed cursor", () => {
  refuses(f => { f.groups[0].sourcePages.pop(); });
  refuses(f => { f.groups[0].sourcePages[1].response.values[0] = clone(f.groups[0].sourcePages[0].response.values[0]); });
  refuses(f => { f.groups[0].sourcePages[1].request.selector = { selector: "name", name: "a" }; });
  refuses(f => { f.groups[0].sourcePages[1].request.page.cursor.query_identity = h(300); });
  refuses(f => { f.groups[0].sourcePages[0].response.next_cursor.position = 3; });
  refuses(f => { f.groups[0].sourcePages[1].response.next_cursor = { query_identity: h(104), position: 4 }; });
});
test("keeps original counts 1/2/2, exact complete SSA rows, stack count and next operation", () => {
  refuses(f => { f.groups[0].control.request.count = 2; });
  refuses(f => { f.groups[1].control.request.count = 1; });
  refuses(f => { f.groups[2].control.request.count = 1; });
  refuses(f => { f.groups[0].stack.response.result.frames[0].values.value_count++; });
  refuses(f => { delete f.groups[0].stack.response.result.frames[0].next_operation; });
  refuses(f => { const rows = f.groups[0].control.response.result.snapshot.snapshot.values; rows[1].path.root.value_ordinal = rows[0].path.root.value_ordinal; });
  refuses(f => { f.groups[0].control.response.result.snapshot.snapshot.values[0].path.root.frame = 2; });
});
test("cannot attribute checkpoint values to uncaptured watchpoint or replay wrong events", () => {
  refuses(f => { f.continuation.response.result.snapshot = clone(f.groups[0].control.response.result.snapshot); });
  refuses(f => { f.continuation.response.result.stop.watchpoint_id = 6; });
  refuses(f => { f.groups[1].control.response.result.events_advanced = 1; });
  refuses(f => { f.groups[2].control.response.result.events_advanced = 1; });
});
test("requires actual reverse prewrite and exact repeat source/SSA/stack/memory restoration", () => {
  refuses(f => { f.groups[1].memory.response.result.memory.availability.bytes = AFTER_FIRST_BYTES; });
  refuses(f => { f.groups[2].control.response.result.snapshot.snapshot.values[2].availability.value.bits = "0x000001d6"; });
  refuses(f => { f.groups[2].sourcePages[1].response.values[1].availability.value.reason = "not_in_scope"; });
  refuses(f => { f.groups[2].stack.response.result.frames[0].next_operation = 2; });
});
test("bounds and generation-zero allocation inventory fail closed", () => {
  refuses(f => { f.inventory.response.page.completeness.status = "partial"; });
  refuses(f => { f.inventory.response.page.next_token = "extra"; });
  refuses(f => { f.inventory.response.result.allocations[0].snapshot_bytes_available = false; });
  refuses(f => { f.groups[0].memory.request.allocation.generation = 1; });
  refuses(f => { f.groups[0].memory.response.result.memory.returned_bytes = 16; });
  refuses(f => { f.groups[0].memory.response.result.memory.availability.initialized = "0x000000"; });
  const g = fixture().groups[0]; g.control.response.result.snapshot.snapshot.values = Array.from({ length: LIMITS.values + 1 },
    (_, i) => ({ path: { root: { kind: "ssa", function_ordinal: 0, frame: 1, value_ordinal: i }, components: [] }, availability: scalar("0x00000000") }));
  assert.throws(() => verifyWatchSourceCheckpoint(g, AFTER_FIRST_BYTES, 1, 1));
  const huge = fixture().groups[0]; huge.sourcePages = Array(LIMITS.pages + 1).fill(huge.sourcePages[0]);
  assert.throws(() => verifyWatchSourceCheckpoint(huge, AFTER_FIRST_BYTES, 1, 1));
});
test("actual-refusal checker retains unavailable and errors without session mutation or stale values", () => {
  for (const [status, reason] of [["unavailable", "checkpoint_not_captured"], ["error", "stale_revision"], ["error", "invalid_cursor"]]) {
    const { p, session } = refusal(status, reason);
    assert.deepEqual(verifyWatchSourceRefusal(p, session, status, reason), p);
    const altered = clone(p); altered.response.session.cursor.event_sequence++;
    assert.throws(() => verifyWatchSourceRefusal(altered, session, status, reason));
    const withRows = clone(p); withRows.response.values = [];
    assert.throws(() => verifyWatchSourceRefusal(withRows, session, status, reason));
  }
  const { p, session } = refusal(); p.response.error.state_changed = true;
  assert.throws(() => verifyWatchSourceRefusal(p, session, "error", "stale_revision"));
});
test("legacy seven-pair excerpt preserves original whitespace and original IDs", () => {
  const f = fixture(), pairs = selected(f), req = pairs.map(p => JSON.stringify(p.request, null, 0).replaceAll('":', '": ') + "\n");
  const resp = pairs.map(p => JSON.stringify(p.response).replaceAll('":', '": ') + "\n");
  const seven = [f.initial, f.inventory, f.registration, f.listing, f.continuation, f.immediate.control, f.immediate.memory];
  const ids = seven.map(p => p.request.request_id), excerpt = originalExcerpt(ids, pairs, req, resp);
  assert.equal(excerpt.requestText, ids.map(id => req[pairs.findIndex(p => p.request.request_id === id)]).join(""));
  assert.equal(excerpt.responseText, ids.map(id => resp[pairs.findIndex(p => p.request.request_id === id)]).join(""));
  assert.deepEqual(seven.map(p => p.request.operation), ["step", "query_allocations", "set_watchpoints", "list_watchpoints", "continue", "step", "read_memory"]);
  const sourceIds = f.groups.flatMap(g => [g.control, g.stack, ...g.sourcePages, g.memory]).map(p => p.request.request_id);
  assert.equal(ids.some(id => sourceIds.includes(id)), false, "separate excerpt identities joined only through full session custody");
});
test("excerpt refuses duplicate IDs, missing pairs, changed originals and non-success selected rows", () => {
  const f = fixture(), pairs = selected(f), req = pairs.map(p => JSON.stringify(p.request) + "\n"), resp = pairs.map(p => JSON.stringify(p.response) + "\n");
  assert.throws(() => originalExcerpt([1, 1], pairs, req, resp));
  assert.throws(() => originalExcerpt([128], pairs, req, resp));
  assert.throws(() => originalExcerpt([1], [...pairs, pairs.at(-1)], [...req, req.at(-1)], [...resp, resp.at(-1)]));
  const changed = [...req]; changed[0] = changed[0].replace('"count":1', '"count":2');
  assert.throws(() => originalExcerpt([1], pairs, changed, resp));
  const bad = clone(pairs); bad[0].response.status = "unavailable";
  assert.throws(() => originalExcerpt([1], bad, req, bad.map(p => JSON.stringify(p.response) + "\n")));
});
test("independent whole-kernel oracle checks all four words, canary, scalar arguments and CPU-only status", () => {
  const value = { schema: "fe2o3-simulation-result-v1", status: "ok", authority: "observation_only", simulated: true,
    hardware_observed: false, hardware_validation: false, performance_prediction: false, kir: { sha256: h(90) },
    counts: { invocations_executed: 4 }, arguments: [{ kind: "buffer", value: { element: "u32", access: "read_write",
      alignment: 4, bytes: FINAL_BYTES, initialized: "0xffffff" } }, { kind: "scalar", type: "u32", bits: "0xfffffff0" },
      { kind: "scalar", type: "u32", bits: "0x00000025" }], shared_buffers: [] };
  assert.deepEqual(verifyIndependentWatchResult(value, h(90)), { expected_u32: 469, output_words: 4, tail_canary_unchanged: true, hardware_observed: false });
  for (const change of [v => { v.arguments[0].value.bytes = AFTER_FIRST_BYTES; },
    v => { v.arguments[0].value.bytes = FINAL_BYTES.slice(0, -1) + "0"; }, v => { v.hardware_validation = true; },
    v => { v.authority = "proof"; }, v => { v.counts.invocations_executed = 1; }, v => { v.kir.sha256 = h(91); }]) {
    const bad = clone(value); change(bad); assert.throws(() => verifyIndependentWatchResult(bad, h(90)));
  }
});

test("both unavailable source queries refuse smuggled values and changed stopped-session identity", () => {
  for (const get of [f => f.watchSourceRefusal, f => f.immediate.sourceRefusal]) {
    refuses(f => { get(f).response.values = []; });
    refuses(f => { get(f).response.status = "ok"; });
    refuses(f => { get(f).response.reason = "variables_not_captured"; });
    refuses(f => { get(f).response.session.cursor.event_sequence++; });
  }
  refuses(f => { f.immediate.stack.response.result.frames[0].next_operation = 0; });
  const i = fixture().immediate; assert.equal(verifyImmediateWatchCheckpoint(i).source_values, "unavailable_checkpoint_not_captured");
});
test("cannot replace immediate seven-pair control or memory with the later lane1 source checkpoint", () => {
  refuses(f => { f.immediate.control = clone(f.groups[0].control); });
  refuses(f => { f.immediate.memory = clone(f.groups[0].memory); });
  refuses(f => { f.groups[0].memory.response.result.snapshot = clone(f.immediate.control.response.result.snapshot.snapshot.anchor); });
  refuses(f => { f.groups[0].sourcePages[0].response.snapshot = {
    ...clone(f.immediate.control.response.result.snapshot.snapshot.anchor), frame: 1, occurrence: 1 }; });
});
test("cross-invocation scope is explicit and cannot silently collapse to lane0", () => {
  refuses(f => {
    const g = f.groups[0], wrong = clone(f.immediate.control.response.result.snapshot.snapshot.anchor.scope);
    g.control.response.result.snapshot.snapshot.anchor.scope = clone(wrong);
    g.stack.response.result.snapshot.scope = clone(wrong);
    for (const p of g.sourcePages) p.response.snapshot.scope = clone(wrong);
    g.memory.response.result.snapshot.scope = clone(wrong);
  });
  refuses(f => {
    const g = f.groups[1]; g.control.response.result.snapshot.snapshot.anchor.scope.logical_workitem = [1, 0, 0];
  });
});
test("later source checkpoint must remain lane1 before its first operation", () => {
  refuses(f => { f.groups[0].stack.response.result.frames[0].next_operation = 1; });
  refuses(f => { f.groups[1].stack.response.result.frames[0].next_operation = 1; });
  refuses(f => {
    const g = f.groups[0]; g.control.response.result.snapshot.snapshot.anchor.site.kir.block_ordinal = 1;
    g.stack.response.result.snapshot.site.kir.block_ordinal = 1; g.stack.response.result.frames[0].block_ordinal = 1;
    for (const p of g.sourcePages) p.response.snapshot.site.kir.block_ordinal = 1;
    g.memory.response.result.snapshot.site.kir.block_ordinal = 1;
  });
});
