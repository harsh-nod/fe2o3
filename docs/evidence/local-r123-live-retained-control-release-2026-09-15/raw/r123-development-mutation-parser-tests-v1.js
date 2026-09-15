// Synthetic diagnostics and in-memory source mutations only, not runtime executions.
const fs = require('fs'), crypto = require('crypto'), assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const helper = root + 'r123-development-mutation-check-v1.js';
const helperHash = 'd2b2839f4e3ad851bb5cb7b25f92ef153bc5673a21fb7a2f7117b67ff5b7c690';
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
    'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1208 filtered out; finished in 0.01s', '',
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
bad('negative unknown failed test', () => M.assertNegative(replaceOne(fixture, row, 'test unknown-test ... FAILED'), sample));
bad('negative wrong executable', () => M.assertNegative(fixture.replace('deps/fe2o3_kfd-', 'deps/fe2o3_runtime-'), sample));
bad('negative extra executable', () => M.assertNegative(fixture + '     Running unknown-target\n', sample));
bad('negative extra doctest', () => M.assertNegative(fixture + '   Doc-tests fe2o3_kfd\n', sample));
bad('negative wrong filtered count', () => M.assertNegative(replaceOne(fixture, '1208 filtered out', '1207 filtered out'), sample));
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
const sticky = p.mutations.find(mutation => mutation.final_panic === 'caught_then_unwrap');
const stickyLog = synthetic(sticky), primary = panic(sticky, sticky.oracle), following = panic(sticky, sticky.following_panic);
bad('sticky outer unwrap alone is insufficient', () => M.assertNegative(replaceOne(stickyLog, primary, ''), sticky));
bad('sticky missing outer unwrap', () => M.assertNegative(replaceOne(stickyLog, following, ''), sticky));
bad('sticky unrelated intervening panic', () => M.assertNegative(replaceOne(stickyLog, primary,
  primary + panic(sticky, {...sticky.oracle, location: sticky.oracle.location + '0'})), sticky));
bad('sticky wrong following location', () => M.assertNegative(replaceOne(stickyLog,
  M.location(sticky.following_panic), M.location(sticky.following_panic) + '0'), sticky));
bad('sticky wrong following diagnostic', () => M.assertNegative(replaceOne(stickyLog,
  sticky.following_panic.fragments[0], 'unrelated outer panic'), sticky));
assert.strictEqual(cases.length, 60);
assert.deepStrictEqual(C.identities(), C.map);
C.stable();
console.log(JSON.stringify({development_only: true, helper_calibration_only: true, runtime_mutation_count: 0,
  helper_sha256: helperHash, passed: cases.length, cases}));
