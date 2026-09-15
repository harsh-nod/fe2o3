const assert = require('assert');
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const p = require('./r118-qualification-plan.js');
const e = require('./r118-qualification-evidence.js');
const support = require('./r118-history-support-v1.js');
const script = 'r118-freeze.js';
const helperBytes = new Map(p.freezeHelpers.map(name => [name, e.bytes(name)]));
const artifacts = new Map([...new Set([...e.historicalNames(), ...support.inputNames(), ...p.freezeHelpers])]
  .map(name => [name, e.bytes(name)]));
const expectedMap = e.identities();
const sourceBytes = new Map(Object.keys(expectedMap).map(name => [name, fs.readFileSync(path.join(e.repo, name))]));
const gitAnswers = new Map();
const captureGit = args => gitAnswers.set(JSON.stringify(args), e.git(args));
for (const name of ['r117-frozen-source.json', 'r117-environment.json', 'r117-reviewed-gnu-all.log', 'r117-reviewed-musl-all.log']) {
  captureGit(['show', p.parent + ':' + p.previous + 'raw/' + name]);
}
captureGit(['ls-tree', '-r', '-z', p.history.contexts.c1.head]);
for (const [context, names] of Object.entries(p.history.cohortSources)) {
  for (const name of names.filter(name => !p.addedSource.includes(name))) {
    for (const head of [p.history.contexts[context].head, p.parent]) captureGit(['show', head + ':' + name]);
  }
}
const binaries = ['cargo', 'rustc'].map(name => ({name, path: '/fixture/bin/' + name, sha256: e.hash(name)}));
const last = 'r118-history-preflight-v1.json';
const latest = e.read(last).clock.source_verified;
const now = BigInt(latest.monotonic_ns) + 1000000n;
let passed = 0;
const cases = [];
function fixture(name, options = {}) {
  const files = new Map(sourceBytes);
  const listed = new Set(files.keys());
  const values = new Map(artifacts);
  let head = options.openingHead || p.parent;
  const state = {files, listed, artifacts: values, changeHead: value => {head = value;}};
  const repin = name => {
    const profile = JSON.parse(values.get('r118-history-plan-v1.json'));
    const item = profile.artifacts.find(item => item.name === name);
    if (item) {
      item.sha256 = e.hash(values.get(name));
      values.set('r118-history-plan-v1.json', Buffer.from(JSON.stringify(profile)));
    }
  };
  state.changeJson = (name, change, updatePin = true) => {
    const value = JSON.parse(values.get(name));
    change(value);
    values.set(name, Buffer.from(JSON.stringify(value)));
    if (updatePin) repin(name);
  };
  state.changeLog = (name, change) => {
    values.set(name, Buffer.from(change(values.get(name).toString())));
    repin(name);
  };
  options.initial?.(state);
  let inventories = 0;
  let heads = 0;
  let observations = 0;
  const writes = [];
  const fakeFs = {
    lstatSync: file => {
      if (!values.has(path.basename(file))) throw Object.assign(new Error('missing artifact ' + file), {code: 'ENOENT'});
      return {isFile: () => path.basename(file) !== options.nonregular};
    },
    readFileSync: (file, encoding) => {
      let value;
      if (file === '/proc/sys/kernel/random/boot_id') value = latest.boot_id;
      else if (file.startsWith('/fixture/bin/')) value = path.basename(file);
      else if (file.startsWith(e.repo + '/')) {
        const name = file.slice(e.repo.length + 1);
        if (!files.has(name)) throw Object.assign(new Error('missing source ' + name), {code: 'ENOENT'});
        value = files.get(name);
      } else if (file.startsWith(e.root + '/') && values.has(path.basename(file))) value = values.get(path.basename(file));
      else throw new Error('unexpected fixture read ' + file);
      return encoding ? value.toString() : Buffer.from(value);
    },
    readdirSync: directory => {assert.strictEqual(directory, e.root); return [...values.keys()];},
    writeFileSync: (file, bytes, options) => {assert.strictEqual(options.flag, 'wx'); writes.push([file, JSON.parse(bytes)]);},
  };
  const fakeCp = {execFileSync: (command, args, executionOptions) => {
    assert.strictEqual(executionOptions.cwd, e.repo);
    if (command === 'git' && args[0] === 'rev-parse') {heads++; return Buffer.from(head);}
    if (command === 'git' && args[0] === 'ls-files') {
      assert.strictEqual(JSON.stringify(args), JSON.stringify(['ls-files', '--cached', '--others', '--exclude-standard', '-z']));
      inventories++; return Buffer.from([...listed].join('\0') + '\0');
    }
    if (command === 'git' && ['show', 'ls-tree'].includes(args[0])) {
      if (args[1] === p.parent + ':' + p.previous + 'raw/r117-environment.json') return Buffer.from(JSON.stringify({binaries}));
      const key = JSON.stringify(args);
      assert(gitAnswers.has(key), 'captured Git response');
      return Buffer.from(gitAnswers.get(key));
    }
    if (command === 'rustup') return Buffer.from('/fixture/bin/' + args.at(-1));
    if (command === 'gh') {
      observations++; options.late?.(state);
      return Buffer.from(JSON.stringify({number: 182, state: 'OPEN'}));
    }
    return Buffer.from('fixture tool observation');
  }};
  const modules = new Map();
  const context = vm.createContext({Buffer, process: {env: {}, hrtime: {bigint: () => now}}, console: {log() {}}});
  vm.runInContext('globalThis.structuredClone = value => JSON.parse(JSON.stringify(value));', context);
  function localRequire(name) {
    if (name === 'fs') return fakeFs;
    if (name === 'child_process') return fakeCp;
    if (!name.startsWith('./r118-')) return require(name);
    const file = name.slice(2);
    if (modules.has(file)) return modules.get(file).exports;
    assert(helperBytes.has(file), 'captured fixture module: ' + file);
    const module = {exports: {}};
    modules.set(file, module);
    const text = values.get(file).toString();
    if (file.endsWith('.json')) module.exports = vm.runInContext('JSON.parse(' + JSON.stringify(text) + ')', context);
    else vm.runInContext('(function(require,module,exports){\n' + text + '\n})', context, {filename: file})(localRequire, module, module.exports);
    return module.exports;
  }
  context.require = localRequire;
  let failure = null;
  try {vm.runInContext(values.get(script).toString(), context, {filename: script});} catch (error) {failure = error;}
  if (options.rejects) {
    assert(failure, name + ' unexpectedly accepted');
    assert.deepStrictEqual(writes, [], 'rejected freeze wrote output');
    if (options.oracle) assert(options.oracle.test(failure.message), name + ': ' + failure.message);
    if (options.phase === 'late') {
      assert.strictEqual(inventories, 2); assert.strictEqual(observations, 1);
    } else {
      assert.strictEqual(observations, 0);
      assert.strictEqual(inventories, options.openingHead ? 0 : 1);
    }
  } else {
    assert.ifError(failure);
    assert.deepStrictEqual(writes.map(([file]) => file), ['frozen-source', 'environment', 'issue-status'].map(name => e.file('r118-' + name + '.json')));
    assert.deepStrictEqual(writes[0][1], expectedMap);
    const environment = writes[1][1];
    assert.deepStrictEqual(environment.source_delta, p.sourceDelta);
    assert.deepStrictEqual(environment.runners, p.freezeHelpers.map(name => ({name, sha256: e.hash(values.get(name))})));
    for (const pin of environment.prerequisite_validation_inputs) assert.strictEqual(pin.sha256, e.hash(values.get(pin.name)));
    assert.strictEqual(environment.history.runs, 30);
    assert.strictEqual(environment.support_history.schema_contracts, 13);
    assert.strictEqual(inventories, 2); assert.strictEqual(heads, 2); assert.strictEqual(observations, 1);
  }
  cases.push(name); passed++; console.log('PASS ' + name);
}
const late = (name, change, oracle) => fixture(name, {rejects: true, phase: 'late', late: change, oracle});
fixture('unchanged');
late('added-source', ({files, listed}) => {files.set('scripts/freeze-added.sh', 'new'); listed.add('scripts/freeze-added.sh');}, /complete source inventory/);
late('deleted-untracked-member-test', ({files, listed}) => {files.delete(p.addedSource[0]); listed.delete(p.addedSource[0]);}, /complete source inventory/);
late('deleted-tracked-source', ({files}) => files.delete('Cargo.lock'), /missing source Cargo.lock/);
late('modified-lockfile', ({files}) => files.set('Cargo.lock', 'changed'));
late('same-count-replacement', ({files, listed}) => {
  files.delete(p.addedSource[0]); listed.delete(p.addedSource[0]); files.set('scripts/replacement.rs', 'new'); listed.add('scripts/replacement.rs');
}, /complete source inventory/);
late('late-head-change', ({changeHead}) => changeHead('f'.repeat(40)));
fixture('wrong-opening-head', {rejects: true, openingHead: 'e'.repeat(40)});
fixture('docs-only-change', {late: ({files, listed}) => {files.set('docs/not-source.md', 'new'); listed.add('docs/not-source.md');}});
for (const [name, change, oracle] of [
  ['unclosed', record => {record.child_closed = false;}, /observed child closure/],
  ['failed', record => {record.returncode = record.child_returncode = 101;}],
  ['raw-child-result', record => {record.child_returncode = 101;}, /raw child result/],
  ['wrong-source-hash', record => {record.source_map_sha256 = '0'.repeat(64);}],
  ['live-group', record => {record.process_group_cleanup.close.live_members = [42];}, /no live owned-group members/],
  ['wrong-command', record => {record.command = ['true'];}, /exact command/],
  ['deadline', record => {record.deadline_ms++;}],
  ['environment', record => {record.environment.RUST_TEST_THREADS = '1';}],
  ['runner', record => {record.runner.sha256 = '0'.repeat(64);}],
  ['path', record => {record.log += '.other';}],
  ['cwd', record => {record.cwd += '/other';}],
  ['predecessor', record => {record.clock.predecessor.sha256 = '0'.repeat(64);}],
  ['boot', record => {record.clock.finish.boot_id = '00000000-0000-0000-0000-000000000002';}, /same boot/],
  ['monotonic', record => {record.clock.finish.monotonic_ns = '0';}, /monotonic order/],
  ['cleanup-clock', record => {record.process_group_cleanup.close.observation.monotonic_ns = '0';}, /monotonic order/],
]) fixture('prerequisite-' + name, {rejects: true, oracle, initial: state => state.changeJson('r118-reviewed-gnu-all.json', change)});
fixture('prerequisite-initial-anchor-unclosed', {rejects: true, oracle: /observed child closure/,
  initial: state => state.changeJson('c1-initial-format.json', record => {record.child_closed = false;})});
for (const kind of ['before', 'after']) fixture('prerequisite-' + kind + '-source', {rejects: true,
  initial: state => state.changeJson('r118-reviewed-gnu-all-source' + (kind === 'after' ? '-after' : '') + '.json', map => {map['Cargo.lock'] = '0'.repeat(64);})});
fixture('prerequisite-nonregular', {rejects: true, nonregular: 'r118-reviewed-gnu-all.log', oracle: /regular artifact/});
fixture('prerequisite-future-verification', {rejects: true, phase: 'late', oracle: /monotonic order/,
  initial: state => state.changeJson(last, record => {
    record.clock.source_verified.monotonic_ns = (now + 1000n).toString();
    record.verification_elapsed_seconds = Number(BigInt(record.clock.source_verified.monotonic_ns) - BigInt(record.clock.finish.monotonic_ns)) / 1e9;
  })});
late('prerequisite-artifact-substitution', ({artifacts}) => artifacts.set('r118-reviewed-gnu-all.log', Buffer.from('substituted')), /validated input unchanged/);
fixture('raw-utc-regression', {initial: state => state.changeJson(last, record => {
  record.clock.finish.utc_ms = record.clock.start.utc_ms - 10;
  record.finished_at = new Date(record.clock.finish.utc_ms).toISOString();
})});
fixture('history-old-boot-remains-independent');
fixture('history-missing-artifact', {rejects: true, initial: ({artifacts}) => artifacts.delete('c1-initial-tests.log'), oracle: /missing artifact/});
fixture('history-nonregular-artifact', {rejects: true, nonregular: 'c1-initial-tests.log', oracle: /regular artifact/});
for (const [name, artifact] of [
  ['c1-sparse-map', 'c1-corrected-tests-source.json'],
  ['c2-formatter-record', 'c2candidate-initial-format.json'],
  ['c3-runner', 'c3candidate-run-v1.js'],
  ['core-input-pins', 'r118-mutation-core-inputs-v1.json'],
  ['plan', 'r118-history-plan-v1.json'],
]) late('history-late-' + name + '-substitution', ({artifacts}) => artifacts.set(artifact, Buffer.from('substituted')), /validated input unchanged/);
fixture('support-unclosed', {rejects: true, oracle: /observed child closure/,
  initial: state => state.changeJson('r118-runner-contract-tests.json', record => {record.child_closed = false;})});
fixture('support-failed-parse-relabelled', {rejects: true,
  initial: state => state.changeJson('r118-history-schema-contracts-v1.json', record => {record.returncode = record.child_returncode = 0;})});
fixture('support-missing-artifact', {rejects: true, oracle: /missing artifact/,
  initial: ({artifacts}) => artifacts.delete('r118-history-schema-contracts-v1.log')});
fixture('support-altered-input', {rejects: true, oracle: /support input manifest/,
  initial: state => state.changeJson('r118-history-schema-inputs-v2.json', value => {value.scope = 'changed';})});
for (const [kind, log] of [['full', 'r118-reviewed-gnu-all.log'], ['runtime', 'r118-reviewed-runtime.log']]) {
  fixture('positive-' + kind + '-failed-row', {rejects: true, oracle: /no contradictory failed test rows/,
    initial: state => state.changeLog(log, text => text + '\ntest invented::contradiction ... FAILED\n')});
  fixture('positive-' + kind + '-failed-summary', {rejects: true, oracle: /no contradictory failed test summary/,
    initial: state => state.changeLog(log, text => text.replace('test result: ok.', 'test result: FAILED.'))});
}
fixture('history-compile-failure-relabelled', {rejects: true,
  initial: state => state.changeJson('c1-initial-tests.json', record => {record.returncode = record.child_returncode = 0;})});
fixture('history-formatter-flattened', {rejects: true,
  initial: state => state.changeJson('c2candidate-initial-format.json', record => {record.source_unchanged = true;})});
fixture('history-sparse-array', {rejects: true, oracle: /stage-zero source descriptor/,
  initial: state => {
    const spec = Object.values(p.history.maps).find(spec => spec.schema === 'sparse-v1');
    state.changeJson(spec.file, map => {
      const name = Object.keys(map).find(name => Object.hasOwn(map[name], 'unmaterialized_index_entry'));
      map[name].unmaterialized_index_entry = [map[name].unmaterialized_index_entry];
    });
  }});
assert.strictEqual(passed, p.freezeContractCount);
assert.strictEqual(new Set(cases).size, passed);
console.log('PASS: ' + passed + ' freeze contract tests');
