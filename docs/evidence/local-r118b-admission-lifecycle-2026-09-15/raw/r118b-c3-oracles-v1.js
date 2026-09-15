const assert = require('assert');
const prior = require('./r118-mutations-c3-v1.js');
const lines = new Map([
  [67, 67], [197, 205], [230, 243], [176, 182], [189, 196],
  [175, 181], [88, 89], [77, 77], [89, 90], [310, 324],
  [306, 320], [317, 338], [414, 433], [406, 425], [431, 458], [318, 339],
]);
const mutations = prior.mutations.map(original => {
  assert(lines.has(original.oracle_line), original.name);
  const result = structuredClone(original);
  result.oracle_line = lines.get(original.oracle_line);
  if (original.name === 'c3-retain-old-waker') {
    assert.deepStrictEqual(original.expected, ['assertion failed: old.lock().unwrap().is_empty()']);
    result.expected = ['assertion failed: old_wakes.is_empty()'];
  }
  return result;
});
assert.strictEqual(mutations.length, 19);
module.exports = {
  tests: prior.tests, mutations,
  source_sha256: 'e729e1c0cfa7458fb8504f9bc1d18519ce59a5abe00be94a7c5aba22762fe7f9',
  source_map_sha256: '3aa1ab32e9e6e6ffd0474ec9055d3c29b02bf53e45129406617e50f0b3c15126',
};
