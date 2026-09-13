const assert = require('assert');
const e = require('./r113-qualification-evidence.js');
const p = require('./r113-qualification-plan.js');
const name = 'r113-final-whitespace-check';
const previous = 'r113-final-format-check.json';
const original = e.read(name + '.json');
const source = e.read('r113-frozen-source.json');
let count = 0;
function check(label, alter, rejects = true) {
  const record = structuredClone(original);
  alter(record);
  const verify = () => e.checkRecord(record, name, ['git', 'diff', '--check'], source, previous, 0);
  if (rejects) assert.throws(verify, assert.AssertionError, label);
  else verify();
  count++; console.log('PASS ' + label);
}
check('unchanged-record', () => {}, false);
check('unclosed-child', r => {r.child_closed = false;});
check('raw-child-mismatch', r => {r.child_returncode = 101;});
check('runner-substitution', r => {r.runner.sha256 = '0'.repeat(64);});
check('changed-source', r => {r.source_unchanged = false;});
check('wrong-source-hash', r => {r.source_map_sha256 = '0'.repeat(64);});
check('timeout', r => {r.timed_out = true;});
check('signal', r => {r.signal = 'SIGKILL';});
check('spawn-error', r => {r.spawn_error = 'ENOENT';});
check('changed-command', r => {r.command.push('--quiet');});
check('changed-deadline', r => {r.deadline_ms++;});
check('changed-environment', r => {r.environment.RUST_TEST_THREADS = '1';});
check('different-boot', r => {r.clock.finish.boot_id = '00000000-0000-0000-0000-000000000001';});
check('different-clock-source', r => {r.clock.finish.clock_source = 'other';});
check('monotonic-regression', r => {r.clock.finish.monotonic_ns = '0';});
check('invalid-raw-utc', r => {r.clock.finish.utc_ms = -1;});
check('timestamp-rewrite', r => {r.finished_at = r.started_at;});
check('predecessor-substitution', r => {r.clock.predecessor.sha256 = '0'.repeat(64);});
check('cleanup-error', r => {r.process_group_cleanup.close.status = 'error';});
check('live-descendant', r => {r.process_group_cleanup.close.live_members = [42];});
check('cleanup-after-verification', r => {r.process_group_cleanup.close.observation.monotonic_ns = String(BigInt(r.clock.source_verified.monotonic_ns) + 1n);});
check('elapsed-rewrite', r => {r.elapsed_seconds++;});
check('raw-utc-regression-is-preserved', r => {
  r.clock.finish.utc_ms = r.clock.start.utc_ms - 10;
  r.finished_at = new Date(r.clock.finish.utc_ms).toISOString();
}, false);
for (const suffix of ['-source.json', '-source-after.json']) {
  const io = {bytes: e.bytes, read: artifact => artifact === name + suffix ? {...source, 'Cargo.lock': '0'.repeat(64)} : e.read(artifact)};
  assert.throws(() => e.checkRecord(original, name, ['git', 'diff', '--check'], source, previous, 0, io), assert.AssertionError);
  count++; console.log('PASS substituted' + suffix);
}
const mutation = p.mutations[0];
const diagnostic = ['Finished `test` profile', 'Running unittests', 'running 1 test',
  'test ' + mutation.test + ' ... FAILED', "thread '" + mutation.test + "' (123) panicked at " + mutation.oracle_path + ':' + mutation.oracle_line + ':1:',
  ...mutation.expected, 'test result: FAILED. 0 passed; 1 failed; 0 ignored;'].join('\n');
e.checkMutation(diagnostic, mutation); count++; console.log('PASS valid-synthetic-diagnostic');
for (const [label, text] of [
  ['wrong-oracle', diagnostic.replace(mutation.oracle_path + ':' + mutation.oracle_line, 'other.rs:1')],
  ['wrong-test', diagnostic.replace('test ' + mutation.test + ' ... FAILED', 'test other ... FAILED')],
  ['compile-error', diagnostic + '\nerror[E0308]: wrong type'],
  ['missing-diagnostic', diagnostic.replace(mutation.expected.at(-1), '')],
  ['unrelated-location-is-not-oracle', diagnostic.replace(mutation.oracle_path + ':' + mutation.oracle_line, 'other.rs:1') + '\n' + mutation.oracle_path + ':' + mutation.oracle_line + ':1:'],
]) {
  assert.throws(() => e.checkMutation(text, mutation), assert.AssertionError, label);
  count++; console.log('PASS ' + label);
}
assert(e.canRestore(original)); count++; console.log('PASS closed-owner-can-restore');
for (const [label, modify] of [
  ['unclosed-owner-cannot-restore', r => {r.child_closed = false;}],
  ['cleanup-error-cannot-restore', r => {r.process_group_cleanup.close.status = 'error';}],
  ['live-owner-cannot-restore', r => {r.process_group_cleanup.close.live_members = [42];}],
]) {
  const record = structuredClone(original); modify(record); assert.strictEqual(e.canRestore(record), false);
  count++; console.log('PASS ' + label);
}
const originalSource = require('fs').readFileSync(require('path').join(e.repo, p.production), 'utf8');
const entries = e.expectedEntries(source, originalSource);
assert.strictEqual(Object.keys(entries).length, 37);
assert.deepStrictEqual(entries['r113-qualified-mut-partial-preflight-epoch.json'].command,
  ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-runtime-model', '--all-features', '--lib', mutation.test, '--', '--exact']);
count++; console.log('PASS exact-planned-command');
for (const [label, text] of [
  ['r113-gnu-target-roster', e.bytes('r113-final-gnu-tests.log').toString()],
  ['r113-musl-target-roster', e.bytes('r113-final-musl-tests.log').toString()],
  ['r112-gnu-target-roster', e.committed('raw/r112-final-gnu-tests.log')],
  ['r112-musl-target-roster', e.committed('raw/r112-final-musl-tests.log')],
]) {
  const targets = e.executables(text);
  assert.strictEqual(targets.length, 49);
  assert.strictEqual(targets.filter(t => t.kind === 'libtest').length, 48);
  assert.strictEqual(targets.filter(t => t.kind === 'harnessless-benchmark').length, 1);
  count++; console.log('PASS ' + label);
}
const summary = 'test result: ok. 1 passed; 0 failed; 0 ignored;';
const ordinary = '     Running unittests src/lib.rs (target/debug/deps/example-0000000000000000)\n' +
  'running 1 test\ntest example ... ok\n' + summary + '\n';
const benchmark = '     Running benches/completion_scaling.rs (target/debug/deps/completion_scaling-0000000000000000)\n' +
  'nodes,construction_ns,transitions_ns,transition_ns_per_node\n' +
  '1024,1,2,0.01\n4096,3,4,0.02\n16384,5,6,0.03\n65536,7,8,0.04\n';
const mixed = ordinary + benchmark;
assert.deepStrictEqual(e.executables(mixed.replace('1024,1,2,0.01', '1024,9,8,1.23')), e.executables(mixed));
count++; console.log('PASS benchmark-timings-are-not-compared');
for (const [label, text] of [
  ['unknown-summary-free-target', mixed.replace('benches/completion_scaling.rs', 'benches/unknown.rs')],
  ['duplicate-benchmark-target', mixed + benchmark],
  ['missing-libtest-summary', mixed.replace(summary, '')],
  ['duplicate-libtest-summary', mixed.replace(summary, summary + '\n' + summary)],
  ['unaccounted-summary', summary + '\n' + mixed],
  ['missing-csv-row', mixed.replace('65536,7,8,0.04\n', '')],
  ['duplicate-csv-row', mixed + '65536,7,8,0.04\n'],
  ['reordered-csv-rows', mixed.replace('1024,1,2,0.01\n4096,3,4,0.02', '4096,3,4,0.02\n1024,1,2,0.01')],
  ['wrong-csv-header', mixed.replace('nodes,construction_ns', 'items,construction_ns')],
  ['malformed-csv-number', mixed.replace('1024,1,2,0.01', '1024,1,NaN,0.01')],
  ['unexpected-benchmark-output', mixed + 'unexpected output\n'],
]) {
  assert.throws(() => e.executables(text), assert.AssertionError, label);
  count++; console.log('PASS ' + label);
}
assert.strictEqual(count, 52);
console.log('HELPER_PINS: ' + JSON.stringify(p.helpers.map(name => ({name, sha256: e.hash(e.bytes(name))}))));
console.log('PASS: ' + count + ' qualification contract tests');
