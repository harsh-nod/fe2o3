//! Ordinary Context/backend accounting composition; synthetic storage, no native publication.

use super::*;
use crate::qualification_gfx942_vecadd_repeat_v1::{
    Gfx942VecaddRepeatQualificationArgumentsV1 as Arguments,
    admit_gfx942_vecadd_repeat_qualification_v1,
};
use crate::qualification_gfx942_vecadd_v1::{
    GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1 as ALIGNMENT,
    GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as BYTES,
};
use crate::{
    RuntimeCancellationV1, RuntimeCompletionFailureV1, RuntimeCompletionStatusV1, RuntimeContextV1,
    RuntimeErrorV1, RuntimeStreamObservationV1,
};
use fe2o3_resource_accounting::ResourceCreditUsageV1;
use std::mem::ManuallyDrop;

const STREAMS: usize = 8;
const PER_STREAM: usize = 128;
const PENDING: usize = STREAMS * PER_STREAM;
const ROUNDS: usize = 3;
const ALIASES: [usize; 3] = [0, PENDING / 2 - 1, PENDING - 1];

#[derive(Debug, PartialEq, Eq)]
struct CustodySnapshot {
    owners: VecDeque<RuntimeAllocationCustodyOwnerV1>,
    owner_counts: [usize; 2],
    sole_stream: Option<u64>,
}

#[derive(Debug, PartialEq, Eq)]
struct LedgerSnapshot {
    next_handle: u64,
    payload: ResourceCreditUsageV1,
    tables: ResourceCreditUsageV1,
    pending: HashMap<u64, (String, usize)>,
    custody: HashMap<u64, CustodySnapshot>,
    streams: HashMap<u64, VecDeque<u64>>,
    tails: HashMap<u64, u64>,
    modules: HashMap<u64, usize>,
    dependencies: HashMap<u64, usize>,
    events: HashMap<u64, usize>,
    reservations: usize,
    terminal: HashMap<u64, (u64, BackendPollV1, usize, bool)>,
    observations: Vec<RuntimeStreamObservationV1>,
}

fn snapshot(
    context: &RuntimeContextV1<KfdRuntimeBackendV1>,
    streams: &[crate::RuntimeStreamIdV1],
    payload: &ResourceCreditAccountV1,
    tables: &ResourceCreditAccountV1,
) -> LedgerSnapshot {
    let backend = context.backend();
    assert!(backend.queue.is_none() && backend.admitted_device.is_none());
    assert!(backend.active.is_none() && backend.compute_pipeline.is_empty());
    LedgerSnapshot {
        next_handle: backend.next_handle,
        payload: payload.usage(),
        tables: tables.usage(),
        pending: backend
            .pending_compute
            .iter()
            .map(|(&id, pending)| {
                (
                    id,
                    (
                        format!("{pending:?}"),
                        Arc::as_ptr(&pending.launch) as usize,
                    ),
                )
            })
            .collect(),
        custody: backend
            .allocation_custody
            .iter()
            .map(|(&id, custody)| {
                (
                    id,
                    CustodySnapshot {
                        owners: custody.owners.clone(),
                        owner_counts: custody.owner_counts,
                        sole_stream: custody.sole_stream,
                    },
                )
            })
            .collect(),
        streams: backend.pending_compute_streams.clone(),
        tails: backend.stream_submission_tails.clone(),
        modules: backend.compute_module_retain_counts.clone(),
        dependencies: backend.compute_dependency_retain_counts.clone(),
        events: backend.event_submission_retain_counts.clone(),
        reservations: backend.compute_completion_reservations,
        terminal: backend
            .submissions
            .iter()
            .map(|(&id, record)| {
                (
                    id,
                    (
                        record.stream,
                        record.status,
                        record.dependency_depth,
                        record.profile_dispatch_published,
                    ),
                )
            })
            .collect(),
        observations: streams
            .iter()
            .map(|&stream| context.query_stream(stream).unwrap())
            .collect(),
    }
}

fn assert_payload(account: &ResourceCreditAccountV1, charge: u64, records: usize) {
    let usage = account.usage();
    assert_eq!(used(account), charge * records as u64);
    assert_eq!(usage.retained_records, records);
    assert_eq!(usage.reserved_records, 0);
    assert_eq!(usage.quarantined_records, 0);
    assert!(!usage.poisoned);
}

fn qualify(record_pressure: bool) {
    let admitted = admit_gfx942_vecadd_repeat_qualification_v1().unwrap();
    let buffers = admitted.host_buffers().unwrap();
    let charge = payload_bytes(admitted.explicit_kernarg().len(), 3).unwrap();
    let payload = account(
        charge
            * if record_pressure {
                2 * PENDING
            } else {
                PENDING
            } as u64,
        if record_pressure {
            PENDING
        } else {
            2 * PENDING
        },
    );
    let tables = account(64 * 1024 * 1024, 64);
    let mut backend = scale_capacity::tests::backend(tables.clone());
    backend.launch_gate = KfdRuntimeLaunchGateV1::ExactGfx942VecaddRepeat(
        admit_gfx942_vecadd_repeat_qualification_v1().unwrap(),
    );
    backend.launch_payload_account = Some(payload.clone());
    // A failed assertion must not invoke native fail-closed Drop on a CPU fixture.
    let mut context = ManuallyDrop::new(RuntimeContextV1::open(backend).unwrap());
    let device = context.devices()[0].id();
    let module = context.load_module(device, admitted.hsaco()).unwrap();
    let kernel = context
        .resolve_kernel::<Arguments>(module, admitted.kernel_name())
        .unwrap();
    let mut streams = Vec::new();
    let mut arguments = Vec::new();
    for _ in 0..STREAMS {
        streams.push(context.create_stream(device).unwrap());
        let allocations = [buffers.left(), buffers.right(), buffers.output()].map(|bytes| {
            let allocation = context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::HostVisible,
                    BYTES as u64,
                    ALIGNMENT,
                )
                .unwrap();
            context.write_allocation(allocation, 0, bytes).unwrap();
            allocation
        });
        arguments.push(Arguments::new(allocations[0], allocations[1], allocations[2]).unwrap());
    }
    {
        let backend = context.backend_mut_for_test_v1();
        assert!(backend.queue.is_none() && backend.admitted_device.is_none());
        for record in backend.allocations.values_mut() {
            assert!(matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::Synthetic
            ));
            record.sdma_backed = true;
            record.sdma_initialized = true;
            record.sdma_shadow_dirty = true;
        }
        // Dirty synthetic shadows defer publication; no native token is constructed.
        backend.native_available = true;
    }
    let table_baseline = tables.usage();
    for _ in 0..ROUNDS {
        let competitor = payload
            .reserve(ResourceVectorV1::ZERO.with(ResourceKindV1::ControlResidentBytes, charge))
            .unwrap()
            .retain();
        let mut submissions = Vec::new();
        for index in 0..PENDING - 1 {
            let group = index / PER_STREAM;
            submissions.push(
                context
                    .launch(
                        streams[group],
                        &kernel,
                        &arguments[group],
                        admitted.geometry(),
                        &[],
                    )
                    .unwrap(),
            );
        }
        assert_payload(&payload, charge, PENDING);
        let before = snapshot(&context, &streams, &payload, &tables);
        let refusal = context.launch(
            streams[STREAMS - 1],
            &kernel,
            &arguments[STREAMS - 1],
            admitted.geometry(),
            &[],
        );
        let expected_detail = if record_pressure {
            "KFD retained launch admission: runtime resource credit error: RecordCapacity"
        } else {
            "KFD retained launch admission: runtime resource credit error: Capacity"
        };
        assert!(matches!(
            refusal,
            Err(RuntimeErrorV1::BackendRejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
                    && error.detail() == expected_detail
        ));
        assert_eq!(snapshot(&context, &streams, &payload, &tables), before);
        competitor.release_after_disposal().unwrap();
        submissions.push(
            context
                .launch(
                    streams[STREAMS - 1],
                    &kernel,
                    &arguments[STREAMS - 1],
                    admitted.geometry(),
                    &[],
                )
                .unwrap(),
        );
        assert_payload(&payload, charge, PENDING);
        let backend = context.backend();
        assert_eq!(backend.pending_compute.len(), PENDING);
        assert_eq!(backend.compute_completion_reservations, PENDING);
        assert_eq!(backend.allocation_custody.len(), STREAMS * 3);
        assert_eq!(
            backend.compute_module_retain_counts.values().sum::<usize>(),
            PENDING
        );
        assert_eq!(
            backend.compute_dependency_retain_counts.len(),
            PENDING - STREAMS
        );
        for custody in backend.allocation_custody.values() {
            assert_eq!(custody.owners.len(), PER_STREAM);
            assert_eq!(custody.owner_counts.iter().sum::<usize>(), PER_STREAM);
            assert!(
                custody
                    .owners
                    .iter()
                    .all(|owner| owner.kind == RuntimeAllocationCustodyKindV1::Compute)
            );
        }
        // Pure Context observers must neither progress native work nor refund payloads.
        let before = snapshot(&context, &streams, &payload, &tables);
        for submission in &submissions {
            assert_eq!(
                context.query_submission(submission).unwrap(),
                RuntimeCompletionStatusV1::Pending
            );
        }
        for &stream in &streams {
            assert_eq!(
                context.query_stream(stream).unwrap(),
                RuntimeStreamObservationV1 {
                    total_submissions: PER_STREAM,
                    pending: PER_STREAM,
                    ..RuntimeStreamObservationV1::default()
                }
            );
        }
        assert_eq!(snapshot(&context, &streams, &payload, &tables), before);
        let aliases: Vec<_> = ALIASES
            .iter()
            .map(|&index| {
                let id = context
                    .backend_submission_for_test_v1(&submissions[index])
                    .unwrap();
                Arc::clone(&context.backend().pending_compute[&id].launch)
            })
            .collect();
        assert_payload(&payload, charge, PENDING);
        // Interior withdrawal must preserve the unfinished FIFO prefix, not
        // turn its successor's ordering prerequisite into a terminal record.
        let before = snapshot(&context, &streams, &payload, &tables);
        for group in 0..STREAMS {
            let index = group * PER_STREAM + PER_STREAM / 2;
            assert_eq!(
                context.cancel(&mut submissions[index]).unwrap(),
                RuntimeCancellationV1::TooLate
            );
            assert_eq!(snapshot(&context, &streams, &payload, &tables), before);
        }
        let mut retained_cancelled = 0;
        for offset in 0..PENDING {
            // Permute stream tails out of global admission order while keeping
            // each stream's withdrawal in reverse FIFO order.
            let group = (3 + 5 * offset) % STREAMS;
            let index = group * PER_STREAM + PER_STREAM - 1 - offset / STREAMS;
            assert_eq!(
                context.cancel(&mut submissions[index]).unwrap(),
                RuntimeCancellationV1::Cancelled
            );
            assert_eq!(
                context.query_submission(&submissions[index]).unwrap(),
                RuntimeCompletionStatusV1::Failed(RuntimeCompletionFailureV1::Cancelled)
            );
            retained_cancelled += usize::from(ALIASES.contains(&index));
            assert_payload(&payload, charge, PENDING - offset - 1 + retained_cancelled);
            let before = payload.usage();
            assert_eq!(
                context.cancel(&mut submissions[index]).unwrap(),
                RuntimeCancellationV1::TooLate
            );
            assert_eq!(payload.usage(), before);
            assert_eq!(
                context.backend().pending_compute.len(),
                PENDING - offset - 1
            );
            assert_eq!(
                context.backend().compute_completion_reservations,
                PENDING - offset - 1
            );
        }
        let backend = context.backend();
        assert!(backend.allocation_custody.is_empty());
        assert!(backend.pending_compute_streams.is_empty());
        assert!(backend.compute_module_retain_counts.is_empty());
        assert!(backend.compute_dependency_retain_counts.is_empty());
        assert_eq!(tables.usage(), table_baseline);
        assert_payload(&payload, charge, ALIASES.len());
        for &stream in &streams {
            assert_eq!(
                context.query_stream(stream).unwrap(),
                RuntimeStreamObservationV1 {
                    total_submissions: PER_STREAM,
                    failed: PER_STREAM,
                    first_failure: Some(RuntimeCompletionFailureV1::Cancelled),
                    ..RuntimeStreamObservationV1::default()
                }
            );
        }
        for submission in submissions.into_iter().rev() {
            context.release_submission(submission).unwrap();
        }
        assert!(context.backend().submissions.is_empty());
        assert!(context.backend().stream_submission_tails.is_empty());
        for &stream in &streams {
            assert_eq!(
                context.query_stream(stream).unwrap(),
                RuntimeStreamObservationV1::default()
            );
        }
        assert_payload(&payload, charge, ALIASES.len());
        for (index, alias) in aliases.into_iter().enumerate() {
            drop(alias);
            assert_payload(&payload, charge, ALIASES.len() - index - 1);
        }
    }
    {
        let backend = context.backend_mut_for_test_v1();
        assert!(backend.queue.is_none() && backend.active.is_none());
        backend.native_available = false;
        for record in backend.allocations.values_mut() {
            assert!(matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::Synthetic
            ));
            record.sdma_backed = false;
            record.sdma_initialized = false;
            record.sdma_shadow_dirty = false;
        }
    }
    drop(ManuallyDrop::into_inner(context).shutdown().unwrap());
    assert_payload(&payload, charge, 0);
    assert_eq!(used(&tables), 0);
    assert_eq!(tables.usage().retained_records, 0);
    assert_eq!(tables.usage().quarantined_records, 0);
}

#[test]
fn retained_launch_context_scale_1024_byte_pressure_cancellation_and_reuse() {
    qualify(false);
}

#[test]
fn retained_launch_context_scale_1024_record_pressure_cancellation_and_reuse() {
    qualify(true);
}
