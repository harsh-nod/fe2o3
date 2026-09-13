const parent = '29505205cc54bab1885a67845aaceda8b22b3a9c';
const accepted = parent;
const previous = 'docs/evidence/local-r111-uninitialized-coherent-insertion-2026-09-12/';
const sourceDelta = [
  'crates/fe2o3-kfd/src/queue_live.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_insertion_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_uninitialized_device_insertion_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/data_insertion.rs',
  'crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests/insertion.rs',
  'crates/fe2o3-kfd/src/shared_memory.rs',
  'crates/fe2o3-kfd/src/shared_memory/device_allocation.rs',
  'crates/fe2o3-kfd/src/shared_memory/device_initialization.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/device_allocation.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/device_initialization.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/live_coherent_insertion.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/live_insertion.rs',
];
const roots = 'shared_memory::tests::device_initialization::allocation_cases::device_allocator_';
const constructed = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::insertion_cases::allocation_cases::device_allocation_insertion_';
const direct = 'queue::live::rebind_tests::insertion::device_allocation_insertion_';
const sourceGuard = direct + 'production_wiring_preserves_uninitialized_custody';
const newTests = [
  ...[
    'success_retains_exact_device_local_owner_until_single_extraction',
    'currentness_matrix_distinguishes_per_call_native_admission',
    'native_error_and_panic_prefixes_retain_actual_owners',
    'map_prefix_matrix_preserves_progress_and_error_precedence',
    'malformed_allocations_never_fabricate_a_lease',
    'invalid_layout_and_coordinates_reject_before_effects',
    'capacity_rejection_and_real_release_allow_fresh_retry',
    'complete_output_can_be_retained_without_reallocation_on_default_stack',
    'mixed_terminal_kinds_block_each_other_without_overwrite',
    'preflight_arithmetic_and_configured_domain_are_side_effect_free',
    'independent_credit_limits_release_only_after_actual_disposal',
  ].map(n => roots + n),
  'shared_memory::tests::live_insertion::device_allocator_partition_does_not_invent_pending_custody_after_extraction',
  ...[
    'constructed_append_preserves_exact_uninitialized_identity',
    'uses_genuine_released_hole_before_append',
    'complete_survives_retake_and_commit_failures',
    'real_fifteen_to_sixteen_capacity_is_pre_effect_bounded',
    'reservation_and_opening_failures_never_enter_native_calls',
    'success_without_complete_cannot_commit',
    'native_prefixes_remain_in_original_terminal_custody',
    'currentness_and_map_prefixes_never_fabricate_output',
    'operation_panic_wins_each_secondary_closing_failure',
    'invalid_layout_retakes_without_native_custody',
  ].map(n => constructed + n),
  ...[
    'public_preflight_preserves_owners_and_precedence',
    'bound_dispatch_precedes_full_and_invalid_request',
    'missing_engine_retains_parent_without_panic_marker',
    'production_wiring_preserves_uninitialized_custody',
  ].map(n => direct + n),
].sort();
const tests = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const focused = [
  ['insertion', 'device_allocation_insertion_', 14],
  ['allocator', 'device_allocator_', 12],
  ['device-initializer', 'device_initializer_', 11],
  ['coherent-allocation', 'coherent_allocation_insertion_', 19],
  ['coherent-insertion', 'coherent_insertion_', 19],
  ['device-insertion', 'device_insertion_', 14],
  ['initializer', 'shared_memory::tests::transitions::coherent_initialization::', 10],
  ['borrowed', 'borrowed_initialization_', 8],
  ['model-loan', 'queue::live::model_loan::tests::', 4],
];
const helpers = ['r112-exact-clock-check-mutation.js', 'r112-exact-clock-evidence-tests.js', 'r112-exact-clock-evidence.js',
  'r112-exact-clock-prepare.js', 'r112-exact-clock-restoration.js', 'r112-exact-clock-retain-local.js', 'r112-exact-clock-run.js',
  'r112-exact-qualification-plan.js'];
const d = 'crates/fe2o3-kfd/src/shared_memory/device_allocation.rs';
const i = 'crates/fe2o3-kfd/src/shared_memory/device_initialization.rs';
const o = 'crates/fe2o3-kfd/src/shared_memory/tests/device_allocation.rs';
const insertion = 'crates/fe2o3-kfd/src/queue_live/data_insertion.rs';
const loan = 'crates/fe2o3-kfd/src/queue_live/model_loan.rs';
const testPath = 'crates/fe2o3-kfd/src/queue_live/construction_primary/../construction_auxiliary/integration_uninitialized_device_insertion_tests.rs';
const eq = 'assertion `left == right` failed';
const low = (name, path, test, line, from, to, expected) => ({
  name, path, test: roots + test, edits: [[from, to]], oracle_path: o, oracle_line: line, expected,
});
const extraction = `        let ledger = context.ledger();
        insert_detached_identity_at(ledger.identities, ledger.next, identity, index);
        *ledger.count = next_count;
        let data = root
            .take_data()
            .expect("borrowed Complete remains rooted through ledger commit");`;
// Predicted behavioral oracles, not executed results. Pin only after source review.
const mutations = [
  low('historical-admission', d, 'currentness_matrix_distinguishes_per_call_native_admission', 377,
    '        self.started = true;\n        let result =',
    '        self.started = true;\n        self.native_started = engine.device_backing_activity_started;\n        let result =',
    [eq, 'left: true', 'right: false']),
  low('omit-immediate-quarantine', d, 'native_error_and_panic_prefixes_retain_actual_owners', 183,
    '            if self.native_started {\n                engine.phase = SharedMemorySessionPhaseV1::Quarantined;\n            }',
    '            if false && self.native_started {\n                engine.phase = SharedMemorySessionPhaseV1::Quarantined;\n            }',
    [eq, 'left: Active', 'right: Quarantined']),
  low('lose-unmapped-owner', d, 'native_error_and_panic_prefixes_retain_actual_owners', 160,
    '        if !matches!(result, Ok(Ok(()))) {\n            self.failed = true;',
    '        if !matches!(result, Ok(Ok(()))) {\n            self.lease = AllocationLeaseV1::None;\n            self.failed = true;',
    [eq, 'left: None', 'right: Some(']),
  low('public-instead-of-device-local', d, 'success_retains_exact_device_local_owner_until_single_extraction', 160,
    '                    KfdAllocMemoryFlags::DEVICE_LOCAL,\n                    &mut self.native_started,',
    '                    KfdAllocMemoryFlags::DEVICE_LOCAL_PUBLIC,\n                    &mut self.native_started,',
    [eq, 'uapi_flags: 2684354561', 'uapi_flags: 2147483649']),
  low('discard-map-progress', d, 'map_prefix_matrix_preserves_progress_and_error_precedence', 544,
    '            engine.map_device_memory_borrowed(lease, &mut self.progress)?;',
    '            engine.map_device_memory_borrowed(lease, &mut NativeTransitionProgressV1::default())?;',
    [eq, '(false, None, None)', '(true, Some(true), Some(0))']),
  low('allow-repreparation', d, 'success_retains_exact_device_local_owner_until_single_extraction', 299,
    '        if self.started {\n            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);\n        }',
    '        if false && self.started {\n            return Err(MemorySessionError::InvalidDeviceMemoryAuthority);\n        }',
    ['assertion failed: matches!', 'InvalidDeviceMemoryAuthority']),
  low('lose-terminal-mapped-owner', d, 'complete_output_can_be_retained_without_reallocation_on_default_stack', 844,
    '        } else {\n            engine\n                .terminal_device_initialization\n                .retain_allocation(self);\n        }',
    '        } else {\n            self.lease = AllocationLeaseV1::None;\n            engine\n                .terminal_device_initialization\n                .retain_allocation(self);\n        }',
    [eq, 'lease: None', 'lease: Some']),
  low('expose-failed-output', d, 'complete_output_can_be_retained_without_reallocation_on_default_stack', 845,
    '        if !self.failed\n            && let AllocationLeaseV1::Mapped(lease) = &self.lease',
    '        if true\n            && let AllocationLeaseV1::Mapped(lease) = &self.lease',
    ['assertion failed: terminal.completed().is_err()']),
  low('allocation-ignores-occupied-slot', d, 'mixed_terminal_kinds_block_each_other_without_overwrite', 905,
    '            if engine.terminal_device_initialization.is_some() {\n                return engine.quarantine(MemorySessionError::SharedSessionQuarantined);\n            }',
    '            if false && engine.terminal_device_initialization.is_some() {\n                return engine.quarantine(MemorySessionError::SharedSessionQuarantined);\n            }',
    ['assertion failed: matches!', 'prepare(&mut fixture', 'SharedSessionQuarantined']),
  low('initialization-ignores-occupied-slot', i, 'mixed_terminal_kinds_block_each_other_without_overwrite', 912,
    '        if engine.terminal_device_initialization.is_some() {\n            return engine.quarantine(MemorySessionError::SharedSessionQuarantined);\n        }',
    '        if false && engine.terminal_device_initialization.is_some() {\n            return engine.quarantine(MemorySessionError::SharedSessionQuarantined);\n        }',
    ['assertion failed: matches!', 'Input::new(false, 17)', 'SharedSessionQuarantined']),
  {name: 'ignore-remembered-hole', path: insertion, test: constructed + 'uses_genuine_released_hole_before_append',
    edits: [['DataInsertionIndexV1::HoleOrAppend => ledger.next.unwrap_or(ledger.identities.len()),',
      'DataInsertionIndexV1::HoleOrAppend => ledger.identities.len(),']],
    expected: [eq], oracle_path: testPath, oracle_line: 335},
  {name: 'grant-initialized-authority', path: insertion, test: constructed + 'constructed_append_preserves_exact_uninitialized_identity',
    edits: [['.map(Gfx942FixedDispatchDataV1::uninitialized)', '.map(Gfx942FixedDispatchDataV1::initialized_after_dispatch)']],
    expected: ['assertion failed: !data.is_fully_initialized()'], oracle_path: testPath, oracle_line: 256},
  {name: 'extract-before-commit-entry', path: insertion, test: constructed + 'complete_survives_retake_and_commit_failures',
    edits: [[extraction, `        let data = root
            .take_data()
            .expect("borrowed Complete remains rooted through ledger commit");
        let ledger = context.ledger();
        insert_detached_identity_at(ledger.identities, ledger.next, identity, index);
        *ledger.count = next_count;`]],
    expected: [eq, 'lease: None', 'lease: Some'], oracle_path: testPath, oracle_line: 424},
  {name: 'accept-missing-complete', path: insertion, test: constructed + 'success_without_complete_cannot_commit',
    edits: [['let identity = root.completed_identity()?;',
      'let identity = root.completed_identity().unwrap_or_else(|_| context.ledger().identities[0]);']],
    expected: ['assertion failed: matches!', 'InvalidDeviceMemoryAuthority'], oracle_path: testPath, oracle_line: 560},
  {name: 'replace-substrate-first-panic', path: loan, test: constructed + 'operation_panic_wins_each_secondary_closing_failure',
    edits: [['core::mem::forget(closing);\n            resume_unwind(payload)',
      'match closing {\n                Err(secondary) => { core::mem::forget(payload); resume_unwind(secondary) }\n                other => { core::mem::forget(other); resume_unwind(payload) }\n            }']],
    expected: [eq, 'left: None', 'N2 native panic', 'map_gpu'], oracle_path: testPath, oracle_line: 775, reused_substrate: true},
  {name: 'suppress-substrate-retake-error', path: loan, test: constructed + 'operation_panic_wins_each_secondary_closing_failure',
    edits: [['Ok(closing) => Ok((result, closing)),', 'Ok(_closing) => Ok((result, Ok(()))),']],
    expected: ['assertion failed: matches!', 'Contract(actual)'], oracle_path: testPath, oracle_line: 789, reused_substrate: true},
];
module.exports = {parent, accepted, previous, sourceDelta, sourceGuard, newTests, tests, focused, helpers, mutations};
