// Read-only R124 CPU/test qualification. Native execution and formal correspondence remain separate.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/', hash = value => crypto.createHash('sha256').update(value).digest('hex');
const initial = new Map();
function initialPin(name, digest) {
  assert(fs.lstatSync(root + name).isFile());
  const value = fs.readFileSync(root + name);
  assert.strictEqual(hash(value), digest, name); initial.set(name, value); return value;
}
const q = JSON.parse(initialPin('r124-development-qualification-plan-v1.json',
  '0efd2e3d6b61d33541ad65cc3cf95a215889ee487fdd404f4a6842ed292b54ad'));
for (const [name, digest] of Object.entries(q.inputs)) initialPin(name, digest);
initialPin(q.runner.name, q.runner.sha256);
const M = require(root + q.mutation_checker), C = M.C, G = require(root + q.gate_checker);
assert.strictEqual(G.C, C);
assert.strictEqual(require(root + q.full_checker), C);
const {p, E, repo, map, bytes, json} = C;
for (const [name, value] of initial) assert.deepStrictEqual(bytes(name), value);
bytes(__filename);
assert.strictEqual(q.accepted, false);
assert.strictEqual(q.source_parent, p.source_parent);
assert.strictEqual(q.source_map_sha256, p.source_map_sha256);
assert.strictEqual(q.source_identities, 5706);
assert.deepStrictEqual(C.identities(), map);
const runs = [], seenRuns = new Set();
function record(spec, expected = map, code = 0) {
  const name = spec.run || spec.name;
  assert(!seenRuns.has(name), 'one collected record per run'); seenRuns.add(name);
  const result = C.completed(name, spec.command, spec.predecessor, expected, code, spec.runner || p.runner);
  runs.push({name, returncode: code, source_map_sha256: result.record.source_map_sha256,
    record_sha256: hash(bytes(name + '.json')), log_sha256: hash(result.log)});
  return result;
}
function replay(spec, expected) {
  assert.strictEqual(spec.command[0], 'node');
  const result = cp.execFileSync(spec.command[0], spec.command.slice(1), {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
  assert.deepStrictEqual(JSON.parse(result), expected, 'exact replay: ' + spec.name);
}
const bundle = json(q.source_bundle);
assert.strictEqual(bundle.development_only, true);
assert.strictEqual(bundle.source_parent, p.source_parent);
assert.strictEqual(bundle.source_map_sha256, p.source_map_sha256);
assert.strictEqual(bundle.source_identities, p.source_count);
assert.strictEqual(bundle.accepted_baseline.commit, p.source_parent);
assert.strictEqual(bundle.accepted_baseline.path,
  'docs/evidence/local-r123-live-retained-control-release-2026-09-15/raw/r123-development-focused-05-source.json');
const baselineMap = JSON.parse(C.git(['show', p.source_parent + ':' + bundle.accepted_baseline.path]));
assert.strictEqual(hash(JSON.stringify(baselineMap)), '8380746325d6717df78a608eb5be8f3ff3c48e42f32eec19aba2ab0677fdcb08');
assert.strictEqual(bundle.accepted_baseline.source_map_sha256, hash(JSON.stringify(baselineMap)));
assert.strictEqual(Object.keys(baselineMap).length, 5703);
assert(Object.keys(baselineMap).every(file => Object.hasOwn(map, file)));
const delta = Object.keys(map).filter(file => baselineMap[file] !== map[file]);
assert.deepStrictEqual(bundle.files.map(file => file.path), delta);
assert.strictEqual(delta.length, 12);
assert.strictEqual(bundle.files.filter(file => file.previous_sha256 === null).length, 3);
for (const file of bundle.files) {
  assert.strictEqual(file.previous_sha256, baselineMap[file.path] || null);
  assert.strictEqual(file.sha256, map[file.path]);
  assert.strictEqual(hash(file.text), file.sha256);
  assert.strictEqual(bytes(path.join(repo, file.path)).toString(), file.text);
  if (file.previous_sha256) assert.strictEqual(hash(C.git(['show', p.source_parent + ':' + file.path])), file.previous_sha256);
}
C.baseline();
const gnuSpec = p.full_runs.find(spec => spec.kind === 'gnu');
for (const spec of p.anchors) record(spec);
let calibrationCount = 0;
function calibration(spec, result) {
  const report = JSON.parse(result.log);
  assert.deepStrictEqual(report, {development_only: true, helper_calibration_only: true, runtime_mutation_count: 0,
    helper_sha256: spec.helper_sha256, passed: spec.cases.length, cases: spec.cases}, 'exact calibration report');
  assert.strictEqual(new Set(report.cases.map(item => item.name)).size, report.passed);
  calibrationCount += report.passed;
  replay(spec, report);
}
let anchorCheck = null;
for (const spec of q.bootstrap) {
  const result = record(spec), report = JSON.parse(result.log);
  if (spec.name === 'r124-development-anchor-check-01') anchorCheck = result.record;
  if (spec.cases) calibration(spec, result);
  else replay(spec, report);
}
const snapshot = record(q.source_snapshot);
assert.deepStrictEqual(JSON.parse(snapshot.log), {source_snapshot_only: true,
  output: root + q.source_bundle, files: 12, new_files: 3, sha256: hash(bytes(q.source_bundle)),
  source_map_sha256: p.source_map_sha256});
// The snapshot producer is exclusive-write; its captured bytes were rederived above, not replayed.

const gateNames = [...G.gates.full_runs.map(spec => spec.name), ...G.gates.gates.map(spec => spec.run)];
assert.strictEqual(gateNames.length, 27); assert.strictEqual(new Set(gateNames).size, 27);
for (const spec of G.gates.full_runs) {
  const result = record(spec); G.full(spec, result.log);
  if (spec.kind === 'gnu') {
    assert(anchorCheck, 'anchor check collected before GNU qualification');
    E.ordered(anchorCheck.clock.source_verified, result.record.clock.start);
  }
  if (spec.kind === 'musl') E.ordered(snapshot.record.clock.source_verified, result.record.clock.start);
}
const muslCheck = record(q.musl_check); replay(q.musl_check, JSON.parse(muslCheck.log));
assert.strictEqual(calibrationCount, q.helper_calibrations);
assert.strictEqual(calibrationCount, 175);
let previousGate = muslCheck.record;
for (const spec of G.gates.gates) {
  const result = record(spec);
  E.ordered(previousGate.clock.source_verified, result.record.clock.start);
  G.leaf(spec, result.log); previousGate = result.record;
}
const gateCheck = record(q.gate_check), gateReport = JSON.parse(gateCheck.log);
assert.deepStrictEqual(gateReport.checked, gateNames);
assert.strictEqual(gateReport.development_only, true);
assert.strictEqual(gateReport.packet_accepted, false);
assert.strictEqual(gateReport.source_map_sha256, p.source_map_sha256);
assert.strictEqual(new Set(gateReport.artifacts.map(item => item.file)).size, gateReport.artifacts.length);
for (const item of gateReport.artifacts) {
  const flat = path.dirname(item.file) === root.slice(0, -1) &&
    (path.basename(item.file).startsWith('r124-development-') || Object.hasOwn(q.inputs, path.basename(item.file)));
  const doc = G.gates.doc_relocations.some(relocation => item.file === path.join(repo, relocation.path));
  assert(flat || doc, 'declared gate input path');
  assert.strictEqual(hash(bytes(item.file)), item.sha256);
}
replay(q.gate_check, gateReport);
assert.deepStrictEqual(q.mutations.map(item => item.id), M.p.mutations.map(item => item.id));
assert.strictEqual(q.mutations.length, 19);
const mutations = [], distinctMaps = new Set();
let previousRestoration = gateCheck.record;
for (const [index, declaration] of q.mutations.entries()) {
  const mutation = M.p.mutations[index], original = bytes(path.join(repo, mutation.patch.path)).toString();
  assert.strictEqual(hash(original), map[mutation.patch.path]);
  const changedSource = E.core.mutatedSource(original, mutation.patch);
  M.assertMutationSource(mutation, changedSource);
  const changed = {...map, [mutation.patch.path]: mutation.expected_file_sha256};
  const changedHash = hash(JSON.stringify(changed));
  assert.strictEqual(changedHash, mutation.expected_source_map_sha256);
  distinctMaps.add(changedHash);
  const negative = record({name: declaration.negative,
    command: [...M.p.test_command, mutation.test, '--', '--exact'],
    predecessor: gnuSpec.name + '.json'}, changed, 101);
  E.ordered(previousRestoration.clock.source_verified, negative.record.clock.start);
  const panic = M.assertNegative(negative.log, mutation);
  const checked = record({name: declaration.check,
    command: ['node', root + q.mutation_checker, 'negative', mutation.id],
    predecessor: declaration.negative + '.json'}, changed);
  assert.deepStrictEqual(JSON.parse(checked.log), {development_only: true, packet_accepted: false, id: mutation.id, quiescent: true,
    expected_failure_observed: true, source_map_sha256: changedHash, log_sha256: hash(negative.log), decisive_panic: panic});
  const restored = record({name: declaration.restored, command: ['node', root + q.mutation_checker, 'restored'],
    predecessor: declaration.check + '.json'});
  assert.deepStrictEqual(JSON.parse(restored.log), {development_only: true, restored: true, process_closure_checked: false,
    source_map_sha256: p.source_map_sha256});
  previousRestoration = restored.record;
  mutations.push({id: mutation.id, test: mutation.test, source_map_sha256: changedHash, decisive_panic: panic});
}
assert.strictEqual(distinctMaps.size, 19);
assert.deepStrictEqual(q.restored.map(spec => spec.passed), [158]);
const restoredNames = [];
for (const spec of q.restored) {
  const result = record(spec);
  E.ordered(previousRestoration.clock.source_verified, result.record.clock.start);
  const expected = C.anchorRoster(spec, result.log);
  assert.strictEqual(spec.filtered, p.counts.kfd - spec.passed);
  assert.deepStrictEqual(spec.command.slice(spec.command.indexOf('--') + 1), spec.filters);
  assert.strictEqual((result.log.match(/^\s*Finished \x60test\x60 profile.*$/gm) || []).length, 1);
  restoredNames.push(...expected); previousRestoration = result.record;
}
assert.strictEqual(restoredNames.length, 158);
assert.strictEqual(new Set(restoredNames).size, 158);
const ownOutputs = new Set(['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json']
  .map(suffix => q.collector_run + suffix));
const inventoryNames = () => fs.readdirSync(root)
  .filter(name => /^r124-development-.*\.(?:json|log|js|md|rs)$/.test(name) && !ownOutputs.has(name)).sort();
const inventory = inventoryNames();
for (const name of inventory) bytes(name);
const historicalRuns = [], groups = new Set();
for (const name of inventory.filter(name => name.endsWith('.json'))) {
  const value = json(name);
  if (!Array.isArray(value.command) || !value.source_head || !value.process_group_cleanup) continue;
  assert.strictEqual(value.child_closed, true, 'retained run has a closed child');
  assert(['absent', 'signaled'].includes(value.process_group_cleanup.close.status));
  assert.deepStrictEqual(value.process_group_cleanup.close.live_members, []);
  const pgid = value.process_group_cleanup.close.pgid;
  assert(Number.isSafeInteger(pgid) && pgid > 1);
  assert.deepStrictEqual(E.liveGroupMembers(pgid), []); groups.add(pgid);
  if (!seenRuns.has(name.slice(0, -5))) historicalRuns.push({name: name.slice(0, -5),
    returncode: value.returncode, source_map_sha256: value.source_map_sha256, qualification: false});
}
assert.deepStrictEqual(C.identities(), map); C.stable();
assert.deepStrictEqual(inventoryNames(), inventory, 'stable evidence namespace');
const raw = C.artifacts().filter(item => path.dirname(item.file) === root.slice(0, -1))
  .map(item => ({name: path.basename(item.file), sha256: item.sha256, bytes: bytes(item.file).length}))
  .sort((a, b) => a.name.localeCompare(b.name));
const qualified = new Set(runs.flatMap(run => ['.json', '.log', '-source.json', '-source-after.json'].map(suffix => run.name + suffix)));
console.log(JSON.stringify({development_only: true, publication_pending: true, scope: q.scope,
  source_parent: p.source_parent, source_map_sha256: p.source_map_sha256, source_identities: p.source_count,
  source_bundle: {name: q.source_bundle, sha256: hash(bytes(q.source_bundle)), changed: 12, added: 3},
  gnu_passed: 2882, musl_passed: 2882, ignored_each: 5, libtest_targets_each: 48, csv_targets_each: 1,
  source_gates: 17, auxiliary_checks: 10, compiled_runtime_negatives: 19, distinct_mutant_maps: 19,
  helper_calibrations: 175, restored_suites: [158], runs, mutations, raw_artifacts: raw,
  retained_other_artifacts: inventory.filter(name => !qualified.has(name)), historical_runs: historicalRuns,
  closed_owned_groups: [...groups].sort((a, b) => a - b), exclusions: q.exclusions}));
