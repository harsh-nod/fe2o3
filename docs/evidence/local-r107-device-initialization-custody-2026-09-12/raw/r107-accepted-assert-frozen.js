const fs = require('fs');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const repo = '/home/harsh/.codex-tmp/fe2o3-r61-execution';
const expected = JSON.parse(fs.readFileSync('/home/harsh/.codex-tmp/r107-accepted-frozen-source.json'));
const paths = [...new Set(cp.execFileSync('git', ['ls-files', '--cached', '--others',
  '--exclude-standard', '-z'], {cwd: repo}).toString().split('\0'))]
  .filter(p => p && !p.startsWith('docs/')).sort();
const current = Object.fromEntries(paths.map(p => [p,
  crypto.createHash('sha256').update(fs.readFileSync(repo + '/' + p)).digest('hex')]));
assert.deepStrictEqual(current, expected);
if (process.argv[2]) {
  const name = process.argv[2];
  assert(/^[a-z0-9-]+$/.test(name));
  fs.writeFileSync('/home/harsh/.codex-tmp/r107-accepted-restoration-' + name + '.json',
    JSON.stringify({verified_at: new Date().toISOString(), mutation: name,
      source_identities: paths.length, source_unchanged: true,
      source_map_sha256: crypto.createHash('sha256').update(JSON.stringify(current)).digest('hex')}, null, 2) + '\n',
    {flag: 'wx'});
}
console.log('PASS: all ' + paths.length + ' frozen source identities match');
