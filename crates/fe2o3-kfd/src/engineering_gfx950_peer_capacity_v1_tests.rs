use super::*;

fn counts(occupied: &[usize]) -> Counts {
    Counts {
        owners: occupied
            .iter()
            .map(|&buffers| OwnerCounts {
                buffers,
                next_buffer: 1,
            })
            .collect(),
        group_buffers: occupied.iter().sum(),
        group_next_buffer: 1,
    }
}

struct Mock {
    counts: Counts,
    closed: bool,
    quarantined: bool,
    checks: usize,
    finish_calls: usize,
    fail_check: Option<usize>,
    change_after_check: bool,
}

impl Mock {
    fn new(occupied: &[usize]) -> Self {
        Self {
            counts: counts(occupied),
            closed: false,
            quarantined: false,
            checks: 0,
            finish_calls: 0,
            fail_check: None,
            change_after_check: false,
        }
    }
}

impl CapacityBackend for Mock {
    fn require_active(&self) -> Result<()> {
        if self.closed || self.quarantined {
            Err("inactive".into())
        } else {
            Ok(())
        }
    }
    fn currentness_and_idle(&mut self) -> Result<()> {
        self.checks += 1;
        if self.fail_check == Some(self.checks) {
            return Err("injected currentness/idle failure".into());
        }
        if self.change_after_check && self.checks == 2 {
            self.counts.owners[0].next_buffer += 1;
        }
        Ok(())
    }
    fn counts(&self) -> Counts {
        self.counts.clone()
    }
    fn finish(&mut self, result: Result<Vec<usize>>) -> Result<Vec<usize>> {
        self.finish_calls += 1;
        if result.is_err() {
            self.quarantined = true;
        }
        result
    }
}

#[test]
fn exact_owner_capacity_succeeds_without_changing_accounting() {
    let mut mock = Mock::new(&[MAX_ALLOCATIONS - 144, MAX_ALLOCATIONS - 144]);
    let before = mock.counts.clone();
    assert_eq!(preflight(&mut mock, &[144, 144]).unwrap(), vec![1904, 1904]);
    assert_eq!(mock.counts, before);
    assert_eq!(
        (mock.checks, mock.finish_calls, mock.quarantined),
        (2, 1, false)
    );
}

#[test]
fn one_short_on_either_owner_quarantines_before_returning_counts() {
    for occupied in [[1905, 0], [0, 1905]] {
        let mut mock = Mock::new(&occupied);
        assert!(preflight(&mut mock, &[144, 144]).is_err());
        assert!(mock.quarantined);
        assert_eq!(mock.checks, 1);
        assert!(preflight(&mut mock, &[0, 0]).is_err());
        assert_eq!(mock.checks, 1);
    }
}

#[test]
fn asymmetric_counts_and_zero_growth_use_each_actual_owner() {
    let mut mock = Mock::new(&[MAX_ALLOCATIONS, 17]);
    assert_eq!(
        preflight(&mut mock, &[0, MAX_ALLOCATIONS - 17]).unwrap(),
        vec![2048, 17]
    );
    assert!(!mock.quarantined);
    assert!(preflight(&mut mock, &[1, 0]).is_err());
}

#[test]
fn group_capacity_is_checked_independently_of_owner_counts() {
    let mut mock = Mock::new(&[0, 0]);
    mock.counts.group_buffers = group_allocation_limit(2).unwrap() - 3;
    assert!(preflight(&mut mock, &[1, 2]).is_ok());
    mock.counts.group_buffers += 1;
    assert!(preflight(&mut mock, &[1, 2]).is_err());
    assert!(mock.quarantined);
}

#[test]
fn owner_and_group_id_exact_room_and_overflow() {
    let mut exact = counts(&[0, 0]);
    exact.owners[0].next_buffer = u64::MAX - 3;
    exact.group_next_buffer = u64::MAX - 5;
    assert!(validate_counts(&exact, &[3, 2]).is_ok());
    let mut owner = exact.clone();
    owner.owners[0].next_buffer += 1;
    assert!(validate_counts(&owner, &[3, 2]).is_err());
    let mut group = exact;
    group.group_next_buffer += 1;
    assert!(validate_counts(&group, &[3, 2]).is_err());
}

#[test]
fn checked_arithmetic_and_closed_world_roster_reject_malformed_counts() {
    assert!(validate_counts(&counts(&[usize::MAX, 0]), &[1, 0]).is_err());
    let mut group = counts(&[0, 0]);
    group.group_buffers = usize::MAX;
    assert!(validate_counts(&group, &[1, 0]).is_err());
    for world in [0, 1, 3, 7, 9] {
        assert!(validate_counts(&counts(&vec![0; world]), &vec![0; world]).is_err());
    }
    for additional in [&[][..], &[0][..], &[0, 0, 0][..]] {
        assert!(validate_counts(&counts(&[0, 0]), additional).is_err());
    }
}

#[test]
fn eight_owner_profile_uses_existing_group_limit_without_model_constants() {
    let occupied = vec![MAX_ALLOCATIONS - 1; 8];
    let mut mock = Mock::new(&occupied);
    assert_eq!(preflight(&mut mock, &[1; 8]).unwrap(), occupied);
    assert!(!mock.quarantined);
}

#[test]
fn entry_and_exit_currentness_failures_quarantine_without_success_or_retry() {
    for failing_check in [1, 2] {
        let mut mock = Mock::new(&[3, 9]);
        let before = mock.counts.clone();
        mock.fail_check = Some(failing_check);
        assert!(preflight(&mut mock, &[2, 3]).is_err());
        assert_eq!(mock.counts, before);
        assert!(mock.quarantined);
        assert_eq!((mock.checks, mock.finish_calls), (failing_check, 1));
        assert!(preflight(&mut mock, &[0, 0]).is_err());
        assert_eq!((mock.checks, mock.finish_calls), (failing_check, 1));
    }
}

#[test]
fn changed_accounting_during_exit_fence_is_terminal() {
    let mut mock = Mock::new(&[3, 9]);
    mock.change_after_check = true;
    assert!(preflight(&mut mock, &[2, 3]).is_err());
    assert!(mock.quarantined);
    assert_eq!(mock.checks, 2);
}

#[test]
fn already_closed_or_quarantined_owner_does_not_observe_or_reopen() {
    for closed in [false, true] {
        let mut mock = Mock::new(&[0, 0]);
        mock.closed = closed;
        mock.quarantined = !closed;
        assert!(preflight(&mut mock, &[0, 0]).is_err());
        assert_eq!((mock.checks, mock.finish_calls), (0, 0));
    }
}

#[test]
fn public_method_marks_real_empty_owner_quarantined_without_device_access() {
    let mut group = Gfx950EngineeringPeerGroupV1 {
        incarnation: 1,
        contexts: Vec::new(),
        buffers: BTreeMap::new(),
        next_buffer: 1,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    };
    assert!(group.preflight_additional_allocations_v1(&[]).is_err());
    assert!(group.poisoned);
    assert!(group.preflight_additional_allocations_v1(&[]).is_err());
    assert!(group.buffers.is_empty() && group.contexts.is_empty());
}

struct Tracing {
    inner: Mock,
    events: std::cell::RefCell<Vec<&'static str>>,
}
impl Tracing {
    fn new() -> Self {
        Self {
            inner: Mock::new(&[3, 9]),
            events: std::cell::RefCell::new(Vec::new()),
        }
    }
}
impl CapacityBackend for Tracing {
    fn require_active(&self) -> Result<()> {
        self.events.borrow_mut().push("active");
        self.inner.require_active()
    }
    fn currentness_and_idle(&mut self) -> Result<()> {
        self.events.borrow_mut().push("fence");
        self.inner.currentness_and_idle()
    }
    fn counts(&self) -> Counts {
        self.events.borrow_mut().push("complete_counts");
        self.inner.counts()
    }
    fn finish(&mut self, result: Result<Vec<usize>>) -> Result<Vec<usize>> {
        self.events.borrow_mut().push("finish");
        self.inner.finish(result)
    }
}
fn drift_field(value: &mut Counts, field: usize) {
    match field {
        0 => value.owners[0].buffers += 1,
        1 => value.owners[0].next_buffer += 1,
        2 => value.owners[1].buffers += 1,
        3 => value.owners[1].next_buffer += 1,
        4 => value.group_buffers += 1,
        5 => value.group_next_buffer += 1,
        _ => panic!("invalid test field"),
    }
}
struct Drifting {
    inner: Mock,
    field: usize,
}
impl CapacityBackend for Drifting {
    fn require_active(&self) -> Result<()> {
        self.inner.require_active()
    }
    fn currentness_and_idle(&mut self) -> Result<()> {
        self.inner.currentness_and_idle()?;
        if self.inner.checks == 2 {
            drift_field(&mut self.inner.counts, self.field);
        }
        Ok(())
    }
    fn counts(&self) -> Counts {
        self.inner.counts()
    }
    fn finish(&mut self, result: Result<Vec<usize>>) -> Result<Vec<usize>> {
        self.inner.finish(result)
    }
}

#[test]
fn capacity_snapshot_reuses_legacy_preflight_order_and_actual_asymmetric_owners() {
    let trace = [
        "active",
        "fence",
        "complete_counts",
        "fence",
        "complete_counts",
        "finish",
    ];
    let mut legacy = Tracing::new();
    assert_eq!(preflight(&mut legacy, &[0, 0]).unwrap(), vec![3, 9]);
    assert_eq!(*legacy.events.borrow(), trace);
    let mut scoped = Tracing::new();
    let snapshot = preflight_snapshot(&mut scoped, &[0, 0]).unwrap();
    assert_eq!(snapshot.owner_counts().unwrap(), [3, 9]);
    assert_eq!(*scoped.events.borrow(), trace);
    assert_eq!(snapshot.counts, scoped.inner.counts);
}
#[test]
fn capacity_snapshot_refuses_each_owner_and_group_field_drift_within_either_fence_pair() {
    for field in 0..6 {
        let mut backend = Drifting {
            inner: Mock::new(&[3, 9]),
            field,
        };
        assert!(preflight_snapshot(&mut backend, &[0, 0]).is_err());
        assert!(backend.inner.quarantined);
        assert_eq!((backend.inner.checks, backend.inner.finish_calls), (2, 1));
        assert!(preflight_snapshot(&mut backend, &[0, 0]).is_err());
        assert_eq!(backend.inner.checks, 2);
    }
}
#[test]
fn capacity_snapshot_equality_retains_ids_and_group_accounting_across_calls() {
    for field in 0..6 {
        let mut backend = Mock::new(&[3, 9]);
        let before = preflight_snapshot(&mut backend, &[0, 0]).unwrap();
        drift_field(&mut backend.counts, field);
        let after = preflight_snapshot(&mut backend, &[0, 0]).unwrap();
        assert_ne!(before, after, "field {field}");
        if ![0, 2].contains(&field) {
            assert_eq!(
                before.owner_counts().unwrap(),
                after.owner_counts().unwrap()
            );
        }
    }
}
#[test]
fn capacity_snapshot_zero_add_keeps_actual_limits_and_legacy_id_edge_semantics() {
    let mut exact = Mock::new(&[MAX_ALLOCATIONS, 17]);
    exact.counts.group_buffers = group_allocation_limit(2).unwrap();
    exact.counts.group_next_buffer = u64::MAX;
    exact.counts.owners[0].next_buffer = u64::MAX;
    assert!(preflight_snapshot(&mut exact, &[0, 0]).is_ok());
    assert!(preflight(&mut exact, &[1, 0]).is_err());
    for owner in [0, 1] {
        let mut invalid = Mock::new(&[0, 0]);
        invalid.counts.owners[owner].buffers = MAX_ALLOCATIONS + 1;
        assert!(preflight_snapshot(&mut invalid, &[0, 0]).is_err());
        assert!(invalid.quarantined);
    }
    let mut invalid = Mock::new(&[3, 9]);
    invalid.counts.group_buffers = group_allocation_limit(2).unwrap() + 1;
    assert!(preflight_snapshot(&mut invalid, &[0, 0]).is_err());
    assert!(invalid.quarantined);
}
#[test]
fn capacity_snapshot_scoped_owner_shape_does_not_change_eight_owner_legacy_admission() {
    let mut backend = Mock::new(&[1; 8]);
    let snapshot = preflight_snapshot(&mut backend, &[0; 8]).unwrap();
    assert!(snapshot.owner_counts().is_err());
    assert_eq!(preflight(&mut backend, &[0; 8]).unwrap(), vec![1; 8]);
    assert!(!backend.quarantined);
}
#[test]
fn capacity_snapshot_refuses_full_route_instead_of_silently_selecting_scoped_policy() {
    let mut group = Gfx950EngineeringPeerGroupV1 {
        incarnation: 1,
        contexts: Vec::new(),
        buffers: BTreeMap::new(),
        next_buffer: 1,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
        projection_mlp_scratch: None,
    };
    let mut route = scoped_currentness::Currentness::Full;
    assert!(scoped_preflight(&mut group, &mut route).is_err());
    assert!(group.poisoned);
    assert!(group.contexts.is_empty() && group.buffers.is_empty());
}
