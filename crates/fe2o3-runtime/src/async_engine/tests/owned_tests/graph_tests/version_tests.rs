use super::*;
use crate::{RuntimeGraphVersionSourceV1 as Source, RuntimeGraphVersionStateV1 as State};

struct Regions(Vec<RuntimeMemoryRegionV1>);
impl crate::RuntimeArgumentsV1 for Regions {
    const SIGNATURE_V1: [u8; 32] = [93; 32];
    fn encode_explicit_kernarg_v1(&self) -> Vec<u8> {
        vec![0; self.0.len() * 8]
    }
    fn bindings_v1(&self) -> Vec<RuntimeBindingV1> {
        self.0
            .iter()
            .enumerate()
            .map(|(index, &region)| RuntimeBindingV1 {
                region,
                kernarg_byte_offset: (index * 8) as u32,
            })
            .collect()
    }
}

fn region(
    allocation: crate::RuntimeAllocationIdV1,
    offset: u64,
    len: u64,
    access: RuntimeAccessV1,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        byte_offset: offset,
        byte_len: len,
        access,
    }
}

fn sequence(
    h: &mut Harness,
    regions: Vec<Vec<RuntimeMemoryRegionV1>>,
) -> RuntimeGraphRequestV1<MockBackend> {
    let stream = h
        .context
        .completion_stream_identity_v1(h.streams[0])
        .unwrap();
    let nodes = (1..=regions.len() as u32)
        .map(|n| {
            CompletionNodeV1::future(
                id(n),
                FutureIdentityV1::new(stream, [n as u8; 32]),
                (n > 1).then(|| id(n - 1)),
            )
        })
        .collect();
    let mut request = RuntimeGraphRequestV1::new(
        CompletionGraphV1::new(stream.context(), vec![stream], nodes).unwrap(),
        vec![(stream, h.streams[0])],
    )
    .unwrap();
    let kernel = Arc::new(
        h.context
            .resolve_kernel::<Regions>(h.module, "regions")
            .unwrap(),
    );
    for (index, regions) in regions.into_iter().enumerate() {
        request
            .bind_launch(
                id(index as u32 + 1),
                kernel.clone(),
                &Regions(regions),
                geometry(),
            )
            .unwrap();
    }
    request
}

#[test]
fn r65_partial_overwrite_reports_exact_segment_input_producers() {
    let mut h = Harness::new();
    let allocation = h.allocate();
    let full = region(allocation, 0, 32, RuntimeAccessV1::Read);
    let request = sequence(
        &mut h,
        vec![
            vec![RuntimeMemoryRegionV1 {
                access: RuntimeAccessV1::Write,
                ..full
            }],
            vec![region(allocation, 16, 16, RuntimeAccessV1::Write)],
            vec![full],
        ],
    );
    let future = h.submit(request);
    h.succeed();
    let report = result(future).unwrap();
    let inputs: Vec<_> = report
        .version_inputs
        .iter()
        .filter(|input| input.consumer == id(3))
        .collect();
    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0].version.byte_offset(), 0);
    assert_eq!(inputs[0].version.byte_len(), 16);
    assert_eq!(inputs[0].version.producer(), Some(id(1)));
    assert_eq!(inputs[1].version.byte_offset(), 16);
    assert_eq!(inputs[1].version.byte_len(), 16);
    assert_eq!(inputs[1].version.producer(), Some(id(2)));
    assert!(inputs.iter().all(|input| input.available_at_issue));
    assert_eq!(
        report
            .versions
            .iter()
            .filter(|version| version.current_at_terminal)
            .count(),
        2
    );
    let overwritten = report
        .versions
        .iter()
        .find(|version| version.version.producer() == Some(id(2)))
        .unwrap();
    assert_eq!(overwritten.predecessor.unwrap().producer(), Some(id(1)));
    assert_eq!(overwritten.state, State::Committed);
    assert!(
        report
            .versions
            .iter()
            .all(|version| version.version.execution() == report.execution)
    );
}

#[test]
fn r65_expected_producer_rejects_partial_and_stale_lineage_before_issue() {
    for expected in [
        Source::InitialAtAdmission,
        Source::ProducedBy(id(1)),
        Source::ProducedBy(id(2)),
        Source::ProducedBy(id(3)),
        Source::ProducedBy(id(99)),
    ] {
        let mut h = Harness::new();
        let allocation = h.allocate();
        let full = region(allocation, 0, 32, RuntimeAccessV1::Read);
        let mut request = sequence(
            &mut h,
            vec![
                vec![RuntimeMemoryRegionV1 {
                    access: RuntimeAccessV1::Write,
                    ..full
                }],
                vec![region(allocation, 16, 16, RuntimeAccessV1::Write)],
                vec![full],
            ],
        );
        request.expect_input_version(id(3), full, expected).unwrap();
        let future = h.submit(request);
        assert!(matches!(
            result(future),
            Err(RuntimeGraphErrorV1::Invalid(
                RuntimeGraphValidationErrorV1::InvalidVersionInput
            ))
        ));
        assert_eq!(h.issue_count(), 0);
        h.context
            .create_stream(h.context.devices()[0].id())
            .unwrap();
    }
}

#[test]
fn r65_readwrite_equal_aliases_consume_prior_version_before_new_output() {
    let mut h = Harness::new();
    let allocation = h.allocate();
    let read = region(allocation, 8, 16, RuntimeAccessV1::Read);
    let mut request = sequence(
        &mut h,
        vec![
            vec![
                read,
                RuntimeMemoryRegionV1 {
                    access: RuntimeAccessV1::Write,
                    ..read
                },
            ],
            vec![read],
        ],
    );
    request
        .expect_input_version(id(1), read, Source::InitialAtAdmission)
        .unwrap();
    request
        .expect_input_version(id(2), read, Source::ProducedBy(id(1)))
        .unwrap();
    let future = h.submit(request);
    h.succeed();
    let report = result(future).unwrap();
    assert_eq!(report.versions.len(), 2);
    assert_eq!(report.version_inputs.len(), 2);
    assert_eq!(report.version_inputs[0].version.producer(), None);
    assert_eq!(report.version_inputs[1].version.producer(), Some(id(1)));
    assert_eq!(
        report.versions[1].predecessor,
        Some(report.versions[0].version)
    );
    assert_eq!(report.versions[1].state, State::Committed);
}

#[test]
fn r65_copy_input_provenance_is_not_destination_storage_predecessor() {
    let mut h = Harness::new();
    let source = region(h.allocate(), 0, 8, RuntimeAccessV1::Read);
    let destination = region(h.allocate(), 0, 8, RuntimeAccessV1::Write);
    let mut request = h.request(false);
    request.bind_copy(id(1), source, destination).unwrap();
    request
        .expect_input_version(id(1), source, Source::InitialAtAdmission)
        .unwrap();
    let future = h.submit(request);
    h.succeed();
    let report = result(future).unwrap();
    assert_eq!(report.version_inputs.len(), 1);
    assert_eq!(
        report.version_inputs[0].version.allocation(),
        source.allocation
    );
    let output = report
        .versions
        .iter()
        .find(|v| v.version.producer() == Some(id(1)))
        .unwrap();
    assert_eq!(
        output.predecessor.unwrap().allocation(),
        destination.allocation
    );
    assert_ne!(
        output.predecessor.unwrap(),
        report.version_inputs[0].version
    );
}

#[test]
fn r65_failed_or_cancelled_producers_never_commit_versions() {
    for cancel in [false, true] {
        let mut h = Harness::new();
        let allocation = h.allocate();
        let rw = region(allocation, 0, 8, RuntimeAccessV1::ReadWrite);
        let request = sequence(&mut h, vec![vec![rw], vec![rw]]);
        let future = h.submit(request);
        if cancel {
            future.control().cancel_unissued();
        } else {
            h.tick(1);
            for status in h.state.lock().unwrap().statuses.values_mut() {
                *status = BackendPollV1::Failed { code: 9 };
            }
        }
        for _ in 0..16 {
            if h.graph.is_none() {
                break;
            }
            h.tick(4);
        }
        let report = result(future).unwrap();
        assert!(report.versions.iter().all(|v| v.state != State::Committed));
        assert_eq!(report.versions[0].current_at_terminal, cancel);
        assert_eq!(
            report.versions[1].state,
            if cancel {
                State::NotProduced
            } else {
                State::Failed
            }
        );
        assert_eq!(report.versions[2].state, State::NotProduced);
        assert!(!report.version_inputs[1].available_at_issue);
    }
}

#[test]
fn r65_version_reference_capacity_rejects_before_reservation_or_issue() {
    let mut h = Harness::new();
    let allocation = h
        .context
        .allocate(
            h.context.devices()[0].id(),
            RuntimeMemoryKindV1::DeviceLocal,
            256,
            8,
        )
        .unwrap();
    let full = region(allocation, 0, 256, RuntimeAccessV1::Read);
    let mut regions = vec![vec![full]; 128];
    regions.push(
        (0..128)
            .map(|offset| region(allocation, offset, 1, RuntimeAccessV1::Read))
            .collect(),
    );
    let request = sequence(&mut h, regions);
    let future = h.submit(request);
    assert!(matches!(
        result(future),
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::Capacity
        ))
    ));
    assert_eq!(h.issue_count(), 0);
    assert!(h.context.cleanup().is_complete());
}

#[test]
fn r65_versions_are_historical_and_distinct_across_mutation_and_rerun() {
    let mut h = Harness::new();
    let allocation = h.allocate();
    let write = region(allocation, 0, 8, RuntimeAccessV1::Write);
    let request = sequence(&mut h, vec![vec![write]]);
    let future = h.submit(request);
    h.succeed();
    let first = result(future).unwrap();
    let saved = first.versions.clone();
    h.context.write_allocation(allocation, 0, &[7; 8]).unwrap();
    let request = sequence(&mut h, vec![vec![write]]);
    let future = h.submit(request);
    h.succeed();
    let second = result(future).unwrap();
    assert_eq!(first.versions, saved);
    assert_ne!(first.versions[1].version, second.versions[1].version);
    assert_eq!(
        first.versions[1].version.producer(),
        second.versions[1].version.producer()
    );
}

#[test]
fn r65_expected_input_requires_exact_declared_read_region() {
    let mut h = Harness::new();
    let allocation = h.allocate();
    let read = region(allocation, 0, 16, RuntimeAccessV1::Read);
    let mut request = sequence(&mut h, vec![vec![read]]);
    request
        .expect_input_version(id(1), read, Source::InitialAtAdmission)
        .unwrap();
    assert_eq!(
        request.expect_input_version(id(1), read, Source::ProducedBy(id(2))),
        Err(RuntimeGraphValidationErrorV1::DuplicateVersionInput)
    );
    request
        .expect_input_version(
            id(1),
            RuntimeMemoryRegionV1 {
                byte_len: 8,
                ..read
            },
            Source::InitialAtAdmission,
        )
        .unwrap();
    assert!(matches!(
        result(h.submit(request)),
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::InvalidVersionInput
        ))
    ));
    assert_eq!(h.issue_count(), 0);
}

#[test]
fn r65_active_graph_drains_to_terminal_report_without_cancelling_nodes() {
    let state = Arc::new(Mutex::new(MockState {
        complete_on_flush: true,
        ..MockState::default()
    }));
    let (engine, handle) = start(state.clone(), Arc::new(Mutex::new(OwnerTrace::default())));
    let request = join_command(
        handle
            .observer()
            .enqueue_with_context(owner_request)
            .unwrap(),
    )
    .unwrap();
    let graph = handle.submit_graph(request).unwrap();
    let drain = handle.begin_drain(128).unwrap();
    let drained = join_command(drain).unwrap();
    assert_eq!(drained.outcome, RuntimeAsyncDrainOutcomeV1::Quiescent);
    let report = futures_executor::block_on(graph).unwrap().unwrap();
    assert!(
        report
            .completion
            .entries()
            .iter()
            .all(|e| e.state() == crate::completion::CompletionNodeStateV1::Succeeded)
    );
    assert_eq!(
        engine.shutdown().unwrap().disposition,
        RuntimeAsyncOwnedDispositionV1::Released
    );
}

#[test]
fn r65_reply_capacity_rejection_does_not_reserve_graph_slot() {
    let config = RuntimeAsyncEngineConfigV1::default()
        .with_reply_capacity(1)
        .unwrap();
    let (engine, handle) = start_with_config(
        Arc::new(Mutex::new(MockState::default())),
        Arc::new(Mutex::new(OwnerTrace::default())),
        config,
    );
    let request = handle.observer().try_with_context(owner_request).unwrap();
    let retained = handle.observer().enqueue_with_context(|_| ()).unwrap();
    assert!(matches!(
        handle.submit_graph(request),
        Err(RuntimeAsyncEngineCallErrorV1::ReplyCapacity)
    ));
    assert!(!handle.observer.graph_slot.load(AtomicOrdering::Acquire));
    join_command(retained).unwrap();
    engine.shutdown().unwrap();
    assert_eq!(handle.observer().reply_cells_in_use(), 0);
}
