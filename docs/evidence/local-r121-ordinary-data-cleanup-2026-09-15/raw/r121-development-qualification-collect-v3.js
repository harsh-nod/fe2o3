// Read-only collection of completed R121 CPU/test evidence, not native/formal acceptance.
const fs = require('fs'), cp = require('child_process'), path = require('path');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/', repo = root + 'fe2o3-r61-execution';
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
const captured = new Map(), runs = [];
function bytes(name) {
  const file = path.isAbsolute(name) ? name : root + name;
  assert(file.startsWith(root), 'owned artifact/source path');
  assert(fs.lstatSync(file).isFile(), 'regular artifact');
  if (!captured.has(file)) captured.set(file, fs.readFileSync(file));
  return captured.get(file);
}
const json = name => JSON.parse(bytes(name));
function pin(name, sha) { const b = bytes(name); assert.strictEqual(hash(b), sha, name); return JSON.parse(b); }
const p = pin('r121-development-mutations-v2.json', 'bfbda6d05aac8905934b1a9b406d3e1a4f57fe2b8d3865c0b6a0808beb7ffe81');
const g = pin('r121-development-gates-v1.json', '472be0df1399a7b971998928732e49c353ba738a0dec98615d5a57f19f43f564');
const gi = pin('r121-development-gate-check-inputs-v1.json', '351300d2feb094fa4ed6c8e758944c538a46d1eba173b515745f7712b8c61b12');
const ci = pin('r121-development-gate-parser-inputs-v1.json', '5644878ef65d0a07c9678c77f1d19f845192f411791ffdfd074b696aa641dffe');
for (const [name, sha] of Object.entries(p.parser_inputs)) assert.strictEqual(hash(bytes(name)), sha);
for (const item of [{name: p.runner, sha256: p.runner_sha256}, gi.checker, ci.harness]) assert.strictEqual(hash(bytes(item.name)), item.sha256);
assert.strictEqual(hash(bytes('r121-development-check-v3.js')), 'acbfa7e6af92817da758f5febc449f908ee9d4a4e41d9aa0dd6bc0d78bcc4cdf');
bytes(__filename);
const E = require('./r119-integrated-qualification-evidence-v1.js');
const map = json(p.source_map), context = {head: p.source_parent, cwd: repo,
  contract: 'r121-development-raw-utc-boot-monotonic-v1', runner: p.runner, after: map};
assert.strictEqual(hash(JSON.stringify(map)), p.source_map_sha256);
assert.strictEqual(g.source_map_sha256, p.source_map_sha256);
assert.strictEqual(Object.keys(map).length, 5696);
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
function identities() {
  assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), p.source_parent);
  return Object.fromEntries([...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
    .filter(n => n && !n.startsWith('docs/')).sort().map(n => [n, hash(fs.readFileSync(path.join(repo, n)))]));
}
assert.deepStrictEqual(identities(), map);
function record(name, command, previous, expected = map, code = 0) {
  const r = json(name + '.json');
  E.checkRecord(r, name, command, expected, previous, code, {read: json, bytes}, {...context, after: expected});
  assert(r.elapsed_seconds >= 0 && r.elapsed_seconds < r.deadline_ms / 1000);
  assert.deepStrictEqual(E.liveGroupMembers(r.process_group_cleanup.close.pgid), []);
  const log = bytes(name + '.log').toString();
  runs.push({name, returncode: code, source_map_sha256: r.source_map_sha256,
    record_sha256: hash(bytes(name + '.json')), log_sha256: hash(log)});
  return {r, log};
}
const completeGates = [...g.full_runs.map(s => s.name), ...g.gates.map(s => s.run)];
assert.strictEqual(completeGates.length, 27);
assert.strictEqual(new Set(completeGates).size, 27);
const gateName = 'r121-development-gate-check-all-v2';
const gate = record(gateName, ['node', root + gi.checker.name, 'all'], g.gates.at(-1).run + '.json');
const report = JSON.parse(gate.log);
assert.deepStrictEqual(report.checked, completeGates);
assert.strictEqual(report.development_only, true);
assert.strictEqual(report.packet_accepted, false);
assert.strictEqual(report.source_map_sha256, p.source_map_sha256);
assert.strictEqual(new Set(report.artifacts.map(a => a.file)).size, report.artifacts.length);
for (const a of report.artifacts) {
  const ownedFlat = path.dirname(a.file) === root.slice(0, -1)
    && (path.basename(a.file).startsWith('r121-development-') || Object.hasOwn(p.parser_inputs, path.basename(a.file)));
  const docSource = g.doc_relocations.some(d => a.file === path.join(repo, d.path));
  assert(ownedFlat || docSource, 'declared gate input path');
  assert.strictEqual(hash(bytes(a.file)), a.sha256, 'closed gate consumed input');
}
for (const s of [...g.full_runs, ...g.gates]) record(s.run || s.name, s.command, s.predecessor);
const cal = record(ci.run, ci.command, ci.predecessor);
const calibration = JSON.parse(cal.log);
assert.strictEqual(calibration.helper_calibration_only, true);
assert.strictEqual(calibration.runtime_mutation_count, 0);
assert.strictEqual(calibration.helper_sha256, gi.checker.sha256);
assert.strictEqual(calibration.passed, 20);
assert.strictEqual(calibration.cases.length, 20);
assert.strictEqual(new Set(calibration.cases.map(c => c.name)).size, 20);
assert.strictEqual(calibration.cases.filter(c => c.expected === 'accept').length, 7);
assert.strictEqual(calibration.cases.filter(c => c.expected === 'reject').length, 13);
const expectedCalibration = [
  ['actual GNU target and complete summary roster', 'accept'],
  ['GNU measured count', 'reject'],
  ['GNU filtered count', 'reject'],
  ['GNU executable identity', 'reject'],
  ['GNU unknown successful row', 'reject'],
  ['accepted Linux split normalization', 'accept'],
  ['Linux malformed extra row', 'reject'],
  ['Linux unknown partial row', 'reject'],
  ['Linux whitespace orphan', 'reject'],
  ['Linux intervening named result', 'reject'],
  ['Linux synthetic corrected-cohort filtered count', 'accept'],
  ['Linux stale filtered count', 'reject'],
  ['GNU exact relocated doctest roster', 'accept'],
  ['GNU stale doctest location', 'reject'],
  ['Python declared test count', 'accept'],
  ['Python wrong test count', 'reject'],
  ['strict Clippy completion', 'accept'],
  ['Clippy warning', 'reject'],
  ['empty formatting transcript', 'accept'],
  ['nonempty formatting transcript', 'reject'],
].map(([name, expected]) => ({name, expected}));
assert.deepStrictEqual(calibration.cases, expectedCalibration, 'exact ordered calibration cases');
// Reviewed source locations distinguish the intended assertion from incidental panics.
const ordinaryTest = 'crates/fe2o3-kfd/src/queue_dispatch_binding/control_release/ordinary_tests.rs';
const dataTest = 'crates/fe2o3-kfd/src/shared_memory/tests/pristine_abort/data_tests.rs';
for (const file of [ordinaryTest, dataTest]) assert.strictEqual(hash(bytes(path.join(repo, file))), map[file]);
const panicLocations = Object.fromEntries([
  ['01', ordinaryTest, 100, 10], ['02', ordinaryTest, 85, 5],
  ['03', dataTest, 603, 21], ['04', dataTest, 626, 21],
  ['05', ordinaryTest, 75, 23], ['06', ordinaryTest, 160, 6],
  ['07', dataTest, 478, 10], ['08', dataTest, 205, 17],
  ['09', ordinaryTest, 642, 5], ['10', dataTest, 442, 13],
  ['11', ordinaryTest, 75, 23], ['12', dataTest, 816, 9],
  ['13', dataTest, 408, 5], ['14', ordinaryTest, 47, 5],
  ['15', dataTest, 797, 9], ['16', dataTest, 797, 9],
  ['17', dataTest, 653, 56], ['18', dataTest, 745, 52],
  ['19', dataTest, 368, 9], ['20', ordinaryTest, 37, 5],
  ['21', ordinaryTest, 318, 53], ['22', ordinaryTest, 75, 23],
  ['23', ordinaryTest, 77, 15], ['24', ordinaryTest, 77, 15],
  ['25', dataTest, 821, 29], ['26', dataTest, 838, 37],
].map(([id, file, line, column]) => [id, file + ':' + line + ':' + column]));
assert.deepStrictEqual(Object.keys(panicLocations).sort(), p.mutations.map(m => m.id.slice(0, 2)).sort());
const execution = [...p.mutations.slice(8), ...p.mutations.slice(0, 8)];
assert.strictEqual(execution.length, 26);
assert.strictEqual(new Set(execution.map(m => m.id)).size, 26);
const mutationResults = [], distinct = new Set();
let previousRestoration = null;
for (const m of execution) {
  assert(p.new_tests.includes(m.test) && !p.source_guards.includes(m.test));
  const source = bytes(path.join(repo, m.patch.path)).toString();
  assert.strictEqual(hash(source), map[m.patch.path]);
  assert.strictEqual(hash(E.core.mutatedSource(source, m.patch)), m.expected_file_sha256);
  const changed = {...map, [m.patch.path]: m.expected_file_sha256};
  const changedHash = hash(JSON.stringify(changed));
  distinct.add(changedHash);
  const name = 'r121-development-corrected-negative-' + m.id + '-v1', n = m.id.slice(0, 2);
  const negative = record(name, [...p.test_command, m.test, '--', '--exact'], p.baseline.name + '.json', changed, 101);
  if (previousRestoration) E.ordered(previousRestoration.clock.source_verified, negative.r.clock.start);
  assert(!/panic in a destructor during cleanup|panic in a function that cannot unwind|thread caused non-unwinding panic|SIGABRT|signal: 6\b|^Aborted(?: \(core dumped\))?\s*$|fatal runtime error:|has overflowed its stack|memory allocation of \d+ bytes failed|could not compile/m.test(negative.log),
    'no aborted test subprocess or compile failure');
  const blocks = negative.log.split('---- ' + m.test + ' stdout ----');
  assert.strictEqual(blocks.length, 2);
  const body = blocks[1].split('\nfailures:\n')[0], panic = body.slice(body.lastIndexOf("thread '")).trim();
  const header = /^thread '([^'\n]+)' \(([1-9][0-9]*)\) panicked at ([^\n]+):\n/.exec(panic);
  assert(header, 'complete final panic header');
  assert.strictEqual(header[1], m.test, 'selected test owns the final assertion');
  assert.strictEqual(header[3], panicLocations[n], 'independently reviewed behavioral source location');
  assert(m.oracle.every(fragment => panic.includes(fragment)), 'intended final panic');
  assert.deepStrictEqual(E.failed(negative.log), [m.test]);
  assert.deepStrictEqual(E.passing(negative.log), []);
  assert.deepStrictEqual(E.totals(negative.log), {harnesses: 1, passed: 0, failed: 1, ignored: 0});
  for (const marker of ['Finished \x60test\x60 profile', 'running 1 test\n',
    'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1171 filtered out;']) assert(negative.log.includes(marker));
  assert(!/^error\[E\d+\]:/m.test(negative.log));
  const checkedName = 'r121-development-corrected-negative-check-' + n + '-v3';
  const checked = record(checkedName, ['node', root + 'r121-development-check-v3.js', 'negative', m.id], name + '.json', changed);
  assert.deepStrictEqual(JSON.parse(checked.log), {development_only: true, id: m.id, quiescent: true,
    expected_failure_observed: true, source_map_sha256: changedHash, log_sha256: hash(negative.log), decisive_panic: panic});
  const restoredName = 'r121-development-corrected-restored-' + n + '-v3';
  const restored = record(restoredName, ['node', root + 'r121-development-check-v3.js', 'restored'], checkedName + '.json');
  assert.deepStrictEqual(JSON.parse(restored.log), {development_only: true, restored: true, process_closure_checked: false,
    source_map_sha256: p.source_map_sha256, identities: 5696});
  previousRestoration = restored.r;
  mutationResults.push({id: m.id, test: m.test, source_map_sha256: changedHash, decisive_panic: panic});
}
assert.strictEqual(distinct.size, 25);
const repeated = mutationResults.filter(m => mutationResults.some(other => other.id !== m.id && other.source_map_sha256 === m.source_map_sha256));
assert.deepStrictEqual(repeated.map(m => m.id), ['20-ordinary-output-allocation', '21-ordinary-premise-policy']);
const controls = 'r121-development-corrected-restored-controls-v1';
const data = 'r121-development-corrected-restored-data-v1';
record(controls, [...p.test_command, 'queue::dispatch_binding::control_release::'], 'r121-development-corrected-restored-08-v3.json');
record(data, [...p.test_command, 'shared_memory::tests::pristine_abort::'], controls + '.json');
// Preserve history as bytes; it does not supply accepted current-cohort results.
const ownRun = 'r121-development-qualification-collect-v3';
const ownOutputs = new Set(['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json'].map(s => ownRun + s));
const inventoryNames = () => fs.readdirSync(root)
  .filter(n => /^r121-development-.*\.(?:json|log|js|md|rs)$/.test(n) && !ownOutputs.has(n)).sort();
const inventory = inventoryNames();
for (const name of inventory) bytes(name);
// Reparse the captured gate inputs rather than accepting a success-shaped report.
const replay = JSON.parse(cp.execFileSync('node', [root + gi.checker.name, 'all'],
  {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString());
assert.deepStrictEqual(replay, report, 'exact independently replayed gate report');
assert.deepStrictEqual(identities(), map);
for (const [file, b] of captured) assert.deepStrictEqual(fs.readFileSync(file), b, 'stable consumed bytes');
assert.deepStrictEqual(inventoryNames(), inventory, 'stable retained-artifact namespace');
const raw = [...captured].filter(([file]) => path.dirname(file) === root.slice(0, -1))
  .map(([file, b]) => ({name: path.basename(file), sha256: hash(b), bytes: b.length})).sort((a, b) => a.name.localeCompare(b.name));
const qualifiedNames = new Set(runs.flatMap(r => ['.json', '.log', '-source.json', '-source-after.json'].map(s => r.name + s)));
console.log(JSON.stringify({development_only: true, publication_pending: true, scope: 'R121 local CPU/test qualification only',
  source_parent: p.source_parent, source_map_sha256: p.source_map_sha256, source_identities: 5696,
  gnu_passed: 2818, musl_passed: 2818, ignored_each: 5, source_gates: 17, auxiliary_checks: 10,
  compiled_runtime_negatives: 26, distinct_mutant_maps: 25, helper_calibrations: 20, restored_suites: [59, 17],
  runs, mutations: mutationResults, raw_artifacts: raw,
  retained_other_artifacts: inventory.filter(n => !qualifiedNames.has(n)),
  exclusions: ['native execution', 'authenticated formal refinement', 'continuous source immutability',
    'aggregate retained-memory bounds', 'multi-device acceptance', 'HIP/HSA performance parity'],
}));


