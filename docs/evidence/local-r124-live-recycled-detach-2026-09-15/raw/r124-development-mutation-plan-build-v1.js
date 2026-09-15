// Generate scoped mutation declarations without editing or executing runtime source.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const checker = 'r124-development-check-v1.js';
const checkerHash = '1caef52ee3c10a2d8efd49d82debadfbe227822fb575fc88ea32f99cd0bb82ad';
const inventory = 'r124-development-mutation-inventory-v1.js';
const inventoryHash = '63aded2e9e5b08cc85723716d23901d9a3a0e22189219ecefe2d6010c67db53a';
assert.strictEqual(hash(fs.readFileSync(root + checker)), checkerHash);
assert.strictEqual(hash(fs.readFileSync(root + inventory)), inventoryHash);
const C = require(root + checker), I = require(root + inventory);
C.pin(checker, checkerHash); C.pin(inventory, inventoryHash); C.bytes(__filename);
assert.strictEqual(I.accepted, false);
assert.strictEqual(I.source_map_sha256, C.p.source_map_sha256);
assert.deepStrictEqual(C.identities(), C.map);
function oracle(location, fragments, following = false) {
  const match = /^([CPR]):([1-9][0-9]*):([1-9][0-9]*)$/.exec(location);
  assert(match, 'declared oracle coordinates');
  const [, key, line, column] = match, file = I.oracleFiles[key];
  const source = C.bytes(C.repo + '/' + file).toString();
  assert.strictEqual(C.hash(source), I.oracleHashes[key]);
  assert.strictEqual(C.hash(source), C.map[file]);
  const token = source.split('\n')[Number(line) - 1].slice(Number(column) - 1);
  assert(following ? token.startsWith('unwrap()') : /^(assert!|assert_eq!|panic!)/.test(token), 'oracle token');
  assert(Array.isArray(fragments) && fragments.length > 0 && fragments.every(value => typeof value === 'string' && value.length > 0));
  return {location: file + ':' + line + ':' + column, fragments};
}
const mutations = I.rows.map(([id, scopeKey, from, to, testKey, location]) => {
  const [file, start, end] = I.scopes[scopeKey], test = I.tests[testKey];
  assert(C.p.new_tests.kfd.includes(test));
  const original = C.bytes(C.repo + '/' + file).toString();
  assert.strictEqual(C.hash(original), C.map[file]);
  const patch = {path: file, scope: {start, end}, edits: [[from, to]]};
  const changed = C.E.core.mutatedSource(original, patch);
  assert.strictEqual(C.E.core.mutatedSource(changed, {...patch, edits: [[to, from]]}), original);
  const map = {...C.map, [file]: C.hash(changed)};
  const item = {id, kind: 'kfd', test, patch, expected_file_sha256: C.hash(changed),
    expected_source_map_sha256: C.hash(JSON.stringify(map)),
    oracle: oracle(location, I.fragments[id.slice(0, 2)]), final_panic: 'primary'};
  const following = I.following_panic[id];
  if (following) {
    item.final_panic = 'caught_then_unwrap';
    item.following_panic = oracle(following.location, following.fragments, true);
  }
  return item;
});
assert.strictEqual(mutations.length, 19);
assert.strictEqual(new Set(mutations.map(item => item.id)).size, 19);
assert.strictEqual(new Set(mutations.map(item => item.expected_source_map_sha256)).size, 19);
assert.strictEqual(mutations.filter(item => item.final_panic === 'caught_then_unwrap').length, 1);
const plan = {
  accepted: false,
  scope: 'Nineteen predeclared R124 behavioral negatives; compile failures, aborts and unrelated assertions do not qualify.',
  source_plan: 'r124-development-full-plan-v1.json', source_plan_sha256: '7b56d255f63cc2c839cd2b16928d0ed4ba7db2b94f73d50baef9bb48697b24b3',
  checker, checker_sha256: checkerHash, inventory, inventory_sha256: inventoryHash,
  source_parent: C.p.source_parent, source_map_sha256: C.p.source_map_sha256, source_count: C.p.source_count,
  runner: C.p.runner, runner_sha256: C.p.runner_sha256,
  test_command: ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '--all-features', '-p', 'fe2o3-kfd', '--lib'],
  test_count: C.p.counts.kfd, diagnostic_path_aliases: I.diagnostic_path_aliases,
  excluded_source_only_guards: I.excluded_source_only_guards, mutations,
};
assert.deepStrictEqual(C.identities(), C.map); C.stable();
const output = root + 'r124-development-mutations-v1.json';
fs.writeFileSync(output, JSON.stringify(plan, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify({output, sha256: hash(fs.readFileSync(output)), accepted: false,
  mutations_declared: mutations.length, runtime_mutations_executed: 0}));
