use super::*;
use std::cell::Cell;

// Script only the adapters. Selection and one-quantum driving are the same
// functions used by the native backend; these fixtures carry no GPU authority.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Record {
    id: u64,
    direction: usize,
    dependencies: Vec<u64>,
    cursor: usize,
    ready_indexed: bool,
    published: bool,
    sequence: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Records {
    active: HashMap<u64, Record>,
    completed: HashMap<u64, BackendPollV1>,
    ready: [VecDeque<u64>; 2],
    in_flight: [Vec<u64>; 2],
    sequence: [Option<u64>; 2],
    retains: HashMap<u64, usize>,
    depths: HashMap<u64, usize>,
    reservations: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Fault {
    Rejected,
    Quiescent,
    Terminal,
    Panic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Error {
    Selection(SelectionError),
    Adapter(Fault),
}

struct Rig {
    records: Records,
    calls: Vec<Action>,
    batches: Vec<Vec<u64>>,
    observe_status: BackendPollV1,
    recover_publication: bool,
    fault: Option<Fault>,
    queries: Cell<usize>,
}

impl Rig {
    fn new() -> Self {
        Self {
            records: Records {
                active: HashMap::new(),
                completed: HashMap::new(),
                ready: std::array::from_fn(|_| VecDeque::with_capacity(4096)),
                in_flight: std::array::from_fn(|_| Vec::with_capacity(63)),
                sequence: [None; 2],
                retains: HashMap::new(),
                depths: HashMap::new(),
                reservations: 0,
            },
            calls: Vec::new(),
            batches: Vec::new(),
            observe_status: BackendPollV1::Succeeded,
            recover_publication: false,
            fault: None,
            queries: Cell::new(0),
        }
    }

    fn add(&mut self, id: u64, direction: usize, dependencies: &[u64]) {
        let depth = dependencies
            .iter()
            .map(|id| self.records.depths[id])
            .max()
            .unwrap_or(0)
            + 1;
        self.records.depths.insert(id, depth);
        for dependency in dependencies {
            *self.records.retains.entry(*dependency).or_default() += 1;
        }
        assert!(
            self.records
                .active
                .insert(
                    id,
                    Record {
                        id,
                        direction,
                        dependencies: dependencies.to_vec(),
                        cursor: 0,
                        ready_indexed: false,
                        published: false,
                        sequence: false,
                    }
                )
                .is_none()
        );
        self.records.reservations += 1;
        self.enqueue_if_ready(id);
    }

    fn enqueue_if_ready(&mut self, id: u64) {
        let node = self.records.active.get_mut(&id).unwrap();
        if !node.published
            && node.dependencies.iter().all(|dependency| {
                self.records.completed.get(dependency) == Some(&BackendPollV1::Succeeded)
            })
        {
            index_xgmi_ready_id_v1(
                &mut self.records.ready[node.direction],
                &mut node.ready_indexed,
                id,
                false,
            );
        }
    }

    fn extract_index(&mut self, id: u64, direction: usize, indexed: bool) {
        let phase = remove_xgmi_progress_index_v1(
            &mut self.records.ready[direction],
            indexed,
            &mut self.records.in_flight[direction],
            id,
        );
        assert_eq!(indexed, phase == XgmiProgressIndexPhaseV1::Ready);
    }

    fn published(&mut self, id: u64) {
        let direction = self.records.active[&id].direction;
        self.extract_index(id, direction, self.records.active[&id].ready_indexed);
        self.records.active.get_mut(&id).unwrap().ready_indexed = false;
        self.records.active.get_mut(&id).unwrap().published = true;
        insert_ordered_xgmi_id_v1(&mut self.records.in_flight[direction], id);
    }

    fn settle(&mut self, id: u64, status: BackendPollV1) {
        assert_ne!(status, BackendPollV1::Pending);
        let node = self.records.active.remove(&id).unwrap();
        self.extract_index(id, node.direction, node.ready_indexed);
        for dependency in node.dependencies {
            let count = self.records.retains.get_mut(&dependency).unwrap();
            *count -= 1;
            if *count == 0 {
                self.records.retains.remove(&dependency);
            }
        }
        self.records.reservations -= 1;
        assert!(self.records.completed.insert(id, status).is_none());
        let waiters: Vec<_> = self
            .records
            .active
            .values()
            .filter(|node| node.dependencies.contains(&id))
            .map(|node| node.id)
            .collect();
        for waiter in waiters {
            self.enqueue_if_ready(waiter);
        }
    }

    fn inject(&self) -> Result<(), Error> {
        match self.fault {
            Some(Fault::Panic) => panic!("scripted adapter unwind"),
            Some(fault) => Err(Error::Adapter(fault)),
            None => Ok(()),
        }
    }

    fn assert_corrupt(&mut self, root: u64) {
        let before = self.calls.len();
        assert_eq!(
            progress(self, root),
            Err(Error::Selection(SelectionError::Corrupt))
        );
        assert_eq!(self.calls.len(), before);
    }
}

impl State for Rig {
    fn completed(&self, id: u64) -> Option<BackendPollV1> {
        self.queries.set(self.queries.get() + 1);
        self.records.completed.get(&id).copied()
    }

    fn node(&self, id: u64) -> Option<Node<'_>> {
        self.queries.set(self.queries.get() + 1);
        self.records.active.get(&id).map(|record| Node {
            id: record.id,
            direction: record.direction,
            dependencies: &record.dependencies,
            cursor: record.cursor,
            published: record.published,
            sequence: record.sequence,
        })
    }

    fn advance(&mut self, id: u64, cursor: usize) {
        self.records.active.get_mut(&id).unwrap().cursor = cursor;
    }

    fn in_flight_ids(&self, direction: usize) -> &[u64] {
        &self.records.in_flight[direction]
    }

    fn ready_queue(&self, direction: usize) -> &VecDeque<u64> {
        &self.records.ready[direction]
    }

    fn ready_contains(&self, direction: usize, id: u64) -> bool {
        self.records
            .active
            .get(&id)
            .is_some_and(|record| record.direction == direction && record.ready_indexed)
    }

    fn sequence(&self, direction: usize) -> Option<u64> {
        self.records.sequence[direction]
    }

    fn retained(&self, id: u64) -> bool {
        self.records
            .retains
            .get(&id)
            .is_some_and(|count| *count != 0)
    }

    fn depth(&self, id: u64) -> Option<usize> {
        self.records.depths.get(&id).copied()
    }
}

impl Driver for Rig {
    type Error = Error;

    fn selection_error(&mut self, error: SelectionError) -> Error {
        Error::Selection(error)
    }

    fn observe(&mut self, id: u64) -> Result<(), Error> {
        self.calls.push(Action::Observe(id));
        self.inject()?;
        if self.observe_status != BackendPollV1::Pending {
            self.settle(id, self.observe_status);
        }
        Ok(())
    }

    fn publish(&mut self, direction: usize) -> Result<(), Error> {
        self.calls.push(Action::Publish(direction));
        self.inject()?;
        let ids: Vec<_> = self.records.ready[direction]
            .iter()
            .take(63)
            .copied()
            .collect();
        assert!(!ids.is_empty());
        for id in &ids {
            if self.recover_publication {
                self.settle(*id, BackendPollV1::Failed { code: 7 });
            } else {
                self.published(*id);
            }
        }
        self.batches.push(ids);
        Ok(())
    }

    fn fail(&mut self, id: u64) {
        self.calls.push(Action::Fail(id));
        self.settle(
            id,
            BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1,
            },
        );
    }
}

#[test]
fn ready_root_requires_separate_publication_and_observation_quanta() {
    for direction in 0..2 {
        let mut rig = Rig::new();
        rig.add(1, direction, &[]);
        assert_eq!(progress(&mut rig, 1), Ok(BackendPollV1::Pending));
        assert_eq!(rig.calls, [Action::Publish(direction)]);
        assert_eq!(rig.records.reservations, 1);
        assert_eq!(progress(&mut rig, 1), Ok(BackendPollV1::Succeeded));
        assert_eq!(rig.calls, [Action::Publish(direction), Action::Observe(1)]);
        assert_eq!(rig.records.reservations, 0);
        assert_eq!(progress(&mut rig, 1), Ok(BackendPollV1::Succeeded));
        assert_eq!(rig.calls.len(), 2);
    }
}

#[test]
fn tail_root_drains_fifo_prefixes_without_reenqueue_or_false_completion() {
    let mut rig = Rig::new();
    for id in 1..=130 {
        rig.add(id, 0, &[]);
    }
    let mut result = BackendPollV1::Pending;
    for _ in 0..133 {
        let before = rig.calls.len();
        result = progress(&mut rig, 130).unwrap();
        assert_eq!(rig.calls.len(), before + 1);
        assert_eq!(
            rig.records.ready[0].len(),
            rig.records
                .active
                .values()
                .filter(|record| record.ready_indexed && record.direction == 0)
                .count()
        );
        assert_eq!(rig.records.reservations, rig.records.active.len());
        if result == BackendPollV1::Succeeded {
            break;
        }
    }
    assert_eq!(result, BackendPollV1::Succeeded);
    assert_eq!(
        rig.batches.iter().map(Vec::len).collect::<Vec<_>>(),
        [63, 63, 4]
    );
    assert_eq!(rig.batches.concat(), (1..=130).collect::<Vec<_>>());
    // Focused observation may complete the root before its unrelated batch peers.
    assert_eq!(rig.records.active.len(), 3);
}

#[test]
fn pending_blocker_is_observed_once_without_consuming_ready_root() {
    let mut rig = Rig::new();
    rig.add(1, 0, &[]);
    rig.published(1);
    rig.add(2, 0, &[]);
    rig.observe_status = BackendPollV1::Pending;
    let before = rig.records.clone();
    for _ in 0..3 {
        assert_eq!(progress(&mut rig, 2), Ok(BackendPollV1::Pending));
    }
    assert_eq!(rig.records, before);
    assert_eq!(rig.calls, vec![Action::Observe(1); 3]);
}

#[test]
fn consumer_only_progress_executes_alternating_dependency_chain() {
    let mut rig = Rig::new();
    rig.add(1, 0, &[]);
    rig.add(2, 1, &[1]);
    rig.add(3, 0, &[2]);
    for _ in 0..5 {
        assert_eq!(progress(&mut rig, 3), Ok(BackendPollV1::Pending));
    }
    assert_eq!(progress(&mut rig, 3), Ok(BackendPollV1::Succeeded));
    assert_eq!(
        rig.calls,
        [
            Action::Publish(0),
            Action::Observe(1),
            Action::Publish(1),
            Action::Observe(2),
            Action::Publish(0),
            Action::Observe(3)
        ]
    );
    assert!(rig.records.retains.is_empty());
    assert_eq!(rig.records.reservations, 0);
}

#[test]
fn diamond_wakes_join_once_and_retains_producer_until_both_children_settle() {
    let mut rig = Rig::new();
    rig.add(1, 0, &[]);
    rig.add(2, 1, &[1]);
    rig.add(3, 1, &[1]);
    rig.add(4, 0, &[2, 3]);
    for _ in 0..3 {
        assert_eq!(progress(&mut rig, 4), Ok(BackendPollV1::Pending));
    }
    assert_eq!(rig.records.retains.get(&1), Some(&2));
    assert_eq!(progress(&mut rig, 4), Ok(BackendPollV1::Pending));
    assert_eq!(rig.records.retains.get(&1), Some(&1));
    assert_eq!(progress(&mut rig, 4), Ok(BackendPollV1::Pending));
    assert!(!rig.records.retains.contains_key(&1));
    rig.enqueue_if_ready(4);
    rig.enqueue_if_ready(4);
    assert_eq!(rig.records.ready[0], [4]);
    assert_eq!(progress(&mut rig, 4), Ok(BackendPollV1::Pending));
    assert_eq!(progress(&mut rig, 4), Ok(BackendPollV1::Succeeded));
    assert_eq!(rig.records.reservations, 0);
}

#[test]
fn failed_dependency_precedes_earlier_pending_dependency() {
    let mut rig = Rig::new();
    rig.add(1, 0, &[]);
    rig.add(2, 0, &[]);
    rig.settle(2, BackendPollV1::Failed { code: 99 });
    rig.add(3, 1, &[1, 2]);
    assert_eq!(
        progress(&mut rig, 3),
        Ok(BackendPollV1::Failed {
            code: COOPERATIVE_COPY_FAILURE_CODE_V1
        })
    );
    assert_eq!(rig.calls, [Action::Fail(3)]);
    assert!(!rig.records.active[&1].published);
}

#[test]
fn failure_propagates_only_one_consumer_per_call() {
    let mut rig = Rig::new();
    rig.add(1, 0, &[]);
    rig.add(2, 1, &[1]);
    rig.add(3, 0, &[2]);
    rig.settle(1, BackendPollV1::Failed { code: 99 });
    assert_eq!(progress(&mut rig, 3), Ok(BackendPollV1::Pending));
    assert_eq!(rig.calls, [Action::Fail(2)]);
    assert!(matches!(
        progress(&mut rig, 3),
        Ok(BackendPollV1::Failed { .. })
    ));
    assert_eq!(rig.calls, [Action::Fail(2), Action::Fail(3)]);
}

#[test]
fn recovered_failure_of_unrelated_prefix_is_not_root_failure() {
    let mut rig = Rig::new();
    for id in 1..=64 {
        rig.add(id, 0, &[]);
    }
    rig.recover_publication = true;
    assert_eq!(progress(&mut rig, 64), Ok(BackendPollV1::Pending));
    assert_eq!(rig.records.completed.len(), 63);
    assert_eq!(rig.records.ready[0], [64]);
    assert_eq!(
        progress(&mut rig, 64),
        Ok(BackendPollV1::Failed { code: 7 })
    );
}

#[test]
fn scripted_adapter_boundary_errors_and_unwinds_preserve_consumer_records() {
    for cursor in 0..=8 {
        for published in [false, true] {
            for fault in [
                Fault::Rejected,
                Fault::Quiescent,
                Fault::Terminal,
                Fault::Panic,
            ] {
                let mut rig = Rig::new();
                for id in 1..=9 {
                    rig.add(id, 0, &[]);
                    if id != cursor + 1 {
                        rig.settle(id, BackendPollV1::Succeeded);
                    }
                }
                if published {
                    rig.published(cursor + 1);
                }
                rig.add(10, 1, &(1..=9).collect::<Vec<_>>());
                rig.records.active.get_mut(&10).unwrap().cursor = cursor as usize;
                let before = rig.records.clone();
                rig.fault = Some(fault);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    progress(&mut rig, 10)
                }));
                if fault == Fault::Panic {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap(), Err(Error::Adapter(fault)));
                }
                assert_eq!(rig.records, before);
                assert_eq!(rig.calls.len(), 1);
            }
        }
    }
}

#[test]
fn cached_prefix_advances_but_never_discards_dependency_identity() {
    let mut rig = Rig::new();
    for id in 1..=3 {
        rig.add(id, 0, &[]);
    }
    rig.settle(1, BackendPollV1::Succeeded);
    rig.settle(2, BackendPollV1::Succeeded);
    rig.add(4, 1, &[1, 2, 3]);
    assert_eq!(progress(&mut rig, 4), Ok(BackendPollV1::Pending));
    assert_eq!(rig.records.active[&4].cursor, 2);
    assert_eq!(rig.records.active[&4].dependencies, [1, 2, 3]);
    rig.records
        .completed
        .insert(1, BackendPollV1::Failed { code: 1 });
    rig.assert_corrupt(4);
}

#[test]
fn maximum_chain_is_iterative_and_one_deeper_rejects_before_effects() {
    for depth in [256, 257] {
        let mut rig = Rig::new();
        rig.add(1, 0, &[]);
        for id in 2..=depth {
            rig.add(id, id as usize % 2, &[id - 1]);
        }
        if depth == 256 {
            assert_eq!(progress(&mut rig, depth), Ok(BackendPollV1::Pending));
            assert_eq!(rig.calls, [Action::Publish(0)]);
            assert!(rig.queries.get() < 4 * depth as usize + 10);
        } else {
            rig.assert_corrupt(depth);
        }
    }
}

#[test]
fn dependency_roster_accepts_limit_and_rejects_one_more() {
    for count in [256, 257] {
        let mut rig = Rig::new();
        for id in 1..=count {
            rig.add(id, 0, &[]);
            rig.settle(id, BackendPollV1::Succeeded);
        }
        rig.add(300, 1, &(1..=count).collect::<Vec<_>>());
        if count == 256 {
            assert_eq!(progress(&mut rig, 300), Ok(BackendPollV1::Pending));
            assert_eq!(rig.records.active[&300].cursor, 256);
        } else {
            rig.assert_corrupt(300);
        }
    }
}

#[test]
fn malformed_dependency_identity_retention_depth_or_cursor_rejects_without_action() {
    for mutation in 0..14 {
        let mut rig = Rig::new();
        rig.add(1, 0, &[]);
        rig.add(2, 1, &[1]);
        match mutation {
            0 => {
                rig.records.active.remove(&1);
            }
            1 => {
                rig.records.completed.insert(1, BackendPollV1::Succeeded);
            }
            2 => {
                rig.records.active.get_mut(&2).unwrap().dependencies = vec![2];
            }
            3 => {
                rig.records.active.get_mut(&2).unwrap().dependencies = vec![1, 1];
            }
            4 => {
                rig.records.active.get_mut(&2).unwrap().cursor = 2;
            }
            5 => {
                rig.records.active.get_mut(&2).unwrap().cursor = 1;
            }
            6 => {
                rig.records.retains.remove(&1);
            }
            7 => {
                rig.records.retains.insert(1, 0);
            }
            8 => {
                rig.records.depths.remove(&1);
            }
            9 => {
                rig.records.depths.insert(1, 0);
            }
            10 => {
                rig.records.depths.insert(1, 2);
            }
            11 => {
                rig.records.depths.insert(2, 3);
            }
            12 => {
                rig.records.active.get_mut(&1).unwrap().dependencies = vec![2];
            }
            _ => {
                rig.records.active.remove(&1);
                rig.records.completed.insert(1, BackendPollV1::Pending);
            }
        }
        rig.assert_corrupt(2);
    }
}

#[test]
fn published_root_requires_matching_inflight_and_successful_dependencies() {
    for mutation in 0..5 {
        let mut rig = Rig::new();
        rig.add(1, 0, &[]);
        rig.settle(1, BackendPollV1::Succeeded);
        rig.add(2, 1, &[1]);
        rig.published(2);
        match mutation {
            0 => rig.records.in_flight[1].clear(),
            1 => {
                rig.records.active.get_mut(&2).unwrap().ready_indexed = true;
            }
            2 => {
                rig.records
                    .completed
                    .insert(1, BackendPollV1::Failed { code: 1 });
            }
            3 => {
                rig.records.active.get_mut(&2).unwrap().id = 3;
            }
            _ => {
                rig.records.active.get_mut(&2).unwrap().direction = 2;
            }
        }
        rig.assert_corrupt(2);
    }
}

#[test]
fn unrelated_blocker_is_validated_before_observation() {
    for mutation in 0..7 {
        let mut rig = Rig::new();
        rig.add(1, 0, &[]);
        rig.published(1);
        rig.add(2, 0, &[]);
        match mutation {
            0 => {
                rig.records.active.remove(&1);
            }
            1 => {
                rig.records.active.get_mut(&1).unwrap().published = false;
            }
            2 => {
                rig.records.active.get_mut(&1).unwrap().direction = 1;
            }
            3 => {
                rig.records.completed.insert(1, BackendPollV1::Succeeded);
            }
            4 => {
                rig.records.active.get_mut(&1).unwrap().cursor = 1;
            }
            5 => {
                rig.records.depths.remove(&1);
            }
            _ => {
                rig.records.active.get_mut(&1).unwrap().ready_indexed = true;
            }
        }
        rig.assert_corrupt(2);
    }
}

#[test]
fn missing_ready_membership_rejects_without_repair() {
    let mut rig = Rig::new();
    rig.add(1, 0, &[]);
    rig.records.active.get_mut(&1).unwrap().ready_indexed = false;
    let before = rig.records.clone();
    rig.assert_corrupt(1);
    assert_eq!(rig.records, before);
}

#[test]
fn hostile_publication_prefixes_reject_before_mapping() {
    for mutation in 0..6 {
        let mut rig = Rig::new();
        rig.add(1, 0, &[]);
        rig.add(2, 0, &[]);
        match mutation {
            0 => rig.records.ready[0].clear(),
            1 => rig.records.ready[0][0] = 99,
            2 => rig.records.ready[0][0] = 2,
            3 => rig.records.active.get_mut(&1).unwrap().ready_indexed = false,
            4 => rig.records.active.get_mut(&1).unwrap().direction = 1,
            _ => {
                rig.records.active.get_mut(&1).unwrap().dependencies.push(2);
            }
        }
        rig.assert_corrupt(2);
    }
}

#[test]
fn hostile_inflight_windows_reject_before_ticket_observation() {
    for mutation in 0..5 {
        let mut rig = Rig::new();
        rig.add(1, 0, &[]);
        rig.add(2, 0, &[]);
        rig.published(1);
        rig.published(2);
        rig.add(3, 0, &[]);
        match mutation {
            0 => rig.records.in_flight[0] = vec![1; 64],
            1 => rig.records.in_flight[0] = vec![1, 1],
            2 => rig.records.in_flight[0] = vec![2, 1],
            3 => rig.records.in_flight[1] = vec![1],
            _ => {
                rig.records.active.get_mut(&2).unwrap().published = false;
            }
        }
        rig.assert_corrupt(3);
    }
}

#[test]
fn ordered_root_rejects_and_ordered_dependency_waits_for_its_own_progress() {
    for published in [false, true] {
        let mut rig = Rig::new();
        rig.add(1, 0, &[]);
        if published {
            rig.published(1);
        }
        rig.records.active.get_mut(&1).unwrap().sequence = true;
        rig.records.sequence[0] = Some(1);
        rig.add(2, 1, &[1]);
        let before = rig.records.clone();
        assert_eq!(
            progress(&mut rig, 1),
            Err(Error::Selection(SelectionError::Unsupported))
        );
        assert_eq!(progress(&mut rig, 2), Ok(BackendPollV1::Pending));
        assert_eq!(rig.records, before);
        assert!(rig.calls.is_empty());
        rig.records.sequence[0] = Some(99);
        rig.assert_corrupt(2);
    }
}

#[test]
fn scalar_direction_with_ordered_marker_is_corruption_not_silent_stall() {
    let mut rig = Rig::new();
    rig.add(1, 0, &[]);
    rig.records.sequence[0] = Some(99);
    rig.assert_corrupt(1);
}

#[test]
fn unknown_and_completed_roots_never_enter_adapters() {
    let mut rig = Rig::new();
    assert_eq!(
        progress(&mut rig, 1),
        Err(Error::Selection(SelectionError::Unknown))
    );
    rig.add(1, 0, &[]);
    rig.records.completed.insert(1, BackendPollV1::Succeeded);
    rig.assert_corrupt(1);
    rig.records.active.remove(&1);
    for status in [BackendPollV1::Succeeded, BackendPollV1::Failed { code: 97 }] {
        rig.records.completed.insert(1, status);
        assert_eq!(progress(&mut rig, 1), Ok(status));
    }
    rig.records.completed.insert(1, BackendPollV1::Pending);
    rig.assert_corrupt(1);
    assert!(rig.calls.is_empty());
}

#[test]
fn indexed_ready_insertion_is_idempotent_and_restores_fifo_without_growth() {
    let mut ready = VecDeque::with_capacity(4096);
    let mut indexed = [false; 4096];
    let capacity = ready.capacity();
    for id in 1..=4096 {
        assert!(index_xgmi_ready_id_v1(
            &mut ready,
            &mut indexed[id as usize - 1],
            id,
            false
        ));
        assert!(!index_xgmi_ready_id_v1(
            &mut ready,
            &mut indexed[id as usize - 1],
            id,
            true
        ));
    }
    for id in 1..=63 {
        assert_eq!(ready.pop_front(), Some(id));
        assert!(core::mem::replace(&mut indexed[id as usize - 1], false));
    }
    for id in (1..=63).rev() {
        assert!(index_xgmi_ready_id_v1(
            &mut ready,
            &mut indexed[id as usize - 1],
            id,
            true
        ));
    }
    assert_eq!(
        ready.iter().copied().collect::<Vec<_>>(),
        (1..=4096).collect::<Vec<_>>()
    );
    assert_eq!(ready.capacity(), capacity);
    assert!(indexed.into_iter().all(|flag| flag));
}

#[test]
fn waiting_removal_does_not_search_or_mutate_unrelated_ready_backlog() {
    let mut ready: VecDeque<_> = (1..=4096).collect();
    let before = ready.clone();
    let mut in_flight = Vec::new();
    // A hostile unindexed marker detects an accidental FIFO search. The helper
    // trusts membership; global index auditing belongs to admission, not this step.
    assert_eq!(
        remove_xgmi_progress_index_v1(&mut ready, false, &mut in_flight, 4096),
        XgmiProgressIndexPhaseV1::Waiting
    );
    assert_eq!(ready, before);
}

#[test]
fn partial_restoration_and_ready_cancellation_preserve_membership() {
    let mut ready = VecDeque::with_capacity(6);
    let mut flags = [false; 6];
    for id in 1..=6 {
        index_xgmi_ready_id_v1(&mut ready, &mut flags[id as usize - 1], id, false);
    }
    for id in 1..=3 {
        assert_eq!(ready.pop_front(), Some(id));
        assert!(core::mem::replace(&mut flags[id as usize - 1], false));
    }
    for id in [3, 2] {
        index_xgmi_ready_id_v1(&mut ready, &mut flags[id as usize - 1], id, true);
    }
    assert_eq!(ready, [2, 3, 4, 5, 6]);
    let mut in_flight = Vec::new();
    assert_eq!(
        remove_xgmi_progress_index_v1(&mut ready, flags[3], &mut in_flight, 4),
        XgmiProgressIndexPhaseV1::Ready
    );
    flags[3] = false;
    assert_eq!(ready, [2, 3, 5, 6]);
    assert_eq!(flags, [false, true, true, false, true, true]);
}

#[test]
fn ready_lookup_cost_is_independent_of_fifo_backlog() {
    let mut queries = Vec::new();
    for size in [63, 64, 4096] {
        let mut rig = Rig::new();
        for id in 1..=size {
            rig.add(id, 0, &[]);
        }
        assert_eq!(select(&mut rig, size), Ok(Action::Publish(0)));
        queries.push(rig.queries.get());
    }
    assert!(queries.windows(2).all(|pair| pair[0] == pair[1]));
}

#[test]
fn unticketed_observation_tail_cannot_publish_recurse_or_enqueue() {
    let source = include_str!("../../kfd_backend.rs");
    let helper = source
        .split("    fn progress_peer_copy(")
        .nth(1)
        .unwrap()
        .split("    fn logical_resource_counts(")
        .next()
        .unwrap();
    let tail = helper
        .split("// Unpublished dependency progress")
        .nth(1)
        .unwrap();
    assert_eq!(
        tail.matches("self.active.insert(active.id, active)")
            .count(),
        1
    );
    assert!(tail.contains("Ok(BackendPollV1::Pending)"));
    for forbidden in [
        "poll_v1(",
        "publish_ready_peer_batch(",
        "progress_peer_copy(",
        "index_ready_v1(",
        "enqueue_xgmi",
    ] {
        assert!(
            !tail.contains(forbidden),
            "unexpected observation path: {forbidden}"
        );
    }
}
