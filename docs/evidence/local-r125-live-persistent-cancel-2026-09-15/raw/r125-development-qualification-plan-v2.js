// Freeze collection requirements before the compiled negative/restoration campaign.
const fs = require('fs'), path = require('path'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const checker = 'r125-development-check-v3.js';
const checkerHash = '308fa81a3d7a475491f48490f8ab77761bcb0c45fb30ffbb77796f61c0e8aaaa';
assert.strictEqual(hash(fs.readFileSync(root + checker)), checkerHash);
const C = require(root + checker), {p, E} = C;
C.pin(checker, checkerHash); C.bytes(__filename);
const mutationChecker = 'r125-development-mutation-check-v4.js';
C.pin(mutationChecker, '608f5d428766e7984fbb085b9fafa0ba37633eceb4837a2fb605c110b4a06ec4');
const M = require(root + mutationChecker);
assert.strictEqual(M.C, C);
const gateChecker = 'r125-development-gate-check-v3.js';
C.pin(gateChecker, '394de8522929f0ea9d4bc76a746d903783e2ce1a18f3131f7cd99fab1c1cba41');
const linuxCalibration = 'r119-integrated-linux-helpers.log';
C.pin(linuxCalibration, 'b78cb48f28f17aaad884d876e8fce4b4cae038155be715c5b678d77113956815');
const gnu = p.full_runs.find(spec => spec.kind === 'gnu');
const musl = p.full_runs.find(spec => spec.kind === 'musl');
const run = (name, helper, args, predecessor) => ({name,
  command: ['node', root + helper, ...args], predecessor});
const gnuCheck = run('r125-development-gnu-check-v3', checker,
  ['baseline', 'gnu'], gnu.name + '.json');
const fullCheck = run('r125-development-full-check-v3', gateChecker,
  ['full', 'musl'], musl.name + '.json');
assert.strictEqual(musl.predecessor, gnuCheck.name + '.json');
assert.strictEqual(p.gates[0].predecessor, fullCheck.name + '.json');
const gnuBaseline = C.baseline();
C.completed(gnuCheck.name, gnuCheck.command, gnuCheck.predecessor);

const diagnosticAudit = {...run('r125-development-zero-scan-audit-01',
  'r125-development-zero-scan-audit-v1.js', [], 'r125-development-zero-scan-fixture-musl-01.json'),
  helper_sha256: '16d28a983e23fa35e825e48c8f6313ac3f9f9ffa9931f3cfc5c86a396954f631'};
C.pin('r125-development-zero-scan-audit-v1.js', diagnosticAudit.helper_sha256);
const diagnosticResult = C.completed(diagnosticAudit.name, diagnosticAudit.command, diagnosticAudit.predecessor);
E.ordered(diagnosticResult.record.clock.source_verified, gnuBaseline.record.clock.start);
const diagnosticReport = JSON.parse(diagnosticResult.log);
assert.strictEqual(diagnosticReport.source_map_sha256, p.source_map_sha256);
assert.strictEqual(diagnosticReport.source_delta_files, 2);
assert.strictEqual(diagnosticReport.runs.length, 6);
assert.strictEqual(gnu.predecessor, diagnosticReport.runs.at(-1).name + '.json');
assert.deepStrictEqual(JSON.parse(require('child_process').execFileSync('node',
  diagnosticAudit.command.slice(1), {cwd: C.repo, maxBuffer: 64 * 1024 * 1024}).toString()), diagnosticReport);

const calibrationSpecs = [
  {...run('r125-development-parser-calibration-03', 'r125-development-parser-tests-v3.js', [],
    gnuCheck.name + '.json'), helper_sha256: checkerHash, accept: 5, reject: 52},
  {...run('r125-development-gate-parser-calibration-03', 'r125-development-gate-parser-tests-v3.js', [],
    'r125-development-parser-calibration-03.json'),
    helper_sha256: '394de8522929f0ea9d4bc76a746d903783e2ce1a18f3131f7cd99fab1c1cba41', accept: 10, reject: 39},
  {...run('r125-development-mutation-parser-calibration-04', 'r125-development-mutation-parser-tests-v4.js', [],
    'r125-development-source-snapshot-03.json'),
    helper_sha256: '608f5d428766e7984fbb085b9fafa0ba37633eceb4837a2fb605c110b4a06ec4', accept: 38, reject: 63},
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
assert.strictEqual(calibrations.reduce((sum, spec) => sum + spec.cases.length, 0), 207);

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
  'queue::dispatch_binding::control_release::tests::persistent::', ...M.p.preserved_tests,
  'queue::live::construction_primary::integration_tests::platform::tests::context_zero_check_preserves_every_byte_including_unaligned_partial_pages'];
const restoredKfdNames = kfd.names.filter(name => filters.some(filter => name.includes(filter)));
assert.strictEqual(restoredKfdNames.length, 47);
assert(M.p.allowed_tests.every(name => restoredKfdNames.includes(name)));
assert(M.p.mutations.every(mutation => restoredKfdNames.includes(mutation.test)));
const restored = [
  {name: 'r125-development-restored-persistent-v1', kind: 'kfd',
    command: [...M.p.test_command, '--', ...filters],
    predecessor: mutations.at(-1).restored + '.json',
    target: kfd.target, tests: restoredKfdNames, passed: 47, filtered: p.counts.kfd - 47},
  {name: 'r125-development-restored-runtime-v1', kind: 'runtime',
    command: ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline',
      '-p', 'fe2o3-runtime', '--all-features', '--lib'],
    predecessor: 'r125-development-restored-persistent-v1.json',
    target: runtime.target, tests: runtime.names, passed: p.counts.runtime, filtered: 0},
];
const names = ['r125-development-full-plan-v4.json', checker, gateChecker,
  'r125-development-mutations-v3.json', mutationChecker, 'r125-development-mutation-inventory-v3.js',
  'r125-development-mutation-plan-build-v3.js', 'r125-development-mutation-parser-tests-v4.js',
  'r125-development-mutation-patch-v3.js', 'r125-development-parser-tests-v3.js',
  'r125-development-gate-parser-tests-v3.js', 'r125-development-source-bundle-v3.js',
  'r125-development-source-bundle-v3.json', 'r125-development-plan-v4.js',
  'r125-development-evidence-v3.js', path.basename(__filename), linuxCalibration, 'r125-development-zero-scan-audit-v1.js', ...Object.keys(p.parser_inputs)];
assert.strictEqual(new Set(names).size, names.length);
const plan = {accepted: false,
  scope: 'R125 local CPU/test qualification only; native execution, aggregate retained memory, formal implementation correspondence and HIP/HSA performance acceptance remain separate.',
  source_parent: p.source_parent, source_map_sha256: p.source_map_sha256, source_identities: p.source_count,
  runner: {name: p.runner, sha256: p.runner_sha256},
  full_checker: checker, gate_checker: gateChecker, mutation_checker: mutationChecker,
  inputs: Object.fromEntries(names.sort().map(name => [name, C.hash(C.bytes(name))])),
  source_bundle: 'r125-development-source-bundle-v3.json', source_changed: 18, source_added: 3,
  source_plan_run: run('r125-development-mutation-plan-03', 'r125-development-mutation-plan-build-v3.js', [],
    'r125-development-zero-scan-audit-01.json'),
  source_snapshot: run('r125-development-source-snapshot-03', 'r125-development-source-bundle-v3.js', [],
    'r125-development-mutation-plan-03.json'),
  diagnostic_audit: diagnosticAudit, calibrations, helper_calibrations: 207, gnu_check: gnuCheck, full_check: fullCheck,
  gate_check: gateCheck, mutations, restored,
  collector: 'r125-development-qualification-collect-v2.js',
  collector_run: 'r125-development-qualification-collect-v2',
  collector_predecessor: restored.at(-1).name + '.json',
  superseded_attempts: p.superseded_attempts,
  exclusions: [...M.p.exclusions,
    'Historical failed, interrupted and intermediate-cohort runs remain nonqualifying retained diagnostics.',
    'The boxed-ledger measurements are inline type sizes, not aggregate-memory or maximum-stack proofs.',
    'Cold three-binding output admission, A1/A2, issue #182 and broad HIP/HSA parity remain incomplete.'],
};
assert.deepStrictEqual(C.identities(), C.map); C.stable();
const output = root + 'r125-development-qualification-plan-v2.json';
fs.writeFileSync(output, JSON.stringify(plan, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify({output, sha256: hash(fs.readFileSync(output)), accepted: false,
  source_map_sha256: p.source_map_sha256, helper_calibrations: 207,
  compiled_negatives_required: 18, restored_test_counts: restored.map(spec => spec.passed)}));
