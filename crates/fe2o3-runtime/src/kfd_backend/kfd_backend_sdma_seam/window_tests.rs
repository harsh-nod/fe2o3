use super::*;

fn release_scripted_directional_pair(
    ops: &mut DirectionalSdmaOpsV1<'_>,
    pair: DirectionalSdmaPairOwnerV1,
) {
    let device = match ops.demote(pair.device) {
        Ok(device) => device,
        Err(_) => panic!("scripted cleanup demotion must succeed"),
    };
    assert!(ops.recycle(pair.host).is_ok());
    assert!(ops.recycle(device).is_ok());
}

#[test]
fn synchronous_single_seam_preserves_success_retry_and_timeout_custody() {
    let direction = Gfx942PersistentSdmaDirectionV1::HostToDevice;
    let request = DirectionalSdmaCopyRequestV1 {
        host_offset: 1,
        device_offset: 2,
        copy_bytes: 4,
    };
    let submit = |outcome| ScriptedSdmaStepV1::Submit {
        direction,
        host_offset: request.host_offset,
        device_offset: request.device_offset,
        copy_bytes: request.copy_bytes,
        outcome,
    };
    let completed_step = || {
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        })
    };
    let cleanup = || {
        [
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]
    };

    let mut success_driver = ScriptedSdmaDriverV1::new(
        [submit(ScriptedFailureModeV1::Success), completed_step()]
            .into_iter()
            .chain(cleanup()),
    );
    let success_pair = DirectionalSdmaPairOwnerV1 {
        device: success_driver.test_device_owner(8),
        host: success_driver.test_host_owner(8),
    };
    {
        let mut success_ops = DirectionalSdmaOpsV1::Scripted(&mut success_driver);
        let completed = success_ops
            .execute_synchronous_single(success_pair, direction, request, Duration::from_millis(1))
            .unwrap_or_else(|_| panic!("scripted fused execution must complete"));
        assert_eq!(completed.direction(), direction);
        assert_eq!(completed.host_offset(), request.host_offset);
        assert_eq!(completed.device_offset(), request.device_offset);
        assert_eq!(completed.copy_bytes(), request.copy_bytes);
        assert_eq!(completed.packet_count(), 1);
        let pair = match success_ops.retire(completed) {
            Ok(pair) => pair,
            Err(_) => panic!("scripted fused completion must retire"),
        };
        release_scripted_directional_pair(&mut success_ops, pair);
    }
    assert!(success_driver.is_exhausted());
    assert_eq!(success_driver.live_owner_count(), 0);
    assert_eq!(success_driver.unexpected_drops(), 0);

    let mut retry_driver = ScriptedSdmaDriverV1::new(
        [submit(ScriptedFailureModeV1::Retryable)]
            .into_iter()
            .chain(cleanup().into_iter().skip(1)),
    );
    let retry_pair = DirectionalSdmaPairOwnerV1 {
        device: retry_driver.test_device_owner(8),
        host: retry_driver.test_host_owner(8),
    };
    {
        let mut retry_ops = DirectionalSdmaOpsV1::Scripted(&mut retry_driver);
        let pair = match retry_ops.execute_synchronous_single(
            retry_pair,
            direction,
            request,
            Duration::from_millis(1),
        ) {
            Err(DirectionalSdmaSynchronousExecutionFailureV1::RetryableBeforePublication {
                pair,
                ..
            }) => pair,
            _ => panic!("scripted clean rejection must return the exact pair"),
        };
        release_scripted_directional_pair(&mut retry_ops, pair);
    }
    assert!(retry_driver.is_exhausted());
    assert_eq!(retry_driver.live_owner_count(), 0);
    assert_eq!(retry_driver.unexpected_drops(), 0);

    let mut timeout_driver = ScriptedSdmaDriverV1::new(
        [
            submit(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Pending),
            completed_step(),
        ]
        .into_iter()
        .chain(cleanup()),
    );
    let timeout_pair = DirectionalSdmaPairOwnerV1 {
        device: timeout_driver.test_device_owner(8),
        host: timeout_driver.test_host_owner(8),
    };
    {
        let mut timeout_ops = DirectionalSdmaOpsV1::Scripted(&mut timeout_driver);
        let submission = match timeout_ops.execute_synchronous_single(
            timeout_pair,
            direction,
            request,
            Duration::from_millis(1),
        ) {
            Err(DirectionalSdmaSynchronousExecutionFailureV1::RetryableTimeout {
                submission,
                ..
            }) => submission,
            _ => panic!("scripted timeout must return the exact published submission"),
        };
        let completed = match timeout_ops.wait(submission, Duration::from_millis(1)) {
            Ok(DirectionalSdmaWaitV1::Completed(completed)) => completed,
            _ => panic!("returned timeout submission must remain waitable"),
        };
        let pair = match timeout_ops.retire(completed) {
            Ok(pair) => pair,
            Err(_) => panic!("completed retry must retire"),
        };
        release_scripted_directional_pair(&mut timeout_ops, pair);
    }
    assert!(timeout_driver.is_exhausted());
    assert_eq!(timeout_driver.live_owner_count(), 0);
    assert_eq!(timeout_driver.unexpected_drops(), 0);
}

#[test]
fn wait_timeout_classification_uses_only_the_typed_lower_variant() {
    assert!(is_exact_sdma_timeout_v1(
        &ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Timeout)
    ));
    assert!(!is_exact_sdma_timeout_v1(
        &ComputeAqlQueueSessionErrorV1::Contract("Timeout")
    ));
    assert!(!is_exact_sdma_timeout_v1(
        &ComputeAqlQueueSessionErrorV1::Sdma(Gfx942SdmaErrorV1::Contract("Timeout"))
    ));

    let direction = Gfx942PersistentSdmaDirectionV1::HostToDevice;
    let request = DirectionalSdmaCopyRequestV1 {
        host_offset: 0,
        device_offset: 0,
        copy_bytes: 8,
    };
    let mut driver = ScriptedSdmaDriverV1::new([
        ScriptedSdmaStepV1::Submit {
            direction,
            host_offset: 0,
            device_offset: 0,
            copy_bytes: 8,
            outcome: ScriptedFailureModeV1::Success,
        },
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Retryable),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let pair = DirectionalSdmaPairOwnerV1 {
        device: driver.test_device_owner(8),
        host: driver.test_host_owner(8),
    };
    let mut ops = DirectionalSdmaOpsV1::Scripted(&mut driver);
    let submission = ops
        .submit(
            pair,
            direction,
            DirectionalSdmaRequestPlanV1::Single(request),
        )
        .unwrap_or_else(|_| panic!("scripted publication must succeed"));
    let submission = match ops.wait(submission, Duration::ZERO) {
        Err(DirectionalSdmaExecutionFailureV1::Retryable { submission, .. }) => submission,
        _ => panic!("a non-timeout retryable result must not become typed timeout"),
    };
    let completed = match ops.wait(submission, Duration::ZERO) {
        Ok(DirectionalSdmaWaitV1::Completed(completed)) => completed,
        _ => panic!("retryable wait custody must remain waitable"),
    };
    let pair = ops
        .retire(completed)
        .unwrap_or_else(|_| panic!("completed retry must retire"));
    release_scripted_directional_pair(&mut ops, pair);
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);

    let mut synchronous_driver = ScriptedSdmaDriverV1::new([
        ScriptedSdmaStepV1::Submit {
            direction,
            host_offset: 0,
            device_offset: 0,
            copy_bytes: 8,
            outcome: ScriptedFailureModeV1::Success,
        },
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Retryable),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let pair = DirectionalSdmaPairOwnerV1 {
        device: synchronous_driver.test_device_owner(8),
        host: synchronous_driver.test_host_owner(8),
    };
    let mut ops = DirectionalSdmaOpsV1::Scripted(&mut synchronous_driver);
    let submission = match ops.execute_synchronous_single(pair, direction, request, Duration::ZERO)
    {
        Err(DirectionalSdmaSynchronousExecutionFailureV1::ProcessTeardown {
            detail,
            custody:
                SdmaTerminalCustodyV1::Scripted(ScriptedTerminalCustodyV1::Submission(submission)),
        }) if detail.to_string().contains("non-timeout retryable custody") => submission,
        _ => panic!("synchronous non-timeout retryable custody must not become timeout"),
    };
    let completed = match ops.wait(submission, Duration::ZERO) {
        Ok(DirectionalSdmaWaitV1::Completed(completed)) => completed,
        _ => panic!("synchronous non-timeout custody must remain recoverable at the seam"),
    };
    let pair = ops
        .retire(completed)
        .unwrap_or_else(|_| panic!("completed synchronous retry must retire"));
    release_scripted_directional_pair(&mut ops, pair);
    assert!(synchronous_driver.is_exhausted());
    assert_eq!(synchronous_driver.live_owner_count(), 0);
    assert_eq!(synchronous_driver.unexpected_drops(), 0);
}

#[test]
fn scripted_ready_promotion_does_not_rehash_host_bytes() {
    let source = include_str!("scripted/transitions.rs");
    let promotion = source
        .rsplit_once("fn promote_full_h2d_to_compute_ready(")
        .expect("scripted promotion method remains present")
        .1
        .split_once("fn retire_same_device(")
        .expect("scripted retirement method follows promotion")
        .0;
    assert!(!promotion.contains("Sha256::digest"));
    assert!(promotion.contains(".full_content_certificate\n"));
}

#[test]
fn ordered_window_validation_rejects_reorder_duplicates_and_noncanonical_packets() {
    let first = DirectionalSdmaCopyRequestV1 {
        host_offset: 11,
        device_offset: 29,
        copy_bytes: GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1,
    };
    let second = DirectionalSdmaCopyRequestV1 {
        host_offset: 11 + u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
        device_offset: 29 + u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
        copy_bytes: 1,
    };
    assert_eq!(
        validate_window_requests_v1(&[first, second]),
        Ok((11, 29, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1))
    );
    assert!(validate_window_requests_v1(&[second, first]).is_err());
    assert!(validate_window_requests_v1(&[first, first]).is_err());
    assert!(
        validate_window_requests_v1(&[
            DirectionalSdmaCopyRequestV1 {
                copy_bytes: 1,
                ..first
            },
            second
        ])
        .is_err()
    );

    let first = SameDeviceSdmaCopyRequestV1 {
        source_offset: 7,
        destination_offset: 41,
        copy_bytes: GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1,
    };
    let second = SameDeviceSdmaCopyRequestV1 {
        source_offset: 7 + u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
        destination_offset: 41 + u64::from(GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1),
        copy_bytes: 1,
    };
    assert_eq!(
        validate_same_device_window_requests_v1(&[first, second]),
        Ok((7, 41, GFX942_SDMA_MAX_LINEAR_COPY_BYTES_V1 + 1))
    );
    assert!(validate_same_device_window_requests_v1(&[second, first]).is_err());
    assert!(validate_same_device_window_requests_v1(&[first, first]).is_err());
    assert!(
        validate_same_device_window_requests_v1(&[
            SameDeviceSdmaCopyRequestV1 {
                copy_bytes: 1,
                ..first
            },
            second
        ])
        .is_err()
    );
}

#[test]
fn one_packet_window_rejection_returns_the_exact_pair_without_consuming_publication() {
    let request = DirectionalSdmaCopyRequestV1 {
        host_offset: 0,
        device_offset: 0,
        copy_bytes: 8,
    };
    let direction = Gfx942PersistentSdmaDirectionV1::HostToDevice;
    let mut driver = ScriptedSdmaDriverV1::new([
        ScriptedSdmaStepV1::Submit {
            direction,
            host_offset: 0,
            device_offset: 0,
            copy_bytes: 8,
            outcome: ScriptedFailureModeV1::Retryable,
        },
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let pair = DirectionalSdmaPairOwnerV1 {
        device: driver.test_device_owner(8),
        host: driver.test_host_owner(8),
    };
    let mut ops = DirectionalSdmaOpsV1::Scripted(&mut driver);
    let pair = match ops.submit(
        pair,
        direction,
        DirectionalSdmaRequestPlanV1::Window(vec![request].into_boxed_slice()),
    ) {
        Err(SdmaTransitionFailureV1::Retryable { custody, .. }) => custody,
        _ => panic!("one packet must reject window custody before publication"),
    };
    let pair = match ops.submit(
        pair,
        direction,
        DirectionalSdmaRequestPlanV1::Single(request),
    ) {
        Err(SdmaTransitionFailureV1::Retryable { custody, .. }) => custody,
        _ => panic!("the restored pair must reach the still-pending single publication"),
    };
    let Ok(device) = ops.demote(pair.device) else {
        panic!("restored device custody must demote")
    };
    assert!(ops.recycle(device).is_ok());
    assert!(ops.recycle(pair.host).is_ok());
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
}

#[test]
fn same_device_wait_timeout_returns_the_exact_pair_for_later_completion() {
    let request = SameDeviceSdmaCopyRequestV1 {
        source_offset: 0,
        destination_offset: 0,
        copy_bytes: 8,
    };
    let mut driver = ScriptedSdmaDriverV1::new([
        ScriptedSdmaStepV1::SubmitSameDeviceWindow {
            requests: vec![request],
            outcome: ScriptedFailureModeV1::Success,
        },
        ScriptedSdmaStepV1::WaitSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::WaitSameDevice(ScriptedSameDeviceExecutionOutcomeV1::Completed {
            copy_bytes: None,
            requests: None,
            swap_allocations: false,
        }),
        ScriptedSdmaStepV1::RetireSameDevice(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ]);
    let pair = SameDeviceSdmaPairOwnerV1 {
        source: driver.test_device_owner(8),
        destination: driver.test_device_owner(8),
    };
    let mut ops = DirectionalSdmaOpsV1::Scripted(&mut driver);
    let Ok(submission) = ops.submit_same_device(pair, vec![request].into_boxed_slice()) else {
        panic!("scripted same-device publication must succeed");
    };
    let submission = match ops.wait_same_device(submission, Duration::ZERO) {
        Ok(SameDeviceSdmaWaitV1::Timeout(submission)) => submission,
        _ => panic!("scripted same-device timeout must return exact pending custody"),
    };
    let completed = match ops.wait_same_device(submission, Duration::from_millis(1)) {
        Ok(SameDeviceSdmaWaitV1::Completed(completed)) => completed,
        _ => panic!("second scripted same-device wait must complete"),
    };
    let Ok(pair) = ops.retire_same_device(completed) else {
        panic!("scripted paired retirement must succeed");
    };
    for device in [pair.source, pair.destination] {
        let Ok(buffer) = ops.demote(device) else {
            panic!("scripted device demotion must succeed");
        };
        assert!(ops.recycle(buffer).is_ok());
    }
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
}
