// Synthetic transcript calibration only. Never writes or substitutes run evidence.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const helper = '/home/harsh/.codex-tmp/r121-development-gate-check-v2.js';
const source = fs.readFileSync(helper, 'utf8');
assert.strictEqual(crypto.createHash('sha256').update(source).digest('hex'), 'b3bda3a365e8dcae9d600535c05a05599d5bcbae11b8a93593669d0bbf4e3ac8');
const marker = "const [mode = 'all', selected] = process.argv.slice(2);";
assert.strictEqual(source.split(marker).length, 2);
const exercise = String.raw`const cases = [];
function good(name, run) { run(); cases.push({name, expected: 'accept'}); }
function bad(name, run) {
  assert.throws(run, error => error.code === 'ERR_ASSERTION', name);
  cases.push({name, expected: 'reject'});
}
function replaceOne(text, before, after) {
  assert.strictEqual(text.split(before).length, 2, 'one calibration target');
  return text.replace(before, after);
}
const currentGnu = bytes(fullGnu.name + '.log').toString();
good('actual GNU target and complete summary roster', () => full(fullGnu, currentGnu));
const first = summaries(currentGnu)[0];
assert.deepStrictEqual(first, ['ok', 17, 0, 0, 0, 0]);
const row = 'test result: ok. 17 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out;';
bad('GNU measured count', () => full(fullGnu, replaceOne(currentGnu, row, row.replace('0 measured', '1 measured'))));
bad('GNU filtered count', () => full(fullGnu, replaceOne(currentGnu, row, row.replace('0 filtered', '1 filtered'))));
const running = /^     Running (.+)$/m.exec(currentGnu)[0];
bad('GNU executable identity', () => full(fullGnu, replaceOne(currentGnu, running, '     Running calibration-not-a-target')));
const firstPass = /^test .+ \.\.\. ok$/m.exec(currentGnu)[0];
bad('GNU unknown successful row', () => full(fullGnu, replaceOne(currentGnu, firstPass, firstPass + '\ntest calibration-not-a-test ... ok')));
const linuxSpec = gates.gates.find(s => s.name === 'linux-helpers');
const originalLinux = accepted(linuxSpec);
good('accepted Linux split normalization', () => linux(originalLinux));
bad('Linux malformed extra row', () => linux(originalLinux + 'test calibration-not-a-test ... unexpected status\n'));
bad('Linux unknown partial row', () => linux(originalLinux + 'test calibration-not-a-test ... \nrunning 1 test\nok\n'));
bad('Linux whitespace orphan', () => linux(originalLinux + ' ok \n'));
const partial = /^test (.+) \.\.\. \nrunning 1 test\nok$/m.exec(originalLinux);
assert(partial, 'retained accepted split');
bad('Linux intervening named result', () => linux(replaceOne(originalLinux, partial[0],
  partial[0].replace('\nrunning 1 test\nok', '\nrunning 1 test\ntest ' + linuxNames[0] + ' ... ok\nok'))));
const correctedLinux = replaceOne(originalLinux, '0 measured; 1129 filtered out;', '0 measured; 1152 filtered out;');
good('Linux synthetic corrected-cohort filtered count', () => leaf(linuxSpec, correctedLinux));
bad('Linux stale filtered count', () => leaf(linuxSpec, originalLinux));
const docsSpec = gates.gates.find(s => s.name === 'gnu-docs');
const relocated = relocate(accepted(docsSpec));
good('GNU exact relocated doctest roster', () => leaf(docsSpec, relocated));
const doc = gates.doc_relocations[0];
const currentName = E.passing(relocated).find(n => n.startsWith(doc.path + ' - ') && n.includes('(line ' + doc.to + ')'));
assert(currentName);
bad('GNU stale doctest location', () => leaf(docsSpec, replaceOne(relocated,
  currentName + ' ... ok', currentName.replace('(line ' + doc.to + ')', '(line ' + doc.from + ')') + ' ... ok')));
const python = gates.gates.find(s => s.name === 'python'), pythonLog = accepted(python);
good('Python declared test count', () => leaf(python, pythonLog));
bad('Python wrong test count', () => leaf(python, replaceOne(pythonLog, 'Ran 151 tests', 'Ran 150 tests')));
const clippy = gates.gates.find(s => s.name === 'clippy-all'), clippyLog = accepted(clippy);
good('strict Clippy completion', () => leaf(clippy, clippyLog));
bad('Clippy warning', () => leaf(clippy, 'warning: calibration-only warning\n' + clippyLog));
const fmt = gates.gates.find(s => s.name === 'fmt');
good('empty formatting transcript', () => leaf(fmt, ''));
bad('nonempty formatting transcript', () => leaf(fmt, '\n'));
assert.strictEqual(cases.length, 20);
for (const [file, b] of captured) assert.deepStrictEqual(fs.readFileSync(file), b);
console.log(JSON.stringify({development_only: true, helper_calibration_only: true, runtime_mutation_count: 0,
  helper_sha256: sha(fs.readFileSync(__filename)), passed: cases.length, cases}));
`;
// Isolate the unchanged parser definitions; the CLI/record chain has its own real GNU check.
new Function('require', '__filename', source.split(marker)[0] + exercise)(require, helper);

