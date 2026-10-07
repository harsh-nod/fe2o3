use super::*;
use fe2o3_kfd::{
    Gfx942SdmaPersistentDiagnosticSleepCeilingV1 as Ceiling, Gfx942SdmaPersistentWaitCpuV1,
    Gfx942SdmaPersistentWaitDiagnosticsV1,
};

fn diagnostic() -> Gfx942SdmaPersistentWaitDiagnosticsV1 {
    Gfx942SdmaPersistentWaitDiagnosticsV1 {
        sleep_ceiling: Ceiling::Micros25,
        packet_count: 1,
        counters: None,
        scan_ns: Some(7),
        cpu: Gfx942SdmaPersistentWaitCpuV1::Unavailable,
    }
}

#[test]
fn unprofiled_settlement_invalidates_even_a_later_complete_capture() {
    for poll in [false, true] {
        let complete = ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        };
        let mut steps = vec![
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            if poll {
                ScriptedSdmaStepV1::Poll(complete)
            } else {
                ScriptedSdmaStepV1::Wait(complete)
            },
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                0,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ];
        steps.extend(scripted_release_steps_v1());
        let (mut backend, stream, host, device) =
            scripted_direct_backend_configured_v1(8, steps, |backend| {
                backend
                    .enable_directional_sdma_wait_diagnostics_v1(Ceiling::Micros25, 1)
                    .unwrap();
            });
        let (source, destination) = scripted_copy_regions_v1(host, device, 8);
        let first = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        let outcome = if poll {
            backend.poll_v1(first)
        } else {
            backend.wait_v1(first, Instant::now())
        };
        assert_eq!(outcome.unwrap(), BackendPollV1::Succeeded);
        backend.release_submission_v1(first).unwrap();
        backend
            .scripted_sdma
            .as_mut()
            .unwrap()
            .wait_diagnostics
            .push_back(diagnostic());
        let second = backend
            .copy_async_v1(stream, source, destination, &[])
            .unwrap();
        backend.flush_stream_v1(stream).unwrap();
        assert_eq!(
            backend.wait_v1(second, Instant::now()).unwrap(),
            BackendPollV1::Succeeded
        );
        clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(second));
        assert!(
            matches!(backend.finish_directional_sdma_wait_diagnostics_v1(),
            Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch)
        );
    }
}

#[test]
fn profiled_two_window_continuation_keeps_the_explicit_flush_boundary() {
    let mut steps = Vec::new();
    for offset in [0, 8] {
        steps.extend([
            scripted_submit_step_v1(
                Gfx942PersistentSdmaDirectionV1::HostToDevice,
                offset,
                offset,
                8,
                ScriptedFailureModeV1::Success,
            ),
            ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
                direction: None,
                copy_bytes: None,
            }),
            ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
        ]);
    }
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) =
        scripted_direct_backend_configured_v1(16, steps, |backend| {
            backend
                .enable_directional_sdma_wait_diagnostics_v1(Ceiling::Micros25, 2)
                .unwrap();
        });
    backend
        .scripted_sdma
        .as_mut()
        .unwrap()
        .wait_diagnostics
        .extend([diagnostic(), diagnostic()]);
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    // Synthetic small windows exercise runtime continuation, not native 63+2 geometry.
    backend.active_sdma.get_mut(&submission).unwrap().byte_len = 16;
    assert_eq!(
        backend.wait_v1(submission, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert!(matches!(
        backend.active_sdma[&submission].phase,
        ActiveSdmaPhaseV1::Ready
    ));
    assert_eq!(backend.active_sdma[&submission].completed_bytes, 8);
    assert!(backend.published_sdma_submissions.is_empty());
    assert_eq!(backend.scripted_sdma.as_ref().unwrap().remaining_steps(), 6);
    backend.flush_stream_v1(stream).unwrap();
    assert_eq!(
        backend.wait_v1(submission, Instant::now()).unwrap(),
        BackendPollV1::Succeeded
    );
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
    let records = backend
        .finish_directional_sdma_wait_diagnostics_v1()
        .unwrap();
    assert_eq!(records.len(), 2);
    for (record, offset) in records.iter().zip([0, 8]) {
        assert_eq!(record.backend_submission, submission);
        assert_eq!(record.completed_prefix_bytes, offset);
        assert_eq!(record.host_offset, offset);
        assert_eq!(record.device_offset, offset);
    }
}

#[test]
fn profiled_timeout_preserves_custody_and_completion_is_recorded_after_retirement() {
    let mut steps = vec![
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Pending),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) =
        scripted_direct_backend_configured_v1(8, steps, |backend| {
            backend
                .enable_directional_sdma_wait_diagnostics_v1(Ceiling::Micros25, 1)
                .unwrap();
        });
    backend
        .scripted_sdma
        .as_mut()
        .unwrap()
        .wait_diagnostics
        .push_back(diagnostic());
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(
        backend.wait_v1(submission, Instant::now()).unwrap(),
        BackendPollV1::Pending
    );
    assert!(backend.published_sdma_submissions.contains(&submission));
    assert!(backend.published_sdma_index_is_consistent_v1());
    assert_eq!(
        backend
            .scripted_sdma
            .as_ref()
            .unwrap()
            .wait_diagnostics
            .len(),
        1
    );
    assert!(
        backend
            .finish_directional_sdma_wait_diagnostics_v1()
            .is_err()
    );
    assert_eq!(
        backend.wait_v1(submission, Instant::now()).unwrap(),
        BackendPollV1::Succeeded
    );
    assert!(
        backend
            .scripted_sdma
            .as_ref()
            .unwrap()
            .wait_diagnostics
            .is_empty()
    );
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
    let records = backend
        .finish_directional_sdma_wait_diagnostics_v1()
        .unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].backend_submission, submission);
    assert_eq!(records[0].window_bytes, 8);
    assert_eq!(records[0].native, diagnostic());
}

#[test]
fn profiled_failed_retirement_retains_custody_and_cannot_release_capture() {
    let steps = [
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Retryable),
    ];
    let (mut backend, stream, host, device) =
        scripted_direct_backend_configured_v1(8, steps, |backend| {
            backend
                .enable_directional_sdma_wait_diagnostics_v1(Ceiling::Micros25, 1)
                .unwrap();
        });
    backend
        .scripted_sdma
        .as_mut()
        .unwrap()
        .wait_diagnostics
        .push_back(diagnostic());
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert!(matches!(
        backend.wait_v1(submission, Instant::now()),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(
        backend
            .finish_directional_sdma_wait_diagnostics_v1()
            .is_err()
    );
    assert!(backend.terminal_sdma_custody.is_some());
    assert!(backend.published_sdma_index_is_consistent_v1());
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert!(driver.is_exhausted());
    assert_eq!(driver.live_owner_count(), 2);
    assert_eq!(driver.unexpected_drops(), 0);
    disarm_scripted_drop_after_inspection_v1(&mut backend);
}

#[test]
fn ordinary_wait_does_not_consume_diagnostic_fixture_or_enable_capture() {
    let mut steps = vec![
        scripted_submit_step_v1(
            Gfx942PersistentSdmaDirectionV1::HostToDevice,
            0,
            0,
            8,
            ScriptedFailureModeV1::Success,
        ),
        ScriptedSdmaStepV1::Wait(ScriptedExecutionOutcomeV1::Completed {
            direction: None,
            copy_bytes: None,
        }),
        ScriptedSdmaStepV1::Retire(ScriptedFailureModeV1::Success),
    ];
    steps.extend(scripted_release_steps_v1());
    let (mut backend, stream, host, device) = scripted_direct_backend_v1(8, steps);
    assert!(
        backend
            .enable_directional_sdma_wait_diagnostics_v1(Ceiling::Micros25, 1)
            .is_err()
    );
    backend
        .scripted_sdma
        .as_mut()
        .unwrap()
        .wait_diagnostics
        .push_back(diagnostic());
    let (source, destination) = scripted_copy_regions_v1(host, device, 8);
    let submission = backend
        .copy_async_v1(stream, source, destination, &[])
        .unwrap();
    assert_eq!(
        backend.wait_v1(submission, Instant::now()).unwrap(),
        BackendPollV1::Succeeded
    );
    assert_eq!(
        backend
            .scripted_sdma
            .as_ref()
            .unwrap()
            .wait_diagnostics
            .len(),
        1
    );
    clean_scripted_direct_backend_v1(&mut backend, stream, host, device, Some(submission));
    assert!(backend.directional_wait_diagnostic.is_none());
}
