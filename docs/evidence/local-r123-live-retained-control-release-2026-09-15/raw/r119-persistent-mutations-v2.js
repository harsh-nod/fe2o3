const prior = require('./r119-persistent-mutations-v1.js');

// The combined test reaches projection failure before actual commit exhaustion.
const rename = name => name === 'persistent-unroot-active-control-actual-commit'
  ? 'persistent-unroot-active-control-projection-commit' : name;

module.exports = {
  ...prior,
  mutations: prior.mutations.map(mutation => ({...mutation, name: rename(mutation.name)})),
  sharedSourceGroups: prior.sharedSourceGroups.map(group => group.map(rename)),
};
