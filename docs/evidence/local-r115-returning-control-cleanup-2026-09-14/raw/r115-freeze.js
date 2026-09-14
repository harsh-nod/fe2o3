const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const p = require('./r115-qualification-plan.js');
const e = require('./r115-qualification-evidence.js');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const execute = command => cp.execFileSync(command[0], command.slice(1), {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const parent = '0265025b96f25a7cb79c97b576bb25cb38b15df5';
const accepted = '9eaf19141e8af6ade490feba3062c8b49d9b38ca';
assert.strictEqual(execute(['git', 'rev-parse', 'HEAD']).trim(), parent);
const inventory = () => [...new Set(execute(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/')))].sort();
const paths = inventory();
assert.strictEqual(paths.length, p.sourceCount);
const frozen = Object.fromEntries(paths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
const prerequisiteRuns = p.prerequisites.map(([name]) => name);
const prerequisiteArtifacts = prerequisiteRuns.flatMap(name =>
  ['.json', '.log', '-source.json', '-source-after.json'].map(suffix => name + suffix)).sort();
const anchorArtifacts = ['.json', '.log', '-source.json', '-source-after.json'].map(suffix => p.rebootAnchor[0] + suffix);
const validationInputs = [...new Set([...prerequisiteArtifacts, ...p.prerequisites.map(([, , previous]) => previous),
  ...anchorArtifacts, ...p.freezeHelpers])].sort();
const captured = new Map(validationInputs.map(name => [name, e.bytes(name)]));
const io = {bytes: name => {assert(captured.has(name), 'captured prerequisite input'); return captured.get(name);},
  read: name => JSON.parse(io.bytes(name))};
for (const [name, command, previous] of [p.rebootAnchor, ...p.prerequisites]) {
  e.checkRecord(io.read(name + '.json'), name, command, frozen, previous, 0, io);
}
const commands = [
  ['cargo', '+nightly-2026-04-03', '--version', '--verbose'],
  ['rustc', '+nightly-2026-04-03', '--version', '--verbose'],
  ['python3', '-VV'], ['node', '--version'], ['git', '--version'], ['uname', '-srmo'],
];
const environment = {
  publication_parent: parent, accepted_runtime_checkpoint: accepted,
  recorded_phase: 'After separately recorded full GNU/musl prerequisites; before remaining source/auxiliary/mutation gates. Earlier preliminary failures preserved. No native or formal qualification.',
  records: commands.map(command => ({command, output: execute(command)})),
  binaries: ['cargo', 'rustc'].map(name => {
    const binary = execute(['rustup', 'which', '--toolchain', 'nightly-2026-04-03', name]).trim();
    return {name, path: binary, sha256: hash(fs.readFileSync(binary))};
  }),
  runners: p.freezeHelpers.map(name => ({name, sha256: hash(io.bytes(name))})),
  environment_overrides: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'},
  environment_removed: ['XDG_RUNTIME_DIR'], test_deadlines_changed: false, hardware_qualification: false, solver_rerun: false,
  campaign_envelope: {deadline_ms: 7200000, individual_gate_deadline_ms: 1800000},
  ambient_stack: {RUST_MIN_STACK: process.env.RUST_MIN_STACK ?? null,
    command: ['bash', '-c', 'ulimit -s'], output: execute(['bash', '-c', 'ulimit -s'])},
  prerequisite_runs: prerequisiteRuns,
  reboot_anchor: p.rebootAnchor[0],
  prerequisite_artifacts: prerequisiteArtifacts.map(name => ({name, sha256: hash(io.bytes(name))})),
  prerequisite_validation_inputs: validationInputs.map(name => ({name, sha256: hash(io.bytes(name))})),
  source_observation_boundary: 'Full prerequisites are checked by matching endpoint source maps, not relabeled as executions inside the later campaign. Ambient state not recorded by those runs is not inferred.',
  prior_artifacts: fs.readdirSync(root).filter(name => /^r115-.*\.(json|log)$/.test(name)).sort()
    .map(name => ({name, sha256: hash(fs.readFileSync(path.join(root, name)))})),
};
const previous = JSON.parse(fs.readFileSync(path.join(repo,
  'docs/evidence/local-r114-pristine-control-cleanup-2026-09-13/raw/r114-environment.json')));
assert.deepStrictEqual(environment.binaries, previous.binaries);
const command = ['gh', 'issue', 'view', '182', '--repo', 'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url'];
const issue = {observed_at: new Date().toISOString(), command, issue: JSON.parse(execute(command))};
const previousSource = JSON.parse(fs.readFileSync(path.join(repo,
  'docs/evidence/local-r114-pristine-control-cleanup-2026-09-13/raw/r114-frozen-source.json')));
environment.source_delta = paths.filter(name => frozen[name] !== previousSource[name]);
assert.deepStrictEqual(environment.source_delta, p.sourceDelta);
assert.deepStrictEqual(paths.filter(name => !Object.hasOwn(previousSource, name)), p.addedSource);
assert.deepStrictEqual(Object.keys(previousSource).filter(name => !Object.hasOwn(frozen, name)), []);
const refreshedPaths = inventory();
assert.deepStrictEqual(refreshedPaths, paths, 'complete source inventory remains unchanged');
assert.deepStrictEqual(Object.fromEntries(refreshedPaths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))])), frozen);
assert.strictEqual(execute(['git', 'rev-parse', 'HEAD']).trim(), parent);
environment.observation = {utc_ms: Date.now(), monotonic_ns: process.hrtime.bigint().toString(),
  boot_id: fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim(),
  clock_source: 'node-process-hrtime-linux-monotonic'};
environment.recorded_at = new Date(environment.observation.utc_ms).toISOString();
for (const name of [p.rebootAnchor[0], ...prerequisiteRuns]) e.ordered(io.read(name + '.json').clock.source_verified, environment.observation);
for (const [name, bytes] of captured) assert.strictEqual(hash(e.bytes(name)), hash(bytes), 'validated prerequisite input unchanged: ' + name);
for (const [name, value] of [['frozen-source', frozen], ['environment', environment], ['issue-status', issue]]) {
  fs.writeFileSync(path.join(root, 'r115-' + name + '.json'), JSON.stringify(value, null, 2) + '\n', {flag: 'wx'});
}
console.log('Frozen ' + paths.length + ' source identities; tools, runners, preliminary bytes and issue status retained');
