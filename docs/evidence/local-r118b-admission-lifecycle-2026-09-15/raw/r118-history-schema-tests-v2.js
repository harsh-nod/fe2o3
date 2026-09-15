const assert = require('node:assert/strict');
const fs = require('node:fs');
const vm = require('node:vm');
const {test} = require('node:test');
const root = '/home/harsh/.codex-tmp/';
const source = fs.readFileSync(root + 'r118-history-v2.js', 'utf8');
const profile = JSON.parse(fs.readFileSync(root + 'r118-history-plan-v1.json', 'utf8'));
const acceptedInventory = new Error('inventory checked; stop before historical evidence validation');
const moduleObject = {exports: {}};
const load = vm.runInThisContext('(function(require, module, exports) {\n' + source + '\n})', {
  filename: root + 'r118-history-v2.js',
});
load(name => {
  if (name === 'assert') return require('assert');
  if (name === './r118-qualification-plan.js') return {history: profile};
  if (name === './r118-qualification-evidence.js') return {hash() {throw acceptedInventory;}};
  throw new Error('unexpected dependency: ' + name);
}, moduleObject, moduleObject.exports);
const {validateMap, checkHistory} = moduleObject.exports;
const digest = 'a'.repeat(64);
const descriptor = '100644 ' + 'b'.repeat(40) + ' 0';
const sparseMap = () => Object.fromEntries([
  ...Array.from({length: 5299}, (_, index) => ['source/' + String(index).padStart(4, '0'), {sha256: digest}]),
  ...Array.from({length: 381}, (_, index) => ['sparse/' + String(index).padStart(4, '0'), {unmaterialized_index_entry: descriptor}]),
].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0));
const sparseSpec = {count: 5680, schema: 'sparse-v1'};
const rejectInventory = candidate => assert.throws(() => checkHistory({}, {}, candidate), error => {
  assert.notStrictEqual(error, acceptedInventory, 'rejected before evidence validation');
  assert.strictEqual(error.code, 'ERR_ASSERTION');
  return true;
});

test('accept flat source hashes', () => {
  validateMap({'source/a': digest}, {count: 1, schema: 'sha256-v1'});
});
test('accept the typed materialized and sparse roster', () => {
  validateMap(sparseMap(), sparseSpec);
});
test('reject an unknown schema even for an empty map', () => {
  assert.throws(() => validateMap({}, {count: 0, schema: 'unknown'}), /known source map schema/);
});
test('reject a coercible array as a flat hash', () => {
  assert.throws(() => validateMap({'source/a': [digest]}, {count: 1, schema: 'sha256-v1'}), /flat source hash/);
});
test('reject a coercible array as a typed source hash', () => {
  const map = sparseMap();
  map['source/0000'].sha256 = [digest];
  assert.throws(() => validateMap(map, sparseSpec), /typed source hash/);
});
test('reject a coercible array as an index descriptor', () => {
  const map = sparseMap();
  map['sparse/0000'].unmaterialized_index_entry = [descriptor];
  assert.throws(() => validateMap(map, sparseSpec), /stage-zero source descriptor/);
});
test('reject multiple identity representations in one entry', () => {
  const map = sparseMap();
  map['source/0000'].unmaterialized_index_entry = descriptor;
  assert.throws(() => validateMap(map, sparseSpec), /one source identity representation/);
});
test('reject a changed sparse roster despite an unchanged total count', () => {
  const map = sparseMap();
  map['sparse/0000'] = {sha256: digest};
  assert.throws(() => validateMap(map, sparseSpec), {code: 'ERR_ASSERTION'});
});
test('accept canonical artifact membership without executing evidence dependencies', () => {
  assert.throws(() => checkHistory({}, {}, profile), error => error === acceptedInventory);
});
test('reject replacement of each required auxiliary artifact', () => {
  const runArtifacts = new Set(Object.keys(profile.runs).flatMap(name =>
    ['.json', '.log', '-source.json', '-source-after.json'].map(suffix => name + suffix)));
  const extras = profile.artifacts.filter(item => !runArtifacts.has(item.name));
  assert.strictEqual(extras.length, 18);
  for (const extra of extras) {
    const candidate = structuredClone(profile);
    candidate.artifacts.find(item => item.name === extra.name).name = 'replacement-artifact';
    rejectInventory(candidate);
  }
});
test('reject replacement of a raw historical run artifact', () => {
  const candidate = structuredClone(profile);
  candidate.artifacts.find(item => item.name === 'c1-initial-tests.log').name = 'replacement-log';
  rejectInventory(candidate);
});
test('reject duplicate artifact names', () => {
  const candidate = structuredClone(profile);
  candidate.artifacts[0].name = candidate.artifacts[1].name;
  rejectInventory(candidate);
});
test('reject changed handoff declarations even when artifact membership is retained', () => {
  const candidate = structuredClone(profile);
  candidate.handoffs[0] = 'replacement-handoff.md';
  rejectInventory(candidate);
});
