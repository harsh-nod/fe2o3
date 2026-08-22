use std::{error::Error, fmt};

use fe2o3_build_authority::{CompilerClosureErrorV2, CompilerClosureV2};
use sha2::{Digest, Sha256};

use crate::MAX_DURABLE_FINALIZED_ARTIFACT_BYTES;

const BINDING_MAGIC_V1: &[u8] = b"FE2O3-WORKER-V3-PUBLICATION-BINDING-V1\0";
const BINDING_VERSION_V1: u16 = 1;
const COMPILER_CLOSURE_BYTES_V2: usize = (6 * 32) + 2 + 32;
const IDENTITY_COUNT_V1: usize = 7;
const BINDING_BODY_BYTES_V1: usize =
    BINDING_MAGIC_V1.len() + 2 + COMPILER_CLOSURE_BYTES_V2 + (IDENTITY_COUNT_V1 * 32) + (2 * 8);
const BINDING_CANONICAL_BYTES_V1: usize = BINDING_BODY_BYTES_V1 + 32;

/// Maximum canonical bytes accepted for one strict Worker V3 publication binding.
pub const MAX_WORKER_V3_PUBLICATION_BINDING_BYTES_V1: usize = BINDING_CANONICAL_BYTES_V1;

/// A required identity axis in one strict Worker V3 publication binding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkerV3PublicationBindingIdentityFieldV1 {
    PublicationIntentRecord,
    Finalization,
    SourceEvidence,
    CompilerHandoffBinding,
    RawInspection,
    RawOutput,
    FinalizedOutput,
}

impl fmt::Display for WorkerV3PublicationBindingIdentityFieldV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::PublicationIntentRecord => "publication-intent record",
            Self::Finalization => "finalization",
            Self::SourceEvidence => "source evidence",
            Self::CompilerHandoffBinding => "compiler-handoff binding",
            Self::RawInspection => "raw inspection",
            Self::RawOutput => "raw output",
            Self::FinalizedOutput => "finalized output",
        };
        formatter.write_str(name)
    }
}

/// Construction or canonical-codec failure for a strict Worker V3 publication binding.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3PublicationBindingErrorV1 {
    ZeroIdentity {
        field: WorkerV3PublicationBindingIdentityFieldV1,
    },
    InvalidArtifactLength {
        field: WorkerV3PublicationBindingIdentityFieldV1,
        actual: u64,
    },
    InvalidCompilerClosure(CompilerClosureErrorV2),
    TooLarge {
        actual: usize,
        maximum: usize,
    },
    Truncated,
    TrailingBytes,
    BadMagic,
    UnsupportedVersion {
        actual: u16,
    },
    ChecksumMismatch,
    NonCanonical,
}

impl fmt::Display for WorkerV3PublicationBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroIdentity { field } => write!(formatter, "{field} identity must be nonzero"),
            Self::InvalidArtifactLength { field, actual } => write!(
                formatter,
                "{field} length {actual} is outside the durable artifact bound"
            ),
            Self::InvalidCompilerClosure(error) => error.fmt(formatter),
            Self::TooLarge { actual, maximum } => write!(
                formatter,
                "Worker V3 publication binding is {actual} bytes; maximum is {maximum}"
            ),
            Self::Truncated => formatter.write_str("truncated Worker V3 publication binding"),
            Self::TrailingBytes => {
                formatter.write_str("trailing bytes in Worker V3 publication binding")
            }
            Self::BadMagic => formatter.write_str("bad Worker V3 publication binding magic"),
            Self::UnsupportedVersion { actual } => write!(
                formatter,
                "unsupported Worker V3 publication binding version {actual}"
            ),
            Self::ChecksumMismatch => {
                formatter.write_str("Worker V3 publication binding checksum mismatch")
            }
            Self::NonCanonical => formatter.write_str("noncanonical Worker V3 publication binding"),
        }
    }
}

impl Error for WorkerV3PublicationBindingErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidCompilerClosure(error) => Some(error),
            _ => None,
        }
    }
}

/// Exact inert lineage bound to one completed strict Worker V3 publication.
///
/// This value deliberately carries the complete compiler closure and independent finalizer axes
/// instead of projecting V3 evidence into the V2 closure. It is coordination evidence only and
/// grants no compiler, proof, publication, load, or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3PublicationBindingV1 {
    compiler_closure: CompilerClosureV2,
    publication_intent_record_identity: [u8; 32],
    finalization_identity: [u8; 32],
    source_evidence_identity: [u8; 32],
    compiler_handoff_binding_identity: [u8; 32],
    raw_inspection_identity: [u8; 32],
    raw_output_sha256: [u8; 32],
    raw_output_length: u64,
    finalized_output_sha256: [u8; 32],
    finalized_output_length: u64,
}

impl WorkerV3PublicationBindingV1 {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        compiler_closure: CompilerClosureV2,
        publication_intent_record_identity: [u8; 32],
        finalization_identity: [u8; 32],
        source_evidence_identity: [u8; 32],
        compiler_handoff_binding_identity: [u8; 32],
        raw_inspection_identity: [u8; 32],
        raw_output_sha256: [u8; 32],
        raw_output_length: u64,
        finalized_output_sha256: [u8; 32],
        finalized_output_length: u64,
    ) -> Result<Self, WorkerV3PublicationBindingErrorV1> {
        let binding = Self {
            compiler_closure,
            publication_intent_record_identity,
            finalization_identity,
            source_evidence_identity,
            compiler_handoff_binding_identity,
            raw_inspection_identity,
            raw_output_sha256,
            raw_output_length,
            finalized_output_sha256,
            finalized_output_length,
        };
        binding.validate()?;
        Ok(binding)
    }

    pub const fn compiler_closure(self) -> CompilerClosureV2 {
        self.compiler_closure
    }

    pub const fn publication_intent_record_identity(self) -> [u8; 32] {
        self.publication_intent_record_identity
    }

    pub const fn finalization_identity(self) -> [u8; 32] {
        self.finalization_identity
    }

    pub const fn source_evidence_identity(self) -> [u8; 32] {
        self.source_evidence_identity
    }

    pub const fn compiler_handoff_binding_identity(self) -> [u8; 32] {
        self.compiler_handoff_binding_identity
    }

    pub const fn raw_inspection_identity(self) -> [u8; 32] {
        self.raw_inspection_identity
    }

    pub const fn raw_output_sha256(self) -> [u8; 32] {
        self.raw_output_sha256
    }

    pub const fn raw_output_length(self) -> u64 {
        self.raw_output_length
    }

    pub const fn finalized_output_sha256(self) -> [u8; 32] {
        self.finalized_output_sha256
    }

    pub const fn finalized_output_length(self) -> u64 {
        self.finalized_output_length
    }

    pub fn encode_canonical(self) -> Result<Vec<u8>, WorkerV3PublicationBindingErrorV1> {
        self.validate()?;
        let mut bytes = Vec::with_capacity(BINDING_CANONICAL_BYTES_V1);
        bytes.extend_from_slice(BINDING_MAGIC_V1);
        bytes.extend_from_slice(&BINDING_VERSION_V1.to_le_bytes());
        push_compiler_closure(&mut bytes, self.compiler_closure);
        for identity in [
            self.publication_intent_record_identity,
            self.finalization_identity,
            self.source_evidence_identity,
            self.compiler_handoff_binding_identity,
            self.raw_inspection_identity,
            self.raw_output_sha256,
        ] {
            bytes.extend_from_slice(&identity);
        }
        bytes.extend_from_slice(&self.raw_output_length.to_le_bytes());
        bytes.extend_from_slice(&self.finalized_output_sha256);
        bytes.extend_from_slice(&self.finalized_output_length.to_le_bytes());
        debug_assert_eq!(bytes.len(), BINDING_BODY_BYTES_V1);
        let checksum: [u8; 32] = Sha256::digest(&bytes).into();
        bytes.extend_from_slice(&checksum);
        debug_assert_eq!(bytes.len(), BINDING_CANONICAL_BYTES_V1);
        Ok(bytes)
    }

    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3PublicationBindingErrorV1> {
        if bytes.len() > MAX_WORKER_V3_PUBLICATION_BINDING_BYTES_V1 {
            return Err(WorkerV3PublicationBindingErrorV1::TooLarge {
                actual: bytes.len(),
                maximum: MAX_WORKER_V3_PUBLICATION_BINDING_BYTES_V1,
            });
        }
        if bytes.len() < BINDING_CANONICAL_BYTES_V1 {
            return Err(WorkerV3PublicationBindingErrorV1::Truncated);
        }
        if bytes.len() > BINDING_CANONICAL_BYTES_V1 {
            return Err(WorkerV3PublicationBindingErrorV1::TrailingBytes);
        }
        let (body, checksum) = bytes.split_at(BINDING_BODY_BYTES_V1);
        if <[u8; 32]>::from(Sha256::digest(body)).as_slice() != checksum {
            return Err(WorkerV3PublicationBindingErrorV1::ChecksumMismatch);
        }
        let mut decoder = Decoder::new(body);
        if decoder.take(BINDING_MAGIC_V1.len())? != BINDING_MAGIC_V1 {
            return Err(WorkerV3PublicationBindingErrorV1::BadMagic);
        }
        let version = decoder.u16()?;
        if version != BINDING_VERSION_V1 {
            return Err(WorkerV3PublicationBindingErrorV1::UnsupportedVersion { actual: version });
        }
        let compiler_closure = decoder.compiler_closure()?;
        let binding = Self::new(
            compiler_closure,
            decoder.identity()?,
            decoder.identity()?,
            decoder.identity()?,
            decoder.identity()?,
            decoder.identity()?,
            decoder.identity()?,
            decoder.u64()?,
            decoder.identity()?,
            decoder.u64()?,
        )?;
        if !decoder.finished() {
            return Err(WorkerV3PublicationBindingErrorV1::TrailingBytes);
        }
        if binding.encode_canonical()?.as_slice() != bytes {
            return Err(WorkerV3PublicationBindingErrorV1::NonCanonical);
        }
        Ok(binding)
    }

    pub const fn grants_compiler_authority(self) -> bool {
        false
    }

    pub const fn grants_proof_authority(self) -> bool {
        false
    }

    pub const fn grants_publication_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }

    fn validate(self) -> Result<(), WorkerV3PublicationBindingErrorV1> {
        for (field, identity) in [
            (
                WorkerV3PublicationBindingIdentityFieldV1::PublicationIntentRecord,
                self.publication_intent_record_identity,
            ),
            (
                WorkerV3PublicationBindingIdentityFieldV1::Finalization,
                self.finalization_identity,
            ),
            (
                WorkerV3PublicationBindingIdentityFieldV1::SourceEvidence,
                self.source_evidence_identity,
            ),
            (
                WorkerV3PublicationBindingIdentityFieldV1::CompilerHandoffBinding,
                self.compiler_handoff_binding_identity,
            ),
            (
                WorkerV3PublicationBindingIdentityFieldV1::RawInspection,
                self.raw_inspection_identity,
            ),
            (
                WorkerV3PublicationBindingIdentityFieldV1::RawOutput,
                self.raw_output_sha256,
            ),
            (
                WorkerV3PublicationBindingIdentityFieldV1::FinalizedOutput,
                self.finalized_output_sha256,
            ),
        ] {
            if identity == [0; 32] {
                return Err(WorkerV3PublicationBindingErrorV1::ZeroIdentity { field });
            }
        }
        validate_length(
            WorkerV3PublicationBindingIdentityFieldV1::RawOutput,
            self.raw_output_length,
        )?;
        validate_length(
            WorkerV3PublicationBindingIdentityFieldV1::FinalizedOutput,
            self.finalized_output_length,
        )
    }
}

fn validate_length(
    field: WorkerV3PublicationBindingIdentityFieldV1,
    actual: u64,
) -> Result<(), WorkerV3PublicationBindingErrorV1> {
    if actual == 0 || actual > MAX_DURABLE_FINALIZED_ARTIFACT_BYTES as u64 {
        return Err(WorkerV3PublicationBindingErrorV1::InvalidArtifactLength { field, actual });
    }
    Ok(())
}

fn push_compiler_closure(bytes: &mut Vec<u8>, closure: CompilerClosureV2) {
    for digest in [
        closure.cargo_executable_sha256(),
        closure.cargo_binding_trampoline_sha256(),
        closure.cargo_fe2o3_binding_wrapper_sha256(),
        closure.rustc_executable_sha256(),
        closure.rustc_runtime_tree_sha256(),
        closure.codegen_backend_sha256(),
    ] {
        bytes.extend_from_slice(&digest);
    }
    bytes.extend_from_slice(
        &closure
            .cargo_binding_transition_protocol_version()
            .to_le_bytes(),
    );
    bytes.extend_from_slice(&closure.identity_sha256());
}

struct Decoder<'a> {
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Decoder<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, cursor: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], WorkerV3PublicationBindingErrorV1> {
        let end = self
            .cursor
            .checked_add(length)
            .ok_or(WorkerV3PublicationBindingErrorV1::Truncated)?;
        let value = self
            .bytes
            .get(self.cursor..end)
            .ok_or(WorkerV3PublicationBindingErrorV1::Truncated)?;
        self.cursor = end;
        Ok(value)
    }

    fn u16(&mut self) -> Result<u16, WorkerV3PublicationBindingErrorV1> {
        let mut bytes = [0; 2];
        bytes.copy_from_slice(self.take(2)?);
        Ok(u16::from_le_bytes(bytes))
    }

    fn u64(&mut self) -> Result<u64, WorkerV3PublicationBindingErrorV1> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(bytes))
    }

    fn identity(&mut self) -> Result<[u8; 32], WorkerV3PublicationBindingErrorV1> {
        let mut identity = [0; 32];
        identity.copy_from_slice(self.take(32)?);
        Ok(identity)
    }

    fn compiler_closure(&mut self) -> Result<CompilerClosureV2, WorkerV3PublicationBindingErrorV1> {
        let cargo = self.identity()?;
        let trampoline = self.identity()?;
        let wrapper = self.identity()?;
        let rustc = self.identity()?;
        let runtime = self.identity()?;
        let backend = self.identity()?;
        let version = self.u16()?;
        let identity = self.identity()?;
        CompilerClosureV2::from_pins_and_identity(
            cargo, trampoline, wrapper, rustc, runtime, backend, version, identity,
        )
        .map_err(WorkerV3PublicationBindingErrorV1::InvalidCompilerClosure)
    }

    const fn finished(&self) -> bool {
        self.cursor == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn binding() -> WorkerV3PublicationBindingV1 {
        WorkerV3PublicationBindingV1::new(
            CompilerClosureV2::new([1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32]).unwrap(),
            [7; 32],
            [8; 32],
            [9; 32],
            [10; 32],
            [11; 32],
            [12; 32],
            13,
            [14; 32],
            15,
        )
        .unwrap()
    }

    #[test]
    fn canonical_round_trip_preserves_every_axis() {
        let binding = binding();
        let bytes = binding.encode_canonical().unwrap();
        assert_eq!(bytes.len(), MAX_WORKER_V3_PUBLICATION_BINDING_BYTES_V1);
        assert_eq!(
            WorkerV3PublicationBindingV1::decode_canonical(&bytes).unwrap(),
            binding
        );
    }

    #[test]
    fn rejects_zero_identity_and_out_of_range_lengths() {
        let closure = binding().compiler_closure();
        assert!(matches!(
            WorkerV3PublicationBindingV1::new(
                closure, [0; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], 7, [8; 32], 9,
            ),
            Err(WorkerV3PublicationBindingErrorV1::ZeroIdentity {
                field: WorkerV3PublicationBindingIdentityFieldV1::PublicationIntentRecord
            })
        ));
        assert!(matches!(
            WorkerV3PublicationBindingV1::new(
                closure, [1; 32], [2; 32], [3; 32], [4; 32], [5; 32], [6; 32], 0, [8; 32], 9,
            ),
            Err(WorkerV3PublicationBindingErrorV1::InvalidArtifactLength {
                field: WorkerV3PublicationBindingIdentityFieldV1::RawOutput,
                actual: 0,
            })
        ));
    }

    #[test]
    fn hostile_codec_inputs_fail_closed() {
        let bytes = binding().encode_canonical().unwrap();
        assert_eq!(
            WorkerV3PublicationBindingV1::decode_canonical(&bytes[..bytes.len() - 1]),
            Err(WorkerV3PublicationBindingErrorV1::Truncated)
        );
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            WorkerV3PublicationBindingV1::decode_canonical(&trailing),
            Err(WorkerV3PublicationBindingErrorV1::TooLarge { .. })
        ));
        let mut corrupt = bytes;
        corrupt[BINDING_MAGIC_V1.len() + 2] ^= 1;
        assert_eq!(
            WorkerV3PublicationBindingV1::decode_canonical(&corrupt),
            Err(WorkerV3PublicationBindingErrorV1::ChecksumMismatch)
        );
    }

    #[test]
    fn binding_is_inert() {
        let binding = binding();
        assert!(!binding.grants_compiler_authority());
        assert!(!binding.grants_proof_authority());
        assert!(!binding.grants_publication_authority());
        assert!(!binding.grants_load_authority());
        assert!(!binding.grants_launch_authority());
    }
}
