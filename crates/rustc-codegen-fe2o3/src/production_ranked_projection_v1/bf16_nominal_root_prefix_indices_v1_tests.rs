//! Pure component controls only; no test here constructs an actual input loan.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn actual_root_pending_owner_starts_empty_without_namespace_or_access_authority() {
    let pending = PendingActualRootPrefixIndicesV1::new();
    assert!(!pending.started && !pending.completed && pending.ledger.is_none());
    assert!(pending.prefix.entry_operations.is_empty());
    assert!(pending.prefix.reserved_reference_values.is_none());
    assert_eq!(pending.prefix.next_value, 0);
    assert!(pending.indices.indices.is_empty() && pending.indices.index_fifo.is_empty());
}
#[test]
fn actual_root_fixed_frame_prepays_owner_view_and_large_closure_result_transfers() {
    let small = assembly_frame::<(), [u8; 1]>().unwrap();
    let large = assembly_frame::<[u8; 16384], [u8; 32768]>().unwrap();
    assert!(
        small
            >= 8192
                + size_of::<PendingActualRootPrefixIndicesV1>()
                + size_of::<ActualRootPrefixIndicesV1<'static>>()
    );
    assert!(large >= small + 2 * (32768 - 1));
    assert!(large >= 2 * size_of::<Result<[u8; 16384]>>());
}
#[test]
fn actual_root_name_comparisons_charge_both_byte_lengths_before_equality() {
    for (left, right, expected) in [("", "", true), ("root", "root", true), ("α", "β", false)] {
        let exact = left.len() + right.len() + 1;
        let mut work = Work::new(exact);
        let mut budget = Budget::new(&mut work, 0);
        let mut owned = 0;
        assert_eq!(
            names_equal_v1(
                left,
                right,
                &mut PreparationResourcesV1::new(&mut budget, &mut owned)
            )
            .unwrap(),
            expected
        );
        assert_eq!(budget.work(), exact);
        assert_eq!(owned, 0);
        let mut short_work = Work::new(exact - 1);
        let mut short_budget = Budget::new(&mut short_work, 0);
        assert!(
            names_equal_v1(
                left,
                right,
                &mut PreparationResourcesV1::new(&mut short_budget, &mut owned)
            )
            .is_err()
        );
        assert!(short_budget.failed_work().is_some());
        assert_eq!(owned, 0);
    }
}
