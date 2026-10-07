#![cfg(test)]

use super::*;
use crate::async_engine::{AdmittedGraphV1, PreparedGraphAdmissionV1, RetiredGraphV1};
use crate::completion::{
    CompletionGraphV1, CompletionNodeIdV1, CompletionNodeV1, FutureIdentityV1,
};
use crate::{
    RuntimeGraphErrorV1, RuntimeGraphPeerShardV1, RuntimeGraphRequestV1,
    RuntimeGraphValidationErrorV1, RuntimeGraphVersionSourceV1, RuntimeGraphVersionStateV1,
};

mod placement;
mod refusals;

type Context = RuntimeContextV1<MockBackend>;
type Request = RuntimeGraphRequestV1<MockBackend>;

fn node(n: u32) -> CompletionNodeIdV1 {
    CompletionNodeIdV1::new(n).unwrap()
}

fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    offset: u64,
    len: u64,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: offset,
        byte_len: len,
    }
}

struct Fixture {
    context: Context,
    stream: RuntimeStreamIdV1,
    sources: [RuntimeAllocationIdV1; 2],
    destination: RuntimeAllocationIdV1,
    sink: RuntimeAllocationIdV1,
}

impl Fixture {
    fn new(journal: bool) -> Self {
        let backend = MockBackend {
            next: 100,
            third_device: true,
            deferred_copies: true,
            ..MockBackend::default()
        };
        let mut context = if journal {
            Context::open_with_version_journal_v1(backend, 32, 16).unwrap()
        } else {
            Context::open(backend).unwrap()
        };
        let devices: Vec<_> = context.devices().iter().map(|d| d.id()).collect();
        let stream = context.create_stream(devices[0]).unwrap();
        let mut allocate = |device| {
            context
                .allocate(device, RuntimeMemoryKindV1::DeviceLocal, 96, 16)
                .unwrap()
        };
        let sources = [allocate(devices[1]), allocate(devices[2])];
        let destination = allocate(devices[0]);
        let sink = allocate(devices[0]);
        context
            .write_allocation(sources[0], 0, &[0x31; 96])
            .unwrap();
        context
            .write_allocation(sources[1], 0, &[0x62; 96])
            .unwrap();
        context
            .write_allocation(destination, 0, &[0xa7; 96])
            .unwrap();
        context.write_allocation(sink, 0, &[0xb8; 96]).unwrap();
        Self {
            context,
            stream,
            sources,
            destination,
            sink,
        }
    }

    fn request(&self, nodes: u32) -> Request {
        let identity = self
            .context
            .completion_stream_identity_v1(self.stream)
            .unwrap();
        let graph = CompletionGraphV1::new(
            identity.context(),
            vec![identity],
            (1..=nodes)
                .map(|n| {
                    CompletionNodeV1::future(
                        node(n),
                        FutureIdentityV1::new(identity, [n as u8; 32]),
                        (n > 1).then(|| node(n - 1)),
                    )
                })
                .collect(),
        )
        .unwrap();
        Request::new(graph, vec![(identity, self.stream)]).unwrap()
    }

    fn shards(&self, count: usize) -> Vec<RuntimeGraphPeerShardV1> {
        (0..count)
            .map(|i| RuntimeGraphPeerShardV1 {
                node: node(i as u32 + 1),
                source: region(self.sources[i % 2], RuntimeAccessV1::Read, i as u64 * 8, 8),
                destination: region(
                    self.destination,
                    RuntimeAccessV1::Write,
                    i as u64 * 8 + 8,
                    8,
                ),
            })
            .collect()
    }

    fn admit(
        &mut self,
        request: Request,
    ) -> (
        AdmittedGraphV1,
        Vec<Option<Box<PreparedContextGraphActionV1>>>,
    ) {
        let mut original = Some(request);
        let prepared = prepare(&mut self.context, &mut original).unwrap();
        assert!(original.is_none());
        match prepared.commit(&mut self.context) {
            Ok(value) => value,
            Err(failure) => panic!("admission failed: {:?}", failure.error),
        }
    }

    fn read(&mut self, allocation: RuntimeAllocationIdV1) -> Vec<u8> {
        let mut bytes = vec![0; 96];
        self.context
            .read_allocation(allocation, 0, &mut bytes)
            .unwrap();
        bytes
    }
}

fn prepare(
    context: &mut Context,
    request: &mut Option<Request>,
) -> Result<PreparedGraphAdmissionV1<MockBackend>, RuntimeGraphErrorV1<MockError>> {
    PreparedGraphAdmissionV1::prepare(context, request, |_, _, _| {
        Err(RuntimeGraphErrorV1::Invalid(
            RuntimeGraphValidationErrorV1::MissingOperation,
        ))
    })
}

// This CPU oracle commits graph success only after the original Context release.
#[allow(unsafe_code)]
fn settle(
    context: &mut Context,
    mut graph: AdmittedGraphV1,
    mut actions: Vec<Option<Box<PreparedContextGraphActionV1>>>,
) -> RetiredGraphV1 {
    let token = graph.token();
    let mut count = 0;
    while let Some(index) = graph.pop_ready_notification() {
        assert!(graph.begin(index));
        let mut submission = context
            .submit_graph_action_v1(token, *actions[index].take().unwrap())
            .unwrap();
        let is_peer = context.scalar_peer_copies.contains_key(&submission.id());
        if is_peer {
            context
                .validate_scalar_peer_custody_v1(submission.id())
                .unwrap();
            assert_eq!(context.scalar_peer_copies.len(), 1);
        }
        assert!(matches!(
            context.poll(&mut submission),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        assert_eq!(
            context
                .poll_with_graph_access_v1(&mut submission, Some(token))
                .unwrap(),
            RuntimePollV1::Pending
        );
        assert_eq!(
            context
                .poll_with_graph_access_v1(&mut submission, Some(token))
                .unwrap(),
            RuntimePollV1::Succeeded
        );
        context
            .release_graph_submission_v1(token, &submission)
            .unwrap();
        assert!(context.scalar_peer_copies.is_empty());
        // SAFETY: the exact original submission succeeded and was released above.
        assert!(unsafe { graph.succeed_operation(index) });
        count += 1;
    }
    assert_eq!(count, graph.len());
    assert!(graph.is_terminal());
    match graph.finish(context) {
        Ok(report) => report,
        Err(failure) => panic!("retirement failed: {:?}", failure.error),
    }
}

#[test]
fn peer_gather_original_allocations_retire_before_segment_versions_and_local_consumer() {
    for journal in [false, true] {
        let mut f = Fixture::new(journal);
        let mut request = f.request(4);
        let shards = f.shards(3);
        request.bind_peer_gather_v1(&shards).unwrap();
        for shard in &shards {
            request
                .expect_input_version(
                    shard.node,
                    shard.source,
                    RuntimeGraphVersionSourceV1::InitialAtAdmission,
                )
                .unwrap();
        }
        request
            .bind_copy(
                node(4),
                region(f.destination, RuntimeAccessV1::Read, 8, 24),
                region(f.sink, RuntimeAccessV1::Write, 16, 24),
            )
            .unwrap();
        let (graph, actions) = f.admit(request);
        assert!(matches!(
            f.context.release_allocation(f.sources[0]),
            Err(RuntimeErrorV1::Validation(
                RuntimeValidationErrorV1::ContextReserved
            ))
        ));
        let report = settle(&mut f.context, graph, actions);
        let mut expected = vec![0xa7; 96];
        expected[8..16].fill(0x31);
        expected[16..24].fill(0x62);
        expected[24..32].fill(0x31);
        assert_eq!(f.read(f.destination), expected);
        let mut sink = vec![0xb8; 96];
        sink[16..40].copy_from_slice(&expected[8..32]);
        assert_eq!(f.read(f.sink), sink);
        assert_eq!(f.read(f.sources[0]), vec![0x31; 96]);
        assert_eq!(f.read(f.sources[1]), vec![0x62; 96]);
        let inputs: Vec<_> = report
            .version_inputs
            .iter()
            .filter(|v| v.consumer == node(4))
            .collect();
        assert_eq!(inputs.len(), 3);
        for (index, input) in inputs.iter().enumerate() {
            assert!(input.available_at_issue);
            assert_eq!(input.version.producer(), Some(node(index as u32 + 1)));
            assert_eq!(input.version.allocation(), f.destination);
        }
        assert!(
            report
                .versions
                .iter()
                .filter(|v| v.version.producer().is_some())
                .all(|v| v.state == RuntimeGraphVersionStateV1::Committed)
        );
        assert_eq!(f.context.backend.copy_call_count, 4);
        assert!(f.context.submissions.is_empty());
        assert!(f.context.backend_submissions.is_empty());
        assert!(f.context.cleanup().is_complete());
    }
}

#[test]
fn peer_gather_maximum_eight_shards_uses_existing_retirement_for_each_original() {
    let mut f = Fixture::new(true);
    let mut request = f.request(8);
    request.bind_peer_gather_v1(&f.shards(8)).unwrap();
    let (graph, actions) = f.admit(request);
    let report = settle(&mut f.context, graph, actions);
    assert_eq!(report.version_inputs.len(), 8);
    assert_eq!(f.context.backend.copy_call_count, 8);
    assert!(f.context.cleanup().is_complete());
}
