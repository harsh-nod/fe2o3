//! One-shot application-side audit of the protected compiler current-record endpoint.

use std::error::Error;
use std::fmt;
use std::time::Duration;

use fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1;
use fe2o3_compiler_execution_client::{CompilerExecutionClientErrorV1, CompilerExecutionClientV1};
use fe2o3_runtime_protocol::{
    CompilerExecutionCurrentRecordAttestationIdentityV1,
    CompilerExecutionCurrentRecordVerificationV1, CompilerExecutionReceiptCarriageV1,
    VerifiedCompilerExecutionCurrentRecordV1,
};

use crate::{
    CompilerGeneratedKernelExpectationV1, WorkerV3AuditorV1, WorkerV3VerificationRequestV1,
};

/// Complete deadline for one application-side current-record verification transaction.
pub const WORKER_V3_COMPILER_CURRENT_RECORD_AUDIT_TIMEOUT_V1: Duration = Duration::from_secs(30);

/// Move-only signed endpoint evidence for one exact Worker V3 compiler receipt.
///
/// The evidence authenticates a fresh response under the receipt's pinned issuer key. It remains
/// non-authoritative because protected key custody and external monotonic currentness are separate
/// production joins.
///
/// ```compile_fail
/// use fe2o3_host::WorkerV3CompilerCurrentRecordAuditV1;
/// fn require_clone<T: Clone>() {}
/// require_clone::<WorkerV3CompilerCurrentRecordAuditV1>();
/// ```
#[derive(Debug)]
pub struct WorkerV3CompilerCurrentRecordAuditV1 {
    verified: VerifiedCompilerExecutionCurrentRecordV1,
}

impl WorkerV3CompilerCurrentRecordAuditV1 {
    pub const fn verification(&self) -> &CompilerExecutionCurrentRecordVerificationV1 {
        self.verified.verification()
    }

    pub const fn attestation_identity(
        &self,
    ) -> CompilerExecutionCurrentRecordAttestationIdentityV1 {
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

    pub const fn authenticates_external_rollback_currentness(&self) -> bool {
        false
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

    use ed25519_dalek::SigningKey;
    use fe2o3_artifact_transaction::{
        INERT_COMPILER_EXECUTION_SUBJECT_BYTES_V1, INERT_COMPILER_EXECUTION_SUBJECT_MAGIC_V1,
        INERT_COMPILER_EXECUTION_SUBJECT_VERSION_V1,
    };
    use fe2o3_runtime_protocol::{
        CompilerExecutionAttestationChallengeV1, CompilerExecutionAttestationReceiptV1,
        CompilerExecutionAttestationRequestV1, CompilerExecutionCurrentRecordAttestationV1,
        CompilerExecutionCurrentRecordVerificationV1, CompilerExecutionIssuerMeasurementV1,
        CompilerExecutionIssuerPolicyV1, CompilerExecutionReceiptPublicationAckV1,
        CompilerExecutionReceiptPublicationV1, CompilerExecutionServiceRequestKindV1,
        CompilerExecutionServiceRequestV1, CompilerExecutionServiceResponseV1,
        MAX_COMPILER_EXECUTION_SERVICE_REQUEST_BYTES_V1,
    };
    use sha2::{Digest, Sha256};

    use super::*;

    const SUBJECT_IDENTITY_DOMAIN: &[u8] = b"FE2O3/INERT-COMPILER-EXECUTION-SUBJECT/V1\0";
    const COMPILER_CLOSURE_IDENTITY_DOMAIN: &[u8] = b"fe2o3-compiler-closure-identity-v2\0";

    struct Fixture {
        signing_key: SigningKey,
        policy: CompilerExecutionIssuerPolicyV1,
        subject: InertCompilerExecutionSubjectV1,
        carriage: CompilerExecutionReceiptCarriageV1,
    }

    impl Fixture {
        fn new(subject_seed: u8) -> Self {
            let signing_key = SigningKey::from_bytes(&[0x51; 32]);
            let policy = CompilerExecutionIssuerPolicyV1::new(
                7,
                CompilerExecutionIssuerMeasurementV1::new([0x61; 32], 123).unwrap(),
                CompilerExecutionIssuerMeasurementV1::new([0x62; 32], 456).unwrap(),
                signing_key.verifying_key().to_bytes(),
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
                policy,
                subject,
                carriage,
            }
        }
    }

    #[test]
    fn signed_current_record_is_owned_once_without_authority() {
        let fixture = Fixture::new(0x20);
        let (client, service) = socket_pair();
        let service_subject = fixture.subject.clone();
        let service_carriage = fixture.carriage.clone();
        let service_policy = fixture.policy.clone();
        let service_key = fixture.signing_key.clone();
        let service = thread::spawn(move || {
            let request = receive_request(&service);
            assert_eq!(
                request.kind(),
                CompilerExecutionServiceRequestKindV1::VerifyCurrent
            );
            assert_eq!(request.carriage(), Some(&service_carriage));
            let verification = CompilerExecutionCurrentRecordVerificationV1::new(
                &service_subject,
                &service_carriage,
                [0x91; 32],
                [0x92; 32],
            )
            .unwrap();
            let attestation = CompilerExecutionCurrentRecordAttestationV1::issue(
                &service_policy,
                verification,
                request.verification_challenge().unwrap(),
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
        assert!(!evidence.authenticates_external_rollback_currentness());
        assert!(!evidence.grants_verification_authority());
        assert!(!evidence.grants_authority());
        assert!(!evidence.grants_load_authority());
        assert!(!evidence.grants_launch_authority());
        assert!(matches!(
            auditor.audit_exact(&fixture.subject, &fixture.carriage),
            Err(WorkerV3CompilerCurrentRecordAuditErrorV1::AlreadyConsumed)
        ));
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
