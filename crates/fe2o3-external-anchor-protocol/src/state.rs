use crate::protocol::{
    AnchorChallengeV1, AnchorKeyIdentityV1, AnchorPositionV1, AnchorProtocolErrorV1, CallerNonceV1,
    ChallengeKindV1, HashChainHeadV1, PinnedAnchorKeyV1, TransactionDigestV1,
    derive_proposed_head_v1, verify_observation,
};

#[derive(Debug, PartialEq, Eq)]
pub struct AnchoredStateV1 {
    sequence: u64,
    head: HashChainHeadV1,
}

impl AnchoredStateV1 {
    /// Constructs caller-asserted local state. This operation does not attest its durability.
    pub const fn from_local_state(sequence: u64, head: HashChainHeadV1) -> Self {
        Self { sequence, head }
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn head(&self) -> HashChainHeadV1 {
        self.head
    }

    pub fn prepare(
        self,
        transaction: TransactionDigestV1,
        key: &PinnedAnchorKeyV1,
    ) -> Result<PreparedAnchorAdvanceV1, AnchorProtocolErrorV1> {
        let expected_sequence = self
            .sequence
            .checked_add(1)
            .ok_or(AnchorProtocolErrorV1::SequenceOverflow)?;
        let proposed_head =
            derive_proposed_head_v1(expected_sequence, self.head, transaction, key.identity());
        Ok(PreparedAnchorAdvanceV1 {
            expected_sequence,
            prior_head: self.head,
            transaction,
            proposed_head,
            anchor_key_identity: key.identity(),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PreparedAnchorAdvanceV1 {
    expected_sequence: u64,
    prior_head: HashChainHeadV1,
    transaction: TransactionDigestV1,
    proposed_head: HashChainHeadV1,
    anchor_key_identity: AnchorKeyIdentityV1,
}

impl PreparedAnchorAdvanceV1 {
    /// Reconstructs a modeled prepared phase after a crash from caller-supplied local fields.
    ///
    /// This validates the hash-chain relation but does not attest where the fields were stored.
    pub fn recover_from_local_state(
        expected_sequence: u64,
        prior_head: HashChainHeadV1,
        transaction: TransactionDigestV1,
        proposed_head: HashChainHeadV1,
        key: &PinnedAnchorKeyV1,
    ) -> Result<Self, AnchorProtocolErrorV1> {
        if expected_sequence == 0 {
            return Err(AnchorProtocolErrorV1::SequenceRegression);
        }
        if proposed_head
            != derive_proposed_head_v1(expected_sequence, prior_head, transaction, key.identity())
        {
            return Err(AnchorProtocolErrorV1::InvalidProposedHead);
        }
        Ok(Self {
            expected_sequence,
            prior_head,
            transaction,
            proposed_head,
            anchor_key_identity: key.identity(),
        })
    }

    pub const fn expected_sequence(&self) -> u64 {
        self.expected_sequence
    }

    pub const fn prior_head(&self) -> HashChainHeadV1 {
        self.prior_head
    }

    pub const fn transaction(&self) -> TransactionDigestV1 {
        self.transaction
    }

    pub const fn proposed_head(&self) -> HashChainHeadV1 {
        self.proposed_head
    }

    pub const fn anchor_key_identity(&self) -> AnchorKeyIdentityV1 {
        self.anchor_key_identity
    }

    pub fn begin_advance(
        self,
        nonce: CallerNonceV1,
        key: &PinnedAnchorKeyV1,
    ) -> Result<PendingAnchorTransitionV1, AnchorProtocolErrorV1> {
        self.begin(ChallengeKindV1::Advance, nonce, key)
    }

    pub fn begin_recovery(
        self,
        nonce: CallerNonceV1,
        key: &PinnedAnchorKeyV1,
    ) -> Result<PendingAnchorTransitionV1, AnchorProtocolErrorV1> {
        self.begin(ChallengeKindV1::Recover, nonce, key)
    }

    fn begin(
        self,
        kind: ChallengeKindV1,
        nonce: CallerNonceV1,
        key: &PinnedAnchorKeyV1,
    ) -> Result<PendingAnchorTransitionV1, AnchorProtocolErrorV1> {
        if self.anchor_key_identity != key.identity() {
            return Err(AnchorProtocolErrorV1::AnchorKeyIdentityMismatch);
        }
        let challenge = AnchorChallengeV1::new(
            kind,
            nonce,
            self.expected_sequence,
            self.prior_head,
            self.transaction,
            self.proposed_head,
            self.anchor_key_identity,
        )?;
        Ok(PendingAnchorTransitionV1 {
            prepared: self,
            challenge,
            pinned_key_bytes: key.to_bytes(),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct PendingAnchorTransitionV1 {
    prepared: PreparedAnchorAdvanceV1,
    challenge: AnchorChallengeV1,
    pinned_key_bytes: [u8; 32],
}

impl PendingAnchorTransitionV1 {
    pub const fn challenge(&self) -> &AnchorChallengeV1 {
        &self.challenge
    }

    /// Consumes the one-shot challenge and returns a deterministic anchored decision.
    pub fn verify(self, observation: &[u8]) -> Result<AnchorDecisionV1, AnchorProtocolErrorV1> {
        let key = PinnedAnchorKeyV1::from_bytes(self.pinned_key_bytes)?;
        let position = verify_observation(&self.challenge, &key, observation)?;
        Ok(match position {
            AnchorPositionV1::Prior => AnchorDecisionV1::Abort(AnchoredAbortDecisionV1 {
                stable: AnchoredStateV1 {
                    sequence: self.prepared.expected_sequence - 1,
                    head: self.prepared.prior_head,
                },
                transaction: self.prepared.transaction,
                proposed_head: self.prepared.proposed_head,
                observed_nonce: self.challenge.nonce(),
            }),
            AnchorPositionV1::Proposed => AnchorDecisionV1::Commit(AnchoredCommitDecisionV1 {
                stable: AnchoredStateV1 {
                    sequence: self.prepared.expected_sequence,
                    head: self.prepared.proposed_head,
                },
                transaction: self.prepared.transaction,
                prior_head: self.prepared.prior_head,
                observed_nonce: self.challenge.nonce(),
            }),
        })
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum AnchorDecisionV1 {
    Commit(AnchoredCommitDecisionV1),
    Abort(AnchoredAbortDecisionV1),
}

#[derive(Debug, PartialEq, Eq)]
pub struct AnchoredCommitDecisionV1 {
    stable: AnchoredStateV1,
    transaction: TransactionDigestV1,
    prior_head: HashChainHeadV1,
    observed_nonce: [u8; 32],
}

impl AnchoredCommitDecisionV1 {
    pub const fn sequence(&self) -> u64 {
        self.stable.sequence
    }

    pub const fn head(&self) -> HashChainHeadV1 {
        self.stable.head
    }

    pub const fn prior_head(&self) -> HashChainHeadV1 {
        self.prior_head
    }

    pub const fn transaction(&self) -> TransactionDigestV1 {
        self.transaction
    }

    pub const fn observed_nonce(&self) -> &[u8; 32] {
        &self.observed_nonce
    }

    pub fn into_stable_state(self) -> AnchoredStateV1 {
        self.stable
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct AnchoredAbortDecisionV1 {
    stable: AnchoredStateV1,
    transaction: TransactionDigestV1,
    proposed_head: HashChainHeadV1,
    observed_nonce: [u8; 32],
}

impl AnchoredAbortDecisionV1 {
    pub const fn sequence(&self) -> u64 {
        self.stable.sequence
    }

    pub const fn head(&self) -> HashChainHeadV1 {
        self.stable.head
    }

    pub const fn proposed_head(&self) -> HashChainHeadV1 {
        self.proposed_head
    }

    pub const fn transaction(&self) -> TransactionDigestV1 {
        self.transaction
    }

    pub const fn observed_nonce(&self) -> &[u8; 32] {
        &self.observed_nonce
    }

    pub fn into_stable_state(self) -> AnchoredStateV1 {
        self.stable
    }
}
