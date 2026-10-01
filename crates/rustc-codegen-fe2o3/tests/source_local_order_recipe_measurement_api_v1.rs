//! External-crate visibility guard. No compiler, source file or process is run.
#![cfg(target_os = "linux")]
#![feature(rustc_private)]
use fe2o3_kernel_ir::{LogicalStorageErrorV1, LogicalStorageLimitsV1};
use rustc_codegen_fe2o3::{
    SourceLocalOrderRecipeAttemptV1 as Attempt, SourceLocalOrderRecipeRequestV1 as Request,
    SourceLocalOrderRecipeRetainedStorageV1 as Storage,
    run_source_local_order_recipe_driver_measured_v1, run_source_local_order_recipe_driver_v1,
};
use std::time::Duration;

#[test]
fn recipe_observations_are_available_to_external_callers() {
    let _: fn(&[String], Request) -> Attempt = run_source_local_order_recipe_driver_v1;
    let _: fn(&[String], Request) -> Attempt = run_source_local_order_recipe_driver_measured_v1;
    let _: fn(&Attempt) -> Option<Duration> = Attempt::callback_stage_elapsed_v1;
    let _: fn(&Attempt, LogicalStorageLimitsV1) -> Result<Storage, LogicalStorageErrorV1> =
        Attempt::retained_logical_storage_v1;
}
