// Bounded ordinary-CLI diagnostic client. Reports observations, never source,
// hardware, proof, artifact, launch, or detached-transcript authority.
import assert from 'node:assert/strict';
import { constants } from 'node:fs';
import { open, mkdir, realpath } from 'node:fs/promises';
import { dirname, isAbsolute, resolve, join } from 'node:path';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';

import { BOUNDS, BoundedByteCapture, parseExactJson, encodeExactJson, validateResourceRequestRevision }
  from './ordered-region-debugger-client.mjs';
export { BOUNDS, BoundedByteCapture, parseExactJson, encodeExactJson, validateResourceRequestRevision };
export const SHARED_UTILITY_URL = new URL('./ordered-region-debugger-client.mjs', import.meta.url);
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const integer = (value, max = Number.MAX_SAFE_INTEGER) => {
  assert(Number.isSafeInteger(value) && value >= 0 && value <= max, 'bounded exact integer required');
  return value;
};
const digest = value => {
  assert(typeof value === 'string' && /^[0-9a-f]{64}$/.test(value), 'SHA-256 required');
  return value;
};
const equal = (left, right, message) => assert.deepEqual(left, right, message);
const hex = (value, max) => {
  assert(typeof value === 'string' && /^0x(?:[0-9a-f]{2})*$/.test(value), 'canonical hex bytes required');
  assert((value.length - 2) / 2 <= max, 'hex extent exceeds bound');
  return value;
};
const path = value => {
  assert(typeof value === 'string' && value.length <= 4096 && isAbsolute(value)
    && resolve(value) === value && !value.includes('\0'), 'canonical absolute path required');
  return value;
};

function preconditions() {
  equal(parseExactJson('{"mask":18446744073709551615,"extent":"18446744073709551615"}'),
    { mask: 18446744073709551615n, extent: '18446744073709551615' });
  equal(encodeExactJson({ mask: 18446744073709551615n }), '{"mask":18446744073709551615}');
}
async function measure(filename, maximum) {
  filename = path(filename);
  assert.equal(await realpath(filename), filename, 'input path redirects');
  const file = await open(filename, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const before = await file.stat({ bigint: true });
    assert(before.isFile() && before.size > 0n && before.size <= BigInt(maximum), 'input regular-file extent');
    const hash = createHash('sha256'), buffer = Buffer.alloc(65536);
    let bytes = 0;
    for (;;) {
      const read = await file.read(buffer, 0, Math.min(buffer.length, maximum + 1 - bytes), null);
      if (!read.bytesRead) break;
      bytes += read.bytesRead;
      assert(bytes <= maximum, 'input grew past bound');
      hash.update(buffer.subarray(0, read.bytesRead));
    }
    const after = await file.stat({ bigint: true });
    for (const field of ['dev', 'ino', 'size', 'mtimeNs', 'ctimeNs']) equal(before[field], after[field], 'input changed during measurement');
    equal(BigInt(bytes), before.size);
    return { path: filename, bytes, sha256: hash.digest('hex'), dev: String(before.dev), ino: String(before.ino), mtimeNs: String(before.mtimeNs), ctimeNs: String(before.ctimeNs) };
  } finally { await file.close(); }
}
function validateOptions(options) {
  const copy = structuredClone(options);
  for (const key of ['debuggerPath', 'kirPath', 'requestPath', 'outputDirectory']) path(copy[key]);
  for (const key of ['debuggerFileSha256', 'kirFileSha256', 'requestFileSha256']) digest(copy[key]);
  const e = copy.expected;
  assert(e && typeof e === 'object', 'explicit expectations required');
  for (const key of ['functionOrdinal', 'blockOrdinal', 'operationOrdinal']) integer(e.region[key], 0xffffffff);
  integer(e.rawBlockId, 0xffffffff); // Kept distinct; never used as protocol block ordinal.
  digest(e.canonicalIdentity); // Caller-declared domain identity, not a file SHA.
  assert.equal(e.inputValueIds.length, 3);
  e.inputValueIds.forEach(value => integer(value, 0xffffffff));
  integer(e.resultValueId, 0xffffffff);
  assert.equal(e.beforeInputs.length, 3);
  e.beforeInputs.forEach(value => integer(value, 0xffffffff));
  integer(e.afterResult, 0xffffffff);
  integer(e.lane, 63);
  assert.deepEqual(e.workgroup, [0, 0, 0], 'bounded first-workgroup qualifier only');
  assert.equal(e.wave, 0, 'bounded first-wave qualifier only');
  integer(e.expectedAccessCount, 4096);
  assert.equal(e.expectedAccessCount, 64, 'closed full-wave access qualification');
  integer(e.expectedAccessByteOffset, 4096);
  const m = e.outputMemory;
  integer(m.allocation.ordinal); integer(m.allocation.generation);
  integer(m.byteOffset, 4096);
  hex(m.bytes, 4096); hex(m.initialized, 512);
  const bytes = (m.bytes.length - 2) / 2;
  assert(bytes > 0 && (m.initialized.length - 2) / 2 === Math.ceil(bytes / 8), 'packed initialization extent');
  return copy;
}

class OrdinaryClient {
  constructor(binary, args) {
    this.requests = [];
    this.responses = new BoundedByteCapture(BOUNDS.stdoutBytes);
    this.stderr = new BoundedByteCapture(BOUNDS.stderrBytes);
    this.requestBytes = 0;
    this.responseCount = 0; this.pending = null; this.partial = Buffer.alloc(0);
    this.failure = null; this.truncated = false; this.closed = false; this.drainDeadlineExceeded = false;
    assert.equal(process.platform, 'linux', 'ordered-program smoke process-group policy requires Linux');
    this.child = spawn(binary, args, { detached: true, stdio: ['pipe', 'pipe', 'pipe'], env: { ...process.env, LC_ALL: 'C' } });
    this.exit = new Promise(resolveExit => {
      this.resolveExit = resolveExit;
      this.child.once('close', (code, signal) => {
        this.closed = true; clearTimeout(this.timer); clearTimeout(this.drainTimer);
        if (this.partial.length) this.fail(new Error('unterminated response at EOF'));
        if (this.pending) this.fail(new Error('EOF before requested response'));
        resolveExit({ code, signal });
      });
    });
    this.child.once('error', error => this.fail(error));
    this.child.stdin.on('error', error => this.fail(error));
    this.child.stdout.on('error', error => this.fail(error));
    this.child.stderr.on('error', error => this.fail(error));
    this.child.stdout.on('data', chunk => this.output(chunk));
    this.child.stderr.on('data', chunk => {
      if (this.stderr.append(chunk)) { this.truncated = true; this.fail(new Error('stderr bound exceeded')); }
    });
    this.timer = setTimeout(() => this.fail(new Error('ordinary debugger process deadline exceeded')), BOUNDS.milliseconds);
  }
  fail(error) {
    this.failure ??= error;
    if (this.pending) { const pending = this.pending; this.pending = null; pending.reject(this.failure); }
    if (!this.closed) {
      // detached:true gave this exact child its own group; never target the
      // caller's group or an unresolved PID. Kill descendants holding the pipes.
      if (Number.isInteger(this.child.pid) && this.child.pid > 0) {
        try { process.kill(-this.child.pid, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') this.child.kill('SIGKILL'); }
      } else this.child.kill('SIGKILL');
      this.drainTimer ??= setTimeout(() => {
        this.truncated = true; this.drainDeadlineExceeded = true; this.closed = true;
        clearTimeout(this.timer);
        for (const stream of [this.child.stdin, this.child.stdout, this.child.stderr]) stream.destroy();
        if (this.pending) { const pending = this.pending; this.pending = null; pending.reject(this.failure); }
        this.resolveExit({ code: null, signal: 'SIGKILL', drainDeadlineExceeded: true });
      }, BOUNDS.drainMilliseconds);
    }
  }
  output(chunk) {
    if (this.responses.append(chunk)) { this.truncated = true; this.fail(new Error('stdout bound exceeded')); return; }
    this.partial = Buffer.concat([this.partial, chunk]);
    try {
      for (;;) {
        const newline = this.partial.indexOf(10);
        if (newline < 0) break;
        assert(newline + 1 <= BOUNDS.lineBytes, 'response line bound exceeded');
        const raw = this.partial.subarray(0, newline);
        this.partial = this.partial.subarray(newline + 1);
        assert(raw.length > 0 && this.pending, 'unsolicited or empty debugger response');
        const value = parseExactJson(new TextDecoder('utf-8', { fatal: true }).decode(raw));
        const pending = this.pending; this.pending = null;
        this.responseCount++; pending.resolve(value);
      }
      assert(this.partial.length < BOUNDS.lineBytes, 'response line bound exceeded');
    } catch (error) { this.fail(error); }
  }
  async send(request) {
    if (this.failure) throw this.failure;
    assert(!this.closed && !this.pending && this.requests.length < BOUNDS.commands, 'command/process bound');
    const bytes = Buffer.from(encodeExactJson(request) + '\n');
    assert(bytes.length <= BOUNDS.lineBytes && this.requestBytes + bytes.length <= BOUNDS.requestBytes, 'request byte bound');
    this.requests.push(bytes); this.requestBytes += bytes.length;
    return new Promise((resolveResponse, reject) => {
      this.pending = { resolve: resolveResponse, reject };
      this.child.stdin.write(bytes, error => { if (error) this.fail(error); });
    });
  }
  async close() {
    this.child.stdin.end();
    const result = await this.exit;
    if (this.failure) throw this.failure;
    assert.equal(result.code, 0, 'debugger exit code'); assert.equal(result.signal, null);
    assert.equal(this.responseCount, this.requests.length, 'one response per exact request');
    return result;
  }
}
const schemas = {
  debug: ['fe2o3-debug-request-v1', 'fe2o3-debug-response-v1'],
  resource: ['fe2o3-debug-resource-request-v1', 'fe2o3-debug-resource-response-v1'],
  diagnosis: ['fe2o3-debug-diagnosis-request-v2', 'fe2o3-debug-diagnosis-response-v2'],
};
function anchorFrom(response) {
  assert.equal(response.result?.snapshot?.status, 'captured', 'actual checkpoint required');
  const anchor = response.result.snapshot.snapshot.anchor;
  equal(anchor.cursor, response.session.cursor, 'checkpoint cursor differs from exact response session');
  return anchor;
}
function scopeFrom(e) { return { level: 'lane', workgroup: e.workgroup, wave: e.wave, lane: e.lane }; }
function pathFor(e, value) {
  return { root: { kind: 'ssa', function_ordinal: e.region.functionOrdinal, frame: 1, value_ordinal: value }, components: [] };
}
function checkValues(response, e, ids, values, anchor) {
  equal(response.result.result, 'values');
  equal(response.result.snapshot, { ...anchor, frame: 1, occurrence: 1 });
  equal(response.result.values.length, ids.length);
  assert(!response.result.next_cursor, 'selected values unexpectedly paginated');
  ids.forEach((id, index) => {
    const row = response.result.values[index];
    equal(row.path, pathFor(e, id));
    equal(row.availability, {
      status: 'captured', value_type: { kind: 'integer', signed: false, bits: 32 },
      value: { encoding: 'bits', bits: '0x' + values[index].toString(16).padStart(8, '0') },
      provenance: 'simulated_observation',
    });
  });
}
function checkAnchor(anchor, e, site) {
  equal(anchor.site.kir, site, 'protocol roster coordinate differs');
  equal(anchor.site.source, { status: 'unavailable', reason: 'requires_authenticated_map' });
  const scope = anchor.scope;
  for (const key of ['level', 'workgroup', 'wave', 'lane']) equal(scope[key], scopeFrom(e)[key]);
  equal(scope.wave_width, 64); equal(scope.interpretation, 'logical_visualization');
  equal(scope.logical_workitem, [e.lane, 0, 0]);
  equal(scope.active_mask, 0xffffffffffffffffn);
}

async function exercise(client, e) {
  let revision = 0, configuration = null, session = null, id = 0;
  const observations = {};
  async function command(operation, fields = {}, { family = 'debug', status = 'ok', mutate = false, errorCode, unavailableReason } = {}) {
    const previous = session;
    const request = { schema: schemas[family][0], request_id: ++id, expected_revision: revision, operation, ...fields };
    if (family === 'resource') validateResourceRequestRevision(request);
    const response = await client.send(request);
    equal(response.schema, schemas[family][1]); equal(response.request_id, id); equal(response.operation, operation);
    equal(response.status, status, 'unexpected debugger response: ' + encodeExactJson(response).slice(0, 1200));
    assert(response.session, 'missing simulator session');
    const current = response.session;
    equal(current.backend, 'cpu_kir_simulator'); equal(current.execution_kind, 'cpu_kir_simulation');
    equal(current.simulated, true); equal(current.hardware_observed, false); equal(current.performance_prediction, false);
    digest(current.configuration_identity);
    configuration ??= current.configuration_identity;
    equal(current.configuration_identity, configuration, 'session configuration changed');
    equal(current.cursor.configuration_identity, configuration);
    equal(current.cursor.state_revision, current.revision);
    equal(current.revision, revision + (mutate ? 1 : 0), 'unexpected revision change');
    if (!mutate && previous) equal(current, previous, 'read-only operation changed session');
    if (errorCode) { equal(response.error.code, errorCode); equal(response.error.state_changed, false); }
    if (unavailableReason) { equal(response.unavailable.reason, unavailableReason); equal(response.unavailable.state_changed, false); }
    revision = current.revision; session = current;
    return response;
  }
  const capabilities = await command('discover_capabilities');
  for (const [name, reason] of [['register_values', 'not_represented'], ['source_sites', 'requires_authenticated_map'], ['hardware_wave_state', 'logical_visualization_only']]) {
    const item = capabilities.result.capabilities.find(row => row.name === name);
    equal(item, { name, availability: 'unavailable', reason });
  }
  const site = { function_ordinal: e.region.functionOrdinal, block_ordinal: e.region.blockOrdinal,
    point: { kind: 'operation', operation_ordinal: e.region.operationOrdinal } };
  const breakpointSpec = { client_label: 'whole-program-before', enabled: true,
    scope: scopeFrom(e), kind: { kind: 'site', site, phase: 'before_operation' } };
  await command('set_breakpoints', { breakpoints: [breakpointSpec] }, { mutate: true });
  const listed = await command('list_breakpoints', { page: { limit: 16 } });
  equal(listed.result.breakpoints.length, 1);
  equal(listed.result.breakpoints[0].spec, breakpointSpec);
  equal(listed.result.breakpoints[0].hit_count, 0);
  const breakpointId = listed.result.breakpoints[0].breakpoint_id;
  const before = await command('continue', { max_events: 16384 }, { mutate: true });
  equal(before.result.stop.reason, 'breakpoint'); equal(before.result.stop.breakpoint_id, breakpointId);
  equal(before.result.stop.exact, true); equal(before.result.stop.outcome, 'active');
  observations.before = anchorFrom(before); checkAnchor(observations.before, e, site);
  const valueRequest = ids => ({ scope: scopeFrom(e), frame: 1, selector: { selector: 'paths', paths: ids.map(value => pathFor(e, value)) }, page: { limit: 16 } });
  checkValues(await command('inspect_values', valueRequest(e.inputValueIds)), e, e.inputValueIds, e.beforeInputs, observations.before);
  const absent = await command('inspect_values', valueRequest([e.resultValueId]));
  equal(absent.result.snapshot, { ...observations.before, frame: 1, occurrence: 1 });
  equal(absent.result.values, [{ path: pathFor(e, e.resultValueId), availability: { status: 'unavailable', reason: 'not_in_scope' } }]);
  await command('inspect_values', { scope: scopeFrom(e), selector: { selector: 'roots', roots: ['register'] }, page: { limit: 16 } },
    { status: 'unavailable', unavailableReason: 'not_represented' });
  const diagnosis = await command('diagnose', { filter: {}, page: { limit: 16 } }, { family: 'diagnosis', status: 'error', errorCode: 'unsupported_schema' });
  assert(diagnosis.error.message.includes('cannot represent raw canonical KIR V17'));
  assert(!encodeExactJson(diagnosis).includes('canonical_kir_v7'));
  const after = await command('step', { direction: 'forward', granularity: 'event', count: 1 }, { mutate: true });
  equal(after.result.stop.exact, true); equal(after.result.stop.outcome, 'active');
  observations.after = anchorFrom(after); checkAnchor(observations.after, e, site);
  equal(observations.after.cursor.event_sequence, observations.before.cursor.event_sequence + 1, 'whole-program pair not adjacent');
  checkValues(await command('inspect_values', valueRequest([e.resultValueId])), e, [e.resultValueId], [e.afterResult], observations.after);
  await command('seek', { cursor: observations.before.cursor }, { status: 'error', errorCode: 'invalid_cursor' });
  // A structurally valid stale request binds its own old revision. Mixing an
  // old snapshot with the current request revision is a framing error instead.
  await command('query_allocations', { expected_revision: observations.before.cursor.state_revision,
    expected_snapshot: observations.before, page: { max_items: 16, max_scanned: 128 } },
    { family: 'resource', status: 'error', errorCode: 'stale_revision' });
  // Separately exercise a current-revision cursor naming an earlier event.
  const staleEvent = { ...observations.before,
    cursor: { ...observations.before.cursor, state_revision: revision } };
  await command('query_allocations', { expected_snapshot: staleEvent, page: { max_items: 16, max_scanned: 128 } },
    { family: 'resource', status: 'error', errorCode: 'invalid_cursor' });
  const reverse = await command('step', { direction: 'reverse', granularity: 'event', count: 1 }, { mutate: true });
  observations.reverseBefore = anchorFrom(reverse); checkAnchor(observations.reverseBefore, e, site);
  equal(observations.reverseBefore.cursor.event_sequence, observations.before.cursor.event_sequence);
  assert.notEqual(observations.reverseBefore.cursor.state_revision, observations.before.cursor.state_revision);
  checkValues(await command('inspect_values', valueRequest(e.inputValueIds)), e, e.inputValueIds, e.beforeInputs, observations.reverseBefore);
  const again = await command('step', { direction: 'forward', granularity: 'event', count: 1 }, { mutate: true });
  observations.repeatedAfter = anchorFrom(again); checkAnchor(observations.repeatedAfter, e, site);
  checkValues(await command('inspect_values', valueRequest([e.resultValueId])), e, [e.resultValueId], [e.afterResult], observations.repeatedAfter);
  await command('remove_breakpoints', { breakpoint_ids: [breakpointId] }, { mutate: true });
  const ended = await command('continue', { max_events: 16384 }, { mutate: true });
  equal(ended.result.stop.exact, true); equal(ended.result.stop.outcome, 'completed');
  const final = await command('step', { direction: 'reverse', granularity: 'operation', count: 1 }, { mutate: true });
  observations.final = anchorFrom(final);
  async function resourcePages(operation, fields, resultKey) {
    const rows = [], tokens = new Set();
    let token, sourceCount, scanned = 0, emptyPages = 0, pageCount = 0, consumedTokenRefused = false;
    for (;;) {
      assert(++pageCount <= BOUNDS.resourcePages, 'resource page bound');
      const page = { max_items: BOUNDS.resourceItems, max_scanned: BOUNDS.resourceScanned, ...(token ? { token } : {}) };
      const response = await command(operation, { expected_snapshot: observations.final, ...fields, page }, { family: 'resource' });
      equal(response.snapshot, observations.final); equal(response.physical_registers, 'not_represented');
      assert(response.page.scanned <= page.max_scanned);
      integer(response.page.scanned, page.max_scanned);
      integer(response.page.source_count, 16384);
      sourceCount ??= response.page.source_count;
      equal(response.page.source_count, sourceCount, 'resource history extent changed');
      equal(response.page.completeness.status, 'complete');
      const batch = response.result[resultKey];
      assert(Array.isArray(batch) && batch.length <= page.max_items);
      if (!batch.length) emptyPages++;
      rows.push(...batch); scanned += response.page.scanned;
      assert(scanned <= sourceCount, 'resource scan repeated source records');
      if (token && !consumedTokenRefused) {
        await command(operation, { expected_snapshot: observations.final, ...fields, page },
          { family: 'resource', status: 'error', errorCode: 'invalid_cursor' });
        consumedTokenRefused = true;
      }
      const next = response.page.next_token;
      if (next === undefined) break;
      assert(typeof next === 'string' && next.length > 0 && next.length <= 128 && !tokens.has(next), 'invalid/repeated resource continuation');
      tokens.add(next); token = next;
    }
    equal(scanned, sourceCount, 'resource pagination ended before the bounded history was scanned');
    return { rows, sourceCount, scanned, emptyPages, pages: pageCount, consumedTokenRefused };
  }
  observations.allocations = await resourcePages('query_allocations', {}, 'allocations');
  const allocation = observations.allocations.rows.find(row => encodeExactJson(row.allocation) === encodeExactJson(e.outputMemory.allocation));
  assert(allocation && allocation.address_space === 'global' && allocation.snapshot_bytes_available === true && allocation.initialization_available === true);
  for (const key of ['owning_scope', 'lifetime', 'physical_base']) equal(allocation[key], 'not_represented');
  const memory = await command('read_memory', { allocation: e.outputMemory.allocation, byte_offset: e.outputMemory.byteOffset, byte_len: (e.outputMemory.bytes.length - 2) / 2 });
  equal(memory.result.snapshot, observations.final);
  equal(memory.result.memory.allocation, e.outputMemory.allocation);
  equal(memory.result.memory.byte_offset, e.outputMemory.byteOffset);
  equal(memory.result.memory.requested_bytes, (e.outputMemory.bytes.length - 2) / 2);
  equal(memory.result.memory.returned_bytes, (e.outputMemory.bytes.length - 2) / 2);
  equal(memory.result.memory.availability, { status: 'captured', address_space: 'global', bytes: e.outputMemory.bytes, initialized: e.outputMemory.initialized, truncated: false });
  observations.memory = memory.result.memory;
  observations.accesses = await resourcePages('query_memory_accesses', { filter: { scope: { level: 'dispatch' } } }, 'accesses');
  equal(observations.accesses.rows.length, e.expectedAccessCount);
  equal(observations.accesses.consumedTokenRefused, true, 'consumed continuation negative not exercised');
  const eventIds = new Set(); let previousSequence = 0;
  for (const [lane, row] of observations.accesses.rows.entries()) {
    equal(row.allocation, e.outputMemory.allocation); equal(row.access, 'write_committed');
    equal(row.address_space, 'global');
    equal(row.range, { byte_offset: String(e.expectedAccessByteOffset + lane * 4), byte_len: '4' });
    equal(row.source_association, 'not_represented');
    const occurrence = row.occurrence;
    integer(occurrence.record_ordinal, 16383); integer(occurrence.event_sequence, 16384);
    equal(occurrence.event_sequence, occurrence.record_ordinal + 1);
    assert(occurrence.event_sequence > previousSequence && occurrence.event_sequence <= observations.final.cursor.event_sequence);
    assert(occurrence.record_ordinal < observations.accesses.sourceCount);
    equal(occurrence.scope, { ...scopeFrom(e), lane, logical_workitem: [lane, 0, 0],
      active_mask: 0xffffffffffffffffn, wave_width: 64, interpretation: 'logical_visualization' });
    assert(!eventIds.has(occurrence.event_sequence), 'duplicated access occurrence');
    eventIds.add(occurrence.event_sequence); previousSequence = occurrence.event_sequence;
  }
  await command('terminate', {}, { mutate: true });
  return { configurationIdentity: configuration, commands: id, observations,
    foreignSessionTokenTested: false, physicalInstructionSteppingTested: false };
}

/**
 * All paths must be absolute, existing regular no-symlink inputs; outputDirectory
 * must be fresh beneath an existing real directory. File SHA pins are plain byte
 * hashes. expected.canonicalIdentity is separate caller-declared metadata, not
 * authenticated by this JSONL session or confused with expected.region ordinals.
 */
export async function runDebuggerQualification(options) {
  preconditions();
  const o = validateOptions(options);
  equal(await realpath(dirname(o.outputDirectory)), dirname(o.outputDirectory), 'output parent redirects');
  await mkdir(o.outputDirectory, { mode: 0o700 });
  const handles = {};
  try {
    for (const name of ['requests.jsonl', 'responses.jsonl', 'stderr.log', 'observation.json']) handles[name] = await open(join(o.outputDirectory, name), 'wx', 0o600);
  } catch (error) {
    await Promise.allSettled(Object.values(handles).map(handle => handle.close()));
    throw error; // Preserve any create-new files; no process has been spawned.
  }
  let client, report, failure;
  const inputs = {};
  try {
    for (const [key, filename, maximum, pin] of [
      ['debugger', o.debuggerPath, BOUNDS.executableBytes, o.debuggerFileSha256],
      ['kir', o.kirPath, BOUNDS.kirBytes, o.kirFileSha256],
      ['request', o.requestPath, BOUNDS.simulationRequestBytes, o.requestFileSha256],
    ]) { inputs[key] = await measure(filename, maximum); equal(inputs[key].sha256, pin, key + ' bytes differ from caller pin'); }
    const args = ['sim', '--diagnostic-kir-v17', o.kirPath, '--request', o.requestPath, '--wave-width', '64', '--protocol', 'jsonl'];
    client = new OrdinaryClient(o.debuggerPath, args);
    const qualified = await exercise(client, o.expected);
    const exit = await client.close();
    for (const [key, maximum] of [['debugger', BOUNDS.executableBytes], ['kir', BOUNDS.kirBytes], ['request', BOUNDS.simulationRequestBytes]]) {
      equal(await measure(inputs[key].path, maximum), inputs[key], 'measured input changed during ordinary CLI session');
    }
    report = { status: 'passed', invocation: { program: o.debuggerPath, args }, exit, ...qualified };
  } catch (error) {
    failure = error;
    let exit = null;
    if (client) { client.fail(error); exit = await client.exit; }
    report = { status: 'failed', exit, error: String(error.message).slice(0, 2048) };
  } finally {
    try {
    const exact = {
      'requests.jsonl': Buffer.concat(client?.requests ?? []),
      'responses.jsonl': client?.responses.snapshot(BOUNDS.stdoutBytes) ?? Buffer.alloc(0),
      'stderr.log': client?.stderr.snapshot(BOUNDS.stderrBytes) ?? Buffer.alloc(0),
    };
    const artifacts = {};
    for (const [name, bytes] of Object.entries(exact)) {
      await handles[name].writeFile(bytes); artifacts[name] = { bytes: bytes.length, sha256: sha(bytes) };
    }
    report = {
      kind: 'ordered_program_debugger_observation_draft_v1', ...report,
      inputs, expected: o.expected, bounds: BOUNDS, artifacts,
      exact_streams_truncated: client?.truncated ?? false,
      forced_drain_deadline_exceeded: client?.drainDeadlineExceeded ?? false,
      source_authentication: false, hardware_observed: false, physical_register_values: 'unavailable',
      instruction_microsteps: 'unavailable', grants_artifact_or_launch_authority: false,
      grants_proof_or_resume_authority: false,
    };
    await handles['observation.json'].writeFile(encodeExactJson(report) + '\n');
    } finally {
      const closes = await Promise.allSettled(Object.values(handles).map(handle => handle.close()));
      const failedClose = closes.find(result => result.status === 'rejected');
      if (failedClose) throw failedClose.reason;
    }
  }
  if (failure) {
    const error = new Error('Ordinary debugger qualification failed; exact evidence retained at ' + o.outputDirectory + ': ' + report.error);
    error.report = report; throw error;
  }
  return report;
}

