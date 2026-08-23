use std::{error::Error, fmt};

/// Magic of the frozen production target-lineage transcript envelope.
pub const INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V3: [u8; 8] = *b"F2O3TLV3";
/// Version of the frozen production target-lineage transcript envelope.
pub const INERT_PROOF_BINDING_ASSOCIATION_VERSION_V3: u16 = 3;
/// Maximum canonical size accepted for a V3 proof-binding association.
pub const MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V3: usize = 4 * 1024 * 1024;

const RECORD_KIND_V3: u16 = 6;
const ASSOCIATION_ONLY_POLICY_V3: u16 = 1;
const HEADER_BYTES_V3: usize = 24;
const FIELD_HEADER_BYTES_V3: usize = 8;
const FIELD_COUNT_V3: usize = 7;
const IDENTITY_BYTES_V3: usize = 40;
const DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-PROOF-BINDING-ASSOCIATION/V3\0";
const CLAIM_V3: &[u8] = b"association-only/no-refinement-proof";

/// Digest and exact byte length of one canonical compiler-lineage input.
///
/// Construction validates shape only. It does not authenticate the producer or the named bytes.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertLineageContentIdentityV3 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl InertLineageContentIdentityV3 {
    /// Constructs one nonzero inert content identity.
    pub fn new(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, InertProofBindingAssociationErrorV3> {
        if sha256 == [0; 32] {
            return Err(InertProofBindingAssociationErrorV3::ZeroIdentity);
        }
        if byte_len == 0 {
            return Err(InertProofBindingAssociationErrorV3::ZeroIdentityLength);
        }
        Ok(Self { sha256, byte_len })
    }

    /// Returns the inert digest bytes.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact named content length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn encode(self) -> [u8; IDENTITY_BYTES_V3] {
        let mut bytes = [0_u8; IDENTITY_BYTES_V3];
        bytes[..32].copy_from_slice(&self.sha256);
        bytes[32..].copy_from_slice(&self.byte_len.to_le_bytes());
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, InertProofBindingAssociationErrorV3> {
        if bytes.len() != IDENTITY_BYTES_V3 {
            return Err(InertProofBindingAssociationErrorV3::InvalidFieldLength);
        }
        let mut sha256 = [0_u8; 32];
        sha256.copy_from_slice(&bytes[..32]);
        let mut byte_len = [0_u8; 8];
        byte_len.copy_from_slice(&bytes[32..]);
        Self::new(sha256, u64::from_le_bytes(byte_len))
    }
}

/// The five exact compiler-stage identities associated by the V3 proof-binding receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertProofBindingAssociationInputsV3 {
    semantic_mir: InertLineageContentIdentityV3,
    middle_end: InertLineageContentIdentityV3,
    kernel_ir: InertLineageContentIdentityV3,
    mir_to_kir_correspondence: InertLineageContentIdentityV3,
    formal_memory: InertLineageContentIdentityV3,
}

impl InertProofBindingAssociationInputsV3 {
    /// Constructs the exact ordered association inputs.
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

    /// Returns the Kernel IR receipt identity.
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

/// Frozen, canonical association of the five semantic compiler stages presented to proof.
///
/// This record deliberately states only association. It does not claim that Verus executed,
/// establish source-to-machine refinement, or grant proof, publication, load, or launch authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InertProofBindingAssociationV3 {
    canonical_bytes: Box<[u8]>,
    inputs: InertProofBindingAssociationInputsV3,
}

impl InertProofBindingAssociationV3 {
    /// Canonically encodes one association while preserving the frozen compiler wire format.
    pub fn new(
        inputs: InertProofBindingAssociationInputsV3,
    ) -> Result<Self, InertProofBindingAssociationErrorV3> {
        let encoded_identities = inputs.ordered().map(InertLineageContentIdentityV3::encode);
        let fields: [&[u8]; FIELD_COUNT_V3] = [
            DOMAIN_V3,
            CLAIM_V3,
            &encoded_identities[0],
            &encoded_identities[1],
            &encoded_identities[2],
            &encoded_identities[3],
            &encoded_identities[4],
        ];
        let total_len = encoded_len(&fields)?;
        let total_len_u32 = u32::try_from(total_len)
            .map_err(|_| InertProofBindingAssociationErrorV3::LengthOverflow)?;

        let mut canonical = Vec::new();
        canonical
            .try_reserve_exact(total_len)
            .map_err(|_| InertProofBindingAssociationErrorV3::AllocationFailed)?;
        canonical.extend_from_slice(&INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V3);
        canonical.extend_from_slice(&INERT_PROOF_BINDING_ASSOCIATION_VERSION_V3.to_le_bytes());
        canonical.extend_from_slice(&RECORD_KIND_V3.to_le_bytes());
        canonical.extend_from_slice(&ASSOCIATION_ONLY_POLICY_V3.to_le_bytes());
        canonical.extend_from_slice(&(FIELD_COUNT_V3 as u16).to_le_bytes());
        canonical.extend_from_slice(&total_len_u32.to_le_bytes());
        canonical.extend_from_slice(&0_u32.to_le_bytes());
        for (index, field) in fields.iter().enumerate() {
            let tag = u16::try_from(index + 1)
                .map_err(|_| InertProofBindingAssociationErrorV3::LengthOverflow)?;
            let field_len = u32::try_from(field.len())
                .map_err(|_| InertProofBindingAssociationErrorV3::LengthOverflow)?;
            canonical.extend_from_slice(&tag.to_le_bytes());
            canonical.extend_from_slice(&0_u16.to_le_bytes());
            canonical.extend_from_slice(&field_len.to_le_bytes());
            canonical.extend_from_slice(field);
        }
        debug_assert_eq!(canonical.len(), total_len);
        Ok(Self {
            canonical_bytes: canonical.into_boxed_slice(),
            inputs,
        })
    }

    /// Strictly decodes one complete association with no version or policy fallback.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertProofBindingAssociationErrorV3> {
        if bytes.len() > MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V3 {
            return Err(InertProofBindingAssociationErrorV3::TooLarge);
        }
        if bytes.len() < HEADER_BYTES_V3 {
            return Err(InertProofBindingAssociationErrorV3::Truncated);
        }
        if bytes[..8] != INERT_PROOF_BINDING_ASSOCIATION_MAGIC_V3 {
            return Err(InertProofBindingAssociationErrorV3::InvalidMagic);
        }
        if read_u16(bytes, 8)? != INERT_PROOF_BINDING_ASSOCIATION_VERSION_V3 {
            return Err(InertProofBindingAssociationErrorV3::UnsupportedVersion);
        }
        if read_u16(bytes, 10)? != RECORD_KIND_V3 {
            return Err(InertProofBindingAssociationErrorV3::WrongRecordKind);
        }
        if read_u16(bytes, 12)? != ASSOCIATION_ONLY_POLICY_V3 {
            return Err(InertProofBindingAssociationErrorV3::WrongPolicy);
        }
        if usize::from(read_u16(bytes, 14)?) != FIELD_COUNT_V3 {
            return Err(InertProofBindingAssociationErrorV3::WrongFieldCount);
        }
        let declared = usize::try_from(read_u32(bytes, 16)?)
            .map_err(|_| InertProofBindingAssociationErrorV3::LengthOverflow)?;
        if declared != bytes.len() {
            return Err(InertProofBindingAssociationErrorV3::DeclaredLengthMismatch);
        }
        if read_u32(bytes, 20)? != 0 {
            return Err(InertProofBindingAssociationErrorV3::NonZeroReserved);
        }

        let expected_lengths = [
            DOMAIN_V3.len(),
            CLAIM_V3.len(),
            IDENTITY_BYTES_V3,
            IDENTITY_BYTES_V3,
            IDENTITY_BYTES_V3,
            IDENTITY_BYTES_V3,
            IDENTITY_BYTES_V3,
        ];
        let mut fields: [&[u8]; FIELD_COUNT_V3] = [&[]; FIELD_COUNT_V3];
        let mut offset = HEADER_BYTES_V3;
        for index in 0..FIELD_COUNT_V3 {
            let expected_tag = u16::try_from(index + 1)
                .map_err(|_| InertProofBindingAssociationErrorV3::LengthOverflow)?;
            if read_u16(bytes, offset)? != expected_tag {
                return Err(InertProofBindingAssociationErrorV3::WrongFieldTag);
            }
            if read_u16(bytes, offset + 2)? != 0 {
                return Err(InertProofBindingAssociationErrorV3::NonZeroFieldFlags);
            }
            let field_len = usize::try_from(read_u32(bytes, offset + 4)?)
                .map_err(|_| InertProofBindingAssociationErrorV3::LengthOverflow)?;
            if field_len != expected_lengths[index] {
                return Err(InertProofBindingAssociationErrorV3::InvalidFieldLength);
            }
            let start = offset
                .checked_add(FIELD_HEADER_BYTES_V3)
                .ok_or(InertProofBindingAssociationErrorV3::LengthOverflow)?;
            let end = start
                .checked_add(field_len)
                .ok_or(InertProofBindingAssociationErrorV3::LengthOverflow)?;
            fields[index] = bytes
                .get(start..end)
                .ok_or(InertProofBindingAssociationErrorV3::Truncated)?;
            offset = end;
        }
        if offset != bytes.len() {
            return Err(InertProofBindingAssociationErrorV3::TrailingBytes);
        }
        if fields[0] != DOMAIN_V3 {
            return Err(InertProofBindingAssociationErrorV3::WrongDomain);
        }
        if fields[1] != CLAIM_V3 {
            return Err(InertProofBindingAssociationErrorV3::WrongClaim);
        }
        let inputs = InertProofBindingAssociationInputsV3::new(
            InertLineageContentIdentityV3::decode(fields[2])?,
            InertLineageContentIdentityV3::decode(fields[3])?,
            InertLineageContentIdentityV3::decode(fields[4])?,
            InertLineageContentIdentityV3::decode(fields[5])?,
            InertLineageContentIdentityV3::decode(fields[6])?,
        );
        let decoded = Self::new(inputs)?;
        if decoded.canonical_bytes() != bytes {
            return Err(InertProofBindingAssociationErrorV3::NonCanonical);
        }
        Ok(decoded)
    }

    /// Returns the exact frozen canonical encoding.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Returns the exact ordered compiler-stage identities.
    pub const fn inputs(&self) -> InertProofBindingAssociationInputsV3 {
        self.inputs
    }

    /// Reports that this association does not claim Verus execution.
    pub const fn claims_verus_verification(&self) -> bool {
        false
    }

    /// Reports that this association does not establish compiler refinement.
    pub const fn establishes_refinement_proof(&self) -> bool {
        false
    }

    /// Reports that this inert association grants no authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn encoded_len(
    fields: &[&[u8]; FIELD_COUNT_V3],
) -> Result<usize, InertProofBindingAssociationErrorV3> {
    let mut total = HEADER_BYTES_V3
        .checked_add(FIELD_COUNT_V3 * FIELD_HEADER_BYTES_V3)
        .ok_or(InertProofBindingAssociationErrorV3::LengthOverflow)?;
    for field in fields {
        total = total
            .checked_add(field.len())
            .ok_or(InertProofBindingAssociationErrorV3::LengthOverflow)?;
    }
    if total > MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V3 {
        return Err(InertProofBindingAssociationErrorV3::TooLarge);
    }
    Ok(total)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, InertProofBindingAssociationErrorV3> {
    let end = offset
        .checked_add(2)
        .ok_or(InertProofBindingAssociationErrorV3::LengthOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(InertProofBindingAssociationErrorV3::Truncated)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, InertProofBindingAssociationErrorV3> {
    let end = offset
        .checked_add(4)
        .ok_or(InertProofBindingAssociationErrorV3::LengthOverflow)?;
    let value = bytes
        .get(offset..end)
        .ok_or(InertProofBindingAssociationErrorV3::Truncated)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

/// Stable failure category for strict V3 proof-binding association construction or decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InertProofBindingAssociationErrorV3 {
    /// The canonical record exceeds its hard byte limit.
    TooLarge,
    /// The input ends before a complete canonical record.
    Truncated,
    /// The transcript magic is not the frozen V3 magic.
    InvalidMagic,
    /// The transcript version is not exactly V3.
    UnsupportedVersion,
    /// The transcript is not the proof-binding record kind.
    WrongRecordKind,
    /// The transcript does not use the association-only policy.
    WrongPolicy,
    /// The transcript does not contain exactly seven fields.
    WrongFieldCount,
    /// The declared and physical record lengths differ.
    DeclaredLengthMismatch,
    /// A header reserved field is nonzero.
    NonZeroReserved,
    /// A field tag is missing, duplicated, or reordered.
    WrongFieldTag,
    /// A field has unsupported flags.
    NonZeroFieldFlags,
    /// A field has a noncanonical length.
    InvalidFieldLength,
    /// Bytes remain after the final canonical field.
    TrailingBytes,
    /// The exact proof-binding domain differs.
    WrongDomain,
    /// The association-only claim differs.
    WrongClaim,
    /// A required SHA-256 identity is zero.
    ZeroIdentity,
    /// A required exact content length is zero.
    ZeroIdentityLength,
    /// A checked length calculation overflowed.
    LengthOverflow,
    /// A bounded exact allocation failed.
    AllocationFailed,
    /// Structured re-encoding changed the supplied bytes.
    NonCanonical,
}

impl fmt::Display for InertProofBindingAssociationErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid inert V3 proof-binding association: {self:?}"
        )
    }
}

impl Error for InertProofBindingAssociationErrorV3 {}
