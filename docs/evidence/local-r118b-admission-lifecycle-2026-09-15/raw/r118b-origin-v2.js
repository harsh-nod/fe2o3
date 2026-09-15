const fs = require('fs');
const path = require('path');
const assert = require('assert');
const e = require('./r118-qualification-evidence.js');
const root = '/home/harsh/.codex-tmp';
const read = name => fs.readFileSync(path.join(root, name));
const pin = name => ({name, sha256: e.hash(read(name))});
const sourcePath = 'crates/fe2o3-runtime/src/async_engine/tests/owned_tests/preparation_tests/completion_tests.rs';
const oldMap = JSON.parse(read('r118-frozen-source.json'));
const current = e.identities();
assert.strictEqual(e.hash(JSON.stringify(oldMap)), 'df2e27085ee920fdd07d2a6955503b1ecdcadc2d7c10c6fd4cec6768cecdadbb');
assert.strictEqual(Object.keys(current).length, 5692);
assert.deepStrictEqual(Object.keys(current), Object.keys(oldMap));
assert.deepStrictEqual(Object.keys(current).filter(name => current[name] !== oldMap[name]), [sourcePath]);
assert.strictEqual(e.hash(read('r118b-prior-completion-tests.rs')), oldMap[sourcePath]);
assert.strictEqual(e.hash(read('r118-run-v1.js')), 'c2af8cce7c2f79fe53b16a8219fa01e4b87b9fb3e0a910f108e47612798a5c53');
assert.strictEqual(read('r118b-run-v1.js').toString(), read('r118-run-v1.js').toString().replaceAll('r118-', 'r118b-'));
const oldManifest = JSON.parse(read('r118-qualification-manifest.json'));
const historicalNames = [...new Set([
  ...oldManifest.historical_artifacts.map(item => item.name),
  ...fs.readdirSync(root).filter(name => /^r118-.*\.(json|log|js|py|md)$/.test(name)),
])].sort();
assert.strictEqual(historicalNames.length, 735);
const historical = historicalNames.map(pin);
for (const item of [...oldManifest.historical_artifacts, ...oldManifest.helpers, oldManifest.launcher]) {
  assert.strictEqual(e.hash(read(item.name)), item.sha256, item.name);
}
for (const name of ['r118b-format-source.json', 'r118b-format-source-after.json']) {
  assert.deepStrictEqual(JSON.parse(read(name)), current);
}
const initialName = 'r118b-initial-completion-tests.rs';
const initialBytes = fs.readFileSync(path.join(e.repo, sourcePath));
assert.strictEqual(e.hash(initialBytes), current[sourcePath]);
const mapName = 'r118b-initial-source.json';
for (const name of [initialName, mapName, 'r118b-origin-v2.json']) {
  assert(!fs.existsSync(path.join(root, name)), 'fresh origin output: ' + name);
}
fs.writeFileSync(path.join(root, initialName), initialBytes, {flag: 'wx'});
fs.writeFileSync(path.join(root, mapName), JSON.stringify(current, null, 2) + '\n', {flag: 'wx'});
const observation = {
  utc_ms: Date.now(), monotonic_ns: process.hrtime.bigint().toString(),
  boot_id: fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim(),
  clock_source: 'node-process-hrtime-linux-monotonic',
};
const record = {
  accepted_parent: 'a07ec44309e214f2a8ef0e687e610c8e60a36224',
  scope: 'R118 test-only unwind correction; stopped history is not acceptance',
  prior_source_map_sha256: e.hash(JSON.stringify(oldMap)),
  initial_source_map_sha256: e.hash(JSON.stringify(current)),
  source_count: 5692, changed_source: [sourcePath],
  prior_campaign: {accepted: false, qualified_negatives: 72, rejected_execution: 73, unexecuted: [74, 75, 76, 77, 78]},
  historical_artifacts: historical,
  preliminary_origin_failure: {helper: 'r118b-origin-v1.js', expected: 735, observed: 652,
    reason: 'prefix-only inventory omitted 83 manifest-pinned C1/C2/C3 historical names',
    raw_record_retained: false, outputs_created: false},
  format_precedes_origin: true,
  explicit_inputs: ['r118b-prior-completion-tests.rs', initialName, mapName, 'r118b-run-v1.js',
    'r118b-origin-v1.js', 'r118b-origin-v2.js', 'r118b-format.json', 'r118b-format.log',
    'r118b-format-source.json', 'r118b-format-source-after.json'].map(pin),
  observation,
};
assert.deepStrictEqual(e.identities(), current);
for (const item of [...historical, ...record.explicit_inputs]) assert.strictEqual(e.hash(read(item.name)), item.sha256);
fs.writeFileSync(path.join(root, 'r118b-origin-v2.json'), JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify({historical_artifacts: historical.length, source_count: record.source_count,
  initial_source_map_sha256: record.initial_source_map_sha256, origin_sha256: e.hash(read('r118b-origin-v2.json'))}));
