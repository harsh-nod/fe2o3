//! Fixed-size root coordinates into shared, replayed V6 correspondence evidence.
//!
//! This record carries bindings, not semantic truth. The complete V6 preimage is
//! retained once in source evidence; consumers must join this record to its exact
//! root there. Decoding never establishes MIR/KIR equivalence or proof authority.

use std::{error::Error, fmt};

/// Distinct wire magic for execution-coordinate root bindings.
pub const MULTI_ROOT_CORRESPONDENCE_PAYLOAD_MAGIC_V3: [u8; 8] = *b"F2MRCOP3";
/// Exact fixed-format byte length, independent of source or graph size.
pub const MULTI_ROOT_CORRESPONDENCE_PAYLOAD_BYTES_V3: usize = 292;

/// Induction coordinates retained by the complete, replayed root report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum MultiRootInductionKindV3 {
    /// Unchanged selected source-body coordinates and original V1 evidence.
    OriginalV1 = 1,
    /// Checked expanded execution coordinates and induction V2 evidence.
    ExpandedV2 = 2,
}

/// Inert coordinate bindings; supplying them never authenticates their contents.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MultiRootCorrespondenceInputsV3 {
    /// Ordinal in the source-ordered kernel roster, not descriptor order.
    pub root_ordinal: u32,
    /// Original Rust MIR function-table root index.
    pub semantic_root: u32,
    /// Selected original body index, which may differ from the physical root.
    pub selected_body: u32,
    /// Exact entry-function ordinal in the source KIR module.
    pub kernel_function_ordinal: u32,
    /// Exact original semantic MIR content identity.
    pub semantic_mir_sha256: [u8; 32],
    /// Content identity of the complete shared V6 correspondence preimage.
    pub correspondence_identity: [u8; 32],
    /// Canonical call-expansion evidence identity, not its aggregate view ID.
    pub expansion_evidence_identity: [u8; 32],
    /// Original physical root's function identity.
    pub semantic_root_identity: [u8; 32],
    /// Original selected body's function identity.
    pub selected_body_identity: [u8; 32],
    /// Exact per-root execution-view content identity, including call-free views.
    pub execution_view_identity: [u8; 32],
    /// Identity of the exact function analyzed and lowered from that view.
    pub execution_function_identity: [u8; 32],
    /// Identity of the complete canonical induction report for this root.
    pub induction_identity: [u8; 32],
    /// Closed tag selecting the report's coordinate space and wire version.
    pub induction_kind: MultiRootInductionKindV3,
}

/// Canonical authority-free root reference into one shared V6 source relation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MultiRootCorrespondencePayloadV3 {
    bytes: [u8; MULTI_ROOT_CORRESPONDENCE_PAYLOAD_BYTES_V3],
    inputs: MultiRootCorrespondenceInputsV3,
}

impl MultiRootCorrespondencePayloadV3 {
    /// Encodes inert bindings. Production must derive and replay every input.
    pub fn new(
        inputs: MultiRootCorrespondenceInputsV3,
    ) -> Result<Self, MultiRootCorrespondencePayloadErrorV3> {
        validate(&inputs)?;
        let mut bytes = [0; MULTI_ROOT_CORRESPONDENCE_PAYLOAD_BYTES_V3];
        bytes[..8].copy_from_slice(&MULTI_ROOT_CORRESPONDENCE_PAYLOAD_MAGIC_V3);
        bytes[8..10].copy_from_slice(&3_u16.to_le_bytes());
        bytes[10..12].copy_from_slice(&1_u16.to_le_bytes());
        bytes[12..16]
            .copy_from_slice(&(MULTI_ROOT_CORRESPONDENCE_PAYLOAD_BYTES_V3 as u32).to_le_bytes());
        for (slot, value) in bytes[16..32].chunks_exact_mut(4).zip([
            inputs.root_ordinal,
            inputs.semantic_root,
            inputs.selected_body,
            inputs.kernel_function_ordinal,
        ]) {
            slot.copy_from_slice(&value.to_le_bytes());
        }
        for (slot, value) in bytes[32..288].chunks_exact_mut(32).zip(identities(&inputs)) {
            slot.copy_from_slice(value);
        }
        bytes[288] = inputs.induction_kind as u8;
        Ok(Self { bytes, inputs })
    }

    /// Strictly decodes the fixed record without accepting legacy payload magic.
    pub fn decode(bytes: &[u8]) -> Result<Self, MultiRootCorrespondencePayloadErrorV3> {
        use MultiRootCorrespondencePayloadErrorV3 as E;
        if bytes.len() != MULTI_ROOT_CORRESPONDENCE_PAYLOAD_BYTES_V3 {
            return Err(E::Length);
        }
        if bytes[..8] != MULTI_ROOT_CORRESPONDENCE_PAYLOAD_MAGIC_V3
            || bytes[8..12] != [3, 0, 1, 0]
            || bytes[12..16] != (MULTI_ROOT_CORRESPONDENCE_PAYLOAD_BYTES_V3 as u32).to_le_bytes()
            || bytes[289..292] != [0; 3]
        {
            return Err(E::Header);
        }
        let u32_at =
            |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().expect("fixed-size field"));
        let identity_at =
            |at: usize| <[u8; 32]>::try_from(&bytes[at..at + 32]).expect("fixed-size identity");
        Self::new(MultiRootCorrespondenceInputsV3 {
            root_ordinal: u32_at(16),
            semantic_root: u32_at(20),
            selected_body: u32_at(24),
            kernel_function_ordinal: u32_at(28),
            semantic_mir_sha256: identity_at(32),
            correspondence_identity: identity_at(64),
            expansion_evidence_identity: identity_at(96),
            semantic_root_identity: identity_at(128),
            selected_body_identity: identity_at(160),
            execution_view_identity: identity_at(192),
            execution_function_identity: identity_at(224),
            induction_identity: identity_at(256),
            induction_kind: match bytes[288] {
                1 => MultiRootInductionKindV3::OriginalV1,
                2 => MultiRootInductionKindV3::ExpandedV2,
                _ => return Err(E::InductionKind),
            },
        })
    }

    /// Returns the complete fixed-size canonical record.
    pub const fn canonical_bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Returns inert bindings for mandatory comparison with source evidence.
    pub const fn inputs(&self) -> &MultiRootCorrespondenceInputsV3 {
        &self.inputs
    }
    /// No record or byte decoder grants independent semantic proof authority.
    pub const fn grants_authority(&self) -> bool {
        false
    }
}

fn identities(inputs: &MultiRootCorrespondenceInputsV3) -> [&[u8; 32]; 8] {
    [
        &inputs.semantic_mir_sha256,
        &inputs.correspondence_identity,
        &inputs.expansion_evidence_identity,
        &inputs.semantic_root_identity,
        &inputs.selected_body_identity,
        &inputs.execution_view_identity,
        &inputs.execution_function_identity,
        &inputs.induction_identity,
    ]
}

fn validate(
    inputs: &MultiRootCorrespondenceInputsV3,
) -> Result<(), MultiRootCorrespondencePayloadErrorV3> {
    if identities(inputs)
        .into_iter()
        .any(|identity| *identity == [0; 32])
    {
        return Err(MultiRootCorrespondencePayloadErrorV3::ZeroIdentity);
    }
    if (inputs.semantic_root == inputs.selected_body
        && inputs.semantic_root_identity != inputs.selected_body_identity)
        || (inputs.induction_kind == MultiRootInductionKindV3::OriginalV1
            && inputs.selected_body_identity != inputs.execution_function_identity)
    {
        return Err(MultiRootCorrespondencePayloadErrorV3::CoordinateMismatch);
    }
    Ok(())
}

/// Fail-closed fixed-record syntax errors; source authentication is a separate check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MultiRootCorrespondencePayloadErrorV3 {
    /// Truncated, extended, or otherwise non-exact record length.
    Length,
    /// Wrong magic, version, policy, length field, or reserved bytes.
    Header,
    /// A required source or evidence identity is zero.
    ZeroIdentity,
    /// Unknown induction coordinate-space tag.
    InductionKind,
    /// Original-coordinate identities contradict the declared index relation.
    CoordinateMismatch,
}
impl fmt::Display for MultiRootCorrespondencePayloadErrorV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "inert expanded correspondence root: {self:?}")
    }
}
impl Error for MultiRootCorrespondencePayloadErrorV3 {}

#[cfg(test)]
mod tests {
    use super::*;

    fn inputs() -> MultiRootCorrespondenceInputsV3 {
        MultiRootCorrespondenceInputsV3 {
            root_ordinal: 0,
            semantic_root: 2,
            selected_body: 3,
            kernel_function_ordinal: 1,
            semantic_mir_sha256: [1; 32],
            correspondence_identity: [2; 32],
            expansion_evidence_identity: [3; 32],
            semantic_root_identity: [4; 32],
            selected_body_identity: [5; 32],
            execution_view_identity: [6; 32],
            execution_function_identity: [7; 32],
            induction_identity: [8; 32],
            induction_kind: MultiRootInductionKindV3::ExpandedV2,
        }
    }

    #[test]
    fn expanded_root_reference_preserves_each_coordinate_without_authority() {
        let value = MultiRootCorrespondencePayloadV3::new(inputs()).unwrap();
        assert_eq!(
            MultiRootCorrespondencePayloadV3::decode(value.canonical_bytes()).unwrap(),
            value
        );
        assert_eq!(*value.inputs(), inputs());
        assert!(!value.grants_authority());
        assert!(crate::MultiRootCorrespondencePayloadV2::decode(value.canonical_bytes()).is_err());
        for offset in [16, 20, 24, 28, 32, 64, 96, 128, 160, 192, 224, 256] {
            let mut bytes = value.bytes;
            bytes[offset] ^= 0x40;
            let changed = MultiRootCorrespondencePayloadV3::decode(&bytes).unwrap();
            assert_ne!(changed.inputs(), value.inputs());
            assert!(!changed.grants_authority());
        }
    }

    #[test]
    fn mixed_root_original_induction_keeps_the_selected_body_identity() {
        let mut input = inputs();
        input.induction_kind = MultiRootInductionKindV3::OriginalV1;
        assert_eq!(
            MultiRootCorrespondencePayloadV3::new(input).unwrap_err(),
            MultiRootCorrespondencePayloadErrorV3::CoordinateMismatch
        );
        input.execution_function_identity = input.selected_body_identity;
        let value = MultiRootCorrespondencePayloadV3::new(input).unwrap();
        assert_eq!(
            MultiRootCorrespondencePayloadV3::decode(value.canonical_bytes()).unwrap(),
            value
        );
        input.semantic_root = input.selected_body;
        assert_eq!(
            MultiRootCorrespondencePayloadV3::new(input).unwrap_err(),
            MultiRootCorrespondencePayloadErrorV3::CoordinateMismatch
        );
    }

    #[test]
    fn root_reference_rejects_unknown_tags_zero_fields_and_noncanonical_bytes() {
        let value = MultiRootCorrespondencePayloadV3::new(inputs()).unwrap();
        for end in 0..value.bytes.len() {
            assert!(MultiRootCorrespondencePayloadV3::decode(&value.bytes[..end]).is_err());
        }
        let mut bytes = value.bytes.to_vec();
        bytes.push(0);
        assert!(MultiRootCorrespondencePayloadV3::decode(&bytes).is_err());
        for offset in [0, 8, 10, 12, 288, 289, 290, 291] {
            let mut bytes = value.bytes;
            bytes[offset] ^= 0x80;
            assert!(MultiRootCorrespondencePayloadV3::decode(&bytes).is_err());
        }
        for offset in (32..288).step_by(32) {
            let mut bytes = value.bytes;
            bytes[offset..offset + 32].fill(0);
            assert_eq!(
                MultiRootCorrespondencePayloadV3::decode(&bytes).unwrap_err(),
                MultiRootCorrespondencePayloadErrorV3::ZeroIdentity
            );
        }
    }
}
