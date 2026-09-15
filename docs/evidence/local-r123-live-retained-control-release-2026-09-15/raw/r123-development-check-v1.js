// Exact R123 CPU/source evidence. Hardware and formal correspondence remain separate.
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
const p = JSON.parse(pin('r123-development-full-plan-v1.json', '049a06fa35d9933cba77c0717fbc769f8244f3d160cb541ba7f4d3efc2213ee7'));
assert.strictEqual(p.accepted, false);
for (const [name, digest] of Object.entries(p.parser_inputs)) pin(name, digest);
for (const [name, digest] of Object.entries(p.runners)) pin(name, digest);
pin('r123-development-evidence-v1.js', '8cebce6de789abe13c1191237091d1443ba392912fcec82c0b790d633a978c8b');
const E = require('./r123-development-evidence-v1.js');
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
  assert(/^r123-development-[a-z0-9-]+$/.test(name));
  assert(Object.hasOwn(p.runners, runner), 'declared runner');
  const record = json(name + '.json');
  E.checkRecord(record, name, command, expected, previous, code, {read: json, bytes}, {
    head: p.source_parent, cwd: repo, contract: 'r123-development-raw-utc-boot-monotonic-v1', runner, after: expected,
  });
  assert(record.elapsed_seconds >= 0 && record.elapsed_seconds < record.deadline_ms / 1000);
  assert.deepStrictEqual(E.liveGroupMembers(record.process_group_cleanup.close.pgid), []);
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
  for (const [kind, count] of [['kfd', 19], ['runtime', 0]]) {
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
  assert.deepStrictEqual(E.passing(log), [...E.passing(oldLog), ...p.new_tests.kfd].sort());
  assert.deepStrictEqual(E.ignored(log), E.ignored(oldLog));
}
function anchorRoster(spec, log) {
  const baseline = E.executables(accepted(p.full_runs.find(run => run.kind === 'gnu')))
    .find(target => target.kind === 'libtest' && target.name.endsWith('/fe2o3_kfd)'));
  assert(baseline, 'accepted KFD target');
  const separator = spec.command.indexOf('--');
  assert(separator >= 0, 'declared focused filter separator');
  const filters = spec.command.slice(separator + 1);
  assert(filters.length > 0 && filters.every(filter => !filter.startsWith('-')), 'substring filters only');
  const expected = [...baseline.passing, ...p.new_tests.kfd]
    .filter(name => filters.some(filter => name.includes(filter))).sort();
  assert.strictEqual(expected.length, spec.passed);
  const markers = [...log.matchAll(/^     Running (.+)$/gm)];
  assert.deepStrictEqual(markers.map(marker => marker[1].replace(/-[0-9a-f]{16}\)$/, ')')),
    ['unittests src/lib.rs (target/debug/deps/fe2o3_kfd)'], 'exact focused executable');
  assert(!/^test result:/m.test(log.slice(0, markers[0].index)), 'no unaccounted focused summary');
  assert(!/^   Doc-tests /m.test(log), 'no extra focused doctest');
  E.assertPassing(log);
  assert.deepStrictEqual(summaries(log), [['ok', spec.passed, 0, 0, 0, p.counts.kfd - spec.passed]]);
  assert.deepStrictEqual(E.passing(log), expected, 'exact filtered KFD test roster');
  assert.deepStrictEqual(E.ignored(log), [], 'no ignored focused tests');
  return expected;
}
function anchors() {
  const logs = {
    'r123-development-clippy-02': '90b02c31a28d2c11061bbfdc5a436153d8d490699889ac0cd622a74f52d7c809',
    'r123-development-focused-05': '5930fd60d41d1606f25be7132392b7d926e91f7f0142323d820cc5aefacb138a',
    'r123-development-r122-regression-01': '6ee61d7e20dbe52aa60f3c4aa37972e604615927fd7b6d0e85027046cb70c93c',
  };
  const names = [];
  for (const spec of p.anchors) {
    pin(spec.name + '.json', spec.record_sha256);
    pin(spec.name + '.log', logs[spec.name]);
    const {log} = completed(spec.name, spec.command, spec.predecessor, map, 0, spec.runner);
    if (spec.passed === null) {
      assert.strictEqual((log.match(/^\s*Finished \x60dev\x60 profile.*$/gm) || []).length, 1);
      assert(/^\s*Finished \x60dev\x60 profile.*$/.test(log.trimEnd().split('\n').at(-1)));
      assert(!/^(?:warning|error)(?:\[|:)/m.test(log));
    } else {
      names.push(...anchorRoster(spec, log));
    }
  }
  assert.strictEqual(names.length, 145);
  assert.strictEqual(new Set(names).size, 145);
}
function baseline(kind = 'gnu') {
  assert(['gnu', 'musl'].includes(kind));
  anchors();
  const spec = p.full_runs.find(spec => spec.kind === kind);
  const result = completed(spec.name, spec.command, spec.predecessor);
  assert.strictEqual(result.record.deadline_ms, spec.deadline_ms);
  assertFull(result.log, accepted(spec));
  return result;
}
function stable() {
  for (const [file, value] of captured) assert.deepStrictEqual(fs.readFileSync(file), value, 'stable consumed bytes: ' + file);
}
function artifacts() {
  return [...captured].map(([file, value]) => ({file, sha256: hash(value)})).sort((a, b) => a.file.localeCompare(b.file));
}
module.exports = {p, E, root, repo, hash, bytes, json, pin, git, map, identities, completed, summaries, accepted, assertFull, anchorRoster, anchors, baseline, stable, artifacts};
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
