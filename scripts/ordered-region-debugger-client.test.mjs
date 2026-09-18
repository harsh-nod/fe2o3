// Pure test-only controls. These tests DO NOT execute a debugger, generate
// backend responses, qualify source, or grant any artifact/runtime authority.
import assert from 'node:assert/strict';
import test from 'node:test';
import { createHash } from 'node:crypto';
import { mkdtemp, readFile, readdir, stat, writeFile } from 'node:fs/promises';
import { join } from 'node:path';
import { tmpdir } from 'node:os';
import { BOUNDS, BoundedByteCapture, encodeExactJson, parseExactJson, runDebuggerQualification,
  validateResourceRequestRevision } from './ordered-region-debugger-client.mjs';

const testRoot = tmpdir();
const sha = bytes => createHash('sha256').update(bytes).digest('hex');

test('byte capture copies a small view of giant backing storage without aliasing', () => {
  const giant = Buffer.alloc(1024 * 1024, 0xa5);
  giant.set([7, 8, 9], 900000);
  const view = giant.subarray(900000, 900003);
  assert.equal(view.buffer.byteLength, 1024 * 1024);
  const capture = new BoundedByteCapture(5);
  assert.equal(capture.append(view), false);
  assert.equal(capture.byteLength, 3); assert.equal(capture.backingByteLength, 5);
  giant.fill(0);
  const snapshot = capture.snapshot(5);
  assert.deepEqual([...snapshot], [7, 8, 9], 'incoming mutation cannot alter captured bytes');
  assert.equal(snapshot.buffer.byteLength, 3, 'snapshot owns only its explicit returned bytes');
  assert.notEqual(snapshot.buffer, giant.buffer);
  snapshot.fill(42);
  assert.deepEqual([...capture.snapshot(3)], [7, 8, 9], 'returned snapshot cannot alias retained storage');
  assert.throws(() => capture.snapshot(2), /snapshot byte bound/);
});

test('byte capture retains exact stream bytes through partial chunks and an exact cap', () => {
  // Arbitrary bytes, not a fabricated backend response; split a UTF-8 sequence.
  const expected = Buffer.from([0, 0xc3, 0xa9, 10, 255]);
  const capture = new BoundedByteCapture(expected.length);
  assert.equal(capture.append(expected.subarray(0, 2)), false);
  assert.equal(capture.append(expected.subarray(2)), false);
  assert.equal(capture.append(expected.subarray(3, 3)), false);
  assert.equal(capture.byteLength, expected.length);
  assert.equal(capture.backingByteLength, expected.length);
  assert.deepEqual(capture.snapshot(expected.length), expected);
  assert.equal(capture.append(Buffer.from([99])), true, 'first byte beyond cap is overflow');
  assert.deepEqual(capture.snapshot(expected.length), expected);
});

test('oversized and repeated post-cap chunks retain only a copied bounded prefix', () => {
  const giant = Buffer.alloc(1024 * 1024, 0xa5);
  giant.set([1, 2, 3, 4]);
  const capture = new BoundedByteCapture(4);
  assert.equal(capture.append(giant), true);
  giant.fill(0);
  const emptyView = giant.subarray(900000, 900000);
  for (let index = 0; index < 10000; index++) {
    assert.equal(capture.append(giant), true);
    assert.equal(capture.append(emptyView), false);
  }
  assert.equal(capture.byteLength, 4); assert.equal(capture.backingByteLength, 4);
  assert.deepEqual([...capture.snapshot(4)], [1, 2, 3, 4]);
});

test('zero-cap captures never retain nonempty or empty incoming backing views', () => {
  const giant = Buffer.alloc(1024 * 1024, 0xa5);
  const capture = new BoundedByteCapture(0);
  for (let index = 0; index < 10000; index++) {
    assert.equal(capture.append(giant.subarray(0, 0)), false);
    assert.equal(capture.append(giant), true);
  }
  assert.equal(capture.byteLength, 0); assert.equal(capture.backingByteLength, 0);
  const snapshot = capture.snapshot(0);
  assert.equal(snapshot.byteLength, 0); assert.equal(snapshot.buffer.byteLength, 0);
});

test('byte capture and independent snapshot allocation limits are explicit', () => {
  for (const maximum of [-1, 0.5, Number.NaN, undefined, '2', BOUNDS.stdoutBytes + 1])
    assert.throws(() => new BoundedByteCapture(maximum), /bounded exact integer/);
  const capture = new BoundedByteCapture(2);
  for (const value of [undefined, null, 'ab', [1, 2], {}]) assert.throws(() => capture.append(value), /byte chunk required/);
  assert.equal(capture.append(new Uint8Array([1, 2])), false);
  for (const maximum of [-1, 0.5, undefined, 3]) assert.throws(() => capture.snapshot(maximum), /bounded exact integer/);
  assert.throws(() => capture.snapshot(1), /snapshot byte bound/);
  assert.deepEqual([...capture.snapshot(2)], [1, 2]);
  assert.equal(capture.backingByteLength, 2);
});

test('unsigned JSON preserves safe integers, MAXu64 and decimal strings exactly', () => {
  const raw = '{"zero":0,"safe":9007199254740991,"aboveSafe":9007199254740992,"max":18446744073709551615,"extent":"18446744073709551615","leadingString":"0008"}';
  const parsed = parseExactJson(raw);
  assert.deepEqual(parsed, {
    zero: 0, safe: Number.MAX_SAFE_INTEGER, aboveSafe: 9007199254740992n,
    max: 18446744073709551615n, extent: '18446744073709551615', leadingString: '0008',
  });
  assert.equal(encodeExactJson(parsed), raw);
});

test('synthetic coordinate metadata keeps roster and raw-block lexemes separate', () => {
  // Generic scalar metadata only, NOT a fabricated debugger response.
  const raw = '{"roster":{"function_ordinal":0,"block_ordinal":1,"operation_ordinal":0},"raw_block":8,"ids":[12,2,4],"result_id":13,"mask":18446744073709551615,"revision":17,"record_index":2,"event_sequence":3,"decimal_offset":"4"}';
  const parsed = parseExactJson(raw);
  assert.equal(parsed.roster.block_ordinal, 1);
  assert.equal(parsed.raw_block, 8);
  assert.notEqual(parsed.record_index, parsed.event_sequence);
  assert.equal(parsed.mask, 18446744073709551615n);
  assert.equal(parsed.decimal_offset, '4');
  assert.equal(encodeExactJson(parsed), raw);
  const unused = { ...parsed, raw_block: 2, ids: [12, 2, 0] };
  assert.deepEqual(parseExactJson(encodeExactJson(unused)), unused);
});

test('malformed, negative, fractional, exponent and overflowing wire numbers refuse', () => {
  for (const raw of [
    '-1', '-0', '1.0', '1.5', '1e0', '1E3', '0e0', '01', '+1',
    '18446744073709551616', '999999999999999999999999999999999999999',
    'NaN', 'Infinity', '', '{', '[0,]', '{"x":}', '{"x":0} trailing',
    '{"x":-1}', '{"x":1e2}', '{"x":18446744073709551616}',
  ]) assert.throws(() => parseExactJson(raw), undefined, raw);
  for (const value of [null, undefined, Buffer.from('0'), 0, {}]) {
    assert.throws(() => parseExactJson(value), /JSON line bound/);
  }
});

test('encoder refuses rounded unsafe Number values and out-of-range numeric types', () => {
  for (const value of [
    -1, 1.5, Number.NaN, Number.POSITIVE_INFINITY, Number.NEGATIVE_INFINITY,
    Number.MAX_SAFE_INTEGER + 1, Number(18446744073709551615n),
    -1n, 18446744073709551616n,
  ]) assert.throws(() => encodeExactJson({ value }));
  assert.equal(encodeExactJson({ value: 9007199254740992n }), '{"value":9007199254740992}');
  assert.equal(encodeExactJson({ value: '1e3' }), '{"value":"1e3"}');
});

test('JSON line limits count UTF-8 bytes and include exact boundary cases', () => {
  const ascii = '"' + 'a'.repeat(BOUNDS.lineBytes - 2) + '"';
  assert.equal(Buffer.byteLength(ascii), BOUNDS.lineBytes);
  assert.equal(parseExactJson(ascii).length, BOUNDS.lineBytes - 2);
  assert.throws(() => parseExactJson(ascii + ' '), /JSON line bound/);
  const utf8 = '"' + 'é'.repeat((BOUNDS.lineBytes - 2) / 2) + '"';
  assert.equal(Buffer.byteLength(utf8), BOUNDS.lineBytes);
  assert(parseExactJson(utf8).length < BOUNDS.lineBytes);
  assert.throws(() => parseExactJson(utf8.slice(0, -1) + 'é"'), /JSON line bound/);
});

test('JSON graph depth, breadth and total-node limits are independently bounded', () => {
  const nested = depth => '['.repeat(depth) + '0' + ']'.repeat(depth);
  assert.doesNotThrow(() => parseExactJson(nested(32)));
  assert.throws(() => parseExactJson(nested(33)), /JSON graph bound/);
  assert.doesNotThrow(() => parseExactJson(JSON.stringify(Array(4096).fill(0))));
  assert.throws(() => parseExactJson(JSON.stringify(Array(4097).fill(0))), /JSON collection bound/);
  // Root + eight arrays + 32,759 scalar leaves = exactly 32,768 nodes.
  const exact = [...Array.from({ length: 7 }, () => Array(4096).fill(0)), Array(4087).fill(0)];
  assert.doesNotThrow(() => parseExactJson(JSON.stringify(exact)));
  exact[7].push(0);
  assert.throws(() => parseExactJson(JSON.stringify(exact)), /JSON graph bound/);
});

function syntheticResourceRequest(operation = 'query_allocations') {
  // Request metadata for a pure consistency check, not a backend response or
  // an admitted checkpoint. Passing this check does not validate an anchor.
  return {
    schema: 'fe2o3-debug-resource-request-v1', request_id: 12,
    operation, expected_revision: 2,
    expected_snapshot: {
      cursor: { configuration_identity: '1'.repeat(64), event_sequence: 3, state_revision: 2 },
      scope: { level: 'lane', workgroup: [0, 0, 0], wave: 0, lane: 0,
        logical_workitem: [0, 0, 0], active_mask: 0xffffffffffffffffn,
        wave_width: 64, interpretation: 'logical_visualization' },
      site: { kir: { function_ordinal: 0, block_ordinal: 1,
        point: { kind: 'operation', operation_ordinal: 0 } },
      source: { status: 'unavailable', reason: 'requires_authenticated_map' } },
    },
    page: { max_items: 16, max_scanned: 128 },
    ...(operation === 'query_memory_accesses' ? { filter: { scope: { level: 'dispatch' } } } : {}),
  };
}

test('mixed resource request and snapshot revisions refuse before transport without mutation', () => {
  for (const operation of ['query_allocations', 'query_memory_accesses']) {
    for (const [requestRevision, snapshotRevision] of [[3, 2], [2, 3]]) {
      const request = syntheticResourceRequest(operation);
      request.expected_revision = requestRevision;
      request.expected_snapshot.cursor.state_revision = snapshotRevision;
      const before = encodeExactJson(request);
      assert.throws(() => validateResourceRequestRevision(request),
        /resource request revision must match its serialized snapshot revision/);
      assert.equal(encodeExactJson(request), before);
    }
  }
});

test('old-revision and current-revision stale-event requests remain separately well formed', () => {
  const currentRevision = 3;
  const currentEventSequence = 4;
  for (const operation of ['query_allocations', 'query_memory_accesses']) {
    const oldRequest = syntheticResourceRequest(operation);
    const originalBytes = encodeExactJson(oldRequest);
    assert.doesNotThrow(() => validateResourceRequestRevision(oldRequest));
    assert(oldRequest.expected_revision < currentRevision);

    // This deliberately rejected query changes only the request revision and
    // matching nested cursor revision. It never relabels the old event as the
    // current event and never changes the retained old request/anchor.
    const staleEventRequest = {
      ...oldRequest, expected_revision: currentRevision,
      expected_snapshot: { ...oldRequest.expected_snapshot,
        cursor: { ...oldRequest.expected_snapshot.cursor, state_revision: currentRevision } },
    };
    assert.doesNotThrow(() => validateResourceRequestRevision(staleEventRequest));
    assert.equal(staleEventRequest.expected_snapshot.cursor.event_sequence, 3);
    assert.notEqual(staleEventRequest.expected_snapshot.cursor.event_sequence, currentEventSequence);
    const restored = structuredClone(staleEventRequest);
    restored.expected_revision = oldRequest.expected_revision;
    restored.expected_snapshot.cursor.state_revision = oldRequest.expected_snapshot.cursor.state_revision;
    assert.deepEqual(restored, oldRequest, 'only the two revision fields may change');
    assert.equal(encodeExactJson(oldRequest), originalBytes, 'old evidence must remain unchanged');
    assert.deepEqual(parseExactJson(encodeExactJson(staleEventRequest)), staleEventRequest);
    // These are pure request-shape controls. Only a real protocol run can
    // establish the separate stale_revision/invalid_cursor response outcomes.
  }
});

async function negativeInputs() {
  const root = await mkdtemp(join(testRoot, 'fe2o3-ordered-region-debugger-controls.'));
  // Deliberately non-executable text and noncanonical payloads: the selected
  // file-pin precondition MUST refuse before any executable or parser is used.
  const entries = {
    debuggerPath: ['not-an-executable.txt', Buffer.from('synthetic file-pin control; never execute\n')],
    kirPath: ['not-a-kernel.bin', Buffer.from('synthetic precondition-only KIR bytes\n')],
    requestPath: ['not-a-request.json', Buffer.from('synthetic precondition-only request bytes\n')],
  };
  const result = { root };
  for (const [key, [name, bytes]] of Object.entries(entries)) {
    result[key] = join(root, name);
    await writeFile(result[key], bytes, { flag: 'wx', mode: 0o600 });
    result[key.replace('Path', 'FileSha256')] = sha(bytes);
  }
  result.expected = {
    region: { functionOrdinal: 0, blockOrdinal: 1, operationOrdinal: 0 },
    rawBlockId: 8, canonicalIdentity: '1'.repeat(64), inputValueIds: [12, 2, 4],
    resultValueId: 13, beforeInputs: [19, 23, 42], afterResult: 46,
    lane: 0, workgroup: [0, 0, 0], wave: 0,
    expectedAccessCount: 64, expectedAccessByteOffset: 4,
    outputMemory: { allocation: { ordinal: 1, generation: 0 }, byteOffset: 0,
      bytes: '0xa5', initialized: '0x00' },
  };
  console.log('Retained synthetic precondition controls (no processes): ' + root);
  return result;
}

test('wrong debugger/KIR/request pins fail before spawning and preserve exact empty streams', { timeout: 10000 }, async () => {
  const options = await negativeInputs();
  assert.equal((await stat(options.debuggerPath)).mode & 0o111, 0, 'control is never executable');
  const fields = ['debuggerFileSha256', 'kirFileSha256', 'requestFileSha256'];
  const measuredKeys = ['debugger', 'kir', 'request'];
  for (const [index, field] of fields.entries()) {
    const outputDirectory = join(options.root, 'wrong-' + field);
    const invocation = { ...options, outputDirectory, [field]: '0'.repeat(64) };
    let failure;
    await assert.rejects(runDebuggerQualification(invocation), error => {
      failure = error;
      return error.report?.status === 'failed'
        && error.report.error.includes(measuredKeys[index] + ' bytes differ from caller pin');
    });
    assert.equal(failure.report.exit, null);
    assert.equal(failure.report.invocation, undefined);
    assert.equal(failure.report.commands, undefined);
    assert.deepEqual(Object.keys(failure.report.inputs), measuredKeys.slice(0, index + 1));
    assert.equal(failure.report.exact_streams_truncated, false);
    assert.equal(failure.report.forced_drain_deadline_exceeded, false);
    assert.equal(failure.report.source_authentication, false);
    assert.equal(failure.report.hardware_observed, false);
    assert.equal(failure.report.grants_artifact_or_launch_authority, false);
    assert.equal(failure.report.grants_proof_or_resume_authority, false);
    assert.equal(failure.report.physical_register_values, 'unavailable');
    const artifacts = ['requests.jsonl', 'responses.jsonl', 'stderr.log', 'observation.json'];
    assert.deepEqual((await readdir(outputDirectory)).sort(), [...artifacts].sort());
    const before = new Map();
    for (const name of artifacts) before.set(name, await readFile(join(outputDirectory, name)));
    for (const name of artifacts.slice(0, 3)) {
      assert.equal(before.get(name).length, 0);
      assert.deepEqual(failure.report.artifacts[name], { bytes: 0, sha256: sha(Buffer.alloc(0)) });
    }
    assert.deepEqual(parseExactJson(before.get('observation.json').toString()), failure.report);
    // Refusing output reuse must not truncate or replace the first failure.
    await assert.rejects(runDebuggerQualification(invocation), { code: 'EEXIST' });
    for (const name of artifacts) assert.deepEqual(await readFile(join(outputDirectory, name)), before.get(name));
  }
});

test('invalid option coordinates and pins refuse without creating output artifacts', { timeout: 10000 }, async () => {
  const options = await negativeInputs();
  const changes = [
    value => { value.debuggerPath = 'relative-program'; },
    value => { value.kirFileSha256 = 'not-a-digest'; },
    value => { value.expected.region.blockOrdinal = 1.5; },
    value => { value.expected.rawBlockId = -1; },
    value => { value.expected.lane = 64; },
    value => { value.expected.beforeInputs[0] = 0x1_0000_0000; },
    value => { value.expected.expectedAccessByteOffset = 4097; },
    value => { value.expected.outputMemory.initialized = '0x'; },
  ];
  for (const [index, change] of changes.entries()) {
    const invocation = structuredClone({ ...options, outputDirectory: join(options.root, 'invalid-options-' + index) });
    change(invocation);
    await assert.rejects(runDebuggerQualification(invocation));
    await assert.rejects(stat(invocation.outputDirectory), { code: 'ENOENT' });
  }
});
