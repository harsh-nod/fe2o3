const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const p = require('./r117-qualification-plan.js');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const contract = 'r117-raw-utc-boot-monotonic-v1';
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
  head: p.parent, cwd: repo, contract, runner: 'r117-run-v1.js', after: expectedMap,
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
  assert.strictEqual(r.deadline_ms, ['r117-source-campaign', 'r117-auxiliary-campaign'].includes(name) ? 7200000 : 1800000);
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
    if (expected.kind === 'libtest' && /fe2o3_kfd(?:-|\))/.test(expected.name)) {
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
function mutatedSource(original, mutation) {
  let source = original;
  for (const [from, to] of mutation.edits) {
    assert.strictEqual(source.split(from).length, 2, 'unique mutation anchor');
    source = source.replace(from, to);
  }
  assert.notStrictEqual(source, original);
  return source;
}
function originalSources(map, originals) {
  assert(Array.isArray(originals), 'original-source collection');
  assert.deepStrictEqual(originals.map(s => s.path), p.productionPaths, 'exact original-source paths');
  for (const source of originals) {
    assert.deepStrictEqual(Object.keys(source).sort(), ['path', 'text']);
    assert.strictEqual(typeof source.text, 'string');
    assert.strictEqual(hash(source.text), map[source.path], 'pinned original bytes');
  }
  return Object.fromEntries(originals.map(s => [s.path, s.text]));
}
function mutationSource(map, originals, mutation) {
  const sources = originalSources(map, originals);
  assert(p.productionPaths.includes(mutation.path), 'declared mutation source');
  return mutatedSource(sources[mutation.path], mutation);
}
function recoverySource(manifest, map, mutation) {
  assert(p.productionPaths.includes(mutation.path), 'declared recovery source');
  return {manifest, path: mutation.path, sha256: map[mutation.path]};
}
function expectedEntries(map, originals) {
  const entries = {};
  let predecessor = 'r117-qualification-manifest.json';
  const hashes = [];
  originalSources(map, originals);
  for (const mutation of p.mutations) {
    const changed = hash(mutationSource(map, originals, mutation));
    const mapHash = hash(JSON.stringify({...map, [mutation.path]: changed}));
    hashes.push([mutation.name, mapHash]);
    const run = 'r117-qualified-mut-' + mutation.name + '.json';
    entries[run] = {kind: 'run', predecessor, source_map_sha256: mapHash,
      command: [...p.test, mutation.test, '--', '--exact', '--test-threads=1'], returncode: 101};
    predecessor = run;
    const restored = 'r117-qualified-restoration-' + mutation.name + '.json';
    entries[restored] = {kind: 'restoration', predecessor, source_map_sha256: hash(JSON.stringify(map))};
    predecessor = restored;
  }
  checkMutationGroups(hashes);
  for (const [kind, filter] of p.focused) {
    const run = 'r117-qualified-restored-' + kind + '.json';
    entries[run] = {kind: 'run', predecessor, source_map_sha256: hash(JSON.stringify(map)), command: [...p.test, filter], returncode: 0};
    predecessor = run;
  }
  entries['r117-qualified-collector-validation.json'] = {kind: 'run', predecessor,
    source_map_sha256: hash(JSON.stringify(map)), command: ['node', file('r117-qualification-collect.js'), '--check'], returncode: 0};
  assert.strictEqual(Object.keys(entries).length, 2 * p.mutations.length + p.focused.length + 1);
  return entries;
}
function canRestore(record) {
  if (!record || record.child_closed !== true) return false;
  const cleanup = record.process_group_cleanup?.close;
  if (cleanup?.status === 'not_spawned') return typeof record.spawn_error === 'string' &&
    record.child_returncode < 0 && !Object.hasOwn(cleanup, 'pgid');
  return record.spawn_error === null && ['absent', 'signaled'].includes(cleanup?.status) &&
    Number.isSafeInteger(cleanup.pgid) && cleanup.pgid > 1 &&
    Array.isArray(cleanup.live_members) && cleanup.live_members.length === 0;
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
function restoreOwnedMutation(target, expectedHash, original, record, io = {
  read: file => fs.readFileSync(file), write: (file, bytes) => fs.writeFileSync(file, bytes), live: liveGroupMembers,
}) {
  const result = {restored: false, quiescent: canRestore(record), live_members: null, observed_source_sha256: null, external_edit: false};
  if (result.quiescent && record.process_group_cleanup.close.pgid) {
    result.live_members = io.live(record.process_group_cleanup.close.pgid);
    result.quiescent = result.live_members.length === 0;
  }
  if (!result.quiescent) return result;
  // Recheck after the process scan; never overwrite a detected external edit.
  result.observed_source_sha256 = hash(io.read(target));
  if (result.observed_source_sha256 !== expectedHash) {
    result.external_edit = true;
    return result;
  }
  io.write(target, original);
  result.restored = true;
  return result;
}

function checkMutationGroups(entries) {
  assert.deepStrictEqual(entries.map(([name]) => name), p.mutations.map(m => m.name), 'exact mutation execution names');
  const byHash = new Map();
  for (const [name, digest] of entries) {
    assert(/^[a-f0-9]{64}$/.test(digest), 'full mutation map hash');
    const group = byHash.get(digest) || [];
    group.push(name);
    byHash.set(digest, group);
  }
  const groups = [...byHash.values()].map(names => names.sort()).sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
  assert.deepStrictEqual(groups, p.mutationSourceGroups, 'exact repeated-mutation grouping');
  assert.strictEqual(byHash.size, p.uniqueMutationCount);
}
function historicalNames() {
  const runs = p.mutations.map(m => 'r117-qualified-mut-' + m.name)
    .concat(p.focused.map(([kind]) => 'r117-qualified-restored-' + kind), ['r117-qualified-collector-validation']);
  const excluded = new Set(runs.flatMap(n => ['.json', '.log', '-source.json', '-source-after.json'].map(s => n + s))
    .concat(p.mutations.flatMap(m => ['.json', '-source.json'].map(s => 'r117-qualified-restoration-' + m.name + s)),
      ['r117-qualification-manifest.json']));
  assert.strictEqual(excluded.size, 6 * p.mutations.length + 4 * p.focused.length + 5);
  const local = fs.readdirSync(root).filter(n => /^r117-.*\.(json|log|js|py)$/.test(n) && !excluded.has(n));
  const isolated = fs.readdirSync(root).filter(n => /^detachedcandidate-.*\.(json|log|js|md)$/.test(n)).sort();
  assert.deepStrictEqual(isolated, p.isolatedArtifacts, 'complete original-context candidate cohort');
  return local.concat(isolated).sort();
}
function checkIsolatedHistory(integratedMap, io = {read, bytes}) {
  assert.strictEqual(hash(io.bytes(p.isolatedContext.runner)), p.isolatedRunnerHash);
  const baseline = JSON.parse(committed('raw/r116-frozen-source.json'));
  const nativeBaseline = JSON.parse(git(['show', p.isolatedContext.head + ':' + p.isolatedPrevious + 'raw/r115-frozen-source.json']));
  const prefix = 'queue::dispatch_binding::control_release::tests::';
  const oldNames = passing(committed('raw/r116-reviewed-gnu-all.log')).filter(n => n.startsWith(prefix));
  assert.strictEqual(oldNames.length, 15);
  const names = [...oldNames, ...p.newTests].sort();
  let previous = null;
  let finalMap = null;
  for (const attempt of p.isolatedRuns) {
    const {name, command, before: beforeHash, after: afterHash, code} = attempt;
    const before = io.read(name + '-source.json');
    const after = io.read(name + '-source-after.json');
    assert.strictEqual(Object.keys(before).length, p.isolatedSourceCount);
    assert.strictEqual(Object.keys(after).length, p.isolatedSourceCount);
    assert.strictEqual(hash(JSON.stringify(before)), beforeHash, 'original-context before map');
    assert.strictEqual(hash(JSON.stringify(after)), afterHash, 'original-context after map');
    checkRecord(io.read(name + '.json'), name, command, before, previous, code, io, {...p.isolatedContext, after});
    const log = io.bytes(name + '.log').toString();
    if (name.endsWith('-controls')) {
      const rejected = code === 101;
      assert.deepStrictEqual(passing(log), names.filter(n => !rejected || n !== p.sourceGuard));
      assert.deepStrictEqual(failed(log), rejected ? [p.sourceGuard] : []);
      assert.deepStrictEqual(totals(log), {harnesses: 1, passed: rejected ? 30 : 31, failed: Number(rejected), ignored: 0});
      assert(log.includes('Finished \x60test\x60 profile'));
      assert(log.includes('Running unittests'));
      assert(!/^error\[E\d+\]:/m.test(log), 'historical guard failure is not a compile failure');
    } else if (name.endsWith('-format')) {
      assert.strictEqual(code, 0);
      assert.strictEqual(log, '');
    } else if (name.endsWith('-clippy')) {
      if (code === 101) {
        assert(log.includes('clippy::collapsible_if'), 'preserved historical Clippy failure');
        assert(log.includes('could not compile \x60fe2o3-kfd\x60 (lib test) due to 1 previous error'));
      } else {
        assert.strictEqual(code, 0);
        assert(!/^error(?:\[|:)/m.test(log));
        assert(log.includes('Finished \x60dev\x60 profile'));
      }
    } else assert.fail('unexpected isolated attempt');
    previous = name + '.json';
    finalMap = after;
  }
  assert(finalMap);
  const delta = [...new Set([...Object.keys(nativeBaseline), ...Object.keys(finalMap)])].filter(n => nativeBaseline[n] !== finalMap[n]).sort();
  assert.deepStrictEqual(delta, p.sourceDelta, 'original native candidate source delta');
  assert.deepStrictEqual(Object.keys(finalMap).filter(n => !Object.hasOwn(nativeBaseline, n)).sort(), p.addedSource);
  assert.deepStrictEqual(Object.keys(nativeBaseline).filter(n => !Object.hasOwn(finalMap, n)), []);
  const expected = Object.fromEntries(Object.entries({...baseline, ...Object.fromEntries(p.sourceDelta.map(n => [n, finalMap[n]]))}).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0));
  assert.deepStrictEqual(integratedMap, expected, 'exact integration above accepted R116');
  return {runs: p.isolatedRuns.length, artifacts: p.isolatedArtifacts.length, source_head: p.isolatedContext.head,
    source_map_sha256: hash(JSON.stringify(finalMap)), retained_failed_attempts: 2, accepted_as_integrated_prerequisite: false};
}
module.exports = {root, repo, contract, file, hash, bytes, read, git, committed, identities,
  sample, ordered, checkRecord, checkRun, passing, ignored, failed, totals, executables, checkFull, checkMutation, mutatedSource,
  originalSources, mutationSource, recoverySource, expectedEntries, canRestore, liveGroupMembers, restoreOwnedMutation, historicalNames,
  checkMutationGroups, checkIsolatedHistory};
