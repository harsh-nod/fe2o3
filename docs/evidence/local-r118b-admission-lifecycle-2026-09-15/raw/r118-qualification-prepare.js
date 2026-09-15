const fs = require('fs');
const path = require('path');
const assert = require('assert');
const p = require('./r118-qualification-plan.js');
const e = require('./r118-qualification-evidence.js');
const c = require('./r118-qualification-collect.js');
const {assertVacant, outputNames} = require('./r118-qualification-launch-v1.js');
function prepare(io = {...e, baseline: c.checkBaseline,
  source: name => fs.readFileSync(path.join(e.repo, name), 'utf8'),
  stat: name => fs.lstatSync(e.file(name)),
  write: (name, value) => fs.writeFileSync(e.file(name), JSON.stringify(value, null, 2) + '\n', {flag: 'wx'})}) {
  const capturedNames = io.historicalNames();
  const captured = new Map(capturedNames.map(name => [name, Buffer.from(io.bytes(name))]));
  const {map, helpers, launcher, previous} = io.baseline();
  assert.strictEqual(e.hash(JSON.stringify(map)), p.sourceMapSha256);
  assert.strictEqual(Object.keys(map).length, p.sourceCount);
  const name = 'r118-qualification-manifest.json';
  assertVacant(name, io.stat);
  const originals = p.mutationPaths.map(path => ({path, text: io.source(path)}));
  e.originalSources(map, originals);
  const entries = e.expectedEntries(map, originals);
  const outputs = [name, ...outputNames(entries)];
  for (const output of outputs) assertVacant(output, io.stat);
  const pins = capturedNames.map(name => ({name, sha256: e.hash(captured.get(name))}));
  assert.deepStrictEqual(io.identities(), map);
  const observation = io.sample();
  const priorBytes = io.bytes(previous);
  const prior = JSON.parse(priorBytes);
  e.ordered(prior.clock.source_verified, observation);
  assert.deepStrictEqual(io.historicalNames(), capturedNames, 'unchanged historical membership');
  for (const pin of pins) assert.strictEqual(e.hash(io.bytes(pin.name)), pin.sha256, 'unchanged manifest input');
  for (const helper of [...helpers, launcher]) assert.strictEqual(e.hash(io.bytes(helper.name)), helper.sha256, 'qualified helper remains unchanged');
  assert.strictEqual(e.hash(io.bytes(previous)), e.hash(priorBytes), 'unchanged manifest predecessor');
  assert.deepStrictEqual(io.identities(), map);
  for (const output of outputs) assertVacant(output, io.stat);
  const manifest = {source_head: p.parent, source_map_sha256: e.hash(JSON.stringify(map)), helpers, launcher,
    original_sources: originals, historical_artifacts: pins, mutations: p.mutations, entries,
    clock: {contract: e.contract, error: null, kind: 'manifest', source_verified: observation,
      predecessor: {name: previous, sha256: e.hash(priorBytes), observation: prior.clock.source_verified}},
    recorded_at: new Date(observation.utc_ms).toISOString()};
  io.write(name, manifest);
  return manifest;
}
if (require.main === module) {
  const manifest = prepare();
  console.log('Pinned ' + Object.keys(manifest.entries).length + ' entries, ' + p.mutations.length + ' prospective mutations and qualified launcher');
}
module.exports = {prepare};
