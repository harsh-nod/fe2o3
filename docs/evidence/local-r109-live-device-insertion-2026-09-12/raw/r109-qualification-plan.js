const parent = '17d0be5476a4f71c473a619903c62aa316a96fc7';
const accepted = '9171bd68d920681e524d8eda5a29053a8fd8ae88';
const previous = 'docs/evidence/local-r108-context-writer-issuance-2026-09-12/';
const sourceDelta = [
  'crates/fe2o3-kfd/src/queue_live.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_insertion_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_prefix_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/data_insertion.rs',
  'crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests/insertion.rs',
  'crates/fe2o3-kfd/src/shared_memory.rs',
  'crates/fe2o3-kfd/src/shared_memory/device_initialization.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/device_initialization.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/live_insertion.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/primary_construction.rs',
];
const constructedPrefix = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::insertion_cases::device_insertion_';
const facadePrefix = 'queue::live::rebind_tests::insertion::device_insertion_';
const newTests = [
  ...[
    'constructed_primary_and_auxiliary_preserve_success_identity_and_order',
    'completed_output_survives_each_failed_retake',
    'real_capacity_growth_and_fifteen_to_sixteen_are_pre_effect_bounded',
    'invalid_ordinal_and_reservation_reject_before_loan',
    'opening_failure_never_initializes_or_retakes',
    'success_without_complete_cannot_commit_metadata',
    'native_and_currentness_failures_retain_exact_prefixes',
    'complete_stays_rooted_until_commit_ledger_access',
    'uses_a_genuine_release_hole_and_explicit_index_overrides_it',
    'operation_panic_wins_secondary_retake_error_or_panic',
  ].map(n => constructedPrefix + n),
  ...[
    'concrete_facade_restores_before_transport_even_after_swallowed_error',
    'concrete_preflight_keeps_error_precedence_and_healthy_parent',
    'concrete_bound_dispatch_precedes_full_and_malformed_ledger',
    'production_routing_roots_reserves_settles_commits_then_extracts',
  ].map(n => facadePrefix + n),
].sort();
const tests = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const focused = [
  ['insertion', 'device_insertion_', 14],
  ['initializer', 'shared_memory::tests::device_initialization::', 11],
  ['model-loan', 'queue::live::model_loan::tests::', 4],
];
const helpers = ['r109-clock-check-mutation.js', 'r109-clock-evidence-tests.js', 'r109-clock-evidence.js',
  'r109-clock-prepare.js', 'r109-clock-restoration.js', 'r109-clock-retain-local.js', 'r109-clock-run.js',
  'r109-qualification-plan.js'];
const d = 'crates/fe2o3-kfd/src/queue_live/data_insertion.rs';
const i = 'crates/fe2o3-kfd/src/shared_memory/device_initialization.rs';
const q = 'crates/fe2o3-kfd/src/queue_live.rs';
const testPath = 'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_insertion_tests.rs';
const facadePath = 'crates/fe2o3-kfd/src/queue_live/rebind_tests/insertion.rs';
const capacity = constructedPrefix + 'real_capacity_growth_and_fifteen_to_sixteen_are_pre_effect_bounded';
const retake = constructedPrefix + 'completed_output_survives_each_failed_retake';
const facade = facadePrefix + 'concrete_facade_restores_before_transport_even_after_swallowed_error';
const extraction = `        let ledger = context.ledger();
        insert_detached_identity_at(ledger.identities, ledger.next, identity, index);
        *ledger.count = next_count;
        let memory = root
            .take_complete()
            .expect("borrowed Complete remains rooted through ledger commit");`;
const mutations = [
  {name: 'omit-reservation', path: d, test: capacity,
    edits: [['context.reserve()?;', 'if false { context.reserve()?; }']],
    expected: ['identity capacity reserved before native effects', 'assertion failed: !settled.transport'], oracle_path: testPath},
  {name: 'admit-seventeenth', path: d, test: capacity,
    edits: [['if *ledger.count >= super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {',
      'if false && *ledger.count >= super::super::dispatch_binding::MAX_DISPATCH_DATA_LEASES_V1 {']],
    expected: ['assertion failed: matches!(settled.result,', 'requested: 17', 'maximum: 16'], oracle_path: testPath},
  {name: 'ignore-retake-error', path: d, test: retake,
    edits: [['context.prepare(&mut root, alignment)?;', 'let _ = context.prepare(&mut root, alignment);']],
    expected: ['assertion failed: settled.transport'], oracle_path: testPath},
  {name: 'drop-complete', path: i, test: retake,
    edits: [['        } else {\n            engine.terminal_device_initialization.retain(self);\n        }\n    }\n\n    fn retain_failure',
      '        } else {\n            core::mem::drop(self);\n        }\n    }\n\n    fn retain_failure']],
    expected: ['called `Option::unwrap()` on a `None` value'], oracle_path: testPath},
  {name: 'complete-still-usable', path: i, test: retake,
    edits: [['        self.failed = true;\n        engine.phase = SharedMemorySessionPhaseV1::Quarantined;',
      '        self.failed = false;\n        engine.phase = SharedMemorySessionPhaseV1::Quarantined;']],
    expected: ['assertion `left == right` failed', 'failed: false', 'failed: true'], oracle_path: testPath},
  {name: 'omit-error-transport', path: d, test: facade,
    edits: [['let transport = result.is_err() || entered && !matches!(result, Ok(Ok(_)));',
      'let transport = result.is_err() || entered && false;']],
    expected: ['assertion failed: *selected.terminal_transport'], oracle_path: facadePath},
  {name: 'globalize-ordinary-error', path: d, test: facade,
    edits: [['if panicked {\n            poison_process_global_after_dispatch_terminal_v1();',
      'if panicked || true {\n            poison_process_global_after_dispatch_terminal_v1();']],
    expected: ['assertion failed: !take_dispatch_terminal_process_gate_record_v1()'], oracle_path: facadePath},
  {name: 'clear-sticky-transport', path: q, test: facade,
    edits: [['*self.terminal_transport |= settled.transport;\n        settled.into_result()\n    }\n\n    pub fn overwrite_detached_initialized_host_visible_fixed_dispatch_data(',
      '*self.terminal_transport = settled.transport;\n        settled.into_result()\n    }\n\n    pub fn overwrite_detached_initialized_host_visible_fixed_dispatch_data(']],
    expected: ['assertion failed: *selected.terminal_transport'], oracle_path: facadePath},
  {name: 'extract-before-commit-entry', path: d, test: constructedPrefix + 'complete_stays_rooted_until_commit_ledger_access',
    edits: [[extraction, `        let memory = root
            .take_complete()
            .expect("borrowed Complete remains rooted through ledger commit");
        let ledger = context.ledger();
        insert_detached_identity_at(ledger.identities, ledger.next, identity, index);
        *ledger.count = next_count;`]],
    expected: ['insertion commit ledger access', 'Complete retains exact initialized content until extraction'],
    oracle_path: 'crates/fe2o3-kfd/src/shared_memory/tests/device_initialization.rs'},
  {name: 'append-instead-of-insert', path: d, test: constructedPrefix + 'constructed_primary_and_auxiliary_preserve_success_identity_and_order',
    edits: [['insert_detached_identity_at(ledger.identities, ledger.next, identity, index);',
      'let _ = index;\n        let end = ledger.identities.len();\n        insert_detached_identity_at(ledger.identities, ledger.next, identity, end);']],
    expected: ['assertion `left == right` failed'], oracle_path: testPath},
  {name: 'ignore-release-hole', path: d, test: constructedPrefix + 'uses_a_genuine_release_hole_and_explicit_index_overrides_it',
    edits: [['data_index.unwrap_or_else(|| ledger.next.unwrap_or(ledger.identities.len()))',
      'data_index.unwrap_or(ledger.identities.len())']],
    expected: ['assertion `left == right` failed'], oracle_path: testPath},
];
const oracleLines = [774, 800, 662, 715, 716, 92, 150, 106, 147, 588, 1210];
for (const [index, mutation] of mutations.entries()) mutation.oracle_line = oracleLines[index];
module.exports = {parent, accepted, previous, sourceDelta, newTests, tests, focused, helpers, mutations};
