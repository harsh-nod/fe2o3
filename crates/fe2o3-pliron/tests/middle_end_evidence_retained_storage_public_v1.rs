//! Public API type check, not construction of live-produced provenance.
use fe2o3_kernel_ir::{LogicalStorageCounterV1, LogicalStorageErrorV1};
use fe2o3_pliron::{InertProductionMiddleEndEvidenceV5, ProductionMiddleEndEvidenceV5};

#[test]
fn both_v5_heap_observers_are_publicly_nameable() {
    let _: fn(
        &InertProductionMiddleEndEvidenceV5,
        &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> =
        InertProductionMiddleEndEvidenceV5::charge_retained_heap_storage_v1;
    let _: fn(
        &ProductionMiddleEndEvidenceV5,
        &mut LogicalStorageCounterV1,
    ) -> Result<(), LogicalStorageErrorV1> =
        ProductionMiddleEndEvidenceV5::charge_retained_heap_storage_v1;
}
