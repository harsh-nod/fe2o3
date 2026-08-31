use std::{collections::BTreeSet, error::Error, fmt, ops::Range, str};

use fe2o3_kernel_descriptor::{KernelId as DescriptorKernelId, MAX_KERNELS, MAX_NAME_BYTES};

use crate::{MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3, TargetLineageClaimV3};

/// Wire version for every canonical multi-root proof-roster envelope.
pub const MULTI_ROOT_PROOF_ROSTER_VERSION_V2: u16 = 2;
/// Association-only policy for every canonical multi-root proof-roster envelope.
pub const MULTI_ROOT_PROOF_ROSTER_POLICY_V2: u16 = 1;
/// Maximum roots in one canonical multi-root proof roster.
pub const MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V2: usize = MAX_KERNELS;

const MIDDLE_END_MAGIC_V2: [u8; 8] = *b"F2MRMID2";
const CORRESPONDENCE_MAGIC_V2: [u8; 8] = *b"F2MRCOR2";
const FORMAL_MEMORY_MAGIC_V2: [u8; 8] = *b"F2MRFOR2";
const VERUS_EXECUTION_MAGIC_V2: [u8; 8] = *b"F2MRVER2";
const HEADER_BYTES_V2: usize = 124;
const ROOT_FIXED_BYTES_V2: usize = 84;
const MAX_LOGICAL_NAME_BYTES_V2: usize = 512;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Exact nested payload represented by one proof-roster envelope.
pub enum MultiRootProofRosterKindV2 {
    /// Per-root production middle-end evidence.
    MiddleEnd,
    /// Per-root lossless MIR-to-KIR correspondence evidence.
    Correspondence,
    /// Per-root formal-memory obligation evidence.
    FormalMemory,
    /// Per-root signed Verus execution evidence.
    VerusExecution,
}

impl MultiRootProofRosterKindV2 {
    const fn magic(self) -> [u8; 8] {
        match self {
            Self::MiddleEnd => MIDDLE_END_MAGIC_V2,
            Self::Correspondence => CORRESPONDENCE_MAGIC_V2,
            Self::FormalMemory => FORMAL_MEMORY_MAGIC_V2,
            Self::VerusExecution => VERUS_EXECUTION_MAGIC_V2,
        }
    }

    fn from_magic(magic: [u8; 8]) -> Result<Self, MultiRootProofRosterErrorV2> {
        match magic {
            MIDDLE_END_MAGIC_V2 => Ok(Self::MiddleEnd),
            CORRESPONDENCE_MAGIC_V2 => Ok(Self::Correspondence),
            FORMAL_MEMORY_MAGIC_V2 => Ok(Self::FormalMemory),
            VERUS_EXECUTION_MAGIC_V2 => Ok(Self::VerusExecution),
            _ => Err(MultiRootProofRosterErrorV2::InvalidMagic),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Canonical neutral Kernel IR version admitted by a multi-root roster.
pub enum MultiRootCanonicalKirVersionV2 {
    /// Canonical Kernel IR V8.
    V8,
    /// Canonical Kernel IR V9.
    V9,
}

impl MultiRootCanonicalKirVersionV2 {
    /// Returns the exact wire version number.
    pub const fn wire_version(self) -> u16 {
        match self {
            Self::V8 => 8,
            Self::V9 => 9,
        }
    }

    fn from_wire_version(version: u16) -> Result<Self, MultiRootProofRosterErrorV2> {
        match version {
            8 => Ok(Self::V8),
            9 => Ok(Self::V9),
            _ => Err(MultiRootProofRosterErrorV2::InvalidKernelIrVersion { observed: version }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
/// Exact canonical target-neutral Kernel IR coordinates shared by every root.
pub struct MultiRootNeutralKirIdentityV2 {
    version: MultiRootCanonicalKirVersionV2,
    canonical_length: u64,
    digest: [u8; 32],
}

impl MultiRootNeutralKirIdentityV2 {
    /// Constructs validated nonzero canonical Kernel IR coordinates.
    pub fn new(
        version: MultiRootCanonicalKirVersionV2,
        canonical_length: u64,
        digest: [u8; 32],
    ) -> Result<Self, MultiRootProofRosterErrorV2> {
        if canonical_length == 0 {
            return Err(MultiRootProofRosterErrorV2::ZeroLength {
                field: "target-neutral Kernel IR",
            });
        }
        if digest == [0; 32] {
            return Err(MultiRootProofRosterErrorV2::ZeroIdentity {
                field: "target-neutral Kernel IR",
            });
        }
        Ok(Self {
            version,
            canonical_length,
            digest,
        })
    }

    /// Returns the exact canonical Kernel IR version.
    pub const fn version(self) -> MultiRootCanonicalKirVersionV2 {
        self.version
    }

    /// Returns the exact canonical Kernel IR byte length.
    pub const fn canonical_length(self) -> u64 {
        self.canonical_length
    }

    /// Returns the exact canonical Kernel IR SHA-256 digest.
    pub const fn digest(self) -> [u8; 32] {
        self.digest
    }
}

#[derive(Clone, Copy, Debug)]
/// One borrowed semantic-root record supplied to the canonical roster encoder.
#[allow(missing_docs)]
pub struct MultiRootProofRosterRootInputV2<'a> {
    pub semantic_root: u32,
    pub semantic_root_identity: [u8; 32],
    pub kernel_binding: [u8; 32],
    pub source_rank: u8,
    pub workgroup: [u32; 3],
    pub logical_name: &'a str,
    pub export_symbol: &'a str,
    pub kernel_id: &'a str,
    pub payload: &'a [u8],
}

#[derive(Clone, Copy, Debug)]
/// Borrowed inputs for one canonical multi-root proof-roster envelope.
#[allow(missing_docs)]
pub struct MultiRootProofRosterInputsV2<'a> {
    pub kind: MultiRootProofRosterKindV2,
    pub semantic_mir_sha256: [u8; 32],
    pub neutral_kir: MultiRootNeutralKirIdentityV2,
    pub roster_identity: [u8; 32],
    pub canonical_kernel_order: &'a [u32],
    pub roots: &'a [MultiRootProofRosterRootInputV2<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ByteRangeV2 {
    start: usize,
    end: usize,
}

impl From<Range<usize>> for ByteRangeV2 {
    fn from(range: Range<usize>) -> Self {
        Self {
            start: range.start,
            end: range.end,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct StoredRootV2 {
    semantic_root: u32,
    semantic_root_identity: [u8; 32],
    kernel_binding: [u8; 32],
    source_rank: u8,
    workgroup: [u32; 3],
    logical_name: ByteRangeV2,
    export_symbol: ByteRangeV2,
    kernel_id: ByteRangeV2,
    payload: ByteRangeV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// One decoded semantic-root record borrowed from a canonical proof roster.
pub struct MultiRootProofRosterRootV2<'a> {
    semantic_root: u32,
    semantic_root_identity: [u8; 32],
    kernel_binding: [u8; 32],
    source_rank: u8,
    workgroup: [u32; 3],
    logical_name: &'a str,
    export_symbol: &'a str,
    kernel_id: &'a str,
    payload: &'a [u8],
}

impl<'a> MultiRootProofRosterRootV2<'a> {
    /// Returns the canonical semantic-function root index.
    pub const fn semantic_root(self) -> u32 {
        self.semantic_root
    }

    /// Returns the semantic-function identity.
    pub const fn semantic_root_identity(self) -> [u8; 32] {
        self.semantic_root_identity
    }

    /// Returns the semantic kernel-binding identity.
    pub const fn kernel_binding(self) -> [u8; 32] {
        self.kernel_binding
    }

    /// Returns the source launch rank.
    pub const fn source_rank(self) -> u8 {
        self.source_rank
    }

    /// Returns the exact target-bound default workgroup.
    pub const fn workgroup(self) -> [u32; 3] {
        self.workgroup
    }

    /// Returns the diagnostic logical kernel name.
    pub const fn logical_name(self) -> &'a str {
        self.logical_name
    }

    /// Returns the exact semantic export symbol.
    pub const fn export_symbol(self) -> &'a str {
        self.export_symbol
    }

    /// Returns the exact Kernel IR kernel identifier.
    pub const fn kernel_id(self) -> &'a str {
        self.kernel_id
    }

    /// Returns the exact opaque per-root payload bytes.
    pub const fn payload(self) -> &'a [u8] {
        self.payload
    }
}

#[derive(Debug, Eq, PartialEq)]
/// Canonical, bounded multi-root proof-roster envelope.
pub struct MultiRootProofRosterTranscriptV2 {
    canonical_bytes: Box<[u8]>,
    kind: MultiRootProofRosterKindV2,
    semantic_mir_sha256: [u8; 32],
    neutral_kir: MultiRootNeutralKirIdentityV2,
    roster_identity: [u8; 32],
    canonical_kernel_order: Box<[u32]>,
    roots: Box<[StoredRootV2]>,
}

impl MultiRootProofRosterTranscriptV2 {
    /// Constructs and revalidates one exact canonical proof-roster envelope.
    pub fn new(
        inputs: MultiRootProofRosterInputsV2<'_>,
    ) -> Result<Self, MultiRootProofRosterErrorV2> {
        validate_inputs_v2(&inputs)?;
        let mut capacity = HEADER_BYTES_V2
            .checked_add(4)
            .and_then(|value| {
                value.checked_add(inputs.canonical_kernel_order.len().checked_mul(4)?)
            })
            .and_then(|value| value.checked_add(4))
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
                    .and_then(|value| value.checked_add(length))
                    .ok_or(MultiRootProofRosterErrorV2::LengthOverflow)?;
            }
        }
        if capacity > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
            return Err(MultiRootProofRosterErrorV2::TooLarge {
                actual: capacity,
                max: MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
            });
        }

        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|_| MultiRootProofRosterErrorV2::AllocationFailed)?;
        bytes.extend_from_slice(&inputs.kind.magic());
        bytes.extend_from_slice(&MULTI_ROOT_PROOF_ROSTER_VERSION_V2.to_le_bytes());
        bytes.extend_from_slice(&MULTI_ROOT_PROOF_ROSTER_POLICY_V2.to_le_bytes());
        bytes.extend_from_slice(
            &u32::try_from(capacity)
                .map_err(|_| MultiRootProofRosterErrorV2::LengthOverflow)?
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&inputs.semantic_mir_sha256);
        bytes.extend_from_slice(&inputs.neutral_kir.version().wire_version().to_le_bytes());
        bytes.extend_from_slice(&0_u16.to_le_bytes());
        bytes.extend_from_slice(&inputs.neutral_kir.canonical_length().to_le_bytes());
        bytes.extend_from_slice(&inputs.neutral_kir.digest());
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
        debug_assert_eq!(bytes.len(), capacity);
        Self::decode_owned(bytes)
    }

    /// Strictly decodes, bounds, and revalidates one untrusted envelope.
    pub fn decode(bytes: &[u8]) -> Result<Self, MultiRootProofRosterErrorV2> {
        if bytes.len() > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
            return Err(MultiRootProofRosterErrorV2::TooLarge {
                actual: bytes.len(),
                max: MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
            });
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(bytes.len())
            .map_err(|_| MultiRootProofRosterErrorV2::AllocationFailed)?;
        owned.extend_from_slice(bytes);
        Self::decode_owned(owned)
    }

    fn decode_owned(bytes: Vec<u8>) -> Result<Self, MultiRootProofRosterErrorV2> {
        let decoded = DecodedRosterV2::decode(&bytes)?;
        Ok(Self {
            canonical_bytes: bytes.into_boxed_slice(),
            kind: decoded.kind,
            semantic_mir_sha256: decoded.semantic_mir_sha256,
            neutral_kir: decoded.neutral_kir,
            roster_identity: decoded.roster_identity,
            canonical_kernel_order: decoded.canonical_kernel_order,
            roots: decoded.roots,
        })
    }

    /// Returns the exact canonical envelope bytes.
    pub fn canonical_bytes(&self) -> &[u8] {
        &self.canonical_bytes
    }

    /// Transfers the exact canonical envelope bytes without copying them.
    pub fn into_canonical_bytes(self) -> Vec<u8> {
        self.canonical_bytes.into_vec()
    }

    /// Returns the nested payload kind.
    pub const fn kind(&self) -> MultiRootProofRosterKindV2 {
        self.kind
    }

    /// Returns the shared canonical semantic MIR digest.
    pub const fn semantic_mir_sha256(&self) -> [u8; 32] {
        self.semantic_mir_sha256
    }

    /// Returns the shared target-neutral canonical Kernel IR coordinates.
    pub const fn neutral_kir(&self) -> MultiRootNeutralKirIdentityV2 {
        self.neutral_kir
    }

    /// Returns the canonical compiler-roster identity.
    pub const fn roster_identity(&self) -> [u8; 32] {
        self.roster_identity
    }

    /// Returns the exact KernelId-derived root permutation.
    pub fn canonical_kernel_order(&self) -> &[u32] {
        &self.canonical_kernel_order
    }

    /// Returns the number of semantic roots.
    pub fn root_count(&self) -> usize {
        self.roots.len()
    }

    /// Returns one root by canonical semantic-root ordinal.
    pub fn root(&self, index: usize) -> Option<MultiRootProofRosterRootV2<'_>> {
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
            payload: self.bytes(root.payload),
        })
    }

    /// Returns the deliberately limited semantic claim carried by this envelope.
    pub const fn claim(&self) -> TargetLineageClaimV3 {
        TargetLineageClaimV3::AssociationOnlyNoRefinementProof
    }

    /// Reports that the envelope itself establishes no compiler refinement.
    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    fn text(&self, range: ByteRangeV2) -> &str {
        str::from_utf8(self.bytes(range)).expect("strict decoder retained canonical ASCII text")
    }

    fn bytes(&self, range: ByteRangeV2) -> &[u8] {
        &self.canonical_bytes[range.start..range.end]
    }
}

struct DecodedRosterV2 {
    kind: MultiRootProofRosterKindV2,
    semantic_mir_sha256: [u8; 32],
    neutral_kir: MultiRootNeutralKirIdentityV2,
    roster_identity: [u8; 32],
    canonical_kernel_order: Box<[u32]>,
    roots: Box<[StoredRootV2]>,
}

impl DecodedRosterV2 {
    fn decode(bytes: &[u8]) -> Result<Self, MultiRootProofRosterErrorV2> {
        if bytes.len() > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3 {
            return Err(MultiRootProofRosterErrorV2::TooLarge {
                actual: bytes.len(),
                max: MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
            });
        }
        let mut reader = ReaderV2::new(bytes);
        let kind = MultiRootProofRosterKindV2::from_magic(reader.fixed::<8>()?)?;
        let version = reader.u16()?;
        if version != MULTI_ROOT_PROOF_ROSTER_VERSION_V2 {
            return Err(MultiRootProofRosterErrorV2::UnsupportedVersion { observed: version });
        }
        let policy = reader.u16()?;
        if policy != MULTI_ROOT_PROOF_ROSTER_POLICY_V2 {
            return Err(MultiRootProofRosterErrorV2::WrongPolicy { observed: policy });
        }
        let declared = reader.u32()? as usize;
        if declared != bytes.len() {
            return Err(MultiRootProofRosterErrorV2::DeclaredLengthMismatch {
                declared,
                actual: bytes.len(),
            });
        }
        let semantic_mir_sha256 = reader.fixed::<32>()?;
        require_nonzero_v2("semantic MIR", semantic_mir_sha256)?;
        let kir_version = MultiRootCanonicalKirVersionV2::from_wire_version(reader.u16()?)?;
        if reader.u16()? != 0 {
            return Err(MultiRootProofRosterErrorV2::NonZeroReserved {
                field: "Kernel IR header",
            });
        }
        let neutral_kir =
            MultiRootNeutralKirIdentityV2::new(kir_version, reader.u64()?, reader.fixed::<32>()?)?;
        let roster_identity = reader.fixed::<32>()?;
        require_nonzero_v2("compiler roster", roster_identity)?;

        let permutation_count = reader.bounded_root_count("KernelId permutation count")?;
        let mut canonical_kernel_order = Vec::new();
        canonical_kernel_order
            .try_reserve_exact(permutation_count)
            .map_err(|_| MultiRootProofRosterErrorV2::AllocationFailed)?;
        for _ in 0..permutation_count {
            canonical_kernel_order.push(reader.u32()?);
        }
        validate_permutation_v2(&canonical_kernel_order, permutation_count)?;

        let root_count = reader.bounded_root_count("root count")?;
        if root_count != permutation_count {
            return Err(MultiRootProofRosterErrorV2::CountMismatch {
                field: "root and KernelId permutation counts",
            });
        }
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(root_count)
            .map_err(|_| MultiRootProofRosterErrorV2::AllocationFailed)?;
        let mut semantic_identities = BTreeSet::new();
        let mut bindings = BTreeSet::new();
        let mut logical_names = BTreeSet::new();
        let mut exports = BTreeSet::new();
        let mut kernels = BTreeSet::new();
        let mut previous_root = None;
        for _ in 0..root_count {
            let semantic_root = reader.u32()?;
            if previous_root.is_some_and(|previous| semantic_root <= previous) {
                return Err(MultiRootProofRosterErrorV2::NonCanonicalRootOrder);
            }
            previous_root = Some(semantic_root);
            let semantic_root_identity = reader.fixed::<32>()?;
            require_nonzero_v2("semantic root", semantic_root_identity)?;
            if !semantic_identities.insert(semantic_root_identity) {
                return Err(MultiRootProofRosterErrorV2::DuplicateRootField {
                    field: "semantic root identity",
                });
            }
            let kernel_binding = reader.fixed::<32>()?;
            require_nonzero_v2("kernel binding", kernel_binding)?;
            if !bindings.insert(kernel_binding) {
                return Err(MultiRootProofRosterErrorV2::DuplicateRootField {
                    field: "kernel binding",
                });
            }
            let source_rank = reader.u8()?;
            if !(1..=3).contains(&source_rank) {
                return Err(MultiRootProofRosterErrorV2::InvalidSourceRank {
                    observed: source_rank,
                });
            }
            if reader.fixed::<3>()? != [0; 3] {
                return Err(MultiRootProofRosterErrorV2::NonZeroReserved {
                    field: "root record",
                });
            }
            let workgroup = [reader.u32()?, reader.u32()?, reader.u32()?];
            if workgroup.contains(&0) {
                return Err(MultiRootProofRosterErrorV2::ZeroWorkgroup);
            }
            let logical_name = reader.text("logical name", MAX_LOGICAL_NAME_BYTES_V2)?;
            let export_symbol = reader.text("export symbol", MAX_NAME_BYTES)?;
            let kernel_id = reader.text("kernel ID", MAX_NAME_BYTES)?;
            let payload = reader.nonempty_bytes("root payload")?;
            let logical = reader.text_at(logical_name);
            let export = reader.text_at(export_symbol);
            let kernel = reader.text_at(kernel_id);
            if export != kernel {
                return Err(MultiRootProofRosterErrorV2::RootFieldMismatch {
                    field: "export symbol and kernel ID",
                });
            }
            if !logical_names.insert(logical) {
                return Err(MultiRootProofRosterErrorV2::DuplicateRootField {
                    field: "logical name",
                });
            }
            if !exports.insert(export) {
                return Err(MultiRootProofRosterErrorV2::DuplicateRootField {
                    field: "export symbol",
                });
            }
            if !kernels.insert(kernel) {
                return Err(MultiRootProofRosterErrorV2::DuplicateRootField { field: "kernel ID" });
            }
            roots.push(StoredRootV2 {
                semantic_root,
                semantic_root_identity,
                kernel_binding,
                source_rank,
                workgroup,
                logical_name,
                export_symbol,
                kernel_id,
                payload,
            });
        }
        if !reader.is_finished() {
            return Err(MultiRootProofRosterErrorV2::TrailingBytes {
                trailing: bytes.len() - reader.offset,
            });
        }
        validate_derived_kernel_order_v2(
            &canonical_kernel_order,
            roots.iter().map(|root| root.kernel_binding),
        )?;
        Ok(Self {
            kind,
            semantic_mir_sha256,
            neutral_kir,
            roster_identity,
            canonical_kernel_order: canonical_kernel_order.into_boxed_slice(),
            roots: roots.into_boxed_slice(),
        })
    }
}

struct ReaderV2<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> ReaderV2<'a> {
    const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], MultiRootProofRosterErrorV2> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(MultiRootProofRosterErrorV2::LengthOverflow)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(MultiRootProofRosterErrorV2::Truncated)?;
        self.offset = end;
        Ok(value)
    }

    fn fixed<const N: usize>(&mut self) -> Result<[u8; N], MultiRootProofRosterErrorV2> {
        self.take(N)?
            .try_into()
            .map_err(|_| MultiRootProofRosterErrorV2::Truncated)
    }

    fn u8(&mut self) -> Result<u8, MultiRootProofRosterErrorV2> {
        Ok(self.fixed::<1>()?[0])
    }

    fn u16(&mut self) -> Result<u16, MultiRootProofRosterErrorV2> {
        Ok(u16::from_le_bytes(self.fixed()?))
    }

    fn u32(&mut self) -> Result<u32, MultiRootProofRosterErrorV2> {
        Ok(u32::from_le_bytes(self.fixed()?))
    }

    fn u64(&mut self) -> Result<u64, MultiRootProofRosterErrorV2> {
        Ok(u64::from_le_bytes(self.fixed()?))
    }

    fn bounded_root_count(
        &mut self,
        field: &'static str,
    ) -> Result<usize, MultiRootProofRosterErrorV2> {
        let count = self.u32()? as usize;
        if !(2..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V2).contains(&count) {
            return Err(MultiRootProofRosterErrorV2::InvalidCount {
                field,
                observed: count,
            });
        }
        Ok(count)
    }

    fn framed_range(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<ByteRangeV2, MultiRootProofRosterErrorV2> {
        let length = self.u32()? as usize;
        if length == 0 {
            return Err(MultiRootProofRosterErrorV2::EmptyField { field });
        }
        if length > max {
            return Err(MultiRootProofRosterErrorV2::FieldTooLarge {
                field,
                actual: length,
                max,
            });
        }
        let start = self.offset;
        self.take(length)?;
        Ok((start..self.offset).into())
    }

    fn text(
        &mut self,
        field: &'static str,
        max: usize,
    ) -> Result<ByteRangeV2, MultiRootProofRosterErrorV2> {
        let range = self.framed_range(field, max)?;
        validate_utf8_text_v2(field, &self.bytes[range.start..range.end])?;
        Ok(range)
    }

    fn nonempty_bytes(
        &mut self,
        field: &'static str,
    ) -> Result<ByteRangeV2, MultiRootProofRosterErrorV2> {
        self.framed_range(field, MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3)
    }

    fn text_at(&self, range: ByteRangeV2) -> &'a str {
        str::from_utf8(&self.bytes[range.start..range.end])
            .expect("strict reader retained canonical ASCII text")
    }

    fn is_finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}

fn validate_inputs_v2(
    inputs: &MultiRootProofRosterInputsV2<'_>,
) -> Result<(), MultiRootProofRosterErrorV2> {
    require_nonzero_v2("semantic MIR", inputs.semantic_mir_sha256)?;
    require_nonzero_v2("compiler roster", inputs.roster_identity)?;
    let root_count = inputs.roots.len();
    if !(2..=MAX_MULTI_ROOT_PROOF_ROSTER_ROOTS_V2).contains(&root_count) {
        return Err(MultiRootProofRosterErrorV2::InvalidCount {
            field: "root count",
            observed: root_count,
        });
    }
    if inputs.canonical_kernel_order.len() != root_count {
        return Err(MultiRootProofRosterErrorV2::CountMismatch {
            field: "root and KernelId permutation counts",
        });
    }
    validate_permutation_v2(inputs.canonical_kernel_order, root_count)?;

    let mut semantic_identities = BTreeSet::new();
    let mut bindings = BTreeSet::new();
    let mut logical_names = BTreeSet::new();
    let mut exports = BTreeSet::new();
    let mut kernels = BTreeSet::new();
    let mut previous_root = None;
    for root in inputs.roots {
        if previous_root.is_some_and(|previous| root.semantic_root <= previous) {
            return Err(MultiRootProofRosterErrorV2::NonCanonicalRootOrder);
        }
        previous_root = Some(root.semantic_root);
        require_nonzero_v2("semantic root", root.semantic_root_identity)?;
        require_nonzero_v2("kernel binding", root.kernel_binding)?;
        if !semantic_identities.insert(root.semantic_root_identity) {
            return Err(MultiRootProofRosterErrorV2::DuplicateRootField {
                field: "semantic root identity",
            });
        }
        if !bindings.insert(root.kernel_binding) {
            return Err(MultiRootProofRosterErrorV2::DuplicateRootField {
                field: "kernel binding",
            });
        }
        if !(1..=3).contains(&root.source_rank) {
            return Err(MultiRootProofRosterErrorV2::InvalidSourceRank {
                observed: root.source_rank,
            });
        }
        if root.workgroup.contains(&0) {
            return Err(MultiRootProofRosterErrorV2::ZeroWorkgroup);
        }
        validate_bounded_utf8_text_v2(
            "logical name",
            root.logical_name,
            MAX_LOGICAL_NAME_BYTES_V2,
        )?;
        validate_bounded_utf8_text_v2("export symbol", root.export_symbol, MAX_NAME_BYTES)?;
        validate_bounded_utf8_text_v2("kernel ID", root.kernel_id, MAX_NAME_BYTES)?;
        if root.export_symbol != root.kernel_id {
            return Err(MultiRootProofRosterErrorV2::RootFieldMismatch {
                field: "export symbol and kernel ID",
            });
        }
        if root.payload.is_empty() {
            return Err(MultiRootProofRosterErrorV2::EmptyField {
                field: "root payload",
            });
        }
        if !logical_names.insert(root.logical_name) {
            return Err(MultiRootProofRosterErrorV2::DuplicateRootField {
                field: "logical name",
            });
        }
        if !exports.insert(root.export_symbol) {
            return Err(MultiRootProofRosterErrorV2::DuplicateRootField {
                field: "export symbol",
            });
        }
        if !kernels.insert(root.kernel_id) {
            return Err(MultiRootProofRosterErrorV2::DuplicateRootField { field: "kernel ID" });
        }
    }
    validate_derived_kernel_order_v2(
        inputs.canonical_kernel_order,
        inputs.roots.iter().map(|root| root.kernel_binding),
    )
}

fn validate_permutation_v2(
    permutation: &[u32],
    root_count: usize,
) -> Result<(), MultiRootProofRosterErrorV2> {
    let mut sorted = permutation.to_vec();
    sorted.sort_unstable();
    let expected = (0..root_count)
        .map(|index| u32::try_from(index).map_err(|_| MultiRootProofRosterErrorV2::LengthOverflow))
        .collect::<Result<Vec<_>, _>>()?;
    if sorted != expected {
        return Err(MultiRootProofRosterErrorV2::InvalidKernelOrder);
    }
    Ok(())
}

fn validate_derived_kernel_order_v2(
    permutation: &[u32],
    bindings: impl Iterator<Item = [u8; 32]>,
) -> Result<(), MultiRootProofRosterErrorV2> {
    let bindings = bindings.collect::<Vec<_>>();
    let mut derived = (0..bindings.len()).collect::<Vec<_>>();
    derived.sort_unstable_by_key(|index| DescriptorKernelId::from_bytes(bindings[*index]));
    let derived = derived
        .into_iter()
        .map(|index| u32::try_from(index).map_err(|_| MultiRootProofRosterErrorV2::LengthOverflow))
        .collect::<Result<Vec<_>, _>>()?;
    if permutation != derived {
        return Err(MultiRootProofRosterErrorV2::KernelOrderMismatch);
    }
    Ok(())
}

fn require_nonzero_v2(
    field: &'static str,
    value: [u8; 32],
) -> Result<(), MultiRootProofRosterErrorV2> {
    if value == [0; 32] {
        Err(MultiRootProofRosterErrorV2::ZeroIdentity { field })
    } else {
        Ok(())
    }
}

fn validate_bounded_utf8_text_v2(
    field: &'static str,
    text: &str,
    max: usize,
) -> Result<(), MultiRootProofRosterErrorV2> {
    if text.len() > max {
        return Err(MultiRootProofRosterErrorV2::FieldTooLarge {
            field,
            actual: text.len(),
            max,
        });
    }
    validate_utf8_text_v2(field, text.as_bytes())
}

fn validate_utf8_text_v2(
    field: &'static str,
    bytes: &[u8],
) -> Result<(), MultiRootProofRosterErrorV2> {
    let text =
        str::from_utf8(bytes).map_err(|_| MultiRootProofRosterErrorV2::InvalidText { field })?;
    if text.is_empty() {
        return Err(MultiRootProofRosterErrorV2::InvalidText { field });
    }
    Ok(())
}

fn push_count_v2(bytes: &mut Vec<u8>, count: usize) -> Result<(), MultiRootProofRosterErrorV2> {
    bytes.extend_from_slice(
        &u32::try_from(count)
            .map_err(|_| MultiRootProofRosterErrorV2::LengthOverflow)?
            .to_le_bytes(),
    );
    Ok(())
}

fn push_bytes_v2(bytes: &mut Vec<u8>, value: &[u8]) -> Result<(), MultiRootProofRosterErrorV2> {
    if value.is_empty() {
        return Err(MultiRootProofRosterErrorV2::EmptyField {
            field: "framed bytes",
        });
    }
    push_count_v2(bytes, value.len())?;
    bytes.extend_from_slice(value);
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
/// Failure while constructing or strictly decoding a multi-root proof roster.
#[allow(missing_docs)]
pub enum MultiRootProofRosterErrorV2 {
    AllocationFailed,
    TooLarge {
        actual: usize,
        max: usize,
    },
    LengthOverflow,
    Truncated,
    InvalidMagic,
    UnsupportedVersion {
        observed: u16,
    },
    WrongPolicy {
        observed: u16,
    },
    DeclaredLengthMismatch {
        declared: usize,
        actual: usize,
    },
    InvalidKernelIrVersion {
        observed: u16,
    },
    NonZeroReserved {
        field: &'static str,
    },
    ZeroIdentity {
        field: &'static str,
    },
    ZeroLength {
        field: &'static str,
    },
    InvalidCount {
        field: &'static str,
        observed: usize,
    },
    CountMismatch {
        field: &'static str,
    },
    InvalidKernelOrder,
    KernelOrderMismatch,
    NonCanonicalRootOrder,
    DuplicateRootField {
        field: &'static str,
    },
    InvalidSourceRank {
        observed: u8,
    },
    ZeroWorkgroup,
    EmptyField {
        field: &'static str,
    },
    FieldTooLarge {
        field: &'static str,
        actual: usize,
        max: usize,
    },
    InvalidText {
        field: &'static str,
    },
    RootFieldMismatch {
        field: &'static str,
    },
    TrailingBytes {
        trailing: usize,
    },
}

impl fmt::Display for MultiRootProofRosterErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllocationFailed => formatter.write_str("proof-roster allocation failed"),
            Self::TooLarge { actual, max } => {
                write!(
                    formatter,
                    "proof roster has {actual} bytes; maximum is {max}"
                )
            }
            Self::LengthOverflow => formatter.write_str("proof-roster length overflow"),
            Self::Truncated => formatter.write_str("truncated proof-roster envelope"),
            Self::InvalidMagic => formatter.write_str("invalid proof-roster magic"),
            Self::UnsupportedVersion { observed } => {
                write!(formatter, "unsupported proof-roster version {observed}")
            }
            Self::WrongPolicy { observed } => {
                write!(formatter, "unsupported proof-roster policy {observed}")
            }
            Self::DeclaredLengthMismatch { declared, actual } => write!(
                formatter,
                "proof roster declares {declared} bytes but contains {actual}"
            ),
            Self::InvalidKernelIrVersion { observed } => {
                write!(formatter, "unsupported proof-roster Kernel IR V{observed}")
            }
            Self::NonZeroReserved { field } => {
                write!(formatter, "proof-roster {field} has nonzero reserved bytes")
            }
            Self::ZeroIdentity { field } => {
                write!(formatter, "proof-roster {field} identity is zero")
            }
            Self::ZeroLength { field } => {
                write!(formatter, "proof-roster {field} length is zero")
            }
            Self::InvalidCount { field, observed } => {
                write!(
                    formatter,
                    "proof-roster {field} has invalid count {observed}"
                )
            }
            Self::CountMismatch { field } => write!(formatter, "proof-roster {field} differ"),
            Self::InvalidKernelOrder => {
                formatter.write_str("proof-roster KernelId order is not a permutation")
            }
            Self::KernelOrderMismatch => {
                formatter.write_str("proof-roster permutation differs from kernel-binding order")
            }
            Self::NonCanonicalRootOrder => {
                formatter.write_str("proof-roster semantic roots are not strictly ordered")
            }
            Self::DuplicateRootField { field } => {
                write!(formatter, "proof-roster {field} is duplicated")
            }
            Self::InvalidSourceRank { observed } => {
                write!(formatter, "proof-roster source rank {observed} is invalid")
            }
            Self::ZeroWorkgroup => formatter.write_str("proof-roster workgroup contains zero"),
            Self::EmptyField { field } => write!(formatter, "proof-roster {field} is empty"),
            Self::FieldTooLarge { field, actual, max } => write!(
                formatter,
                "proof-roster {field} has {actual} bytes; maximum is {max}"
            ),
            Self::InvalidText { field } => {
                write!(
                    formatter,
                    "proof-roster {field} is not canonical UTF-8 text"
                )
            }
            Self::RootFieldMismatch { field } => {
                write!(formatter, "proof-roster {field} mismatch")
            }
            Self::TrailingBytes { trailing } => {
                write!(formatter, "proof roster has {trailing} trailing bytes")
            }
        }
    }
}

impl Error for MultiRootProofRosterErrorV2 {}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    fn neutral() -> MultiRootNeutralKirIdentityV2 {
        MultiRootNeutralKirIdentityV2::new(MultiRootCanonicalKirVersionV2::V8, 4096, [3; 32])
            .unwrap()
    }

    fn roots() -> [MultiRootProofRosterRootInputV2<'static>; 2] {
        [
            MultiRootProofRosterRootInputV2 {
                semantic_root: 3,
                semantic_root_identity: [0x61; 32],
                kernel_binding: [0x72; 32],
                source_rank: 1,
                workgroup: [64, 1, 1],
                logical_name: "alpha",
                export_symbol: "alpha_kernel",
                kernel_id: "alpha_kernel",
                payload: b"alpha-payload",
            },
            MultiRootProofRosterRootInputV2 {
                semantic_root: 9,
                semantic_root_identity: [0x62; 32],
                kernel_binding: [0x71; 32],
                source_rank: 2,
                workgroup: [8, 8, 1],
                logical_name: "zeta",
                export_symbol: "zeta_kernel",
                kernel_id: "zeta_kernel",
                payload: b"zeta-payload",
            },
        ]
    }

    fn transcript(kind: MultiRootProofRosterKindV2) -> MultiRootProofRosterTranscriptV2 {
        let roots = roots();
        MultiRootProofRosterTranscriptV2::new(MultiRootProofRosterInputsV2 {
            kind,
            semantic_mir_sha256: [2; 32],
            neutral_kir: neutral(),
            roster_identity: [4; 32],
            canonical_kernel_order: &[1, 0],
            roots: &roots,
        })
        .unwrap()
    }

    #[test]
    fn every_kind_round_trips_with_exact_root_order() {
        for kind in [
            MultiRootProofRosterKindV2::MiddleEnd,
            MultiRootProofRosterKindV2::Correspondence,
            MultiRootProofRosterKindV2::FormalMemory,
            MultiRootProofRosterKindV2::VerusExecution,
        ] {
            let transcript = transcript(kind);
            let decoded =
                MultiRootProofRosterTranscriptV2::decode(transcript.canonical_bytes()).unwrap();
            assert_eq!(decoded, transcript);
            assert_eq!(decoded.kind(), kind);
            assert_eq!(decoded.semantic_mir_sha256(), [2; 32]);
            assert_eq!(decoded.neutral_kir(), neutral());
            assert_eq!(decoded.roster_identity(), [4; 32]);
            assert_eq!(decoded.canonical_kernel_order(), [1, 0]);
            assert_eq!(decoded.root_count(), 2);
            assert_eq!(decoded.root(0).unwrap().semantic_root(), 3);
            assert_eq!(decoded.root(0).unwrap().kernel_id(), "alpha_kernel");
            assert_eq!(decoded.root(0).unwrap().payload(), b"alpha-payload");
            assert_eq!(decoded.root(1).unwrap().semantic_root(), 9);
            assert_eq!(decoded.root(1).unwrap().kernel_id(), "zeta_kernel");
            assert_eq!(
                decoded.claim(),
                TargetLineageClaimV3::AssociationOnlyNoRefinementProof
            );
            assert!(!decoded.establishes_compiler_refinement());
        }
    }

    #[test]
    fn frozen_middle_end_wire_has_stable_length_and_digest() {
        let transcript = transcript(MultiRootProofRosterKindV2::MiddleEnd);
        assert_eq!(transcript.canonical_bytes().len(), 420);
        assert_eq!(
            <[u8; 32]>::from(Sha256::digest(transcript.canonical_bytes())),
            [
                0x8c, 0x3e, 0x67, 0xe9, 0x37, 0x6c, 0x88, 0x26, 0xfa, 0xcd, 0x8d, 0xe2, 0xc8, 0xb9,
                0x1b, 0xc0, 0x92, 0xd1, 0xe1, 0x0d, 0xe8, 0x5c, 0xd9, 0x01, 0xda, 0x0b, 0x67, 0x44,
                0x09, 0x9a, 0x06, 0xac,
            ]
        );
    }

    #[test]
    fn every_prefix_and_trailing_byte_is_rejected() {
        let transcript = transcript(MultiRootProofRosterKindV2::Correspondence);
        for length in 0..transcript.canonical_bytes().len() {
            assert!(
                MultiRootProofRosterTranscriptV2::decode(&transcript.canonical_bytes()[..length])
                    .is_err(),
                "accepted truncated prefix of {length} bytes",
            );
        }
        let mut trailing = transcript.canonical_bytes().to_vec();
        trailing.push(0);
        let declared = u32::try_from(trailing.len()).unwrap();
        trailing[12..16].copy_from_slice(&declared.to_le_bytes());
        assert!(matches!(
            MultiRootProofRosterTranscriptV2::decode(&trailing),
            Err(MultiRootProofRosterErrorV2::TrailingBytes { trailing: 1 })
        ));
    }

    #[test]
    fn hostile_header_permutation_and_root_splices_are_rejected() {
        let transcript = transcript(MultiRootProofRosterKindV2::FormalMemory);
        let bytes = transcript.canonical_bytes();

        let mut wrong_version = bytes.to_vec();
        wrong_version[8..10].copy_from_slice(&3_u16.to_le_bytes());
        assert!(matches!(
            MultiRootProofRosterTranscriptV2::decode(&wrong_version),
            Err(MultiRootProofRosterErrorV2::UnsupportedVersion { observed: 3 })
        ));

        let mut wrong_reserved = bytes.to_vec();
        wrong_reserved[50] = 1;
        assert!(matches!(
            MultiRootProofRosterTranscriptV2::decode(&wrong_reserved),
            Err(MultiRootProofRosterErrorV2::NonZeroReserved { .. })
        ));

        let mut wrong_permutation = bytes.to_vec();
        wrong_permutation[128..132].copy_from_slice(&0_u32.to_le_bytes());
        wrong_permutation[132..136].copy_from_slice(&1_u32.to_le_bytes());
        assert!(matches!(
            MultiRootProofRosterTranscriptV2::decode(&wrong_permutation),
            Err(MultiRootProofRosterErrorV2::KernelOrderMismatch)
        ));

        let second_root = bytes
            .windows(b"zeta".len())
            .position(|window| window == b"zeta")
            .unwrap()
            - ROOT_FIXED_BYTES_V2
            - 4;
        let mut reordered = bytes.to_vec();
        reordered[second_root..second_root + 4].copy_from_slice(&2_u32.to_le_bytes());
        assert!(matches!(
            MultiRootProofRosterTranscriptV2::decode(&reordered),
            Err(MultiRootProofRosterErrorV2::NonCanonicalRootOrder)
        ));
    }

    #[test]
    fn construction_rejects_duplicate_and_cross_wired_root_fields() {
        let mut duplicate_roots = roots();
        duplicate_roots[1].kernel_binding = duplicate_roots[0].kernel_binding;
        assert!(matches!(
            MultiRootProofRosterTranscriptV2::new(MultiRootProofRosterInputsV2 {
                kind: MultiRootProofRosterKindV2::VerusExecution,
                semantic_mir_sha256: [2; 32],
                neutral_kir: neutral(),
                roster_identity: [4; 32],
                canonical_kernel_order: &[1, 0],
                roots: &duplicate_roots,
            }),
            Err(MultiRootProofRosterErrorV2::DuplicateRootField {
                field: "kernel binding"
            })
        ));

        let mut roots = roots();
        roots[1].export_symbol = "foreign_kernel";
        assert!(matches!(
            MultiRootProofRosterTranscriptV2::new(MultiRootProofRosterInputsV2 {
                kind: MultiRootProofRosterKindV2::VerusExecution,
                semantic_mir_sha256: [2; 32],
                neutral_kir: neutral(),
                roster_identity: [4; 32],
                canonical_kernel_order: &[1, 0],
                roots: &roots,
            }),
            Err(MultiRootProofRosterErrorV2::RootFieldMismatch { .. })
        ));
    }
}
