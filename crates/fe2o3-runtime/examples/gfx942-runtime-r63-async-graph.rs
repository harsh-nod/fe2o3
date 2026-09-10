//! One-device copy diamond. No compute admission or performance claim.
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
            return Err("graph qualification deadline expired".into());
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
fn main() -> ResultV1<()> {
    let arguments: Vec<_> = std::env::args().collect();
    if arguments.len() != 2 {
        return Err("usage: graph-copy-qualifier <unique-id>".into());
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
            // External queue-census observation window, excluded from performance claims.
            thread::sleep(Duration::from_millis(50));
            Ok::<_, RuntimeErrorV1<KfdRuntimeBackendErrorV1>>((
                streams,
                identities,
                allocations,
                thread::current().id(),
            ))
        })?)???;
    assert_ne!(owner, thread::current().id());
    let s = &identities;
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
    let report =
        wait(handle.submit_graph(request)?)??.map_err(|e| format!("graph execution: {e:?}"))?;
    if report.completion.graph_identity() != identity
        || report.execution.graph_identity() != identity
        || report.execution.context() != s[0].context()
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
    let mut observed: Vec<_> = report
        .observations
        .iter()
        .map(|(node, status)| (node.get(), *status))
        .collect();
    observed.sort_unstable_by_key(|entry| entry.0);
    if observed != [1, 4, 7, 11, 12].map(|node| (node, RuntimeCompletionStatusV1::Succeeded)) {
        return Err("native observation roster mismatch".into());
    }
    wait(
        handle
            .observer()
            .enqueue_with_context(move |context| -> ResultV1<()> {
                assert_eq!(thread::current().id(), owner);
                for stream in streams {
                    if context.query_stream(stream)?.total_submissions != 0 {
                        return Err("graph retained submission records".into());
                    }
                }
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
                            return Err(
                                format!("canary mismatch allocation={index} byte={i}").into()
                            );
                        }
                    }
                }
                Ok(())
            })?,
    )???;
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
        "PASS schema=fe2o3.runtime.r63-async-graph-copy.v1 bytes={TOTAL} owner_threads=1 streams=4 nodes=12 copies=5 joins=host canaries=complete submissions=released cleanup=complete"
    );
    Ok(())
}
