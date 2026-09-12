const fs = require('fs');
const cp = require('child_process');
const assert = require('assert');
const c = require('./r108-clock-evidence.js');
const read = name => JSON.parse(fs.readFileSync(c.root + name));
const repo = c.root + 'fe2o3-r61-execution';
const helpers = ['r108-clock-check-mutation.js', 'r108-clock-evidence-tests.js', 'r108-clock-evidence.js',
  'r108-clock-prepare.js', 'r108-clock-restoration.js', 'r108-clock-retain-local.js', 'r108-clock-run.js'];
(async () => {
  cp.execFileSync('node', [c.root + 'r108-assert-frozen.js']);
  const sourceHead = cp.execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repo}).toString().trim();
  assert.strictEqual(sourceHead, 'f767fad35d24f585e8ecb95558bce97a9d543cfe');
  const baseline = read('r108-frozen-source.json');
  const sourceHash = c.hash(JSON.stringify(baseline));
  const testLog = fs.readFileSync(c.root + 'r108-clock-contract-tests.log', 'utf8');
  const testedPins = testLog.match(/^HELPER_PINS: (.+)$/m);
  assert(testedPins);
  const pinnedHelpers = helpers.map(name => ({name, sha256: c.hash(fs.readFileSync(c.root + name))}));
  assert.deepStrictEqual(JSON.parse(testedPins[1]), pinnedHelpers, 'tested external helpers');
  const testRun = read('r108-clock-contract-tests.json');
  assert.strictEqual(testRun.returncode, 0);
  assert.strictEqual(testRun.source_unchanged, true);
  const mutations = read('r108-mutations.json');
  const model = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-runtime-model', '--lib'];
  const entries = {};
  let predecessor = {name: 'r108-clock-manifest.json', field: 'recorded_at'};
  for (const m of mutations) {
    let candidate = fs.readFileSync(repo + '/' + m.path, 'utf8');
    for (const [from, to] of m.edits) { assert.strictEqual(candidate.split(from).length, 2); candidate = candidate.replace(from, to); }
    const inputs = {...baseline, [m.path]: c.hash(candidate)};
    const runName = 'r108-clock-mut-' + m.name + '.json';
    entries[runName] = {kind: 'run', source_map_sha256: c.hash(JSON.stringify(inputs)),
      command: [...model, m.test, '--', '--exact'], returncode: 101, predecessor};
    const restorationName = 'r108-clock-restoration-' + m.name + '.json';
    entries[restorationName] = {kind: 'restoration', source_map_sha256: sourceHash,
      predecessor: {name: runName, field: 'finished_at'}};
    predecessor = {name: restorationName, field: 'verified_at'};
  }
  for (const [kind, filter] of [['model', 'context_version_journal'], ['credits', 'r67_resource_credits'], ['batch', 'r70_resource_batch']]) {
    const name = 'r108-clock-restored-' + kind + '.json';
    entries[name] = {kind: 'run', source_map_sha256: sourceHash, command: [...model, filter], returncode: 0, predecessor};
    predecessor = {name, field: 'finished_at'};
  }
  entries['r108-clock-collector-validation.json'] = {kind: 'run', source_map_sha256: sourceHash,
    command: ['node', c.root + 'r108-clock-retain-local.js', '--check'], returncode: 0, predecessor};
  const names = fs.readdirSync(c.root).filter(n => /^r108-.*\.(json|log|py|js)$/.test(n) && !n.startsWith('r108-clock-')).sort();
  assert.strictEqual(names.length, 210);
  const floors = [
    ['r108-source-campaign.json', 'finished_at'], ['r108-accepted-auxiliary-campaign.json', 'finished_at'],
    ...['model', 'credits', 'batch'].map(n => ['r108-frozen-' + n + '.json', 'finished_at']),
    ['r108-environment.json', 'recorded_at'], ['r108-mutation-tool-pins.json', 'recorded_at'],
    ['r108-accepted-revalidation.json', 'recorded_at'],
    ...mutations.flatMap(m => [['r108-repeat-mut-' + m.name + '.json', 'finished_at'], ['r108-restoration-repeat-' + m.name + '.json', 'verified_at']]),
    ['r108-clock-contract-tests.json', 'finished_at'],
  ].map(([name, field]) => {
    const bytes = fs.readFileSync(c.root + name);
    const utc = Date.parse(JSON.parse(bytes)[field]); assert(Number.isSafeInteger(utc));
    return {name, field, sha256: c.hash(bytes), utc_ms: utc};
  });
  const observation = await c.gate(Math.max(...floors.map(f => f.utc_ms)));
  const manifest = {recorded_at: new Date(observation.samples.at(-1).utc_ms).toISOString(),
    source_head: sourceHead, source_map_sha256: sourceHash, entries,
    helpers: pinnedHelpers,
    historical_artifacts: names.map(name => ({name, sha256: c.hash(fs.readFileSync(c.root + name))})),
    prerequisite_floors: floors,
    contract_tests: ['r108-clock-contract-tests.json', 'r108-clock-contract-tests.log', 'r108-clock-contract-tests-source.json']
      .map(name => ({name, sha256: c.hash(fs.readFileSync(c.root + name))})),
    test_environment_changed: false, test_deadlines_changed: false,
    clock: {contract: c.contract, policy: c.policy, kind: 'manifest', observation}};
  fs.writeFileSync(c.root + 'r108-clock-manifest.json', JSON.stringify(manifest, null, 2) + '\n', {flag: 'wx'});
  console.log('Pinned ' + helpers.length + ' helpers, ' + names.length + ' historical artifacts and ' + Object.keys(entries).length + ' causal-chain entries');
})().catch(error => {console.error(error); process.exitCode = 1;});
