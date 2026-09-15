const fs = require('fs');
const assert = require('assert');
const p = require('./r118-qualification-plan.js');
const e = require('./r118-qualification-evidence.js');
const c = require('./r118-qualification-collect.js');
const r = require('./r118-qualification-run.js');
const manifestName = 'r118-qualification-manifest.json';
const stat = name => fs.lstatSync(e.file(name));
function assertVacant(name, inspect = stat) {
  try {inspect(name);}
  catch (error) {if (error.code === 'ENOENT') return; throw error;}
  throw new Error('immutable output already exists: ' + name);
}
function entryOutputs(name, entry) {
  return (entry.kind === 'restoration' ? ['.json', '-source.json'] :
    ['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json'])
    .map(suffix => name.replace('.json', suffix));
}
function outputNames(entries) {
  return Object.entries(entries).flatMap(([name, entry]) => entryOutputs(name, entry))
    .concat(p.mutations.map(mutation => 'r118-qualification-failure-' + mutation.name + '.json'));
}
function launch(io = {...e, stat, baseline: c.checkBaseline, qualify: r.qualifyMutation, run: r.runEntry, log: console.log}) {
  const manifestBytes = io.bytes(manifestName);
  const manifest = JSON.parse(manifestBytes);
  const manifestHash = e.hash(manifestBytes);
  const base = io.baseline();
  c.validateManifest(manifest, base, io);
  const pins = [...manifest.historical_artifacts, ...manifest.helpers, manifest.launcher];
  const completed = new Map();
  const retain = (name, expected, json) => {
    const bytes = io.bytes(name);
    assert.deepStrictEqual(json ? JSON.parse(bytes) : bytes.toString(), expected, 'validated output bytes: ' + name);
    assert(!completed.has(name), 'one completed artifact identity');
    completed.set(name, e.hash(bytes));
  };
  const guard = (expected, outputs = []) => {
    assert.strictEqual(e.hash(io.bytes(manifestName)), manifestHash, 'unchanged campaign manifest');
    assert.deepStrictEqual(io.historicalNames(), manifest.historical_artifacts.map(item => item.name), 'unchanged historical membership');
    for (const pin of pins) assert.strictEqual(e.hash(io.bytes(pin.name)), pin.sha256, 'unchanged qualified input: ' + pin.name);
    for (const [name, digest] of completed) assert.strictEqual(e.hash(io.bytes(name)), digest, 'unchanged completed output: ' + name);
    assert.deepStrictEqual(io.identities(), expected, 'expected current source map');
    for (const output of outputs) assertVacant(output, io.stat);
  };
  const run = (name, expected, beforeSpawn = () => {}, outputs = entryOutputs(name + '.json', manifest.entries[name + '.json'])) => {
    guard(expected, outputs);
    const result = io.run(manifest, name, expected, () => {
      guard(expected, outputs);
      beforeSpawn();
    });
    retain(name + '.json', result.record, true);
    retain(name + '.log', result.log, false);
    retain(name + '-source.json', expected, true);
    retain(name + '-source-after.json', expected, true);
    return result;
  };
  guard(base.map, outputNames(manifest.entries));
  for (const [index, mutation] of p.mutations.entries()) {
    const names = ['r118-qualified-mut-' + mutation.name + '.json', 'r118-qualified-restoration-' + mutation.name + '.json'];
    const outputs = names.flatMap(name => entryOutputs(name, manifest.entries[name]))
      .concat('r118-qualification-failure-' + mutation.name + '.json');
    guard(base.map, outputs);
    const restoration = io.qualify({manifest, manifestHash, map: base.map, mutation,
      run: (name, expected, beforeSpawn) => run(name, expected, beforeSpawn, outputs)});
    retain(names[1], restoration, true);
    retain(names[1].replace('.json', '-source.json'), base.map, true);
    guard(base.map);
    io.log('PASS ' + (index + 1) + '/' + p.mutations.length + ': ' + mutation.name + '; exact source restored');
  }
  for (const focused of p.focused) {
    const result = run('r118-qualified-restored-' + focused[0], base.map);
    c.checkFocused(result, focused, base.allNames);
    guard(base.map);
    io.log('PASS restored ' + focused[0] + ': ' + focused[2]);
  }
  run('r118-qualified-collector-validation', base.map);
  guard(base.map);
  io.log('PASS closed collector validation; archive and independent review remain');
}
if (require.main === module) launch();
module.exports = {assertVacant, entryOutputs, outputNames, launch};
