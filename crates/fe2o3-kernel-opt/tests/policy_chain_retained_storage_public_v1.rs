//! Cross-crate compile control for all selected composition methods and report.
use fe2o3_kernel_ir::{
    LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error,
    LogicalStorageLimitsV1 as Limits,
};
use fe2o3_kernel_opt::{
    CheckedCanonicalKernelIrOwnerPolicy4V1 as P4, CheckedCanonicalKernelIrOwnerPolicy5V1 as P5,
    CheckedCanonicalKernelIrOwnerPolicy6V1 as P6, Policy6RetainedLogicalStorageV1 as Report,
};
use fe2o3_pliron::{
    CheckedNeutralKernelIrOwnerIntegerContinuationV1 as Integer,
    CheckedNeutralKernelIrOwnerPolicy3V1 as P3, KirBridgeOptimizedReceiptV1 as Bridge,
    PlironOptimizationReportV1 as NativeReport,
};

#[test]
fn report_and_selected_owner_methods_are_publicly_nameable() {
    let _: fn(&P6, Limits) -> Result<Report, Error> = P6::retained_logical_storage_v11;
    let _: fn(&P6, &mut Counter) -> Result<(), Error> = P6::charge_retained_heap_v11;
    let _: fn(&P5, &mut Counter) -> Result<(), Error> = P5::charge_retained_heap_v11;
    let _: fn(&P4, &mut Counter) -> Result<(), Error> = P4::charge_retained_heap_v11;
    let _: fn(&P3, &mut Counter) -> Result<(), Error> = P3::charge_retained_heap_v11;
    let _: fn(&Integer, &mut Counter) -> Result<(), Error> = Integer::charge_retained_heap_v11;
    let _: fn(&NativeReport, &mut Counter) -> Result<(), Error> =
        NativeReport::charge_retained_heap_storage_v1;
    let _: fn(&Bridge, &mut Counter) -> Result<(), Error> = Bridge::charge_retained_heap_storage_v1;
}
