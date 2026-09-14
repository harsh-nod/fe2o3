const parent = '0265025b96f25a7cb79c97b576bb25cb38b15df5';
const accepted = '9eaf19141e8af6ade490feba3062c8b49d9b38ca';
const previous = 'docs/evidence/local-r114-pristine-control-cleanup-2026-09-13/';
const base = 'crates/fe2o3-kfd/src/';
const control = base + 'queue_dispatch_binding/control_release.rs';
const binding = base + 'queue_dispatch_binding.rs';
const tests = base + 'queue_dispatch_binding/control_release/tests.rs';
const lowerTests = base + 'shared_memory/tests/pristine_abort/cleanup_tests.rs';
const returning = 'queue::dispatch_binding::control_release::tests::';
const lower = 'shared_memory::tests::pristine_abort::cleanup_tests::';
const transport = 'queue::live::pristine_abort::tests::transport_tests::';
const S = returning + 'returning_success_preserves_exact_data_premises_storage_and_forward_order';
const G = returning + 'returning_generation_rejection_precedes_cardinality_and_preserves_full_owner';
const C = returning + 'returning_cardinality_and_capacity_reject_before_native_entry';
const M = returning + 'returning_modes_preserve_cancelled_history_max_recycle_and_exhaustion';
const N = returning + 'returning_native_failures_retain_every_control_position_and_exact_prefix';
const I = returning + 'returning_incomplete_callbacks_cannot_advance_extract_or_retry';
const W = returning + 'returning_wrapper_retains_full_root_before_error_or_original_panic';
const P = returning + 'returning_populated_preparation_metadata_survives_rejection_and_cleanup';
const sourceGuard = returning + 'returning_consuming_methods_use_shared_root_before_validation';
const newTests = [S, G, C, M, N, I, W, P, sourceGuard,
  returning + 'returning_currentness_failures_preserve_mapped_unmapped_and_disposed_custody',
  returning + 'returning_projection_failures_retain_disposal_receipt_and_uncommitted_model',
  returning + 'returning_partial_unmap_keeps_original_authority_at_each_position',
  returning + 'returning_actual_commit_rejection_retains_native_settlement_without_model_commit',
  returning + 'returning_cancelled_only_history_returns_zero_without_recycle',
  returning + 'returning_populated_metadata_survives_real_destructive_errors_and_panics',
].sort();
const fullTest = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '--no-fail-fast',
  ...['fe2o3-completion', 'fe2o3-runtime-model', 'fe2o3-resource-accounting', 'fe2o3-kfd', 'fe2o3-runtime'].flatMap(n => ['-p', n]),
  '--all-features', '--all-targets'];
const test = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const format = ['cargo', '+nightly-2026-04-03', 'fmt', '--all'];
const clippy = ['cargo', '+nightly-2026-04-03', 'clippy', '--locked', '--offline',
  ...['fe2o3-completion', 'fe2o3-runtime-model', 'fe2o3-resource-accounting', 'fe2o3-kfd', 'fe2o3-runtime', 'fe2o3-host', 'fe2o3-macros'].flatMap(n => ['-p', n]),
  '--all-features', '--all-targets', '--', '-D', 'warnings'];
const historicalAttempts = [
  ['preliminary-format', format, 1, 'd23f16e4aad88c5ddd18d934a117369a9ee7d67cb1226ef5d64683e79c76f8ec', false, 'missing-module'],
  ['preliminary-format-wired', format, 0, '0472f4b9a1ddf324b5f15731225d3ad486faf92b592f6e02dfddfbe948d746f1', true, null],
  ['preliminary-returning', [...test, returning], 101, '6cd251e1b670821066942e311adff2609feded082c12802be4e86643844a9756', false, 'compile-errors'],
  ['preliminary-returning-fixed', [...test, returning], 0, '4db25bcd35ede3d22c401d5a1f8d0419668ea2071ffd4c061cb74a27e3d503ac', false, 11],
  ['review-format', format, 0, '1d38504e0147aa4d72c32695a11b110f4d365f743bbc9efa3567a6ab95998b3b', true, null],
  ['reviewed-returning', [...test, returning], 0, '9717af6e56ed2467b1cb37492c303711057b12097b6aa6e8f0f3949ac5325c57', false, 15],
  ['final-oracle-format', format, 0, '83a47268cfe50d4767b5c77fb0b8d348b72fda3f3bad2c6eafff3a2d3b7a348d', true, null],
  ['preliminary-clippy', clippy, 0, 'cb4cbb046fa2394f900723ce8bbf6a37a26624fd6114026c9f29fbe576a9eeab', false, null],
];
const eq = 'assertion `left == right` failed';
const mutation = (name, from, to, test, oracle_line, expected, oracle_path = tests) => ({
  name, path: control, edits: [[from, to]], test, oracle_path, oracle_line, expected,
});
const reservation = `        let capacity = self.data.len();
        #[cfg(test)]
        let capacity = self.return_capacity_override.unwrap_or(capacity);
        self.returned.try_reserve_exact(capacity).map_err(|_| {
            Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                operation: "returning dispatch data",
            }
        })?;
        if self.returned.capacity() < self.data.len() {
            return Err(Gfx942DispatchBindingErrorV1::HostAllocationCapacity {
                operation: "returning dispatch data",
            });
        }
        self.returned_generation = Some(generation);`;
// Predicted behavioral failures, pinned before any mutation executes.
const mutations = [
  mutation('constructor-metadata-loss', 'code_identity: owner.code_identity,', 'code_identity: Vec::new(),', P, 222, [eq, 'constructor changed original owner']),
  mutation('reverse-control-order', 'self.code.next()', 'self.code.next_back()', S, 303, [eq, 'exact native identities and order'], lowerTests),
  mutation('admit-unrecycled-mode', 'ReturningControlModeV1::AfterRecycle => self.generation.returned_generation()?,',
    'ReturningControlModeV1::AfterRecycle => self.generation.returning_destroy_generation()?,', M, 422, ['assertion failed: matches!', 'ResourcePhase']),
  mutation('swallow-generation-rejection', 'self.generation.returned_generation()?', 'self.generation.returned_generation().unwrap_or(0)', G, 359,
    ['assertion failed: matches!', 'Poisoned']),
  mutation('skip-cardinality', 'if self.data.len() != self.data_premises.len() {', 'if false && self.data.len() != self.data_premises.len() {', C, 392,
    ['assertion failed: matches!', 'InvalidData', 'index: 4']),
  mutation('skip-return-capacity-check', 'if self.returned.capacity() < self.data.len() {', 'if false && self.returned.capacity() < self.data.len() {', C, 400,
    ['assertion failed: matches!', 'HostAllocationCapacity']),
  mutation('permit-cleanup-retry', 'if self.started {', 'if false && self.started {', G, 242, ['assertion failed: matches!', 'ResourcePhase']),
  mutation('remove-active-custody', '        let active = self.active_control.as_mut().expect("rooted active control");\n        memory.release_control(active)?;',
    '        let mut active = self.active_control.take().expect("rooted active control");\n        memory.release_control(&mut active)?;', N, 465, ['failed control stays rooted']),
  mutation('accept-incomplete-callback', 'if !active.is_complete() {', 'if false && !active.is_complete() {', I, 766, ['assertion failed: matches!', 'ResourcePhase']),
  mutation('extract-failed-output', 'if !self.complete {', 'if false && !self.complete {', N, 247, ['assertion failed: matches!', 'ResourcePhase']),
  mutation('reverse-returned-authorities', 'self.data.drain(..).zip(self.data_premises.drain(..))', 'self.data.drain(..).rev().zip(self.data_premises.drain(..))', S, 314, [eq, 'Data {']),
  mutation('corrupt-returned-premise', 'ReturnedDispatchDataLeaseV1 { authority, premise }',
    'ReturnedDispatchDataLeaseV1 { authority, premise: RetainedDataPremiseV1 { fully_initialized: !premise.fully_initialized, ..premise } }', S, 315, [eq, 'fully_initialized:']),
  mutation('drop-error-root', '        Ok(Err(error)) => {\n            retain(root);', '        Ok(Err(error)) => {\n            drop(root);', W, 802, ['failure retained its complete root']),
  mutation('drop-panic-root', '        Err(payload) => {\n            retain(root);', '        Err(payload) => {\n            drop(root);', W, 802, ['failure retained its complete root']),
  {...mutation('late-return-capacity', reservation, '', N, 491, ['return capacity precedes disposal']),
    edits: [[reservation, ''], ['        // Capacity and cardinality were checked before any disposal callback.',
      reservation + '\n        // Capacity and cardinality were checked before any disposal callback.']]},
  mutation('truncate-returned-output', '        self.complete = true;', '        self.returned.pop();\n        self.complete = true;', S, 302, [eq, 'left: 4', 'right: 5']),
];
module.exports = {
  parent, accepted, previous, historicalAttempts,
  rebootAnchor: ['r115-reboot-anchor', ['true'], null],
  prerequisites: [
    ['r115-refreshed-gnu-all', fullTest, 'r115-reboot-anchor.json'],
    ['r115-refreshed-musl-all', [...fullTest, '--target', 'x86_64-unknown-linux-musl'], 'r115-refreshed-gnu-all.json'],
  ],
  freezeHelpers: ['r115-run-v4.js', 'r115-source-gate.py', 'r115-auxiliary-gates.py', 'r115-freeze.js',
    'r115-runner-tests.js', 'r115-freeze-tests.js', 'r115-qualification-plan.js', 'r115-qualification-evidence.js'],
  freezeContractCount: 30, qualificationContractCount: 87,
  productionPaths: [binding, control].sort(),
  sourceDelta: [binding, control, tests, base + 'queue_dispatch_binding/preparation.rs',
    base + 'queue_dispatch_binding/preparation_tests.rs', base + 'shared_memory/tests/preparation.rs',
    base + 'shared_memory/tests/pristine_abort.rs', lowerTests].sort(),
  addedSource: [control, tests].sort(), sourceCount: 5685, newTests, sourceGuard,
  docRelocations: [
    ['queue_dispatch_binding.rs', 'queue::dispatch_binding::Gfx942FixedDispatchPacketV1', 160, 163],
    ['queue_dispatch_binding.rs', 'queue::dispatch_binding::Gfx942FixedDispatchDataV1', 334, 337],
    ['queue_dispatch_binding.rs', 'queue::dispatch_binding::Gfx942DispatchBatchV1', 3054, 3047],
    ['queue_dispatch_binding.rs', 'queue::dispatch_binding::Gfx942DispatchBatchV1', 3064, 3057],
    ['queue_dispatch_binding.rs', 'queue::dispatch_binding::Gfx942DispatchBatchV1', 3072, 3065],
    ['queue_dispatch_binding.rs', 'queue::dispatch_binding::Gfx942CompletedDispatchBatchV1', 3098, 3091],
  ],
  test, focused: [['returning', returning, 15], ['pristine', 'pristine_abort', 37], ['cleanup', lower, 7], ['transport', transport, 9]],
  mutations, allowedMutationTests: newTests.filter(n => n !== sourceGuard),
  helpers: ['r115-run-v4.js', 'r115-qualification-plan.js', 'r115-qualification-evidence.js',
    'r115-qualification-tests.js', 'r115-qualification-prepare.js', 'r115-qualification-run.js', 'r115-qualification-collect.js'],
};
