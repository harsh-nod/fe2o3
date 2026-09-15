// Local gate evidence only; mutation, native and formal acceptance remain separate.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp', repo = path.join(root, 'fe2o3-r61-execution');
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
const captured = new Map();
function bytes(name) {
  const file = path.isAbsolute(name) ? name : path.join(root, name);
  assert(fs.lstatSync(file).isFile(), 'regular input: ' + file);
  if (!captured.has(file)) captured.set(file, fs.readFileSync(file));
  return captured.get(file);
}
const json = name => JSON.parse(bytes(name));
function pinned(name, digest) { const b = bytes(name); assert.strictEqual(sha(b), digest, name); return b; }
const gates = JSON.parse(pinned('r121-development-gates-v1.json', '472be0df1399a7b971998928732e49c353ba738a0dec98615d5a57f19f43f564'));
const mutations = JSON.parse(pinned('r121-development-mutations-v2.json', 'bfbda6d05aac8905934b1a9b406d3e1a4f57fe2b8d3865c0b6a0808beb7ffe81'));
for (const [name, digest] of Object.entries(mutations.parser_inputs)) pinned(name, digest);
pinned(gates.runner, gates.runner_sha256);
pinned('r121-development-check-v3.js', 'acbfa7e6af92817da758f5febc449f908ee9d4a4e41d9aa0dd6bc0d78bcc4cdf');
const E = require('./r119-integrated-qualification-evidence-v1.js');
const map = json(gates.source_map);
assert.strictEqual(sha(JSON.stringify(map)), gates.source_map_sha256);
assert.strictEqual(gates.source_map_sha256, mutations.source_map_sha256);
assert.strictEqual(Object.keys(map).length, 5696);
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), gates.source_parent);
assert.strictEqual(gates.gates.length, 25);
assert.strictEqual(gates.full_runs.length, 2);
const accepted = spec => {
  const b = git(['show', gates.source_parent + ':' + spec.accepted_log]);
  assert.strictEqual(sha(b), spec.accepted_log_sha256);
  return b;
};
for (const [name, digest] of Object.entries(gates.accepted_inputs)) {
  const b = git(['show', gates.source_parent + ':docs/evidence/local-r119-persistent-returned-data-cleanup-2026-09-15/raw/' + name]);
  assert.strictEqual(sha(b), digest);
}
function record(spec, expectedMap = map, code = 0) {
  const name = spec.run || spec.name, r = json(name + '.json');
  E.checkRecord(r, name, spec.command, expectedMap, spec.predecessor, code,
    {read: json, bytes}, {head: gates.source_parent, cwd: repo,
      contract: 'r121-development-raw-utc-boot-monotonic-v1', runner: gates.runner, after: expectedMap});
  assert(r.elapsed_seconds >= 0 && r.elapsed_seconds < r.deadline_ms / 1000);
  assert.deepStrictEqual(E.liveGroupMembers(r.process_group_cleanup.close.pgid), []);
  return bytes(name + '.log').toString();
}
const summaries = log => [...log.matchAll(/^test result: (ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;/gm)]
  .map(m => [m[1], ...m.slice(2).map(Number)]);
function roster(log) {
  E.assertPassing(log);
  const passes = E.passing(log), ignored = E.ignored(log), totals = E.totals(log);
  assert.strictEqual(passes.length, totals.passed);
  assert.strictEqual(ignored.length, totals.ignored);
  assert.strictEqual(new Set([...passes, ...ignored]).size, passes.length + ignored.length);
  return {passes, ignored, summaries: summaries(log)};
}
function groups(log, docs) {
  const re = docs ? /^   Doc-tests (.+)$/gm : /^     Running (.+)$/gm;
  const markers = [...log.matchAll(re)];
  assert(markers.length > 0);
  assert(!/^test result:/m.test(log.slice(0, markers[0].index)));
  return markers.map((m, i) => ({name: m[1].replace(/-[0-9a-f]{16}\)$/, ')'),
    ...roster(log.slice(m.index, markers[i + 1]?.index ?? log.length))}));
}
function full(spec, log) {
  const old = accepted(spec);
  E.assertPassing(old); E.assertPassing(log);
  assert.deepStrictEqual(E.totals(old), {harnesses: 48, passed: 2795, failed: 0, ignored: 5});
  assert.deepStrictEqual(E.totals(log), {harnesses: 48, passed: 2818, failed: 0, ignored: 5});
  const wanted = E.executables(old), actual = E.executables(log);
  assert.strictEqual(wanted.length, 49); assert.strictEqual(actual.length, 49);
  const kfd = wanted.filter(t => t.kind === 'libtest' && /fe2o3_kfd\)/.test(t.name));
  assert.strictEqual(kfd.length, 1);
  assert.strictEqual(mutations.new_tests.length, 23);
  assert.strictEqual(new Set(mutations.new_tests).size, 23);
  assert(mutations.new_tests.every(n => !kfd[0].passing.includes(n)));
  kfd[0].passing = [...kfd[0].passing, ...mutations.new_tests].sort();
  kfd[0].totals.passed += 23;
  assert.deepStrictEqual(actual, wanted);
  const expectedSummaries = summaries(old);
  assert.strictEqual(expectedSummaries.length, 48);
  const kfdIndex = wanted.filter(t => t.kind === 'libtest').findIndex(t => /fe2o3_kfd\)/.test(t.name));
  assert(kfdIndex >= 0);
  expectedSummaries[kfdIndex][1] += 23;
  assert.deepStrictEqual(summaries(log), expectedSummaries, 'complete full-suite summary fields');
  for (const t of actual.filter(t => t.kind === 'libtest')) {
    assert.strictEqual(t.passing.length, t.totals.passed);
    assert.strictEqual(t.ignored.length, t.totals.ignored);
    assert.strictEqual(new Set([...t.passing, ...t.ignored]).size, t.passing.length + t.ignored.length);
  }
  assert.strictEqual(kfd[0].totals.passed, 1172);
  assert.strictEqual(actual.find(t => /fe2o3_runtime\)/.test(t.name)).totals.passed, 733);
}
function fence(text, line) {
  const rows = text.split('\n'), start = line - 1;
  assert(/^\s*\/\/\/ \x60{3}/.test(rows[start]));
  let end = start + 1;
  while (end < rows.length && !/^\s*\/\/\/ \x60{3}\s*$/.test(rows[end])) end++;
  assert(end < rows.length, 'closed doc fence');
  return rows.slice(start, end + 1).join('\n');
}
function relocate(log) {
  for (const r of gates.doc_relocations) {
    const current = bytes(path.join(repo, r.path)).toString();
    assert.strictEqual(sha(current), map[r.path]);
    assert.strictEqual(fence(git(['show', gates.source_parent + ':' + r.path]), r.from), fence(current, r.to));
    const token = '(line ' + r.from + ')', names = E.passing(log).filter(n => n.startsWith(r.path + ' - ') && n.includes(token));
    if (!/^   Doc-tests fe2o3_kfd$/m.test(log)) { assert.strictEqual(names.length, 0); continue; }
    assert.strictEqual(names.length, 1);
    log = log.replace('test ' + names[0] + ' ... ok', 'test ' + names[0].replace(token, '(line ' + r.to + ')') + ' ... ok');
  }
  return log;
}
const fullGnu = gates.full_runs.find(s => s.kind === 'gnu');
const linuxNames = E.executables(accepted(fullGnu)).find(t => /fe2o3_kfd\)/.test(t.name)).passing
  .filter(n => n.startsWith('queue_linux::tests::'));
assert.strictEqual(linuxNames.length, 20);
function linux(log) {
  const lines = log.split('\n'), normalized = [];
  for (let i = 0; i < lines.length; i++) {
    const partial = /^test (.+) \.\.\. $/.exec(lines[i]);
    if (partial) {
      assert(linuxNames.includes(partial[1]), 'known split result');
      assert.strictEqual(lines[i + 1], 'running 1 test');
      assert.strictEqual(lines[i + 2], 'ok');
      normalized.push(lines[i] + 'ok', lines[i + 1]); i += 2;
    } else {
      assert(!/^\s*ok\s*$/.test(lines[i]), 'no orphan result');
      normalized.push(lines[i]);
    }
  }
  const result = normalized.join('\n');
  for (const line of normalized) {
    if (/^test /.test(line) && !/^test result:/.test(line)) {
      assert(linuxNames.some(name => line === 'test ' + name + ' ... ok'), 'exact allowlisted result row');
    }
  }
  assert.deepStrictEqual(E.passing(result), linuxNames);
  assert.strictEqual((result.match(/^running 20 tests$/gm) || []).length, 1);
  assert.strictEqual((result.match(/^running 1 test$/gm) || []).length, 3);
  return result;
}
const auxCounts = {registration: 2, ordinary: 1, 'linux-helpers': 20, initialization: 4, transitions: 30, preparation: 23, bind: 4};
const leafCounts = {'gnu-docs': [108, 0, 7], 'musl-docs': [91, 0, 5], 'musl-host-docs': [16, 0, 2],
  'gnu-host': [258, 4, 1], 'musl-host': [141, 0, 1], 'macro-fixtures': [7, 0, 1]};
function leaf(spec, log) {
  let old = accepted(spec);
  if (spec.name in leafCounts || spec.name in auxCounts) {
    const docs = spec.name.endsWith('docs');
    if (docs) old = relocate(old);
    if (spec.name === 'linux-helpers') { old = linux(old); log = linux(log); }
    const wanted = groups(old, docs), actual = groups(log, docs);
    if (spec.name in auxCounts) {
      assert.strictEqual(wanted.length, 1);
      assert.strictEqual(wanted[0].summaries.length, 1);
      wanted[0].summaries[0][5] = 1172 - auxCounts[spec.name];
    }
    assert.deepStrictEqual(actual, wanted, spec.name + ' exact target/roster/summary');
    const [passed, ignored, harnesses] = leafCounts[spec.name] || [auxCounts[spec.name], 0, 1];
    assert.deepStrictEqual(E.totals(log), {harnesses, passed, failed: 0, ignored});
    if (!docs) assert.strictEqual((log.match(/^\s*Finished \x60test\x60 profile.*$/gm) || []).length, 1);
  } else if (spec.name.startsWith('clippy-')) {
    assert.strictEqual((log.match(/^\s*Finished \x60dev\x60 profile.*$/gm) || []).length, 1);
    assert(/^\s*Finished \x60dev\x60 profile.*$/.test(log.trimEnd().split('\n').at(-1)));
    assert(!/^(?:warning|error)(?:\[|:)/m.test(log));
  } else if (['python', 'dependency-tests'].includes(spec.name)) {
    const expected = spec.name === 'python' ? 151 : 8;
    const rows = [...log.matchAll(/^Ran (\d+) tests? in [0-9.]+s$/gm)];
    assert.strictEqual(rows.length, 1); assert.strictEqual(Number(rows[0][1]), expected);
    assert.strictEqual(log.trimEnd().split('\n').at(-1), 'OK');
    assert(!/^FAILED(?: |\()|^ERROR:|^FAIL:/m.test(log));
  } else if (spec.name === 'production-metadata') {
    assert.strictEqual(bytes(spec.stderr_log).length, 0);
    const metadata = JSON.parse(log);
    assert.deepStrictEqual(metadata, JSON.parse(old));
    assert(metadata.packages.some(p => p.name === 'fe2o3-runtime'));
    assert.strictEqual(metadata.workspace_root, repo);
  } else assert.strictEqual(log, old, spec.name + ' exact deterministic log');
}
function focused(name, filter, count, previous) {
  const log = record({name, command: [...mutations.test_command, filter], predecessor: previous});
  const expected = E.passing(bytes(fullGnu.name + '.log').toString()).filter(n => n.includes(filter));
  assert.strictEqual(expected.length, count);
  assert.deepStrictEqual(E.passing(log), expected);
  assert.deepStrictEqual(E.ignored(log), []);
  E.assertPassing(log);
  assert.deepStrictEqual(summaries(log), [['ok', count, 0, 0, 0, 1172 - count]]);
}
const [mode = 'all', selected] = process.argv.slice(2);
assert(['all', 'full', 'leaf'].includes(mode));
const checked = [];
bytes(__filename);
// Interrupted GNU-v1 supplies chronology only, never a successful prerequisite.
pinned('r121-development-gnu-all-v1.json', 'cc93526a61fdc0c602ba922cf620ed67dbc0364b58b29a2dffb9089cda86898c');
const anchors = [
  {name: 'r121-development-corrected-format-v1', command: ['cargo', '+nightly-2026-04-03', 'fmt', '--all', '--', '--check'],
    predecessor: 'r121-development-gnu-all-v1.json'},
  {name: 'r121-development-corrected-cold-mode-v1', test: 'queue::dispatch_binding::control_release::tests::ordinary::ordinary_cleanup_rejects_incomplete_data_callback_and_wrong_modes',
    predecessor: 'r121-development-corrected-format-v1.json'},
  {name: 'r121-development-corrected-metadata-v1', test: 'shared_memory::tests::pristine_abort::cleanup_tests::data::typed_data_cleanup_preserves_all_five_inputs_and_refunds_once',
    predecessor: 'r121-development-corrected-cold-mode-v1.json'},
];
for (const spec of anchors) {
  if (spec.test) spec.command = [...mutations.test_command, spec.test, '--', '--exact'];
  const log = record(spec);
  if (!spec.test) assert.strictEqual(log, '');
  else {
    assert.deepStrictEqual(roster(log), {passes: [spec.test], ignored: [], summaries: [['ok', 1, 0, 0, 0, 1171]]});
    const target = groups(log, false);
    assert.strictEqual(target.length, 1);
    assert.strictEqual(target[0].name, 'unittests src/lib.rs (target/debug/deps/fe2o3_kfd)');
  }
}
const gnuLog = record(fullGnu);
const baselineOutput = cp.execFileSync('node', [path.join(root, 'r121-development-check-v3.js'), 'baseline'], {cwd: repo}).toString();
assert.strictEqual(JSON.parse(baselineOutput).baseline_passed, 2818);
full(fullGnu, gnuLog); checked.push(fullGnu.name);
if (mode !== 'full' || selected !== 'gnu') {
  // The last mutation's closed checker and restoration anchor the focused suites.
  const m = mutations.mutations.find(m => m.id.startsWith('08-'));
  const changed = {...map, [m.patch.path]: m.expected_file_sha256};
  const negativeName = 'r121-development-corrected-negative-' + m.id + '-v1';
  record({name: negativeName, command: [...mutations.test_command, m.test, '--', '--exact'], predecessor: fullGnu.name + '.json'}, changed, 101);
  const checkName = 'r121-development-corrected-negative-check-08-v3';
  const outcome = JSON.parse(record({name: checkName, command: ['node', path.join(root, 'r121-development-check-v3.js'), 'negative', m.id],
    predecessor: negativeName + '.json'}, changed));
  assert.strictEqual(outcome.id, m.id); assert.strictEqual(outcome.expected_failure_observed, true);
  assert.strictEqual(outcome.source_map_sha256, sha(JSON.stringify(changed)));
  assert.strictEqual(outcome.log_sha256, sha(bytes(negativeName + '.log')));
  const restoredName = 'r121-development-corrected-restored-08-v3';
  const restored = JSON.parse(record({name: restoredName, command: ['node', path.join(root, 'r121-development-check-v3.js'), 'restored'],
    predecessor: checkName + '.json'}));
  assert.strictEqual(restored.restored, true); assert.strictEqual(restored.source_map_sha256, gates.source_map_sha256);
  focused('r121-development-corrected-restored-controls-v1', 'queue::dispatch_binding::control_release::', 59, restoredName + '.json');
  focused('r121-development-corrected-restored-data-v1', 'shared_memory::tests::pristine_abort::', 17, 'r121-development-corrected-restored-controls-v1.json');
  const musl = gates.full_runs.find(s => s.kind === 'musl');
  full(musl, record(musl)); checked.push(musl.name);
  if (mode !== 'full') {
    assert(mode === 'all' || gates.gates.some(s => s.name === selected));
    for (const spec of gates.gates) {
      const prior = json(spec.predecessor);
      assert.strictEqual(prior.returncode, 0); assert.strictEqual(prior.child_closed, true);
      leaf(spec, record(spec)); checked.push(spec.run);
      if (mode === 'leaf' && spec.name === selected) break;
    }
  } else assert.strictEqual(selected, 'musl');
}
for (const [file, b] of captured) assert.deepStrictEqual(fs.readFileSync(file), b, 'stable consumed input: ' + file);
const names = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
  .filter(n => n && !n.startsWith('docs/')).sort();
const actualMap = Object.fromEntries(names.map(n => [n, sha(fs.readFileSync(path.join(repo, n)))]));
assert.deepStrictEqual(actualMap, map);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), gates.source_parent);
console.log(JSON.stringify({development_only: true, packet_accepted: false, checked,
  source_map_sha256: gates.source_map_sha256, artifacts: [...captured].map(([file, b]) => ({file, sha256: sha(b)}))}));

