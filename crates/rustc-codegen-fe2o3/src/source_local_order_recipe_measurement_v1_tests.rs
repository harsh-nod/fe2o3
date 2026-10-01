//! Focused arithmetic/ownership/observation controls; no genuine rustc execution.
use super::*;
use fe2o3_kernel_ir::{
    LogicalStorageCounterV1 as Counter, LogicalStorageErrorV1 as Error,
    LogicalStorageLimitsV1 as Limits,
};
use std::time::Duration;

fn limits() -> Limits {
    Limits {
        max_bytes: None,
        max_items: 16,
    }
}
fn measured(attempt: &SourceLocalOrderRecipeAttemptV1) -> SourceLocalOrderRecipeRetainedStorageV1 {
    attempt.retained_logical_storage_v1(limits()).unwrap()
}
#[test]
fn create_success_counts_one_header_and_actual_output_spare_capacities() {
    let request = request();
    let request_owned =
        request.retained_input_storage() - size_of::<SourceLocalOrderRecipeRequestV1>();
    let mut output = output();
    output.llvm.reserve_exact(911);
    let mut recipe = Vec::with_capacity(7001);
    recipe.extend_from_slice(b"inert test output");
    let llvm = output.llvm.capacity();
    let recipe_capacity = recipe.capacity();
    output.created_recipe = Some(recipe);
    let attempt = SourceLocalOrderRecipeAttemptV1::new(request, Ok(output), 1, 1);
    let storage = measured(&attempt);
    assert_eq!(
        storage.inline_bytes,
        size_of::<SourceLocalOrderRecipeAttemptV1>()
    );
    assert_eq!(storage.request_owned_bytes, request_owned);
    assert_eq!(storage.llvm_owned_bytes, llvm);
    assert_eq!(storage.created_recipe_owned_bytes, recipe_capacity);
    assert_eq!(storage.failure_diagnostic_owned_bytes, 0);
    assert_eq!(
        storage.total_bytes,
        storage.inline_bytes + request_owned + llvm + recipe_capacity
    );
    assert!(storage.llvm_owned_bytes > attempt.result().unwrap().llvm_ir().len());
    assert!(
        !attempt
            .result()
            .unwrap()
            .grants_artifact_or_launch_authority()
    );
    assert!(attempt.callback_stage_elapsed_v1().is_none());
}
#[test]
fn replay_request_bytes_are_its_own_immutable_copy_not_caller_storage() {
    let bytes = recipe();
    let request = SourceLocalOrderRecipeRequestV1::replay("src/lib.rs", [1; 32], &bytes).unwrap();
    let expected = request.retained_input_storage() - size_of::<SourceLocalOrderRecipeRequestV1>();
    let replay_pointer = request.replay_recipe_bytes().unwrap().as_ptr();
    assert_ne!(replay_pointer, bytes.as_ptr());
    let attempt = SourceLocalOrderRecipeAttemptV1::new(request, Ok(output()), 1, 1);
    let before = measured(&attempt);
    drop(bytes);
    let after = measured(&attempt);
    assert_eq!(before, after);
    assert_eq!(after.request_owned_bytes, expected);
    assert_eq!(after.created_recipe_owned_bytes, 0);
    assert_eq!(
        attempt.request().replay_recipe_bytes().unwrap().as_ptr(),
        replay_pointer
    );
}
#[test]
fn truncated_failure_retains_and_counts_original_diagnostic_capacity() {
    let mut diagnostic = String::with_capacity(32_768);
    diagnostic.push_str(&"é".repeat(3000));
    let capacity = diagnostic.capacity();
    let failure = SourceLocalOrderRecipeFailureV1::new(
        SourceLocalOrderRecipeFailurePhaseV1::Constraint,
        diagnostic,
    );
    assert!(failure.diagnostic().len() < capacity);
    let attempt = SourceLocalOrderRecipeAttemptV1::new(request(), Err(failure), 1, 1);
    let storage = measured(&attempt);
    assert_eq!(storage.failure_diagnostic_owned_bytes, capacity);
    assert_eq!(storage.llvm_owned_bytes, 0);
    assert_eq!(storage.created_recipe_owned_bytes, 0);
    assert_eq!(
        storage.total_bytes,
        storage.inline_bytes + storage.request_owned_bytes + capacity
    );
    assert_eq!(
        attempt.result().unwrap_err().phase(),
        SourceLocalOrderRecipeFailurePhaseV1::Constraint
    );
}
#[test]
fn explicit_observer_limits_refuse_without_partial_receipt_or_owner_mutation() {
    let attempt = SourceLocalOrderRecipeAttemptV1::new(request(), Ok(output()), 1, 1);
    let exact = measured(&attempt);
    assert_eq!(
        attempt
            .retained_logical_storage_v1(Limits {
                max_bytes: Some(exact.total_bytes),
                max_items: exact.visited_items,
            })
            .unwrap(),
        exact
    );
    assert_eq!(
        attempt.retained_logical_storage_v1(Limits {
            max_bytes: Some(exact.total_bytes - 1),
            max_items: exact.visited_items,
        }),
        Err(Error::ByteLimit)
    );
    assert_eq!(
        attempt.retained_logical_storage_v1(Limits {
            max_bytes: None,
            max_items: exact.visited_items - 1,
        }),
        Err(Error::ItemLimit)
    );
    assert_eq!(measured(&attempt), exact);
    let mut c = Counter::new(Limits {
        max_bytes: None,
        max_items: usize::MAX,
    });
    c.charge(usize::MAX, 1).unwrap();
    assert_eq!(
        c.charge(exact.request_owned_bytes, 1),
        Err(Error::Arithmetic)
    );
    let mut invalid = request();
    invalid.retained_storage = size_of::<SourceLocalOrderRecipeRequestV1>() - 1;
    let invalid = SourceLocalOrderRecipeAttemptV1::new(invalid, Ok(output()), 1, 1);
    assert_eq!(
        invalid.retained_logical_storage_v1(limits()),
        Err(Error::Arithmetic)
    );
}
#[test]
fn optional_clock_is_observation_only_and_requires_unique_nonfatal_callback() {
    let duration = Duration::from_nanos(123);
    for (calls, entries, fatal, expected) in [
        (1, 1, false, Some(duration)),
        (0, 0, false, None),
        (2, 1, false, None),
        (1, 2, false, None),
        (1, 1, true, None),
    ] {
        let attempt = SourceLocalOrderRecipeAttemptV1::new(request(), Ok(output()), calls, entries)
            .with_callback_stage_elapsed_v1(Some(duration), fatal);
        assert_eq!(attempt.callback_stage_elapsed_v1(), expected);
        assert_eq!(attempt.callback_count(), calls);
        assert_eq!(attempt.compiler_callback_count(), entries);
        assert_eq!(attempt.result().unwrap().llvm_ir(), "test-only inert stub");
        assert!(
            !attempt
                .result()
                .unwrap()
                .grants_artifact_or_launch_authority()
        );
    }
    let refusal = SourceLocalOrderRecipeAttemptV1::new(
        request(),
        Err(SourceLocalOrderRecipeFailureV1::new(
            SourceLocalOrderRecipeFailurePhaseV1::Constraint,
            "refused".into(),
        )),
        1,
        1,
    )
    .with_callback_stage_elapsed_v1(Some(duration), false);
    assert_eq!(refusal.callback_stage_elapsed_v1(), Some(duration));
    assert!(refusal.result().is_err());
    let zero = SourceLocalOrderRecipeAttemptV1::new(request(), Ok(output()), 1, 1)
        .with_callback_stage_elapsed_v1(Some(Duration::ZERO), false);
    assert_eq!(zero.callback_stage_elapsed_v1(), Some(Duration::ZERO));
}
#[test]
fn normal_and_measured_driver_preflight_refuse_without_clock_or_callback() {
    use crate::production_rustc_driver_v1::source_local_order_recipe_driver_v1::{
        run_source_local_order_recipe_driver_measured_v1, run_source_local_order_recipe_driver_v1,
    };
    for driver in [
        run_source_local_order_recipe_driver_v1,
        run_source_local_order_recipe_driver_measured_v1,
    ] {
        let attempt = driver(&[], request());
        assert_eq!(attempt.callback_count(), 0);
        assert_eq!(attempt.compiler_callback_count(), 0);
        assert!(attempt.callback_stage_elapsed_v1().is_none());
        assert_eq!(
            attempt.result().unwrap_err().phase(),
            SourceLocalOrderRecipeFailurePhaseV1::Request
        );
        assert!(measured(&attempt).failure_diagnostic_owned_bytes > 0);
    }
}
