use super::*;

#[test]
fn delayed_worker_response_cannot_extend_wait_deadline() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    assert_delayed_wait_times_out_and_reaps(DELAYED_RESPONSE_WORKER_SERVER);
}

#[test]
fn worker_wait_reserves_response_grace_inside_parent_deadline() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let command = RuntimeWorkerCommandV1::new("python3")
        .argument("-u")
        .argument("-c")
        .argument(WAIT_BUDGET_OBSERVER_WORKER_SERVER);
    let mut backend = RuntimeWorkerBackendV1::spawn(
        &command,
        RuntimeBinaryCodecV1,
        Duration::from_secs(2),
        Duration::from_secs(2),
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_millis(400);

    assert_eq!(
        backend.wait_v1(1, deadline).unwrap(),
        BackendPollV1::Pending
    );
    assert!(Instant::now() < deadline);
    assert!(!backend.is_terminal());
}

#[test]
fn decoded_terminal_failure_seals_the_worker_backend() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let command = RuntimeWorkerCommandV1::new("python3")
        .argument("-u")
        .argument("-c")
        .argument(SINGLE_RESPONSE_WORKER_SERVER);
    let mut backend = RuntimeWorkerBackendV1::spawn(
        &command,
        TestWorkerCodecV1,
        Duration::from_secs(2),
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(matches!(
        backend.enumerate_devices_v1(),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(backend.is_terminal());
    assert!(matches!(
        backend.enumerate_devices_v1(),
        Err(RuntimeBackendFailureV1::Terminal(
            RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited)
        ))
    ));
}

#[test]
fn response_shape_mismatch_seals_the_worker_backend() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let command = RuntimeWorkerCommandV1::new("python3")
        .argument("-u")
        .argument("-c")
        .argument(SINGLE_RESPONSE_WORKER_SERVER);
    let mut backend = RuntimeWorkerBackendV1::spawn(
        &command,
        MismatchedCodecV1,
        Duration::from_secs(2),
        Duration::from_secs(2),
    )
    .unwrap();
    assert!(matches!(
        backend.create_stream_v1(1),
        Err(RuntimeBackendFailureV1::Terminal(
            RuntimeWorkerBackendErrorV1::Protocol("nonzero handle")
        ))
    ));
    assert!(backend.is_terminal());
    assert!(matches!(
        backend.create_stream_v1(1),
        Err(RuntimeBackendFailureV1::Terminal(
            RuntimeWorkerBackendErrorV1::Transport(RuntimeWorkerErrorV1::WorkerExited)
        ))
    ));
}

#[test]
fn worker_backend_codec_drives_context_and_cleanup_over_child_process() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let command = RuntimeWorkerCommandV1::new("python3")
        .argument("-u")
        .argument("-c")
        .argument(TEST_WORKER_SERVER);
    let backend = RuntimeWorkerBackendV1::spawn(
        &command,
        TestWorkerCodecV1,
        Duration::from_secs(2),
        Duration::from_secs(2),
    )
    .unwrap();
    let mut context = RuntimeContextV1::open(backend).unwrap();
    let device = context.devices()[0].id();
    let stream = context.create_stream(device).unwrap();
    let allocation = context
        .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 64, 16)
        .unwrap();
    let module = context.load_module(device, b"object").unwrap();
    let kernel = context
        .resolve_kernel::<WorkerArgumentsV1>(module, "kernel")
        .unwrap();
    let mut submission = context
        .launch(
            stream,
            &kernel,
            &WorkerArgumentsV1 { allocation },
            RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0,
            },
            &[],
        )
        .unwrap();
    assert_eq!(
        context
            .wait(&mut submission, Duration::from_secs(1))
            .unwrap(),
        crate::RuntimePollV1::Succeeded
    );
    context.record_event(&submission).unwrap();
    let backend = context.shutdown().unwrap();
    backend.shutdown(Duration::from_secs(2)).unwrap();
}

#[test]
fn worker_termination_disconnects_full_response_channel() {
    assert_teardown_disconnects_full_response_channel(false, true, |mut transport| {
        transport.terminate();
        assert!(transport.is_terminal());
        assert!(transport.responses.is_none());
        assert!(transport.reader.is_none());
        assert!(transport.writer.is_none());
        transport.terminate();
    });
}

#[test]
fn worker_shutdown_disconnects_full_response_channel() {
    assert_teardown_disconnects_full_response_channel(false, true, |transport| {
        transport.shutdown(Duration::from_secs(2)).unwrap();
    });
}

#[test]
fn failed_worker_shutdown_disconnects_full_response_channel() {
    assert_teardown_disconnects_full_response_channel(false, false, |transport| {
        assert!(matches!(
            transport.shutdown(Duration::from_secs(2)),
            Err(RuntimeWorkerErrorV1::WorkerExited)
        ));
    });
}

#[test]
fn terminal_worker_shutdown_disconnects_full_response_channel() {
    assert_teardown_disconnects_full_response_channel(true, true, |transport| {
        transport.shutdown(Duration::from_secs(2)).unwrap();
    });
}

#[test]
fn worker_drop_disconnects_full_response_channel() {
    assert_teardown_disconnects_full_response_channel(false, true, drop);
}

#[test]
fn terminal_worker_drop_disconnects_full_response_channel() {
    assert_teardown_disconnects_full_response_channel(true, true, drop);
}

#[test]
fn worker_shutdown_rejects_deadline_overflow() {
    if std::process::Command::new("python3")
        .arg("--version")
        .output()
        .is_err()
    {
        return;
    }
    let command = RuntimeWorkerCommandV1::new("python3")
        .argument("-u")
        .argument("-c")
        .argument(TEST_WORKER_SERVER);
    let transport = RuntimeWorkerTransportV1::spawn(&command, Duration::from_secs(2)).unwrap();
    assert!(matches!(
        transport.shutdown(Duration::MAX),
        Err(RuntimeWorkerErrorV1::InvalidDeadline)
    ));
}
