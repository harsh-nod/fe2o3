const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const p = require('./r118-qualification-plan.js');
const e = require('./r118-qualification-evidence.js');
const h = require('./r118-history-v3.js');
const support = require('./r118-history-support-v1.js');
const execute = command => cp.execFileSync(command[0], command.slice(1), {cwd: e.repo, maxBuffer: 64 * 1024 * 1024}).toString();
assert.strictEqual(e.git(['rev-parse', 'HEAD']).trim(), p.parent);
const inventory = () => [...new Set(e.git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(name => name && !name.startsWith('docs/')))].sort();
const paths = inventory();
assert.strictEqual(paths.length, p.sourceCount);
const observe = names => Object.fromEntries(names.map(name => [name, e.hash(fs.readFileSync(path.join(e.repo, name)))]));
const frozen = observe(paths);
assert.strictEqual(e.hash(JSON.stringify(frozen)), p.sourceMapSha256);
const priorNames = e.historicalNames();
const validationInputs = [...new Set([...p.history.artifacts.map(item => item.name),
  ...support.inputNames(), ...p.freezeHelpers, ...priorNames])].sort();
const captured = new Map(validationInputs.map(name => [name, Buffer.from(e.bytes(name))]));
const io = {
  bytes: name => {assert(captured.has(name), 'captured validation input'); return captured.get(name);},
  read: name => JSON.parse(io.bytes(name)),
  git: e.git,
};
const history = h.checkHistory(frozen, io);
const tail = support.check(frozen, history, io);
const commands = [
  ['cargo', '+nightly-2026-04-03', '--version', '--verbose'],
  ['rustc', '+nightly-2026-04-03', '--version', '--verbose'],
  ['python3', '-VV'], ['node', '--version'], ['git', '--version'], ['uname', '-srmo'],
];
const pin = name => ({name, sha256: e.hash(io.bytes(name))});
const prerequisiteRuns = p.prerequisites.map(([name]) => name);
const environment = {
  publication_parent: p.parent, accepted_runtime_checkpoint: p.accepted,
  recorded_phase: 'After reviewed GNU/musl prerequisites, original history preflight and support contracts; before remaining source/auxiliary/mutation gates. Original failures and source transitions remain separate. No native or formal qualification.',
  records: commands.map(command => ({command, output: execute(command)})),
  binaries: ['cargo', 'rustc'].map(name => {
    const binary = execute(['rustup', 'which', '--toolchain', 'nightly-2026-04-03', name]).trim();
    return {name, path: binary, sha256: e.hash(fs.readFileSync(binary))};
  }),
  runners: p.freezeHelpers.map(pin),
  environment_overrides: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'},
  environment_removed: ['XDG_RUNTIME_DIR'], test_deadlines_changed: false, hardware_qualification: false, solver_rerun: false,
  campaign_envelope: {deadline_ms: 7200000, individual_gate_deadline_ms: 1800000},
  ambient_stack: {RUST_MIN_STACK: process.env.RUST_MIN_STACK ?? null,
    command: ['bash', '-c', 'ulimit -s'], output: execute(['bash', '-c', 'ulimit -s'])},
  prerequisite_runs: prerequisiteRuns, history, support_history: tail,
  prerequisite_artifacts: prerequisiteRuns.flatMap(name =>
    ['.json', '.log', '-source.json', '-source-after.json'].map(suffix => name + suffix)).sort().map(pin),
  prerequisite_validation_inputs: validationInputs.map(pin),
  source_observation_boundary: 'Endpoint equality is not continuous immutability. Original isolated history retains independent contexts; only final-source current-boot prerequisites and the validated support tail precede this observation.',
  prior_artifacts: priorNames.map(pin),
};
assert.strictEqual(environment.ambient_stack.RUST_MIN_STACK, null);
assert.deepStrictEqual(environment.binaries, JSON.parse(e.committed('raw/r117-environment.json')).binaries);
const command = ['gh', 'issue', 'view', '182', '--repo', 'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url'];
const issue = {observed_at: new Date().toISOString(), command, issue: JSON.parse(execute(command))};
const oldMap = JSON.parse(e.committed('raw/r117-frozen-source.json'));
environment.source_delta = paths.filter(name => frozen[name] !== oldMap[name]);
assert.deepStrictEqual(environment.source_delta, p.sourceDelta);
assert.deepStrictEqual(paths.filter(name => !Object.hasOwn(oldMap, name)), p.addedSource);
assert.deepStrictEqual(Object.keys(oldMap).filter(name => !Object.hasOwn(frozen, name)), []);
const refreshedPaths = inventory();
assert.deepStrictEqual(refreshedPaths, paths, 'complete source inventory remains unchanged');
assert.deepStrictEqual(observe(refreshedPaths), frozen);
assert.strictEqual(e.git(['rev-parse', 'HEAD']).trim(), p.parent);
environment.observation = e.sample();
environment.recorded_at = new Date(environment.observation.utc_ms).toISOString();
for (const name of p.history.prerequisiteNames) e.ordered(io.read(name + '.json').clock.source_verified, environment.observation);
e.ordered(io.read(tail.previous).clock.source_verified, environment.observation);
for (const [name, bytes] of captured) assert.strictEqual(e.hash(e.bytes(name)), e.hash(bytes), 'validated input unchanged: ' + name);
for (const [name, value] of [['frozen-source', frozen], ['environment', environment], ['issue-status', issue]]) {
  fs.writeFileSync(e.file('r118-' + name + '.json'), JSON.stringify(value, null, 2) + '\n', {flag: 'wx'});
}
console.log('Frozen ' + paths.length + ' source identities; original history and support tail checked; tools, input bytes and issue status retained');
