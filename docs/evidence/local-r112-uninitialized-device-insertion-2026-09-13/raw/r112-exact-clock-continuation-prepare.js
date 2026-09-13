const c = require('./r112-exact-clock-continuation-evidence.js');
const p = require('./r112-exact-qualification-plan.js');
const old = require('./r112-exact-clock-evidence.js');
const {fs, assert} = c;
const tests = c.prefix + 'contract-tests';
const testRun = c.read(tests + '.json');
assert.strictEqual(testRun.returncode, 0);
assert.strictEqual(testRun.signal, null);
assert.strictEqual(testRun.timed_out, false);
assert.strictEqual(testRun.source_unchanged, true);
assert.strictEqual(testRun.source_head, p.parent);
assert.deepStrictEqual(testRun.command, ['node', c.file(c.prefix + 'evidence-tests.js')]);
assert.deepStrictEqual(c.read(tests + '-source.json'), c.read('r112-frozen-source.json'));
const testLog = fs.readFileSync(c.file(tests + '.log'), 'utf8');
assert(testLog.endsWith('PASS: all 16 continuation contract tests\n'));
assert.strictEqual(testLog.split('\n').filter(l => l.startsWith('PASS: ')).length, 17);
const helpers = c.pins(c.helpers);
assert.deepStrictEqual(JSON.parse(testLog.split('\n').find(l => l.startsWith('HELPER_PINS: ')).slice(13)), helpers);
const original = c.read('r112-exact-clock-manifest.json');
c.checkPins([...original.helpers, ...original.historical_artifacts, ...original.contract_tests]);
const rejectedName = 'r112-exact-clock-restored-allocator';
const rejected = c.read(rejectedName + '.json');
assert.strictEqual(rejected.returncode, 0);
assert.strictEqual(rejected.signal, null);
assert.strictEqual(rejected.timed_out, false);
assert.strictEqual(rejected.source_unchanged, true);
assert.strictEqual(rejected.clock.error, 'AssertionError [ERR_ASSERTION]: raw child-close UTC regression');
assert(rejected.clock.finish.utc_ms < rejected.clock.start.samples.at(-1).utc_ms);
assert(BigInt(rejected.clock.finish.monotonic_ns) >= BigInt(rejected.clock.start.samples.at(-1).monotonic_ns));
assert.deepStrictEqual(c.read(rejectedName + '-source.json'), c.read('r112-frozen-source.json'));
const before = c.identities();
assert.deepStrictEqual(before, c.read('r112-frozen-source.json'));
assert.strictEqual(c.git(['rev-parse', 'HEAD']).trim(), p.parent);
const prior = fs.readdirSync(c.root).filter(n => /^r112-.*\.(json|log|py|js)$/.test(n) && !n.startsWith(c.prefix)).sort();
const entries = {};
let previous = c.manifestName;
for (const [kind, filter] of p.focused.slice(1)) {
  const name = c.prefix + 'restored-' + kind + '.json';
  entries[name] = {command: [...p.tests, filter], predecessor: previous};
  previous = name;
}
entries[c.prefix + 'collector-validation.json'] = {command: ['node', c.file(c.prefix + 'retain-local.js'), '--check'], predecessor: previous};
const bootId = c.boot();
const manifest = {contract: c.contract, source_head: p.parent, source_map_sha256: c.hash(JSON.stringify(before)),
  original_manifest_sha256: c.hash(fs.readFileSync(c.file('r112-exact-clock-manifest.json'))),
  helpers, prior_artifacts: c.pins(prior), contract_tests: c.pins(['.json', '.log', '-source.json'].map(s => tests + s)),
  excluded: {name: rejectedName, child_returncode: 0, wrapper_exit_code: 125, reason: rejected.clock.error},
  bridge: 'Exact closed original artifacts and freshly checked frozen source; no retroactive boot binding for original records.',
  unchanged: {child_deadline_ms: 1800000, source_tests: true, original_helpers: true, original_manifest: true},
  boot_id: bootId, entries, observation: c.observation(bootId)};
c.valid(manifest.observation, bootId);
fs.writeFileSync(c.file(c.manifestName), JSON.stringify(manifest, null, 2) + '\n', {flag: 'wx'});
console.log('Pinned ' + helpers.length + ' continuation helpers, ' + prior.length + ' original artifacts and ' + Object.keys(entries).length + ' continuation commands');
