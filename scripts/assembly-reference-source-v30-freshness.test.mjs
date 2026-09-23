// Synthetic framing tests only. Zero signature bytes are not protected proof evidence.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { test } from 'node:test';
import {
  FEATURES, PREFIX, expectedStage, parseObservation, validateOriginal,
} from './assembly-reference-source-v30-freshness.mjs';

const hash = 'a'.repeat(64);
const digest = byte => Array(32).fill(byte);
const binding = offset => ({
  reference_identity: digest(1 + offset), reference_mir: digest(2 + offset),
  kernel_identity: digest(3 + offset), kernel_mir: digest(4 + offset),
  normalized_obligation: digest(5 + offset),
});
const positive = 'source-to-proof V2 ranked admission failed: error[FE2O3-OWN-002]: launch dimension 0 is dynamic';
const negative = 'functional-refinement proof execution failed: UnexpectedProofResult exit=Some(1), signal=None stdout="verification results:: 0 verified, 1 errors\\n" stderr="assertion failed"';
function record(index) {
  return { schema: 'fe2o3-test-source-isa-reference-invocation-v30',
    feature: FEATURES[index], args: ['test-only synthetic argv'],
    crate_binding: String(index + 1).repeat(64), cargo_observation: String(index + 2).repeat(64),
    source_sha256: hash, root_source_sha256: hash, manifest_sha256: hash };
}
const original = {
  schema: 'fe2o3-test-source-proof-original-v1', invocation: record(0),
  receipt: { schema: 'fe2o3-test-source-proof-inert-receipt-v1',
    binding: binding(0), wire: Array(524).fill(0), verifying_key: digest(9) },
};
const originalHash = value => createHash('sha256')
  .update(JSON.stringify(value) + '\n').digest('hex');
function observation(index) {
  const input = record(index);
  const diagnostic = index === 0 ? positive : negative;
  return { schema: 'fe2o3-test-source-proof-freshness-observation-v1',
    feature: input.feature, stage: expectedStage(input.feature, diagnostic), invocation: input,
    diagnostic, observed: {
      request_count: 1, normal_import_count: index === 0 ? 1 : 0, binding: binding(index * 10),
      original_receipt: null, stale_error: index === 0 ? null : 'StaleSafeReferenceIdentity',
      rejected_import_count: 0, original_reimport_count: index === 0 ? 0 : 1,
      normalized_obligation_changed: index !== 0, kernel_mir_changed: index !== 0,
    },
    original: index === 0 ? structuredClone(original) : null,
    original_transport_sha256: index === 0 ? null : originalHash(original),
    actual_rustc_callback: true, source_file_bytes_unchanged: true,
    semantic_change_is_feature_selected: true, proof_subject_kind: 'mir',
    source_hash_is_proof_subject: false, source_admission_complete: false,
    v17_proof_support: false, grants_artifact_or_launch_authority: false, hardware_observed: false,
  };
}
const transcript = value => PREFIX + JSON.stringify(value)
  + '\ntest result: ok. 1 passed; 0 failed; 0 ignored; 999 filtered out;\n';
const parse = (value, index) => parseObservation(transcript(value), record(index),
  index === 0 ? null : original, index === 0 ? null : originalHash(original));

test('closed positive and two negative records remain distinct with exact counts', () => {
  for (let index = 0; index < 3; index++) assert.deepEqual(parse(observation(index), index), observation(index));
});

test('inert framing refuses unknown fields, wrong dimensions and zero subjects', () => {
  validateOriginal(original, record(0));
  for (const change of [
    value => { value.extra = true; },
    value => { value.receipt.extra = true; },
    value => { value.receipt.wire.pop(); },
    value => { value.receipt.wire.push(0); },
    value => { value.receipt.verifying_key[0] = 256; },
    value => { value.receipt.binding.kernel_mir.fill(0); },
    value => { value.invocation.feature = FEATURES[1]; },
  ]) {
    const value = structuredClone(original); change(value);
    assert.throws(() => validateOriginal(value, record(0)));
  }
});

test('positive requires real normal-import count and denies every later authority', () => {
  for (const change of [
    value => { value.observed.normal_import_count = 0; },
    value => { value.observed.original_reimport_count = 1; },
    value => { value.observed.binding = binding(1); },
    value => { value.original_transport_sha256 = hash; },
    value => { value.diagnostic = negative; },
    ...['source_hash_is_proof_subject', 'source_admission_complete', 'v17_proof_support',
      'grants_artifact_or_launch_authority', 'hardware_observed'].map(key => value => { value[key] = true; }),
  ]) {
    const value = observation(0); change(value); assert.throws(() => parse(value, 0));
  }
});

test('negative requires current identity, changed obligation, exact refusal and original recovery', () => {
  for (const index of [1, 2]) for (const change of [
    value => { value.observed.stale_error = 'SignatureRejected'; },
    value => { value.observed.rejected_import_count = 1; },
    value => { value.observed.original_reimport_count = 0; },
    value => { value.observed.normal_import_count = 1; },
    value => { value.observed.normalized_obligation_changed = false; },
    value => { value.observed.binding.normalized_obligation = binding(0).normalized_obligation; },
    value => { value.observed.binding.reference_identity = binding(0).reference_identity; },
    value => { value.original_transport_sha256 = hash; },
    value => { value.original = original; },
    value => { value.diagnostic = positive; },
    value => { value.invocation = record(0); },
  ]) {
    const value = observation(index); change(value); assert.throws(() => parse(value, index));
  }
});

test('transport pin cannot be replaced by another advertised pin or signed-byte tuple', () => {
  const changed = structuredClone(original);
  changed.receipt.wire[0] = 1;
  assert.throws(() => parseObservation(transcript(observation(1)), record(1), changed, originalHash(original)));
  assert.throws(() => parseObservation(transcript(observation(1)), record(1), original, hash));
});

test('extra observations, failed test summaries and oversize bodies refuse', () => {
  const value = observation(0);
  assert.throws(() => parseObservation(transcript(value) + PREFIX + JSON.stringify(value), record(0)));
  assert.throws(() => parseObservation(transcript(value).replace('1 passed', '0 passed'), record(0)));
  value.diagnostic = 'x'.repeat(64 * 1024 + 1);
  assert.throws(() => parse(value, 0));
});
