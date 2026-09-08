use std::error::Error;
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_worker_v3_verification_protocol::{
    WorkerV3VerificationCapabilityRequestIdentityV5, WorkerV3VerificationFreshChallengeV1,
    WorkerV3VerificationMeasurementIdentityV1, WorkerV3VerificationPolicyIdentityV1,
    WorkerV3VerificationProductionAttemptV5, WorkerV3VerificationRequestV1,
};
use sha2::{Digest as _, Sha256};

use crate::WorkerV3ProtectedCompletionResponseV5;
use crate::service::{
    WorkerV3VerificationCallerV1, WorkerV3VerificationChallengeReplayGuardV1,
    WorkerV3VerificationMeasurementResolverV1, WorkerV3VerificationPolicyResolverV1,
};
use crate::service_v5::{
    WorkerV3VerificationCapabilityAttemptQuarantineV5,
    WorkerV3VerificationCapabilityCompletionJournalV5,
    WorkerV3VerificationCapabilityRejectionReasonV5,
};

const STATE_VERSION: &[u8] = b"FE2O3/WORKER-V3/VERIFIER-DURABLE-STATE/V5\0";
static TEMPORARY_SEQUENCE: AtomicU64 = AtomicU64::new(1);

/// Fsync-backed terminal journal for the protected verifier deployment.
///
/// A successful method call means the complete record was written, the file was fsynced, the
/// rename was committed, and the containing directory was fsynced. Existing records are accepted
/// only when their bytes are identical, making restart retries idempotent but not substitutable.
pub struct DurableWorkerV3VerificationStateV5 {
    root: PathBuf,
    policy: Option<WorkerV3VerificationPolicyIdentityV1>,
    measurement: Option<WorkerV3VerificationMeasurementIdentityV1>,
}

impl DurableWorkerV3VerificationStateV5 {
    pub fn open(root: impl AsRef<Path>) -> Result<Self, DurableWorkerV3VerificationStateErrorV5> {
        let root = root.as_ref();
        ensure_directory(root)?;
        sync_directory(root)?;
        Ok(Self {
            root: root.to_owned(),
            policy: None,
            measurement: None,
        })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Pins and durably records the sole policy/measurement pair resolved by this instance.
    pub fn install_policy(
        &mut self,
        policy: WorkerV3VerificationPolicyIdentityV1,
        measurement: WorkerV3VerificationMeasurementIdentityV1,
    ) -> Result<(), DurableWorkerV3VerificationStateErrorV5> {
        self.persist(
            "configuration",
            *policy.as_bytes(),
            &[policy.as_bytes(), measurement.as_bytes()],
        )?;
        self.policy = Some(policy);
        self.measurement = Some(measurement);
        Ok(())
    }

    fn persist(
        &self,
        class: &str,
        identity: [u8; 32],
        fields: &[&[u8]],
    ) -> Result<(), DurableWorkerV3VerificationStateErrorV5> {
        self.persist_with_retry_policy(class, identity, fields, true)
    }

    fn persist_fresh(
        &self,
        class: &str,
        identity: [u8; 32],
        fields: &[&[u8]],
    ) -> Result<(), DurableWorkerV3VerificationStateErrorV5> {
        self.persist_with_retry_policy(class, identity, fields, false)
    }

    fn persist_with_retry_policy(
        &self,
        class: &str,
        identity: [u8; 32],
        fields: &[&[u8]],
        allow_identical_retry: bool,
    ) -> Result<(), DurableWorkerV3VerificationStateErrorV5> {
        let directory = self.root.join(class);
        ensure_directory(&directory)?;
        sync_directory(&self.root)?;
        let name = hex(identity);
        let destination = directory.join(&name);
        let bytes = encode_record(class, identity, fields)?;
        if destination.exists() {
            let existing =
                fs::read(&destination).map_err(DurableWorkerV3VerificationStateErrorV5::Io)?;
            return if allow_identical_retry && existing == bytes {
                Ok(())
            } else {
                Err(DurableWorkerV3VerificationStateErrorV5::RecordConflict)
            };
        }

        let sequence = TEMPORARY_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = directory.join(format!(
            ".{name}.{}.{}.pending",
            std::process::id(),
            sequence
        ));
        let result = (|| {
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&temporary)
                .map_err(DurableWorkerV3VerificationStateErrorV5::Io)?;
            file.write_all(&bytes)
                .map_err(DurableWorkerV3VerificationStateErrorV5::Io)?;
            file.sync_all()
                .map_err(DurableWorkerV3VerificationStateErrorV5::Io)?;
            match fs::hard_link(&temporary, &destination) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                    let existing = fs::read(&destination)
                        .map_err(DurableWorkerV3VerificationStateErrorV5::Io)?;
                    if !allow_identical_retry || existing != bytes {
                        return Err(DurableWorkerV3VerificationStateErrorV5::RecordConflict);
                    }
                }
                Err(error) => return Err(DurableWorkerV3VerificationStateErrorV5::Io(error)),
            }
            fs::remove_file(&temporary).map_err(DurableWorkerV3VerificationStateErrorV5::Io)?;
            sync_directory(&directory)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }
}

impl WorkerV3VerificationPolicyResolverV1 for DurableWorkerV3VerificationStateV5 {
    fn resolve_expected_policy(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        _request: &WorkerV3VerificationRequestV1,
    ) -> Option<WorkerV3VerificationPolicyIdentityV1> {
        self.policy
    }
}

impl WorkerV3VerificationMeasurementResolverV1 for DurableWorkerV3VerificationStateV5 {
    fn resolve_expected_measurement(
        &mut self,
        _caller: WorkerV3VerificationCallerV1,
        policy: WorkerV3VerificationPolicyIdentityV1,
        _request: &WorkerV3VerificationRequestV1,
    ) -> Option<WorkerV3VerificationMeasurementIdentityV1> {
        (self.policy == Some(policy))
            .then_some(self.measurement)
            .flatten()
    }
}

impl WorkerV3VerificationChallengeReplayGuardV1 for DurableWorkerV3VerificationStateV5 {
    fn admit_fresh_challenge(
        &mut self,
        caller: WorkerV3VerificationCallerV1,
        policy: WorkerV3VerificationPolicyIdentityV1,
        challenge: WorkerV3VerificationFreshChallengeV1,
    ) -> bool {
        if self.policy != Some(policy) {
            return false;
        }
        let caller = caller_bytes(caller);
        let mut digest = Sha256::new();
        digest.update(b"FE2O3/WORKER-V3/VERIFIER-CHALLENGE/V5\0");
        digest.update(caller);
        digest.update(policy.as_bytes());
        digest.update(challenge.as_bytes());
        let identity = digest.finalize().into();
        let destination = self.root.join("challenges").join(hex(identity));
        if destination.exists() {
            return false;
        }
        self.persist_fresh(
            "challenges",
            identity,
            &[&caller, policy.as_bytes(), challenge.as_bytes()],
        )
        .is_ok()
    }
}

impl WorkerV3VerificationCapabilityAttemptQuarantineV5 for DurableWorkerV3VerificationStateV5 {
    type Error = DurableWorkerV3VerificationStateErrorV5;

    fn quarantine_unidentified_submission(
        &mut self,
        caller: WorkerV3VerificationCallerV1,
        submission_sha256: Option<[u8; 32]>,
    ) -> Result<(), Self::Error> {
        let digest = submission_sha256.unwrap_or([0; 32]);
        self.persist("quarantine-unidentified", digest, &[&caller_bytes(caller)])
    }

    fn quarantine_attempt_coordinates(
        &mut self,
        caller: WorkerV3VerificationCallerV1,
        request: WorkerV3VerificationCapabilityRequestIdentityV5,
        attempt: WorkerV3VerificationProductionAttemptV5,
    ) -> Result<(), Self::Error> {
        let attempt_bytes = encode_attempt(attempt);
        self.persist(
            "quarantine-attempt",
            *request.as_bytes(),
            &[&caller_bytes(caller), &attempt_bytes],
        )
    }

    fn quarantine_evidence(
        &mut self,
        caller: WorkerV3VerificationCallerV1,
        request: WorkerV3VerificationCapabilityRequestIdentityV5,
        evidence_identity: Option<[u8; 32]>,
        reason: WorkerV3VerificationCapabilityRejectionReasonV5,
    ) -> Result<(), Self::Error> {
        let evidence_identity = evidence_identity.unwrap_or([0; 32]);
        let reason = [rejection_reason_tag(reason)];
        self.persist(
            "quarantine-evidence",
            *request.as_bytes(),
            &[&caller_bytes(caller), &evidence_identity, &reason],
        )
    }
}

impl WorkerV3VerificationCapabilityCompletionJournalV5 for DurableWorkerV3VerificationStateV5 {
    type Error = DurableWorkerV3VerificationStateErrorV5;

    fn record_issued(
        &mut self,
        caller: WorkerV3VerificationCallerV1,
        request: WorkerV3VerificationCapabilityRequestIdentityV5,
        evidence_identity: [u8; 32],
        response: &WorkerV3ProtectedCompletionResponseV5,
    ) -> Result<(), Self::Error> {
        if response.request_identity() != request.as_bytes() {
            return Err(DurableWorkerV3VerificationStateErrorV5::RequestMismatch);
        }
        self.persist(
            "issued",
            *request.as_bytes(),
            &[
                &caller_bytes(caller),
                &evidence_identity,
                &response.identity(),
                response.canonical_bytes(),
            ],
        )
    }
}

const fn rejection_reason_tag(reason: WorkerV3VerificationCapabilityRejectionReasonV5) -> u8 {
    match reason {
        WorkerV3VerificationCapabilityRejectionReasonV5::PolicyUnresolved => 1,
        WorkerV3VerificationCapabilityRejectionReasonV5::PolicyMismatch => 2,
        WorkerV3VerificationCapabilityRejectionReasonV5::MeasurementUnresolved => 3,
        WorkerV3VerificationCapabilityRejectionReasonV5::MeasurementMismatch => 4,
        WorkerV3VerificationCapabilityRejectionReasonV5::ChallengeReplay => 5,
        WorkerV3VerificationCapabilityRejectionReasonV5::PayloadDescriptor => 6,
        WorkerV3VerificationCapabilityRejectionReasonV5::PayloadAlias => 7,
        WorkerV3VerificationCapabilityRejectionReasonV5::PayloadCustody => 8,
        WorkerV3VerificationCapabilityRejectionReasonV5::EvidenceMalformed => 9,
        WorkerV3VerificationCapabilityRejectionReasonV5::EvidenceMismatch => 10,
        WorkerV3VerificationCapabilityRejectionReasonV5::ProtectedVerification => 11,
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum DurableWorkerV3VerificationStateErrorV5 {
    InvalidRoot,
    RequestMismatch,
    RecordConflict,
    LengthOverflow,
    Io(io::Error),
}

impl fmt::Display for DurableWorkerV3VerificationStateErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "durable Worker V3 verifier state failure: {self:?}"
        )
    }
}

impl Error for DurableWorkerV3VerificationStateErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
            _ => None,
        }
    }
}

fn encode_record(
    class: &str,
    identity: [u8; 32],
    fields: &[&[u8]],
) -> Result<Vec<u8>, DurableWorkerV3VerificationStateErrorV5> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(STATE_VERSION);
    put_field(&mut bytes, class.as_bytes())?;
    bytes.extend_from_slice(&identity);
    bytes.extend_from_slice(
        &u32::try_from(fields.len())
            .map_err(|_| DurableWorkerV3VerificationStateErrorV5::LengthOverflow)?
            .to_le_bytes(),
    );
    for field in fields {
        put_field(&mut bytes, field)?;
    }
    Ok(bytes)
}

fn put_field(
    output: &mut Vec<u8>,
    field: &[u8],
) -> Result<(), DurableWorkerV3VerificationStateErrorV5> {
    output.extend_from_slice(
        &u64::try_from(field.len())
            .map_err(|_| DurableWorkerV3VerificationStateErrorV5::LengthOverflow)?
            .to_le_bytes(),
    );
    output.extend_from_slice(field);
    Ok(())
}

fn encode_attempt(attempt: WorkerV3VerificationProductionAttemptV5) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(72);
    bytes.extend_from_slice(&attempt.generation().to_le_bytes());
    bytes.extend_from_slice(&attempt.session());
    bytes.extend_from_slice(&attempt.invocation());
    bytes
}

fn caller_bytes(caller: WorkerV3VerificationCallerV1) -> [u8; 12] {
    let mut bytes = [0; 12];
    bytes[..4].copy_from_slice(&caller.pid().to_le_bytes());
    bytes[4..8].copy_from_slice(&caller.uid().to_le_bytes());
    bytes[8..].copy_from_slice(&caller.gid().to_le_bytes());
    bytes
}

fn sync_directory(path: &Path) -> Result<(), DurableWorkerV3VerificationStateErrorV5> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(DurableWorkerV3VerificationStateErrorV5::Io)
}

fn ensure_directory(path: &Path) -> Result<(), DurableWorkerV3VerificationStateErrorV5> {
    fs::create_dir_all(path).map_err(DurableWorkerV3VerificationStateErrorV5::Io)?;
    let metadata =
        fs::symlink_metadata(path).map_err(DurableWorkerV3VerificationStateErrorV5::Io)?;
    if !metadata.file_type().is_dir() || metadata.file_type().is_symlink() {
        return Err(DurableWorkerV3VerificationStateErrorV5::InvalidRoot);
    }
    Ok(())
}

fn hex(bytes: [u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0xf) as usize] as char);
    }
    output
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::{DurableWorkerV3VerificationStateErrorV5, DurableWorkerV3VerificationStateV5};

    #[test]
    fn committed_record_survives_reopen_and_identical_retry() {
        let temporary = tempdir().unwrap();
        let state = DurableWorkerV3VerificationStateV5::open(temporary.path()).unwrap();
        state
            .persist("completed", [7; 32], &[b"owner", b"result"])
            .unwrap();
        drop(state);
        let reopened = DurableWorkerV3VerificationStateV5::open(temporary.path()).unwrap();
        reopened
            .persist("completed", [7; 32], &[b"owner", b"result"])
            .unwrap();
        assert!(
            temporary
                .path()
                .join("completed")
                .join(super::hex([7; 32]))
                .is_file()
        );
    }

    #[test]
    fn request_identity_cannot_be_reused_for_a_different_result() {
        let temporary = tempdir().unwrap();
        let state = DurableWorkerV3VerificationStateV5::open(temporary.path()).unwrap();
        state
            .persist("completed", [8; 32], &[b"owner", b"first"])
            .unwrap();
        assert!(matches!(
            state.persist("completed", [8; 32], &[b"owner", b"spliced"]),
            Err(DurableWorkerV3VerificationStateErrorV5::RecordConflict)
        ));
    }

    #[test]
    fn crash_residue_never_counts_as_a_committed_record() {
        let temporary = tempdir().unwrap();
        let state = DurableWorkerV3VerificationStateV5::open(temporary.path()).unwrap();
        let directory = temporary.path().join("completed");
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join(".interrupted.pending"), b"partial").unwrap();
        state
            .persist("completed", [9; 32], &[b"owner", b"result"])
            .unwrap();
        assert_eq!(
            fs::read(directory.join(super::hex([9; 32]))).unwrap()[..super::STATE_VERSION.len()],
            *super::STATE_VERSION
        );
    }

    #[test]
    fn storage_failures_are_reported() {
        let temporary = tempdir().unwrap();
        let root = temporary.path().join("not-a-directory");
        fs::write(&root, b"occupied").unwrap();
        assert!(DurableWorkerV3VerificationStateV5::open(root).is_err());
    }

    #[test]
    fn fresh_records_are_exclusive_across_reopened_instances() {
        let temporary = tempdir().unwrap();
        let first = DurableWorkerV3VerificationStateV5::open(temporary.path()).unwrap();
        let second = DurableWorkerV3VerificationStateV5::open(temporary.path()).unwrap();
        first
            .persist_fresh("challenges", [10; 32], &[b"challenge"])
            .unwrap();
        assert!(matches!(
            second.persist_fresh("challenges", [10; 32], &[b"challenge"]),
            Err(DurableWorkerV3VerificationStateErrorV5::RecordConflict)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_record_class_is_rejected() {
        use std::os::unix::fs::symlink;

        let temporary = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let state = DurableWorkerV3VerificationStateV5::open(temporary.path()).unwrap();
        symlink(outside.path(), temporary.path().join("completed")).unwrap();
        assert!(matches!(
            state.persist("completed", [11; 32], &[b"owner"]),
            Err(DurableWorkerV3VerificationStateErrorV5::InvalidRoot)
        ));
    }
}
