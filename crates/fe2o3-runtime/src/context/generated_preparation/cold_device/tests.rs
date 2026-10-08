//! Test-only descriptive witnesses are never accepted as actual lower custody.
#![cfg(test)]
use super::*;

#[test]
fn cold_context_join_rejects_other_preparation_context_device_stream_and_hold() {
    let mut context =
        RuntimeContextV1::open(KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1()).unwrap();
    let foreign =
        RuntimeContextV1::open(KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1()).unwrap();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let streams = [
        context.create_stream(devices[0]).unwrap(),
        context.create_stream(devices[0]).unwrap(),
        context.create_stream(devices[1]).unwrap(),
    ];
    let holds = streams.map(|stream| {
        context
            .hold_unpublished_stream_with_access_v1(stream, None)
            .unwrap()
    });
    let prepared = context.bound_multi_preparation_for_test_v1(devices[0], ());
    let other_device = context.bound_multi_preparation_for_test_v1(devices[1], ());
    let other_context = foreign.bound_multi_preparation_for_test_v1(foreign.devices()[0].id(), ());
    let failure = context
        .synthetic_cold_failure_for_test_v1(&prepared, &holds[0])
        .unwrap();
    assert!(
        context
            .validate_synthetic_cold_failure_for_test_v1(&prepared, &holds[0], &failure)
            .is_ok()
    );
    assert!(
        context
            .validate_synthetic_cold_failure_for_test_v1(&prepared, &holds[1], &failure)
            .is_err()
    );
    assert!(
        context
            .validate_synthetic_cold_failure_for_test_v1(&other_device, &holds[2], &failure)
            .is_err()
    );
    assert!(
        context
            .validate_synthetic_cold_failure_for_test_v1(&other_context, &holds[0], &failure)
            .is_err()
    );
    let mut substituted_generation = context
        .synthetic_cold_failure_for_test_v1(&prepared, &holds[0])
        .unwrap();
    substituted_generation.binding.native_device = other_device.binding.native_device;
    assert!(
        context
            .validate_synthetic_cold_failure_for_test_v1(
                &prepared,
                &holds[0],
                &substituted_generation
            )
            .is_err()
    );
    context.release_unpublished_hold_v1(&holds[0]).unwrap();
    let replacement = context
        .hold_unpublished_stream_with_access_v1(streams[0], None)
        .unwrap();
    assert!(
        context
            .validate_synthetic_cold_failure_for_test_v1(&prepared, &replacement, &failure)
            .is_err()
    );
    for hold in [&replacement, &holds[1], &holds[2]] {
        context.release_unpublished_hold_v1(hold).unwrap();
    }
    assert!(context.cleanup().is_complete());
}

#[test]
fn scalar_cold_context_binding_still_requires_its_original_unpublished_hold() {
    let mut context =
        RuntimeContextV1::open(KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1()).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let hold = context
        .hold_unpublished_stream_with_access_v1(stream, None)
        .unwrap();
    let prepared = context.bound_preparation_for_test_v1(());
    let failure = context
        .synthetic_cold_failure_for_test_v1(&prepared, &hold)
        .unwrap();
    assert!(
        context
            .validate_synthetic_cold_failure_for_test_v1(&prepared, &hold, &failure)
            .is_ok()
    );
    context.release_unpublished_hold_v1(&hold).unwrap();
    assert!(
        context
            .validate_synthetic_cold_failure_for_test_v1(&prepared, &hold, &failure)
            .is_err()
    );
    assert!(context.cleanup().is_complete());
}

#[test]
fn cold_coordinates_without_actual_reset_owner_are_terminal_not_device_local_failure() {
    let mut context =
        RuntimeContextV1::open(KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1()).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let hold = context
        .hold_unpublished_stream_with_access_v1(stream, None)
        .unwrap();
    let prepared = context.bound_multi_preparation_for_test_v1(device, ());
    let failure = context
        .synthetic_cold_failure_for_test_v1(&prepared, &hold)
        .unwrap();
    assert!(matches!(
        context
            .backend
            .validate_generated_cold_failure_v1(&failure.original),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(context.has_unpublished_holds_v1());
    // The backend is now terminal. This native-free CPU fixture intentionally
    // keeps the original mocked Context instead of relaxing production Drop.
    core::mem::forget(context);
}
