use super::*;
use crate::queue_linux::{
    LinuxDoorbellErrorV1, LinuxKfdRuntimeEnabledV1,
    arm_process_global_kfd_runtime_gate_for_teardown_v1,
};

#[test]
fn multi_queue_failures_distinguish_retryable_and_terminal_truth() {
    assert_eq!(
        classify_multi_queue_preparation_failure(false, false),
        Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
    );
    assert_eq!(
        classify_multi_queue_preparation_failure(true, false),
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
    );
    assert_eq!(
        classify_multi_queue_preparation_failure(false, true),
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
    );
    assert_eq!(
        classify_multi_queue_publication_failure(0, false),
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
    );
    assert_eq!(
        classify_multi_queue_publication_failure(0, true),
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPartialPublication
    );
    assert_eq!(
        classify_multi_queue_publication_failure(1, false),
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPartialPublication
    );
}

#[test]
fn submission_disposition_terminalizer_is_exact_and_retryable_is_inert() {
    for disposition in [
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication,
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPartialPublication,
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPostPublication,
    ] {
        let local_calls = core::cell::Cell::new(0_u8);
        let process_calls = core::cell::Cell::new(0_u8);
        enforce_multi_queue_submission_disposition_v1(
            disposition,
            || local_calls.set(local_calls.get() + 1),
            || process_calls.set(process_calls.get() + 1),
        );
        assert_eq!(local_calls.get(), 1, "disposition={disposition:?}");
        assert_eq!(process_calls.get(), 1, "disposition={disposition:?}");
    }

    let local_calls = core::cell::Cell::new(0_u8);
    let process_calls = core::cell::Cell::new(0_u8);
    enforce_multi_queue_submission_disposition_v1(
        Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight,
        || local_calls.set(local_calls.get() + 1),
        || process_calls.set(process_calls.get() + 1),
    );
    assert_eq!(local_calls.get(), 0);
    assert_eq!(process_calls.get(), 0);
}

#[test]
fn already_terminal_multi_queue_session_never_advertises_retryable_custody() {
    assert_eq!(
        classify_multi_queue_availability_failure(false),
        Gfx942SdmaMultiQueueFailureDispositionV1::RetryablePreflight
    );
    assert_eq!(
        classify_multi_queue_availability_failure(true),
        Gfx942SdmaMultiQueueFailureDispositionV1::TerminalPrePublication
    );
}

#[test]
fn terminal_shard_observation_returns_the_source_slice_lifetime() {
    fn request_indices<'a>(observation: Gfx942SdmaTerminalShardObservationV1<'a>) -> &'a [u16] {
        observation.request_indices()
    }

    let indices = [1_u16, 5, 9];
    let observation = Gfx942SdmaTerminalShardObservationV1 {
        queue_ordinal: 3,
        queue_id: 17,
        request_indices: &indices,
        retained_ticket_count: indices.len(),
    };
    let retained = request_indices(observation);
    assert_eq!(retained, indices);
}

#[test]
fn injected_wait_panic_retains_the_exact_sealed_submission() {
    let submission = crate::sdma::striped_submission_for_unwind_test();
    let exact = submission.exact_identity_for_unwind_test();
    let mut retained = Some(submission);
    let caught = catch_striped_wait_epoch_unwind_v1(|| {
        assert_eq!(
            retained
                .as_ref()
                .expect("borrowed panic custody")
                .exact_identity_for_unwind_test(),
            exact
        );
        panic!("injected striped wait panic");
    });
    assert!(caught.is_err());

    let local_poisoned = core::cell::Cell::new(false);
    let process_poisoned = core::cell::Cell::new(false);
    let recovered = take_striped_wait_panic_custody_v1(&mut retained, || {
        poison_multi_queue_terminal_boundary_v1(
            || local_poisoned.set(true),
            || process_poisoned.set(true),
        );
    });
    assert!(local_poisoned.get());
    assert!(process_poisoned.get());
    assert!(retained.is_none());
    assert_eq!(recovered.exact_identity_for_unwind_test(), exact);

    let terminal = Gfx942SdmaMultiQueueTerminalCustodyV1::complete_publication(recovered);
    assert_eq!(
        terminal.exact_complete_publication_identity_for_test(),
        Some(exact)
    );
}

#[test]
fn multi_queue_terminal_boundary_is_process_global_in_a_subprocess() {
    const CHILD_ENV: &str = "FE2O3_TEST_MULTI_QUEUE_TERMINAL_POISON";
    if std::env::var_os(CHILD_ENV).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .arg("multi_queue_terminal_boundary_is_process_global_in_a_subprocess")
            .arg("--nocapture")
            .env(CHILD_ENV, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "child failed:\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }

    let teardown_arm = arm_process_global_kfd_runtime_gate_for_teardown_v1();
    let local_calls = core::cell::Cell::new(0_u8);
    poison_multi_queue_terminal_boundary_v1(
        || local_calls.set(local_calls.get() + 1),
        permanently_poison_process_global_kfd_runtime_gate_v1,
    );
    assert_eq!(local_calls.get(), 1);
    teardown_arm.confirm_destroyed();

    use std::os::fd::AsFd;
    let file = std::fs::File::open("/dev/null").unwrap();
    assert!(matches!(
        LinuxKfdRuntimeEnabledV1::enable(file.as_fd(), std::process::id()),
        Err(LinuxDoorbellErrorV1::Runtime(
            "process-global gate poisoned"
        ))
    ));
}

#[test]
fn every_process_teardown_exit_uses_one_terminalizer_and_timeout_does_not() {
    let live = include_str!("../sdma_multi_queue.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let submit = live
        .split("pub fn submit_gfx942_striped_sdma_copy_batch_v1")
        .nth(1)
        .unwrap()
        .split("pub fn poll_gfx942_striped_sdma_copy_batch_v1")
        .next()
        .unwrap();
    let poll = live
        .split("pub fn poll_gfx942_striped_sdma_copy_batch_v1")
        .nth(1)
        .unwrap()
        .split("pub fn wait_gfx942_striped_sdma_copy_batch_for_v1")
        .next()
        .unwrap();
    let wait = live
        .split("pub fn wait_gfx942_striped_sdma_copy_batch_for_v1")
        .nth(1)
        .unwrap()
        .split("pub fn execute_sdma_copy_batch_for")
        .next()
        .unwrap();
    let terminalizer = live
        .split("fn poison_gfx942_multi_queue_terminal_v1")
        .nth(1)
        .unwrap()
        .split("fn enforce_gfx942_multi_queue_submission_disposition_v1")
        .next()
        .unwrap();
    let disposition_enforcer = live
        .split("fn enforce_gfx942_multi_queue_submission_disposition_v1")
        .nth(1)
        .unwrap()
        .split("/// Preflights and then publishes")
        .next()
        .unwrap();

    assert_eq!(terminalizer.matches("self.poison_terminal()").count(), 1);
    assert_eq!(
        terminalizer
            .matches("permanently_poison_process_global_kfd_runtime_gate_v1")
            .count(),
        1
    );
    assert_eq!(
        disposition_enforcer
            .matches("enforce_multi_queue_submission_disposition_v1")
            .count(),
        1
    );
    assert_eq!(
        disposition_enforcer
            .matches("self.poison_terminal()")
            .count(),
        1
    );
    assert_eq!(
        disposition_enforcer
            .matches("permanently_poison_process_global_kfd_runtime_gate_v1")
            .count(),
        1
    );

    let submit_terminalizers = submit
        .matches("self.poison_gfx942_multi_queue_terminal_v1()")
        .count()
        + submit
            .matches("self.enforce_gfx942_multi_queue_submission_disposition_v1(disposition)")
            .count();
    assert_eq!(submit.matches("ProcessTeardown(").count(), 6);
    assert_eq!(submit_terminalizers, 6);
    assert_eq!(poll.matches("ProcessTeardown(").count(), 3);
    assert_eq!(
        poll.matches("self.poison_gfx942_multi_queue_terminal_v1()")
            .count(),
        3
    );
    assert_eq!(wait.matches("ProcessTeardown(").count(), 4);
    assert_eq!(
        wait.matches("self.poison_gfx942_multi_queue_terminal_v1()")
            .count(),
        4
    );

    for body in [submit, poll, wait] {
        assert!(!body.contains("permanently_poison_process_global_kfd_runtime_gate_v1"));
        assert!(!body.contains("self.poison_terminal()"));
    }
    let timeout = wait
        .split("Some(Gfx942SdmaStripedTailWaitOutcomeV1::Pending) => {")
        .nth(1)
        .unwrap()
        .split("Some(Gfx942SdmaStripedTailWaitOutcomeV1::Terminal(error))")
        .next()
        .unwrap();
    assert!(timeout.contains("Gfx942SdmaErrorV1::Timeout"));
    assert!(timeout.contains("Gfx942SdmaMultiQueueExecutionCustodyV1::Pending("));
    assert!(!timeout.contains("poison_gfx942_multi_queue_terminal_v1"));
}

#[test]
fn aggregate_completion_has_one_envelope_and_never_partially_retires() {
    let live = include_str!("../sdma_multi_queue.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let submit = live
        .split("pub fn submit_gfx942_striped_sdma_copy_batch_v1")
        .nth(1)
        .unwrap()
        .split("pub fn poll_gfx942_striped_sdma_copy_batch_v1")
        .next()
        .unwrap();
    assert_eq!(submit.matches("with_striped_sdma_owner_memory").count(), 1);
    assert_eq!(
        submit
            .matches("check_queue_operational_currentness")
            .count(),
        2
    );
    let opening = submit.find("check_queue_operational_currentness").unwrap();
    let publication = submit.find("submit_striped_multi_queue_batch").unwrap();
    let closing = submit.rfind("check_queue_operational_currentness").unwrap();
    let cursor = submit.find("commit_striped_multi_queue_success").unwrap();
    assert!(opening < publication && publication < closing && closing < cursor);

    let poll = live
        .split("pub fn poll_gfx942_striped_sdma_copy_batch_v1")
        .nth(1)
        .unwrap()
        .split("pub fn wait_gfx942_striped_sdma_copy_batch_for_v1")
        .next()
        .unwrap();
    assert_eq!(poll.matches("with_striped_sdma_owner_memory").count(), 1);
    assert_eq!(
        poll.matches("check_queue_operational_currentness").count(),
        2
    );
    assert!(!poll.contains("Vec::"));
    assert!(!poll.contains("try_reserve"));
    assert!(!poll.contains("collect"));
    assert!(!poll.contains("prepare_striped_multi_queue_completion"));
    let observe = poll
        .find("observe_prepared_striped_multi_queue_completion")
        .unwrap();
    let closing = poll.rfind("check_queue_operational_currentness").unwrap();
    let retire = poll
        .find("retire_prepared_striped_multi_queue_completion")
        .unwrap();
    assert!(observe < closing && closing < retire);
    assert!(!poll.contains(".striped_sdma.as_mut()"));
    assert!(poll.contains("completed_opaque(completed)"));

    let wait = live
        .split("pub fn wait_gfx942_striped_sdma_copy_batch_for_v1")
        .nth(1)
        .unwrap()
        .split("pub fn execute_sdma_copy_batch_for")
        .next()
        .unwrap();
    assert_eq!(wait.matches("with_striped_sdma_owner_memory").count(), 1);
    assert!(!wait.contains("Vec::"));
    assert!(!wait.contains("try_reserve"));
    assert!(!wait.contains("collect"));
    assert!(!wait.contains("prepare_striped_multi_queue_completion"));
    assert!(wait.contains("Gfx942SdmaErrorV1::Timeout"));
    assert!(wait.contains("Gfx942SdmaMultiQueueExecutionCustodyV1::Pending("));
    assert!(!wait.contains(".striped_sdma.as_mut()"));
    assert!(wait.contains("completed_opaque(completed)"));
    assert!(wait.contains("wait_prepared_striped_multi_queue_tails_retaining_for"));
    assert!(wait.contains("catch_striped_wait_epoch_unwind_v1"));
    assert!(wait.contains("take_striped_wait_panic_custody_v1"));
    assert!(wait.contains("poison_gfx942_multi_queue_terminal_v1"));
    assert!(!wait.contains("permanently_poison_process_global_kfd_runtime_gate_v1"));
    assert!(!wait.contains("wait_prepared_striped_multi_queue_completion_for"));
    assert!(!wait.contains("retire_prepared_striped_multi_queue_completion"));
    let lower_wait = wait
        .find("wait_prepared_striped_multi_queue_tails_retaining_for")
        .unwrap();
    assert!(wait[..lower_wait].contains("let mut retained = Some(submission)"));
    assert!(wait[lower_wait..].contains("&mut retained"));

    let optimized = include_str!("../../sdma/multi_queue/tail_wait.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let optimized_wait = optimized
        .split("pub(crate) fn wait_prepared_striped_multi_queue_tails_retaining_for")
        .nth(1)
        .unwrap()
        .split("fn observe_bound_striped_tail_v1")
        .next()
        .unwrap();
    assert_eq!(
        optimized_wait
            .matches("check_queue_operational_currentness")
            .count(),
        2
    );
    let borrow = optimized_wait
        .find("with_borrowed_striped_submission_v1")
        .unwrap();
    let audit = optimized_wait
        .find("audit_prepared_striped_multi_queue_tails_for")
        .unwrap();
    let authorize = optimized_wait.find("audit.authorize_retirement()").unwrap();
    let consume = optimized_wait.find("retained.take()").unwrap();
    let retire = optimized_wait
        .find("retire_after_striped_full_audit_no_unwind_v1")
        .unwrap();
    assert!(borrow < audit && audit < authorize);
    assert!(authorize < consume && consume < retire);
}

#[test]
fn profiled_and_ordinary_waits_share_one_custody_machine() {
    let live = include_str!("../sdma_multi_queue.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let ordinary = live
        .split("pub fn wait_gfx942_striped_sdma_copy_batch_for_v1")
        .nth(1)
        .unwrap()
        .split("pub fn wait_gfx942_striped_sdma_copy_batch_profiled_for_v1")
        .next()
        .unwrap();
    let profiled = live
        .split("pub fn wait_gfx942_striped_sdma_copy_batch_profiled_for_v1")
        .nth(1)
        .unwrap()
        .split("pub fn wait_gfx942_striped_sdma_copy_batch_profiled_with_diagnostic_spin_budget_for_v1")
        .next()
        .unwrap();
    let diagnostic = live
        .split("pub fn wait_gfx942_striped_sdma_copy_batch_profiled_with_diagnostic_spin_budget_for_v1")
        .nth(1)
        .unwrap()
        .split("fn wait_gfx942_striped_sdma_copy_batch_impl_v1")
        .next()
        .unwrap();
    let common = live
        .split("fn wait_gfx942_striped_sdma_copy_batch_impl_v1")
        .nth(1)
        .unwrap()
        .split("pub fn execute_sdma_copy_batch_for")
        .next()
        .unwrap();

    assert!(ordinary.contains("impl_v1::<false>"));
    assert!(ordinary.contains("Gfx942SdmaStripedDiagnosticSpinBudgetV1::Current"));
    assert!(profiled.contains("impl_v1::<true>"));
    assert!(profiled.contains("Gfx942SdmaStripedDiagnosticSpinBudgetV1::Current"));
    assert!(diagnostic.contains("impl_v1::<true>"));
    assert!(diagnostic.contains("diagnostic_spin_budget"));
    for wrapper in [ordinary, profiled, diagnostic] {
        assert!(!wrapper.contains("with_striped_sdma_owner_memory"));
        assert!(!wrapper.contains("retained.take()"));
        assert!(!wrapper.contains("ProcessTeardown("));
    }
    assert_eq!(common.matches("with_striped_sdma_owner_memory").count(), 1);
    assert_eq!(common.matches("ProcessTeardown(").count(), 4);
    assert_eq!(
        common
            .matches("self.poison_gfx942_multi_queue_terminal_v1()")
            .count(),
        4
    );
    assert!(common.contains("Gfx942SdmaMultiQueueExecutionCustodyV1::Pending("));
    assert!(common.contains("wait_prepared_striped_multi_queue_tails_retaining_for"));
}

#[test]
fn terminal_custody_is_observation_only_and_drop_is_physically_inert() {
    let source = include_str!("../sdma_multi_queue.rs")
        .split("#[cfg(test)]\nmod tests")
        .next()
        .unwrap();
    let terminal_impl = source
        .split("impl Gfx942SdmaMultiQueueTerminalCustodyV1")
        .nth(1)
        .unwrap()
        .split("pub enum Gfx942SdmaMultiQueueFailureCustodyV1")
        .next()
        .unwrap();

    for forbidden in [
        "pub fn into_",
        "pub fn release",
        "pub fn recycle",
        "pub fn drain",
        "pub fn ticket",
        "pub fn request(",
        "pub fn buffer",
    ] {
        assert!(
            !terminal_impl.contains(forbidden),
            "forbidden terminal API: {forbidden}"
        );
    }
    assert!(terminal_impl.contains("pub const fn plan"));
    assert!(terminal_impl.contains("pub fn confirmed_shard"));
    assert!(terminal_impl.contains("pub fn untouched_request_index"));
    assert!(!source.contains("impl Drop for Gfx942SdmaMultiQueueTerminalCustodyV1"));
    assert!(source.contains("Dropping the wrapper discards audit observations only"));
    assert!(source.contains("performs no native"));
    assert!(source.contains("CompletePublication(Gfx942SdmaMultiQueueSubmissionV1)"));
    assert!(!source.contains("let (plan, confirmed, _completion) = submission.into_parts()"));
}
