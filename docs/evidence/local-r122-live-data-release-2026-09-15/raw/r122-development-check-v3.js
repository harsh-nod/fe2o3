// Read-only R122 source, full-suite and behavioral-negative checks.
// Native execution and authenticated formal correspondence are separate acceptance gates.
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
const p = JSON.parse(pin('r122-development-mutations-v1.json', '8baf6407a324331eea491857c2c3a051dac30ae6e61b1c76b130a490ab3a94f8'));
for (const [name, digest] of Object.entries(p.parser_inputs)) pin(name, digest);
pin(p.runner, p.runner_sha256);
const E = require('./r119-integrated-qualification-evidence-v1.js');
const core = require('./r119-integrated-mutation-core-v1.js');
const map = json(p.source_map);
assert.strictEqual(hash(JSON.stringify(map)), p.source_map_sha256);
assert.strictEqual(Object.keys(map).length, p.source_count);
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), p.source_parent);
function identities() {
  assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), p.source_parent);
  return Object.fromEntries([...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
    .filter(name => name && !name.startsWith('docs/')).sort()
    .map(name => [name, hash(fs.readFileSync(path.join(repo, name)))]));
}
function completed(name, command, previous, expected = map, code = 0) {
  assert(/^r122-development-[a-z0-9-]+$/.test(name));
  const record = json(name + '.json');
  E.checkRecord(record, name, command, expected, previous, code, {read: json, bytes}, {
    head: p.source_parent, cwd: repo, contract: 'r122-development-raw-utc-boot-monotonic-v1',
    runner: p.runner, after: expected,
  });
  assert(record.elapsed_seconds >= 0 && record.elapsed_seconds < record.deadline_ms / 1000);
  assert.deepStrictEqual(E.liveGroupMembers(record.process_group_cleanup.close.pgid), []);
  return {record, log: bytes(name + '.log').toString()};
}
const summaries = log => [...log.matchAll(/^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/gm)]
  .map(match => [match[1], ...match.slice(2).map(Number)]);
function assertFull(log, oldLog) {
  E.assertPassing(oldLog); E.assertPassing(log);
  assert.deepStrictEqual(E.totals(oldLog), {harnesses: p.counts.harnesses, passed: p.counts.baseline_full, failed: 0, ignored: p.counts.ignored});
  assert.deepStrictEqual(E.totals(log), {harnesses: p.counts.harnesses, passed: p.counts.full, failed: 0, ignored: p.counts.ignored});
  const expected = E.executables(oldLog), actual = E.executables(log);
  assert.strictEqual(expected.length, 49); assert.strictEqual(actual.length, 49);
  const expectedSummaries = summaries(oldLog);
  assert.strictEqual(expectedSummaries.length, p.counts.harnesses);
  const libraries = expected.filter(target => target.kind === 'libtest');
  for (const [kind, count] of [['kfd', 18], ['runtime', 10]]) {
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
function baseline() {
  const previous = completed('r122-development-clippy-v2',
    ['cargo', 'clippy', '-p', 'fe2o3-kfd', '-p', 'fe2o3-runtime', '--all-features', '--all-targets', '--', '-D', 'warnings'],
    'r122-development-clippy-v1.json');
  assert.strictEqual((previous.log.match(/^\s*Finished \x60dev\x60 profile.*$/gm) || []).length, 1);
  assert(/^\s*Finished \x60dev\x60 profile.*$/.test(previous.log.trimEnd().split('\n').at(-1)));
  assert(!/^(?:warning|error)(?:\[|:)/m.test(previous.log));
  const result = completed(p.baseline.name, p.baseline.command, p.baseline.predecessor);
  const accepted = p.accepted_baseline;
  assert.strictEqual(accepted.commit, p.source_parent);
  const oldLog = git(['show', accepted.commit + ':' + accepted.path]);
  assert.strictEqual(hash(oldLog), accepted.sha256);
  assertFull(result.log, oldLog);
  return result;
}
function assertMutationSource(mutation, changed) {
  assert.strictEqual(hash(changed), mutation.expected_file_sha256, 'declared mutated bytes');
  const reverse = {...mutation.patch, edits: mutation.patch.edits.toReversed().map(([from, to]) => [to, from])};
  const original = core.mutatedSource(changed, reverse);
  assert.strictEqual(hash(original), map[mutation.patch.path], 'authenticated recovered original');
  assert.strictEqual(core.mutatedSource(original, mutation.patch), changed, 'exact scoped patch derivation');
}
function assertNegative(log, mutation) {
  assert(p.new_tests[mutation.kind].includes(mutation.test));
  assert(!p.source_guards.includes(mutation.test));
  assert(!/panic in a destructor during cleanup|panic in a function that cannot unwind|thread caused non-unwinding panic|SIGABRT|signal: 6\b|^Aborted(?: \(core dumped\))?\s*$|fatal runtime error:|has overflowed its stack|memory allocation of \d+ bytes failed|could not compile|^error\[E\d+\]:/m.test(log));
  const blocks = log.split('---- ' + mutation.test + ' stdout ----');
  assert.strictEqual(blocks.length, 2);
  const body = blocks[1].split('\nfailures:\n')[0];
  const panic = body.slice(body.lastIndexOf("thread '")).trim();
  const header = /^thread '([^'\n]+)' \(([1-9][0-9]*)\) panicked at ([^\n]+):\n/.exec(panic);
  assert(header, 'complete final panic header');
  assert.strictEqual(header[1], mutation.test);
  // Rust preserves the #[path] module traversal in this diagnostic location.
  const canonical = 'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_data_release_tests.rs';
  const compiler = 'crates/fe2o3-kfd/src/queue_live/construction_primary/../construction_auxiliary/integration_data_release_tests.rs';
  const location = /^(.+):([1-9][0-9]*):([1-9][0-9]*)$/.exec(mutation.oracle_location);
  assert(location, 'declared assertion coordinates');
  const expectedLocation = [location[1] === canonical ? compiler : location[1], location[2], location[3]].join(':');
  assert.strictEqual(header[3], expectedLocation, 'predeclared behavioral assertion');
  assert(mutation.oracle.every(fragment => panic.includes(fragment)), 'predeclared diagnostic');
  assert.deepStrictEqual(E.failed(log), [mutation.test]);
  assert.deepStrictEqual(E.passing(log), []);
  assert.deepStrictEqual(E.ignored(log), []);
  assert.deepStrictEqual(E.totals(log), {harnesses: 1, passed: 0, failed: 1, ignored: 0});
  assert.deepStrictEqual(summaries(log), [['FAILED', 0, 1, 0, 0, p.counts[mutation.kind] - 1]]);
  const markers = [...log.matchAll(/^     Running (.+)$/gm)];
  assert.deepStrictEqual(markers.map(marker => marker[1].replace(/-[0-9a-f]{16}\)$/, ')')),
    ['unittests src/lib.rs (target/debug/deps/fe2o3_' + mutation.kind + ')'], 'exact sole negative executable');
  assert(!/^test result:/m.test(log.slice(0, markers[0].index)), 'no unaccounted summary');
  assert(!/^   Doc-tests /m.test(log), 'no extra doc-test executable');
  assert(log.includes('Finished \x60test\x60 profile'));
  assert(log.includes('\nrunning 1 test\n'));
  return panic;
}
function negative(mutation) {
  const changed = identities();
  assert.deepStrictEqual(Object.keys(changed), Object.keys(map));
  assert.deepStrictEqual(Object.keys(map).filter(name => map[name] !== changed[name]), [mutation.patch.path]);
  assertMutationSource(mutation, bytes(path.join(repo, mutation.patch.path)).toString());
  baseline();
  const name = 'r122-development-final-negative-' + mutation.id + '-v1';
  const {record, log} = completed(name, [...p.test_commands[mutation.kind], mutation.test, '--', '--exact'],
    p.baseline.name + '.json', changed, 101);
  const panic = assertNegative(log, mutation);
  assert.deepStrictEqual(identities(), changed, 'unchanged closing mutant source map');
  return {development_only: true, id: mutation.id, quiescent: true, expected_failure_observed: true,
    source_map_sha256: record.source_map_sha256, log_sha256: hash(log), decisive_panic: panic};
}
function stable() {
  for (const [file, value] of captured) assert.deepStrictEqual(fs.readFileSync(file), value, 'stable consumed bytes: ' + file);
}
function artifacts() {
  return [...captured].map(([file, value]) => ({file, sha256: hash(value)})).sort((a, b) => a.file.localeCompare(b.file));
}
module.exports = {p, E, root, repo, hash, bytes, json, pin, git, map, identities, completed, summaries, assertFull, baseline, assertMutationSource, assertNegative, negative, stable, artifacts};
if (require.main === module) {
  bytes(__filename);
  const [mode, id] = process.argv.slice(2);
  let report;
  if (mode === 'baseline') {
    const result = baseline();
    assert.deepStrictEqual(identities(), map);
    report = {development_only: true, baseline_passed: p.counts.full, kfd_passed: p.counts.kfd, runtime_passed: p.counts.runtime,
      elapsed_seconds: result.record.elapsed_seconds, source_map_sha256: p.source_map_sha256, quiescent: true};
  } else if (mode === 'restored') {
    assert.deepStrictEqual(identities(), map);
    report = {development_only: true, restored: true, process_closure_checked: false, source_map_sha256: p.source_map_sha256, identities: p.source_count};
  } else {
    assert.strictEqual(mode, 'negative');
    const mutation = p.mutations.find(m => m.id === id);
    assert(mutation, 'declared mutation');
    report = negative(mutation);
  }
  stable();
  console.log(JSON.stringify(report));
}
