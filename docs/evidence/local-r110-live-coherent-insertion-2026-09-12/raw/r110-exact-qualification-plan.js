const parent = 'a9e1c73bb9b752610e58be9cd96861962e140787';
const accepted = parent;
const previous = 'docs/evidence/local-r109-live-device-insertion-2026-09-12/';
const sourceDelta = [
  'crates/fe2o3-kfd/src/queue_live.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_coherent_insertion_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_insertion_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/data_insertion.rs',
  'crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests/insertion.rs',
  'crates/fe2o3-kfd/src/shared_memory.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/live_coherent_insertion.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/primary_projection.rs',
  'crates/fe2o3-kfd/src/shared_memory/transitions.rs',
];
const constructed = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::insertion_cases::coherent_cases::coherent_insertion_';
const roots = 'shared_memory::tests::live_coherent_insertion::coherent_insertion_';
const facade = 'queue::live::rebind_tests::insertion::coherent_insertion_';
const newTests = [
  ...['currentness_matrix_retains_exact_prefix_and_model', 'native_allocation_matrix_preserves_pending_owner',
    'copy_and_map_matrix_retains_cpu_authority', 'projection_matrix_preserves_checkpoint_and_exact_successor',
    'constructed_success_preserves_exact_order_source_and_accounts', 'completed_owner_survives_retake_and_commit_failure',
    'required_hole_and_validation_reject_before_reservation', 'opening_failure_and_missing_complete_never_commit',
    'admits_sixteenth_then_rejects_full_before_index_or_reservation', 'uses_real_release_hole_and_explicit_override',
    'operation_panic_wins_secondary_retake_error_or_panic', 'retake_failure_precedes_ordinary_operation_error'].map(n => constructed + n),
  ...['root_is_one_shot_and_extracts_once', 'empty_root_retention_and_rejected_attempt_have_no_effects',
    'retention_preserves_earlier_lower_failure'].map(n => roots + n),
  ...['concrete_facade_restores_before_transport_for_all_routes', 'all_public_entrypoints_preserve_preflight_precedence',
    'actual_bound_dispatch_precedes_full_and_malformed_ledger', 'production_routing_preserves_root_and_terminal_owner'].map(n => facade + n),
].sort();
const tests = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const focused = [
  ['insertion', 'coherent_insertion_', 19],
  ['device-insertion', 'device_insertion_', 14],
  ['initializer', 'shared_memory::tests::transitions::coherent_initialization::', 10],
  ['borrowed', 'borrowed_initialization_', 8],
  ['model-loan', 'queue::live::model_loan::tests::', 4],
];
const helpers = ['r110-exact-clock-check-mutation.js', 'r110-exact-clock-evidence-tests.js', 'r110-exact-clock-evidence.js',
  'r110-exact-clock-prepare.js', 'r110-exact-clock-restoration.js', 'r110-exact-clock-retain-local.js', 'r110-exact-clock-run.js',
  'r110-exact-qualification-plan.js'];
const d = 'crates/fe2o3-kfd/src/queue_live/data_insertion.rs';
const s = 'crates/fe2o3-kfd/src/shared_memory.rs';
const t = 'crates/fe2o3-kfd/src/shared_memory/transitions.rs';
const l = 'crates/fe2o3-kfd/src/queue_live/model_loan.rs';
const testPath = 'crates/fe2o3-kfd/src/queue_live/construction_primary/../construction_auxiliary/integration_coherent_insertion_tests.rs';
const rootPath = 'crates/fe2o3-kfd/src/shared_memory/tests/live_coherent_insertion.rs';
const extraction = `        let ledger = context.ledger();
        insert_detached_identity_at(ledger.identities, ledger.next, identity, index);
        *ledger.count = next_count;
        let data = root
            .take_data()
            .expect("borrowed Complete remains rooted through ledger commit");`;
const mutations = [
  {name: 'omit-complete-retention', path: s, test: constructed + 'completed_owner_survives_retake_and_commit_failure',
    edits: [['transitions::retain_coherent_insertion_output_v1(engine, completed);', 'let _retained = core::mem::ManuallyDrop::new(completed);']],
    expected: ['coherent owner retained after failed settlement'], oracle_path: rootPath, oracle_line: 767},
  {name: 'extract-before-commit-entry', path: d, test: constructed + 'completed_owner_survives_retake_and_commit_failure',
    edits: [[extraction, `        let data = root
            .take_data()
            .expect("borrowed Complete remains rooted through ledger commit");
        let ledger = context.ledger();
        insert_detached_identity_at(ledger.identities, ledger.next, identity, index);
        *ledger.count = next_count;`]],
    expected: ['Complete retained until the full ledger commit'], oracle_path: testPath, oracle_line: 711},
  {name: 'allow-root-repreparation', path: s, test: roots + 'root_is_one_shot_and_extracts_once',
    edits: [['core::mem::replace(&mut self.started, true)', 'core::mem::replace(&mut self.started, false)']],
    expected: ['assertion failed: matches!', 'InvalidAllocationAuthority'], oracle_path: rootPath, oracle_line: 41},
  {name: 'overwrite-earlier-terminal', path: t, test: roots + 'retention_preserves_earlier_lower_failure',
    edits: [['if engine.terminal_transition.is_some() {\n        // Failure retention', 'if false && engine.terminal_transition.is_some() {\n        // Failure retention']],
    expected: ['assertion `left == right` failed', 'LiveInsertion', 'Copy'], oracle_path: rootPath, oracle_line: 768},
  {name: 'append-without-required-hole', path: d, test: constructed + 'required_hole_and_validation_reject_before_reservation',
    edits: [['DataInsertionIndexV1::RequiredHole => ledger\n                .next\n                .ok_or(Gfx942DispatchBindingErrorV1::ResourcePhase)?,',
      'DataInsertionIndexV1::RequiredHole => ledger.next.unwrap_or(ledger.identities.len()),']],
    expected: ['assertion failed: !matches!(settled.result, Ok(Ok(_)))'], oracle_path: testPath, oracle_line: 758},
  {name: 'hole-overrides-explicit-index', path: d, test: constructed + 'uses_real_release_hole_and_explicit_override',
    edits: [['DataInsertionIndexV1::Explicit(index) => index,', 'DataInsertionIndexV1::Explicit(index) => ledger.next.unwrap_or(index),']],
    expected: ['assertion `left == right` failed'], oracle_path: testPath, oracle_line: 940},
  {name: 'admit-seventeenth', path: d, test: constructed + 'admits_sixteenth_then_rejects_full_before_index_or_reservation',
    edits: [['if *ledger.count >= super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {', 'if *ledger.count > super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {']],
    expected: ['assertion failed: !settled.transport'], oracle_path: testPath, oracle_line: 883},
  {name: 'skip-explicit-index-validation', path: d, test: constructed + 'required_hole_and_validation_reject_before_reservation',
    edits: [['validate_new_detached_data_index(*ledger.count, index)?;', 'validate_new_detached_data_index(*ledger.count, 0)?;']],
    expected: ['assertion `left == right` failed'], oracle_path: testPath, oracle_line: 757},
  {name: 'suppress-substrate-retake-error', path: l, test: constructed + 'retake_failure_precedes_ordinary_operation_error',
    edits: [['Ok(closing) => Ok((result, closing)),', 'Ok(_closing) => Ok((result, Ok(()))),']],
    expected: ['retake must precede the ordinary operation error'], oracle_path: testPath, oracle_line: 1058, reused_substrate: true},
  {name: 'replace-substrate-first-panic', path: l, test: constructed + 'operation_panic_wins_secondary_retake_error_or_panic',
    edits: [['core::mem::forget(closing);\n            resume_unwind(payload)',
      'match closing {\n                Err(secondary) => resume_unwind(secondary),\n                Ok(secondary) => { core::mem::forget(secondary); resume_unwind(payload) }\n            }']],
    expected: ['assertion `left == right` failed', 'coherent insertion copy', 'auxiliary-retake'], oracle_path: testPath, oracle_line: 983, reused_substrate: true},
];
module.exports = {parent, accepted, previous, sourceDelta, newTests, tests, focused, helpers, mutations};
