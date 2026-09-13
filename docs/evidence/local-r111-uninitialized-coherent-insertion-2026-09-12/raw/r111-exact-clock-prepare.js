const fs = require('fs');
const cp = require('child_process');
const assert = require('assert');
const c = require('./r111-exact-clock-evidence.js');
const p = require('./r111-exact-qualification-plan.js');
const read = name => JSON.parse(fs.readFileSync(c.root + name));
const repo = c.root + 'fe2o3-r61-execution';
(async () => {
  cp.execFileSync('node', [c.root + 'r111-assert-frozen.js']);
  const sourceHead = cp.execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repo}).toString().trim();
  assert.strictEqual(sourceHead, p.parent);
  const baseline = read('r111-frozen-source.json');
  const sourceHash = c.hash(JSON.stringify(baseline));
  const testLog = fs.readFileSync(c.root + 'r111-exact-clock-contract-tests.log', 'utf8');
  const testedPins = testLog.match(/^HELPER_PINS: (.+)$/m);
  assert(testedPins);
  const pinnedHelpers = p.helpers.map(name => ({name, sha256: c.hash(fs.readFileSync(c.root + name))}));
  assert.deepStrictEqual(JSON.parse(testedPins[1]), pinnedHelpers, 'tested external helpers');
  const testRun = read('r111-exact-clock-contract-tests.json');
  assert.strictEqual(testRun.returncode, 0);
  assert.strictEqual(testRun.source_unchanged, true);
  const entries = {};
  let predecessor = {name: 'r111-exact-clock-manifest.json', field: 'recorded_at'};
  for (const m of p.mutations) {
    let candidate = fs.readFileSync(repo + '/' + m.path, 'utf8');
    for (const [from, to] of m.edits) { assert.strictEqual(candidate.split(from).length, 2); candidate = candidate.replace(from, to); }
    const inputs = {...baseline, [m.path]: c.hash(candidate)};
    const runName = 'r111-exact-clock-mut-' + m.name + '.json';
    entries[runName] = {kind: 'run', source_map_sha256: c.hash(JSON.stringify(inputs)),
      command: [...p.tests, m.test, '--', '--exact'], returncode: 101, predecessor};
    const restorationName = 'r111-exact-clock-restoration-' + m.name + '.json';
    entries[restorationName] = {kind: 'restoration', source_map_sha256: sourceHash,
      predecessor: {name: runName, field: 'finished_at'}};
    predecessor = {name: restorationName, field: 'verified_at'};
  }
  for (const [kind, filter] of p.focused) {
    const name = 'r111-exact-clock-restored-' + kind + '.json';
    entries[name] = {kind: 'run', source_map_sha256: sourceHash, command: [...p.tests, filter], returncode: 0, predecessor};
    predecessor = {name, field: 'finished_at'};
  }
  entries['r111-exact-clock-collector-validation.json'] = {kind: 'run', source_map_sha256: sourceHash,
    command: ['node', c.root + 'r111-exact-clock-retain-local.js', '--check'], returncode: 0, predecessor};
  const names = fs.readdirSync(c.root).filter(n => /^r111-.*\.(json|log|py|js)$/.test(n) && !n.startsWith('r111-exact-clock-')).sort();
  const floorRoster = [
    ['r111-source-campaign.json', 'finished_at'], ['r111-auxiliary-campaign.json', 'finished_at'],
    ...p.focused.map(([name]) => ['r111-frozen-' + name + '.json', 'finished_at']),
    ['r111-environment.json', 'recorded_at'], ['r111-exact-clock-contract-tests.json', 'finished_at'],
  ];
  const floors = floorRoster.map(([name, field]) => {
    const bytes = fs.readFileSync(c.root + name);
    const record = JSON.parse(bytes);
    if (field === 'finished_at') {
      assert.strictEqual(record.returncode, 0);
      assert.strictEqual(record.source_unchanged, true);
      assert.strictEqual(record.timed_out, false);
      assert.strictEqual(record.signal, null);
    }
    const utc = Date.parse(record[field]); assert(Number.isSafeInteger(utc));
    return {name, field, sha256: c.hash(bytes), utc_ms: utc};
  });
  const observation = await c.gate(Math.max(...floors.map(f => f.utc_ms)));
  const manifest = {recorded_at: new Date(observation.samples.at(-1).utc_ms).toISOString(),
    source_head: sourceHead, source_map_sha256: sourceHash, entries,
    helpers: pinnedHelpers,
    historical_artifacts: names.map(name => ({name, sha256: c.hash(fs.readFileSync(c.root + name))})),
    prerequisite_floors: floors,
    contract_tests: ['r111-exact-clock-contract-tests.json', 'r111-exact-clock-contract-tests.log', 'r111-exact-clock-contract-tests-source.json']
      .map(name => ({name, sha256: c.hash(fs.readFileSync(c.root + name))})),
    test_environment_changed: false, test_deadlines_changed: false,
    clock: {contract: c.contract, policy: c.policy, kind: 'manifest', observation}};
  fs.writeFileSync(c.root + 'r111-exact-mutations.json', JSON.stringify(p.mutations, null, 2) + '\n', {flag: 'wx'});
  manifest.historical_artifacts.push({name: 'r111-exact-mutations.json', sha256: c.hash(fs.readFileSync(c.root + 'r111-exact-mutations.json'))});
  manifest.historical_artifacts.sort((a, b) => a.name < b.name ? -1 : a.name > b.name ? 1 : 0);
  fs.writeFileSync(c.root + 'r111-exact-clock-manifest.json', JSON.stringify(manifest, null, 2) + '\n', {flag: 'wx'});
  console.log('Pinned ' + p.helpers.length + ' helpers, ' + manifest.historical_artifacts.length + ' historical artifacts and ' + Object.keys(entries).length + ' causal-chain entries');
})().catch(error => {console.error(error); process.exitCode = 1;});
