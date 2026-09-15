const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const p = require('./r118-history-preflight-plan-v1.js');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const contract = 'r118-raw-utc-boot-monotonic-v1';
const file = name => path.join(root, name);
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const bytes = name => {assert(fs.lstatSync(file(name)).isFile(), 'regular artifact ' + name); return fs.readFileSync(file(name));};
const read = name => JSON.parse(bytes(name));
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const committed = name => git(['show', p.parent + ':' + p.previous + name]);
function identities() {
  assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), p.parent);
  const names = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
    .filter(n => n && !n.startsWith('docs/')).sort();
  return Object.fromEntries(names.map(n => [n, hash(fs.readFileSync(path.join(repo, n)))]));
}
const sample = () => ({utc_ms: Date.now(), monotonic_ns: process.hrtime.bigint().toString(),
  boot_id: fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim(), clock_source: 'node-process-hrtime-linux-monotonic'});
function ordered(a, b) {
  for (const s of [a, b]) {
    assert(Number.isSafeInteger(s.utc_ms) && s.utc_ms >= 0, 'valid raw UTC');
    assert(typeof s.monotonic_ns === 'string' && /^\d+$/.test(s.monotonic_ns), 'valid monotonic observation');
    assert(/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(s.boot_id), 'valid boot identity');
    assert.strictEqual(s.clock_source, 'node-process-hrtime-linux-monotonic', 'exact monotonic source');
  }
  assert.strictEqual(a.boot_id, b.boot_id, 'same boot');
  assert(BigInt(b.monotonic_ns) >= BigInt(a.monotonic_ns), 'monotonic order');
}
function checkRecord(r, name, command, expectedMap, previousName, code, io = {read, bytes}, context = {
  head: p.parent, cwd: repo, contract, runner: 'r118-run-v1.js', after: expectedMap,
}) {
  assert.strictEqual(r.source_head, context.head);
  assert.strictEqual(r.cwd, context.cwd);
  assert.deepStrictEqual(r.command, command, 'exact command');
  assert.strictEqual(r.log, file(name + '.log'));
  assert.strictEqual(r.source, file(name + '-source.json'));
  assert.strictEqual(r.source_after, file(name + '-source-after.json'));
  assert.strictEqual(r.source_unchanged, hash(JSON.stringify(expectedMap)) === hash(JSON.stringify(context.after)));
  assert.strictEqual(r.returncode, code);
  assert.strictEqual(r.child_returncode, code, 'raw child result');
  assert.strictEqual(r.child_closed, true, 'observed child closure');
  assert.strictEqual(r.spawn_error, null);
  assert.strictEqual(r.signal, null);
  assert.strictEqual(r.timed_out, false);
  assert.strictEqual(r.deadline_ms, ['r118-source-campaign', 'r118-auxiliary-campaign'].includes(name) ? 7200000 : 1800000);
  assert.deepStrictEqual(r.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
  assert.deepStrictEqual(r.runner, {name: context.runner, sha256: hash(io.bytes(context.runner))});
  assert.deepStrictEqual(io.read(name + '-source.json'), expectedMap);
  assert.deepStrictEqual(io.read(name + '-source-after.json'), context.after);
  assert.strictEqual(r.source_map_sha256, hash(JSON.stringify(expectedMap)));
  assert.strictEqual(r.clock.contract, context.contract);
  assert.strictEqual(r.clock.error, null);
  if (previousName === null) {
    assert.strictEqual(r.clock.predecessor, null, 'independent anchor has no predecessor');
  } else {
    const previous = io.read(previousName);
    assert.strictEqual(previous.source_head, context.head);
    assert.strictEqual(previous.clock.contract, context.contract);
    assert.strictEqual(previous.clock.error, null);
    assert.deepStrictEqual(r.clock.predecessor, {name: previousName, sha256: hash(io.bytes(previousName)), observation: previous.clock.source_verified});
    ordered(previous.clock.source_verified, r.clock.start);
  }
  ordered(r.clock.start, r.clock.finish);
  const cleanup = r.process_group_cleanup;
  assert.strictEqual(cleanup.timeout, null);
  assert(['absent', 'signaled'].includes(cleanup.close.status), 'successful group cleanup');
  assert(Number.isSafeInteger(cleanup.close.pgid) && cleanup.close.pgid > 1);
  assert.deepStrictEqual(cleanup.close.live_members, [], 'no live owned-group members');
  if (cleanup.close.status === 'absent') assert(cleanup.close.error.includes('ESRCH'));
  else assert.strictEqual(cleanup.close.error, undefined);
  ordered(r.clock.finish, cleanup.close.observation);
  ordered(cleanup.close.observation, r.clock.source_verified);
  assert.strictEqual(r.started_at, new Date(r.clock.start.utc_ms).toISOString());
  assert.strictEqual(r.finished_at, new Date(r.clock.finish.utc_ms).toISOString());
  assert.strictEqual(r.elapsed_seconds, Number(BigInt(r.clock.finish.monotonic_ns) - BigInt(r.clock.start.monotonic_ns)) / 1e9);
  assert.strictEqual(r.verification_elapsed_seconds, Number(BigInt(r.clock.source_verified.monotonic_ns) - BigInt(r.clock.finish.monotonic_ns)) / 1e9);
}
function checkRun(name, command, map, previous, code = 0) {
  const record = read(name + '.json');
  checkRecord(record, name, command, map, previous, code);
  return {record, log: bytes(name + '.log').toString()};
}
const passing = text => [...text.matchAll(/^test (.+) \.\.\. ok$/gm)].map(m => m[1]).sort();
const ignored = text => [...text.matchAll(/^test (.+) \.\.\. ignored(?:,.*)?$/gm)].map(m => m[1]).sort();
const failed = text => [...text.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map(m => m[1]).sort();
function totals(text) {
  const rows = [...text.matchAll(/^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;/gm)];
  assert(rows.length > 0, 'nonempty test summaries');
  return rows.reduce((a, m) => ({harnesses: a.harnesses + 1, passed: a.passed + Number(m[1]),
    failed: a.failed + Number(m[2]), ignored: a.ignored + Number(m[3])}), {harnesses: 0, passed: 0, failed: 0, ignored: 0});
}
function executables(log) {
  const markers = [...log.matchAll(/^     Running (.+)$/gm)];
  assert(markers.length > 0, 'nonempty executable roster');
  assert(!/^test result:/m.test(log.slice(0, markers[0].index)), 'unaccounted test summary');
  const benchmarkNames = [
    'benches/completion_scaling.rs (target/debug/deps/completion_scaling)',
    'benches/completion_scaling.rs (target/x86_64-unknown-linux-musl/debug/deps/completion_scaling)',
  ];
  let benchmarks = 0;
  const result = markers.map((marker, i) => {
    const text = log.slice(marker.index, markers[i + 1]?.index ?? log.length);
    const name = marker[1].replace(/-[0-9a-f]{16}\)$/, ')');
    if (benchmarkNames.includes(name)) {
      benchmarks++;
      const lines = text.slice(text.indexOf('\n') + 1).trim().split('\n');
      assert.strictEqual(lines.length, 5, 'exact benchmark output rows');
      assert.strictEqual(lines[0], 'nodes,construction_ns,transitions_ns,transition_ns_per_node');
      const cases = lines.slice(1).map(line => {
        assert(/^\d+,\d+,\d+,\d+\.\d+$/.test(line), 'numeric benchmark CSV');
        const values = line.split(',').map(Number);
        assert(values.every(value => Number.isFinite(value) && value >= 0));
        assert(values.slice(0, 3).every(Number.isSafeInteger));
        return values[0];
      });
      assert.deepStrictEqual(cases, [1024, 4096, 16384, 65536]);
      return {kind: 'harnessless-benchmark', name, cases};
    }
    const summary = totals(text);
    assert.strictEqual(summary.harnesses, 1, 'one summary per libtest executable');
    return {kind: 'libtest', name, passing: passing(text), ignored: ignored(text), totals: summary};
  });
  assert.strictEqual(benchmarks, 1, 'one named harnessless benchmark');
  return result;
}
function checkFull(text, oldText, additions = p.newTests) {
  assert.deepStrictEqual(totals(oldText), {harnesses: 48, passed: p.baselineFullPassed, failed: 0, ignored: 5});
  assert.deepStrictEqual(passing(text), [...passing(oldText), ...additions].sort());
  assert.deepStrictEqual(ignored(text), ignored(oldText));
  const targets = executables(text);
  const oldTargets = executables(oldText);
  assert.strictEqual(targets.length, 49);
  assert.strictEqual(oldTargets.length, 49);
  assert.strictEqual(targets.filter(t => t.kind === 'libtest').length, 48);
  let changed = 0;
  for (const [index, actual] of targets.entries()) {
    const expected = structuredClone(oldTargets[index]);
    if (expected.kind === 'libtest' && /fe2o3_runtime(?:-|\))/.test(expected.name)) {
      expected.passing = [...expected.passing, ...additions].sort();
      expected.totals.passed += additions.length;
      changed++;
    }
    assert.deepStrictEqual(actual, expected, 'exact executable roster: ' + actual.name);
  }
  assert.strictEqual(changed, 1);
  assert.deepStrictEqual(totals(text), {harnesses: 48, passed: p.baselineFullPassed + additions.length, failed: 0, ignored: 5});
  return totals(text);
}
function checkMutation(log, mutation) {
  assert(p.allowedMutationTests.includes(mutation.test));
  assert.notStrictEqual(mutation.test, p.sourceGuard);
  assert.notStrictEqual(mutation.test, p.retainedSourceGuard);
  const escaped = text => text.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const panic = new RegExp("^thread '" + escaped(mutation.test) + "'(?: \\(\\d+\\))? panicked at " +
    escaped(mutation.oracle_path) + ':' + mutation.oracle_line + ':\\d+:$', 'm');
  assert(panic.test(log), 'exact failing-test panic location');
  assert.deepStrictEqual(failed(log), [mutation.test], 'one named failure');
  assert.deepStrictEqual(passing(log), []);
  assert.deepStrictEqual(totals(log), {harnesses: 1, passed: 0, failed: 1, ignored: 0});
  assert(!/^error\[E\d+\]:/m.test(log), 'not a compile failure');
  for (const marker of ['Finished `test` profile', 'Running unittests', 'running 1 test', ...mutation.expected]) {
    assert(log.includes(marker), 'expected mutation diagnostic: ' + marker);
  }
}
const core = require('./r118-mutation-core-v1.js');
const originalSources = (map, originals) => core.originalSources(map, originals, p.mutationPaths);
const mutationSources = (map, originals, mutation) => core.mutationSources(map, originals, mutation, p.mutationPaths);
const mutationMap = core.mutationMap;
const focusedCommand = ([, filter, , exact]) => [...p.test, filter, ...(exact ? ['--', '--exact'] : [])];
function checkMutationGroups(entries) {
  assert.deepStrictEqual(entries.map(([name]) => name), p.mutations.map(mutation => mutation.name));
  const byHash = new Map();
  for (const [name, digest] of entries) {
    assert(/^[a-f0-9]{64}$/.test(digest));
    byHash.set(digest, [...(byHash.get(digest) || []), name]);
  }
  const groups = [...byHash.values()].map(names => names.sort()).sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
  assert.deepStrictEqual(groups, p.mutationSourceGroups, 'exact repeated-mutation grouping');
  assert.strictEqual(byHash.size, p.uniqueMutationCount);
}
function expectedEntries(map, originals) {
  const entries = {};
  let predecessor = 'r118-qualification-manifest.json';
  const hashes = [];
  originalSources(map, originals);
  for (const mutation of p.mutations) {
    const files = mutationSources(map, originals, mutation);
    const mapHash = hash(JSON.stringify(mutationMap(map, files)));
    hashes.push([mutation.name, mapHash]);
    const run = 'r118-qualified-mut-' + mutation.name + '.json';
    entries[run] = {kind: 'run', predecessor, source_map_sha256: mapHash,
      command: [...p.test, mutation.test, '--', '--exact', '--test-threads=1'], returncode: 101};
    predecessor = run;
    const restored = 'r118-qualified-restoration-' + mutation.name + '.json';
    entries[restored] = {kind: 'restoration', predecessor, source_map_sha256: hash(JSON.stringify(map))};
    predecessor = restored;
  }
  checkMutationGroups(hashes);
  for (const focused of p.focused) {
    const run = 'r118-qualified-restored-' + focused[0] + '.json';
    entries[run] = {kind: 'run', predecessor, source_map_sha256: hash(JSON.stringify(map)), command: focusedCommand(focused), returncode: 0};
    predecessor = run;
  }
  entries['r118-qualified-collector-validation.json'] = {kind: 'run', predecessor,
    source_map_sha256: hash(JSON.stringify(map)), command: ['node', file('r118-qualification-collect.js'), '--check'], returncode: 0};
  assert.strictEqual(Object.keys(entries).length, 2 * p.mutations.length + p.focused.length + 1);
  return entries;
}
function checkRestoration(restoration, files, map, manifestHash, predecessor, record, restoredMap) {
  assert.strictEqual(restoration.source_head, p.parent);
  assert.strictEqual(restoration.manifest_sha256, manifestHash);
  assert.strictEqual(restoration.source_map_sha256, hash(JSON.stringify(map)));
  assert.strictEqual(restoration.source_unchanged, true);
  assert.strictEqual(restoration.source_identities, p.sourceCount);
  assert.deepStrictEqual(restoration.mutated_sources, files.map(source => ({path: source.path, sha256: source.changed_sha256})));
  assert.deepStrictEqual(restoration.application_attempts, files.map(source => ({path: source.path, verified: true})));
  assert.deepStrictEqual(restoration.restoration, {restored: true, quiescent: true,
    all_mutant_before_restore: true, live_members: [], external_edit: false, error: null,
    files: files.map(source => ({path: source.path, before_sha256: source.changed_sha256,
      after_sha256: source.original_sha256, write_attempted: true}))});
  assert.deepStrictEqual(restoredMap, map);
  assert.strictEqual(restoration.clock.contract, contract);
  assert.strictEqual(restoration.clock.error, null);
  assert.strictEqual(restoration.clock.kind, 'restoration');
  assert.deepStrictEqual(restoration.clock.predecessor, {name: predecessor.name,
    sha256: predecessor.sha256, observation: record.clock.source_verified});
  ordered(record.clock.source_verified, restoration.clock.start);
  ordered(restoration.clock.start, restoration.clock.source_verified);
}
function historicalNames() {
  const runs = p.mutations.map(mutation => 'r118-qualified-mut-' + mutation.name)
    .concat(p.focused.map(([kind]) => 'r118-qualified-restored-' + kind), ['r118-qualified-collector-validation']);
  const excluded = new Set(runs.flatMap(name => ['.json', '.log', '-source.json', '-source-after.json'].map(suffix => name + suffix))
    .concat(p.mutations.flatMap(mutation => ['.json', '-source.json'].map(suffix => 'r118-qualified-restoration-' + mutation.name + suffix)),
      ['r118-qualification-manifest.json']));
  const local = fs.readdirSync(root).filter(name => /^r118-.*\.(json|log|js|py|md)$/.test(name) && !excluded.has(name));
  return [...new Set([...local, ...p.history.artifacts.map(item => item.name)])].sort();
}
function liveGroupMembers(pgid) {
  assert(Number.isSafeInteger(pgid) && pgid > 1);
  const members = [];
  for (const name of fs.readdirSync('/proc')) {
    if (!/^[1-9][0-9]*$/.test(name)) continue;
    let text;
    try {text = fs.readFileSync('/proc/' + name + '/stat', 'utf8');}
    catch (error) {if (['ENOENT', 'ESRCH'].includes(error.code)) continue; throw error;}
    const fields = text.slice(text.lastIndexOf(')') + 2).trim().split(/\s+/);
    if (Number(fields[2]) === pgid && !['Z', 'X'].includes(fields[0])) members.push(Number(name));
  }
  return members.sort((a, b) => a - b);
}
module.exports = {root, repo, contract, file, hash, bytes, read, git, committed, identities,
  sample, ordered, checkRecord, checkRun, passing, ignored, failed, totals, executables, checkFull, checkMutation,
  core, originalSources, mutationSources, mutationMap, focusedCommand, checkMutationGroups, expectedEntries,
  checkRestoration, historicalNames, liveGroupMembers};
