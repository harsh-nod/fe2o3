const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const p = require('./r113-qualification-plan.js');
const e = require('./r113-qualification-evidence.js');
const manifestName = 'r113-qualification-manifest.json';
const manifest = e.read(manifestName);
const manifestHash = e.hash(e.bytes(manifestName));
const map = e.read('r113-frozen-source.json');
assert.strictEqual(manifest.source_head, p.parent);
assert.strictEqual(manifest.source_map_sha256, e.hash(JSON.stringify(map)));
assert.deepStrictEqual(manifest.mutations, p.mutations);
assert.strictEqual(manifest.original_source.path, p.production);
assert.strictEqual(e.hash(manifest.original_source.text), map[p.production]);
assert.deepStrictEqual(manifest.entries, e.expectedEntries(map, manifest.original_source.text));
assert.deepStrictEqual(manifest.helpers.map(x => x.name), p.helpers);
for (const helper of manifest.helpers) assert.strictEqual(e.hash(e.bytes(helper.name)), helper.sha256);
assert.deepStrictEqual(e.identities(), map);
function run(name, expectedMap) {
  const entry = manifest.entries[name + '.json'];
  assert(entry && entry.kind === 'run');
  assert.strictEqual(entry.source_map_sha256, e.hash(JSON.stringify(expectedMap)));
  for (const suffix of ['.json', '.log', '-source.json', '-source-after.json']) assert(!fs.existsSync(e.file(name + suffix)));
  const result = cp.spawnSync(process.execPath, [e.file('r113-run-v3.js'), name, entry.predecessor, ...entry.command],
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
  const changed = e.mutatedSource(original, mutation);
  const expected = {...map, [mutation.path]: e.hash(changed)};
  const name = 'r113-qualified-mut-' + mutation.name;
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
    const observed = e.hash(fs.readFileSync(target));
    // Restore only the exact mutation owned by this cohort, never an external edit.
    assert.strictEqual(observed, expected[mutation.path]);
    const terminal = fs.existsSync(e.file(name + '.json')) ? e.read(name + '.json') : null;
    let quiescent = e.canRestore(terminal);
    let liveMembers = null;
    if (quiescent && terminal.process_group_cleanup.close.pgid) {
      liveMembers = e.liveGroupMembers(terminal.process_group_cleanup.close.pgid);
      quiescent = liveMembers.length === 0;
    }
    if (!quiescent) {
      fs.writeFileSync(e.file('r113-qualification-failure-' + mutation.name + '.json'), JSON.stringify({
        accepted: false, source_restored: false, cleanup_required: true, live_members: liveMembers,
        original_source: {manifest: manifestName, path: p.production, sha256: map[p.production]},
        error: String(failure || 'child or group quiescence is not established'),
      }, null, 2) + '\n', {flag: 'wx'});
      throw Error('Restoration withheld until task-owned worker cleanup; original bytes are retained in the manifest');
    }
    fs.writeFileSync(target, original);
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
      const restoredName = 'r113-qualified-restoration-' + mutation.name;
      fs.writeFileSync(e.file(restoredName + '-source.json'), JSON.stringify(restored, null, 2) + '\n', {flag: 'wx'});
      fs.writeFileSync(e.file(restoredName + '.json'), JSON.stringify(restoration, null, 2) + '\n', {flag: 'wx'});
    }
  }
  if (failure) {
    fs.writeFileSync(e.file('r113-qualification-failure-' + mutation.name + '.json'),
      JSON.stringify({accepted: false, source_restored: true, error: String(failure)}, null, 2) + '\n', {flag: 'wx'});
    throw failure;
  }
  console.log('PASS ' + (index + 1) + '/17: ' + mutation.name + '; exact source restored');
}
for (const [kind, filter, count] of p.focused) {
  const result = run('r113-qualified-restored-' + kind, map);
  assert.strictEqual(e.passing(result.log).length, count);
  console.log('PASS restored ' + kind + ': ' + count);
}
run('r113-qualified-collector-validation', map);
console.log('PASS closed collector validation; archive and independent review remain');
