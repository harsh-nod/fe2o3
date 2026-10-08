//! Pure state, concurrency and retained-owner controls; no compiler invocation.
use super::super::*;
use super::*;
fn request() -> SourceLocalOrderRecipeRequestV1 {
    SourceLocalOrderRecipeRequestV1::create(
        "src/lib.rs",
        [1; 32],
        SourceLocalOrderOrderV1::SourceOrder,
        SourceLocalOrderRelationV1::XorBeforeOr,
        SourceLocalOrderStrengthV1::Exact,
        SourceLocalOrderSourceBindingModeV1::RebindCurrent,
    )
    .unwrap()
}
fn output() -> SourceLocalOrderRecipeOutputV1 {
    let identity = SourceLocalOrderIdentityObservationV1 {
        digest: [0; 32],
        canonical_length: 0,
    };
    SourceLocalOrderRecipeOutputV1 {
        llvm: "test-only inert stub".into(),
        created_recipe: None,
        evidence: SourceLocalOrderRecipeEvidenceV1 {
            source_initializer: [0; 4],
            source_sha256: [0; 32],
            semantic_sha256: [0; 32],
            instance_axes: [[0; 32]; 5],
            original: identity,
            input: identity,
            output: identity,
            requested_order: SourceLocalOrderOrderV1::SourceOrder,
            requested_relation: SourceLocalOrderRelationV1::XorBeforeOr,
            strength: SourceLocalOrderStrengthV1::Exact,
            source_binding_mode: SourceLocalOrderSourceBindingModeV1::RebindCurrent,
            actual_relation: SourceLocalOrderRelationV1::XorBeforeOr,
            constraint_outcome: SourceLocalOrderConstraintOutcomeV1::Honored {
                relation: SourceLocalOrderRelationV1::XorBeforeOr,
            },
            region: [0; 4],
            output_result_order: [0; 3],
            prefix_execution_bytes: [0; 256],
            transition_sha256: [0; 32],
            transition_bytes: 0,
            transition_rows: [0; 9],
            fresh_formal_counts: [0; 5],
            llvm_sha256: [0; 32],
            descriptor_sha256: [0; 32],
            recipe_sha256: [0; 32],
            canonical_work: 0,
            canonical_peak_storage: 0,
            created: true,
        },
    }
}

#[test]
fn pre_cancel_returns_typed_failure_before_input_or_frontend() {
    let token = SourceLocalOrderRecipeCancellationV1::new();
    assert_eq!(
        token.request_cancellation(),
        SourceLocalOrderRecipeCancellationRequestV1::Requested
    );
    let attempt =
        crate::run_source_local_order_recipe_driver_cancellable_v1(&[], request(), &token);
    let error = attempt.result().unwrap_err();
    assert_eq!(
        error.cancellation().unwrap().checkpoint(),
        Checkpoint::BeforeInput
    );
    assert_eq!(attempt.callback_count(), 0);
    assert_eq!(attempt.compiler_callback_count(), 0);
    assert!(!error.compiler_fatal());
    assert!(attempt.callback_stage_elapsed_v1().is_none());
    assert_eq!(token.state.load(Ordering::SeqCst), CANCELLED);
}
#[test]
fn one_claim_reuse_and_tokens_are_independent() {
    let first = SourceLocalOrderRecipeCancellationV1::new();
    let other = SourceLocalOrderRecipeCancellationV1::new();
    let gate = first.claim().ok().unwrap();
    assert!(first.claim().is_err());
    assert!(other.claim().is_ok());
    first.request_cancellation();
    assert!(gate.poll(Checkpoint::AfterImport).is_err());
    drop(gate);
    assert_eq!(first.state.load(Ordering::SeqCst), FAILED);
    assert!(first.claim().is_err());
}
#[test]
fn repeated_requests_and_terminal_success_have_closed_results() {
    use SourceLocalOrderRecipeCancellationRequestV1::*;
    let token = SourceLocalOrderRecipeCancellationV1::new();
    let gate = token.claim().ok().unwrap();
    assert_eq!(token.request_cancellation(), Requested);
    assert_eq!(token.request_cancellation(), AlreadyRequested);
    let error = gate.poll(Checkpoint::BeforeReplay).unwrap_err();
    let attempt = gate.finish_attempt(Attempt::new(request(), Err(error), 1, 1));
    assert_eq!(
        attempt
            .result()
            .unwrap_err()
            .cancellation()
            .unwrap()
            .checkpoint(),
        Checkpoint::BeforeReplay
    );
    assert_eq!(token.request_cancellation(), AlreadyRequested);
    let done = SourceLocalOrderRecipeCancellationV1::new();
    let attempt =
        done.claim()
            .ok()
            .unwrap()
            .finish_attempt(Attempt::new(request(), Ok(output()), 1, 1));
    assert!(attempt.result().is_ok());
    assert_eq!(done.request_cancellation(), TooLate);
}
#[test]
fn existing_refusal_wins_later_request_and_preserves_fatal_metadata() {
    let token = SourceLocalOrderRecipeCancellationV1::new();
    let gate = token.claim().ok().unwrap();
    let error = SourceLocalOrderRecipeFailureV1::new(
        SourceLocalOrderRecipeFailurePhaseV1::SourceCurrentness,
        "original currentness refusal".into(),
    );
    token.request_cancellation();
    let result = finish_callback(Some(Err(error)), 2, true);
    let attempt = gate.finish_attempt(Attempt::new(request(), result, 2, 1));
    let error = attempt.result().unwrap_err();
    assert_eq!(error.diagnostic(), "original currentness refusal");
    assert!(error.cancellation().is_none());
    assert!(error.compiler_fatal());
    assert_eq!(token.state.load(Ordering::SeqCst), FAILED);
}
#[test]
fn cancelled_first_result_survives_callback_reentry_and_fatal() {
    let token = SourceLocalOrderRecipeCancellationV1::new();
    let gate = token.claim().ok().unwrap();
    token.request_cancellation();
    let result = finish_callback(
        Some(Err(gate
            .poll(Checkpoint::AfterMaterialization)
            .unwrap_err())),
        2,
        true,
    );
    let attempt = gate.finish_attempt(Attempt::new(request(), result, 2, 1));
    let error = attempt.result().unwrap_err();
    assert_eq!(
        error.cancellation().unwrap().checkpoint(),
        Checkpoint::AfterMaterialization
    );
    assert!(error.compiler_fatal());
    assert_eq!(token.state.load(Ordering::SeqCst), CANCELLED);
}
#[test]
fn final_commit_cancellation_drops_provisional_output() {
    let token = SourceLocalOrderRecipeCancellationV1::new();
    let hook = |checkpoint, token: &SourceLocalOrderRecipeCancellationV1| {
        assert_eq!(checkpoint, Checkpoint::DriverCommit);
        token.request_cancellation();
    };
    let gate = token.claim().ok().unwrap().with_hook(&hook);
    let attempt = gate.finish_attempt(Attempt::new(request(), Ok(output()), 1, 1));
    assert_eq!(
        attempt
            .result()
            .unwrap_err()
            .cancellation()
            .unwrap()
            .checkpoint(),
        Checkpoint::DriverCommit
    );
    assert_eq!(token.state.load(Ordering::SeqCst), CANCELLED);
}
#[test]
fn strong_commit_request_race_has_no_success_after_winning_request() {
    for _ in 0..64 {
        let token = SourceLocalOrderRecipeCancellationV1::new();
        let gate = token.claim().ok().unwrap();
        let barrier = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let requester = scope.spawn(|| {
                barrier.wait();
                token.request_cancellation()
            });
            barrier.wait();
            let attempt = gate.finish_attempt(Attempt::new(request(), Ok(output()), 1, 1));
            match requester.join().unwrap() {
                SourceLocalOrderRecipeCancellationRequestV1::Requested => {
                    assert!(attempt.result().unwrap_err().cancellation().is_some())
                }
                SourceLocalOrderRecipeCancellationRequestV1::TooLate => {
                    assert!(attempt.result().is_ok())
                }
                SourceLocalOrderRecipeCancellationRequestV1::AlreadyRequested => {
                    panic!("only one requester")
                }
            }
        });
    }
}
#[test]
fn concurrent_claim_has_exactly_one_winner_without_reset() {
    let token = SourceLocalOrderRecipeCancellationV1::new();
    let barrier = std::sync::Barrier::new(2);
    std::thread::scope(|scope| {
        let a = scope.spawn(|| {
            barrier.wait();
            token.claim().is_ok()
        });
        barrier.wait();
        let b = token.claim().is_ok();
        assert_ne!(a.join().unwrap(), b);
    });
    assert!(token.claim().is_err());
}
#[test]
fn unwind_marks_token_terminal_without_intercepting_owner_drop() {
    let token = SourceLocalOrderRecipeCancellationV1::new();
    let dropped = std::cell::Cell::new(false);
    struct Witness<'a>(&'a std::cell::Cell<bool>);
    impl Drop for Witness<'_> {
        fn drop(&mut self) {
            self.0.set(true);
        }
    }
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _gate = token.claim().ok().unwrap();
        let _owner = Witness(&dropped);
        panic!("controlled pure unwind");
    }));
    assert!(caught.is_err());
    assert!(dropped.get());
    assert_eq!(token.state.load(Ordering::SeqCst), FAILED);
}
#[test]
fn cancellation_retained_measurement_counts_inline_field_and_no_token_owner() {
    use fe2o3_kernel_ir::{LogicalStorageErrorV1 as Error, LogicalStorageLimitsV1 as Limits};
    let token = SourceLocalOrderRecipeCancellationV1::new();
    token.request_cancellation();
    let gate = token.claim().ok().unwrap();
    let attempt = gate.finish_attempt(Attempt::new(
        request(),
        Err(cancelled(Checkpoint::BeforeInput)),
        0,
        0,
    ));
    let measured = attempt
        .retained_logical_storage_v1(Limits {
            max_bytes: None,
            max_items: 16,
        })
        .unwrap();
    assert_eq!(measured.inline_bytes, std::mem::size_of::<Attempt>());
    assert_eq!(
        measured.total_bytes,
        measured.inline_bytes
            + measured.request_owned_bytes
            + measured.failure_diagnostic_owned_bytes
    );
    assert_eq!(measured.llvm_owned_bytes, 0);
    assert_eq!(measured.created_recipe_owned_bytes, 0);
    assert_eq!(
        attempt.retained_logical_storage_v1(Limits {
            max_bytes: Some(measured.total_bytes - 1),
            max_items: measured.visited_items
        }),
        Err(Error::ByteLimit)
    );
    assert_eq!(
        attempt
            .retained_logical_storage_v1(Limits {
                max_bytes: Some(measured.total_bytes),
                max_items: measured.visited_items
            })
            .unwrap(),
        measured
    );
    // Atomic token is caller storage, not hidden in returned Attempt or heap.
    assert_eq!(
        std::mem::size_of::<SourceLocalOrderRecipeCancellationV1>(),
        std::mem::size_of::<AtomicU8>()
    );
}
#[test]
fn never_cancel_preserves_output_bytes_and_no_observation() {
    let token = SourceLocalOrderRecipeCancellationV1::new();
    let gate = token.claim().ok().unwrap();
    assert!(gate.poll(Checkpoint::AfterReplay).is_ok());
    let attempt = gate.finish_attempt(Attempt::new(request(), Ok(output()), 1, 1));
    assert_eq!(attempt.result().unwrap().llvm_ir(), "test-only inert stub");
    assert!(
        SourceLocalOrderRecipeFailureV1::new(Phase::Constraint, "ordinary".into())
            .cancellation()
            .is_none()
    );
}

#[test]
fn ordinary_failure_debug_and_diagnostic_keep_the_original_representation() {
    let error = SourceLocalOrderRecipeFailureV1::new(Phase::Constraint, "ordinary".into());
    assert_eq!(error.to_string(), "ordinary");
    assert_eq!(
        format!("{error:?}"),
        "SourceLocalOrderRecipeFailureV1 { phase: Constraint, diagnostic: \"ordinary\", compiler_fatal: false }"
    );
}
