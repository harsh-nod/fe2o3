//! Public API/type coverage without fabricating an admitted Verus execution.
use fe2o3_verifier::{
    ProductionMirPlironPerCompilationVerusExecutionV1,
    RetainedImportedFunctionalRefinementReceiptV2,
};

fn retained(owner: &RetainedImportedFunctionalRefinementReceiptV2) -> Result<(), &'static str> {
    owner.visit_retained_heap_storage_v1(|count, width| {
        count.checked_mul(width).ok_or("arithmetic").map(|_| ())
    })
}

fn execution(
    owner: &ProductionMirPlironPerCompilationVerusExecutionV1,
) -> Result<(), &'static str> {
    owner.visit_retained_heap_storage_v1(|count, width| {
        count.checked_mul(width).ok_or("arithmetic").map(|_| ())
    })
}

#[test]
fn retained_receipt_heap_api_is_publicly_nameable() {
    let _: fn(&RetainedImportedFunctionalRefinementReceiptV2) -> Result<(), &'static str> =
        retained;
}

#[test]
fn admitted_execution_heap_api_is_publicly_nameable_without_minting_one() {
    let _: fn(&ProductionMirPlironPerCompilationVerusExecutionV1) -> Result<(), &'static str> =
        execution;
}
