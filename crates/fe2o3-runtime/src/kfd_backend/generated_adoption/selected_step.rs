//! One original receipt step, shared by closed resident and arena profiles.

use super::*;
use fe2o3_kfd::{ComputeAqlQueueSessionErrorV1 as Error, Gfx942FixedDispatchSubmissionFailureV1};

#[cfg(test)]
mod tests;

pub(super) enum SelectedPoll<B, C> {
    Pending(B),
    Ready(C),
}
pub(super) struct SelectedPollFailure<B> {
    pub(super) error: Error,
    pub(super) refused: Option<B>,
}

pub(super) fn step<S, B, C>(
    session: &mut S,
    receipt: &mut ReceiptV1<B, C>,
    index: usize,
    submit: impl FnOnce(&mut S, usize) -> Result<B, Gfx942FixedDispatchSubmissionFailureV1>,
    poll: impl FnOnce(&mut S, B) -> Result<SelectedPoll<B, C>, SelectedPollFailure<B>>,
    recycle: impl FnOnce(&mut S, C) -> Result<(), (Error, Option<C>)>,
) -> Result<(), Error> {
    match receipt {
        ReceiptV1::Ready | ReceiptV1::RetryReady => receipt.issue(|| submit(session, index)),
        ReceiptV1::Published(_) => {
            let ReceiptV1::Published(batch) =
                core::mem::replace(receipt, ReceiptV1::HandedToLower(receipt::HandoffV1::Poll))
            else {
                std::process::abort();
            };
            match poll(session, batch) {
                Ok(SelectedPoll::Pending(batch)) => {
                    *receipt = ReceiptV1::Published(batch);
                    Ok(())
                }
                Ok(SelectedPoll::Ready(completed)) => {
                    *receipt = ReceiptV1::Completed(completed);
                    Ok(())
                }
                Err(failure) => {
                    if let Some(original) = failure.refused {
                        *receipt = ReceiptV1::Published(original);
                    }
                    Err(failure.error)
                }
            }
        }
        ReceiptV1::Completed(_) => receipt
            .recycle(|completed| recycle(session, completed))
            .map(|_| ()),
        ReceiptV1::Recycled => Ok(()),
        // Resident/Arena issue never opts into singleton classified refusal.
        // Preserve unexpected original custody for terminal teardown.
        ReceiptV1::RejectedUnpublished { .. } | ReceiptV1::RejectedDisposed(_) => Err(
            Error::Contract("registry cannot consume singleton rejected-publication custody"),
        ),
        ReceiptV1::HandedToLower(_) => Err(Error::Contract("registry unknown receipt")),
    }
}
