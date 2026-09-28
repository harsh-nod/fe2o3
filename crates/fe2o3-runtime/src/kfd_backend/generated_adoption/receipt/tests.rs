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
