const assert = require('assert');
const p = require('./r119-integrated-campaign-plan-v1.js');
const e = require('./r119-integrated-qualification-evidence-v1.js');
function inputs(io = e) {
  const raw = io.bytes(p.campaignInputs);
  const manifest = JSON.parse(raw);
  assert.strictEqual(manifest.parent, p.parent);
  assert.strictEqual(manifest.source_map_sha256, p.sourceMapSha256);
  assert.deepStrictEqual(manifest.helpers.map(pin => pin.name), [...p.helpers, p.launcherName].sort());
  for (const pin of manifest.helpers) assert.strictEqual(e.hash(io.bytes(pin.name)), pin.sha256, pin.name);
  return {manifest, sha256: e.hash(raw)};
}
function guard() {
  const start = inputs();
  console.log('R119_CAMPAIGN_INPUTS_START ' + start.sha256);
  process.on('exit', () => {
    assert.strictEqual(inputs().sha256, start.sha256, 'unchanged campaign inputs');
    console.log('R119_CAMPAIGN_INPUTS_END ' + start.sha256);
  });
}
function transcript(log, digest, tap = false) {
  const prefix = tap ? '# ' : '';
  const markers = ['START', 'END'].map(kind => prefix + 'R119_CAMPAIGN_INPUTS_' + kind + ' ' + digest);
  const lines = log.split('\n');
  assert.deepStrictEqual(lines.filter(line => line.includes('R119_CAMPAIGN_INPUTS_')), markers);
  return lines.filter(line => !markers.includes(line)).join('\n');
}
module.exports = {inputs, guard, transcript};
