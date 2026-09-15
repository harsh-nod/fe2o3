// Emit a prospective qualification plan; do not execute pending gates or mutate sources.
const fs = require('fs'), path = require('path'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/', hash = value => crypto.createHash('sha256').update(value).digest('hex');
function pin(name, digest) {
  const bytes = fs.readFileSync(root + name);
  assert.strictEqual(hash(bytes), digest, name);
  return bytes;
}
const mutationChecker = 'r123-development-mutation-check-v2.js';
const gateChecker = 'r123-development-gate-check-v2.js';
pin(mutationChecker, '143f0e5dd9747a6ab4436ea8c78de351f9598d2b783c948b657bdfda0d5d3d23');
pin(gateChecker, '10ceeaf5d5635ea08646bc399f72a4c9bcf28d8097ff07b167d852b1eee7fd65');
const M = require(root + mutationChecker), C = M.C, G = require(root + gateChecker);
assert.strictEqual(G.C, C);
const p = C.p;
const inputs = {...p.parser_inputs, ...p.runners};
for (const name of [
  'r123-development-full-plan-v1.json', 'r123-development-mutations-v1.json',
  'r123-development-evidence-v1.js', 'r123-development-check-v1.js',
  gateChecker, mutationChecker, 'r123-development-parser-tests-v1.js',
  'r123-development-mutation-parser-tests-v2.js', 'r123-development-gate-parser-tests-v2.js',
  'r123-development-source-bundle-v1.js', 'r123-development-source-bundle-v1.json',
]) inputs[name] = hash(fs.readFileSync(root + name));
inputs['r119-integrated-linux-helpers.log'] = 'b78cb48f28f17aaad884d876e8fce4b4cae038155be715c5b678d77113956815';
for (const [name, digest] of Object.entries(inputs)) pin(name, digest);
function calibration(name, previous, helper, count, rosterHash) {
  const report = JSON.parse(fs.readFileSync(root + name + '.log'));
  assert.strictEqual(report.passed, count);
  assert.strictEqual(report.cases.length, count);
  assert.strictEqual(hash(JSON.stringify(report.cases)), rosterHash);
  assert.strictEqual(report.development_only, true);
  assert.strictEqual(report.helper_calibration_only, true);
  assert.strictEqual(report.runtime_mutation_count, 0);
  return {name, command: ['node', root + helper], predecessor: previous,
    cases: report.cases, helper_sha256: report.helper_sha256};
}
const bootstrap = [
  {name: 'r123-development-gnu-check-v1',
    command: ['node', root + 'r123-development-check-v1.js', 'baseline', 'gnu'],
    predecessor: 'r123-development-gnu-all-v1.json'},
  calibration('r123-development-parser-calibration-01', 'r123-development-gnu-check-v1.json',
    'r123-development-parser-tests-v1.js', 32, 'f9886b470838d1676c65d7deb1754949fb442d830523be2b6d2654b9cd1a47a6'),
  calibration('r123-development-mutation-parser-calibration-01', 'r123-development-gate-parser-calibration-01.json',
    'r123-development-mutation-parser-tests-v2.js', 64, '333c5431180a9390c726d579fc0a0dafdb5b00694c804948815ce88d00cf1ba0'),
];
const priorGate = calibration('r123-development-gate-parser-calibration-01', 'r123-development-parser-calibration-01.json',
  'r123-development-gate-parser-tests-v1.js', 30, '891d63b2867aaa05697ed3686e6ed89ee0eaff03a862d0db48eeb32835202035');
const gateCases = priorGate.cases.slice();
const insertion = gateCases.findIndex(item => item.name === 'Python declared test count');
assert(insertion > 0);
gateCases.splice(insertion, 0, ...G.extraDocRelocations.flatMap(r => [
  {name: 'GNU stale dispatch-binding doctest ' + r.to, expected: 'reject'},
  {name: 'GNU missing dispatch-binding doctest ' + r.to, expected: 'reject'},
]));
assert.strictEqual(gateCases.length, 38);
const muslCheck = {name: 'r123-development-musl-gate-check-v2',
  command: ['node', root + gateChecker, 'full', 'musl'], predecessor: 'r123-development-musl-all-v1.json'};
const gateCalibration = {name: 'r123-development-gate-parser-calibration-02',
  command: ['node', root + 'r123-development-gate-parser-tests-v2.js'], predecessor: muslCheck.name + '.json',
  cases: gateCases, helper_sha256: inputs[gateChecker]};
const mutations = M.p.mutations.map(m => ({id: m.id,
  negative: 'r123-development-negative-' + m.id + '-v1',
  check: 'r123-development-negative-check-' + m.id + '-v1',
  restored: 'r123-development-restored-' + m.id + '-v1'}));
assert.strictEqual(mutations.length, 10);
const restored = p.anchors.filter(a => a.passed !== null).map((anchor, i) => {
  const name = i === 0 ? 'r123-development-final-restored-focused-v1' : 'r123-development-final-restored-r122-v1';
  const command = ['cargo', '+nightly-2026-04-03', ...anchor.command.slice(1)];
  return {name, kind: 'kfd', command, filters: command.slice(command.indexOf('--') + 1),
    passed: anchor.passed, filtered: p.counts.kfd - anchor.passed,
    predecessor: (i === 0 ? mutations.at(-1).restored : 'r123-development-final-restored-focused-v1') + '.json'};
});
assert.deepStrictEqual(restored.map(s => s.passed), [128, 17]);
const q = {accepted: false, scope: 'R123 local CPU/test qualification only',
  source_parent: p.source_parent, source_map_sha256: p.source_map_sha256, source_identities: p.source_count,
  runner: {name: p.runner, sha256: p.runners[p.runner]}, inputs,
  full_checker: 'r123-development-check-v1.js', gate_checker: gateChecker, mutation_checker: mutationChecker,
  source_bundle: 'r123-development-source-bundle-v1.json',
  source_snapshot: {name: 'r123-development-source-snapshot-01',
    command: ['node', root + 'r123-development-source-bundle-v1.js'],
    predecessor: 'r123-development-mutation-parser-calibration-01.json'},
  bootstrap, musl_check: muslCheck, gate_calibration: gateCalibration,
  gate_check: {name: 'r123-development-gate-check-all-v2',
    command: ['node', root + gateChecker, 'all'], predecessor: p.gates.at(-1).run + '.json'},
  mutations, restored, collector_run: 'r123-development-qualification-collect-v1',
  collector_predecessor: restored.at(-1).name + '.json',
  helper_calibrations: 134, superseded_gate_calibration: priorGate.name,
  exclusions: ['native execution', 'authenticated formal refinement', 'continuous source immutability',
    'aggregate retained-memory bounds', 'multi-device acceptance', 'HIP/HSA performance parity']};
assert.deepStrictEqual(C.identities(), C.map); C.stable();
console.log(JSON.stringify(q, null, 2));
