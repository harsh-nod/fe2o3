// Read-only R122 CPU/test qualification. Native execution and formal correspondence remain separate.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/', hash = value => crypto.createHash('sha256').update(value).digest('hex');
const initial = new Map();
function initialPin(name, digest) {
  assert(fs.lstatSync(root + name).isFile());
  const value = fs.readFileSync(root + name);
  assert.strictEqual(hash(value), digest, name); initial.set(name, value); return value;
}
const q = JSON.parse(initialPin('r122-development-qualification-plan-v1.json',
  '355bd7a04de8feb54f41113507620e2dc26be1e70e65e91fe37bb9ed13c865ab'));
for (const [name, digest] of Object.entries(q.inputs)) initialPin(name, digest);
initialPin(q.runner.name, q.runner.sha256);
const C = require(root + q.mutation_checker), G = require(root + q.gate_checker);
const {p, E, repo, map, bytes, json} = C;
for (const [name, value] of initial) assert.deepStrictEqual(bytes(name), value);
bytes(__filename);
assert.strictEqual(q.accepted, false);
assert.strictEqual(q.source_parent, p.source_parent);
assert.strictEqual(q.source_map_sha256, p.source_map_sha256);
assert.strictEqual(q.source_identities, 5700);
assert.deepStrictEqual(C.identities(), map);
const runs = [], seenRuns = new Set();
function record(spec, expected = map, code = 0) {
  const name = spec.run || spec.name;
  assert(!seenRuns.has(name), 'one collected record per run'); seenRuns.add(name);
  const result = C.completed(name, spec.command, spec.predecessor, expected, code);
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
assert.strictEqual(bundle.source_parent, p.source_parent);
assert.strictEqual(bundle.source_map_sha256, p.source_map_sha256);
assert.strictEqual(bundle.source_identities, p.source_count);
assert.strictEqual(bundle.accepted_baseline.commit, p.source_parent);
const baselineMap = JSON.parse(C.git(['show', p.source_parent + ':' + bundle.accepted_baseline.path]));
assert.strictEqual(hash(JSON.stringify(baselineMap)), '5fe28e8a3accfc845c4c20182b702c1e96961e542c02405b3841a91b4be62f04');
assert.strictEqual(bundle.accepted_baseline.source_map_sha256, hash(JSON.stringify(baselineMap)));
assert.strictEqual(Object.keys(baselineMap).length, 5696);
assert(Object.keys(baselineMap).every(file => Object.hasOwn(map, file)));
const delta = Object.keys(map).filter(file => baselineMap[file] !== map[file]);
assert.deepStrictEqual(bundle.files.map(file => file.path), delta);
assert.strictEqual(delta.length, 15);
assert.strictEqual(bundle.files.filter(file => file.previous_sha256 === null).length, 4);
for (const file of bundle.files) {
  assert.strictEqual(file.previous_sha256, baselineMap[file.path] || null);
  assert.strictEqual(file.sha256, map[file.path]);
  assert.strictEqual(hash(file.text), file.sha256);
  assert.strictEqual(bytes(path.join(repo, file.path)).toString(), file.text);
  if (file.previous_sha256) assert.strictEqual(hash(C.git(['show', p.source_parent + ':' + file.path])), file.previous_sha256);
}
C.baseline();
record({name: 'r122-development-clippy-v2',
  command: ['cargo', 'clippy', '-p', 'fe2o3-kfd', '-p', 'fe2o3-runtime', '--all-features', '--all-targets', '--', '-D', 'warnings'],
  predecessor: 'r122-development-clippy-v1.json'});
for (const spec of q.bootstrap) {
  const result = record(spec), report = JSON.parse(result.log);
  if (spec.cases) {
    assert.strictEqual(report.development_only, true);
    assert.strictEqual(report.helper_calibration_only, true);
    assert.strictEqual(report.runtime_mutation_count, 0);
    assert.strictEqual(report.helper_sha256, spec.helper_sha256);
    assert.strictEqual(report.passed, spec.cases.length);
    assert.deepStrictEqual(report.cases, spec.cases, 'exact ordered calibration roster');
    assert.strictEqual(new Set(report.cases.map(item => item.name)).size, report.passed);
  }
  replay(spec, report);
}
const gateNames = [...G.gates.full_runs.map(spec => spec.name), ...G.gates.gates.map(spec => spec.run)];
assert.strictEqual(gateNames.length, 27); assert.strictEqual(new Set(gateNames).size, 27);
for (const spec of G.gates.full_runs) G.full(spec, record(spec).log);
const muslCheck = record(q.musl_check); replay(q.musl_check, JSON.parse(muslCheck.log));
for (const spec of G.gates.gates) G.leaf(spec, record(spec).log);
const gateCheck = record(q.gate_check), gateReport = JSON.parse(gateCheck.log);
assert.deepStrictEqual(gateReport.checked, gateNames);
assert.strictEqual(gateReport.development_only, true);
assert.strictEqual(gateReport.packet_accepted, false);
assert.strictEqual(gateReport.source_map_sha256, p.source_map_sha256);
assert.strictEqual(new Set(gateReport.artifacts.map(item => item.file)).size, gateReport.artifacts.length);
for (const item of gateReport.artifacts) {
  const flat = path.dirname(item.file) === root.slice(0, -1) &&
    (path.basename(item.file).startsWith('r122-development-') || Object.hasOwn(q.inputs, path.basename(item.file)));
  const doc = G.gates.doc_relocations.some(relocation => item.file === path.join(repo, relocation.path));
  assert(flat || doc, 'declared gate input path');
  assert.strictEqual(hash(bytes(item.file)), item.sha256);
}
replay(q.gate_check, gateReport);
assert.deepStrictEqual(q.mutations.map(item => item.id), p.mutations.map(item => item.id));
assert.strictEqual(q.mutations.length, 12);
const mutations = [], distinctMaps = new Set();
let previousRestoration = gateCheck.record;
for (const [index, declaration] of q.mutations.entries()) {
  const mutation = p.mutations[index], original = bytes(path.join(repo, mutation.patch.path)).toString();
  assert.strictEqual(hash(original), map[mutation.patch.path]);
  const changedSource = E.core.mutatedSource(original, mutation.patch);
  C.assertMutationSource(mutation, changedSource);
  const changed = {...map, [mutation.patch.path]: mutation.expected_file_sha256};
  const changedHash = hash(JSON.stringify(changed)); distinctMaps.add(changedHash);
  const negative = record({name: declaration.negative,
    command: [...p.test_commands[mutation.kind], mutation.test, '--', '--exact'],
    predecessor: p.baseline.name + '.json'}, changed, 101);
  E.ordered(previousRestoration.clock.source_verified, negative.record.clock.start);
  const panic = C.assertNegative(negative.log, mutation);
  const checked = record({name: declaration.check,
    command: ['node', root + q.mutation_checker, 'negative', mutation.id],
    predecessor: declaration.negative + '.json'}, changed);
  assert.deepStrictEqual(JSON.parse(checked.log), {development_only: true, id: mutation.id, quiescent: true,
    expected_failure_observed: true, source_map_sha256: changedHash, log_sha256: hash(negative.log), decisive_panic: panic});
  const restored = record({name: declaration.restored, command: ['node', root + q.mutation_checker, 'restored'],
    predecessor: declaration.check + '.json'});
  assert.deepStrictEqual(JSON.parse(restored.log), {development_only: true, restored: true, process_closure_checked: false,
    source_map_sha256: p.source_map_sha256, identities: p.source_count});
  previousRestoration = restored.record;
  mutations.push({id: mutation.id, test: mutation.test, source_map_sha256: changedHash, decisive_panic: panic});
}
assert.strictEqual(distinctMaps.size, 12);
const fullTests = E.passing(bytes(p.baseline.name + '.log').toString());
assert.deepStrictEqual(q.restored.map(spec => spec.passed), [35, 10]);
for (const spec of q.restored) {
  const {log} = record(spec), expected = fullTests.filter(name => spec.filters.some(filter => name.includes(filter)));
  assert.strictEqual(expected.length, spec.passed);
  assert.strictEqual(new Set(expected).size, spec.passed);
  E.assertPassing(log);
  assert.deepStrictEqual(E.passing(log), expected);
  assert.deepStrictEqual(E.ignored(log), []);
  assert.deepStrictEqual(C.summaries(log), [['ok', spec.passed, 0, 0, 0, spec.filtered]]);
  assert.strictEqual(spec.filtered, p.counts[spec.kind] - spec.passed);
  const groups = G.groups(log, false);
  assert.strictEqual(groups.length, 1);
  assert.strictEqual(groups[0].name, 'unittests src/lib.rs (target/debug/deps/fe2o3_' + spec.kind + ')');
  assert.strictEqual((log.match(/^\s*Finished \x60test\x60 profile.*$/gm) || []).length, 1);
}
const ownOutputs = new Set(['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json']
  .map(suffix => q.collector_run + suffix));
const inventoryNames = () => fs.readdirSync(root)
  .filter(name => /^r122-development-.*\.(?:json|log|js|md|rs)$/.test(name) && !ownOutputs.has(name)).sort();
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
  source_bundle: {name: q.source_bundle, sha256: hash(bytes(q.source_bundle)), changed: 15, added: 4},
  gnu_passed: 2846, musl_passed: 2846, ignored_each: 5, libtest_targets_each: 48, csv_targets_each: 1,
  source_gates: 17, auxiliary_checks: 10, compiled_runtime_negatives: 12, distinct_mutant_maps: 12,
  helper_calibrations: 81, restored_suites: [35, 10], runs, mutations, raw_artifacts: raw,
  retained_other_artifacts: inventory.filter(name => !qualified.has(name)), historical_runs: historicalRuns,
  closed_owned_groups: [...groups].sort((a, b) => a - b), exclusions: q.exclusions}));
