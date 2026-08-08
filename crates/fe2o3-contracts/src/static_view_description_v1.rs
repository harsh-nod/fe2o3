//! Non-authoritative descriptions of fixed-size memory regions.
//!
//! These records check only arithmetic and internal coherence. Every identity,
//! address, region, and access mode is supplied by the caller. A coherent
//! description is not runtime access authority, a proof fact, an artifact
//! binding, or evidence that the described allocation exists.

use core::fmt;

use crate::{AllocationSpecV1, ByteRegionV1};

/// Largest fixed element count represented by a V1 description.
pub const MAX_STATIC_VIEW_ELEMENTS_V1: u64 = u32::MAX as u64;

/// Caller-described access mode for a symbolic region.
///
/// `ExclusiveWrite` describes a claim. It does not establish Rust exclusivity,
/// GPU cross-invocation partitioning, synchronization, or write authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StaticViewAccessDescriptionV1 {
    SharedRead,
    ExclusiveWrite,
}

/// Why symbolic static-view inputs are internally incoherent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StaticViewDescriptionErrorV1 {
    EmptyParent,
    EmptyView,
    ElementCountBoundExceeded {
        actual: u64,
        maximum: u64,
    },
    ZeroSizedElement,
    InvalidElementAlignment {
        alignment: u64,
    },
    ElementLayoutMismatch {
        element_size: u64,
        alignment: u64,
    },
    ParentRegionOutsideDescribedAllocation,
    ParentExtentOverflow,
    ParentExtentMismatch {
        expected: u64,
        actual: u64,
    },
    MisalignedParentRegion {
        address: u64,
        alignment: u64,
    },
    ElementRangeOverflow,
    ElementRangeOutsideParent {
        start: u64,
        count: u64,
        parent_count: u64,
    },
    RegionArithmeticOverflow,
    ElementIndexOutsideView {
        index: u64,
        count: u64,
    },
}

impl fmt::Display for StaticViewDescriptionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "incoherent static-view description: {self:?}")
    }
}

/// Internally coherent, caller-authored description of a fixed-size region.
///
/// This type deliberately does not implement [`crate::SpecificationFactV1`].
/// Construction proves only that the supplied numbers agree with each other.
/// In particular, the provenance ID and `ExclusiveWrite` access description
/// remain freely chosen symbolic data and must never be converted into a
/// runtime capability, proof result, launch authorization, or artifact claim.
///
/// ```compile_fail
/// use fe2o3_contracts::{SpecificationFactV1, StaticViewDescriptionV1};
/// fn require_fact<T: SpecificationFactV1>() {}
/// require_fact::<StaticViewDescriptionV1>();
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticViewDescriptionV1 {
    described_allocation: AllocationSpecV1,
    described_parent_region: ByteRegionV1,
    described_region: ByteRegionV1,
    parent_element_count: u64,
    start_element: u64,
    element_count: u64,
    element_size: u64,
    element_alignment: u64,
    access: StaticViewAccessDescriptionV1,
}

impl StaticViewDescriptionV1 {
    /// Creates an internally coherent description from caller-supplied data.
    ///
    /// Success grants no authority and authenticates none of the inputs.
    #[allow(clippy::too_many_arguments)]
    pub const fn describe(
        described_allocation: AllocationSpecV1,
        described_parent_region: ByteRegionV1,
        parent_element_count: u64,
        start_element: u64,
        element_count: u64,
        element_size: u64,
        element_alignment: u64,
        access: StaticViewAccessDescriptionV1,
    ) -> Result<Self, StaticViewDescriptionErrorV1> {
        if parent_element_count == 0 {
            return Err(StaticViewDescriptionErrorV1::EmptyParent);
        }
        if element_count == 0 {
            return Err(StaticViewDescriptionErrorV1::EmptyView);
        }
        if parent_element_count > MAX_STATIC_VIEW_ELEMENTS_V1 {
            return Err(StaticViewDescriptionErrorV1::ElementCountBoundExceeded {
                actual: parent_element_count,
                maximum: MAX_STATIC_VIEW_ELEMENTS_V1,
            });
        }
        if element_count > MAX_STATIC_VIEW_ELEMENTS_V1 {
            return Err(StaticViewDescriptionErrorV1::ElementCountBoundExceeded {
                actual: element_count,
                maximum: MAX_STATIC_VIEW_ELEMENTS_V1,
            });
        }
        if element_size == 0 {
            return Err(StaticViewDescriptionErrorV1::ZeroSizedElement);
        }
        if !element_alignment.is_power_of_two() {
            return Err(StaticViewDescriptionErrorV1::InvalidElementAlignment {
                alignment: element_alignment,
            });
        }
        if !element_size.is_multiple_of(element_alignment) {
            return Err(StaticViewDescriptionErrorV1::ElementLayoutMismatch {
                element_size,
                alignment: element_alignment,
            });
        }
        if !described_allocation.contains(described_parent_region) {
            return Err(StaticViewDescriptionErrorV1::ParentRegionOutsideDescribedAllocation);
        }

        let parent_bytes = match parent_element_count.checked_mul(element_size) {
            Some(bytes) => bytes,
            None => return Err(StaticViewDescriptionErrorV1::ParentExtentOverflow),
        };
        if parent_bytes != described_parent_region.byte_length() {
            return Err(StaticViewDescriptionErrorV1::ParentExtentMismatch {
                expected: parent_bytes,
                actual: described_parent_region.byte_length(),
            });
        }

        let parent_address = match described_allocation
            .base_address()
            .checked_add(described_parent_region.byte_offset())
        {
            Some(address) => address,
            None => return Err(StaticViewDescriptionErrorV1::RegionArithmeticOverflow),
        };
        if !parent_address.is_multiple_of(element_alignment) {
            return Err(StaticViewDescriptionErrorV1::MisalignedParentRegion {
                address: parent_address,
                alignment: element_alignment,
            });
        }

        let end_element = match start_element.checked_add(element_count) {
            Some(end) => end,
            None => return Err(StaticViewDescriptionErrorV1::ElementRangeOverflow),
        };
        if end_element > parent_element_count {
            return Err(StaticViewDescriptionErrorV1::ElementRangeOutsideParent {
                start: start_element,
                count: element_count,
                parent_count: parent_element_count,
            });
        }

        let relative_offset = match start_element.checked_mul(element_size) {
            Some(offset) => offset,
            None => return Err(StaticViewDescriptionErrorV1::RegionArithmeticOverflow),
        };
        let byte_length = match element_count.checked_mul(element_size) {
            Some(length) => length,
            None => return Err(StaticViewDescriptionErrorV1::RegionArithmeticOverflow),
        };
        let byte_offset = match described_parent_region
            .byte_offset()
            .checked_add(relative_offset)
        {
            Some(offset) => offset,
            None => return Err(StaticViewDescriptionErrorV1::RegionArithmeticOverflow),
        };
        let described_region = match ByteRegionV1::new(
            described_parent_region.provenance(),
            described_parent_region.address_space(),
            byte_offset,
            byte_length,
        ) {
            Ok(region) => region,
            Err(_) => return Err(StaticViewDescriptionErrorV1::RegionArithmeticOverflow),
        };

        Ok(Self {
            described_allocation,
            described_parent_region,
            described_region,
            parent_element_count,
            start_element,
            element_count,
            element_size,
            element_alignment,
            access,
        })
    }

    pub const fn described_allocation(self) -> AllocationSpecV1 {
        self.described_allocation
    }

    pub const fn described_parent_region(self) -> ByteRegionV1 {
        self.described_parent_region
    }

    pub const fn described_region(self) -> ByteRegionV1 {
        self.described_region
    }

    pub const fn parent_element_count(self) -> u64 {
        self.parent_element_count
    }

    pub const fn start_element(self) -> u64 {
        self.start_element
    }

    pub const fn element_count(self) -> u64 {
        self.element_count
    }

    pub const fn element_size(self) -> u64 {
        self.element_size
    }

    pub const fn element_alignment(self) -> u64 {
        self.element_alignment
    }

    pub const fn access_description(self) -> StaticViewAccessDescriptionV1 {
        self.access
    }

    pub const fn contains_element_index(self, index: u64) -> bool {
        index < self.element_count
    }

    pub const fn described_element_region(
        self,
        index: u64,
    ) -> Result<ByteRegionV1, StaticViewDescriptionErrorV1> {
        if !self.contains_element_index(index) {
            return Err(StaticViewDescriptionErrorV1::ElementIndexOutsideView {
                index,
                count: self.element_count,
            });
        }
        let relative_offset = match index.checked_mul(self.element_size) {
            Some(offset) => offset,
            None => return Err(StaticViewDescriptionErrorV1::RegionArithmeticOverflow),
        };
        let byte_offset = match self
            .described_region
            .byte_offset()
            .checked_add(relative_offset)
        {
            Some(offset) => offset,
            None => return Err(StaticViewDescriptionErrorV1::RegionArithmeticOverflow),
        };
        match ByteRegionV1::new(
            self.described_region.provenance(),
            self.described_region.address_space(),
            byte_offset,
            self.element_size,
        ) {
            Ok(region) => Ok(region),
            Err(_) => Err(StaticViewDescriptionErrorV1::RegionArithmeticOverflow),
        }
    }
}
