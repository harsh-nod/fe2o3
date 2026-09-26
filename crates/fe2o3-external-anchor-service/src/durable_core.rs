//! Shared durable state and recovery engine; signing custody belongs to its caller.

use std::{fmt, os::fd::OwnedFd};

use fe2o3_external_anchor_protocol::{
    AnchorChallengeV1, AnchorKeyIdentityV1, AnchorPositionV1, ChallengeKindV1, HashChainHeadV1,
    PinnedAnchorKeyV1,
};

use super::{
    DurableAnchorStateV1, DurableExternalAnchorOpenDispositionV1, ExternalAnchorServiceErrorV1,
    PersistenceHooksV1, admit_and_lock_root, create_initial_state, read_state,
    remove_leftover_next, replace_state_with_hooks,
};

/// Owns the exclusive root lock and admitted public key, never a signing key.
pub(crate) struct DurableAnchorCoreV1 {
    root: OwnedFd,
    pinned_key: PinnedAnchorKeyV1,
    state: DurableAnchorStateV1,
    poisoned: bool,
}

impl DurableAnchorCoreV1 {
    pub(crate) fn initialize(
        root: OwnedFd,
        pinned_key: PinnedAnchorKeyV1,
    ) -> Result<Self, ExternalAnchorServiceErrorV1> {
        admit_and_lock_root(&root)?;
        remove_leftover_next(&root)?;
        let state = DurableAnchorStateV1::genesis();
        create_initial_state(&root, &state.encode(&pinned_key))?;
        Ok(Self {
            root,
            pinned_key,
            state,
            poisoned: false,
        })
    }

    pub(crate) fn open(
        root: OwnedFd,
        pinned_key: PinnedAnchorKeyV1,
    ) -> Result<Self, ExternalAnchorServiceErrorV1> {
        admit_and_lock_root(&root)?;
        remove_leftover_next(&root)?;
        let state = read_state(&root, &pinned_key)?;
        Ok(Self {
            root,
            pinned_key,
            state,
            poisoned: false,
        })
    }

    /// Only exact absence permits genesis; invalid existing state is never reset.
    pub(crate) fn open_or_initialize(
        root: OwnedFd,
        pinned_key: PinnedAnchorKeyV1,
    ) -> Result<(Self, DurableExternalAnchorOpenDispositionV1), ExternalAnchorServiceErrorV1> {
        admit_and_lock_root(&root)?;
        remove_leftover_next(&root)?;
        let (state, disposition) = match read_state(&root, &pinned_key) {
            Ok(state) => (state, DurableExternalAnchorOpenDispositionV1::Existing),
            Err(error) if error.is_missing_state_file() => {
                let state = DurableAnchorStateV1::genesis();
                create_initial_state(&root, &state.encode(&pinned_key))?;
                (state, DurableExternalAnchorOpenDispositionV1::Initialized)
            }
            Err(error) => return Err(error),
        };
        Ok((
            Self {
                root,
                pinned_key,
                state,
                poisoned: false,
            },
            disposition,
        ))
    }

    pub(crate) const fn sequence(&self) -> u64 {
        self.state.sequence
    }

    pub(crate) const fn head(&self) -> HashChainHeadV1 {
        self.state.head
    }

    pub(crate) const fn key_identity(&self) -> AnchorKeyIdentityV1 {
        self.pinned_key.identity()
    }

    pub(crate) const fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    /// Resolves the exact prior/proposed position, persisting an advance before return.
    /// Signing is deliberately separate: later signing or resource refusal cannot undo
    /// committed state. Persistence error or unwind leaves this instance poisoned and
    /// requires reopening, even when a hook panics after the durable rename or sync.
    pub(crate) fn observe_with_hooks(
        &mut self,
        bytes: &[u8],
        hooks: &mut impl PersistenceHooksV1,
    ) -> Result<(AnchorChallengeV1, AnchorPositionV1), ExternalAnchorServiceErrorV1> {
        if self.poisoned {
            return Err(ExternalAnchorServiceErrorV1::Poisoned);
        }
        let challenge = AnchorChallengeV1::decode(bytes)?;
        if challenge.anchor_key_identity() != self.pinned_key.identity() {
            return Err(ExternalAnchorServiceErrorV1::ChallengeKeyIdentityMismatch);
        }

        let at_prior = self.state.sequence.checked_add(1) == Some(challenge.expected_sequence())
            && self.state.head == challenge.prior_head();
        let at_proposed = self.state.sequence == challenge.expected_sequence()
            && self.state.head == challenge.proposed_head();
        if !at_prior && !at_proposed {
            return Err(ExternalAnchorServiceErrorV1::ChallengeStateMismatch);
        }

        let position = match (challenge.kind(), at_prior, at_proposed) {
            (ChallengeKindV1::Advance, true, false) => {
                let next = DurableAnchorStateV1 {
                    sequence: challenge.expected_sequence(),
                    head: challenge.proposed_head(),
                };
                // Poison before any hook so unwind cannot leave a usable stale cache.
                self.poisoned = true;
                replace_state_with_hooks(&self.root, &next.encode(&self.pinned_key), hooks)?;
                self.state = next;
                self.poisoned = false;
                AnchorPositionV1::Proposed
            }
            (ChallengeKindV1::Advance | ChallengeKindV1::Recover, false, true) => {
                AnchorPositionV1::Proposed
            }
            (ChallengeKindV1::Recover, true, false) => AnchorPositionV1::Prior,
            _ => return Err(ExternalAnchorServiceErrorV1::ChallengeStateMismatch),
        };
        Ok((challenge, position))
    }
}

impl fmt::Debug for DurableAnchorCoreV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DurableAnchorCoreV1")
            .field("sequence", &self.sequence())
            .field("head", &self.head())
            .field("key_identity", &self.key_identity())
            .field("poisoned", &self.is_poisoned())
            .finish_non_exhaustive()
    }
}
