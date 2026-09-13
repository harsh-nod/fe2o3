const parent = 'fb5e19002e5a451aaf27f4bc16fc78c00b9163b3';
const accepted = 'd85d6d7dc4f3b065dce9dc6505eac065c48cb0fa';
const previous = 'docs/evidence/local-r113-context-version-membership-2026-09-13/';
const base = 'crates/fe2o3-kfd/src/';
const control = base + 'shared_memory/control_cleanup.rs';
const shared = base + 'shared_memory.rs';
const binding = base + 'queue_dispatch_binding/pristine_abort.rs';
const live = base + 'queue_live/pristine_abort.rs';
const lowerTests = base + 'shared_memory/tests/pristine_abort/cleanup_tests.rs';
const transportTests = base + 'queue_live/tests/pristine_abort_transport.rs';
const lower = 'shared_memory::tests::pristine_abort::cleanup_tests::';
const abort = 'queue::dispatch_binding::pristine_abort::tests::';
const transport = 'queue::live::pristine_abort::tests::transport_tests::';
const KS = lower + 'cleanup_success_commits_original_model_and_exact_native_control_order';
const KP = lower + 'cleanup_projection_failures_keep_native_disposal_distinct_from_model_commit';
const KC = lower + 'cleanup_currentness_sweep_retains_disposed_receipt_before_final_check';
const KN = lower + 'cleanup_native_failure_retains_typed_control_and_exact_destructive_prefix';
const KH = lower + 'cleanup_preserves_two_separate_revision_preflights_and_last_valid_commit';
const KU = lower + 'cleanup_unmap_outcomes_preserve_prefix_errno_precedence_without_retag';
const BI = abort + 'pristine_abort_incomplete_callback_keeps_active_control_and_rejects_retry';
const BA = abort + 'pristine_abort_native_errors_and_panics_keep_data_and_disallow_retry';
const TF = transport + 'abort_facade_restores_all_lanes_before_retaining_failed_control_parent';
const TS = transport + 'abort_cleanup_panic_keeps_secondary_payload_destructor_inert';
const TD = transport + 'abort_direct_finisher_retains_exact_parent_before_original_panic';
const sourceGuard = transport + 'abort_public_paths_use_settled_transport_before_return_or_resume';
const fullTest = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '--no-fail-fast',
  ...['fe2o3-completion', 'fe2o3-runtime-model', 'fe2o3-resource-accounting', 'fe2o3-kfd', 'fe2o3-runtime'].flatMap(n => ['-p', n]),
  '--all-features', '--all-targets'];
const newTests = [BI, KS, KP, KC, KN, KH, KU,
  lower + 'cleanup_actual_commit_rejection_keeps_unmapped_or_disposed_custody',
  TF, TS, TD, sourceGuard,
  transport + 'abort_cleanup_panic_survives_panicking_secondary_destructor',
  transport + 'abort_direct_finisher_retains_exact_parent_before_returned_error',
  transport + 'abort_direct_finisher_returns_success_without_transport',
  transport + 'abort_direct_rejection_and_terminal_retry_do_not_transport_parent',
  transport + 'abort_public_facade_preflight_and_missing_engine_do_not_transfer_healthy_parent',
].sort();
const eq = 'assertion `left == right` failed';
const mutation = (name, path, from, to, test, oracle_path, oracle_line, expected) => ({
  name, path, edits: [[from, to]], test, oracle_path, oracle_line, expected,
});
const preflight = `        preflight_queue_foundation_native_memory_transition_v1(
            projection.foundation,
            engine,
            1,
            &mut process_poison,
        )?;`;
const commit = value => `        projection
            .foundation
            .replace_memory_after_sealed_transition(${value})
            .map_err(MemorySessionError::Model)?;`;
// These are predicted behavioral failures, not executed qualification results.
const mutations = [
  mutation('root-retry', control, '    if custody.started {', '    if false && custody.started {',
    KS, lowerTests, 307, [eq, 'retry changed retained cleanup state']),
  mutation('missing-failure-receipt', control, '    custody.retain_disposed_receipt();\n    match result {', '    match result {',
    KC, lowerTests, 751, [eq, 'left: "Unmapped"', 'right: "NativeDisposed"']),
  mutation('late-disposal-marker', shared,
    '        progress.native_disposed = true;\n        self.allocations[index].reservation = None;\n        self.check_currentness()?;',
    '        self.allocations[index].reservation = None;\n        self.check_currentness()?;\n        progress.native_disposed = true;',
    KC, lowerTests, 751, [eq, 'left: "Unmapped"', 'right: "NativeDisposed"']),
  mutation('missing-unmap-commit', control, commit('unmapped'), '        let _ = unmapped;',
    KP, lowerTests, 676, [eq, 'MemoryLifecycleStateV1']),
  mutation('missing-release-commit', control, commit('released'), '        let _ = released;',
    KS, lowerTests, 493, [eq, 'MemoryLifecycleStateV1']),
  mutation('combined-revision-preflight', control,
    '        projection.stage(custody, CleanupStageV1::UnmapPreflight)?;\n' + preflight,
    '        projection.stage(custody, CleanupStageV1::UnmapPreflight)?;\n' + preflight.replace('            1,', '            2,'),
    KH, lowerTests, 841, [eq, 'left: "Mapped"', 'right: "Unmapped"']),
  mutation('missing-release-preflight', control,
    '        projection.stage(custody, CleanupStageV1::ReleasePreflight)?;\n' + preflight,
    '        projection.stage(custody, CleanupStageV1::ReleasePreflight)?;',
    KH, lowerTests, 839, [eq, 'left: 0', 'right: 1']),
  mutation('lost-attempted-free', shared,
    '        self.allocations[index].free_attempted = true;\n        progress.free.attempted = true;',
    '        progress.free.attempted = true;',
    KN, lowerTests, 574, [eq, 'free_attempted: false', 'free_attempted: true']),
  mutation('malformed-unmap-precedence', shared,
    '        if outcome.value > 1 {\n            return self.quarantine(MemorySessionError::KernelResultMalformed(\n                "shared UNMAP_MEMORY_FROM_GPU cumulative n_success",',
    '        if false && outcome.value > 1 {\n            return self.quarantine(MemorySessionError::KernelResultMalformed(\n                "shared UNMAP_MEMORY_FROM_GPU cumulative n_success",',
    KU, lowerTests, 616, [eq, 'shared UNMAP_MEMORY_FROM_GPU full prefix', 'shared UNMAP_MEMORY_FROM_GPU cumulative n_success']),
  mutation('unroot-active-callback', binding,
    '        let active = self.active_control.as_mut().expect("rooted active control");\n        memory.release_control(active)?;',
    '        let mut active = self.active_control.take().expect("rooted active control");\n        memory.release_control(&mut active)?;',
    BA, binding, 693, ['interrupted control remains rooted']),
  mutation('accept-incomplete-callback', binding, '        if !active.is_complete() {', '        if false && !active.is_complete() {',
    BI, binding, 637, ['assertion failed: matches!', 'abort.release_controls(&mut cleanup)', 'ResourcePhase']),
  mutation('missing-facade-transport', live,
    '        *self.terminal_transport |= settled.transport;', '        let _ = settled.transport;',
    TF, transportTests, 283, [eq, 'transport must precede return or panic resumption', 'left: true', 'right: false']),
  mutation('clear-sticky-transport', live,
    '        *self.terminal_transport |= settled.transport;', '        *self.terminal_transport = settled.transport;',
    TF, transportTests, 283, [eq, 'poisoned retry cannot clear an earlier transfer', 'left: true', 'right: false']),
  mutation('destroy-secondary-payload', live, '                core::mem::forget(envelope);', '                drop(envelope);',
    TS, transportTests, 726, [eq, 'secondary retake payload was destroyed', 'left: 1', 'right: 0']),
  mutation('missing-direct-retention', live, '        if settled.transport {', '        if false && settled.transport {',
    TD, transportTests, 483, [eq, 'direct parent must be retained before return or panic resumption', 'left: 0', 'right: 1']),
];
module.exports = {
  parent, accepted, previous,
  prerequisites: [
    ['r114-preliminary-gnu-all', fullTest, 'r114-device-oracle-format.json'],
    ['r114-preliminary-musl-all', [...fullTest, '--target', 'x86_64-unknown-linux-musl'], 'r114-preliminary-gnu-all.json'],
  ],
  freezeHelpers: ['r114-run-v3.js', 'r114-run-v4.js', 'r114-source-gate.py', 'r114-auxiliary-gates.py',
    'r114-freeze.js', 'r114-runner-tests.js', 'r114-freeze-tests.js', 'r114-qualification-plan.js', 'r114-qualification-evidence.js'],
  freezeContractCount: 30,
  qualificationContractCount: 80,
  productionPaths: [binding, live, shared, control].sort(),
  sourceDelta: [base + 'queue.rs', binding, base + 'queue_live.rs', live, base + 'queue_live/rebind_tests.rs',
    transportTests, shared, control, base + 'shared_memory/tests/pristine_abort.rs', lowerTests,
    base + 'shared_memory/transitions.rs'].sort(),
  sourceCount: 5683, newTests, sourceGuard,
  docRelocations: [
    ['shared_memory.rs', 'shared_memory::Gfx942DeviceMemoryLeaseV1', 249, 254],
    ['shared_memory.rs', 'shared_memory::Gfx942DeviceMemoryLeaseV1', 260, 265],
    ['shared_memory.rs', 'shared_memory::Gfx942InitializedDeviceMemoryV1', 475, 480],
    ['shared_memory.rs', 'shared_memory::Gfx942InitializedDeviceMemoryV1', 483, 488],
    ['shared_memory.rs', 'shared_memory::SharedGttAllocationV1', 823, 828],
  ],
  test: ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'],
  focused: [['pristine', 'pristine_abort', 37], ['cleanup', lower, 7], ['transport', transport, 9]],
  mutations, allowedMutationTests: [...new Set([...newTests.filter(n => n !== sourceGuard), BA])].sort(),
  helpers: ['r114-run-v4.js', 'r114-qualification-plan.js', 'r114-qualification-evidence.js',
    'r114-qualification-tests.js', 'r114-qualification-prepare.js', 'r114-qualification-run.js',
    'r114-qualification-collect.js'],
};
