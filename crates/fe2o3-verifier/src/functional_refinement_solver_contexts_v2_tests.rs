use super::*;

#[test]
fn closed_fill_enforces_exact_total_and_live_caps_with_sticky_refusal() {
    let policy = GeneratedProofProcessPolicyV2::ClosedConditionalFillV1;
    assert_eq!(policy.max_total(), 12);
    assert_eq!(policy.max_live(), 2);
    let mut census = SolverContextsV2::with_policy(policy, false);
    for _ in 0..12 {
        start(&mut census, 10);
        census
            .terminal(TraceeRole::Solver, 10, true, 0, false)
            .unwrap();
    }
    assert_eq!(
        (census.born, census.executed, census.completed),
        (12, 12, 12)
    );
    census.finish().unwrap();
    assert!(census.birth(TraceeRole::Verifier, 10).is_err());
    assert_eq!(
        (census.born, census.executed, census.completed),
        (12, 12, 12)
    );
    assert!(census.finish().is_err());

    let mut census = SolverContextsV2::with_policy(policy, false);
    census.birth(TraceeRole::Verifier, 10).unwrap();
    census.birth(TraceeRole::Verifier, 20).unwrap();
    assert_eq!((census.born, census.executed), (2, 0));
    assert!(census.birth(TraceeRole::Verifier, 30).is_err());
    assert_eq!((census.born, census.executed), (2, 0));
    assert!(census.executed(10, TraceeRole::Solver).is_err());
    assert!(census.finish().is_err());
}

#[test]
fn closed_fill_authenticates_auxiliary_separately_and_refuses_incomplete_solver_census() {
    let policy = GeneratedProofProcessPolicyV2::ClosedConditionalFillV1;
    let mut census = SolverContextsV2::with_policy(policy, true);
    census.birth(TraceeRole::Verifier, 5).unwrap();
    census.executed(5, TraceeRole::AuxiliaryVerifier).unwrap();
    census
        .terminal(TraceeRole::AuxiliaryVerifier, 5, true, 0, false)
        .unwrap();
    assert_eq!((census.born, census.executed, census.completed), (0, 0, 0));
    for _ in 0..12 {
        start(&mut census, 10);
        census
            .terminal(TraceeRole::Solver, 10, true, 0, false)
            .unwrap();
    }
    census.finish().unwrap();

    let mut incomplete = SolverContextsV2::with_policy(policy, false);
    start(&mut incomplete, 10);
    assert!(incomplete.finish().is_err());
    assert!(
        incomplete
            .terminal(TraceeRole::Solver, 10, true, 0, false)
            .is_err()
    );
}

#[test]
fn interpreter_stack_policy_keeps_pending_live_total_and_sticky_terminal_census() {
    let policy = GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV3;
    assert_eq!(policy.max_live(), MAX_LIVE_CONTEXTS_V2);
    let mut census = SolverContextsV2::with_policy(policy, false);
    for _ in 0..4096 {
        start(&mut census, 10);
        census
            .terminal(TraceeRole::Solver, 10, true, 0, false)
            .unwrap();
    }
    census.finish().unwrap();
    assert!(census.birth(TraceeRole::Verifier, 10).is_err());
    assert!(census.finish().is_err());
    let mut census = SolverContextsV2::with_policy(policy, false);
    census.birth(TraceeRole::Verifier, 10).unwrap();
    census.birth(TraceeRole::Verifier, 20).unwrap();
    assert_eq!((census.born, census.executed), (2, 0));
    assert!(census.birth(TraceeRole::Verifier, 30).is_err());
    assert!(census.finish().is_err());
    let mut census = SolverContextsV2::with_policy(policy, false);
    start(&mut census, 10);
    start(&mut census, 20);
    assert!(
        census
            .terminal(TraceeRole::Solver, 10, true, 1 << 8, false)
            .is_err()
    );
    assert!(
        census
            .terminal(TraceeRole::Solver, 20, true, 0, false)
            .is_err()
    );
    assert!(census.finish().is_err());
}

#[test]
fn context_terminal_diagnostic_preserves_zero_and_incomplete_census_refusal() {
    for born in 0..=2 {
        let mut census = SolverContextsV2::new(false);
        for index in 0..born {
            census.birth(TraceeRole::Verifier, 10 + index).unwrap();
        }
        if born == 2 {
            census.executed(10, TraceeRole::Solver).unwrap();
        }
        let error = census.finish().unwrap_err();
        assert_eq!(
            error.kind(),
            RetainedFunctionalRefinementRuntimeErrorKindV1::Process
        );
        let executed = usize::from(born == 2);
        assert!(error.detail().contains(&format!(
            "born={born} executed={executed} completed=0 live={born}"
        )));
        assert!(error.detail().contains("prior_failed=false auxiliary_required=false auxiliary_complete=true auxiliary_pending=false"));
        assert!(census.failed);
        assert!(
            census
                .finish()
                .unwrap_err()
                .detail()
                .contains("prior_failed=true")
        );
    }
}

fn start(census: &mut SolverContextsV2, pid: i32) {
    census.birth(TraceeRole::Verifier, pid).unwrap();
    census.executed(pid, TraceeRole::Solver).unwrap();
}

fn terminal_task(role: TraceeRole, group: i32, leader: bool, status: i32) -> Tracee {
    let mut task = Tracee::pending(role, group, leader);
    task.exit_boundary = Some(ExitBoundary::Group(status));
    task.terminal_consumed = true;
    task.queued_status = Some(status);
    task
}

#[test]
fn context_policy_counts_pending_births_and_refuses_a_third_live_group() {
    let mut census = SolverContextsV2::new(false);
    census.birth(TraceeRole::Verifier, 10).unwrap();
    census.birth(TraceeRole::Verifier, 20).unwrap();
    assert_eq!((census.born, census.executed, census.completed), (2, 0, 0));
    assert!(census.before_birth(TraceeRole::Verifier).is_err());
    assert!(census.executed(10, TraceeRole::Solver).is_err());
    assert!(census.finish().is_err());
}

#[test]
fn context_policy_finished_observation_is_reset_and_never_accepts_an_incomplete_census() {
    reset_solver_context_observation();
    assert_eq!(finished_solver_contexts_for_test(), None);
    let mut census = SolverContextsV2::new(false);
    start(&mut census, 10);
    census
        .terminal(TraceeRole::Solver, 10, true, 0, false)
        .unwrap();
    census.finish().unwrap();
    assert_eq!(finished_solver_contexts_for_test(), Some((1, 1, 1, 1)));
    assert_eq!(
        std::thread::spawn(finished_solver_contexts_for_test)
            .join()
            .unwrap(),
        None
    );
    reset_solver_context_observation();
    let mut incomplete = SolverContextsV2::new(false);
    incomplete.birth(TraceeRole::Verifier, 20).unwrap();
    assert!(incomplete.finish().is_err());
    assert_eq!(finished_solver_contexts_for_test(), None);
}

#[test]
fn context_policy_total_boundary_is_exact_and_denial_is_sticky() {
    let mut census = SolverContextsV2::new(false);
    for _ in 0..4_096 {
        start(&mut census, 10);
        census
            .terminal(TraceeRole::Solver, 10, true, 0, false)
            .unwrap();
    }
    assert_eq!(
        (census.born, census.executed, census.completed),
        (4_096, 4_096, 4_096)
    );
    census.finish().unwrap();
    assert!(census.birth(TraceeRole::Verifier, 10).is_err());
    assert!(census.finish().is_err());
}

#[test]
fn context_policy_refuses_nested_births_and_reexec_or_role_substitution() {
    for role in [
        TraceeRole::PendingExecutable,
        TraceeRole::Solver,
        TraceeRole::AuxiliaryVerifier,
    ] {
        let mut census = SolverContextsV2::new(false);
        assert!(census.birth(role, 10).is_err());
        assert_eq!(census.born, 0);
        assert!(census.before_birth(TraceeRole::Verifier).is_err());
    }
    let mut census = SolverContextsV2::new(false);
    start(&mut census, 10);
    assert!(census.executed(10, TraceeRole::Solver).is_err());
    let mut census = SolverContextsV2::new(true);
    census.birth(TraceeRole::Verifier, 10).unwrap();
    assert!(census.executed(10, TraceeRole::Solver).is_err());
}

#[test]
fn context_policy_requires_successful_auxiliary_terminal_before_solver_birth() {
    let mut census = SolverContextsV2::new(true);
    census.birth(TraceeRole::Verifier, 10).unwrap();
    assert_eq!(
        census.expected_exec(10).unwrap(),
        TraceeRole::AuxiliaryVerifier
    );
    census.executed(10, TraceeRole::AuxiliaryVerifier).unwrap();
    census
        .terminal(TraceeRole::AuxiliaryVerifier, 10, true, 0, false)
        .unwrap();
    assert_eq!((census.born, census.executed, census.completed), (0, 0, 0));
    start(&mut census, 20);
    census
        .terminal(TraceeRole::Solver, 20, true, 0, false)
        .unwrap();
    census.finish().unwrap();
    let mut blocked = SolverContextsV2::new(true);
    blocked.birth(TraceeRole::Verifier, 10).unwrap();
    assert!(blocked.birth(TraceeRole::Verifier, 20).is_err());
}

#[test]
fn context_policy_failed_earlier_group_cannot_be_overwritten_by_later_success() {
    let mut census = SolverContextsV2::new(false);
    start(&mut census, 10);
    start(&mut census, 20);
    assert!(
        census
            .terminal(TraceeRole::Solver, 10, true, 7 << 8, false)
            .is_err()
    );
    assert!(
        census
            .terminal(TraceeRole::Solver, 20, true, 0, false)
            .is_err()
    );
    assert_eq!(census.completed, 0);
    assert!(census.finish().is_err());
}

#[test]
fn context_queued_terminals_require_each_thread_and_allow_only_retired_pid_reuse() {
    for leader_first in [true, false] {
        let mut census = SolverContextsV2::new(false);
        start(&mut census, 10);
        let mut tree = Tracees::new().unwrap();
        let first = if leader_first { 10 } else { 11 };
        let last = if leader_first { 11 } else { 10 };
        let leader = terminal_task(TraceeRole::Solver, 10, true, 0);
        let sibling = terminal_task(TraceeRole::Solver, 10, false, 0);
        tree.insert(10, leader).unwrap();
        tree.insert(11, sibling).unwrap();
        tree.get_mut(&last).unwrap().queued_status = None;
        census.retire_queued(&mut tree).unwrap();
        assert!(!tree.contains_key(&first));
        assert_eq!(census.completed, 0);
        assert_eq!(census.solvers.iter().flatten().count(), 1);
        tree.get_mut(&last).unwrap().queued_status = Some(0);
        census.retire_queued(&mut tree).unwrap();
        assert!(tree.is_empty());
        assert_eq!(census.completed, 1);
        start(&mut census, 10);
        tree.insert(10, terminal_task(TraceeRole::Solver, 10, true, 0))
            .unwrap();
        census.retire_queued(&mut tree).unwrap();
        assert_eq!((census.born, census.executed, census.completed), (2, 2, 2));
        census.finish().unwrap();
    }
}

#[test]
fn context_queued_terminal_refuses_missing_boundary_pending_exec_and_prior_failure() {
    for fault in 0..3 {
        let mut census = SolverContextsV2::new(false);
        census.birth(TraceeRole::Verifier, 10).unwrap();
        if fault != 1 {
            census.executed(10, TraceeRole::Solver).unwrap();
        }
        let mut tree = Tracees::new().unwrap();
        let status = if fault == 2 { 7 << 8 } else { 0 };
        let mut task = terminal_task(
            if fault == 1 {
                TraceeRole::PendingExecutable
            } else {
                TraceeRole::Solver
            },
            10,
            true,
            status,
        );
        if fault == 0 {
            task.exit_boundary = None;
        }
        tree.insert(10, task).unwrap();
        let result = if fault == 1 {
            census.retire_terminal(&mut tree, 10, status)
        } else {
            census.retire_queued(&mut tree)
        };
        assert!(result.is_err());
        assert!(census.birth(TraceeRole::Verifier, 20).is_err());
        assert_eq!(census.completed, 0);
        assert!(census.finish().is_err());
    }
}
