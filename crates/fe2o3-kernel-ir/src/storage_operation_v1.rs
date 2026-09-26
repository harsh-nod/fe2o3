//! Typed operations over the current module's physical storage-layout table.
//! This carrier records operations and obligations, never source/runtime authority.

use crate::{MemoryAccess, MemoryEffect, ValueId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageProjectionV1 {
    /// Record/union field ordinal; slice roles are zero=data and one=length.
    Field(u32),
    /// Index-typed SSA operand. Execution must check the declared array bound.
    ArrayIndex(ValueId),
    /// Reads and validates the actual active tag before creating a variant view.
    /// This is not an initialization operation or a trusted active-variant flag.
    Variant { index: u32, access: MemoryAccess },
    /// Selects an inhabited payload layout for construction without reading its
    /// tag. The result is strictly WriteOnly and grants no active-variant proof.
    /// Runtime views retain the parent's bounds, provenance and enclosing guards.
    VariantForWrite { index: u32 },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageCopyOverlapV1 {
    /// Requires actual source/destination ranges to be disjoint.
    NonOverlapping,
    /// Snapshots bytes, initialization and complete relocation state before commit.
    MayOverlap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageMemoryAccessKindV1 {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageOperationV1 {
    Project {
        base: ValueId,
        step: StorageProjectionV1,
    },
    ReadValue {
        address: ValueId,
        access: MemoryAccess,
    },
    /// Reads and validates the current tag, returning the selected table row's
    /// logical discriminant as U128 raw bits, never its ordinal or encoded tag.
    /// Only actual tag bytes are read. Initialization, validity, enclosing view
    /// guards and pointer provenance remain ordered, potentially trapping checks.
    /// Access is nonvolatile; this is not a pure value or an active-payload permit.
    ReadDiscriminant {
        address: ValueId,
        access: MemoryAccess,
    },
    WriteValue {
        address: ValueId,
        value: ValueId,
        access: MemoryAccess,
    },
    CopyObject {
        source: ValueId,
        destination: ValueId,
        source_access: MemoryAccess,
        destination_access: MemoryAccess,
        overlap: StorageCopyOverlapV1,
    },
    /// Preserves source statement order; this is not an initialization barrier.
    /// Direct tags store direct_tag_bits at the declared tag width. Encoded niche
    /// variants store (niche_start + variant - first) modulo that width. These
    /// stores initialize only their actual tag bytes and invalidate overlapping
    /// relocations/active-tag guards, even when the bits are unchanged.
    /// The untagged niche variant performs no physical read or write, including
    /// no validity trap, relocation change or active-variant certification.
    /// Access is nonvolatile. Untouched payload, padding and initialization state
    /// remain unchanged. Pointer niches never manufacture provenance from bits.
    SetDiscriminant {
        address: ValueId,
        variant: u32,
        access: MemoryAccess,
    },
}

impl StorageOperationV1 {
    pub const fn operand_count(self) -> usize {
        match self {
            Self::Project {
                step: StorageProjectionV1::ArrayIndex(_),
                ..
            }
            | Self::WriteValue { .. }
            | Self::CopyObject { .. } => 2,
            Self::Project { .. } | Self::ReadValue { .. } | Self::ReadDiscriminant { .. }
            | Self::SetDiscriminant { .. } => 1,
        }
    }

    /// Stable semantic order, with no allocation and immediate visitor refusal.
    pub fn try_visit_operands<E>(
        &self,
        mut visitor: impl FnMut(ValueId) -> Result<(), E>,
    ) -> Result<(), E> {
        match *self {
            Self::Project { base, step } => {
                visitor(base)?;
                if let StorageProjectionV1::ArrayIndex(index) = step {
                    visitor(index)?;
                }
            }
            Self::ReadValue { address, .. } | Self::ReadDiscriminant { address, .. }
            | Self::SetDiscriminant { address, .. } => {
                visitor(address)?;
            }
            Self::WriteValue { address, value, .. } => {
                visitor(address)?;
                visitor(value)?;
            }
            Self::CopyObject {
                source,
                destination,
                ..
            } => {
                visitor(source)?;
                visitor(destination)?;
            }
        }
        Ok(())
    }

    /// Addresses are actual operands, not allocation identities. The precise
    /// ranges come from the same module's checked row and projection state.
    pub fn try_visit_memory_accesses<E>(
        &self,
        mut visitor: impl FnMut(ValueId, MemoryAccess, StorageMemoryAccessKindV1) -> Result<(), E>,
    ) -> Result<(), E> {
        use StorageMemoryAccessKindV1::{Read, Write};
        match *self {
            Self::Project {
                base,
                step: StorageProjectionV1::Variant { access, .. },
            } => visitor(base, access, Read)?,
            Self::Project { .. } => {}
            Self::ReadValue { address, access } | Self::ReadDiscriminant { address, access } => visitor(address, access, Read)?,
            Self::WriteValue {
                address, access, ..
            }
            | Self::SetDiscriminant {
                address, access, ..
            } => visitor(address, access, Write)?,
            Self::CopyObject {
                source,
                destination,
                source_access,
                destination_access,
                ..
            } => {
                visitor(source, source_access, Read)?;
                visitor(destination, destination_access, Write)?;
            }
        }
        Ok(())
    }

    /// Coarse existing memory effects; this does not discharge bounds, active
    /// variants, relocation validity, overlap or complete region extraction.
    /// SetDiscriminant conservatively reports Write here: only the owning layout
    /// can distinguish a real tag store from an untagged-niche physical no-op.
    pub fn try_visit_memory_effects<E>(
        &self,
        mut visitor: impl FnMut(MemoryEffect) -> Result<(), E>,
    ) -> Result<(), E> {
        self.try_visit_memory_accesses(|_, access, kind| {
            visitor(match (kind, access.volatile) {
                (StorageMemoryAccessKindV1::Read, false) => {
                    MemoryEffect::Read(access.address_space)
                }
                (StorageMemoryAccessKindV1::Write, false) => {
                    MemoryEffect::Write(access.address_space)
                }
                (StorageMemoryAccessKindV1::Read, true) => {
                    MemoryEffect::VolatileRead(access.address_space)
                }
                (StorageMemoryAccessKindV1::Write, true) => {
                    MemoryEffect::VolatileWrite(access.address_space)
                }
            })
        })
    }
}

#[cfg(test)]
#[path = "storage_operation_v1_tests.rs"]
mod tests;
