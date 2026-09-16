// Freeze collection requirements before the compiled negative/restoration campaign.
const fs = require('fs'), path = require('path'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const checker = 'r125-development-check-v2.js';
const checkerHash = '8de74c2bbd0bf464654692614754aea48eeefb29a74314a8223acd30234fb1c4';
assert.strictEqual(hash(fs.readFileSync(root + checker)), checkerHash);
const C = require(root + checker), {p, E} = C;
C.pin(checker, checkerHash); C.bytes(__filename);
const mutationChecker = 'r125-development-mutation-check-v3.js';
C.pin(mutationChecker, 'd6cf905321ad87c86f583ccc77dcc9c8df168d30a6784ad16e36ed5fd2ae9793');
const M = require(root + mutationChecker);
assert.strictEqual(M.C, C);
const gateChecker = 'r125-development-gate-check-v2.js';
C.pin(gateChecker, 'ca743ebf03fee6732c1aa486d39ca426f1103df42c2b88349d00c704949725d4');
const linuxCalibration = 'r119-integrated-linux-helpers.log';
C.pin(linuxCalibration, 'b78cb48f28f17aaad884d876e8fce4b4cae038155be715c5b678d77113956815');
const gnu = p.full_runs.find(spec => spec.kind === 'gnu');
const musl = p.full_runs.find(spec => spec.kind === 'musl');
const run = (name, helper, args, predecessor) => ({name,
  command: ['node', root + helper, ...args], predecessor});
const gnuCheck = run('r125-development-gnu-check-v2', checker,
  ['baseline', 'gnu'], gnu.name + '.json');
const fullCheck = run('r125-development-full-check-v2', gateChecker,
  ['full', 'musl'], musl.name + '.json');
assert.strictEqual(musl.predecessor, gnuCheck.name + '.json');
assert.strictEqual(p.gates[0].predecessor, fullCheck.name + '.json');
C.baseline();
C.completed(gnuCheck.name, gnuCheck.command, gnuCheck.predecessor);

const calibrationSpecs = [
  {...run('r125-development-parser-calibration-02', 'r125-development-parser-tests-v2.js', [],
    gnuCheck.name + '.json'), helper_sha256: checkerHash, accept: 5, reject: 50},
  {...run('r125-development-gate-parser-calibration-02', 'r125-development-gate-parser-tests-v2.js', [],
    'r125-development-parser-calibration-02.json'),
    helper_sha256: 'ca743ebf03fee6732c1aa486d39ca426f1103df42c2b88349d00c704949725d4', accept: 10, reject: 39},
  {...run('r125-development-mutation-parser-calibration-03', 'r125-development-mutation-parser-tests-v3.js', [],
    'r125-development-source-snapshot-02.json'),
    helper_sha256: 'd6cf905321ad87c86f583ccc77dcc9c8df168d30a6784ad16e36ed5fd2ae9793', accept: 38, reject: 63},
];
const calibrations = calibrationSpecs.map(spec => {
  const result = C.completed(spec.name, spec.command, spec.predecessor), report = JSON.parse(result.log);
  assert.strictEqual(report.passed, spec.accept + spec.reject);
  assert.strictEqual(report.cases.filter(item => item.expected === 'accept').length, spec.accept);
  assert.strictEqual(report.cases.filter(item => item.expected === 'reject').length, spec.reject);
  assert.strictEqual(new Set(report.cases.map(item => item.name)).size, report.passed);
  assert(report.cases.every(item => typeof item.name === 'string' && item.name.length > 0));
  assert.deepStrictEqual(report, {development_only: true, helper_calibration_only: true,
    runtime_mutation_count: 0, helper_sha256: spec.helper_sha256,
    passed: spec.accept + spec.reject, cases: report.cases});
  return {...spec, cases: report.cases};
});
assert.strictEqual(calibrations.reduce((sum, spec) => sum + spec.cases.length, 0), 205);

const mutations = M.p.mutations.map(mutation => ({id: mutation.id,
  predecessor: mutation.predecessor,
  negative: 'r125-development-negative-' + mutation.id + '-v1',
  check: 'r125-development-negative-check-' + mutation.id + '-v1',
  restored: 'r125-development-restored-' + mutation.id + '-v1'}));
assert.strictEqual(mutations.length, 18);
const gateCheck = run('r125-development-gate-check-all-v1', gateChecker, ['all'],
  p.gates.at(-1).run + '.json');
assert.strictEqual(mutations[0].predecessor, gateCheck.name + '.json');
for (let i = 1; i < mutations.length; i++)
  assert.strictEqual(mutations[i].predecessor, mutations[i - 1].restored + '.json');

const targets = E.executables(C.accepted(gnu));
function allNames(kind) {
  const found = targets.filter(target => target.kind === 'libtest' &&
    target.name.endsWith('/fe2o3_' + kind + ')'));
  assert.strictEqual(found.length, 1);
  const names = [...found[0].passing, ...p.new_tests[kind]].sort();
  assert.strictEqual(names.length, p.counts[kind]);
  assert.strictEqual(new Set(names).size, names.length);
  return {target: found[0].name, names};
}
const kfd = allNames('kfd'), runtime = allNames('runtime');
const filters = ['persistent_cancel', 'persistent_allocation::tests',
  'queue::dispatch_binding::control_release::tests::persistent::', ...M.p.preserved_tests];
const restoredKfdNames = kfd.names.filter(name => filters.some(filter => name.includes(filter)));
assert.strictEqual(restoredKfdNames.length, 46);
assert(M.p.allowed_tests.every(name => restoredKfdNames.includes(name)));
assert(M.p.mutations.every(mutation => restoredKfdNames.includes(mutation.test)));
const restored = [
  {name: 'r125-development-restored-persistent-v1', kind: 'kfd',
    command: [...M.p.test_command, '--', ...filters],
    predecessor: mutations.at(-1).restored + '.json',
    target: kfd.target, tests: restoredKfdNames, passed: 46, filtered: p.counts.kfd - 46},
  {name: 'r125-development-restored-runtime-v1', kind: 'runtime',
    command: ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline',
      '-p', 'fe2o3-runtime', '--all-features', '--lib'],
    predecessor: 'r125-development-restored-persistent-v1.json',
    target: runtime.target, tests: runtime.names, passed: p.counts.runtime, filtered: 0},
];
const names = ['r125-development-full-plan-v3.json', checker, gateChecker,
  'r125-development-mutations-v2.json', mutationChecker, 'r125-development-mutation-inventory-v2.js',
  'r125-development-mutation-plan-build-v2.js', 'r125-development-mutation-parser-tests-v3.js',
  'r125-development-mutation-patch-v2.js', 'r125-development-parser-tests-v2.js',
  'r125-development-gate-parser-tests-v2.js', 'r125-development-source-bundle-v2.js',
  'r125-development-source-bundle-v2.json', 'r125-development-plan-v3.js',
  'r125-development-evidence-v2.js', path.basename(__filename), linuxCalibration, ...Object.keys(p.parser_inputs)];
assert.strictEqual(new Set(names).size, names.length);
const plan = {accepted: false,
  scope: 'R125 local CPU/test qualification only; native execution, aggregate retained memory, formal implementation correspondence and HIP/HSA performance acceptance remain separate.',
  source_parent: p.source_parent, source_map_sha256: p.source_map_sha256, source_identities: p.source_count,
  runner: {name: p.runner, sha256: p.runner_sha256},
  full_checker: checker, gate_checker: gateChecker, mutation_checker: mutationChecker,
  inputs: Object.fromEntries(names.sort().map(name => [name, C.hash(C.bytes(name))])),
  source_bundle: 'r125-development-source-bundle-v2.json', source_changed: 16, source_added: 3,
  source_plan_run: run('r125-development-mutation-plan-02', 'r125-development-mutation-plan-build-v2.js', [],
    'r125-development-stack-repair-audit-01.json'),
  source_snapshot: run('r125-development-source-snapshot-02', 'r125-development-source-bundle-v2.js', [],
    'r125-development-mutation-plan-02.json'),
  calibrations, helper_calibrations: 205, gnu_check: gnuCheck, full_check: fullCheck,
  gate_check: gateCheck, mutations, restored,
  collector: 'r125-development-qualification-collect-v1.js',
  collector_run: 'r125-development-qualification-collect-v1',
  collector_predecessor: restored.at(-1).name + '.json',
  superseded_attempts: p.superseded_attempts,
  exclusions: [...M.p.exclusions,
    'Historical failed, interrupted and intermediate-cohort runs remain nonqualifying retained diagnostics.',
    'The boxed-ledger measurements are inline type sizes, not aggregate-memory or maximum-stack proofs.',
    'Cold three-binding output admission, A1/A2, issue #182 and broad HIP/HSA parity remain incomplete.'],
};
assert.deepStrictEqual(C.identities(), C.map); C.stable();
const output = root + 'r125-development-qualification-plan-v1.json';
fs.writeFileSync(output, JSON.stringify(plan, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify({output, sha256: hash(fs.readFileSync(output)), accepted: false,
  source_map_sha256: p.source_map_sha256, helper_calibrations: 205,
  compiled_negatives_required: 18, restored_test_counts: restored.map(spec => spec.passed)}));
