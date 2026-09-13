const fs = require('fs');
const path = require('path');
const assert = require('assert');
const p = require('./r113-qualification-plan.js');
const e = require('./r113-qualification-evidence.js');
const {checkBaseline} = require('./r113-qualification-collect.js');
const {map, helpers, previous} = checkBaseline();
const name = 'r113-qualification-manifest.json';
assert(!fs.existsSync(e.file(name)), 'manifest is immutable');
const originalSource = fs.readFileSync(path.join(e.repo, p.production), 'utf8');
const entries = {};
let predecessor = name;
const hashes = new Set();
for (const mutation of p.mutations) {
  assert(p.allowedMutationTests.includes(mutation.test));
  const original = fs.readFileSync(path.join(e.repo, mutation.path), 'utf8');
  assert.strictEqual(e.hash(original), map[mutation.path]);
  const mutated = e.mutatedSource(original, mutation);
  hashes.add(e.hash(mutated));
  const mutatedMap = {...map, [mutation.path]: e.hash(mutated)};
  const run = 'r113-qualified-mut-' + mutation.name + '.json';
  entries[run] = {kind: 'run', predecessor, source_map_sha256: e.hash(JSON.stringify(mutatedMap)),
    command: [...p.test, mutation.test, '--', '--exact'], returncode: 101};
  predecessor = run;
  const restoration = 'r113-qualified-restoration-' + mutation.name + '.json';
  entries[restoration] = {kind: 'restoration', predecessor, source_map_sha256: e.hash(JSON.stringify(map))};
  predecessor = restoration;
}
assert.strictEqual(hashes.size, 17);
for (const [kind, filter] of p.focused) {
  const run = 'r113-qualified-restored-' + kind + '.json';
  entries[run] = {kind: 'run', predecessor, source_map_sha256: e.hash(JSON.stringify(map)), command: [...p.test, filter], returncode: 0};
  predecessor = run;
}
entries['r113-qualified-collector-validation.json'] = {kind: 'run', predecessor,
  source_map_sha256: e.hash(JSON.stringify(map)), command: ['node', e.file('r113-qualification-collect.js'), '--check'], returncode: 0};
assert.strictEqual(Object.keys(entries).length, 37);
assert.deepStrictEqual(entries, e.expectedEntries(map, originalSource));
for (const [name, entry] of Object.entries(entries)) {
  const suffixes = entry.kind === 'restoration' ? ['.json', '-source.json'] : ['.json', '.log', '-source.json', '-source-after.json'];
  for (const suffix of suffixes) assert(!fs.existsSync(e.file(name.replace('.json', suffix))), 'fresh cohort output');
}
const historical = e.historicalNames();
const pins = historical.map(name => ({name, sha256: e.hash(e.bytes(name))}));
assert.deepStrictEqual(e.identities(), map);
const observation = e.sample();
const prior = e.read(previous);
e.ordered(prior.clock.source_verified, observation);
const manifest = {source_head: p.parent, source_map_sha256: e.hash(JSON.stringify(map)), helpers,
  original_source: {path: p.production, text: originalSource},
  historical_artifacts: pins, mutations: p.mutations, entries,
  clock: {contract: e.contract, error: null, kind: 'manifest',
    predecessor: {name: previous, sha256: e.hash(e.bytes(previous)), observation: prior.clock.source_verified},
    source_verified: observation}, recorded_at: new Date(observation.utc_ms).toISOString()};
fs.writeFileSync(e.file(name), JSON.stringify(manifest, null, 2) + '\n', {flag: 'wx'});
console.log('Pinned 37 cohort entries, 17 prospective mutations, ' + helpers.length + ' helpers and ' + pins.length + ' historical artifacts');
