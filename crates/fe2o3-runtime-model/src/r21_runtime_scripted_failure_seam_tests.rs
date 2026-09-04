use super::*;

const PACKET_CAP: u64 = R18_SDMA_MAX_LINEAR_COPY_BYTES_V1;

fn device() -> DeviceKeyV1 {
    DeviceKeyV1 {
        physical: PhysicalDeviceIdV1(0x211),
        generation: DeviceGenerationV1(21),
    }
}

fn vm() -> VmKeyV1 {
    VmKeyV1 {
        device: device(),
        id: VmIdV1(22),
    }
}

fn allocation() -> R18NativeAllocationKeyV1 {
    let allocation = MemoryAllocationKeyV1 {
        vm: vm(),
        id: AllocationIdV1(23),
        generation: AllocationGenerationV1(24),
    };
    R18NativeAllocationKeyV1 {
        owner: R17PersistentAllocationOwnerIdV1(25),
        allocation,
        mapping: MemoryMappingKeyV1 {
            allocation,
            id: MappingIdV1(26),
        },
    }
}

fn binding(byte_len: u64) -> R21SeamBindingV1 {
    R21SeamBindingV1 {
        allocation: allocation(),
        pair: R19DirectionalQueuePairV1 {
            parent_queue: QueueKeyV1 {
                vm: vm(),
                id: QueueInstanceIdV1(27),
                generation: QueueGenerationV1(28),
            },
            pair_occurrence: 29,
            device_to_host: R19DirectionalChildQueueV1 {
                native_queue_id: 3,
                engine_id: R18_LOCAL_SDMA_DEVICE_TO_HOST_ENGINE_V1,
            },
            host_to_device: R19DirectionalChildQueueV1 {
                native_queue_id: 4,
                engine_id: R18_LOCAL_SDMA_HOST_TO_DEVICE_ENGINE_V1,
            },
        },
        attachment_generation: 30,
        pool_generation: 31,
        logical_byte_len: byte_len,
        physical_byte_len: byte_len,
        host_storage_id: 32,
        host_storage_generation: 33,
    }
}

fn seam(byte_len: u64) -> R21RuntimeScriptedFailureSeamV1 {
    R21RuntimeScriptedFailureSeamV1::new_model_only(binding(byte_len)).unwrap()
}

fn host(byte_len: u64) -> R18HostBufferKeyV1 {
    R18HostBufferKeyV1 {
        session_id: 34,
        id: 35,
        generation: 36,
        byte_len,
        coherence: MemoryCoherenceV1::HostCoherent,
    }
}

fn request(byte_len: u64, transfer_id: u64) -> R20CopyRequestV1 {
    request_for_direction(byte_len, transfer_id, R18LocalSdmaDirectionV1::HostToDevice)
}

fn request_for_direction(
    byte_len: u64,
    transfer_id: u64,
    direction: R18LocalSdmaDirectionV1,
) -> R20CopyRequestV1 {
    let host = R20CopyEndpointV1::Host {
        buffer: host(byte_len),
        offset: 0,
    };
    let device = R20CopyEndpointV1::Device {
        allocation: allocation(),
        offset: 0,
    };
    match direction {
        R18LocalSdmaDirectionV1::HostToDevice => R20CopyRequestV1 {
            transfer_id,
            source: host,
            destination: device,
            byte_len,
        },
        R18LocalSdmaDirectionV1::DeviceToHost => R20CopyRequestV1 {
            transfer_id,
            source: device,
            destination: host,
            byte_len,
        },
    }
}

fn promote_and_begin(seam: &mut R21RuntimeScriptedFailureSeamV1, byte_len: u64) {
    assert_eq!(
        seam.promote_model_only(R21OperationDispositionV1::Succeeded),
        Ok(R21FacadeClassificationV1::Applied)
    );
    assert_eq!(
        seam.begin_model_only(request(byte_len, 41)),
        Ok(R21FacadeClassificationV1::Applied)
    );
}

fn publish(seam: &mut R21RuntimeScriptedFailureSeamV1) -> R21CompletionMetadataV1 {
    match seam
        .submit_model_only(R21SubmitDispositionV1::Published)
        .unwrap()
    {
        R21FacadeClassificationV1::Published(metadata) => metadata,
        outcome => panic!("unexpected outcome: {outcome:?}"),
    }
}

fn complete_packet(
    seam: &mut R21RuntimeScriptedFailureSeamV1,
    status: R18SdmaTerminalStatusV1,
) -> R21FacadeClassificationV1 {
    let metadata = publish(seam);
    let retirement = match seam
        .poll_model_only(R21PollDispositionV1::Terminal(status), Some(metadata))
        .unwrap()
    {
        R21FacadeClassificationV1::TerminalObserved(key) => key,
        outcome => panic!("unexpected outcome: {outcome:?}"),
    };
    let recycle = match seam
        .retire_model_only(retirement, R21OperationDispositionV1::Succeeded)
        .unwrap()
    {
        R21FacadeClassificationV1::RecyclePending(key) => key,
        outcome => panic!("unexpected outcome: {outcome:?}"),
    };
    seam.recycle_model_only(recycle, R21OperationDispositionV1::Succeeded)
        .unwrap()
}

#[test]
fn promotion_retry_is_atomic_and_success_moves_exact_authority() {
    let mut seam = seam(4096);
    let before = seam.snapshot();
    assert_eq!(
        seam.promote_model_only(R21OperationDispositionV1::Retryable),
        Ok(R21FacadeClassificationV1::Retryable)
    );
    assert_eq!(seam.snapshot(), before);
    seam.promote_model_only(R21OperationDispositionV1::Succeeded)
        .unwrap();
    let after = seam.snapshot();
    assert_eq!(after.phase, R21FacadePhaseV1::DeviceReady);
    assert_eq!(after.custody, Some(R21CustodyKindV1::Device));
    assert_eq!(after.authority_count, 1);
    assert_eq!(after.binding, before.binding);
}

#[test]
fn promotion_and_demotion_teardown_retain_one_opaque_authority() {
    for promotion in [true, false] {
        let mut seam = seam(4096);
        let point = if promotion {
            seam.promote_model_only(R21OperationDispositionV1::ProcessTeardown)
                .unwrap()
        } else {
            seam.promote_model_only(R21OperationDispositionV1::Succeeded)
                .unwrap();
            seam.demote_model_only(R21DemotionDispositionV1::ProcessTeardown)
                .unwrap()
        };
        assert!(matches!(
            point,
            R21FacadeClassificationV1::ProcessTeardown { .. }
        ));
        let snapshot = seam.snapshot();
        assert_eq!(snapshot.phase, R21FacadePhaseV1::ProcessTeardown);
        assert_eq!(snapshot.custody, Some(R21CustodyKindV1::Opaque));
        assert_eq!(snapshot.authority_count, 1);
        assert_eq!(
            seam.release_allocation_model_only(),
            Err(R21SeamErrorV1::ProcessTeardown)
        );
    }
}

#[test]
fn retryable_demotion_restores_device_and_recovered_recycle_enters_cleanup_only() {
    let mut seam = seam(4096);
    seam.promote_model_only(R21OperationDispositionV1::Succeeded)
        .unwrap();
    assert_eq!(
        seam.demote_model_only(R21DemotionDispositionV1::RetryableBeforeDemotion),
        Ok(R21FacadeClassificationV1::Retryable)
    );
    assert_eq!(seam.snapshot().phase, R21FacadePhaseV1::DeviceReady);
    assert_eq!(seam.snapshot().custody, Some(R21CustodyKindV1::Device));
    assert_eq!(
        seam.demote_model_only(R21DemotionDispositionV1::RecoveredDemotedNeedsCleanup),
        Ok(R21FacadeClassificationV1::Retryable)
    );
    let cleanup = seam.snapshot();
    assert_eq!(cleanup.phase, R21FacadePhaseV1::DemotedDeviceCleanup);
    assert_eq!(cleanup.custody, Some(R21CustodyKindV1::DemotedDevice));
    assert_eq!(
        seam.hidden_cleanup_model_only(R21OperationDispositionV1::Retryable),
        Ok(R21FacadeClassificationV1::Retryable)
    );
    assert_eq!(seam.snapshot(), cleanup);
    seam.hidden_cleanup_model_only(R21OperationDispositionV1::Succeeded)
        .unwrap();
    assert_eq!(seam.snapshot().phase, R21FacadePhaseV1::HostReady);
}

#[test]
fn hidden_cleanup_teardown_is_fail_closed() {
    let mut seam = seam(4096);
    seam.promote_model_only(R21OperationDispositionV1::Succeeded)
        .unwrap();
    seam.demote_model_only(R21DemotionDispositionV1::RecoveredDemotedNeedsCleanup)
        .unwrap();
    assert_eq!(
        seam.hidden_cleanup_model_only(R21OperationDispositionV1::ProcessTeardown),
        Ok(R21FacadeClassificationV1::ProcessTeardown {
            point: R21FailurePointV1::HiddenCleanup
        })
    );
    assert_eq!(seam.snapshot().authority_count, 1);
}

#[test]
fn unsupported_direction_is_preflight_mutation_free() {
    let mut seam = seam(4096);
    seam.promote_model_only(R21OperationDispositionV1::Succeeded)
        .unwrap();
    let host = R20CopyEndpointV1::Host {
        buffer: host(4096),
        offset: 0,
    };
    let before = seam.snapshot();
    assert_eq!(
        seam.begin_model_only(R20CopyRequestV1 {
            transfer_id: 41,
            source: host,
            destination: host,
            byte_len: 4096,
        }),
        Err(R21SeamErrorV1::InvalidRequest)
    );
    assert_eq!(seam.snapshot(), before);
}

#[test]
fn initial_retryable_submission_is_conclusive_failed_and_releasable() {
    let mut seam = seam(4096);
    promote_and_begin(&mut seam, 4096);
    let completion = R21CompletionRecordV1 {
        transfer_id: 41,
        succeeded: false,
        failure_code: Some(-1),
        completed_bytes: 0,
    };
    assert_eq!(
        seam.submit_model_only(R21SubmitDispositionV1::Retryable),
        Ok(R21FacadeClassificationV1::FailedBeforeProgress(completion))
    );
    let snapshot = seam.snapshot();
    assert_eq!(snapshot.phase, R21FacadePhaseV1::Completed);
    assert_eq!(snapshot.custody, Some(R21CustodyKindV1::Device));
    assert!(snapshot.target_retained);
    seam.release_submission_model_only(41).unwrap();
    assert_eq!(seam.snapshot().phase, R21FacadePhaseV1::DeviceReady);
}

#[test]
fn partial_retryable_submission_is_exact_quiescence_without_result() {
    let byte_len = PACKET_CAP + 1024;
    let mut seam = seam(byte_len);
    promote_and_begin(&mut seam, byte_len);
    assert_eq!(
        complete_packet(&mut seam, R18SdmaTerminalStatusV1::Succeeded),
        R21FacadeClassificationV1::ReadyContinuation {
            completed_bytes: PACKET_CAP
        }
    );
    let marker = R21QuiescentRecordV1 {
        transfer_id: 41,
        completed_bytes: PACKET_CAP,
        total_bytes: byte_len,
    };
    assert_eq!(
        seam.submit_model_only(R21SubmitDispositionV1::Retryable),
        Ok(R21FacadeClassificationV1::QuiescentWithoutResult(marker))
    );
    let snapshot = seam.snapshot();
    assert_eq!(snapshot.quiescent, Some(marker));
    assert_eq!(snapshot.dirty_through, PACKET_CAP);
    assert_eq!(snapshot.authority_count, 1);
    seam.release_submission_model_only(41).unwrap();
}

#[test]
fn partial_d2h_host_mutation_upgrades_retryable_submit_to_quiescence() {
    let byte_len = PACKET_CAP + 1024;
    let mut seam = seam(byte_len);
    seam.promote_model_only(R21OperationDispositionV1::Succeeded)
        .unwrap();
    seam.begin_model_only(request_for_direction(
        byte_len,
        41,
        R18LocalSdmaDirectionV1::DeviceToHost,
    ))
    .unwrap();
    assert_eq!(
        complete_packet(&mut seam, R18SdmaTerminalStatusV1::Succeeded),
        R21FacadeClassificationV1::ReadyContinuation {
            completed_bytes: PACKET_CAP
        }
    );
    assert_eq!(seam.snapshot().host_dirty_through, PACKET_CAP);
    assert_eq!(
        seam.submit_model_only(R21SubmitDispositionV1::Retryable),
        Ok(R21FacadeClassificationV1::QuiescentWithoutResult(
            R21QuiescentRecordV1 {
                transfer_id: 41,
                completed_bytes: PACKET_CAP,
                total_bytes: byte_len,
            }
        ))
    );
    assert_eq!(seam.snapshot().host_dirty_through, PACKET_CAP);
}

#[test]
fn pending_submission_is_observation_only() {
    let mut seam = seam(4096);
    promote_and_begin(&mut seam, 4096);
    let before = seam.snapshot();
    assert_eq!(
        seam.submit_model_only(R21SubmitDispositionV1::DependenciesPending),
        Ok(R21FacadeClassificationV1::DependencyPending)
    );
    assert_eq!(seam.snapshot(), before);
}

#[test]
fn pending_retryable_and_timeout_polls_are_observation_only() {
    for disposition in [
        R21PollDispositionV1::Pending,
        R21PollDispositionV1::Retryable,
        R21PollDispositionV1::TimedOut,
    ] {
        let mut seam = seam(4096);
        promote_and_begin(&mut seam, 4096);
        publish(&mut seam);
        let before = seam.snapshot();
        seam.poll_model_only(disposition, None).unwrap();
        assert_eq!(seam.snapshot(), before);
    }
}

#[test]
fn poll_teardown_retains_published_authority_opaquely() {
    let mut seam = seam(4096);
    promote_and_begin(&mut seam, 4096);
    publish(&mut seam);
    assert_eq!(
        seam.poll_model_only(R21PollDispositionV1::ProcessTeardown, None),
        Ok(R21FacadeClassificationV1::ProcessTeardown {
            point: R21FailurePointV1::Poll
        })
    );
    let snapshot = seam.snapshot();
    assert_eq!(snapshot.custody, Some(R21CustodyKindV1::Opaque));
    assert_eq!(snapshot.authority_count, 1);
}

#[test]
fn completion_metadata_mismatch_enters_teardown_without_dirtying() {
    let mut seam = seam(4096);
    promote_and_begin(&mut seam, 4096);
    let mut metadata = publish(&mut seam);
    metadata.pool_generation += 1;
    assert_eq!(
        seam.poll_model_only(
            R21PollDispositionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            Some(metadata)
        ),
        Ok(R21FacadeClassificationV1::ProcessTeardown {
            point: R21FailurePointV1::CompletionMetadata
        })
    );
    let snapshot = seam.snapshot();
    assert_eq!(snapshot.dirty_through, 0);
    assert_eq!(snapshot.authority_count, 1);
}

#[test]
fn retirement_retry_is_atomic_and_key_mismatch_tears_down() {
    let mut seam = seam(4096);
    promote_and_begin(&mut seam, 4096);
    let metadata = publish(&mut seam);
    let key = match seam
        .poll_model_only(
            R21PollDispositionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            Some(metadata),
        )
        .unwrap()
    {
        R21FacadeClassificationV1::TerminalObserved(key) => key,
        outcome => panic!("unexpected outcome: {outcome:?}"),
    };
    let before = seam.snapshot();
    assert_eq!(
        seam.retire_model_only(key, R21OperationDispositionV1::Retryable),
        Ok(R21FacadeClassificationV1::Retryable)
    );
    assert_eq!(seam.snapshot(), before);

    let mut wrong = key;
    wrong.pool_generation += 1;
    assert_eq!(
        seam.retire_model_only(wrong, R21OperationDispositionV1::Succeeded),
        Ok(R21FacadeClassificationV1::ProcessTeardown {
            point: R21FailurePointV1::Retirement
        })
    );
    assert_eq!(seam.snapshot().authority_count, 1);
}

#[test]
fn recycle_retry_is_atomic_and_success_advances_exact_generation() {
    let mut seam = seam(4096);
    promote_and_begin(&mut seam, 4096);
    let metadata = publish(&mut seam);
    let retirement = match seam
        .poll_model_only(
            R21PollDispositionV1::Terminal(R18SdmaTerminalStatusV1::Succeeded),
            Some(metadata),
        )
        .unwrap()
    {
        R21FacadeClassificationV1::TerminalObserved(key) => key,
        outcome => panic!("unexpected outcome: {outcome:?}"),
    };
    let recycle = match seam
        .retire_model_only(retirement, R21OperationDispositionV1::Succeeded)
        .unwrap()
    {
        R21FacadeClassificationV1::RecyclePending(key) => key,
        outcome => panic!("unexpected outcome: {outcome:?}"),
    };
    let before = seam.snapshot();
    assert_eq!(
        seam.recycle_model_only(recycle, R21OperationDispositionV1::Retryable),
        Ok(R21FacadeClassificationV1::Retryable)
    );
    assert_eq!(seam.snapshot(), before);
    assert!(matches!(
        seam.recycle_model_only(recycle, R21OperationDispositionV1::Succeeded),
        Ok(R21FacadeClassificationV1::Completed(_))
    ));
    assert_eq!(seam.snapshot().slot_generation, before.slot_generation + 1);
}

#[test]
fn failed_completion_retires_and_recycles_before_surface_completion() {
    let mut seam = seam(4096);
    promote_and_begin(&mut seam, 4096);
    assert_eq!(
        complete_packet(&mut seam, R18SdmaTerminalStatusV1::Failed { code: -7 }),
        R21FacadeClassificationV1::Completed(R21CompletionRecordV1 {
            transfer_id: 41,
            succeeded: false,
            failure_code: Some(-7),
            completed_bytes: 0,
        })
    );
    assert_eq!(seam.snapshot().dirty_through, 0);
}

#[test]
fn scripted_path_covers_release_demote_cleanup_and_allocation_release() {
    let byte_len = 4096;
    let mut seam = seam(byte_len);
    let first = seam.run_script_model_only(&[
        R21ScriptStepV1::Promote(R21OperationDispositionV1::Succeeded),
        R21ScriptStepV1::Begin(request(byte_len, 41)),
        R21ScriptStepV1::Submit(R21SubmitDispositionV1::Retryable),
        R21ScriptStepV1::ReleaseSubmission { transfer_id: 41 },
        R21ScriptStepV1::Demote(R21DemotionDispositionV1::RecoveredDemotedNeedsCleanup),
        R21ScriptStepV1::HiddenCleanup(R21OperationDispositionV1::Succeeded),
        R21ScriptStepV1::ReleaseAllocation,
    ]);
    assert_eq!(first.len(), 7);
    assert!(first.iter().all(Result::is_ok));
    let snapshot = seam.snapshot();
    assert_eq!(snapshot.phase, R21FacadePhaseV1::Released);
    assert_eq!(snapshot.custody, None);
    assert_eq!(snapshot.authority_count, 0);
}

#[test]
fn terminal_target_blocks_demotion_until_exact_release() {
    let mut seam = seam(4096);
    promote_and_begin(&mut seam, 4096);
    seam.submit_model_only(R21SubmitDispositionV1::Retryable)
        .unwrap();
    assert_eq!(
        seam.demote_model_only(R21DemotionDispositionV1::Succeeded),
        Err(R21SeamErrorV1::InvalidPhase)
    );
    assert_eq!(
        seam.release_submission_model_only(42),
        Err(R21SeamErrorV1::InvalidTransfer)
    );
    seam.release_submission_model_only(41).unwrap();
    assert_eq!(
        seam.demote_model_only(R21DemotionDispositionV1::Succeeded),
        Ok(R21FacadeClassificationV1::Applied)
    );
}
