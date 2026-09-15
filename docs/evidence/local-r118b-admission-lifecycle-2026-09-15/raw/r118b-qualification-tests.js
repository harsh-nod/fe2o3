const assert = require('assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const p = require('./r118b-qualification-plan.js');
const e = require('./r118b-qualification-evidence.js');
const c = require('./r118b-qualification-collect.js');
const l = require('./r118b-qualification-launch-v1.js');
const prep = require('./r118b-qualification-prepare.js');
const cases = [];
const tests = [];
function check(name, test) {cases.push(name); tests.push(test);}
let cached;
function data() {
  if (cached) return cached;
  const map = e.read('r118b-frozen-source.json');
  const originals = p.mutationPaths.map(name => ({path: name, text: fs.readFileSync(path.join(e.repo, name), 'utf8')}));
  cached = {map, originals, entries: e.expectedEntries(map, originals),
    names: e.passing(e.bytes('r118b-reviewed-gnu-all.log').toString()),
    helperBytes: new Map([...p.helpers, 'r118b-qualification-launch-v1.js'].map(name => [name, e.bytes(name)]))};
  return cached;
}
const obs = n => ({utc_ms: 1700000000000 + n, monotonic_ns: String(n),
  boot_id: '00000000-0000-0000-0000-000000000001', clock_source: 'node-process-hrtime-linux-monotonic'});
function fixture() {
  const d = data();
  const values = new Map(d.helperBytes);
  const previous = 'r118b-qualification-contract-tests.json';
  values.set(previous, Buffer.from(JSON.stringify({clock: {source_verified: obs(1)}})));
  values.set('fixture-history.json', Buffer.from('retained history'));
  const names = [...values.keys()].sort();
  const helpers = p.helpers.map(name => ({name, sha256: e.hash(values.get(name))}));
  const launcher = {name: 'r118b-qualification-launch-v1.js', sha256: e.hash(values.get('r118b-qualification-launch-v1.js'))};
  const base = {map: {...d.map}, helpers, launcher, previous, allNames: d.names, cases,
    env: {}, history: e.read('r118b-environment.json').history};
  const manifest = {source_head: p.parent, source_map_sha256: p.sourceMapSha256, helpers: structuredClone(helpers),
    launcher: {...launcher}, original_sources: structuredClone(d.originals), mutations: structuredClone(p.mutations),
    entries: structuredClone(d.entries), historical_artifacts: names.map(name => ({name, sha256: e.hash(values.get(name))})),
    clock: {contract: e.contract, error: null, kind: 'manifest', source_verified: obs(2),
      predecessor: {name: previous, sha256: e.hash(values.get(previous)), observation: obs(1)}},
    recorded_at: new Date(obs(2).utc_ms).toISOString()};
  const state = {base, manifest, values, names, occupied: new Set(), current: base.map, writes: [], spawns: [], callbacks: 0, mutations: 0};
  const publish = () => values.set('r118b-qualification-manifest.json', Buffer.from(JSON.stringify(manifest)));
  const io = {...e, bytes: name => {assert(values.has(name), 'fixture artifact ' + name); return values.get(name);},
    historicalNames: () => names.slice(), identities: () => state.current,
    stat: name => {if (state.occupied.has(name)) return {isSymbolicLink: () => true}; throw Object.assign(new Error('absent'), {code: 'ENOENT'});},
    baseline: () => base, log() {}};
  io.read = name => JSON.parse(io.bytes(name));
  publish();
  return {...state, state, io, publish};
}
function mutationLog(mutation) {
  return ['Finished `test` profile', 'Running unittests', 'running 1 test',
    'test ' + mutation.test + ' ... FAILED',
    "thread '" + mutation.test + "' panicked at " + mutation.oracle_path + ':' + mutation.oracle_line + ':9:',
    ...mutation.expected, 'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out;', ''].join('\n');
}
function positiveLog(names) {
  return names.map(name => 'test ' + name + ' ... ok').join('\n') + '\n' +
    'test result: ok. ' + names.length + ' passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;\n';
}
function load(name, overrides) {
  const module = {exports: {}};
  const localRequire = name => Object.hasOwn(overrides, name) ? overrides[name] : require(name.startsWith('./r118b-') ? e.file(name.slice(2)) : name);
  vm.runInThisContext('(function(require,module,exports){\n' + e.bytes(name).toString() + '\n})', {filename: name})(localRequire, module, module.exports);
  return module.exports;
}
function launchFixture(options = {}) {
  const f = fixture();
  const {state, io, values} = f;
  options.initial?.(f);
  f.publish();
  io.log = line => options.logged?.(f, line);
  io.run = (manifest, name, expected, beforeSpawn) => {
    options.beforeCallback?.(f, name);
    beforeSpawn(); state.spawns.push(name);
    const focused = p.focused.find(([kind]) => name === 'r118b-qualified-restored-' + kind);
    const names = focused ? state.base.allNames.filter(test => focused[3] ? test === focused[1] : test.startsWith(focused[1])) : [];
    const result = {record: {name}, log: focused ? positiveLog(names) : '{}'};
    values.set(name + '.json', Buffer.from(JSON.stringify(result.record)));
    values.set(name + '.log', Buffer.from(result.log));
    for (const suffix of ['-source.json', '-source-after.json']) values.set(name + suffix, Buffer.from(JSON.stringify(expected)));
    options.afterRun?.(f, name);
    return result;
  };
  io.qualify = ({manifest, map, mutation, run}) => {
    state.mutations++;
    state.current = e.mutationMap(map, e.mutationSources(map, manifest.original_sources, mutation));
    let record;
    try {
      options.applied?.(f, mutation);
      run('r118b-qualified-mut-' + mutation.name, state.current, () => {state.callbacks++;});
      record = {mutation: mutation.name};
      values.set('r118b-qualified-restoration-' + mutation.name + '.json', Buffer.from(JSON.stringify(record)));
      values.set('r118b-qualified-restoration-' + mutation.name + '-source.json', Buffer.from(JSON.stringify(map)));
    } finally {state.current = map;}
    options.afterMutation?.(f, mutation);
    return record;
  };
  let error = null;
  try {l.launch(io);} catch (caught) {error = caught;}
  return {...f, error};
}
for (const mutation of p.mutations) check('exact-oracle-' + mutation.name, () => e.checkMutation(mutationLog(mutation), mutation));
for (const [name, change] of [
  ['wrong-panic', text => text.replace(' panicked at ', ' panicked elsewhere ')],
  ['zero-tests', text => text.replace('running 1 test', 'running 0 tests')],
  ['successful-summary', text => text.replace('test result: FAILED.', 'test result: ok.')],
  ['compile-error', text => text + '\nerror[E0308]: compile failure\n'],
  ['extra-failure', text => text + '\ntest unrelated::test ... FAILED\n'],
]) check('mutation-rejects-' + name, () => assert.throws(() => e.checkMutation(change(mutationLog(p.mutations[0])), p.mutations[0])));
check('manifest-valid', () => {const f = fixture(); c.validateManifest(f.manifest, f.base, f.io);});
for (const [name, modify] of [
  ['coherent-other-map', f => {f.base.map = {...f.base.map, 'Cargo.lock': '0'.repeat(64)}; f.manifest.source_map_sha256 = e.hash(JSON.stringify(f.base.map));}],
  ['helper', f => {f.values.set(p.helpers[0], Buffer.from('substituted'));}],
  ['launcher', f => {f.manifest.launcher.sha256 = '0'.repeat(64);}],
  ['predecessor', f => {f.manifest.clock.predecessor.sha256 = '0'.repeat(64);}],
  ['history-membership', f => {f.manifest.historical_artifacts.pop();}],
  ['history-bytes', f => {f.values.set('fixture-history.json', Buffer.from('changed'));}],
  ['entries', f => {delete f.manifest.entries[Object.keys(f.manifest.entries)[0]];}],
  ['original-source', f => {f.manifest.original_sources[0].text += '\n';}],
  ['clock', f => {f.manifest.clock.source_verified.monotonic_ns = '0';}],
]) check('launch-preflight-rejects-' + name, () => {
  const result = launchFixture({initial: modify}); assert(result.error); assert.strictEqual(result.state.mutations, 0); assert.strictEqual(result.state.spawns.length, 0);
});
check('launch-exact-78-plus-six-plus-collector-sequence', () => {
  const f = launchFixture(); assert.ifError(f.error);
  assert.deepStrictEqual(f.state.spawns, p.mutations.map(m => 'r118b-qualified-mut-' + m.name)
    .concat(p.focused.map(([kind]) => 'r118b-qualified-restored-' + kind), ['r118b-qualified-collector-validation']));
  assert.strictEqual(f.state.callbacks, 78); assert.deepStrictEqual(f.state.current, f.base.map);
});
for (const suffix of ['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json']) {
  check('occupied-run-output-' + suffix, () => {
    const f = launchFixture({initial: f => f.state.occupied.add('r118b-qualified-mut-' + p.mutations[0].name + suffix)});
    assert(f.error); assert.strictEqual(f.state.mutations, 0);
  });
}
for (const suffix of ['.json', '-source.json']) check('occupied-restoration-' + suffix, () => {
  const f = launchFixture({initial: f => f.state.occupied.add('r118b-qualified-restoration-' + p.mutations[0].name + suffix)});
  assert(f.error); assert.strictEqual(f.state.mutations, 0);
});
check('occupied-failure-output', () => {
  const f = launchFixture({initial: f => f.state.occupied.add('r118b-qualification-failure-' + p.mutations[0].name + '.json')});
  assert(f.error); assert.strictEqual(f.state.mutations, 0);
});
check('dangling-symlink-is-occupied', () => assert.throws(() => l.assertVacant('dangling', () => ({isSymbolicLink: () => true})), /already exists/));
check('late-freshness-failure-never-calls-before-spawn', () => {
  const f = launchFixture({beforeCallback: (f, name) => f.state.occupied.add(name + '-gate-failure.json')});
  assert(f.error); assert.strictEqual(f.state.callbacks, 0); assert.strictEqual(f.state.spawns.length, 0);
});
check('guard-after-application-uses-mutant-map', () => {
  const f = launchFixture({applied: f => {assert.notDeepStrictEqual(f.state.current, f.base.map);}});
  assert.ifError(f.error);
});
for (const at of [1, 78]) check('helper-change-after-mutation-' + at, () => {
  const f = launchFixture({afterMutation: f => {if (f.state.mutations === at) f.values.set(p.helpers[0], Buffer.from('changed'));}});
  assert(f.error); assert.strictEqual(f.state.spawns.length, at);
});
for (const kind of ['identity', 'reservation']) check('helper-change-after-focused-' + kind, () => {
  const f = launchFixture({afterRun: (f, name) => {if (name === 'r118b-qualified-restored-' + kind) f.values.set(p.helpers[0], Buffer.from('changed'));}});
  assert(f.error); assert(!f.state.spawns.includes('r118b-qualified-collector-validation'));
});
for (const [name, artifact] of [
  ['log', 'r118b-qualified-mut-' + p.mutations[0].name + '.log'],
  ['source', 'r118b-qualified-mut-' + p.mutations[0].name + '-source.json'],
  ['restoration-predecessor', 'r118b-qualified-restoration-' + p.mutations[0].name + '.json'],
]) check('completed-output-change-' + name, () => {
  const f = launchFixture({beforeCallback: (f, run) => {
    if (run === 'r118b-qualified-mut-' + p.mutations[1].name) f.values.set(artifact, Buffer.from('changed'));
  }});
  assert(f.error); assert.strictEqual(f.state.spawns.length, 1); assert.strictEqual(f.state.callbacks, 1);
});
for (const suffix of ['.json', '.log', '-source.json', '-source-after.json']) check('retain-rejects-run-substitution-' + suffix, () => {
  const f = launchFixture({afterRun: (f, name) => f.values.set(name + suffix,
    Buffer.from(suffix === '.log' ? 'changed' : JSON.stringify({changed: true})))});
  assert.match(String(f.error), /validated output bytes/);
  assert.strictEqual(f.state.mutations, 1); assert.strictEqual(f.state.spawns.length, 1);
});
for (const suffix of ['.json', '-source.json']) check('retain-rejects-restoration-substitution-' + suffix, () => {
  const f = launchFixture({afterMutation: (f, mutation) => f.values.set('r118b-qualified-restoration-' + mutation.name + suffix,
    Buffer.from(JSON.stringify({changed: true})))});
  assert.match(String(f.error), /validated output bytes/);
  assert.strictEqual(f.state.mutations, 1); assert.strictEqual(f.state.spawns.length, 1);
});
for (const occupied of [false, true]) check('pre-effect-guard-' + (occupied ? 'occupied-output' : 'changed-completed'), () => {
  const f = launchFixture({logged: f => {
    if (occupied) f.state.occupied.add('r118b-qualified-restoration-' + p.mutations[1].name + '.json');
    else f.values.set('r118b-qualified-mut-' + p.mutations[0].name + '.log', Buffer.from('changed'));
  }});
  assert(f.error); assert.strictEqual(f.state.mutations, 1); assert.strictEqual(f.state.spawns.length, 1);
});
function prepareFixture(options = {}) {
  const f = fixture(); f.values.delete('r118b-qualification-manifest.json');
  const io = {...f.io, source: name => data().originals.find(source => source.path === name).text,
    baseline: () => {options.baseline?.(f); return f.base;},
    sample: () => {options.late?.(f); return obs(2);},
    write: (name, manifest) => {f.state.writes.push({name, manifest});}};
  options.initial?.(f);
  let error = null;
  try {prep.prepare(io);} catch (caught) {error = caught;}
  return {...f, error};
}
check('prepare-single-qualified-manifest', () => {
  const f = prepareFixture(); assert.ifError(f.error); assert.strictEqual(f.state.writes.length, 1);
  c.validateManifest(f.state.writes[0].manifest, f.base, f.io);
});
for (const [name, modify] of [
  ['helper-after-baseline', f => f.values.set(p.helpers[0], Buffer.from('changed'))],
  ['launcher', f => f.values.set('r118b-qualification-launch-v1.js', Buffer.from('changed'))],
  ['new-history', f => {f.names.push('late.json'); f.values.set('late.json', Buffer.from('{}'));}],
  ['predecessor', f => f.values.set(f.base.previous, Buffer.from(JSON.stringify({clock: {source_verified: obs(0)}})))],
  ['output', f => f.state.occupied.add('r118b-qualified-mut-' + p.mutations[0].name + '.log')],
  ['source', f => {f.state.current = {...f.base.map, 'Cargo.lock': '0'.repeat(64)};}],
]) check('prepare-rejects-late-' + name, () => {
  const f = prepareFixture({late: modify}); assert(f.error); assert.strictEqual(f.state.writes.length, 0);
});
check('prepare-rejects-change-during-baseline', () => {
  const f = prepareFixture({baseline: f => f.values.set(p.helpers[0], Buffer.from('changed'))});
  assert(f.error); assert.strictEqual(f.state.writes.length, 0);
});
function runEntryFixture(options = {}) {
  const events = [];
  const command = ['cargo', 'fixture'];
  const name = 'r118b-qualified-mut-fixture';
  const entry = {kind: 'run', command, predecessor: 'r118b-qualification-manifest.json', source_map_sha256: e.hash(JSON.stringify(data().map)), returncode: 101};
  const manifest = {entries: {[name + '.json']: entry}};
  const runner = load('r118b-qualification-run.js', {
    fs: {existsSync: file => Boolean(options.occupied && file.endsWith(options.occupied))},
    child_process: {spawnSync: (executable, args, settings) => {
      events.push('spawn'); assert.deepStrictEqual(events, ['callback', 'spawn']);
      assert.strictEqual(executable, process.execPath);
      assert.deepStrictEqual(args, [e.file('r118b-run-v1.js'), name, entry.predecessor, ...command]);
      assert.deepStrictEqual(settings, {cwd: e.repo, encoding: 'utf8', maxBuffer: 4 * 1024 * 1024});
      if (options.throws) throw new Error('spawn throw');
      return {error: null, signal: null, status: 101, stdout: '', stderr: '', ...options.result};
    }},
    './r118b-qualification-evidence.js': {...e, checkRun: () => {events.push('record'); if (options.recordFailure) throw new Error('bad terminal'); return 'checked';}},
  });
  let error = null;
  let result;
  try {result = runner.runEntry(manifest, name, data().map, () => {if (options.callbackFailure) throw new Error('guard'); events.push('callback');});}
  catch (caught) {error = caught;}
  return {events, error, result};
}
check('run-entry-exact-spawn-and-callback', () => {const f = runEntryFixture(); assert.ifError(f.error); assert.strictEqual(f.result, 'checked');});
for (const [name, options] of [
  ['occupied', {occupied: '.log'}], ['guard', {callbackFailure: true}],
  ['throw', {throws: true}], ['error', {result: {error: new Error('spawn error')}}],
  ['signal', {result: {signal: 'SIGTERM'}}], ['status', {result: {status: 0}}], ['terminal', {recordFailure: true}],
]) check('run-entry-rejects-' + name, () => {const f = runEntryFixture(options); assert(f.error); if (['occupied', 'guard'].includes(name)) assert.deepStrictEqual(f.events, []);});
check('campaign-transcript-valid', () => c.checkCampaignTranscript('{"name":"fixture"}\nPASS: 5692 auxiliary source identities unchanged\n', [{name: 'fixture'}], true));
for (const text of ['{}\n', '{}\nPASS: incorrect\n', '{}\nPASS: 5692 auxiliary source identities unchanged\n']) {
  check('campaign-transcript-rejects-' + cases.length, () => assert.throws(() => c.checkCampaignTranscript(text, [{name: 'fixture'}], true)));
}
function collectorFixture() {
  const f = fixture();
  const {io, values, manifest, base} = f;
  const put = (name, value) => values.set(name, Buffer.isBuffer(value) ? value : Buffer.from(JSON.stringify(value)));
  put('r118b-environment.json', base.env); put('r118b-frozen-source.json', base.map);
  f.names.push('r118b-environment.json', 'r118b-frozen-source.json'); f.names.sort();
  manifest.historical_artifacts = f.names.map(name => ({name, sha256: e.hash(values.get(name))}));
  f.publish();
  const manifestHash = e.hash(io.bytes('r118b-qualification-manifest.json'));
  let tick = 2;
  const next = () => obs(++tick);
  const run = (name, map, log) => {
    const entry = manifest.entries[name + '.json'];
    const priorBytes = io.bytes(entry.predecessor);
    const start = next(), finish = next(), cleanup = next(), verified = next();
    const record = {source_head: p.parent, cwd: e.repo, command: entry.command,
      log: e.file(name + '.log'), source: e.file(name + '-source.json'), source_after: e.file(name + '-source-after.json'),
      source_unchanged: true, source_map_sha256: e.hash(JSON.stringify(map)), returncode: entry.returncode,
      child_returncode: entry.returncode, child_closed: true, spawn_error: null, signal: null, timed_out: false, deadline_ms: 1800000,
      environment: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null},
      runner: {name: 'r118b-run-v1.js', sha256: e.hash(io.bytes('r118b-run-v1.js'))},
      process_group_cleanup: {timeout: null, close: {status: 'absent', pgid: 12345, live_members: [], error: 'ESRCH', observation: cleanup}},
      started_at: new Date(start.utc_ms).toISOString(), finished_at: new Date(finish.utc_ms).toISOString(),
      elapsed_seconds: 1e-9, verification_elapsed_seconds: 2e-9,
      clock: {contract: e.contract, error: null, start, finish, source_verified: verified,
        predecessor: {name: entry.predecessor, sha256: e.hash(priorBytes), observation: JSON.parse(priorBytes).clock.source_verified}}};
    put(name + '.json', record); put(name + '.log', Buffer.from(log));
    const mapBytes = Buffer.from(JSON.stringify(map));
    put(name + '-source.json', mapBytes); put(name + '-source-after.json', mapBytes);
    return record;
  };
  for (const mutation of p.mutations) {
    const files = e.mutationSources(base.map, manifest.original_sources, mutation);
    const name = 'r118b-qualified-mut-' + mutation.name;
    const record = run(name, e.mutationMap(base.map, files), mutationLog(mutation));
    const restoration = {source_head: p.parent, manifest_sha256: manifestHash,
      source_map_sha256: p.sourceMapSha256, source_unchanged: true, source_identities: p.sourceCount,
      mutated_sources: files.map(source => ({path: source.path, sha256: source.changed_sha256})),
      application_attempts: files.map(source => ({path: source.path, verified: true})),
      restoration: {restored: true, quiescent: true, all_mutant_before_restore: true, live_members: [], external_edit: false, error: null,
        files: files.map(source => ({path: source.path, before_sha256: source.changed_sha256, after_sha256: source.original_sha256, write_attempted: true}))},
      clock: {contract: e.contract, error: null, kind: 'restoration', start: next(), source_verified: next(),
        predecessor: {name: name + '.json', sha256: e.hash(io.bytes(name + '.json')), observation: record.clock.source_verified}}};
    put('r118b-qualified-restoration-' + mutation.name + '.json', restoration);
    put('r118b-qualified-restoration-' + mutation.name + '-source.json', base.map);
  }
  for (const focused of p.focused) run('r118b-qualified-restored-' + focused[0], base.map,
    positiveLog(base.allNames.filter(name => focused[3] ? name === focused[1] : name.startsWith(focused[1]))));
  run('r118b-qualified-collector-validation', base.map, '{}');
  io.checkRun = (name, command, map, previous, code = 0) => {
    const record = io.read(name + '.json');
    e.checkRecord(record, name, command, map, previous, code, io);
    return {record, log: io.bytes(name + '.log').toString()};
  };
  const collector = load('r118b-qualification-collect.js', {'./r118b-qualification-evidence.js': io});
  const close = () => {
    const open = collector.collect(false, () => base);
    put('r118b-qualified-collector-validation.log', Buffer.from(JSON.stringify(open.summary)));
    return collector.collect(true, () => base);
  };
  return {...f, collector, put, close};
}
check('collector-complete-open-and-closed-cohort', () => {
  const f = collectorFixture(); const result = f.close();
  assert.strictEqual(result.summary.compiled_negatives, 78); assert.strictEqual(result.summary.distinct_mutations, 74);
  assert.strictEqual(result.names.length, 163);
  assert.strictEqual(result.captured.size, f.names.length + 1 + 78 * 6 + 6 * 4 + 4);
  for (const key of ['formal_qualification', 'native_qualification', 'performance_qualification', 'production_semantics_changed', 'total_memory_qualification']) {
    assert.strictEqual(result.summary[key], false);
  }
});
for (const [name, modify] of [
  ['wrong-oracle', f => f.put('r118b-qualified-mut-' + p.mutations[0].name + '.log', Buffer.from('compile failure'))],
  ['missing-restoration', f => f.values.delete('r118b-qualified-restoration-' + p.mutations[0].name + '.json')],
  ['failed-focused', f => f.put('r118b-qualified-restored-identity.log', Buffer.from(positiveLog(f.base.allNames.filter(name => name.startsWith(p.focused[0][1]))) + 'test result: FAILED. 0 passed; 1 failed; 0 ignored;\n'))],
  ['closed-transcript', () => {}],
]) check('collector-rejects-' + name, () => {
  const f = collectorFixture(); modify(f);
  assert.throws(() => f.collector.collect(name === 'closed-transcript', () => f.base));
});
for (const membership of [false, true]) check('collector-rejects-late-' + (membership ? 'membership' : 'cohort-bytes'), () => {
  const f = collectorFixture();
  assert.throws(() => f.collector.collect(false, () => {
    if (membership) f.names.push('late-history.json');
    else {
      const name = 'r118b-qualified-restoration-' + p.mutations[0].name + '.json';
      f.put(name, Buffer.from(f.io.bytes(name).toString() + '\n'));
    }
    return f.base;
  }));
});
let cachedRestoration;
check('final-captured-byte-check-rejects-semantically-identical-source-map', () => {
  const f = collectorFixture();
  const artifact = 'r118b-qualified-mut-' + p.mutations[0].name + '-source-after.json';
  assert.throws(() => f.collector.collect(false, () => {
    f.put(artifact, Buffer.from(f.io.bytes(artifact).toString() + '\n'));
    return f.base;
  }), /unchanged collection input: .*source-after\.json/, 'semantic equality must not bypass captured-byte equality');
});
function restorationFixture() {
  if (cachedRestoration) return {restoration: structuredClone(cachedRestoration.restoration), args: cachedRestoration.args};
  const f = collectorFixture();
  const mutation = p.mutations.find(mutation => mutation.patches.length === 2);
  assert(mutation, 'one two-file mutation');
  const name = 'r118b-qualified-mut-' + mutation.name + '.json';
  const restoration = f.io.read('r118b-qualified-restoration-' + mutation.name + '.json');
  const files = e.mutationSources(f.base.map, f.manifest.original_sources, mutation);
  const args = [files, f.base.map, e.hash(f.io.bytes('r118b-qualification-manifest.json')),
    {name, sha256: e.hash(f.io.bytes(name))}, f.io.read(name), f.base.map];
  cachedRestoration = {restoration, args};
  return {restoration: structuredClone(restoration), args};
}
check('strict-two-file-restoration-valid', () => {const f = restorationFixture(); e.checkRestoration(f.restoration, ...f.args);});
for (const [name, modify] of [
  ['manifest', r => {r.manifest_sha256 = '0'.repeat(64);}],
  ['source-count', r => {r.source_identities--;}],
  ['mutated-membership', r => {r.mutated_sources.pop();}],
  ['application-order', r => {r.application_attempts.reverse();}],
  ['application-unverified', r => {r.application_attempts[0].verified = false;}],
  ['not-restored', r => {r.restoration.restored = false;}],
  ['not-quiescent', r => {r.restoration.quiescent = false;}],
  ['partial-before-map', r => {r.restoration.all_mutant_before_restore = false;}],
  ['live-process', r => {r.restoration.live_members.push(12345);}],
  ['external-edit', r => {r.restoration.external_edit = true;}],
  ['write-error', r => {r.restoration.error = 'write failed after restoration';}],
  ['wrong-restored-hash', r => {r.restoration.files[1].after_sha256 = '0'.repeat(64);}],
  ['no-write', r => {r.restoration.files[0].write_attempted = false;}],
  ['predecessor', r => {r.clock.predecessor.sha256 = '0'.repeat(64);}],
  ['wrong-clock-kind', r => {r.clock.kind = 'manifest';}],
]) check('strict-restoration-rejects-' + name, () => {
  const f = restorationFixture(); modify(f.restoration); assert.throws(() => e.checkRestoration(f.restoration, ...f.args));
});
function archiveFixture(result, collector, corrupt = false) {
  const files = new Map(); const directories = [];
  const io = {mkdir: name => directories.push(name), write: (name, value) => {
    assert(!files.has(name)); files.set(name, Buffer.from(value));
  }, read: name => corrupt ? Buffer.from('corrupted copy') : files.get(name)};
  return {files, directories, run: () => collector.archive(result, io)};
}
check('archive-copies-captured-cohort-not-live-substitutions', () => {
  const f = collectorFixture(); const result = f.close();
  const name = 'r118b-qualified-mut-' + p.mutations[0].name + '.log';
  const expected = Buffer.from(result.captured.get(name)); f.put(name, Buffer.from('later live substitution'));
  const archive = archiveFixture(result, f.collector); const written = archive.run();
  assert.deepStrictEqual(archive.files.get(path.join(written.archive, 'raw', name)), expected);
  assert.strictEqual(written.artifacts, result.captured.size); assert.strictEqual(archive.directories.length, 2);
  for (const pin of written.summary.artifacts) assert.strictEqual(e.hash(archive.files.get(path.join(written.archive, 'raw', pin.name))), pin.sha256);
});
check('archive-rejects-readback-corruption', () => {
  const f = collectorFixture(); const archive = archiveFixture(f.close(), f.collector, true);
  assert.throws(archive.run, /exact archived bytes/);
  assert(![...archive.files.keys()].some(name => name.endsWith('/summary.json')));
});
check('archive-rejects-missing-captured-artifact-before-writing', () => {
  const f = collectorFixture(); const result = f.close(); result.captured.delete('r118b-qualification-manifest.json');
  const archive = archiveFixture(result, f.collector);
  assert.throws(archive.run, /complete validated archive snapshot/);
  assert.strictEqual(archive.files.size, 0); assert.strictEqual(archive.directories.length, 0);
});
function main() {
  const helpers = p.helpers.map(name => ({name, sha256: e.hash(e.bytes(name))}));
  const launcher = {name: 'r118b-qualification-launch-v1.js', sha256: e.hash(e.bytes('r118b-qualification-launch-v1.js'))};
  for (const [index, test] of tests.entries()) {test(); console.log('PASS ' + cases[index]);}
  for (const pin of [...helpers, launcher]) assert.strictEqual(e.hash(e.bytes(pin.name)), pin.sha256);
  assert.strictEqual(new Set(cases).size, cases.length);
  console.log('HELPER_PINS: ' + JSON.stringify(helpers));
  console.log('LAUNCHER_PIN: ' + JSON.stringify(launcher));
  console.log('PASS: ' + cases.length + ' qualification contract tests');
}
module.exports = {cases};
if (require.main === module) main();
