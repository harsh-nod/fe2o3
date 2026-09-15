// Parser calibration only: in-memory transcript/source mutations are not runtime negatives.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/', helper = root + 'r122-development-check-v2.js';
const helperHash = '3a9c56f38d9ffcb9df09cd4980cab107a4fd536bc24d858dfa520838c519b81e';
assert.strictEqual(crypto.createHash('sha256').update(fs.readFileSync(helper)).digest('hex'), helperHash);
const C = require(helper), core = require(root + 'r119-integrated-mutation-core-v1.js');
C.pin(helper, helperHash); C.bytes(__filename);
const cases = [];
function good(name, run) { run(); cases.push({name, expected: 'accept'}); }
function bad(name, run) {
  assert.throws(run, error => error.code === 'ERR_ASSERTION', name);
  cases.push({name, expected: 'reject'});
}
function replaceOne(text, before, after) {
  assert.strictEqual(text.split(before).length, 2, 'unique calibration target');
  return text.replace(before, after);
}
const baseline = C.baseline(), old = C.p.accepted_baseline;
const oldLog = C.git(['show', old.commit + ':' + old.path]);
assert.strictEqual(C.hash(oldLog), old.sha256);
good('actual GNU exact target and test roster', () => C.assertFull(baseline.log, oldLog));
for (const kind of ['kfd', 'runtime']) {
  const row = 'test ' + C.p.new_tests[kind][0] + ' ... ok\n';
  bad('GNU missing ' + kind + ' addition', () => C.assertFull(replaceOne(baseline.log, row, ''), oldLog));
}
const summary = 'test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;';
for (const field of ['measured', 'filtered']) {
  bad('GNU wrong ' + field + ' count', () => C.assertFull(replaceOne(baseline.log, summary,
    summary.replace('0 ' + field, '1 ' + field)), oldLog));
}
const fixture = C.pin('r122-development-negative-direct-transport-v1.log',
  '45d664f7ad8face0a30aba87bd5573edebc9377040ca9097747ace427d03244f').toString();
const m = C.p.mutations.find(item => item.id === '07-direct-transport');
good('historical direct log parser only', () => C.assertNegative(fixture, m));
const sticky = C.pin('r122-development-negative-sticky-transport-v1.log',
  '4ce8e7597338619a6b056f502cf6120b6a39f8704cb5aaab0b84cdceaf24de95').toString();
good('historical sticky log parser only', () => C.assertNegative(sticky,
  C.p.mutations.find(item => item.id === '08-sticky-transport')));
const marker = /^     Running .+$/m.exec(fixture)[0];
bad('negative wrong package executable', () => C.assertNegative(replaceOne(fixture, marker,
  marker.replace('fe2o3_kfd-', 'fe2o3_runtime-')), m));
bad('negative additional executable', () => C.assertNegative(fixture +
  '     Running examples/calibration.rs (target/debug/examples/calibration-0123456789abcdef)\n', m));
bad('negative extra doctest executable', () => C.assertNegative(fixture + '   Doc-tests fe2o3_kfd\n', m));
bad('negative wrong assertion owner', () => C.assertNegative(replaceOne(fixture,
  "thread '" + m.test + "'", "thread 'calibration-other-test'"), m));
bad('negative wrong assertion location', () => C.assertNegative(replaceOne(fixture,
  m.oracle_location, m.oracle_location.replace(':184:5', ':185:5')), m));
bad('negative wrong diagnostic', () => C.assertNegative(replaceOne(fixture,
  m.oracle[0], 'assertion failed: calibration_unrelated_failure'), m));
bad('negative compilation error', () => C.assertNegative(fixture + 'error[E0308]: calibration compile failure\n', m));
bad('negative subprocess abort', () => C.assertNegative(fixture + 'signal: 6 (SIGABRT)\n', m));
bad('negative wrong filtered count', () => C.assertNegative(replaceOne(fixture,
  '1189 filtered out', '1188 filtered out'), m));
const runtime = C.p.mutations.find(item => item.id === '09-error-suffix');
const synthetic = [
  '    Finished ' + String.fromCharCode(96) + 'test' + String.fromCharCode(96) + ' profile [unoptimized + debuginfo] target(s) in 0.01s',
  '     Running unittests src/lib.rs (target/debug/deps/fe2o3_runtime-0123456789abcdef)',
  '', 'running 1 test', 'test ' + runtime.test + ' ... FAILED', '', 'failures:', '',
  '---- ' + runtime.test + ' stdout ----', '',
  "thread '" + runtime.test + "' (12345) panicked at " + runtime.oracle_location + ':',
  ...runtime.oracle, '', 'failures:', '    ' + runtime.test, '',
  'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 742 filtered out; finished in 0.00s', '',
].join('\n');
good('synthetic runtime parser shape only', () => C.assertNegative(synthetic, runtime));
bad('synthetic runtime wrong executable', () => C.assertNegative(synthetic.replace(
  'deps/fe2o3_runtime-', 'deps/fe2o3_kfd-'), runtime));
for (const item of C.p.mutations) {
  const original = C.bytes(C.repo + '/' + item.patch.path).toString();
  assert.strictEqual(C.hash(original), C.map[item.patch.path]);
  const changed = core.mutatedSource(original, item.patch);
  good(item.id + ' authenticated forward and reverse source derivation', () => C.assertMutationSource(item, changed));
  const unrelated = changed + '\n// calibration-only out-of-scope change\n';
  bad(item.id + ' rejects out-of-scope source', () => C.assertMutationSource(
    {...item, expected_file_sha256: C.hash(unrelated)}, unrelated));
}
const sample = C.p.mutations[0], sampleOriginal = C.bytes(C.repo + '/' + sample.patch.path).toString();
const sampleChanged = core.mutatedSource(sampleOriginal, sample.patch);
bad('source rejects wrong declared mutant hash', () => C.assertMutationSource(
  {...sample, expected_file_sha256: '0'.repeat(64)}, sampleChanged));
const wrongInverse = {...sample, patch: {...sample.patch, edits: sample.patch.edits.map(([from, to]) =>
  [from + '\n    // calibration-only inverse mismatch', to])}};
bad('source rejects mismatched inverse patch', () => C.assertMutationSource(wrongInverse, sampleChanged));
const originalRecord = baseline.record;
const validate = value => C.E.checkRecord(value, C.p.baseline.name, C.p.baseline.command, C.map,
  C.p.baseline.predecessor, 0, {read: C.json, bytes: C.bytes}, {
    head: C.p.source_parent, cwd: C.repo, contract: 'r122-development-raw-utc-boot-monotonic-v1',
    runner: C.p.runner, after: C.map,
  });
good('actual GNU record contract', () => validate(originalRecord));
for (const [name, edit] of [
  ['unclosed child', value => value.child_closed = false],
  ['recorded live group', value => value.process_group_cleanup.close.live_members = [12345]],
  ['changed source endpoint', value => value.source_unchanged = false],
  ['wrong source identity', value => value.source_map_sha256 = '0'.repeat(64)],
  ['wrong predecessor identity', value => value.clock.predecessor.sha256 = '0'.repeat(64)],
  ['nonmonotonic finish', value => value.clock.finish.monotonic_ns = '0'],
]) {
  bad('record rejects ' + name, () => {
    const value = JSON.parse(JSON.stringify(originalRecord)); edit(value); validate(value);
  });
}
assert.strictEqual(cases.length, 51);
assert.deepStrictEqual(C.identities(), C.map);
C.stable();
console.log(JSON.stringify({development_only: true, helper_calibration_only: true,
  runtime_mutation_count: 0, helper_sha256: helperHash, passed: cases.length, cases}));
