const assert = require('assert');
const fs = require('fs');
const {test} = require('node:test');
const C = require('./r118b-mutation-core-v1.js');
const P = require('./r118b-mutation-plan-v1.js');
const root = '/home/harsh/.codex-tmp/';
const repo = root + 'fe2o3-r61-execution/';
const a = 'crates/example/src/a.rs';
const b = 'crates/example/src/b.rs';
const c = 'crates/example/src/c.rs';
const closed = () => ({child_closed: true, spawn_error: null, child_returncode: 101,
  process_group_cleanup: {close: {status: 'absent', pgid: 4242, live_members: []}}});
test('executed helper bytes match the separately frozen input manifest', () => {
  const inputs = JSON.parse(fs.readFileSync(root + 'r118b-mutation-core-inputs-v1.json', 'utf8'));
  assert.strictEqual(inputs.parent, P.parent);
  assert.strictEqual(inputs.source_map_sha256, P.sourceMapSha256);
  assert.deepStrictEqual(inputs.helpers.map(helper => helper.name), [
    'r118b-mutation-core-tests-v1.js', 'r118b-mutation-core-v1.js', 'r118b-mutation-plan-v1.js',
    'r118b-mutations-c1-v1.js', 'r118b-mutations-c2-v1.js', 'r118b-mutations-c3-v1.js', 'r118b-run-v1.js',
  ]);
  for (const helper of inputs.helpers) assert.strictEqual(C.hash(fs.readFileSync(root + helper.name)), helper.sha256);
});
function fixture() {
  const source = {[a]: 'old_a\n', [b]: 'old_b\n', [c]: 'untouched\n'};
  const map = Object.fromEntries(Object.entries(source).map(([path, text]) => [path, C.hash(text)]));
  const originals = [a, b].map(path => ({path, text: source[path]}));
  const mutation = {patches: [a, b].map((path, index) => ({path, edits: [['old_' + (index ? 'b' : 'a'), 'new_' + (index ? 'b' : 'a')]]}))};
  const files = C.mutationSources(map, originals, mutation, [a, b]);
  const writes = [];
  const io = {read: path => source[path], write: (path, text) => {writes.push(path); source[path] = text;},
    live: () => [], identities: () => Object.fromEntries(Object.entries(source).map(([path, text]) => [path, C.hash(text)]))};
  const attempts = [];
  const state = {runnerAttempted: false, record: null, recovery: false, attempts};
  const apply = () => C.applyOwnedMutation(files, io, attempts);
  const launch = () => {state.runnerAttempted = true; state.record = closed();};
  const restore = () => C.restoreOwnedMutation(map, files, state, io);
  return {source, map, originals, mutation, files, writes, io, attempts, state, apply, launch, restore};
}
test('whole-file and scoped patches preserve all unselected text', () => {
  const source = 'fn a { old }\nfn b { old }\n';
  assert.strictEqual(C.mutatedSource(source, {path: a, scope: {start: 'fn a {', end: 'fn b {'}, edits: [['old', 'new']]}),
    'fn a { new }\nfn b { old }\n');
  assert.throws(() => C.mutatedSource(source, {path: a, edits: [['old', 'new']]}), /unique/);
});
for (const scope of [null, false, 0, '', {}, {start: 'fn a {'}, {start: 'fn a {', end: 'fn b {', extra: true}]) {
  test('present malformed scope rejects: ' + JSON.stringify(scope), () => {
    assert.throws(() => C.mutatedSource('fn a { old }\nfn b { end }', {path: a, scope, edits: [['old', 'new']]}));
  });
}
for (const [name, change] of [
  ['empty patches', m => {m.patches = [];}],
  ['duplicate paths', m => {m.patches[1].path = a;}],
  ['undeclared path', m => {m.patches[1].path = c;}],
  ['legacy path', m => {m.path = a;}],
  ['empty edits', m => {m.patches[1].edits = [];}],
  ['empty anchor', m => {m.patches[1].edits = [['', 'x']];}],
  ['missing second anchor', m => {m.patches[1].edits = [['missing', 'x']];}],
  ['unchanged patch', m => {m.patches[1].edits = [['old_b', 'new_b'], ['new_b', 'old_b']];}],
]) test('derivation rejects before writes: ' + name, () => {
  const f = fixture();
  change(f.mutation);
  assert.throws(() => {
    const files = C.mutationSources(f.map, f.originals, f.mutation, [a, b]);
    C.applyOwnedMutation(files, f.io, f.attempts);
  });
  assert.deepStrictEqual(f.writes, []);
  assert.deepStrictEqual(f.io.identities(), f.map);
});
test('two-file overlay changes exactly both files and preserves unrelated identities', () => {
  const f = fixture();
  const changed = C.mutationMap(f.map, f.files);
  assert.deepStrictEqual(changed, {...f.map, [a]: C.hash('new_a\n'), [b]: C.hash('new_b\n')});
  assert.notDeepStrictEqual({...changed, [b]: f.map[b]}, changed);
  assert.notDeepStrictEqual({...changed, [c]: C.hash('other')}, changed);
  const variant = structuredClone(f.files);
  variant[1].changed += 'extra';
  variant[1].changed_sha256 = C.hash(variant[1].changed);
  assert.notStrictEqual(C.hash(JSON.stringify(C.mutationMap(f.map, variant))), C.hash(JSON.stringify(changed)));
});
test('complete application and closed-child restoration preserve the full map', () => {
  const f = fixture(); f.apply(); f.launch();
  assert.deepStrictEqual(f.io.identities(), C.mutationMap(f.map, f.files));
  const result = f.restore();
  assert(result.restored && result.quiescent && result.all_mutant_before_restore);
  assert.strictEqual(result.error, null);
  assert.strictEqual(result.external_edit, false);
  assert(result.files.every((row, index) => row.before_sha256 === f.files[index].changed_sha256 &&
    row.after_sha256 === f.files[index].original_sha256 && row.write_attempted));
});
for (const mode of ['before-first', 'between-files', 'complete-write-then-throw', 'partial-write', 'partial-second-write']) {
  test('application failure is recoverable without overwriting unknown bytes: ' + mode, () => {
    const f = fixture(); const write = f.io.write;
    let calls = 0;
    f.io.write = (path, text) => {
      calls++;
      const fail = mode === 'between-files' || mode === 'partial-second-write' ? calls === 2 : calls === 1;
      if (!fail) return write(path, text);
      if (mode === 'complete-write-then-throw') write(path, text);
      if (mode === 'partial-write' || mode === 'partial-second-write') write(path, text.slice(0, 3));
      throw Error('application I/O failure');
    };
    assert.throws(f.apply, /application I\/O failure/);
    f.io.write = write;
    const before = structuredClone(f.source);
    const result = f.restore();
    assert.strictEqual(result.quiescent, true);
    assert.strictEqual(result.all_mutant_before_restore, false);
    if (mode === 'partial-write' || mode === 'partial-second-write') {
      assert.strictEqual(result.restored, false);
      assert.strictEqual(result.external_edit, true);
      assert.deepStrictEqual(f.source, before);
      if (mode === 'partial-second-write') assert.strictEqual(f.source[a], f.files[0].changed);
    } else assert.strictEqual(result.restored, true);
  });
}
test('unattempted planned-mutant external edit is not owned by the campaign', () => {
  const f = fixture(); f.source[b] = f.files[1].changed;
  assert.throws(f.apply, /application preflight/);
  assert.deepStrictEqual(f.attempts, []);
  const before = structuredClone(f.source);
  const result = f.restore();
  assert(!result.restored && result.external_edit);
  assert.deepStrictEqual(f.writes, []);
  assert.deepStrictEqual(f.source, before);
});
test('unvisited second-file mutant after first write is not restored', () => {
  const f = fixture(); const write = f.io.write;
  f.io.write = (path, text) => {write(path, text); if (path === a) f.source[b] = f.files[1].changed;};
  assert.throws(f.apply, /immediate pre-write identity/);
  assert.deepStrictEqual(f.attempts, [{path: a, verified: true}]);
  f.io.write = write; const before = structuredClone(f.source); const written = f.writes.length;
  const result = f.restore();
  assert(!result.restored && result.external_edit);
  assert.strictEqual(f.writes.length, written);
  assert.deepStrictEqual(f.source, before);
});
for (const where of ['before-restoration', 'during-group-scan', 'between-restoration-writes']) {
  test('detected external edits are preserved: ' + where, () => {
    const f = fixture(); f.apply(); f.launch(); const written = f.writes.length;
    if (where === 'before-restoration') f.source[b] = 'external';
    if (where === 'during-group-scan') f.io.live = () => {f.source[b] = 'external'; return [];};
    if (where === 'between-restoration-writes') {
      const write = f.io.write;
      f.io.write = (path, text) => {write(path, text); if (path === a) f.source[b] = 'external';};
    }
    const result = f.restore();
    assert(!result.restored && result.external_edit);
    assert.strictEqual(f.source[b], 'external');
    assert.strictEqual(f.writes.length - written, where === 'between-restoration-writes' ? 1 : 0);
  });
}
test('restoration I/O failure reports exact mixed state and retains recovery information', () => {
  const f = fixture(); f.apply(); f.launch(); const write = f.io.write;
  f.io.write = (path, text) => {if (path === b) throw Error('restore write failure'); write(path, text);};
  const result = f.restore();
  assert(!result.restored && result.quiescent && result.error.includes('restore write failure'));
  assert.strictEqual(result.files[0].after_sha256, f.map[a]);
  assert.strictEqual(result.files[1].after_sha256, f.files[1].changed_sha256);
  f.state.recovery = true;
  f.state.manifest = {name: 'r118b-qualification-manifest.json', sha256: C.hash('manifest')};
  f.state.recovery_record = C.recoverySources(f.state.manifest, f.files);
  f.io.write = write;
  const written = f.writes.length;
  const retry = f.restore();
  assert(retry.restored && retry.quiescent && !retry.all_mutant_before_restore);
  assert.strictEqual(retry.error, null);
  assert.strictEqual(f.writes.length, written + 1);
  assert.strictEqual(retry.files[0].write_attempted, false);
  assert.strictEqual(retry.files[1].write_attempted, true);
});
test('complete restoration followed by I/O error is physical recovery, not acceptance', () => {
  const f = fixture(); f.apply(); f.launch(); const write = f.io.write;
  f.io.write = (path, text) => {write(path, text); if (path === b) throw Error('post-write error');};
  const result = f.restore();
  assert(result.restored && result.quiescent && result.all_mutant_before_restore);
  assert(result.error.includes('post-write error'));
  assert.deepStrictEqual(f.io.identities(), f.map);
});
for (const [name, change] of [
  ['unclosed child', f => {f.state.record.child_closed = false;}],
  ['cleanup failure', f => {f.state.record.process_group_cleanup.close.status = 'error';}],
  ['recorded live child', f => {f.state.record.process_group_cleanup.close.live_members = [42];}],
  ['fresh live child', f => {f.io.live = () => [42];}],
  ['throwing scan', f => {f.io.live = () => {throw Error('scan failed');};}],
  ['malformed scan', f => {f.io.live = () => null;}],
  ['missing terminal record', f => {f.state.record = null;}],
]) test('unknown or live process state withholds all restoration: ' + name, () => {
  const f = fixture(); f.apply(); f.launch(); change(f); const written = f.writes.length;
  const result = f.restore();
  assert(!result.quiescent && !result.restored);
  assert.strictEqual(f.writes.length, written);
});
test('genuine failed spawn permits physical restoration but coercible status does not', () => {
  const f = fixture(); f.apply(); f.launch();
  f.state.record = {child_closed: true, spawn_error: 'ENOENT', child_returncode: -2,
    process_group_cleanup: {close: {status: 'not_spawned'}}};
  assert.strictEqual(C.canRestore({...f.state.record, child_returncode: '-2'}), false);
  assert.strictEqual(f.restore().restored, true);
});
test('a new unrelated source change prevents full-map restoration acceptance', () => {
  const f = fixture(); f.apply(); f.launch(); f.source[c] = 'external';
  const result = f.restore();
  assert(!result.restored && result.error.includes('entire frozen map restored'));
  assert.strictEqual(f.source[c], 'external');
});
test('already-original files require explicit authenticated recovery after a child attempt', () => {
  const f = fixture(); f.apply(); f.launch(); f.source[a] = f.files[0].original;
  assert.strictEqual(f.restore().external_edit, true);
  f.state.recovery = true;
  f.state.manifest = {name: 'r118b-qualification-manifest.json', sha256: C.hash('manifest')};
  f.state.recovery_record = C.recoverySources(f.state.manifest, f.files);
  const result = f.restore();
  assert(result.restored && !result.all_mutant_before_restore);
});
for (const [name, change] of [
  ['manifest substitution', r => {r.manifest.sha256 = C.hash('other');}],
  ['missing path', r => {r.sources.pop();}],
  ['duplicate path', r => {r.sources[1] = r.sources[0];}],
  ['wrong original hash', r => {r.sources[1].original_sha256 = C.hash('other');}],
  ['missing second mutant hash', r => {delete r.sources[1].mutated_sha256;}],
]) test('recovery rejects substituted evidence: ' + name, () => {
  const f = fixture(); const manifest = {name: 'r118b-qualification-manifest.json', sha256: C.hash('manifest')};
  const recovery = C.recoverySources(manifest, f.files); change(recovery);
  assert.throws(() => C.validateRecovery(recovery, manifest, f.files));
});
test('invalid application-attempt history is rejected before restoration', () => {
  const f = fixture(); f.apply(); f.launch(); f.state.attempts.reverse();
  const written = f.writes.length;
  assert.throws(f.restore, /ordered application attempt prefix/);
  assert.strictEqual(f.writes.length, written);
});
test('real prospective plan derives 78 executions and 74 exact maps without writes', () => {
  const map = JSON.parse(fs.readFileSync(root + 'r118b-clippy-source.json', 'utf8'));
  assert.strictEqual(C.hash(JSON.stringify(map)), P.sourceMapSha256);
  assert.strictEqual(Object.keys(map).length, P.sourceCount);
  const originals = P.mutationPaths.map(path => ({path, text: fs.readFileSync(repo + path, 'utf8')}));
  assert.strictEqual(P.newTests.length, 18);
  assert.strictEqual(new Set(P.newTests).size, 18);
  assert.strictEqual(P.mutations.length, P.expected.executions);
  assert.strictEqual(new Set(P.mutations.map(m => m.name)).size, P.expected.executions);
  const groups = new Map(); const counts = {};
  for (const mutation of P.mutations) {
    assert(P.newTests.includes(mutation.test));
    assert(mutation.expected.length > 0 && mutation.expected.every(marker => typeof marker === 'string' && marker.length > 0));
    assert(/^\s*assert(?:_eq|_ne)?!\(/.test(fs.readFileSync(repo + mutation.oracle_path, 'utf8').split('\n')[mutation.oracle_line - 1]), 'named assertion: ' + mutation.name);
    const files = C.mutationSources(map, originals, mutation, P.mutationPaths);
    const key = C.hash(JSON.stringify(C.mutationMap(map, files)));
    groups.set(key, [...(groups.get(key) || []), mutation.name]);
    counts[mutation.kind] = (counts[mutation.kind] || 0) + 1;
  }
  assert.strictEqual(groups.size, P.expected.distinctSources);
  const shared = [...groups.values()].filter(names => names.length > 1).map(names => names.sort())
    .sort((a, b) => a[0] < b[0] ? -1 : a[0] > b[0] ? 1 : 0);
  assert.deepStrictEqual(shared, P.sharedSourceGroups);
  assert.deepStrictEqual(counts, {production: 75, 'production-combined-defense': 1, 'helper-calibration': 2});
  assert.strictEqual(P.mutations.filter(m => m.patches.length === 2).length, 1);
});
