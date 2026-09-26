//! Inert storage layout records. No wire version is allocated here.
//! These public records carry no source, module, target or runtime authority.
//! Positional IDs are meaningful only with their exact owning layout table.

use crate::{AccessMode, AddressSpace, FixedVectorTypeV12, ScalarType};

/// An inert row ordinal, never a cross-module type or authority identity.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct StorageLayoutIdV1(pub u32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StorageFieldV1 {
    pub offset: u64,
    pub layout: StorageLayoutIdV1,
}

impl StorageFieldV1 {
    /// Placement may reduce alignment for packed fields. This is an inert
    /// arithmetic fact, conditional on the containing object's actual alignment.
    pub fn placement_alignment(self, parent_alignment: u32) -> Option<u32> {
        if !parent_alignment.is_power_of_two() {
            return None;
        }
        if self.offset == 0 {
            return Some(parent_alignment);
        }
        Some(parent_alignment.min(1_u32 << self.offset.trailing_zeros().min(31)))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StoragePointerV1 {
    /// Referent layout in the same owning table.
    pub pointee: StorageLayoutIdV1,
    /// Provenance address space of the contained pointer value.
    pub value_space: AddressSpace,
    /// Address-space representation used for its stored bits, not holder space.
    pub encoded_space: AddressSpace,
    pub access: AccessMode,
    pub stored_bits: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StorageVariantEncodingV1 {
    Direct {
        tag: StorageFieldV1,
    },
    Niche {
        tag: StorageFieldV1,
        untagged_variant: u32,
        first_niche_variant: u32,
        last_niche_variant: u32,
        niche_start: u128,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageVariantV1 {
    /// Logical source discriminant identity, not its physical tag encoding.
    pub discriminant: u128,
    /// Present only for direct encodings; signed tags use their width's bits.
    pub direct_tag_bits: Option<u128>,
    /// An inert source-layout fact; only the source owner can authenticate it.
    pub uninhabited: bool,
    pub layout: StorageLayoutIdV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StorageLayoutKindV1 {
    Scalar(ScalarType),
    Vector(FixedVectorTypeV12),
    Pointer(StoragePointerV1),
    Record(Box<[StorageFieldV1]>),
    Union(Box<[StorageFieldV1]>),
    Array {
        element: StorageLayoutIdV1,
        length: u64,
        stride: u64,
    },
    Slice {
        element: StorageLayoutIdV1,
        value_space: AddressSpace,
        access: AccessMode,
        data: StorageFieldV1,
        length: StorageFieldV1,
    },
    Variants {
        encoding: StorageVariantEncodingV1,
        variants: Box<[StorageVariantV1]>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StorageLayoutV1 {
    pub size: u64,
    pub alignment: u32,
    pub kind: StorageLayoutKindV1,
}

impl StorageLayoutKindV1 {
    pub(crate) fn containment_count(&self) -> Option<usize> {
        match self {
            Self::Scalar(_) | Self::Vector(_) | Self::Pointer(_) => Some(0),
            Self::Record(fields) | Self::Union(fields) => Some(fields.len()),
            Self::Array { .. } => Some(1),
            Self::Slice { .. } => Some(2),
            Self::Variants { variants, .. } => variants.len().checked_add(1),
        }
    }

    pub(crate) fn containment_child(&self, index: usize) -> Option<StorageLayoutIdV1> {
        match self {
            Self::Scalar(_) | Self::Vector(_) | Self::Pointer(_) => None,
            Self::Record(fields) | Self::Union(fields) => fields.get(index).map(|f| f.layout),
            Self::Array { element, .. } => (index == 0).then_some(*element),
            Self::Slice { data, length, .. } => match index {
                0 => Some(data.layout),
                1 => Some(length.layout),
                _ => None,
            },
            Self::Variants { encoding, variants } => {
                if index == variants.len() {
                    Some(encoding.tag().layout)
                } else {
                    variants.get(index).map(|v| v.layout)
                }
            }
        }
    }
}

impl StorageVariantEncodingV1 {
    pub const fn tag(self) -> StorageFieldV1 {
        match self {
            Self::Direct { tag } | Self::Niche { tag, .. } => tag,
        }
    }
}

#[cfg(test)]
#[path = "storage_layout_v1_tests.rs"]
mod tests;
