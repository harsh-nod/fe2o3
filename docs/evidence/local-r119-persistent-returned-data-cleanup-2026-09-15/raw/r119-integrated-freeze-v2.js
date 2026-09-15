const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const p = require('./r119-integrated-qualification-plan-v2.js');
const e = require('./r119-integrated-qualification-evidence-v1.js');
const h = require('./r119-integrated-history-v2.js');
const v = require('./r119-integrated-validation-v2.js');
const runnerEvidence = require('./r119-integrated-runner-evidence-v1.js');
if (require.main === module) v.guard();
const outputs = ['r119-integrated-frozen-source.json', 'r119-integrated-environment.json'];
const helpers = [...v.helperNames, v.manifestName].sort();
const runnerCases = ['real-clean', 'real-success', 'real-failure', 'real-inner-timeout', 'real-spawn',
  'cleanup-eperm-retains-success', 'outer-timeout-retains-signal', 'cleanup-esrch-is-benign',
  'timeout-eperm-unclosed-is-bounded'];
const supportRuns = [
  ['history-contracts-v2', ['node', '--test', e.file('r119-integrated-history-tests-v2.js')], 46],
  ['history-validation-v2', ['node', e.file('r119-integrated-history-v2.js')], null],
  ['runner-contracts-v2', ['node', e.file('r119-integrated-runner-tests-v2.js')], null],
  ['freeze-contracts-v2', ['node', '--test', e.file('r119-integrated-freeze-tests-v2.js')], 26],
];
function checkSupport(map, history, io) {
  const validation = v.inputs(io);
  let previous = history.previous;
  for (const [kind, command, count] of supportRuns) {
    const name = 'r119-integrated-' + kind;
    e.checkRecord(io.read(name + '.json'), name, command, map, previous, 0, io);
    const log = v.transcript(io.bytes(name + '.log').toString(), validation.sha256, count !== null);
    if (count !== null) {
      const names = kind === 'history-contracts-v2' ? v.historyNames : v.freezeNames;
      assert.strictEqual(names.length, count);
      v.checkTap(log, names);
    } else if (kind === 'history-validation-v2') {
      assert.deepStrictEqual(JSON.parse(log), history);
    } else {
      runnerEvidence.checkFixtures(map, io.read(name + '.json'), io);
      const pins = runnerEvidence.fixtureNames.map(name => 'R119_RUNNER_FIXTURE ' + name + ' ' + e.hash(io.bytes(name)));
      assert.deepStrictEqual(log.trimEnd().split('\n'), [...runnerCases.map(name => 'PASS ' + name), ...pins, 'PASS: 9 runner contract tests']);
    }
    previous = name + '.json';
  }
  const pins = io.read('r119-integrated-runner-inputs-v2.json');
  assert.strictEqual(pins.parent, p.parent);
  assert.strictEqual(pins.source_map_sha256, p.sourceMapSha256);
  assert.deepStrictEqual(pins.helpers.map(pin => pin.name), ['r119-integrated-runner-tests-v2.js', 'r119-integrated-run-v1.js']);
  for (const pin of pins.helpers) assert.strictEqual(e.hash(io.bytes(pin.name)), pin.sha256);
  return {history_contracts: 46, runner_contracts: 9, freeze_boundary_contracts: 17, validation_contracts: 9, previous};
}
const defaults = {
  head: () => e.git(['rev-parse', 'HEAD']).trim(),
  identities: e.identities, names: e.historicalNames, bytes: e.bytes, git: e.git,
  sample: e.sample, env: process.env,
  execute: command => cp.execFileSync(command[0], command.slice(1), {cwd: e.repo, maxBuffer: 64 * 1024 * 1024}).toString(),
  binary: name => fs.readFileSync(name), checkHistory: h.checkRecords, checkSupport,
  vacant: name => {
    try {fs.lstatSync(e.file(name));} catch (error) {if (error.code === 'ENOENT') return; throw error;}
    throw new Error('freeze output already exists: ' + name);
  },
  write: (name, value) => fs.writeFileSync(e.file(name), JSON.stringify(value, null, 2) + '\n', {flag: 'wx'}),
};
function freeze(io = defaults) {
  assert.strictEqual(io.head(), p.parent, 'opening HEAD');
  outputs.forEach(io.vacant);
  const map = io.identities();
  assert.strictEqual(Object.keys(map).length, p.sourceCount);
  assert.strictEqual(e.hash(JSON.stringify(map)), p.sourceMapSha256, 'opening source map');
  const names = io.names();
  assert.deepStrictEqual(names, [...new Set(names)].sort(), 'canonical artifact roster');
  for (const name of helpers) assert(names.includes(name), 'required helper: ' + name);
  const captured = new Map(names.map(name => [name, Buffer.from(io.bytes(name))]));
  const snapshot = {bytes: name => {
    assert(captured.has(name), 'captured freeze input: ' + name);
    return Buffer.from(captured.get(name));
  }, git: io.git};
  snapshot.read = name => JSON.parse(snapshot.bytes(name));
  const history = io.checkHistory(map, snapshot);
  const support = io.checkSupport(map, history, snapshot);
  const commands = [
    ['cargo', '+nightly-2026-04-03', '--version', '--verbose'],
    ['rustc', '+nightly-2026-04-03', '--version', '--verbose'],
    ['python3', '-VV'], ['node', '--version'], ['git', '--version'], ['uname', '-srmo'],
  ];
  const environment = {
    accepted: false, publication_parent: p.parent, accepted_runtime_checkpoint: p.parent,
    recorded_phase: 'Integrated full prerequisites and helper contracts checked; source, auxiliary and mutation qualification still pending.',
    source_map_sha256: p.sourceMapSha256, source_identities: p.sourceCount, history, support,
    records: commands.map(command => ({command, output: io.execute(command)})),
    binaries: ['cargo', 'rustc'].map(name => {
      const file = io.execute(['rustup', 'which', '--toolchain', 'nightly-2026-04-03', name]).trim();
      return {name, path: file, sha256: e.hash(io.binary(file))};
    }),
    environment_overrides: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'},
    environment_removed: ['XDG_RUNTIME_DIR'], test_deadlines_changed: false,
    hardware_qualification: false, solver_rerun: false,
    campaign_envelope: {deadline_ms: 7200000, individual_gate_deadline_ms: 1800000},
    ambient_stack: {RUST_MIN_STACK: io.env.RUST_MIN_STACK ?? null,
      command: ['bash', '-c', 'ulimit -s'], output: io.execute(['bash', '-c', 'ulimit -s'])},
    helpers: helpers.map(name => ({name, sha256: e.hash(snapshot.bytes(name))})),
    prior_artifacts: names.map(name => ({name, sha256: e.hash(snapshot.bytes(name))})),
    source_observation_boundary: 'Endpoint equality is not continuous immutability. Original isolated records retain their own parent, source and commands.',
  };
  assert.strictEqual(environment.ambient_stack.RUST_MIN_STACK, null);
  const previousEnvironment = JSON.parse(io.git(['show', p.parent + ':' + p.previous + 'raw/r118b-environment.json']));
  assert.deepStrictEqual(environment.binaries, previousEnvironment.binaries, 'accepted toolchain binaries');
  assert.deepStrictEqual(io.identities(), map, 'closing source map');
  assert.strictEqual(io.head(), p.parent, 'closing HEAD');
  const observation = io.sample();
  e.ordered(snapshot.read(support.previous).clock.source_verified, observation);
  environment.observation = observation;
  environment.recorded_at = new Date(observation.utc_ms).toISOString();
  assert.deepStrictEqual(io.names(), names, 'unchanged artifact membership');
  for (const [name, bytes] of captured) assert.strictEqual(e.hash(io.bytes(name)), e.hash(bytes), 'unchanged freeze input: ' + name);
  outputs.forEach(io.vacant);
  io.write(outputs[0], map);
  io.write(outputs[1], environment);
  return {accepted: false, source_identities: p.sourceCount, captured_artifacts: names.length, history, support};
}
if (require.main === module) console.log(JSON.stringify(freeze(), null, 2));
module.exports = {outputs, helpers, supportRuns, checkSupport, defaults, freeze};
