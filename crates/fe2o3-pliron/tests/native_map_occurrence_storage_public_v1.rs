//! Compile-time public API control; it does not execute any native optimizer.
use fe2o3_kernel_ir::{LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error};
use fe2o3_pliron::{
    KirNeutralOccurrenceRowsV1, KirOptimizationMapIntegerContinuationV12,
    KirOptimizationMapPolicy3V12, KirOptimizationMapV12,
};
#[test]
fn all_four_heap_only_methods_are_externally_nameable() {
    let _: fn(&KirOptimizationMapV12, &mut Counter) -> Result<(), Error> =
        KirOptimizationMapV12::charge_retained_heap_storage_v1;
    let _: fn(&KirOptimizationMapPolicy3V12, &mut Counter) -> Result<(), Error> =
        KirOptimizationMapPolicy3V12::charge_retained_heap_storage_v1;
    let _: fn(&KirOptimizationMapIntegerContinuationV12, &mut Counter) -> Result<(), Error> =
        KirOptimizationMapIntegerContinuationV12::charge_retained_heap_storage_v1;
    let _: fn(&KirNeutralOccurrenceRowsV1, &mut Counter) -> Result<(), Error> =
        KirNeutralOccurrenceRowsV1::charge_retained_heap_storage_v1;
}
