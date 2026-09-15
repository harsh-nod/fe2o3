// Freeze the remaining CPU qualification after bootstrap calibration and source capture.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
function pin(name, digest) {
  const value = fs.readFileSync(root + name);
  assert.strictEqual(hash(value), digest, name);
  return value;
}
const mutationChecker = 'r124-development-mutation-check-v2.js';
const gateChecker = 'r124-development-gate-check-v2.js';
pin(mutationChecker, '7a943a91d7e81869f91bae66e7dc90b513c9afbf8ff460caa149f794d44ac6eb');
pin(gateChecker, '3032fc3037336e27d09a74db7d703c0f370ea5cb63279daee4f0c79c76716821');
const M = require(root + mutationChecker), C = M.C, G = require(root + gateChecker);
assert.strictEqual(G.C, C);
const p = C.p, inputs = {...C.resumption.inputs, ...p.parser_inputs, ...p.runners};
for (const name of [
  'r124-development-full-plan-v2.json', 'r124-development-mutations-v2.json',
  'r124-development-evidence-v2.js', 'r124-development-check-v2.js',
  'r124-development-resumption-plan-v1.json', 'r124-development-qualification-plan-build-v2.js', gateChecker, mutationChecker,
  'r124-development-mutation-inventory-v1.js', 'r124-development-mutation-patch-v2.js',
  'r124-development-parser-tests-v2.js', 'r124-development-gate-parser-tests-v2.js',
  'r124-development-mutation-parser-tests-v2.js',
  'r124-development-source-bundle-v1.js', 'r124-development-source-bundle-v1.json',
]) inputs[name] = hash(fs.readFileSync(root + name));
inputs['r119-integrated-linux-helpers.log'] = 'b78cb48f28f17aaad884d876e8fce4b4cae038155be715c5b678d77113956815';
for (const [name, digest] of Object.entries(inputs)) pin(name, digest);
const rosterHashes = {
  full: '04159e679898762673f1906c3349342b18a5a263487540281ae04554ef943a2d',
  gates: '2bfa2a96e1cc201bc6f58c7b17ab43b53bbc549fdad4ed33bcbb9a75dcfd16e8',
  mutations: 'c0948c40bdab60c6771c75673bbf4b5542d1274be41fc3d02cf786fe993041d4',
};
function calibration(name, predecessor, helper, checker, count, rosterHash) {
  assert(/^[0-9a-f]{64}$/.test(rosterHash), 'calibration roster must be frozen');
  const report = JSON.parse(fs.readFileSync(root + name + '.log'));
  assert.strictEqual(report.passed, count);
  assert.strictEqual(report.cases.length, count);
  assert.strictEqual(new Set(report.cases.map(test => test.name)).size, count);
  assert.strictEqual(hash(JSON.stringify(report.cases)), rosterHash);
  assert.deepStrictEqual(report, {development_only: true, helper_calibration_only: true,
    runtime_mutation_count: 0, helper_sha256: inputs[checker], passed: count, cases: report.cases});
  return {name, command: ['node', root + helper], predecessor,
    cases: report.cases, helper_sha256: report.helper_sha256};
}
const bootstrap = [
  p.resumption.admission,
  calibration('r124-development-parser-calibration-02', 'r124-development-readmission-v1.json',
    'r124-development-parser-tests-v2.js', 'r124-development-check-v2.js', 54, rosterHashes.full),
  calibration('r124-development-gate-parser-calibration-02', 'r124-development-parser-calibration-02.json',
    'r124-development-gate-parser-tests-v2.js', gateChecker, 43, rosterHashes.gates),
  calibration('r124-development-mutation-parser-calibration-02', 'r124-development-gate-parser-calibration-02.json',
    'r124-development-mutation-parser-tests-v2.js', mutationChecker, 100, rosterHashes.mutations),
];
for (const spec of bootstrap) {
  const result = C.completed(spec.name, spec.command, spec.predecessor, C.map, 0, spec.runner || p.runner);
  if (spec.name === p.resumption.admission.name) assert.deepStrictEqual(JSON.parse(result.log), C.readmission());
  for (const suffix of ['.json', '.log', '-source.json', '-source-after.json'])
    inputs[spec.name + suffix] = hash(C.bytes(spec.name + suffix));
}
const previousQualification = JSON.parse(C.pin('r124-development-qualification-plan-v1.json',
  '0efd2e3d6b61d33541ad65cc3cf95a215889ee487fdd404f4a6842ed292b54ad'));
const mutations = M.p.mutations.map(mutation => ({id: mutation.id, predecessor: mutation.predecessor,
  negative: 'r124-development-negative-' + mutation.id + '-v1',
  check: 'r124-development-negative-check-' + mutation.id + '-v1',
  restored: 'r124-development-restored-' + mutation.id + '-v1'}));
assert.strictEqual(mutations.length, 19);
const regression = p.anchors.filter(anchor => anchor.passed !== null);
assert.strictEqual(regression.length, 1);
assert.strictEqual(regression[0].passed, 158);
assert.deepStrictEqual(regression[0].command.slice(0, 3), ['cargo', '+nightly-2026-04-03', 'test']);
const restored = [{name: 'r124-development-final-restored-regression-v1', kind: 'kfd',
  command: regression[0].command.slice(),
  filters: regression[0].command.slice(regression[0].command.indexOf('--') + 1),
  passed: 158, filtered: p.counts.kfd - 158, predecessor: mutations.at(-1).restored + '.json'}];
const q = {
  accepted: false, scope: 'R124 local CPU/test qualification only',
  source_parent: p.source_parent, source_map_sha256: p.source_map_sha256, source_identities: p.source_count,
  runner: {name: p.runner, sha256: p.runners[p.runner]}, inputs,
  full_checker: 'r124-development-check-v2.js', gate_checker: gateChecker, mutation_checker: mutationChecker,
  source_bundle: 'r124-development-source-bundle-v1.json',
  source_snapshot: {name: 'r124-development-source-snapshot-01',
    command: ['node', root + 'r124-development-source-bundle-v1.js'],
    predecessor: 'r124-development-mutation-parser-calibration-01.json', runner: 'r124-development-run-v2.js'},
  bootstrap,
  historical_bootstrap: previousQualification.bootstrap.map(spec => ({...spec, runner: 'r124-development-run-v2.js'})),
  resumption: p.resumption, retained_helper_calibrations: 175,
  musl_check: {name: 'r124-development-musl-gate-check-v2',
    command: ['node', root + gateChecker, 'full', 'musl'],
    predecessor: 'r124-development-musl-all-v2.json'},
  gate_check: {name: 'r124-development-gate-check-all-v2',
    command: ['node', root + gateChecker, 'all'], predecessor: p.gates.at(-1).run + '.json'},
  mutations, restored, collector_run: 'r124-development-qualification-collect-v2',
  collector_predecessor: restored[0].name + '.json', helper_calibrations: 197,
  exclusions: [...new Set([...C.resumption.exclusions, 'authenticated formal refinement',
    'aggregate retained-memory bounds', 'multi-device acceptance'])],
};
assert.deepStrictEqual(C.identities(), C.map); C.stable();
const output = root + 'r124-development-qualification-plan-v2.json';
fs.writeFileSync(output, JSON.stringify(q, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify({output, sha256: hash(fs.readFileSync(output)), accepted: false,
  helper_calibrations: q.helper_calibrations, mutations: mutations.length, restored: [158]}));
