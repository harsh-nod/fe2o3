const assert = require('assert');
const e = require('./r119-integrated-qualification-evidence-v1.js');
const p = require('./r119-integrated-qualification-plan-v1.js');
if (require.main === module) require('./r119-integrated-validation-v1.js').guard();
const originName = 'r119-integrated-origin-v1.json';
const originHash = '9d400e58f95e4a4f8aa8c8112f7ce787fa90e5bcbe07a2bd9c6d66994a24f885';
const runnerHash = '0da05bbcbcd6ead667a63ccfbd4e2479e7231c3fa25bc4d81f1fe81a56adb694';
const suffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const integrated = ['format', 'persistent-focused', 'control-focused', 'gnu-all', 'clippy', 'musl-all'];
const format = ['cargo', '+nightly-2026-04-03', 'fmt', '--all', '--', '--check'];
const lint = ['cargo', '+nightly-2026-04-03', 'clippy', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--all-targets', '--', '-D', 'warnings'];
const focused = [...p.test, p.focused[0][1], '--', '--test-threads=1'];
function inputNames(io = e) {
  return [...new Set([originName, 'r119-integrated-run-v1.js',
    ...io.read(originName).isolated_artifacts.map(pin => pin.name),
    ...integrated.flatMap(kind => suffixes.map(suffix => 'r119-integrated-' + kind + suffix))])].sort();
}
function positive(log, names) {
  e.assertPassing(log);
  assert.deepStrictEqual(e.passing(log), [...names].sort());
  assert.deepStrictEqual(e.ignored(log), []);
  assert.deepStrictEqual(e.totals(log), {harnesses: 1, passed: names.length, failed: 0, ignored: 0});
}
function checkRecords(map, io = e) {
  assert.strictEqual(e.hash(io.bytes(originName)), originHash, 'immutable promotion origin');
  const origin = io.read(originName);
  assert.strictEqual(origin.accepted, false, 'promotion is not acceptance');
  assert.strictEqual(origin.publication_parent, p.parent);
  assert.strictEqual(origin.source_map_sha256, p.sourceMapSha256);
  assert.strictEqual(origin.source_identities, p.sourceCount);
  assert.strictEqual(e.hash(JSON.stringify(map)), p.sourceMapSha256);
  assert.strictEqual(Object.keys(map).length, p.sourceCount);
  assert.strictEqual(origin.isolated_artifacts.length, 24);
  assert.strictEqual(new Set(origin.isolated_artifacts.map(pin => pin.name)).size, 24);
  for (const pin of origin.isolated_artifacts) {
    const bytes = io.bytes(pin.name);
    assert.strictEqual(bytes.length, pin.bytes, 'isolated artifact length: ' + pin.name);
    assert.strictEqual(e.hash(bytes), pin.sha256, 'isolated artifact hash: ' + pin.name);
  }
  assert.deepStrictEqual(origin.runner, {name: 'r119-integrated-run-v1.js', sha256: runnerHash});
  assert.strictEqual(e.hash(io.bytes(origin.runner.name)), runnerHash);
  const committed = name => io.git(['show', p.parent + ':' + p.previous + 'raw/' + name]);
  const baseline = JSON.parse(committed('r118b-frozen-source.json'));
  const prior = JSON.parse(io.git(['show', origin.isolated_parent + ':docs/evidence/local-r117-detached-persistent-control-cleanup-2026-09-14/raw/r117-frozen-source.json']));
  assert.deepStrictEqual(Object.keys(map).filter(name => map[name] !== baseline[name]), p.sourceDelta);
  assert.deepStrictEqual(Object.keys(map).filter(name => !Object.hasOwn(baseline, name)), p.addedSource);
  assert.deepStrictEqual(Object.keys(baseline).filter(name => !Object.hasOwn(map, name)), []);
  assert.deepStrictEqual(origin.changed_source.map(pin => pin.path), p.sourceDelta);
  for (const pin of origin.changed_source) {
    assert.strictEqual(map[pin.path], pin.sha256);
    assert.strictEqual(baseline[pin.path] ?? null, pin.previous_sha256);
    assert.strictEqual(prior[pin.path] ?? null, pin.previous_sha256, 'disjoint parent delta');
    prior[pin.path] = pin.sha256;
  }
  const isolatedMap = Object.fromEntries(Object.entries(prior).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0));
  assert.strictEqual(e.hash(JSON.stringify(isolatedMap)), origin.isolated_source_map_sha256);
  assert.strictEqual(Object.keys(isolatedMap).length, origin.isolated_source_identities);
  function run(name, command, expectedMap, previous, context) {
    const record = io.read(name + '.json');
    e.checkRecord(record, name, command, expectedMap, previous, 0, io, context);
    return {record, log: io.bytes(name + '.log').toString()};
  }
  const current = (kind, command, previous) => run('r119-integrated-' + kind, command, map, previous,
    {head: p.parent, cwd: e.repo, contract: e.contract, runner: origin.runner.name, after: map});
  const gnu = current('gnu-all', p.fullTest, 'r119-integrated-control-focused.json');
  const musl = current('musl-all', [...p.fullTest, '--target', 'x86_64-unknown-linux-musl'], 'r119-integrated-gnu-all.json');
  const full = {
    gnu: e.checkFull(gnu.log, committed('r118b-reviewed-gnu-all.log')),
    musl: e.checkFull(musl.log, committed('r118b-reviewed-musl-all.log')),
  };
  const kfd = e.executables(gnu.log).filter(target => target.kind === 'libtest' && /fe2o3_kfd(?:-|\))/.test(target.name));
  assert.strictEqual(kfd.length, 1);
  assert.strictEqual(kfd[0].passing.length, 1149);
  const fmt = current('format', format, null);
  assert.strictEqual(fmt.log, '');
  e.ordered(origin.observation, fmt.record.clock.start);
  positive(current('persistent-focused', focused, 'r119-integrated-format.json').log, p.newTests);
  const controls = kfd[0].passing.filter(name => name.includes('queue::dispatch_binding::control_release::tests::'));
  assert.strictEqual(controls.length, 46);
  positive(current('control-focused', [...p.test, 'queue::dispatch_binding::control_release::tests::', '--', '--test-threads=1'],
    'r119-integrated-persistent-focused.json').log, controls);
  const clippy = current('clippy', lint, 'r119-integrated-gnu-all.json');
  assert(clippy.log.includes('Finished `dev` profile'));
  e.ordered(clippy.record.clock.source_verified, musl.record.clock.start);
  const isolatedContext = {head: origin.isolated_parent, cwd: e.file('fe2o3-r119-persistent-data'),
    contract: 'r119-raw-utc-boot-monotonic-v1', runner: 'r119-run-v1.js', after: isolatedMap};
  const isolated = (kind, command, previous) => {
    const result = run('r119-' + kind, command, isolatedMap, previous, isolatedContext);
    e.ordered(result.record.clock.source_verified, origin.observation);
    return result;
  };
  assert.strictEqual(isolated('format', format, null).log, '');
  positive(isolated('persistent-focused', [...p.test, 'control_release::tests::persistent_tests::', '--', '--test-threads=1'],
    'r119-format.json').log, []);
  positive(isolated('persistent-focused-v2', focused, 'r119-persistent-focused.json').log, p.newTests);
  positive(isolated('kfd-runtime', p.test, 'r119-persistent-focused-v2.json').log, kfd[0].passing);
  assert(isolated('clippy', lint, 'r119-kfd-runtime.json').log.includes('Finished `dev` profile'));
  return {accepted: false, source_map_sha256: p.sourceMapSha256, source_identities: p.sourceCount,
    isolated_artifacts: 24, isolated_zero_test_run: 'compile-only', full,
    persistent_tests: 15, control_tests: 46, kfd_tests: 1149, integrated_clippy: true,
    previous: 'r119-integrated-musl-all.json'};
}
function checkHistory(map, io = e) {
  const names = inputNames(io);
  const captured = new Map(names.map(name => [name, Buffer.from(io.bytes(name))]));
  const snapshot = {bytes: name => {
    assert(captured.has(name), 'captured history input: ' + name);
    return Buffer.from(captured.get(name));
  }, git: io.git};
  snapshot.read = name => JSON.parse(snapshot.bytes(name));
  const result = checkRecords(map, snapshot);
  assert.deepStrictEqual(inputNames(io), names, 'unchanged history membership');
  for (const [name, bytes] of captured) assert.strictEqual(e.hash(io.bytes(name)), e.hash(bytes), 'unchanged history input: ' + name);
  return result;
}
if (require.main === module) console.log(JSON.stringify(checkHistory(e.identities()), null, 2));
module.exports = {originName, originHash, runnerHash, integrated, inputNames, positive, checkRecords, checkHistory};
