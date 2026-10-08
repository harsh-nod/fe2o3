use super::*;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Token(Arc<AtomicUsize>);
impl Drop for Token {
    fn drop(&mut self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn token() -> (Token, Arc<AtomicUsize>) {
    let drops = Arc::new(AtomicUsize::new(0));
    (Token(drops.clone()), drops)
}

fn lower_error() -> fe2o3_kfd::ComputeAqlQueueSessionErrorV1 {
    fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract("injected receipt failure")
}

#[test]
fn retirement_observation_never_polls_ready_or_retry_ready_originals() {
    for mut receipt in [
        ReceiptV1::<(), ()>::Ready,
        ReceiptV1::RetryReady,
        ReceiptV1::Recycled,
    ] {
        let checks = std::cell::Cell::new(0);
        let accepted = observe_generated_retirement_v1(
            &mut receipt,
            |owner| {
                checks.set(checks.get() + 1);
                owner.retirement().is_some()
            },
            |_| -> Result<(), ()> { panic!("ready owner must not enter native progress") },
        )
        .unwrap();
        assert!(accepted);
        assert_eq!(checks.get(), 2);
        // Legacy retirement accepts Recycled; unpublished cancellation does not.
        assert_eq!(
            receipt.issue_ready(),
            !matches!(receipt, ReceiptV1::Recycled)
        );
    }
}

#[test]
fn retirement_observation_preserves_two_steps_final_check_and_original_error() {
    for pending in [false, true] {
        let trace = std::cell::RefCell::new(Vec::new());
        let mut receipt = ReceiptV1::Published(());
        let result = observe_generated_retirement_v1(
            &mut receipt,
            |owner| {
                trace.borrow_mut().push("ready");
                owner.retirement().is_some()
            },
            |owner| {
                trace.borrow_mut().push("progress");
                if !pending {
                    *owner = match owner {
                        ReceiptV1::Published(()) => ReceiptV1::Completed(()),
                        ReceiptV1::Completed(()) => ReceiptV1::Recycled,
                        _ => panic!("unexpected progress"),
                    };
                }
                Ok::<_, ()>(())
            },
        )
        .unwrap();
        assert_eq!(result, !pending);
        assert_eq!(
            *trace.borrow(),
            ["ready", "progress", "ready", "progress", "ready"]
        );
        assert!(!receipt.issue_ready());
    }
    let mut receipt = ReceiptV1::<(), ()>::HandedToLower(HandoffV1::Issue);
    let checks = std::cell::Cell::new(0);
    assert_eq!(
        observe_generated_retirement_v1(
            &mut receipt,
            |owner| {
                checks.set(checks.get() + 1);
                owner.retirement().is_some()
            },
            |_| Err(19)
        ),
        Err(19)
    );
    assert_eq!(checks.get(), 1);
    assert!(matches!(
        receipt,
        ReceiptV1::HandedToLower(HandoffV1::Issue)
    ));
}

#[test]
fn returned_publication_is_rooted_before_closing_unwind() {
    let (batch, drops) = token();
    let mut receipt: ReceiptV1<Token, Token> = ReceiptV1::Ready;
    let result = catch_unwind(AssertUnwindSafe(|| {
        receipt.issue(|| Ok(batch)).unwrap();
        panic!("closing lane/currentness failure");
    }));
    assert!(result.is_err());
    assert!(matches!(receipt, ReceiptV1::Published(_)));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    let again = catch_unwind(AssertUnwindSafe(|| {
        receipt.issue(|| panic!("must not reissue"))
    }));
    assert!(again.is_err());
    assert!(matches!(receipt, ReceiptV1::Published(_)));
    drop(receipt);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn explicit_before_effect_retry_preserves_cancelled_retirement_then_exact_publication() {
    let mut receipt: ReceiptV1<Token, Token> = ReceiptV1::Ready;
    assert_eq!(receipt.retirement(), Some(RetirementV1::Pristine));
    for _ in 0..3 {
        receipt.issue(|| Err(Gfx942FixedDispatchSubmissionFailureV1::RetryableBeforeSideEffect(lower_error()))).unwrap();
        assert!(matches!(receipt, ReceiptV1::RetryReady));
        assert!(receipt.issue_ready());
        assert_eq!(receipt.retirement(), Some(RetirementV1::CancelledOnly));
    }
    let (batch, drops) = token();
    receipt.issue(|| Ok(batch)).unwrap();
    assert!(!receipt.issue_ready());
    assert_eq!(receipt.retirement(), None);
    let ReceiptV1::Published(batch) = &receipt else {
        panic!("missing returned publication")
    };
    assert!(Arc::ptr_eq(&batch.0, &drops));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    drop(receipt);
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn rejected_or_terminal_issue_never_permits_retry_or_retirement() {
    for (mut receipt, terminal) in [
        (ReceiptV1::<(), ()>::Ready, false),
        (ReceiptV1::Ready, true),
        (ReceiptV1::RetryReady, false),
        (ReceiptV1::RetryReady, true),
    ] {
        assert!(
            receipt
                .issue(|| Err(if terminal {
                    Gfx942FixedDispatchSubmissionFailureV1::Terminal(lower_error())
                } else {
                    Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(lower_error())
                }))
                .is_err()
        );
        assert!(matches!(
            receipt,
            ReceiptV1::HandedToLower(HandoffV1::Issue)
        ));
        assert!(!receipt.issue_ready());
        assert_eq!(receipt.retirement(), None);
    }
}

#[test]
fn preserved_rejection_is_neither_retry_custody_nor_generic_retirement() {
    for (mut receipt, expected) in [
        (ReceiptV1::<(), ()>::Ready, RetirementV1::Pristine),
        (ReceiptV1::RetryReady, RetirementV1::CancelledOnly),
    ] {
        receipt
            .issue_with_rejection(true, || {
                Err(Gfx942FixedDispatchSubmissionFailureV1::RejectedBeforeSideEffect(lower_error()))
            })
            .unwrap();
        let (prior, error) = receipt.rejected().unwrap();
        assert_eq!(prior, expected);
        assert!(matches!(
            error,
            fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract("injected receipt failure")
        ));
        assert!(!receipt.issue_ready());
        assert_eq!(receipt.retirement(), None);
        assert!(receipt.rejected_disposed_error().is_none());
        // Synthetic state-transition test only: production calls this private
        // transition after real control/DATA disposal and closing currentness.
        receipt.mark_rejected_disposed();
        assert!(receipt.rejected().is_none());
        assert!(matches!(
            receipt.rejected_disposed_error(),
            Some(fe2o3_kfd::ComputeAqlQueueSessionErrorV1::Contract(
                "injected receipt failure"
            ))
        ));
        assert!(!receipt.issue_ready());
        assert_eq!(receipt.retirement(), None);
    }
}

#[test]
fn rejection_preservation_does_not_absorb_terminal_or_unwinding_issue() {
    for panic in [false, true] {
        let mut receipt = ReceiptV1::<(), ()>::Ready;
        let result = catch_unwind(AssertUnwindSafe(|| {
            receipt.issue_with_rejection(true, || {
                assert!(!panic, "injected publication ambiguity");
                Err(Gfx942FixedDispatchSubmissionFailureV1::Terminal(
                    lower_error(),
                ))
            })
        }));
        if panic {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(matches!(
            receipt,
            ReceiptV1::HandedToLower(HandoffV1::Issue)
        ));
        assert!(receipt.rejected().is_none());
        assert!(receipt.rejected_disposed_error().is_none());
        assert!(!receipt.issue_ready());
        assert_eq!(receipt.retirement(), None);
    }
}

#[test]
fn aggregate_descriptive_rejection_cannot_use_singleton_disposal_boundary() {
    // Pure metadata substitution, with no queue, DATA or native receipt.
    let aggregate = NativeReceiptV1::Cohort3(ReceiptV1::RejectedUnpublished {
        prior: RetirementV1::Pristine,
        error: lower_error(),
    });
    assert_eq!(
        aggregate.profile(),
        crate::generated_source::GeneratedProfileV1::NativeFillCohort3
    );
    assert_eq!(aggregate.rejected_retirement(), None);
    assert!(!aggregate.rejected_disposed());
    assert_eq!(aggregate.retirement(), None);
    assert!(!aggregate.issue_ready());
    let disposed = NativeReceiptV1::Cohort3(ReceiptV1::RejectedDisposed(lower_error()));
    assert!(!disposed.rejected_disposed());
    assert_eq!(disposed.rejected_retirement(), None);
    assert_eq!(disposed.retirement(), None);
}

#[test]
fn publication_panic_retains_unknown_handoff_not_a_retry_permit() {
    for mut receipt in [ReceiptV1::<(), ()>::Ready, ReceiptV1::RetryReady] {
        assert!(
            catch_unwind(AssertUnwindSafe(
                || receipt.issue(|| panic!("publication unknown"))
            ))
            .is_err()
        );
        assert!(matches!(
            receipt,
            ReceiptV1::HandedToLower(HandoffV1::Issue)
        ));
        assert!(!receipt.issue_ready());
        assert_eq!(receipt.retirement(), None);
    }
}

#[test]
fn retirement_rejects_every_live_or_unknown_handoff_state() {
    for receipt in [
        ReceiptV1::Published(()),
        ReceiptV1::Completed(()),
        ReceiptV1::HandedToLower(HandoffV1::Issue),
        ReceiptV1::HandedToLower(HandoffV1::Poll),
        ReceiptV1::HandedToLower(HandoffV1::Recycle),
    ] {
        assert_eq!(receipt.retirement(), None);
        assert!(!receipt.issue_ready());
    }
    assert_eq!(
        ReceiptV1::<(), ()>::Recycled.retirement(),
        Some(RetirementV1::Recycled)
    );
    assert!(!ReceiptV1::<(), ()>::Recycled.issue_ready());
}

#[test]
fn returned_pending_and_completed_tokens_survive_closing_unwind() {
    for completed in [false, true] {
        let (batch, drops) = token();
        let mut receipt = ReceiptV1::Published(batch);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                receipt
                    .poll::<()>(|batch| {
                        Ok(if completed {
                            PollV1::Completed(batch)
                        } else {
                            PollV1::Pending(batch)
                        })
                    })
                    .unwrap();
                panic!("closing observation failure");
            }))
            .is_err()
        );
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        if completed {
            assert!(matches!(receipt, ReceiptV1::Completed(_)));
        } else {
            assert!(matches!(receipt, ReceiptV1::Published(_)));
        }
        drop(receipt);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}

#[test]
fn consuming_poll_failure_retains_exact_lower_handoff() {
    for panic in [false, true] {
        let (batch, drops) = token();
        let mut receipt: ReceiptV1<Token, Token> = ReceiptV1::Published(batch);
        let mut lower = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            receipt.poll(|batch| {
                lower = Some(batch);
                if panic {
                    panic!("lower poll panic");
                }
                Err(())
            })
        }));
        assert!(if panic {
            result.is_err()
        } else {
            result.unwrap().is_err()
        });
        assert!(matches!(receipt, ReceiptV1::HandedToLower(HandoffV1::Poll)));
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert!(lower.is_some());
    }
}

#[test]
fn recycle_retains_returned_completed_before_reporting_error() {
    let (completed, drops) = token();
    let mut receipt: ReceiptV1<Token, Token> = ReceiptV1::Completed(completed);
    assert!(
        !receipt
            .recycle(|completed| Err(((), Some(completed))))
            .unwrap()
    );
    assert!(matches!(receipt, ReceiptV1::Completed(_)));
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    receipt
        .recycle::<()>(|completed| {
            drop(completed);
            Ok(())
        })
        .unwrap();
    assert!(matches!(receipt, ReceiptV1::Recycled));
    assert_eq!(drops.load(Ordering::SeqCst), 1);
}

#[test]
fn recycle_failure_without_returned_owner_never_fabricates_completion() {
    for panic in [false, true] {
        let (completed, drops) = token();
        let mut receipt: ReceiptV1<Token, Token> = ReceiptV1::Completed(completed);
        let mut lower = None;
        let result = catch_unwind(AssertUnwindSafe(|| {
            receipt.recycle(|completed| {
                lower = Some(completed);
                if panic {
                    panic!("lower recycle panic");
                }
                Err(((), None))
            })
        }));
        assert!(if panic {
            result.is_err()
        } else {
            result.unwrap().is_err()
        });
        assert!(matches!(
            receipt,
            ReceiptV1::HandedToLower(HandoffV1::Recycle)
        ));
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        assert!(lower.is_some());
    }
}
