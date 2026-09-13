const parent = '356523e6ea8c61c70c6812762aeaab33506de6e2';
const accepted = parent;
const previous = 'docs/evidence/local-r110-live-coherent-insertion-2026-09-12/';
const sourceDelta = [
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_coherent_insertion_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_uninitialized_coherent_insertion_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/data_insertion.rs',
  'crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests/insertion.rs',
  'crates/fe2o3-kfd/src/shared_memory.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/live_coherent_insertion.rs',
  'crates/fe2o3-kfd/src/shared_memory/transitions.rs',
];
const constructed = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::insertion_cases::coherent_cases::uninitialized_cases::coherent_allocation_insertion_';
const roots = 'shared_memory::tests::live_coherent_insertion::coherent_allocation_insertion_';
const facade = 'queue::live::rebind_tests::insertion::coherent_allocation_insertion_';
const newTests = [
  ...['currentness_matrix_retains_exact_prefix_and_model', 'native_allocation_matrix_preserves_pending_owner',
    'map_matrix_retains_cpu_authority', 'projection_matrix_preserves_checkpoint_and_exact_successor',
    'success_preserves_exact_uninitialized_order_and_accounts', 'completed_owner_survives_retake_and_commit_failure',
    'required_hole_validation_and_lower_size_rejection', 'opening_failure_and_missing_complete_never_commit',
    'admits_sixteenth_then_rejects_full_before_index_or_reservation', 'uses_real_release_hole_and_explicit_override',
    'operation_panic_wins_secondary_retake_error_or_panic', 'retake_failure_precedes_ordinary_operation_error'].map(n => constructed + n),
  ...['root_is_one_shot_without_copy_or_readback', 'empty_root_preserves_lower_size_rejection_checkpoint',
    'preserves_earlier_map_failure_custody'].map(n => roots + n),
  ...['public_preflight_preserves_owners_and_precedence', 'bound_dispatch_precedes_full_and_invalid_size',
    'missing_engine_retains_terminal_parent_without_panic_marker', 'production_wiring_never_grants_initialized_authority'].map(n => facade + n),
].sort();
const tests = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const focused = [
  ['insertion', 'coherent_allocation_insertion_', 19],
  ['coherent-insertion', 'coherent_insertion_', 19],
  ['device-insertion', 'device_insertion_', 14],
  ['initializer', 'shared_memory::tests::transitions::coherent_initialization::', 10],
  ['borrowed', 'borrowed_initialization_', 8],
  ['model-loan', 'queue::live::model_loan::tests::', 4],
];
const helpers = ['r111-exact-clock-check-mutation.js', 'r111-exact-clock-evidence-tests.js', 'r111-exact-clock-evidence.js',
  'r111-exact-clock-prepare.js', 'r111-exact-clock-restoration.js', 'r111-exact-clock-retain-local.js', 'r111-exact-clock-run.js',
  'r111-exact-qualification-plan.js'];
const d = 'crates/fe2o3-kfd/src/queue_live/data_insertion.rs';
const s = 'crates/fe2o3-kfd/src/shared_memory.rs';
const t = 'crates/fe2o3-kfd/src/shared_memory/transitions.rs';
const l = 'crates/fe2o3-kfd/src/queue_live/model_loan.rs';
const testPath = 'crates/fe2o3-kfd/src/queue_live/construction_primary/../construction_auxiliary/integration_uninitialized_coherent_insertion_tests.rs';
const rootPath = 'crates/fe2o3-kfd/src/shared_memory/tests/live_coherent_insertion.rs';
const extraction = `        let ledger = context.ledger();
        insert_detached_identity_at(ledger.identities, ledger.next, identity, index);
        *ledger.count = next_count;
        let data = root
            .take_data()
            .expect("borrowed Complete remains rooted through ledger commit");`;
const preparation = `    fn prepare_with_memory(
        &mut self,
        memory: &mut impl coherent_initialization::CoherentInitializationV1,
        requested_bytes: usize,
    ) -> Result<(), MemorySessionError> {
        if core::mem::replace(&mut self.started, true) {`;
const allocation = '        let allocation = memory.allocate(requested_bytes)?;';
const mutations = [
  {name: 'omit-complete-retention', path: s, test: constructed + 'completed_owner_survives_retake_and_commit_failure',
    edits: [['transitions::retain_coherent_insertion_output_v1(engine, completed);',
      'let _ = engine; let _retained = core::mem::ManuallyDrop::new(completed);']],
    expected: ['coherent owner retained after failed settlement'], oracle_path: rootPath, oracle_line: 988},
  {name: 'extract-before-commit-entry', path: d, test: constructed + 'completed_owner_survives_retake_and_commit_failure',
    edits: [[extraction, `        let data = root
            .take_data()
            .expect("borrowed Complete remains rooted through ledger commit");
        let ledger = context.ledger();
        insert_detached_identity_at(ledger.identities, ledger.next, identity, index);
        *ledger.count = next_count;`]],
    expected: ['Complete retained until the full ledger commit'], oracle_path: testPath, oracle_line: 397},
  {name: 'insert-copy-into-allocation', path: s, test: roots + 'root_is_one_shot_without_copy_or_readback',
    edits: [[allocation, allocation + '\n        let allocation = memory.copy(allocation, &vec![0u8; requested_bytes])?;']],
    expected: ['assertion `left == right` failed', '[1, 1, 1]', '[1, 0, 1]'], oracle_path: rootPath, oracle_line: 217},
  {name: 'grant-initialized-authority', path: d, test: constructed + 'success_preserves_exact_uninitialized_order_and_accounts',
    edits: [['.map(Gfx942FixedDispatchDataV1::host_visible_uninitialized)',
      '.map(|token| Gfx942FixedDispatchDataV1::host_visible_initialized(crate::shared_memory::Gfx942InitializedHostVisibleMemoryV1::from_completed_dispatch(token)))']],
    expected: ['assertion `left == right` failed', 'HostVisibleInitialized', 'HostVisibleUninitialized'], oracle_path: testPath, oracle_line: 291},
  {name: 'allow-root-repreparation', path: s, test: roots + 'root_is_one_shot_without_copy_or_readback',
    edits: [[preparation, preparation.replace('if core::mem::replace', 'if false && core::mem::replace')]],
    expected: ['assertion failed: matches!', 'InvalidAllocationAuthority'], oracle_path: rootPath, oracle_line: 234},
  {name: 'hoist-size-rejection', path: s, test: roots + 'empty_root_preserves_lower_size_rejection_checkpoint',
    edits: [[allocation, '        if requested_bytes == 0 { return Err(MemorySessionError::InvalidRequestedSize); }\n' + allocation]],
    expected: ['assertion `left == right` failed', '[0, 0, 0]', '[1, 0, 0]'], oracle_path: rootPath, oracle_line: 291},
  {name: 'overwrite-earlier-terminal', path: t, test: roots + 'preserves_earlier_map_failure_custody',
    edits: [['if engine.terminal_transition.is_some() {\n        // Failure retention',
      'if false && engine.terminal_transition.is_some() {\n        // Failure retention']],
    expected: ['assertion `left == right` failed', 'LiveInsertion', 'Map'], oracle_path: rootPath, oracle_line: 989},
  {name: 'change-requested-extent', path: s, test: roots + 'root_is_one_shot_without_copy_or_readback',
    edits: [[allocation, '        let allocation = memory.allocate(requested_bytes.saturating_add(1))?;']],
    expected: ['assertion `left == right` failed', 'requested_bytes: 2', 'requested_bytes: 1'], oracle_path: rootPath, oracle_line: 882},
  {name: 'append-without-required-hole', path: d, test: constructed + 'required_hole_validation_and_lower_size_rejection',
    edits: [['DataInsertionIndexV1::RequiredHole => ledger\n                .next\n                .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?,',
      'DataInsertionIndexV1::RequiredHole => ledger.next.unwrap_or(ledger.identities.len()),']],
    expected: ['assertion failed: !matches!(settled.result, Ok(Ok(_)))'], oracle_path: testPath, oracle_line: 451},
  {name: 'hole-overrides-explicit-index', path: d, test: constructed + 'uses_real_release_hole_and_explicit_override',
    edits: [['DataInsertionIndexV1::Explicit(index) => index,', 'DataInsertionIndexV1::Explicit(index) => ledger.next.unwrap_or(index),']],
    expected: ['assertion `left == right` failed'], oracle_path: testPath, oracle_line: 664},
  {name: 'admit-seventeenth', path: d, test: constructed + 'admits_sixteenth_then_rejects_full_before_index_or_reservation',
    edits: [['if *ledger.count >= super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {',
      'if false && *ledger.count >= super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {']],
    expected: ['assertion failed: !settled.transport'], oracle_path: testPath, oracle_line: 603},
  {name: 'skip-explicit-index-validation', path: d, test: constructed + 'required_hole_validation_and_lower_size_rejection',
    edits: [['validate_new_detached_data_index(*ledger.count, index)?;', 'if false { validate_new_detached_data_index(*ledger.count, index)?; }']],
    expected: ['assertion `left == right` failed'], oracle_path: testPath, oracle_line: 450},
  {name: 'replace-substrate-first-panic', path: l, test: constructed + 'operation_panic_wins_secondary_retake_error_or_panic',
    edits: [['core::mem::forget(closing);\n            resume_unwind(payload)',
      'match closing {\n                Err(secondary) => { core::mem::forget(payload); resume_unwind(secondary) }\n                other => { core::mem::forget(other); resume_unwind(payload) }\n            }']],
    expected: ['assertion `left == right` failed', 'left: None', 'N2 native panic', 'map_gpu'], oracle_path: testPath, oracle_line: 716, reused_substrate: true},
  {name: 'suppress-substrate-retake-error', path: l, test: constructed + 'retake_failure_precedes_ordinary_operation_error',
    edits: [['Ok(closing) => Ok((result, closing)),', 'Ok(_closing) => Ok((result, Ok(()))),']],
    expected: ['retake must precede the ordinary operation error'], oracle_path: testPath, oracle_line: 801, reused_substrate: true},
];
module.exports = {parent, accepted, previous, sourceDelta, newTests, tests, focused, helpers, mutations};
