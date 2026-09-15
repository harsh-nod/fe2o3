const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const p = require('./r118b-qualification-plan.js');
const e = require('./r118b-qualification-evidence.js');
const history = require('./r118b-current-history-v1.js');
const support = require('./r118b-history-support-v1.js');
const outputs = ['frozen-source', 'environment', 'issue-status'].map(kind => 'r118b-' + kind + '.json');
const defaults = {
  head: () => e.git(['rev-parse', 'HEAD']).trim(),
  inventory: () => e.git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'),
  source: name => fs.readFileSync(path.join(e.repo, name)),
  bytes: e.bytes, names: () => fs.readdirSync(e.root), historicalNames: e.historicalNames,
  git: e.git, sample: e.sample, env: process.env,
  execute: command => cp.execFileSync(command[0], command.slice(1), {cwd: e.repo, maxBuffer: 64 * 1024 * 1024}).toString(),
  binary: file => fs.readFileSync(file),
  vacant: name => {
    try {fs.lstatSync(e.file(name));} catch (error) {if (error.code === 'ENOENT') return; throw error;}
    throw new Error('freeze output already exists: ' + name);
  },
  write: (name, value) => fs.writeFileSync(e.file(name), JSON.stringify(value, null, 2) + '\n', {flag: 'wx'}),
  checkHistory: history.checkCurrentHistory, checkSupport: support.check,
};
const sourceNames = io => [...new Set(io.inventory().filter(name => name && !name.startsWith('docs/')))].sort();
const artifactNames = names => names.filter(name => /^(?:r118|r118b)-/.test(name) && /\.(?:json|log|js|py|md|rs)$/.test(name)).sort();
function freeze(io = defaults) {
  assert.strictEqual(io.head(), p.parent, 'opening HEAD');
  outputs.forEach(io.vacant);
  const paths = sourceNames(io);
  assert.strictEqual(paths.length, p.sourceCount);
  const observe = names => Object.fromEntries(names.map(name => [name, e.hash(io.source(name))]));
  const frozen = observe(paths);
  assert.strictEqual(e.hash(JSON.stringify(frozen)), p.sourceMapSha256, 'opening source map');
  const listing = io.names();
  const priorNames = io.historicalNames();
  const live = {bytes: io.bytes, read: name => JSON.parse(io.bytes(name)), git: io.git, names: () => [...listing]};
  const validationInputs = [...new Set([...history.inputNames(live), ...support.inputNames(live),
    ...p.freezeHelpers, ...priorNames])].sort();
  const captured = new Map(validationInputs.map(name => [name, Buffer.from(io.bytes(name))]));
  const snapshot = {bytes: name => {
    assert(captured.has(name), 'captured validation input: ' + name);
    return Buffer.from(captured.get(name));
  }, git: io.git, names: () => [...listing]};
  snapshot.read = name => JSON.parse(snapshot.bytes(name));
  const current = io.checkHistory(frozen, snapshot);
  const tail = io.checkSupport(frozen, current, snapshot);
  const commands = [
    ['cargo', '+nightly-2026-04-03', '--version', '--verbose'],
    ['rustc', '+nightly-2026-04-03', '--version', '--verbose'],
    ['python3', '-VV'], ['node', '--version'], ['git', '--version'], ['uname', '-srmo'],
  ];
  const pin = name => ({name, sha256: e.hash(snapshot.bytes(name))});
  const committed = name => io.git(['show', p.parent + ':' + p.previous + 'raw/' + name]);
  const environment = {
    publication_parent: p.parent, accepted_runtime_checkpoint: p.accepted,
    recorded_phase: 'After fresh corrected-source full runs, current-history validation and support contracts; before the remaining source/auxiliary/mutation gates. Stopped R118 remains unaccepted history.',
    records: commands.map(command => ({command, output: io.execute(command)})),
    binaries: ['cargo', 'rustc'].map(name => {
      const file = io.execute(['rustup', 'which', '--toolchain', 'nightly-2026-04-03', name]).trim();
      return {name, path: file, sha256: e.hash(io.binary(file))};
    }),
    runners: p.freezeHelpers.map(pin),
    environment_overrides: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'},
    environment_removed: ['XDG_RUNTIME_DIR'], test_deadlines_changed: false, hardware_qualification: false, solver_rerun: false,
    campaign_envelope: {deadline_ms: 7200000, individual_gate_deadline_ms: 1800000},
    ambient_stack: {RUST_MIN_STACK: io.env.RUST_MIN_STACK ?? null,
      command: ['bash', '-c', 'ulimit -s'], output: io.execute(['bash', '-c', 'ulimit -s'])},
    prerequisite_runs: p.prerequisites.map(([name]) => name), history: current, support_history: tail,
    prerequisite_artifacts: p.prerequisites.flatMap(([name]) =>
      ['.json', '.log', '-source.json', '-source-after.json'].map(suffix => name + suffix)).sort().map(pin),
    prerequisite_validation_inputs: validationInputs.map(pin),
    source_observation_boundary: 'Endpoint equality is not continuous immutability. Stopped history has its own original contexts; only corrected-source prerequisites precede this freeze.',
    prior_artifacts: priorNames.map(pin),
  };
  assert.strictEqual(environment.ambient_stack.RUST_MIN_STACK, null);
  assert.deepStrictEqual(environment.binaries, JSON.parse(committed('r117-environment.json')).binaries, 'accepted toolchain binaries');
  const command = ['gh', 'issue', 'view', '182', '--repo', 'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url'];
  const issue = {observed_at: new Date().toISOString(), command, issue: JSON.parse(io.execute(command))};
  const oldMap = JSON.parse(committed('r117-frozen-source.json'));
  environment.source_delta = paths.filter(name => frozen[name] !== oldMap[name]);
  assert.deepStrictEqual(environment.source_delta, p.sourceDelta);
  assert.deepStrictEqual(paths.filter(name => !Object.hasOwn(oldMap, name)), p.addedSource);
  assert.deepStrictEqual(Object.keys(oldMap).filter(name => !Object.hasOwn(frozen, name)), []);
  assert.deepStrictEqual(sourceNames(io), paths, 'complete source inventory remains unchanged');
  assert.deepStrictEqual(observe(paths), frozen, 'closing source map');
  assert.strictEqual(io.head(), p.parent, 'closing HEAD');
  environment.observation = io.sample();
  environment.recorded_at = new Date(environment.observation.utc_ms).toISOString();
  for (const name of [...p.prerequisites.map(([name]) => name + '.json'), tail.previous]) {
    e.ordered(snapshot.read(name).clock.source_verified, environment.observation);
  }
  assert.deepStrictEqual(artifactNames(io.names()), artifactNames(listing), 'artifact namespace membership unchanged');
  assert.deepStrictEqual(io.historicalNames(), priorNames, 'historical membership unchanged');
  for (const [name, bytes] of captured) assert.strictEqual(e.hash(io.bytes(name)), e.hash(bytes), 'validated input unchanged: ' + name);
  outputs.forEach(io.vacant);
  [frozen, environment, issue].forEach((value, index) => io.write(outputs[index], value));
  return {source_identities: paths.length, validation_inputs: captured.size, history: current, support_history: tail};
}
if (require.main === module) console.log(JSON.stringify(freeze(), null, 2));
module.exports = {defaults, outputs, sourceNames, artifactNames, freeze};
