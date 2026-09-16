// Derive exact production mutations and unchanged oracles without editing runtime source.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const checker = 'r125-development-check-v1.js';
const checkerHash = 'e37aab2693167424232e925bec843e3a187fcf7192add1f78244cf52ecbd8981';
const inventory = 'r125-development-mutation-inventory-v1.js';
const inventoryHash = 'ad89aa7c52400ee80dd8fac69e2b6a2f3e8c7566e6a77f1f9bbe81c44c232ecd';
assert.strictEqual(hash(fs.readFileSync(root + checker)), checkerHash);
assert.strictEqual(hash(fs.readFileSync(root + inventory)), inventoryHash);
const C = require(root + checker), I = require(root + inventory);
C.pin(checker, checkerHash); C.pin(inventory, inventoryHash); C.bytes(__filename);
assert.strictEqual(I.accepted, false);
assert.strictEqual(I.source_map_sha256, C.p.source_map_sha256);
assert.deepStrictEqual(C.identities(), C.map);
const baseline = C.accepted(C.p.full_runs.find(spec => spec.kind === 'gnu'));
const preserved = C.E.executables(baseline).filter(target =>
  target.kind === 'libtest' && target.name.endsWith('/fe2o3_kfd)'));
assert.strictEqual(preserved.length, 1);
assert.deepStrictEqual(I.preserved_tests, [I.tests.public]);
assert(I.preserved_tests.every(test => preserved[0].passing.includes(test)));
const allowedTests = [...new Set([...C.p.new_tests.kfd, ...I.preserved_tests])].sort();
const publicPath = I.oracleFiles.P;
const publicStart = '    fn prepared_persistent_compute_cancellation_restores_initialized_rebind_input() {';
const publicEnd = '\n    #[test]\n    fn single_uninitialized_write_bind_cancellation_preserves_uninitialized_custody() {';
function publicTest(source) {
  assert.strictEqual(source.split(publicStart).length, 2);
  assert.strictEqual(source.split(publicEnd).length, 2);
  const start = source.indexOf(publicStart), end = source.indexOf(publicEnd);
  assert(end > start);
  return source.slice(start, end);
}
assert.strictEqual(publicTest(C.bytes(C.repo + '/' + publicPath).toString()),
  publicTest(C.git(['show', C.p.accepted_commit + ':' + publicPath])), 'preserved public test body');
function oracle(location, fragments, following = false) {
  const match = /^([A-Z]):([1-9][0-9]*):([1-9][0-9]*)$/.exec(location);
  assert(match, 'declared oracle coordinates');
  const [, key, line, column] = match, file = I.oracleFiles[key];
  assert(file, 'declared oracle file');
  const source = C.bytes(C.repo + '/' + file).toString();
  assert.strictEqual(C.hash(source), I.oracleHashes[key]);
  assert.strictEqual(C.hash(source), C.map[file]);
  const token = source.split('\n')[Number(line) - 1].slice(Number(column) - 1);
  assert(following ? token.startsWith('unwrap()')
    : /^(assert!|assert_eq!|debug_assert!|panic!|unwrap\(\))/.test(token), 'oracle token');
  assert(Array.isArray(fragments) && fragments.length > 0 && fragments.every(value => typeof value === 'string' && value.length > 0));
  return {location: file + ':' + line + ':' + column, fragments};
}
const mutations = I.rows.map(([id, scopeKey, edits, testKey, location], index) => {
  const [file, start, end] = I.scopes[scopeKey], test = I.tests[testKey];
  assert(allowedTests.includes(test), 'new or explicitly preserved test');
  assert(!Object.values(I.oracleFiles).includes(file), 'mutation cannot change any oracle source');
  const original = C.bytes(C.repo + '/' + file).toString();
  assert.strictEqual(C.hash(original), C.map[file]);
  const patch = {path: file, scope: {start, end}, edits};
  const changed = C.E.core.mutatedSource(original, patch);
  const reverse = {...patch, edits: edits.toReversed().map(([from, to]) => [to, from])};
  assert.strictEqual(C.E.core.mutatedSource(changed, reverse), original);
  const map = {...C.map, [file]: C.hash(changed)};
  const item = {id, kind: 'kfd', test, patch, expected_file_sha256: C.hash(changed),
    expected_source_map_sha256: C.hash(JSON.stringify(map)),
    predecessor: index === 0 ? 'r125-development-gate-check-all-v1.json'
      : 'r125-development-restored-' + I.rows[index - 1][0] + '-v1.json',
    oracle: oracle(location, I.fragments[id.slice(0, 2)]), final_panic: 'primary'};
  const following = I.following_panic[id];
  if (following) {
    item.final_panic = 'caught_then_unwrap';
    item.following_panic = oracle(following.location, following.fragments, true);
  }
  return item;
});
assert.strictEqual(mutations.length, 18);
assert.strictEqual(new Set(mutations.map(item => item.id)).size, 18);
assert.strictEqual(new Set(mutations.map(item => item.expected_source_map_sha256)).size, 18);
assert.strictEqual(new Set(mutations.map(item => item.patch.path)).size, 4);
assert.strictEqual(mutations.filter(item => item.final_panic === 'caught_then_unwrap').length, 1);
const plan = {
  accepted: false,
  scope: 'Eighteen predeclared R125 CPU behavioral negatives; compile failures, aborts and unrelated assertions do not qualify.',
  source_plan: 'r125-development-full-plan-v2.json', source_plan_sha256: '7b4216028de5e3b004130b14870a0fe8765f205c927cac1b98616ca636db2454',
  checker, checker_sha256: checkerHash, inventory, inventory_sha256: inventoryHash,
  source_parent: C.p.source_parent, source_map_sha256: C.p.source_map_sha256, source_count: C.p.source_count,
  runner: C.p.runner, runner_sha256: C.p.runner_sha256,
  test_command: ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '--all-features', '-p', 'fe2o3-kfd', '--lib'],
  test_count: C.p.counts.kfd, allowed_tests: allowedTests, preserved_tests: I.preserved_tests,
  preserved_public_test_sha256: hash(publicTest(C.bytes(C.repo + '/' + publicPath).toString())),
  diagnostic_path_aliases: I.diagnostic_path_aliases, exclusions: I.exclusions, mutations,
};
assert.deepStrictEqual(C.identities(), C.map); C.stable();
const output = root + 'r125-development-mutations-v1.json';
fs.writeFileSync(output, JSON.stringify(plan, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify({output, sha256: hash(fs.readFileSync(output)), accepted: false,
  mutations_declared: mutations.length, runtime_mutations_executed: 0}));
