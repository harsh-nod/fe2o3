//! Crash-safe ancillary evidence retained beside one completed V5 result.

use core::fmt;
use std::error::Error;

use fe2o3_artifact_transaction::{
    AuthenticatedCompilerCapabilityEvidenceIdentityV5, BuildAttempt,
    CompletedCompilerCapabilityTransactionV5, MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5,
    MAX_COMPILER_HSACO_BYTES_V1, NoRetainedDurableDirectoryHooksV1,
    RetainedDurableDirectoryErrorV1, RetainedDurableDirectoryV1,
    authenticated_compiler_capability_evidence_identity_v5,
};
use fe2o3_compiler_ffi::{
    InertProductionCapabilityResultV5, MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5,
};
use sha2::{Digest as _, Sha256};

use crate::{
    RecoveredWorkerV3LoadEnvelopeV2, WorkerV3LoadEnvelopeV2, WorkerV3PendingCapabilityResultV1,
};

const MAGIC: [u8; 8] = *b"F3CPAE01";
const VERSION: u16 = 1;
const HEADER_BYTES: usize = 8 + 2 + 2 + 8 + 8 + 8 + 8;
const ATTEMPT_BYTES: usize = 8 + 16 + 32;
const COORDINATE_BYTES: usize = 32 + 8;
const FIXED_BODY_BYTES: usize = HEADER_BYTES
    + ATTEMPT_BYTES
    + 32
    + COORDINATE_BYTES
    + COORDINATE_BYTES
    + COORDINATE_BYTES
    + COORDINATE_BYTES;
const TERMINAL_BYTES: usize = 32 + 32;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-ANCILLARY-EVIDENCE-IDENTITY/V1\0";
const CHECKSUM_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-ANCILLARY-EVIDENCE-CHECKSUM/V1\0";
const NAMESPACE_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/CAPABILITY-ANCILLARY-EVIDENCE-NAMESPACE/V1\0";

/// Maximum complete ancillary evidence journal.
pub const MAX_WORKER_V3_CAPABILITY_ANCILLARY_EVIDENCE_BYTES_V1: usize = FIXED_BODY_BYTES
    + MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5
    + MAX_COMPILER_HSACO_BYTES_V1
    + MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5
    + TERMINAL_BYTES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Coordinate {
    sha256: [u8; 32],
    byte_len: u64,
}

impl Coordinate {
    fn raw(bytes: &[u8]) -> Result<Self, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        Ok(Self {
            sha256: Sha256::digest(bytes).into(),
            byte_len: u64::try_from(bytes.len())
                .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::LengthOverflow)?,
        })
    }
}

/// Exact compiler-owned bytes omitted from the frozen completed-result carrier.
#[derive(Debug, Eq, PartialEq)]
pub struct WorkerV3CapabilityAncillaryEvidenceV1 {
    envelope: Coordinate,
    attempt: BuildAttempt,
    transaction: [u8; 32],
    result: Coordinate,
    object: Coordinate,
    checker: Coordinate,
    checker_typed: Coordinate,
    result_bytes: Box<[u8]>,
    object_bytes: Box<[u8]>,
    checker_bytes: Box<[u8]>,
    canonical_bytes: Box<[u8]>,
    identity: [u8; 32],
}

impl WorkerV3CapabilityAncillaryEvidenceV1 {
    /// Captures every completed compiler-owned byte before load readiness is published.
    pub fn new(
        envelope: &WorkerV3LoadEnvelopeV2,
        completed: &CompletedCompilerCapabilityTransactionV5,
    ) -> Result<Self, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        if completed.object_bytes().is_empty()
            || completed.object_bytes().len() > MAX_COMPILER_HSACO_BYTES_V1
            || completed.checker_evidence_bytes().is_empty()
            || completed.checker_evidence_bytes().len()
                > MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5
            || !completed
                .checker_evidence_identity()
                .matches_bytes(completed.checker_evidence_bytes())
        {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::BindingMismatch(
                "completed compiler evidence",
            ));
        }
        let result_identity = completed.result().identity();
        let checker_typed = completed.checker_evidence_identity();
        let value = Self {
            envelope: Coordinate::raw(&envelope.encode_canonical().map_err(|_| {
                WorkerV3CapabilityAncillaryEvidenceErrorV1::BindingMismatch("load envelope")
            })?)?,
            attempt: completed.attempt(),
            transaction: *completed.transaction_identity().as_bytes(),
            result: Coordinate {
                sha256: result_identity.sha256(),
                byte_len: result_identity.byte_len(),
            },
            object: Coordinate::raw(completed.object_bytes())?,
            checker: Coordinate::raw(completed.checker_evidence_bytes())?,
            checker_typed: Coordinate {
                sha256: checker_typed.sha256(),
                byte_len: checker_typed.byte_len(),
            },
            result_bytes: completed
                .result()
                .canonical_bytes()
                .to_vec()
                .into_boxed_slice(),
            object_bytes: completed.object_bytes().to_vec().into_boxed_slice(),
            checker_bytes: completed
                .checker_evidence_bytes()
                .to_vec()
                .into_boxed_slice(),
            canonical_bytes: Box::new([]),
            identity: [0; 32],
        };
        Self::decode_canonical(&encode(&value)?)
    }

    /// Strictly decodes one complete canonical journal.
    pub fn decode_canonical(
        bytes: &[u8],
    ) -> Result<Self, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        if bytes.len() < FIXED_BODY_BYTES + 2 + TERMINAL_BYTES
            || bytes.len() > MAX_WORKER_V3_CAPABILITY_ANCILLARY_EVIDENCE_BYTES_V1
        {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::WireLength);
        }
        let checksum_offset = bytes.len() - 32;
        let identity_offset = checksum_offset - 32;
        if hash(CHECKSUM_DOMAIN, &bytes[..checksum_offset]).as_slice() != &bytes[checksum_offset..]
            || hash(IDENTITY_DOMAIN, &bytes[..identity_offset]).as_slice()
                != &bytes[identity_offset..checksum_offset]
        {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Integrity);
        }
        let mut reader = Reader::new(&bytes[..identity_offset]);
        if reader.array::<8>()? != MAGIC {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Magic);
        }
        if reader.u16()? != VERSION || reader.u16()? != 0 {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Version);
        }
        let total = reader.u64()?;
        let result_len = reader.u64()?;
        let object_len = reader.u64()?;
        let checker_len = reader.u64()?;
        if total != bytes.len() as u64
            || result_len == 0
            || result_len > MAX_INERT_PRODUCTION_CAPABILITY_RESULT_BYTES_V5 as u64
            || object_len == 0
            || object_len > MAX_COMPILER_HSACO_BYTES_V1 as u64
            || checker_len == 0
            || checker_len > MAX_COMPILER_CAPABILITY_VERIFIER_RESPONSE_BYTES_V5 as u64
        {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::WireLength);
        }
        let generation = reader.u64()?;
        let session = reader.array::<16>()?;
        let invocation = reader.array::<32>()?;
        let attempt = BuildAttempt::from_env_value(&format!(
            "{}:{}:{}",
            generation,
            hex(&session),
            hex(&invocation)
        ))
        .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::Attempt)?;
        let transaction = reader.array::<32>()?;
        let envelope = reader.coordinate()?;
        let result = reader.coordinate()?;
        let object = reader.coordinate()?;
        let checker = reader.coordinate()?;
        let checker_typed = reader.coordinate()?;
        let result_bytes = reader.take(
            usize::try_from(result_len)
                .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::LengthOverflow)?,
        )?;
        let object_bytes = reader.take(
            usize::try_from(object_len)
                .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::LengthOverflow)?,
        )?;
        let checker_bytes = reader.take(
            usize::try_from(checker_len)
                .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::LengthOverflow)?,
        )?;
        let decoded_result = InertProductionCapabilityResultV5::decode(result_bytes)
            .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::Integrity)?;
        let derived_checker_typed =
            authenticated_compiler_capability_evidence_identity_v5(checker_bytes)
                .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::Integrity)?;
        if !reader.is_empty()
            || decoded_result.canonical_bytes() != result_bytes
            || result.sha256 != decoded_result.identity().sha256()
            || result.byte_len != decoded_result.identity().byte_len()
            || object != Coordinate::raw(object_bytes)?
            || checker != Coordinate::raw(checker_bytes)?
            || checker_typed.sha256 != derived_checker_typed.sha256()
            || checker_typed.byte_len != derived_checker_typed.byte_len()
            || transaction == [0; 32]
        {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Integrity);
        }
        Ok(Self {
            envelope,
            attempt,
            transaction,
            result,
            object,
            checker,
            checker_typed,
            result_bytes: result_bytes.to_vec().into_boxed_slice(),
            object_bytes: object_bytes.to_vec().into_boxed_slice(),
            checker_bytes: checker_bytes.to_vec().into_boxed_slice(),
            canonical_bytes: bytes.to_vec().into_boxed_slice(),
            identity: bytes[identity_offset..checksum_offset]
                .try_into()
                .expect("checked identity slice"),
        })
    }

    pub fn object_bytes(&self) -> &[u8] {
        &self.object_bytes
    }

    pub fn production_result_bytes(&self) -> &[u8] {
        &self.result_bytes
    }

    pub fn checker_evidence_bytes(&self) -> &[u8] {
        &self.checker_bytes
    }

    pub fn checker_evidence_identity(
        &self,
    ) -> Result<
        AuthenticatedCompilerCapabilityEvidenceIdentityV5,
        WorkerV3CapabilityAncillaryEvidenceErrorV1,
    > {
        let identity = authenticated_compiler_capability_evidence_identity_v5(&self.checker_bytes)
            .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::Integrity)?;
        if identity.sha256() != self.checker_typed.sha256
            || identity.byte_len() != self.checker_typed.byte_len
        {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Integrity);
        }
        Ok(identity)
    }

    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    pub const fn identity(&self) -> [u8; 32] {
        self.identity
    }

    /// Reconstructs the pending-result owner after a crash between the two journal commits.
    pub fn into_pending_result(
        self,
        envelope: &WorkerV3LoadEnvelopeV2,
    ) -> Result<WorkerV3PendingCapabilityResultV1, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        self.validate_envelope(envelope)?;
        let result = InertProductionCapabilityResultV5::decode(&self.result_bytes)
            .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::Integrity)?;
        WorkerV3PendingCapabilityResultV1::new(envelope, result)
            .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::BindingMismatch("result"))
    }

    /// Durably commits the package before load readiness.
    pub fn persist_durable_journal_v1(
        &self,
        directory: &RetainedDurableDirectoryV1,
    ) -> Result<(), WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        let (canonical, redo) = durable_names(self.attempt);
        let mut hooks = NoRetainedDurableDirectoryHooksV1;
        directory
            .commit_record(
                &canonical,
                &redo,
                self.canonical_bytes(),
                MAX_WORKER_V3_CAPABILITY_ANCILLARY_EVIDENCE_BYTES_V1,
                &mut hooks,
            )
            .map_err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Durable)
    }

    /// Rechecks the package against the matching envelope and completed result.
    pub fn validate_bindings(
        &self,
        envelope: &WorkerV3LoadEnvelopeV2,
        result: &InertProductionCapabilityResultV5,
    ) -> Result<(), WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        self.validate_envelope(envelope)?;
        let expected_result = result.identity();
        if self.transaction != result.transaction().identity().sha256()
            || self.result
                != (Coordinate {
                    sha256: expected_result.sha256(),
                    byte_len: expected_result.byte_len(),
                })
            || self.result_bytes.as_ref() != result.canonical_bytes()
            || self.object.sha256 != result.object_output().output_sha256()
            || self.object.byte_len != result.object_output().output_bytes()
            || self.checker_bytes.as_ref() != result.machine_refinement().canonical_preimage()
        {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::BindingMismatch(
                "envelope or completed result",
            ));
        }
        Ok(())
    }

    fn validate_envelope(
        &self,
        envelope: &WorkerV3LoadEnvelopeV2,
    ) -> Result<(), WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        if self.envelope
            != Coordinate::raw(&envelope.encode_canonical().map_err(|_| {
                WorkerV3CapabilityAncillaryEvidenceErrorV1::BindingMismatch("load envelope")
            })?)?
            || self.attempt != envelope.wire().published_claim().plan().attempt()
        {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::BindingMismatch(
                "load envelope",
            ));
        }
        Ok(())
    }
}

/// Recovers a package for the live envelope, including redo promotion after a crash.
pub fn recover_worker_v3_capability_ancillary_evidence_for_live_v1(
    directory: &RetainedDurableDirectoryV1,
    envelope: &WorkerV3LoadEnvelopeV2,
) -> Result<WorkerV3CapabilityAncillaryEvidenceV1, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
    let package = recover_package(
        directory,
        envelope.wire().published_claim().plan().attempt(),
    )?;
    package.validate_envelope(envelope)?;
    Ok(package)
}

/// Recovers the exact package for a not-yet-consumed result carrier.
pub fn recover_worker_v3_capability_ancillary_evidence_v1(
    directory: &RetainedDurableDirectoryV1,
    envelope: &WorkerV3LoadEnvelopeV2,
    result: &InertProductionCapabilityResultV5,
) -> Result<WorkerV3CapabilityAncillaryEvidenceV1, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
    let package = recover_worker_v3_capability_ancillary_evidence_for_live_v1(directory, envelope)?;
    package.validate_bindings(envelope, result)?;
    Ok(package)
}

/// Recovers and joins the package to restart-recovered envelope/result custody.
pub fn recover_worker_v3_capability_ancillary_evidence_for_recovered_v1(
    directory: &RetainedDurableDirectoryV1,
    envelope: &RecoveredWorkerV3LoadEnvelopeV2,
    result: &InertProductionCapabilityResultV5,
) -> Result<WorkerV3CapabilityAncillaryEvidenceV1, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
    let attempt = envelope.wire().published_claim().plan().attempt();
    let package = recover_package(directory, attempt)?;
    let exact = envelope.canonical_evidence_view();
    let expected_result = result.identity();
    if package.envelope != Coordinate::raw(exact.exact_canonical_bytes())?
        || package.attempt != attempt
        || package.transaction != result.transaction().identity().sha256()
        || package.result
            != (Coordinate {
                sha256: expected_result.sha256(),
                byte_len: expected_result.byte_len(),
            })
        || package.result_bytes.as_ref() != result.canonical_bytes()
        || package.object.sha256 != result.object_output().output_sha256()
        || package.object.byte_len != result.object_output().output_bytes()
        || package.checker_bytes.as_ref() != result.machine_refinement().canonical_preimage()
    {
        return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::BindingMismatch(
            "recovered envelope or completed result",
        ));
    }
    Ok(package)
}

fn recover_package(
    directory: &RetainedDurableDirectoryV1,
    attempt: BuildAttempt,
) -> Result<WorkerV3CapabilityAncillaryEvidenceV1, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
    let (canonical, redo) = durable_names(attempt);
    let canonical_bytes = directory
        .read_private(
            &canonical,
            MAX_WORKER_V3_CAPABILITY_ANCILLARY_EVIDENCE_BYTES_V1,
        )
        .map_err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Durable)?;
    let redo_bytes = directory
        .read_private(&redo, MAX_WORKER_V3_CAPABILITY_ANCILLARY_EVIDENCE_BYTES_V1)
        .map_err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Durable)?;
    let bytes = match (canonical_bytes, redo_bytes) {
        (None, None) => return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Missing),
        (Some(bytes), None) => bytes,
        (expected, Some(redo_bytes)) => {
            if expected.as_deref().is_some_and(|bytes| bytes != redo_bytes) {
                return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Conflict);
            }
            let mut hooks = NoRetainedDurableDirectoryHooksV1;
            directory
                .promote_validated_redo(
                    &canonical,
                    &redo,
                    expected.as_deref(),
                    &redo_bytes,
                    MAX_WORKER_V3_CAPABILITY_ANCILLARY_EVIDENCE_BYTES_V1,
                    &mut hooks,
                )
                .map_err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Durable)?;
            redo_bytes
        }
    };
    WorkerV3CapabilityAncillaryEvidenceV1::decode_canonical(&bytes)
}

fn encode(
    value: &WorkerV3CapabilityAncillaryEvidenceV1,
) -> Result<Vec<u8>, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
    let total = FIXED_BODY_BYTES
        .checked_add(value.result_bytes.len())
        .and_then(|size| size.checked_add(value.object_bytes.len()))
        .and_then(|size| size.checked_add(value.checker_bytes.len()))
        .and_then(|size| size.checked_add(TERMINAL_BYTES))
        .ok_or(WorkerV3CapabilityAncillaryEvidenceErrorV1::LengthOverflow)?;
    if total > MAX_WORKER_V3_CAPABILITY_ANCILLARY_EVIDENCE_BYTES_V1 {
        return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::WireLength);
    }
    let mut bytes = Vec::with_capacity(total);
    bytes.extend_from_slice(&MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(&0_u16.to_le_bytes());
    bytes.extend_from_slice(&(total as u64).to_le_bytes());
    bytes.extend_from_slice(&(value.result_bytes.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&(value.object_bytes.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&(value.checker_bytes.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&value.attempt.generation().to_le_bytes());
    bytes.extend_from_slice(value.attempt.session().as_bytes());
    bytes.extend_from_slice(value.attempt.invocation().as_bytes());
    bytes.extend_from_slice(&value.transaction);
    for coordinate in [
        value.envelope,
        value.result,
        value.object,
        value.checker,
        value.checker_typed,
    ] {
        bytes.extend_from_slice(&coordinate.sha256);
        bytes.extend_from_slice(&coordinate.byte_len.to_le_bytes());
    }
    bytes.extend_from_slice(&value.result_bytes);
    bytes.extend_from_slice(&value.object_bytes);
    bytes.extend_from_slice(&value.checker_bytes);
    let identity = hash(IDENTITY_DOMAIN, &bytes);
    bytes.extend_from_slice(&identity);
    let checksum = hash(CHECKSUM_DOMAIN, &bytes);
    bytes.extend_from_slice(&checksum);
    Ok(bytes)
}

fn durable_names(attempt: BuildAttempt) -> (String, String) {
    let mut digest = Sha256::new();
    digest.update(NAMESPACE_DOMAIN);
    digest.update(attempt.generation().to_le_bytes());
    digest.update(attempt.session().as_bytes());
    digest.update(attempt.invocation().as_bytes());
    let key = hex(&digest.finalize());
    (
        format!(".fe2o3-worker-v3-capability-ancillary-v1-{key}.package"),
        format!(".fe2o3-worker-v3-capability-ancillary-v1-{key}.package.redo"),
    )
}

fn hash(domain: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update(bytes);
    digest.finalize().into()
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(DIGITS[(byte >> 4) as usize] as char);
        encoded.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    encoded
}

struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(
        &mut self,
        length: usize,
    ) -> Result<&'a [u8], WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(WorkerV3CapabilityAncillaryEvidenceErrorV1::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(WorkerV3CapabilityAncillaryEvidenceErrorV1::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn array<const N: usize>(
        &mut self,
    ) -> Result<[u8; N], WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerV3CapabilityAncillaryEvidenceErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        Ok(u64::from_le_bytes(self.array()?))
    }

    fn coordinate(&mut self) -> Result<Coordinate, WorkerV3CapabilityAncillaryEvidenceErrorV1> {
        let coordinate = Coordinate {
            sha256: self.array()?,
            byte_len: self.u64()?,
        };
        if coordinate.sha256 == [0; 32] || coordinate.byte_len == 0 {
            return Err(WorkerV3CapabilityAncillaryEvidenceErrorV1::Integrity);
        }
        Ok(coordinate)
    }

    fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum WorkerV3CapabilityAncillaryEvidenceErrorV1 {
    Durable(RetainedDurableDirectoryErrorV1),
    BindingMismatch(&'static str),
    WireLength,
    LengthOverflow,
    Truncated,
    Magic,
    Version,
    Integrity,
    Attempt,
    Missing,
    Conflict,
}

impl fmt::Display for WorkerV3CapabilityAncillaryEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid Worker V3 capability ancillary evidence: {self:?}"
        )
    }
}

impl Error for WorkerV3CapabilityAncillaryEvidenceErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Durable(source) => Some(source),
            _ => None,
        }
    }
}
