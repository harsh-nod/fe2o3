//! The original typed batch is retained inline, never split into member receipts.

use super::*;
use crate::generated_source::GeneratedProfileV1;
use fe2o3_kfd::{
    ComputeAqlQueueLaneDispatchV1, ComputeAqlQueueSessionErrorV1, Gfx942CompletedDispatchBatchV1,
};

type BatchReceipt<const N: usize> =
    ReceiptV1<Gfx942DispatchBatchV1<N>, Gfx942CompletedDispatchBatchV1<N>>;

#[allow(
    clippy::large_enum_variant,
    reason = "original linear native receipts remain inline without a post-publication allocation"
)]
pub(super) enum NativeReceiptV1 {
    Singleton(BatchReceipt<1>),
    Cohort3(BatchReceipt<3>),
}

impl From<BatchReceipt<1>> for NativeReceiptV1 {
    fn from(value: BatchReceipt<1>) -> Self {
        Self::Singleton(value)
    }
}

impl NativeReceiptV1 {
    pub(super) fn ready(profile: GeneratedProfileV1) -> Option<Self> {
        match profile {
            GeneratedProfileV1::Singleton => Some(Self::Singleton(ReceiptV1::Ready)),
            GeneratedProfileV1::NativeFillCohort3 => Some(Self::Cohort3(ReceiptV1::Ready)),
            GeneratedProfileV1::NativeFillRegistry4
            | GeneratedProfileV1::NativeFillRegistry4Repeat2
            | GeneratedProfileV1::NativeFillRegistry16
            | GeneratedProfileV1::NativeFillArena1024
            | GeneratedProfileV1::IndependentFillArena1024
            | GeneratedProfileV1::IndependentFillArena2048
            | GeneratedProfileV1::IndependentArenaMember => None,
        }
    }

    pub(super) fn profile(&self) -> GeneratedProfileV1 {
        match self {
            Self::Singleton(_) => GeneratedProfileV1::Singleton,
            Self::Cohort3(_) => GeneratedProfileV1::NativeFillCohort3,
        }
    }

    pub(super) fn issue_ready(&self) -> bool {
        match self {
            Self::Singleton(receipt) => receipt.issue_ready(),
            Self::Cohort3(receipt) => receipt.issue_ready(),
        }
    }

    pub(super) fn retirement(&self) -> Option<RetirementV1> {
        match self {
            Self::Singleton(receipt) => receipt.retirement(),
            Self::Cohort3(receipt) => receipt.retirement(),
        }
    }

    #[cfg(feature = "hardware-qualification")]
    pub(super) fn published(&self) -> bool {
        matches!(
            self,
            Self::Singleton(ReceiptV1::Published(_)) | Self::Cohort3(ReceiptV1::Published(_))
        )
    }

    pub(super) fn recycled(&self) -> bool {
        matches!(
            self,
            Self::Singleton(ReceiptV1::Recycled) | Self::Cohort3(ReceiptV1::Recycled)
        )
    }

    pub(super) fn rejected_retirement(&self) -> Option<RetirementV1> {
        match self {
            Self::Singleton(receipt) => receipt.rejected().map(|(prior, _)| prior),
            Self::Cohort3(_) => None,
        }
    }

    pub(super) fn mark_rejected_disposed(&mut self) {
        match self {
            Self::Singleton(receipt) => receipt.mark_rejected_disposed(),
            Self::Cohort3(_) => std::process::abort(),
        }
    }

    pub(super) fn rejected_disposed(&self) -> bool {
        match self {
            Self::Singleton(receipt) => receipt.rejected_disposed_error().is_some(),
            Self::Cohort3(_) => false,
        }
    }

    pub(super) fn issue_preserving_rejection(
        &mut self,
        lane: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Singleton(receipt) => receipt
                .issue_with_rejection(true, || lane.submit_fixed_dispatch_classified_v1::<1>()),
            Self::Cohort3(_) => Err(ComputeAqlQueueSessionErrorV1::Contract(
                "local rejected publication is singleton-only",
            )),
        }
    }

    pub(super) fn issue(
        &mut self,
        lane: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Singleton(receipt) => {
                receipt.issue(|| lane.submit_fixed_dispatch_classified_v1::<1>())
            }
            Self::Cohort3(receipt) => {
                receipt.issue(|| lane.submit_fixed_dispatch_classified_v1::<3>())
            }
        }
    }

    pub(super) fn progress(
        &mut self,
        lane: &mut ComputeAqlQueueLaneDispatchV1<'_>,
    ) -> Result<(), ComputeAqlQueueSessionErrorV1> {
        match self {
            Self::Singleton(receipt) => progress(receipt, lane),
            Self::Cohort3(receipt) => progress(receipt, lane),
        }
    }
}

#[allow(
    clippy::result_large_err,
    reason = "original completion custody stays inline without a post-effect allocation"
)]
fn progress<const N: usize>(
    receipt: &mut BatchReceipt<N>,
    lane: &mut ComputeAqlQueueLaneDispatchV1<'_>,
) -> Result<(), ComputeAqlQueueSessionErrorV1> {
    match receipt {
        ReceiptV1::Ready | ReceiptV1::RetryReady | ReceiptV1::Recycled => Ok(()),
        ReceiptV1::Published(_) => receipt.poll(|batch| {
            lane.poll_fixed_dispatch(batch).map(|poll| match poll {
                Gfx942DispatchPollV1::Pending(batch) => receipt::PollV1::Pending(batch),
                Gfx942DispatchPollV1::Ready(completed) => receipt::PollV1::Completed(completed),
            })
        }),
        ReceiptV1::Completed(_) => receipt
            .recycle(|completed| {
                lane.recycle_fixed_dispatch(completed)
                    .map(|_| ())
                    .map_err(|failure| failure.into_parts())
            })
            .map(|_| ()),
        ReceiptV1::RejectedUnpublished { .. } | ReceiptV1::RejectedDisposed(_) => {
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "rejected publication is not completion or generic retirement",
            ))
        }
        ReceiptV1::HandedToLower(stage) => {
            let _ = stage;
            Err(ComputeAqlQueueSessionErrorV1::Contract(
                "generated consuming handoff cannot retry",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cohort3_receipt_profile_is_preserved_for_every_ownerless_terminal_or_preissue_state() {
        for (receipt, expected, ready, recycled) in [
            (ReceiptV1::Ready, Some(RetirementV1::Pristine), true, false),
            (
                ReceiptV1::RetryReady,
                Some(RetirementV1::CancelledOnly),
                true,
                false,
            ),
            (
                ReceiptV1::Recycled,
                Some(RetirementV1::Recycled),
                false,
                true,
            ),
            (
                ReceiptV1::HandedToLower(receipt::HandoffV1::Issue),
                None,
                false,
                false,
            ),
            (
                ReceiptV1::HandedToLower(receipt::HandoffV1::Poll),
                None,
                false,
                false,
            ),
            (
                ReceiptV1::HandedToLower(receipt::HandoffV1::Recycle),
                None,
                false,
                false,
            ),
        ] {
            let receipt = NativeReceiptV1::Cohort3(receipt);
            assert_eq!(receipt.profile(), GeneratedProfileV1::NativeFillCohort3);
            assert_eq!(receipt.retirement(), expected);
            assert_eq!(receipt.issue_ready(), ready);
            assert_eq!(receipt.recycled(), recycled);
        }
        assert_eq!(
            NativeReceiptV1::ready(GeneratedProfileV1::Singleton)
                .unwrap()
                .profile(),
            GeneratedProfileV1::Singleton
        );
        assert_eq!(
            NativeReceiptV1::ready(GeneratedProfileV1::NativeFillCohort3)
                .unwrap()
                .profile(),
            GeneratedProfileV1::NativeFillCohort3
        );
        assert!(NativeReceiptV1::ready(GeneratedProfileV1::IndependentArenaMember).is_none());
        assert!(NativeReceiptV1::ready(GeneratedProfileV1::IndependentFillArena1024).is_none());
        assert!(NativeReceiptV1::ready(GeneratedProfileV1::IndependentFillArena2048).is_none());
        // Published/Completed require actual linear native owners, not fixture tokens.
    }
}
