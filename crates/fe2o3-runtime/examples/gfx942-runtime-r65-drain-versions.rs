//! Repeated copy-diamond lineage followed by idle drain. No compute or overlap claim.
use fe2o3_runtime::completion::*;
use fe2o3_runtime::*;
use std::error::Error;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::thread;
use std::time::{Duration, Instant};

const BODY: usize = 1024 * 1024;
const PAD: usize = 128;
const TOTAL: usize = BODY + 2 * PAD;
const EXECUTIONS: usize = 2;
const DRAIN_TICKS: usize = 128;
type ResultV1<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[derive(Debug)]
struct CopyOnly;
// SAFETY: this authorizer cannot admit any executable request.
unsafe impl KfdRuntimeLaunchAuthorityV1 for CopyOnly {
    fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        false
    }
}

struct ThreadWake(thread::Thread);
impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

fn wait<F: Future>(future: F) -> ResultV1<F::Output> {
    let mut future = Box::pin(future);
    let waker = Waker::from(Arc::new(ThreadWake(thread::current())));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        if let Poll::Ready(result) = future.as_mut().poll(&mut Context::from_waker(&waker)) {
            return Ok(result);
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err("R65 qualification deadline expired".into());
        }
        thread::park_timeout(remaining);
    }
}

fn node(n: u32) -> CompletionNodeIdV1 {
    CompletionNodeIdV1::new(n).unwrap()
}

fn pattern(i: usize, right: bool) -> u8 {
    ((i * if right { 43 } else { 29 } + i / 257 + usize::from(right) * 71) % 251) as u8
}

fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    offset: usize,
    len: usize,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: offset as u64,
        byte_len: len as u64,
    }
}

fn request(
    streams: &[RuntimeStreamIdV1],
    identities: &[StreamIdentityV1],
    allocations: &[RuntimeAllocationIdV1],
) -> ResultV1<(
    CompletionGraphIdentityV1,
    RuntimeGraphRequestV1<KfdRuntimeBackendV1>,
)> {
    let s = identities;
    let root = EventIdentityV1::new(s[0].context(), [1; 32]);
    let left = EventIdentityV1::new(s[0].context(), [2; 32]);
    let right = EventIdentityV1::new(s[0].context(), [3; 32]);
    let future = |n, stream, predecessor: Option<u32>| {
        CompletionNodeV1::future(
            node(n),
            FutureIdentityV1::new(stream, [n as u8; 32]),
            predecessor.map(node),
        )
    };
    let graph = CompletionGraphV1::new(
        s[0].context(),
        s.to_vec(),
        vec![
            future(1, s[0], None),
            CompletionNodeV1::record_event(node(2), s[0], root, Some(node(1))),
            CompletionNodeV1::wait_event(node(3), s[1], root, node(2), None),
            future(4, s[1], Some(3)),
            CompletionNodeV1::record_event(node(5), s[1], left, Some(node(4))),
            CompletionNodeV1::wait_event(node(6), s[2], root, node(2), None),
            future(7, s[2], Some(6)),
            CompletionNodeV1::record_event(node(8), s[2], right, Some(node(7))),
            CompletionNodeV1::wait_event(node(9), s[3], left, node(5), None),
            CompletionNodeV1::wait_event(node(10), s[3], right, node(8), Some(node(9))),
            future(11, s[3], Some(10)),
            future(12, s[3], Some(11)),
        ],
    )?;
    let identity = graph.identity();
    let mut request = RuntimeGraphRequestV1::new(
        graph,
        s.iter().copied().zip(streams.iter().copied()).collect(),
    )?;
    for (id, from, to, offset, len) in [
        (1, 0, 1, PAD, BODY),
        (4, 1, 2, 0, TOTAL),
        (7, 3, 4, PAD, BODY),
        (11, 2, 4, PAD, BODY / 2),
        (12, 4, 5, 0, TOTAL),
    ] {
        request.bind_copy(
            node(id),
            region(allocations[from], RuntimeAccessV1::Read, offset, len),
            region(allocations[to], RuntimeAccessV1::Write, offset, len),
        )?;
    }
    for (id, allocation, offset, len, source) in [
        (
            1,
            0,
            PAD,
            BODY,
            RuntimeGraphVersionSourceV1::InitialAtAdmission,
        ),
        (
            7,
            3,
            PAD,
            BODY,
            RuntimeGraphVersionSourceV1::InitialAtAdmission,
        ),
        (
            11,
            2,
            PAD,
            BODY / 2,
            RuntimeGraphVersionSourceV1::ProducedBy(node(4)),
        ),
    ] {
        request.expect_input_version(
            node(id),
            region(allocations[allocation], RuntimeAccessV1::Read, offset, len),
            source,
        )?;
    }
    Ok((identity, request))
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct VersionKey {
    allocation: usize,
    offset: u64,
    len: u64,
    producer: Option<u32>,
}

fn key(allocation: usize, offset: usize, len: usize, producer: Option<u32>) -> VersionKey {
    VersionKey {
        allocation,
        offset: offset as u64,
        len: len as u64,
        producer,
    }
}

fn version_key(
    version: RuntimeGraphDataVersionV1,
    execution: RuntimeGraphExecutionIdentityV1,
    allocations: &[RuntimeAllocationIdV1],
) -> ResultV1<VersionKey> {
    if version.execution() != execution {
        return Err("version has a foreign execution identity".into());
    }
    let allocation = allocations
        .iter()
        .position(|&id| id == version.allocation())
        .ok_or("version has a foreign allocation")?;
    Ok(VersionKey {
        allocation,
        offset: version.byte_offset(),
        len: version.byte_len(),
        producer: version.producer().map(CompletionNodeIdV1::get),
    })
}

fn validate_versions(
    report: &RuntimeGraphReportV1<KfdRuntimeBackendErrorV1>,
    allocations: &[RuntimeAllocationIdV1],
) -> ResultV1<()> {
    // Segmentation is determined by every read/write endpoint, not just the final copies.
    let segments: [(usize, usize, usize, &[u32]); 13] = [
        (0, PAD, BODY, &[]),
        (1, 0, PAD, &[]),
        (1, PAD, BODY, &[1]),
        (1, PAD + BODY, PAD, &[]),
        (2, 0, PAD, &[4]),
        (2, PAD, BODY / 2, &[4]),
        (2, PAD + BODY / 2, BODY / 2 + PAD, &[4]),
        (3, PAD, BODY, &[]),
        (4, 0, PAD, &[]),
        (4, PAD, BODY / 2, &[7, 11]),
        (4, PAD + BODY / 2, BODY / 2, &[7]),
        (4, PAD + BODY, PAD, &[]),
        (5, 0, TOTAL, &[12]),
    ];
    let mut expected = Vec::new();
    for (allocation, offset, len, producers) in segments {
        let mut predecessor = key(allocation, offset, len, None);
        expected.push((
            predecessor,
            None,
            RuntimeGraphVersionStateV1::AvailableAtAdmission,
            producers.is_empty(),
        ));
        for (index, &producer) in producers.iter().enumerate() {
            let version = key(allocation, offset, len, Some(producer));
            expected.push((
                version,
                Some(predecessor),
                RuntimeGraphVersionStateV1::Committed,
                index + 1 == producers.len(),
            ));
            predecessor = version;
        }
    }
    let mut actual = report
        .versions
        .iter()
        .map(|record| {
            Ok((
                version_key(record.version, report.execution, allocations)?,
                record
                    .predecessor
                    .map(|version| version_key(version, report.execution, allocations))
                    .transpose()?,
                record.state,
                record.current_at_terminal,
            ))
        })
        .collect::<ResultV1<Vec<_>>>()?;
    expected.sort_unstable_by_key(|row| row.0);
    actual.sort_unstable_by_key(|row| row.0);
    if actual != expected || actual.len() != 21 || actual.iter().filter(|row| row.3).count() != 13 {
        return Err(format!("version ledger mismatch: {actual:?}").into());
    }
    let mut expected_inputs = vec![
        (1, key(0, PAD, BODY, None), true),
        (4, key(1, 0, PAD, None), true),
        (4, key(1, PAD, BODY, Some(1)), true),
        (4, key(1, PAD + BODY, PAD, None), true),
        (7, key(3, PAD, BODY, None), true),
        (11, key(2, PAD, BODY / 2, Some(4)), true),
        (12, key(4, 0, PAD, None), true),
        (12, key(4, PAD, BODY / 2, Some(11)), true),
        (12, key(4, PAD + BODY / 2, BODY / 2, Some(7)), true),
        (12, key(4, PAD + BODY, PAD, None), true),
    ];
    let mut actual_inputs = report
        .version_inputs
        .iter()
        .map(|input| {
            Ok((
                input.consumer.get(),
                version_key(input.version, report.execution, allocations)?,
                input.available_at_issue,
            ))
        })
        .collect::<ResultV1<Vec<_>>>()?;
    expected_inputs.sort_unstable();
    actual_inputs.sort_unstable();
    if actual_inputs != expected_inputs {
        return Err(format!("input version ledger mismatch: {actual_inputs:?}").into());
    }
    Ok(())
}

fn reset(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    allocations: &[RuntimeAllocationIdV1],
) -> ResultV1<()> {
    context.write_allocation(
        allocations[0],
        0,
        &(0..TOTAL).map(|i| pattern(i, false)).collect::<Vec<_>>(),
    )?;
    context.write_allocation(allocations[1], 0, &vec![0xa7; TOTAL])?;
    context.write_allocation(allocations[2], 0, &vec![0x3c; TOTAL])?;
    context.write_allocation(
        allocations[3],
        0,
        &(0..TOTAL).map(|i| pattern(i, true)).collect::<Vec<_>>(),
    )?;
    context.write_allocation(allocations[4], 0, &vec![0x6d; TOTAL])?;
    context.write_allocation(allocations[5], 0, &vec![0x5e; TOTAL])?;
    Ok(())
}

fn readback(
    context: &mut RuntimeContextV1<KfdRuntimeBackendV1>,
    allocations: &[RuntimeAllocationIdV1],
) -> ResultV1<()> {
    for index in [0, 2, 3, 5] {
        let mut actual = vec![0; TOTAL];
        context.read_allocation(allocations[index], 0, &mut actual)?;
        for (i, &actual) in actual.iter().enumerate() {
            let expected = match index {
                0 => pattern(i, false),
                3 => pattern(i, true),
                2 if (PAD..PAD + BODY).contains(&i) => pattern(i, false),
                2 => 0xa7,
                5 if (PAD..PAD + BODY / 2).contains(&i) => pattern(i, false),
                5 if (PAD + BODY / 2..PAD + BODY).contains(&i) => pattern(i, true),
                5 => 0x6d,
                _ => unreachable!(),
            };
            if actual != expected {
                return Err(format!("canary mismatch allocation={index} byte={i}").into());
            }
        }
    }
    Ok(())
}

fn main() -> ResultV1<()> {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() != 2 {
        return Err("usage: drain-versions-qualifier <unique-id>".into());
    }
    let unique_id = u64::from_str_radix(
        arguments[1]
            .strip_prefix("0x")
            .ok_or("hexadecimal unique ID required")?,
        16,
    )?;
    if unique_id == 0 {
        return Err("nonzero unique ID required".into());
    }
    let (engine, handle) = RuntimeAsyncOwnedEngineV1::spawn_with_progress(
        move || -> ResultV1<_> {
            Ok(RuntimeContextV1::open(KfdRuntimeBackendV1::open_default(
                unique_id, CopyOnly,
            )?)?)
        },
        RuntimeAsyncEngineConfigV1::default(),
        RuntimeAsyncProgressConfigV1::default(),
    )
    .map_err(|e| format!("owner initialization: {e}"))?;
    let (streams, identities, allocations, owner) =
        wait(handle.observer().enqueue_with_context(|context| {
            let selected = context.devices()[0].id();
            let streams = (0..4)
                .map(|_| context.create_stream(selected))
                .collect::<Result<Vec<_>, _>>()?;
            let identities = streams
                .iter()
                .map(|&s| context.completion_stream_identity_v1(s))
                .collect::<Result<Vec<_>, _>>()?;
            // Hroot, Droot, Hleft, Hright, Dright, Hout. Branch custody is disjoint.
            let mut allocations = Vec::new();
            for kind in [
                RuntimeMemoryKindV1::HostVisible,
                RuntimeMemoryKindV1::DeviceLocal,
                RuntimeMemoryKindV1::HostVisible,
                RuntimeMemoryKindV1::HostVisible,
                RuntimeMemoryKindV1::DeviceLocal,
                RuntimeMemoryKindV1::HostVisible,
            ] {
                allocations.push(context.allocate(selected, kind, TOTAL as u64, 4096)?);
            }
            Ok::<_, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>((
                streams,
                identities,
                allocations,
                thread::current().id(),
            ))
        })?)???;
    assert_ne!(owner, thread::current().id());
    let mut previous_execution: Option<RuntimeGraphExecutionIdentityV1> = None;
    let mut previous_versions = Vec::new();
    for _ in 0..EXECUTIONS {
        let reset_allocations = allocations.clone();
        wait(handle.observer().enqueue_with_context(move |context| {
            assert_eq!(thread::current().id(), owner);
            reset(context, &reset_allocations)?;
            // External census window only; this qualifier makes no timing claim.
            thread::sleep(Duration::from_millis(50));
            Ok::<_, Box<dyn Error + Send + Sync>>(())
        })?)???;
        let (identity, request) = request(&streams, &identities, &allocations)?;
        let report =
            wait(handle.submit_graph(request)?)??.map_err(|e| format!("graph execution: {e:?}"))?;
        if report.completion.graph_identity() != identity
            || report.execution.graph_identity() != identity
            || report.execution.context() != identities[0].context()
            || report.execution.generation() == 0
            || report.completion.entries().len() != 12
            || report
                .completion
                .entries()
                .iter()
                .enumerate()
                .any(|(i, e)| {
                    e.node() != node(i as u32 + 1) || e.state() != CompletionNodeStateV1::Succeeded
                })
            || !report.errors.is_empty()
        {
            return Err(format!("graph did not succeed: {report:?}").into());
        }
        let mut observations: Vec<_> = report
            .observations
            .iter()
            .map(|(node, status)| (node.get(), *status))
            .collect();
        observations.sort_unstable_by_key(|entry| entry.0);
        if observations
            != [1, 4, 7, 11, 12].map(|node| (node, RuntimeCompletionStatusV1::Succeeded))
        {
            return Err("native observation roster mismatch".into());
        }
        validate_versions(&report, &allocations)?;
        if previous_execution.is_some_and(|previous| {
            previous.context() != report.execution.context()
                || previous.graph_identity() != identity
                || previous.generation() >= report.execution.generation()
        }) || report
            .versions
            .iter()
            .any(|record| previous_versions.contains(&record.version))
        {
            return Err("repeated graph occurrence or version identity was reused".into());
        }
        previous_execution = Some(report.execution);
        previous_versions = report
            .versions
            .iter()
            .map(|record| record.version)
            .collect();
        let read_streams = streams.clone();
        let read_allocations = allocations.clone();
        wait(handle.observer().enqueue_with_context(move |context| {
            assert_eq!(thread::current().id(), owner);
            for stream in read_streams {
                if context.query_stream(stream)?.total_submissions != 0 {
                    return Err("graph retained submission records".into());
                }
            }
            readback(context, &read_allocations)
        })?)???;
    }
    // All graph results and readbacks are complete before admission closes.
    let clone = handle.clone();
    let draining = handle.begin_drain(DRAIN_TICKS)?;
    if !matches!(
        clone.observer().enqueue_with_context(|_| ()),
        Err(RuntimeAsyncEngineCallErrorV1::EngineStopped)
    ) || !matches!(
        clone.begin_drain(DRAIN_TICKS),
        Err(RuntimeAsyncDrainErrorV1::AdmissionClosed)
    ) {
        return Err("drain did not close cloned-handle admission".into());
    }
    let drain = wait(draining)??;
    if drain.outcome != RuntimeAsyncDrainOutcomeV1::Quiescent
        || drain.ticks == 0
        || drain.ticks > DRAIN_TICKS
        || drain.retained_submissions != RuntimeStreamObservationV1::default()
        || !drain.queued_commands_exhausted
        || drain.operations_remaining != 0
        || drain.graph_active
    {
        return Err(format!("idle drain did not become quiescent: {drain:?}").into());
    }
    let shutdown = engine.shutdown()?;
    if shutdown.disposition != RuntimeAsyncOwnedDispositionV1::Released
        || shutdown.worker_panicked
        || shutdown.native_failure.is_some()
        || shutdown
            .cleanup
            .as_ref()
            .is_none_or(|c| !c.is_complete() || !c.failures().is_empty())
    {
        return Err(format!("owner custody retained: {shutdown:?}").into());
    }
    println!(
        "PASS schema=fe2o3.runtime.r65-graph-versions-idle-drain.v1 bytes={TOTAL} owner_threads=1 streams=4 nodes=12 copies=5 executions={EXECUTIONS} versions=21 version_inputs=10 current_versions=13 occurrences=distinct joins=host canaries=complete submissions=released drain=idle-quiescent admission=closed cleanup=complete"
    );
    Ok(())
}
