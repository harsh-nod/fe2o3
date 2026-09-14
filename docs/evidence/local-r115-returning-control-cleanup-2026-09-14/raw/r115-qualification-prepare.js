const fs = require('fs');
const path = require('path');
const assert = require('assert');
const p = require('./r115-qualification-plan.js');
const e = require('./r115-qualification-evidence.js');
const {checkBaseline} = require('./r115-qualification-collect.js');
const {map, helpers, previous} = checkBaseline();
const name = 'r115-qualification-manifest.json';
assert(!fs.existsSync(e.file(name)), 'manifest is immutable');
const originals = p.productionPaths.map(source => ({path: source,
  text: fs.readFileSync(path.join(e.repo, source), 'utf8')}));
e.originalSources(map, originals);
const entries = e.expectedEntries(map, originals);
assert.strictEqual(Object.keys(entries).length, 2 * p.mutations.length + p.focused.length + 1);
for (const [name, entry] of Object.entries(entries)) {
  const suffixes = entry.kind === 'restoration' ? ['.json', '-source.json'] : ['.json', '.log', '-source.json', '-source-after.json'];
  for (const suffix of suffixes) assert(!fs.existsSync(e.file(name.replace('.json', suffix))), 'fresh cohort output');
}
const pins = e.historicalNames().map(name => ({name, sha256: e.hash(e.bytes(name))}));
assert.deepStrictEqual(e.identities(), map);
const observation = e.sample();
const prior = e.read(previous);
e.ordered(prior.clock.source_verified, observation);
const manifest = {source_head: p.parent, source_map_sha256: e.hash(JSON.stringify(map)), helpers,
  original_sources: originals, historical_artifacts: pins, mutations: p.mutations, entries,
  clock: {contract: e.contract, error: null, kind: 'manifest',
    predecessor: {name: previous, sha256: e.hash(e.bytes(previous)), observation: prior.clock.source_verified},
    source_verified: observation}, recorded_at: new Date(observation.utc_ms).toISOString()};
fs.writeFileSync(e.file(name), JSON.stringify(manifest, null, 2) + '\n', {flag: 'wx'});
console.log('Pinned ' + Object.keys(entries).length + ' cohort entries, ' + p.mutations.length + ' prospective mutations, ' + helpers.length + ' helpers and ' + pins.length + ' historical artifacts');
