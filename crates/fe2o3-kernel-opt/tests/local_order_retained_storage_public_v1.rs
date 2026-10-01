//! External compile control for every new compositional API and report name.
use fe2o3_kernel_ir::{
    InertCanonicalKirTransitionReceiptV1 as Receipt, LogicalStorageCounterV1 as Counter,
    LogicalStorageErrorV1 as Error, LogicalStorageLimitsV1 as Limits,
    VerifiedCanonicalKernelIrModuleV12 as ModuleOwner, VerifiedCanonicalKernelIrV12 as BytesOwner,
};
use fe2o3_kernel_opt::{
    OwnedU32LocalOrderContinuationV1 as Tail, OwnedU32LocalOrderRetainedStorageV1 as Report,
};
#[test]
fn public_named_report_and_cross_crate_composition_methods_are_nameable() {
    let _: fn(&Tail, Limits) -> Result<Report, Error> = Tail::retained_logical_storage_v11;
    let _: fn(&Tail, &mut Counter) -> Result<(), Error> = Tail::charge_retained_heap_v11;
    let _: fn(&ModuleOwner, &mut Counter) -> Result<(), Error> =
        ModuleOwner::charge_retained_heap_v11;
    let _: fn(&BytesOwner, &mut Counter) -> Result<(), Error> =
        BytesOwner::charge_retained_heap_storage_v1;
    let _: fn(&Receipt, &mut Counter) -> Result<(), Error> =
        Receipt::charge_retained_heap_storage_v1;
}
