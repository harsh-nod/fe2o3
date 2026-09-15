const assert = require('assert');
const p = require('./r118-qualification-plan.js');
const e = require('./r118-qualification-evidence.js');
const h = require('./r118-history-v3.js');
const suffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const runs = [
  ['r118-runner-contract-tests', ['node', e.file('r118-runner-tests.js')], 0],
  ['r118-history-schema-contracts-v1', ['node', '--test', e.file('r118-history-schema-tests-v1.js')], 1],
  ['r118-history-schema-contracts-v2', ['node', '--test', e.file('r118-history-schema-tests-v2.js')], 0],
  ['r118-history-preflight-v1', ['node', e.file('r118-history-preflight-v1.js')], 0],
];
const inputManifests = [
  ['r118-history-schema-inputs-v1.json', 'ae55f9e24c66dbaf0d00ebf9a9fa785474c48621d03698dcecd0d3e80309271d'],
  ['r118-history-schema-inputs-v2.json', 'a481afd8b2e694d3845ae7558c624c22925496d212eab4d214b880e1f6891438'],
  ['r118-history-preflight-inputs-v1.json', '2ce672632929db466ef31e43a5ca5c4f08c9ef9e8e117905808289b3e3b1ba64'],
];
function inputNames(io = e) {
  return [...new Set([
    ...runs.flatMap(([name]) => suffixes.map(suffix => name + suffix)),
    ...['clean', 'success', 'failure', 'inner-timeout', 'spawn'].flatMap(kind =>
      suffixes.map(suffix => 'r118-runner-v1-fixture-' + kind + suffix)),
    'r118-runner-tests.js',
    ...inputManifests.flatMap(([name]) => [name, ...io.read(name).helpers.map(helper => helper.name)]),
  ])].sort();
}
function check(map, history, io = e) {
  for (const [name, digest] of inputManifests) {
    assert.strictEqual(e.hash(io.bytes(name)), digest, 'support input manifest');
    const manifest = io.read(name);
    assert.strictEqual(manifest.parent, p.parent);
    assert.strictEqual(manifest.source_map_sha256, p.sourceMapSha256);
    for (const helper of manifest.helpers) assert.strictEqual(e.hash(io.bytes(helper.name)), helper.sha256, helper.name);
  }
  assert.strictEqual(e.hash(io.bytes('r118-runner-tests.js')), '7a390ecd90d5a141d9572a65cda3b14825bcd232b2be96cca02b1ca6d7c652b4');
  let previous = 'r118-mutation-core-contracts-v1.json';
  for (const [name, command, code] of runs) {
    e.checkRecord(io.read(name + '.json'), name, command, map, previous, code, io);
    previous = name + '.json';
  }
  const runner = io.bytes(runs[0][0] + '.log').toString();
  assert.deepStrictEqual(runner.trimEnd().split('\n'), [
    ...['real-clean', 'real-success', 'real-failure', 'real-inner-timeout', 'real-spawn',
      'cleanup-eperm-retains-success', 'outer-timeout-retains-signal', 'cleanup-esrch-is-benign',
      'timeout-eperm-unclosed-is-bounded'].map(name => 'PASS ' + name),
    'PASS: 9 runner contract tests',
  ]);
  const failure = io.bytes(runs[1][0] + '.log').toString();
  assert(failure.includes('SyntaxError: Invalid or unexpected token'));
  assert(/^not ok 1 - \/home\/harsh\/\.codex-tmp\/r118-history-schema-tests-v1\.js$/m.test(failure));
  assert(!/^ok \d+ - /m.test(failure));
  for (const line of ['1..1', '# tests 1', '# pass 0', '# fail 1', '# cancelled 0', '# skipped 0', '# todo 0']) {
    assert(failure.split('\n').includes(line), 'retained parse-failure summary');
  }
  const passing = h.tapNames(io.bytes(runs[2][0] + '.log').toString(), 13);
  assert.strictEqual(e.hash(JSON.stringify(passing)), '88e64e9b29981cc7b7171911f6820b6b8ec572e28e3e67763535f529831ca9a8');
  assert.deepStrictEqual(JSON.parse(io.bytes(runs[3][0] + '.log')), history);
  for (const name of inputNames(io)) io.bytes(name);
  return {runs: runs.map(([name]) => name), runner_contracts: 9, schema_contracts: 13,
    preserved_parse_failures: 1, historical_preflight: true, previous};
}
module.exports = {runs, inputManifests, inputNames, check};
