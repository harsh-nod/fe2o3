// Synthetic diagnostics and in-memory source mutations, not runtime executions.
const fs = require('fs'), crypto = require('crypto'), assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const helper = root + 'r125-development-mutation-check-v4.js';
const helperHash = '608f5d428766e7984fbb085b9fafa0ba37633eceb4837a2fb605c110b4a06ec4';
assert.strictEqual(crypto.createHash('sha256').update(fs.readFileSync(helper)).digest('hex'), helperHash);
const M = require(helper), {C, p} = M;
C.pin(helper, helperHash); C.bytes(__filename);
const cases = [];
function good(name, run) { run(); cases.push({name, expected: 'accept'}); }
function bad(name, run) {
  assert.throws(run, error => error.code === 'ERR_ASSERTION', name);
  cases.push({name, expected: 'reject'});
}
function replaceOne(text, before, after) {
  if (text.split(before).length !== 2) throw new Error('missing or ambiguous calibration target');
  return text.replace(before, after);
}
function panic(mutation, oracle) {
  return "thread '" + mutation.test + "' (12345) panicked at " + M.location(oracle) + ':\n' + oracle.fragments.join('\n') + '\n';
}
function synthetic(mutation) {
  return [
    '    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.01s',
    '     Running unittests src/lib.rs (target/debug/deps/fe2o3_kfd-0123456789abcdef)',
    '', 'running 1 test', 'test ' + mutation.test + ' ... FAILED', '', 'failures:', '',
    '---- ' + mutation.test + ' stdout ----', '', panic(mutation, mutation.oracle),
    ...(mutation.following_panic ? [panic(mutation, mutation.following_panic)] : []),
    'failures:', '    ' + mutation.test, '',
    'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; ' + (p.test_count - 1) + ' filtered out; finished in 0.01s', '',
  ].join('\n');
}
for (const mutation of p.mutations) {
  const log = synthetic(mutation);
  good(mutation.id + ' synthetic diagnostic shape', () => M.assertNegative(log, mutation));
  bad(mutation.id + ' wrong assertion location', () => M.assertNegative(replaceOne(log,
    M.location(mutation.oracle), M.location(mutation.oracle) + '0'), mutation));
  const original = C.bytes(C.repo + '/' + mutation.patch.path).toString();
  assert.strictEqual(C.hash(original), C.map[mutation.patch.path]);
  const changed = C.E.core.mutatedSource(original, mutation.patch);
  good(mutation.id + ' authenticated forward/reverse derivation', () => M.assertMutationSource(mutation, changed));
  const unrelated = changed + '\n// calibration-only out-of-scope change\n';
  bad(mutation.id + ' out-of-scope source', () => M.assertMutationSource(
    {...mutation, expected_file_sha256: C.hash(unrelated)}, unrelated));
}
const sample = p.mutations[0], fixture = synthetic(sample), row = 'test ' + sample.test + ' ... FAILED';
const progress = 'test ' + sample.test + ' has been running for over 60 seconds';
good('negative selected libtest progress precedes failure', () =>
  M.assertNegative(replaceOne(fixture, row, progress + '\n' + row), sample));
bad('negative wrong test progress', () =>
  M.assertNegative(replaceOne(fixture, row, 'test unknown-test has been running for over 60 seconds\n' + row), sample));
bad('negative duplicate progress', () =>
  M.assertNegative(replaceOne(fixture, row, progress + '\n' + progress + '\n' + row), sample));
bad('negative progress after failure', () =>
  M.assertNegative(replaceOne(fixture, row, row + '\n' + progress), sample));
bad('negative unknown failed test', () => M.assertNegative(replaceOne(fixture, row, 'test unknown-test ... FAILED'), sample));
bad('negative wrong executable', () => M.assertNegative(fixture.replace('deps/fe2o3_kfd-', 'deps/fe2o3_runtime-'), sample));
bad('negative extra executable', () => M.assertNegative(fixture + '     Running unknown-target\n', sample));
bad('negative extra doctest', () => M.assertNegative(fixture + '   Doc-tests fe2o3_kfd\n', sample));
bad('negative wrong filtered count', () => M.assertNegative(replaceOne(fixture, (p.test_count - 1) + ' filtered out', (p.test_count - 2) + ' filtered out'), sample));
bad('negative compilation error', () => M.assertNegative(fixture + 'error[E0308]: synthetic compile failure\n', sample));
bad('negative subprocess abort', () => M.assertNegative(fixture + 'signal: 6 (SIGABRT)\n', sample));
bad('negative wrong diagnostic', () => M.assertNegative(replaceOne(fixture,
  sample.oracle.fragments[0], 'unrelated assertion'), sample));
bad('negative missing panic PID', () => M.assertNegative(replaceOne(fixture, ' (12345) panicked at ', ' panicked at '), sample));
bad('negative wrong panic owner', () => M.assertNegative(replaceOne(fixture,
  "thread '" + sample.test + "'", "thread 'unknown-test'"), sample));
bad('negative extra passing row', () => M.assertNegative(fixture + 'test unknown-test ... ok\n', sample));
bad('negative extra ignored row', () => M.assertNegative(fixture + 'test unknown-test ... ignored\n', sample));
const unrelatedPanic = panic(sample, {...sample.oracle, location: sample.oracle.location + '0'});
bad('negative unrelated final panic', () => M.assertNegative(replaceOne(fixture,
  panic(sample, sample.oracle), panic(sample, sample.oracle) + unrelatedPanic), sample));
const original = C.bytes(C.repo + '/' + sample.patch.path).toString();
const changed = C.E.core.mutatedSource(original, sample.patch);
bad('source wrong declared mutant hash', () => M.assertMutationSource({...sample, expected_file_sha256: '0'.repeat(64)}, changed));
const wrongInverse = {...sample, patch: {...sample.patch, edits: sample.patch.edits.map(([from, to]) =>
  [from + '\n    // calibration-only inverse mismatch', to])}};
bad('source mismatched inverse patch', () => M.assertMutationSource(wrongInverse, changed));
const caught = p.mutations.find(mutation => mutation.final_panic === 'caught_then_unwrap');
const caughtLog = synthetic(caught), primary = panic(caught, caught.oracle), following = panic(caught, caught.following_panic);
bad('caught extra leading panic', () => M.assertNegative(replaceOne(caughtLog, primary,
  panic(caught, {...caught.oracle, location: caught.oracle.location + '0'}) + primary), caught));
bad('caught outer unwrap alone is insufficient', () => M.assertNegative(replaceOne(caughtLog, primary, ''), caught));
bad('caught missing outer unwrap', () => M.assertNegative(replaceOne(caughtLog, following, ''), caught));
bad('caught unrelated intervening panic', () => M.assertNegative(replaceOne(caughtLog, primary,
  primary + panic(caught, {...caught.oracle, location: caught.oracle.location + '0'})), caught));
bad('caught wrong following location', () => M.assertNegative(replaceOne(caughtLog,
  M.location(caught.following_panic), M.location(caught.following_panic) + '0'), caught));
bad('caught wrong following diagnostic', () => M.assertNegative(replaceOne(caughtLog,
  caught.following_panic.fragments[0], 'unrelated outer panic'), caught));
const completeExtra = panic(sample, {...sample.oracle, location: 'calibration-fixture.rs:1:1'});
const malformedExtra = completeExtra.replace(' (12345)', '');
good('negative prior caught panic remains distinguishable', () => M.assertNegative(replaceOne(fixture,
  panic(sample, sample.oracle), completeExtra + panic(sample, sample.oracle)), sample));
bad('negative malformed trailing header cannot hide later failure', () => M.assertNegative(replaceOne(fixture,
  panic(sample, sample.oracle), panic(sample, sample.oracle) + malformedExtra), sample));
bad('negative complete header outside selected body', () => M.assertNegative(completeExtra + fixture, sample));
bad('negative malformed header outside selected body', () => M.assertNegative(fixture + malformedExtra, sample));
assert.strictEqual(cases.length, 101);
assert.strictEqual(cases.filter(test => test.expected === 'accept').length, 38);
assert.strictEqual(cases.filter(test => test.expected === 'reject').length, 63);
assert.deepStrictEqual(C.identities(), C.map);
C.stable();
console.log(JSON.stringify({development_only: true, helper_calibration_only: true, runtime_mutation_count: 0,
  helper_sha256: helperHash, passed: cases.length, cases}));
