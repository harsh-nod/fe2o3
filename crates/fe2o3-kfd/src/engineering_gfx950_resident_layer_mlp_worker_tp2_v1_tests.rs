use super::*;

pub(super) fn metadata(digest: [u8; 32]) -> KernelMetadataV1 {
    KernelMetadataV1 {
        symbol: MLP_WORKER.into(),
        object_sha256: digest,
        kernarg_bytes: 344,
        kernarg_alignment: 8,
        wavefront_size: 64,
        group_segment_bytes: 512,
        private_segment_bytes: 0,
        implicit_argument_offset: Some(88),
        implicit_argument_bytes: 256,
        explicit_arguments: (0..11)
            .map(|index| ExplicitArgumentV1 {
                offset: index as u32 * 8,
                bytes: 8,
                global_buffer: true,
                pointee_alignment: Some(if index >= 9 { 4 } else { 2 }),
                access: Some(mlp_worker::role_access(index)),
            })
            .collect(),
    }
}

fn run_worker(
    fault: Fault,
) -> (
    Result<Gfx950EngineeringResidentLayerMlpWorkerResultV1>,
    Vec<String>,
) {
    let f = fixture();
    let inputs = f.inputs.each_ref().map(Vec::as_slice);
    let caches = f.caches.each_ref().map(Vec::as_slice);
    let weights = [
        f.norm.as_slice(),
        f.weight.as_slice(),
        f.weight.as_slice(),
        f.weight.as_slice(),
    ];
    let events = Arc::new(Mutex::new(Vec::new()));
    let fake = Fake {
        fault,
        events: events.clone(),
        produced: false,
        first: false,
        stages: 0,
        final_done: false,
    };
    // SAFETY: mock-only transaction, no native device/queue terminals.
    let result = unsafe {
        coordinate_route::<_, FiniteWorker>(
            fake,
            vec![1],
            [1; 32],
            vec![2],
            [2; 32],
            vec![3],
            [3; 32],
            Vec::new(),
            [0; 32],
            [inputs; 2],
            [caches; 2],
            [weights; 2],
            10,
        )
    };
    let events = events.lock().unwrap().clone();
    (result, events)
}

#[test]
fn worker_has_one_real_dispatch_pair_and_publishes_only_after_group_close() {
    let (result, events) = run_worker(Fault::None);
    let result = result.unwrap();
    assert_eq!(result.producer_dispatch_ns, [11, 12]);
    assert_eq!(result.first_residual_dispatch_ns, [21, 22]);
    assert_eq!(result.mlp_dispatch_ns, [91, 92]);
    assert_eq!(result.final_residual_dispatch_ns, [81, 82]);
    for state in result.mlp_final_states {
        mlp_worker::validate_final_state(state).unwrap();
    }
    assert_eq!(
        result.ranks[0].prefix.residual_words.as_ref(),
        &[0x4040; 4096]
    );
    assert_eq!(result.ranks[0].down_partial.as_ref(), &[5.0; 4096]);
    assert_eq!(result.ranks[1].down_partial.as_ref(), &[6.0; 4096]);
    assert_eq!(
        events
            .iter()
            .filter(|e| e.as_str() == "mlp:dispatch")
            .count(),
        1
    );
    assert!(!events.iter().any(|e| e.starts_with("stage:")));
    assert_eq!(events.last().unwrap(), "close");
    let producer = events.iter().position(|e| e == "producers").unwrap();
    assert!(events[producer..].iter().all(|e| !e.starts_with("write:")));
    assert_eq!(
        events
            .iter()
            .filter(|e| e.starts_with("allocate:") && e.ends_with(":true"))
            .count(),
        4
    );
    assert!(
        events.iter().position(|e| e == "first").unwrap()
            < events.iter().position(|e| e == "mlp:dispatch").unwrap()
    );
    assert!(
        events.iter().position(|e| e == "mlp:dispatch").unwrap()
            < events.iter().position(|e| e == "final").unwrap()
    );
}

#[test]
fn finite_worker_cannot_enter_with_failed_load_metadata_or_fresh_state() {
    for fault in [
        Fault::MlpLoad,
        Fault::MlpMetadata,
        Fault::MlpStateAllocation,
        Fault::MlpFresh,
        Fault::MlpFreshBeforeDispatch,
    ] {
        let (result, events) = run_worker(fault);
        assert!(result.is_err(), "{fault:?}");
        assert!(
            !events
                .iter()
                .any(|e| e == "mlp:dispatch" || e == "final" || e == "close")
        );
        assert_eq!(events.last().unwrap(), "quarantine");
    }
}

#[test]
fn worker_dispatch_state_and_payload_failures_never_reach_final_consumers() {
    for fault in [
        Fault::MlpDispatch,
        Fault::MlpStateRead,
        Fault::MlpIncomplete,
        Fault::MlpOwner,
        Fault::MlpRead,
        Fault::ShortRead,
        Fault::Nonfinite(Kind::Norm),
        Fault::Nonfinite(Kind::Gate),
        Fault::Nonfinite(Kind::Up),
        Fault::Nonfinite(Kind::Activation),
        Fault::Nonfinite(Kind::Down),
    ] {
        let (result, events) = run_worker(fault);
        assert!(result.is_err(), "{fault:?}");
        assert!(!events.iter().any(|e| e == "final" || e == "close"));
        assert_eq!(events.last().unwrap(), "quarantine");
    }
}

#[test]
fn worker_preserves_prefix_cache_weights_and_actual_first_residual() {
    for fault in [
        Fault::State,
        Fault::Cache,
        Fault::Weight,
        Fault::FirstResidual,
        Fault::NormReplica,
        Fault::Alias,
        Fault::FirstConsumer,
    ] {
        let (result, events) = run_worker(fault);
        assert!(result.is_err(), "{fault:?}");
        assert!(!events.iter().any(|e| e == "final" || e == "close"));
        assert_eq!(events.last().unwrap(), "quarantine");
    }
}

#[test]
fn post_worker_consumer_mutation_or_close_failure_returns_no_arrays() {
    for fault in [
        Fault::FinalConsumer,
        Fault::DownWrite,
        Fault::Replica,
        Fault::MlpChangedAfterFinal,
        Fault::Close,
    ] {
        let (result, events) = run_worker(fault);
        assert!(result.is_err(), "{fault:?}");
        assert_eq!(events.last().unwrap(), "quarantine");
        assert_eq!(
            events.iter().filter(|e| e.as_str() == "close").count(),
            usize::from(fault == Fault::Close)
        );
    }
}

#[test]
fn worker_pointer_roster_uses_original_residual_and_down_without_reordering() {
    for rank in 0..2 {
        let pointers = mlp_bindings(rank);
        let mut roles = Vec::new();
        for (index, pointer) in pointers.iter().enumerate() {
            assert_eq!(pointer.rank, rank);
            assert_eq!(pointer.offset, index as u32 * 8);
            assert_eq!(pointer.role, if index == 0 { 15 } else { 15 + index });
            assert_eq!(
                pointer.extent,
                if index == 0 {
                    8192
                } else {
                    EXTENTS[index - 1] as u64
                }
            );
            assert_eq!(
                pointer.access,
                if index < 5 {
                    BufferAccessV1::Read
                } else {
                    BufferAccessV1::ReadWrite
                }
            );
            assert!(!roles.contains(&pointer.role));
            roles.push(pointer.role);
        }
        let final_ = final_bindings(rank);
        assert_eq!(pointers[0].role, final_[8].role);
        assert_eq!(pointers[9].role, final_[rank].role);
        assert_eq!(pointers[9].extent, final_[rank].extent);
        assert_eq!(final_[rank].access, BufferAccessV1::Read);
    }
}

#[test]
fn route_result_types_reject_crossed_timing_and_state_profiles_before_close() {
    let (worker, _) = run_worker(Fault::None);
    let worker = worker.unwrap();
    assert!(
        Queued::result(
            worker.ranks,
            [0; 2],
            [0; 2],
            [0; 2],
            SuffixCompletion::Worker {
                dispatch: [1; 2],
                states: worker.mlp_final_states
            }
        )
        .is_err()
    );
    let (queued, _) = run(Fault::None);
    assert!(
        FiniteWorker::result(
            queued.unwrap().ranks,
            [0; 2],
            [0; 2],
            [0; 2],
            SuffixCompletion::Queued([[1; 2]; 5])
        )
        .is_err()
    );
}
