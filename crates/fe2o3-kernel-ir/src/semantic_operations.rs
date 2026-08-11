//! Versioned contracts shared by target-neutral semantic operation families.
//!
//! This module is deliberately separate from the module wire format. Kernel IR
//! V1 through V3 remain frozen. A later module wire version can carry an
//! operation family only after that family's strongly typed payload, semantic
//! instance identity, verifier, and lowering are implemented.

use std::collections::BTreeSet;
use std::error::Error;
use std::fmt;

use crate::{
    AccessMode, AddressSpace, Axis, IndexKind, IntrinsicKind, IntrinsicOperation, MemoryEffect,
    ScalarType, TargetCapability, Type, ValueDef, ValueId,
};

pub const SEMANTIC_OPERATION_SCHEMA_MAGIC_V1: [u8; 8] = *b"FE2O3SO\0";
pub const SEMANTIC_OPERATION_INSTANCE_MAGIC_V1: [u8; 8] = *b"FE2O3SI\0";
pub const SEMANTIC_OPERATION_VERSION_V1: u16 = 1;
pub const SEMANTIC_OPERATION_SCHEMA_BYTES_V1: usize = 16;
pub const SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1: usize = 20;
pub const MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1: usize = 4096;

/// Target-neutral semantic family. The family does not select a target dialect.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticOperationFamily {
    MemoryIntrinsic,
    Collective,
    Debug,
    /// Launch queries and declarative launch constraints.
    Launch,
    Matrix,
}

impl SemanticOperationFamily {
    const fn tag(self) -> u8 {
        match self {
            Self::MemoryIntrinsic => 1,
            Self::Collective => 2,
            Self::Debug => 3,
            Self::Launch => 4,
            Self::Matrix => 5,
        }
    }

    const fn from_tag(tag: u8) -> Option<Self> {
        match tag {
            1 => Some(Self::MemoryIntrinsic),
            2 => Some(Self::Collective),
            3 => Some(Self::Debug),
            4 => Some(Self::Launch),
            5 => Some(Self::Matrix),
            _ => None,
        }
    }
}

/// Registered operation opcode. Numeric values are scoped to a family.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticOperationKind {
    PointerDistance,
    VolatileLoad,
    VolatileStore,
    CopyNonOverlapping,
    LaunchInvocationIndex,
    LaunchExtent,
}

impl SemanticOperationKind {
    pub const fn family(self) -> SemanticOperationFamily {
        match self {
            Self::PointerDistance
            | Self::VolatileLoad
            | Self::VolatileStore
            | Self::CopyNonOverlapping => SemanticOperationFamily::MemoryIntrinsic,
            Self::LaunchInvocationIndex | Self::LaunchExtent => SemanticOperationFamily::Launch,
        }
    }

    const fn opcode(self) -> u16 {
        match self {
            Self::PointerDistance => 1,
            Self::VolatileLoad => 2,
            Self::VolatileStore => 3,
            Self::CopyNonOverlapping => 4,
            Self::LaunchInvocationIndex => 1,
            Self::LaunchExtent => 2,
        }
    }

    const fn from_parts(family: SemanticOperationFamily, opcode: u16) -> Option<Self> {
        match (family, opcode) {
            (SemanticOperationFamily::MemoryIntrinsic, 1) => Some(Self::PointerDistance),
            (SemanticOperationFamily::MemoryIntrinsic, 2) => Some(Self::VolatileLoad),
            (SemanticOperationFamily::MemoryIntrinsic, 3) => Some(Self::VolatileStore),
            (SemanticOperationFamily::MemoryIntrinsic, 4) => Some(Self::CopyNonOverlapping),
            (SemanticOperationFamily::Launch, 1) => Some(Self::LaunchInvocationIndex),
            (SemanticOperationFamily::Launch, 2) => Some(Self::LaunchExtent),
            _ => None,
        }
    }
}

/// Closed element-type set admitted by the first memory-intrinsic contract.
///
/// Aggregates, slices, and pointer-valued elements remain unsupported until
/// their layout and LLVM representation have independent frontend and backend
/// evidence.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryElementType {
    Unit,
    Scalar(ScalarType),
}

impl MemoryElementType {
    pub const fn ir_type(self) -> Type {
        match self {
            Self::Unit => Type::Unit,
            Self::Scalar(scalar) => Type::Scalar(scalar),
        }
    }

    pub const fn expected_layout(self) -> MemoryLayout {
        let (size_bytes, alignment_bytes) = match self {
            Self::Unit => (0, 1),
            Self::Scalar(ScalarType::Bool | ScalarType::I8 | ScalarType::U8) => (1, 1),
            Self::Scalar(
                ScalarType::I16 | ScalarType::U16 | ScalarType::F16 | ScalarType::Bf16,
            ) => (2, 2),
            Self::Scalar(ScalarType::I32 | ScalarType::U32 | ScalarType::F32) => (4, 4),
            Self::Scalar(
                ScalarType::I64 | ScalarType::U64 | ScalarType::Index | ScalarType::F64,
            ) => (8, 8),
            Self::Scalar(ScalarType::I128 | ScalarType::U128) => (16, 16),
        };
        MemoryLayout::new(size_bytes, alignment_bytes)
    }
}

/// Rust layout observed by the compiler bridge for one monomorphized pointee.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryLayout {
    pub size_bytes: u64,
    pub alignment_bytes: u32,
}

impl MemoryLayout {
    pub const fn new(size_bytes: u64, alignment_bytes: u32) -> Self {
        Self {
            size_bytes,
            alignment_bytes,
        }
    }

    /// Returns the byte count represented by an element count, failing closed
    /// when target-sized arithmetic would overflow.
    pub fn checked_byte_count(self, count: u64) -> Option<u64> {
        count.checked_mul(self.size_bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointerDistanceKind {
    Signed,
    Unsigned,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointerDistanceUnit {
    Elements,
    Bytes,
}

/// Provenance obligation for Rust pointer-distance operations.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointerDistanceProvenanceContract {
    EqualAddressesOrSameAllocation,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointerDistanceRangeContract {
    BothPointersInBoundsOrOnePastWhenAddressesDiffer,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointerDistanceDivisibilityContract {
    ExactMultipleOfDeclaredUnit,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointerDistanceOverflowContract {
    DifferenceFitsIsize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PointerDistanceOrderingContract {
    SignedMayBeNegative,
    UnsignedPointerAtOrAfterOrigin,
}

/// Closed obligation profile supported before pointer-distance LLVM
/// exact/no-wrap flags are valid. Equal addresses satisfy the first branch;
/// otherwise the same-allocation and in-bounds requirements both apply. These
/// fields are obligations, not source authority or proofs.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PointerDistanceContract {
    pub provenance: PointerDistanceProvenanceContract,
    pub range: PointerDistanceRangeContract,
    pub divisibility: PointerDistanceDivisibilityContract,
    pub overflow: PointerDistanceOverflowContract,
    pub ordering: PointerDistanceOrderingContract,
}

impl PointerDistanceContract {
    pub const fn supported_rust(kind: PointerDistanceKind) -> Self {
        Self {
            provenance: PointerDistanceProvenanceContract::EqualAddressesOrSameAllocation,
            range: PointerDistanceRangeContract::BothPointersInBoundsOrOnePastWhenAddressesDiffer,
            divisibility: PointerDistanceDivisibilityContract::ExactMultipleOfDeclaredUnit,
            overflow: PointerDistanceOverflowContract::DifferenceFitsIsize,
            ordering: match kind {
                PointerDistanceKind::Signed => PointerDistanceOrderingContract::SignedMayBeNegative,
                PointerDistanceKind::Unsigned => {
                    PointerDistanceOrderingContract::UnsignedPointerAtOrAfterOrigin
                }
            },
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum MemoryAlignmentContract {
    AlignedForElement,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VolatileProvenanceContract {
    RustAllocation,
    ExternalMmioNotRustAllocation,
    ZeroSizedNoAccess,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VolatileRangeContract {
    ReadableInitializedElement,
    WritableElement,
    ZeroSizedNoAccess,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VolatileTrapContract {
    NonTrapping,
    ZeroSizedNoAccess,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VolatileExternalEffectContract {
    NotExternal,
    SideEffectsDoNotModifyRustAllocatedMemory,
}

/// Closed volatile obligation profiles. Positive-sized accesses are either to
/// a Rust allocation or external MMIO. External accesses independently bind
/// that their side effects cannot modify Rust-allocated memory. The currently
/// representable `Unit` ZST requires alignment but is explicitly modeled as
/// performing no access; other ZST layouts remain unrepresentable.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VolatileAccessContract {
    pub provenance: VolatileProvenanceContract,
    pub range: VolatileRangeContract,
    pub alignment: MemoryAlignmentContract,
    pub trap: VolatileTrapContract,
    pub external_effect: VolatileExternalEffectContract,
}

impl VolatileAccessContract {
    pub const fn rust_allocation_load() -> Self {
        Self::load(VolatileProvenanceContract::RustAllocation)
    }

    pub const fn rust_allocation_store() -> Self {
        Self::store(VolatileProvenanceContract::RustAllocation)
    }

    pub const fn external_mmio_load() -> Self {
        Self::load(VolatileProvenanceContract::ExternalMmioNotRustAllocation)
    }

    pub const fn external_mmio_store() -> Self {
        Self::store(VolatileProvenanceContract::ExternalMmioNotRustAllocation)
    }

    pub const fn zero_sized_aligned_no_access() -> Self {
        Self {
            provenance: VolatileProvenanceContract::ZeroSizedNoAccess,
            range: VolatileRangeContract::ZeroSizedNoAccess,
            alignment: MemoryAlignmentContract::AlignedForElement,
            trap: VolatileTrapContract::ZeroSizedNoAccess,
            external_effect: VolatileExternalEffectContract::NotExternal,
        }
    }

    pub fn matches_supported_load(
        self,
        element: MemoryElementType,
        address_space: AddressSpace,
    ) -> bool {
        self.matches_supported_access(element, address_space, true)
    }

    pub fn matches_supported_store(
        self,
        element: MemoryElementType,
        address_space: AddressSpace,
    ) -> bool {
        self.matches_supported_access(element, address_space, false)
    }

    const fn load(provenance: VolatileProvenanceContract) -> Self {
        Self {
            provenance,
            range: VolatileRangeContract::ReadableInitializedElement,
            alignment: MemoryAlignmentContract::AlignedForElement,
            trap: VolatileTrapContract::NonTrapping,
            external_effect: match provenance {
                VolatileProvenanceContract::ExternalMmioNotRustAllocation => {
                    VolatileExternalEffectContract::SideEffectsDoNotModifyRustAllocatedMemory
                }
                VolatileProvenanceContract::RustAllocation
                | VolatileProvenanceContract::ZeroSizedNoAccess => {
                    VolatileExternalEffectContract::NotExternal
                }
            },
        }
    }

    const fn store(provenance: VolatileProvenanceContract) -> Self {
        Self {
            provenance,
            range: VolatileRangeContract::WritableElement,
            alignment: MemoryAlignmentContract::AlignedForElement,
            trap: VolatileTrapContract::NonTrapping,
            external_effect: match provenance {
                VolatileProvenanceContract::ExternalMmioNotRustAllocation => {
                    VolatileExternalEffectContract::SideEffectsDoNotModifyRustAllocatedMemory
                }
                VolatileProvenanceContract::RustAllocation
                | VolatileProvenanceContract::ZeroSizedNoAccess => {
                    VolatileExternalEffectContract::NotExternal
                }
            },
        }
    }

    fn matches_supported_access(
        self,
        element: MemoryElementType,
        address_space: AddressSpace,
        load: bool,
    ) -> bool {
        if element == MemoryElementType::Unit {
            return self == Self::zero_sized_aligned_no_access();
        }
        match self.provenance {
            VolatileProvenanceContract::RustAllocation => {
                self == if load {
                    Self::rust_allocation_load()
                } else {
                    Self::rust_allocation_store()
                }
            }
            VolatileProvenanceContract::ExternalMmioNotRustAllocation => {
                address_space == AddressSpace::Global
                    && self
                        == if load {
                            Self::external_mmio_load()
                        } else {
                            Self::external_mmio_store()
                        }
            }
            VolatileProvenanceContract::ZeroSizedNoAccess => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CopySourceContract {
    ReadableWhenBytesPositive,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CopyDestinationContract {
    WritableWhenBytesPositive,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CopyOverlapContract {
    NonOverlappingWhenBytesPositive,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CopyByteCountContract {
    CountTimesElementSizeFitsUsize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CopyAddressContract {
    SignedOffsetsFitIsizeAndStayInAllocationsWhenBytesPositive,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum CopyZeroByteContract {
    AlignmentRequiredRangesAndOverlapConditionalOnPositiveBytes,
}

/// Closed `copy_nonoverlapping` obligation profile. Alignment is required
/// even when the byte count is zero (including ZST copies); range validity and
/// non-overlap are required only when the computed byte count is positive.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CopyNonOverlappingContract {
    pub alignment: MemoryAlignmentContract,
    pub source: CopySourceContract,
    pub destination: CopyDestinationContract,
    pub overlap: CopyOverlapContract,
    pub byte_count: CopyByteCountContract,
    pub address: CopyAddressContract,
    pub zero_bytes: CopyZeroByteContract,
}

impl CopyNonOverlappingContract {
    pub const fn supported_rust() -> Self {
        Self {
            alignment: MemoryAlignmentContract::AlignedForElement,
            source: CopySourceContract::ReadableWhenBytesPositive,
            destination: CopyDestinationContract::WritableWhenBytesPositive,
            overlap: CopyOverlapContract::NonOverlappingWhenBytesPositive,
            byte_count: CopyByteCountContract::CountTimesElementSizeFitsUsize,
            address:
                CopyAddressContract::SignedOffsetsFitIsizeAndStayInAllocationsWhenBytesPositive,
            zero_bytes:
                CopyZeroByteContract::AlignmentRequiredRangesAndOverlapConditionalOnPositiveBytes,
        }
    }
}

/// Payload-blind schema key used only for operation dispatch and codec selection.
///
/// A schema does not distinguish axes, index levels, types, layouts, scopes, or
/// other operation payload. It must never be used as a proof, artifact, cache,
/// semantic-equivalence, or executable identity. Use
/// SemanticOperationInstanceId for those bindings.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticOperationSchema {
    version: u16,
    kind: SemanticOperationKind,
}

impl SemanticOperationSchema {
    pub const fn v1(kind: SemanticOperationKind) -> Self {
        Self {
            version: SEMANTIC_OPERATION_VERSION_V1,
            kind,
        }
    }

    pub const fn version(self) -> u16 {
        self.version
    }

    pub const fn family(self) -> SemanticOperationFamily {
        self.kind.family()
    }

    pub const fn kind(self) -> SemanticOperationKind {
        self.kind
    }
}

/// Encodes a payload-blind schema key in its fixed-width canonical form.
pub fn encode_semantic_operation_schema(
    schema: SemanticOperationSchema,
) -> [u8; SEMANTIC_OPERATION_SCHEMA_BYTES_V1] {
    let mut bytes = [0_u8; SEMANTIC_OPERATION_SCHEMA_BYTES_V1];
    bytes[..8].copy_from_slice(&SEMANTIC_OPERATION_SCHEMA_MAGIC_V1);
    bytes[8..10].copy_from_slice(&schema.version.to_le_bytes());
    bytes[10] = schema.family().tag();
    bytes[12..14].copy_from_slice(&schema.kind.opcode().to_le_bytes());
    bytes
}

/// Decodes a schema key and rejects unknown dispatch authority.
pub fn decode_semantic_operation_schema(
    bytes: &[u8],
) -> Result<SemanticOperationSchema, SemanticOperationSchemaDecodeError> {
    if bytes.len() < SEMANTIC_OPERATION_SCHEMA_BYTES_V1 {
        return Err(SemanticOperationSchemaDecodeError::Truncated {
            actual: bytes.len(),
        });
    }
    if bytes.len() > SEMANTIC_OPERATION_SCHEMA_BYTES_V1 {
        return Err(SemanticOperationSchemaDecodeError::TrailingBytes {
            actual: bytes.len(),
        });
    }
    if bytes[..8] != SEMANTIC_OPERATION_SCHEMA_MAGIC_V1 {
        return Err(SemanticOperationSchemaDecodeError::InvalidMagic);
    }

    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != SEMANTIC_OPERATION_VERSION_V1 {
        return Err(SemanticOperationSchemaDecodeError::UnknownVersion(version));
    }
    for offset in [11, 14, 15] {
        if bytes[offset] != 0 {
            return Err(SemanticOperationSchemaDecodeError::ReservedNonZero { offset });
        }
    }

    let family = SemanticOperationFamily::from_tag(bytes[10])
        .ok_or(SemanticOperationSchemaDecodeError::UnknownFamily(bytes[10]))?;
    let opcode = u16::from_le_bytes([bytes[12], bytes[13]]);
    let kind = SemanticOperationKind::from_parts(family, opcode)
        .ok_or(SemanticOperationSchemaDecodeError::UnknownOperation { family, opcode })?;
    Ok(SemanticOperationSchema { version, kind })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticOperationSchemaDecodeError {
    Truncated {
        actual: usize,
    },
    TrailingBytes {
        actual: usize,
    },
    InvalidMagic,
    UnknownVersion(u16),
    UnknownFamily(u8),
    UnknownOperation {
        family: SemanticOperationFamily,
        opcode: u16,
    },
    ReservedNonZero {
        offset: usize,
    },
}

impl fmt::Display for SemanticOperationSchemaDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { actual } | Self::TrailingBytes { actual } => write!(
                formatter,
                "semantic-operation schema has {actual} bytes; expected {SEMANTIC_OPERATION_SCHEMA_BYTES_V1}"
            ),
            Self::InvalidMagic => formatter.write_str("invalid semantic-operation schema magic"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unknown semantic-operation schema version {version}"
                )
            }
            Self::UnknownFamily(family) => {
                write!(formatter, "unknown semantic-operation family {family}")
            }
            Self::UnknownOperation { family, opcode } => write!(
                formatter,
                "unknown {family:?} semantic-operation opcode {opcode}"
            ),
            Self::ReservedNonZero { offset } => write!(
                formatter,
                "semantic-operation schema reserved byte at offset {offset} is nonzero"
            ),
        }
    }
}

impl Error for SemanticOperationSchemaDecodeError {}

/// Canonical payload represented by a V1 semantic instance identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticOperationInstancePayloadV1 {
    PointerDistance {
        kind: PointerDistanceKind,
        unit: PointerDistanceUnit,
        element: MemoryElementType,
        address_space: AddressSpace,
        layout: MemoryLayout,
        contract: PointerDistanceContract,
    },
    VolatileLoad {
        element: MemoryElementType,
        address_space: AddressSpace,
        layout: MemoryLayout,
        contract: VolatileAccessContract,
    },
    VolatileStore {
        element: MemoryElementType,
        address_space: AddressSpace,
        layout: MemoryLayout,
        contract: VolatileAccessContract,
    },
    CopyNonOverlapping {
        element: MemoryElementType,
        source_address_space: AddressSpace,
        destination_address_space: AddressSpace,
        layout: MemoryLayout,
        contract: CopyNonOverlappingContract,
    },
    LaunchInvocationIndex {
        kind: IndexKind,
        axis: Axis,
    },
    LaunchExtent {
        axis: Axis,
    },
}

/// Full identity of one target-neutral semantic operation instance.
///
/// Unlike SemanticOperationSchema, this value includes every semantic payload
/// field admitted by its V1 operation contract.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SemanticOperationInstanceId {
    schema: SemanticOperationSchema,
    payload: SemanticOperationInstancePayloadV1,
}

impl SemanticOperationInstanceId {
    #[allow(clippy::too_many_arguments)]
    pub const fn pointer_distance(
        kind: PointerDistanceKind,
        unit: PointerDistanceUnit,
        element: MemoryElementType,
        address_space: AddressSpace,
        layout: MemoryLayout,
        contract: PointerDistanceContract,
    ) -> Self {
        Self {
            schema: SemanticOperationSchema::v1(SemanticOperationKind::PointerDistance),
            payload: SemanticOperationInstancePayloadV1::PointerDistance {
                kind,
                unit,
                element,
                address_space,
                layout,
                contract,
            },
        }
    }

    pub const fn volatile_load(
        element: MemoryElementType,
        address_space: AddressSpace,
        layout: MemoryLayout,
        contract: VolatileAccessContract,
    ) -> Self {
        Self {
            schema: SemanticOperationSchema::v1(SemanticOperationKind::VolatileLoad),
            payload: SemanticOperationInstancePayloadV1::VolatileLoad {
                element,
                address_space,
                layout,
                contract,
            },
        }
    }

    pub const fn volatile_store(
        element: MemoryElementType,
        address_space: AddressSpace,
        layout: MemoryLayout,
        contract: VolatileAccessContract,
    ) -> Self {
        Self {
            schema: SemanticOperationSchema::v1(SemanticOperationKind::VolatileStore),
            payload: SemanticOperationInstancePayloadV1::VolatileStore {
                element,
                address_space,
                layout,
                contract,
            },
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub const fn copy_nonoverlapping(
        element: MemoryElementType,
        source_address_space: AddressSpace,
        destination_address_space: AddressSpace,
        layout: MemoryLayout,
        contract: CopyNonOverlappingContract,
    ) -> Self {
        Self {
            schema: SemanticOperationSchema::v1(SemanticOperationKind::CopyNonOverlapping),
            payload: SemanticOperationInstancePayloadV1::CopyNonOverlapping {
                element,
                source_address_space,
                destination_address_space,
                layout,
                contract,
            },
        }
    }

    pub const fn launch_invocation_index(kind: IndexKind, axis: Axis) -> Self {
        Self {
            schema: SemanticOperationSchema::v1(SemanticOperationKind::LaunchInvocationIndex),
            payload: SemanticOperationInstancePayloadV1::LaunchInvocationIndex { kind, axis },
        }
    }

    pub const fn launch_extent(axis: Axis) -> Self {
        Self {
            schema: SemanticOperationSchema::v1(SemanticOperationKind::LaunchExtent),
            payload: SemanticOperationInstancePayloadV1::LaunchExtent { axis },
        }
    }

    pub const fn schema(self) -> SemanticOperationSchema {
        self.schema
    }

    pub const fn payload(self) -> SemanticOperationInstancePayloadV1 {
        self.payload
    }
}

/// Encodes the full canonical semantic instance, including operation payload.
pub fn encode_semantic_operation_instance_id(id: SemanticOperationInstanceId) -> Vec<u8> {
    let payload = match id.payload {
        SemanticOperationInstancePayloadV1::PointerDistance {
            kind,
            unit,
            element,
            address_space,
            layout,
            contract,
        } => encode_memory_payload(
            &[
                pointer_distance_kind_tag(kind),
                pointer_distance_unit_tag(unit),
                memory_element_tag(element),
                address_space_tag(address_space),
                pointer_distance_provenance_tag(contract.provenance),
                pointer_distance_range_tag(contract.range),
                pointer_distance_divisibility_tag(contract.divisibility),
                pointer_distance_overflow_tag(contract.overflow),
                pointer_distance_ordering_tag(contract.ordering),
            ],
            layout,
        ),
        SemanticOperationInstancePayloadV1::VolatileLoad {
            element,
            address_space,
            layout,
            contract,
        }
        | SemanticOperationInstancePayloadV1::VolatileStore {
            element,
            address_space,
            layout,
            contract,
        } => encode_memory_payload(
            &[
                memory_element_tag(element),
                address_space_tag(address_space),
                volatile_provenance_tag(contract.provenance),
                volatile_range_tag(contract.range),
                memory_alignment_tag(contract.alignment),
                volatile_trap_tag(contract.trap),
                volatile_external_effect_tag(contract.external_effect),
            ],
            layout,
        ),
        SemanticOperationInstancePayloadV1::CopyNonOverlapping {
            element,
            source_address_space,
            destination_address_space,
            layout,
            contract,
        } => encode_memory_payload(
            &[
                memory_element_tag(element),
                address_space_tag(source_address_space),
                address_space_tag(destination_address_space),
                memory_alignment_tag(contract.alignment),
                copy_source_tag(contract.source),
                copy_destination_tag(contract.destination),
                copy_overlap_tag(contract.overlap),
                copy_byte_count_tag(contract.byte_count),
                copy_address_tag(contract.address),
                copy_zero_byte_tag(contract.zero_bytes),
            ],
            layout,
        ),
        SemanticOperationInstancePayloadV1::LaunchInvocationIndex { kind, axis } => {
            vec![index_kind_tag(kind), axis_tag(axis)]
        }
        SemanticOperationInstancePayloadV1::LaunchExtent { axis } => vec![axis_tag(axis)],
    };
    debug_assert!(payload.len() <= MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1);

    let mut bytes = Vec::with_capacity(SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + payload.len());
    bytes.extend_from_slice(&SEMANTIC_OPERATION_INSTANCE_MAGIC_V1);
    bytes.extend_from_slice(&id.schema.version.to_le_bytes());
    bytes.push(id.schema.family().tag());
    bytes.push(0);
    bytes.extend_from_slice(&id.schema.kind.opcode().to_le_bytes());
    bytes.extend_from_slice(&(payload.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&payload);
    bytes
}

fn encode_memory_payload(tags: &[u8], layout: MemoryLayout) -> Vec<u8> {
    let mut payload = Vec::with_capacity(tags.len() + 12);
    payload.extend_from_slice(tags);
    payload.extend_from_slice(&layout.size_bytes.to_le_bytes());
    payload.extend_from_slice(&layout.alignment_bytes.to_le_bytes());
    payload
}

/// Decodes a full canonical instance and rejects malformed or unknown payload.
pub fn decode_semantic_operation_instance_id(
    bytes: &[u8],
) -> Result<SemanticOperationInstanceId, SemanticOperationInstanceDecodeError> {
    if bytes.len() < SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 {
        return Err(SemanticOperationInstanceDecodeError::Truncated {
            actual: bytes.len(),
            expected: SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1,
        });
    }
    if bytes[..8] != SEMANTIC_OPERATION_INSTANCE_MAGIC_V1 {
        return Err(SemanticOperationInstanceDecodeError::InvalidMagic);
    }

    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version != SEMANTIC_OPERATION_VERSION_V1 {
        return Err(SemanticOperationInstanceDecodeError::UnknownVersion(
            version,
        ));
    }
    if bytes[11] != 0 {
        return Err(SemanticOperationInstanceDecodeError::UnsupportedFlags(
            bytes[11],
        ));
    }
    for (relative_offset, byte) in bytes[16..20].iter().enumerate() {
        if *byte != 0 {
            let offset = 16 + relative_offset;
            return Err(SemanticOperationInstanceDecodeError::ReservedNonZero { offset });
        }
    }

    let family = SemanticOperationFamily::from_tag(bytes[10]).ok_or(
        SemanticOperationInstanceDecodeError::UnknownFamily(bytes[10]),
    )?;
    let opcode = u16::from_le_bytes([bytes[12], bytes[13]]);
    let kind = SemanticOperationKind::from_parts(family, opcode)
        .ok_or(SemanticOperationInstanceDecodeError::UnknownOperation { family, opcode })?;
    let payload_length = u16::from_le_bytes([bytes[14], bytes[15]]) as usize;
    if payload_length > MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1 {
        return Err(SemanticOperationInstanceDecodeError::PayloadLimitExceeded {
            actual: payload_length,
            max: MAX_SEMANTIC_OPERATION_INSTANCE_PAYLOAD_BYTES_V1,
        });
    }
    let expected_payload_length = match kind {
        SemanticOperationKind::PointerDistance => 21,
        SemanticOperationKind::VolatileLoad | SemanticOperationKind::VolatileStore => 19,
        SemanticOperationKind::CopyNonOverlapping => 22,
        SemanticOperationKind::LaunchInvocationIndex => 2,
        SemanticOperationKind::LaunchExtent => 1,
    };
    if payload_length != expected_payload_length {
        return Err(SemanticOperationInstanceDecodeError::InvalidPayloadLength {
            kind,
            actual: payload_length,
            expected: expected_payload_length,
        });
    }

    let expected_length = SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1 + payload_length;
    if bytes.len() < expected_length {
        return Err(SemanticOperationInstanceDecodeError::Truncated {
            actual: bytes.len(),
            expected: expected_length,
        });
    }
    if bytes.len() > expected_length {
        return Err(SemanticOperationInstanceDecodeError::TrailingBytes {
            actual: bytes.len(),
            expected: expected_length,
        });
    }
    let payload = &bytes[SEMANTIC_OPERATION_INSTANCE_HEADER_BYTES_V1..];
    match kind {
        SemanticOperationKind::PointerDistance => {
            let distance_kind = decode_pointer_distance_kind(payload[0])?;
            let element = decode_memory_element(payload[2])?;
            let layout = decode_memory_layout(payload, 9);
            require_canonical_memory_layout(kind, element, layout)?;
            let contract = PointerDistanceContract {
                provenance: decode_pointer_distance_provenance(payload[4])?,
                range: decode_pointer_distance_range(payload[5])?,
                divisibility: decode_pointer_distance_divisibility(payload[6])?,
                overflow: decode_pointer_distance_overflow(payload[7])?,
                ordering: decode_pointer_distance_ordering(payload[8])?,
            };
            if contract != PointerDistanceContract::supported_rust(distance_kind) {
                return Err(SemanticOperationInstanceDecodeError::InvalidContract { kind });
            }
            Ok(SemanticOperationInstanceId::pointer_distance(
                distance_kind,
                decode_pointer_distance_unit(payload[1])?,
                element,
                decode_address_space(payload[3])?,
                layout,
                contract,
            ))
        }
        SemanticOperationKind::VolatileLoad | SemanticOperationKind::VolatileStore => {
            let element = decode_memory_element(payload[0])?;
            let address_space = decode_address_space(payload[1])?;
            let layout = decode_memory_layout(payload, 7);
            require_canonical_memory_layout(kind, element, layout)?;
            let contract = VolatileAccessContract {
                provenance: decode_volatile_provenance(payload[2])?,
                range: decode_volatile_range(payload[3])?,
                alignment: decode_memory_alignment(payload[4])?,
                trap: decode_volatile_trap(payload[5])?,
                external_effect: decode_volatile_external_effect(payload[6])?,
            };
            let matches_supported_contract = if kind == SemanticOperationKind::VolatileLoad {
                contract.matches_supported_load(element, address_space)
            } else {
                contract.matches_supported_store(element, address_space)
            };
            if !matches_supported_contract {
                return Err(SemanticOperationInstanceDecodeError::InvalidContract { kind });
            }
            if kind == SemanticOperationKind::VolatileLoad {
                Ok(SemanticOperationInstanceId::volatile_load(
                    element,
                    address_space,
                    layout,
                    contract,
                ))
            } else {
                Ok(SemanticOperationInstanceId::volatile_store(
                    element,
                    address_space,
                    layout,
                    contract,
                ))
            }
        }
        SemanticOperationKind::CopyNonOverlapping => {
            let element = decode_memory_element(payload[0])?;
            let layout = decode_memory_layout(payload, 10);
            require_canonical_memory_layout(kind, element, layout)?;
            let contract = CopyNonOverlappingContract {
                alignment: decode_memory_alignment(payload[3])?,
                source: decode_copy_source(payload[4])?,
                destination: decode_copy_destination(payload[5])?,
                overlap: decode_copy_overlap(payload[6])?,
                byte_count: decode_copy_byte_count(payload[7])?,
                address: decode_copy_address(payload[8])?,
                zero_bytes: decode_copy_zero_byte(payload[9])?,
            };
            if contract != CopyNonOverlappingContract::supported_rust() {
                return Err(SemanticOperationInstanceDecodeError::InvalidContract { kind });
            }
            Ok(SemanticOperationInstanceId::copy_nonoverlapping(
                element,
                decode_address_space(payload[1])?,
                decode_address_space(payload[2])?,
                layout,
                contract,
            ))
        }
        SemanticOperationKind::LaunchInvocationIndex => {
            let index_kind = decode_index_kind(payload[0])?;
            let axis = decode_axis(payload[1])?;
            Ok(SemanticOperationInstanceId::launch_invocation_index(
                index_kind, axis,
            ))
        }
        SemanticOperationKind::LaunchExtent => {
            let axis = decode_axis(payload[0])?;
            Ok(SemanticOperationInstanceId::launch_extent(axis))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SemanticOperationInstanceDecodeError {
    Truncated {
        actual: usize,
        expected: usize,
    },
    TrailingBytes {
        actual: usize,
        expected: usize,
    },
    InvalidMagic,
    UnknownVersion(u16),
    UnsupportedFlags(u8),
    UnknownFamily(u8),
    UnknownOperation {
        family: SemanticOperationFamily,
        opcode: u16,
    },
    ReservedNonZero {
        offset: usize,
    },
    PayloadLimitExceeded {
        actual: usize,
        max: usize,
    },
    InvalidPayloadLength {
        kind: SemanticOperationKind,
        actual: usize,
        expected: usize,
    },
    NonCanonicalMemoryLayout {
        kind: SemanticOperationKind,
        element: MemoryElementType,
        actual: MemoryLayout,
        expected: MemoryLayout,
    },
    InvalidContract {
        kind: SemanticOperationKind,
    },
    UnknownPayloadTag {
        field: &'static str,
        tag: u8,
    },
}

impl fmt::Display for SemanticOperationInstanceDecodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Truncated { actual, expected } | Self::TrailingBytes { actual, expected } => {
                write!(
                    formatter,
                    "semantic-operation instance has {actual} bytes; expected {expected}"
                )
            }
            Self::InvalidMagic => formatter.write_str("invalid semantic-operation instance magic"),
            Self::UnknownVersion(version) => {
                write!(
                    formatter,
                    "unknown semantic-operation instance version {version}"
                )
            }
            Self::UnsupportedFlags(flags) => {
                write!(formatter, "unsupported semantic-operation flags {flags:#x}")
            }
            Self::UnknownFamily(family) => {
                write!(formatter, "unknown semantic-operation family {family}")
            }
            Self::UnknownOperation { family, opcode } => write!(
                formatter,
                "unknown {family:?} semantic-operation opcode {opcode}"
            ),
            Self::ReservedNonZero { offset } => write!(
                formatter,
                "semantic-operation instance reserved byte at offset {offset} is nonzero"
            ),
            Self::PayloadLimitExceeded { actual, max } => write!(
                formatter,
                "semantic-operation payload has {actual} bytes; maximum is {max}"
            ),
            Self::InvalidPayloadLength {
                kind,
                actual,
                expected,
            } => write!(
                formatter,
                "{kind:?} semantic payload has {actual} bytes; expected {expected}"
            ),
            Self::NonCanonicalMemoryLayout {
                kind,
                element,
                actual,
                expected,
            } => write!(
                formatter,
                "{kind:?} semantic payload uses {actual:?} for {element:?}; expected canonical layout {expected:?}"
            ),
            Self::InvalidContract { kind } => {
                write!(
                    formatter,
                    "{kind:?} semantic payload has inconsistent obligations"
                )
            }
            Self::UnknownPayloadTag { field, tag } => {
                write!(formatter, "unknown semantic-operation {field} tag {tag}")
            }
        }
    }
}

impl Error for SemanticOperationInstanceDecodeError {}

fn memory_element_tag(element: MemoryElementType) -> u8 {
    match element {
        MemoryElementType::Unit => 0,
        MemoryElementType::Scalar(scalar) => scalar_type_tag(scalar),
    }
}

fn decode_memory_element(
    tag: u8,
) -> Result<MemoryElementType, SemanticOperationInstanceDecodeError> {
    if tag == 0 {
        Ok(MemoryElementType::Unit)
    } else {
        decode_scalar_type(tag).map(MemoryElementType::Scalar)
    }
}

fn scalar_type_tag(scalar: ScalarType) -> u8 {
    match scalar {
        ScalarType::Bool => 1,
        ScalarType::I8 => 2,
        ScalarType::I16 => 3,
        ScalarType::I32 => 4,
        ScalarType::I64 => 5,
        ScalarType::U8 => 6,
        ScalarType::U16 => 7,
        ScalarType::U32 => 8,
        ScalarType::U64 => 9,
        ScalarType::Index => 10,
        ScalarType::F16 => 11,
        ScalarType::Bf16 => 12,
        ScalarType::F32 => 13,
        ScalarType::F64 => 14,
        ScalarType::I128 => 15,
        ScalarType::U128 => 16,
    }
}

fn decode_scalar_type(tag: u8) -> Result<ScalarType, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(ScalarType::Bool),
        2 => Ok(ScalarType::I8),
        3 => Ok(ScalarType::I16),
        4 => Ok(ScalarType::I32),
        5 => Ok(ScalarType::I64),
        6 => Ok(ScalarType::U8),
        7 => Ok(ScalarType::U16),
        8 => Ok(ScalarType::U32),
        9 => Ok(ScalarType::U64),
        10 => Ok(ScalarType::Index),
        11 => Ok(ScalarType::F16),
        12 => Ok(ScalarType::Bf16),
        13 => Ok(ScalarType::F32),
        14 => Ok(ScalarType::F64),
        15 => Ok(ScalarType::I128),
        16 => Ok(ScalarType::U128),
        tag => Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag {
            field: "memory element",
            tag,
        }),
    }
}

fn address_space_tag(address_space: AddressSpace) -> u8 {
    match address_space {
        AddressSpace::Private => 1,
        AddressSpace::Workgroup => 2,
        AddressSpace::Global => 3,
        AddressSpace::Constant => 4,
        AddressSpace::Generic => 5,
    }
}

fn decode_address_space(tag: u8) -> Result<AddressSpace, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(AddressSpace::Private),
        2 => Ok(AddressSpace::Workgroup),
        3 => Ok(AddressSpace::Global),
        4 => Ok(AddressSpace::Constant),
        5 => Ok(AddressSpace::Generic),
        tag => Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag {
            field: "address space",
            tag,
        }),
    }
}

fn pointer_distance_kind_tag(kind: PointerDistanceKind) -> u8 {
    match kind {
        PointerDistanceKind::Signed => 1,
        PointerDistanceKind::Unsigned => 2,
    }
}

fn decode_pointer_distance_kind(
    tag: u8,
) -> Result<PointerDistanceKind, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(PointerDistanceKind::Signed),
        2 => Ok(PointerDistanceKind::Unsigned),
        tag => Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag {
            field: "pointer-distance kind",
            tag,
        }),
    }
}

fn pointer_distance_unit_tag(unit: PointerDistanceUnit) -> u8 {
    match unit {
        PointerDistanceUnit::Elements => 1,
        PointerDistanceUnit::Bytes => 2,
    }
}

fn decode_pointer_distance_unit(
    tag: u8,
) -> Result<PointerDistanceUnit, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(PointerDistanceUnit::Elements),
        2 => Ok(PointerDistanceUnit::Bytes),
        tag => Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag {
            field: "pointer-distance unit",
            tag,
        }),
    }
}

fn pointer_distance_provenance_tag(value: PointerDistanceProvenanceContract) -> u8 {
    match value {
        PointerDistanceProvenanceContract::EqualAddressesOrSameAllocation => 1,
    }
}

fn decode_pointer_distance_provenance(
    tag: u8,
) -> Result<PointerDistanceProvenanceContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "pointer-distance provenance",
        PointerDistanceProvenanceContract::EqualAddressesOrSameAllocation,
    )
}

fn pointer_distance_range_tag(value: PointerDistanceRangeContract) -> u8 {
    match value {
        PointerDistanceRangeContract::BothPointersInBoundsOrOnePastWhenAddressesDiffer => 1,
    }
}

fn decode_pointer_distance_range(
    tag: u8,
) -> Result<PointerDistanceRangeContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "pointer-distance range",
        PointerDistanceRangeContract::BothPointersInBoundsOrOnePastWhenAddressesDiffer,
    )
}

fn pointer_distance_divisibility_tag(value: PointerDistanceDivisibilityContract) -> u8 {
    match value {
        PointerDistanceDivisibilityContract::ExactMultipleOfDeclaredUnit => 1,
    }
}

fn decode_pointer_distance_divisibility(
    tag: u8,
) -> Result<PointerDistanceDivisibilityContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "pointer-distance divisibility",
        PointerDistanceDivisibilityContract::ExactMultipleOfDeclaredUnit,
    )
}

fn pointer_distance_overflow_tag(value: PointerDistanceOverflowContract) -> u8 {
    match value {
        PointerDistanceOverflowContract::DifferenceFitsIsize => 1,
    }
}

fn decode_pointer_distance_overflow(
    tag: u8,
) -> Result<PointerDistanceOverflowContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "pointer-distance overflow",
        PointerDistanceOverflowContract::DifferenceFitsIsize,
    )
}

fn pointer_distance_ordering_tag(value: PointerDistanceOrderingContract) -> u8 {
    match value {
        PointerDistanceOrderingContract::SignedMayBeNegative => 1,
        PointerDistanceOrderingContract::UnsignedPointerAtOrAfterOrigin => 2,
    }
}

fn decode_pointer_distance_ordering(
    tag: u8,
) -> Result<PointerDistanceOrderingContract, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(PointerDistanceOrderingContract::SignedMayBeNegative),
        2 => Ok(PointerDistanceOrderingContract::UnsignedPointerAtOrAfterOrigin),
        tag => unknown_payload_tag("pointer-distance ordering", tag),
    }
}

fn volatile_provenance_tag(value: VolatileProvenanceContract) -> u8 {
    match value {
        VolatileProvenanceContract::RustAllocation => 1,
        VolatileProvenanceContract::ExternalMmioNotRustAllocation => 2,
        VolatileProvenanceContract::ZeroSizedNoAccess => 3,
    }
}

fn decode_volatile_provenance(
    tag: u8,
) -> Result<VolatileProvenanceContract, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(VolatileProvenanceContract::RustAllocation),
        2 => Ok(VolatileProvenanceContract::ExternalMmioNotRustAllocation),
        3 => Ok(VolatileProvenanceContract::ZeroSizedNoAccess),
        tag => unknown_payload_tag("volatile provenance", tag),
    }
}

fn volatile_range_tag(value: VolatileRangeContract) -> u8 {
    match value {
        VolatileRangeContract::ReadableInitializedElement => 1,
        VolatileRangeContract::WritableElement => 2,
        VolatileRangeContract::ZeroSizedNoAccess => 3,
    }
}

fn decode_volatile_range(
    tag: u8,
) -> Result<VolatileRangeContract, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(VolatileRangeContract::ReadableInitializedElement),
        2 => Ok(VolatileRangeContract::WritableElement),
        3 => Ok(VolatileRangeContract::ZeroSizedNoAccess),
        tag => unknown_payload_tag("volatile range", tag),
    }
}

fn memory_alignment_tag(value: MemoryAlignmentContract) -> u8 {
    match value {
        MemoryAlignmentContract::AlignedForElement => 1,
    }
}

fn decode_memory_alignment(
    tag: u8,
) -> Result<MemoryAlignmentContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "memory alignment",
        MemoryAlignmentContract::AlignedForElement,
    )
}

fn volatile_trap_tag(value: VolatileTrapContract) -> u8 {
    match value {
        VolatileTrapContract::NonTrapping => 1,
        VolatileTrapContract::ZeroSizedNoAccess => 2,
    }
}

fn decode_volatile_trap(
    tag: u8,
) -> Result<VolatileTrapContract, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(VolatileTrapContract::NonTrapping),
        2 => Ok(VolatileTrapContract::ZeroSizedNoAccess),
        tag => unknown_payload_tag("volatile trap", tag),
    }
}

fn volatile_external_effect_tag(value: VolatileExternalEffectContract) -> u8 {
    match value {
        VolatileExternalEffectContract::NotExternal => 1,
        VolatileExternalEffectContract::SideEffectsDoNotModifyRustAllocatedMemory => 2,
    }
}

fn decode_volatile_external_effect(
    tag: u8,
) -> Result<VolatileExternalEffectContract, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(VolatileExternalEffectContract::NotExternal),
        2 => Ok(VolatileExternalEffectContract::SideEffectsDoNotModifyRustAllocatedMemory),
        tag => unknown_payload_tag("volatile external effect", tag),
    }
}

fn copy_source_tag(value: CopySourceContract) -> u8 {
    match value {
        CopySourceContract::ReadableWhenBytesPositive => 1,
    }
}

fn decode_copy_source(tag: u8) -> Result<CopySourceContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "copy source",
        CopySourceContract::ReadableWhenBytesPositive,
    )
}

fn copy_destination_tag(value: CopyDestinationContract) -> u8 {
    match value {
        CopyDestinationContract::WritableWhenBytesPositive => 1,
    }
}

fn decode_copy_destination(
    tag: u8,
) -> Result<CopyDestinationContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "copy destination",
        CopyDestinationContract::WritableWhenBytesPositive,
    )
}

fn copy_overlap_tag(value: CopyOverlapContract) -> u8 {
    match value {
        CopyOverlapContract::NonOverlappingWhenBytesPositive => 1,
    }
}

fn decode_copy_overlap(
    tag: u8,
) -> Result<CopyOverlapContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "copy overlap",
        CopyOverlapContract::NonOverlappingWhenBytesPositive,
    )
}

fn copy_byte_count_tag(value: CopyByteCountContract) -> u8 {
    match value {
        CopyByteCountContract::CountTimesElementSizeFitsUsize => 1,
    }
}

fn decode_copy_byte_count(
    tag: u8,
) -> Result<CopyByteCountContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "copy byte count",
        CopyByteCountContract::CountTimesElementSizeFitsUsize,
    )
}

fn copy_address_tag(value: CopyAddressContract) -> u8 {
    match value {
        CopyAddressContract::SignedOffsetsFitIsizeAndStayInAllocationsWhenBytesPositive => 1,
    }
}

fn decode_copy_address(
    tag: u8,
) -> Result<CopyAddressContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "copy address",
        CopyAddressContract::SignedOffsetsFitIsizeAndStayInAllocationsWhenBytesPositive,
    )
}

fn copy_zero_byte_tag(value: CopyZeroByteContract) -> u8 {
    match value {
        CopyZeroByteContract::AlignmentRequiredRangesAndOverlapConditionalOnPositiveBytes => 1,
    }
}

fn decode_copy_zero_byte(
    tag: u8,
) -> Result<CopyZeroByteContract, SemanticOperationInstanceDecodeError> {
    decode_singleton_tag(
        tag,
        "copy zero-byte",
        CopyZeroByteContract::AlignmentRequiredRangesAndOverlapConditionalOnPositiveBytes,
    )
}

fn decode_singleton_tag<T: Copy>(
    tag: u8,
    field: &'static str,
    value: T,
) -> Result<T, SemanticOperationInstanceDecodeError> {
    if tag == 1 {
        Ok(value)
    } else {
        unknown_payload_tag(field, tag)
    }
}

fn unknown_payload_tag<T>(
    field: &'static str,
    tag: u8,
) -> Result<T, SemanticOperationInstanceDecodeError> {
    Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag { field, tag })
}

fn decode_memory_layout(payload: &[u8], offset: usize) -> MemoryLayout {
    let mut size = [0_u8; 8];
    size.copy_from_slice(&payload[offset..offset + 8]);
    let mut alignment = [0_u8; 4];
    alignment.copy_from_slice(&payload[offset + 8..offset + 12]);
    MemoryLayout::new(u64::from_le_bytes(size), u32::from_le_bytes(alignment))
}

fn require_canonical_memory_layout(
    kind: SemanticOperationKind,
    element: MemoryElementType,
    actual: MemoryLayout,
) -> Result<(), SemanticOperationInstanceDecodeError> {
    let expected = element.expected_layout();
    if actual == expected {
        Ok(())
    } else {
        Err(
            SemanticOperationInstanceDecodeError::NonCanonicalMemoryLayout {
                kind,
                element,
                actual,
                expected,
            },
        )
    }
}

fn axis_tag(axis: Axis) -> u8 {
    match axis {
        Axis::X => 1,
        Axis::Y => 2,
        Axis::Z => 3,
    }
}

fn decode_axis(tag: u8) -> Result<Axis, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(Axis::X),
        2 => Ok(Axis::Y),
        3 => Ok(Axis::Z),
        tag => Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag { field: "axis", tag }),
    }
}

fn index_kind_tag(kind: IndexKind) -> u8 {
    match kind {
        IndexKind::Global => 1,
        IndexKind::Workgroup => 2,
        IndexKind::Local => 3,
        IndexKind::WorkgroupSize => 4,
        IndexKind::WorkgroupCount => 5,
    }
}

fn decode_index_kind(tag: u8) -> Result<IndexKind, SemanticOperationInstanceDecodeError> {
    match tag {
        1 => Ok(IndexKind::Global),
        2 => Ok(IndexKind::Workgroup),
        3 => Ok(IndexKind::Local),
        4 => Ok(IndexKind::WorkgroupSize),
        5 => Ok(IndexKind::WorkgroupCount),
        tag => Err(SemanticOperationInstanceDecodeError::UnknownPayloadTag {
            field: "index kind",
            tag,
        }),
    }
}

/// Local shape and effects declared by a strongly typed semantic operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticOperationContract {
    instance_id: SemanticOperationInstanceId,
    pub operand_count: usize,
    pub result_types: Vec<Type>,
    pub memory_effects: Vec<MemoryEffect>,
    pub required_capabilities: BTreeSet<TargetCapability>,
}

impl SemanticOperationContract {
    pub fn new(
        instance_id: SemanticOperationInstanceId,
        operand_count: usize,
        result_types: Vec<Type>,
        memory_effects: Vec<MemoryEffect>,
        required_capabilities: BTreeSet<TargetCapability>,
    ) -> Self {
        Self {
            instance_id,
            operand_count,
            result_types,
            memory_effects,
            required_capabilities,
        }
    }

    pub const fn schema(&self) -> SemanticOperationSchema {
        self.instance_id.schema()
    }

    pub const fn instance_id(&self) -> SemanticOperationInstanceId {
        self.instance_id
    }
}

/// Independently extracted operation data supplied by the module verifier.
#[derive(Clone, Copy, Debug)]
pub struct SemanticOperationVerificationContext<'a> {
    pub operands: &'a [ValueId],
    pub results: &'a [ValueDef],
    /// None means the normal SSA verifier has diagnosed an unknown value.
    pub operand_types: &'a [Option<Type>],
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum SemanticOperationIssueKind {
    InvalidStructure,
    InvalidOperandType,
    ResultArity,
    TypeMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticOperationIssue {
    pub kind: SemanticOperationIssueKind,
    pub message: String,
}

impl SemanticOperationIssue {
    pub fn new(kind: SemanticOperationIssueKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

/// Contract implemented by a strongly typed target-neutral operation payload.
///
/// Implementing this trait does not make an operation serializable or lowerable.
/// OperationKind, the module wire decoder, and each backend remain closed
/// admission boundaries.
pub trait SemanticOperation {
    fn contract(&self) -> SemanticOperationContract;

    /// Adds payload-specific structural and type checks after generic shape checks.
    fn verify_additional(
        &self,
        _context: SemanticOperationVerificationContext<'_>,
        _issues: &mut Vec<SemanticOperationIssue>,
    ) {
    }

    fn verify(
        &self,
        context: SemanticOperationVerificationContext<'_>,
    ) -> Vec<SemanticOperationIssue> {
        let contract = self.contract();
        let mut issues = Vec::new();
        if context.operand_types.len() != context.operands.len() {
            issues.push(SemanticOperationIssue::new(
                SemanticOperationIssueKind::InvalidStructure,
                format!(
                    "semantic verifier received {} operands but {} operand types",
                    context.operands.len(),
                    context.operand_types.len()
                ),
            ));
        }
        if context.operands.len() != contract.operand_count {
            issues.push(SemanticOperationIssue::new(
                SemanticOperationIssueKind::InvalidStructure,
                format!(
                    "operation contains {} operands but schema requires {}",
                    context.operands.len(),
                    contract.operand_count
                ),
            ));
        }
        if context.results.len() != contract.result_types.len() {
            issues.push(SemanticOperationIssue::new(
                SemanticOperationIssueKind::ResultArity,
                format!(
                    "operation defines {} results but {} are required",
                    context.results.len(),
                    contract.result_types.len()
                ),
            ));
        }
        for (result, expected_ty) in context.results.iter().zip(&contract.result_types) {
            if &result.ty != expected_ty {
                issues.push(SemanticOperationIssue::new(
                    SemanticOperationIssueKind::TypeMismatch,
                    format!(
                        "result {} has type {:?}, expected {expected_ty:?}",
                        result.id, result.ty
                    ),
                ));
            }
        }
        self.verify_additional(context, &mut issues);
        issues
    }
}

/// Typed target-neutral memory operations. Their contract fields are explicit
/// unsafe caller obligations, not compiler-import, proof, or artifact authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryIntrinsicOperation {
    PointerDistance {
        pointer: ValueId,
        origin: ValueId,
        kind: PointerDistanceKind,
        unit: PointerDistanceUnit,
        element: MemoryElementType,
        address_space: AddressSpace,
        layout: MemoryLayout,
        contract: PointerDistanceContract,
    },
    VolatileLoad {
        pointer: ValueId,
        element: MemoryElementType,
        address_space: AddressSpace,
        layout: MemoryLayout,
        contract: VolatileAccessContract,
    },
    VolatileStore {
        pointer: ValueId,
        value: ValueId,
        element: MemoryElementType,
        address_space: AddressSpace,
        layout: MemoryLayout,
        contract: VolatileAccessContract,
    },
    CopyNonOverlapping {
        source: ValueId,
        destination: ValueId,
        count: ValueId,
        element: MemoryElementType,
        source_address_space: AddressSpace,
        destination_address_space: AddressSpace,
        layout: MemoryLayout,
        contract: CopyNonOverlappingContract,
    },
}

impl MemoryIntrinsicOperation {
    pub fn operands(&self) -> Vec<ValueId> {
        match self {
            Self::PointerDistance {
                pointer, origin, ..
            } => vec![*pointer, *origin],
            Self::VolatileLoad { pointer, .. } => vec![*pointer],
            Self::VolatileStore { pointer, value, .. } => vec![*pointer, *value],
            Self::CopyNonOverlapping {
                source,
                destination,
                count,
                ..
            } => vec![*source, *destination, *count],
        }
    }
}

impl SemanticOperation for MemoryIntrinsicOperation {
    fn contract(&self) -> SemanticOperationContract {
        match *self {
            Self::PointerDistance {
                kind,
                unit,
                element,
                address_space,
                layout,
                contract,
                ..
            } => SemanticOperationContract::new(
                SemanticOperationInstanceId::pointer_distance(
                    kind,
                    unit,
                    element,
                    address_space,
                    layout,
                    contract,
                ),
                2,
                vec![match kind {
                    PointerDistanceKind::Signed => Type::Scalar(ScalarType::I64),
                    PointerDistanceKind::Unsigned => Type::INDEX,
                }],
                Vec::new(),
                BTreeSet::new(),
            ),
            Self::VolatileLoad {
                element,
                address_space,
                layout,
                contract,
                ..
            } => SemanticOperationContract::new(
                SemanticOperationInstanceId::volatile_load(
                    element,
                    address_space,
                    layout,
                    contract,
                ),
                1,
                if element == MemoryElementType::Unit {
                    Vec::new()
                } else {
                    vec![element.ir_type()]
                },
                if element == MemoryElementType::Unit {
                    Vec::new()
                } else {
                    vec![MemoryEffect::VolatileRead(address_space)]
                },
                BTreeSet::new(),
            ),
            Self::VolatileStore {
                element,
                address_space,
                layout,
                contract,
                ..
            } => SemanticOperationContract::new(
                SemanticOperationInstanceId::volatile_store(
                    element,
                    address_space,
                    layout,
                    contract,
                ),
                2,
                Vec::new(),
                if element == MemoryElementType::Unit {
                    Vec::new()
                } else {
                    vec![MemoryEffect::VolatileWrite(address_space)]
                },
                BTreeSet::new(),
            ),
            Self::CopyNonOverlapping {
                element,
                source_address_space,
                destination_address_space,
                layout,
                contract,
                ..
            } => SemanticOperationContract::new(
                SemanticOperationInstanceId::copy_nonoverlapping(
                    element,
                    source_address_space,
                    destination_address_space,
                    layout,
                    contract,
                ),
                3,
                Vec::new(),
                vec![
                    MemoryEffect::Read(source_address_space),
                    MemoryEffect::Write(destination_address_space),
                ],
                BTreeSet::new(),
            ),
        }
    }

    fn verify_additional(
        &self,
        context: SemanticOperationVerificationContext<'_>,
        issues: &mut Vec<SemanticOperationIssue>,
    ) {
        match *self {
            Self::PointerDistance {
                kind,
                unit,
                element,
                address_space,
                layout,
                contract,
                ..
            } => {
                verify_layout(element, layout, issues);
                if unit == PointerDistanceUnit::Elements && layout.size_bytes == 0 {
                    invalid_structure(
                        issues,
                        "element pointer distance rejects zero-sized pointees",
                    );
                }
                if contract != PointerDistanceContract::supported_rust(kind) {
                    invalid_structure(
                        issues,
                        "pointer distance requires equal-address or same-allocation provenance, differing-address in-bounds-or-one-past range, exact unit divisibility, isize fit, and kind-specific ordering obligations",
                    );
                }
                verify_pointer_operand(
                    context.operand_types.first(),
                    element,
                    address_space,
                    false,
                    issues,
                );
                verify_pointer_operand(
                    context.operand_types.get(1),
                    element,
                    address_space,
                    false,
                    issues,
                );
            }
            Self::VolatileLoad {
                element,
                address_space,
                layout,
                contract,
                ..
            } => {
                verify_layout(element, layout, issues);
                if !contract.matches_supported_load(element, address_space) {
                    invalid_structure(
                        issues,
                        "volatile load requires either an aligned ZST no-access contract or a positive-sized Rust-allocation/external-MMIO readable initialized-element, aligned, nontrapping contract with external side-effect isolation",
                    );
                }
                verify_pointer_operand(
                    context.operand_types.first(),
                    element,
                    address_space,
                    false,
                    issues,
                );
            }
            Self::VolatileStore {
                element,
                address_space,
                layout,
                contract,
                ..
            } => {
                verify_layout(element, layout, issues);
                if !contract.matches_supported_store(element, address_space) {
                    invalid_structure(
                        issues,
                        "volatile store requires either an aligned ZST no-access contract or a positive-sized Rust-allocation/external-MMIO writable-element, aligned, nontrapping contract with external side-effect isolation",
                    );
                }
                verify_pointer_operand(
                    context.operand_types.first(),
                    element,
                    address_space,
                    true,
                    issues,
                );
                verify_value_operand(context.operand_types.get(1), element.ir_type(), issues);
            }
            Self::CopyNonOverlapping {
                element,
                source_address_space,
                destination_address_space,
                layout,
                contract,
                ..
            } => {
                verify_layout(element, layout, issues);
                if contract != CopyNonOverlappingContract::supported_rust() {
                    invalid_structure(
                        issues,
                        "copy_nonoverlapping requires alignment even at zero bytes, positive-byte range and non-overlap obligations, checked usize scaling, and signed in-allocation address bounds",
                    );
                }
                verify_pointer_operand(
                    context.operand_types.first(),
                    element,
                    source_address_space,
                    false,
                    issues,
                );
                verify_pointer_operand(
                    context.operand_types.get(1),
                    element,
                    destination_address_space,
                    true,
                    issues,
                );
                verify_value_operand(context.operand_types.get(2), Type::INDEX, issues);
            }
        }
    }
}

fn verify_layout(
    element: MemoryElementType,
    layout: MemoryLayout,
    issues: &mut Vec<SemanticOperationIssue>,
) {
    if layout != element.expected_layout() {
        invalid_structure(
            issues,
            format!(
                "memory layout {layout:?} does not match the closed {element:?} layout {:?}",
                element.expected_layout()
            ),
        );
    }
}

fn verify_pointer_operand(
    actual: Option<&Option<Type>>,
    element: MemoryElementType,
    address_space: AddressSpace,
    writable: bool,
    issues: &mut Vec<SemanticOperationIssue>,
) {
    let Some(Some(Type::Pointer(pointer))) = actual else {
        if !matches!(actual, Some(None)) {
            issues.push(SemanticOperationIssue::new(
                SemanticOperationIssueKind::InvalidOperandType,
                "memory intrinsic requires a pointer operand",
            ));
        }
        return;
    };
    if pointer.pointee.as_ref() != &element.ir_type()
        || pointer.address_space != address_space
        || (writable && pointer.access != AccessMode::ReadWrite)
    {
        issues.push(SemanticOperationIssue::new(
            SemanticOperationIssueKind::InvalidOperandType,
            format!(
                "pointer operand {pointer:?} does not match element {element:?}, address space {address_space:?}, writable {writable}"
            ),
        ));
    }
}

fn verify_value_operand(
    actual: Option<&Option<Type>>,
    expected: Type,
    issues: &mut Vec<SemanticOperationIssue>,
) {
    if let Some(Some(actual)) = actual
        && actual != &expected
    {
        issues.push(SemanticOperationIssue::new(
            SemanticOperationIssueKind::InvalidOperandType,
            format!("memory intrinsic operand has type {actual:?}, expected {expected:?}"),
        ));
    }
}

fn invalid_structure(issues: &mut Vec<SemanticOperationIssue>, message: impl Into<String>) {
    issues.push(SemanticOperationIssue::new(
        SemanticOperationIssueKind::InvalidStructure,
        message,
    ));
}

impl SemanticOperation for IntrinsicOperation {
    fn contract(&self) -> SemanticOperationContract {
        let metadata = self.metadata();
        let instance_id = match self.kind {
            IntrinsicKind::InvocationIndex { kind, axis } => {
                SemanticOperationInstanceId::launch_invocation_index(kind, axis)
            }
            IntrinsicKind::LaunchExtent { axis } => {
                SemanticOperationInstanceId::launch_extent(axis)
            }
        };
        SemanticOperationContract::new(
            instance_id,
            0,
            vec![metadata.result_type],
            metadata.memory_effects.effects().iter().cloned().collect(),
            metadata.required_capabilities,
        )
    }

    fn verify_additional(
        &self,
        _context: SemanticOperationVerificationContext<'_>,
        issues: &mut Vec<SemanticOperationIssue>,
    ) {
        let expected = self.metadata().result_type;
        if self.result_type != expected {
            issues.push(SemanticOperationIssue::new(
                SemanticOperationIssueKind::TypeMismatch,
                format!(
                    "intrinsic declares result type {:?}, expected {:?}",
                    self.result_type, expected
                ),
            ));
        }
    }
}
