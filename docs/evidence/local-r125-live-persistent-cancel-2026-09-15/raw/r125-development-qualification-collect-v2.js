// Unexecuted collector; every declared prerequisite must complete before use.
// This collects CPU/test evidence, not native execution or formal correspondence.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const initial = new Map();
function initialPin(name, digest) {
  assert.strictEqual(path.basename(name), name, 'flat declared helper input');
  assert(/^[a-f0-9]{64}$/.test(digest), 'qualified plan digest must be set before execution');
  assert(fs.lstatSync(root + name).isFile());
  const value = fs.readFileSync(root + name);
  assert.strictEqual(hash(value), digest, name);
  initial.set(name, value); return value;
}
const q = JSON.parse(initialPin('r125-development-qualification-plan-v2.json',
  '2d165da7c976a5735d18520d413de786d5d543b8ab08a3d8f923a787101dc7af'));
for (const [name, digest] of Object.entries(q.inputs)) initialPin(name, digest);
initialPin(q.runner.name, q.runner.sha256);
const M = require(root + q.mutation_checker), C = M.C, G = require(root + q.gate_checker);
assert.strictEqual(G.C, C);
assert.strictEqual(require(root + q.full_checker), C);
const {p, E, repo, map, bytes, json} = C;
for (const [name, value] of initial) assert.deepStrictEqual(bytes(name), value);
bytes(__filename);
assert.strictEqual(path.basename(__filename), q.collector);
assert.strictEqual(q.accepted, false);
assert.strictEqual(q.source_parent, p.source_parent);
assert.strictEqual(q.source_map_sha256, p.source_map_sha256);
assert.strictEqual(q.source_identities, p.source_count);
assert.strictEqual(q.source_identities, 5709);
assert.deepStrictEqual(q.runner, {name: p.runner, sha256: p.runner_sha256});
assert.deepStrictEqual(q.superseded_attempts, p.superseded_attempts);
assert.deepStrictEqual(C.identities(), map);
const runs = [], seenRuns = new Set();
function record(spec, expected = map, code = 0) {
  const name = spec.run || spec.name;
  assert(!seenRuns.has(name), 'one collected record per run'); seenRuns.add(name);
  const result = C.completed(name, spec.command, spec.predecessor,
    expected, code, spec.runner || p.runner);
  runs.push({name, returncode: code, source_map_sha256: result.record.source_map_sha256,
    record_sha256: hash(bytes(name + '.json')), log_sha256: hash(result.log)});
  return result;
}
function replay(spec, expected) {
  assert.strictEqual(spec.command[0], 'node');
  const result = cp.execFileSync(spec.command[0], spec.command.slice(1),
    {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
  assert.deepStrictEqual(JSON.parse(result), expected, 'exact replay: ' + spec.name);
}
function artifacts(report) {
  assert(Array.isArray(report.artifacts) && report.artifacts.length > 0);
  assert.strictEqual(new Set(report.artifacts.map(item => item.file)).size, report.artifacts.length);
  for (const item of report.artifacts) {
    assert.deepStrictEqual(Object.keys(item).sort(), ['file', 'sha256']);
    const flat = path.dirname(item.file) === root.slice(0, -1) &&
      (path.basename(item.file).startsWith('r125-development-') ||
        Object.hasOwn(q.inputs, path.basename(item.file)));
    const relocated = p.doc_relocations.some(item2 => item.file === path.join(repo, item2.path));
    assert(flat || relocated, 'declared checker input path');
    assert.strictEqual(hash(bytes(item.file)), item.sha256);
  }
}
assert.deepStrictEqual(q.diagnostic_audit.command,
  ['node', root + 'r125-development-zero-scan-audit-v1.js']);
assert.strictEqual(q.diagnostic_audit.helper_sha256,
  '16d28a983e23fa35e825e48c8f6313ac3f9f9ffa9931f3cfc5c86a396954f631');
C.pin('r125-development-zero-scan-audit-v1.js', q.diagnostic_audit.helper_sha256);
const diagnostic = record(q.diagnostic_audit), diagnosticReport = JSON.parse(diagnostic.log);
assert.strictEqual(diagnosticReport.source_map_sha256, p.source_map_sha256);
assert.strictEqual(diagnosticReport.source_delta_files, 2);
assert.strictEqual(diagnosticReport.runs.length, 6);
assert.strictEqual(p.full_runs.find(spec => spec.kind === 'gnu').predecessor,
  diagnosticReport.runs.at(-1).name + '.json');
replay(q.diagnostic_audit, diagnosticReport);
const bundle = json(q.source_bundle);
assert.strictEqual(bundle.development_only, true);
assert.strictEqual(bundle.source_parent, p.source_parent);
assert.strictEqual(bundle.source_map_sha256, p.source_map_sha256);
assert.strictEqual(bundle.source_identities, p.source_count);
const baselinePath = 'docs/evidence/local-r124-live-recycled-detach-2026-09-15/raw/r124-development-gnu-all-v1-source.json';
const baselineBytes = C.git(['show', p.source_parent + ':' + baselinePath]);
assert.strictEqual(hash(baselineBytes), 'dcb514f96f92c948914e640035f853ad02f2663109963aaf8135fae4a3247f2e');
const baselineMap = JSON.parse(baselineBytes);
assert.strictEqual(hash(JSON.stringify(baselineMap)), 'a633edf685c4cbe3a24b1500a6a086ba36d39bec9b042abb5f88c52e4024cc24');
assert.deepStrictEqual(bundle.accepted_baseline, {commit: p.source_parent, path: baselinePath,
  source_map_sha256: hash(JSON.stringify(baselineMap))});
assert.strictEqual(Object.keys(baselineMap).length, 5706);
assert(Object.keys(baselineMap).every(file => Object.hasOwn(map, file)));
const delta = Object.keys(map).filter(file => baselineMap[file] !== map[file]);
assert.deepStrictEqual(bundle.files.map(file => file.path), delta);
assert.strictEqual(q.source_changed, 18); assert.strictEqual(q.source_added, 3);
assert.strictEqual(delta.length, q.source_changed);
assert.strictEqual(bundle.files.filter(file => file.previous_sha256 === null).length, q.source_added);
for (const file of bundle.files) {
  assert(file.path.startsWith('crates/') && file.path.endsWith('.rs'));
  assert.strictEqual(file.previous_sha256, baselineMap[file.path] || null);
  assert.strictEqual(file.sha256, map[file.path]);
  assert.strictEqual(hash(file.text), file.sha256);
  assert.strictEqual(bytes(path.join(repo, file.path)).toString(), file.text);
  if (file.previous_sha256)
    assert.strictEqual(hash(C.git(['show', p.source_parent + ':' + file.path])), file.previous_sha256);
}
const sourcePlan = record(q.source_plan_run);
assert.deepStrictEqual(JSON.parse(sourcePlan.log), {output: root + 'r125-development-mutations-v3.json',
  sha256: hash(bytes('r125-development-mutations-v3.json')), accepted: false,
  mutations_declared: 18, runtime_mutations_executed: 0});
const snapshot = record(q.source_snapshot);
assert.deepStrictEqual(JSON.parse(snapshot.log), {source_snapshot_only: true,
  output: root + q.source_bundle, files: 18, new_files: 3,
  sha256: hash(bytes(q.source_bundle)), source_map_sha256: p.source_map_sha256});
// Exclusive-write producers are not replayed. Their immutable outputs are reconstructed.
let calibrationCount = 0;
assert.deepStrictEqual(q.calibrations.map(spec => [spec.accept, spec.reject]), [[5, 52], [10, 39], [38, 63]]);
for (const spec of q.calibrations) {
  const result = record(spec), report = JSON.parse(result.log);
  assert.strictEqual(new Set(spec.cases.map(item => item.name)).size, spec.cases.length);
  assert.strictEqual(spec.cases.filter(item => item.expected === 'accept').length, spec.accept);
  assert.strictEqual(spec.cases.filter(item => item.expected === 'reject').length, spec.reject);
  assert.strictEqual(spec.cases.length, spec.accept + spec.reject);
  assert.deepStrictEqual(report, {development_only: true, helper_calibration_only: true,
    runtime_mutation_count: 0, helper_sha256: spec.helper_sha256,
    passed: spec.cases.length, cases: spec.cases});
  replay(spec, report); calibrationCount += report.passed;
}
assert.strictEqual(q.helper_calibrations, 207);
assert.strictEqual(calibrationCount, q.helper_calibrations);
const gnu = p.full_runs.find(spec => spec.kind === 'gnu');
const musl = p.full_runs.find(spec => spec.kind === 'musl');
assert.strictEqual(p.full_runs.length, 2);
const gnuRun = record(gnu); G.full(gnu, gnuRun.log);
E.ordered(diagnostic.record.clock.source_verified, gnuRun.record.clock.start);
assert.strictEqual(gnuRun.record.deadline_ms, gnu.deadline_ms);
assert.strictEqual(q.gnu_check.predecessor, gnu.name + '.json');
assert.deepStrictEqual(q.gnu_check.command, ['node', root + q.full_checker, 'baseline', 'gnu']);
const gnuCheck = record(q.gnu_check), gnuReport = JSON.parse(gnuCheck.log);
assert.deepStrictEqual(gnuReport, {development_only: true, packet_accepted: false,
  mode: 'baseline', kind: 'gnu', full_passed: p.counts.full,
  source_map_sha256: p.source_map_sha256, artifacts: gnuReport.artifacts});
artifacts(gnuReport); replay(q.gnu_check, gnuReport);
assert.strictEqual(musl.predecessor, q.gnu_check.name + '.json');
const muslRun = record(musl); G.full(musl, muslRun.log);
assert.strictEqual(muslRun.record.deadline_ms, musl.deadline_ms);
assert.strictEqual(q.full_check.predecessor, musl.name + '.json');
assert.deepStrictEqual(q.full_check.command, ['node', root + q.gate_checker, 'full', 'musl']);
const fullCheck = record(q.full_check), fullReport = JSON.parse(fullCheck.log);
assert.deepStrictEqual(fullReport, {development_only: true, packet_accepted: false,
  checked: [gnu.name, musl.name], source_map_sha256: p.source_map_sha256,
  artifacts: fullReport.artifacts});
artifacts(fullReport); replay(q.full_check, fullReport);
assert.strictEqual(p.gates.length, 25);
let previousGate = fullCheck.record;
for (const [index, spec] of p.gates.entries()) {
  assert.strictEqual(spec.predecessor, (index ? p.gates[index - 1].run : q.full_check.name) + '.json');
  const result = record(spec);
  E.ordered(previousGate.clock.source_verified, result.record.clock.start);
  G.leaf(spec, result.log); previousGate = result.record;
}
assert.strictEqual(q.gate_check.predecessor, p.gates.at(-1).run + '.json');
assert.deepStrictEqual(q.gate_check.command, ['node', root + q.gate_checker, 'all']);
const gateCheck = record(q.gate_check), gateReport = JSON.parse(gateCheck.log);
const gateNames = [...p.full_runs.map(spec => spec.name), ...p.gates.map(spec => spec.run)];
assert.strictEqual(new Set(gateNames).size, 27);
assert.deepStrictEqual(gateReport, {development_only: true, packet_accepted: false,
  checked: gateNames, source_map_sha256: p.source_map_sha256, artifacts: gateReport.artifacts});
artifacts(gateReport); replay(q.gate_check, gateReport);
assert.deepStrictEqual(q.mutations.map(item => item.id), M.p.mutations.map(item => item.id));
assert.strictEqual(q.mutations.length, 18);
const mutations = [], distinctMaps = new Set();
let previousRestoration = gateCheck.record;
for (const [index, declaration] of q.mutations.entries()) {
  const mutation = M.p.mutations[index], original = bytes(path.join(repo, mutation.patch.path)).toString();
  assert.strictEqual(hash(original), map[mutation.patch.path]);
  M.assertMutationSource(mutation, E.core.mutatedSource(original, mutation.patch));
  const changed = {...map, [mutation.patch.path]: mutation.expected_file_sha256};
  const changedHash = hash(JSON.stringify(changed));
  assert.strictEqual(changedHash, mutation.expected_source_map_sha256); distinctMaps.add(changedHash);
  assert.strictEqual(declaration.predecessor, mutation.predecessor);
  assert.strictEqual(declaration.predecessor,
    (index ? q.mutations[index - 1].restored : q.gate_check.name) + '.json');
  assert.strictEqual(declaration.negative, 'r125-development-negative-' + mutation.id + '-v1');
  assert.strictEqual(declaration.check, 'r125-development-negative-check-' + mutation.id + '-v1');
  assert.strictEqual(declaration.restored, 'r125-development-restored-' + mutation.id + '-v1');
  const negative = record({name: declaration.negative,
    command: [...M.p.test_command, mutation.test, '--', '--exact'],
    predecessor: declaration.predecessor}, changed, 101);
  E.ordered(previousRestoration.clock.source_verified, negative.record.clock.start);
  const panic = M.assertNegative(negative.log, mutation);
  const checked = record({name: declaration.check,
    command: ['node', root + q.mutation_checker, 'negative', mutation.id],
    predecessor: declaration.negative + '.json'}, changed);
  assert.deepStrictEqual(JSON.parse(checked.log), {development_only: true, packet_accepted: false,
    id: mutation.id, quiescent: true, expected_failure_observed: true,
    source_map_sha256: changedHash, log_sha256: hash(negative.log), decisive_panic: panic});
  const restored = record({name: declaration.restored,
    command: ['node', root + q.mutation_checker, 'restored'],
    predecessor: declaration.check + '.json'});
  assert.deepStrictEqual(JSON.parse(restored.log), {development_only: true, restored: true,
    process_closure_checked: false, source_map_sha256: p.source_map_sha256});
  previousRestoration = restored.record;
  mutations.push({id: mutation.id, test: mutation.test, source_map_sha256: changedHash, decisive_panic: panic});
}
assert.strictEqual(distinctMaps.size, 18);
assert.deepStrictEqual(q.restored.map(spec => [spec.kind, spec.passed, spec.filtered]),
  [['kfd', 47, 1197], ['runtime', 744, 0]]);
const baselineTargets = E.executables(C.accepted(gnu));
for (const [index, spec] of q.restored.entries()) {
  const original = baselineTargets.filter(target => target.kind === 'libtest' &&
    target.name.endsWith('/fe2o3_' + spec.kind + ')'));
  assert.strictEqual(original.length, 1);
  const all = [...original[0].passing, ...p.new_tests[spec.kind]].sort();
  assert.strictEqual(all.length, p.counts[spec.kind]);
  assert.strictEqual(new Set(all).size, all.length);
  const filters = ['persistent_cancel', 'persistent_allocation::tests',
    'queue::dispatch_binding::control_release::tests::persistent::', ...M.p.preserved_tests,
    'queue::live::construction_primary::integration_tests::platform::tests::context_zero_check_preserves_every_byte_including_unaligned_partial_pages'];
  const tests = spec.kind === 'kfd' ? all.filter(name => filters.some(filter => name.includes(filter))) : all;
  const command = spec.kind === 'kfd' ? [...M.p.test_command, '--', ...filters] :
    ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-runtime', '--all-features', '--lib'];
  assert.deepStrictEqual(spec.command, command);
  assert.deepStrictEqual(spec.tests, tests);
  assert.strictEqual(spec.target, original[0].name);
  assert.strictEqual(spec.predecessor,
    (index ? q.restored[index - 1].name : q.mutations.at(-1).restored) + '.json');
  const result = record(spec);
  E.ordered(previousRestoration.clock.source_verified, result.record.clock.start);
  assert.deepStrictEqual(G.groups(result.log, false), [{name: spec.target, passes: tests, ignored: [],
    summaries: [['ok', spec.passed, 0, 0, 0, spec.filtered]]}]);
  assert.strictEqual((result.log.match(/^\s*Finished \x60test\x60 profile.*$/gm) || []).length, 1);
  assert.deepStrictEqual([...result.log.matchAll(/^running (\d+) tests$/gm)].map(match => Number(match[1])), [spec.passed]);
  assert(!/^   Doc-tests /m.test(result.log));
  previousRestoration = result.record;
}
assert.strictEqual(q.collector_predecessor, q.restored.at(-1).name + '.json');
const ownOutputs = new Set(['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json']
  .map(suffix => q.collector_run + suffix));
const inventoryNames = () => fs.readdirSync(root)
  .filter(name => /^r125-development-.*\.(?:json|log|js|md|rs)$/.test(name) && !ownOutputs.has(name)).sort();
const inventory = inventoryNames();
for (const name of inventory) bytes(name);
const historicalRuns = [], groups = new Map();
for (const name of inventory.filter(name => name.endsWith('.json'))) {
  const value = json(name);
  if (!Array.isArray(value.command) || !value.source_head || !value.process_group_cleanup) continue;
  assert.strictEqual(value.child_closed, true, 'retained run has a closed child');
  assert(['absent', 'signaled'].includes(value.process_group_cleanup.close.status));
  assert.deepStrictEqual(value.process_group_cleanup.close.live_members, []);
  const pgid = value.process_group_cleanup.close.pgid;
  assert(Number.isSafeInteger(pgid) && pgid > 1);
  const closure = C.observeClosure(name.slice(0, -5), value);
  groups.set(closure.boot_id + ':' + pgid, closure);
  if (!seenRuns.has(name.slice(0, -5))) historicalRuns.push({name: name.slice(0, -5),
    returncode: value.returncode, source_map_sha256: value.source_map_sha256, qualification: false});
}
assert.deepStrictEqual(C.identities(), map); C.stable();
assert.deepStrictEqual(inventoryNames(), inventory, 'stable evidence namespace');
const raw = C.artifacts().filter(item => path.dirname(item.file) === root.slice(0, -1))
  .map(item => ({name: path.basename(item.file), sha256: item.sha256, bytes: bytes(item.file).length}))
  .sort((a, b) => a.name.localeCompare(b.name));
const qualified = new Set(runs.flatMap(run =>
  ['.json', '.log', '-source.json', '-source-after.json'].map(suffix => run.name + suffix)));
console.log(JSON.stringify({development_only: true, publication_pending: true, scope: q.scope,
  source_parent: p.source_parent, source_map_sha256: p.source_map_sha256, source_identities: p.source_count,
  source_bundle: {name: q.source_bundle, sha256: hash(bytes(q.source_bundle)), changed: 18, added: 3},
  gnu_passed: p.counts.full, musl_passed: p.counts.full, ignored_each: p.counts.ignored,
  libtest_targets_each: 48, csv_targets_each: 1, source_gates: 17, auxiliary_checks: 10,
  compiled_runtime_negatives: 18, distinct_mutant_maps: 18, helper_calibrations: calibrationCount,
  execution_boot: p.admitted_boot, restored_suites: q.restored.map(spec => spec.passed),
  runs, mutations, raw_artifacts: raw,
  retained_other_artifacts: inventory.filter(name => !qualified.has(name)), historical_runs: historicalRuns,
  owned_group_observations: [...groups.values()].sort((a, b) => a.pgid - b.pgid),
  exclusions: q.exclusions}));
