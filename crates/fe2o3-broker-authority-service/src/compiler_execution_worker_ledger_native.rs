//! Native Worker records consumed under the issuer's one directory singleton.
use super::ledger::Stored;
use super::*;
use crate::compiler_execution_journal_recovery::{
    JournalNames, NATIVE_ANCHOR_STATE_FILES, NATIVE_WORKER_STATE_FILES,
};
use fe2o3_artifact_transaction::{
    NoRetainedDurableDirectoryHooksV1 as NoHooks, RetainedDurableDirectoryHooksV1 as Hooks,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_TRANSACTION_BYTES_V2 as T,
    COMPILER_EXECUTION_WORKER_ANCHOR_JOURNAL_BYTES_V2 as A,
    CompilerExecutionCurrentRecordVerificationV3 as Current,
    CompilerExecutionExternalAnchorTransactionV2 as Transaction,
    CompilerExecutionReceiptCarriageV2 as Carriage,
    CompilerExecutionWorkerAnchorJournalStageV1 as Stage,
    CompilerExecutionWorkerAnchorJournalV2 as AnchorJournal,
};
use fe2o3_external_anchor_protocol::{
    ANCHOR_TRANSITION_RECEIPT_BYTES_V1 as R, AnchorChallengeV1 as AnchorChallenge,
    AnchorPositionV1, AnchorTransitionReceiptV1 as AnchorReceipt, AnchoredStateV1, CallerNonceV1,
    ChallengeKindV1, HashChainHeadV1, PinnedAnchorKeyV1,
};
use sha2::{Digest, Sha256};
use std::mem::size_of;

const N: usize = 24 + T + R + 32;
pub(super) const WORKER: JournalNames = JournalNames {
    canonical: NATIVE_WORKER_STATE_FILES[0],
    redo: NATIVE_WORKER_STATE_FILES[1],
    recovery: NATIVE_WORKER_STATE_FILES[2],
    maximum_bytes: N,
};
pub(super) const ANCHOR: JournalNames = JournalNames {
    canonical: NATIVE_ANCHOR_STATE_FILES[0],
    redo: NATIVE_ANCHOR_STATE_FILES[1],
    recovery: NATIVE_ANCHOR_STATE_FILES[2],
    maximum_bytes: A,
};

pub(super) struct WorkerRecord {
    transaction: Transaction,
    receipt: AnchorReceipt,
    identity: [u8; 32],
    bytes: [u8; N],
}
impl WorkerRecord {
    fn new(t: &Transaction, receipt: &AnchorReceipt, b: &mut Budget<'_>) -> Result<Self> {
        b.with_prepaid_scope(
            t.retained_storage() + size_of::<AnchorReceipt>(),
            8,
            1024 * N,
            8 * Self::STORAGE + 8192,
            |b| {
                let key =
                    PinnedAnchorKeyV1::from_bytes(*t.policy().external_anchor_verifying_key())?;
                let receipt = AnchorReceipt::decode(receipt.canonical_bytes(), &key)?;
                let c = receipt.challenge();
                if receipt.position() != AnchorPositionV1::Proposed
                    || c.kind() != ChallengeKindV1::Advance
                    || c.expected_sequence() != t.sequence()
                    || c.transaction() != t.external_anchor_digest(b)?
                    || ((t.sequence() == 1)
                        != (c.prior_head() == HashChainHeadV1::from_bytes([0; 32])))
                {
                    return Err(Error::rejected("native Worker external commit mismatch"));
                }
                let (transaction, charge) = Transaction::decode(t.canonical_bytes(), b)?;
                b.reserve_storage(charge.additional_storage())?;
                let mut bytes = [0; N];
                bytes[..8].copy_from_slice(b"F2O3CEW3");
                bytes[8..10].copy_from_slice(&3u16.to_le_bytes());
                bytes[12..20].copy_from_slice(&(N as u64).to_le_bytes());
                bytes[24..24 + T].copy_from_slice(t.canonical_bytes());
                bytes[24 + T..N - 32].copy_from_slice(receipt.canonical_bytes());
                let identity = digest(b"FE2O3/NATIVE-WORKER-RECORD/V3\0", &[&bytes[..N - 32]]);
                bytes[N - 32..].copy_from_slice(&identity);
                Ok(Self {
                    transaction,
                    receipt,
                    identity,
                    bytes,
                })
            },
        )
    }
    fn ack(&self, b: &mut Budget<'_>) -> Result<Ack> {
        retain(
            Ack::new(self.transaction.publication(), self.identity, b)?,
            b,
        )
    }
    fn carriage(&self, b: &mut Budget<'_>) -> Result<Carriage> {
        let p = retain(
            Policy::decode(self.transaction.policy().canonical_bytes(), b)?,
            b,
        )?;
        let q = retain(
            Request::decode(self.transaction.request().canonical_bytes(), b)?,
            b,
        )?;
        let u = retain(
            Publication::decode(self.transaction.publication().canonical_bytes(), b)?,
            b,
        )?;
        let ack = self.ack(b)?;
        retain(Carriage::new(p, q, u, ack, b)?, b)
    }
    fn matches(&self, t: &Transaction) -> bool {
        self.transaction.canonical_bytes() == t.canonical_bytes()
    }
}
impl Stored for WorkerRecord {
    const STORAGE: usize = size_of::<Self>() + 64;
    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    fn decode(bytes: &[u8], p: &Policy, b: &mut Budget<'_>) -> Result<Self> {
        b.with_prepaid_scope(
            bytes.len().min(N),
            8,
            1024 * N,
            8 * Self::STORAGE + 8192,
            |b| {
                if bytes.len() != N {
                    return Err(Error::rejected("native Worker record width"));
                }
                let (t, charge) = Transaction::decode(&bytes[24..24 + T], b)?;
                b.reserve_storage(charge.additional_storage())?;
                if t.policy().canonical_bytes() != p.canonical_bytes() {
                    return Err(Error::rejected("native Worker policy mismatch"));
                }
                let key = PinnedAnchorKeyV1::from_bytes(*p.external_anchor_verifying_key())?;
                let receipt = AnchorReceipt::decode(&bytes[24 + T..N - 32], &key)?;
                b.reserve_storage(size_of::<AnchorReceipt>())?;
                let record = Self::new(&t, &receipt, b)?;
                if record.bytes.as_slice() != bytes {
                    return Err(Error::rejected("native Worker noncanonical record"));
                }
                Ok(record)
            },
        )
    }
    fn successor(prior: Option<&Self>, next: &Self, _: &mut Budget<'_>) -> Result<()> {
        let t = &next.transaction;
        let legal = match prior {
            None => t.sequence() == 1 && t.prior_rollback_anchor() == [0; 32],
            Some(p) => {
                p.transaction.sequence().checked_add(1) == Some(t.sequence())
                    && t.prior_rollback_anchor() == p.transaction.current_rollback_anchor()
                    && next.receipt.challenge().prior_head()
                        == p.receipt.challenge().proposed_head()
                    && t.policy().canonical_bytes() == p.transaction.policy().canonical_bytes()
            }
        };
        if legal {
            Ok(())
        } else {
            Err(Error::rejected("illegal native Worker successor"))
        }
    }
}
impl Stored for AnchorJournal {
    const STORAGE: usize = Self::RETAINED_STORAGE;
    fn bytes(&self) -> &[u8] {
        self.canonical_bytes()
    }
    fn decode(bytes: &[u8], p: &Policy, b: &mut Budget<'_>) -> Result<Self> {
        let (v, _) = AnchorJournal::decode(bytes, b)?;
        if v.transaction().policy().canonical_bytes() != p.canonical_bytes() {
            return Err(Error::rejected("native anchor journal policy mismatch"));
        }
        Ok(v)
    }
    fn successor(prior: Option<&Self>, next: &Self, b: &mut Budget<'_>) -> Result<()> {
        let legal = match prior {
            None => next.is_genesis_prepared(),
            Some(p) => next.is_legal_successor_of(p, b)?,
        };
        if legal {
            Ok(())
        } else {
            Err(Error::rejected("illegal native anchor successor"))
        }
    }
}

pub(super) fn validate_join(
    i: &Record,
    w: Option<&WorkerRecord>,
    a: Option<&AnchorJournal>,
    b: &mut Budget<'_>,
) -> Result<()> {
    let issuer_matches = match w {
        None => i.sequence == 1 && i.prior == [0; 32] && i.last_ack.is_none(),
        Some(w) => {
            let t = &w.transaction;
            let ack = w.ack(b)?;
            (t.sequence().checked_add(1) == Some(i.sequence)
                && t.current_rollback_anchor() == i.prior
                && i.last_ack.as_ref().map(Ack::canonical_bytes) == Some(ack.canonical_bytes()))
                || issued_matches(i, t, b)?
        }
    };
    if !issuer_matches {
        return Err(Error::rejected(
            "native journal requires the unjoined Worker/anchor ledger",
        ));
    }
    let Some(a) = a else {
        return if w.is_none() {
            Ok(())
        } else {
            Err(Error::rejected("native Worker missing anchor journal"))
        };
    };
    let t = a.transaction();
    if a.stage() == Stage::Published {
        return if w.is_some_and(|w| {
            w.matches(t)
                && w.identity == a.worker_record_identity()
                && a.receipt() == Some(&w.receipt)
        }) {
            Ok(())
        } else {
            Err(Error::rejected("native published anchor/Worker mismatch"))
        };
    }
    if !issued_matches(i, t, b)? {
        return Err(Error::rejected("native pending anchor/issuer mismatch"));
    }
    if a.stage() == Stage::AnchorCommitted
        && w.is_some_and(|w| w.matches(t) && a.receipt() == Some(&w.receipt))
    {
        return Ok(());
    }
    let prior = match w {
        None => {
            t.sequence() == 1
                && t.prior_rollback_anchor() == [0; 32]
                && a.challenge().prior_head() == HashChainHeadV1::from_bytes([0; 32])
        }
        Some(w) => {
            w.transaction.sequence().checked_add(1) == Some(t.sequence())
                && w.transaction.current_rollback_anchor() == t.prior_rollback_anchor()
                && w.receipt.challenge().proposed_head() == a.challenge().prior_head()
        }
    };
    if prior {
        Ok(())
    } else {
        Err(Error::rejected(
            "native pending anchor/Worker position mismatch",
        ))
    }
}
fn issued_matches(i: &Record, t: &Transaction, b: &mut Budget<'_>) -> Result<bool> {
    match &i.body {
        Body::Issued { request, .. } => Ok(request.canonical_bytes()
            == t.request().canonical_bytes()
            && i.sequence == t.sequence()
            && i.prior == t.prior_rollback_anchor()
            && i.publication(b)?.canonical_bytes() == t.publication().canonical_bytes()),
        _ => Ok(false),
    }
}

impl Ledger {
    pub(super) fn publish(
        &mut self,
        p: &Policy,
        key: &Key,
        request: &Request,
        publication: &Publication,
        exchange: &mut impl FnMut(&AnchorChallenge, &mut Budget<'_>) -> Result<AnchorReceipt>,
        b: &mut Budget<'_>,
    ) -> Result<(Ack, bool)> {
        self.publish_with_hooks(p, key, request, publication, exchange, &mut NoHooks, b)
    }

    pub(super) fn publish_with_hooks(
        &mut self,
        p: &Policy,
        key: &Key,
        request: &Request,
        publication: &Publication,
        exchange: &mut impl FnMut(&AnchorChallenge, &mut Budget<'_>) -> Result<AnchorReceipt>,
        hooks: &mut impl Hooks,
        b: &mut Budget<'_>,
    ) -> Result<(Ack, bool)> {
        let floor = Self::STORAGE
            + p.retained_storage()
            + key.retained_storage()
            + request.retained_storage()
            + publication.retained_storage();
        b.with_prepaid_scope(floor, 8, 1024 * Self::STORAGE, 32 * Self::STORAGE, |b| {
            self.validate(b)?;
            let policy = retain(Policy::decode(p.canonical_bytes(), b)?, b)?;
            let q = retain(Request::decode(request.canonical_bytes(), b)?, b)?;
            let u = retain(Publication::decode(publication.canonical_bytes(), b)?, b)?;
            let t = retain(Transaction::new(policy, q, u, b)?, b)?;
            if matches!(self.record.body, Body::Ready) && self.record.sequence > 1 {
                let w = self
                    .worker
                    .as_ref()
                    .ok_or_else(|| Error::rejected("native replay missing Worker"))?;
                if !w.matches(&t) {
                    return Err(Error::rejected("native publication replay substitution"));
                }
                return Ok((w.ack(b)?, false));
            }
            if !issued_matches(&self.record, &t, b)? {
                return Err(Error::rejected(
                    "native publication differs from issued occurrence",
                ));
            }
            let existing = self
                .anchor
                .as_ref()
                .filter(|a| a.transaction().canonical_bytes() == t.canonical_bytes());
            if existing.is_none() {
                let stable = match &self.anchor {
                    None if self.worker.is_none() => {
                        AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
                    }
                    Some(a) if a.stage() == Stage::Published => AnchoredStateV1::from_local_state(
                        a.transaction().sequence(),
                        a.challenge().proposed_head(),
                    ),
                    Some(a) if a.stage() == Stage::Aborted => AnchoredStateV1::from_local_state(
                        a.transaction().sequence() - 1,
                        a.challenge().prior_head(),
                    ),
                    _ => return Err(Error::rejected("native anchor transaction already active")),
                };
                let anchor_key = PinnedAnchorKeyV1::from_bytes(*p.external_anchor_verifying_key())?;
                let prepared = stable.prepare(t.external_anchor_digest(b)?, &anchor_key)?;
                let pending = prepared
                    .begin_advance(CallerNonceV1::from_bytes(fresh_nonce(b)?), &anchor_key)?;
                b.reserve_storage(size_of::<AnchorChallenge>())?;
                let next = retain(AnchorJournal::prepared(&t, pending.challenge(), b)?, b)?;
                self.commit_anchor(next, hooks, b)?;
            }
            let anchor = self
                .anchor
                .as_ref()
                .ok_or_else(|| Error::rejected("native prepared anchor missing"))?;
            if anchor.stage() == Stage::PreparedAnchor {
                // The exact challenge is durably reacquired before it leaves
                // the service. A replay never draws a replacement nonce.
                self.validate(b)?;
                let receipt = exchange(anchor.challenge(), b)?;
                b.reserve_storage(size_of::<AnchorReceipt>())?;
                self.validate(b)?;
                let next = retain(anchor.record_anchor_receipt(&receipt, b)?, b)?;
                self.commit_anchor(next, hooks, b)?;
            }
            let anchor = self
                .anchor
                .as_ref()
                .ok_or_else(|| Error::rejected("native committed anchor missing"))?;
            if anchor.stage() == Stage::Aborted {
                return Err(Error::rejected(
                    "native external anchor refused the proposed position",
                ));
            }
            if anchor.stage() == Stage::AnchorCommitted {
                let receipt = anchor
                    .receipt()
                    .ok_or_else(|| Error::rejected("native anchor receipt missing"))?;
                let next = WorkerRecord::new(anchor.transaction(), receipt, b)?;
                b.reserve_storage(WorkerRecord::STORAGE)?;
                if self.worker.as_ref().is_none_or(|w| w.bytes != next.bytes) {
                    self.poisoned = true;
                    ledger::commit_named(
                        &self.store,
                        &WORKER,
                        self.worker.as_ref(),
                        &next,
                        hooks,
                        b,
                    )?;
                    self.worker = Some(next);
                    validate_join(&self.record, self.worker.as_ref(), self.anchor.as_ref(), b)?;
                    self.poisoned = false;
                }
                self.validate(b)?;
                let w = self
                    .worker
                    .as_ref()
                    .ok_or_else(|| Error::rejected("native committed Worker missing"))?;
                let anchor = self
                    .anchor
                    .as_ref()
                    .ok_or_else(|| Error::rejected("native committed anchor missing"))?;
                let next = retain(anchor.mark_published(w.identity, b)?, b)?;
                self.commit_anchor(next, hooks, b)?;
            }
            self.validate(b)?;
            let w = self
                .worker
                .as_ref()
                .ok_or_else(|| Error::rejected("native published Worker missing"))?;
            let ack = w.ack(b)?;
            let next = self.record.advance(&ack, p, key, b)?;
            b.reserve_storage(Record::STORAGE)?;
            self.commit_with_hooks(next, hooks, b)?;
            self.validate(b)?;
            Ok((ack, true))
        })
    }

    fn commit_anchor(
        &mut self,
        next: AnchorJournal,
        hooks: &mut impl Hooks,
        b: &mut Budget<'_>,
    ) -> Result<()> {
        self.validate(b)?;
        validate_join(&self.record, self.worker.as_ref(), Some(&next), b)?;
        self.poisoned = true;
        ledger::commit_named(&self.store, &ANCHOR, self.anchor.as_ref(), &next, hooks, b)?;
        self.anchor = Some(next);
        self.poisoned = false;
        if let Err(e) = self.validate(b) {
            self.poisoned = true;
            return Err(e);
        }
        Ok(())
    }

    pub(super) fn recover_carriage(
        &self,
        subject: &Subject,
        b: &mut Budget<'_>,
    ) -> Result<Option<Carriage>> {
        b.with_prepaid_scope(
            Self::STORAGE + size_of::<Subject>(),
            8,
            1024 * Self::STORAGE,
            16 * Self::STORAGE,
            |b| {
                self.validate(b)?;
                let Some(w) = self.completed_worker(b)? else {
                    return Ok(None);
                };
                // Recover is a query for this exact subject, never a subject source
                // for issuance. Prepare/Issue still independently inspect rustc.
                if w.transaction.request().subject().canonical_bytes() != subject.canonical_bytes()
                {
                    return Ok(None);
                }
                Ok(Some(w.carriage(b)?))
            },
        )
    }

    // A valid crash position is not a completed publication. No ACK carriage or
    // currentness signature may escape between Worker commit and issuer advance.
    fn completed_worker(&self, b: &mut Budget<'_>) -> Result<Option<&WorkerRecord>> {
        let (Some(w), Some(a)) = (&self.worker, &self.anchor) else {
            return Ok(None);
        };
        let ack = w.ack(b)?;
        let completed = a.stage() == Stage::Published
            && a.worker_record_identity() == w.identity
            && w.matches(a.transaction())
            && a.receipt() == Some(&w.receipt)
            && w.transaction.sequence().checked_add(1) == Some(self.record.sequence)
            && self.record.prior == w.transaction.current_rollback_anchor()
            && self.record.last_ack.as_ref().map(Ack::canonical_bytes)
                == Some(ack.canonical_bytes());
        Ok(completed.then_some(w))
    }

    pub(super) fn verify_current(
        &self,
        expected: &Carriage,
        challenge: [u8; 32],
        exchange: &mut impl FnMut(&AnchorChallenge, &mut Budget<'_>) -> Result<AnchorReceipt>,
        b: &mut Budget<'_>,
    ) -> Result<Current> {
        b.with_prepaid_scope(
            Self::STORAGE + expected.retained_storage(),
            8,
            1024 * Self::STORAGE,
            32 * Self::STORAGE,
            |b| {
                self.validate(b)?;
                let w = self
                    .completed_worker(b)?
                    .ok_or_else(|| Error::rejected("native current publication is incomplete"))?;
                let carriage = w.carriage(b)?;
                if carriage.canonical_bytes() != expected.canonical_bytes() {
                    return Err(Error::rejected("native current carriage substitution"));
                }
                let c = retain(
                    Current::external_anchor_currentness_challenge_native(
                        &carriage, &w.receipt, challenge, b,
                    )?,
                    b,
                )?;
                let current = exchange(&c, b)?;
                b.reserve_storage(2 * size_of::<AnchorReceipt>())?;
                self.validate(b)?;
                let policy_join = digest(
                    b"FE2O3/NATIVE-PROTECTED-POLICY-VERIFICATION/V2\0",
                    &[
                        w.transaction.policy().canonical_bytes(),
                        w.transaction.request().subject().canonical_bytes(),
                        carriage.canonical_bytes(),
                        &w.identity,
                    ],
                );
                let worker_join = digest(
                    b"FE2O3/NATIVE-PROTECTED-WORKER-VERIFICATION/V2\0",
                    &[&w.bytes, carriage.canonical_bytes(), &policy_join],
                );
                retain(
                    Current::new_native(
                        &carriage,
                        w.receipt.clone(),
                        current,
                        challenge,
                        policy_join,
                        worker_join,
                        b,
                    )?,
                    b,
                )
            },
        )
    }
}

fn digest(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(domain);
    for part in parts {
        h.update((part.len() as u64).to_le_bytes());
        h.update(part);
    }
    h.finalize().into()
}
