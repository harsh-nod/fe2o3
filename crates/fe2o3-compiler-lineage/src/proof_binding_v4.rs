use std::{error::Error, fmt, ops::Range};

use crate::InertLineageContentIdentityV3;

/// Magic of the current proof-binding association that retains signed Verus evidence.
pub const INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V4: [u8; 8] = *b"F2O3TLV4";
/// Version of the current signed-evidence proof-binding association.
pub const INERT_PROOF_BINDING_ASSOCIATION_VERSION_V4: u16 = 4;
/// Maximum canonical size accepted for a V4 proof-binding association.
pub const MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V4: usize = 4 * 1024 * 1024;
/// Maximum nested canonical aggregate Verus-execution evidence.
pub const MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4: usize = 64 * 1024;

const RECORD_KIND_V4: u16 = 6;
const SIGNED_VERUS_EVIDENCE_POLICY_V4: u16 = 2;
const HEADER_BYTES_V4: usize = 24;
const FIELD_HEADER_BYTES_V4: usize = 8;
const FIELD_COUNT_V4: usize = 8;
const IDENTITY_BYTES_V4: usize = 40;
const DOMAIN_V4: &[u8] = b"FE2O3/PRODUCTION-PROOF-BINDING-ASSOCIATION/V4\0";
const CLAIM_V4: &[u8] = b"exact-signed-mir-pliron-verus-receipt/no-llvm-or-later-refinement-proof";

/// The five exact compiler stages associated with one nested signed Verus-execution record.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertProofBindingAssociationInputsV4 {
    semantic_mir: InertLineageContentIdentityV3,
    middle_end: InertLineageContentIdentityV3,
    kernel_ir: InertLineageContentIdentityV3,
    mir_to_kir_correspondence: InertLineageContentIdentityV3,
    formal_memory: InertLineageContentIdentityV3,
}

impl InertProofBindingAssociationInputsV4 {
    /// Constructs the exact ordered compiler-stage identities.
    pub const fn new(
        semantic_mir: InertLineageContentIdentityV3,
        middle_end: InertLineageContentIdentityV3,
        kernel_ir: InertLineageContentIdentityV3,
        mir_to_kir_correspondence: InertLineageContentIdentityV3,
        formal_memory: InertLineageContentIdentityV3,
    ) -> Self {
        Self {
            semantic_mir,
            middle_end,
            kernel_ir,
            mir_to_kir_correspondence,
            formal_memory,
        }
    }

    /// Returns the semantic-MIR receipt identity.
    pub const fn semantic_mir(self) -> InertLineageContentIdentityV3 {
        self.semantic_mir
    }

    /// Returns the middle-end receipt identity.
    pub const fn middle_end(self) -> InertLineageContentIdentityV3 {
        self.middle_end
    }

    /// Returns the canonical Kernel IR receipt identity.
    pub const fn kernel_ir(self) -> InertLineageContentIdentityV3 {
        self.kernel_ir
    }

    /// Returns the MIR-to-KIR correspondence receipt identity.
    pub const fn mir_to_kir_correspondence(self) -> InertLineageContentIdentityV3 {
        self.mir_to_kir_correspondence
    }

    /// Returns the formal-memory receipt identity.
    pub const fn formal_memory(self) -> InertLineageContentIdentityV3 {
        self.formal_memory
    }

    const fn ordered(self) -> [InertLineageContentIdentityV3; 5] {
        [
            self.semantic_mir,
            self.middle_end,
            self.kernel_ir,
            self.mir_to_kir_correspondence,
            self.formal_memory,
        ]
    }
}

/// Canonical V4 association of exact compiler inputs and one exact signed Verus record.
///
/// This envelope preserves bytes and relationships only. The nested verifier codec must import
/// the signature, and protected compiler custody must authenticate producer origin. This value
/// never claims LLVM or machine refinement.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProofBindingAssociationV4 {
    canonical_bytes: Box<[u8]>,
    inputs: InertProofBindingAssociationInputsV4,
    verus_evidence_range: Range<usize>,
}

impl InertProofBindingAssociationV4 {
    /// Canonically associates five compiler stages with exact nested Verus evidence.
    pub fn new(
        inputs: InertProofBindingAssociationInputsV4,
        verus_execution_evidence: &[u8],
    ) -> Result<Self, InertProofBindingAssociationErrorV4> {
        validate_verus_evidence_len(verus_execution_evidence.len())?;
        let identities = inputs.ordered().map(encode_identity);
        let fields: [&[u8]; FIELD_COUNT_V4] = [
            DOMAIN_V4,
            CLAIM_V4,
            &identities[0],
            &identities[1],
            &identities[2],
            &identities[3],
            &identities[4],
            verus_execution_evidence,
        ];
        let total_len = encoded_len(&fields)?;
        let total_len_u32 = u32::try_from(total_len)
            .map_err(|_| InertProofBindingAssociationErrorV4::LengthOverflow)?;
        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(total_len)
            .map_err(|_| InertProofBindingAssociationErrorV4::AllocationFailed)?;
        canonical.extend_from_slice(&INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V4);
        canonical.extend_from_slice(&INERT_PROOF_BINDING_ASSOCIATION_VERSION_V4.to_le_bytes());
        canonical.extend_from_slice(&RECORD_KIND_V4.to_le_bytes());
        canonical.extend_from_slice(&SIGNED_VERUS_EVIDENCE_POLICY_V4.to_le_bytes());
        canonical.extend_from_slice(&(FIELD_COUNT_V4 as u16).to_le_bytes());
        canonical.extend_from_slice(&total_len_u32.to_le_bytes());
        canonical.extend_from_slice(&0_u32.to_le_bytes());
        let mut verus_evidence_range = 0..0;
        for (index, field) in fields.iter().enumerate() {
            let tag = u16::try_from(index + 1)
                .map_err(|_| InertProofBindingAssociationErrorV4::LengthOverflow)?;
            let field_len = u32::try_from(field.len())
                .map_err(|_| InertProofBindingAssociationErrorV4::LengthOverflow)?;
            canonical.extend_from_slice(&tag.to_le_bytes());
            canonical.extend_from_slice(&0_u16.to_le_bytes());
            canonical.extend_from_slice(&field_len.to_le_bytes());
            let start = canonical.len();
            canonical.extend_from_slice(field);
            if index == FIELD_COUNT_V4 - 1 {
                verus_evidence_range = start..canonical.len();
            }
        }
        debug_assert_eq!(canonical.len(), total_len);
        Ok(Self {
            canonical_bytes: canonical.into_boxed_slice(),
            inputs,
            verus_evidence_range,
        })
    }

    /// Strictly decodes one complete V4 association without fallback.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertProofBindingAssociationErrorV4> {
        if bytes.len() > MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V4 {
            return Err(InertProofBindingAssociationErrorV4::TooLarge);
        }
        if bytes.len() < HEADER_BYTES_V4 {
            return Err(InertProofBindingAssociationErrorV4::Truncated);
        }
        if bytes[..8] != INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V4 {
            return Err(InertProofBindingAssociationErrorV4::InvalidMagic);
        }
        if read_u16(bytes, 8)? != INERT_PROOF_BINDING_ASSOCIATION_VERSION_V4 {
            return Err(InertProofBindingAssociationErrorV4::UnsupportedVersion);
        }
        if read_u16(bytes, 10)? != RECORD_KIND_V4 {
            return Err(InertProofBindingAssociationErrorV4::WrongRecordKind);
        }
        if read_u16(bytes, 12)? != SIGNED_VERUS_EVIDENCE_POLICY_V4 {
            return Err(InertProofBindingAssociationErrorV4::WrongPolicy);
        }
        if usize::from(read_u16(bytes, 14)?) != FIELD_COUNT_V4 {
            return Err(InertProofBindingAssociationErrorV4::WrongFieldCount);
        }
        let declared = usize::try_from(read_u32(bytes, 16)?)
            .map_err(|_| InertProofBindingAssociationErrorV4::LengthOverflow)?;
        if declared != bytes.len() {
            return Err(InertProofBindingAssociationErrorV4::DeclaredLengthMismatch);
        }
        if read_u32(bytes, 20)? != 0 {
            return Err(InertProofBindingAssociationErrorV4::NonzeroReserved);
        }

        let fixed_lengths = [
            DOMAIN_V4.len(),
            CLAIM_V4.len(),
            IDENTITY_BYTES_V4,
            IDENTITY_BYTES_V4,
            IDENTITY_BYTES_V4,
            IDENTITY_BYTES_V4,
            IDENTITY_BYTES_V4,
        ];
        let mut fields: [&[u8]; FIELD_COUNT_V4] = [&[]; FIELD_COUNT_V4];
        let mut offset = HEADER_BYTES_V4;
        for index in 0..FIELD_COUNT_V4 {
            let expected_tag = u16::try_from(index + 1)
                .map_err(|_| InertProofBindingAssociationErrorV4::LengthOverflow)?;
            if read_u16(bytes, offset)? != expected_tag {
                return Err(InertProofBindingAssociationErrorV4::WrongFieldTag);
            }
            if read_u16(bytes, offset + 2)? != 0 {
                return Err(InertProofBindingAssociationErrorV4::NonzeroFieldFlags);
            }
            let field_len = usize::try_from(read_u32(bytes, offset + 4)?)
                .map_err(|_| InertProofBindingAssociationErrorV4::LengthOverflow)?;
            if index < fixed_lengths.len() && field_len != fixed_lengths[index] {
                return Err(InertProofBindingAssociationErrorV4::InvalidFieldLength);
            }
            if index == FIELD_COUNT_V4 - 1 {
                validate_verus_evidence_len(field_len)?;
            }
            let start = offset
                .checked_add(FIELD_HEADER_BYTES_V4)
                .ok_or(InertProofBindingAssociationErrorV4::LengthOverflow)?;
            let end = start
                .checked_add(field_len)
                .ok_or(InertProofBindingAssociationErrorV4::LengthOverflow)?;
            fields[index] = bytes
                .get(start..end)
                .ok_or(InertProofBindingAssociationErrorV4::Truncated)?;
            offset = end;
        }
        if offset != bytes.len() {
            return Err(InertProofBindingAssociationErrorV4::TrailingBytes);
        }
        if fields[0] != DOMAIN_V4 {
            return Err(InertProofBindingAssociationErrorV4::WrongDomain);
        }
        if fields[1] != CLAIM_V4 {
            return Err(InertProofBindingAssociationErrorV4::WrongClaim);
        }
        let inputs = InertProofBindingAssociationInputsV4::new(
            decode_identity(fields[2])?,
            decode_identity(fields[3])?,
            decode_identity(fields[4])?,
            decode_identity(fields[5])?,
            decode_identity(fields[6])?,
        );
        let decoded = Self::new(inputs, fields[7])?;
        if decoded.canonical_bytes() != bytes {
            return Err(InertProofBindingAssociationErrorV4::NonCanonical);
        }
        Ok(decoded)
    }

    /// Returns the exact canonical V4 bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the exact ordered compiler-stage identities.
    pub const fn inputs(&self) -> InertProofBindingAssociationInputsV4 {
        self.inputs
    }

    /// Returns the exact nested canonical aggregate Verus-execution evidence.
    pub fn verus_execution_evidence(&self) -> &[u8] {
        &self.canonical_bytes[self.verus_evidence_range.clone()]
    }

    /// Reports that the association retains nonempty bounded Verus evidence bytes.
    pub const fn retains_exact_signed_verus_execution_evidence(&self) -> bool {
        true
    }

    /// Reports that inert association bytes do not authenticate their compiler producer.
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    /// Reports that source-side evidence does not establish LLVM or machine refinement.
    pub const fn establishes_llvm_or_machine_refinement(&self) -> bool {
        false
    }

    /// Reports that this inert association grants no compiler or runtime authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn validate_verus_evidence_len(len: usize) -> Result<(), InertProofBindingAssociationErrorV4> {
    if len == 0 {
        return Err(InertProofBindingAssociationErrorV4::EmptyVerusEvidence);
    }
    if len > MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4 {
        return Err(InertProofBindingAssociationErrorV4::VerusEvidenceTooLarge);
    }
    Ok(())
}

fn encode_identity(identity: InertLineageContentIdentityV3) -> [u8; IDENTITY_BYTES_V4] {
    let mut bytes = [0_u8; IDENTITY_BYTES_V4];
    bytes[..32].copy_from_slice(&identity.sha256());
    bytes[32..].copy_from_slice(&identity.byte_len().to_le_bytes());
    bytes
}

fn decode_identity(
    bytes: &[u8],
) -> Result<InertLineageContentIdentityV3, InertProofBindingAssociationErrorV4> {
    if bytes.len() != IDENTITY_BYTES_V4 {
        return Err(InertProofBindingAssociationErrorV4::InvalidFieldLength);
    }
    let mut sha256 = [0_u8; 32];
    sha256.copy_from_slice(&bytes[..32]);
    let mut byte_len = [0_u8; 8];
    byte_len.copy_from_slice(&bytes[32..]);
    InertLineageContentIdentityV3::new(sha256, u64::from_le_bytes(byte_len))
        .map_err(|_| InertProofBindingAssociationErrorV4::InvalidIdentity)
}

fn encoded_len(fields: &[&[u8]]) -> Result<usize, InertProofBindingAssociationErrorV4> {
    let field_headers = fields
        .len()
        .checked_mul(FIELD_HEADER_BYTES_V4)
        .ok_or(InertProofBindingAssociationErrorV4::LengthOverflow)?;
    let payload = fields.iter().try_fold(0_usize, |total, field| {
        total
            .checked_add(field.len())
            .ok_or(InertProofBindingAssociationErrorV4::LengthOverflow)
    })?;
    let total = HEADER_BYTES_V4
        .checked_add(field_headers)
        .and_then(|value| value.checked_add(payload))
        .ok_or(InertProofBindingAssociationErrorV4::LengthOverflow)?;
    if total > MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V4 {
        return Err(InertProofBindingAssociationErrorV4::TooLarge);
    }
    Ok(total)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, InertProofBindingAssociationErrorV4> {
    let value = bytes
        .get(offset..offset.saturating_add(2))
        .and_then(|value| value.try_into().ok())
        .ok_or(InertProofBindingAssociationErrorV4::Truncated)?;
    Ok(u16::from_le_bytes(value))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, InertProofBindingAssociationErrorV4> {
    let value = bytes
        .get(offset..offset.saturating_add(4))
        .and_then(|value| value.try_into().ok())
        .ok_or(InertProofBindingAssociationErrorV4::Truncated)?;
    Ok(u32::from_le_bytes(value))
}

/// Strict V4 association construction and decoding failures.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InertProofBindingAssociationErrorV4 {
    /// The complete association exceeds its hard size limit.
    TooLarge,
    /// Exact allocation for canonical bytes failed.
    AllocationFailed,
    /// A length calculation or conversion overflowed.
    LengthOverflow,
    /// Required bytes are absent.
    Truncated,
    /// The fixed magic does not match V4.
    InvalidMagic,
    /// The record version is not exactly V4.
    UnsupportedVersion,
    /// The transcript record kind is not proof binding.
    WrongRecordKind,
    /// The policy is not the signed-Verus-evidence policy.
    WrongPolicy,
    /// The fixed field count differs.
    WrongFieldCount,
    /// The declared record length differs from the exact input.
    DeclaredLengthMismatch,
    /// A reserved header value is nonzero.
    NonzeroReserved,
    /// An ordered field tag differs.
    WrongFieldTag,
    /// A field has nonzero flags.
    NonzeroFieldFlags,
    /// A fixed-width field has the wrong length.
    InvalidFieldLength,
    /// The nested Verus evidence field is empty.
    EmptyVerusEvidence,
    /// The nested Verus evidence exceeds its independent hard limit.
    VerusEvidenceTooLarge,
    /// A nested stage identity is zero or has zero length.
    InvalidIdentity,
    /// The fixed domain field differs.
    WrongDomain,
    /// The fixed no-later-refinement claim differs.
    WrongClaim,
    /// Bytes remain after the final ordered field.
    TrailingBytes,
    /// Re-encoding the decoded value does not reproduce the exact input.
    NonCanonical,
}

impl fmt::Display for InertProofBindingAssociationErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::TooLarge => "V4 proof-binding association exceeds its hard limit",
            Self::AllocationFailed => "V4 proof-binding association allocation failed",
            Self::LengthOverflow => "V4 proof-binding association length overflowed",
            Self::Truncated => "V4 proof-binding association is truncated",
            Self::InvalidMagic => "V4 proof-binding association magic mismatch",
            Self::UnsupportedVersion => "unsupported V4 proof-binding association version",
            Self::WrongRecordKind => "V4 proof-binding association record kind mismatch",
            Self::WrongPolicy => "V4 proof-binding association policy mismatch",
            Self::WrongFieldCount => "V4 proof-binding association field count mismatch",
            Self::DeclaredLengthMismatch => "V4 proof-binding association declared length mismatch",
            Self::NonzeroReserved => "V4 proof-binding association reserved field is nonzero",
            Self::WrongFieldTag => "V4 proof-binding association field tag mismatch",
            Self::NonzeroFieldFlags => "V4 proof-binding association field flags are nonzero",
            Self::InvalidFieldLength => "V4 proof-binding association field length is invalid",
            Self::EmptyVerusEvidence => "V4 proof-binding association has no Verus evidence",
            Self::VerusEvidenceTooLarge => {
                "V4 proof-binding association Verus evidence is too large"
            }
            Self::InvalidIdentity => "V4 proof-binding association contains an invalid identity",
            Self::WrongDomain => "V4 proof-binding association domain mismatch",
            Self::WrongClaim => "V4 proof-binding association claim mismatch",
            Self::TrailingBytes => "V4 proof-binding association has trailing bytes",
            Self::NonCanonical => "V4 proof-binding association encoding is noncanonical",
        })
    }
}

impl Error for InertProofBindingAssociationErrorV4 {}
