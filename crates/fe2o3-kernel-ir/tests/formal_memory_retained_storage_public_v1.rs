//! External API compile check; no formal derivation authority is added.
use fe2o3_kernel_ir::{FormalMemoryObligations, LogicalStorageCounterV1, LogicalStorageErrorV1};
#[test]
fn formal_report_heap_observer_is_publicly_nameable() {
    let _: fn(
        &FormalMemoryObligations,
        &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> =
        FormalMemoryObligations::charge_retained_heap_storage_v1;
}
