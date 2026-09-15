const assert = require('assert');
const vm = require('vm');
const e = require('./r118b-qualification-evidence.js');
const p = require('./r118b-qualification-plan.js');
const names = [...p.helpers, 'r118b-qualification-launch-v1.js', 'r118b-qualification-snapshot-tests-v1.js'];
const pins = names.map(name => ({name, sha256: e.hash(e.bytes(name))}));
const fixtureModule = {exports: {}};
const fixtureRequire = name => require(name.startsWith('./r118b-') ? e.file(name.slice(2)) : name);
// Reuse the pinned scripted cohort without executing its registered tests.
vm.runInThisContext('(function(require,module,exports){\n' + e.bytes('r118b-qualification-tests.js').toString() +
  '\nmodule.exports.collectorFixture = collectorFixture;\n})', {filename: 'r118b-pinned-qualification-fixtures'})(fixtureRequire, fixtureModule, fixtureModule.exports);
const f = fixtureModule.exports.collectorFixture();
const artifact = 'r118b-qualified-mut-' + p.mutations[0].name + '-source-after.json';
assert.throws(() => f.collector.collect(false, () => {
  f.put(artifact, Buffer.from(f.io.bytes(artifact).toString() + '\n'));
  return f.base;
}), /unchanged collection input: .*source-after\.json/, 'semantic equality must not bypass captured-byte equality');
console.log('PASS final-captured-byte-check-rejects-semantically-identical-source-map');
for (const pin of pins) assert.strictEqual(e.hash(e.bytes(pin.name)), pin.sha256, 'unchanged supplemental input');
console.log('HELPER_PINS: ' + JSON.stringify(pins));
console.log('PASS: 1 snapshot contract test');
