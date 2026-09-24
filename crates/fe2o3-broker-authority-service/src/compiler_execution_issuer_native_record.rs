//! Signed native issuer state. Subject/occurrence authority stays outside this
//! codec and enters only through the consuming service's live occurrence guard.
use super::*;
use ed25519_dalek::{Signature, VerifyingKey};
use fe2o3_artifact_transaction::INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V2 as S;
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ATTESTATION_CHALLENGE_BYTES_V2 as H,
    COMPILER_EXECUTION_ATTESTATION_RECEIPT_BYTES_V2 as R,
    COMPILER_EXECUTION_ATTESTATION_REQUEST_BYTES_V2 as Q,
    COMPILER_EXECUTION_RECEIPT_PUBLICATION_ACK_BYTES_V2 as A,
};
use sha2::{Digest, Sha256};

const ACK: usize = 104;
const OCCURRENCE: usize = ACK + A;
const BODY: usize = OCCURRENCE + 32;
const SIGNATURE: usize = BODY + Q + R;
const IDENTITY: usize = SIGNATURE + 64;
pub(super) const BYTES: usize = IDENTITY + 32;
const MAGIC: &[u8; 8] = b"F2O3CEJ3";
const SIGN_DOMAIN: &[u8] = b"FE2O3/NATIVE-ISSUER-JOURNAL-SIGNATURE/V3\0";
const ID_DOMAIN: &[u8] = b"FE2O3/NATIVE-ISSUER-JOURNAL-IDENTITY/V3\0";

pub(super) enum Body {
    Ready,
    Prepared {
        subject: Subject,
        challenge: Challenge,
    },
    Issued {
        request: Request,
        receipt: Receipt,
    },
}
pub(super) struct Record {
    pub sequence: u64,
    pub prior: [u8; 32],
    pub last_ack: Option<Ack>,
    pub occurrence: [u8; 32],
    pub body: Body,
    wire: [u8; BYTES],
}
impl Record {
    pub const STORAGE: usize = std::mem::size_of::<Self>() + 64;
    const FRAME: usize = 8 * Self::STORAGE + 8 * BYTES + 16384;
    const WORK: usize = 1024 * BYTES + 262144;

    pub fn genesis(p: &Policy, key: &Key, b: &mut Budget<'_>) -> Result<Self> {
        Self::build(p, key, 1, [0; 32], None, [0; 32], Body::Ready, b)
    }
    fn build(
        p: &Policy,
        key: &Key,
        sequence: u64,
        prior: [u8; 32],
        last_ack: Option<Ack>,
        occurrence: [u8; 32],
        body: Body,
        b: &mut Budget<'_>,
    ) -> Result<Self> {
        b.with_prepaid_scope(
            p.retained_storage() + key.retained_storage(),
            8,
            Self::WORK,
            Self::FRAME,
            |b| {
                let mut record = Self {
                    sequence,
                    prior,
                    last_ack,
                    occurrence,
                    body,
                    wire: [0; BYTES],
                };
                record.check(p, b)?;
                let wire = &mut record.wire;
                wire[..8].copy_from_slice(MAGIC);
                wire[8..10].copy_from_slice(&3u16.to_le_bytes());
                wire[12..20].copy_from_slice(&(BYTES as u64).to_le_bytes());
                wire[32..64].copy_from_slice(p.identity().as_bytes());
                wire[64..72].copy_from_slice(&sequence.to_le_bytes());
                wire[72..104].copy_from_slice(&prior);
                if let Some(ack) = &record.last_ack {
                    wire[ACK..OCCURRENCE].copy_from_slice(ack.canonical_bytes());
                }
                wire[OCCURRENCE..BODY].copy_from_slice(&occurrence);
                match &record.body {
                    Body::Ready => wire[24] = 1,
                    Body::Prepared { subject, challenge } => {
                        wire[24] = 2;
                        wire[BODY..BODY + S].copy_from_slice(subject.canonical_bytes());
                        wire[BODY + S..BODY + S + H].copy_from_slice(challenge.canonical_bytes());
                    }
                    Body::Issued { request, receipt } => {
                        wire[24] = 3;
                        wire[BODY..BODY + Q].copy_from_slice(request.canonical_bytes());
                        wire[BODY + Q..SIGNATURE].copy_from_slice(receipt.canonical_bytes());
                    }
                }
                let digest = hash(SIGN_DOMAIN, &wire[..SIGNATURE]);
                let (signature, charge) = key.sign_journal_digest(p, &digest, b)?;
                b.reserve_storage(charge.additional_storage())?;
                wire[SIGNATURE..IDENTITY].copy_from_slice(&signature);
                let identity = hash(ID_DOMAIN, &wire[..IDENTITY]);
                wire[IDENTITY..].copy_from_slice(&identity);
                Ok(record)
            },
        )
    }

    pub fn decode(bytes: &[u8], p: &Policy, b: &mut Budget<'_>) -> Result<Self> {
        b.with_prepaid_scope(
            if bytes.len() == BYTES { BYTES } else { 0 },
            8,
            Self::WORK,
            Self::FRAME,
            |b| {
                if bytes.len() != BYTES
                    || &bytes[..8] != MAGIC
                    || bytes[8..10] != 3u16.to_le_bytes()
                    || bytes[10..12] != [0; 2]
                    || bytes[12..20] != (BYTES as u64).to_le_bytes()
                    || bytes[20..24] != [0; 4]
                    || bytes[25..32] != [0; 7]
                    || &bytes[32..64] != p.identity().as_bytes()
                    || bytes[IDENTITY..] != hash(ID_DOMAIN, &bytes[..IDENTITY])
                {
                    return Err(Error::rejected("invalid native issuer journal frame"));
                }
                VerifyingKey::from_bytes(p.verifying_key())
                    .map_err(|_| Error::rejected("invalid issuer journal key"))?
                    .verify_strict(
                        &hash(SIGN_DOMAIN, &bytes[..SIGNATURE]),
                        &Signature::from_bytes(&fixed(&bytes[SIGNATURE..IDENTITY])?),
                    )
                    .map_err(|_| Error::rejected("native issuer journal signature mismatch"))?;
                let last_ack = if bytes[ACK..OCCURRENCE].iter().all(|x| *x == 0) {
                    None
                } else {
                    Some(retain(Ack::decode(&bytes[ACK..OCCURRENCE], b)?, b)?)
                };
                let body = match bytes[24] {
                    1 if bytes[BODY..SIGNATURE].iter().all(|x| *x == 0) => Body::Ready,
                    2 if bytes[BODY + S + H..SIGNATURE].iter().all(|x| *x == 0) => {
                        let (subject, charge) = Subject::decode(&bytes[BODY..BODY + S], b)?;
                        b.reserve_storage(charge.retained_storage())?;
                        let challenge =
                            retain(Challenge::decode(&bytes[BODY + S..BODY + S + H], b)?, b)?;
                        Body::Prepared { subject, challenge }
                    }
                    3 => Body::Issued {
                        request: retain(Request::decode(&bytes[BODY..BODY + Q], b)?, b)?,
                        receipt: retain(Receipt::decode(&bytes[BODY + Q..SIGNATURE], b)?, b)?,
                    },
                    _ => {
                        return Err(Error::rejected(
                            "invalid native issuer journal stage payload",
                        ));
                    }
                };
                let record = Self {
                    sequence: u64::from_le_bytes(fixed(&bytes[64..72])?),
                    prior: fixed(&bytes[72..104])?,
                    last_ack,
                    occurrence: fixed(&bytes[OCCURRENCE..BODY])?,
                    body,
                    wire: fixed(bytes)?,
                };
                record.check(p, b)?;
                Ok(record)
            },
        )
    }

    fn check(&self, p: &Policy, b: &mut Budget<'_>) -> Result<()> {
        // Recovery may mutate names immediately after decode. Reject positions
        // needing the unimplemented Worker/anchor join before returning a record.
        if self.sequence != 1 || self.prior != [0; 32] || self.last_ack.is_some() {
            return Err(Error::rejected(
                "native journal requires the unjoined Worker/anchor ledger",
            ));
        }
        match &self.body {
            Body::Ready if self.occurrence == [0; 32] => Ok(()),
            Body::Prepared { subject, challenge }
                if self.occurrence != [0; 32]
                    && challenge.policy_identity() == p.identity()
                    && challenge.sequence() == self.sequence
                    && challenge.prior_rollback_anchor() == self.prior
                    && challenge.subject().matches_subject(subject, b)? =>
            {
                Ok(())
            }
            Body::Issued { request, receipt }
                if self.occurrence != [0; 32]
                    && request.challenge().sequence() == self.sequence
                    && request.challenge().prior_rollback_anchor() == self.prior =>
            {
                let receipt = retain(Receipt::decode(receipt.canonical_bytes(), b)?, b)?;
                let (_, charge) = receipt.verify(p, request, self.prior, b)?;
                b.reserve_storage(charge.additional_storage())?;
                Ok(())
            }
            _ => Err(Error::rejected("issuer journal payload binding mismatch")),
        }
    }

    pub fn prepare(
        &self,
        p: &Policy,
        key: &Key,
        occurrence: &NativeOccurrence,
        nonce: [u8; 32],
        b: &mut Budget<'_>,
    ) -> Result<Self> {
        if !matches!(self.body, Body::Ready) {
            return Err(Error::rejected("prepare requires ready journal"));
        }
        let (subject, charge) = Subject::decode(occurrence.subject().canonical_bytes(), b)?;
        b.reserve_storage(charge.retained_storage())?;
        let challenge = retain(
            Challenge::new(p, &subject, nonce, self.sequence, self.prior, b)?,
            b,
        )?;
        Self::build(
            p,
            key,
            self.sequence,
            self.prior,
            self.copy_ack(b)?,
            *occurrence.identity(),
            Body::Prepared { subject, challenge },
            b,
        )
    }
    pub fn issue(
        &self,
        p: &Policy,
        key: &Key,
        occurrence: &NativeOccurrence,
        request: Request,
        b: &mut Budget<'_>,
    ) -> Result<Self> {
        let Body::Prepared { subject, challenge } = &self.body else {
            return Err(Error::rejected("issue requires prepared journal"));
        };
        if occurrence.identity() != &self.occurrence
            || occurrence.subject().canonical_bytes() != subject.canonical_bytes()
            || request.subject().canonical_bytes() != subject.canonical_bytes()
            || request.challenge().canonical_bytes() != challenge.canonical_bytes()
        {
            return Err(Error::rejected(
                "issue differs from the retained occurrence/challenge",
            ));
        }
        let receipt = retain(key.issue_receipt(p, &request, b)?, b)?;
        Self::build(
            p,
            key,
            self.sequence,
            self.prior,
            self.copy_ack(b)?,
            self.occurrence,
            Body::Issued { request, receipt },
            b,
        )
    }
    pub fn publication(&self, b: &mut Budget<'_>) -> Result<Publication> {
        let Body::Issued { receipt, .. } = &self.body else {
            return Err(Error::rejected("journal has no issued receipt"));
        };
        let receipt = retain(Receipt::decode(receipt.canonical_bytes(), b)?, b)?;
        retain(
            Publication::new(self.identity(), self.occurrence, receipt, b)?,
            b,
        )
    }
    fn copy_ack(&self, b: &mut Budget<'_>) -> Result<Option<Ack>> {
        self.last_ack
            .as_ref()
            .map(|ack| retain(Ack::decode(ack.canonical_bytes(), b)?, b))
            .transpose()
    }
    pub fn bytes(&self) -> &[u8; BYTES] {
        &self.wire
    }
    pub fn identity(&self) -> [u8; 32] {
        hash(ID_DOMAIN, &self.wire[..IDENTITY])
    }

    pub fn successor(prior: Option<&Self>, next: &Self, b: &mut Budget<'_>) -> Result<()> {
        let Some(prior) = prior else {
            return if next.sequence == 1 && matches!(next.body, Body::Ready) {
                Ok(())
            } else {
                Err(Error::rejected("native issuer genesis is not ready"))
            };
        };
        let same_ack = prior.last_ack.as_ref().map(Ack::canonical_bytes)
            == next.last_ack.as_ref().map(Ack::canonical_bytes);
        let legal = match (&prior.body, &next.body) {
            (Body::Ready, Body::Prepared { .. }) => {
                same_ack && prior.sequence == next.sequence && prior.prior == next.prior
            }
            (Body::Prepared { subject, challenge }, Body::Issued { request, .. }) => {
                same_ack
                    && prior.sequence == next.sequence
                    && prior.prior == next.prior
                    && prior.occurrence == next.occurrence
                    && subject.canonical_bytes() == request.subject().canonical_bytes()
                    && challenge.canonical_bytes() == request.challenge().canonical_bytes()
            }
            (Body::Issued { .. }, Body::Ready) => {
                if let Some(ack) = &next.last_ack {
                    ack.matches_publication(&prior.publication(b)?, b)?;
                    prior.sequence.checked_add(1) == Some(next.sequence)
                        && ack.current_rollback_anchor() == next.prior
                } else {
                    false
                }
            }
            _ => false,
        };
        if legal {
            Ok(())
        } else {
            Err(Error::rejected("illegal native issuer journal successor"))
        }
    }
}

#[cfg(test)]
pub(super) fn signed_unsupported_ready_for_test(
    p: &Policy,
    key: &Key,
    b: &mut Budget<'_>,
) -> Result<[u8; BYTES]> {
    b.with_prepaid_scope(
        p.retained_storage() + key.retained_storage(),
        8,
        Record::WORK,
        Record::FRAME,
        |b| {
            // Construct an inert ACK claim, not Worker publication authority.
            // Its public decoder must accept it before it enters the signed fixture.
            let mut ack = [0; A];
            ack[..8].copy_from_slice(b"F2O3CEA2");
            ack[8..10].copy_from_slice(&2u16.to_le_bytes());
            ack[12..20].copy_from_slice(&(A as u64).to_le_bytes());
            ack[24..56].copy_from_slice(p.identity().as_bytes());
            for (field, value) in ack[56..216].chunks_exact_mut(32).zip(1u8..=5) {
                field.fill(value);
            }
            ack[216..224].copy_from_slice(&1u64.to_le_bytes());
            ack[224..256].fill(6);
            let mut digest = Sha256::new();
            digest.update(b"FE2O3/COMPILER-EXECUTION-RECEIPT-PUBLICATION-ACK/V2\0");
            digest.update(256u64.to_le_bytes());
            digest.update(&ack[..256]);
            ack[256..].copy_from_slice(&digest.finalize());
            let ack = retain(Ack::decode(&ack, b)?, b)?;
            assert_eq!(ack.policy_identity(), p.identity());
            assert_eq!(ack.sequence().checked_add(1), Some(2));
            assert_eq!(ack.current_rollback_anchor(), [6; 32]);

            let mut wire = *Record::genesis(p, key, b)?.bytes();
            wire[64..72].copy_from_slice(&2u64.to_le_bytes());
            wire[72..104].copy_from_slice(&ack.current_rollback_anchor());
            wire[ACK..OCCURRENCE].copy_from_slice(ack.canonical_bytes());
            let digest = hash(SIGN_DOMAIN, &wire[..SIGNATURE]);
            let (signature, charge) = key.sign_journal_digest(p, &digest, b)?;
            b.reserve_storage(charge.additional_storage())?;
            VerifyingKey::from_bytes(p.verifying_key())
                .unwrap()
                .verify_strict(&digest, &Signature::from_bytes(&signature))
                .unwrap();
            wire[SIGNATURE..IDENTITY].copy_from_slice(&signature);
            let identity = hash(ID_DOMAIN, &wire[..IDENTITY]);
            wire[IDENTITY..].copy_from_slice(&identity);
            Ok(wire)
        },
    )
}

fn hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(domain);
    h.update(bytes);
    h.finalize().into()
}
fn fixed<const N: usize>(bytes: &[u8]) -> Result<[u8; N]> {
    bytes
        .try_into()
        .map_err(|_| Error::rejected("incorrect fixed native journal width"))
}
