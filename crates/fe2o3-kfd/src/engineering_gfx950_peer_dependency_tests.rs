use super::*;
use std::collections::VecDeque;

#[test]
fn signal_completion_policy_is_explicit_and_eight_packet_only() {
    assert!(require_completion_policy::<7>(CompletionPolicy::RetiredRing).is_ok());
    assert!(require_completion_policy::<8>(CompletionPolicy::RetiredRing).is_ok());
    assert!(require_completion_policy::<7>(CompletionPolicy::Signals).is_err());
    assert!(require_completion_policy::<8>(CompletionPolicy::Signals).is_ok());
    for policy in [CompletionPolicy::RetiredRing, CompletionPolicy::Signals] {
        assert!(require_completion_policy::<0>(policy).is_err());
        assert!(require_completion_policy::<6>(policy).is_err());
        assert!(require_completion_policy::<9>(policy).is_err());
        assert!(require_completion_policy::<{ usize::MAX }>(policy).is_err());
    }
}

#[test]
fn signal_completion_helper_rejects_other_packet_counts() {
    assert!(
        complete_at_observed_counters::<0>([[]; 2], [(8, 8); 2], [8; 2], [0; 2], [true; 2],)
            .is_err()
    );
    assert!(
        complete_at_observed_counters::<7>([[0; 7]; 2], [(7, 7); 2], [7; 2], [0; 2], [true; 2],)
            .is_err()
    );
    assert!(
        complete_at_observed_counters::<9>([[0; 9]; 2], [(9, 9); 2], [9; 2], [0; 2], [true; 2],)
            .is_err()
    );
}

#[test]
fn signal_completion_requires_every_signal_and_refuses_unexpected_values() {
    assert!(
        complete_at_observed_counters([[0; 8]; 2], [(8, 3); 2], [8; 2], [0; 2], [true; 2],)
            .unwrap()
    );
    for rank in 0..2 {
        for slot in 0..8 {
            let mut values = [[0; 8]; 2];
            values[rank][slot] = 1;
            assert!(
                !complete_at_observed_counters(values, [(8, 3); 2], [8; 2], [0; 2], [true; 2],)
                    .unwrap()
            );
            for invalid in [-1, 2, i64::MIN, i64::MAX] {
                values[rank][slot] = invalid;
                assert!(
                    complete_at_observed_counters(values, [(8, 3); 2], [8; 2], [0; 2], [true; 2],)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn signal_completion_requires_both_publications() {
    for published in [[false, false], [true, false], [false, true]] {
        assert!(
            !complete_at_observed_counters([[0; 8]; 2], [(8, 3); 2], [8; 2], [0; 2], published,)
                .unwrap()
        );
        let mut invalid = [[0; 8]; 2];
        invalid[1][7] = -1;
        assert!(
            complete_at_observed_counters(invalid, [(8, 3); 2], [8; 2], [0; 2], published,)
                .is_err()
        );
    }
}

#[test]
fn signal_completion_accepts_lagging_and_distinct_nonzero_frontiers() {
    for read0 in [0, 3, 7, 8] {
        for read1 in [0, 3, 7, 8] {
            let frontiers = [(8, read0), (8, read1)];
            assert!(
                complete_at_observed_counters(
                    [[0; 8]; 2],
                    frontiers,
                    [8; 2],
                    [read0, read1],
                    [true; 2],
                )
                .unwrap()
            );
            assert_eq!(frontiers, [(8, read0), (8, read1)]);
        }
    }
    assert!(
        complete_at_observed_counters(
            [[0; 8]; 2],
            [(123, 120), (456, 450)],
            [123, 456],
            [119, 449],
            [true; 2],
        )
        .unwrap()
    );
}

#[test]
fn signal_completion_rejects_wrong_write_read_ahead_and_regression() {
    for rank in 0..2 {
        for invalid in [(7, 3), (9, 3), (8, 9)] {
            let mut frontiers = [(8, 3); 2];
            frontiers[rank] = invalid;
            assert!(
                complete_at_observed_counters([[0; 8]; 2], frontiers, [8; 2], [0; 2], [true; 2],)
                    .is_err()
            );
        }
        let mut previous_reads = [3; 2];
        previous_reads[rank] = 4;
        assert!(
            complete_at_observed_counters(
                [[0; 8]; 2],
                [(8, 3); 2],
                [8; 2],
                previous_reads,
                [true; 2],
            )
            .is_err()
        );
    }
}

#[test]
fn signal_completion_still_checks_counters_while_signals_are_pending() {
    for pending_rank in 0..2 {
        let mut values = [[0; 8]; 2];
        values[pending_rank][7] = 1;
        for bad_rank in 0..2 {
            for invalid in [(7, 3), (9, 3), (8, 9)] {
                let mut frontiers = [(8, 3); 2];
                frontiers[bad_rank] = invalid;
                assert!(
                    complete_at_observed_counters(values, frontiers, [8; 2], [0; 2], [true; 2],)
                        .is_err()
                );
            }
        }
    }
}

#[test]
fn signal_completion_requires_nonzero_expected_write_on_each_rank() {
    for rank in 0..2 {
        let mut frontiers = [(8, 3); 2];
        let mut expected_next = [8; 2];
        frontiers[rank] = (0, 0);
        expected_next[rank] = 0;
        assert!(complete_at_observed_counters(
            [[0; 8]; 2], frontiers, expected_next, [0; 2], [true; 2],
        ).is_err());
    }
}

#[test]
fn signal_completion_preserves_outstanding_capacity_and_u64_counters() {
    let full = MAX_UNRETIRED_RING_PACKETS_V1;
    let write = full + 3;
    assert!(
        complete_at_observed_counters([[0; 8]; 2], [(write, 3); 2], [write; 2], [3; 2], [true; 2],)
            .unwrap()
    );
    for rank in 0..2 {
        let mut frontiers = [(write, 3); 2];
        let mut expected_next = [write; 2];
        frontiers[rank] = (write + 1, 3);
        expected_next[rank] = write + 1;
        assert!(complete_at_observed_counters(
            [[0; 8]; 2], frontiers, expected_next, [3; 2], [true; 2],
        ).is_err());
    }
    assert!(
        complete_at_observed_counters(
            [[0; 8]; 2],
            [(u64::MAX, u64::MAX - 5); 2],
            [u64::MAX; 2],
            [u64::MAX - 6; 2],
            [true; 2],
        )
        .unwrap()
    );
    assert!(require_sequence_capacity(u64::MAX, u64::MAX - 5, 1).is_err());
}

#[test]
fn signal_completion_at_eight_three_does_not_promote_the_ring_read() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(RING_BYTES as u32).unwrap();
    let mut ring = AqlSingleProducerRingModelV1::new(capacity, 8, 3).unwrap();
    assert!(
        complete_at_observed_counters(
            [[0; 8]; 2],
            [(ring.write(), ring.last_read()); 2],
            [8; 2],
            [3; 2],
            [true; 2],
        )
        .unwrap()
    );
    assert_eq!((ring.write(), ring.last_read()), (8, 3));
    assert_eq!(
        MAX_UNRETIRED_RING_PACKETS_V1 - (ring.write() - ring.last_read()),
        MAX_UNRETIRED_RING_PACKETS_V1 - 5
    );
    let next = ring.reserve_one(3).unwrap();
    assert_eq!(next.packet_id(), 8);
    assert_eq!((ring.write(), ring.last_read()), (9, 3));
    assert!(ring.reserve_one(2).is_err());
}

#[test]
fn signal_completion_cannot_grant_a_full_ring_any_reuse_credit() {
    let capacity = AqlRingCapacityV1::from_ring_bytes(RING_BYTES as u32).unwrap();
    let write = MAX_UNRETIRED_RING_PACKETS_V1 + 3;
    let mut ring = AqlSingleProducerRingModelV1::new(capacity, write, 3).unwrap();
    assert!(
        complete_at_observed_counters([[0; 8]; 2], [(write, 3); 2], [write; 2], [3; 2], [true; 2],)
            .unwrap()
    );
    require_completed_frontier(write, write).unwrap();
    assert!(ring.reserve_one(3).is_err());
    assert_eq!((ring.write(), ring.last_read()), (write, 3));
    assert!(require_sequence_capacity(write, 3, 1).is_err());
    let next = ring.reserve_one(4).unwrap();
    assert_eq!(next.packet_id(), write);
    assert_eq!((ring.write(), ring.last_read()), (write + 1, 4));
    assert!(ring.reserve_one(4).is_err());
    assert!(ring.reserve_one(3).is_err());
}

#[test]
fn signal_completion_does_not_change_seven_or_eight_packet_strict_policy() {
    assert!(!complete_at_frontiers([[0; 7]; 2], [(7, 3); 2], [7; 2], [true; 2],).unwrap());
    assert!(!complete_at_frontiers([[0; 8]; 2], [(8, 3); 2], [8; 2], [true; 2],).unwrap());
    assert!(complete_at_frontiers([[0; 7]; 2], [(7, 7); 2], [7; 2], [true; 2],).unwrap());
    assert!(complete_at_frontiers([[0; 8]; 2], [(8, 8); 2], [8; 2], [true; 2],).unwrap());
    assert!(
        complete_at_observed_counters([[0; 8]; 2], [(8, 3); 2], [8; 2], [3; 2], [true; 2],)
            .unwrap()
    );
}

fn terminal_arenas<const N: usize>() -> [Arena<N>; 2] {
    std::array::from_fn(|rank| Arena {
        token: Gfx950EngineeringPeerBufferV1 {
            group: 1,
            id: rank as u64 + 1,
            owner: rank,
            bytes: ARENA_BYTES as u64,
        },
        local: rank as u64 + 1,
        allocation: [
            rank as u64 + 1,
            0x100_0000 + rank as u64 * 0x100_0000,
            ARENA_BYTES as u64,
            ARENA_BYTES as u64,
        ],
        participant: [rank as u64 + 1, rank as u64 + 10, 1, rank as u64 + 20],
        peer_gpu: 11 - rank as u32,
    })
}

fn terminal_prepared() -> Vec<PreparedDispatch> {
    (0..4)
        .map(|index| PreparedDispatch {
            bytes: vec![index as u8; 16],
            geometry: AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            descriptor: 0x400_0000 + index * 64,
            alignment: 16,
            group_bytes: 0,
        })
        .collect()
}

struct TerminalCapture<const N: usize> {
    bodies: [Option<[u8; 64]>; N],
    headers: Vec<(u32, u16)>,
}

impl<const N: usize> TerminalCapture<N> {
    fn new() -> Self {
        Self {
            bodies: [None; N],
            headers: Vec::new(),
        }
    }

    fn body(&mut self, index: u32, bytes: [u8; 64]) -> Result<()> {
        assert!(self.headers.is_empty());
        assert!(self.bodies[index as usize].replace(bytes).is_none());
        Ok(())
    }
}

impl<const N: usize> AqlPeerPacketBatchPublicationTargetV1 for TerminalCapture<N> {
    type Error = String;

    fn write_unpublished_kernel(
        &mut self,
        index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<()> {
        self.body(index, packet.encode_unpublished_le())
    }

    fn write_unpublished_barrier(
        &mut self,
        index: u32,
        packet: &AqlPeerBarrierAndPacketV1,
    ) -> Result<()> {
        self.body(index, packet.encode_unpublished_le())
    }

    fn publish_release_header(&mut self, index: u32, header: u16) -> Result<()> {
        assert!(self.bodies.iter().all(Option::is_some));
        assert_eq!(index as usize, self.headers.len());
        self.headers.push((index, header));
        Ok(())
    }
}

fn terminal_graph<const N: usize>() -> [TerminalCapture<N>; 2] {
    let arenas = terminal_arenas::<N>();
    std::array::from_fn(|rank| {
        let mut capture = TerminalCapture::new();
        make_batch::<N>(rank, terminal_prepared(), &arenas)
            .unwrap()
            .publish_with(&mut capture)
            .unwrap();
        capture
    })
}

fn terminal_word(bytes: &[u8; 64], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[test]
fn terminal_join_only_seven_and_eight_packet_shapes_are_admitted() {
    assert!(require_packet_count::<7>().is_ok());
    assert!(require_packet_count::<8>().is_ok());
    assert!(require_packet_count::<0>().is_err());
    assert!(require_packet_count::<1>().is_err());
    assert!(require_packet_count::<6>().is_err());
    assert!(require_packet_count::<9>().is_err());
    assert!(require_packet_count::<64>().is_err());
    assert!(require_packet_count::<{ usize::MAX }>().is_err());
    assert!(make_batch::<0>(0, terminal_prepared(), &terminal_arenas::<0>()).is_err());
    assert!(make_batch::<9>(0, terminal_prepared(), &terminal_arenas::<9>()).is_err());
}

#[test]
fn terminal_join_preserves_first_seven_packet_bodies_and_headers_exactly() {
    let original = terminal_graph::<7>();
    let terminal = terminal_graph::<8>();
    for rank in 0..2 {
        assert_eq!(&terminal[rank].bodies[..7], &original[rank].bodies);
        assert_eq!(&terminal[rank].headers[..7], &original[rank].headers);
        assert_eq!(terminal[rank].headers[7], (7, 0x1503));
    }
}

#[test]
fn terminal_join_real_batches_have_two_c1_dependencies_and_sixteen_distinct_slots() {
    let arenas = terminal_arenas::<8>();
    let graph = terminal_graph::<8>();
    let mut completions = BTreeSet::new();
    for rank in 0..2 {
        assert_eq!(graph[rank].headers.len(), 8);
        for slot in 0..8 {
            let body = graph[rank].bodies[slot].unwrap();
            assert_eq!(&body[..2], &1_u16.to_le_bytes());
            let completion = terminal_word(&body, 56);
            assert_eq!(completion, arenas[rank].signal(slot).unwrap().raw());
            assert!(completions.insert(completion));
        }
        let body = graph[rank].bodies[7].unwrap();
        assert_eq!(&body[2..8], &[0; 6]);
        assert_eq!(&body[24..56], &[0; 32]);
        let completion = terminal_word(&body, 56);
        for source_rank in 0..2 {
            let dependency = terminal_word(&body, 8 + source_rank * 8);
            assert_eq!(dependency, arenas[source_rank].signal(6).unwrap().raw());
            assert_ne!(dependency, completion);
        }
        assert_ne!(terminal_word(&body, 8), terminal_word(&body, 16));
    }
    assert_eq!(completions.len(), 16);
}

#[test]
fn terminal_join_signal_slot_uses_existing_arena_without_kernarg_overlap() {
    let original = terminal_arenas::<7>();
    let terminal = terminal_arenas::<8>();
    assert_eq!(ARENA_BYTES, PAGE_BYTES + 4 * MAX_KERNARG_BYTES_V1 as usize);
    assert!(8 * AMD_SIGNAL_BYTES_V1 <= PAGE_BYTES);
    for rank in 0..2 {
        assert_eq!(terminal[rank].allocation, original[rank].allocation);
        assert_eq!(terminal[rank].token.bytes, original[rank].token.bytes);
        assert!(original[rank].signal(7).is_err());
        assert_eq!(
            terminal[rank].signal(7).unwrap().raw(),
            terminal[rank].allocation[1] + 7 * AMD_SIGNAL_BYTES_V1 as u64,
        );
        assert!(terminal[rank].signal(8).is_err());
        assert!(terminal[rank].signal(usize::MAX).is_err());
    }
}

#[test]
fn terminal_join_witness_requires_all_fifteen_other_signals_pending() {
    let mut values = [[1; 8]; 2];
    assert!(!witness_ready::<8>(values, [true, false]).unwrap());
    values[0][0] = 0;
    assert!(witness_ready::<8>(values, [true, false]).unwrap());
    for rank in 0..2 {
        for slot in 0..8 {
            if (rank, slot) != (0, 0) {
                let mut early = values;
                early[rank][slot] = 0;
                assert!(witness_ready::<8>(early, [true, false]).is_err());
            }
            for invalid in [-1, 2, i64::MIN, i64::MAX] {
                let mut bad = values;
                bad[rank][slot] = invalid;
                assert!(witness_ready::<8>(bad, [true, false]).is_err());
            }
        }
    }
    for published in [[false, false], [false, true], [true, true]] {
        assert!(witness_ready::<8>(values, published).is_err());
    }
}

#[test]
fn terminal_join_completion_requires_all_sixteen_not_only_terminal_signals() {
    assert!(signals_complete::<8>([[0; 8]; 2]).unwrap());
    let mut finals_only = [[1; 8]; 2];
    finals_only[0][7] = 0;
    finals_only[1][7] = 0;
    assert!(!signals_complete::<8>(finals_only).unwrap());
    for rank in 0..2 {
        for slot in 0..8 {
            let mut pending = [[0; 8]; 2];
            pending[rank][slot] = 1;
            assert!(!signals_complete::<8>(pending).unwrap());
            for invalid in [-1, 2, i64::MIN, i64::MAX] {
                pending[rank][slot] = invalid;
                assert!(signals_complete::<8>(pending).is_err());
            }
        }
    }
}

#[test]
fn terminal_join_completion_keeps_exact_publication_and_frontier_requirements() {
    let complete = [[0; 8]; 2];
    assert!(complete_at_frontiers::<8>(complete, [(8, 8); 2], [8; 2], [true; 2]).unwrap());
    for published in [[false, false], [false, true], [true, false]] {
        assert!(!complete_at_frontiers::<8>(complete, [(8, 8); 2], [8; 2], published).unwrap());
    }
    for rank in 0..2 {
        for frontier in [(8, 0), (8, 3), (8, 7), (7, 7), (9, 8), (8, 9)] {
            let mut frontiers = [(8, 8); 2];
            frontiers[rank] = frontier;
            assert!(!complete_at_frontiers::<8>(complete, frontiers, [8; 2], [true; 2]).unwrap());
        }
        let mut pending_terminal = complete;
        pending_terminal[rank][7] = 1;
        assert!(
            !complete_at_frontiers::<8>(pending_terminal, [(8, 8); 2], [8; 2], [true; 2],).unwrap()
        );
        pending_terminal[rank][7] = -1;
        assert!(
            complete_at_frontiers::<8>(pending_terminal, [(8, 8); 2], [8; 2], [true; 2]).is_err()
        );
    }
    // The retained seven-packet failure is still a refusal, not new authority.
    assert!(!complete_at_frontiers::<7>([[0; 7]; 2], [(7, 3); 2], [7; 2], [true; 2]).unwrap());
}

#[test]
fn terminal_join_completion_uses_actual_nonzero_frontiers_not_packet_count() {
    let expected = [123, 456];
    let frontiers = [(123, 123), (456, 456)];
    assert!(complete_at_frontiers::<8>([[0; 8]; 2], frontiers, expected, [true; 2]).unwrap());
    assert!(!complete_at_frontiers::<8>([[0; 8]; 2], [(8, 8); 2], expected, [true; 2]).unwrap());
    for rank in 0..2 {
        let mut stale = frontiers;
        stale[rank].1 -= 1;
        assert!(!complete_at_frontiers::<8>([[0; 8]; 2], stale, expected, [true; 2]).unwrap());
    }
}

fn terminal_trace_state<const N: usize>() -> TraceState<N> {
    TraceState {
        values: [[1; N]; 2],
        frontiers: [(0, 0); 2],
        headers: Some(std::array::from_fn(|_| {
            std::array::from_fn(|slot| (slot as u32, 1, 0))
        })),
        published: [false; 2],
    }
}

#[test]
fn terminal_join_trace_deduplicates_state_but_keeps_latest_observation_time() {
    let mut trace = DependencyTrace::<8>::default();
    assert!(trace.changes.is_empty() && trace.last.is_none());
    assert_eq!(trace.first_all_zero_elapsed_ns, None);
    assert_eq!(trace.dropped_changes, 0);
    let state = terminal_trace_state::<8>();
    trace.record(10, state);
    trace.record(20, state);
    assert_eq!(trace.changes.len(), 1);
    assert_eq!(trace.changes[0].elapsed_ns, 10);
    assert_eq!(trace.changes[0].state, state);
    assert_eq!(trace.last.as_ref().unwrap().elapsed_ns, 20);
    assert_eq!(trace.last.as_ref().unwrap().state, state);
    assert_eq!(trace.dropped_changes, 0);
}

#[test]
fn terminal_join_trace_detects_each_state_field_and_missing_header_observation() {
    let mut trace = DependencyTrace::<8>::default();
    let mut state = terminal_trace_state::<8>();
    trace.record(0, state);
    state.values[0][0] = 0;
    trace.record(1, state);
    state.frontiers[0] = (8, 3);
    trace.record(2, state);
    state.headers.as_mut().unwrap()[0][7] = (7, 0x1503, 0);
    trace.record(3, state);
    state.published[0] = true;
    trace.record(4, state);
    state.headers = None;
    trace.record(5, state);
    assert_eq!(trace.changes.len(), 6);
    assert_eq!(trace.last.as_ref().unwrap().state.headers, None);
    assert_eq!(trace.first_all_zero_elapsed_ns, None);
    assert_eq!(trace.dropped_changes, 0);
}

#[test]
fn terminal_join_trace_caps_changes_but_preserves_last_and_first_zero_after_cap() {
    assert_eq!(TRACE_CHANGE_LIMIT, 16);
    let mut trace = DependencyTrace::<8>::default();
    let mut state = terminal_trace_state::<8>();
    for index in 0..20 {
        state.frontiers[0] = (index, 0);
        trace.record(index, state);
    }
    assert_eq!(trace.changes.len(), 16);
    assert_eq!(trace.dropped_changes, 4);
    for (index, retained) in trace.changes.iter().enumerate() {
        assert_eq!(retained.elapsed_ns, index as u64);
        assert_eq!(retained.state.frontiers[0], (index as u64, 0));
    }
    trace.record(100, state);
    assert_eq!(trace.dropped_changes, 4);
    state.values = [[0; 8]; 2];
    trace.record(101, state);
    assert_eq!(trace.first_all_zero_elapsed_ns, Some(101));
    assert_eq!(trace.dropped_changes, 5);
    trace.record(102, state);
    assert_eq!(trace.dropped_changes, 5);
    state.frontiers[0].0 = 20;
    trace.record(103, state);
    assert_eq!(trace.changes.len(), 16);
    assert_eq!(trace.dropped_changes, 6);
    assert_eq!(trace.first_all_zero_elapsed_ns, Some(101));
    assert_eq!(trace.last.as_ref().unwrap().elapsed_ns, 103);
    assert_eq!(trace.last.as_ref().unwrap().state, state);
}

#[test]
fn terminal_join_trace_first_zero_time_does_not_imply_frontier_retirement() {
    let mut trace = DependencyTrace::<8>::default();
    let mut state = terminal_trace_state::<8>();
    state.values = [[0; 8]; 2];
    state.frontiers = [(8, 3); 2];
    state.published = [true; 2];
    trace.record(0, state);
    assert_eq!(trace.first_all_zero_elapsed_ns, Some(0));
    assert!(
        !complete_at_frontiers::<8>(state.values, state.frontiers, [8; 2], state.published)
            .unwrap()
    );
    trace.record(50, state);
    assert_eq!(trace.changes.len(), 1);
    state.frontiers = [(8, 8); 2];
    trace.record(100, state);
    assert_eq!(trace.first_all_zero_elapsed_ns, Some(0));
    assert_eq!(trace.changes.len(), 2);
    assert!(
        complete_at_frontiers::<8>(state.values, state.frontiers, [8; 2], state.published).unwrap()
    );
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Event {
    Fence,
    Publish(usize),
    Witness,
    Complete,
    Retire,
    Pause,
    Poison,
}

#[derive(Default)]
struct Driver {
    events: Vec<Event>,
    fail_at: Option<usize>,
    witnesses: VecDeque<bool>,
    completions: VecDeque<bool>,
    published: [bool; 2],
    retired: bool,
    poisoned: bool,
}

impl Driver {
    fn step(&mut self, event: Event) -> Result<()> {
        assert!(!self.poisoned);
        let index = self.events.len();
        self.events.push(event);
        if self.fail_at == Some(index) {
            Err("injected dependency stage failure".into())
        } else {
            Ok(())
        }
    }
}

impl DependencyBackend for Driver {
    fn fence(&mut self) -> Result<()> {
        self.step(Event::Fence)
    }

    fn publish(&mut self, rank: usize) -> Result<()> {
        assert!(rank < 2 && !self.published[rank]);
        assert!(rank == 0 || self.published[0]);
        // Failure may follow publication; the driver must not assume rollback.
        self.published[rank] = true;
        self.step(Event::Publish(rank))
    }

    fn witness(&mut self) -> Result<bool> {
        assert_eq!(self.published, [true, false]);
        self.step(Event::Witness)?;
        Ok(self.witnesses.pop_front().unwrap_or(true))
    }

    fn complete(&mut self) -> Result<bool> {
        assert_eq!(self.published, [true, true]);
        self.step(Event::Complete)?;
        Ok(self.completions.pop_front().unwrap_or(true))
    }

    fn retire(&mut self) -> Result<()> {
        assert_eq!(self.published, [true, true]);
        self.retired = true;
        self.step(Event::Retire)
    }

    fn pause(&mut self) {
        self.events.push(Event::Pause);
    }

    fn poison(&mut self) {
        assert!(!self.poisoned);
        self.poisoned = true;
        self.events.push(Event::Poison);
    }
}

fn future_deadline() -> Instant {
    Instant::now().checked_add(Duration::from_secs(30)).unwrap()
}

#[test]
fn dependency_driver_normal_mode_publishes_both_before_completion_wait() {
    let mut driver = Driver::default();
    run_dependencies(&mut driver, false, future_deadline()).unwrap();
    assert_eq!(
        driver.events,
        [
            Event::Fence,
            Event::Publish(0),
            Event::Fence,
            Event::Publish(1),
            Event::Fence,
            Event::Complete,
            Event::Fence,
            Event::Retire,
        ]
    );
    assert!(driver.retired && !driver.poisoned);
}

#[test]
fn dependency_driver_witness_is_rechecked_after_fence_before_rank1() {
    let mut driver = Driver::default();
    run_dependencies(&mut driver, true, future_deadline()).unwrap();
    assert_eq!(
        driver.events,
        [
            Event::Fence,
            Event::Publish(0),
            Event::Fence,
            Event::Witness,
            Event::Fence,
            Event::Witness,
            Event::Publish(1),
            Event::Fence,
            Event::Complete,
            Event::Fence,
            Event::Retire,
        ]
    );
    assert!(driver.retired && !driver.poisoned);
}

#[test]
fn dependency_driver_every_fallible_stage_stops_and_poisons_once() {
    for witness in [false, true] {
        let mut success = Driver::default();
        run_dependencies(&mut success, witness, future_deadline()).unwrap();
        for index in 0..success.events.len() {
            let mut driver = Driver {
                fail_at: Some(index),
                ..Driver::default()
            };
            assert!(run_dependencies(&mut driver, witness, future_deadline()).is_err());
            let mut expected = success.events[..=index].to_vec();
            expected.push(Event::Poison);
            assert_eq!(driver.events, expected, "stage {index}, witness={witness}");
            assert!(driver.poisoned);
            assert_eq!(driver.retired, success.events[index] == Event::Retire);
        }
    }
}

#[test]
fn dependency_driver_pending_observations_repeat_fences_without_republication() {
    let mut driver = Driver {
        witnesses: VecDeque::from([false, true, true]),
        completions: VecDeque::from([false, true]),
        ..Driver::default()
    };
    run_dependencies(&mut driver, true, future_deadline()).unwrap();
    assert_eq!(
        driver.events,
        [
            Event::Fence,
            Event::Publish(0),
            Event::Fence,
            Event::Witness,
            Event::Pause,
            Event::Fence,
            Event::Witness,
            Event::Fence,
            Event::Witness,
            Event::Publish(1),
            Event::Fence,
            Event::Complete,
            Event::Pause,
            Event::Fence,
            Event::Complete,
            Event::Fence,
            Event::Retire,
        ]
    );
    assert!(driver.retired && !driver.poisoned);
}

#[test]
fn dependency_driver_changed_witness_refuses_rank1_and_retirement() {
    let mut driver = Driver {
        witnesses: VecDeque::from([true, false]),
        ..Driver::default()
    };
    let error = run_dependencies(&mut driver, true, future_deadline()).unwrap_err();
    assert!(error.contains("witness changed"));
    assert_eq!(driver.published, [true, false]);
    assert!(!driver.retired && driver.poisoned);
    assert_eq!(
        driver.events,
        [
            Event::Fence,
            Event::Publish(0),
            Event::Fence,
            Event::Witness,
            Event::Fence,
            Event::Witness,
            Event::Poison,
        ]
    );
}

#[test]
fn dependency_driver_expired_deadline_has_no_backend_operation_except_poison() {
    for witness in [false, true] {
        let deadline = Instant::now();
        let mut driver = Driver::default();
        assert!(run_dependencies(&mut driver, witness, deadline).is_err());
        assert_eq!(driver.events, [Event::Poison]);
        assert_eq!(driver.published, [false; 2]);
        assert!(!driver.retired && driver.poisoned);
    }
}

#[test]
fn dependency_witness_requires_exactly_one_completed_producer() {
    let mut values = [[1; PACKETS]; 2];
    assert!(!witness_ready(values, [true, false]).unwrap());
    values[0][0] = 0;
    assert!(witness_ready(values, [true, false]).unwrap());
    for rank in 0..2 {
        for slot in 0..PACKETS {
            if (rank, slot) != (0, 0) {
                let mut early = values;
                early[rank][slot] = 0;
                assert!(witness_ready(early, [true, false]).is_err());
            }
        }
    }
}

#[test]
fn dependency_witness_rejects_wrong_publication_state_and_every_invalid_signal() {
    let mut values = [[1; PACKETS]; 2];
    values[0][0] = 0;
    for published in [[false, false], [false, true], [true, true]] {
        assert!(witness_ready(values, published).is_err());
    }
    for rank in 0..2 {
        for slot in 0..PACKETS {
            for invalid in [-1, 2, i64::MIN, i64::MAX] {
                let mut bad = values;
                bad[rank][slot] = invalid;
                assert!(witness_ready(bad, [true, false]).is_err());
            }
        }
    }
}

#[test]
fn dependency_completion_requires_all_fourteen_not_only_final_signals() {
    assert!(signals_complete([[0; PACKETS]; 2]).unwrap());
    let mut finals_only = [[1; PACKETS]; 2];
    finals_only[0][PACKETS - 1] = 0;
    finals_only[1][PACKETS - 1] = 0;
    assert!(!signals_complete(finals_only).unwrap());
    for rank in 0..2 {
        for slot in 0..PACKETS {
            let mut pending = [[0; PACKETS]; 2];
            pending[rank][slot] = 1;
            assert!(!signals_complete(pending).unwrap());
            for invalid in [-1, 2, i64::MIN, i64::MAX] {
                pending[rank][slot] = invalid;
                assert!(signals_complete(pending).is_err());
            }
        }
    }
}

fn arenas() -> [Arena; 2] {
    std::array::from_fn(|rank| Arena {
        token: Gfx950EngineeringPeerBufferV1 {
            group: 1,
            id: rank as u64 + 1,
            owner: rank,
            bytes: ARENA_BYTES as u64,
        },
        local: rank as u64 + 1,
        allocation: [
            rank as u64 + 1,
            0x100_0000 + rank as u64 * 0x100_0000,
            ARENA_BYTES as u64,
            ARENA_BYTES as u64,
        ],
        participant: [rank as u64 + 1, rank as u64 + 10, 1, rank as u64 + 20],
        peer_gpu: 11 - rank as u32,
    })
}

fn prepared() -> Vec<PreparedDispatch> {
    (0..4)
        .map(|index| PreparedDispatch {
            bytes: vec![index as u8; 16],
            geometry: AqlDispatchGeometryV1::new([64, 1, 1], [64, 1, 1]).unwrap(),
            descriptor: 0x400_0000 + index * 64,
            alignment: 16,
            group_bytes: 0,
        })
        .collect()
}

#[derive(Default)]
struct PacketCapture {
    bodies: [Option<[u8; 64]>; PACKETS],
    headers: Vec<(u32, u16)>,
}

impl PacketCapture {
    fn body(&mut self, index: u32, bytes: [u8; 64]) -> Result<()> {
        assert!(self.headers.is_empty());
        assert!(self.bodies[index as usize].replace(bytes).is_none());
        Ok(())
    }
}

impl AqlPeerPacketBatchPublicationTargetV1 for PacketCapture {
    type Error = String;

    fn write_unpublished_kernel(
        &mut self,
        index: u32,
        packet: &AqlKernelDispatchPacketV1,
    ) -> Result<()> {
        self.body(index, packet.encode_unpublished_le())
    }

    fn write_unpublished_barrier(
        &mut self,
        index: u32,
        packet: &AqlPeerBarrierAndPacketV1,
    ) -> Result<()> {
        self.body(index, packet.encode_unpublished_le())
    }

    fn publish_release_header(&mut self, index: u32, header: u16) -> Result<()> {
        assert!(self.bodies.iter().all(Option::is_some));
        assert_eq!(index as usize, self.headers.len());
        self.headers.push((index, header));
        Ok(())
    }
}

fn graph() -> [PacketCapture; 2] {
    let arenas = arenas();
    std::array::from_fn(|rank| {
        let mut capture = PacketCapture::default();
        make_batch(rank, prepared(), &arenas)
            .unwrap()
            .publish_with(&mut capture)
            .unwrap();
        capture
    })
}

fn word(bytes: &[u8; 64], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

#[test]
fn dependency_real_batches_encode_exact_graph_and_fourteen_disjoint_slots() {
    assert_eq!(PACKETS, 7);
    assert_eq!(KERNEL_SLOTS, [0, 2, 4, 6]);
    assert_eq!(BARRIER_PRODUCERS, [(1, 0), (3, 2), (5, 4)]);
    let arenas = arenas();
    let graph = graph();
    let mut completions = BTreeSet::new();
    for rank in 0..2 {
        assert_eq!(
            graph[rank].headers,
            vec![
                (0, 0x1502),
                (1, 0x1503),
                (2, 0x1502),
                (3, 0x1503),
                (4, 0x1502),
                (5, 0x1503),
                (6, 0x1502),
            ]
        );
        for slot in 0..PACKETS {
            let body = graph[rank].bodies[slot].unwrap();
            assert_eq!(&body[..2], &1_u16.to_le_bytes());
            let completion = word(&body, 56);
            assert_eq!(completion, arenas[rank].allocation[1] + slot as u64 * 64);
            assert!(completions.insert(completion));
        }
        for (slot, producer) in [(1, 0), (3, 2), (5, 4)] {
            let body = graph[rank].bodies[slot].unwrap();
            assert_eq!(&body[2..8], &[0; 6]);
            assert_eq!(&body[24..56], &[0; 32]);
            for source_rank in 0..2 {
                assert_eq!(
                    word(&body, 8 + source_rank * 8),
                    arenas[source_rank].allocation[1] + producer as u64 * 64
                );
            }
        }
        for (index, slot) in [0, 2, 4, 6].into_iter().enumerate() {
            let body = graph[rank].bodies[slot].unwrap();
            assert_eq!(
                word(&body, 40),
                arenas[rank].allocation[1]
                    + (PAGE_BYTES + index * MAX_KERNARG_BYTES_V1 as usize) as u64
            );
        }
    }
    assert_eq!(completions.len(), 14);
    assert!(PACKETS * AMD_SIGNAL_BYTES_V1 <= PAGE_BYTES);
    assert_eq!(ARENA_BYTES, PAGE_BYTES + 4 * MAX_KERNARG_BYTES_V1 as usize);
}

#[test]
fn dependency_batch_refuses_cardinality_rank_and_aliased_producer_addresses() {
    let arenas = arenas();
    let mut short = prepared();
    short.pop();
    assert!(make_batch(0, short, &arenas).is_err());
    let mut long = prepared();
    long.extend(prepared().into_iter().take(1));
    assert!(make_batch(0, long, &arenas).is_err());
    assert!(make_batch(2, prepared(), &arenas).is_err());
    assert!(make_batch(0, prepared(), &arenas[..1]).is_err());
    let mut alias = arenas;
    alias[1].allocation[1] = alias[0].allocation[1];
    assert!(make_batch(0, prepared(), &alias).is_err());
}

#[test]
fn dependency_signal_address_checks_slot_bound_and_overflow() {
    let mut arenas = arenas();
    assert!(arenas[0].signal(PACKETS).is_err());
    assert!(arenas[0].signal(usize::MAX).is_err());
    arenas[0].allocation[1] = u64::MAX - 31;
    assert!(arenas[0].signal(1).is_err());
}

#[derive(Clone, Copy, Default)]
struct ScheduleState {
    next: [usize; 2],
    middle: [u8; 2],
    wrong_read: bool,
}

#[derive(Default)]
struct ScheduleTotals {
    visited: usize,
    completed: usize,
    wrong_reads: usize,
    deadlocks: usize,
}

// This is a logical graph oracle, not a GPU visibility or memory-model proof.
// Every rank executes in WaitForPrior order; barriers use the real wire handles.
fn schedules(
    graph: &[PacketCapture; 2],
    missing_barrier: Option<usize>,
    state: ScheduleState,
    totals: &mut ScheduleTotals,
) {
    totals.visited += 1;
    assert!(totals.visited < 32768);
    if state.next == [PACKETS; 2] {
        totals.completed += 1;
        totals.wrong_reads += usize::from(state.wrong_read);
        return;
    }
    let mut runnable = 0;
    for rank in 0..2 {
        let slot = state.next[rank];
        if slot == PACKETS {
            continue;
        }
        let body = graph[rank].bodies[slot].unwrap();
        if graph[rank].headers[slot].1 == 0x1503 && missing_barrier != Some(slot) {
            let mut ready = true;
            for offset in [8, 16] {
                let handle = word(&body, offset);
                let source = (0..2)
                    .flat_map(|r| (0..PACKETS).map(move |s| (r, s)))
                    .find(|&(r, s)| word(&graph[r].bodies[s].unwrap(), 56) == handle)
                    .expect("dependency refers to a packet completion");
                ready &= state.next[source.0] > source.1;
            }
            if !ready {
                continue;
            }
        }
        runnable += 1;
        let mut next = state;
        match slot {
            0 => next.middle[rank] = 1,
            2 => next.wrong_read |= next.middle[1 - rank] != 1,
            4 => next.middle[rank] = 2,
            6 => next.wrong_read |= next.middle[1 - rank] != 2,
            _ => {}
        }
        next.next[rank] += 1;
        schedules(graph, missing_barrier, next, totals);
    }
    if runnable == 0 {
        totals.deadlocks += 1;
    }
}

fn explore(missing_barrier: Option<usize>) -> ScheduleTotals {
    let mut totals = ScheduleTotals::default();
    schedules(
        &graph(),
        missing_barrier,
        ScheduleState::default(),
        &mut totals,
    );
    totals
}

#[test]
fn dependency_all_legal_interleavings_preserve_both_input_patterns() {
    let totals = explore(None);
    assert!(totals.completed > 1);
    assert_eq!(totals.deadlocks, 0);
    assert_eq!(totals.wrong_reads, 0);
}

#[test]
fn dependency_missing_both_p0_has_a_stale_peer_read_counterexample() {
    let totals = explore(Some(1));
    assert!(totals.completed > 0 && totals.wrong_reads > 0);
    assert_eq!(totals.deadlocks, 0);
}

#[test]
fn dependency_missing_both_c0_has_an_overwrite_counterexample() {
    let totals = explore(Some(3));
    assert!(totals.completed > 0 && totals.wrong_reads > 0);
    assert_eq!(totals.deadlocks, 0);
}
