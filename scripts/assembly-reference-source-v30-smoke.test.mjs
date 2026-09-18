import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { test } from 'node:test';
import {
  FEATURES, PREFIX, expectedStage, parseObservation, parseOptions, readBounded,
  validateInvocation, validatePreparation,
} from './assembly-reference-source-v30-smoke.mjs';

const h = 'a'.repeat(64);
const hashes = Object.fromEntries(['metadata_sha256', 'artifacts_sha256', 'source_sha256',
  'root_source_sha256', 'manifest_sha256'].map(key => [key, h]));
const preparation = {
  schema: 'fe2o3-test-source-isa-reference-preparation-v30', features: FEATURES,
  ...hashes, rustc_callback_executed: false, proof_executed: false,
  grants_artifact_or_launch_authority: false,
};
const positive = 'source-to-proof V2 ranked admission failed: error[FE2O3-OWN-002]: GPU hierarchy ownership is incomplete because guarded invocation tracing failed: launch dimension 0 is dynamic';
const negative = 'functional-refinement proof execution failed: UnexpectedProofResult exit=Some(1), signal=None stdout="verification results:: 0 verified, 1 errors\\n" stderr="assertion failed"';
function record(feature = FEATURES[0]) {
  return { schema: 'fe2o3-test-source-isa-reference-invocation-v30', feature,
    args: ['rustc', 'source.rs', '--edition=2024', '--crate-type=lib', '--emit=metadata',
      '-Cpanic=abort', '--target=amdgcn-amd-amdhsa', '-Zalways-encode-mir', '-Copt-level=3',
      '-Coverflow-checks=on', '--cfg', 'feature="' + feature + '"'],
    crate_binding: h, cargo_observation: h,
    source_sha256: h, root_source_sha256: h, manifest_sha256: h };
}
function observation(input = record()) {
  const diagnostic = input.feature === FEATURES[0] ? positive : negative;
  return { schema: 'fe2o3-test-source-isa-reference-observation-v30', feature: input.feature,
    stage: expectedStage(input.feature, diagnostic), actual_rustc_callback: true, source_unchanged: true,
    source_sha256: input.source_sha256, root_source_sha256: input.root_source_sha256,
    manifest_sha256: input.manifest_sha256, crate_binding: input.crate_binding,
    cargo_observation: input.cargo_observation, diagnostic,
    proof_stage_evidence: input.feature === FEATURES[0]
      ? 'normal protected receipt import precedes the ranked-admission diagnostic; no separate receipt exposed by this API'
      : 'actual Verus assertion failure in the normal protected proof path, before ranked ownership admission',
    source_admission_complete: false, grants_artifact_or_launch_authority: false, hardware_observed: false };
}
const receiptLine = value => PREFIX + JSON.stringify(value) + '\n';
const transcript = value => receiptLine(value) + 'test result: ok. 1 passed; 0 failed; 0 ignored; 746 filtered out;\n';

test('CLI requires closed keys, immutable image, task-owned volume and new-path shape', () => {
  const args = ['--prepared', '/task/inputs', '--harness', '/task/test', '--image', 'sha256:' + h,
    '--native-volume', 'fe2o3-authoring-proof-native-20260917-r1', '--output', '/task/source-proof-r1'];
  assert.equal(parseOptions(args).prepared, '/task/inputs');
  for (const mutated of [
    args.slice(0, -2), [...args, '--other', 'x'], [...args, '--image', 'sha256:' + h],
    args.map(value => value === 'sha256:' + h ? 'latest' : value),
    args.map(value => value === '/task/test' ? '/task/../test' : value),
    args.map(value => value === '/task/test' ? '/task/test,readonly' : value),
    args.map(value => value === 'fe2o3-authoring-proof-native-20260917-r1' ? 'unrelated-volume' : value),
  ]) assert.throws(() => parseOptions(mutated));
});

test('preparation is exact, unexecuted and bound to freshly read bytes', () => {
  validatePreparation(preparation, hashes);
  for (const key of Object.keys(preparation)) {
    const missing = { ...preparation };
    delete missing[key];
    assert.throws(() => validatePreparation(missing, hashes));
  }
  assert.throws(() => validatePreparation({ ...preparation, extra: true }, hashes));
  assert.throws(() => validatePreparation({ ...preparation, features: FEATURES.slice(1) }, hashes));
  assert.throws(() => validatePreparation(preparation, { ...hashes, artifacts_sha256: 'b'.repeat(64) }));
  for (const key of ['rustc_callback_executed', 'proof_executed', 'grants_artifact_or_launch_authority']) {
    assert.throws(() => validatePreparation({ ...preparation, [key]: true }, hashes));
  }
});

test('invocation transport rejects missing or stale identities and unchecked overflow', () => {
  for (const feature of FEATURES) validateInvocation(record(feature), feature, preparation);
  assert.throws(() => validateInvocation({}, FEATURES[0], preparation));
  for (const change of [
    { extra: true }, { feature: FEATURES[1] }, { crate_binding: '' },
    { cargo_observation: 'synthetic' }, { source_sha256: 'b'.repeat(64) },
    { args: record().args.filter(arg => arg !== '-Coverflow-checks=on') },
    { args: record().args.map(arg => arg === 'feature="' + FEATURES[0] + '"' ? 'feature="other"' : arg) },
  ]) assert.throws(() => validateInvocation({ ...record(), ...change }, FEATURES[0], preparation));
});

test('positive requires exact later ownership failure; negatives require actual assertion failure', () => {
  assert.equal(expectedStage(FEATURES[0], positive), 'protected_proof_completed_then_dynamic_total_view_refused');
  for (const feature of FEATURES.slice(1)) {
    assert.equal(expectedStage(feature, negative), 'protected_verus_assertion_rejected_before_ownership');
    assert.throws(() => expectedStage(feature, positive));
  }
  assert.throws(() => expectedStage(FEATURES[0], negative));
  for (const feature of FEATURES) for (const diagnostic of [
    '', 'ok', 'functional-refinement proof runtime unavailable',
    'functional-refinement proof execution failed: TimedOut',
    'functional-refinement proof execution failed: UnexpectedProofResult exit=Some(1), signal=None syntax error',
    'source-to-proof V2 effect mismatch', 'source-to-proof V2 cannot normalize the GPU store value',
    negative.replace('1 errors', '2 errors'), negative.replace('signal=None', 'signal=Some(9)'),
  ]) assert.throws(() => expectedStage(feature, diagnostic));
});

test('all three actual observations retain identities and explicitly deny final authority', () => {
  for (const feature of FEATURES) {
    const input = record(feature);
    assert.deepEqual(parseObservation(transcript(observation(input)), input), observation(input));
  }
  for (const key of ['feature', 'source_sha256', 'root_source_sha256', 'manifest_sha256',
    'crate_binding', 'cargo_observation']) {
    assert.throws(() => parseObservation(transcript({ ...observation(), [key]: 'wrong' }), record()));
  }
  for (const key of ['source_admission_complete', 'grants_artifact_or_launch_authority', 'hardware_observed']) {
    assert.throws(() => parseObservation(transcript({ ...observation(), [key]: true }), record()));
  }
  for (const key of ['actual_rustc_callback', 'source_unchanged']) {
    assert.throws(() => parseObservation(transcript({ ...observation(), [key]: false }), record()));
  }
  assert.throws(() => parseObservation(transcript({ ...observation(), extra: true }), record()));
});

test('zero-test success, ignored callbacks, duplicate and absent receipts fail', () => {
  const value = observation();
  for (const text of [
    '', 'test result: ok. 0 passed; 0 failed; 0 ignored;\n',
    receiptLine(value) + 'test result: ok. 0 passed; 0 failed; 1 ignored;\n',
    receiptLine(value) + transcript(value), receiptLine(value),
    transcript(value) + 'test result: ok. 1 passed; 0 failed; 0 ignored;\n',
  ]) assert.throws(() => parseObservation(text, record()));
});

test('bounded input rejects symlinks, FIFOs and oversized files without blocking', () => {
  const directory = mkdtempSync(join(tmpdir(), 'fe2o3-source-proof-controls-'));
  try {
    const regular = join(directory, 'regular');
    writeFileSync(regular, 'abc');
    assert.equal(readBounded(regular, 3).toString(), 'abc');
    assert.throws(() => readBounded(regular, 2));
    const link = join(directory, 'link');
    symlinkSync(regular, link);
    assert.throws(() => readBounded(link));
    assert.throws(() => readBounded(directory));
    assert.throws(() => readBounded(join(directory, 'absent')));
    const fifo = join(directory, 'fifo');
    const made = spawnSync('/usr/bin/mkfifo', [fifo], { timeout: 1000 });
    assert.equal(made.status, 0);
    assert.throws(() => readBounded(fifo));
  } finally { rmSync(directory, { recursive: true }); }
});
