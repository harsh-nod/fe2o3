import assert from "node:assert/strict";
import test from "node:test";
import { verifyHelperSourceCheckpoint, verifyHelperSourceReplay, verifyHelperResult } from "./resource-helper-source-values-v2-smoke.mjs";

// Hand-built protocol-shape checks only. These are not an executed Rust fixture,
// an authenticated source map, or evidence for any real helper activation.
const initialBytes = "0x" + "a5a5a5a5".repeat(4) + "deadbeefcafebabe";
const digest = character => character.repeat(64);
const clone = value => structuredClone(value);
function scalar(bits = "0x3f800000") {
  return { status: "captured", value_type: { kind: "float", bits: 32 },
    value: { encoding: "bits", bits }, provenance: "simulated_observation" };
}
function syntheticGroup(revision = 2, event = 20, direction = "forward", firstId = 10, extra = true) {
  const cursor = { configuration_identity: digest("a"), event_sequence: event, state_revision: revision };
  const session = { backend: "cpu_kir_simulator", execution_kind: "cpu_kir_simulation", state: "stopped",
    revision, configuration_identity: digest("a"), cursor, simulated: true, hardware_observed: false, performance_prediction: false };
  const anchor = { cursor, scope: { level: "lane", workgroup: [0, 0, 0], wave: 0, lane: 0,
    logical_workitem: [0, 0, 0], active_mask: 15, wave_width: 32, interpretation: "logical_visualization" },
    site: { kir: { function_ordinal: 7, block_ordinal: 0, point: { kind: "operation", operation_ordinal: 0 } },
      source: { status: "resolved", location: { map_identity: digest("b"), file_identity: digest("c"),
        provenance: "compiler_bundle_bound", byte_start: 10, byte_end: 20 } } } };
  const sourceAnchor = { ...clone(anchor), frame: 2, occurrence: 1 };
  let nextId = firstId;
  function pair(operation, fields, result, schema = "fe2o3-debug-request-v1") {
    const id = nextId++, request = { schema, request_id: id, expected_revision: revision, operation, ...fields };
    const response = { status: "ok", schema: schema.replace("request", "response"), request_id: id, operation,
      session: clone(session), ...result };
    return { request, response };
  }
  const ssa = [
    { path: { root: { kind: "ssa", function_ordinal: 0, frame: 1, value_ordinal: 10 }, components: [] }, availability: scalar() },
    { path: { root: { kind: "ssa", function_ordinal: 7, frame: 2, value_ordinal: 0 }, components: [] }, availability: scalar() },
    ...(extra ? [{ path: { root: { kind: "ssa", function_ordinal: 7, frame: 2, value_ordinal: 1 }, components: [] }, availability: scalar() }] : []),
  ];
  const stop = { reason: "step", outcome: "active", exact: true };
  const control = pair("step", { expected_revision: revision - 1, direction, granularity: "operation", count: 1 },
    { result: { result: "control", stop, snapshot: { status: "captured", snapshot: { anchor: clone(anchor), stop: clone(stop), values: ssa } },
      events_advanced: 1 } });
  const stack = pair("inspect_stack", { scope: { level: "dispatch" }, page: { limit: 16 } }, {
    result: { result: "stack", snapshot: clone(anchor), frames: [
      { frame: 1, function_ordinal: 0, block_ordinal: 1, next_operation: 1, values: { status: "captured", value_count: 1 } },
      { frame: 2, function_ordinal: 7, block_ordinal: 0, next_operation: extra ? 1 : 0, values: { status: "captured", value_count: extra ? 2 : 1 } },
    ] },
  });
  const rows = [
    { variable_identity: digest("d"), name: "value", function_ordinal: 7, scope_identity: digest("e"), scope_depth: 0,
      generation: 1, availability: { status: "value", value: scalar() } },
    { variable_identity: digest("f"), name: "adjusted", function_ordinal: 7, scope_identity: digest("e"), scope_depth: 1,
      generation: 0, availability: { status: "value", value: { status: "unavailable", reason: "not_represented" } } },
  ];
  const pageCursor = { query_identity: digest("1"), position: 1 };
  const sourcePages = rows.map((row, index) => pair("inspect_source_variables", {
    scope: { level: "dispatch" }, frame: 2, selector: { selector: "all" }, page: { limit: 1, ...(index ? { cursor: clone(pageCursor) } : {}) },
  }, { snapshot: clone(sourceAnchor), values: [row], ...(index ? {} : { next_cursor: clone(pageCursor) }) },
  "fe2o3-debug-source-variable-request-v2"));
  const allocation = { ordinal: 1, generation: 0 };
  const memory = pair("read_memory", { allocation, byte_offset: 0, byte_len: 24 },
    { result: { result: "memory", snapshot: clone(anchor), memory: { allocation: clone(allocation), byte_offset: 0,
      requested_bytes: 24, returned_bytes: 24, availability: { status: "captured", address_space: "global",
        bytes: initialBytes, initialized: "0xffffff", truncated: false } } } });
  return { control, stack, sourcePages, memory };
}
test("synthetic shape retains selected helper variables and both independent SSA frames", () => {
  const group = syntheticGroup(), result = verifyHelperSourceCheckpoint(group, initialBytes);
  assert.equal(result.selected_frame, 2); assert.equal(result.stack_frames.length, 2);
  assert.equal(result.ssa_values.length, 3); assert.equal(result.values.length, 2);
  assert.equal(result.source_to_ssa_mapping, "not_supplied");
  assert.notDeepEqual(result.source_variable_anchor, result.checkpoint_anchor);
  assert.deepEqual(result.source_variable_anchor, group.sourcePages[0].response.snapshot);
  assert.deepEqual(result.checkpoint_anchor, group.control.response.result.snapshot.snapshot.anchor);
});
test("synthetic forward/reverse/repeat preserves exact helper source values while SSA differs", () => {
  const groups = [syntheticGroup(), syntheticGroup(3, 19, "reverse", 20, false), syntheticGroup(4, 20, "forward", 30)];
  assert.equal(verifyHelperSourceReplay(groups, initialBytes).observations.length, 3);
});
test("retains an omitted caller next_operation without inventing one", () => {
  const group = syntheticGroup(); delete group.stack.response.result.frames[0].next_operation;
  const result = verifyHelperSourceCheckpoint(group, initialBytes);
  assert.equal(Object.hasOwn(result.stack_frames[0], "next_operation"), false);
});
const mutations = [
  ["explicit null caller next operation", group => { group.stack.response.result.frames[0].next_operation = null; }],
  ["missing root frame", group => group.stack.response.result.frames.shift()],
  ["third unknown frame", group => group.stack.response.result.frames.push(clone(group.stack.response.result.frames[1]))],
  ["noncontiguous frame identity", group => { group.stack.response.result.frames[1].frame = 3; }],
  ["helper selected against caller function", group => { group.stack.response.result.frames[1].function_ordinal = 0; }],
  ["helper next operation unavailable", group => { group.stack.response.result.frames[1].next_operation = null; }],
  ["per-frame SSA truncation", group => { group.stack.response.result.frames[1].values.value_count = 3; }],
  ["same total but false frame counts", group => {
    group.stack.response.result.frames[0].values.value_count = 2; group.stack.response.result.frames[1].values.value_count = 1;
  }],
  ["SSA path wrong frame", group => { group.control.response.result.snapshot.snapshot.values[0].path.root.frame = 2; }],
  ["SSA path wrong function", group => { group.control.response.result.snapshot.snapshot.values[0].path.root.function_ordinal = 7; }],
  ["duplicated SSA identity", group => {
    group.control.response.result.snapshot.snapshot.values[2].path = clone(group.control.response.result.snapshot.snapshot.values[1].path);
  }],
  ["frame1 source request", group => { group.sourcePages[0].request.frame = 1; }],
  ["frame1 source anchor", group => { group.sourcePages[0].response.snapshot.frame = 1; }],
  ["invented dynamic occurrence", group => { group.sourcePages[0].response.snapshot.occurrence = 2; }],
  ["source variable from caller", group => { group.sourcePages[0].response.values[0].function_ordinal = 0; }],
  ["fabricated local value", group => { group.sourcePages[1].response.values[0].availability = { status: "value", value: scalar() }; }],
  ["absent final page", group => { group.sourcePages.pop(); }],
  ["altered page cursor", group => { group.sourcePages[1].request.page.cursor.position = 2; }],
  ["changed selector", group => { group.sourcePages[1].request.selector = { selector: "name", name: "value" }; }],
  ["duplicate source variable", group => { group.sourcePages[1].response.values[0] = clone(group.sourcePages[0].response.values[0]); }],
  ["stale source revision", group => { group.sourcePages[0].request.expected_revision--; }],
  ["stale stack anchor", group => { group.stack.response.result.snapshot.cursor.event_sequence--; }],
  ["stale memory anchor", group => { group.memory.response.result.snapshot.cursor.event_sequence--; }],
  ["truth classification changed", group => { group.sourcePages[0].response.session.hardware_observed = true; }],
  ["unsolicited optional metadata", group => { group.sourcePages[0].response.dynamic_activation = 1; }],
  ["request ID reordering", group => {
    group.sourcePages[0].request.request_id = group.stack.request.request_id;
    group.sourcePages[0].response.request_id = group.stack.request.request_id;
  }],
  ["allocation reuse invented", group => { group.memory.request.allocation.generation = 1; }],
];
for (const [name, mutate] of mutations) test("rejects synthetic " + name, () => {
  const group = syntheticGroup(); mutate(group);
  assert.throws(() => verifyHelperSourceCheckpoint(group, initialBytes));
});
test("replay refuses unchanged SSA masquerading as an observed intermediate state", () => {
  const groups = [syntheticGroup(), syntheticGroup(3, 19, "reverse", 20), syntheticGroup(4, 20, "forward", 30)];
  assert.throws(() => verifyHelperSourceReplay(groups, initialBytes));
});
test("replay refuses caller-only changes masquerading as a distinct selected-helper state", () => {
  const groups = [syntheticGroup(), syntheticGroup(3, 19, "reverse", 20), syntheticGroup(4, 20, "forward", 30)];
  groups[1].control.response.result.snapshot.snapshot.values[0].availability = scalar("0x40000000");
  assert.throws(() => verifyHelperSourceReplay(groups, initialBytes));
});
test("replay also refuses changed suspended-caller SSA when helper SSA differs", () => {
  const groups = [syntheticGroup(), syntheticGroup(3, 19, "reverse", 20, false), syntheticGroup(4, 20, "forward", 30)];
  groups[1].control.response.result.snapshot.snapshot.values[0].availability = scalar("0x40000000");
  assert.throws(() => verifyHelperSourceReplay(groups, initialBytes));
});
test("independent f32 result requires all four outputs and tail canaries", () => {
  const simulation = { status: "ok", hardware_observed: false, counts: { invocations_executed: 4 },
    arguments: [{ bits: "0x3f800000" }, { value: { bytes: "0x" + "00000040".repeat(4) + "deadbeefcafebabe", initialized: "0xffffff" } }] };
  assert.equal(verifyHelperResult(simulation).result_bits, "0x40000000");
  const wrong = clone(simulation); wrong.arguments[1].value.bytes = wrong.arguments[1].value.bytes.replace("deadbeef", "00000000");
  assert.throws(() => verifyHelperResult(wrong));
  const missing = clone(simulation); missing.counts.invocations_executed = 3;
  assert.throws(() => verifyHelperResult(missing));
});
