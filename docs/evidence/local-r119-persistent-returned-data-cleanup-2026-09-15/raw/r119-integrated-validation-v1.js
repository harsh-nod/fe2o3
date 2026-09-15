const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const manifestName = 'r119-integrated-validation-inputs-v1.json';
const helperNames = [
  'r119-integrated-run-v1.js', 'r119-integrated-runner-tests-v1.js',
  'r119-integrated-runner-inputs-v1.json', 'r119-integrated-history-v1.js',
  'r119-integrated-history-tests-v1.js', 'r119-integrated-freeze-v1.js',
  'r119-integrated-freeze-tests-v1.js', 'r119-integrated-qualification-plan-v1.js',
  'r119-integrated-qualification-evidence-v1.js', 'r119-integrated-mutation-core-v1.js',
  'r119-integrated-source-gate-v1.py', 'r119-integrated-auxiliary-gates-v1.py',
  'r119-integrated-validation-v1.js', 'r119-persistent-mutations-v1.js',
  'r119-persistent-mutations-v2.js', 'r119-integrated-origin-v1.json',
  'r119-integrated-runner-evidence-v1.js',
].sort();
const historyNames = ['accepts exact isolated and integrated cohorts without packet acceptance',
  ...['missing origin input', 'rewritten origin acceptance', 'missing isolated log',
    'relabelled zero-test isolated history', 'changed integrated runner', 'wrong current source',
    'missing current source', 'wrong integrated parent', 'wrong integrated cwd', 'wrong musl command',
    'forged predecessor hash', 'wrong execution boot', 'unclosed child', 'surviving owned process',
    'timed out execution', 'changed source after execution', 'false successful command status',
    'lost test with unchanged summary', 'changed ignored roster', 'contradictory failed result',
    'missing preliminary Clippy record'].map(name => 'rejects ' + name),
  'rejects an input changed after capture',
  'rejects a coherent Clippy branch after the advertised musl tail'];
const freezeNames = ['captures inputs before validation and writes only after closing checks',
  ...['opening HEAD', 'closing HEAD', 'opening source hash', 'closing source map', 'missing helper',
    'duplicate artifact names', 'changed artifact membership', 'changed captured input',
    'live input change during captured validation', 'toolchain binary drift', 'ambient stack override',
    'occupied opening output', 'output created before final write check', 'history rejection',
    'support rejection', 'toolchain observation failure'].map(name => 'rejects ' + name + ' without writes'),
  'accepts exact input pins and TAP roster', 'rejects changed helper bytes under old pins',
  'rejects an old transcript after helper repinning', 'rejects same-count substituted TAP cases',
  'rejects repeated successful TAP summaries', 'rejects contradictory TAP summaries',
  'rejects duplicate TAP case numbers', 'rejects missing input pin markers', 'rejects reversed input pin markers'];
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const bytes = name => {
  const file = path.join(root, name);
  assert(fs.lstatSync(file).isFile(), 'regular validation input: ' + name);
  return fs.readFileSync(file);
};
function inputs(io = {bytes}) {
  const raw = io.bytes(manifestName);
  const manifest = JSON.parse(raw);
  assert.strictEqual(manifest.parent, '8e2c8532cb60918de523c6cab1b861bcd61fc319');
  assert.strictEqual(manifest.source_map_sha256, '542110b3429161394392f24a398929aba37aa1d74950d0a86204adda2d0959d2');
  assert.deepStrictEqual(manifest.helpers.map(pin => pin.name), helperNames);
  for (const pin of manifest.helpers) assert.strictEqual(hash(io.bytes(pin.name)), pin.sha256, 'validation input pin: ' + pin.name);
  return {sha256: hash(raw), manifest};
}
function guard() {
  const start = inputs();
  console.log('R119_VALIDATION_INPUTS_START ' + start.sha256);
  process.on('exit', () => {
    const finish = inputs();
    assert.strictEqual(finish.sha256, start.sha256, 'validation manifest unchanged');
    console.log('R119_VALIDATION_INPUTS_END ' + finish.sha256);
  });
}
function transcript(log, digest, tap = false) {
  const prefix = tap ? '# ' : '';
  const start = prefix + 'R119_VALIDATION_INPUTS_START ' + digest;
  const end = prefix + 'R119_VALIDATION_INPUTS_END ' + digest;
  const lines = log.split('\n');
  assert.deepStrictEqual(lines.filter(line => line.includes('R119_VALIDATION_INPUTS_')), [start, end], 'exact ordered start/end input pins');
  return lines.filter(line => line !== start && line !== end).join('\n');
}
function checkTap(log, names) {
  assert(log.startsWith('TAP version 13\n'));
  assert.deepStrictEqual([...log.matchAll(/^ok (\d+) - (.+)$/gm)].map(match => [Number(match[1]), match[2]]),
    names.map((name, index) => [index + 1, name]), 'exact ordered TAP roster');
  assert(!/^not ok /m.test(log));
  assert.deepStrictEqual(log.match(/^\d+\.\.\d+$/gm), ['1..' + names.length]);
  for (const [key, value] of Object.entries({tests: names.length, pass: names.length, fail: 0, cancelled: 0, skipped: 0, todo: 0})) {
    assert.deepStrictEqual(log.match(new RegExp('^# ' + key + ' .*$', 'gm')), ['# ' + key + ' ' + value], 'unique exact TAP ' + key);
  }
}
module.exports = {manifestName, helperNames, historyNames, freezeNames, inputs, guard, transcript, checkTap};
