//! One-shot application-side audit of the protected compiler current-record endpoint.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use fe2o3_compiler_execution_client::{CompilerExecutionClientErrorV1, CompilerExecutionClientV1};
use fe2o3_runtime_protocol::{
    CompilerExecutionCurrentRecordAttestationIdentityV3,
    CompilerExecutionCurrentRecordAttestationV3, CompilerExecutionCurrentRecordVerificationErrorV3,
    CompilerExecutionCurrentRecordVerificationV3, CompilerExecutionIssuerPolicyV1,
    CompilerExecutionReceiptCarriageV1, VerifiedCompilerExecutionCurrentRecordV3,
};

use crate::{
    CompilerGeneratedKernelExpectationRosterV1, CompilerGeneratedKernelExpectationV1,
    WorkerV3AuditorV1, WorkerV3CompilerExecutionEvidenceErrorV1,
    WorkerV3CompilerExecutionVerificationV1, WorkerV3RosterVerificationRequestV1,
    WorkerV3VerificationRequestV1,
};

/// Complete deadline for one application-side current-record verification transaction.
pub const WORKER_V3_COMPILER_CURRENT_RECORD_AUDIT_TIMEOUT_V1: Duration = Duration::from_secs(30);

/// Move-only signed endpoint evidence for one exact Worker V3 compiler receipt.
///
/// The evidence authenticates a fresh response under the receipt's pinned issuer key and a fresh
/// signed recovery observation under the separately pinned external-anchor key. It remains
/// non-authoritative because protected key custody and independently administered anchor deployment
/// are separate production joins.
///
/// ```compile_fail
/// use fe2o3_host::WorkerV3CompilerCurrentRecordAuditV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<WorkerV3CompilerCurrentRecordAuditV1>();
/// ```
#[derive(Debug)]
pub struct WorkerV3CompilerCurrentRecordAuditV1 {
    verified: VerifiedCompilerExecutionCurrentRecordV3,
}

impl WorkerV3CompilerCurrentRecordAuditV1 {
    pub const fn verification(&self) -> &CompilerExecutionCurrentRecordVerificationV3 {
        self.verified.verification()
    }

    pub const fn attestation_identity(
        &self,
    ) -> CompilerExecutionCurrentRecordAttestationIdentityV3 {
        self.verified.attestation().identity()
    }

    pub const fn authenticates_pinned_signing_key(&self) -> bool {
        self.verified.authenticates_pinned_signing_key()
    }

    pub const fn authenticates_expected_fresh_challenge(&self) -> bool {
        self.verified.authenticates_expected_challenge()
    }

    pub const fn authenticates_protected_key_custody(&self) -> bool {
        false
    }

    pub const fn authenticates_protected_current_record(&self) -> bool {
        self.verified.authenticates_protected_current_record()
    }

    pub const fn authenticates_external_anchor_commit(&self) -> bool {
        self.verified.authenticates_external_anchor_commit()
    }

    pub const fn external_rollback_verification_identity(&self) -> [u8; 32] {
        self.verified.external_rollback_verification_identity()
    }

    pub const fn authenticates_external_rollback_currentness(&self) -> bool {
        self.verified.authenticates_external_rollback_currentness()
    }

    pub const fn grants_verification_authority(&self) -> bool {
        false
    }

    pub const fn grants_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    /// Consumes this one-use audit into an exact compiler-execution evidence lane.
    ///
    /// Every V3 current-record coordinate is compared with the supplied subject and complete
    /// carriage before the signed attestation is retained. This transition still grants no final
    /// verification, load, or launch authority; protected deployment trust and the remaining
    /// refinement receipts must be joined by the crate-owned production verifier.
    pub fn bind_exact_compiler_execution_v1(
        self,
        subject: &InertCompilerExecutionSubjectV1,
        carriage: &CompilerExecutionReceiptCarriageV1,
    ) -> Result<WorkerV3CompilerExecutionVerificationV1, WorkerV3CompilerExecutionEvidenceErrorV1>
    {
        WorkerV3CompilerExecutionVerificationV1::from_current_record_audit(subject, carriage, self)
    }
}

/// Admits exact canonical compiler current-record evidence inside a future protected Worker V3
/// backend.
///
/// The caller supplies an independently pinned policy and a cryptographically fresh challenge
/// whose replay exclusion it owns. Both canonical records are decoded independently, the nested
/// verification must byte-match the separately transported verification, and the signed
/// attestation is checked against the exact subject, carriage, policy, challenge, Worker-ledger
/// position, and external rollback-currentness observation. The host repeats the exact coordinate
/// comparisons before constructing the move-only compiler-execution lane.
///
/// This crate-internal bridge does not authenticate the transport, establish protected key
/// custody, or grant verification, load, or launch authority. It must remain unreachable to
/// applications until a production backend supplies a move-only authenticated measured-service
/// session and owns those unsafe obligations.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn admit_worker_v3_compiler_current_record_evidence_v1(
    pinned_policy: &CompilerExecutionIssuerPolicyV1,
    subject: &InertCompilerExecutionSubjectV1,
    carriage: &CompilerExecutionReceiptCarriageV1,
    expected_fresh_challenge: [u8; 32],
    canonical_verification: &[u8],
    canonical_attestation: &[u8],
) -> Result<
    WorkerV3CompilerExecutionVerificationV1,
    WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1,
> {
    if carriage.request().subject() != subject {
        return Err(WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::RequestMismatch);
    }
    if carriage.policy() != pinned_policy {
        return Err(WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::PolicyMismatch);
    }

    let verification = CompilerExecutionCurrentRecordVerificationV3::decode(canonical_verification)
        .map_err(WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::Verification)?;
    let attestation = CompilerExecutionCurrentRecordAttestationV3::decode(canonical_attestation)
        .map_err(WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::Attestation)?;
    if attestation.verification() != &verification
        || attestation.verification().canonical_bytes().as_slice() != canonical_verification
    {
        return Err(
            WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::VerificationAttestationMismatch,
        );
    }

    let verified = attestation
        .verify(pinned_policy, carriage, expected_fresh_challenge)
        .map_err(WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::Attestation)?;
    independently_recheck_current_record_v1(
        pinned_policy,
        subject,
        carriage,
        expected_fresh_challenge,
        &verified,
    )?;

    WorkerV3CompilerExecutionVerificationV1::from_current_record_audit(
        subject,
        carriage,
        WorkerV3CompilerCurrentRecordAuditV1 { verified },
    )
    .map_err(WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::Evidence)
}

fn independently_recheck_current_record_v1(
    pinned_policy: &CompilerExecutionIssuerPolicyV1,
    subject: &InertCompilerExecutionSubjectV1,
    carriage: &CompilerExecutionReceiptCarriageV1,
    expected_fresh_challenge: [u8; 32],
    verified: &VerifiedCompilerExecutionCurrentRecordV3,
) -> Result<(), WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1> {
    let attestation = verified.attestation();
    let verification = verified.verification();
    for (matches, field) in [
        (
            attestation.challenge() == expected_fresh_challenge,
            "fresh compiler current-record challenge",
        ),
        (
            attestation.verifying_key() == *pinned_policy.verifying_key(),
            "pinned compiler current-record signing key",
        ),
        (
            verification.policy_identity() == *pinned_policy.identity().as_bytes(),
            "compiler-execution policy",
        ),
        (
            verification.subject_identity() == *subject.identity().sha256(),
            "compiler-execution subject",
        ),
        (
            verification.carriage_identity() == *carriage.identity().as_bytes(),
            "compiler-execution carriage",
        ),
        (
            verification.issuer_journal_identity()
                == carriage.acknowledgment().issuer_journal_identity(),
            "compiler-execution issuer journal",
        ),
        (
            verification.worker_ledger_record_identity()
                == carriage.acknowledgment().worker_ledger_record_identity(),
            "compiler-execution Worker ledger record",
        ),
        (
            verification.sequence() == carriage.acknowledgment().sequence(),
            "compiler-execution rollback sequence",
        ),
        (
            verification.prior_rollback_anchor()
                == carriage.publication().receipt().prior_rollback_anchor(),
            "compiler-execution prior rollback anchor",
        ),
        (
            verification.current_rollback_anchor()
                == carriage.acknowledgment().current_rollback_anchor(),
            "compiler-execution current rollback anchor",
        ),
        (
            verification.external_anchor_verifying_key()
                == *pinned_policy.external_anchor_verifying_key(),
            "pinned external rollback signing key",
        ),
    ] {
        if !matches {
            return Err(
                WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::IdentityMismatch(field),
            );
        }
    }

    let expected_currentness =
        CompilerExecutionCurrentRecordVerificationV3::external_anchor_currentness_challenge(
            carriage,
            verification.external_anchor_commit_receipt(),
            expected_fresh_challenge,
        )
        .map_err(WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::Verification)?;
    if verification
        .external_anchor_currentness_receipt()
        .challenge()
        != &expected_currentness
    {
        return Err(
            WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::StaleExternalRollbackCurrentness,
        );
    }
    Ok(())
}

/// One-use auditor that owns the application endpoint inherited at FD 195.
///
/// ```compile_fail
/// use fe2o3_host::InheritedWorkerV3CompilerCurrentRecordAuditorV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<InheritedWorkerV3CompilerCurrentRecordAuditorV1>();
/// ```
pub struct InheritedWorkerV3CompilerCurrentRecordAuditorV1 {
    client: Option<CompilerExecutionClientV1>,
}

impl fmt::Debug for InheritedWorkerV3CompilerCurrentRecordAuditorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InheritedWorkerV3CompilerCurrentRecordAuditorV1")
            .field("available", &self.client.is_some())
            .field("authority", &"none")
            .finish()
    }
}

impl InheritedWorkerV3CompilerCurrentRecordAuditorV1 {
    /// Consumes the inherited public FD slot into one private close-on-exec client.
    pub fn admit_inherited_application_service() -> Result<Self, CompilerExecutionClientErrorV1> {
        CompilerExecutionClientV1::admit_inherited_child(
            WORKER_V3_COMPILER_CURRENT_RECORD_AUDIT_TIMEOUT_V1,
        )
        .map(|client| Self {
            client: Some(client),
        })
    }

    #[cfg(test)]
    fn from_client(client: CompilerExecutionClientV1) -> Self {
        Self {
            client: Some(client),
        }
    }

    /// Audits the exact compiler current record retained by one aggregate roster request.
    ///
    /// The inherited endpoint remains one-use and is consumed on every success or failure. The
    /// returned move-only audit authenticates the signed current-record response but grants no
    /// protected-key, verification, load, or launch authority.
    pub fn audit_roster<R>(
        &mut self,
        request: &WorkerV3RosterVerificationRequestV1<'_, R>,
    ) -> Result<WorkerV3CompilerCurrentRecordAuditV1, WorkerV3CompilerCurrentRecordAuditErrorV1>
    where
        R: CompilerGeneratedKernelExpectationRosterV1,
    {
        self.audit_exact(
            request.compiler_execution_subject(),
            request.compiler_execution_receipt_carriage(),
        )
    }

    fn audit_exact(
        &mut self,
        subject: &InertCompilerExecutionSubjectV1,
        carriage: &CompilerExecutionReceiptCarriageV1,
    ) -> Result<WorkerV3CompilerCurrentRecordAuditV1, WorkerV3CompilerCurrentRecordAuditErrorV1>
    {
        let client = self
            .client
            .take()
            .ok_or(WorkerV3CompilerCurrentRecordAuditErrorV1::AlreadyConsumed)?;
        if carriage.request().subject() != subject {
            return Err(WorkerV3CompilerCurrentRecordAuditErrorV1::RequestMismatch);
        }
        let verified = client
            .verify_current_only(carriage.policy(), carriage.clone())
            .map_err(WorkerV3CompilerCurrentRecordAuditErrorV1::Client)?;
        Ok(WorkerV3CompilerCurrentRecordAuditV1 { verified })
    }
}

impl<K: CompilerGeneratedKernelExpectationV1> WorkerV3AuditorV1<K>
    for InheritedWorkerV3CompilerCurrentRecordAuditorV1
{
    type Error = WorkerV3CompilerCurrentRecordAuditErrorV1;
    type Evidence = WorkerV3CompilerCurrentRecordAuditV1;

    fn audit(
        &mut self,
        request: &WorkerV3VerificationRequestV1<'_, K>,
    ) -> Result<Self::Evidence, Self::Error> {
        self.audit_exact(
            request.compiler_execution_subject(),
            request.compiler_execution_receipt_carriage(),
        )
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3CompilerCurrentRecordAuditErrorV1 {
    AlreadyConsumed,
    RequestMismatch,
    Client(CompilerExecutionClientErrorV1),
}

/// Failure to admit canonical current-record evidence into the Worker V3 host lane.
#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1 {
    RequestMismatch,
    PolicyMismatch,
    Verification(CompilerExecutionCurrentRecordVerificationErrorV3),
    Attestation(CompilerExecutionCurrentRecordVerificationErrorV3),
    VerificationAttestationMismatch,
    IdentityMismatch(&'static str),
    StaleExternalRollbackCurrentness,
    Evidence(WorkerV3CompilerExecutionEvidenceErrorV1),
}

impl fmt::Display for WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestMismatch => formatter.write_str(
                "compiler current-record evidence subject differs from its receipt carriage",
            ),
            Self::PolicyMismatch => formatter.write_str(
                "compiler current-record evidence carriage differs from the pinned policy",
            ),
            Self::Verification(error) => {
                write!(formatter, "compiler current-record verification failed: {error}")
            }
            Self::Attestation(error) => {
                write!(formatter, "compiler current-record attestation failed: {error}")
            }
            Self::VerificationAttestationMismatch => formatter.write_str(
                "compiler current-record attestation embeds a different verification",
            ),
            Self::IdentityMismatch(field) => {
                write!(formatter, "compiler current-record {field} identity mismatch")
            }
            Self::StaleExternalRollbackCurrentness => formatter.write_str(
                "compiler current-record external rollback observation is stale for the expected challenge",
            ),
            Self::Evidence(error) => write!(
                formatter,
                "compiler current-record evidence could not enter the Worker V3 lane: {error}"
            ),
        }
    }
}

impl Error for WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Verification(error) | Self::Attestation(error) => Some(error),
            Self::Evidence(error) => Some(error),
            Self::RequestMismatch
            | Self::PolicyMismatch
            | Self::VerificationAttestationMismatch
            | Self::IdentityMismatch(_)
            | Self::StaleExternalRollbackCurrentness => None,
        }
    }
}

impl fmt::Display for WorkerV3CompilerCurrentRecordAuditErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AlreadyConsumed => {
                formatter.write_str("compiler current-record auditor was already consumed")
            }
            Self::RequestMismatch => formatter.write_str(
                "compiler current-record audit subject differs from its receipt carriage",
            ),
            Self::Client(error) => {
                write!(formatter, "compiler current-record service failed: {error}")
            }
        }
    }
}

impl Error for WorkerV3CompilerCurrentRecordAuditErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Client(error) => Some(error),
            Self::AlreadyConsumed | Self::RequestMismatch => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::thread;

    use ed25519_dalek::{Signer, SigningKey};
    use fe2o3_artifact_transaction::{
        INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1,
    };
    use fe2o3_external_anchor_protocol::{
        AnchorPositionV1, AnchorTransitionReceiptV1, AnchoredStateV1, CallerNonceV1,
        HashChainHeadV1, PinnedAnchorKeyV1, UnsignedAnchorObservationV1,
    };
    use fe2o3_runtime_protocol::{
        COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3,
        CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationReceiptV1,
        CompilerExecutionAttestationRequestV1, CompilerExecutionCurrentRecordAttestationV3,
        CompilerExecutionCurrentRecordVerificationV3, CompilerExecutionExternalAnchorTransactionV1,
        CompilerExecutionIssuerMeasurementV1, CompilerExecutionIssuerPolicyV1,
        CompilerExecutionReceiptPublicationAckV1, CompilerExecutionReceiptPublicationV1,
        CompilerExecutionServiceRequestKindV1, CompilerExecutionServiceRequestV1,
        CompilerExecutionServiceResponseV1, MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1,
    };
    use sha2::{Digest, Sha256};

    use super::*;

    const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
    const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";
    const CURRENT_RECORD_VERIFICATION_IDENTITY_DOMAIN: &[u8] =
        b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-VERIFICATION/V3\0";
    const CURRENT_RECORD_ATTESTATION_IDENTITY_DOMAIN: &[u8] =
        b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-ATTESTATION/V3\0";
    const CURRENT_RECORD_ATTESTATION_SIGNATURE_DOMAIN: &[u8] =
        b"FE2O3/COMPILER-EXECUTION-CURRENT-RECORD-ATTESTATION-SIGNATURE/V3\0";
    const CURRENT_RECORD_HEADER_BYTES: usize = 24;
    const CURRENT_RECORD_ATTESTATION_VERIFICATION_OFFSET: usize = CURRENT_RECORD_HEADER_BYTES + 32;
    const CURRENT_RECORD_ATTESTATION_SIGNED_PREFIX_BYTES: usize =
        CURRENT_RECORD_ATTESTATION_VERIFICATION_OFFSET
            + COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3
            + 32;
    const CURRENT_RECORD_ATTESTATION_PREIMAGE_BYTES: usize =
        CURRENT_RECORD_ATTESTATION_SIGNED_PREFIX_BYTES + 64;
    const CURRENT_RECORD_WORKER_LEDGER_OFFSET: usize = CURRENT_RECORD_HEADER_BYTES + 4 * 32;
    const CURRENT_RECORD_CURRENT_ROLLBACK_ANCHOR_OFFSET: usize =
        CURRENT_RECORD_HEADER_BYTES + 5 * 32 + 8 + 32;

    struct Fixture {
        signing_key: SigningKey,
        anchor_signing_key: SigningKey,
        policy: CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
        carriage: CompilerExecutionReceiptCarriageV1,
    }

    impl Fixture {
        fn new(subject_seed: u8) -> Self {
            let signing_key = SigningKey::from_bytes(&[0x51; 32]);
            let anchor_signing_key = SigningKey::from_bytes(&[0x52; 32]);
            let policy = CompilerExecutionIssuerPolicyV1::new(
                7,
                CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 123).unwrap(),
                CompilerExecutionIssuerMeasurementV1::new([0x62; 32], 456).unwrap(),
                signing_key.verifying_key().to_bytes(),
                anchor_signing_key.verifying_key().to_bytes(),
            )
            .unwrap();
            let subject = subject(subject_seed);
            let challenge = CompilerExecutionAttestationChallengeV1::new(
                &policy, &subject, [0x63; 32], 1, [0; 32],
            )
            .unwrap();
            let request =
                CompilerExecutionAttestationRequestV1::new(challenge, subject.clone()).unwrap();
            let receipt =
                CompilerExecutionAttestationReceiptV1::issue(&policy, &request, &signing_key)
                    .unwrap();
            let publication =
                CompilerExecutionReceiptPublicationV1::new([0x64; 32], [0x65; 32], receipt)
                    .unwrap();
            let acknowledgment =
                CompilerExecutionReceiptPublicationAckV1::new(&publication, [0x66; 32]).unwrap();
            let carriage = CompilerExecutionReceiptCarriageV1::new(
                policy.clone(),
                request,
                publication,
                acknowledgment,
            )
            .unwrap();
            Self {
                signing_key,
                anchor_signing_key,
                policy,
                subject,
                carriage,
            }
        }

        fn anchor_receipt(&self) -> AnchorTransitionReceiptV1 {
            let transaction = CompilerExecutionExternalAnchorTransactionV1::new(
                self.policy.clone(),
                self.carriage.request().clone(),
                self.carriage.publication().clone(),
            )
            .unwrap();
            let key =
                PinnedAnchorKeyV1::from_bytes(self.anchor_signing_key.verifying_key().to_bytes())
                    .unwrap();
            let pending =
                AnchoredStateV1::from_local_state(0, HashChainHeadV1::from_bytes([0; 32]))
                    .prepare(transaction.external_anchor_digest(), &key)
                    .unwrap()
                    .begin_advance(CallerNonceV1::from_bytes([0x67; 32]), &key)
                    .unwrap();
            let unsigned = UnsignedAnchorObservationV1::from_challenge(
                pending.challenge(),
                AnchorPositionV1::Proposed,
            );
            let signature = self.anchor_signing_key.sign(&unsigned.signing_bytes());
            AnchorTransitionReceiptV1::new(
                pending.challenge().clone(),
                &unsigned.attach_signature(signature.to_bytes()),
                &key,
            )
            .unwrap()
        }
    }

    fn direct_current_record_attestation(
        fixture: &Fixture,
        verification_challenge: [u8; 32],
    ) -> CompilerExecutionCurrentRecordAttestationV3 {
        let commit_receipt = fixture.anchor_receipt();
        let currentness_challenge =
            CompilerExecutionCurrentRecordVerificationV3::external_anchor_currentness_challenge(
                &fixture.carriage,
                &commit_receipt,
                verification_challenge,
            )
            .unwrap();
        let anchor_key =
            PinnedAnchorKeyV1::from_bytes(fixture.anchor_signing_key.verifying_key().to_bytes())
                .unwrap();
        let unsigned = UnsignedAnchorObservationV1::from_challenge(
            &currentness_challenge,
            AnchorPositionV1::Proposed,
        );
        let signature = fixture.anchor_signing_key.sign(&unsigned.signing_bytes());
        let currentness_receipt = AnchorTransitionReceiptV1::new(
            currentness_challenge,
            &unsigned.attach_signature(signature.to_bytes()),
            &anchor_key,
        )
        .unwrap();
        let verification = CompilerExecutionCurrentRecordVerificationV3::new(
            &fixture.carriage,
            commit_receipt,
            currentness_receipt,
            verification_challenge,
            [0x91; 32],
            [0x92; 32],
        )
        .unwrap();
        CompilerExecutionCurrentRecordAttestationV3::issue(
            &fixture.policy,
            &fixture.carriage,
            verification,
            verification_challenge,
            &fixture.signing_key,
        )
        .unwrap()
    }

    fn direct_current_record_audit(
        fixture: &Fixture,
        verification_challenge: [u8; 32],
    ) -> WorkerV3CompilerCurrentRecordAuditV1 {
        let attestation = direct_current_record_attestation(fixture, verification_challenge);
        let verified = attestation
            .verify(&fixture.policy, &fixture.carriage, verification_challenge)
            .unwrap();
        WorkerV3CompilerCurrentRecordAuditV1 { verified }
    }

    fn canonical_current_record_evidence(
        fixture: &Fixture,
        verification_challenge: [u8; 32],
    ) -> (Vec<u8>, Vec<u8>) {
        let attestation = direct_current_record_attestation(fixture, verification_challenge);
        (
            attestation.verification().canonical_bytes().to_vec(),
            attestation.canonical_bytes().to_vec(),
        )
    }

    fn domain_identity(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(domain);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
        digest.finalize().into()
    }

    fn rebuild_current_record_attestation(
        fixture: &Fixture,
        mut attestation: Vec<u8>,
        challenge: [u8; 32],
        verification: &[u8],
    ) -> Vec<u8> {
        assert_eq!(
            verification.len(),
            COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3
        );
        attestation[CURRENT_RECORD_HEADER_BYTES..CURRENT_RECORD_ATTESTATION_VERIFICATION_OFFSET]
            .copy_from_slice(&challenge);
        attestation[CURRENT_RECORD_ATTESTATION_VERIFICATION_OFFSET
            ..CURRENT_RECORD_ATTESTATION_VERIFICATION_OFFSET
                + COMPILER_EXECUTION_CURRENT_RECORD_VERIFICATION_BYTES_V3]
            .copy_from_slice(verification);
        let signature_message = domain_identity(
            CURRENT_RECORD_ATTESTATION_SIGNATURE_DOMAIN,
            &attestation[..CURRENT_RECORD_ATTESTATION_SIGNED_PREFIX_BYTES],
        );
        let signature = fixture.signing_key.sign(&signature_message).to_bytes();
        attestation[CURRENT_RECORD_ATTESTATION_SIGNED_PREFIX_BYTES
            ..CURRENT_RECORD_ATTESTATION_PREIMAGE_BYTES]
            .copy_from_slice(&signature);
        let identity = domain_identity(
            CURRENT_RECORD_ATTESTATION_IDENTITY_DOMAIN,
            &attestation[..CURRENT_RECORD_ATTESTATION_PREIMAGE_BYTES],
        );
        attestation[CURRENT_RECORD_ATTESTATION_PREIMAGE_BYTES..].copy_from_slice(&identity);
        attestation
    }

    fn mutate_verification_identity_field(
        fixture: &Fixture,
        challenge: [u8; 32],
        verification: &[u8],
        attestation: &[u8],
        offset: usize,
        replacement: [u8; 32],
    ) -> (Vec<u8>, Vec<u8>) {
        let mut verification = verification.to_vec();
        verification[offset..offset + 32].copy_from_slice(&replacement);
        let identity_offset = verification.len() - 32;
        let identity = domain_identity(
            CURRENT_RECORD_VERIFICATION_IDENTITY_DOMAIN,
            &verification[..identity_offset],
        );
        verification[identity_offset..].copy_from_slice(&identity);
        let attestation = rebuild_current_record_attestation(
            fixture,
            attestation.to_vec(),
            challenge,
            &verification,
        );
        (verification, attestation)
    }

    fn alternate_carriage_for_same_subject(
        fixture: &Fixture,
    ) -> CompilerExecutionReceiptCarriageV1 {
        let challenge = CompilerExecutionAttestationChallengeV1::new(
            &fixture.policy,
            &fixture.subject,
            [0xa1; 32],
            2,
            fixture.carriage.acknowledgment().current_rollback_anchor(),
        )
        .unwrap();
        let request =
            CompilerExecutionAttestationRequestV1::new(challenge, fixture.subject.clone()).unwrap();
        let receipt = CompilerExecutionAttestationReceiptV1::issue(
            &fixture.policy,
            &request,
            &fixture.signing_key,
        )
        .unwrap();
        let publication =
            CompilerExecutionReceiptPublicationV1::new([0xa2; 32], [0xa3; 32], receipt).unwrap();
        let acknowledgment =
            CompilerExecutionReceiptPublicationAckV1::new(&publication, [0xa4; 32]).unwrap();
        CompilerExecutionReceiptCarriageV1::new(
            fixture.policy.clone(),
            request,
            publication,
            acknowledgment,
        )
        .unwrap()
    }

    #[test]
    fn signed_current_record_is_owned_once_without_authority() {
        let fixture = Fixture::new(0x20);
        let (client, service) = socket_pair();
        let service_carriage = fixture.carriage.clone();
        let service_policy = fixture.policy.clone();
        let service_key = fixture.signing_key.clone();
        let service_anchor_receipt = fixture.anchor_receipt();
        let service_anchor_key = fixture.anchor_signing_key.clone();
        let service = thread::spawn(move || {
            let request = receive_request(&service);
            assert_eq!(
                request.kind(),
                CompilerExecutionServiceRequestKindV1::VerifyCurrent
            );
            assert_eq!(request.carriage(), Some(&service_carriage));
            let verification_challenge = request.verification_challenge().unwrap();
            let currentness_challenge = CompilerExecutionCurrentRecordVerificationV3::external_anchor_currentness_challenge(
                &service_carriage,
                &service_anchor_receipt,
                verification_challenge,
            )
            .unwrap();
            let anchor_key =
                PinnedAnchorKeyV1::from_bytes(service_anchor_key.verifying_key().to_bytes())
                    .unwrap();
            let unsigned = UnsignedAnchorObservationV1::from_challenge(
                &currentness_challenge,
                AnchorPositionV1::Proposed,
            );
            let signature = service_anchor_key.sign(&unsigned.signing_bytes());
            let currentness_receipt = AnchorTransitionReceiptV1::new(
                currentness_challenge,
                &unsigned.attach_signature(signature.to_bytes()),
                &anchor_key,
            )
            .unwrap();
            let verification = CompilerExecutionCurrentRecordVerificationV3::new(
                &service_carriage,
                service_anchor_receipt,
                currentness_receipt,
                verification_challenge,
                [0x91; 32],
                [0x92; 32],
            )
            .unwrap();
            let attestation = CompilerExecutionCurrentRecordAttestationV3::issue(
                &service_policy,
                &service_carriage,
                verification,
                verification_challenge,
                &service_key,
            )
            .unwrap();
            let response = CompilerExecutionServiceResponseV1::verified_current(
                request.identity(),
                attestation,
            )
            .unwrap();
            send_response(&service, response.canonical_bytes());
        });
        let client = CompilerExecutionClientV1::admit(client, Duration::from_secs(2)).unwrap();
        let mut auditor = InheritedWorkerV3CompilerCurrentRecordAuditorV1::from_client(client);
        let evidence = auditor
            .audit_exact(&fixture.subject, &fixture.carriage)
            .unwrap();
        assert!(evidence.authenticates_pinned_signing_key());
        assert!(evidence.authenticates_expected_fresh_challenge());
        assert_eq!(
            evidence
                .verification()
                .protected_policy_verification_identity(),
            [0x91; 32]
        );
        assert_eq!(
            evidence
                .verification()
                .protected_worker_ledger_verification_identity(),
            [0x92; 32]
        );
        assert_ne!(evidence.attestation_identity().as_bytes(), &[0; 32]);
        assert!(!evidence.authenticates_protected_key_custody());
        assert!(!evidence.authenticates_protected_current_record());
        assert!(evidence.authenticates_external_anchor_commit());
        assert_ne!(evidence.external_rollback_verification_identity(), [0; 32]);
        assert!(evidence.authenticates_external_rollback_currentness());
        assert!(!evidence.grants_verification_authority());
        assert!(!evidence.grants_authority());
        assert!(!evidence.grants_load_authority());
        assert!(!evidence.grants_launch_authority());
        assert!(matches!(
            auditor.audit_exact(&fixture.subject, &fixture.carriage),
            Err(WorkerV3CompilerCurrentRecordAuditErrorV1::AlreadyConsumed)
        ));
        let bound = evidence
            .bind_exact_compiler_execution_v1(&fixture.subject, &fixture.carriage)
            .unwrap();
        assert_eq!(bound.subject_sha256(), *fixture.subject.identity().sha256());
        assert_eq!(
            bound.carriage_sha256(),
            *fixture.carriage.identity().as_bytes()
        );
        assert_ne!(bound.current_record_verification_sha256(), [0; 32]);
        assert_ne!(bound.current_record_attestation_sha256(), [0; 32]);
        assert!(bound.authenticates_signed_currentness_evidence());
        assert!(!bound.grants_verification_authority());
        service.join().unwrap();
    }

    #[test]
    fn subject_substitution_and_closed_service_fail_closed_and_consume_once() {
        let fixture = Fixture::new(0x20);
        let substituted = subject(0x21);
        let (client, _service) = socket_pair();
        let client = CompilerExecutionClientV1::admit(client, Duration::from_secs(2)).unwrap();
        let mut auditor = InheritedWorkerV3CompilerCurrentRecordAuditorV1::from_client(client);
        assert!(matches!(
            auditor.audit_exact(&substituted, &fixture.carriage),
            Err(WorkerV3CompilerCurrentRecordAuditErrorV1::RequestMismatch)
        ));
        assert!(matches!(
            auditor.audit_exact(&fixture.subject, &fixture.carriage),
            Err(WorkerV3CompilerCurrentRecordAuditErrorV1::AlreadyConsumed)
        ));

        let (client, service) = socket_pair();
        drop(service);
        let client = CompilerExecutionClientV1::admit(client, Duration::from_secs(2)).unwrap();
        let mut auditor = InheritedWorkerV3CompilerCurrentRecordAuditorV1::from_client(client);
        assert!(matches!(
            auditor.audit_exact(&fixture.subject, &fixture.carriage),
            Err(WorkerV3CompilerCurrentRecordAuditErrorV1::Client(_))
        ));
    }

    #[test]
    fn move_only_current_record_join_rejects_subject_and_carriage_substitution() {
        let fixture = Fixture::new(0x20);
        let substituted_subject = subject(0x21);
        assert!(matches!(
            direct_current_record_audit(&fixture, [0xb1; 32])
                .bind_exact_compiler_execution_v1(&substituted_subject, &fixture.carriage),
            Err(WorkerV3CompilerExecutionEvidenceErrorV1::RequestMismatch)
        ));

        let substituted_carriage = alternate_carriage_for_same_subject(&fixture);
        assert!(matches!(
            direct_current_record_audit(&fixture, [0xb2; 32])
                .bind_exact_compiler_execution_v1(&fixture.subject, &substituted_carriage),
            Err(WorkerV3CompilerExecutionEvidenceErrorV1::IdentityMismatch(
                "compiler-execution carriage"
            ))
        ));
    }

    #[test]
    fn canonical_current_record_bridge_admits_exact_evidence_without_authority() {
        let fixture = Fixture::new(0x20);
        let challenge = [0xc1; 32];
        let (verification, attestation) = canonical_current_record_evidence(&fixture, challenge);
        let admitted = admit_worker_v3_compiler_current_record_evidence_v1(
            &fixture.policy,
            &fixture.subject,
            &fixture.carriage,
            challenge,
            &verification,
            &attestation,
        )
        .unwrap();
        assert_eq!(
            admitted.current_record_verification_sha256(),
            *CompilerExecutionCurrentRecordVerificationV3::decode(&verification)
                .unwrap()
                .identity()
                .as_bytes()
        );
        assert!(admitted.authenticates_signed_currentness_evidence());
        assert!(!admitted.authenticates_protected_current_record());
        assert!(!admitted.grants_verification_authority());
    }

    #[test]
    fn canonical_current_record_bridge_rejects_challenge_policy_subject_and_carriage_mismatch() {
        let fixture = Fixture::new(0x20);
        let challenge = [0xc2; 32];
        let (verification, attestation) = canonical_current_record_evidence(&fixture, challenge);

        assert!(matches!(
            admit_worker_v3_compiler_current_record_evidence_v1(
                &fixture.policy,
                &fixture.subject,
                &fixture.carriage,
                [0xc3; 32],
                &verification,
                &attestation,
            ),
            Err(
                WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::Attestation(
                    CompilerExecutionCurrentRecordVerificationErrorV3::ChallengeMismatch
                )
            )
        ));

        let alternate_policy = CompilerExecutionIssuerPolicyV1::new(
            8,
            CompilerExecutionIssuerMeasurementV1::new([0x71; 32], 789).unwrap(),
            CompilerExecutionIssuerMeasurementV1::new([0x72; 32], 987).unwrap(),
            SigningKey::from_bytes(&[0x73; 32])
                .verifying_key()
                .to_bytes(),
            SigningKey::from_bytes(&[0x74; 32])
                .verifying_key()
                .to_bytes(),
        )
        .unwrap();
        assert!(matches!(
            admit_worker_v3_compiler_current_record_evidence_v1(
                &alternate_policy,
                &fixture.subject,
                &fixture.carriage,
                challenge,
                &verification,
                &attestation,
            ),
            Err(WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::PolicyMismatch)
        ));

        assert!(matches!(
            admit_worker_v3_compiler_current_record_evidence_v1(
                &fixture.policy,
                &subject(0x21),
                &fixture.carriage,
                challenge,
                &verification,
                &attestation,
            ),
            Err(WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::RequestMismatch)
        ));

        assert!(matches!(
            admit_worker_v3_compiler_current_record_evidence_v1(
                &fixture.policy,
                &fixture.subject,
                &alternate_carriage_for_same_subject(&fixture),
                challenge,
                &verification,
                &attestation,
            ),
            Err(
                WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::Attestation(
                    CompilerExecutionCurrentRecordVerificationErrorV3::VerificationMismatch
                )
            )
        ));
    }

    #[test]
    fn canonical_current_record_bridge_rejects_ledger_and_rollback_substitution() {
        let fixture = Fixture::new(0x20);
        let challenge = [0xc4; 32];
        let (verification, attestation) = canonical_current_record_evidence(&fixture, challenge);

        let (ledger_verification, ledger_attestation) = mutate_verification_identity_field(
            &fixture,
            challenge,
            &verification,
            &attestation,
            CURRENT_RECORD_WORKER_LEDGER_OFFSET,
            [0xd1; 32],
        );
        assert!(matches!(
            admit_worker_v3_compiler_current_record_evidence_v1(
                &fixture.policy,
                &fixture.subject,
                &fixture.carriage,
                challenge,
                &ledger_verification,
                &ledger_attestation,
            ),
            Err(
                WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::Attestation(
                    CompilerExecutionCurrentRecordVerificationErrorV3::VerificationMismatch
                )
            )
        ));

        let (rollback_verification, rollback_attestation) = mutate_verification_identity_field(
            &fixture,
            challenge,
            &verification,
            &attestation,
            CURRENT_RECORD_CURRENT_ROLLBACK_ANCHOR_OFFSET,
            [0xd2; 32],
        );
        assert!(matches!(
            admit_worker_v3_compiler_current_record_evidence_v1(
                &fixture.policy,
                &fixture.subject,
                &fixture.carriage,
                challenge,
                &rollback_verification,
                &rollback_attestation,
            ),
            Err(
                WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::Attestation(
                    CompilerExecutionCurrentRecordVerificationErrorV3::VerificationMismatch
                )
            )
        ));
    }

    #[test]
    fn canonical_current_record_bridge_rejects_stale_external_currentness() {
        let fixture = Fixture::new(0x20);
        let stale_challenge = [0xc5; 32];
        let expected_challenge = [0xc6; 32];
        let (verification, attestation) =
            canonical_current_record_evidence(&fixture, stale_challenge);
        let attestation = rebuild_current_record_attestation(
            &fixture,
            attestation,
            expected_challenge,
            &verification,
        );

        assert!(matches!(
            admit_worker_v3_compiler_current_record_evidence_v1(
                &fixture.policy,
                &fixture.subject,
                &fixture.carriage,
                expected_challenge,
                &verification,
                &attestation,
            ),
            Err(WorkerV3CompilerCurrentRecordEvidenceAdmissionErrorV1::Attestation(
                CompilerExecutionCurrentRecordVerificationErrorV3::ExternalAnchorCurrentnessReceiptMismatch
            ))
        ));
    }

    fn socket_pair() -> (OwnedFd, OwnedFd) {
        let mut descriptors = [-1_i32; 2];
        assert_eq!(
            unsafe {
                libc::socketpair(
                    libc::AF_UNIX,
                    libc::SOCK_SEQPACKET | libc::SOCK_CLOEXEC,
                    0,
                    descriptors.as_mut_ptr(),
                )
            },
            0
        );
        unsafe {
            (
                OwnedFd::from_raw_fd(descriptors[0]),
                OwnedFd::from_raw_fd(descriptors[1]),
            )
        }
    }

    fn receive_request(service: &OwnedFd) -> CompilerExecutionServiceRequestV1 {
        let mut bytes = [0_u8; MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1];
        let received = unsafe {
            libc::recv(
                service.as_raw_fd(),
                bytes.as_mut_ptr().cast(),
                bytes.len(),
                0,
            )
        };
        assert!(received > 0);
        CompilerExecutionServiceRequestV1::decode(&bytes[..received as usize]).unwrap()
    }

    fn send_response(service: &OwnedFd, bytes: &[u8]) {
        let sent = unsafe {
            libc::send(
                service.as_raw_fd(),
                bytes.as_ptr().cast(),
                bytes.len(),
                libc::MSG_NOSIGNAL,
            )
        };
        assert_eq!(sent, bytes.len() as isize);
    }

    fn subject(seed: u8) -> InertCompilerExecutionSubjectV1 {
        let closure_pins = [
            [seed; 32],
            [seed + 1; 32],
            [seed + 2; 32],
            [seed + 3; 32],
            [seed + 4; 32],
            [seed + 5; 32],
        ];
        let mut closure_digest = Sha256::new();
        closure_digest.update(COMPILER_CLOSURE_IDENTITY_DOMAIN);
        closure_digest.update(1_u16.to_le_bytes());
        for pin in closure_pins {
            closure_digest.update(pin);
        }
        let closure_identity: [u8; 32] = closure_digest.finalize().into();
        let mut bytes = [0_u8; INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1];
        let mut offset = 0;
        put(
            &mut bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        );
        put(
            &mut bytes,
            &mut offset,
            &INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1.to_le_bytes(),
        );
        put(&mut bytes, &mut offset, &0_u16.to_le_bytes());
        put(
            &mut bytes,
            &mut offset,
            &(INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1 as u64).to_le_bytes(),
        );
        put(&mut bytes, &mut offset, &0_u32.to_le_bytes());
        put(&mut bytes, &mut offset, &9_u64.to_le_bytes());
        put(&mut bytes, &mut offset, &[seed + 6; 16]);
        put(&mut bytes, &mut offset, &[seed + 7; 32]);
        bytes[offset] = 0;
        offset += 8;
        put(&mut bytes, &mut offset, &[seed + 8; 32]);
        put(&mut bytes, &mut offset, &[seed + 9; 32]);
        for pin in closure_pins {
            put(&mut bytes, &mut offset, &pin);
        }
        put(&mut bytes, &mut offset, &1_u16.to_le_bytes());
        put(&mut bytes, &mut offset, &closure_identity);
        for axis in 0_u8..7 {
            put(&mut bytes, &mut offset, &[seed + 10 + axis; 32]);
            put(
                &mut bytes,
                &mut offset,
                &(1_000_u64 + u64::from(axis)).to_le_bytes(),
            );
        }
        let identity = digest(SUBJECT_IDENTITY_DOMAIN, &bytes[..offset]);
        put(&mut bytes, &mut offset, &identity);
        assert_eq!(offset, bytes.len());
        InertCompilerExecutionSubjectV1::decode(&bytes).unwrap()
    }

    fn digest(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(domain);
        digest.update((bytes.len() as u64).to_le_bytes());
        digest.update(bytes);
        digest.finalize().into()
    }

    fn put(output: &mut [u8], offset: &mut usize, value: &[u8]) {
        let end = *offset + value.len();
        output[*offset..end].copy_from_slice(value);
        *offset = end;
    }
}
