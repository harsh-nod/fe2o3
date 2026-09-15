const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const manifestName = 'r119-integrated-validation-inputs-v2.json';
const prior = require('./r119-integrated-validation-v1.js');
const helperNames = [...prior.helperNames, prior.manifestName,
  'r119-integrated-qualification-plan-v2.js', 'r119-integrated-timeout-history-v2.js',
  'r119-integrated-history-v2.js', 'r119-integrated-history-tests-v2.js',
  'r119-integrated-validation-v2.js', 'r119-integrated-freeze-v2.js',
  'r119-integrated-freeze-tests-v2.js', 'r119-integrated-runner-tests-v2.js',
  'r119-integrated-runner-inputs-v2.json',
].sort();
const historyNames = [...prior.historyNames,
  'classifies the original timeout only as rejected closed history',
  ...['relabelled success', 'wrong raw child result', 'wrong signal', 'unclosed child',
    'spawn failure', 'missing cleanup', 'failed timeout cleanup', 'failed closing cleanup',
    'mismatched process groups', 'surviving members', 'shortened deadline',
    'coherent early timeout', 'timeout after finish', 'changed command', 'changed predecessor']
    .map(name => 'rejects timeout ' + name + ' before artifact-pin checks'),
  'rejects timeout changed source before artifact-pin checks',
  'rejects missing original timeout record', 'rejects retry bypassing the timeout predecessor',
  'rejects a coherent retry before timeout closure', 'rejects changed historical timeout bytes',
  'rejects changed historical helper bytes'];
const freezeNames = prior.freezeNames;
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
