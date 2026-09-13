const assert = require('assert');
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const vm = require('vm');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const script = path.join(root, 'r113-freeze-v2.js');
const source = fs.readFileSync(script, 'utf8');
const parent = 'a2feef229758381b66f962ad8be2b87843eb33a3';
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const delta = ['crates/fe2o3-runtime-model/src/context_version_journal.rs',
  'crates/fe2o3-runtime-model/src/context_version_journal/membership_tests.rs',
  'crates/fe2o3-runtime-model/src/context_version_journal/tests.rs'];
function fixture(name, mutate, rejects, openingHead = parent) {
  const files = new Map([['Cargo.lock', 'lock'], ...delta.map(file => [file, 'candidate'])]);
  for (let index = 0; files.size < 5680; index++) files.set('fixture/source-' + index + '.rs', String(index));
  const listed = new Set(files.keys());
  const expectedMap = Object.fromEntries([...files].sort(([a], [b]) => a.localeCompare(b)).map(([file, bytes]) => [file, hash(bytes)]));
  const previousMap = {...expectedMap};
  for (const file of delta) previousMap[file] = hash('parent');
  delete previousMap[delta[1]];
  const binaries = ['cargo', 'rustc'].map(name => ({name, path: '/fixture/bin/' + name, sha256: hash(name)}));
  let head = openingHead;
  let inventories = 0;
  let headChecks = 0;
  let observations = 0;
  const writes = [];
  const fakeFs = {
    readFileSync: file => {
      if (file.includes('/raw/r112-environment.json')) return JSON.stringify({binaries});
      if (file.includes('/raw/r112-frozen-source.json')) return JSON.stringify(previousMap);
      if (file === '/proc/sys/kernel/random/boot_id') return '00000000-0000-0000-0000-000000000001';
      if (file.startsWith('/fixture/bin/')) return path.basename(file);
      if (file.startsWith(repo + '/')) {
        const relative = file.slice(repo.length + 1);
        if (!files.has(relative)) throw Object.assign(new Error('missing source ' + relative), {code: 'ENOENT'});
        return files.get(relative);
      }
      if (file.startsWith(root + '/r113-')) return 'helper-or-prior-artifact';
      throw Error('unexpected read ' + file);
    },
    readdirSync: directory => {assert.strictEqual(directory, root); return [];},
    writeFileSync: (file, bytes, options) => {assert.strictEqual(options.flag, 'wx'); writes.push([file, JSON.parse(bytes)]);},
  };
  const fakeCp = {execFileSync: (command, args, options) => {
    assert.strictEqual(options.cwd, repo);
    if (command === 'git' && args[0] === 'rev-parse') {headChecks++; return Buffer.from(head);}
    if (command === 'git' && args[0] === 'ls-files') {
      assert.strictEqual(JSON.stringify(args), JSON.stringify(['ls-files', '--cached', '--others', '--exclude-standard', '-z']));
      inventories++;
      return Buffer.from([...listed].join('\0') + '\0');
    }
    if (command === 'rustup') return Buffer.from('/fixture/bin/' + args.at(-1));
    if (command === 'gh') {
      observations++;
      mutate({files, listed, changeHead: value => {head = value;}});
      return Buffer.from(JSON.stringify({number: 182, state: 'OPEN'}));
    }
    return Buffer.from('fixture tool observation');
  }};
  let failure = null;
  try {
    vm.runInNewContext(source, {require: module => module === 'fs' ? fakeFs : module === 'child_process' ? fakeCp : require(module),
      process: {env: {}, hrtime: {bigint: () => 1000n}}, console: {log: () => {}}}, {filename: script});
  } catch (error) {failure = error;}
  if (rejects) {
    assert(failure, name + ' was accepted');
    assert.deepStrictEqual(writes, [], 'rejection wrote partial freeze artifacts');
  } else {
    assert.ifError(failure);
    assert.strictEqual(writes.length, 3);
    assert.deepStrictEqual(writes[0][1], expectedMap);
    assert.strictEqual(writes[1][1].publication_parent, parent);
    assert.strictEqual(writes[1][1].accepted_runtime_checkpoint, parent);
    assert.deepStrictEqual(writes[1][1].source_delta, delta);
    assert.strictEqual(inventories, 2);
    assert.strictEqual(headChecks, 2);
    assert.strictEqual(observations, 1);
  }
  console.log('PASS ' + name);
}
fixture('unchanged', () => {}, false);
fixture('added-source', ({files, listed}) => {files.set('scripts/freeze-added.sh', 'new'); listed.add('scripts/freeze-added.sh');}, true);
fixture('deleted-untracked-member-test', ({files, listed}) => {files.delete(delta[1]); listed.delete(delta[1]);}, true);
fixture('deleted-tracked-source', ({files}) => files.delete('Cargo.lock'), true);
fixture('modified-lockfile', ({files}) => files.set('Cargo.lock', 'changed'), true);
fixture('same-count-replacement', ({files, listed}) => {
  files.delete(delta[1]); listed.delete(delta[1]); files.set('scripts/replacement.rs', 'candidate'); listed.add('scripts/replacement.rs');
}, true);
fixture('late-head-change', ({changeHead}) => changeHead('f'.repeat(40)), true);
fixture('wrong-opening-head', () => {}, true, 'e'.repeat(40));
fixture('docs-only-change', ({files, listed}) => {files.set('docs/not-source.md', 'new'); listed.add('docs/not-source.md');}, false);
console.log('PASS: 9 freeze contract tests');
