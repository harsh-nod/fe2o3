const mutationPlan = require('./r118b-mutation-plan-v1.js');
const retained = require('./r118-qualification-plan.js');
const shared = new Set(mutationPlan.sharedSourceGroups.flat());
const mutationSourceGroups = [...mutationPlan.sharedSourceGroups,
  ...mutationPlan.mutations.filter(mutation => !shared.has(mutation.name)).map(mutation => [mutation.name])]
  .sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
const mutationHelpers = [
  'r118b-mutation-core-inputs-v1.json', 'r118b-mutation-core-tests-v1.js', 'r118b-mutation-core-v1.js',
  'r118b-mutation-plan-v1.js', 'r118b-mutations-c1-v1.js', 'r118b-mutations-c2-v1.js', 'r118b-mutations-c3-v1.js',
];
const sharedHelpers = ['r118b-run-v1.js', 'r118b-qualification-plan.js', 'r118b-qualification-evidence.js',
  'r118b-current-history-v1.js', 'r118b-current-history-inputs-v1.json', 'r118b-current-history-tests-v1.js',
  'r118b-prior-history-v1.js', 'r118b-prior-history-inputs-v1.json', 'r118b-prior-history-tests-v1.js',
  'r118b-origin-v2.json', 'r118b-history-support-v1.js', 'r118b-runner-inputs-v1.json', ...mutationHelpers];
module.exports = {
  ...mutationPlan,
  accepted: mutationPlan.parent,
  previous: retained.previous,
  fullTest: retained.fullTest,
  test: retained.test,
  baselineFullPassed: retained.baselineFullPassed,
  sourceDelta: retained.sourceDelta,
  addedSource: retained.addedSource,
  sourceGuard: null,
  retainedSourceGuard: null,
  allowedMutationTests: mutationPlan.newTests,
  uniqueMutationCount: mutationPlan.expected.distinctSources,
  mutationSourceGroups,
  prerequisites: [
    ['r118b-reviewed-gnu-all', retained.fullTest, 'r118b-mutation-core-contracts-v1.json'],
    ['r118b-reviewed-musl-all', [...retained.fullTest, '--target', 'x86_64-unknown-linux-musl'], 'r118b-prior-history-validation-v1.json'],
  ],
  focused: retained.focused,
  docRelocations: [],
  freezeContractCount: 57,
  freezeHelpers: [...sharedHelpers, 'r118b-source-gate.py', 'r118b-auxiliary-gates.py',
    'r118b-freeze.js', 'r118b-runner-tests.js', 'r118b-freeze-tests.js'],
  helpers: [...sharedHelpers, 'r118b-qualification-tests.js', 'r118b-qualification-prepare.js',
    'r118b-qualification-run.js', 'r118b-qualification-collect.js'],
};
