const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const p = require('./r115-qualification-plan.js');
const e = require('./r115-qualification-evidence.js');
const manifestName = 'r115-qualification-manifest.json';
const manifest = e.read(manifestName);
const manifestHash = e.hash(e.bytes(manifestName));
const map = e.read('r115-frozen-source.json');
assert.strictEqual(manifest.source_head, p.parent);
assert.strictEqual(manifest.source_map_sha256, e.hash(JSON.stringify(map)));
assert.deepStrictEqual(manifest.mutations, p.mutations);
const originals = e.originalSources(map, manifest.original_sources);
assert.deepStrictEqual(manifest.entries, e.expectedEntries(map, manifest.original_sources));
assert.deepStrictEqual(manifest.helpers.map(x => x.name), p.helpers);
for (const helper of manifest.helpers) assert.strictEqual(e.hash(e.bytes(helper.name)), helper.sha256);
assert.deepStrictEqual(e.identities(), map);
function run(name, expectedMap) {
  const entry = manifest.entries[name + '.json'];
  assert(entry && entry.kind === 'run');
  assert.strictEqual(entry.source_map_sha256, e.hash(JSON.stringify(expectedMap)));
  for (const suffix of ['.json', '.log', '-source.json', '-source-after.json']) assert(!fs.existsSync(e.file(name + suffix)));
  const result = cp.spawnSync(process.execPath, [e.file('r115-run-v4.js'), name, entry.predecessor, ...entry.command],
    {cwd: e.repo, encoding: 'utf8', maxBuffer: 4 * 1024 * 1024});
  assert.ifError(result.error);
  assert.strictEqual(result.signal, null);
  assert.strictEqual(result.status, entry.returncode, (result.stdout + result.stderr).slice(-4000));
  return e.checkRun(name, entry.command, expectedMap, entry.predecessor, entry.returncode);
}
for (const [index, mutation] of p.mutations.entries()) {
  assert.deepStrictEqual(e.identities(), map);
  const target = path.join(e.repo, mutation.path);
  const original = fs.readFileSync(target, 'utf8');
  assert.strictEqual(original, originals[mutation.path], 'original matches selected pinned source');
  const changed = e.mutatedSource(original, mutation);
  const expected = {...map, [mutation.path]: e.hash(changed)};
  const name = 'r115-qualified-mut-' + mutation.name;
  let completed = null;
  let failure = null;
  fs.writeFileSync(target, changed);
  try {
    assert.deepStrictEqual(e.identities(), expected);
    completed = run(name, expected);
    e.checkMutation(completed.log, mutation);
  } catch (error) {failure = error;}
  finally {
    const start = e.sample();
    const terminal = fs.existsSync(e.file(name + '.json')) ? e.read(name + '.json') : null;
    const restoration = e.restoreOwnedMutation(target, expected[mutation.path], original, terminal);
    if (!restoration.restored) {
      fs.writeFileSync(e.file('r115-qualification-failure-' + mutation.name + '.json'), JSON.stringify({
        accepted: false, source_restored: false, cleanup_required: !restoration.quiescent,
        live_members: restoration.live_members, external_edit: restoration.external_edit,
        observed_source_sha256: restoration.observed_source_sha256,
        original_source: e.recoverySource(manifestName, map, mutation),
        error: String(failure || (restoration.external_edit ? 'external edit detected' : 'child or group quiescence is not established')),
      }, null, 2) + '\n', {flag: 'wx'});
      throw Error('Restoration withheld: process closure or source identity changed; original bytes are retained in the manifest');
    }
    const observed = restoration.observed_source_sha256;
    const restored = e.identities();
    assert.deepStrictEqual(restored, map);
    const verified = e.sample();
    if (completed) {
      e.ordered(completed.record.clock.source_verified, start);
      e.ordered(start, verified);
      const priorName = name + '.json';
      const restoration = {source_head: p.parent, manifest_sha256: manifestHash,
        source_map_sha256: e.hash(JSON.stringify(map)), mutated_source_sha256: observed,
        source_unchanged: true, source_identities: p.sourceCount,
        clock: {contract: e.contract, error: null, kind: 'restoration', start, source_verified: verified,
          predecessor: {name: priorName, sha256: e.hash(e.bytes(priorName)), observation: completed.record.clock.source_verified}}};
      const restoredName = 'r115-qualified-restoration-' + mutation.name;
      fs.writeFileSync(e.file(restoredName + '-source.json'), JSON.stringify(restored, null, 2) + '\n', {flag: 'wx'});
      fs.writeFileSync(e.file(restoredName + '.json'), JSON.stringify(restoration, null, 2) + '\n', {flag: 'wx'});
    }
  }
  if (failure) {
    fs.writeFileSync(e.file('r115-qualification-failure-' + mutation.name + '.json'),
      JSON.stringify({accepted: false, source_restored: true, error: String(failure)}, null, 2) + '\n', {flag: 'wx'});
    throw failure;
  }
  console.log('PASS ' + (index + 1) + '/' + p.mutations.length + ': ' + mutation.name + '; exact source restored');
}
for (const [kind, filter, count] of p.focused) {
  const result = run('r115-qualified-restored-' + kind, map);
  assert.strictEqual(e.passing(result.log).length, count);
  console.log('PASS restored ' + kind + ': ' + count);
}
run('r115-qualified-collector-validation', map);
console.log('PASS closed collector validation; archive and independent review remain');

