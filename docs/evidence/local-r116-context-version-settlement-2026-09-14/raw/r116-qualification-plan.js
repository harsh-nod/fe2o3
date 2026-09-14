const parent = '4756971168f6b4f2c33d217f5d476ccde8ea2740';
const accepted = parent;
const previous = 'docs/evidence/local-r115-returning-control-cleanup-2026-09-14/';
const base = 'crates/fe2o3-runtime-model/src/context_version_journal';
const journal = base + '.rs';
const implementation = base + '/settlement.rs';
const tests = base + '/settlement_tests.rs';
const referenceTests = base + '/settlement_reference_tests.rs';
const prefix = 'context_version_journal::tests::settlement::';
const S = prefix + 'settlements_return_exact_canonical_members_and_preserve_unrelated_storage';
const G = prefix + 'no_effect_burns_epochs_and_success_advances_lineage_across_gaps';
const U = prefix + 'unknown_is_sticky_revalidated_and_retains_empty_and_nonempty_writers';
const K = prefix + 'exact_reserved_writers_reject_settlement_and_unknown_before_evidence';
const F = prefix + 'full_reference_and_evidence_rejections_precede_retained_corruption';
const M = prefix + 'every_retained_member_rejects_corruption_before_any_scratch_write';
const L = prefix + 'retained_prior_lineage_mismatch_below_admitted_epoch_rejects_atomically';
const H = prefix + 'header_cardinality_and_noncanonical_chains_reject_atomically';
const B = prefix + 'rejected_release_cost_covers_each_scratch_cell_and_return_limit';
const D = prefix + 'reference_traces::independent_traces_preserve_disjoint_writers_and_lineage_below_watermark';
const sourceGuard = prefix + 'settlement_routes_bounded_preflight_before_plan_and_commit';
const originalNewTests = [S, G, U, K, F, M, L, H, B, D, sourceGuard,
  prefix + 'final_admitted_epoch_settles_without_rolling_back_exhaustion',
  prefix + 'reference_traces::independent_action_sequences_cover_replay_pressure_and_all_writer_phases',
  prefix + 'rejected_settlement_work_is_bounded_by_the_examined_prefix',
  prefix + 'release_headroom_and_scratch_reject_but_do_not_gate_unknown',
  prefix + 'settlement_replay_cannot_alias_a_reused_writer_slot',
  prefix + 'settlement_work_depends_on_touched_members_not_unrelated_populations',
].sort();
const reviewedTests = [
  prefix + 'settlement_preserves_nonempty_allocation_free_stack',
  prefix + 'unknown_custody_rejects_fresh_reserved_begin_atomically',
].sort();
const newTests = [...originalNewTests, ...reviewedTests].sort();
const fullTest = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '--no-fail-fast',
  ...['fe2o3-completion', 'fe2o3-runtime-model', 'fe2o3-resource-accounting', 'fe2o3-kfd', 'fe2o3-runtime'].flatMap(n => ['-p', n]),
  '--all-features', '--all-targets'];
const test = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-runtime-model', '--all-features', '--lib'];
const format = ['cargo', '+nightly-2026-04-03', 'fmt', '--all'];
const modelAll = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-runtime-model', '--all-features', '--all-targets'];
const modelClippy = ['cargo', '+nightly-2026-04-03', 'clippy', '--locked', '--offline', '-p', 'fe2o3-runtime-model', '--all-features', '--all-targets', '--', '-D', 'warnings'];
const eq = 'assertion `left == right` failed';
const mutation = (name, from, to, test, oracle_line, expected = [eq], oracle_path = tests) => ({
  name, path: implementation, edits: [[from, to]], test, oracle_path, oracle_line, expected,
});
const disable = (name, expression, test, line, expected) => mutation(name, expression, 'false && ' + expression, test, line, expected);
const early = `        if let Some(slot) = head {
            if let Some(member) = self.members[slot] {
                self.store_plan(0, BeginMemberPlanV1 {
                    member_slot: slot,
                    allocation: member.allocation,
                    prior_lineage: member.prior_lineage,
                    attempt_epoch: member.attempt_epoch,
                });
            }
        }
`;
const releaseValidation = '        self.validate_retained_chain(writer, head, count)?;\n        let writer_returns';
// Prospective exact behavioral oracles, fixed before any compiled mutation.
const mutations = [
  disable('skip-writer-identity', 'key != writer.key', F, 236, [eq, 'InvalidState', 'InvalidReference']),
  mutation('admit-reserved-settlement', '            Some(WriterEntryV1::Pending { key, head, count }) => (key, head, count, false),',
    '            Some(WriterEntryV1::Reserved(key)) => (key, None, 0, false),\n            Some(WriterEntryV1::Pending { key, head, count }) => (key, head, count, false),', K, 207, [eq, 'Ok(())', 'InvalidReference']),
  disable('skip-evidence-identity', 'evidence != writer', F, 241, [eq, 'InvalidState', 'SettlementEvidenceMismatch']),
  disable('skip-member-writer', 'member.writer != writer', M, 310, [eq, 'Ok(())', 'InvalidState']),
  mutation('slot-only-allocation', '.exact_allocation(member.allocation)',
    '.read_allocation(member.allocation.slot).ok_or(ContextVersionJournalErrorV1::InvalidAllocationReference)', M, 310, [eq, 'Ok(())', 'InvalidState']),
  disable('skip-allocation-backlink', 'allocation.pending_member != Some(slot)', M, 310, [eq, 'Ok(())', 'InvalidState']),
  disable('skip-admitted-epoch', 'allocation.attempt_epoch != member.attempt_epoch', M, 310, [eq, 'Ok(())', 'InvalidState']),
  disable('skip-prior-lineage-match', 'allocation.content_lineage != member.prior_lineage', L, 351, [eq, 'Ok(())', 'InvalidState']),
  disable('skip-strict-lineage-bound', 'member.prior_lineage >= member.attempt_epoch', M, 310, [eq, 'Ok(())', 'InvalidState']),
  mutation('skip-exact-chain-tail', 'if head.is_some()', 'if false && head.is_some()', H, 389, [eq, 'Ok(())', 'InvalidState']),
  disable('skip-canonical-order', 'previous_key.is_some_and(|key| key >= member.allocation.key)', H, 389, [eq, 'Ok(())', 'InvalidState']),
  mutation('no-effect-advances-lineage', 'self.settle_retained(writer, evidence.writer, false)',
    'self.settle_retained(writer, evidence.writer, true)', G, 140),
  mutation('no-effect-rolls-back-epoch', '            allocation.pending_member = None;',
    '            if !success { allocation.attempt_epoch = plan.prior_lineage; }\n            allocation.pending_member = None;', G, 140),
  mutation('success-keeps-old-lineage', 'allocation.content_lineage = plan.attempt_epoch;',
    'allocation.content_lineage = plan.prior_lineage;', G, 140),
  mutation('early-writer-removal', '        let (mut head, count, _) = self.retained_header(writer, false)?;',
    '        let (mut head, count, _) = self.retained_header(writer, false)?;\n        self.store_slot(writer.slot, None);', F, 245, [eq, 'Snapshot {']),
  mutation('early-scratch-write', releaseValidation, early + releaseValidation, M, 311, [eq, 'Snapshot {']),
  disable('skip-writer-physical-headroom', 'writer_returns > self.free.capacity()', B, 673, [eq, 'Ok(())', 'InvalidState']),
  disable('skip-member-physical-headroom', 'member_returns > self.member_free.capacity()', B, 673, [eq, 'Ok(())', 'InvalidState']),
  disable('skip-writer-logical-headroom', 'writer_returns > self.writer_capacity', B, 673, [eq, 'Ok(())', 'InvalidState']),
  disable('skip-member-logical-headroom', 'member_returns > self.allocation_capacity', B, 673, [eq, 'Ok(())', 'InvalidState']),
  disable('skip-vacant-scratch', 'self.scratch[index].is_some()', B, 673, [eq, 'Ok(())', 'InvalidState']),
  mutation('reverse-member-returns', '        for index in 0..count {\n            self.count_indexed_access();\n            let plan = self.scratch[index]',
    '        for index in (0..count).rev() {\n            self.count_indexed_access();\n            let plan = self.scratch[index]', S, 121, [eq, 'Snapshot {']),
  mutation('return-wrong-writer-slot', 'self.push_free(writer.slot);', 'self.push_free(0);', S, 121, [eq, 'Snapshot {']),
  mutation('settle-sticky-unknown', 'self.retained_header(writer, false)?', 'self.retained_header(writer, true)?', U, 180, [eq, 'Ok(())', 'InvalidReference']),
  mutation('skip-repeated-unknown-validation', '        self.validate_retained_chain(writer, head, count)?;\n        if !unknown {',
    '        if !unknown { self.validate_retained_chain(writer, head, count)?; }\n        if !unknown {', M, 310, [eq, 'Ok(())', 'InvalidState']),
  mutation('destroy-disjoint-writer', '        self.store_slot(writer.slot, None);',
    '        self.store_slot(writer.slot, None);\n        if count != 0 { self.store_slot((writer.slot + 2) % self.writer_capacity, None); }',
    D, 385, [eq, 'independent retained state: Success'], referenceTests),
  mutation('reverse-unrelated-allocation-free-on-settle',
    '        self.push_free(writer.slot);\n        Ok(())',
    '        self.push_free(writer.slot);\n        self.allocation_free.reverse();\n        Ok(())',
    prefix + 'settlement_preserves_nonempty_allocation_free_stack', 836,
    [eq, 'settlement changed allocation-free order: mode=0 count=0']),
  mutation('reverse-unrelated-allocation-free-on-unknown',
    '        }\n        Ok(())\n    }\n\n    fn retained_header(',
    '        }\n        self.allocation_free.reverse();\n        Ok(())\n    }\n\n    fn retained_header(',
    prefix + 'settlement_preserves_nonempty_allocation_free_stack', 836,
    [eq, 'settlement changed allocation-free order: mode=2 count=0']),
  {...mutation('begin-bypasses-unknown-custody',
    '            if entry.pending_member.is_some() {',
    '            if entry.pending_member.is_some_and(|slot| {\n                let owner = self.members[slot].unwrap().writer;\n                matches!(self.writers[owner.slot], Some(WriterEntryV1::Pending { .. }))\n            }) {',
    prefix + 'unknown_custody_rejects_fresh_reserved_begin_atomically', 904,
    [eq, 'Unknown custody must reject fresh Reserved Begin: position=0', 'Ok(())', 'AllocationBusy']), path: journal},
];
const sixteen = '7b21d2dec494f5ee9d6efd47deb3ca53243265eb44c1f96d4bee311fad2552ec';
const seventeen = '51c996704992ad6f6a46965eeeeffb389c451d1ba051fbf7917a9fe3e490ddd0';
const isolatedRuns = [
  ['initial-format', format, '751e970bcf2de62cd13bec87bc597bdf310cf9880113edc25a511eadce335004', 'edfa8d57cb6ffb5b517f8f40e8af6f28d98e3b92a9ed828396c11959dc315ea4', 5685, null],
  ['reference-format', format, 'd9797538f061a80bca7c6204ce5b3547699a40856d66068994d455a032af8b19', '98e8a76240df3fc049ebe015e00fb00ce5d5f277956ae0d9ffc0c3e13ee44c65', 5686, null],
  ['coverage-format', format, 'a3e0f6e65b29e72639c257400209ca915601d861deaf6e3861d2285c2708478d', 'c6b029c77a89fec88feec1f8f8133cca732a0ebdadb0658ca518c54d08402d2a', 5686, null],
  ['initial-journal', [...test, 'context_version_journal::tests::'], sixteen, sixteen, 5686, ['journal', 39, 0]],
  ['model-all', modelAll, sixteen, sixteen, 5686, ['model', 760, 2]],
  ['clippy', modelClippy, sixteen, sixteen, 5686, null],
  ['lineage-format', ['cargo', '+nightly-2026-04-03', 'fmt', '-p', 'fe2o3-runtime-model'], seventeen, seventeen, 5686, null],
  ['lineage-settlement', [...test, prefix], seventeen, seventeen, 5686, ['settlement', 17, 0]],
  ['lineage-model-all', modelAll, seventeen, seventeen, 5686, ['model', 761, 2]],
  ['lineage-clippy', modelClippy, seventeen, seventeen, 5686, null],
];
const isolatedArtifacts = isolatedRuns.flatMap(([n]) => ['.json', '.log', '-source.json', '-source-after.json'].map(s => 'v3candidate-' + n + s))
  .concat(['v3candidate-run-v1.js', 'v3candidate-negative-handoff.md']).sort();
const originalIntegratedRuns = [
  ['r116-initial-format', [...format, '--', '--check'], null],
  ['r116-initial-journal', [...test, 'context_version_journal::tests::'], 'r116-initial-format.json'],
  ['r116-preliminary-gnu-all', fullTest, 'r116-initial-journal.json'],
  ['r116-preliminary-musl-all', [...fullTest, '--target', 'x86_64-unknown-linux-musl'], 'r116-preliminary-gnu-all.json'],
];
module.exports = {
  parent, accepted, previous, fullTest, test, newTests, originalNewTests, reviewedTests, sourceGuard,
  originalIntegratedRuns,
  originalIntegratedMapHash: '66db1c7f75544d74dc0eba5753836518d192e7014776581432ce154d84b6a5a8',
  originalIntegratedArtifacts: originalIntegratedRuns.flatMap(([n]) =>
    ['.json', '.log', '-source.json', '-source-after.json'].map(s => n + s)).sort(),
  testExtension: {path: tests, originalBytes: 29353,
    originalHash: '5859062698593d1cdc88936b65d1a3ace12039a8ef6ffc7e39fc3534f4c2d237'},
  initialRuns: [
    ['r116-reviewed-format', [...format, '--', '--check'], null],
    ['r116-reviewed-journal', [...test, 'context_version_journal::tests::'], 'r116-reviewed-format.json'],
  ],
  prerequisites: [
    ['r116-reviewed-gnu-all', fullTest, 'r116-reviewed-journal.json'],
    ['r116-reviewed-musl-all', [...fullTest, '--target', 'x86_64-unknown-linux-musl'], 'r116-reviewed-gnu-all.json'],
  ],
  freezeHelpers: ['r116-run-v1.js', 'r116-source-gate.py', 'r116-auxiliary-gates.py', 'r116-freeze.js',
    'r116-runner-tests.js', 'r116-freeze-tests.js', 'r116-qualification-plan.js', 'r116-qualification-evidence.js'],
  freezeContractCount: 31, qualificationContractCount: 122,
  productionPaths: [journal, implementation].sort(),
  sourceDelta: [journal, base + '/tests.rs', implementation, tests, referenceTests].sort(),
  addedSource: [implementation, tests, referenceTests].sort(), sourceCount: 5688, docRelocations: [],
  focused: [['settlement', prefix, 19], ['journal', 'context_version_journal::tests::', 42],
    ['membership', 'context_version_journal::tests::membership::', 12], ['traces', prefix + 'reference_traces::', 2]],
  mutations, allowedMutationTests: newTests.filter(n => n !== sourceGuard),
  isolatedRuns, isolatedArtifacts, isolatedLineageTest: L,
  isolatedContext: {head: '0265025b96f25a7cb79c97b576bb25cb38b15df5', cwd: '/home/harsh/.codex-tmp/fe2o3-v3-settlement',
    contract: 'v3candidate-raw-utc-boot-monotonic-v1', runner: 'v3candidate-run-v1.js'},
  isolatedRunnerHash: '701c2e71b377daab9e5dfa44f3f7ac196f3b53ead380cab61b732eda25c9e831',
  helpers: ['r116-run-v1.js', 'r116-qualification-plan.js', 'r116-qualification-evidence.js',
    'r116-qualification-tests.js', 'r116-qualification-prepare.js', 'r116-qualification-run.js', 'r116-qualification-collect.js'],
};
