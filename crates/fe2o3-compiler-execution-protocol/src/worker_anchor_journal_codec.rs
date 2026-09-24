//! Shared inert framing and phase rules. Nominal transaction validation stays
//! with each family; no journal bytes grant durable or endpoint custody.
use crate::worker_anchor_journal::{
    CompilerExecutionWorkerAnchorJournalErrorV1 as Error,
    CompilerExecutionWorkerAnchorJournalStageV1 as Stage,
};
use fe2o3_external_anchor_protocol::{
    ANCHOR_CHALLENGE_WIRE_LEN_V1 as C, ANCHOR_TRANSITION_RECEIPT_BYTES_V1 as R, AnchorChallengeV1,
    AnchorPositionV1, AnchorTransitionReceiptV1, PinnedAnchorKeyV1,
};
use sha2::{Digest, Sha256};

pub(crate) struct Schema {
    pub magic: [u8; 8],
    pub version: u16,
    pub domain: &'static [u8],
}
pub(crate) struct Parts<'a> {
    pub stage: Stage,
    pub transaction: &'a [u8],
    pub challenge: AnchorChallengeV1,
    pub receipt: &'a [u8],
    pub worker: [u8; 32],
}
impl Schema {
    pub fn encode<const N: usize>(
        &self,
        stage: Stage,
        transaction: &[u8],
        challenge: &AnchorChallengeV1,
        receipt: Option<&AnchorTransitionReceiptV1>,
        worker: [u8; 32],
    ) -> Result<[u8; N], Error> {
        if transaction.len().checked_add(32 + C + R + 64) != Some(N) {
            return Err(Error::InvalidEncoding("journal layout mismatch"));
        }
        let mut bytes = [0; N];
        bytes[..8].copy_from_slice(&self.magic);
        bytes[8..10].copy_from_slice(&self.version.to_le_bytes());
        bytes[12..20].copy_from_slice(&(N as u64).to_le_bytes());
        bytes[24] = stage as u8;
        let end = 32 + transaction.len();
        bytes[32..end].copy_from_slice(transaction);
        bytes[end..end + C].copy_from_slice(challenge.as_bytes());
        if let Some(receipt) = receipt {
            bytes[end + C..end + C + R].copy_from_slice(receipt.canonical_bytes());
        }
        bytes[N - 64..N - 32].copy_from_slice(&worker);
        let identity = self.identity(&bytes[..N - 32]);
        bytes[N - 32..].copy_from_slice(&identity);
        Ok(bytes)
    }
    pub fn decode<'a>(&self, bytes: &'a [u8], transaction_len: usize) -> Result<Parts<'a>, Error> {
        let n = transaction_len
            .checked_add(32 + C + R + 64)
            .ok_or(Error::InvalidEncoding("journal layout overflow"))?;
        if bytes.len() != n {
            return Err(Error::InvalidLength {
                expected: n,
                actual: bytes.len(),
            });
        }
        if bytes[..8] != self.magic
            || bytes[8..10] != self.version.to_le_bytes()
            || bytes[10..12] != [0; 2]
            || bytes[12..20] != (n as u64).to_le_bytes()
            || bytes[20..24] != [0; 4]
            || bytes[25..32] != [0; 7]
        {
            return Err(Error::InvalidEncoding("journal header is not canonical"));
        }
        if bytes[n - 32..] != self.identity(&bytes[..n - 32]) {
            return Err(Error::IdentityMismatch);
        }
        let end = 32 + transaction_len;
        Ok(Parts {
            stage: Stage::decode(bytes[24])?,
            transaction: &bytes[32..end],
            challenge: AnchorChallengeV1::decode(&bytes[end..end + C])?,
            receipt: &bytes[end + C..end + C + R],
            worker: bytes[n - 64..n - 32]
                .try_into()
                .map_err(|_| Error::InvalidEncoding("journal worker identity"))?,
        })
    }
    pub fn identity(&self, bytes: &[u8]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(self.domain);
        h.update(bytes);
        h.finalize().into()
    }
}

pub(crate) fn validate_payload(
    stage: Stage,
    challenge: &AnchorChallengeV1,
    receipt: Option<&AnchorTransitionReceiptV1>,
    worker: [u8; 32],
    key: &PinnedAnchorKeyV1,
) -> Result<(), Error> {
    if stage == Stage::PreparedAnchor {
        return if receipt.is_none() && worker == [0; 32] {
            Ok(())
        } else {
            Err(Error::StagePayloadMismatch)
        };
    }
    let receipt = receipt.ok_or(Error::StagePayloadMismatch)?;
    if AnchorTransitionReceiptV1::decode(receipt.canonical_bytes(), key)? != *receipt
        || receipt.challenge() != challenge
    {
        return Err(Error::ReceiptMismatch);
    }
    let expected = if stage == Stage::Aborted {
        AnchorPositionV1::Prior
    } else {
        AnchorPositionV1::Proposed
    };
    if receipt.position() != expected || (worker != [0; 32]) != (stage == Stage::Published) {
        return Err(Error::StagePayloadMismatch);
    }
    Ok(())
}

pub(crate) struct Position<'a> {
    pub stage: Stage,
    pub transaction: &'a [u8],
    pub policy: &'a [u8],
    pub sequence: u64,
    pub prior: [u8; 32],
    pub current: [u8; 32],
    pub challenge: &'a AnchorChallengeV1,
    pub receipt: Option<&'a AnchorTransitionReceiptV1>,
    pub worker: [u8; 32],
}
pub(crate) fn successor(prior: Position<'_>, next: Position<'_>) -> bool {
    let same = prior.transaction == next.transaction && prior.challenge == next.challenge;
    match (prior.stage, next.stage) {
        (Stage::PreparedAnchor, Stage::AnchorCommitted | Stage::Aborted) => {
            same && prior.receipt.is_none() && next.receipt.is_some()
        }
        (Stage::AnchorCommitted, Stage::Published) => {
            same && prior.receipt == next.receipt
                && prior.worker == [0; 32]
                && next.worker != [0; 32]
        }
        (Stage::Published, Stage::PreparedAnchor) => {
            prior.sequence.checked_add(1) == Some(next.sequence)
                && prior.policy == next.policy
                && next.prior == prior.current
                && next.challenge.prior_head() == prior.challenge.proposed_head()
        }
        (Stage::Aborted, Stage::PreparedAnchor) => {
            prior.transaction != next.transaction
                && prior.policy == next.policy
                && prior.sequence == next.sequence
                && prior.prior == next.prior
                && prior.challenge.prior_head() == next.challenge.prior_head()
        }
        _ => false,
    }
}
