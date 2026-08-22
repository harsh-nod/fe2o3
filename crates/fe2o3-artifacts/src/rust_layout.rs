use std::{fmt, num::NonZeroU64};

use crate::{
    DeclaredRustLayoutIdentity, DeclaredRustTypeIdentity, DigestAlgorithm, PointerWidth,
    TypeIdentity,
};

/// Version of the canonical Rust type and layout evidence schema.
pub const RUST_LAYOUT_EVIDENCE_VERSION_V1: u16 = 1;
/// Domain framing the canonical source-type preimage.
pub const RUST_TYPE_EVIDENCE_DOMAIN_V1: &[u8] = b"FE2O3/RUST-TYPE-EVIDENCE/V1\0";
/// Domain framing the canonical rustc-layout preimage.
pub const RUST_LAYOUT_EVIDENCE_DOMAIN_V1: &[u8] = b"FE2O3/RUST-LAYOUT-EVIDENCE/V1\0";
/// Maximum size represented by one Rust layout evidence record.
pub const MAX_RUST_LAYOUT_BYTES: u64 = 1 << 20;
/// Maximum ABI alignment represented by one Rust layout evidence record.
pub const MAX_RUST_LAYOUT_ALIGNMENT: u32 = 1 << 20;
/// Maximum number of physical components in one Rust layout evidence record.
pub const MAX_RUST_LAYOUT_COMPONENTS: usize = 32;

/// Scalar identity used by the V1 source-type schema.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RustScalarElementTypeV1 {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F16,
    F32,
    F64,
}

impl RustScalarElementTypeV1 {
    pub const fn size_bytes(self) -> u64 {
        match self {
            Self::I8 | Self::U8 => 1,
            Self::I16 | Self::U16 | Self::F16 => 2,
            Self::I32 | Self::U32 | Self::F32 => 4,
            Self::I64 | Self::U64 | Self::F64 => 8,
        }
    }
}

/// Index-space type argument of `fe2o3_device::DisjointSlice`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RustDisjointIndexSpaceV1 {
    Index1D,
    /// One injective constant translation of the logical 1D invocation ID.
    ShiftedIndex1D {
        offset: u64,
    },
    /// One globally unique invocation owns every element in the view.
    GridExclusive,
    /// One fixed injective blocked mapping of the logical 1D invocation ID.
    BlockedIndex1D {
        lanes_per_block: NonZeroU64,
        elements_per_lane: NonZeroU64,
    },
}

impl RustDisjointIndexSpaceV1 {
    /// Constructs a blocked 1D mapping only when both dimensions are nonzero.
    pub const fn blocked_index_1d(lanes_per_block: u64, elements_per_lane: u64) -> Option<Self> {
        let Some(lanes_per_block) = NonZeroU64::new(lanes_per_block) else {
            return None;
        };
        let Some(elements_per_lane) = NonZeroU64::new(elements_per_lane) else {
            return None;
        };
        Some(Self::BlockedIndex1D {
            lanes_per_block,
            elements_per_lane,
        })
    }
}

/// Fully specified source-level Rust type shape.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RustSourceTypeShapeV1 {
    Scalar {
        scalar: RustScalarElementTypeV1,
    },
    SharedSlice {
        element: RustScalarElementTypeV1,
    },
    DisjointSlice {
        element: RustScalarElementTypeV1,
        index_space: RustDisjointIndexSpaceV1,
    },
}

impl RustSourceTypeShapeV1 {
    pub const fn scalar(scalar: RustScalarElementTypeV1) -> Self {
        Self::Scalar { scalar }
    }

    pub const fn shared_slice(element: RustScalarElementTypeV1) -> Self {
        Self::SharedSlice { element }
    }

    pub const fn disjoint_slice(
        element: RustScalarElementTypeV1,
        index_space: RustDisjointIndexSpaceV1,
    ) -> Self {
        Self::DisjointSlice {
            element,
            index_space,
        }
    }

    pub const fn element(self) -> RustScalarElementTypeV1 {
        match self {
            Self::Scalar { scalar } => scalar,
            Self::SharedSlice { element } | Self::DisjointSlice { element, .. } => element,
        }
    }
}

/// rustc ABI classification reported for a monomorphized Rust type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RustcAbiClassV1 {
    Scalar,
    ScalarPair,
    Aggregate,
    Uninhabited,
}

/// Mutability carried by a physical pointer component.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RustPointerMutabilityV1 {
    Const,
    Mut,
}

/// Kind and semantic identity of one physical Rust layout component.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum RustPhysicalComponentKindV1 {
    Scalar {
        scalar: RustScalarElementTypeV1,
    },
    Pointer {
        mutability: RustPointerMutabilityV1,
        pointee: RustScalarElementTypeV1,
    },
    Usize,
    /// A zero-sized component explicitly present in rustc's physical ABI evidence.
    Zst,
    /// Explicit bytes not occupied by a source field.
    Padding,
}

/// One ordered component in rustc's physical representation of a Rust type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RustPhysicalComponentV1 {
    offset: u64,
    size: u64,
    abi_alignment: u32,
    kind: RustPhysicalComponentKindV1,
}

impl RustPhysicalComponentV1 {
    pub fn new(
        offset: u64,
        size: u64,
        abi_alignment: u32,
        kind: RustPhysicalComponentKindV1,
    ) -> Result<Self, RustLayoutEvidenceError> {
        validate_alignment("physical component", abi_alignment)?;
        if offset > MAX_RUST_LAYOUT_BYTES || size > MAX_RUST_LAYOUT_BYTES {
            return Err(RustLayoutEvidenceError::BoundExceeded {
                field: "physical component",
                max: MAX_RUST_LAYOUT_BYTES,
            });
        }
        if !offset.is_multiple_of(u64::from(abi_alignment)) {
            return Err(RustLayoutEvidenceError::MisalignedOffset {
                offset,
                alignment: abi_alignment,
            });
        }
        let end = offset
            .checked_add(size)
            .ok_or(RustLayoutEvidenceError::Overflow("physical component end"))?;
        if end > MAX_RUST_LAYOUT_BYTES {
            return Err(RustLayoutEvidenceError::BoundExceeded {
                field: "physical component end",
                max: MAX_RUST_LAYOUT_BYTES,
            });
        }

        match kind {
            RustPhysicalComponentKindV1::Zst if size != 0 => {
                return Err(RustLayoutEvidenceError::InvalidComponent(
                    "a ZST component must have size zero",
                ));
            }
            RustPhysicalComponentKindV1::Scalar { .. }
            | RustPhysicalComponentKindV1::Pointer { .. }
            | RustPhysicalComponentKindV1::Usize
            | RustPhysicalComponentKindV1::Padding
                if size == 0 =>
            {
                return Err(RustLayoutEvidenceError::InvalidComponent(
                    "a pointer, usize, or padding component must have nonzero size",
                ));
            }
            RustPhysicalComponentKindV1::Padding if abi_alignment != 1 => {
                return Err(RustLayoutEvidenceError::InvalidComponent(
                    "padding must have ABI alignment one",
                ));
            }
            _ => {}
        }

        Ok(Self {
            offset,
            size,
            abi_alignment,
            kind,
        })
    }

    pub const fn offset(self) -> u64 {
        self.offset
    }

    pub const fn size(self) -> u64 {
        self.size
    }

    pub const fn abi_alignment(self) -> u32 {
        self.abi_alignment
    }

    pub const fn kind(self) -> RustPhysicalComponentKindV1 {
        self.kind
    }
}

/// Canonical source-type evidence whose identity is independent of target layout.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RustTypeEvidenceV1 {
    source_type: RustSourceTypeShapeV1,
}

impl RustTypeEvidenceV1 {
    pub const fn new(source_type: RustSourceTypeShapeV1) -> Self {
        Self { source_type }
    }

    pub const fn source_type(self) -> RustSourceTypeShapeV1 {
        self.source_type
    }

    /// Returns the complete canonical preimage used for the Rust type digest.
    pub fn canonical_bytes(self) -> Vec<u8> {
        let mut writer = CanonicalWriter::default();
        writer.frame(RUST_TYPE_EVIDENCE_DOMAIN_V1);
        writer.u16(RUST_LAYOUT_EVIDENCE_VERSION_V1);
        writer.frame(&encode_source_type(self.source_type));
        writer.finish()
    }

    /// Derives the manifest-compatible opaque Rust type identity with SHA-256.
    pub fn declared_identity(self) -> DeclaredRustTypeIdentity {
        DeclaredRustTypeIdentity::from_untrusted_bytes(
            DigestAlgorithm::Sha256
                .calculate(&self.canonical_bytes())
                .bytes(),
        )
    }
}

/// Canonical, validated rustc type-layout evidence.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RustLayoutEvidenceV1 {
    rust_type: RustTypeEvidenceV1,
    abi_class: RustcAbiClassV1,
    pointer_width: PointerWidth,
    size: u64,
    abi_alignment: u32,
    components: Vec<RustPhysicalComponentV1>,
}

impl RustLayoutEvidenceV1 {
    pub fn new(
        rust_type: RustTypeEvidenceV1,
        abi_class: RustcAbiClassV1,
        pointer_width: PointerWidth,
        size: u64,
        abi_alignment: u32,
        components: Vec<RustPhysicalComponentV1>,
    ) -> Result<Self, RustLayoutEvidenceError> {
        validate_alignment("Rust layout", abi_alignment)?;
        if size == 0 || size > MAX_RUST_LAYOUT_BYTES {
            return Err(RustLayoutEvidenceError::BoundExceeded {
                field: "Rust layout size",
                max: MAX_RUST_LAYOUT_BYTES,
            });
        }
        if !size.is_multiple_of(u64::from(abi_alignment)) {
            return Err(RustLayoutEvidenceError::InvalidLayout(
                "layout size must be a multiple of its ABI alignment",
            ));
        }
        if components.is_empty() {
            return Err(RustLayoutEvidenceError::EmptyComponents);
        }
        if components.len() > MAX_RUST_LAYOUT_COMPONENTS {
            return Err(RustLayoutEvidenceError::TooManyComponents {
                max: MAX_RUST_LAYOUT_COMPONENTS,
            });
        }

        validate_physical_coverage(size, abi_alignment, &components)?;
        validate_source_semantics(
            rust_type.source_type,
            abi_class,
            pointer_width,
            size,
            abi_alignment,
            &components,
        )?;

        Ok(Self {
            rust_type,
            abi_class,
            pointer_width,
            size,
            abi_alignment,
            components,
        })
    }

    pub const fn rust_type(&self) -> RustTypeEvidenceV1 {
        self.rust_type
    }

    pub const fn abi_class(&self) -> RustcAbiClassV1 {
        self.abi_class
    }

    pub const fn pointer_width(&self) -> PointerWidth {
        self.pointer_width
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn abi_alignment(&self) -> u32 {
        self.abi_alignment
    }

    pub fn components(&self) -> &[RustPhysicalComponentV1] {
        &self.components
    }

    /// Returns the complete canonical preimage used for the Rust layout digest.
    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut writer = CanonicalWriter::default();
        writer.frame(RUST_LAYOUT_EVIDENCE_DOMAIN_V1);
        writer.u16(RUST_LAYOUT_EVIDENCE_VERSION_V1);
        writer.frame(&self.rust_type.canonical_bytes());
        writer.u8(abi_class_tag(self.abi_class));
        writer.u8(pointer_width_tag(self.pointer_width));
        writer.u64(self.size);
        writer.u32(self.abi_alignment);
        writer.u32(self.components.len() as u32);
        for component in &self.components {
            writer.frame(&encode_component(*component));
        }
        writer.finish()
    }

    /// Derives the existing manifest `TypeIdentity` pair with SHA-256.
    pub fn type_identity(&self) -> TypeIdentity {
        let layout = DigestAlgorithm::Sha256
            .calculate(&self.canonical_bytes())
            .bytes();
        TypeIdentity::new(
            self.rust_type.declared_identity(),
            DeclaredRustLayoutIdentity::from_untrusted_bytes(layout),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RustLayoutEvidenceError {
    InvalidAlignment { field: &'static str, value: u32 },
    BoundExceeded { field: &'static str, max: u64 },
    TooManyComponents { max: usize },
    EmptyComponents,
    MisalignedOffset { offset: u64, alignment: u32 },
    Overflow(&'static str),
    InvalidComponent(&'static str),
    InvalidLayout(&'static str),
    SemanticMismatch(&'static str),
}

impl fmt::Display for RustLayoutEvidenceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidAlignment { field, value } => {
                write!(formatter, "{field} ABI alignment {value} is invalid")
            }
            Self::BoundExceeded { field, max } => {
                write!(formatter, "{field} exceeds {max} bytes")
            }
            Self::TooManyComponents { max } => {
                write!(formatter, "Rust layout exceeds {max} physical components")
            }
            Self::EmptyComponents => write!(formatter, "Rust layout has no physical components"),
            Self::MisalignedOffset { offset, alignment } => write!(
                formatter,
                "physical component offset {offset} is not aligned to {alignment}"
            ),
            Self::Overflow(field) => write!(formatter, "{field} overflows its representation"),
            Self::InvalidComponent(reason) => {
                write!(formatter, "invalid Rust physical component: {reason}")
            }
            Self::InvalidLayout(reason) => write!(formatter, "invalid Rust layout: {reason}"),
            Self::SemanticMismatch(reason) => {
                write!(formatter, "Rust source/layout semantic mismatch: {reason}")
            }
        }
    }
}

impl std::error::Error for RustLayoutEvidenceError {}

fn validate_alignment(field: &'static str, alignment: u32) -> Result<(), RustLayoutEvidenceError> {
    if alignment == 0 || !alignment.is_power_of_two() || alignment > MAX_RUST_LAYOUT_ALIGNMENT {
        return Err(RustLayoutEvidenceError::InvalidAlignment {
            field,
            value: alignment,
        });
    }
    Ok(())
}

fn validate_physical_coverage(
    size: u64,
    abi_alignment: u32,
    components: &[RustPhysicalComponentV1],
) -> Result<(), RustLayoutEvidenceError> {
    let mut covered_end = 0_u64;
    let mut maximum_alignment = 1_u32;
    for component in components {
        if component.abi_alignment > abi_alignment {
            return Err(RustLayoutEvidenceError::InvalidLayout(
                "a component is over-aligned relative to the layout",
            ));
        }
        if component.offset != covered_end {
            return Err(RustLayoutEvidenceError::InvalidLayout(
                "components must be ordered, non-overlapping, and cover every byte",
            ));
        }
        let end = component
            .offset
            .checked_add(component.size)
            .ok_or(RustLayoutEvidenceError::Overflow("physical component end"))?;
        if end > size {
            return Err(RustLayoutEvidenceError::InvalidLayout(
                "a component extends beyond the layout size",
            ));
        }
        covered_end = end;
        maximum_alignment = maximum_alignment.max(component.abi_alignment);
    }
    if covered_end != size {
        return Err(RustLayoutEvidenceError::InvalidLayout(
            "physical components do not cover the full layout size",
        ));
    }
    if maximum_alignment != abi_alignment {
        return Err(RustLayoutEvidenceError::InvalidLayout(
            "layout ABI alignment does not match its components",
        ));
    }
    Ok(())
}

fn validate_source_semantics(
    source_type: RustSourceTypeShapeV1,
    abi_class: RustcAbiClassV1,
    pointer_width: PointerWidth,
    size: u64,
    abi_alignment: u32,
    components: &[RustPhysicalComponentV1],
) -> Result<(), RustLayoutEvidenceError> {
    if let RustSourceTypeShapeV1::Scalar { scalar } = source_type {
        return validate_scalar_semantics(scalar, abi_class, size, abi_alignment, components);
    }

    let width = pointer_width.bytes();
    let expected_size = width
        .checked_mul(2)
        .ok_or(RustLayoutEvidenceError::Overflow("slice layout size"))?;
    if size != expected_size || u64::from(abi_alignment) > width {
        return Err(RustLayoutEvidenceError::SemanticMismatch(
            "slice size or ABI alignment is inconsistent with pointer width",
        ));
    }

    let (element, pointer_mutability) = match source_type {
        RustSourceTypeShapeV1::Scalar { .. } => {
            unreachable!("scalar source types return before slice validation")
        }
        RustSourceTypeShapeV1::SharedSlice { element } => (element, RustPointerMutabilityV1::Const),
        RustSourceTypeShapeV1::DisjointSlice { element, .. } => {
            (element, RustPointerMutabilityV1::Mut)
        }
    };
    if abi_class != RustcAbiClassV1::ScalarPair {
        return Err(RustLayoutEvidenceError::SemanticMismatch(
            "rustc ABI class does not match the source type shape",
        ));
    }

    if components.len() != 2 {
        return Err(RustLayoutEvidenceError::SemanticMismatch(
            "slice ABI must contain exactly one pointer and one usize",
        ));
    }
    let pointer = components[0];
    if pointer.offset != 0
        || pointer.size != width
        || pointer.kind
            != (RustPhysicalComponentKindV1::Pointer {
                mutability: pointer_mutability,
                pointee: element,
            })
    {
        return Err(RustLayoutEvidenceError::SemanticMismatch(
            "data pointer does not match the source type",
        ));
    }
    let length = components[1];
    if length.offset != width
        || length.size != width
        || length.kind != RustPhysicalComponentKindV1::Usize
    {
        return Err(RustLayoutEvidenceError::SemanticMismatch(
            "slice length is not a pointer-width usize after the data pointer",
        ));
    }
    Ok(())
}

fn validate_scalar_semantics(
    scalar: RustScalarElementTypeV1,
    abi_class: RustcAbiClassV1,
    size: u64,
    abi_alignment: u32,
    components: &[RustPhysicalComponentV1],
) -> Result<(), RustLayoutEvidenceError> {
    let scalar_size = scalar.size_bytes();
    let scalar_alignment = scalar_size as u32;
    if abi_class != RustcAbiClassV1::Scalar {
        return Err(RustLayoutEvidenceError::SemanticMismatch(
            "rustc ABI class does not match the scalar source type",
        ));
    }
    if size != scalar_size || abi_alignment != scalar_alignment {
        return Err(RustLayoutEvidenceError::SemanticMismatch(
            "scalar size or ABI alignment does not match its source type",
        ));
    }
    if components
        != [RustPhysicalComponentV1 {
            offset: 0,
            size: scalar_size,
            abi_alignment: scalar_alignment,
            kind: RustPhysicalComponentKindV1::Scalar { scalar },
        }]
    {
        return Err(RustLayoutEvidenceError::SemanticMismatch(
            "scalar physical component does not match its source type",
        ));
    }
    Ok(())
}

fn encode_source_type(source_type: RustSourceTypeShapeV1) -> Vec<u8> {
    let mut writer = CanonicalWriter::default();
    match source_type {
        RustSourceTypeShapeV1::Scalar { scalar } => {
            writer.u8(3);
            writer.u8(scalar_tag(scalar));
        }
        RustSourceTypeShapeV1::SharedSlice { element } => {
            writer.u8(1);
            writer.u8(scalar_tag(element));
        }
        RustSourceTypeShapeV1::DisjointSlice {
            element,
            index_space,
        } => {
            writer.u8(2);
            writer.u8(scalar_tag(element));
            encode_index_space(&mut writer, index_space);
        }
    }
    writer.finish()
}

fn encode_component(component: RustPhysicalComponentV1) -> Vec<u8> {
    let mut writer = CanonicalWriter::default();
    writer.u64(component.offset);
    writer.u64(component.size);
    writer.u32(component.abi_alignment);
    match component.kind {
        RustPhysicalComponentKindV1::Scalar { scalar } => {
            writer.u8(5);
            writer.u8(scalar_tag(scalar));
        }
        RustPhysicalComponentKindV1::Pointer {
            mutability,
            pointee,
        } => {
            writer.u8(1);
            writer.u8(pointer_mutability_tag(mutability));
            writer.u8(scalar_tag(pointee));
        }
        RustPhysicalComponentKindV1::Usize => writer.u8(2),
        RustPhysicalComponentKindV1::Zst => {
            writer.u8(3);
        }
        RustPhysicalComponentKindV1::Padding => writer.u8(4),
    }
    writer.finish()
}

const fn scalar_tag(scalar: RustScalarElementTypeV1) -> u8 {
    match scalar {
        RustScalarElementTypeV1::I8 => 1,
        RustScalarElementTypeV1::U8 => 2,
        RustScalarElementTypeV1::I16 => 3,
        RustScalarElementTypeV1::U16 => 4,
        RustScalarElementTypeV1::I32 => 5,
        RustScalarElementTypeV1::U32 => 6,
        RustScalarElementTypeV1::I64 => 7,
        RustScalarElementTypeV1::U64 => 8,
        RustScalarElementTypeV1::F16 => 9,
        RustScalarElementTypeV1::F32 => 10,
        RustScalarElementTypeV1::F64 => 11,
    }
}

fn encode_index_space(writer: &mut CanonicalWriter, index_space: RustDisjointIndexSpaceV1) {
    match index_space {
        RustDisjointIndexSpaceV1::Index1D => writer.u8(1),
        RustDisjointIndexSpaceV1::ShiftedIndex1D { offset } => {
            writer.u8(2);
            writer.u64(offset);
        }
        RustDisjointIndexSpaceV1::GridExclusive => writer.u8(3),
        RustDisjointIndexSpaceV1::BlockedIndex1D {
            lanes_per_block,
            elements_per_lane,
        } => {
            writer.u8(4);
            writer.u64(lanes_per_block.get());
            writer.u64(elements_per_lane.get());
        }
    }
}

const fn abi_class_tag(abi_class: RustcAbiClassV1) -> u8 {
    match abi_class {
        RustcAbiClassV1::Scalar => 1,
        RustcAbiClassV1::ScalarPair => 2,
        RustcAbiClassV1::Aggregate => 3,
        RustcAbiClassV1::Uninhabited => 4,
    }
}

const fn pointer_width_tag(pointer_width: PointerWidth) -> u8 {
    match pointer_width {
        PointerWidth::Bits32 => 1,
        PointerWidth::Bits64 => 2,
    }
}

const fn pointer_mutability_tag(mutability: RustPointerMutabilityV1) -> u8 {
    match mutability {
        RustPointerMutabilityV1::Const => 1,
        RustPointerMutabilityV1::Mut => 2,
    }
}

#[derive(Default)]
struct CanonicalWriter {
    bytes: Vec<u8>,
}

impl CanonicalWriter {
    fn u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    fn u16(&mut self, value: u16) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_le_bytes());
    }

    fn frame(&mut self, value: &[u8]) {
        let length = u32::try_from(value.len()).expect("bounded canonical record length fits u32");
        self.u32(length);
        self.bytes.extend_from_slice(value);
    }

    fn finish(self) -> Vec<u8> {
        self.bytes
    }
}
