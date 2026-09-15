// In-memory record/transcript calibration only, not compiled runtime mutations.
const fs = require('fs'), crypto = require('crypto'), assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const helper = root + 'r124-development-check-v1.js';
const helperHash = '1caef52ee3c10a2d8efd49d82debadfbe227822fb575fc88ea32f99cd0bb82ad';
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
const validate = (record, run = spec, runner = C.p.runner) => C.E.checkRecord(record, run.name,
  run.command, C.map, run.predecessor, 0, {read: C.json, bytes: C.bytes}, {
    head: C.p.source_parent, cwd: C.repo, contract: 'r124-development-raw-utc-boot-monotonic-v1',
    runner, after: C.map,
  });
good('actual GNU 60-minute record', () => validate(baseline.record));
for (const [name, edit] of [
  ['old full deadline', value => value.deadline_ms = 1800000],
  ['excess full deadline', value => value.deadline_ms = 7200000],
  ['wrong runner version', value => value.runner = {name: 'r124-development-run-v1.js', sha256: C.p.runners['r124-development-run-v1.js']}],
  ['wrong command', value => value.command = [...value.command, '--ignored']],
  ['unclosed child', value => value.child_closed = false],
  ['timed-out child', value => value.timed_out = true],
  ['recorded live group', value => value.process_group_cleanup.close.live_members = [12345]],
  ['changed source endpoint', value => value.source_unchanged = false],
  ['wrong source identity', value => value.source_map_sha256 = '0'.repeat(64)],
  ['wrong predecessor identity', value => value.clock.predecessor.sha256 = '0'.repeat(64)],
  ['nonmonotonic finish', value => value.clock.finish.monotonic_ns = '0'],
  ['wrong elapsed time', value => value.elapsed_seconds += 1],
]) {
  bad('record rejects ' + name, () => {
    const value = JSON.parse(JSON.stringify(baseline.record)); edit(value); validate(value);
  });
}
const anchor = C.p.anchors.at(-1), ordinary = C.json(anchor.name + '.json');
good('actual ordinary runner-v1 record', () => validate(ordinary, anchor, anchor.runner));
bad('ordinary record cannot borrow full-run deadline', () => {
  const value = JSON.parse(JSON.stringify(ordinary)); value.deadline_ms = 3600000;
  validate(value, anchor, anchor.runner);
});
const focused = C.p.anchors.find(anchor => anchor.passed === 158);
const focusedLog = C.bytes(focused.name + '.log').toString();
good('actual focused executable and exact filtered roster', () => C.anchorRoster(focused, focusedLog));
const focusedRow = /^test .+ \.\.\. ok$/m.exec(focusedLog)[0];
bad('focused replacement preserves count but changes identity', () => C.anchorRoster(focused,
  replaceOne(focusedLog, focusedRow, 'test calibration-unknown-test ... ok')));
bad('focused unexpected ignored row', () => C.anchorRoster(focused, focusedLog + '\ntest calibration-ignored ... ignored\n'));
const focusedMarker = /^     Running .+$/m.exec(focusedLog)[0];
bad('focused wrong executable', () => C.anchorRoster(focused,
  replaceOne(focusedLog, focusedMarker, focusedMarker.replace('fe2o3_kfd-', 'fe2o3_runtime-'))));
bad('focused duplicate passing row', () => C.anchorRoster(focused,
  replaceOne(focusedLog, focusedRow, focusedRow + '\n' + focusedRow)));
assert.strictEqual(cases.length, 32);
assert.deepStrictEqual(C.identities(), C.map);
C.stable();
console.log(JSON.stringify({development_only: true, helper_calibration_only: true, runtime_mutation_count: 0,
  helper_sha256: helperHash, passed: cases.length, cases}));
