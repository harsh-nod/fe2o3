//! Session-owned graph epochs, exact transient snapshots, and checked mutation custody.

use std::{
    fmt,
    num::NonZeroU64,
    panic::{AssertUnwindSafe, catch_unwind},
};

use pliron::{
    operation::{Operation, verify_operation},
    printable::Printable,
};
use sha2::{Digest as _, Sha256};

use crate::{
    ContextIdentity, OperationHandle, OperationHandleError, OperationHandleIdentity, PlironSession,
    inspect_operation_tree_details,
};

pub(crate) const OPERATION_GRAPH_DIGEST_DOMAIN_V1: &[u8] =
    b"fe2o3.pliron.transient-graph-digest.v1\0";
pub(crate) const OPERATION_GRAPH_REPLAY_DIGEST_DOMAIN_V1: &[u8] =
    b"fe2o3.pliron.operation-graph-replay-digest.v1\0";

/// Monotonic mutation epoch for one session-owned operation graph.
///
/// The value is process-local custody metadata, not a durable compiler or artifact identity.
#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct OperationGraphEpochV1(NonZeroU64);

impl OperationGraphEpochV1 {
    pub const fn sequence(self) -> u64 {
        self.0.get()
    }

    pub(crate) fn checked_next(self) -> Option<Self> {
        self.sequence()
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .map(Self)
    }
}

impl fmt::Debug for OperationGraphEpochV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("OperationGraphEpochV1")
            .field(&self.sequence())
            .finish()
    }
}

/// Opaque binding to one exact graph state in one owner session.
///
/// Its digest is derived from the live Pliron presentation only for same-session mutation and
/// cache checks. It is deliberately not exposed as a canonical or durable identity.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct OperationGraphSnapshotV1 {
    owner: ContextIdentity,
    root: OperationHandleIdentity,
    epoch: OperationGraphEpochV1,
    digest: [u8; 32],
}

impl OperationGraphSnapshotV1 {
    pub const fn epoch(self) -> OperationGraphEpochV1 {
        self.epoch
    }
}

impl fmt::Debug for OperationGraphSnapshotV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationGraphSnapshotV1")
            .field("epoch", &self.epoch)
            .finish_non_exhaustive()
    }
}

/// Owner-independent structural identity published in deterministic reports.
///
/// The canonical digest is domain-separated from the private custody digest and binds the exact
/// graph presentation used by the pinned Pliron revision. The epoch and graph facts make cache
/// and replay transitions explicit without exposing process-local owners or handles.
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct OperationGraphReplayIdentityV1 {
    epoch: OperationGraphEpochV1,
    canonical_digest: [u8; 32],
    tree_work: usize,
    operation_count: usize,
}

impl OperationGraphReplayIdentityV1 {
    pub(crate) fn from_transient_digest(
        epoch: OperationGraphEpochV1,
        transient_digest: [u8; 32],
        tree_work: usize,
        operation_count: usize,
    ) -> Self {
        let mut digest = Sha256::new();
        digest.update(OPERATION_GRAPH_REPLAY_DIGEST_DOMAIN_V1);
        digest.update(transient_digest);
        Self {
            epoch,
            canonical_digest: digest.finalize().into(),
            tree_work,
            operation_count,
        }
    }

    pub const fn epoch(self) -> OperationGraphEpochV1 {
        self.epoch
    }

    pub const fn canonical_digest(self) -> [u8; 32] {
        self.canonical_digest
    }

    pub const fn tree_work(self) -> usize {
        self.tree_work
    }

    pub const fn operation_count(self) -> usize {
        self.operation_count
    }
}

impl fmt::Debug for OperationGraphReplayIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OperationGraphReplayIdentityV1")
            .field("epoch", &self.epoch)
            .field("canonical_digest", &self.canonical_digest)
            .field("tree_work", &self.tree_work)
            .field("operation_count", &self.operation_count)
            .finish()
    }
}

/// Cached recursive verification facts for one exact graph epoch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperationGraphAnalysisV1 {
    snapshot: OperationGraphSnapshotV1,
    tree_work: usize,
    operation_count: usize,
}

impl OperationGraphAnalysisV1 {
    pub const fn snapshot(self) -> OperationGraphSnapshotV1 {
        self.snapshot
    }

    pub const fn tree_work(self) -> usize {
        self.tree_work
    }

    pub const fn operation_count(self) -> usize {
        self.operation_count
    }

    /// Returns the owner-independent projection suitable for report equality and replay.
    pub fn replay_identity(self) -> OperationGraphReplayIdentityV1 {
        OperationGraphReplayIdentityV1::from_transient_digest(
            self.snapshot.epoch,
            self.snapshot.digest,
            self.tree_work,
            self.operation_count,
        )
    }
}

#[derive(Clone, Copy)]
pub(super) struct CachedOperationGraphAnalysisV1 {
    analysis: OperationGraphAnalysisV1,
}

/// Private transaction capability captured before a trusted graph mutation begins.
pub(crate) struct CheckedOperationGraphMutationV1 {
    before: OperationGraphSnapshotV1,
}

/// Cache disposition committed only after exact digest and recursive-verification checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct OperationGraphMutationCommitV1 {
    snapshot: OperationGraphSnapshotV1,
    invalidated_analysis_count: usize,
    preserved_analysis_count: usize,
}

impl OperationGraphMutationCommitV1 {
    pub(crate) const fn snapshot(self) -> OperationGraphSnapshotV1 {
        self.snapshot
    }

    pub(crate) const fn invalidated_analysis_count(self) -> usize {
        self.invalidated_analysis_count
    }

    pub(crate) const fn preserved_analysis_count(self) -> usize {
        self.preserved_analysis_count
    }
}

impl PlironSession {
    /// Captures the exact current graph epoch after authenticating its owner and live root.
    pub fn operation_graph_snapshot_v1(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<OperationGraphSnapshotV1, OperationHandleError> {
        let root = self.authenticated_operation_root_v1(handle)?;
        self.checked_current_operation_graph_snapshot_v1(root)
    }

    /// Rejects stale, foreign, or graph-mismatched snapshot tokens before returning graph facts.
    pub fn require_operation_graph_snapshot_v1(
        &mut self,
        handle: &OperationHandle,
        expected: OperationGraphSnapshotV1,
    ) -> Result<(), OperationHandleError> {
        if expected.owner != self.identity {
            return Err(OperationHandleError::ForeignSession);
        }
        let actual = self.operation_graph_snapshot_v1(handle)?;
        if actual != expected {
            return Err(OperationHandleError::OperationGraphSnapshotMismatch);
        }
        Ok(())
    }

    /// Returns cached recursively verified facts only for the exact current epoch and digest.
    pub fn analyze_operation_graph_v1(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<OperationGraphAnalysisV1, OperationHandleError> {
        let root = self.authenticated_operation_root_v1(handle)?;
        let snapshot = self.checked_current_operation_graph_snapshot_v1(root)?;
        if let Some(cached) = self.operation_graph_analysis_cache.get(&root)
            && cached.analysis.snapshot == snapshot
        {
            return Ok(cached.analysis);
        }
        let pointer = self
            .operations
            .get(&root)
            .copied()
            .ok_or(OperationHandleError::StaleHandle)?;
        let (tree_work, operations) = catch_unwind(AssertUnwindSafe(|| {
            inspect_operation_tree_details(pointer, &mut self.context)
        }))
        .map_err(|_| {
            self.poisoned = true;
            OperationHandleError::UpstreamPanicked
        })??;
        let charged = self
            .owned_tree_work
            .get(&root)
            .copied()
            .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?;
        if tree_work != charged {
            self.poisoned = true;
            return Err(OperationHandleError::OperationGraphOwnershipMismatch);
        }
        catch_unwind(AssertUnwindSafe(|| {
            verify_operation(pointer, &self.context)
        }))
        .map_err(|_| {
            self.poisoned = true;
            OperationHandleError::UpstreamPanicked
        })?
        .map_err(|_| {
            self.poisoned = true;
            OperationHandleError::OperationVerificationRejected
        })?;
        let analysis = OperationGraphAnalysisV1 {
            snapshot,
            tree_work,
            operation_count: operations.len(),
        };
        self.operation_graph_analysis_cache
            .insert(root, CachedOperationGraphAnalysisV1 { analysis });
        Ok(analysis)
    }

    pub(crate) fn begin_checked_operation_graph_mutation_v1(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<CheckedOperationGraphMutationV1, OperationHandleError> {
        Ok(CheckedOperationGraphMutationV1 {
            before: self.operation_graph_snapshot_v1(handle)?,
        })
    }

    pub(crate) fn commit_checked_operation_graph_mutation_v1(
        &mut self,
        transaction: CheckedOperationGraphMutationV1,
        reported_changed: bool,
    ) -> Result<OperationGraphMutationCommitV1, OperationHandleError> {
        self.validate_identity()?;
        let before = transaction.before;
        if before.owner != self.identity
            || self.operation_graph_epochs.get(&before.root).copied() != Some(before.epoch)
            || self.operation_graph_digests.get(&before.root).copied() != Some(before.digest)
        {
            return Err(OperationHandleError::OperationGraphSnapshotMismatch);
        }
        let pointer = self
            .operations
            .get(&before.root)
            .copied()
            .ok_or(OperationHandleError::StaleHandle)?;
        catch_unwind(AssertUnwindSafe(|| {
            verify_operation(pointer, &self.context)
        }))
        .map_err(|_| {
            self.poisoned = true;
            OperationHandleError::UpstreamPanicked
        })?
        .map_err(|_| {
            self.poisoned = true;
            OperationHandleError::OperationVerificationRejected
        })?;
        let digest = self.operation_graph_digest_v1(pointer)?;
        if !reported_changed && digest != before.digest {
            self.poisoned = true;
            return Err(OperationHandleError::OperationGraphMutationReportMismatch);
        }
        if !reported_changed {
            let preserved_analysis_count = usize::from(
                self.operation_graph_analysis_cache
                    .get(&before.root)
                    .is_some_and(|cached| cached.analysis.snapshot == before),
            );
            return Ok(OperationGraphMutationCommitV1 {
                snapshot: before,
                invalidated_analysis_count: 0,
                preserved_analysis_count,
            });
        }

        let next = before.epoch.checked_next().ok_or_else(|| {
            self.poisoned = true;
            OperationHandleError::OperationGraphEpochSpaceExhausted
        })?;
        let invalidated_analysis_count = usize::from(
            self.operation_graph_analysis_cache
                .remove(&before.root)
                .is_some(),
        );
        self.operation_graph_epochs.insert(before.root, next);
        self.operation_graph_digests.insert(before.root, digest);
        Ok(OperationGraphMutationCommitV1 {
            snapshot: OperationGraphSnapshotV1 {
                owner: before.owner,
                root: before.root,
                epoch: next,
                digest,
            },
            invalidated_analysis_count,
            preserved_analysis_count: 0,
        })
    }

    pub(crate) fn register_operation_graph_v1(
        &mut self,
        root: &OperationHandle,
    ) -> Result<(), OperationHandleError> {
        let registered_root = self.authenticated_operation_root_v1(root)?;
        if registered_root != root.identity
            || self.operation_graph_epochs.contains_key(&registered_root)
            || self.operation_graph_digests.contains_key(&registered_root)
        {
            self.poisoned = true;
            return Err(OperationHandleError::OperationGraphOwnershipMismatch);
        }
        let pointer = self.operations[&registered_root];
        let digest = self.operation_graph_digest_v1(pointer)?;
        self.operation_graph_epochs.insert(
            registered_root,
            OperationGraphEpochV1(NonZeroU64::new(1).expect("one is non-zero")),
        );
        self.operation_graph_digests.insert(registered_root, digest);
        Ok(())
    }

    pub(crate) fn forget_operation_graph_v1(&mut self, root: OperationHandleIdentity) {
        self.operation_graph_epochs.remove(&root);
        self.operation_graph_digests.remove(&root);
        self.operation_graph_analysis_cache.remove(&root);
    }

    fn authenticated_operation_root_v1(
        &mut self,
        handle: &OperationHandle,
    ) -> Result<OperationHandleIdentity, OperationHandleError> {
        self.with_operation(handle, |_, _| ())?;
        self.operation_roots
            .get(&handle.identity)
            .copied()
            .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)
    }

    fn checked_current_operation_graph_snapshot_v1(
        &mut self,
        root: OperationHandleIdentity,
    ) -> Result<OperationGraphSnapshotV1, OperationHandleError> {
        let epoch = self
            .operation_graph_epochs
            .get(&root)
            .copied()
            .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?;
        let registered_digest = self
            .operation_graph_digests
            .get(&root)
            .copied()
            .ok_or(OperationHandleError::OperationGraphOwnershipMismatch)?;
        let pointer = self
            .operations
            .get(&root)
            .copied()
            .ok_or(OperationHandleError::StaleHandle)?;
        let digest = self.operation_graph_digest_v1(pointer)?;
        if digest != registered_digest {
            self.poisoned = true;
            return Err(OperationHandleError::OperationGraphChangedOutsideTransaction);
        }
        Ok(OperationGraphSnapshotV1 {
            owner: self.identity,
            root,
            epoch,
            digest,
        })
    }

    fn operation_graph_digest_v1(
        &mut self,
        pointer: pliron::context::Ptr<Operation>,
    ) -> Result<[u8; 32], OperationHandleError> {
        let presentation =
            catch_unwind(AssertUnwindSafe(|| pointer.disp(&self.context).to_string())).map_err(
                |_| {
                    self.poisoned = true;
                    OperationHandleError::UpstreamPanicked
                },
            )?;
        let length = u64::try_from(presentation.len()).map_err(|_| {
            self.poisoned = true;
            OperationHandleError::OperationGraphOwnershipMismatch
        })?;
        let mut digest = Sha256::new();
        digest.update(OPERATION_GRAPH_DIGEST_DOMAIN_V1);
        digest.update(length.to_le_bytes());
        digest.update(presentation.as_bytes());
        Ok(digest.finalize().into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pliron::{
        builtin::{op_interfaces::SingleBlockRegionInterface, ops::ModuleOp},
        op::Op,
    };

    #[test]
    fn false_unchanged_report_after_mutation_poisons_the_candidate_session() {
        let mut session = PlironSession::new(crate::ShellLimits::default(), []).unwrap();
        let root = session.create_module("root").unwrap();
        let transaction = session
            .begin_checked_operation_graph_mutation_v1(&root)
            .unwrap();
        session
            .with_operation(&root, |pointer, context| {
                let root = ModuleOp::from_operation(pointer);
                let child = ModuleOp::new(context, "child".try_into().unwrap());
                root.append_operation(context, child.get_operation(), 0);
            })
            .unwrap();

        assert!(matches!(
            session.commit_checked_operation_graph_mutation_v1(transaction, false),
            Err(OperationHandleError::OperationGraphMutationReportMismatch)
        ));
        assert!(session.is_poisoned());
        assert_eq!(
            session.operation_graph_snapshot_v1(&root),
            Err(OperationHandleError::SessionPoisoned)
        );
    }

    #[test]
    fn invalid_post_mutation_graph_is_rejected_before_epoch_commit() {
        let mut session = PlironSession::new(crate::ShellLimits::default(), []).unwrap();
        let root = session.create_module("root").unwrap();
        let transaction = session
            .begin_checked_operation_graph_mutation_v1(&root)
            .unwrap();
        session
            .with_operation(&root, |pointer, context| {
                Operation::erase_region(pointer, context, 0);
            })
            .unwrap();

        assert!(matches!(
            session.commit_checked_operation_graph_mutation_v1(transaction, true),
            Err(OperationHandleError::OperationVerificationRejected)
        ));
        assert!(session.is_poisoned());
    }
}
