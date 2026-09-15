const V = require('./r119-integrated-campaign-validation-v1.js');
V.guard();
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {test} = require('node:test');
const p = require('./r119-integrated-campaign-plan-v1.js');
const e = require('./r119-integrated-qualification-evidence-v1.js');
const {qualifyMutation} = require('./r119-integrated-qualification-run-v1.js');
const map = e.identities();
assert.strictEqual(e.hash(JSON.stringify(map)), p.sourceMapSha256);
const originals = p.mutationPaths.map(name => ({path: name, text: fs.readFileSync(path.join(e.repo, name), 'utf8')}));
const runner = e.bytes('r119-integrated-run-v1.js');
const boot = '00000000-0000-0000-0000-000000000001';
const observation = n => ({utc_ms: 1700000000000 + n, monotonic_ns: String(n), boot_id: boot,
  clock_source: 'node-process-hrtime-linux-monotonic'});
function fixture(options = {}) {
  const mutation = p.mutations[0];
  assert(mutation);
  const files = e.mutationSources(map, originals, mutation);
  const changed = e.mutationMap(map, files);
  const name = 'r119-integrated-qualified-mut-' + mutation.name;
  const command = [...p.test, mutation.test, '--', '--exact', '--test-threads=1'];
  const manifest = {original_sources: originals, helpers: [{name: 'r119-integrated-run-v1.js', sha256: e.hash(runner)}],
    entries: {[name + '.json']: {kind: 'run', command, predecessor: 'r119-integrated-qualification-manifest.json',
      source_map_sha256: e.hash(JSON.stringify(changed)), returncode: 101}},
    clock: {source_verified: observation(1)}};
  const manifestBytes = Buffer.from(JSON.stringify(manifest));
  const bytes = new Map([['r119-integrated-run-v1.js', runner], ['r119-integrated-qualification-manifest.json', manifestBytes]]);
  const values = new Map(originals.map(source => [source.path, source.text]));
  const writes = [];
  const emitted = new Map();
  let samples = 0;
  let liveChecks = 0;
  let spawned = false;
  let rawRecord = null;
  const state = {files, changed, name, bytes, values, writes, emitted};
  const io = {
    read: name => {assert(values.has(name)); return values.get(name);},
    write: (name, text) => {
      const kind = text === originals.find(source => source.path === name).text ? 'restore' : 'apply';
      writes.push({name, kind});
      options.beforeWrite?.({...state, name, text, kind});
      values.set(name, text);
      options.afterWrite?.({...state, name, text, kind});
    },
    live: () => {liveChecks++; return options.live ? [42] : [];},
    identities: () => Object.fromEntries(Object.entries(map).map(([name, hash]) => [name, values.has(name) ? e.hash(values.get(name)) : hash])),
  };
  const run = (actualName, expected, beforeSpawn) => {
    assert.strictEqual(actualName, name);
    assert.deepStrictEqual(expected, changed);
    assert.deepStrictEqual(io.identities(), changed);
    options.beforeSpawn?.(state);
    beforeSpawn(); spawned = true;
    options.afterSpawn?.(state);
    rawRecord = {source_head: p.parent, cwd: e.repo, command,
      source_map_sha256: e.hash(JSON.stringify(changed)), log: e.file(name + '.log'),
      source: e.file(name + '-source.json'), source_after: e.file(name + '-source-after.json'),
      runner: {name: 'r119-integrated-run-v1.js', sha256: e.hash(runner)}, child_closed: true, spawn_error: null,
      process_group_cleanup: {close: {status: 'absent', pgid: 42, live_members: []}},
      clock: {contract: e.contract, source_verified: observation(20), predecessor: {
        name: 'r119-integrated-qualification-manifest.json', sha256: e.hash(manifestBytes), observation: observation(1)}}};
    options.record?.(rawRecord, state);
    bytes.set(name + '.json', Buffer.from(JSON.stringify(rawRecord)));
    bytes.set(name + '-source.json', Buffer.from(JSON.stringify(changed)));
    options.afterRecord?.(state);
    const log = ['Finished `test` profile', 'Running unittests', 'running 1 test',
      'test ' + mutation.test + ' ... FAILED',
      "thread '" + mutation.test + "' panicked at " + mutation.oracle_path + ':' + mutation.oracle_line + ':9:',
      ...mutation.expected, 'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out;', ''].join('\n');
    return {record: rawRecord, log: options.badOracle ? 'running 0 tests' : log};
  };
  let result = null;
  let error = null;
  try {
    result = qualifyMutation({manifest, manifestHash: e.hash(manifestBytes), map, mutation, io, run,
      readArtifact: name => {
        options.beforeRead?.(name, state);
        assert(bytes.has(name), 'fixture artifact exists: ' + name);
        return bytes.get(name);
      },
      write: (name, value) => {
        options.beforeArtifact?.(name, value, state);
        assert(!emitted.has(name), 'exclusive artifact');
        emitted.set(name, structuredClone(value));
      },
      sample: () => {
        samples++;
        if (options.failSample === samples) throw new Error('sample I/O failure');
        return observation(100 + samples);
      }});
  } catch (caught) {error = caught;}
  const diagnostic = error ? JSON.parse(error.message.slice(error.message.indexOf('; ') + 2)) : null;
  return {...state, result, error, diagnostic, liveChecks, spawned, restored: JSON.stringify(io.identities()) === JSON.stringify(map)};
}
function rejected(result, restored, attempted = true) {
  assert(result.error);
  assert.strictEqual(result.result, null);
  assert.strictEqual(result.restored, restored);
  assert.strictEqual(result.diagnostic.accepted, false);
  assert.strictEqual(result.diagnostic.runner_attempted, attempted);
  assert.deepStrictEqual(result.diagnostic.recovery_record.sources.map(source => source.path), result.files.map(file => file.path));
  assert(![...result.emitted.keys()].some(name => name.startsWith('r119-integrated-qualified-restoration-')));
}
test('single-file mutation is applied, checked and restored', () => {
  const result = fixture();
  assert.ifError(result.error); assert(result.restored); assert.strictEqual(result.liveChecks, 1);
  assert.deepStrictEqual(result.writes.map(write => write.kind), ['apply', 'restore']);
});
test('single-file qualification records exact application and restoration order', () => {
  const result = fixture();
  assert.ifError(result.error); assert(result.restored);
  assert.deepStrictEqual(result.writes.map(write => write.name), [...result.files, ...result.files].map(file => file.path));
  assert.deepStrictEqual(result.writes.map(write => write.kind), ['apply', 'restore']);
  assert.strictEqual(result.emitted.size, 2);
});
test('complete first application write followed by I/O failure is restored without execution', () => {
  const result = fixture({afterWrite: ({kind}) => {if (kind === 'apply') throw new Error('application write failure');}});
  rejected(result, true, false); assert.strictEqual(result.diagnostic.application_attempts.length, 1);
});
test('failure before the first application write leaves the original source intact', () => {
  const result = fixture({beforeWrite: ({kind, writes}) => {if (kind === 'apply' && writes.length === 1) throw new Error('first write failure');}});
  rejected(result, true, false); assert.strictEqual(result.diagnostic.application_attempts.length, 1);
});
test('partial first-file bytes are retained for explicit recovery', () => {
  const result = fixture({beforeWrite: ({kind, writes, values, name}) => {
    if (kind === 'apply' && writes.length === 1) {values.set(name, 'partial bytes'); throw new Error('partial write');}
  }});
  rejected(result, false, false); assert.strictEqual(result.diagnostic.restoration.external_edit, true);
});
test('failure before the spawn callback permits non-spawned source recovery', () => {
  rejected(fixture({beforeSpawn: () => {throw new Error('preflight failure');}}), true, false);
});
test('missing terminal record after spawn withholds restoration', () => {
  rejected(fixture({afterSpawn: () => {throw new Error('lost runner');}}), false);
});
test('malformed terminal record after spawn withholds restoration', () => {
  rejected(fixture({afterRecord: ({bytes, name}) => {bytes.set(name + '.json', Buffer.from('{'));}}), false);
});
test('closed raw record allows physical recovery after wrapper rejection', () => {
  rejected(fixture({afterRecord: () => {throw new Error('wrapper rejected');}}), true);
});
test('wrong behavioral oracle does not qualify a physically restored mutation', () => {
  rejected(fixture({badOracle: true}), true);
});
test('unclosed child withholds restoration', () => {
  rejected(fixture({record: record => {record.child_closed = false;}}), false);
});
test('fresh live process observation withholds restoration', () => {
  const result = fixture({live: true}); rejected(result, false); assert.strictEqual(result.liveChecks, 1);
});
for (const [name, change] of [
  ['command', record => {record.command = ['true'];}],
  ['source identity', record => {record.source_map_sha256 = '0'.repeat(64);}],
  ['predecessor', record => {record.clock.predecessor.sha256 = '0'.repeat(64);}],
]) test('foreign terminal ' + name + ' cannot authorize restoration', () => {
  const result = fixture({record: change}); rejected(result, false); assert.strictEqual(result.diagnostic.authenticated_terminal, false);
});
test('changed live runner bytes cannot authenticate a pinned terminal', () => {
  rejected(fixture({afterRecord: ({bytes}) => {bytes.set('r119-integrated-run-v1.js', Buffer.from('substituted'));}}), false);
});
test('matching substituted runner bytes and record still fail the manifest pin', () => {
  rejected(fixture({record: (record, {bytes}) => {
    const other = Buffer.from('substituted'); bytes.set('r119-integrated-run-v1.js', other); record.runner.sha256 = e.hash(other);
  }}), false);
});
for (const failSample of [1, 2]) test('clock sample failure ' + failSample + ' still restores owned sources', () => {
  rejected(fixture({failSample}), true);
});
test('post-write restoration error prevents acceptance despite complete physical recovery', () => {
  const result = fixture({afterWrite: ({kind, writes}) => {if (kind === 'restore' && writes.length === 2) throw new Error('late restoration I/O');}});
  rejected(result, true); assert.strictEqual(result.diagnostic.restoration.restored, true);
  assert.match(result.diagnostic.restoration.error, /late restoration I\/O/);
});
test('failure-artifact write failure retains full recovery diagnostics in the surfaced error', () => {
  const result = fixture({badOracle: true, beforeArtifact: () => {throw new Error('artifact write failure');}});
  rejected(result, true); assert.strictEqual(result.emitted.size, 0);
  assert(result.diagnostic.errors.some(error => error.phase === 'failure-artifact' && error.error.includes('artifact write failure')));
});
test('external source changes during child execution are not overwritten', () => {
  const result = fixture({afterRecord: ({values, files}) => {values.set(files[0].path, 'external bytes');}});
  rejected(result, false); assert.strictEqual(result.diagnostic.restoration.external_edit, true);
});
