const parent = 'a2feef229758381b66f962ad8be2b87843eb33a3';
const previous = 'docs/evidence/local-r112-uninitialized-device-insertion-2026-09-13/';
const production = 'crates/fe2o3-runtime-model/src/context_version_journal.rs';
const memberTests = 'crates/fe2o3-runtime-model/src/context_version_journal/membership_tests.rs';
const issuanceTests = 'crates/fe2o3-runtime-model/src/context_version_journal/tests.rs';
const prefix = 'context_version_journal::tests::membership::';
const newTests = [
  'enrollment_uses_complete_coordinates_without_an_allocation_watermark',
  'empty_begin_retains_writer_capacity_without_members_or_epochs',
  'canonical_keys_not_slot_order_bind_complete_chains_for_older_reservations',
  'first_middle_last_bad_destinations_leave_every_owner_and_scratch_unchanged',
  'whole_roster_order_and_capacity_precede_destination_validation',
  'allocation_writer_and_member_limits_have_independent_effects',
  'final_epoch_is_admitted_but_exhaustion_is_whole_roster_atomic',
  'stale_copied_and_foreign_references_never_alias_reused_slots',
  'selected_private_slot_corruption_rejects_before_any_planning_write',
  'fixed_roster_work_and_storage_are_independent_of_unrelated_capacities',
  'short_traces_match_independent_writer_and_allocation_maps',
  'begin_routes_full_preflight_before_bounded_planning_and_commit',
].map(n => prefix + n).sort();
const sourceGuard = prefix + 'begin_routes_full_preflight_before_bounded_planning_and_commit';
const test = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline',
  '-p', 'fe2o3-runtime-model', '--all-features', '--lib'];
const focused = [['journal', 'context_version_journal::tests::', 23], ['membership', prefix, 12]];
const B = prefix + 'first_middle_last_bad_destinations_leave_every_owner_and_scratch_unchanged';
const C = prefix + 'canonical_keys_not_slot_order_bind_complete_chains_for_older_reservations';
const E = prefix + 'empty_begin_retains_writer_capacity_without_members_or_epochs';
const W = prefix + 'whole_roster_order_and_capacity_precede_destination_validation';
const L = prefix + 'allocation_writer_and_member_limits_have_independent_effects';
const P = prefix + 'selected_private_slot_corruption_rejects_before_any_planning_write';
const V = 'context_version_journal::tests::existing_ids_preserve_gaps_and_older_reserved_lookup_and_abort';
const eq = 'assertion `left == right` failed';
const mutation = (name, from, to, target, oracle_path, oracle_line, expected) => ({
  name, path: production, edits: [[from, to]], test: target, oracle_path, oracle_line, expected,
});
const mutations = [
  mutation('partial-preflight-epoch',
    '.ok_or(ContextVersionJournalErrorV1::EpochExhausted)?;\n        }\n        if count > self.member_free.len() {',
    '.ok_or(ContextVersionJournalErrorV1::EpochExhausted)?;\n            self.allocations[destination.allocation.slot].as_mut().unwrap().attempt_epoch += 1;\n        }\n        if count > self.member_free.len() {',
    B, memberTests, 54, [eq, 'whole journal and scratch remain unchanged']),
  mutation('omit-last-member',
    'for index in 0..count {\n            self.count_indexed_access();\n            let plan = self.scratch[index].take()',
    'for index in 0..count.saturating_sub(1) {\n            self.count_indexed_access();\n            let plan = self.scratch[index].take()',
    L, memberTests, 457, ['assertion failed: journal.member_free.is_empty()']),
  mutation('omit-epoch-burn', '            entry.attempt_epoch = plan.attempt_epoch;\n', '',
    C, memberTests, 280, [eq, 'left: (0, 0)', 'right: (1, 0)']),
  mutation('premature-lineage', '            entry.pending_member = Some(plan.member_slot);',
    '            entry.content_lineage = plan.attempt_epoch;\n            entry.pending_member = Some(plan.member_slot);',
    C, memberTests, 280, [eq, 'left: (1, 1)', 'right: (1, 0)']),
  mutation('omit-allocation-identity',
    'entry.key == reference.key\n                    && entry.key.context_generation == self.context_generation',
    'entry.key.context_generation == self.context_generation',
    B, memberTests, 53, [eq, 'left: Ok(())', 'right: Err(InvalidAllocationReference)']),
  mutation('device-local-only', 'if entry.device != destination.device {',
    'if entry.device.local != destination.device.local {',
    B, memberTests, 53, [eq, 'left: Ok(())', 'right: Err(AllocationDeviceMismatch)']),
  mutation('omit-extent', 'if entry.byte_extent != destination.byte_extent {',
    'if false && entry.byte_extent != destination.byte_extent {',
    B, memberTests, 53, [eq, 'left: Ok(())', 'right: Err(AllocationExtentMismatch)']),
  mutation('member-capacity-before-identity',
    '        for destination in canonical {\n            let entry = self.exact_allocation(destination.allocation)?;',
    '        if count > self.member_free.len() {\n            return Err(ContextVersionJournalErrorV1::MemberCapacity);\n        }\n        for destination in canonical {\n            let entry = self.exact_allocation(destination.allocation)?;',
    W, memberTests, 53, [eq, 'left: Err(MemberCapacity)', 'right: Err(InvalidAllocationReference)']),
  mutation('pending-abort', '        self.lookup_reserved(reference)?;',
    '        self.lookup_writer(reference)?;',
    E, issuanceTests, 206, [eq, 'left: Err(InvalidState)', 'right: Err(InvalidReference)']),
  mutation('pending-begin', '        self.lookup_reserved(writer)?;',
    '        self.lookup_writer(writer)?;',
    E, memberTests, 53, [eq, 'left: Err(InvalidState)', 'right: Err(InvalidReference)']),
  mutation('empty-frees-writer', '        let head = if count == 0 {\n            None',
    '        let head = if count == 0 {\n            self.push_free(writer.slot);\n            None',
    E, memberTests, 227, [eq, 'left: 1', 'right: 0']),
  mutation('canonical-local-only', 'if pair[0].allocation.key >= pair[1].allocation.key {',
    'if pair[0].allocation.key.local >= pair[1].allocation.key.local {',
    W, memberTests, 53, [eq, 'left: Err(InvalidAllocationReference)', 'right: Err(NonCanonicalRoster)']),
  mutation('pending-counted-reserved',
    '    pub fn reserved_writer_count(&self) -> usize {\n        self.reserved_count\n    }',
    '    pub fn reserved_writer_count(&self) -> usize {\n        self.writer_capacity - self.free.len()\n    }',
    E, memberTests, 226, [eq, 'left: 1', 'right: 0']),
  mutation('allow-duplicate-allocation', 'if pair[0].allocation.key >= pair[1].allocation.key {',
    'if pair[0].allocation.key > pair[1].allocation.key {',
    W, memberTests, 53, [eq, 'left: Ok(())', 'right: Err(NonCanonicalRoster)']),
  mutation('overwrite-enrollment-slot', 'if self.allocations.get(slot) != Some(&None) {',
    'if false && self.allocations.get(slot) != Some(&None) {',
    P, memberTests, 690, [eq, 'left: Ok(ContextAllocationReferenceV1', 'right: Err(InvalidState)']),
  mutation('reject-older-reserved',
    'if key == reference.key && key.context_generation == self.context_generation =>',
    'if key == reference.key && key.context_generation == self.context_generation && key.local == self.registration_watermark =>',
    V, issuanceTests, 263, [eq, 'left: Err(InvalidReference)', 'right: Ok(ContextWriterKeyV1']),
  mutation('omit-busy', 'if entry.pending_member.is_some() {',
    'if false && entry.pending_member.is_some() {',
    B, memberTests, 53, [eq, 'left: Ok(())', 'right: Err(AllocationBusy)']),
];
module.exports = {parent, previous, production, sourceDelta: [production, memberTests, issuanceTests],
  sourceCount: 5680, newTests, sourceGuard, test, focused, mutations,
  allowedMutationTests: [...new Set([...newTests.filter(n => n !== sourceGuard), V])].sort(),
  helpers: ['r113-run-v3.js', 'r113-qualification-plan.js', 'r113-qualification-evidence.js',
    'r113-qualification-tests.js', 'r113-qualification-prepare.js', 'r113-qualification-run.js',
    'r113-qualification-collect.js'],
};
