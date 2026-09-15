const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const e = require('./r118-qualification-evidence.js');
const core = require('./r118-mutation-core-v1.js');
const oldPlan = require('./r118-qualification-plan.js');
const revised = require('./r118b-c3-oracles-v1.js');
const contract = 'r118b-raw-utc-boot-monotonic-v1';
const runner = 'r118b-run-v1.js';
const priorName = 'r118b-clippy.json';
const manifestName = 'r118b-retirement-regression-inputs-v1.json';
const runName = 'r118b-retirement-regression';
const restorationName = 'r118b-retirement-regression-restoration.json';
const map = e.read('r118b-initial-source.json');
const origin = e.read('r118b-origin-v2.json');
const mutation = revised.mutations.find(item => item.name === 'c3-continue-retirement-after-failure');
const originalMutation = oldPlan.mutations.find(item => item.name === mutation.name);
const originalSources = e.read('r118-qualification-manifest.json').original_sources;
const files = core.mutationSources(map, originalSources, mutation, oldPlan.mutationPaths);
const changed = core.mutationMap(map, files);
const command = [...oldPlan.test, mutation.test, '--', '--exact', '--test-threads=1'];
const context = expected => ({head: oldPlan.parent, cwd: e.repo, contract, runner, after: expected});
const pin = name => ({name, sha256: e.hash(e.bytes(name))});
function checkRecord(record, name, command, expected, previous, code) {
  e.checkRecord(record, name, command, expected, previous, code, e, context(expected));
}
function vacant(name) {
  try {fs.lstatSync(e.file(name));}
  catch (error) {if (error.code === 'ENOENT') return; throw error;}
  throw new Error('immutable output already exists: ' + name);
}
assert.strictEqual(e.hash(JSON.stringify(map)), revised.source_map_sha256);
assert.deepStrictEqual(e.identities(), map);
assert.strictEqual(map[mutation.oracle_path], revised.source_sha256);
assert.strictEqual(mutation.oracle_line, 338);
assert.strictEqual(originalMutation.oracle_line, 317);
assert.throws(() => e.checkMutation(e.bytes('r118-qualified-mut-' + mutation.name + '.log').toString(), originalMutation), /one named failure/);
const prior = e.read(priorName);
checkRecord(prior, 'r118b-clippy', ['cargo', '+nightly-2026-04-03', 'clippy', '--locked', '--offline',
  '-p', 'fe2o3-runtime', '--all-features', '--all-targets', '--', '-D', 'warnings'], map, 'r118b-runtime.json', 0);
const inputNames = [...new Set([...origin.historical_artifacts.map(item => item.name),
  ...origin.explicit_inputs.map(item => item.name), 'r118b-origin-v2.json',
  'r118b-c3-oracles-v1.js', 'r118b-retirement-regression-v1.js',
  ...['r118b-focused', 'r118b-runtime', 'r118b-clippy'].flatMap(name =>
    ['.json', '.log', '-source.json', '-source-after.json'].map(suffix => name + suffix))])].sort();
for (const item of [...origin.historical_artifacts, ...origin.explicit_inputs]) {
  assert.strictEqual(e.hash(e.bytes(item.name)), item.sha256);
}
const inputs = inputNames.map(pin);
const runnerPin = inputs.find(input => input.name === runner);
assert(runnerPin);
const outputs = [manifestName, restorationName, restorationName.replace('.json', '-source.json'),
  ...['.json', '.log', '-source.json', '-source-after.json'].map(suffix => runName + suffix)];
for (const name of outputs) vacant(name);
const observed = e.sample();
e.ordered(prior.clock.source_verified, observed);
const manifest = {
  source_head: oldPlan.parent, source_map_sha256: revised.source_map_sha256,
  preliminary: true, mutation, original_sources: originalSources, inputs,
  clock: {contract, kind: 'regression-inputs', error: null,
    predecessor: {name: priorName, sha256: e.hash(e.bytes(priorName)), observation: prior.clock.source_verified},
    source_verified: observed},
};
fs.writeFileSync(e.file(manifestName), JSON.stringify(manifest, null, 2) + '\n', {flag: 'wx'});
const manifestHash = e.hash(e.bytes(manifestName));
function guard(expected) {
  assert.strictEqual(e.hash(e.bytes(manifestName)), manifestHash);
  for (const item of inputs) assert.strictEqual(e.hash(e.bytes(item.name)), item.sha256, item.name);
  assert.deepStrictEqual(e.identities(), expected);
}
const attempts = [];
let attempted = false;
let terminal = null;
let terminalIdentity = null;
let oraclePassed = false;
let failure = null;
try {
  guard(map);
  core.applyOwnedMutation(files, {
    read: name => fs.readFileSync(path.join(e.repo, name)),
    write: (name, text) => fs.writeFileSync(path.join(e.repo, name), text),
  }, attempts);
  guard(changed);
  for (const name of outputs.filter(name => name !== manifestName)) vacant(name);
  attempted = true;
  const child = cp.spawnSync(process.execPath, [e.file(runner), runName, manifestName, ...command], {
    cwd: e.repo, encoding: 'utf8', maxBuffer: 16 * 1024 * 1024,
  });
  const terminalBytes = e.bytes(runName + '.json');
  const record = JSON.parse(terminalBytes);
  assert(Number.isInteger(record.returncode));
  assert.deepStrictEqual(record.runner, runnerPin);
  assert.strictEqual(e.hash(e.bytes(runner)), runnerPin.sha256);
  checkRecord(record, runName, command, changed, manifestName, record.returncode);
  assert.strictEqual(e.hash(e.bytes(runner)), runnerPin.sha256);
  terminal = record;
  terminalIdentity = {name: runName + '.json', sha256: e.hash(terminalBytes)};
  assert.strictEqual(child.error, undefined);
  assert.strictEqual(child.signal, null);
  assert.strictEqual(child.status, 101);
  assert.strictEqual(record.returncode, 101);
  e.checkMutation(e.bytes(runName + '.log').toString(), mutation);
  oraclePassed = true;
} catch (error) {
  failure = String(error);
} finally {
  let clockError = null;
  let started = null;
  try {started = e.sample();}
  catch (error) {clockError = String(error);}
  const restoration = core.restoreOwnedMutation(map, files, {
    runnerAttempted: attempted, recovery: false, record: terminal, attempts,
  }, {
    read: name => fs.readFileSync(path.join(e.repo, name)),
    write: (name, text) => fs.writeFileSync(path.join(e.repo, name), text),
    live: e.liveGroupMembers,
    identities: e.identities,
  });
  const after = e.identities();
  let verified = null;
  try {
    verified = e.sample();
    e.ordered(terminal?.clock.source_verified ?? observed, started);
    e.ordered(started, verified);
  } catch (error) {clockError ??= String(error);}
  const result = {
    source_head: oldPlan.parent, source_map_sha256: e.hash(JSON.stringify(after)),
    preliminary: true, manifest_sha256: manifestHash, oracle_passed: oraclePassed,
    terminal_authenticated: terminal !== null, terminal_identity: terminalIdentity,
    runner_pin: runnerPin, application_attempts: attempts,
    source_unchanged: e.hash(JSON.stringify(after)) === revised.source_map_sha256,
    source_identities: Object.keys(after).length, restoration, failure,
    clock: {contract, kind: 'regression-restoration', error: clockError,
      predecessor: terminal === null ? null : {...terminalIdentity, observation: terminal.clock.source_verified},
      start: started, source_verified: verified},
  };
  fs.writeFileSync(e.file(restorationName.replace('.json', '-source.json')), JSON.stringify(after, null, 2) + '\n', {flag: 'wx'});
  fs.writeFileSync(e.file(restorationName), JSON.stringify(result, null, 2) + '\n', {flag: 'wx'});
  assert.deepStrictEqual(restoration, {
    restored: true, quiescent: true, all_mutant_before_restore: true,
    live_members: [], external_edit: false, error: null,
    files: files.map(file => ({path: file.path, before_sha256: file.changed_sha256,
      after_sha256: file.original_sha256, write_attempted: true})),
  }, 'exact successful restoration required');
  assert.strictEqual(clockError, null);
  assert(terminalIdentity);
  assert.strictEqual(e.hash(e.bytes(terminalIdentity.name)), terminalIdentity.sha256);
  guard(map);
}
assert.strictEqual(failure, null);
assert.strictEqual(oraclePassed, true);
console.log('PASS preliminary retirement regression: normal named failure and exact source recovery; not full R118 qualification');
