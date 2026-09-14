//! Native graph-plus-catalog proof rosters. Legacy V2 headers remain distinct.

use super::*;
use crate::{
    InertNativeNeutralSubjectV1, NATIVE_NEUTRAL_SUBJECT_BYTES_V1, NativeNeutralSubjectErrorV1,
};

/// Native roster wire version; this does not extend the legacy KIR version enum.
pub const MULTI_ROOT_PROOF_ROSTER_VERSION_V3: u16 = 3;
/// Inert association policy for native rosters.
pub const MULTI_ROOT_PROOF_ROSTER_POLICY_V3: u16 = 1;
/// Native rosters admit singleton and multi-root compiler modules.
pub const MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3: usize = MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V2;
const HEADER_BYTES_V3: usize = 16 + 32 + NATIVE_NEUTRAL_SUBJECT_BYTES_V1 + 32;

/// Payload kinds retain the existing vocabulary, with distinct native wire magic.
pub type MultiRootProofRosterKindV3 = MultiRootProofRosterKindV2;
/// Canonical per-root fields are unchanged; the containing envelope is native V3.
pub type MultiRootProofRosterRootInputV3<'a> = MultiRootProofRosterRootInputV2<'a>;
/// Borrowed per-root fields from the exact retained native envelope.
pub type MultiRootProofRosterRootV3<'a> = MultiRootProofRosterRootV2<'a>;

/// Inputs to one native association-only roster.
#[derive(Clone, Copy, Debug)]
#[allow(missing_docs)]
pub struct MultiRootProofRosterInputsV3<'a> {
    pub kind: MultiRootProofRosterKindV3,
    pub semantic_mir_sha256: [u8; 32],
    pub native_neutral_subject: InertNativeNeutralSubjectV1,
    pub roster_identity: [u8; 32],
    pub canonical_kernel_order: &'a [u32],
    pub roots: &'a [MultiRootProofRosterRootInputV3<'a>],
}

/// Exact immutable native roster; decoding grants neither graph nor source custody.
#[derive(Debug)]
pub struct MultiRootProofRosterTranscriptV3 {
    canonical_bytes: Box<[u8]>,
    kind: MultiRootProofRosterKindV3,
    semantic_mir_sha256: [u8; 32],
    native_neutral_subject: InertNativeNeutralSubjectV1,
    roster_identity: [u8; 32],
    canonical_kernel_order: Box<[u32]>,
    roots: Box<[StoredRootV2]>,
}

impl MultiRootProofRosterTranscriptV3 {
    /// Constructs a bounded canonical native roster, preserving explicit root axes.
    /// This inert codec uses the shared receipt/row hard bounds, not a work ledger.
    pub fn new(
        inputs: MultiRootProofRosterInputsV3<'_>,
    ) -> Result<Self, MultiRootProofRosterErrorV3> {
        let count = inputs.roots.len();
        if !(1..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V3).contains(&count) {
            return Err(MultiRootProofRosterErrorV2::InvalidCount {
                field: "root count",
                observed: count,
            }
            .into());
        }
        let mut capacity = HEADER_BYTES_V3
            .checked_add(8)
            .and_then(|n| n.checked_add(inputs.canonical_kernel_order.len().checked_mul(4)?))
            .ok_or(MultiRootProofRosterErrorV2::LengthOverflow)?;
        for root in inputs.roots {
            capacity = capacity
                .checked_add(ROOT_FIXED_BYTES_V2)
                .ok_or(MultiRootProofRosterErrorV2::LengthOverflow)?;
            for length in [
                root.logical_name.len(),
                root.export_symbol.len(),
                root.kernel_id.len(),
                root.payload.len(),
            ] {
                capacity = capacity
                    .checked_add(4)
                    .and_then(|n| n.checked_add(length))
                    .ok_or(MultiRootProofRosterErrorV2::LengthOverflow)?;
            }
        }
        if capacity > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
            return Err(MultiRootProofRosterErrorV2::TooLarge {
                actual: capacity,
                max: MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
            }
            .into());
        }
        validate_root_inputs_v1(
            inputs.semantic_mir_sha256,
            inputs.roster_identity,
            inputs.canonical_kernel_order,
            inputs.roots,
            1,
        )?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| MultiRootProofRosterErrorV2::AllocationFailed)?;
        bytes.extend_from_slice(&native_magic(inputs.kind));
        bytes.extend_from_slice(&MULTI_ROOT_PROOF_ROSTER_VERSION_V3.to_le_bytes());
        bytes.extend_from_slice(&MULTI_ROOT_PROOF_ROSTER_POLICY_V3.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(capacity)
                .map_err(|_| MultiRootProofRosterErrorV2::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&inputs.semantic_mir_sha256);
        bytes.extend_from_slice(inputs.native_neutral_subject.canonical_bytes());
        bytes.extend_from_slice(&inputs.roster_identity);
        push_count_v2(&mut bytes, inputs.canonical_kernel_order.len())?;
        for index in inputs.canonical_kernel_order {
            bytes.extend_from_slice(&index.to_le_bytes());
        }
        push_count_v2(&mut bytes, inputs.roots.len())?;
        for root in inputs.roots {
            bytes.extend_from_slice(&root.semantic_root.to_le_bytes());
            bytes.extend_from_slice(&root.semantic_root_identity);
            bytes.extend_from_slice(&root.kernel_binding);
            bytes.push(root.source_rank);
            bytes.extend_from_slice(&[0; 3]);
            for dimension in root.workgroup {
                bytes.extend_from_slice(&dimension.to_le_bytes());
            }
            push_bytes_v2(&mut bytes, root.logical_name.as_bytes())?;
            push_bytes_v2(&mut bytes, root.export_symbol.as_bytes())?;
            push_bytes_v2(&mut bytes, root.kernel_id.as_bytes())?;
            push_bytes_v2(&mut bytes, root.payload)?;
        }
        if bytes.len() != capacity {
            return Err(MultiRootProofRosterErrorV2::LengthOverflow.into());
        }
        Self::decode_owned(bytes)
    }

    /// Strictly decodes native V3 only; V2 magic or legacy-neutral bodies reject.
    pub fn decode(bytes: &[u8]) -> Result<Self, MultiRootProofRosterErrorV3> {
        if bytes.len() > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
            return Err(MultiRootProofRosterErrorV2::TooLarge {
                actual: bytes.len(),
                max: MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
            }
            .into());
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| MultiRootProofRosterErrorV2::AllocationFailed)?;
        owned.extend_from_slice(bytes);
        Self::decode_owned(owned)
    }

    fn decode_owned(bytes: Vec<u8>) -> Result<Self, MultiRootProofRosterErrorV3> {
        let mut reader = ReaderV2::new(&bytes);
        let kind = native_kind(reader.fixed::<8>()?)?;
        let version = reader.u16()?;
        if version != MULTI_ROOT_PROOF_ROSTER_VERSION_V3 {
            return Err(
                MultiRootProofRosterErrorV2::UnsupportedVersion { observed: version }.into(),
            );
        }
        let policy = reader.u16()?;
        if policy != MULTI_ROOT_PROOF_ROSTER_POLICY_V3 {
            return Err(MultiRootProofRosterErrorV2::WrongPolicy { observed: policy }.into());
        }
        let declared = reader.u32()? as usize;
        if declared != bytes.len() {
            return Err(MultiRootProofRosterErrorV2::DeclaredLengthMismatch {
                declared,
                actual: bytes.len(),
            }
            .into());
        }
        let semantic_mir_sha256 = reader.fixed::<32>()?;
        require_nonzero_v2("semantic MIR", semantic_mir_sha256)?;
        let native_neutral_subject =
            InertNativeNeutralSubjectV1::decode(reader.take(NATIVE_NEUTRAL_SUBJECT_BYTES_V1)?)?;
        let roster_identity = reader.fixed::<32>()?;
        require_nonzero_v2("compiler roster", roster_identity)?;
        let (canonical_kernel_order, roots) = decode_root_rows_v1(&mut reader, 1)?;
        Ok(Self {
            canonical_bytes: bytes.into_boxed_slice(),
            kind,
            semantic_mir_sha256,
            native_neutral_subject,
            roster_identity,
            canonical_kernel_order,
            roots,
        })
    }

    /// Returns complete canonical native roster bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }
    /// Transfers complete native bytes without re-encoding a legacy schema.
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes.into_vec()
    }
    /// Returns the nested payload kind.
    pub const fn kind(&self) -> MultiRootProofRosterKindV3 {
        self.kind
    }
    /// Returns the semantic MIR identity shared by every root.
    pub const fn semantic_mir_sha256(&self) -> [u8; 32] {
        self.semantic_mir_sha256
    }
    /// Returns both exact native constituent identities, not merely the graph hash.
    pub const fn native_neutral_subject(&self) -> &InertNativeNeutralSubjectV1 {
        &self.native_neutral_subject
    }
    /// Returns the independently replayable compiler roster identity.
    pub const fn roster_identity(&self) -> [u8; 32] {
        self.roster_identity
    }
    /// Maps canonical KernelId ordinal to semantic-root roster ordinal.
    pub fn canonical_kernel_order(&self) -> &[u32] {
        &self.canonical_kernel_order
    }
    /// Returns the number of source roots, including an admitted singleton.
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }
    /// Returns one root by sorted semantic-root ordinal, not canonical KernelId ordinal.
    pub fn root(&self, index: usize) -> Option<MultiRootProofRosterRootV3<'_>> {
        let root = self.roots.get(index)?;
        Some(MultiRootProofRosterRootV2 {
            semantic_root: root.semantic_root,
            semantic_root_identity: root.semantic_root_identity,
            kernel_binding: root.kernel_binding,
            source_rank: root.source_rank,
            workgroup: root.workgroup,
            logical_name: self.text(root.logical_name),
            export_symbol: self.text(root.export_symbol),
            kernel_id: self.text(root.kernel_id),
            payload: &self.canonical_bytes[root.payload.start..root.payload.end],
        })
    }
    fn text(&self, range: ByteRangeV2) -> &str {
        str::from_utf8(&self.canonical_bytes[range.start..range.end])
            .expect("native roster decoder checked UTF-8")
    }
    /// Reports the deliberately limited association-only claim.
    pub const fn claim(&self) -> TargetLineageClaimV3 {
        TargetLineageClaimV3::AssociationOnlyNoRefinementProof
    }
    /// Native framing establishes no compiler refinement.
    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }
}

const fn native_magic(kind: MultiRootProofRosterKindV3) -> [u8; 8] {
    match kind {
        MultiRootProofRosterKindV3::MiddleEnd => *b"F2MRMID3",
        MultiRootProofRosterKindV3::Correspondence => *b"F2MRCOR3",
        MultiRootProofRosterKindV3::FormalMemory => *b"F2MRFOR3",
        MultiRootProofRosterKindV3::VerusExecution => *b"F2MRVER3",
    }
}
fn native_kind(magic: [u8; 8]) -> Result<MultiRootProofRosterKindV3, MultiRootProofRosterErrorV2> {
    [
        MultiRootProofRosterKindV3::MiddleEnd,
        MultiRootProofRosterKindV3::Correspondence,
        MultiRootProofRosterKindV3::FormalMemory,
        MultiRootProofRosterKindV3::VerusExecution,
    ]
    .into_iter()
    .find(|kind| native_magic(*kind) == magic)
    .ok_or(MultiRootProofRosterErrorV2::InvalidMagic)
}

/// Native-header failure or unchanged canonical root-record rejection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MultiRootProofRosterErrorV3 {
    /// Existing shared canonical row/framing rules rejected the input.
    Framing(MultiRootProofRosterErrorV2),
    /// Exact native graph-plus-catalog subject framing failed.
    Subject(NativeNeutralSubjectErrorV1),
}
impl From<MultiRootProofRosterErrorV2> for MultiRootProofRosterErrorV3 {
    fn from(error: MultiRootProofRosterErrorV2) -> Self {
        Self::Framing(error)
    }
}
impl From<NativeNeutralSubjectErrorV1> for MultiRootProofRosterErrorV3 {
    fn from(error: NativeNeutralSubjectErrorV1) -> Self {
        Self::Subject(error)
    }
}
impl fmt::Display for MultiRootProofRosterErrorV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Framing(error) => error.fmt(f),
            Self::Subject(error) => error.fmt(f),
        }
    }
}
impl Error for MultiRootProofRosterErrorV3 {}

#[cfg(test)]
#[path = "multi_root_proof_roster_v3_tests.rs"]
mod tests;
