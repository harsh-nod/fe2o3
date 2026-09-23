// Synthetic protocol-only controls. No Rust export, KIR/map authoring or process execution.
import assert from 'node:assert/strict';
import test from 'node:test';
import { LIMITS, SCHEMA, FALSE_CLAIMS, OUTPUT_AFTER, requests, options, verifyRequests,
  verifyPositive, verifyDiagnostic, verifyControl, verifyTerminal, verifyCheckpointGroup,
  verifyInventory, verifyStale, verifyReplay, verifyTranscript } from './resource-source-fault-replay-v2-smoke.mjs';

const copy = value => structuredClone(value), digest = n => n.toString(16).padStart(64, '0');
const summary = { source_map_identity: digest(21), canonical_kir_digest: digest(22) };
const operations = [{ coordinate: { function: 7, block: 4, operation: 2 }, kind: 'load', function_name: 'synthetic_vecadd' }];
const sourceLocation = { map_identity: summary.source_map_identity, file_identity: digest(23),
  provenance: 'compiler_bundle_bound', byte_start: 10, byte_end: 12 };
function session(revision, event, terminated = false) {
  return { backend: 'cpu_kir_simulator', execution_kind: 'cpu_kir_simulation', state: terminated ? 'terminated' : 'stopped',
    revision, configuration_identity: digest(20), cursor: { configuration_identity: digest(20), event_sequence: event, state_revision: revision },
    simulated: true, hardware_observed: false, performance_prediction: false };
}
function anchor(stopped) {
  return { cursor: copy(stopped.cursor), scope: { level: 'lane', workgroup: [0, 0, 0], wave: 0, lane: 0,
    logical_workitem: [0, 0, 0], active_mask: 15, wave_width: 32, interpretation: 'logical_visualization' },
    site: { kir: { function_ordinal: 7, block_ordinal: 4, point: { kind: 'operation', operation_ordinal: 2 } },
      source: { status: 'resolved', location: copy(sourceLocation) } } };
}
function ssaRows() {
  return [{ path: { root: { kind: 'ssa', function_ordinal: 7, frame: 1, value_ordinal: 10 }, components: [] },
    availability: { status: 'captured', value_type: { kind: 'pointer', address_space: 'global' },
      value: { encoding: 'allocation_relative_pointer', allocation: { ordinal: 9, generation: 0 }, byte_offset: 0 },
      provenance: 'simulated_observation' } }];
}
function allocationRows() {
  return [9, 4, 13].map((ordinal, index) => ({ allocation: { ordinal, generation: 0 }, address_space: 'global',
    access: index === 2 ? 'read_write' : 'read_only', alignment: 4, capacity_bytes: index === 2 ? '24' : '16',
    snapshot_bytes_available: true, initialization_available: true, owning_scope: 'not_represented',
    lifetime: 'not_represented', physical_base: 'not_represented' }));
}
function sourceRows() {
  return [0, 1, 2].map(i => ({ variable_identity: digest(30 + i), name: 'synthetic_variable_' + i,
    function_ordinal: 7, scope_identity: digest(40), scope_depth: 0, generation: 0,
    availability: { status: 'value', value: { status: 'unavailable', reason: 'not_represented' } } }));
}
const responseSchema = schema => ({ [SCHEMA.v1]: 'fe2o3-debug-response-v1',
  [SCHEMA.source]: 'fe2o3-debug-source-variable-response-v2', [SCHEMA.resource]: 'fe2o3-debug-resource-response-v1' })[schema];
function fixture(sourceAvailable = true) {
  const pairs = []; let stopped = session(0, 0), nextId = 1;
  function make(fields, body, after = stopped, status = 'ok') {
    const request = { schema: SCHEMA.v1, request_id: nextId++, expected_revision: stopped.revision, ...fields };
    const response = { status, schema: responseSchema(request.schema), request_id: request.request_id,
      operation: request.operation, session: copy(after), ...body };
    const pair = { request, response }; pairs.push(pair); stopped = copy(after); return pair;
  }
  function control(reason, direction, event) {
    const fields = direction ? { operation: 'step', direction, granularity: 'operation', count: 1 }
      : { operation: 'continue', max_events: LIMITS.maxEvents };
    const next = session(stopped.revision + 1, event), stop = { reason, outcome: reason === 'step' ? 'active' : 'failed', exact: true };
    const snapshot = reason === 'step' ? { status: 'captured', snapshot: { anchor: anchor(next), stop, values: ssaRows() } }
      : { status: 'unavailable', reason: 'not_captured' };
    return make(fields, { result: { result: 'control', stop, snapshot, events_advanced: Math.abs(event - stopped.cursor.event_sequence) } }, next);
  }
  function inventory() {
    return make({ schema: SCHEMA.resource, operation: 'query_allocations', expected_snapshot: anchor(stopped),
      page: { max_items: 16, max_scanned: 16 } }, { snapshot: anchor(stopped),
      page: { source_count: 3, scanned: 3, completeness: { status: 'complete' } },
      result: { result: 'allocations', allocations: allocationRows() }, physical_registers: 'not_represented' });
  }
  const stackQuery = { operation: 'inspect_stack', scope: { level: 'dispatch' }, page: { limit: 16 } };
  const ssaQuery = { operation: 'inspect_values', scope: { level: 'dispatch' }, selector: { selector: 'all' }, page: { limit: 64 } };
  const sourceQuery = { schema: SCHEMA.source, operation: 'inspect_source_variables', scope: { level: 'dispatch' },
    frame: 1, selector: { selector: 'all' }, page: { limit: 2 } };
  const memoryQuery = { operation: 'read_memory', allocation: { ordinal: 9, generation: 0 }, byte_offset: 0, byte_len: 16 };
  const unavailable = (query, capability) => make(query, { unavailable: { capability, reason: 'not_captured',
    state_changed: false, detail: 'Synthetic uncaptured terminal control.' } }, stopped, 'unavailable');
  function terminal(control) {
    return { control, stack: unavailable(stackQuery, 'call_stack'), ssa: unavailable(ssaQuery, 'kir_ssa_values'),
      source: make(sourceQuery, { reason: sourceAvailable ? 'checkpoint_not_captured' : 'variables_not_captured' }, stopped, 'unavailable'),
      memory: unavailable(memoryQuery, 'allocation_relative_memory') };
  }
  function checkpoint(control) {
    const stack = make(stackQuery, { result: { result: 'stack', snapshot: anchor(stopped), frames: [
      { frame: 1, function_ordinal: 7, block_ordinal: 4, next_operation: 2, values: { status: 'captured', value_count: 1 } }] } });
    const ssa = make(ssaQuery, { result: { result: 'values', snapshot: anchor(stopped), values: ssaRows() } });
    const inv = inventory(), sourcePages = [];
    if (sourceAvailable) {
      const cursor = { query_identity: digest(100 + stopped.revision), position: 2 };
      sourcePages.push(make(sourceQuery, { snapshot: { ...anchor(stopped), frame: 1, occurrence: 1 }, values: sourceRows().slice(0, 2), next_cursor: cursor }));
      sourcePages.push(make({ ...sourceQuery, page: { limit: 2, cursor } }, { snapshot: { ...anchor(stopped), frame: 1, occurrence: 1 }, values: sourceRows().slice(2) }));
    } else sourcePages.push(make(sourceQuery, { reason: 'variables_not_captured' }, stopped, 'unavailable'));
    const memory = allocationRows().map((row, i) => {
      const model = requests().uninitialized.arguments[i], length = Number(row.capacity_bytes);
      return make({ operation: 'read_memory', allocation: row.allocation, byte_offset: 0, byte_len: length },
        { result: { result: 'memory', snapshot: anchor(stopped), memory: { allocation: row.allocation, byte_offset: 0,
          requested_bytes: length, returned_bytes: length, availability: { status: 'captured', address_space: 'global',
            bytes: model.bytes, initialized: model.initialized, truncated: false } } } });
    });
    return { control, stack, ssa, inventory: inv, sourcePages, memory };
  }
  const initial = control('step', 'forward', 1), initialInventory = inventory();
  const terminals = [terminal(control('fault', undefined, 15))];
  const checkpoints = [checkpoint(control('step', 'reverse', 14))];
  terminals.push(terminal(control('fault', 'forward', 15)));
  checkpoints.push(checkpoint(control('step', 'reverse', 14)));
  const stale = make({ ...ssaQuery, expected_revision: checkpoints[0].control.response.session.revision },
    { error: { stage: 'session', code: 'stale_revision', message: 'Synthetic old revision.', state_changed: false } }, stopped, 'error');
  terminals.push(terminal(control('fault', 'forward', 15)));
  const end = terminal(control('completed', undefined, 15));
  make({ operation: 'terminate' }, { result: { result: 'terminated' } }, session(stopped.revision + 1, 15, true));
  const replay = { initial, initialInventory, terminals, checkpoints, stale, end };
  return { replay, pairs, requestLines: pairs.map(p => JSON.stringify(p.request) + '\n'),
    responseLines: pairs.map(p => JSON.stringify(p.response) + '\n') };
}
const verify = data => verifyReplay(data.replay, summary, operations);
function rejectMutation(change, check = verify) {
  const value = fixture(); change(value); assert.throws(() => check(value));
}
function positive() {
  return { schema: 'fe2o3-simulation-result-v1', status: 'ok', authority: 'observation_only',
    simulated: true, hardware_observed: false, hardware_validation: false, performance_prediction: false,
    target_profile: { identity: 'amdgpu_64_little_endian_v1', index_bits: 64, max_workgroup_invocations: 1024 },
    kir: { sha256: summary.canonical_kir_digest, canonical_bytes: 123 },
    counts: { arguments: 3, shared_buffers: 0, invocations_executed: 4, workgroups_visited: 1, scheduled_slots_visited: 256, steps_executed: 100, events_emitted: 0 },
    schedule: { identity: 'workgroup_major_local_zyx_cooperative_v1', transcript_sha256: digest(90),
      coverage: { decisions: 4, workgroups: 1, barrier_releases: 0, complete: true } },
    conflict_assessment: { status: 'no_conflicts_observed' }, shared_buffers: [],
    arguments: requests().positive.arguments.map(({ kind, ...value }, i) => ({ kind, value: i === 2 ? { ...value, bytes: OUTPUT_AFTER } : value })) };
}
function diagnostic() {
  return { schema: 'fe2o3-simulation-error-v1', status: 'error', stage: 'execution', kind: 'execution_uninitialized_read',
    message: 'Synthetic explicit initialization-mask diagnostic; not a real run.',
    invocation: { global: [0, 0, 0], workgroup: [0, 0, 0], local: [0, 0, 0], workgroup_size: [256, 1, 1],
      workgroup_count: [1, 1, 1], launch_extent: [4, 1, 1] },
    site: { function: 'synthetic_vecadd', function_bytes: 16, function_truncated: false, block: 9, operation: 2 } };
}

test('synthetic complete replay: distinct uncaptured terminal, exact prior values, no authority', () => {
  const data = fixture(), observed = verify(data);
  assert.equal(observed.checkpoints.length, 2); assert.equal(observed.terminals.length, 3);
  assert.equal(observed.checkpoints[0].source.values.length, 3); assert.deepEqual(observed.claims, FALSE_CLAIMS);
  assert.ok(Object.values(observed.claims).every(value => value === false));
  assert.equal(verifyTranscript(data.pairs, data.requestLines, data.responseLines).pairs, data.pairs.length);
});
test('source producer unavailable is explicit and never filled from SSA', () => {
  const data = fixture(false), result = verify(data);
  assert.deepEqual(result.checkpoints[0].source, { status: 'unavailable', reason: 'variables_not_captured' });
  assert.ok(result.checkpoints[0].ssa_values.length > 0);
  verifyTranscript(data.pairs, data.requestLines, data.responseLines);
});
test('fixed request oracle permits only explicit first-byte initialization-bit change', () => {
  verifyRequests(requests());
  for (const change of [
    v => { v.uninitialized.arguments[0].bytes = v.uninitialized.arguments[1].bytes; },
    v => { v.uninitialized.arguments[0].initialized = '0xffff'; },
    v => { v.uninitialized.arguments[1].initialized = '0xfeff'; },
    v => { v.positive.arguments[2].access = 'read_only'; },
    v => { v.uninitialized.grid[0] = 5; },
    v => { v.uninitialized.kernel = 'other'; },
  ]) { const value = requests(); change(value); assert.throws(() => verifyRequests(value)); }
  for (const key of ['positive', 'uninitialized']) for (const width of [64, 255, 257]) {
    const value = requests(); value[key].workgroup = [width, 1, 1];
    assert.throws(() => verifyRequests(value), key + ' exact vecadd workgroup');
  }
});
test('independent dyadic result and both canaries are checked', () => {
  const good = positive(); verifyPositive(good, summary.canonical_kir_digest);
  for (const change of [
    v => { v.arguments[2].value.bytes = requests().positive.arguments[2].bytes; },
    v => { v.arguments[2].value.bytes = OUTPUT_AFTER.slice(0, -2) + 'ff'; },
    v => { v.arguments[0].value.initialized = '0xfeff'; },
    v => { v.arguments[1].value.bytes = v.arguments[0].value.bytes; },
    v => { v.counts.invocations_executed = 3; }, v => { v.schedule.coverage.complete = false; },
    v => { v.kir.sha256 = digest(500); }, v => { v.hardware_observed = true; },
    v => { v.hardware_validation = true; }, v => { v.performance_prediction = true; },
    v => { v.authority = 'production'; },
    v => { v.source_authenticated = true; }, v => { v.conflict_assessment.status = 'incomplete'; },
    v => { v.kir.canonical_bytes = 0; }, v => { v.schedule.coverage.proof_authority = true; },
  ]) { const value = copy(good); change(value); assert.throws(() => verifyPositive(value, summary.canonical_kir_digest)); }
});
test('ordinary partial-grid success requires 256 slots, four cooperative decisions and no delivered events', () => {
  const good = positive(), check = value => verifyPositive(value, summary.canonical_kir_digest);
  check(good);
  for (const change of [
    value => { value.counts.scheduled_slots_visited = 4; },
    value => { value.counts.scheduled_slots_visited = 64; },
    value => { value.counts.scheduled_slots_visited = 255; },
    value => { value.counts.scheduled_slots_visited = 257; },
    value => { value.counts.events_emitted = 1; },
    value => { value.counts.events_emitted = 30; },
    value => { value.counts.steps_executed = 0; },
    value => { value.counts.steps_executed = 1.5; },
    value => { value.counts.steps_executed = LIMITS.maxEvents + 1; },
    value => { value.schedule.identity = 'workgroup_major_local_zyx_serial_v1'; },
    value => { value.schedule.identity = 'workgroup_major_seeded_runnable_cooperative_v1'; },
    value => { value.schedule.coverage.decisions = 0; },
    value => { value.schedule.coverage.decisions = 64; },
    value => { value.schedule.coverage.workgroups = 4; },
    value => { value.schedule.coverage.barrier_releases = 1; },
  ]) { const value = copy(good); change(value); assert.throws(() => check(value)); }
  // Keep only the finite selected-profile step bound, not synthetic exact100.
  for (const steps of [1, LIMITS.maxEvents]) {
    const value = copy(good); value.counts.steps_executed = steps; check(value);
  }
});
test('positive writer schema refuses missing fields, unknown authority and unrequested optional race evidence', () => {
  const good = positive(), check = value => verifyPositive(value, summary.canonical_kir_digest);
  assert.equal(Object.hasOwn(good, 'race_assessment'), false); check(good);
  const required = [
    [[], ['schema', 'status', 'authority', 'simulated', 'hardware_observed', 'hardware_validation',
      'performance_prediction', 'target_profile', 'kir', 'counts', 'schedule', 'conflict_assessment',
      'arguments', 'shared_buffers']],
    [['target_profile'], ['identity', 'index_bits', 'max_workgroup_invocations']],
    [['kir'], ['sha256', 'canonical_bytes']],
    [['counts'], ['arguments', 'shared_buffers', 'invocations_executed', 'workgroups_visited',
      'scheduled_slots_visited', 'steps_executed', 'events_emitted']],
    [['schedule'], ['identity', 'transcript_sha256', 'coverage']],
    [['schedule', 'coverage'], ['decisions', 'workgroups', 'barrier_releases', 'complete']],
    [['conflict_assessment'], ['status']],
  ];
  for (const [route, keys] of required) for (const key of keys) {
    const value = copy(good), object = route.reduce((current, field) => current[field], value);
    delete object[key]; assert.throws(() => check(value), [...route, key].join('.'));
  }
  for (const change of [
    value => { value.proof_authority = true; },
    value => { value.source_authentication = false; },
    value => { value.race_assessment = { status: 'no_races_observed', first_ordered_conflict: null }; },
    value => { value.target_profile.extra = false; },
    value => { value.kir.extra = false; },
    value => { value.counts.extra = 0; },
    value => { value.schedule.extra = false; },
    value => { value.conflict_assessment.extra = false; },
  ]) { const value = copy(good); change(value); assert.throws(() => check(value)); }
});
test('standalone error is exact initialized-model execution diagnosis, not prose-derived range', () => {
  const good = diagnostic(); verifyDiagnostic(good);
  for (const change of [
    v => { v.kind = 'execution_out_of_bounds'; }, v => { v.stage = 'input'; },
    v => { v.invocation.local[0] = 1; }, v => { v.site.operation = null; },
    v => { v.site.function_truncated = true; }, v => { v.site.function_bytes = 10; },
    v => { v.detail = { allocation: 1 }; }, v => { v.source_authenticated = true; },
  ]) { const value = copy(good); change(value); assert.throws(() => verifyDiagnostic(value)); }
  for (const width of [64, 255, 257]) {
    const value = copy(good); value.invocation.workgroup_size = [width, 1, 1];
    assert.throws(() => verifyDiagnostic(value), 'exact source-required vecadd workgroup');
  }
});
test('terminal snapshot cannot masquerade as prior checkpoint or carry values', () => {
  for (const change of [
    d => { d.replay.terminals[0].control.response.result.snapshot = copy(d.replay.checkpoints[0].control.response.result.snapshot); },
    d => { d.replay.terminals[0].control.response.result.snapshot.values = ssaRows(); },
    d => { d.replay.terminals[0].source.response.values = sourceRows(); },
    d => { d.replay.terminals[0].memory.response.result = copy(d.replay.checkpoints[0].memory[0].response.result); },
    d => { d.replay.terminals[0].ssa.response.status = 'ok'; },
    d => { d.replay.terminals[0].stack.response.unavailable.reason = 'not_represented'; },
    d => { d.replay.terminals[0].control.response.result.stop.outcome = 'completed'; },
    d => { d.replay.end.control.response.result.stop.outcome = 'completed'; },
  ]) rejectMutation(change);
});
test('terminal refusals preserve exact revision/configuration and request scope', () => {
  for (const change of [
    d => { d.replay.terminals[0].ssa.response.session.revision++; },
    d => { d.replay.terminals[0].source.response.session.configuration_identity = digest(999); },
    d => { d.replay.terminals[0].memory.response.unavailable.state_changed = true; },
    d => { d.replay.terminals[0].memory.request.allocation.ordinal = 99; },
    d => { d.replay.terminals[0].source.request.frame = 2; },
    d => { d.replay.terminals[0].source.response.reason = 'frame_unavailable'; },
  ]) rejectMutation(change);
});
test('before-load join discovers coordinates, rejects next-operation and operation-kind mismatch', () => {
  const data = fixture(), group = data.replay.checkpoints[0];
  verifyCheckpointGroup(group, summary, operations);
  assert.throws(() => verifyCheckpointGroup(group, summary, [{ ...operations[0], kind: 'store' }]));
  assert.throws(() => verifyCheckpointGroup(group, summary, [{ ...operations[0], coordinate: { function: 0, block: 4, operation: 2 } }]));
  assert.throws(() => verifyCheckpointGroup(group, summary, [...operations, ...operations]));
  rejectMutation(d => { d.replay.checkpoints[0].stack.response.result.frames[0].next_operation = 3; });
  rejectMutation(d => { delete d.replay.checkpoints[0].stack.response.result.frames[0].next_operation; });
});
test('allocation inventory is complete observed three-buffer state, never generation/reuse inference', () => {
  for (const change of [
    d => { d.replay.initialInventory.response.page.completeness.status = 'partial'; },
    d => { d.replay.initialInventory.response.page.next_token = 'synthetic'; },
    d => { d.replay.initialInventory.response.result.allocations[0].allocation.generation = 1; },
    d => { d.replay.initialInventory.response.result.allocations[0].lifetime = 'reused'; },
    d => { d.replay.initialInventory.response.result.allocations[0].capacity_bytes = '160'; },
    d => { d.replay.initialInventory.response.physical_registers = 'captured'; },
  ]) rejectMutation(change);
});
test('all memory bytes, masks, capacities and anchors must match the unchanged request model', () => {
  for (const change of [
    d => { d.replay.checkpoints[0].memory[0].response.result.memory.availability.initialized = '0xffff'; },
    d => { d.replay.checkpoints[0].memory[2].response.result.memory.availability.bytes = OUTPUT_AFTER; },
    d => { d.replay.checkpoints[0].memory[1].response.result.memory.availability.truncated = true; },
    d => { d.replay.checkpoints[0].memory[1].response.result.memory.returned_bytes = 15; },
    d => { d.replay.checkpoints[0].memory[1].request.allocation.ordinal = 9; },
    d => { d.replay.checkpoints[0].memory[0].response.result.snapshot.cursor.event_sequence++; },
    d => { d.replay.checkpoints[0].memory.pop(); },
  ]) rejectMutation(change);
});
test('source and SSA stay separate and source frame refinement is explicit', () => {
  for (const change of [
    d => { delete d.replay.checkpoints[0].sourcePages[0].response.snapshot.frame; },
    d => { d.replay.checkpoints[0].sourcePages[0].response.snapshot.occurrence = 2; },
    d => { d.replay.checkpoints[0].sourcePages[0].response.values[0].ssa_value_ordinal = 10; },
    d => { d.replay.checkpoints[0].sourcePages[0].response.values[0].availability.value = copy(ssaRows()[0].availability); d.replay.checkpoints[0].sourcePages[0].response.values[0].availability.value.provenance = 'authenticated'; },
    d => { d.replay.checkpoints[0].ssa.response.result.values[0].path.root.frame = 2; },
    d => { d.replay.checkpoints[0].control.response.result.snapshot.snapshot.values[0].availability.provenance = 'hardware'; },
  ]) rejectMutation(change);
});
test('source pages refuse missing tail, cursor substitution, duplicate variable and cross-stop state', () => {
  for (const change of [
    d => { d.replay.checkpoints[0].sourcePages.pop(); },
    d => { d.replay.checkpoints[0].sourcePages[1].request.page.cursor.position = 1; },
    d => { const q = d.replay.checkpoints[0].sourcePages[1].request;
      q.page.cursor = { ...q.page.cursor, query_identity: digest(999) }; },
    d => { d.replay.checkpoints[0].sourcePages[1].response.values[0].variable_identity = digest(30); },
    d => { d.replay.checkpoints[1].sourcePages[0] = copy(d.replay.checkpoints[0].sourcePages[0]); },
    d => { d.replay.checkpoints[0].sourcePages[0].response.snapshot.cursor.state_revision++; },
  ]) rejectMutation(change);
});
test('repeat requires same actual checkpoint and entire observed SSA/memory/source rows', () => {
  for (const change of [
    d => { d.replay.checkpoints[1].control.response.session.configuration_identity = digest(88); },
    d => { d.replay.checkpoints[1].stack.response.result.frames[0].function_ordinal = 6; },
    d => { d.replay.checkpoints[1].sourcePages[0].response.values[0].generation = 2; },
    d => { d.replay.checkpoints[1].sourcePages[0].response.values[0].name = 'different'; },
    d => { d.replay.terminals[1].control.request.count = 2; },
    d => { d.replay.checkpoints[1].control.request.direction = 'forward'; },
  ]) rejectMutation(change);
});
test('stale revision control is a refusal with every session field unchanged', () => {
  const data = fixture(); verifyStale(data.replay.stale, data.replay.checkpoints[1].control.response.session);
  for (const change of [
    d => { d.replay.stale.response.error.state_changed = true; },
    d => { d.replay.stale.response.error.code = 'invalid_cursor'; },
    d => { d.replay.stale.response.session.cursor.event_sequence++; },
    d => { d.replay.stale.request.expected_revision = d.replay.stale.response.session.revision; },
  ]) rejectMutation(change);
});
test('full raw-line custody refuses rewriting, substitution, reorder, missing pairs and ID aliasing', () => {
  const check = d => verifyTranscript(d.pairs, d.requestLines, d.responseLines);
  for (const change of [
    d => { d.requestLines[0] = d.requestLines[0].trimEnd(); },
    d => { d.responseLines[0] = '\ufeff' + d.responseLines[0]; },
    d => { d.responseLines[0] = d.responseLines[0].replace('\n', '\r\n'); },
    d => { d.responseLines[0] = d.responseLines[1]; },
    d => { d.pairs[1].request.request_id = 1; },
    d => { d.pairs.pop(); },
    d => { d.responseLines[0] = ' '.repeat(LIMITS.line) + d.responseLines[0]; },
  ]) rejectMutation(change, check);
});
test('raw transcript semantic guard rejects cross-session and forged event distance even after reserialization', () => {
  const data = fixture(); data.pairs[2].response.result.events_advanced++;
  data.responseLines = data.pairs.map(p => JSON.stringify(p.response) + '\n');
  assert.throws(() => verifyTranscript(data.pairs, data.requestLines, data.responseLines));
  const other = fixture(); other.pairs[5].response.session.configuration_identity = digest(700);
  other.pairs[5].response.session.cursor.configuration_identity = digest(700);
  other.responseLines = other.pairs.map(p => JSON.stringify(p.response) + '\n');
  assert.throws(() => verifyTranscript(other.pairs, other.requestLines, other.responseLines));
});
test('no hardware/source/production authority can enter closed public observations', () => {
  for (const change of [
    d => { d.replay.initial.response.session.hardware_observed = true; },
    d => { d.replay.initial.response.session.performance_prediction = true; },
    d => { d.replay.initial.response.result.proof_authority = true; },
    d => { d.replay.checkpoints[0].sourcePages[0].response.source_authenticated = true; },
    d => { d.replay.terminals[0].memory.response.unavailable.physical_address = 4096; },
  ]) rejectMutation(change);
});
test('options require bounded absolute new-target siblings disjoint from source/tool inputs', () => {
  const args = ['--repo', '/work/source', '--bin-dir', '/work/tools', '--cargo', '/toolchain/bin/cargo',
    '--rustc', '/toolchain/bin/rustc', '--export-target', '/work/runs/cargo-scratch', '--output', '/work/runs/observations'];
  const good = options(args); assert.equal(good.output, '/work/runs/observations');
  for (const [index, replacement] of [[11, '/work/source/output'], [9, '/work/runs/observations'],
    [11, '/work/runs/../observations'], [1, 'relative'], [0, '--unknown']]) {
    const invalid = copy(args); invalid[index] = replacement; assert.throws(() => options(invalid));
  }
  assert.throws(() => options([...args, '--output', '/more']));
});
test('capture constants are finite; compiler scratch is not falsely included in observation allowance', () => {
  assert.equal(LIMITS.commands, 128); assert.equal(LIMITS.line, 65536);
  assert.equal(LIMITS.totalMs, 300000); assert.equal(LIMITS.observationBytes, 64 * 1024 ** 2);
  assert.equal(LIMITS.cargoJobs, 2); assert.equal(LIMITS.sourceValues, 64);
});
