// Synthetic pure assertion controls, never source/backend qualification.
import assert from "node:assert/strict";
import test from "node:test";
import { verifySourceValueCheckpoint, verifySourceValueReplay } from "./resource-source-values-v2-smoke.mjs";

const bytes = "0x" + "a5a5a5a5".repeat(4) + "deadbeefcafebabe";
const digest = digit => digit.repeat(64);
const scalar = bits => ({ status: "captured", value_type: { kind: "integer", signed: false, bits: 32 },
  value: { encoding: "bits", bits }, provenance: "simulated_observation" });
function fixture({ event = 3, revision = 2, id = 3, direction = "forward", count = 4 } = {}) {
  const session = { backend: "cpu_kir_simulator", execution_kind: "cpu_kir_simulation", state: "stopped", revision,
    configuration_identity: digest("1"), cursor: { configuration_identity: digest("1"), event_sequence: event, state_revision: revision },
    simulated: true, hardware_observed: false, performance_prediction: false };
  const anchor = { cursor: session.cursor,
    scope: { level: "lane", workgroup: [0, 0, 0], wave: 0, lane: 0, logical_workitem: [0, 0, 0],
      active_mask: 15, wave_width: 32, interpretation: "logical_visualization" },
    site: { kir: { function_ordinal: 0, block_ordinal: 0, point: { kind: "operation", operation_ordinal: 0 } },
      source: { status: "resolved", location: { map_identity: digest("2"), file_identity: digest("3"),
        provenance: "compiler_bundle_bound", byte_start: 377, byte_end: 402 } } } };
  const ssa = Array.from({ length: count }, (_, value_ordinal) => ({ path: { root: { kind: "ssa", function_ordinal: 0, frame: 1, value_ordinal }, components: [] }, availability: scalar("0x00000025") }));
  const stop = { reason: "step", outcome: "active", exact: true };
  const pair = (operation, requestFields, result) => ({
    request: { schema: "fe2o3-debug-request-v1", request_id: id++, expected_revision: revision, operation, ...requestFields },
    response: { status: "ok", schema: "fe2o3-debug-response-v1", request_id: id - 1, operation, session: structuredClone(session), result: structuredClone(result) },
  });
  const control = pair("step", { expected_revision: revision - 1, direction, granularity: "operation", count: 2 },
    { result: "control", stop, snapshot: { status: "captured", snapshot: { anchor, stop, values: ssa } }, events_advanced: 2 });
  const stack = pair("inspect_stack", { scope: { level: "dispatch" }, page: { limit: 16 } },
    { result: "stack", snapshot: anchor, frames: [{ frame: 1, function_ordinal: 0, block_ordinal: 0,
      next_operation: event === 1 ? 0 : 1, values: { status: "captured", value_count: count } }] });
  const vars = [["a", "4", scalar("0xfffffff0")], ["b", "5", scalar("0x00000025")],
    ["out", "6", { status: "unavailable", reason: "not_represented" }], ["result", "7", { status: "unavailable", reason: "not_represented" }]]
    .map(([name, identity, value]) => ({ variable_identity: digest(identity), name, function_ordinal: 0,
      scope_identity: digest("8"), scope_depth: 0, generation: value.status === "captured" ? 1 : 0,
      availability: { status: "value", value } }));
  const cursor = { query_identity: digest("9"), position: 2 };
  const sourcePages = [0, 1].map(index => {
    const request_id = id++;
    return { request: { schema: "fe2o3-debug-source-variable-request-v2", request_id, expected_revision: revision,
      operation: "inspect_source_variables", scope: { level: "dispatch" }, frame: 1, selector: { selector: "all" },
      page: { limit: 2, ...(index ? { cursor: structuredClone(cursor) } : {}) } },
    response: { status: "ok", schema: "fe2o3-debug-source-variable-response-v2", request_id, operation: "inspect_source_variables",
      session: structuredClone(session), snapshot: structuredClone({ ...anchor, frame: 1, occurrence: 1 }),
      values: structuredClone(vars.slice(index * 2, index * 2 + 2)), ...(index ? {} : { next_cursor: cursor }) } };
  });
  const allocation = { ordinal: 1, generation: 0 };
  const memory = pair("read_memory", { allocation, byte_offset: 0, byte_len: 24 },
    { result: "memory", snapshot: anchor, memory: { allocation, byte_offset: 0, requested_bytes: 24, returned_bytes: 24,
      availability: { status: "captured", address_space: "global", bytes, initialized: "0xffffff", truncated: false } } });
  return structuredClone({ control, stack, sourcePages, memory });
}
function groups() {
  return [fixture(), fixture({ event: 1, revision: 3, id: 10, direction: "reverse", count: 3 }),
    fixture({ event: 3, revision: 4, id: 20 })];
}
function rejects(mutations) {
  for (const mutate of mutations) { const value = fixture(); mutate(value); assert.throws(() => verifySourceValueCheckpoint(value, bytes)); }
}

test("preserves the distinct complete checkpoint and framed source anchors", () => {
  const input = fixture(), original = structuredClone(input), result = verifySourceValueCheckpoint(input, bytes);
  assert.equal(Object.hasOwn(result.checkpoint_anchor, "frame"), false);
  assert.equal(result.source_variable_anchor.frame, 1); assert.equal(result.source_variable_anchor.occurrence, 1);
  assert.equal(result.stack_frame.next_operation, 1); assert.equal(result.checkpoint_anchor.site.kir.point.operation_ordinal, 0);
  assert.equal(result.source_to_ssa_mapping, "not_supplied"); assert.equal(result.hardware_observed, false);
  assert.deepEqual(input, original); result.source_variable_anchor.cursor.event_sequence = 999;
  result.values[0].name = "changed"; assert.deepEqual(input, original, "outputs do not alias retained inputs");
});
test("actual reverse can restore fewer SSA values; repeat must restore exact original state", () => {
  const result = verifySourceValueReplay(groups(), bytes);
  assert.equal(result.observations[0].ssa_values.length, 4); assert.equal(result.observations[1].ssa_values.length, 3);
  assert.equal(result.observations[2].ssa_values.length, 4);
  assert.equal(result.observations[0].checkpoint_anchor.cursor.event_sequence, result.observations[2].checkpoint_anchor.cursor.event_sequence);
  assert.notEqual(result.observations[0].checkpoint_anchor.cursor.state_revision, result.observations[2].checkpoint_anchor.cursor.state_revision);
});
test("refuses optional-field stripping and fabricated checkpoint frames", () => rejects([
  x => delete x.sourcePages[0].response.snapshot.frame,
  x => delete x.sourcePages[0].response.snapshot.occurrence,
  x => x.sourcePages[0].response.snapshot.frame = 2,
  x => x.sourcePages[0].response.snapshot.occurrence = 2,
  x => x.sourcePages[0].response.snapshot.frame = null,
  x => x.control.response.result.snapshot.snapshot.anchor.frame = 1,
  x => x.control.response.result.snapshot.snapshot.anchor.occurrence = 1,
]));
test("requires a complete independently captured top-level stack", () => rejects([
  x => x.stack.response.result.frames = [],
  x => x.stack.response.result.frames.push(structuredClone(x.stack.response.result.frames[0])),
  x => x.stack.response.result.next_cursor = { query_identity: digest("9"), position: 1 },
  x => x.stack.response.result.frames[0].frame = 2,
  x => x.stack.response.result.frames[0].function_ordinal = 1,
  x => x.stack.response.result.frames[0].block_ordinal = 3,
  x => delete x.stack.response.result.frames[0].next_operation,
  x => x.stack.response.result.frames[0].values.value_count++,
  x => x.stack.response.result.frames[0].values = { status: "unavailable", reason: "truncated" },
]));
test("requires exact read-only stop identity on every query", () => {
  for (const name of ["stack", "memory"]) rejects([
    x => x[name].request.expected_revision--,
    x => x[name].response.session.revision++,
    x => x[name].response.session.cursor.event_sequence++,
    x => x[name].response.session.cursor.configuration_identity = digest("f"),
    x => x[name].response.session.hardware_observed = true,
  ]);
  rejects([
    x => x.sourcePages[0].request.expected_revision--,
    x => x.sourcePages[0].response.session.cursor.state_revision++,
    x => x.sourcePages[0].response.session.simulated = false,
    x => x.sourcePages[0].response.snapshot.scope.lane = 1,
    x => x.sourcePages[0].response.snapshot.site.source.location.byte_start++,
    x => x.sourcePages[0].response.snapshot.site.source.location.map_identity = digest("f"),
    x => x.stack.response.result.snapshot.site.kir.block_ordinal++,
    x => x.memory.response.result.snapshot.cursor.event_sequence++,
  ]);
});
test("retains the full source page chain with unchanged selector, frame and query", () => rejects([
  x => x.sourcePages.pop(),
  x => x.sourcePages[0].request.page.cursor = { query_identity: digest("9"), position: 0 },
  x => delete x.sourcePages[1].request.page.cursor,
  x => x.sourcePages[1].request.page.cursor.position++,
  x => x.sourcePages[1].request.page.cursor.query_identity = digest("f"),
  x => x.sourcePages[1].request.selector = { selector: "name", name: "a" },
  x => x.sourcePages[1].request.frame = 2,
  x => x.sourcePages[1].request.page.limit = 64,
  x => x.sourcePages[1].response.values.push(structuredClone(x.sourcePages[0].response.values[0])),
  x => x.sourcePages[1].response.values[0].variable_identity = x.sourcePages[0].response.values[0].variable_identity,
  x => x.sourcePages[0].response.next_cursor.position = 3,
]));
test("keeps observed parameters and unsupported locals separate", () => rejects([
  x => x.sourcePages[0].response.values[0].generation = 0,
  x => x.sourcePages[0].response.values[0].availability.value.value.bits = "0x00000000",
  x => x.sourcePages[0].response.values[0].availability.value.provenance = "hardware_observation",
  x => x.sourcePages[0].response.values[1].name = "a",
  x => x.sourcePages[1].response.values[0].generation = 1,
  x => x.sourcePages[1].response.values[0].availability = { status: "value", value: scalar("0x00000000") },
  x => x.sourcePages[1].response.values[1].availability.value.reason = "not_captured",
  x => x.sourcePages[1].response.values[1].name = "another_local",
]));
test("rejects unsafe metadata and duplicate SSA identity without inferring source linkage", () => rejects([
  x => x.control.response.result.snapshot.snapshot.values[0].path.root.value_ordinal = Number.MAX_SAFE_INTEGER + 1,
  x => x.control.response.result.snapshot.snapshot.values[0].path.root.frame = 2,
  x => x.control.response.result.snapshot.snapshot.values[0].path.components = [{ kind: "field", index: 0 }],
  x => x.control.response.result.snapshot.snapshot.values[1].path.root.value_ordinal = 0,
  x => x.sourcePages[0].response.values[0].variable_identity = digest("0"),
  x => x.sourcePages[0].response.values[0].scope_depth = 5000,
  x => x.sourcePages[0].response.values[0].name = "a\u0000",
  x => x.sourcePages[0].response.values[0].extra = true,
]));
test("memory must be same-stop exact retained initial storage and canaries", () => rejects([
  x => x.memory.request.allocation.generation = 1,
  x => x.memory.request.byte_offset = 4,
  x => x.memory.request.byte_len = 4,
  x => x.memory.response.result.memory.returned_bytes = 4,
  x => x.memory.response.result.memory.availability.initialized = "0x000000",
  x => x.memory.response.result.memory.availability.bytes = "0x" + "00".repeat(24),
  x => x.memory.response.result.memory.availability.truncated = true,
]));
test("control captures remain exact independent steps, never inferred watchpoint snapshots", () => rejects([
  x => x.control.response.result.snapshot = { status: "unavailable", reason: "not_captured" },
  x => x.control.response.result.stop.reason = "watchpoint",
  x => x.control.response.result.snapshot.snapshot.stop.exact = false,
  x => x.control.request.granularity = "source",
  x => x.control.request.count = 3,
  x => x.control.response.result.snapshot.snapshot.anchor.site.source.location.provenance = "caller_bound",
]));
test("rejects response correlation errors and nonmonotone retained pairs", () => {
  for (const name of ["control", "stack", "memory"]) rejects([
    x => x[name].response.request_id++,
    x => x[name].response.schema = "different",
    x => x[name].response.operation = "get_state",
    x => x[name].response.status = "unavailable",
  ]);
  rejects([x => { x.memory.request.request_id = 3; x.memory.response.request_id = 3; }]);
});
test("rejects wrong replay order, event and repeat state despite individually valid groups", () => {
  for (const mutate of [
    value => value.reverse(),
    value => value[1].control.request.direction = "forward",
    value => value[2].control.response.result.snapshot.snapshot.values[0].availability.value.bits = "0x00000026",
    value => value[2].sourcePages[0].response.values[0].scope_identity = digest("f"),
  ]) { const value = groups(); mutate(value); assert.throws(() => verifySourceValueReplay(value, bytes)); }
});
