// Exact R125 CPU/source evidence. Hardware and formal correspondence remain separate.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp', repo = path.join(root, 'fe2o3-r61-execution');
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const captured = new Map();
function bytes(name) {
  const file = path.isAbsolute(name) ? name : path.join(root, name);
  assert(fs.lstatSync(file).isFile(), 'regular input: ' + file);
  if (!captured.has(file)) captured.set(file, fs.readFileSync(file));
  return captured.get(file);
}
const json = name => JSON.parse(bytes(name));
function pin(name, digest) {
  const value = bytes(name);
  assert.strictEqual(hash(value), digest, name);
  return value;
}
const p = JSON.parse(pin('r125-development-full-plan-v3.json', 'a5df79bc041fa305a23b640aaa962d7a0d1804a32ada2b81a2814fbbd5063b2c'));
assert.strictEqual(p.accepted, false);
for (const [name, digest] of Object.entries(p.parser_inputs)) pin(name, digest);
for (const [name, digest] of Object.entries(p.runners)) pin(name, digest);
pin('r125-development-evidence-v2.js', 'f13ef55ef7b1ab1cfd93bcbc095f6513345c647f704fc98e4c9c94c40d20628c');
const E = require('./r125-development-evidence-v2.js');
const boot = () => fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim();
assert.strictEqual(boot(), p.admitted_boot, 'admitted execution boot');
for (const [name, digest] of Object.entries(p.superseded_attempts.inputs)) pin(name, digest);
const map = json(p.source_map);
assert.strictEqual(hash(JSON.stringify(map)), p.source_map_sha256);
assert.strictEqual(Object.keys(map).length, p.source_count);
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), p.source_parent);
assert.strictEqual(p.accepted_commit, p.source_parent);
for (const [name, digest] of Object.entries(p.accepted_inputs)) {
  assert.strictEqual(hash(git(['show', p.accepted_commit + ':' + name])), digest);
}
function identities() {
  assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), p.source_parent);
  return Object.fromEntries([...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
    .filter(name => name && !name.startsWith('docs/')).sort()
    .map(name => [name, hash(fs.readFileSync(path.join(repo, name)))]));
}
function completed(name, command, previous, expected = map, code = 0, runner = p.runner) {
  assert(/^r125-development-[a-z0-9-]+$/.test(name));
  assert(Object.hasOwn(p.runners, runner), 'declared runner');
  const record = json(name + '.json');
  E.checkRecord(record, name, command, expected, previous, code, {read: json, bytes}, {
    head: p.source_parent, cwd: repo, contract: 'r125-development-raw-utc-boot-monotonic-v1', runner, after: expected,
  });
  assert(record.elapsed_seconds >= 0 && record.elapsed_seconds < record.deadline_ms / 1000);
  observeClosure(name, record);
  return {record, log: bytes(name + '.log').toString()};
}
const summaries = log => [...log.matchAll(/^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/gm)]
  .map(match => [match[1], ...match.slice(2).map(Number)]);
function accepted(spec) {
  const log = git(['show', p.accepted_commit + ':' + spec.accepted_log]);
  assert.strictEqual(hash(log), spec.accepted_log_sha256);
  return log;
}
function assertFull(log, oldLog) {
  E.assertPassing(oldLog); E.assertPassing(log);
  assert.deepStrictEqual(E.totals(oldLog), {harnesses: p.counts.harnesses, passed: p.counts.baseline_full, failed: 0, ignored: p.counts.ignored});
  assert.deepStrictEqual(E.totals(log), {harnesses: p.counts.harnesses, passed: p.counts.full, failed: 0, ignored: p.counts.ignored});
  const expected = E.executables(oldLog), actual = E.executables(log);
  assert.strictEqual(expected.length, 49); assert.strictEqual(actual.length, 49);
  const expectedSummaries = summaries(oldLog);
  assert.strictEqual(expectedSummaries.length, p.counts.harnesses);
  const libraries = expected.filter(target => target.kind === 'libtest');
  for (const [kind, count] of [['kfd', 17], ['runtime', 1]]) {
    const matches = expected.filter(target => target.kind === 'libtest' && target.name.endsWith('/fe2o3_' + kind + ')'));
    assert.strictEqual(matches.length, 1, kind + ' target identity');
    const target = matches[0], additions = p.new_tests[kind];
    assert.strictEqual(additions.length, count);
    assert.strictEqual(new Set(additions).size, count);
    assert.strictEqual(target.totals.passed, p.counts['baseline_' + kind]);
    assert(additions.every(name => !target.passing.includes(name)));
    target.passing = [...target.passing, ...additions].sort();
    target.totals.passed += count;
    expectedSummaries[libraries.indexOf(target)][1] += count;
    assert.strictEqual(target.totals.passed, p.counts[kind]);
  }
  assert.deepStrictEqual(actual, expected, 'exact full executable and test roster');
  assert.deepStrictEqual(summaries(log), expectedSummaries, 'all summary fields, including measured and filtered');
  for (const target of actual.filter(target => target.kind === 'libtest')) {
    assert.strictEqual(target.passing.length, target.totals.passed);
    assert.strictEqual(target.ignored.length, target.totals.ignored);
    assert.strictEqual(new Set([...target.passing, ...target.ignored]).size, target.passing.length + target.ignored.length);
  }
  assert.deepStrictEqual(E.passing(log), [...E.passing(oldLog), ...p.new_tests.kfd, ...p.new_tests.runtime].sort());
  assert.deepStrictEqual(E.ignored(log), E.ignored(oldLog));
}
function baseline(kind = 'gnu') {
  assert(['gnu', 'musl'].includes(kind));
  const spec = p.full_runs.find(spec => spec.kind === kind);
  const result = completed(spec.name, spec.command, spec.predecessor, map, 0, spec.runner);
  assert.strictEqual(result.record.deadline_ms, spec.deadline_ms);
  assertFull(result.log, accepted(spec));
  return result;
}
function observeClosure(name, record, scan = E.liveGroupMembers, readRecord = json) {
  assert.deepStrictEqual(record, readRecord(name + '.json'), 'closure object matches recorded bytes');
  assert.strictEqual(boot(), p.admitted_boot, 'current admitted boot');
  assert.strictEqual(record.clock.start.boot_id, p.admitted_boot, 'current-cohort record boot');
  const close = record.process_group_cleanup.close;
  assert.deepStrictEqual(scan(close.pgid), [], 'no live current-boot owned group');
  return {boot_id: p.admitted_boot, pgid: close.pgid, closure: 'current_boot_observed_absent'};
}
function stable() {
  for (const [file, value] of captured) assert.deepStrictEqual(fs.readFileSync(file), value, 'stable consumed bytes: ' + file);
}
function artifacts() {
  return [...captured].map(([file, value]) => ({file, sha256: hash(value)})).sort((a, b) => a.file.localeCompare(b.file));
}
module.exports = {observeClosure, p, E, root, repo, hash, bytes, json, pin, git, map, identities, completed, summaries, accepted, assertFull, baseline, stable, artifacts};
if (require.main === module) {
  bytes(__filename);
  const [mode = 'baseline', kind = 'gnu'] = process.argv.slice(2);
  assert(['baseline', 'restored'].includes(mode));
  const result = mode === 'baseline' ? baseline(kind) : null;
  assert.deepStrictEqual(identities(), map);
  stable();
  console.log(JSON.stringify({development_only: true, packet_accepted: false, mode, kind,
    full_passed: result ? p.counts.full : null, source_map_sha256: p.source_map_sha256, artifacts: artifacts()}));
}
