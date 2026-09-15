const mutationPlan = require('./r118-mutation-plan-v1.js');
const history = require('./r118-history-plan-v1.json');
const test = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-runtime', '--all-features', '--lib'];
const fullTest = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '--no-fail-fast',
  ...['fe2o3-completion', 'fe2o3-runtime-model', 'fe2o3-resource-accounting', 'fe2o3-kfd', 'fe2o3-runtime'].flatMap(name => ['-p', name]),
  '--all-features', '--all-targets'];
const shared = new Set(mutationPlan.sharedSourceGroups.flat());
const mutationSourceGroups = [...mutationPlan.sharedSourceGroups,
  ...mutationPlan.mutations.filter(mutation => !shared.has(mutation.name)).map(mutation => [mutation.name])]
  .sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
const immutableMutationHelpers = [
  'r118-mutation-core-inputs-v1.json', 'r118-mutation-core-tests-v1.js', 'r118-mutation-core-v1.js',
  'r118-mutation-plan-v1.js', 'r118-mutations-c1-v1.js', 'r118-mutations-c2-v1.js', 'r118-mutations-c3-v1.js',
];
const sharedHelpers = ['r118-run-v1.js', 'r118-qualification-plan.js', 'r118-qualification-evidence.js',
  'r118-history-plan-v1.json', 'r118-history-v3.js', 'r118-history-support-v1.js',
  'r118-c1-index-corroboration-v1.json', ...immutableMutationHelpers];
module.exports = {
  ...mutationPlan, history,
  accepted: mutationPlan.parent,
  previous: 'docs/evidence/local-r117-detached-persistent-control-cleanup-2026-09-14/',
  fullTest, test, baselineFullPassed: 2762,
  sourceDelta: Object.values(history.cohortSources).flat().sort(),
  addedSource: [
    'crates/fe2o3-runtime/src/async_engine/tests/owned_tests/preparation_tests/completion_tests.rs',
    'crates/fe2o3-runtime/src/authorized_execution/tests/generated_identity.rs',
    'crates/fe2o3-runtime/src/context/tests/submission_identity_tests.rs',
  ],
  sourceGuard: null, retainedSourceGuard: null,
  allowedMutationTests: mutationPlan.newTests,
  uniqueMutationCount: mutationPlan.expected.distinctSources, mutationSourceGroups,
  prerequisites: [
    ['r118-reviewed-gnu-all', fullTest, 'r118-reviewed-clippy.json'],
    ['r118-reviewed-musl-all', [...fullTest, '--target', 'x86_64-unknown-linux-musl'], 'r118-reviewed-gnu-all.json'],
  ],
  focused: [
    ['identity', 'context::tests::submission_identity_tests::', 9, false],
    ['descriptor', 'authorized_execution::tests::generated_identity::', 4, false],
    ['completion', 'async_engine::tests::owned_tests::preparation_tests::completion_tests::', 5, false],
    ['reply', 'async_engine::tests::owned_tests::control_tests::completion_tests::co1_existing_reply_gate_is_sticky_and_preserves_credit', 1, true],
    ['adoption', 'async_engine::tests::owned_tests::preparation_tests::adoption_tests::', 15, false],
    ['reservation', 'async_engine::tests::owned_tests::preparation_tests::reservation_tests::', 14, false],
  ],
  docRelocations: [],
  freezeContractCount: 50,
  freezeHelpers: [...sharedHelpers, 'r118-source-gate.py', 'r118-auxiliary-gates.py',
    'r118-freeze.js', 'r118-runner-tests.js', 'r118-freeze-tests.js'],
  helpers: [...sharedHelpers, 'r118-qualification-tests.js', 'r118-qualification-prepare.js',
    'r118-qualification-run.js', 'r118-qualification-collect.js'],
};
