const assert = require('assert');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const vm = require('vm');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const script = path.join(root, 'r115-freeze.js');
const source = fs.readFileSync(script, 'utf8');
const p = require('./r115-qualification-plan.js');
const {parent, accepted, sourceDelta: delta} = p;
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const helpers = new Map(p.freezeHelpers.map(name => [name, fs.readFileSync(path.join(root, name), 'utf8')]));
const boot = '00000000-0000-0000-0000-000000000001';
const observation = n => ({utc_ms: 1700000000000 + n, monotonic_ns: String(n), boot_id: boot,
  clock_source: 'node-process-hrtime-linux-monotonic'});
let passed = 0;
function fixture(name, mutate, rejects, openingHead = parent, prerequisite = value => value) {
  const files = new Map([['Cargo.lock', 'lock'], ...delta.map(file => [file, 'candidate'])]);
  for (let index = 0; files.size < p.sourceCount; index++) files.set('fixture/source-' + index + '.rs', String(index));
  const listed = new Set(files.keys());
  const expectedMap = Object.fromEntries([...files].sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0)
    .map(([file, bytes]) => [file, hash(bytes)]));
  const previousMap = {...expectedMap};
  for (const file of delta) previousMap[file] = hash('parent');
  for (const file of p.addedSource) delete previousMap[file];
  const binaries = ['cargo', 'rustc'].map(name => ({name, path: '/fixture/bin/' + name, sha256: hash(name)}));
  const artifacts = new Map(helpers);
  const prior = {source_head: parent, clock: {contract: 'r115-raw-utc-boot-monotonic-v1', error: null, source_verified: observation(0)}};
  artifacts.set(p.prerequisites[0][2], JSON.stringify(prior));
  for (const [index, [run, command, previous]] of p.prerequisites.entries()) {
    const prior = JSON.parse(artifacts.get(previous));
    const start = observation(1 + index * 4);
    const finish = observation(2 + index * 4);
    const close = observation(3 + index * 4);
    const verified = observation(4 + index * 4);
    const record = prerequisite({source_head: parent, source_map_sha256: hash(JSON.stringify(expectedMap)), cwd: repo, command,
      log: path.join(root, run + '.log'), source: path.join(root, run + '-source.json'), source_after: path.join(root, run + '-source-after.json'),
      source_unchanged: true, returncode: 0, child_returncode: 0, child_closed: true, spawn_error: null, signal: null, timed_out: false,
      deadline_ms: 1800000, environment: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null},
      runner: {name: 'r115-run-v4.js', sha256: hash(helpers.get('r115-run-v4.js'))},
      process_group_cleanup: {timeout: null, close: {status: 'absent', pgid: 42, error: 'Error: kill ESRCH', live_members: [], observation: close}},
      clock: {contract: 'r115-raw-utc-boot-monotonic-v1', error: null, start, finish, source_verified: verified,
        predecessor: {name: previous, sha256: hash(artifacts.get(previous)), observation: prior.clock.source_verified}},
    }, run);
    record.started_at = new Date(record.clock.start.utc_ms).toISOString();
    record.finished_at = new Date(record.clock.finish.utc_ms).toISOString();
    record.elapsed_seconds = Number(BigInt(record.clock.finish.monotonic_ns) - BigInt(record.clock.start.monotonic_ns)) / 1e9;
    record.verification_elapsed_seconds = Number(BigInt(record.clock.source_verified.monotonic_ns) - BigInt(record.clock.finish.monotonic_ns)) / 1e9;
    artifacts.set(run + '.json', JSON.stringify(record));
    artifacts.set(run + '.log', 'fixture log');
    for (const suffix of ['-source.json', '-source-after.json']) {
      const substituted = (name === 'prerequisite-after-source' && suffix === '-source-after.json') ||
        (name === 'prerequisite-before-source' && suffix === '-source.json');
      artifacts.set(run + suffix, JSON.stringify(substituted ? {...expectedMap, 'Cargo.lock': hash('substituted')} : expectedMap));
    }
  }
  let head = openingHead;
  let inventories = 0;
  let headChecks = 0;
  let observations = 0;
  const writes = [];
  const fakeFs = {
    lstatSync: file => ({isFile: () => !(name === 'prerequisite-nonregular' && file.endsWith('gnu-all.log'))}),
    readFileSync: file => {
      if (file.includes('/raw/r114-environment.json')) return JSON.stringify({binaries});
      if (file.includes('/raw/r114-frozen-source.json')) return JSON.stringify(previousMap);
      if (file === '/proc/sys/kernel/random/boot_id') return boot;
      if (file.startsWith('/fixture/bin/')) return path.basename(file);
      if (file.startsWith(repo + '/')) {
        const relative = file.slice(repo.length + 1);
        if (!files.has(relative)) throw Object.assign(new Error('missing source ' + relative), {code: 'ENOENT'});
        return files.get(relative);
      }
      if (file.startsWith(root + '/') && artifacts.has(path.basename(file))) return artifacts.get(path.basename(file));
      throw Error('unexpected read ' + file);
    },
    readdirSync: directory => {assert.strictEqual(directory, root); return [...artifacts.keys()];},
    writeFileSync: (file, bytes, options) => {assert.strictEqual(options.flag, 'wx'); writes.push([file, JSON.parse(bytes)]);},
  };
  const fakeCp = {execFileSync: (command, args, options) => {
    assert.strictEqual(options.cwd, repo);
    if (command === 'git' && args[0] === 'rev-parse') {headChecks++; return Buffer.from(head);}
    if (command === 'git' && args[0] === 'ls-files') {
      assert.strictEqual(JSON.stringify(args), JSON.stringify(['ls-files', '--cached', '--others', '--exclude-standard', '-z']));
      inventories++; return Buffer.from([...listed].join('\0') + '\0');
    }
    if (command === 'rustup') return Buffer.from('/fixture/bin/' + args.at(-1));
    if (command === 'gh') {
      observations++; mutate({files, listed, artifacts, changeHead: value => {head = value;}});
      return Buffer.from(JSON.stringify({number: 182, state: 'OPEN'}));
    }
    return Buffer.from('fixture tool observation');
  }};
  const modules = new Map();
  const context = vm.createContext({process: {env: {}, hrtime: {bigint: () => 1000n}}, console: {log: () => {}}});
  function localRequire(name) {
    if (name === 'fs') return fakeFs;
    if (name === 'child_process') return fakeCp;
    if (!name.startsWith('./r115-')) return require(name);
    const file = name.slice(2);
    if (modules.has(file)) return modules.get(file).exports;
    assert(helpers.has(file), 'captured fixture module');
    const module = {exports: {}};
    modules.set(file, module);
    const wrapper = vm.runInContext('(function(require,module,exports){\n' + helpers.get(file) + '\n})', context, {filename: file});
    wrapper(localRequire, module, module.exports);
    return module.exports;
  }
  context.require = localRequire;
  let failure = null;
  try {vm.runInContext(source, context, {filename: script});} catch (error) {failure = error;}
  if (rejects) {
    assert(failure, name + ' was accepted');
    assert.deepStrictEqual(writes, [], 'rejection wrote partial freeze artifacts');
    if (name.startsWith('prerequisite-')) {
      const late = ['prerequisite-future-verification', 'prerequisite-artifact-substitution'].includes(name);
      assert.strictEqual(inventories, late ? 2 : 1); assert.strictEqual(observations, late ? 1 : 0); assert.strictEqual(headChecks, late ? 2 : 1);
      assert.strictEqual(failure.code, 'ERR_ASSERTION');
      if (late) assert(failure.message.includes(name.endsWith('substitution') ? 'validated prerequisite input unchanged' : 'monotonic order'));
    } else if (name === 'wrong-opening-head') {
      assert.strictEqual(inventories, 0); assert.strictEqual(observations, 0); assert.strictEqual(headChecks, 1);
      assert.strictEqual(failure.actual, openingHead); assert.strictEqual(failure.expected, parent);
    } else {
      assert.strictEqual(inventories, 2); assert.strictEqual(observations, 1);
      if (['added-source', 'deleted-untracked-member-test', 'same-count-replacement'].includes(name)) {
        assert(failure.message.includes('complete source inventory remains unchanged'));
      } else if (name === 'deleted-tracked-source') {
        assert.strictEqual(failure.code, 'ENOENT'); assert(failure.message.includes('Cargo.lock'));
      } else if (name === 'modified-lockfile') {
        assert.strictEqual(failure.actual['Cargo.lock'], hash('changed')); assert.strictEqual(failure.expected['Cargo.lock'], hash('lock'));
      } else if (name === 'late-head-change') {
        assert.strictEqual(headChecks, 2); assert.strictEqual(failure.actual, 'f'.repeat(40)); assert.strictEqual(failure.expected, parent);
      } else throw Error('unbound negative oracle ' + name);
    }
  } else {
    assert.ifError(failure);
    assert.deepStrictEqual(writes.map(([file]) => file), ['frozen-source', 'environment', 'issue-status'].map(n => path.join(root, 'r115-' + n + '.json')));
    const env = writes[1][1];
    assert.deepStrictEqual(env.runners, p.freezeHelpers.map(name => ({name, sha256: hash(helpers.get(name))})));
    assert.deepStrictEqual(writes[0][1], expectedMap);
    assert.strictEqual(env.publication_parent, parent); assert.strictEqual(env.accepted_runtime_checkpoint, accepted);
    assert.deepStrictEqual(env.source_delta, delta); assert.strictEqual(env.prerequisite_artifacts.length, 8);
    for (const pin of env.prerequisite_validation_inputs) assert.strictEqual(pin.sha256, hash(artifacts.get(pin.name)));
    assert.strictEqual(inventories, 2); assert.strictEqual(headChecks, 2); assert.strictEqual(observations, 1);
  }
  passed++; console.log('PASS ' + name);
}
fixture('unchanged', () => {}, false);
fixture('added-source', ({files, listed}) => {files.set('scripts/freeze-added.sh', 'new'); listed.add('scripts/freeze-added.sh');}, true);
fixture('deleted-untracked-member-test', ({files, listed}) => {files.delete(p.addedSource[1]); listed.delete(p.addedSource[1]);}, true);
fixture('deleted-tracked-source', ({files}) => files.delete('Cargo.lock'), true);
fixture('modified-lockfile', ({files}) => files.set('Cargo.lock', 'changed'), true);
fixture('same-count-replacement', ({files, listed}) => {
  files.delete(p.addedSource[1]); listed.delete(p.addedSource[1]); files.set('scripts/replacement.rs', 'candidate'); listed.add('scripts/replacement.rs');
}, true);
fixture('late-head-change', ({changeHead}) => changeHead('f'.repeat(40)), true);
fixture('wrong-opening-head', () => {}, true, 'e'.repeat(40));
fixture('docs-only-change', ({files, listed}) => {files.set('docs/not-source.md', 'new'); listed.add('docs/not-source.md');}, false);
for (const [name, modify] of [
  ['unclosed', r => {r.child_closed = false;}],
  ['failed', r => {r.returncode = r.child_returncode = 101;}],
  ['raw-child-result', r => {r.child_returncode = 101;}],
  ['wrong-source-hash', r => {r.source_map_sha256 = '0'.repeat(64);}],
  ['live-group', r => {r.process_group_cleanup.close.live_members = [42];}],
  ['wrong-command', r => {r.command = ['true'];}],
  ['deadline', r => {r.deadline_ms++;}],
  ['environment', r => {r.environment.RUST_TEST_THREADS = '1';}],
  ['runner', r => {r.runner.sha256 = '0'.repeat(64);}],
  ['path', r => {r.log += '.other';}],
  ['cwd', r => {r.cwd += '/other';}],
  ['predecessor', r => {r.clock.predecessor.sha256 = '0'.repeat(64);}],
  ['boot', r => {r.clock.finish.boot_id = '00000000-0000-0000-0000-000000000002';}],
  ['monotonic', r => {r.clock.finish.monotonic_ns = '0';}],
  ['cleanup-clock', r => {r.process_group_cleanup.close.observation.monotonic_ns = '100';}],
]) fixture('prerequisite-' + name, () => {}, true, parent, r => {modify(r); return r;});
fixture('prerequisite-after-source', () => {}, true);
fixture('prerequisite-before-source', () => {}, true);
fixture('prerequisite-nonregular', () => {}, true);
fixture('prerequisite-future-verification', () => {}, true, parent, (r, name) => {
  if (name.includes('musl')) r.clock.source_verified.monotonic_ns = '2000'; return r;
});
fixture('prerequisite-artifact-substitution', ({artifacts}) => artifacts.set('r115-preliminary-gnu-all.log', 'substituted'), true);
fixture('raw-utc-regression', () => {}, false, parent, r => {r.clock.finish.utc_ms = r.clock.start.utc_ms - 10; return r;});
assert.strictEqual(passed, p.freezeContractCount);
console.log('PASS: ' + passed + ' freeze contract tests');
