const prior = require('./r119-integrated-qualification-plan-v1.js');
module.exports = {...prior, prerequisites: [prior.prerequisites[0],
  ['r119-integrated-musl-retry1', [...prior.fullTest, '--target', 'x86_64-unknown-linux-musl'], 'r119-integrated-musl-all.json']],
  rejectedTimeout: 'r119-integrated-musl-all'};
