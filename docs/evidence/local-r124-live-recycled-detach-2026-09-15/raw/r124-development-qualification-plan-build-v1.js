// Freeze the remaining CPU qualification after bootstrap calibration and source capture.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
function pin(name, digest) {
  const value = fs.readFileSync(root + name);
  assert.strictEqual(hash(value), digest, name);
  return value;
}
const mutationChecker = 'r124-development-mutation-check-v1.js';
const gateChecker = 'r124-development-gate-check-v1.js';
pin(mutationChecker, '0d72aad2e3d8bed06cd6e954c0cbe2c7f13c1704d624dd702e059fa7c2a190cc');
pin(gateChecker, '29276457e534266a342fa7e2bc577286f4ec2d11f9f91fd3ebb88f4d1f5a12aa');
const M = require(root + mutationChecker), C = M.C, G = require(root + gateChecker);
assert.strictEqual(G.C, C);
const p = C.p, inputs = {...p.parser_inputs, ...p.runners};
for (const name of [
  'r124-development-full-plan-v1.json', 'r124-development-mutations-v1.json',
  'r124-development-evidence-v1.js', 'r124-development-check-v1.js',
  'r124-development-anchor-check-v1.js', gateChecker, mutationChecker,
  'r124-development-mutation-inventory-v1.js', 'r124-development-mutation-patch-v1.js',
  'r124-development-parser-tests-v1.js', 'r124-development-gate-parser-tests-v1.js',
  'r124-development-mutation-parser-tests-v1.js',
  'r124-development-source-bundle-v1.js', 'r124-development-source-bundle-v1.json',
]) inputs[name] = hash(fs.readFileSync(root + name));
inputs['r119-integrated-linux-helpers.log'] = 'b78cb48f28f17aaad884d876e8fce4b4cae038155be715c5b678d77113956815';
for (const [name, digest] of Object.entries(inputs)) pin(name, digest);
const rosterHashes = {
  full: 'f9886b470838d1676c65d7deb1754949fb442d830523be2b6d2654b9cd1a47a6',
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
  {name: 'r124-development-anchor-check-01',
    command: ['node', root + 'r124-development-anchor-check-v1.js'],
    predecessor: 'r124-development-clippy-02.json'},
  {name: 'r124-development-gnu-check-v1',
    command: ['node', root + 'r124-development-check-v1.js', 'baseline', 'gnu'],
    predecessor: 'r124-development-gnu-all-v1.json'},
  calibration('r124-development-parser-calibration-01', 'r124-development-gnu-check-v1.json',
    'r124-development-parser-tests-v1.js', 'r124-development-check-v1.js', 32, rosterHashes.full),
  calibration('r124-development-gate-parser-calibration-01', 'r124-development-parser-calibration-01.json',
    'r124-development-gate-parser-tests-v1.js', gateChecker, 43, rosterHashes.gates),
  calibration('r124-development-mutation-parser-calibration-01', 'r124-development-gate-parser-calibration-01.json',
    'r124-development-mutation-parser-tests-v1.js', mutationChecker, 100, rosterHashes.mutations),
];
const mutations = M.p.mutations.map(mutation => ({id: mutation.id,
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
  full_checker: 'r124-development-check-v1.js', gate_checker: gateChecker, mutation_checker: mutationChecker,
  source_bundle: 'r124-development-source-bundle-v1.json',
  source_snapshot: {name: 'r124-development-source-snapshot-01',
    command: ['node', root + 'r124-development-source-bundle-v1.js'],
    predecessor: 'r124-development-mutation-parser-calibration-01.json'},
  bootstrap,
  musl_check: {name: 'r124-development-musl-gate-check-v1',
    command: ['node', root + gateChecker, 'full', 'musl'],
    predecessor: 'r124-development-musl-all-v1.json'},
  gate_check: {name: 'r124-development-gate-check-all-v1',
    command: ['node', root + gateChecker, 'all'], predecessor: p.gates.at(-1).run + '.json'},
  mutations, restored, collector_run: 'r124-development-qualification-collect-v1',
  collector_predecessor: restored[0].name + '.json', helper_calibrations: 175,
  exclusions: ['native execution', 'authenticated formal refinement', 'continuous source immutability',
    'aggregate retained-memory bounds', 'multi-device acceptance', 'HIP/HSA performance parity'],
};
assert.deepStrictEqual(C.identities(), C.map); C.stable();
const output = root + 'r124-development-qualification-plan-v1.json';
fs.writeFileSync(output, JSON.stringify(q, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify({output, sha256: hash(fs.readFileSync(output)), accepted: false,
  helper_calibrations: q.helper_calibrations, mutations: mutations.length, restored: [158]}));
