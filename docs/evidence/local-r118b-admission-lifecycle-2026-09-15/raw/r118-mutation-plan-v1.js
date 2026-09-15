// No mutation in this plan is accepted until its compiled oracle and restoration pass.
const contracts = [
  require('./r118-mutations-c1-v1.js'),
  require('./r118-mutations-c2-v1.js'),
  require('./r118-mutations-c3-v1.js'),
];
const mutations = contracts.flatMap(contract => contract.mutations);
module.exports = {
  parent: 'a07ec44309e214f2a8ef0e687e610c8e60a36224',
  sourceMapSha256: 'df2e27085ee920fdd07d2a6955503b1ecdcadc2d7c10c6fd4cec6768cecdadbb',
  sourceCount: 5692,
  newTests: contracts.flatMap(contract => Object.values(contract.tests)).sort(),
  mutationPaths: [...new Set(mutations.flatMap(mutation => mutation.patches.map(patch => patch.path)))].sort(),
  mutations,
  expected: {executions: 78, distinctSources: 74, production: 75, combinedDefense: 1, helperCalibration: 2},
  sharedSourceGroups: [
    ['c1-lookup-by-native-id', 'c1-lookup-by-reused-native-id'],
    ['c1-trust-cached-completion', 'c1-trust-released-cached-completion'],
    ['c1-wait-deadline-before-terminal-context', 'c1-wait-deadline-before-reserved-context'],
    ['c1-drain-expiry-before-terminal-context', 'c1-drain-expiry-before-reserved-context'],
  ].map(group => group.sort()).sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0),
};
