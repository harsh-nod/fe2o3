use std::fmt;

use fe2o3_artifacts::{
    ArtifactContainerV1, BundleIndexV1, DigestAlgorithm, DigestBytes, DirectLinkBindingSourceV1,
    DirectLinkBundleEvidenceV1, DirectLinkDecodeError, MAX_DIRECT_LINK_EVIDENCE_BYTES, MAX_KERNELS,
    MAX_PROOF_RECORD_BYTES, PayloadDigest, ProofDecodeError, ProofRecordV1,
};
use sha2::{Digest, Sha256};

use crate::{
    EnvelopeValidationError, ExactRawHsacoV1, MAX_WORKER_V2_PROOF_EVIDENCE_BYTES,
    MAX_WORKER_V2_RAW_HSACO_BYTES,
};

pub const WORKER_V2_ENVELOPE_INPUTS_MAGIC: [u8; 8] = *b"FE2W2I1\0";
pub const WORKER_V2_ENVELOPE_INPUTS_VERSION: u16 = 1;
pub const MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES: usize = MAX_DIRECT_LINK_EVIDENCE_BYTES
    + MAX_WORKER_V2_PROOF_EVIDENCE_BYTES
    + MAX_WORKER_V2_RAW_HSACO_BYTES
    + 4096;

const FIXED_HEADER_BYTES: usize = 61;
const IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V2-ENVELOPE-INPUTS/V1\0";

/// SHA-256 identity of one exact canonical pre-envelope input capsule.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV2EnvelopeInputsIdentityV1([u8; 32]);

impl WorkerV2EnvelopeInputsIdentityV1 {
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Independently supplied canonical inputs needed to assemble a Worker V2 envelope.
///
/// This capsule is inert. Canonical decoding checks structure, bounds, and content digests, but
/// does not authenticate the producer, proof execution, trusted items, or compiler provenance.
/// It grants no currentness, loading, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV2EnvelopeInputsV1 {
    direct_link_evidence: DirectLinkBundleEvidenceV1,
    proof_records: Vec<ProofRecordV1>,
    raw_hsaco: ExactRawHsacoV1,
}

impl WorkerV2EnvelopeInputsV1 {
    pub fn new(
        direct_link_evidence: DirectLinkBundleEvidenceV1,
        mut proof_records: Vec<ProofRecordV1>,
        raw_hsaco: ExactRawHsacoV1,
    ) -> Result<Self, EnvelopeInputsValidationError> {
        if direct_link_evidence.bindings().len() != 1 {
            return Err(EnvelopeInputsValidationError::DirectLinkBindingCount {
                actual: direct_link_evidence.bindings().len(),
            });
        }
        if proof_records.is_empty() || proof_records.len() > MAX_KERNELS {
            return Err(EnvelopeInputsValidationError::ProofCount {
                actual: proof_records.len(),
            });
        }
        proof_records.sort_unstable_by_key(|record| record.target().artifact().kernel_id());
        if proof_records.windows(2).any(|pair| {
            pair[0].target().artifact().kernel_id() == pair[1].target().artifact().kernel_id()
        }) {
            return Err(EnvelopeInputsValidationError::DuplicateProofKernel);
        }
        let proof_bytes = proof_records.iter().try_fold(0usize, |total, proof| {
            total.checked_add(proof.encoded_len())
        });
        if proof_bytes.is_none_or(|total| total > MAX_WORKER_V2_PROOF_EVIDENCE_BYTES) {
            return Err(EnvelopeInputsValidationError::ProofEvidenceTooLarge {
                max: MAX_WORKER_V2_PROOF_EVIDENCE_BYTES,
            });
        }
        let total_len = canonical_length(
            direct_link_evidence.encoded_len(),
            raw_hsaco.bytes().len(),
            proof_records.iter().map(ProofRecordV1::encoded_len),
        );
        if total_len.is_none_or(|length| length > MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES) {
            return Err(EnvelopeInputsValidationError::CapsuleTooLarge {
                max: MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES,
            });
        }
        Ok(Self {
            direct_link_evidence,
            proof_records,
            raw_hsaco,
        })
    }

    pub const fn direct_link_evidence(&self) -> &DirectLinkBundleEvidenceV1 {
        &self.direct_link_evidence
    }

    pub fn proof_records(&self) -> &[ProofRecordV1] {
        &self.proof_records
    }

    pub const fn raw_hsaco(&self) -> &ExactRawHsacoV1 {
        &self.raw_hsaco
    }

    /// Checks the complete capsule join against one exact finalized container.
    ///
    /// This is structural validation only. It does not authenticate any identity carried by the
    /// direct-link or proof records and grants no publication, load, or launch authority.
    pub fn validate_against_container(
        &self,
        container: &ArtifactContainerV1,
    ) -> Result<(), EnvelopeValidationError> {
        let bundle = BundleIndexV1::from_containers(std::slice::from_ref(container))?;
        let binding = self
            .direct_link_evidence
            .bindings()
            .first()
            .ok_or(EnvelopeValidationError::DirectLinkBindingCount { actual: 0 })?;
        let source = DirectLinkBindingSourceV1::new(container, binding.expectation().clone());
        self.direct_link_evidence.validate_against(
            &bundle,
            &[container],
            std::slice::from_ref(&source),
        )?;
        crate::model::validate_payloads(container, binding.expectation(), &self.raw_hsaco)?;
        let mut proofs = self.proof_records.clone();
        crate::model::canonicalize_and_validate_proofs(container, &mut proofs)
    }

    pub fn identity(&self) -> WorkerV2EnvelopeInputsIdentityV1 {
        let mut digest = Sha256::new();
        digest.update(IDENTITY_DOMAIN);
        digest.update(self.to_bytes());
        WorkerV2EnvelopeInputsIdentityV1(digest.finalize().into())
    }

    pub const fn grants_currentness_authority(&self) -> bool {
        false
    }

    pub const fn grants_load_authority(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let direct_link = self.direct_link_evidence.to_bytes();
        let proofs = self
            .proof_records
            .iter()
            .map(ProofRecordV1::to_bytes)
            .collect::<Vec<_>>();
        let total_len = FIXED_HEADER_BYTES
            + direct_link.len()
            + proofs.iter().map(|proof| 4 + proof.len()).sum::<usize>()
            + self.raw_hsaco.bytes().len();
        let mut bytes = Vec::with_capacity(total_len);
        bytes.extend_from_slice(&WORKER_V2_ENVELOPE_INPUTS_MAGIC);
        bytes.extend_from_slice(&WORKER_V2_ENVELOPE_INPUTS_VERSION.to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&(total_len as u32).to_le_bytes());
        bytes.extend_from_slice(&(direct_link.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(self.raw_hsaco.bytes().len() as u32).to_le_bytes());
        bytes.extend_from_slice(&(proofs.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.push(0);
        bytes.extend_from_slice(self.raw_hsaco.identity().bytes().as_bytes());
        debug_assert_eq!(bytes.len(), FIXED_HEADER_BYTES);
        bytes.extend_from_slice(&direct_link);
        for proof in proofs {
            bytes.extend_from_slice(&(proof.len() as u32).to_le_bytes());
            bytes.extend_from_slice(&proof);
        }
        bytes.extend_from_slice(self.raw_hsaco.bytes());
        debug_assert_eq!(bytes.len(), total_len);
        debug_assert!(bytes.len() <= MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES);
        bytes
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, EnvelopeInputsDecodeError> {
        if bytes.len() > MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES {
            return Err(EnvelopeInputsDecodeError::TooLarge {
                max: MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES,
            });
        }
        let mut reader = Reader::new(bytes);
        if reader.array::<8>()? != WORKER_V2_ENVELOPE_INPUTS_MAGIC {
            return Err(EnvelopeInputsDecodeError::InvalidMagic);
        }
        let version = reader.u16()?;
        if version != WORKER_V2_ENVELOPE_INPUTS_VERSION {
            return Err(EnvelopeInputsDecodeError::UnknownVersion(version));
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(EnvelopeInputsDecodeError::UnsupportedFlags(flags));
        }
        let total_len = reader.length("capsule", MAX_WORKER_V2_ENVELOPE_INPUTS_BYTES)?;
        if total_len > bytes.len() {
            return Err(EnvelopeInputsDecodeError::Truncated);
        }
        if total_len < bytes.len() {
            return Err(EnvelopeInputsDecodeError::TrailingBytes);
        }
        let direct_link_len =
            reader.length("direct-link evidence", MAX_DIRECT_LINK_EVIDENCE_BYTES)?;
        let raw_len = reader.length("raw HSACO", MAX_WORKER_V2_RAW_HSACO_BYTES)?;
        let proof_count = usize::from(reader.u16()?);
        if proof_count == 0 || proof_count > MAX_KERNELS {
            return Err(EnvelopeInputsDecodeError::CountOutOfRange {
                field: "proof records",
                value: proof_count as u64,
                max: MAX_KERNELS,
            });
        }
        if reader.u16()? != 0 {
            return Err(EnvelopeInputsDecodeError::NonZeroReserved);
        }
        if reader.u8()? != 0 {
            return Err(EnvelopeInputsDecodeError::UnknownDigestAlgorithm);
        }
        let raw_identity = PayloadDigest::new(
            DigestAlgorithm::Sha256,
            DigestBytes::from_bytes(reader.array()?),
        );
        let minimum_aggregate = FIXED_HEADER_BYTES
            .checked_add(direct_link_len)
            .and_then(|length| length.checked_add(raw_len))
            .and_then(|length| length.checked_add(proof_count.checked_mul(4)?))
            .ok_or(EnvelopeInputsDecodeError::LengthOverflow)?;
        if minimum_aggregate > total_len {
            return Err(EnvelopeInputsDecodeError::AggregateLengthMismatch);
        }
        let body = reader.remaining();
        let mut preflight = Reader::new(body);
        preflight.take(direct_link_len)?;
        let mut proof_bytes = 0usize;
        for _ in 0..proof_count {
            let proof_len = preflight.length("proof record", MAX_PROOF_RECORD_BYTES)?;
            proof_bytes = proof_bytes
                .checked_add(proof_len)
                .ok_or(EnvelopeInputsDecodeError::LengthOverflow)?;
            if proof_bytes > MAX_WORKER_V2_PROOF_EVIDENCE_BYTES {
                return Err(EnvelopeInputsDecodeError::LengthOutOfRange {
                    field: "proof evidence",
                    value: proof_bytes as u64,
                    max: MAX_WORKER_V2_PROOF_EVIDENCE_BYTES,
                });
            }
            preflight.take(proof_len)?;
        }
        let raw_bytes = preflight.take(raw_len)?;
        if !preflight.is_empty() {
            return Err(EnvelopeInputsDecodeError::AggregateLengthMismatch);
        }
        let aggregate = FIXED_HEADER_BYTES
            .checked_add(direct_link_len)
            .and_then(|length| length.checked_add(raw_len))
            .and_then(|length| length.checked_add(proof_count.checked_mul(4)?))
            .and_then(|length| length.checked_add(proof_bytes))
            .ok_or(EnvelopeInputsDecodeError::LengthOverflow)?;
        if aggregate != total_len {
            return Err(EnvelopeInputsDecodeError::AggregateLengthMismatch);
        }
        if raw_len == 0 {
            return Err(EnvelopeInputsDecodeError::Envelope(
                EnvelopeValidationError::EmptyRawHsaco,
            ));
        }
        raw_identity.verify(raw_bytes).map_err(|_| {
            EnvelopeInputsDecodeError::Envelope(EnvelopeValidationError::RawHsacoDigestMismatch)
        })?;

        let mut body_reader = Reader::new(body);
        let direct_link_evidence =
            DirectLinkBundleEvidenceV1::from_bytes(body_reader.take(direct_link_len)?)
                .map_err(EnvelopeInputsDecodeError::DirectLink)?;
        let mut proof_records = Vec::new();
        reserve_exact(&mut proof_records, proof_count, "proof records")?;
        for _ in 0..proof_count {
            let proof_len = body_reader.length("proof record", MAX_PROOF_RECORD_BYTES)?;
            proof_records.push(
                ProofRecordV1::from_bytes(body_reader.take(proof_len)?)
                    .map_err(EnvelopeInputsDecodeError::Proof)?,
            );
        }
        if proof_records.windows(2).any(|pair| {
            pair[0].target().artifact().kernel_id() >= pair[1].target().artifact().kernel_id()
        }) {
            return Err(EnvelopeInputsDecodeError::NonCanonical);
        }
        let raw_slice = body_reader.take(raw_len)?;
        let mut raw_bytes = Vec::new();
        reserve_exact(&mut raw_bytes, raw_len, "raw HSACO")?;
        raw_bytes.extend_from_slice(raw_slice);
        let raw_hsaco = ExactRawHsacoV1::new(raw_identity, raw_bytes)
            .map_err(EnvelopeInputsDecodeError::Envelope)?;
        if !body_reader.is_empty() {
            return Err(EnvelopeInputsDecodeError::TrailingBytes);
        }
        let capsule = Self::new(direct_link_evidence, proof_records, raw_hsaco)
            .map_err(EnvelopeInputsDecodeError::Validation)?;
        Ok(capsule)
    }
}

fn canonical_length(
    direct_link_len: usize,
    raw_len: usize,
    proof_lengths: impl IntoIterator<Item = usize>,
) -> Option<usize> {
    proof_lengths.into_iter().try_fold(
        FIXED_HEADER_BYTES
            .checked_add(direct_link_len)?
            .checked_add(raw_len)?,
        |total, proof_len| total.checked_add(4)?.checked_add(proof_len),
    )
}

fn reserve_exact<T>(
    values: &mut Vec<T>,
    additional: usize,
    field: &'static str,
) -> Result<(), EnvelopeInputsDecodeError> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| EnvelopeInputsDecodeError::AllocationFailed {
            field,
            requested: additional,
        })
}

#[derive(Debug)]
#[non_exhaustive]
pub enum EnvelopeInputsValidationError {
    DirectLinkBindingCount { actual: usize },
    ProofCount { actual: usize },
    DuplicateProofKernel,
    ProofEvidenceTooLarge { max: usize },
    CapsuleTooLarge { max: usize },
}

impl fmt::Display for EnvelopeInputsValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DirectLinkBindingCount { actual } => write!(
                formatter,
                "Worker V2 envelope inputs require exactly one direct-link binding, found {actual}"
            ),
            Self::ProofCount { actual } => write!(
                formatter,
                "Worker V2 envelope inputs require 1..={MAX_KERNELS} proof records, found {actual}"
            ),
            Self::DuplicateProofKernel => {
                formatter.write_str("Worker V2 envelope inputs contain a duplicate proof kernel")
            }
            Self::ProofEvidenceTooLarge { max } => {
                write!(formatter, "Worker V2 proof evidence exceeds {max} bytes")
            }
            Self::CapsuleTooLarge { max } => {
                write!(
                    formatter,
                    "Worker V2 envelope input capsule exceeds {max} bytes"
                )
            }
        }
    }
}

impl std::error::Error for EnvelopeInputsValidationError {}

#[derive(Debug)]
#[non_exhaustive]
pub enum EnvelopeInputsDecodeError {
    TooLarge {
        max: usize,
    },
    Truncated,
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u16),
    NonZeroReserved,
    UnknownDigestAlgorithm,
    LengthOutOfRange {
        field: &'static str,
        value: u64,
        max: usize,
    },
    CountOutOfRange {
        field: &'static str,
        value: u64,
        max: usize,
    },
    LengthOverflow,
    AggregateLengthMismatch,
    AllocationFailed {
        field: &'static str,
        requested: usize,
    },
    TrailingBytes,
    DirectLink(DirectLinkDecodeError),
    Proof(ProofDecodeError),
    Envelope(EnvelopeValidationError),
    Validation(EnvelopeInputsValidationError),
    NonCanonical,
}

impl fmt::Display for EnvelopeInputsDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge { max } => {
                write!(formatter, "Worker V2 envelope inputs exceed {max} bytes")
            }
            Self::Truncated => formatter.write_str("Worker V2 envelope inputs are truncated"),
            Self::InvalidMagic => formatter.write_str("Worker V2 envelope inputs magic is invalid"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unsupported Worker V2 envelope inputs version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => write!(
                formatter,
                "unsupported Worker V2 envelope inputs flags {flags:#x}"
            ),
            Self::NonZeroReserved => {
                formatter.write_str("Worker V2 envelope inputs reserved field is nonzero")
            }
            Self::UnknownDigestAlgorithm => {
                formatter.write_str("Worker V2 envelope inputs use an unknown raw digest algorithm")
            }
            Self::LengthOutOfRange { field, value, max } => {
                write!(formatter, "{field} length {value} exceeds {max}")
            }
            Self::CountOutOfRange { field, value, max } => {
                write!(formatter, "{field} count {value} is outside 1..={max}")
            }
            Self::LengthOverflow => {
                formatter.write_str("Worker V2 envelope inputs length overflows")
            }
            Self::AggregateLengthMismatch => formatter
                .write_str("Worker V2 envelope input component lengths do not match the capsule"),
            Self::AllocationFailed { field, requested } => write!(
                formatter,
                "could not allocate {requested} elements for Worker V2 envelope input {field}"
            ),
            Self::TrailingBytes => {
                formatter.write_str("Worker V2 envelope inputs have trailing bytes")
            }
            Self::DirectLink(error) => error.fmt(formatter),
            Self::Proof(error) => error.fmt(formatter),
            Self::Envelope(error) => error.fmt(formatter),
            Self::Validation(error) => error.fmt(formatter),
            Self::NonCanonical => {
                formatter.write_str("Worker V2 envelope inputs are not canonical")
            }
        }
    }
}

impl std::error::Error for EnvelopeInputsDecodeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::DirectLink(error) => Some(error),
            Self::Proof(error) => Some(error),
            Self::Envelope(error) => Some(error),
            Self::Validation(error) => Some(error),
            _ => None,
        }
    }
}

struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    const fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    const fn remaining(&self) -> &'a [u8] {
        self.remaining
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], EnvelopeInputsDecodeError> {
        if self.remaining.len() < count {
            return Err(EnvelopeInputsDecodeError::Truncated);
        }
        let (value, remaining) = self.remaining.split_at(count);
        self.remaining = remaining;
        Ok(value)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], EnvelopeInputsDecodeError> {
        let mut value = [0; N];
        value.copy_from_slice(self.take(N)?);
        Ok(value)
    }

    fn u8(&mut self) -> Result<u8, EnvelopeInputsDecodeError> {
        Ok(self.array::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, EnvelopeInputsDecodeError> {
        Ok(u16::from_le_bytes(self.array()?))
    }

    fn length(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<usize, EnvelopeInputsDecodeError> {
        let value = u64::from(u32::from_le_bytes(self.array()?));
        if value > max as u64 {
            Err(EnvelopeInputsDecodeError::LengthOutOfRange { field, value, max })
        } else {
            Ok(value as usize)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_failure_is_explicit_before_copy() {
        let mut bytes = Vec::<u8>::new();
        assert!(matches!(
            reserve_exact(&mut bytes, usize::MAX, "test bytes"),
            Err(EnvelopeInputsDecodeError::AllocationFailed {
                field: "test bytes",
                requested: usize::MAX,
            })
        ));
        assert!(bytes.is_empty());
    }
}
