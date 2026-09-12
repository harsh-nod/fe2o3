const fs = require('fs');
const cp = require('child_process');
const assert = require('assert');
const c = require('./r110-exact-clock-evidence.js');
const name = process.argv[2];
assert(/^[a-z0-9-]+$/.test(name));
(async () => {
  const repo = c.root + 'fe2o3-r61-execution';
  const baseline = JSON.parse(fs.readFileSync(c.root + 'r110-frozen-source.json'));
  const planned = c.plan('r110-exact-clock-restoration-' + name + '.json');
  const predecessor = c.predecessor('r110-exact-clock-mut-' + name + '.json', 'finished_at', planned.expected);
  const run = JSON.parse(fs.readFileSync(c.root + predecessor.name));
  assert.strictEqual(run.clock.error, null);
  const paths = [...new Set(cp.execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {cwd: repo}).toString().split('\0'))]
    .filter(p => p && !p.startsWith('docs/')).sort();
  const current = Object.fromEntries(paths.map(p => [p, c.hash(fs.readFileSync(repo + '/' + p))]));
  assert.deepStrictEqual(current, baseline);
  const observation = await c.gate(predecessor.utc_ms);
  const verified = observation.samples.at(-1);
  const sourceHead = cp.execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repo}).toString().trim();
  assert.strictEqual(sourceHead, planned.manifest.source_head);
  const record = {source_head: sourceHead, manifest_sha256: planned.manifest_sha256,
    verified_at: new Date(verified.utc_ms).toISOString(), mutation: name,
    source_identities: paths.length, source_unchanged: true, source_map_sha256: c.hash(JSON.stringify(current)),
    clock: {contract: c.contract, policy: c.policy, kind: 'restoration', predecessor, observation}};
  fs.writeFileSync(c.root + 'r110-exact-clock-restoration-' + name + '.json', JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
  console.log('PASS: all ' + paths.length + ' source identities restored with observed UTC floor');
})().catch(error => {
  fs.writeFileSync(c.root + 'r110-exact-clock-restoration-' + name + '-failure.json', JSON.stringify({accepted: false, error: String(error), clock_failure: error.clock_failure || null}, null, 2) + '\n', {flag: 'wx'});
  console.error(error); process.exitCode = 1;
});
