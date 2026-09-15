const assert = require('assert');
const vm = require('vm');
const e = require('./r118-qualification-evidence.js');
const p = require('./r118-qualification-plan.js');
const names = [...p.helpers, 'r118-qualification-launch-v1.js', 'r118-qualification-snapshot-tests-v1.js'];
const pins = names.map(name => ({name, sha256: e.hash(e.bytes(name))}));
const fixtureModule = {exports: {}};
const fixtureRequire = name => require(name.startsWith('./r118-') ? e.file(name.slice(2)) : name);
// Reuse the pinned scripted cohort without executing its registered tests.
vm.runInThisContext('(function(require,module,exports){\n' + e.bytes('r118-qualification-tests.js').toString() +
  '\nmodule.exports.collectorFixture = collectorFixture;\n})', {filename: 'r118-pinned-qualification-fixtures'})(fixtureRequire, fixtureModule, fixtureModule.exports);
const f = fixtureModule.exports.collectorFixture();
const artifact = 'r118-qualified-mut-' + p.mutations[0].name + '-source-after.json';
assert.throws(() => f.collector.collect(false, () => {
  f.put(artifact, Buffer.from(f.io.bytes(artifact).toString() + '\n'));
  return f.base;
}), /unchanged collection input: .*source-after\.json/, 'semantic equality must not bypass captured-byte equality');
console.log('PASS final-captured-byte-check-rejects-semantically-identical-source-map');
for (const pin of pins) assert.strictEqual(e.hash(e.bytes(pin.name)), pin.sha256, 'unchanged supplemental input');
console.log('HELPER_PINS: ' + JSON.stringify(pins));
console.log('PASS: 1 snapshot contract test');
