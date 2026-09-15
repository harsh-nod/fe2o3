// Exact compiler-path calibration; these transcript edits are not compiled runtime mutations.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/', helper = root + 'r122-development-check-v3.js';
const helperHash = '9029a09437783161dd692ebb46d149f9bd10bc7b55b747fc15d4946a1ee20fec';
assert.strictEqual(crypto.createHash('sha256').update(fs.readFileSync(helper)).digest('hex'), helperHash);
const C = require(helper);
C.pin(helper, helperHash); C.bytes(__filename);
const log = C.pin('r122-development-final-negative-01-preflight-custody-v1.log', '4a628a6535255e47d1d43ec0b8c2fabcc3f8cbab919b9bcdb246b1e2d72dfcad').toString();
const mutation = C.p.mutations[0];
const actual = 'crates/fe2o3-kfd/src/queue_live/construction_primary/../construction_auxiliary/integration_data_release_tests.rs:228:9';
const cases = [];
function good(name, run) { run(); cases.push({name, expected: 'accept'}); }
function bad(name, before, after) {
  assert.strictEqual(log.split(before).length, 2, 'one calibration target');
  assert.throws(() => C.assertNegative(log.replace(before, after), mutation),
    error => error.code === 'ERR_ASSERTION', name);
  cases.push({name, expected: 'reject'});
}
good('actual compiled preflight failure with exact module traversal', () => C.assertNegative(log, mutation));
bad('canonical spelling is not the observed compiler spelling', actual, mutation.oracle_location);
bad('alternate equivalent traversal is rejected', actual, actual.replace('construction_primary/../', './'));
bad('unexpected extra traversal is rejected', actual, actual.replace('construction_primary/../', 'other/../'));
bad('wrong assertion file is rejected', actual, actual.replace('integration_data_release_tests.rs', 'integration_insertion_tests.rs'));
bad('wrong assertion line is rejected', actual, actual.replace(':228:9', ':229:9'));
bad('wrong assertion column is rejected', actual, actual.replace(':228:9', ':228:10'));
bad('unrelated assertion diagnostic is rejected', mutation.oracle[0], 'assertion failed: unrelated_path_calibration');
assert.strictEqual(cases.length, 8);
assert.deepStrictEqual(C.identities(), C.map);
C.stable();
console.log(JSON.stringify({development_only: true, helper_calibration_only: true,
  runtime_mutation_count: 0, helper_sha256: helperHash, passed: cases.length, cases}));
