// Prospective read-only inventory; no mutation has been executed by this module.
const base = 'crates/fe2o3-kfd/src/';
const C = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::recycled_detach_cases::';
const P = 'queue::live::rebind_tests::recycled_detach::';
const scopes = {
  S: [base + 'queue_live/recycled_detach.rs', 'pub(in crate::queue) fn settle_recycled_detach_v1(', '\nimpl RecycledDetachContextV1 for ComputeAqlQueueSessionV1 {'],
  B: [base + 'queue_dispatch_binding.rs', '    pub(super) fn recycled_data_shape_v1(', '\n    pub(super) fn ensure_returnable_for_destroy('],
  D: [base + 'queue_live/fixed_dispatch.rs', '    pub(super) fn detach_recycled_fixed_dispatch_inner(', '\n    /// Binds a new fixed batch'],
  F: [base + 'queue_live.rs', "impl ComputeAqlQueueLaneDispatchV1<'_> {", '\n    /// Returns complete data from a strictly pristine recipe'],
  L: [base + 'queue_live/recycled_detach.rs', "impl RecycledDetachLedgerV1<'_> {", '\npub(in crate::queue) trait RecycledDetachContextV1 {'],
};
const tests = {
  opening: C + 'recycled_detach_opening_rejection_restores_exact_owner_without_retake_or_poison',
  reserves: C + 'recycled_detach_both_output_reserves_reject_before_currentness_or_ownership_transfer',
  retake: C + 'recycled_detach_closing_error_precedes_lower_error_but_not_original_lower_panic',
  completed: C + 'recycled_detach_completed_returned_authorities_survive_every_failed_retake',
  panic: C + 'recycled_detach_original_panic_survives_envelope_and_final_poison_panics',
  converted: C + 'recycled_detach_post_conversion_panic_keeps_all_data_without_partial_ledger_commit',
  shape: C + 'recycled_detach_malformed_shape_is_rejected_before_currentness_and_loan',
  currentness: C + 'recycled_detach_currentness_failure_preserves_unconsumed_dispatch',
  direct: P + 'recycled_detach_direct_missing_engine_transports_parent_before_error',
  sticky: P + 'recycled_detach_facade_restores_selected_owner_and_keeps_transport_sticky',
  ledger: C + 'recycled_detach_each_nonempty_ledger_field_precedes_completion_and_loan',
};
// id, scope, original, replacement, selected test key, diagnostic file/line/column.
const rows = [
  ['01-opening-terminal', 'S', 'terminal_on_error = entered;\n                return Err(error);', 'terminal_on_error = true;\n                return Err(error);', 'opening', 'C:500:9'],
  ['02-original-restore', 'S', '*context.dispatch() = Some(dispatch);', 'core::mem::forget(dispatch);', 'opening', 'C:507:9'],
  ['03-reserve-terminal', 'S', 'terminal_on_error = false;\n        let capacities = (count, count);', 'terminal_on_error = true;\n        let capacities = (count, count);', 'reserves', 'C:528:13'],
  ['04-reserve-order', 'S', 'root.reserve_output(count, capacities)?;\n        terminal_on_error = true;\n        context.check_currentness()?;', 'context.check_currentness()?;\n        root.reserve_output(count, capacities)?;\n        terminal_on_error = true;', 'reserves', 'C:538:13'],
  ['05-retake-error', 'S', 'retake?;', 'let _ = retake;', 'retake', 'C:575:14'],
  ['06-early-ledger', 'S', 'retake?;', '*context.ledger().count = count;\n        retake?;', 'completed', 'C:376:13'],
  ['07-lower-panic', 'S', 'core::mem::forget(result);\n            Err(payload)', 'core::mem::forget(payload);\n            result', 'panic', 'C:763:9'],
  ['08-final-poison', 'S', 'if result.is_err() {\n                core::mem::forget(payload);', 'if result.is_err() {\n                result = Err(payload);', 'panic', 'C:763:9'],
  ['09-converted-data', 'S', 'context.retain(root);', 'core::mem::forget(core::mem::take(&mut root.data));\n        context.retain(root);', 'converted', 'C:746:9'],
  ['10-zero-generation', 'S', 'if generation == 0 {', 'if false && generation == 0 {', 'shape', 'C:788:13'],
  ['11-borrowed-generation', 'B', 'let generation = self.ensure_returnable()?;', 'let generation = self.ensure_returnable().unwrap_or(1);', 'shape', 'C:813:13'],
  ['12-borrowed-cardinality', 'B', 'if self.data.len() != self.data_premises.len() {', 'if false && self.data.len() != self.data_premises.len() {', 'shape', 'C:813:13'],
  ['13-currentness', 'S', 'context.check_currentness()?;', 'context.check_currentness().unwrap_or(());', 'currentness', 'C:842:17'],
  ['14-direct-transport', 'D', 'if settled.transport {', 'if false && settled.transport {', 'direct', 'R:190:5'],
  ['15-sticky-transport', 'F', '*self.terminal_transport |= settled.transport;', '*self.terminal_transport = settled.transport;', 'sticky', 'P:93:25'],
  ['16-ledger-generation', 'L', 'self.generation.is_none()', 'true', 'ledger', 'C:898:13'],
  ['17-ledger-count', 'L', '&& *self.count == 0', '&& true', 'ledger', 'C:898:13'],
  ['18-ledger-identities', 'L', '&& self.identities.is_empty()', '&& true', 'ledger', 'C:898:13'],
  ['19-ledger-next', 'L', '&& self.next.is_none()', '&& true', 'ledger', 'C:898:13'],
];
const oracleFiles = {
  C: base + 'queue_live/construction_auxiliary/integration_recycled_detach_tests.rs',
  P: base + 'queue_live/rebind_tests/recycled_detach.rs',
  R: base + 'queue_live/rebind_tests.rs',
};
const oracleHashes = {
  C: '4f08cb4bdb2fcc76719fa94a32ccbc0e76d450e9b066671906f58dacad02117b',
  P: '1f4b4c56a31a545f6571fe8bc941cb37cab15b38c0d53f1af5419c34b13c5dd4',
  R: 'a4bbde5fa1af01ee662aba7696b9dbcc5b836e3e1f53561ec47a2e45e0be5973',
};
const fragments = {
  '01': ['assertion failed: !result.transport'],
  '02': ['opening rejection restores dispatch custody'],
  '03': ['assertion failed: !result.transport'],
  '04': ['assertion `left == right` failed', 'left: Snapshot {', 'right: Snapshot {', 'currentness:'],
  '05': ['unexpected recycled-detach retake provenance'],
  '06': ['failed settlement cannot alter any ledger field or backing', 'left: (None, 5, []', 'right: (None, 0, []'],
  '07': ['assertion `left == right` failed', 'left: None', 'right: Some(("N2 native panic", "free"))'],
  '08': ['assertion `left == right` failed', 'left: None', 'right: Some(("N2 native panic", "free"))'],
  '09': ['assertion `left == right` failed', 'left: []', 'right: [(Device('],
  '10': ['assertion failed: result.transport'],
  '11': ['borrowed shape rejection preserves original dispatch custody'],
  '12': ['borrowed shape rejection preserves original dispatch custody'],
  '13': ['assertion failed: matches!(result.result,', 'ComputeAqlQueueSessionErrorV1::Memory', 'MemorySessionError::Injected("currentness")'],
  '14': ['assertion failed: session.completion_owner.0.is_none()'],
  '15': ['assertion failed: *selected.terminal_transport'],
  '16': ['assertion failed: result.transport'],
  '17': ['assertion failed: result.transport'],
  '18': ['assertion failed: result.transport'],
  '19': ['assertion failed: result.transport'],
};
module.exports = {accepted: false, source_map_sha256: 'a633edf685c4cbe3a24b1500a6a086ba36d39bec9b042abb5f88c52e4024cc24',
  scopes, tests, rows, oracleFiles, oracleHashes, fragments,
  diagnostic_path_aliases: {[oracleFiles.C]: base + 'queue_live/construction_primary/../construction_auxiliary/integration_recycled_detach_tests.rs'},
  following_panic: {'15-sticky-transport': {location: 'P:127:35', fragments: ['called `Result::unwrap()` on an `Err` value: Any { .. }']}},
  excluded_source_only_guards: ['Outer completeness and returned-shape checks are additionally protected by unchanged lower guarantees; isolated deletion has no decisive reachable behavioral negative.'],
};
