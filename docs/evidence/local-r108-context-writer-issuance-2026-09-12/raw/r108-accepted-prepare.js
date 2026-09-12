const fs = require('fs');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const read = name => JSON.parse(fs.readFileSync(root + name, 'utf8'));
const mutations = read('r108-mutations.json');
const inversions = [];
for (let i = 1; i < mutations.length; i++) {
  const previous = mutations[i - 1].name;
  const current = mutations[i].name;
  const restored = read('r108-restoration-' + previous + '.json').verified_at;
  const started = read('r108-mut-' + current + '.json').started_at;
  const delta_ms = Date.parse(started) - Date.parse(restored);
  if (delta_ms < 0) inversions.push({previous, current, restored, started, delta_ms});
}
assert.deepStrictEqual(inversions.map(r => [r.current, r.delta_ms]), [['omit-kind', -55], ['registration-growth', -349]]);
const names = fs.readdirSync(root).filter(name => /^r108-.*\.(json|log|py|js)$/.test(name)
  && !name.startsWith('r108-accepted-') && !name.startsWith('r108-repeat-')
  && !name.startsWith('r108-restoration-repeat-')).sort();
const record = {
  recorded_at: new Date().toISOString(),
  reason: 'Repeat auxiliary and mutation cohorts after original strict roster rejection and two wall-clock chronology inversions; original bytes remain retained and unaccepted.',
  source_map_sha256: hash(JSON.stringify(read('r108-frozen-source.json'))),
  between_mutations_ms: 5000, test_environment_changed: false, test_deadlines_changed: false,
  original_clock_inversions: inversions,
  prior_artifacts: names.map(name => ({name, sha256: hash(fs.readFileSync(root + name))})),
  runners: ['r108-accepted-prepare.js', 'r108-accepted-auxiliary-gates.py', 'r108-accepted-retain-local.js']
    .map(name => ({name, sha256: hash(fs.readFileSync(root + name))})),
};
fs.writeFileSync(root + 'r108-accepted-revalidation.json', JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
console.log('Pinned ' + names.length + ' original artifacts; fresh auxiliary/mutation cohort with unchanged tests and explicit inter-mutation gaps');
