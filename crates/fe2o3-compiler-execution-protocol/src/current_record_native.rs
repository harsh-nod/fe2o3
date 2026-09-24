//! Typed native-carriage authentication over the shared identity-only V3 wire.
//! No admitted V1 policy/carriage is constructed and no protected authority is minted.
use super::*;
use crate::{
    CompilerExecutionExternalAnchorTransactionV2 as Transaction,
    CompilerExecutionIssuerPolicyV2 as NativePolicy,
    CompilerExecutionNativeJournalErrorV2 as NativeError,
    CompilerExecutionReceiptCarriageV2 as NativeCarriage,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    external_anchor_transaction_v2::Result,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::mem::size_of;

const VERIFICATION_STORAGE: usize =
    size_of::<(CompilerExecutionCurrentRecordVerificationV3, Storage)>();
const ATTESTATION_STORAGE: usize =
    size_of::<(CompilerExecutionCurrentRecordAttestationV3, Storage)>();
// Covers repeated verification of both external receipts, public-key checks,
// issuer signing/verification, fixed hashing/copies and guarded result frames.
// Transaction/receipt reconstruction charges its nested work separately.
const WORK: usize = 8
    + 16 * (resources::STRICT_VERIFY_WORK + resources::KEY_VALIDATION_WORK)
    + 64 * (crate::COMPILER_EXECUTION_RECEIPT_CARRIAGE_BYTES_V2
        + COMPILER_EXECUTION_CURRENT_RECORD_ATTESTATION_BYTES_V3);
const SCRATCH: usize = 16 * ATTESTATION_STORAGE + 8192;

impl CompilerExecutionCurrentRecordVerificationV3 {
    /// Consumes prepaid anchor observations and borrows an exact native carriage.
    /// Returns a full output charge; retire consumed anchor storage after return.
    pub fn new_native(
        carriage: &NativeCarriage,
        commit: AnchorTransitionReceiptV1,
        current: AnchorTransitionReceiptV1,
        challenge: [u8; 32],
        protected_policy: [u8; 32],
        protected_worker: [u8; 32],
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        let floor = carriage.retained_storage() + 2 * size_of::<AnchorTransitionReceiptV1>();
        b.with_prepaid_scope(floor, 8, WORK, SCRATCH, |b| {
            validate_native_commit(carriage, &commit, b)?;
            let key = *carriage.policy().external_anchor_verifying_key();
            let expected = build_external_anchor_currentness_challenge_for(
                *carriage.identity().as_bytes(), key, &commit, challenge,
            )?;
            reverify_external_anchor_receipt(key, &current)?;
            if current.position() != AnchorPositionV1::Proposed || current.challenge() != &expected {
                return Err(NativeError::Current(
                    CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchorCurrentnessReceiptMismatch,
                ));
            }
            let record = Self::encode(FieldsV3 {
                policy_identity: *carriage.policy().identity().as_bytes(),
                subject_identity: *carriage.request().subject().identity().sha256(),
                carriage_identity: *carriage.identity().as_bytes(),
                issuer_journal_identity: carriage.acknowledgment().issuer_journal_identity(),
                worker_ledger_record_identity: carriage.acknowledgment().worker_ledger_record_identity(),
                sequence: carriage.acknowledgment().sequence(),
                prior_rollback_anchor: carriage.publication().receipt().prior_rollback_anchor(),
                current_rollback_anchor: carriage.acknowledgment().current_rollback_anchor(),
                external_anchor_verifying_key: key,
                external_anchor_commit_receipt: commit,
                external_anchor_currentness_receipt: current,
                protected_policy_verification_identity: protected_policy,
                protected_worker_ledger_verification_identity: protected_worker,
            })?;
            Ok((record, Storage(VERIFICATION_STORAGE)))
        })
    }

    /// Binds the fresh client challenge to native carriage and committed anchor.
    pub fn external_anchor_currentness_challenge_native(
        carriage: &NativeCarriage,
        commit: &AnchorTransitionReceiptV1,
        challenge: [u8; 32],
        b: &mut Budget<'_>,
    ) -> Result<(AnchorChallengeV1, Storage)> {
        b.with_prepaid_scope(
            carriage.retained_storage() + size_of::<AnchorTransitionReceiptV1>(),
            8,
            WORK,
            SCRATCH,
            |b| {
                validate_native_commit(carriage, commit, b)?;
                let challenge = build_external_anchor_currentness_challenge_for(
                    *carriage.identity().as_bytes(),
                    *carriage.policy().external_anchor_verifying_key(),
                    commit,
                    challenge,
                )?;
                Ok((
                    challenge,
                    Storage(size_of::<(AnchorChallengeV1, Storage)>()),
                ))
            },
        )
    }

    fn verify_expected_native(
        &self,
        policy: &NativePolicy,
        carriage: &NativeCarriage,
        challenge: [u8; 32],
        b: &mut Budget<'_>,
    ) -> Result<()> {
        if policy.canonical_bytes() != carriage.policy().canonical_bytes() {
            return Err(NativeError::Current(
                CompilerExecutionCurrentRecordVerificationErrorV3::PolicyMismatch,
            ));
        }
        let (expected, _) = Self::new_native(
            carriage,
            self.fields.external_anchor_commit_receipt.clone(),
            self.fields.external_anchor_currentness_receipt.clone(),
            challenge,
            self.fields.protected_policy_verification_identity,
            self.fields.protected_worker_ledger_verification_identity,
            b,
        )?;
        if expected != *self {
            return Err(NativeError::Current(
                CompilerExecutionCurrentRecordVerificationErrorV3::VerificationMismatch,
            ));
        }
        Ok(())
    }
}

impl CompilerExecutionCurrentRecordAttestationV3 {
    /// Signs a native current-record join. Protected journal/key custody remains
    /// the consuming issuer's obligation; this protocol operation alone is inert.
    /// Consumes the prepaid verification; returns only additional output storage.
    pub fn issue_native(
        policy: &NativePolicy,
        carriage: &NativeCarriage,
        verification: CompilerExecutionCurrentRecordVerificationV3,
        challenge: [u8; 32],
        key: &SigningKey,
        b: &mut Budget<'_>,
    ) -> Result<(Self, Storage)> {
        let floor = policy.retained_storage()
            + carriage.retained_storage()
            + VERIFICATION_STORAGE
            + size_of::<SigningKey>();
        b.with_prepaid_scope(floor, 8, WORK, SCRATCH, |b| {
            if key.verifying_key().as_bytes() != policy.verifying_key() {
                return Err(NativeError::Current(
                    CompilerExecutionCurrentRecordVerificationErrorV3::SigningKeyMismatch,
                ));
            }
            verification.verify_expected_native(policy, carriage, challenge, b)?;
            Ok((
                Self::sign_checked(verification, challenge, key)?,
                Storage(ATTESTATION_STORAGE - VERIFICATION_STORAGE),
            ))
        })
    }

    /// Consumes the prepaid V3 wire owner, checking both pinned keys and the
    /// exact native carriage/fresh challenge. No native-to-legacy owner conversion.
    pub fn verify_native(
        self,
        policy: &NativePolicy,
        carriage: &NativeCarriage,
        challenge: [u8; 32],
        b: &mut Budget<'_>,
    ) -> Result<(VerifiedCompilerExecutionCurrentRecordV3, Storage)> {
        b.with_prepaid_scope(
            ATTESTATION_STORAGE + policy.retained_storage() + carriage.retained_storage(),
            8,
            WORK,
            SCRATCH,
            |b| {
                verify_attestation_signature(&self)?;
                if self.verifying_key != *policy.verifying_key() {
                    return Err(NativeError::Current(
                        CompilerExecutionCurrentRecordVerificationErrorV3::PolicyMismatch,
                    ));
                }
                if self.challenge != challenge {
                    return Err(NativeError::Current(
                        CompilerExecutionCurrentRecordVerificationErrorV3::ChallengeMismatch,
                    ));
                }
                self.verification
                    .verify_expected_native(policy, carriage, challenge, b)?;
                Ok((
                    VerifiedCompilerExecutionCurrentRecordV3 { attestation: self },
                    Storage(0),
                ))
            },
        )
    }
}

fn validate_native_commit(
    carriage: &NativeCarriage,
    receipt: &AnchorTransitionReceiptV1,
    b: &mut Budget<'_>,
) -> Result<()> {
    reverify_external_anchor_receipt(*carriage.policy().external_anchor_verifying_key(), receipt)?;
    validate_external_anchor_commit_position(carriage.acknowledgment().sequence(), receipt)?;
    let digest = Transaction::digest_for_parts(
        carriage.policy(),
        carriage.request(),
        carriage.publication(),
        b,
    )?;
    if receipt.challenge().transaction() != digest {
        return Err(NativeError::Current(
            CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchorReceiptMismatch,
        ));
    }
    Ok(())
}
