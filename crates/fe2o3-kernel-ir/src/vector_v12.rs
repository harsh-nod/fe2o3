//! Target-neutral fixed-lane vector, memory-access, and layout carriers for Kernel IR V12.

use std::{error::Error, fmt};

use crate::{MemoryAccess, ScalarType, ValueId};

/// Largest fixed lane count admitted by the V12 carrier and canonical wire.
pub const MAX_FIXED_VECTOR_LANES_V12: u16 = 1024;

/// A target-neutral physical lane arrangement.
///
/// Logical lane values are invariant under layout conversion. `Interleaved`
/// groups physical lanes by `logical_lane % factor`; it is legal only when the
/// factor divides the lane count exactly.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VectorLayoutV12 {
    /// Logical and physical lane order are identical.
    Contiguous,
    /// Physical lanes are grouped into `factor` interleaved sets.
    Interleaved { factor: u16 },
}

/// A first-class fixed-lane vector type with an explicit physical layout.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FixedVectorTypeV12 {
    pub element: ScalarType,
    pub lanes: u16,
    pub layout: VectorLayoutV12,
}

impl FixedVectorTypeV12 {
    pub const fn new(element: ScalarType, lanes: u16, layout: VectorLayoutV12) -> Self {
        Self {
            element,
            lanes,
            layout,
        }
    }

    /// Checks target-neutral type and layout legality.
    pub fn validate(self) -> Result<(), FixedVectorTypeErrorV12> {
        if !(2..=MAX_FIXED_VECTOR_LANES_V12).contains(&self.lanes) {
            return Err(FixedVectorTypeErrorV12::LaneCount {
                lanes: self.lanes,
                maximum: MAX_FIXED_VECTOR_LANES_V12,
            });
        }
        let Some(bits) = self.element.bit_width() else {
            return Err(FixedVectorTypeErrorV12::Element(self.element));
        };
        if !self.element.is_numeric() || bits % 8 != 0 {
            return Err(FixedVectorTypeErrorV12::Element(self.element));
        }
        if let VectorLayoutV12::Interleaved { factor } = self.layout
            && (factor < 2 || factor >= self.lanes || !self.lanes.is_multiple_of(factor))
        {
            return Err(FixedVectorTypeErrorV12::Interleave {
                lanes: self.lanes,
                factor,
            });
        }
        Ok(())
    }

    /// Exact byte width when the vector type is legal.
    pub fn byte_width(self) -> Option<u32> {
        self.validate().ok()?;
        let element_bytes = u32::from(self.element.bit_width()? / 8);
        element_bytes.checked_mul(u32::from(self.lanes))
    }

    /// Returns the same logical vector under a different physical layout.
    pub const fn with_layout(self, layout: VectorLayoutV12) -> Self {
        Self { layout, ..self }
    }
}

/// Stable legality error for a fixed vector descriptor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixedVectorTypeErrorV12 {
    LaneCount { lanes: u16, maximum: u16 },
    Element(ScalarType),
    Interleave { lanes: u16, factor: u16 },
}

impl fmt::Display for FixedVectorTypeErrorV12 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LaneCount { lanes, maximum } => write!(
                formatter,
                "fixed vector lane count {lanes} is outside 2..={maximum}"
            ),
            Self::Element(element) => write!(
                formatter,
                "fixed vector element {element:?} has no target-neutral whole-byte width"
            ),
            Self::Interleave { lanes, factor } => write!(
                formatter,
                "interleave factor {factor} must divide {lanes} lanes and be in 2..{lanes}"
            ),
        }
    }
}

impl Error for FixedVectorTypeErrorV12 {}

/// The exact provenance source for a vector memory operation.
///
/// V12 admits only an SSA pointer. It does not allow an operation to invent an
/// allocation identity independently of the pointer derivation already modeled
/// by Kernel IR.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum VectorAccessProvenanceV12 {
    Pointer(ValueId),
}

impl VectorAccessProvenanceV12 {
    pub const fn pointer(self) -> ValueId {
        match self {
            Self::Pointer(pointer) => pointer,
        }
    }
}

/// Exact type, layout, address-space, alignment, and volatility of a vector access.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VectorMemoryAccessV12 {
    pub vector: FixedVectorTypeV12,
    pub memory: MemoryAccess,
}

impl VectorMemoryAccessV12 {
    pub const fn new(vector: FixedVectorTypeV12, memory: MemoryAccess) -> Self {
        Self { vector, memory }
    }
}

/// Explicit fixed-vector load from a provenance-bearing scalar pointer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VectorLoadOperationV12 {
    pub provenance: VectorAccessProvenanceV12,
    pub access: VectorMemoryAccessV12,
}

impl VectorLoadOperationV12 {
    pub const fn new(pointer: ValueId, access: VectorMemoryAccessV12) -> Self {
        Self {
            provenance: VectorAccessProvenanceV12::Pointer(pointer),
            access,
        }
    }
}

/// Explicit fixed-vector store to a provenance-bearing scalar pointer.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VectorStoreOperationV12 {
    pub provenance: VectorAccessProvenanceV12,
    pub value: ValueId,
    pub access: VectorMemoryAccessV12,
}

impl VectorStoreOperationV12 {
    pub const fn new(pointer: ValueId, value: ValueId, access: VectorMemoryAccessV12) -> Self {
        Self {
            provenance: VectorAccessProvenanceV12::Pointer(pointer),
            value,
            access,
        }
    }
}

/// Layout-only conversion preserving logical lanes, element type, and lane count.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VectorLayoutConversionV12 {
    pub value: ValueId,
    pub to: VectorLayoutV12,
}

impl VectorLayoutConversionV12 {
    pub const fn new(value: ValueId, to: VectorLayoutV12) -> Self {
        Self { value, to }
    }
}
