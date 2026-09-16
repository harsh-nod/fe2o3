// In-memory record/transcript calibration only, not compiled runtime mutations.
const fs = require('fs'), crypto = require('crypto'), assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const helper = root + 'r125-development-check-v2.js';
const helperHash = '8de74c2bbd0bf464654692614754aea48eeefb29a74314a8223acd30234fb1c4';
assert.strictEqual(crypto.createHash('sha256').update(fs.readFileSync(helper)).digest('hex'), helperHash);
const C = require(helper);
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
const baseline = C.baseline();
const spec = C.p.full_runs.find(spec => spec.kind === 'gnu');
const oldLog = C.accepted(spec);
good('actual GNU exact executable and test roster', () => C.assertFull(baseline.log, oldLog));
for (const [label, test] of [
  ['new KFD test', C.p.new_tests.kfd[0]],
  ['new runtime test', C.p.new_tests.runtime[0]],
  ['preserved KFD test', C.E.executables(oldLog).find(t => t.kind === 'libtest' && t.name.endsWith('/fe2o3_kfd)')).passing[0]],
  ['preserved runtime test', C.E.executables(oldLog).find(t => t.kind === 'libtest' && t.name.endsWith('/fe2o3_runtime)')).passing[0]],
]) {
  const row = 'test ' + test + ' ... ok\n';
  bad('GNU missing ' + label, () => C.assertFull(replaceOne(baseline.log, row, ''), oldLog));
  bad('GNU duplicate ' + label, () => C.assertFull(replaceOne(baseline.log, row, row + row), oldLog));
}
const summary = 'test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;';
for (const field of ['measured', 'filtered']) {
  bad('GNU wrong ' + field, () => C.assertFull(replaceOne(baseline.log, summary,
    summary.replace('0 ' + field, '1 ' + field)), oldLog));
}
const marker = /^     Running .+$/m.exec(baseline.log)[0];
bad('GNU unknown executable', () => C.assertFull(replaceOne(baseline.log, marker, '     Running unknown-target'), oldLog));
bad('GNU unknown passing row', () => C.assertFull(baseline.log + '\ntest unknown-test ... ok\n', oldLog));
bad('GNU contradictory failed row', () => C.assertFull(baseline.log + '\ntest unknown-test ... FAILED\n', oldLog));
const validate = (record, run = spec, runner = run.runner || C.p.runner) => C.E.checkRecord(record, run.name,
  run.command, C.map, run.predecessor, 0, {read: C.json, bytes: C.bytes}, {
    head: C.p.source_parent, cwd: C.repo, contract: 'r125-development-raw-utc-boot-monotonic-v1',
    runner, after: C.map,
  });
good('actual GNU 60-minute record', () => validate(baseline.record));
for (const [name, edit] of [
  ['old full deadline', value => value.deadline_ms = 1800000],
  ['excess full deadline', value => value.deadline_ms = 7200000],
  ['wrong runner version', value => value.runner = {name: 'r125-development-run-v1.js', sha256: C.p.runners['r125-development-run-v1.js']}],
  ['wrong command', value => value.command = [...value.command, '--ignored']],
  ['unclosed child', value => value.child_closed = false],
  ['timed-out child', value => value.timed_out = true],
  ['recorded live group', value => value.process_group_cleanup.close.live_members = [12345]],
  ['changed source endpoint', value => value.source_unchanged = false],
  ['wrong source identity', value => value.source_map_sha256 = '0'.repeat(64)],
  ['wrong predecessor identity', value => value.clock.predecessor = {name: 'unexpected', sha256: '0'.repeat(64)}],
  ['nonmonotonic finish', value => value.clock.finish.monotonic_ns = '0'],
  ['wrong elapsed time', value => value.elapsed_seconds += 1],
]) {
  bad('record rejects ' + name, () => {
    const value = JSON.parse(JSON.stringify(baseline.record)); edit(value); validate(value);
  });
}
const clone = value => JSON.parse(JSON.stringify(value));
function namedFixture(run) {
  const record = clone(baseline.record);
  record.command = run.command;
  for (const [field, suffix] of [['log', '.log'], ['source', '-source.json'], ['source_after', '-source-after.json']])
    record[field] = C.root + '/' + run.name + suffix;
  record.clock.predecessor = null;
  return record;
}
function fixtureValidator(run, record) {
  return C.E.checkRecord(record, run.name, run.command, C.map, run.predecessor, 0, {
    read: name => [run.name + '-source.json', run.name + '-source-after.json'].includes(name) ? C.map : C.json(name),
    bytes: C.bytes,
  }, {head: C.p.source_parent, cwd: C.repo, contract: 'r125-development-raw-utc-boot-monotonic-v1',
    runner: C.p.runner, after: C.map});
}
const ordinarySpec = {name: 'r125-development-parser-ordinary-fixture', command: ['git', 'diff', '--check'],
  predecessor: spec.name + '.json'};
const ordinary = namedFixture(ordinarySpec);
ordinary.deadline_ms = 1800000;
const previous = baseline.record.clock.source_verified;
for (const [index, observation] of [ordinary.clock.start, ordinary.clock.finish,
  ordinary.process_group_cleanup.close.observation, ordinary.clock.source_verified].entries()) {
  observation.monotonic_ns = (BigInt(previous.monotonic_ns) + BigInt(index + 1) * 1000000n).toString();
  observation.utc_ms = previous.utc_ms + index + 1;
}
ordinary.started_at = new Date(ordinary.clock.start.utc_ms).toISOString();
ordinary.finished_at = new Date(ordinary.clock.finish.utc_ms).toISOString();
ordinary.elapsed_seconds = 0.001;
ordinary.verification_elapsed_seconds = 0.002;
ordinary.clock.predecessor = {name: ordinarySpec.predecessor,
  sha256: C.hash(C.bytes(ordinarySpec.predecessor)), observation: previous};
good('synthetic ordinary record with exact actual predecessor', () => fixtureValidator(ordinarySpec, ordinary));
bad('ordinary record cannot borrow full deadline', () =>
  fixtureValidator(ordinarySpec, {...ordinary, deadline_ms: 3600000}));
bad('ordinary substituted predecessor hash', () => {
  const record = clone(ordinary); record.clock.predecessor.sha256 = '0'.repeat(64);
  fixtureValidator(ordinarySpec, record);
});
bad('ordinary predecessor from a different boot', () => {
  const record = clone(ordinary);
  for (const observation of [record.clock.start, record.clock.finish, record.clock.source_verified,
    record.process_group_cleanup.close.observation]) observation.boot_id = '00000000-0000-0000-0000-000000000000';
  fixtureValidator(ordinarySpec, record);
});
const muslSpec = {...C.p.full_runs.find(run => run.kind === 'musl'), predecessor: null};
const musl = namedFixture(muslSpec);
good('synthetic exact musl sixty-minute declaration', () => fixtureValidator(muslSpec, musl));
for (const deadline of [1800000, 7200000])
  bad('musl rejects deadline ' + deadline, () => fixtureValidator(muslSpec, {...musl, deadline_ms: deadline}));
bad('musl rejects wrong runner', () => fixtureValidator(muslSpec,
  {...musl, runner: {...musl.runner, name: 'r125-development-run-v1.js'}}));
bad('musl rejects changed command', () => fixtureValidator(muslSpec,
  {...musl, command: [...musl.command, '--ignored']}));
good('current-boot closure scans exactly its declared group', () => {
  let calls = 0;
  const closure = C.observeClosure(spec.name, baseline.record, pgid => {
    calls++; assert.strictEqual(pgid, baseline.record.process_group_cleanup.close.pgid); return [];
  });
  assert.strictEqual(calls, 1);
  assert.strictEqual(closure.closure, 'current_boot_observed_absent');
});
bad('closure rejects substituted record object', () =>
  C.observeClosure(spec.name, {...baseline.record, returncode: 101}, () => []));
{
  const record = clone(baseline.record);
  const historicalBoot = '00000000-0000-0000-0000-000000000000';
  assert.notStrictEqual(historicalBoot, C.p.admitted_boot);
  for (const observation of [record.clock.start, record.clock.finish, record.clock.source_verified,
    record.process_group_cleanup.close.observation]) observation.boot_id = historicalBoot;
  const reads = [];
  let scans = 0;
  bad('closure rejects a uniformly historical record before scanning', () =>
    C.observeClosure(spec.name, record, () => { scans++; return []; }, name => {
      reads.push(name); return record;
    }));
  assert.deepStrictEqual(reads, [spec.name + '.json']);
  assert.strictEqual(scans, 0);
}
bad('closure rejects a live group', () => C.observeClosure(spec.name, baseline.record, pgid => [pgid]));
for (const [field, value] of [['CARGO_BUILD_JOBS', '8'], ['CARGO_INCREMENTAL', '1'],
  ['RUST_TEST_THREADS', '8'], ['XDG_RUNTIME_DIR', '/tmp/calibration-only']]) {
  bad('record rejects environment ' + field, () => {
    const record = clone(baseline.record); record.environment[field] = value; validate(record);
  });
}
for (const suffix of ['-source.json', '-source-after.json']) {
  bad('record rejects substituted ' + suffix, () => C.E.checkRecord(baseline.record, spec.name,
    spec.command, C.map, spec.predecessor, 0, {
      read: name => name === spec.name + suffix ? {...C.map, [Object.keys(C.map)[0]]: '0'.repeat(64)} : C.json(name),
      bytes: C.bytes,
    }, {head: C.p.source_parent, cwd: C.repo, contract: 'r125-development-raw-utc-boot-monotonic-v1',
      runner: C.p.runner, after: C.map}));
}
for (const [field, value] of [['returncode', 101], ['child_returncode', 101],
  ['spawn_error', 'calibration-only'], ['signal', 'SIGTERM']]) {
  bad('record rejects outcome ' + field, () => validate({...baseline.record, [field]: value}));
}
for (const [name, edit] of [
  ['mixed observation boots', r => r.clock.finish.boot_id = '00000000-0000-0000-0000-000000000000'],
  ['clock error', r => r.clock.error = 'calibration-only'],
  ['clock contract', r => r.clock.contract = 'unexpected'],
  ['wrong log path', r => r.log += '.substituted'],
  ['unproven group closure', r => r.process_group_cleanup.close.status = 'error'],
]) bad(name, () => {const record = clone(baseline.record); edit(record); validate(record);});
assert.strictEqual(cases.length, 55);
assert.deepStrictEqual(C.identities(), C.map);
C.stable();
console.log(JSON.stringify({development_only: true, helper_calibration_only: true, runtime_mutation_count: 0,
  helper_sha256: helperHash, passed: cases.length, cases}));
