const fs = require('fs');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const sourcePath = 'crates/fe2o3-runtime-model/src/context_version_journal.rs';
const testPath = 'crates/fe2o3-runtime-model/src/context_version_journal/tests.rs';
const equality = 'key == reference.key && key.context_generation == self.context_generation';
const bounds = 'value != 0 && value != u64::MAX';
const capacity = 'if self.free.len() >= self.writer_capacity || self.free.len() >= self.free.capacity() {';
const rows = [
  ['lookup-watermark', 'existing_ids_preserve_gaps_and_older_reserved_lookup_and_abort', 153,
    equality, equality + ' && key.local == self.registration_watermark',
    'Older Reserved identity survives newer registrations', ['left: Err(InvalidReference)', 'local: 41']],
  ['abort-watermark', 'abort_never_rolls_back_watermark_or_revives_old_keys', 176,
    'self.push_free(reference.slot);', 'self.push_free(reference.slot); self.registration_watermark = reference.key.local - 1;',
    'Abort preserves the maximum ever registered', ['left: 43', 'right: 44']],
  ['capacity-watermark', 'dropped_reference_retains_capacity_and_full_rejection_keeps_watermark', 85,
    'let slot = self\n            .next_free()', 'self.registration_watermark = key.local;\n        let slot = self\n            .next_free()',
    'Full-capacity rejection preserves the entire snapshot', ['Snapshot', 'watermark: 44', 'watermark: 41']],
  ['slot-only', 'full_key_and_slot_checks_reject_replay_and_every_substitution', 91,
    equality, 'key.context_generation == self.context_generation',
    'Stale reference cannot resolve a reused slot', ['left: Ok(', 'local: 47', 'right: Err(InvalidReference)']],
  ['omit-kind', 'full_key_and_slot_checks_reject_replay_and_every_substitution', 91,
    equality, 'key.context_generation == reference.key.context_generation && key.local == reference.key.local && key.context_generation == self.context_generation',
    'Writer kind is part of exact identity', ['left: Ok(', 'local: 47', 'right: Err(InvalidReference)']],
  ['retain-free-slot', 'dropped_reference_retains_capacity_and_full_rejection_keeps_watermark', 70,
    'let _ = self.free.pop();', 'let _ = self.free.last();',
    'Registration consumes its exact free slot', ['occupied slot on free stack']],
  ['reject-max-minus-one', 'checked_add_allocator_edges_accept_max_minus_one_and_reject_max', 277,
    bounds, 'value != 0 && value < u64::MAX - 1',
    'Existing checked-add allocator can issue MAX-1', ['called `Result::unwrap()` on an `Err` value: InvalidContextGeneration']],
  ['admit-max', 'checked_add_allocator_edges_accept_max_minus_one_and_reject_max', 84,
    bounds, 'value != 0',
    'MAX cannot be registered as an issued identity', ['left: Ok(', 'local: 18446744073709551615', 'right: Err(InvalidWriterId)']],
  ['linear-lookup', 'indexed_work_is_constant_and_storage_does_not_grow', 324,
    'match self.read_slot(reference.slot).copied().flatten() {',
    'for slot in 0..self.writer_capacity { let _ = self.read_slot(slot); }\n        match self.read_slot(reference.slot).copied().flatten() {',
    'Dynamic lookup uses exactly one counted primitive', ['left: 2', 'right: 1']],
  ['registration-growth', 'indexed_work_is_constant_and_storage_does_not_grow', 328,
    'self.pop_free();', 'self.reserved.reserve_exact(self.reserved.capacity() + 1); self.pop_free();',
    'Storage capacity and pointers remain constructor-fixed', ['assertion `left == right` failed']],
  ['omit-writer-capacity', 'internal_slot_and_capacity_invariants_reject_before_mutation', 407,
    capacity, 'if self.free.len() >= self.free.capacity() {',
    'Logical-capacity guard rejects despite spare backing capacity', ['left: Ok(())', 'right: Err(InvalidState)']],
  ['omit-storage-capacity', 'internal_slot_and_capacity_invariants_reject_before_mutation', 407,
    capacity, 'if self.free.len() >= self.writer_capacity {',
    'Storage-capacity guard prevents an allocating abort', ['left: Ok(())', 'right: Err(InvalidState)']],
];
const source = fs.readFileSync(root + 'fe2o3-r61-execution/' + sourcePath, 'utf8');
const tests = fs.readFileSync(root + 'fe2o3-r61-execution/' + testPath, 'utf8').split('\n');
const mutations = rows.map(([name, test, line, from, to, oracle, markers]) => {
  assert.strictEqual(source.split(from).length, 2, name + ' unique mutation');
  assert(/assert|unwrap/.test(tests[line - 1]), name + ' pinned assertion');
  return {name, path: sourcePath, test: 'context_version_journal::tests::' + test,
    oracle, oracle_line: line, edits: [[from, to]], expected: [testPath + ':' + line + ':', ...markers]};
});
fs.writeFileSync(root + 'r108-mutations.json', JSON.stringify(mutations, null, 2) + '\n', {flag: 'wx'});
const names = ['r108-mutations.json', 'r108-prepare-mutations.js', 'r108-mutation-patch.js',
  'r108-check-mutation.js', 'r108-assert-frozen.js'];
const pins = Object.fromEntries(names.map(name => [name,
  crypto.createHash('sha256').update(fs.readFileSync(root + name)).digest('hex')]));
fs.writeFileSync(root + 'r108-mutation-tool-pins.json', JSON.stringify({recorded_at: new Date().toISOString(), pins}, null, 2) + '\n', {flag: 'wx'});
console.log('Pinned ' + mutations.length + ' independent compiled-negative recipes and exact assertion lines');
