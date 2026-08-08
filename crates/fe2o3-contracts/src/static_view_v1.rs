//! Pure contracts for fixed-size views derived from bounded allocations.
//!
//! These records are proof inputs. They do not authenticate allocation
//! provenance or grant runtime memory access.

use core::fmt;

use crate::memory_v1::sealed;
use crate::{AllocationSpecV1, ByteRegionV1, PermissionKindV1, SpecificationFactV1};

/// Largest fixed element count admitted by the V1 contract.
pub const MAX_STATIC_VIEW_ELEMENTS_V1: u64 = u32::MAX as u64;

/// Why a fixed-size view contract could not be constructed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StaticViewContractErrorV1 {
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
    ParentRegionOutsideAllocation,
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

impl fmt::Display for StaticViewContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid static-view contract: {self:?}")
    }
}

/// Exact pure-data contract for one nonempty fixed-size view.
///
/// `parent_region` identifies the complete runtime parent view. `region` is
/// derived from `start_element`, `element_count`, and the element layout. Both
/// retain the same allocation provenance and address-space identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticViewContractV1 {
    allocation: AllocationSpecV1,
    parent_region: ByteRegionV1,
    region: ByteRegionV1,
    parent_element_count: u64,
    start_element: u64,
    element_count: u64,
    element_size: u64,
    element_alignment: u64,
    permission: PermissionKindV1,
}

impl StaticViewContractV1 {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        allocation: AllocationSpecV1,
        parent_region: ByteRegionV1,
        parent_element_count: u64,
        start_element: u64,
        element_count: u64,
        element_size: u64,
        element_alignment: u64,
        permission: PermissionKindV1,
    ) -> Result<Self, StaticViewContractErrorV1> {
        if parent_element_count == 0 {
            return Err(StaticViewContractErrorV1::EmptyParent);
        }
        if element_count == 0 {
            return Err(StaticViewContractErrorV1::EmptyView);
        }
        if parent_element_count > MAX_STATIC_VIEW_ELEMENTS_V1 {
            return Err(StaticViewContractErrorV1::ElementCountBoundExceeded {
                actual: parent_element_count,
                maximum: MAX_STATIC_VIEW_ELEMENTS_V1,
            });
        }
        if element_count > MAX_STATIC_VIEW_ELEMENTS_V1 {
            return Err(StaticViewContractErrorV1::ElementCountBoundExceeded {
                actual: element_count,
                maximum: MAX_STATIC_VIEW_ELEMENTS_V1,
            });
        }
        if element_size == 0 {
            return Err(StaticViewContractErrorV1::ZeroSizedElement);
        }
        if !element_alignment.is_power_of_two() {
            return Err(StaticViewContractErrorV1::InvalidElementAlignment {
                alignment: element_alignment,
            });
        }
        if !element_size.is_multiple_of(element_alignment) {
            return Err(StaticViewContractErrorV1::ElementLayoutMismatch {
                element_size,
                alignment: element_alignment,
            });
        }
        if !allocation.contains(parent_region) {
            return Err(StaticViewContractErrorV1::ParentRegionOutsideAllocation);
        }

        let parent_bytes = match parent_element_count.checked_mul(element_size) {
            Some(bytes) => bytes,
            None => return Err(StaticViewContractErrorV1::ParentExtentOverflow),
        };
        if parent_bytes != parent_region.byte_length() {
            return Err(StaticViewContractErrorV1::ParentExtentMismatch {
                expected: parent_bytes,
                actual: parent_region.byte_length(),
            });
        }

        let parent_address = match allocation
            .base_address()
            .checked_add(parent_region.byte_offset())
        {
            Some(address) => address,
            None => return Err(StaticViewContractErrorV1::RegionArithmeticOverflow),
        };
        if parent_address % element_alignment != 0 {
            return Err(StaticViewContractErrorV1::MisalignedParentRegion {
                address: parent_address,
                alignment: element_alignment,
            });
        }

        let end_element = match start_element.checked_add(element_count) {
            Some(end) => end,
            None => return Err(StaticViewContractErrorV1::ElementRangeOverflow),
        };
        if end_element > parent_element_count {
            return Err(StaticViewContractErrorV1::ElementRangeOutsideParent {
                start: start_element,
                count: element_count,
                parent_count: parent_element_count,
            });
        }

        let relative_offset = match start_element.checked_mul(element_size) {
            Some(offset) => offset,
            None => return Err(StaticViewContractErrorV1::RegionArithmeticOverflow),
        };
        let byte_length = match element_count.checked_mul(element_size) {
            Some(length) => length,
            None => return Err(StaticViewContractErrorV1::RegionArithmeticOverflow),
        };
        let byte_offset = match parent_region.byte_offset().checked_add(relative_offset) {
            Some(offset) => offset,
            None => return Err(StaticViewContractErrorV1::RegionArithmeticOverflow),
        };
        let region = match ByteRegionV1::new(
            parent_region.provenance(),
            parent_region.address_space(),
            byte_offset,
            byte_length,
        ) {
            Ok(region) => region,
            Err(_) => return Err(StaticViewContractErrorV1::RegionArithmeticOverflow),
        };

        Ok(Self {
            allocation,
            parent_region,
            region,
            parent_element_count,
            start_element,
            element_count,
            element_size,
            element_alignment,
            permission,
        })
    }

    pub const fn allocation(self) -> AllocationSpecV1 {
        self.allocation
    }

    pub const fn parent_region(self) -> ByteRegionV1 {
        self.parent_region
    }

    pub const fn region(self) -> ByteRegionV1 {
        self.region
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

    pub const fn permission(self) -> PermissionKindV1 {
        self.permission
    }

    pub const fn contains_element_index(self, index: u64) -> bool {
        index < self.element_count
    }

    pub const fn element_region(
        self,
        index: u64,
    ) -> Result<ByteRegionV1, StaticViewContractErrorV1> {
        if !self.contains_element_index(index) {
            return Err(StaticViewContractErrorV1::ElementIndexOutsideView {
                index,
                count: self.element_count,
            });
        }
        let relative_offset = match index.checked_mul(self.element_size) {
            Some(offset) => offset,
            None => return Err(StaticViewContractErrorV1::RegionArithmeticOverflow),
        };
        let byte_offset = match self.region.byte_offset().checked_add(relative_offset) {
            Some(offset) => offset,
            None => return Err(StaticViewContractErrorV1::RegionArithmeticOverflow),
        };
        match ByteRegionV1::new(
            self.region.provenance(),
            self.region.address_space(),
            byte_offset,
            self.element_size,
        ) {
            Ok(region) => Ok(region),
            Err(_) => Err(StaticViewContractErrorV1::RegionArithmeticOverflow),
        }
    }
}

impl sealed::Sealed for StaticViewContractV1 {}
impl SpecificationFactV1 for StaticViewContractV1 {}
