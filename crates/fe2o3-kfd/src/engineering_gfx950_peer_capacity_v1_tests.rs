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
