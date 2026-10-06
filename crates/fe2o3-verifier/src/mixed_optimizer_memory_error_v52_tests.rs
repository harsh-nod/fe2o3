use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirMemorySsaErrorV1 as MemoryError, CanonicalKirMemorySsaResourceV1,
};
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;
use std::error::Error as _;

#[test]
fn refinement_memory_ssa_errors_retain_exact_analysis_diagnostics() {
    for original in [
        MemoryError::InconsistentInventory,
        MemoryError::InputLimit {
            resource: CanonicalKirMemorySsaResourceV1::Edges,
            actual: 13,
            limit: 12,
        },
    ] {
        let error = Error::from(original);
        assert!(matches!(&error, Error::MemorySsa(value) if *value == original));
        let cause = error.source().expect("exact MemorySSA cause");
        assert_eq!(cause.downcast_ref::<MemoryError>(), Some(&original));
        assert!(cause.source().is_none());
        assert!(cause.downcast_ref::<Resource>().is_none());
    }
}

#[test]
fn refinement_memory_ssa_resource_chain_preserves_exact_first_denials() {
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 0);
    let work_error = budget.charge_work(1).unwrap_err();
    assert!(matches!(work_error, Resource::Work(error)
        if error.actual() == 1 && error.limit() == 0));
    let mut storage_work = Work::new(0);
    let mut storage_budget = Budget::new(&mut storage_work, 0);
    let storage_error = storage_budget.reserve_storage(1).unwrap_err();
    for resource in [
        Resource::Allocation,
        Resource::Accounting,
        Resource::Arithmetic,
        work_error,
        storage_error,
    ] {
        let original = MemoryError::Resource(resource);
        let error = Error::from(original);
        let analysis = error.source().expect("retained MemorySSA error");
        assert_eq!(analysis.downcast_ref::<MemoryError>(), Some(&original));
        let cause = analysis.source().expect("retained resource cause");
        assert_eq!(cause.downcast_ref::<Resource>(), Some(&resource));
        let mut cursor: &(dyn std::error::Error + 'static) = &error;
        while cursor.downcast_ref::<Resource>().is_none() {
            cursor = cursor.source().expect("resource must remain discoverable");
        }
        assert_eq!(cursor.downcast_ref::<Resource>(), Some(&resource));
    }
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 0);
    assert_eq!(storage_budget.storage(), 0);
}
