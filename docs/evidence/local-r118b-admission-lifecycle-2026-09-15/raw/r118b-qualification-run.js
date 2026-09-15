const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const p = require('./r118b-qualification-plan.js');
const e = require('./r118b-qualification-evidence.js');
const manifestName = 'r118b-qualification-manifest.json';
const suffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const sourceIO = {
  read: name => fs.readFileSync(path.join(e.repo, name)),
  write: (name, text) => fs.writeFileSync(path.join(e.repo, name), text),
  live: e.liveGroupMembers,
  identities: e.identities,
};
const writeArtifact = (name, value) => fs.writeFileSync(e.file(name), JSON.stringify(value, null, 2) + '\n', {flag: 'wx'});
function runEntry(manifest, name, expectedMap, beforeSpawn = () => {}) {
  const entry = manifest.entries[name + '.json'];
  assert(entry && entry.kind === 'run');
  assert.strictEqual(entry.source_map_sha256, e.hash(JSON.stringify(expectedMap)));
  for (const suffix of suffixes) assert(!fs.existsSync(e.file(name + suffix)), 'fresh run output');
  beforeSpawn();
  const result = cp.spawnSync(process.execPath, [e.file('r118b-run-v1.js'), name, entry.predecessor, ...entry.command],
    {cwd: e.repo, encoding: 'utf8', maxBuffer: 4 * 1024 * 1024});
  assert.ifError(result.error);
  assert.strictEqual(result.signal, null);
  assert.strictEqual(result.status, entry.returncode, (result.stdout + result.stderr).slice(-4000));
  return e.checkRun(name, entry.command, expectedMap, entry.predecessor, entry.returncode);
}
function bindTerminal(record, name, expected, entry, readArtifact, runnerDigest) {
  assert.strictEqual(record.source_head, p.parent);
  assert.strictEqual(record.cwd, e.repo);
  assert.deepStrictEqual(record.command, entry.command);
  assert.strictEqual(record.source_map_sha256, e.hash(JSON.stringify(expected)));
  assert.strictEqual(record.log, e.file(name + '.log'));
  assert.strictEqual(record.source, e.file(name + '-source.json'));
  assert.strictEqual(record.source_after, e.file(name + '-source-after.json'));
  assert.deepStrictEqual(record.runner, {name: 'r118b-run-v1.js', sha256: runnerDigest}, 'manifest-pinned terminal runner');
  assert.strictEqual(e.hash(readArtifact('r118b-run-v1.js')), runnerDigest, 'manifest-pinned live runner');
  assert.strictEqual(record.clock.contract, e.contract);
  const predecessor = readArtifact(entry.predecessor);
  assert.deepStrictEqual(record.clock.predecessor, {name: entry.predecessor, sha256: e.hash(predecessor),
    observation: JSON.parse(predecessor).clock.source_verified});
  assert.deepStrictEqual(JSON.parse(readArtifact(name + '-source.json')), expected);
  return record;
}
function qualifyMutation({manifest, manifestHash, map, mutation, io = sourceIO,
  readArtifact = e.bytes, write = writeArtifact, sample = e.sample,
  run = (name, expected, beforeSpawn) => runEntry(manifest, name, expected, beforeSpawn)}) {
  const files = e.mutationSources(map, manifest.original_sources, mutation);
  const expected = e.mutationMap(map, files);
  const name = 'r118b-qualified-mut-' + mutation.name;
  const entry = manifest.entries[name + '.json'];
  assert(entry && entry.kind === 'run');
  const runnerPins = manifest.helpers.filter(helper => helper.name === 'r118b-run-v1.js');
  assert.strictEqual(runnerPins.length, 1);
  const runnerDigest = runnerPins[0].sha256;
  assert(typeof runnerDigest === 'string' && /^[a-f0-9]{64}$/.test(runnerDigest));
  const manifestIdentity = {name: manifestName, sha256: manifestHash};
  const recovery = e.core.recoverySources(manifestIdentity, files);
  const state = {runnerAttempted: false, record: null, recovery: false, attempts: []};
  const errors = [];
  const note = (phase, error) => errors.push({phase, error: String(error)});
  let completed = null;
  let oraclePassed = false;
  let restoration = null;
  let terminalIdentity = null;
  let restoredMap = null;
  let start = null;
  let verified = null;
  try {
    assert.deepStrictEqual(io.identities(), map);
    e.core.applyOwnedMutation(files, io, state.attempts);
    assert.deepStrictEqual(io.identities(), expected);
    completed = run(name, expected, () => {state.runnerAttempted = true;});
    assert.strictEqual(state.runnerAttempted, true, 'execution attempt recorded before spawn');
    e.checkMutation(completed.log, mutation);
    oraclePassed = true;
  } catch (error) {note('execution-or-oracle', error);}
  finally {
    try {start = sample();} catch (error) {note('restoration-start-observation', error);}
    if (state.runnerAttempted) {
      try {
        const raw = readArtifact(name + '.json');
        terminalIdentity = {name: name + '.json', sha256: e.hash(raw)};
        state.record = bindTerminal(JSON.parse(raw), name, expected, entry, readArtifact, runnerDigest);
      } catch (error) {note('terminal-record', error);}
    }
    try {restoration = e.core.restoreOwnedMutation(map, files, state, io);}
    catch (error) {note('restoration', error);}
    try {restoredMap = io.identities();} catch (error) {note('restored-source-observation', error);}
    try {verified = sample();} catch (error) {note('restoration-finish-observation', error);}
  }
  let record = null;
  if (errors.length === 0 && oraclePassed && completed && state.record) {
    try {
      assert.deepStrictEqual(state.record, completed.record, 'same authenticated terminal record');
      record = {source_head: p.parent, manifest_sha256: manifestHash,
        source_map_sha256: e.hash(JSON.stringify(map)), source_unchanged: true, source_identities: p.sourceCount,
        mutated_sources: files.map(file => ({path: file.path, sha256: file.changed_sha256})),
        application_attempts: state.attempts, restoration,
        clock: {contract: e.contract, error: null, kind: 'restoration', start, source_verified: verified,
          predecessor: {...terminalIdentity, observation: state.record.clock.source_verified}}};
      e.checkRestoration(record, files, map, manifestHash, terminalIdentity, state.record, restoredMap);
      const restoredName = 'r118b-qualified-restoration-' + mutation.name;
      write(restoredName + '-source.json', restoredMap);
      write(restoredName + '.json', record);
    } catch (error) {note('qualification-or-artifact', error);}
  }
  if (errors.length || !record) {
    const diagnostic = {accepted: false, mutation: mutation.name, manifest: manifestIdentity,
      expected_mutation_map_sha256: e.hash(JSON.stringify(expected)),
      runner_attempted: state.runnerAttempted, application_attempts: state.attempts,
      terminal_record: terminalIdentity, authenticated_terminal: state.record !== null,
      oracle_passed: oraclePassed, restoration, recovery_record: recovery, errors};
    try {write('r118b-qualification-failure-' + mutation.name + '.json', diagnostic);}
    catch (error) {note('failure-artifact', error);}
    throw new Error('Mutation qualification stopped: ' + mutation.name + '; ' + JSON.stringify(diagnostic));
  }
  return record;
}
function main() {
  const manifest = e.read(manifestName);
  const manifestHash = e.hash(e.bytes(manifestName));
  const map = e.read('r118b-frozen-source.json');
  assert.strictEqual(manifest.source_head, p.parent);
  assert.strictEqual(manifest.source_map_sha256, e.hash(JSON.stringify(map)));
  assert.deepStrictEqual(manifest.mutations, p.mutations);
  e.originalSources(map, manifest.original_sources);
  assert.deepStrictEqual(manifest.entries, e.expectedEntries(map, manifest.original_sources));
  assert.deepStrictEqual(manifest.helpers.map(helper => helper.name), p.helpers);
  for (const helper of manifest.helpers) assert.strictEqual(e.hash(e.bytes(helper.name)), helper.sha256);
  assert.deepStrictEqual(e.identities(), map);
  for (const [index, mutation] of p.mutations.entries()) {
    for (const helper of manifest.helpers) assert.strictEqual(e.hash(e.bytes(helper.name)), helper.sha256);
    qualifyMutation({manifest, manifestHash, map, mutation});
    console.log('PASS ' + (index + 1) + '/' + p.mutations.length + ': ' + mutation.name + '; exact source restored');
  }
  const all = e.passing(e.bytes('r118b-reviewed-gnu-all.log').toString());
  for (const focused of p.focused) {
    const [kind, filter, count, exact] = focused;
    const result = runEntry(manifest, 'r118b-qualified-restored-' + kind, map);
    e.assertPassing(result.log);
    const expected = all.filter(name => exact ? name === filter : name.startsWith(filter));
    assert.strictEqual(expected.length, count);
    assert.deepStrictEqual(e.passing(result.log), expected);
    assert.deepStrictEqual(e.totals(result.log), {harnesses: 1, passed: count, failed: 0, ignored: 0});
    console.log('PASS restored ' + kind + ': ' + count);
  }
  runEntry(manifest, 'r118b-qualified-collector-validation', map);
  console.log('PASS closed collector validation; archive and independent review remain');
}
if (require.main === module) main();
module.exports = {bindTerminal, runEntry, qualifyMutation, main};
