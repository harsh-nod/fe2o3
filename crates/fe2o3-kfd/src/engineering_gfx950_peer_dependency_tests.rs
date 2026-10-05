use super::*;
use std::collections::VecDeque;

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
