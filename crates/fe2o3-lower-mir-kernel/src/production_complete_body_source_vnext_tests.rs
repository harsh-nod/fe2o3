//! Pure bridge controls only. Actual rustc source files are a separate suite.
//! These tests do not counterfeit authenticated compiler/launch owners.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrWorkBudgetV1,
};
fn transport() -> CompleteBodyArgumentTransportVNext {
    CompleteBodyArgumentTransportVNext::new(16).unwrap()
}
#[test]
fn exact_symbol_intersection_and_unchanged_spelling() {
    for good in ["_", "complete_body", "A0123"] {
        assert!(complete_body_symbol_vnext(good));
    }
    assert!(complete_body_symbol_vnext(&"x".repeat(128)));
    for bad in ["", "0body", "has.dot", "has-dash", "é", "with space"] {
        assert!(!complete_body_symbol_vnext(bad));
    }
    assert!(!complete_body_symbol_vnext(&"x".repeat(129)));
    assert!(!complete_body_symbol_vnext(&"x".repeat(256)));
}
#[test]
fn local_bounds_and_reserved_parameters() {
    for size in [0, 5, COMPLETE_BODY_SOURCE_LOCAL_LIMIT + 1] {
        assert!(CompleteBodyArgumentTransportVNext::new(size).is_err());
    }
    assert!(CompleteBodyArgumentTransportVNext::new(COMPLETE_BODY_SOURCE_LOCAL_LIMIT).is_ok());
    let mut state = transport();
    for local in [0, 1, 2, 3, 4, 5, 16] {
        assert!(state.storage(local).is_err());
    }
}
#[test]
fn output_is_moved_while_scalar_copies_are_transport() {
    let mut state = transport();
    assert!(state.assign(6, 1, false).is_err());
    state.assign(6, 1, true).unwrap();
    state.assign(7, 2, false).unwrap();
    state.consume_marker([6, 7, 3, 4, 5], 1).unwrap();
    state.require_marker().unwrap();
    assert!(state.assign(8, 1, true).is_err());
    assert!(state.assign(8, 6, true).is_err());
}
#[test]
fn swapped_arguments_refuse_before_consumption() {
    let mut state = transport();
    assert!(state.consume_marker([1, 3, 2, 4, 5], 1).is_err());
    assert!(state.consume_marker([1, 2, 3, 4, 5], 0).is_err());
    assert!(state.consume_marker([1, 2, 3, 4, 5], 0x21).is_err());
    state.consume_marker([1, 2, 3, 4, 5], 1).unwrap();
    assert!(state.consume_marker([1, 2, 3, 4, 5], 1).is_err());
}
#[test]
fn self_move_and_killed_temporary_are_exact() {
    let mut state = transport();
    state.assign(6, 1, true).unwrap();
    state.assign(6, 6, true).unwrap();
    state.storage(6).unwrap();
    assert!(state.consume_marker([6, 2, 3, 4, 5], 1).is_err());
}
#[test]
fn missing_and_out_of_range_source_inputs_refuse() {
    let mut state = transport();
    assert!(state.require_marker().is_err());
    assert!(state.assign(6, 15, true).is_err());
    assert!(state.assign(6, 16, true).is_err());
    assert!(state.consume_marker([u32::MAX, 2, 3, 4, 5], 1).is_err());
}
#[test]
fn all_five_moved_arguments_cannot_be_reused() {
    let mut state = transport();
    state.consume_marker([1, 2, 3, 4, 5], 31).unwrap();
    for local in 1..=5 {
        assert!(state.assign(6, local, true).is_err());
    }
}
#[test]
fn storage_scope_restores_nonzero_floor_on_success() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    {
        let mut scope = CompleteBodyLedgerScopeVNext::new(&mut budget);
        scope.reserve(83).unwrap();
        assert!(scope.budget.storage() == 100);
    }
    assert!(budget.storage() == 17);
}
#[test]
fn storage_scope_restores_nonzero_floor_on_refusal() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    {
        let mut scope = CompleteBodyLedgerScopeVNext::new(&mut budget);
        scope.reserve(50).unwrap();
        assert!(scope.reserve(34).is_err());
    }
    assert!(budget.storage() == 17);
}
#[test]
fn storage_scope_restores_nonzero_floor_on_unwind() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 100);
    budget.reserve_storage(17).unwrap();
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut scope = CompleteBodyLedgerScopeVNext::new(&mut budget);
        scope.reserve(50).unwrap();
        panic!("test-only storage unwind");
    }));
    assert!(failure.is_err());
    assert!(budget.storage() == 17);
}
#[test]
fn complete_source_record_rejects_each_empty_axis() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticCompleteBodySourceVNext as Source;
    let axes = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]];
    assert!(Source::new(axes, [6; 32], [7; 32], [8; 32], [9; 32], [10; 32], 0).is_ok());
    for index in 0..5 {
        let mut missing = axes;
        missing[index] = [0; 32];
        assert!(Source::new(missing, [6; 32], [7; 32], [8; 32], [9; 32], [10; 32], 0).is_err());
    }
    for index in 0..5 {
        let mut rest = [[6; 32], [7; 32], [8; 32], [9; 32], [10; 32]];
        rest[index] = [0; 32];
        assert!(Source::new(axes, rest[0], rest[1], rest[2], rest[3], rest[4], 0).is_err());
    }
}

#[test]
fn identity_sorted_argument_roles_preserve_transport() {
    // Return and each argument occupy semantic positions unlike raw MIR.
    let mut state =
        CompleteBodyArgumentTransportVNext::with_roles(16, 8, [12, 3, 0, 9, 4]).unwrap();
    state.assign(6, 12, true).unwrap();
    state.consume_marker([6, 3, 0, 9, 4], 1).unwrap();
    state.require_marker().unwrap();
    for protected in [8, 12, 3, 0, 9, 4] {
        assert!(state.storage(protected).is_err());
    }
    // Numeric semantic position 1 is a temporary in this mapping.
    state.storage(1).unwrap();
}

#[test]
fn sorted_roles_never_accept_raw_coordinate_substitution() {
    let mut state =
        CompleteBodyArgumentTransportVNext::with_roles(16, 8, [12, 3, 0, 9, 4]).unwrap();
    assert!(state.consume_marker([1, 2, 3, 4, 5], 1).is_err());
    assert!(state.consume_marker([12, 0, 3, 9, 4], 1).is_err());
    state.consume_marker([12, 3, 0, 9, 4], 1).unwrap();
}

#[test]
fn duplicate_overlapping_or_absent_role_coordinates_refuse() {
    for (ret, args) in [
        (8, [12, 3, 0, 9, 9]),
        (8, [12, 3, 8, 9, 4]),
        (8, [12, 3, 0, 9, usize::MAX]),
        (16, [12, 3, 0, 9, 4]),
    ] {
        assert!(CompleteBodyArgumentTransportVNext::with_roles(16, ret, args).is_err());
    }
}
