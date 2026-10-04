#[test]
fn context_policy_observes_sequential_solver_execs_and_genuine_terminals() {
    let run = run_contexts("/bin/true; /bin/true; :", "/bin/true", None);
    let output = run.result.unwrap();
    assert_eq!(
        output.policy,
        GeneratedProofProcessPolicyV2::PinnedSingleThreadContextsV2
    );
    assert_eq!((output.exit_code, output.signal), (Some(0), None));
    assert!(run.first_descendant > 0);
    assert!(run.last_descendant > 0);
    assert_ne!(run.first_descendant, run.last_descendant);
    assert_process_disappears(run.first_descendant);
    assert_process_disappears(run.last_descendant);
}

#[test]
fn context_policy_observes_two_live_direct_solver_groups_but_refuses_a_third() {
    let run = run_contexts("/bin/sleep 1 & /bin/sleep 1 & wait; :", "/bin/sleep", None);
    assert_eq!(
        (
            run.result.as_ref().unwrap().exit_code,
            run.result.as_ref().unwrap().signal
        ),
        (Some(0), None)
    );
    assert_eq!(run.peak_executed_solver_groups, 2);
    assert_process_disappears(run.first_descendant);
    assert_process_disappears(run.last_descendant);
    // These lifetimes exceed the controller's three-second deadline. A third
    // birth cannot succeed because a preceding sleep happened to finish early.
    let run = run_contexts(
        "/bin/sleep 10 & /bin/sleep 10 & /bin/sleep 10 & wait; :",
        "/bin/sleep",
        None,
    );
    let error = expect_error(run.result);
    assert!(
        error.to_string().contains("live group bound exceeded"),
        "{error}"
    );
    assert_process_disappears(run.first_descendant);
    assert_process_disappears(run.last_descendant);
}

#[test]
fn context_policy_rejects_nested_solver_birth_and_solver_reexec() {
    let nested = run_contexts("/bin/sh -c '/bin/true; :'; :", "/bin/sh", None);
    let error = expect_error(nested.result);
    assert!(
        error
            .to_string()
            .contains("not from the authenticated verifier"),
        "{error}"
    );
    assert_process_disappears(nested.first_descendant);
    let reexec = run_contexts("/bin/sh -c 'exec /bin/true'; :", "/bin/sh", None);
    let error = expect_error(reexec.result);
    assert!(error.to_string().contains("unexpected re-exec"), "{error}");
    assert_process_disappears(reexec.first_descendant);
}

#[test]
fn context_policy_rejects_changed_solver_fds_and_failed_earlier_context() {
    let leaked = File::open("/dev/null").unwrap();
    let run = run_contexts("/bin/true; :", "/bin/true", Some(&leaked));
    let error = expect_error(run.result);
    assert!(error.to_string().contains("descriptor"), "{error}");
    let failed = run_contexts(
        "/bin/sh -c 'exit 7'; /bin/sh -c 'exit 0'; :",
        "/bin/sh",
        None,
    );
    let error = expect_error(failed.result);
    assert!(
        error.to_string().contains("did not exit successfully"),
        "{error}"
    );
    assert_process_disappears(failed.first_descendant);
}
