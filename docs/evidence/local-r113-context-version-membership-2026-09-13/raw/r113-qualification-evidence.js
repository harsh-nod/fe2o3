const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const p = require('./r113-qualification-plan.js');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const contract = 'r113-raw-utc-boot-monotonic-v1';
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
function checkRecord(r, name, command, expectedMap, previousName, code, io = {read, bytes}) {
  assert.strictEqual(r.source_head, p.parent);
  assert.strictEqual(r.cwd, repo);
  assert.deepStrictEqual(r.command, command, 'exact command');
  assert.strictEqual(r.log, file(name + '.log'));
  assert.strictEqual(r.source, file(name + '-source.json'));
  assert.strictEqual(r.source_after, file(name + '-source-after.json'));
  assert.strictEqual(r.source_unchanged, true);
  assert.strictEqual(r.returncode, code);
  assert.strictEqual(r.child_returncode, code, 'raw child result');
  assert.strictEqual(r.child_closed, true, 'observed child closure');
  assert.strictEqual(r.spawn_error, null);
  assert.strictEqual(r.signal, null);
  assert.strictEqual(r.timed_out, false);
  assert.strictEqual(r.deadline_ms, ['r113-source-campaign', 'r113-auxiliary-campaign'].includes(name) ? 7200000 : 1800000);
  assert.deepStrictEqual(r.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
  assert.deepStrictEqual(r.runner, {name: 'r113-run-v3.js', sha256: hash(io.bytes('r113-run-v3.js'))});
  assert.deepStrictEqual(io.read(name + '-source.json'), expectedMap);
  assert.deepStrictEqual(io.read(name + '-source-after.json'), expectedMap);
  assert.strictEqual(r.source_map_sha256, hash(JSON.stringify(expectedMap)));
  assert.strictEqual(r.clock.contract, contract);
  assert.strictEqual(r.clock.error, null);
  const previous = io.read(previousName);
  assert.strictEqual(previous.source_head, p.parent);
  assert.strictEqual(previous.clock.contract, contract);
  assert.strictEqual(previous.clock.error, null);
  assert.deepStrictEqual(r.clock.predecessor, {name: previousName, sha256: hash(io.bytes(previousName)), observation: previous.clock.source_verified});
  ordered(previous.clock.source_verified, r.clock.start);
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
function checkMutation(log, mutation) {
  assert(p.allowedMutationTests.includes(mutation.test));
  assert.notStrictEqual(mutation.test, p.sourceGuard);
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
function expectedEntries(map, original) {
  const entries = {};
  let predecessor = 'r113-qualification-manifest.json';
  const hashes = new Set();
  assert.strictEqual(hash(original), map[p.production]);
  for (const mutation of p.mutations) {
    assert.strictEqual(mutation.path, p.production);
    const changed = hash(mutatedSource(original, mutation));
    hashes.add(changed);
    const run = 'r113-qualified-mut-' + mutation.name + '.json';
    entries[run] = {kind: 'run', predecessor, source_map_sha256: hash(JSON.stringify({...map, [mutation.path]: changed})),
      command: [...p.test, mutation.test, '--', '--exact'], returncode: 101};
    predecessor = run;
    const restored = 'r113-qualified-restoration-' + mutation.name + '.json';
    entries[restored] = {kind: 'restoration', predecessor, source_map_sha256: hash(JSON.stringify(map))};
    predecessor = restored;
  }
  assert.strictEqual(hashes.size, 17);
  for (const [kind, filter] of p.focused) {
    const run = 'r113-qualified-restored-' + kind + '.json';
    entries[run] = {kind: 'run', predecessor, source_map_sha256: hash(JSON.stringify(map)), command: [...p.test, filter], returncode: 0};
    predecessor = run;
  }
  entries['r113-qualified-collector-validation.json'] = {kind: 'run', predecessor,
    source_map_sha256: hash(JSON.stringify(map)), command: ['node', file('r113-qualification-collect.js'), '--check'], returncode: 0};
  assert.strictEqual(Object.keys(entries).length, 37);
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
function historicalNames() {
  const runs = p.mutations.map(m => 'r113-qualified-mut-' + m.name)
    .concat(p.focused.map(([kind]) => 'r113-qualified-restored-' + kind), ['r113-qualified-collector-validation']);
  const excluded = new Set(runs.flatMap(n => ['.json', '.log', '-source.json', '-source-after.json'].map(s => n + s))
    .concat(p.mutations.flatMap(m => ['.json', '-source.json'].map(s => 'r113-qualified-restoration-' + m.name + s)),
      ['r113-qualification-manifest.json']));
  assert.strictEqual(excluded.size, 115);
  return fs.readdirSync(root).filter(n => /^r113-.*\.(json|log|js|py)$/.test(n) && !excluded.has(n)).sort();
}
module.exports = {root, repo, contract, file, hash, bytes, read, git, committed, identities,
  sample, ordered, checkRecord, checkRun, passing, ignored, failed, totals, executables, checkMutation, mutatedSource,
  expectedEntries, canRestore, liveGroupMembers, historicalNames};
