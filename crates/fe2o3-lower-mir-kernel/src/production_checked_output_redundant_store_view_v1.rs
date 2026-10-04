use super::*;
use fe2o3_kernel_analysis::{
    CanonicalKirRedundantStoreRetainedOperationV1, CanonicalKirRedundantStoreRowV1,
    CanonicalKirRedundantStoreStorageV1, CheckedCanonicalKirRedundantStoreV1,
};
use fe2o3_kernel_opt::OwnedRedundantStoreContinuationV1;

// This closed internal view is never supplied by a public caller. The owning
// arm's temporary input borrow always comes from the actual retained Prefix6.
#[derive(Clone, Copy)]
pub(super) enum StoreDeletionView<'a> {
    Borrowed(&'a StoreOutput<'a>),
    Owned {
        input: &'a StoreOwner,
        continuation: &'a OwnedRedundantStoreContinuationV1,
    },
}
impl<'a> StoreDeletionView<'a> {
    pub(super) fn input(self) -> &'a StoreOwner {
        match self {
            Self::Borrowed(deletion) => deletion.input(),
            Self::Owned { input, .. } => input,
        }
    }
    pub(super) fn output(self) -> &'a StoreOwner {
        match self {
            Self::Borrowed(deletion) => deletion.output(),
            Self::Owned { continuation, .. } => continuation.output(),
        }
    }
    pub(super) fn rows(self) -> &'a [CanonicalKirRedundantStoreRowV1] {
        match self {
            Self::Borrowed(deletion) => deletion.rows(),
            Self::Owned { continuation, .. } => continuation.rows(),
        }
    }
    pub(super) fn retained_operations(self) -> &'a [CanonicalKirRedundantStoreRetainedOperationV1] {
        match self {
            Self::Borrowed(deletion) => deletion.retained_operations(),
            Self::Owned { continuation, .. } => continuation.retained_operations(),
        }
    }
    pub(super) fn replay(
        self,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> StoreResult<(
        CheckedCanonicalKirRedundantStoreV1<'a>,
        CanonicalKirRedundantStoreStorageV1,
    )> {
        match self {
            Self::Borrowed(deletion) => deletion.replay(budget),
            Self::Owned {
                input,
                continuation,
            } => continuation.replay_against(input, budget),
        }
        .map_err(StoreError::Deletion)
    }
}
