use super::*;

#[test]
fn invalid_progress_config_retains_the_unstarted_context() {
    let (context, _state, _stream, _event, _submission) = progress_fixture();
    let invalid = RuntimeAsyncProgressConfigV1 {
        stream_capacity: 0,
        ..RuntimeAsyncProgressConfigV1::default()
    };
    let failure = match RuntimeAsyncEngineV1::spawn_with_progress(
        context,
        RuntimeAsyncEngineConfigV1::default(),
        invalid,
    ) {
        Err(failure) => failure,
        Ok(_) => panic!("invalid progress configuration must retain the context"),
    };
    assert!(matches!(
        failure.error(),
        RuntimeAsyncProgressEngineSpawnErrorV1::InvalidProgressConfig(
            RuntimeAsyncProgressConfigErrorV1::StreamCapacity
        )
    ));
    let (_context, _) = failure.into_parts();
}

#[test]
fn drain_capture_configuration_is_disabled_by_default_and_bounded() {
    let config = RuntimeAsyncEngineConfigV1::default();
    assert_eq!(config.drain_capture_byte_capacity(), 0);
    let enabled = config
        .with_drain_capture_byte_capacity(MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_BYTES_V1)
        .unwrap();
    assert_eq!(enabled.drain_capture_byte_capacity(), 64 * 1024 * 1024);
    assert_eq!(
        enabled
            .with_drain_capture_byte_capacity(0)
            .unwrap()
            .drain_capture_byte_capacity(),
        0,
    );
    assert_eq!(
        config.with_drain_capture_byte_capacity(MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_BYTES_V1 + 1),
        Err(RuntimeAsyncEngineConfigErrorV1::DrainCaptureByteCapacity),
    );
    let grouped = config
        .with_drain_capture_group_byte_capacity(MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_GROUP_BYTES_V1)
        .unwrap();
    assert_eq!(grouped.drain_capture_byte_capacity(), 128 * 1024 * 1024);
    assert_eq!(
        grouped.with_drain_capture_group_byte_capacity(
            MAX_RUNTIME_ASYNC_DRAIN_CAPTURE_GROUP_BYTES_V1 + 1
        ),
        Err(RuntimeAsyncEngineConfigErrorV1::DrainCaptureByteCapacity),
    );
    assert_eq!(
        grouped
            .with_drain_capture_group_byte_capacity(0)
            .unwrap()
            .drain_capture_byte_capacity(),
        0
    );
    assert_eq!(
        grouped
            .with_drain_capture_byte_capacity(16)
            .unwrap()
            .drain_capture_byte_capacity(),
        16
    );
}

#[test]
fn invalid_config_retains_the_unstarted_context() {
    let (context, _state, _event, _backend_submission) = fixture();
    let invalid = RuntimeAsyncEngineConfigV1 {
        command_capacity: 0,
        ..RuntimeAsyncEngineConfigV1::default()
    };
    let failure = match RuntimeAsyncEngineV1::spawn(context, invalid) {
        Err(failure) => failure,
        Ok(_) => panic!("invalid configuration must retain the context"),
    };
    assert!(matches!(
        failure.error(),
        RuntimeAsyncEngineSpawnErrorV1::InvalidConfig(
            RuntimeAsyncEngineConfigErrorV1::CommandCapacity
        )
    ));
    let (_context, _) = failure.into_parts();
}
