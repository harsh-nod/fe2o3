use core::{fmt, str};
use std::error::Error;

use fe2o3_hsaco::MAX_HSACO_BYTES;
use fe2o3_kernel_descriptor::{MAX_KERNELS, MAX_NAME_BYTES, ValidName, ValidationError};
use fe2o3_runtime_protocol::MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2;
use sha2::{Digest, Sha256};

const SHA256_BYTES: usize = 32;
const REQUEST_HEADER_BYTES: usize = 32;
const RESPONSE_HEADER_BYTES: usize = 32;
const FD_PAYLOAD_DESCRIPTOR_BYTES: usize = 48;
const ENTRY_COORDINATE_FIXED_BYTES: usize = 108;
const MIN_ENTRY_COORDINATE_BYTES: usize = ENTRY_COORDINATE_FIXED_BYTES + 2;
const MAX_ENTRY_COORDINATE_BYTES: usize = ENTRY_COORDINATE_FIXED_BYTES + (2 * MAX_NAME_BYTES);
const REQUEST_FIXED_BYTES: usize = REQUEST_HEADER_BYTES
    + (4 * SHA256_BYTES)
    + (WORKER_V3_VERIFICATION_FD_PAYLOADS_V1 * FD_PAYLOAD_DESCRIPTOR_BYTES)
    + SHA256_BYTES;
const RESPONSE_PREIMAGE_BYTES: usize = RESPONSE_HEADER_BYTES + (6 * SHA256_BYTES);
const REQUEST_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/ROSTER-VERIFICATION-REQUEST/V1\0";
const RESPONSE_IDENTITY_DOMAIN: &[u8] = b"FE2O3/WORKER-V3/ROSTER-VERIFICATION-RESPONSE/V1\0";

/// Magic for one authority-free Worker V3 verification request frame.
pub const WORKER_V3_VERIFICATION_REQUEST_MAGIC_V1: [u8; 8] = *b"F3WVRQ1\0";
/// Magic for one authority-free Worker V3 verification response frame.
pub const WORKER_V3_VERIFICATION_RESPONSE_MAGIC_V1: [u8; 8] = *b"F3WVRS1\0";
/// Version of the Worker V3 verification request and response frame schemas.
pub const WORKER_V3_VERIFICATION_REQUEST_VERSION_V1: u16 = 1;
/// Exact number of file descriptors bound by every request frame.
pub const WORKER_V3_VERIFICATION_FD_PAYLOADS_V1: usize = 2;
/// Maximum number of canonical descriptor-order roster coordinates in one request.
pub const MAX_WORKER_V3_VERIFICATION_ENTRIES_V1: usize = MAX_KERNELS;
/// Maximum UTF-8 byte length of a logical or export name, matching the kernel descriptor.
pub const MAX_WORKER_V3_VERIFICATION_ENTRY_NAME_BYTES_V1: usize = MAX_NAME_BYTES;
/// Maximum exact V2 load-envelope payload length accepted by a request descriptor.
pub const MAX_WORKER_V3_VERIFICATION_ENVELOPE_FD_BYTES_V1: u64 =
    MAX_WORKER_V3_LOAD_ENVELOPE_BYTES_V2 as u64;
/// Maximum exact finalized-HSACO payload length accepted by a request descriptor.
pub const MAX_WORKER_V3_VERIFICATION_HSACO_FD_BYTES_V1: u64 = MAX_HSACO_BYTES as u64;
/// Maximum canonical request frame length.
pub const MAX_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1: usize =
    REQUEST_FIXED_BYTES + (MAX_WORKER_V3_VERIFICATION_ENTRIES_V1 * MAX_ENTRY_COORDINATE_BYTES);
/// Minimum canonical request frame length, including one roster coordinate.
pub const MIN_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1: usize =
    REQUEST_FIXED_BYTES + MIN_ENTRY_COORDINATE_BYTES;
/// Exact canonical response frame length.
pub const WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1: usize = RESPONSE_PREIMAGE_BYTES + SHA256_BYTES;

/// Identity field rejected because it contains the all-zero sentinel.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3VerificationIdentityFieldV1 {
    /// Caller-generated challenge.
    Challenge,
    /// Exact canonical roster.
    Roster,
    /// Caller-pinned verification policy.
    Policy,
    /// Caller-pinned verifier measurement.
    Measurement,
    /// Authority-free service transcript.
    Transcript,
}

/// Entry coordinate field rejected because it is duplicated in one roster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3VerificationEntryIdentityFieldV1 {
    /// Logical kernel name.
    LogicalName,
    /// Exported ELF symbol name.
    ExportName,
    /// Host lineage identity.
    Lineage,
    /// Compiler-generated marker binding identity.
    MarkerBinding,
}

/// Bounded kernel-name field rejected while decoding an entry coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3VerificationEntryNameFieldV1 {
    /// Logical kernel name.
    LogicalName,
    /// Exported ELF symbol name.
    ExportName,
}

/// Kind and canonical file-descriptor ordinal of a request payload.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum WorkerV3VerificationFdPayloadKindV1 {
    /// Complete canonical Worker V3 V2 load envelope at descriptor ordinal zero.
    LoadEnvelopeV2,
    /// Exact finalized HSACO bytes at descriptor ordinal one.
    FinalizedHsaco,
}

impl WorkerV3VerificationFdPayloadKindV1 {
    const fn wire_tag(self) -> u16 {
        match self {
            Self::LoadEnvelopeV2 => 1,
            Self::FinalizedHsaco => 2,
        }
    }

    const fn from_wire_tag(tag: u16) -> Option<Self> {
        match tag {
            1 => Some(Self::LoadEnvelopeV2),
            2 => Some(Self::FinalizedHsaco),
            _ => None,
        }
    }

    /// Returns the only permitted `SCM_RIGHTS` descriptor ordinal for this payload.
    pub const fn fd_ordinal(self) -> u32 {
        match self {
            Self::LoadEnvelopeV2 => 0,
            Self::FinalizedHsaco => 1,
        }
    }

    /// Returns the maximum exact byte length accepted for this payload kind.
    pub const fn maximum_byte_len(self) -> u64 {
        match self {
            Self::LoadEnvelopeV2 => MAX_WORKER_V3_VERIFICATION_ENVELOPE_FD_BYTES_V1,
            Self::FinalizedHsaco => MAX_WORKER_V3_VERIFICATION_HSACO_FD_BYTES_V1,
        }
    }
}

macro_rules! caller_identity {
    ($name:ident, $field:expr, $doc:literal) => {
        #[doc = $doc]
        #[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name([u8; SHA256_BYTES]);

        impl $name {
            /// Constructs a nonzero caller-owned identity.
            pub fn new(
                bytes: [u8; SHA256_BYTES],
            ) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
                if bytes == [0; SHA256_BYTES] {
                    return Err(WorkerV3VerificationProtocolErrorV1::ZeroIdentity($field));
                }
                Ok(Self(bytes))
            }

            /// Returns the exact identity bytes.
            pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
                &self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }
    };
}

caller_identity!(
    WorkerV3VerificationFreshChallengeV1,
    WorkerV3VerificationIdentityFieldV1::Challenge,
    "Nonzero caller-generated challenge for one request. It requires entropy and replay exclusion and is not the host's deterministic roster challenge."
);

impl WorkerV3VerificationFreshChallengeV1 {
    /// Reports that the protocol cannot establish freshness without caller replay state.
    pub const fn freshness_must_be_enforced_externally(&self) -> bool {
        true
    }
}
caller_identity!(
    WorkerV3VerificationRosterIdentityV1,
    WorkerV3VerificationIdentityFieldV1::Roster,
    "Caller-owned identity of one exact canonical descriptor-order roster."
);
caller_identity!(
    WorkerV3VerificationPolicyIdentityV1,
    WorkerV3VerificationIdentityFieldV1::Policy,
    "Caller-pinned identity of the complete verification policy."
);
caller_identity!(
    WorkerV3VerificationMeasurementIdentityV1,
    WorkerV3VerificationIdentityFieldV1::Measurement,
    "Caller-pinned identity of the expected verifier executable and runtime closure."
);
caller_identity!(
    WorkerV3VerificationTranscriptIdentityV1,
    WorkerV3VerificationIdentityFieldV1::Transcript,
    "Nonzero identity of an authority-free service transcript. It is not a theorem identity."
);

/// Domain-separated identity of one exact canonical request frame.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3VerificationRequestIdentityV1([u8; SHA256_BYTES]);

impl WorkerV3VerificationRequestIdentityV1 {
    /// Returns the exact domain-separated identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    /// Recomputes this identity over one complete canonical request frame.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        WorkerV3VerificationRequestV1::decode_canonical(bytes)
            .is_ok_and(|request| request.identity == self)
    }
}

impl fmt::Debug for WorkerV3VerificationRequestIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("WorkerV3VerificationRequestIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Domain-separated identity of one exact canonical response frame.
#[derive(Clone, Copy, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkerV3VerificationResponseIdentityV1([u8; SHA256_BYTES]);

impl WorkerV3VerificationResponseIdentityV1 {
    /// Returns the exact domain-separated identity bytes.
    pub const fn as_bytes(&self) -> &[u8; SHA256_BYTES] {
        &self.0
    }

    /// Recomputes this identity over one complete canonical response frame.
    pub fn matches_canonical_bytes(self, bytes: &[u8]) -> bool {
        WorkerV3VerificationResponseV1::decode_canonical(bytes)
            .is_ok_and(|response| response.identity == self)
    }
}

impl fmt::Debug for WorkerV3VerificationResponseIdentityV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("WorkerV3VerificationResponseIdentityV1")
            .field(&self.0)
            .finish()
    }
}

/// Inert description of one exact file-descriptor payload.
///
/// The digest and length bind bytes but do not authenticate descriptor provenance, immutability,
/// or custody. The transport owner must enforce those properties before using payload bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkerV3VerificationFdPayloadDescriptorV1 {
    kind: WorkerV3VerificationFdPayloadKindV1,
    fd_ordinal: u32,
    byte_len: u64,
    sha256: [u8; SHA256_BYTES],
}

impl WorkerV3VerificationFdPayloadDescriptorV1 {
    /// Constructs the descriptor for one exact complete V2 load envelope at fd ordinal zero.
    pub fn load_envelope_v2(
        byte_len: u64,
        sha256: [u8; SHA256_BYTES],
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        Self::new(
            WorkerV3VerificationFdPayloadKindV1::LoadEnvelopeV2,
            0,
            byte_len,
            sha256,
        )
    }

    /// Constructs the descriptor for exact finalized HSACO bytes at fd ordinal one.
    pub fn finalized_hsaco(
        byte_len: u64,
        sha256: [u8; SHA256_BYTES],
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        Self::new(
            WorkerV3VerificationFdPayloadKindV1::FinalizedHsaco,
            1,
            byte_len,
            sha256,
        )
    }

    fn new(
        kind: WorkerV3VerificationFdPayloadKindV1,
        fd_ordinal: u32,
        byte_len: u64,
        sha256: [u8; SHA256_BYTES],
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        if fd_ordinal != kind.fd_ordinal() {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidFdOrdinal {
                kind,
                actual: fd_ordinal,
            });
        }
        if byte_len == 0 || byte_len > kind.maximum_byte_len() {
            return Err(
                WorkerV3VerificationProtocolErrorV1::PayloadLengthOutOfRange {
                    kind,
                    actual: byte_len,
                    maximum: kind.maximum_byte_len(),
                },
            );
        }
        if sha256 == [0; SHA256_BYTES] {
            return Err(WorkerV3VerificationProtocolErrorV1::ZeroPayloadDigest { kind });
        }
        Ok(Self {
            kind,
            fd_ordinal,
            byte_len,
            sha256,
        })
    }

    /// Returns the semantic payload kind.
    pub const fn kind(&self) -> WorkerV3VerificationFdPayloadKindV1 {
        self.kind
    }

    /// Returns the required zero-based position in the accompanying descriptor array.
    pub const fn fd_ordinal(&self) -> u32 {
        self.fd_ordinal
    }

    /// Returns the exact number of payload bytes that must be read.
    pub const fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// Returns the SHA-256 digest of exactly `byte_len` payload bytes.
    pub const fn sha256(&self) -> &[u8; SHA256_BYTES] {
        &self.sha256
    }

    fn encode_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.kind.wire_tag().to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&self.fd_ordinal.to_le_bytes());
        bytes.extend_from_slice(&self.byte_len.to_le_bytes());
        bytes.extend_from_slice(&self.sha256);
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        let tag = reader.u16()?;
        let kind = WorkerV3VerificationFdPayloadKindV1::from_wire_tag(tag)
            .ok_or(WorkerV3VerificationProtocolErrorV1::UnknownPayloadKind { actual: tag })?;
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(
                WorkerV3VerificationProtocolErrorV1::UnsupportedPayloadFlags { actual: flags },
            );
        }
        Self::new(kind, reader.u32()?, reader.u64()?, reader.fixed()?)
    }
}

/// Canonical descriptor-order coordinate for one expected roster entry.
///
/// Names are untrusted duplicates of policy and envelope coordinates. The protected service must
/// cross-check all fields. Generated host-contract identities may repeat across otherwise distinct
/// entries because multiple kernels can share one generated ABI and effect contract.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3VerificationEntryCoordinateV1 {
    ordinal: u32,
    logical_name: ValidName,
    export_name: ValidName,
    lineage_identity: [u8; SHA256_BYTES],
    marker_binding_identity: [u8; SHA256_BYTES],
    generated_host_contract_identity: [u8; SHA256_BYTES],
}

impl WorkerV3VerificationEntryCoordinateV1 {
    /// Constructs a nonzero inert coordinate. The enclosing request enforces order and uniqueness.
    pub fn new(
        ordinal: u32,
        logical_name: impl Into<String>,
        export_name: impl Into<String>,
        lineage_identity: [u8; SHA256_BYTES],
        marker_binding_identity: [u8; SHA256_BYTES],
        generated_host_contract_identity: [u8; SHA256_BYTES],
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        let logical_name = ValidName::new(logical_name).map_err(|source| {
            WorkerV3VerificationProtocolErrorV1::InvalidEntryName {
                ordinal,
                field: WorkerV3VerificationEntryNameFieldV1::LogicalName,
                source,
            }
        })?;
        let export_name = ValidName::new(export_name).map_err(|source| {
            WorkerV3VerificationProtocolErrorV1::InvalidEntryName {
                ordinal,
                field: WorkerV3VerificationEntryNameFieldV1::ExportName,
                source,
            }
        })?;
        for (identity, field) in [
            (lineage_identity, "lineage"),
            (marker_binding_identity, "marker binding"),
            (generated_host_contract_identity, "generated host contract"),
        ] {
            if identity == [0; SHA256_BYTES] {
                return Err(WorkerV3VerificationProtocolErrorV1::ZeroEntryIdentity {
                    ordinal,
                    field,
                });
            }
        }
        Ok(Self {
            ordinal,
            logical_name,
            export_name,
            lineage_identity,
            marker_binding_identity,
            generated_host_contract_identity,
        })
    }

    /// Returns the required descriptor-table ordinal.
    pub const fn ordinal(&self) -> u32 {
        self.ordinal
    }

    /// Returns the bounded logical kernel name supplied for cross-checking.
    pub fn logical_name(&self) -> &str {
        self.logical_name.as_str()
    }

    /// Returns the bounded exported ELF symbol name supplied for cross-checking.
    pub fn export_name(&self) -> &str {
        self.export_name.as_str()
    }

    /// Returns the exact host lineage identity at this ordinal.
    pub const fn lineage_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.lineage_identity
    }

    /// Returns the exact compiler-generated marker binding identity at this ordinal.
    pub const fn marker_binding_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.marker_binding_identity
    }

    /// Returns the exact generated host-contract identity at this ordinal.
    pub const fn generated_host_contract_identity(&self) -> &[u8; SHA256_BYTES] {
        &self.generated_host_contract_identity
    }

    fn encode_into(&self, bytes: &mut Vec<u8>) {
        bytes.extend_from_slice(&self.ordinal.to_le_bytes());
        bytes.extend_from_slice(&(self.logical_name.as_str().len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(self.export_name.as_str().len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&self.lineage_identity);
        bytes.extend_from_slice(&self.marker_binding_identity);
        bytes.extend_from_slice(&self.generated_host_contract_identity);
        bytes.extend_from_slice(self.logical_name.as_str().as_bytes());
        bytes.extend_from_slice(self.export_name.as_str().as_bytes());
    }

    fn encoded_len(&self) -> usize {
        ENTRY_COORDINATE_FIXED_BYTES
            + self.logical_name.as_str().len()
            + self.export_name.as_str().len()
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        let ordinal = reader.u32()?;
        let logical_name_len =
            reader.entry_name_len(ordinal, WorkerV3VerificationEntryNameFieldV1::LogicalName)?;
        let export_name_len =
            reader.entry_name_len(ordinal, WorkerV3VerificationEntryNameFieldV1::ExportName)?;
        if reader.u32()? != 0 {
            return Err(WorkerV3VerificationProtocolErrorV1::NoncanonicalReservedBytes);
        }
        let lineage_identity = reader.fixed()?;
        let marker_binding_identity = reader.fixed()?;
        let generated_host_contract_identity = reader.fixed()?;
        let logical_name = reader.entry_name(
            ordinal,
            WorkerV3VerificationEntryNameFieldV1::LogicalName,
            logical_name_len,
        )?;
        let export_name = reader.entry_name(
            ordinal,
            WorkerV3VerificationEntryNameFieldV1::ExportName,
            export_name_len,
        )?;
        Self::new(
            ordinal,
            logical_name,
            export_name,
            lineage_identity,
            marker_binding_identity,
            generated_host_contract_identity,
        )
    }
}

/// Bounded canonical request for one exact Worker V3 roster verification job.
///
/// Construction and decoding establish structure and byte identity only. They do not validate the
/// two payloads, authenticate a service, or establish any compiler, executable, or safety theorem.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3VerificationRequestV1 {
    challenge: WorkerV3VerificationFreshChallengeV1,
    roster_identity: WorkerV3VerificationRosterIdentityV1,
    policy_identity: WorkerV3VerificationPolicyIdentityV1,
    measurement_identity: WorkerV3VerificationMeasurementIdentityV1,
    payloads: [WorkerV3VerificationFdPayloadDescriptorV1; WORKER_V3_VERIFICATION_FD_PAYLOADS_V1],
    entries: Vec<WorkerV3VerificationEntryCoordinateV1>,
    identity: WorkerV3VerificationRequestIdentityV1,
    canonical_bytes: Vec<u8>,
}

impl WorkerV3VerificationRequestV1 {
    /// Constructs one canonical authority-free request frame.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        challenge: WorkerV3VerificationFreshChallengeV1,
        roster_identity: WorkerV3VerificationRosterIdentityV1,
        policy_identity: WorkerV3VerificationPolicyIdentityV1,
        measurement_identity: WorkerV3VerificationMeasurementIdentityV1,
        load_envelope: WorkerV3VerificationFdPayloadDescriptorV1,
        finalized_hsaco: WorkerV3VerificationFdPayloadDescriptorV1,
        entries: Vec<WorkerV3VerificationEntryCoordinateV1>,
    ) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        let payloads = [load_envelope, finalized_hsaco];
        validate_payload_order(&payloads)?;
        validate_entries(&entries)?;
        let entry_bytes = entries.iter().try_fold(0_usize, |total, entry| {
            total
                .checked_add(entry.encoded_len())
                .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)
        })?;
        let total_len = REQUEST_FIXED_BYTES
            .checked_add(entry_bytes)
            .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?;
        let mut canonical_bytes = Vec::new();
        canonical_bytes.try_reserve_exact(total_len).map_err(|_| {
            WorkerV3VerificationProtocolErrorV1::AllocationFailed {
                requested: total_len,
            }
        })?;
        canonical_bytes.extend_from_slice(&WORKER_V3_VERIFICATION_REQUEST_MAGIC_V1);
        canonical_bytes.extend_from_slice(&WORKER_V3_VERIFICATION_REQUEST_VERSION_V1.to_le_bytes());
        canonical_bytes.extend_from_slice(&0_u16.to_le_bytes());
        canonical_bytes.extend_from_slice(&(total_len as u64).to_le_bytes());
        canonical_bytes.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        canonical_bytes
            .extend_from_slice(&(WORKER_V3_VERIFICATION_FD_PAYLOADS_V1 as u16).to_le_bytes());
        canonical_bytes.extend_from_slice(&[0; 6]);
        canonical_bytes.extend_from_slice(challenge.as_bytes());
        canonical_bytes.extend_from_slice(roster_identity.as_bytes());
        canonical_bytes.extend_from_slice(policy_identity.as_bytes());
        canonical_bytes.extend_from_slice(measurement_identity.as_bytes());
        for payload in &payloads {
            payload.encode_into(&mut canonical_bytes);
        }
        for entry in &entries {
            entry.encode_into(&mut canonical_bytes);
        }
        debug_assert_eq!(canonical_bytes.len(), total_len - SHA256_BYTES);
        let identity = WorkerV3VerificationRequestIdentityV1(derive_identity(
            REQUEST_IDENTITY_DOMAIN,
            &canonical_bytes,
        ));
        canonical_bytes.extend_from_slice(identity.as_bytes());
        debug_assert_eq!(canonical_bytes.len(), total_len);
        Ok(Self {
            challenge,
            roster_identity,
            policy_identity,
            measurement_identity,
            payloads,
            entries,
            identity,
            canonical_bytes,
        })
    }

    /// Strictly decodes one complete bounded canonical request frame.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        if !(MIN_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1
            ..=MAX_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1)
            .contains(&bytes.len())
        {
            return Err(
                WorkerV3VerificationProtocolErrorV1::RequestLengthOutOfRange {
                    actual: bytes.len(),
                    minimum: MIN_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1,
                    maximum: MAX_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1,
                },
            );
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != WORKER_V3_VERIFICATION_REQUEST_MAGIC_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::BadRequestMagic);
        }
        let version = reader.u16()?;
        if version != WORKER_V3_VERIFICATION_REQUEST_VERSION_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::UnsupportedVersion {
                actual: version,
            });
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(WorkerV3VerificationProtocolErrorV1::UnsupportedFlags { actual: flags });
        }
        let declared_len = reader.u64()?;
        if declared_len != bytes.len() as u64 {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidTotalLength {
                declared: declared_len,
                actual: bytes.len(),
            });
        }
        let entry_count = reader.u32()? as usize;
        if entry_count == 0 || entry_count > MAX_WORKER_V3_VERIFICATION_ENTRIES_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::EntryCountOutOfRange {
                actual: entry_count,
                maximum: MAX_WORKER_V3_VERIFICATION_ENTRIES_V1,
            });
        }
        let payload_count = reader.u16()? as usize;
        if payload_count != WORKER_V3_VERIFICATION_FD_PAYLOADS_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidPayloadCount {
                actual: payload_count,
            });
        }
        if reader.fixed::<6>()? != [0; 6] {
            return Err(WorkerV3VerificationProtocolErrorV1::NoncanonicalReservedBytes);
        }
        let minimum_len = REQUEST_FIXED_BYTES
            .checked_add(
                entry_count
                    .checked_mul(MIN_ENTRY_COORDINATE_BYTES)
                    .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?,
            )
            .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?;
        let maximum_len = REQUEST_FIXED_BYTES
            .checked_add(
                entry_count
                    .checked_mul(MAX_ENTRY_COORDINATE_BYTES)
                    .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?,
            )
            .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?;
        if !(minimum_len..=maximum_len).contains(&bytes.len()) {
            return Err(
                WorkerV3VerificationProtocolErrorV1::InvalidEntrySectionLength {
                    entry_count,
                    actual: bytes.len(),
                    minimum: minimum_len,
                    maximum: maximum_len,
                },
            );
        }
        let challenge = WorkerV3VerificationFreshChallengeV1::new(reader.fixed()?)?;
        let roster_identity = WorkerV3VerificationRosterIdentityV1::new(reader.fixed()?)?;
        let policy_identity = WorkerV3VerificationPolicyIdentityV1::new(reader.fixed()?)?;
        let measurement_identity = WorkerV3VerificationMeasurementIdentityV1::new(reader.fixed()?)?;
        let payloads = [
            WorkerV3VerificationFdPayloadDescriptorV1::decode(&mut reader)?,
            WorkerV3VerificationFdPayloadDescriptorV1::decode(&mut reader)?,
        ];
        let mut entries = Vec::new();
        entries.try_reserve_exact(entry_count).map_err(|_| {
            WorkerV3VerificationProtocolErrorV1::AllocationFailed {
                requested: entry_count,
            }
        })?;
        for _ in 0..entry_count {
            entries.push(WorkerV3VerificationEntryCoordinateV1::decode(&mut reader)?);
        }
        let declared_identity = WorkerV3VerificationRequestIdentityV1(reader.fixed()?);
        if !reader.is_empty() {
            return Err(WorkerV3VerificationProtocolErrorV1::TrailingBytes);
        }
        let decoded = Self::new(
            challenge,
            roster_identity,
            policy_identity,
            measurement_identity,
            payloads[0],
            payloads[1],
            entries,
        )?;
        if decoded.identity != declared_identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(WorkerV3VerificationProtocolErrorV1::RequestIdentityMismatch);
        }
        Ok(decoded)
    }

    /// Returns the complete canonical request encoding.
    pub fn encode_canonical(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the request identity.
    pub const fn identity(&self) -> WorkerV3VerificationRequestIdentityV1 {
        self.identity
    }

    /// Returns the caller challenge. The caller must separately enforce freshness.
    pub const fn challenge(&self) -> WorkerV3VerificationFreshChallengeV1 {
        self.challenge
    }

    /// Returns the exact roster identity.
    pub const fn roster_identity(&self) -> WorkerV3VerificationRosterIdentityV1 {
        self.roster_identity
    }

    /// Returns the caller-pinned policy identity.
    pub const fn policy_identity(&self) -> WorkerV3VerificationPolicyIdentityV1 {
        self.policy_identity
    }

    /// Returns the caller-pinned verifier measurement identity.
    pub const fn measurement_identity(&self) -> WorkerV3VerificationMeasurementIdentityV1 {
        self.measurement_identity
    }

    /// Returns the two canonical fd payload descriptors in fd order.
    pub const fn payloads(
        &self,
    ) -> &[WorkerV3VerificationFdPayloadDescriptorV1; WORKER_V3_VERIFICATION_FD_PAYLOADS_V1] {
        &self.payloads
    }

    /// Returns the descriptor-table-ordered entry coordinates.
    pub fn entries(&self) -> &[WorkerV3VerificationEntryCoordinateV1] {
        &self.entries
    }

    /// Reports that this inert frame grants no theorem, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

/// Authority-free result of parsing a request frame at the service boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3VerificationResponseDispositionV1 {
    /// The exact request frame was accepted for processing; no theorem result is represented.
    RequestFramed,
    /// The request was rejected before any theorem result was represented.
    RequestRejected,
}

impl WorkerV3VerificationResponseDispositionV1 {
    const fn wire_tag(self) -> u16 {
        match self {
            Self::RequestFramed => 1,
            Self::RequestRejected => 2,
        }
    }

    const fn from_wire_tag(tag: u16) -> Option<Self> {
        match tag {
            1 => Some(Self::RequestFramed),
            2 => Some(Self::RequestRejected),
            _ => None,
        }
    }
}

/// Fixed-size authority-free response binding a service transcript to one request frame.
///
/// This type deliberately has no verified or successful theorem disposition and carries no
/// protected-roster evidence. Peer authentication and transcript semantics remain external.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkerV3VerificationResponseV1 {
    disposition: WorkerV3VerificationResponseDispositionV1,
    entry_count: u32,
    request_identity: WorkerV3VerificationRequestIdentityV1,
    challenge: WorkerV3VerificationFreshChallengeV1,
    roster_identity: WorkerV3VerificationRosterIdentityV1,
    policy_identity: WorkerV3VerificationPolicyIdentityV1,
    measurement_identity: WorkerV3VerificationMeasurementIdentityV1,
    transcript_identity: WorkerV3VerificationTranscriptIdentityV1,
    identity: WorkerV3VerificationResponseIdentityV1,
    canonical_bytes: [u8; WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1],
}

impl WorkerV3VerificationResponseV1 {
    /// Constructs a framing-only response bound to one exact decoded request.
    pub fn new(
        request: &WorkerV3VerificationRequestV1,
        disposition: WorkerV3VerificationResponseDispositionV1,
        transcript_identity: WorkerV3VerificationTranscriptIdentityV1,
    ) -> Self {
        Self::encode(ResponseFields {
            disposition,
            entry_count: request.entries.len() as u32,
            request_identity: request.identity,
            challenge: request.challenge,
            roster_identity: request.roster_identity,
            policy_identity: request.policy_identity,
            measurement_identity: request.measurement_identity,
            transcript_identity,
        })
    }

    /// Strictly decodes one complete canonical response frame.
    pub fn decode_canonical(bytes: &[u8]) -> Result<Self, WorkerV3VerificationProtocolErrorV1> {
        if bytes.len() != WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidResponseLength {
                actual: bytes.len(),
            });
        }
        let mut reader = Reader::new(bytes);
        if reader.fixed::<8>()? != WORKER_V3_VERIFICATION_RESPONSE_MAGIC_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::BadResponseMagic);
        }
        let version = reader.u16()?;
        if version != WORKER_V3_VERIFICATION_REQUEST_VERSION_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::UnsupportedVersion {
                actual: version,
            });
        }
        let flags = reader.u16()?;
        if flags != 0 {
            return Err(WorkerV3VerificationProtocolErrorV1::UnsupportedFlags { actual: flags });
        }
        let declared_len = reader.u64()?;
        if declared_len != WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1 as u64 {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidTotalLength {
                declared: declared_len,
                actual: bytes.len(),
            });
        }
        let disposition_tag = reader.u16()?;
        let disposition = WorkerV3VerificationResponseDispositionV1::from_wire_tag(disposition_tag)
            .ok_or(
                WorkerV3VerificationProtocolErrorV1::UnknownResponseDisposition {
                    actual: disposition_tag,
                },
            )?;
        if reader.u16()? != 0 {
            return Err(WorkerV3VerificationProtocolErrorV1::NoncanonicalReservedBytes);
        }
        let entry_count = reader.u32()?;
        if entry_count == 0 || entry_count as usize > MAX_WORKER_V3_VERIFICATION_ENTRIES_V1 {
            return Err(WorkerV3VerificationProtocolErrorV1::EntryCountOutOfRange {
                actual: entry_count as usize,
                maximum: MAX_WORKER_V3_VERIFICATION_ENTRIES_V1,
            });
        }
        if reader.u32()? != 0 {
            return Err(WorkerV3VerificationProtocolErrorV1::NoncanonicalReservedBytes);
        }
        let fields = ResponseFields {
            disposition,
            entry_count,
            request_identity: WorkerV3VerificationRequestIdentityV1(reader.fixed()?),
            challenge: WorkerV3VerificationFreshChallengeV1::new(reader.fixed()?)?,
            roster_identity: WorkerV3VerificationRosterIdentityV1::new(reader.fixed()?)?,
            policy_identity: WorkerV3VerificationPolicyIdentityV1::new(reader.fixed()?)?,
            measurement_identity: WorkerV3VerificationMeasurementIdentityV1::new(reader.fixed()?)?,
            transcript_identity: WorkerV3VerificationTranscriptIdentityV1::new(reader.fixed()?)?,
        };
        let declared_identity = WorkerV3VerificationResponseIdentityV1(reader.fixed()?);
        if !reader.is_empty() {
            return Err(WorkerV3VerificationProtocolErrorV1::TrailingBytes);
        }
        let decoded = Self::encode(fields);
        if decoded.identity != declared_identity || decoded.canonical_bytes.as_slice() != bytes {
            return Err(WorkerV3VerificationProtocolErrorV1::ResponseIdentityMismatch);
        }
        Ok(decoded)
    }

    fn encode(fields: ResponseFields) -> Self {
        let mut canonical_bytes = [0_u8; WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1];
        let mut offset = 0;
        put(
            &mut canonical_bytes,
            &mut offset,
            &WORKER_V3_VERIFICATION_RESPONSE_MAGIC_V1,
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &WORKER_V3_VERIFICATION_REQUEST_VERSION_V1.to_le_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &0_u16.to_le_bytes());
        put(
            &mut canonical_bytes,
            &mut offset,
            &(WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1 as u64).to_le_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.disposition.wire_tag().to_le_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &0_u16.to_le_bytes());
        put(
            &mut canonical_bytes,
            &mut offset,
            &fields.entry_count.to_le_bytes(),
        );
        put(&mut canonical_bytes, &mut offset, &0_u32.to_le_bytes());
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.request_identity.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.challenge.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.roster_identity.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.policy_identity.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.measurement_identity.as_bytes(),
        );
        put(
            &mut canonical_bytes,
            &mut offset,
            fields.transcript_identity.as_bytes(),
        );
        debug_assert_eq!(offset, RESPONSE_PREIMAGE_BYTES);
        let identity = WorkerV3VerificationResponseIdentityV1(derive_identity(
            RESPONSE_IDENTITY_DOMAIN,
            &canonical_bytes[..offset],
        ));
        put(&mut canonical_bytes, &mut offset, identity.as_bytes());
        debug_assert_eq!(offset, canonical_bytes.len());
        Self {
            disposition: fields.disposition,
            entry_count: fields.entry_count,
            request_identity: fields.request_identity,
            challenge: fields.challenge,
            roster_identity: fields.roster_identity,
            policy_identity: fields.policy_identity,
            measurement_identity: fields.measurement_identity,
            transcript_identity: fields.transcript_identity,
            identity,
            canonical_bytes,
        }
    }

    /// Returns the complete canonical response encoding.
    pub const fn encode_canonical(&self) -> &[u8; WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1] {
        &self.canonical_bytes
    }

    /// Returns the framing disposition, never a theorem result.
    pub const fn disposition(&self) -> WorkerV3VerificationResponseDispositionV1 {
        self.disposition
    }

    /// Returns the roster entry count copied from the exact request.
    pub const fn entry_count(&self) -> u32 {
        self.entry_count
    }

    /// Returns the exact request-frame identity.
    pub const fn request_identity(&self) -> WorkerV3VerificationRequestIdentityV1 {
        self.request_identity
    }

    /// Returns the caller challenge copied from the exact request.
    pub const fn challenge(&self) -> WorkerV3VerificationFreshChallengeV1 {
        self.challenge
    }

    /// Returns the roster identity copied from the exact request.
    pub const fn roster_identity(&self) -> WorkerV3VerificationRosterIdentityV1 {
        self.roster_identity
    }

    /// Returns the policy identity copied from the exact request.
    pub const fn policy_identity(&self) -> WorkerV3VerificationPolicyIdentityV1 {
        self.policy_identity
    }

    /// Returns the expected measurement identity copied from the exact request.
    pub const fn measurement_identity(&self) -> WorkerV3VerificationMeasurementIdentityV1 {
        self.measurement_identity
    }

    /// Returns the authority-free service transcript identity.
    pub const fn transcript_identity(&self) -> WorkerV3VerificationTranscriptIdentityV1 {
        self.transcript_identity
    }

    /// Returns the exact response-frame identity.
    pub const fn identity(&self) -> WorkerV3VerificationResponseIdentityV1 {
        self.identity
    }

    /// Checks every copied request coordinate and the exact request identity.
    pub fn matches_request(&self, request: &WorkerV3VerificationRequestV1) -> bool {
        self.request_identity == request.identity
            && self.entry_count as usize == request.entries.len()
            && self.challenge == request.challenge
            && self.roster_identity == request.roster_identity
            && self.policy_identity == request.policy_identity
            && self.measurement_identity == request.measurement_identity
    }

    /// Reports that this inert frame grants no theorem, load, or launch authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }

    /// Reports that canonical decoding does not authenticate the service peer.
    pub const fn authenticates_service(&self) -> bool {
        false
    }
}

#[derive(Clone, Copy)]
struct ResponseFields {
    disposition: WorkerV3VerificationResponseDispositionV1,
    entry_count: u32,
    request_identity: WorkerV3VerificationRequestIdentityV1,
    challenge: WorkerV3VerificationFreshChallengeV1,
    roster_identity: WorkerV3VerificationRosterIdentityV1,
    policy_identity: WorkerV3VerificationPolicyIdentityV1,
    measurement_identity: WorkerV3VerificationMeasurementIdentityV1,
    transcript_identity: WorkerV3VerificationTranscriptIdentityV1,
}

/// Construction or strict canonical-decoding failure.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WorkerV3VerificationProtocolErrorV1 {
    /// A caller-owned identity used the forbidden all-zero sentinel.
    ZeroIdentity(WorkerV3VerificationIdentityFieldV1),
    /// One roster entry identity used the forbidden all-zero sentinel.
    ZeroEntryIdentity {
        /// Entry ordinal.
        ordinal: u32,
        /// Rejected field.
        field: &'static str,
    },
    /// One roster entry name violated kernel-descriptor `ValidName` semantics.
    InvalidEntryName {
        /// Entry ordinal.
        ordinal: u32,
        /// Rejected name field.
        field: WorkerV3VerificationEntryNameFieldV1,
        /// Exact kernel-descriptor validation failure.
        source: ValidationError,
    },
    /// Request entry count is outside the nonempty protocol bound.
    EntryCountOutOfRange {
        /// Observed count.
        actual: usize,
        /// Protocol maximum.
        maximum: usize,
    },
    /// One entry did not occur at its declared canonical ordinal.
    UnexpectedEntryOrdinal {
        /// Ordinal required at this wire position.
        expected: u32,
        /// Ordinal encoded at this wire position.
        actual: u32,
    },
    /// Two entries reused an identity that must be unique within one roster.
    DuplicateEntryIdentity {
        /// Duplicated field.
        field: WorkerV3VerificationEntryIdentityFieldV1,
        /// First ordinal carrying the identity.
        first_ordinal: u32,
        /// Later ordinal carrying the identity.
        duplicate_ordinal: u32,
    },
    /// A payload appeared at the wrong position in the fixed descriptor array.
    UnexpectedPayloadKind {
        /// Required payload kind.
        expected: WorkerV3VerificationFdPayloadKindV1,
        /// Observed payload kind.
        actual: WorkerV3VerificationFdPayloadKindV1,
    },
    /// A descriptor named a noncanonical `SCM_RIGHTS` position.
    InvalidFdOrdinal {
        /// Payload kind.
        kind: WorkerV3VerificationFdPayloadKindV1,
        /// Observed ordinal.
        actual: u32,
    },
    /// A payload descriptor length was zero or exceeded its protocol bound.
    PayloadLengthOutOfRange {
        /// Payload kind.
        kind: WorkerV3VerificationFdPayloadKindV1,
        /// Observed length.
        actual: u64,
        /// Protocol maximum.
        maximum: u64,
    },
    /// A payload descriptor used the forbidden all-zero digest.
    ZeroPayloadDigest {
        /// Payload kind.
        kind: WorkerV3VerificationFdPayloadKindV1,
    },
    /// Request wire length is outside the protocol bounds.
    RequestLengthOutOfRange {
        /// Observed length.
        actual: usize,
        /// Smallest canonical request.
        minimum: usize,
        /// Largest canonical request.
        maximum: usize,
    },
    /// Response wire length differs from the fixed schema length.
    InvalidResponseLength {
        /// Observed length.
        actual: usize,
    },
    /// Request magic mismatch.
    BadRequestMagic,
    /// Response magic mismatch.
    BadResponseMagic,
    /// Unsupported schema version.
    UnsupportedVersion {
        /// Observed version.
        actual: u16,
    },
    /// Unsupported top-level flags.
    UnsupportedFlags {
        /// Observed flags.
        actual: u16,
    },
    /// Unsupported per-payload flags.
    UnsupportedPayloadFlags {
        /// Observed flags.
        actual: u16,
    },
    /// Declared total length differs from the complete received frame.
    InvalidTotalLength {
        /// Declared length.
        declared: u64,
        /// Received length.
        actual: usize,
    },
    /// Request did not declare exactly two fd payloads.
    InvalidPayloadCount {
        /// Observed count.
        actual: usize,
    },
    /// Payload kind tag is unknown.
    UnknownPayloadKind {
        /// Observed tag.
        actual: u16,
    },
    /// Response disposition tag is unknown.
    UnknownResponseDisposition {
        /// Observed tag.
        actual: u16,
    },
    /// Reserved bytes were not canonical zeroes.
    NoncanonicalReservedBytes,
    /// Entry count and complete wire length disagree.
    InvalidEntrySectionLength {
        /// Declared entry count.
        entry_count: usize,
        /// Received total length.
        actual: usize,
        /// Minimum length implied by the declared entry count.
        minimum: usize,
        /// Maximum length implied by the declared entry count.
        maximum: usize,
    },
    /// Arithmetic overflow while computing a bounded frame length.
    LengthOverflow,
    /// Bounded allocation failed.
    AllocationFailed {
        /// Number of bytes or elements requested.
        requested: usize,
    },
    /// Frame ended before a fixed-width field was complete.
    Truncated,
    /// Complete frame contained unconsumed trailing bytes.
    TrailingBytes,
    /// Declared request identity or exact canonical reconstruction mismatched.
    RequestIdentityMismatch,
    /// Declared response identity or exact canonical reconstruction mismatched.
    ResponseIdentityMismatch,
}

impl fmt::Display for WorkerV3VerificationProtocolErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroIdentity(field) => write!(formatter, "{field:?} identity is zero"),
            Self::ZeroEntryIdentity { ordinal, field } => {
                write!(formatter, "roster entry {ordinal} {field} identity is zero")
            }
            Self::InvalidEntryName {
                ordinal,
                field,
                source,
            } => write!(
                formatter,
                "roster entry {ordinal} {field:?} is not a canonical kernel name: {source}"
            ),
            Self::EntryCountOutOfRange { actual, maximum } => write!(
                formatter,
                "roster entry count {actual} is outside 1..={maximum}"
            ),
            Self::UnexpectedEntryOrdinal { expected, actual } => write!(
                formatter,
                "roster entry ordinal {actual} occurs at canonical position {expected}"
            ),
            Self::DuplicateEntryIdentity {
                field,
                first_ordinal,
                duplicate_ordinal,
            } => write!(
                formatter,
                "roster entry {duplicate_ordinal} duplicates {field:?} identity from entry {first_ordinal}"
            ),
            Self::UnexpectedPayloadKind { expected, actual } => {
                write!(
                    formatter,
                    "expected {expected:?} fd payload, got {actual:?}"
                )
            }
            Self::InvalidFdOrdinal { kind, actual } => {
                write!(
                    formatter,
                    "{kind:?} payload used invalid fd ordinal {actual}"
                )
            }
            Self::PayloadLengthOutOfRange {
                kind,
                actual,
                maximum,
            } => write!(
                formatter,
                "{kind:?} payload length {actual} is outside 1..={maximum} bytes"
            ),
            Self::ZeroPayloadDigest { kind } => {
                write!(formatter, "{kind:?} payload digest is zero")
            }
            Self::RequestLengthOutOfRange {
                actual,
                minimum,
                maximum,
            } => write!(
                formatter,
                "request length {actual} is outside {minimum}..={maximum} bytes"
            ),
            Self::InvalidResponseLength { actual } => write!(
                formatter,
                "response length must be {WORKER_V3_VERIFICATION_RESPONSE_BYTES_V1} bytes, got {actual}"
            ),
            Self::BadRequestMagic => {
                formatter.write_str("Worker V3 verification request magic mismatch")
            }
            Self::BadResponseMagic => {
                formatter.write_str("Worker V3 verification response magic mismatch")
            }
            Self::UnsupportedVersion { actual } => {
                write!(formatter, "unsupported version {actual}")
            }
            Self::UnsupportedFlags { actual } => {
                write!(formatter, "unsupported flags {actual:#06x}")
            }
            Self::UnsupportedPayloadFlags { actual } => {
                write!(formatter, "unsupported fd payload flags {actual:#06x}")
            }
            Self::InvalidTotalLength { declared, actual } => write!(
                formatter,
                "declared frame length {declared} differs from received length {actual}"
            ),
            Self::InvalidPayloadCount { actual } => write!(
                formatter,
                "request must bind exactly {WORKER_V3_VERIFICATION_FD_PAYLOADS_V1} fd payloads, got {actual}"
            ),
            Self::UnknownPayloadKind { actual } => {
                write!(formatter, "unknown fd payload kind {actual}")
            }
            Self::UnknownResponseDisposition { actual } => {
                write!(formatter, "unknown response disposition {actual}")
            }
            Self::NoncanonicalReservedBytes => formatter.write_str("reserved bytes are nonzero"),
            Self::InvalidEntrySectionLength {
                entry_count,
                actual,
                minimum,
                maximum,
            } => write!(
                formatter,
                "entry count {entry_count} requires {minimum}..={maximum} request bytes, got {actual}"
            ),
            Self::LengthOverflow => formatter.write_str("frame length overflow"),
            Self::AllocationFailed { requested } => {
                write!(formatter, "failed to allocate bounded capacity {requested}")
            }
            Self::Truncated => formatter.write_str("truncated frame"),
            Self::TrailingBytes => formatter.write_str("frame has trailing bytes"),
            Self::RequestIdentityMismatch => formatter.write_str("request identity mismatch"),
            Self::ResponseIdentityMismatch => formatter.write_str("response identity mismatch"),
        }
    }
}

impl Error for WorkerV3VerificationProtocolErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InvalidEntryName { source, .. } => Some(source),
            _ => None,
        }
    }
}

fn validate_payload_order(
    payloads: &[WorkerV3VerificationFdPayloadDescriptorV1; WORKER_V3_VERIFICATION_FD_PAYLOADS_V1],
) -> Result<(), WorkerV3VerificationProtocolErrorV1> {
    for (actual, expected) in payloads.iter().zip([
        WorkerV3VerificationFdPayloadKindV1::LoadEnvelopeV2,
        WorkerV3VerificationFdPayloadKindV1::FinalizedHsaco,
    ]) {
        if actual.kind != expected {
            return Err(WorkerV3VerificationProtocolErrorV1::UnexpectedPayloadKind {
                expected,
                actual: actual.kind,
            });
        }
    }
    Ok(())
}

fn validate_entries(
    entries: &[WorkerV3VerificationEntryCoordinateV1],
) -> Result<(), WorkerV3VerificationProtocolErrorV1> {
    if entries.is_empty() || entries.len() > MAX_WORKER_V3_VERIFICATION_ENTRIES_V1 {
        return Err(WorkerV3VerificationProtocolErrorV1::EntryCountOutOfRange {
            actual: entries.len(),
            maximum: MAX_WORKER_V3_VERIFICATION_ENTRIES_V1,
        });
    }
    for (position, entry) in entries.iter().enumerate() {
        let expected = position as u32;
        if entry.ordinal != expected {
            return Err(
                WorkerV3VerificationProtocolErrorV1::UnexpectedEntryOrdinal {
                    expected,
                    actual: entry.ordinal,
                },
            );
        }
        for (first_position, first) in entries[..position].iter().enumerate() {
            let duplicate = if entry.logical_name == first.logical_name {
                Some(WorkerV3VerificationEntryIdentityFieldV1::LogicalName)
            } else if entry.export_name == first.export_name {
                Some(WorkerV3VerificationEntryIdentityFieldV1::ExportName)
            } else if entry.lineage_identity == first.lineage_identity {
                Some(WorkerV3VerificationEntryIdentityFieldV1::Lineage)
            } else if entry.marker_binding_identity == first.marker_binding_identity {
                Some(WorkerV3VerificationEntryIdentityFieldV1::MarkerBinding)
            } else {
                None
            };
            if let Some(field) = duplicate {
                return Err(
                    WorkerV3VerificationProtocolErrorV1::DuplicateEntryIdentity {
                        field,
                        first_ordinal: first_position as u32,
                        duplicate_ordinal: entry.ordinal,
                    },
                );
            }
        }
    }
    Ok(())
}

fn derive_identity(domain: &[u8], bytes: &[u8]) -> [u8; SHA256_BYTES] {
    let mut digest = Sha256::new();
    digest.update(domain);
    digest.update((bytes.len() as u64).to_le_bytes());
    digest.update(bytes);
    digest.finalize().into()
}

fn put<const N: usize>(output: &mut [u8], offset: &mut usize, bytes: &[u8; N]) {
    output[*offset..*offset + N].copy_from_slice(bytes);
    *offset += N;
}

struct Reader<'bytes> {
    bytes: &'bytes [u8],
    offset: usize,
}

impl<'bytes> Reader<'bytes> {
    const fn new(bytes: &'bytes [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'bytes [u8], WorkerV3VerificationProtocolErrorV1> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(WorkerV3VerificationProtocolErrorV1::LengthOverflow)?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or(WorkerV3VerificationProtocolErrorV1::Truncated)?;
        self.offset = end;
        Ok(bytes)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], WorkerV3VerificationProtocolErrorV1> {
        self.take(N)?
            .try_into()
            .map_err(|_| WorkerV3VerificationProtocolErrorV1::Truncated)
    }

    fn u16(&mut self) -> Result<u16, WorkerV3VerificationProtocolErrorV1> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, WorkerV3VerificationProtocolErrorV1> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, WorkerV3VerificationProtocolErrorV1> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn entry_name_len(
        &mut self,
        ordinal: u32,
        field: WorkerV3VerificationEntryNameFieldV1,
    ) -> Result<usize, WorkerV3VerificationProtocolErrorV1> {
        let length = usize::from(self.u16()?);
        if length > MAX_NAME_BYTES {
            return Err(WorkerV3VerificationProtocolErrorV1::InvalidEntryName {
                ordinal,
                field,
                source: ValidationError::TooLong {
                    field: "name",
                    max: MAX_NAME_BYTES,
                },
            });
        }
        Ok(length)
    }

    fn entry_name(
        &mut self,
        ordinal: u32,
        field: WorkerV3VerificationEntryNameFieldV1,
        length: usize,
    ) -> Result<&'bytes str, WorkerV3VerificationProtocolErrorV1> {
        str::from_utf8(self.take(length)?).map_err(|_| {
            WorkerV3VerificationProtocolErrorV1::InvalidEntryName {
                ordinal,
                field,
                source: ValidationError::InvalidText { field: "name" },
            }
        })
    }

    const fn is_empty(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REQUEST_ENTRIES_OFFSET: usize = REQUEST_HEADER_BYTES
        + (4 * SHA256_BYTES)
        + (WORKER_V3_VERIFICATION_FD_PAYLOADS_V1 * FD_PAYLOAD_DESCRIPTOR_BYTES);

    fn id(byte: u8) -> [u8; SHA256_BYTES] {
        [byte; SHA256_BYTES]
    }

    fn entry(ordinal: u32) -> WorkerV3VerificationEntryCoordinateV1 {
        let byte = ordinal as u8 + 16;
        WorkerV3VerificationEntryCoordinateV1::new(
            ordinal,
            format!("logical_{ordinal}"),
            format!("export_{ordinal}"),
            id(byte),
            id(byte.wrapping_add(32)),
            id(byte.wrapping_add(64)),
        )
        .unwrap()
    }

    fn request() -> WorkerV3VerificationRequestV1 {
        WorkerV3VerificationRequestV1::new(
            WorkerV3VerificationFreshChallengeV1::new(id(1)).unwrap(),
            WorkerV3VerificationRosterIdentityV1::new(id(2)).unwrap(),
            WorkerV3VerificationPolicyIdentityV1::new(id(3)).unwrap(),
            WorkerV3VerificationMeasurementIdentityV1::new(id(4)).unwrap(),
            WorkerV3VerificationFdPayloadDescriptorV1::load_envelope_v2(8_192, id(5)).unwrap(),
            WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(4_096, id(6)).unwrap(),
            vec![entry(0), entry(1), entry(2)],
        )
        .unwrap()
    }

    fn refresh_request_identity(bytes: &mut [u8]) {
        let identity_offset = bytes.len() - SHA256_BYTES;
        let identity = derive_identity(REQUEST_IDENTITY_DOMAIN, &bytes[..identity_offset]);
        bytes[identity_offset..].copy_from_slice(&identity);
    }

    #[test]
    fn request_and_response_round_trip_without_authority() {
        let request = request();
        let decoded =
            WorkerV3VerificationRequestV1::decode_canonical(request.encode_canonical()).unwrap();
        assert_eq!(decoded, request);
        assert!(
            request
                .identity()
                .matches_canonical_bytes(request.encode_canonical())
        );
        assert!(!decoded.grants_authority());
        assert_eq!(decoded.payloads()[0].fd_ordinal(), 0);
        assert_eq!(decoded.payloads()[1].fd_ordinal(), 1);
        assert_eq!(decoded.entries()[0].logical_name(), "logical_0");
        assert_eq!(decoded.entries()[0].export_name(), "export_0");
        assert!(decoded.challenge().freshness_must_be_enforced_externally());

        let response = WorkerV3VerificationResponseV1::new(
            &request,
            WorkerV3VerificationResponseDispositionV1::RequestFramed,
            WorkerV3VerificationTranscriptIdentityV1::new(id(7)).unwrap(),
        );
        let decoded_response =
            WorkerV3VerificationResponseV1::decode_canonical(response.encode_canonical()).unwrap();
        assert_eq!(decoded_response, response);
        assert!(decoded_response.matches_request(&request));
        assert!(
            response
                .identity()
                .matches_canonical_bytes(response.encode_canonical())
        );
        assert!(!decoded_response.grants_authority());
        assert!(!decoded_response.authenticates_service());
    }

    #[test]
    fn every_single_byte_request_and_response_mutation_is_rejected() {
        let request = request();
        for offset in 0..request.encode_canonical().len() {
            let mut hostile = request.encode_canonical().to_vec();
            hostile[offset] ^= 0x80;
            assert!(
                WorkerV3VerificationRequestV1::decode_canonical(&hostile).is_err(),
                "request mutation at offset {offset} decoded"
            );
        }

        let response = WorkerV3VerificationResponseV1::new(
            &request,
            WorkerV3VerificationResponseDispositionV1::RequestRejected,
            WorkerV3VerificationTranscriptIdentityV1::new(id(8)).unwrap(),
        );
        for offset in 0..response.encode_canonical().len() {
            let mut hostile = response.encode_canonical().to_vec();
            hostile[offset] ^= 0x40;
            assert!(
                WorkerV3VerificationResponseV1::decode_canonical(&hostile).is_err(),
                "response mutation at offset {offset} decoded"
            );
        }
    }

    #[test]
    fn recomputed_identity_cannot_hide_duplicate_or_reordered_entries() {
        let request = request();
        let first_entry_len = request.entries()[0].encoded_len();
        let second_entry_offset = REQUEST_ENTRIES_OFFSET + first_entry_len;
        let mut duplicate = request.encode_canonical().to_vec();
        let first_lineage = duplicate
            [REQUEST_ENTRIES_OFFSET + 12..REQUEST_ENTRIES_OFFSET + 12 + SHA256_BYTES]
            .to_vec();
        duplicate[second_entry_offset + 12..second_entry_offset + 12 + SHA256_BYTES]
            .copy_from_slice(&first_lineage);
        refresh_request_identity(&mut duplicate);
        assert!(matches!(
            WorkerV3VerificationRequestV1::decode_canonical(&duplicate),
            Err(WorkerV3VerificationProtocolErrorV1::DuplicateEntryIdentity { .. })
        ));

        let mut reordered = request.encode_canonical().to_vec();
        let first = reordered[REQUEST_ENTRIES_OFFSET..second_entry_offset].to_vec();
        let second = reordered[second_entry_offset..second_entry_offset + first_entry_len].to_vec();
        reordered[REQUEST_ENTRIES_OFFSET..second_entry_offset].copy_from_slice(&second);
        reordered[second_entry_offset..second_entry_offset + first_entry_len]
            .copy_from_slice(&first);
        refresh_request_identity(&mut reordered);
        assert!(matches!(
            WorkerV3VerificationRequestV1::decode_canonical(&reordered),
            Err(
                WorkerV3VerificationProtocolErrorV1::UnexpectedEntryOrdinal {
                    expected: 0,
                    actual: 1
                }
            )
        ));
    }

    #[test]
    fn duplicate_and_oversized_payload_descriptors_are_rejected() {
        let request = request();
        let second_payload_offset =
            REQUEST_HEADER_BYTES + (4 * SHA256_BYTES) + FD_PAYLOAD_DESCRIPTOR_BYTES;
        let mut duplicate = request.encode_canonical().to_vec();
        duplicate[second_payload_offset..second_payload_offset + 2]
            .copy_from_slice(&1_u16.to_le_bytes());
        duplicate[second_payload_offset + 4..second_payload_offset + 8]
            .copy_from_slice(&0_u32.to_le_bytes());
        refresh_request_identity(&mut duplicate);
        assert!(matches!(
            WorkerV3VerificationRequestV1::decode_canonical(&duplicate),
            Err(WorkerV3VerificationProtocolErrorV1::UnexpectedPayloadKind { .. })
        ));

        let mut oversized = request.encode_canonical().to_vec();
        oversized[second_payload_offset + 8..second_payload_offset + 16]
            .copy_from_slice(&(MAX_WORKER_V3_VERIFICATION_HSACO_FD_BYTES_V1 + 1).to_le_bytes());
        refresh_request_identity(&mut oversized);
        assert!(matches!(
            WorkerV3VerificationRequestV1::decode_canonical(&oversized),
            Err(WorkerV3VerificationProtocolErrorV1::PayloadLengthOutOfRange { .. })
        ));
    }

    #[test]
    fn oversized_rosters_and_noncanonical_lengths_are_rejected_before_allocation() {
        let entries: Vec<_> = (0..=MAX_WORKER_V3_VERIFICATION_ENTRIES_V1 as u32)
            .map(entry)
            .collect();
        assert!(matches!(
            WorkerV3VerificationRequestV1::new(
                WorkerV3VerificationFreshChallengeV1::new(id(1)).unwrap(),
                WorkerV3VerificationRosterIdentityV1::new(id(2)).unwrap(),
                WorkerV3VerificationPolicyIdentityV1::new(id(3)).unwrap(),
                WorkerV3VerificationMeasurementIdentityV1::new(id(4)).unwrap(),
                WorkerV3VerificationFdPayloadDescriptorV1::load_envelope_v2(1, id(5)).unwrap(),
                WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(1, id(6)).unwrap(),
                entries,
            ),
            Err(WorkerV3VerificationProtocolErrorV1::EntryCountOutOfRange { .. })
        ));

        let mut oversized_count = request().encode_canonical().to_vec();
        oversized_count[20..24]
            .copy_from_slice(&((MAX_WORKER_V3_VERIFICATION_ENTRIES_V1 + 1) as u32).to_le_bytes());
        refresh_request_identity(&mut oversized_count);
        assert!(matches!(
            WorkerV3VerificationRequestV1::decode_canonical(&oversized_count),
            Err(WorkerV3VerificationProtocolErrorV1::EntryCountOutOfRange { .. })
        ));

        let too_long = vec![0; MAX_WORKER_V3_VERIFICATION_REQUEST_BYTES_V1 + 1];
        assert!(matches!(
            WorkerV3VerificationRequestV1::decode_canonical(&too_long),
            Err(WorkerV3VerificationProtocolErrorV1::RequestLengthOutOfRange { .. })
        ));

        assert!(matches!(
            WorkerV3VerificationRequestV1::new(
                WorkerV3VerificationFreshChallengeV1::new(id(1)).unwrap(),
                WorkerV3VerificationRosterIdentityV1::new(id(2)).unwrap(),
                WorkerV3VerificationPolicyIdentityV1::new(id(3)).unwrap(),
                WorkerV3VerificationMeasurementIdentityV1::new(id(4)).unwrap(),
                WorkerV3VerificationFdPayloadDescriptorV1::load_envelope_v2(1, id(5)).unwrap(),
                WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(1, id(6)).unwrap(),
                Vec::new(),
            ),
            Err(WorkerV3VerificationProtocolErrorV1::EntryCountOutOfRange { actual: 0, .. })
        ));
    }

    #[test]
    fn kernel_names_use_exact_descriptor_validation_and_wire_bounds() {
        for invalid in [
            String::new(),
            "has\0nul".to_string(),
            "has space".to_string(),
            "x".repeat(MAX_WORKER_V3_VERIFICATION_ENTRY_NAME_BYTES_V1 + 1),
        ] {
            assert!(matches!(
                WorkerV3VerificationEntryCoordinateV1::new(
                    0,
                    invalid,
                    "valid_export",
                    id(1),
                    id(2),
                    id(3),
                ),
                Err(WorkerV3VerificationProtocolErrorV1::InvalidEntryName {
                    field: WorkerV3VerificationEntryNameFieldV1::LogicalName,
                    ..
                })
            ));
        }

        let request = request();
        let mut oversized_name = request.encode_canonical().to_vec();
        oversized_name[REQUEST_ENTRIES_OFFSET + 4..REQUEST_ENTRIES_OFFSET + 6].copy_from_slice(
            &((MAX_WORKER_V3_VERIFICATION_ENTRY_NAME_BYTES_V1 + 1) as u16).to_le_bytes(),
        );
        refresh_request_identity(&mut oversized_name);
        assert!(matches!(
            WorkerV3VerificationRequestV1::decode_canonical(&oversized_name),
            Err(WorkerV3VerificationProtocolErrorV1::InvalidEntryName { .. })
        ));

        let mut nul_name = request.encode_canonical().to_vec();
        nul_name[REQUEST_ENTRIES_OFFSET + ENTRY_COORDINATE_FIXED_BYTES] = 0;
        refresh_request_identity(&mut nul_name);
        assert!(matches!(
            WorkerV3VerificationRequestV1::decode_canonical(&nul_name),
            Err(WorkerV3VerificationProtocolErrorV1::InvalidEntryName { .. })
        ));
    }

    #[test]
    fn duplicate_names_are_rejected_but_shared_generated_contracts_are_allowed() {
        let duplicate_name = vec![
            WorkerV3VerificationEntryCoordinateV1::new(
                0,
                "same",
                "export_0",
                id(10),
                id(20),
                id(30),
            )
            .unwrap(),
            WorkerV3VerificationEntryCoordinateV1::new(
                1,
                "same",
                "export_1",
                id(11),
                id(21),
                id(31),
            )
            .unwrap(),
        ];
        assert!(matches!(
            validate_entries(&duplicate_name),
            Err(
                WorkerV3VerificationProtocolErrorV1::DuplicateEntryIdentity {
                    field: WorkerV3VerificationEntryIdentityFieldV1::LogicalName,
                    ..
                }
            )
        ));

        let shared_contract = id(30);
        let entries = vec![
            WorkerV3VerificationEntryCoordinateV1::new(
                0,
                "logical_0",
                "export_0",
                id(10),
                id(20),
                shared_contract,
            )
            .unwrap(),
            WorkerV3VerificationEntryCoordinateV1::new(
                1,
                "logical_1",
                "export_1",
                id(11),
                id(21),
                shared_contract,
            )
            .unwrap(),
        ];
        assert!(validate_entries(&entries).is_ok());
    }

    #[test]
    fn zero_identities_digests_and_payload_lengths_are_rejected() {
        assert!(matches!(
            WorkerV3VerificationFreshChallengeV1::new([0; SHA256_BYTES]),
            Err(WorkerV3VerificationProtocolErrorV1::ZeroIdentity(
                WorkerV3VerificationIdentityFieldV1::Challenge
            ))
        ));
        assert!(matches!(
            WorkerV3VerificationEntryCoordinateV1::new(
                0,
                "logical",
                "export",
                [0; SHA256_BYTES],
                id(1),
                id(2),
            ),
            Err(WorkerV3VerificationProtocolErrorV1::ZeroEntryIdentity { .. })
        ));
        assert!(matches!(
            WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(0, id(1)),
            Err(WorkerV3VerificationProtocolErrorV1::PayloadLengthOutOfRange { .. })
        ));
        assert!(matches!(
            WorkerV3VerificationFdPayloadDescriptorV1::finalized_hsaco(1, [0; SHA256_BYTES]),
            Err(WorkerV3VerificationProtocolErrorV1::ZeroPayloadDigest { .. })
        ));
    }
}
