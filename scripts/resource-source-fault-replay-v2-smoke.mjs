#!/usr/bin/env node
// Ordinary Rust -> CPU simulation only. Importing starts no capture.
// Terminal Fault/Failed navigation does not imply a captured value snapshot.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseBoundedJson } from './ordered-program-source-native.mjs';
import { runNavigationCommand } from './authoring-navigation-v1-process.mjs';
import { validateSummary, validatePage, validateRoster } from './const-u32-helper-source-smoke.mjs';

const MiB = 1024 ** 2, GiB = 1024 ** 3;
export const LIMITS = Object.freeze({ source: MiB, bundle: 4 * MiB, json: MiB,
  tool: 512 * MiB, selectedInputBytes: 2 * GiB, inputReads: 4 * GiB,
  line: 65536, commands: 128, requests: 256 * 1024, responses: 8 * MiB,
  stderr: 65536, sourcePages: 32, sourceValues: 64, ssaValues: 64,
  stream: MiB, stages: 16, observationBytes: 64 * MiB, failureReserve: 65536,
  exportMs: 180000, stageMs: 30000, replyMs: 15000, drainMs: 10000,
  debuggerMs: 90000, totalMs: 300000, freeBytes: 40 * GiB, ramBytes: 64 * GiB,
  cargoJobs: 2, maxEvents: 65536 });
export const SCHEMA = Object.freeze({ v1: 'fe2o3-debug-request-v1',
  source: 'fe2o3-debug-source-variable-request-v2', resource: 'fe2o3-debug-resource-request-v1' });
const responseSchema = schema => ({ [SCHEMA.v1]: 'fe2o3-debug-response-v1',
  [SCHEMA.source]: 'fe2o3-debug-source-variable-response-v2',
  [SCHEMA.resource]: 'fe2o3-debug-resource-response-v1' })[schema];
const clone = value => structuredClone(value), sha = bytes => createHash('sha256').update(bytes).digest('hex');
const json = bytes => parseBoundedJson(bytes, LIMITS.json);
const OUTPUT_BEFORE = '0x' + 'a5a5a5a5'.repeat(4) + 'deadbeefcafebabe';
// Independent exact dyadic f32 oracle: [1,2,4,8] + [.5,1.5,2.5,3.5].
export const OUTPUT_AFTER = '0x0000c03f000060400000d04000003841deadbeefcafebabe';
export const FALSE_CLAIMS = Object.freeze({ source_authentication: false,
  compiler_closure_attestation: false, proof_authority: false, production_resume: false,
  hardware_execution: false, physical_register_values: false, performance_prediction: false,
  terminal_values_captured: false, allocation_reuse_observed: false });
function exact(value, keys) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value), 'object required');
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), 'closed fields'); return value;
}
function nat(value, max = Number.MAX_SAFE_INTEGER, min = 0) {
  assert.ok(Number.isSafeInteger(value) && value >= min && value <= max, 'bounded exact integer'); return value;
}
function digest(value) { assert.equal(typeof value, 'string'); assert.match(value, /^[a-f0-9]{64}$/u);
  assert.notEqual(value, '0'.repeat(64)); return value; }
function text(value, cap = 4096) { assert.equal(typeof value, 'string');
  assert.ok(value.length > 0 && Buffer.byteLength(value) <= cap && !/[\p{Cc}\p{Cs}]/u.test(value)); return value; }
function allocation(value) { assert.deepEqual(value, { ordinal: nat(value.ordinal, 65536, 1), generation: 0 }); }
export function requests() {
  const positive = { schema: 'fe2o3-simulation-request-v1', kernel: 'vecadd', grid: [4, 1, 1], workgroup: [256, 1, 1],
    arguments: [
      { kind: 'buffer', element: 'f32', access: 'read_only', alignment: 4, bytes: '0x0000803f000000400000804000000041', initialized: '0xffff' },
      { kind: 'buffer', element: 'f32', access: 'read_only', alignment: 4, bytes: '0x0000003f0000c03f0000204000006040', initialized: '0xffff' },
      { kind: 'buffer', element: 'f32', access: 'read_write', alignment: 4, bytes: OUTPUT_BEFORE, initialized: '0xffffff' }] };
  const uninitialized = clone(positive); uninitialized.arguments[0].initialized = '0xfeff';
  return { positive, uninitialized };
}
export function verifyRequests(value) { assert.deepEqual(value, requests(), 'only one explicit initialization bit differs'); return clone(value); }
export function verifyPositive(value, kirDigest) {
  exact(value, ['schema', 'status', 'authority', 'simulated', 'hardware_observed', 'hardware_validation',
    'performance_prediction', 'target_profile', 'kir', 'counts', 'schedule', 'conflict_assessment', 'arguments', 'shared_buffers']);
  assert.equal(value.schema, 'fe2o3-simulation-result-v1'); assert.equal(value.status, 'ok');
  assert.equal(value.authority, 'observation_only'); assert.equal(value.simulated, true);
  for (const key of ['hardware_observed', 'hardware_validation', 'performance_prediction']) assert.equal(value[key], false);
  assert.deepEqual(value.target_profile, { identity: 'amdgpu_64_little_endian_v1', index_bits: 64, max_workgroup_invocations: 1024 });
  exact(value.kir, ['sha256', 'canonical_bytes']); assert.equal(value.kir.sha256, digest(kirDigest)); nat(value.kir.canonical_bytes, LIMITS.bundle, 1);
  exact(value.counts, ['arguments', 'shared_buffers', 'invocations_executed', 'workgroups_visited', 'scheduled_slots_visited', 'steps_executed', 'events_emitted']);
  assert.equal(value.counts.arguments, 3); assert.equal(value.counts.shared_buffers, 0);
  // The scheduler counts all 256 local slots before culling outside grid[4,1,1].
  // Only four invocation machines run; this barrier-free source selects each
  // once. The ordinary CLI request disables event delivery, unlike debug capture.
  assert.equal(value.counts.scheduled_slots_visited, 256);
  nat(value.counts.steps_executed, LIMITS.maxEvents, 1); // observed, not a guessed MIR step total
  assert.equal(value.counts.events_emitted, 0);
  assert.equal(value.counts.invocations_executed, 4);
  assert.equal(value.counts.workgroups_visited, 1);
  exact(value.schedule, ['identity', 'transcript_sha256', 'coverage']); digest(value.schedule.transcript_sha256);
  assert.equal(value.schedule.identity, 'workgroup_major_local_zyx_cooperative_v1');
  assert.deepEqual(value.schedule.coverage, { decisions: 4, workgroups: 1, barrier_releases: 0, complete: true });
  assert.deepEqual(value.conflict_assessment, { status: 'no_conflicts_observed' });
  assert.deepEqual(value.shared_buffers, []);
  const expected = requests().positive.arguments.map(({ kind, ...entry }) => ({ kind, value: entry }));
  expected[2].value.bytes = OUTPUT_AFTER; assert.deepEqual(value.arguments, expected);
  return { checked_output_words: 4, checked_canary_bytes: 8, input_bytes_and_initialization_unchanged: true };
}
export function verifyDiagnostic(value) {
  exact(value, ['schema', 'status', 'stage', 'kind', 'message', 'invocation', 'site']);
  assert.equal(value.schema, 'fe2o3-simulation-error-v1'); assert.equal(value.status, 'error');
  assert.equal(value.stage, 'execution'); assert.equal(value.kind, 'execution_uninitialized_read'); text(value.message);
  assert.deepEqual(value.invocation, { global: [0, 0, 0], workgroup: [0, 0, 0], local: [0, 0, 0],
    workgroup_size: [256, 1, 1], workgroup_count: [1, 1, 1], launch_extent: [4, 1, 1] });
  exact(value.site, ['function', 'function_bytes', 'function_truncated', 'block', 'operation']);
  text(value.site.function); assert.equal(value.site.function_bytes, Buffer.byteLength(value.site.function));
  assert.equal(value.site.function_truncated, false); nat(value.site.block, 0xffffffff); nat(value.site.operation, 0xffffffff);
  // The public error has no typed allocation/range: never parse its prose.
  return clone(value);
}
export function verifySession(value, terminated = false) {
  exact(value, ['backend', 'execution_kind', 'state', 'revision', 'configuration_identity', 'cursor',
    'simulated', 'hardware_observed', 'performance_prediction']);
  assert.equal(value.backend, 'cpu_kir_simulator'); assert.equal(value.execution_kind, 'cpu_kir_simulation');
  assert.equal(value.state, terminated ? 'terminated' : 'stopped'); nat(value.revision);
  digest(value.configuration_identity); assert.equal(value.simulated, true);
  assert.equal(value.hardware_observed, false); assert.equal(value.performance_prediction, false);
  assert.deepEqual(value.cursor, { configuration_identity: value.configuration_identity,
    event_sequence: nat(value.cursor.event_sequence), state_revision: value.revision }); return value;
}
function pairHeader(pair, operation, schema, stopped, status = 'ok') {
  exact(pair, ['request', 'response']); const { request: q, response: r } = pair;
  nat(q.request_id, LIMITS.commands, 1); nat(q.expected_revision); assert.equal(q.schema, schema);
  assert.equal(q.operation, operation); assert.equal(r.schema, responseSchema(schema));
  assert.equal(r.request_id, q.request_id); assert.equal(r.operation, operation); assert.equal(r.status, status);
  verifySession(r.session);
  if (stopped) { assert.equal(q.expected_revision, stopped.revision); assert.deepEqual(r.session, stopped, 'same stopped session'); }
  return r;
}
export function verifyControl(pair, reason, direction) {
  const op = direction ? 'step' : 'continue', r = pairHeader(pair, op, SCHEMA.v1);
  exact(pair.request, ['schema', 'request_id', 'expected_revision', 'operation', ...(direction ? ['direction', 'granularity', 'count'] : ['max_events'])]);
  if (direction) { assert.equal(pair.request.direction, direction); assert.equal(pair.request.granularity, 'operation'); assert.equal(pair.request.count, 1); }
  else assert.equal(pair.request.max_events, LIMITS.maxEvents);
  assert.equal(r.session.revision, pair.request.expected_revision + 1);
  exact(r, ['status', 'schema', 'request_id', 'operation', 'session', 'result']);
  exact(r.result, ['result', 'stop', 'snapshot', 'events_advanced']); assert.equal(r.result.result, 'control');
  assert.deepEqual(r.result.stop, { reason, outcome: reason === 'step' ? 'active' : 'failed', exact: true });
  nat(r.result.events_advanced, LIMITS.maxEvents, reason === 'completed' ? 0 : 1);
  if (reason !== 'step') { assert.deepEqual(r.result.snapshot, { status: 'unavailable', reason: 'not_captured' }); return null; }
  exact(r.result.snapshot, ['status', 'snapshot']); assert.equal(r.result.snapshot.status, 'captured');
  const captured = exact(r.result.snapshot.snapshot, ['anchor', 'stop', 'values']); assert.deepEqual(captured.stop, r.result.stop);
  const anchor = exact(captured.anchor, ['cursor', 'scope', 'site']); assert.deepEqual(anchor.cursor, r.session.cursor);
  assert.deepEqual(anchor.scope, { level: 'lane', workgroup: [0, 0, 0], wave: 0, lane: 0,
    logical_workitem: [0, 0, 0], active_mask: 15, wave_width: 32, interpretation: 'logical_visualization' });
  exact(anchor.site, ['kir', 'source']); const kir = exact(anchor.site.kir, ['function_ordinal', 'block_ordinal', 'point']);
  nat(kir.function_ordinal, 65535); nat(kir.block_ordinal, 65535);
  exact(kir.point, ['kind', 'operation_ordinal']); assert.equal(kir.point.kind, 'operation'); nat(kir.point.operation_ordinal, 65535);
  exact(anchor.site.source, ['status', 'location']); assert.equal(anchor.site.source.status, 'resolved');
  const location = exact(anchor.site.source.location, ['map_identity', 'file_identity', 'provenance', 'byte_start', 'byte_end']);
  digest(location.map_identity); digest(location.file_identity); assert.equal(location.provenance, 'compiler_bundle_bound');
  nat(location.byte_start); nat(location.byte_end, Number.MAX_SAFE_INTEGER, location.byte_start + 1);
  assert.ok(Array.isArray(captured.values) && captured.values.length > 0 && captured.values.length <= LIMITS.ssaValues);
  const seen = new Set();
  for (const row of captured.values) {
    exact(row, ['path', 'availability']); exact(row.path, ['root', 'components']); assert.deepEqual(row.path.components, []);
    exact(row.path.root, ['kind', 'function_ordinal', 'frame', 'value_ordinal']); const root = row.path.root;
    assert.equal(root.kind, 'ssa'); nat(root.frame, 16, 1); nat(root.function_ordinal, 65535); nat(root.value_ordinal, 65535);
    const key = JSON.stringify(root); assert.ok(!seen.has(key)); seen.add(key); verifyAvailability(row.availability);
  }
  return captured;
}
function verifyAvailability(value) {
  if (value.status === 'unavailable') {
    exact(value, ['status', 'reason']); assert.ok(['not_represented', 'not_captured', 'optimized_out', 'outside_capture_scope',
      'not_in_scope', 'not_live', 'uninitialized', 'truncated', 'unsupported_by_backend', 'requires_authenticated_map'].includes(value.reason)); return;
  }
  exact(value, ['status', 'value_type', 'value', 'provenance']); assert.equal(value.status, 'captured');
  assert.equal(value.provenance, 'simulated_observation'); const type = value.value_type, bits = value.value;
  if (type.kind === 'pointer') {
    exact(type, ['kind', 'address_space']); assert.ok(['private', 'workgroup', 'global', 'constant', 'generic'].includes(type.address_space));
    exact(bits, ['encoding', 'allocation', 'byte_offset']); assert.equal(bits.encoding, 'allocation_relative_pointer'); allocation(bits.allocation); nat(bits.byte_offset); return;
  }
  let width;
  if (type.kind === 'bool') { exact(type, ['kind']); width = 1; }
  else if (type.kind === 'integer') { exact(type, ['kind', 'bits', 'signed']); assert.equal(typeof type.signed, 'boolean'); width = nat(type.bits, 64, 1); }
  else { exact(type, ['kind', 'bits']); assert.ok(['index', 'float'].includes(type.kind));
    assert.ok((type.kind === 'float' ? [16, 32, 64] : [32, 64]).includes(type.bits)); width = type.bits; }
  exact(bits, ['encoding', 'bits']); assert.equal(bits.encoding, 'bits'); assert.match(bits.bits, /^0x[0-9a-f]+$/u);
  assert.equal(bits.bits.length, 2 + Math.ceil(width / 4)); assert.ok(BigInt(bits.bits) < (1n << BigInt(width)));
}
export function verifyUnavailable(pair, stopped, operation, schema = SCHEMA.v1) {
  const r = pairHeader(pair, operation, schema, stopped, 'unavailable');
  if (schema === SCHEMA.source) {
    exact(r, ['status', 'schema', 'request_id', 'operation', 'session', 'reason']);
    assert.ok(['checkpoint_not_captured', 'source_map_v2_required', 'variables_not_captured'].includes(r.reason));
  } else {
    exact(r, ['status', 'schema', 'request_id', 'operation', 'session', 'unavailable']);
    exact(r.unavailable, ['capability', 'reason', 'state_changed', 'detail']);
    assert.equal(r.unavailable.capability, { inspect_stack: 'call_stack', inspect_values: 'kir_ssa_values', read_memory: 'allocation_relative_memory' }[operation]);
    assert.equal(r.unavailable.reason, 'not_captured'); assert.equal(r.unavailable.state_changed, false); text(r.unavailable.detail);
  }
  return schema === SCHEMA.source ? r.reason : r.unavailable.reason;
}
function queryShape(pair, operation) {
  const q = pair.request;
  if (operation === 'read_memory') { exact(q, ['schema', 'request_id', 'expected_revision', 'operation', 'allocation', 'byte_offset', 'byte_len']);
    allocation(q.allocation); assert.equal(q.byte_offset, 0); assert.ok([16, 24].includes(q.byte_len)); return; }
  exact(q, ['schema', 'request_id', 'expected_revision', 'operation', 'scope', 'page',
    ...(operation === 'inspect_stack' ? [] : ['selector']), ...(operation === 'inspect_source_variables' ? ['frame'] : [])]);
  assert.deepEqual(q.scope, { level: 'dispatch' });
  if (operation !== 'inspect_stack') assert.deepEqual(q.selector, { selector: 'all' });
  if (operation === 'inspect_source_variables') { assert.equal(q.frame, 1); exact(q.page, ['limit', ...(q.page.cursor === undefined ? [] : ['cursor'])]); assert.equal(q.page.limit, 2); }
  else assert.deepEqual(q.page, { limit: operation === 'inspect_stack' ? 16 : 64 });
}
export function verifyTerminal(group, reason, direction, observedAllocation) {
  exact(group, ['control', 'stack', 'ssa', 'source', 'memory']); verifyControl(group.control, reason, direction);
  const stopped = group.control.response.session;
  for (const [key, operation, schema] of [['stack', 'inspect_stack', SCHEMA.v1], ['ssa', 'inspect_values', SCHEMA.v1],
    ['source', 'inspect_source_variables', SCHEMA.source], ['memory', 'read_memory', SCHEMA.v1]]) {
    queryShape(group[key], operation); verifyUnavailable(group[key], stopped, operation, schema);
  }
  assert.deepEqual(group.memory.request.allocation, observedAllocation); assert.equal(group.memory.request.byte_len, 16);
  assert.deepEqual(group.source.request.page, { limit: 2 });
  return { cursor: clone(stopped.cursor), stop: clone(group.control.response.result.stop),
    source_unavailable_reason: group.source.response.reason, terminal_values: 'not_supplied' };
}
export function verifyInventory(pair, captured, stopped) {
  const r = pairHeader(pair, 'query_allocations', SCHEMA.resource, stopped);
  exact(pair.request, ['schema', 'request_id', 'expected_revision', 'operation', 'expected_snapshot', 'page']);
  assert.deepEqual(pair.request.expected_snapshot, captured.anchor); assert.deepEqual(pair.request.page, { max_items: 16, max_scanned: 16 });
  exact(r, ['status', 'schema', 'request_id', 'operation', 'session', 'snapshot', 'page', 'result', 'physical_registers']);
  assert.deepEqual(r.snapshot, captured.anchor); assert.equal(r.physical_registers, 'not_represented');
  assert.deepEqual(r.page, { source_count: 3, scanned: 3, completeness: { status: 'complete' } });
  exact(r.result, ['result', 'allocations']); assert.equal(r.result.result, 'allocations'); assert.equal(r.result.allocations.length, 3);
  const seen = new Set();
  for (const row of r.result.allocations) {
    exact(row, ['allocation', 'address_space', 'access', 'alignment', 'capacity_bytes', 'snapshot_bytes_available',
      'initialization_available', 'owning_scope', 'lifetime', 'physical_base']); allocation(row.allocation);
    assert.ok(!seen.has(row.allocation.ordinal)); seen.add(row.allocation.ordinal);
    assert.equal(row.address_space, 'global'); assert.equal(row.alignment, 4);
    assert.ok(['16', '24'].includes(row.capacity_bytes)); assert.equal(row.access, row.capacity_bytes === '24' ? 'read_write' : 'read_only');
    assert.equal(row.snapshot_bytes_available, true); assert.equal(row.initialization_available, true);
    for (const field of ['owning_scope', 'lifetime', 'physical_base']) assert.equal(row[field], 'not_represented');
  }
  assert.deepEqual(r.result.allocations.map(row => row.capacity_bytes).sort(), ['16', '16', '24']);
  return clone(r.result.allocations);
}
export function verifyCheckpointGroup(group, summary, operations) {
  exact(group, ['control', 'stack', 'ssa', 'inventory', 'sourcePages', 'memory']);
  const captured = verifyControl(group.control, 'step', 'reverse'), anchor = captured.anchor, stopped = group.control.response.session;
  assert.equal(anchor.site.source.location.map_identity, summary.source_map_identity);
  const k = anchor.site.kir;
  const selected = operations.filter(op => op.coordinate.function === k.function_ordinal && op.coordinate.block === k.block_ordinal
    && op.coordinate.operation === k.point.operation_ordinal);
  assert.equal(selected.length, 1, 'one actual canonical operation selector'); assert.equal(selected[0].kind, 'load');
  const stack = pairHeader(group.stack, 'inspect_stack', SCHEMA.v1, stopped); queryShape(group.stack, 'inspect_stack');
  exact(stack, ['status', 'schema', 'request_id', 'operation', 'session', 'result']);
  exact(stack.result, ['result', 'snapshot', 'frames']); assert.equal(stack.result.result, 'stack'); assert.deepEqual(stack.result.snapshot, anchor);
  assert.equal(stack.result.frames.length, 1, 'bounded ordinary root-frame profile');
  const frame = exact(stack.result.frames[0], ['frame', 'function_ordinal', 'block_ordinal', 'next_operation', 'values']);
  assert.equal(frame.frame, 1); assert.equal(frame.function_ordinal, k.function_ordinal); assert.equal(frame.block_ordinal, k.block_ordinal);
  assert.equal(frame.next_operation, k.point.operation_ordinal, 'actual before-load checkpoint');
  assert.deepEqual(frame.values, { status: 'captured', value_count: captured.values.length });
  for (const row of captured.values) { assert.equal(row.path.root.frame, 1); assert.equal(row.path.root.function_ordinal, k.function_ordinal); }
  const ssa = pairHeader(group.ssa, 'inspect_values', SCHEMA.v1, stopped); queryShape(group.ssa, 'inspect_values');
  exact(ssa, ['status', 'schema', 'request_id', 'operation', 'session', 'result']);
  assert.deepEqual(ssa.result, { result: 'values', snapshot: anchor, values: captured.values });
  const inventory = verifyInventory(group.inventory, captured, stopped); assert.equal(group.memory.length, 3);
  const models = requests().uninitialized.arguments, matches = new Set(), windows = [];
  for (const [index, pair] of group.memory.entries()) {
    const r = pairHeader(pair, 'read_memory', SCHEMA.v1, stopped); queryShape(pair, 'read_memory');
    const row = inventory[index], length = Number(row.capacity_bytes);
    assert.deepEqual(pair.request.allocation, row.allocation); assert.equal(pair.request.byte_len, length);
    exact(r, ['status', 'schema', 'request_id', 'operation', 'session', 'result']); exact(r.result, ['result', 'snapshot', 'memory']);
    assert.equal(r.result.result, 'memory'); assert.deepEqual(r.result.snapshot, anchor);
    const memory = exact(r.result.memory, ['allocation', 'byte_offset', 'requested_bytes', 'returned_bytes', 'availability']);
    assert.deepEqual(memory.allocation, row.allocation); assert.equal(memory.byte_offset, 0);
    assert.equal(memory.requested_bytes, length); assert.equal(memory.returned_bytes, length);
    exact(memory.availability, ['status', 'address_space', 'bytes', 'initialized', 'truncated']);
    assert.equal(memory.availability.status, 'captured'); assert.equal(memory.availability.address_space, 'global'); assert.equal(memory.availability.truncated, false);
    const candidates = models.map((m, i) => ({ m, i })).filter(({ m }) => m.access === row.access &&
      m.bytes === memory.availability.bytes && m.initialized === memory.availability.initialized && (m.bytes.length - 2) / 2 === length);
    assert.equal(candidates.length, 1, 'exact byte/access/init match, not ordinal or variable-name inference');
    assert.ok(!matches.has(candidates[0].i)); matches.add(candidates[0].i); windows.push(clone(memory));
  }
  assert.equal(matches.size, 3);
  assert.ok(group.sourcePages.length > 0 && group.sourcePages.length <= LIMITS.sourcePages);
  let cursor, queryIdentity, sourceAnchor, unavailableReason; const sourceValues = [], seen = new Set();
  for (const [index, pair] of group.sourcePages.entries()) {
    queryShape(pair, 'inspect_source_variables'); assert.deepEqual(pair.request.page, { limit: 2, ...(cursor ? { cursor } : {}) });
    if (pair.response.status === 'unavailable') {
      assert.equal(group.sourcePages.length, 1); assert.equal(index, 0);
      unavailableReason = verifyUnavailable(pair, stopped, 'inspect_source_variables', SCHEMA.source); break;
    }
    const r = pairHeader(pair, 'inspect_source_variables', SCHEMA.source, stopped);
    exact(r, ['status', 'schema', 'request_id', 'operation', 'session', 'snapshot', 'values', ...(r.next_cursor === undefined ? [] : ['next_cursor'])]);
    assert.deepEqual(r.snapshot, { ...anchor, frame: 1, occurrence: 1 }, 'explicit static frame refinement only');
    sourceAnchor ??= r.snapshot; assert.deepEqual(r.snapshot, sourceAnchor);
    assert.ok(Array.isArray(r.values) && r.values.length <= 2);
    for (const row of r.values) {
      exact(row, ['variable_identity', 'name', 'function_ordinal', 'scope_identity', 'scope_depth', 'generation', 'availability']);
      digest(row.variable_identity); digest(row.scope_identity); text(row.name); nat(row.scope_depth, 4096); nat(row.generation);
      assert.equal(row.function_ordinal, k.function_ordinal); assert.ok(!seen.has(row.variable_identity)); seen.add(row.variable_identity);
      if (row.availability.status === 'ambiguous') exact(row.availability, ['status']);
      else { exact(row.availability, ['status', 'value']); assert.equal(row.availability.status, 'value'); verifyAvailability(row.availability.value); }
      sourceValues.push(row); assert.ok(sourceValues.length <= LIMITS.sourceValues);
    }
    cursor = r.next_cursor;
    if (cursor !== undefined) { exact(cursor, ['query_identity', 'position']); digest(cursor.query_identity);
      assert.equal(cursor.position, sourceValues.length); assert.equal(r.values.length, 2);
      queryIdentity ??= cursor.query_identity; assert.equal(cursor.query_identity, queryIdentity); }
    assert.equal(cursor === undefined, index === group.sourcePages.length - 1, 'complete source page chain');
  }
  return clone({ anchor, selected_operation: selected[0], stack: frame, ssa_values: captured.values, inventory, memory: windows,
    source: unavailableReason ? { status: 'unavailable', reason: unavailableReason } : { status: 'queried', anchor: sourceAnchor, values: sourceValues } });
}
export function verifyStale(pair, stopped) {
  const q = pair.request, r = pair.response;
  queryShape(pair, 'inspect_values'); nat(q.request_id, LIMITS.commands, 1); nat(q.expected_revision);
  assert.ok(q.expected_revision < stopped.revision); assert.equal(q.schema, SCHEMA.v1); assert.equal(q.operation, 'inspect_values');
  exact(r, ['status', 'schema', 'request_id', 'operation', 'session', 'error']);
  assert.equal(r.status, 'error'); assert.equal(r.schema, responseSchema(q.schema)); assert.equal(r.request_id, q.request_id); assert.equal(r.operation, q.operation);
  assert.deepEqual(r.session, stopped); exact(r.error, ['stage', 'code', 'message', 'state_changed']);
  assert.equal(r.error.stage, 'session'); assert.equal(r.error.code, 'stale_revision'); assert.equal(r.error.state_changed, false); text(r.error.message);
}
export function verifyReplay(value, summary, operations) {
  exact(value, ['initial', 'initialInventory', 'terminals', 'checkpoints', 'stale', 'end']);
  const initial = verifyControl(value.initial, 'step', 'forward'); assert.equal(initial.anchor.site.source.location.map_identity, summary.source_map_identity);
  const rows = verifyInventory(value.initialInventory, initial, value.initial.response.session);
  const selected = rows.find(row => row.capacity_bytes === '16'); assert.ok(selected);
  assert.equal(value.terminals.length, 3); assert.equal(value.checkpoints.length, 2);
  const terminals = value.terminals.map((group, i) => verifyTerminal(group, 'fault', i === 0 ? undefined : 'forward', selected.allocation));
  const checkpoints = value.checkpoints.map(group => verifyCheckpointGroup(group, summary, operations));
  const [first, repeat] = checkpoints;
  assert.equal(initial.anchor.cursor.configuration_identity, first.anchor.cursor.configuration_identity);
  assert.ok(initial.anchor.cursor.event_sequence <= first.anchor.cursor.event_sequence);
  assert.ok(initial.anchor.cursor.state_revision < first.anchor.cursor.state_revision);
  assert.deepEqual(first.inventory, rows); assert.deepEqual(repeat.inventory, rows);
  for (const key of ['site', 'scope']) assert.deepEqual(first.anchor[key], repeat.anchor[key]);
  assert.equal(first.anchor.cursor.event_sequence, repeat.anchor.cursor.event_sequence);
  assert.equal(first.anchor.cursor.configuration_identity, repeat.anchor.cursor.configuration_identity);
  assert.ok(repeat.anchor.cursor.state_revision > first.anchor.cursor.state_revision);
  for (const key of ['selected_operation', 'stack', 'ssa_values', 'inventory', 'memory']) assert.deepEqual(first[key], repeat[key], 'exact prior-checkpoint repetition');
  assert.equal(first.source.status, repeat.source.status);
  if (first.source.status === 'queried') {
    assert.deepEqual(first.source.values, repeat.source.values); assert.deepEqual(first.source.anchor, { ...first.anchor, frame: 1, occurrence: 1 });
    assert.deepEqual(repeat.source.anchor, { ...repeat.anchor, frame: 1, occurrence: 1 });
  } else assert.deepEqual(first.source, repeat.source);
  for (const terminal of terminals) {
    assert.equal(terminal.cursor.configuration_identity, first.anchor.cursor.configuration_identity);
    assert.ok(terminal.cursor.event_sequence > first.anchor.cursor.event_sequence, 'terminal differs from captured checkpoint');
    assert.equal(terminal.cursor.event_sequence, terminals[0].cursor.event_sequence);
  }
  verifyStale(value.stale, value.checkpoints[1].control.response.session);
  verifyTerminal(value.end, 'completed', undefined, selected.allocation);
  assert.equal(value.end.control.response.session.cursor.event_sequence, terminals[0].cursor.event_sequence);
  return { terminals, checkpoints, end: clone(value.end.control.response.result.stop), claims: clone(FALSE_CLAIMS),
    source_to_ssa_mapping: 'not_supplied', frame_identity: 'static_stack_depth_not_dynamic_activation',
    allocation_generations: 'zero_only_no_reuse_claim', diagnostic_to_debugger: 'separate_executions_of_identical_bundle_and_request',
    terminal_failed_read_descriptor: 'not_exposed_by_current_public_protocol' };
}
export function verifyTranscript(pairs, requestLines, responseLines) {
  assert.ok(pairs.length > 0 && pairs.length <= LIMITS.commands); assert.equal(requestLines.length, pairs.length); assert.equal(responseLines.length, pairs.length);
  let previous, configuration; let requestBytes = 0, responseBytes = 0;
  for (const [index, pair] of pairs.entries()) {
    assert.equal(pair.request.request_id, index + 1); assert.equal(pair.response.request_id, index + 1);
    assert.equal(pair.response.operation, pair.request.operation); assert.equal(pair.response.schema, responseSchema(pair.request.schema));
    for (const [line, object] of [[requestLines[index], pair.request], [responseLines[index], pair.response]]) {
      assert.equal(typeof line, 'string'); assert.ok(line.endsWith('\n') && !line.includes('\r') && line.charCodeAt(0) !== 0xfeff);
      assert.ok(Buffer.byteLength(line) <= LIMITS.line); assert.deepEqual(json(Buffer.from(line)), object);
    }
    requestBytes += Buffer.byteLength(requestLines[index]); responseBytes += Buffer.byteLength(responseLines[index]);
    const stopped = verifySession(pair.response.session, pair.request.operation === 'terminate');
    configuration ??= stopped.configuration_identity; assert.equal(stopped.configuration_identity, configuration);
    if (previous) {
      if (pair.response.status === 'error') { assert.equal(pair.response.error.state_changed, false); assert.deepEqual(stopped, previous); }
      else {
        assert.equal(pair.request.expected_revision, previous.revision);
        if (!['step', 'continue', 'terminate'].includes(pair.request.operation)) assert.deepEqual(stopped, previous);
        else assert.equal(stopped.revision, previous.revision + 1);
      }
      if (['step', 'continue'].includes(pair.request.operation)) assert.equal(pair.response.result.events_advanced,
        Math.abs(stopped.cursor.event_sequence - previous.cursor.event_sequence));
    } else { assert.equal(pair.request.expected_revision, 0); assert.equal(stopped.revision, 1); }
    previous = stopped;
  }
  assert.ok(requestBytes <= LIMITS.requests && responseBytes <= LIMITS.responses);
  return { pairs: pairs.length, request_bytes: requestBytes, response_bytes: responseBytes, configuration_identity: configuration };
}

const statIdentity = stat => ['dev', 'ino', 'mode', 'nlink', 'size', 'mtimeNs', 'ctimeNs'].map(key => String(stat[key]));
function environment(opt) {
  const env = { ...process.env };
  for (const key of Object.keys(env)) if (key.startsWith('FE2O3_') || (key.startsWith('CARGO_TARGET_') && key.endsWith('_RUSTFLAGS')) ||
    ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
      'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER', 'CARGO_TARGET_DIR'].includes(key)) delete env[key];
  return { ...env, CARGO: opt.cargo, RUSTC: opt.rustc, CARGO_INCREMENTAL: '0', CARGO_BUILD_JOBS: '2', CARGO_NET_OFFLINE: 'true',
    LD_LIBRARY_PATH: path.join(path.dirname(path.dirname(opt.rustc)), 'lib') + ':' + opt['bin-dir'] };
}
export function options(argv) {
  const keys = ['repo', 'bin-dir', 'cargo', 'rustc', 'export-target', 'output'], opt = {};
  assert.equal(argv.length, keys.length * 2);
  for (let i = 0; i < argv.length; i += 2) {
    assert.ok(argv[i].startsWith('--')); const key = argv[i].slice(2); assert.ok(keys.includes(key) && !Object.hasOwn(opt, key));
    const value = text(argv[i + 1]); assert.ok(path.isAbsolute(value) && path.resolve(value) === value); opt[key] = value;
  }
  for (const key of keys) assert.ok(Object.hasOwn(opt, key));
  const overlaps = (a, b) => a === b || a.startsWith(b + path.sep) || b.startsWith(a + path.sep);
  assert.equal(path.dirname(opt.output), path.dirname(opt['export-target']), 'exclusive siblings under one supervisor');
  assert.ok(!overlaps(opt.output, opt['export-target']));
  for (const output of [opt.output, opt['export-target']]) for (const input of [opt.repo, opt['bin-dir'], opt.cargo, opt.rustc])
    assert.ok(!overlaps(output, input), 'outputs disjoint from inputs');
  return opt;
}
export async function run(argumentsObject) {
  // Copy/revalidate before any await; caller aliasing cannot alter tool/path custody.
  const opt = options(Object.entries(argumentsObject).flatMap(([key, value]) => ['--' + key, value]));
  const started = Date.now(), deadline = started + LIMITS.totalMs, env = environment(opt), output = opt.output;
  const pins = new Map(), saved = new Map(), stages = [], pairs = [], requestLines = [], responseLines = [], raw = {};
  let inputBytes = 0, selectedBytes = 0, retainedBytes = 0;
  for (const dir of [opt.repo, opt['bin-dir'], path.dirname(output)]) {
    assert.ok(fs.lstatSync(dir).isDirectory()); assert.equal(fs.realpathSync(dir), dir);
  }
  for (const dir of [output, opt['export-target']]) assert.equal(fs.existsSync(dir), false, 'new exclusive output required');
  fs.mkdirSync(output, { mode: 0o700 }); fs.mkdirSync(opt['export-target'], { mode: 0o700 });
  const scratchIdentity = statIdentity(fs.lstatSync(opt['export-target'], { bigint: true })).slice(0, 3);
  const outputIdentity = statIdentity(fs.lstatSync(output, { bigint: true })).slice(0, 3);
  const timeGuard = () => assert.ok(Date.now() < deadline, 'five-minute aggregate wall bound');
  const guard = () => {
    timeGuard();
    const disk = fs.statfsSync(output, { bigint: true }); assert.ok(disk.bavail * disk.bsize >= BigInt(LIMITS.freeBytes));
    const mem = fs.readFileSync('/proc/meminfo', 'utf8'); assert.ok(Buffer.byteLength(mem) <= LIMITS.line);
    const match = /^MemAvailable:\s+([0-9]+) kB$/mu.exec(mem); assert.ok(match && BigInt(match[1]) * 1024n >= BigInt(LIMITS.ramBytes));
    for (const [dir, identity] of [[output, outputIdentity], [opt['export-target'], scratchIdentity]]) {
      assert.equal(fs.realpathSync(dir), dir); assert.deepEqual(statIdentity(fs.lstatSync(dir, { bigint: true })).slice(0, 3), identity);
    }
    const entries = fs.readdirSync(output, { withFileTypes: true }); assert.ok(entries.length <= 64);
    let total = 0n;
    for (const entry of entries) { assert.ok(entry.isFile()); const stat = fs.lstatSync(path.join(output, entry.name), { bigint: true });
      assert.ok(stat.isFile()); total += stat.size; }
    assert.ok(total <= BigInt(LIMITS.observationBytes), 'cooperative observation-directory budget');
  };
  const measure = (file, cap, keep = false, allowEmpty = false) => {
    guard(); assert.equal(fs.realpathSync(file), file);
    const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
    try {
      const before = fs.fstatSync(fd, { bigint: true }); assert.ok(before.isFile() && before.size <= BigInt(cap) && (allowEmpty || before.size > 0n));
      const hash = createHash('sha256'), chunks = [], block = Buffer.alloc(65536); let bytes = 0;
      while (bytes < Number(before.size)) { timeGuard(); const count = fs.readSync(fd, block, 0, Math.min(block.length, Number(before.size) - bytes), bytes);
        assert.ok(count > 0); bytes += count; inputBytes += count; assert.ok(inputBytes <= LIMITS.inputReads);
        hash.update(block.subarray(0, count)); if (keep) chunks.push(Buffer.from(block.subarray(0, count))); }
      assert.deepEqual(statIdentity(fs.fstatSync(fd, { bigint: true })), statIdentity(before));
      assert.deepEqual(statIdentity(fs.lstatSync(file, { bigint: true })), statIdentity(before));
      return { pin: { path: file, bytes, sha256: hash.digest('hex'), identity: statIdentity(before) }, bytes: keep ? Buffer.concat(chunks) : undefined };
    } finally { fs.closeSync(fd); }
  };
  const pin = (file, cap, keep = false, allowEmpty = false) => {
    const result = measure(file, cap, keep, allowEmpty), prior = pins.get(file);
    if (prior) assert.deepEqual(result.pin, prior.pin, 'selected input or artifact drift');
    else { selectedBytes += result.pin.bytes; assert.ok(selectedBytes <= LIMITS.selectedInputBytes && pins.size < 64); pins.set(file, { pin: result.pin, cap, allowEmpty }); }
    return result;
  };
  const save = (name, value, failure = false) => {
    assert.match(name, /^[a-zA-Z0-9][a-zA-Z0-9.-]*$/u);
    const bytes = Buffer.isBuffer(value) ? value : Buffer.from(typeof value === 'string' ? value : JSON.stringify(value, null, 2) + '\n');
    assert.ok(retainedBytes + bytes.length <= LIMITS.observationBytes - (failure ? 0 : LIMITS.failureReserve));
    fs.writeFileSync(path.join(output, name), bytes, { flag: 'wx', mode: 0o600 }); retainedBytes += bytes.length;
    const record = { path: name, bytes: bytes.length, sha256: sha(bytes) }; saved.set(name, record); return record;
  };
  const command = async (name, executable, args, input = Buffer.alloc(0), expectedCode = 0) => {
    guard(); assert.ok(stages.length < LIMITS.stages);
    const result = await runNavigationCommand({ executable, args, cwd: opt.repo, env, input,
      timeoutMs: Math.max(1, Math.min(name === 'export' ? LIMITS.exportMs : LIMITS.stageMs, deadline - Date.now())), outputCap: LIMITS.stream, guard });
    const stdout = save(name + '.stdout', result.stdout), stderr = save(name + '.stderr', result.stderr);
    stages.push({ name, executable, args, stdin_bytes: input.length, stdin_sha256: sha(input), code: result.code, signal: result.signal,
      reason: result.reason, elapsed_ms: result.elapsed_ms, stdout, stderr });
    assert.equal(result.reason, null); assert.equal(result.signal, null); assert.equal(result.code, expectedCode); guard(); return result;
  };
  let child, pending, failure, monitor, timer, drainTimer, exited, exitValue, closed = false, closing = false;
  let revision = 0, id = 1, buffered = '', stderr = Buffer.alloc(0), responseBytes = 0, requestBytes = 0;
  const decoder = new TextDecoder('utf-8', { fatal: true });
  const kill = () => { if (child?.pid && !closed) { try { process.kill(-child.pid, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') failure ??= error; } } };
  const fail = error => { failure ??= error; const active = pending; pending = undefined; active?.reject(error); kill(); };
  const append = (which, bytes) => {
    assert.ok(retainedBytes + bytes.length <= LIMITS.observationBytes - LIMITS.failureReserve);
    let at = 0; while (at < bytes.length) { const n = fs.writeSync(raw[which], bytes, at, bytes.length - at); assert.ok(n > 0); at += n; } retainedBytes += bytes.length;
  };
  const ask = async fields => {
    guard(); if (failure) throw failure; assert.equal(pending, undefined); nat(id, LIMITS.commands, 1);
    const request = { schema: SCHEMA.v1, request_id: id++, expected_revision: revision, ...fields }, line = JSON.stringify(request) + '\n';
    requestBytes += Buffer.byteLength(line); assert.ok(Buffer.byteLength(line) <= LIMITS.line && requestBytes <= LIMITS.requests);
    append('requests', Buffer.from(line)); requestLines.push(line);
    const response = await new Promise((resolve, reject) => {
      const replyTimer = setTimeout(() => fail(new Error('debugger reply deadline')), Math.max(1, Math.min(LIMITS.replyMs, deadline - Date.now())));
      pending = { resolve(value) { clearTimeout(replyTimer); resolve(value); }, reject(error) { clearTimeout(replyTimer); reject(error); } };
      child.stdin.write(line, error => { if (error) fail(error); });
    });
    assert.equal(response.request_id, request.request_id); assert.equal(response.operation, request.operation); assert.equal(response.schema, responseSchema(request.schema));
    revision = nat(response.session.revision); const pair = { request, response }; pairs.push(pair); return pair;
  };
  try {
    guard();
    const sourceFiles = ['examples/vecadd/src/lib.rs', 'examples/vecadd/src/vecadd_body.rs', 'examples/vecadd/Cargo.toml', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'];
    for (const name of sourceFiles) pin(path.join(opt.repo, name), LIMITS.source);
    for (const name of ['resource-source-fault-replay-v2-smoke.mjs', 'const-u32-helper-source-smoke.mjs', 'ordered-program-source-native.mjs',
      'ordered-program-worker-prototype.mjs', 'assembly-region-worker-prototype.mjs', 'authoring-navigation-v1-process.mjs']) pin(fileURLToPath(new URL(name, import.meta.url)), LIMITS.source);
    for (const file of [opt.cargo, opt.rustc, process.execPath]) pin(file, LIMITS.tool);
    const tools = Object.fromEntries(['fe2o3-export-sim', 'fe2o3-author', 'fe2o3-kir-sim', 'fe2o3-debug'].map(name => {
      const file = path.join(opt['bin-dir'], name); pin(file, LIMITS.tool); return [name, file]; }));
    const bundlePath = path.join(output, 'vecadd-v6.fe2sim');
    await command('export', tools['fe2o3-export-sim'], ['--crate', 'fe2o3_vecadd', '--output', bundlePath, '--bundle-version', '6',
      '--target', 'gfx942', '--target-dir', opt['export-target'], '--', '--manifest-path', path.join(opt.repo, 'examples/vecadd/Cargo.toml'), '--lib', '--offline']);
    const bundle = pin(bundlePath, LIMITS.bundle, true); retainedBytes += bundle.pin.bytes;
    assert.ok(retainedBytes <= LIMITS.observationBytes - LIMITS.failureReserve);
    const inspection = await command('inspection', tools['fe2o3-author'], ['inspect'], bundle.bytes);
    assert.equal(inspection.stderr.length, 0); const summary = json(inspection.stdout); validateSummary(summary);
    const operations = [];
    for (let start = 0; start < summary.operation_count;) {
      const result = await command('operations-' + start, tools['fe2o3-author'], ['operations', '--bundle-identity', summary.bundle_identity, '--start', String(start), '--limit', '64'], bundle.bytes);
      assert.equal(result.stderr.length, 0); const page = json(result.stdout); start = validatePage(page, summary, start); operations.push(...page.operations);
    }
    validateRoster(operations, summary);
    const profile = verifyRequests(requests());
    save('positive-request.json', profile.positive); save('uninitialized-request.json', profile.uninitialized);
    const positivePath = path.join(output, 'positive-request.json'), faultPath = path.join(output, 'uninitialized-request.json');
    pin(positivePath, LIMITS.line); pin(faultPath, LIMITS.line);
    const positive = await command('positive', tools['fe2o3-kir-sim'], ['--bundle-v6', bundlePath, '--request', positivePath]);
    assert.equal(positive.stderr.length, 0); const positiveDocument = json(positive.stdout);
    const positiveObservation = verifyPositive(positiveDocument, summary.canonical_kir_digest);
    assert.equal(String(positiveDocument.kir.canonical_bytes), summary.canonical_kir_bytes);
    const fault = await command('uninitialized', tools['fe2o3-kir-sim'], ['--bundle-v6', bundlePath, '--request', faultPath], Buffer.alloc(0), 1);
    assert.equal(fault.stdout.length, 0); const diagnostic = verifyDiagnostic(json(fault.stderr));
    for (const name of ['requests', 'responses']) raw[name] = fs.openSync(path.join(output, 'debug-' + name + '.jsonl'), 'wx', 0o600);
    child = spawn(tools['fe2o3-debug'], ['sim', '--bundle-v6', bundlePath, '--request', faultPath, '--wave-width', '32'],
      { cwd: opt.repo, env, detached: true, stdio: ['pipe', 'pipe', 'pipe'] });
    exited = new Promise(resolve => child.once('close', (code, signal) => {
      closed = true; exitValue = { code, signal }; clearTimeout(drainTimer);
      try { assert.equal(decoder.decode(), ''); assert.equal(buffered, ''); if (pending) fail(new Error('closed before pending reply drained')); } catch (error) { fail(error); }
      resolve(exitValue);
    }));
    child.once('error', fail); child.stdin.on('error', fail); child.stdout.on('error', fail); child.stderr.on('error', fail);
    child.stderr.on('data', chunk => { try { assert.ok(stderr.length + chunk.length <= LIMITS.stderr); stderr = Buffer.concat([stderr, chunk]); } catch (error) { fail(error); } });
    child.stdout.on('data', chunk => {
      try {
        responseBytes += chunk.length; assert.ok(responseBytes <= LIMITS.responses); append('responses', chunk);
        buffered += decoder.decode(chunk, { stream: true }); assert.ok(Buffer.byteLength(buffered) <= LIMITS.line);
        const at = buffered.indexOf('\n'); if (at < 0) return;
        assert.ok(pending); const line = buffered.slice(0, at + 1); buffered = buffered.slice(at + 1); assert.equal(buffered, '');
        assert.ok(!line.includes('\r') && line.charCodeAt(0) !== 0xfeff); const response = json(Buffer.from(line)); responseLines.push(line);
        const active = pending; pending = undefined; active.resolve(response);
      } catch (error) { fail(error); }
    });
    child.once('exit', (code, signal) => {
      if (!closing || code !== 0 || signal !== null) fail(new Error('unexpected debugger exit'));
      drainTimer = setTimeout(() => { fail(new Error('debugger drain deadline')); child.stdout.destroy(); child.stderr.destroy(); child.unref(); }, LIMITS.drainMs);
    });
    timer = setTimeout(() => fail(new Error('debugger process deadline')), Math.max(1, Math.min(LIMITS.debuggerMs, deadline - Date.now())));
    monitor = setInterval(() => { try { guard(); } catch (error) { fail(error); } }, 1000);
    const step = direction => ask({ operation: 'step', direction, granularity: 'operation', count: 1 });
    const stackQuery = { operation: 'inspect_stack', scope: { level: 'dispatch' }, page: { limit: 16 } };
    const ssaQuery = { operation: 'inspect_values', scope: { level: 'dispatch' }, selector: { selector: 'all' }, page: { limit: 64 } };
    const sourceQuery = { schema: SCHEMA.source, operation: 'inspect_source_variables', scope: { level: 'dispatch' }, frame: 1, selector: { selector: 'all' }, page: { limit: 2 } };
    const inventoryAt = captured => ask({ schema: SCHEMA.resource, operation: 'query_allocations', expected_snapshot: captured.anchor, page: { max_items: 16, max_scanned: 16 } });
    const initial = await step('forward'), initialCaptured = verifyControl(initial, 'step', 'forward');
    const initialInventory = await inventoryAt(initialCaptured), rows = verifyInventory(initialInventory, initialCaptured, initial.response.session);
    const selectedAllocation = rows.find(row => row.capacity_bytes === '16').allocation;
    const memoryQuery = { operation: 'read_memory', allocation: selectedAllocation, byte_offset: 0, byte_len: 16 };
    const terminal = async (control, reason, direction) => {
      verifyControl(control, reason, direction);
      const group = { control, stack: await ask(stackQuery), ssa: await ask(ssaQuery), source: await ask(sourceQuery), memory: await ask(memoryQuery) };
      verifyTerminal(group, reason, direction, selectedAllocation); return group;
    };
    const checkpoint = async control => {
      const captured = verifyControl(control, 'step', 'reverse'), stack = await ask(stackQuery), ssa = await ask(ssaQuery), inventory = await inventoryAt(captured);
      const allocations = verifyInventory(inventory, captured, control.response.session), sourcePages = []; let cursor;
      for (let page = 0; page < LIMITS.sourcePages; page++) {
        const pair = await ask({ ...sourceQuery, page: { limit: 2, ...(cursor ? { cursor } : {}) } }); sourcePages.push(pair);
        if (pair.response.status === 'unavailable') { assert.equal(page, 0); break; }
        assert.equal(pair.response.status, 'ok'); cursor = pair.response.next_cursor; if (cursor === undefined) break;
      }
      assert.equal(cursor, undefined, 'complete bounded source page chain');
      const memory = []; for (const row of allocations) memory.push(await ask({ operation: 'read_memory', allocation: row.allocation, byte_offset: 0, byte_len: Number(row.capacity_bytes) }));
      const group = { control, stack, ssa, inventory, sourcePages, memory }; verifyCheckpointGroup(group, summary, operations); return group;
    };
    const terminals = [await terminal(await ask({ operation: 'continue', max_events: LIMITS.maxEvents }), 'fault')];
    const checkpoints = [await checkpoint(await step('reverse'))];
    terminals.push(await terminal(await step('forward'), 'fault', 'forward'));
    checkpoints.push(await checkpoint(await step('reverse')));
    const stale = await ask({ ...ssaQuery, expected_revision: checkpoints[0].control.response.session.revision });
    verifyStale(stale, checkpoints[1].control.response.session);
    terminals.push(await terminal(await step('forward'), 'fault', 'forward'));
    const end = await terminal(await ask({ operation: 'continue', max_events: LIMITS.maxEvents }), 'completed');
    const observed = verifyReplay({ initial, initialInventory, terminals, checkpoints, stale, end }, summary, operations);
    closing = true; const termination = await ask({ operation: 'terminate' });
    assert.equal(termination.response.status, 'ok'); assert.deepEqual(termination.response.result, { result: 'terminated' });
    verifySession(termination.response.session, true); child.stdin.end(); assert.deepEqual(await exited, { code: 0, signal: null });
    clearTimeout(timer); clearInterval(monitor); if (failure) throw failure; assert.equal(stderr.length, 0);
    const transcript = verifyTranscript(pairs, requestLines, responseLines);
    const observations = save('observations.json', observed), debugStderr = save('debug-stderr.txt', stderr);
    const rawPins = ['requests', 'responses'].map(name => {
      const filename = 'debug-' + name + '.jsonl', observed = pin(path.join(output, filename), LIMITS[name]).pin;
      return { path: filename, bytes: observed.bytes, sha256: observed.sha256 };
    });
    for (const { pin: before, cap, allowEmpty } of pins.values()) assert.deepEqual(measure(before.path, cap, false, allowEmpty).pin, before, 'final selected pin readback');
    for (const record of saved.values()) {
      const reread = measure(path.join(output, record.path), Math.max(record.bytes, 1), false, record.bytes === 0).pin;
      assert.equal(reread.bytes, record.bytes); assert.equal(reread.sha256, record.sha256, 'retained stage/output drift');
    }
    guard(); save('receipt.json', { schema: 'fe2o3-source-fault-replay-observation-v1', status: 'passed',
      claims: FALSE_CLAIMS, source_files: sourceFiles, bundle: bundle.pin, bundle_identity: summary.bundle_identity,
      canonical_kir_digest: summary.canonical_kir_digest, source_map_identity: summary.source_map_identity,
      positive: positiveObservation, standalone_diagnostic: diagnostic, standalone_and_debugger_are_separate_executions: true,
      transcript, raw_transcripts: rawPins, observations, debugger_stderr: debugStderr, stages,
      selected_input_and_artifact_pins: [...pins.values()].map(value => value.pin), limits: LIMITS,
      compiler_scratch: { path: opt['export-target'], initial_directory_identity: scratchIdentity,
        accounting: 'separate_root_supervisor_budget_not_part_of_64MiB_observation_bound',
        full_compiler_closure_pinning: 'required_from_outer_supervisor_not_attested_here' },
      retained_observation_bytes_before_receipt: retainedBytes, raw_line_preservation: true,
      source_variables: observed.checkpoints.map(group => group.source.status), elapsed_ms: Date.now() - started });
    process.stdout.write('CPU source diagnostic and fault replay observation passed: ' + output + '\n');
  } catch (error) {
    fail(error); clearTimeout(timer); clearInterval(monitor);
    if (exited && !closed) {
      let bounded;
      try { await Promise.race([exited, new Promise(resolve => { bounded = setTimeout(resolve, LIMITS.drainMs); })]); }
      finally { clearTimeout(bounded); }
      if (!closed) { child.stdin.destroy(); child.stdout.destroy(); child.stderr.destroy(); child.unref(); }
    }
    clearTimeout(drainTimer);
    save('failure.json', { status: 'failed', detail: String(error).slice(0, 4096), debugger_exit: exitValue ?? null,
      debugger_stderr_prefix_base64: stderr.subarray(0, 8192).toString('base64'), raw_transcripts_may_be_partial: true,
      compiler_scratch_retained: opt['export-target'], successful_fallback: false, claims: FALSE_CLAIMS }, true);
    throw error;
  } finally { clearTimeout(timer); clearTimeout(drainTimer); clearInterval(monitor); for (const fd of Object.values(raw)) fs.closeSync(fd); }
}
if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) await run(options(process.argv.slice(2)));
