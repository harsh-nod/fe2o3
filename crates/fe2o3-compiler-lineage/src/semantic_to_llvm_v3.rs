use std::{error::Error, fmt};

/// Magic of the frozen production target-lineage transcript envelope.
pub const INERT_SEMANTIC_TO_LLVM_ASSOCIATION_MAGIC_V3: [u8; 8] = *b"F2O3TLV3";
/// Version of the frozen semantic-to-LLVM association.
pub const INERT_SEMANTIC_TO_LLVM_ASSOCIATION_VERSION_V3: u16 = 3;
/// Maximum canonical size admitted for one association.
pub const MAX_INERT_SEMANTIC_TO_LLVM_ASSOCIATION_BYTES_V3: usize = 4 * 1024 * 1024;

const RECORD_KIND_V3: u16 = 5;
const ASSOCIATION_ONLY_POLICY_V3: u16 = 1;
const HEADER_BYTES_V3: usize = 24;
const FIELD_HEADER_BYTES_V3: usize = 8;
const FIELD_COUNT_V3: usize = 15;
const IDENTITY_BYTES_V3: usize = 40;
const DOMAIN_V3: &[u8] = b"FE2O3/PRODUCTION-SEMANTIC-TO-LLVM-ASSOCIATION/V3\0";
const CLAIM_V3: &[u8] = b"association-only/no-refinement-proof";

/// Exact digest and length of one input named by the association.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct InertSemanticToLlvmContentIdentityV3 {
    sha256: [u8; 32],
    byte_len: u64,
}

impl InertSemanticToLlvmContentIdentityV3 {
    /// Constructs a nonempty identity.
    pub fn new(
        sha256: [u8; 32],
        byte_len: u64,
    ) -> Result<Self, InertSemanticToLlvmAssociationErrorV3> {
        if sha256 == [0; 32] {
            return Err(InertSemanticToLlvmAssociationErrorV3::ZeroIdentity);
        }
        if byte_len == 0 {
            return Err(InertSemanticToLlvmAssociationErrorV3::ZeroIdentityLength);
        }
        Ok(Self { sha256, byte_len })
    }

    /// Returns the digest bytes.
    pub const fn sha256(self) -> [u8; 32] {
        self.sha256
    }

    /// Returns the exact named byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    fn encode(self) -> [u8; IDENTITY_BYTES_V3] {
        let mut bytes = [0; IDENTITY_BYTES_V3];
        bytes[..32].copy_from_slice(&self.sha256);
        bytes[32..].copy_from_slice(&self.byte_len.to_le_bytes());
        bytes
    }

    fn decode(bytes: &[u8]) -> Result<Self, InertSemanticToLlvmAssociationErrorV3> {
        if bytes.len() != IDENTITY_BYTES_V3 {
            return Err(InertSemanticToLlvmAssociationErrorV3::InvalidFieldLength);
        }
        let mut sha256 = [0; 32];
        sha256.copy_from_slice(&bytes[..32]);
        let mut byte_len = [0; 8];
        byte_len.copy_from_slice(&bytes[32..]);
        Self::new(sha256, u64::from_le_bytes(byte_len))
    }
}

/// Ordered exact inputs associated by the frozen V3 receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InertSemanticToLlvmAssociationInputsV3 {
    identities: [InertSemanticToLlvmContentIdentityV3; 13],
}

impl InertSemanticToLlvmAssociationInputsV3 {
    /// Constructs the exact schema-ordered association inputs.
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        semantic_mir: InertSemanticToLlvmContentIdentityV3,
        middle_end: InertSemanticToLlvmContentIdentityV3,
        kernel_ir: InertSemanticToLlvmContentIdentityV3,
        mir_to_kir_correspondence: InertSemanticToLlvmContentIdentityV3,
        formal_memory: InertSemanticToLlvmContentIdentityV3,
        proof_binding: InertSemanticToLlvmContentIdentityV3,
        target_binding: InertSemanticToLlvmContentIdentityV3,
        data_layout: InertSemanticToLlvmContentIdentityV3,
        abi: InertSemanticToLlvmContentIdentityV3,
        export_manifest: InertSemanticToLlvmContentIdentityV3,
        amdgpu_lowering: InertSemanticToLlvmContentIdentityV3,
        final_llvm: InertSemanticToLlvmContentIdentityV3,
        final_compiler_module_commitment: InertSemanticToLlvmContentIdentityV3,
    ) -> Self {
        Self {
            identities: [
                semantic_mir,
                middle_end,
                kernel_ir,
                mir_to_kir_correspondence,
                formal_memory,
                proof_binding,
                target_binding,
                data_layout,
                abi,
                export_manifest,
                amdgpu_lowering,
                final_llvm,
                final_compiler_module_commitment,
            ],
        }
    }

    /// Returns the semantic-MIR identity.
    pub const fn semantic_mir(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[0]
    }
    /// Returns the middle-end identity.
    pub const fn middle_end(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[1]
    }
    /// Returns the Kernel IR identity.
    pub const fn kernel_ir(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[2]
    }
    /// Returns the MIR-to-KIR correspondence identity.
    pub const fn mir_to_kir_correspondence(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[3]
    }
    /// Returns the formal-memory identity.
    pub const fn formal_memory(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[4]
    }
    /// Returns the proof-binding identity.
    pub const fn proof_binding(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[5]
    }
    /// Returns the target-binding identity.
    pub const fn target_binding(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[6]
    }
    /// Returns the data-layout identity.
    pub const fn data_layout(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[7]
    }
    /// Returns the ABI identity.
    pub const fn abi(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[8]
    }
    /// Returns the export-manifest identity.
    pub const fn export_manifest(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[9]
    }
    /// Returns the AMDGPU-lowering identity.
    pub const fn amdgpu_lowering(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[10]
    }
    /// Returns the exact final-LLVM identity.
    pub const fn final_llvm(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[11]
    }
    /// Returns the compact final-module commitment identity.
    pub const fn final_compiler_module_commitment(self) -> InertSemanticToLlvmContentIdentityV3 {
        self.identities[12]
    }
}

/// Strict frozen V3 semantic-to-LLVM association.
#[derive(Debug, Eq, PartialEq)]
pub struct InertSemanticToLlvmAssociationV3 {
    canonical_bytes: Box<[u8]>,
    inputs: InertSemanticToLlvmAssociationInputsV3,
}

impl InertSemanticToLlvmAssociationV3 {
    /// Encodes one association using the existing frozen V3 wire schema.
    pub fn new(
        inputs: InertSemanticToLlvmAssociationInputsV3,
    ) -> Result<Self, InertSemanticToLlvmAssociationErrorV3> {
        let identities = inputs
            .identities
            .map(InertSemanticToLlvmContentIdentityV3::encode);
        let mut fields: Vec<&[u8]> = Vec::new();
        fields
            .try_reserve_exact(FIELD_COUNT_V3)
            .map_err(|_| InertSemanticToLlvmAssociationErrorV3::AllocationFailed)?;
        fields.push(DOMAIN_V3);
        fields.push(CLAIM_V3);
        for identity in &identities {
            fields.push(identity);
        }
        let total = encoded_len(&fields)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(total)
            .map_err(|_| InertSemanticToLlvmAssociationErrorV3::AllocationFailed)?;
        bytes.extend_from_slice(&INERT_SEMANTIC_TO_LLVM_ASSOCIATION_MAGIC_V3);
        bytes.extend_from_slice(&INERT_SEMANTIC_TO_LLVM_ASSOCIATION_VERSION_V3.to_le_bytes());
        bytes.extend_from_slice(&RECORD_KIND_V3.to_le_bytes());
        bytes.extend_from_slice(&ASSOCIATION_ONLY_POLICY_V3.to_le_bytes());
        bytes.extend_from_slice(&(FIELD_COUNT_V3 as u16).to_le_bytes());
        bytes.extend_from_slice(&(total as u32).to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        for (index, field) in fields.iter().enumerate() {
            bytes.extend_from_slice(
                &u16::try_from(index + 1)
                    .map_err(|_| InertSemanticToLlvmAssociationErrorV3::LengthOverflow)?
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(&0_u16.to_le_bytes());
            bytes.extend_from_slice(
                &u32::try_from(field.len())
                    .map_err(|_| InertSemanticToLlvmAssociationErrorV3::LengthOverflow)?
                    .to_le_bytes(),
            );
            bytes.extend_from_slice(field);
        }
        Ok(Self {
            canonical_bytes: bytes.into_boxed_slice(),
            inputs,
        })
    }

    /// Strictly decodes a complete frozen V3 association.
    pub fn decode(bytes: &[u8]) -> Result<Self, InertSemanticToLlvmAssociationErrorV3> {
        if bytes.len() > MAX_INERT_SEMANTIC_TO_LLVM_ASSOCIATION_BYTES_V3 {
            return Err(InertSemanticToLlvmAssociationErrorV3::TooLarge);
        }
        if bytes.len() < HEADER_BYTES_V3 {
            return Err(InertSemanticToLlvmAssociationErrorV3::Truncated);
        }
        if bytes[..8] != INERT_SEMANTIC_TO_LLVM_ASSOCIATION_MAGIC_V3 {
            return Err(InertSemanticToLlvmAssociationErrorV3::InvalidMagic);
        }
        if read_u16(bytes, 8)? != INERT_SEMANTIC_TO_LLVM_ASSOCIATION_VERSION_V3 {
            return Err(InertSemanticToLlvmAssociationErrorV3::UnsupportedVersion);
        }
        if read_u16(bytes, 10)? != RECORD_KIND_V3 {
            return Err(InertSemanticToLlvmAssociationErrorV3::WrongRecordKind);
        }
        if read_u16(bytes, 12)? != ASSOCIATION_ONLY_POLICY_V3 {
            return Err(InertSemanticToLlvmAssociationErrorV3::WrongPolicy);
        }
        if usize::from(read_u16(bytes, 14)?) != FIELD_COUNT_V3 {
            return Err(InertSemanticToLlvmAssociationErrorV3::WrongFieldCount);
        }
        if usize::try_from(read_u32(bytes, 16)?)
            .map_err(|_| InertSemanticToLlvmAssociationErrorV3::LengthOverflow)?
            != bytes.len()
        {
            return Err(InertSemanticToLlvmAssociationErrorV3::DeclaredLengthMismatch);
        }
        if read_u32(bytes, 20)? != 0 {
            return Err(InertSemanticToLlvmAssociationErrorV3::NonZeroReserved);
        }
        let mut offset = HEADER_BYTES_V3;
        let mut decoded = Vec::new();
        decoded
            .try_reserve_exact(13)
            .map_err(|_| InertSemanticToLlvmAssociationErrorV3::AllocationFailed)?;
        for index in 0..FIELD_COUNT_V3 {
            if read_u16(bytes, offset)? != (index + 1) as u16 {
                return Err(InertSemanticToLlvmAssociationErrorV3::WrongFieldTag);
            }
            if read_u16(bytes, offset + 2)? != 0 {
                return Err(InertSemanticToLlvmAssociationErrorV3::NonZeroFieldFlags);
            }
            let len = usize::try_from(read_u32(bytes, offset + 4)?)
                .map_err(|_| InertSemanticToLlvmAssociationErrorV3::LengthOverflow)?;
            let start = offset
                .checked_add(FIELD_HEADER_BYTES_V3)
                .ok_or(InertSemanticToLlvmAssociationErrorV3::LengthOverflow)?;
            let end = start
                .checked_add(len)
                .ok_or(InertSemanticToLlvmAssociationErrorV3::LengthOverflow)?;
            let field = bytes
                .get(start..end)
                .ok_or(InertSemanticToLlvmAssociationErrorV3::Truncated)?;
            match index {
                0 if field != DOMAIN_V3 => {
                    return Err(InertSemanticToLlvmAssociationErrorV3::DomainMismatch);
                }
                1 if field != CLAIM_V3 => {
                    return Err(InertSemanticToLlvmAssociationErrorV3::ClaimMismatch);
                }
                2..=14 => decoded.push(InertSemanticToLlvmContentIdentityV3::decode(field)?),
                _ => {}
            }
            offset = end;
        }
        if offset != bytes.len() {
            return Err(InertSemanticToLlvmAssociationErrorV3::TrailingBytes);
        }
        let identities: [InertSemanticToLlvmContentIdentityV3; 13] = decoded
            .try_into()
            .map_err(|_| InertSemanticToLlvmAssociationErrorV3::WrongFieldCount)?;
        let inputs = InertSemanticToLlvmAssociationInputsV3 { identities };
        let canonical = Self::new(inputs)?;
        if canonical.canonical_bytes() != bytes {
            return Err(InertSemanticToLlvmAssociationErrorV3::NonCanonical);
        }
        Ok(canonical)
    }

    /// Returns the exact ordered input identities.
    pub const fn inputs(&self) -> InertSemanticToLlvmAssociationInputsV3 {
        self.inputs
    }
    /// Returns the exact frozen canonical bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Reports that this record establishes association only.
    pub const fn establishes_refinement_proof(&self) -> bool {
        false
    }
}

fn encoded_len(fields: &[&[u8]]) -> Result<usize, InertSemanticToLlvmAssociationErrorV3> {
    let mut total = HEADER_BYTES_V3;
    for field in fields {
        total = total
            .checked_add(FIELD_HEADER_BYTES_V3)
            .and_then(|value| value.checked_add(field.len()))
            .ok_or(InertSemanticToLlvmAssociationErrorV3::LengthOverflow)?;
    }
    if total > MAX_INERT_SEMANTIC_TO_LLVM_ASSOCIATION_BYTES_V3 || total > u32::MAX as usize {
        return Err(InertSemanticToLlvmAssociationErrorV3::TooLarge);
    }
    Ok(total)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, InertSemanticToLlvmAssociationErrorV3> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(InertSemanticToLlvmAssociationErrorV3::Truncated)?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, InertSemanticToLlvmAssociationErrorV3> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or(InertSemanticToLlvmAssociationErrorV3::Truncated)?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

/// Strict decode/encode failure for the frozen V3 association.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InertSemanticToLlvmAssociationErrorV3 {
    /// Input exceeds the fixed bound.
    TooLarge,
    /// Input ends before a complete field.
    Truncated,
    /// Envelope magic differs.
    InvalidMagic,
    /// Envelope version differs.
    UnsupportedVersion,
    /// Record kind differs.
    WrongRecordKind,
    /// Policy differs.
    WrongPolicy,
    /// Field count differs.
    WrongFieldCount,
    /// Declared size differs.
    DeclaredLengthMismatch,
    /// Reserved header bits are nonzero.
    NonZeroReserved,
    /// A field tag is not canonical.
    WrongFieldTag,
    /// Field flags are nonzero.
    NonZeroFieldFlags,
    /// Domain differs.
    DomainMismatch,
    /// Claim differs.
    ClaimMismatch,
    /// An identity field has the wrong length.
    InvalidFieldLength,
    /// An identity digest is zero.
    ZeroIdentity,
    /// An identity length is zero.
    ZeroIdentityLength,
    /// Arithmetic overflowed.
    LengthOverflow,
    /// Allocation failed.
    AllocationFailed,
    /// Extra bytes follow the record.
    TrailingBytes,
    /// Input has a noncanonical representation.
    NonCanonical,
}

impl fmt::Display for InertSemanticToLlvmAssociationErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "invalid inert semantic-to-LLVM V3 association: {self:?}"
        )
    }
}

impl Error for InertSemanticToLlvmAssociationErrorV3 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(byte: u8) -> InertSemanticToLlvmContentIdentityV3 {
        InertSemanticToLlvmContentIdentityV3::new([byte; 32], u64::from(byte)).unwrap()
    }

    #[test]
    fn strict_round_trip_and_hostile_fields() {
        let inputs = InertSemanticToLlvmAssociationInputsV3::new(
            identity(1),
            identity(2),
            identity(3),
            identity(4),
            identity(5),
            identity(6),
            identity(7),
            identity(8),
            identity(9),
            identity(10),
            identity(11),
            identity(12),
            identity(13),
        );
        let association = InertSemanticToLlvmAssociationV3::new(inputs).unwrap();
        assert_eq!(
            InertSemanticToLlvmAssociationV3::decode(association.canonical_bytes())
                .unwrap()
                .inputs(),
            inputs
        );
        assert_eq!(
            [
                inputs.semantic_mir(),
                inputs.middle_end(),
                inputs.kernel_ir(),
                inputs.mir_to_kir_correspondence(),
                inputs.formal_memory(),
                inputs.proof_binding(),
                inputs.target_binding(),
                inputs.data_layout(),
                inputs.abi(),
                inputs.export_manifest(),
                inputs.amdgpu_lowering(),
                inputs.final_llvm(),
                inputs.final_compiler_module_commitment(),
            ],
            core::array::from_fn(|index| identity((index + 1) as u8))
        );
        for offset in [0, 10, 12, 14, 20, 24, 24 + 8 + DOMAIN_V3.len()] {
            let mut hostile = association.canonical_bytes().to_vec();
            hostile[offset] ^= 1;
            assert!(InertSemanticToLlvmAssociationV3::decode(&hostile).is_err());
        }
        let mut trailing = association.canonical_bytes().to_vec();
        trailing.push(0);
        assert_eq!(
            InertSemanticToLlvmAssociationV3::decode(&trailing),
            Err(InertSemanticToLlvmAssociationErrorV3::DeclaredLengthMismatch)
        );
        for length in 0..association.canonical_bytes().len() {
            assert!(
                InertSemanticToLlvmAssociationV3::decode(&association.canonical_bytes()[..length])
                    .is_err()
            );
        }
        assert_eq!(
            InertSemanticToLlvmAssociationV3::decode(&vec![
                0;
                MAX_INERT_SEMANTIC_TO_LLVM_ASSOCIATION_BYTES_V3
                    + 1
            ]),
            Err(InertSemanticToLlvmAssociationErrorV3::TooLarge)
        );
    }
}
