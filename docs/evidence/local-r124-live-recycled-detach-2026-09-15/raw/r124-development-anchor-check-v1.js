// Validate completed development anchors without claiming unexecuted full runs.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const helper = root + 'r124-development-check-v1.js';
const helperHash = '1caef52ee3c10a2d8efd49d82debadfbe227822fb575fc88ea32f99cd0bb82ad';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
assert.strictEqual(hash(fs.readFileSync(helper)), helperHash);
const C = require(helper);
C.pin(helper, helperHash);
C.bytes(__filename);
C.anchors();
const format = C.completed('r124-development-format-02',
  ['cargo', '+nightly-2026-04-03', 'fmt', '--all', '--', '--check'],
  'r124-development-format-01.json', C.map, 0, 'r124-development-run-v1.js');
assert.strictEqual(format.log, '');
assert.deepStrictEqual(C.identities(), C.map);
C.stable();
console.log(JSON.stringify({development_only: true, packet_accepted: false,
  source_map_sha256: C.p.source_map_sha256, exact_regression_tests: C.p.anchor_test_count,
  strict_clippy: true, format: true, full_runs_executed: 0, runtime_mutations_executed: 0,
  helper_sha256: helperHash, artifacts: C.artifacts()}));
