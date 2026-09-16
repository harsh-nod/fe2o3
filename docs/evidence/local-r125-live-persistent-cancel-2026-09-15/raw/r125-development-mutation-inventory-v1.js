// Prospective production-only mutations; this module never edits or executes source.
const base = 'crates/fe2o3-kfd/src/';
const C = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::persistent_cancel_cases::';
const scopes = {
  S: [base + 'queue_live/persistent_cancel.rs', 'pub(in crate::queue) fn settle_persistent_cancel_v1(', '\nimpl PersistentCancelContextV1 for ComputeAqlQueueSessionV1 {'],
  T: [base + 'queue_live/persistent_cancel.rs', '    fn retain(&mut self, mut root: PersistentCancelRootV1) {', '\n    fn poison(&mut self, process: bool) {'],
  V: [base + 'queue_live/persistent_cancel.rs', '    fn validate_returned(&self, shape: CancelShapeV1) -> Result<(), ComputeAqlQueueSessionErrorV1> {', '\n    fn map_returned('],
  R: [base + 'queue_live/persistent_cancel.rs', '    fn restore(\n', '\n    fn cancel_preflight('],
  K: [base + 'queue_live/persistent_cancel.rs', '    fn cancel(\n', '\n}\n\npub(in crate::queue) fn settle_persistent_cancel_v1('],
  F: [base + 'queue_live/fixed_dispatch.rs', '    pub(super) fn absorb_terminal_prepared_persistent_compute_v1(', '\n    pub(super) fn absorb_terminal_published_persistent_compute_v1('],
  B: [base + 'queue_live/fixed_dispatch.rs', '    pub fn cancel_prepared_three_binding_directional_persistent_fixed_dispatch_v1(', '\n    #[allow(clippy::result_large_err)]\n    pub(super) fn poll_directional_persistent_fixed_dispatch_inner_v1'],
  L: [base + 'queue_dispatch_binding/control_release.rs', '    fn release_control_phase(', '\n    fn release_controls('],
  G: [base + 'queue_dispatch_binding.rs', '    fn persistent_after_detached(', '\n    fn reserve('],
};
const tests = {
  model: C + 'persistent_cancel_constructed_model_errors_and_panics_retain_complete_roster',
  native: C + 'persistent_cancel_constructed_native_cleanup_preserves_exact_control_prefix_and_data',
  retake: C + 'persistent_cancel_constructed_retake_error_or_panic_overrides_normal_lower_error',
  lower: C + 'persistent_cancel_constructed_lower_cleanup_retains_data_and_primary_panic',
  shape: C + 'persistent_cancel_constructed_shape_rejection_precedes_native_restore_and_ledger',
  prefix: C + 'persistent_cancel_constructed_restore_and_cancel_prefixes_retain_failed_leases',
  output: C + 'persistent_cancel_constructed_output_and_ledger_panics_keep_all_restored_owners',
  success: C + 'persistent_cancel_constructed_restores_exact_initialized_cold_and_digest_free_inputs',
  public: 'queue::live::tests::prepared_persistent_compute_cancellation_restores_initialized_rebind_input',
  generation: 'queue::dispatch_binding::control_release::tests::persistent::persistent_cancellation_inherits_detached_continuation_without_minting_recycle_history',
};
// id, function scope, ordered exact edits, selected test, decisive diagnostic coordinates.
const rows = [
  ['01-unopened-original', 'S', [['*context.dispatch() = original;', '*context.dispatch() = None;']], 'model', 'C:543:21'],
  ['02-native-retention', 'T', [['attachment.terminal_custody = Some(\n                PersistentComputeTerminalNativeCustodyV1::Cancellation(root.native),\n            );', 'attachment.terminal_custody = Some(PersistentComputeTerminalNativeCustodyV1::Attached);']], 'native', 'H:197:9'],
  ['03-single-terminal', 'F', [['        }) {\n            return true;\n        }\n        let Some(mut attachment) = self.take_single_persistent_compute_attachment_v1() else {', '        }) {\n            self.persistent_compute.as_mut().unwrap().terminal_custody =\n                Some(PersistentComputeTerminalNativeCustodyV1::Attached);\n            return true;\n        }\n        let Some(mut attachment) = self.take_single_persistent_compute_attachment_v1() else {']], 'native', 'H:204:9'],
  ['04-three-terminal', 'B', [['if attachment.terminal_custody.is_none() {', 'if true {']], 'native', 'H:204:9'],
  ['05-retake-precedence', 'S', [
    ['            retake?;\n            match lower.take() {', '            match lower.take() {'],
    ['            }\n        }\n        #[cfg(test)]\n        context.checkpoint(CancelPointV1::Returned, &mut root)?;', '            }\n            retake?;\n        }\n        #[cfg(test)]\n        context.checkpoint(CancelPointV1::Returned, &mut root)?;'],
  ], 'retake', 'C:714:17'],
  ['06-lower-panic', 'S', [['    let mut result = match lower {\n        Some(Err(payload)) => {\n            core::mem::forget(result);\n            Err(payload)\n        }\n        _ => result,\n    };', '    let mut result = result;']], 'lower', 'C:756:21'],
  ['07-returned-generation', 'V', [['        let exact = self.native.generation\n            == Some(attachment.predecessor_dispatch_generation.unwrap_or(0))', '        let exact = true']], 'shape', 'C:576:13'],
  ['08-returned-identity', 'V', [['entry.storage_identity.is_some_and(|identity| {\n                        data.sdma_storage_identity()\n                            == Gfx942SdmaBufferStorageIdentityV1::Device(identity)\n                    })', 'true']], 'shape', 'C:576:13'],
  ['09-returned-initialization', 'V', [['(shape == CancelShapeV1::Single\n                        || data.is_fully_initialized() == entry.fully_initialized)', 'true']], 'shape', 'C:576:13'],
  ['10-digest-on-cold', 'V', [['(entry.authenticated_sha256.is_none() || data.is_fully_initialized())', 'true']], 'shape', 'D:319:17'],
  ['11-failed-native-lease', 'R', [['            self.native.mapped[index] = Some(lease);\n            return Err(shape.error(\n                "persistent compute cancellation native restore",', '            drop(lease);\n            return Err(shape.error(\n                "persistent compute cancellation native restore",']], 'prefix', 'C:658:38'],
  ['12-failed-prepared-lease', 'K', [['        if !cancel_persistent_compute_prepublication_entries_v1([entry]) {\n            return Err(shape.error(', '        if !cancel_persistent_compute_prepublication_entries_v1([&mut *entry]) {\n            entry.state = PersistentComputeUseStateV1::Quarantined;\n            return Err(shape.error(']], 'prefix', 'C:668:33'],
  ['13-preserve-started-prefix', 'S', [['            if !root.native.restore_started\n                && let Some(attachment) = &mut root.attachment\n            {', '            if let Some(attachment) = &mut root.attachment {']], 'prefix', 'C:668:33'],
  ['14-output-before-ledger', 'S', [['        #[cfg(test)]\n        context.checkpoint(CancelPointV1::Output, &mut root)?;', '        {\n            let ledger = context.ledger();\n            *ledger.generation = root.native.generation;\n            *ledger.next = Some(0);\n            *ledger.count = 0;\n            ledger.identities.clear();\n        }\n        #[cfg(test)]\n        context.checkpoint(CancelPointV1::Output, &mut root)?;']], 'output', 'C:262:9'],
  ['15-detached-count', 'S', [['        *ledger.count = 0;', '        *ledger.count = 1;']], 'public', 'P:18283:9'],
  ['16-detached-identities', 'S', [
    ['        let ledger = context.ledger();\n        for (index, entry) in root', '        let first_storage_identity = root.attachment.as_ref().unwrap().entries[0].storage_identity.unwrap();\n        let ledger = context.ledger();\n        for (index, entry) in root'],
    ['        ledger.identities.clear();', '        ledger.identities.push(Gfx942FixedDispatchStorageIdentityV1::DeviceUninitialized(first_storage_identity));'],
  ], 'public', 'P:18284:9'],
  ['17-inherited-cancellation', 'L', [['            ReturningControlModeV1::PersistentBeforePublication => {\n                Some(self.generation.persistent_cancellation_generation()?)\n            }', '            ReturningControlModeV1::PersistentBeforePublication => {\n                Some(self.generation.returning_destroy_generation()?)\n            }']], 'success', 'C:484:50'],
  ['18-no-fabricated-recycle', 'G', [['        owner.predecessor_detached_generation = predecessor;', '        owner.predecessor_detached_generation = predecessor;\n        owner.recycled_generation = predecessor;']], 'generation', 'G:15:9'],
];
const oracleFiles = {
  C: base + 'queue_live/construction_auxiliary/integration_persistent_cancel_tests.rs',
  H: base + 'queue_live/rebind_tests/persistent_cancel.rs',
  D: base + 'persistent_compute.rs',
  P: base + 'queue_live.rs',
  G: base + 'queue_dispatch_binding/control_release/persistent_tests.rs',
};
const oracleHashes = {
  C: '6cab27800cdbd2277a14863f58b488389c584afb34b5a193407ba4632ad1c9a7',
  H: 'fa84de7a35647ef89e9548416f143ff577b6cfd5cd518a810f1921d009e10f40',
  D: '050a747686560fee19a04248928c86c9fb4ffab310a1f43eac18bb9114febc11',
  P: 'cd3e3bdda943dfe9a201fc44fce1f434c1019434f043ec3112a33c172550f7b5',
  G: 'b8cbc2eab5bca57f05d818f7978b6a7892fda01ffd08ca9b2ad0763a6758ece5',
};
const fragments = {
  '01': ['assertion failed: case.dispatch.is_some()'],
  '02': ['production retention lost constructed cancellation custody'],
  '03': ['assertion `left == right` failed', 'Some(Attached)', 'PersistentBeforePublication'],
  '04': ['assertion `left == right` failed', 'Some(Attached)', 'PersistentBeforePublication'],
  '05': ['assertion failed:', 'result.unwrap()', 'auxiliary-retake-complete'],
  '06': ['assertion `left == right` failed', 'left: None', 'right: Some(("control cleanup projection", ReleaseCommit))'],
  '07': ['assertion failed: result.is_err()'],
  '08': ['assertion failed: result.is_err()'],
  '09': ['assertion failed: result.is_err()'],
  '10': ['authenticated persistent input must be initialized'],
  '11': ['called `Option::unwrap()` on a `None` value'],
  '12': ['failed/suffix Prepared lease lost'],
  '13': ['failed/suffix Prepared lease lost'],
  '14': ['failure must not commit any ledger field'],
  '15': ['assertion `left == right` failed', 'left: 1', 'right: 0'],
  '16': ['assertion failed: session.detached_data_identities.is_empty()'],
  '17': ['called `Result::unwrap()` on an `Err` value', 'persistent compute cancellation returned substituted storage'],
  '18': ['assertion failed: generation.returned_generation().is_err()'],
};
module.exports = {accepted: false, source_map_sha256: '902a842fa783f96fcb96e44e03a9be07aea7302bc6ccd02949abaa1416f50833',
  scopes, tests, rows, oracleFiles, oracleHashes, fragments,
  preserved_tests: [tests.public],
  diagnostic_path_aliases: {[oracleFiles.C]: base + 'queue_live/construction_primary/../construction_auxiliary/integration_persistent_cancel_tests.rs'},
  following_panic: {'10-digest-on-cold': {location: 'C:566:37', fragments: ['called `Result::unwrap()` on an `Err` value: Any { .. }']}},
  exclusions: [
    'These are CPU fixture behavioral negatives, not native GPU execution or formal production refinement.',
    '01 establishes restoration after an unopened loan, not independent skipped-callback custody coverage.',
    '10 requires the debug output-constructor invariant followed by the outer unexpected-unwind assertion; it does not claim the borrowed preflight assertion fired.',
    'Deleting detached count/identity clears is equivalent under the admitted empty-roster invariant and is excluded; 15/16 corrupt the final postconditions instead.',
  ],
};
