//! Fixed-size views checked against allocation and byte-region contracts.
//!
//! The views borrow an existing [`crate::DisjointSlice`]. Construction checks
//! the type-level extent once; constant access then requires no runtime bounds
//! branch. The embedded contract remains specification data and grants no
//! allocation, proof, artifact, or launch authority.

use core::fmt;
use core::marker::PhantomData;
use core::mem::{align_of, size_of};

use fe2o3_contracts::{
    AllocationSpecV1, ByteRegionV1, PermissionKindV1, StaticViewContractErrorV1,
    StaticViewContractV1,
};

use crate::{DisjointSlice, Index1D};

/// Zero-sized proof that constant `I` lies within fixed extent `N`.
///
/// Safe code can construct this witness only through [`Self::CHECKED`], whose
/// assertion is evaluated by rustc at the use site. Passing the witness to a
/// static view access therefore adds no runtime data or bounds branch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StaticIndex<const N: usize, const I: usize> {
    _private: (),
}

impl<const N: usize, const I: usize> StaticIndex<N, I> {
    pub const CHECKED: Self = {
        assert!(I < N, "static view index is out of bounds");
        Self { _private: () }
    };

    pub const fn get(self) -> usize {
        I
    }
}

/// Failure to derive a runtime static view from its parent capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StaticViewError {
    Contract(StaticViewContractErrorV1),
    ParentLengthNotRepresentable,
    ElementLayoutNotRepresentable,
    ParentExtentTooLarge { bytes: u64 },
    NullParent,
    ParentPointerAddressNotRepresentable,
    ParentPointerMismatch { expected: u64, actual: u64 },
}

impl fmt::Display for StaticViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid static view: {self:?}")
    }
}

impl From<StaticViewContractErrorV1> for StaticViewError {
    fn from(error: StaticViewContractErrorV1) -> Self {
        Self::Contract(error)
    }
}

/// Shared fixed-size view derived from a borrowed parent capability.
///
/// Fields are private so safe code cannot forge pointer provenance. This type
/// intentionally has no raw-parts constructor. Its lifetime and `IndexSpace`
/// retain the parent view's borrow and indexing domain.
pub struct StaticView<'parent, T, const N: usize, IndexSpace = Index1D> {
    ptr: *const T,
    contract: StaticViewContractV1,
    _borrow: PhantomData<&'parent [T]>,
    _index_space: PhantomData<fn() -> IndexSpace>,
}

impl<'parent, T, const N: usize, IndexSpace> StaticView<'parent, T, N, IndexSpace> {
    /// Derives a fixed-size shared view after validating the parent contract.
    ///
    /// The allocation base plus `parent_region.byte_offset()` must equal the
    /// parent slice's pointer address. The region must describe every element
    /// of the parent with the exact Rust size and alignment of `T`.
    pub fn from_disjoint_slice(
        parent: &'parent DisjointSlice<T, IndexSpace>,
        allocation: AllocationSpecV1,
        parent_region: ByteRegionV1,
        start_element: usize,
    ) -> Result<Self, StaticViewError> {
        let (ptr, contract) = checked_parts::<T, N>(
            parent.ptr.cast_const(),
            parent.len,
            allocation,
            parent_region,
            start_element,
            PermissionKindV1::SharedRead,
        )?;
        Ok(Self {
            ptr,
            contract,
            _borrow: PhantomData,
            _index_space: PhantomData,
        })
    }

    pub const fn len(&self) -> usize {
        N
    }

    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Returns the pure contract checked during construction.
    pub const fn contract(&self) -> StaticViewContractV1 {
        self.contract
    }

    /// Returns an element whose bound is established during monomorphization.
    #[inline(always)]
    pub fn at_const<const I: usize>(&self, _index: StaticIndex<N, I>) -> &T {
        // SAFETY: construction establishes `N` valid elements and `StaticIndex`
        // can be created by safe code only after proving `I < N`.
        unsafe { &*self.ptr.add(I) }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= N {
            return None;
        }
        // SAFETY: the branch establishes `index < N`.
        Some(unsafe { &*self.ptr.add(index) })
    }

    pub fn as_array(&self) -> &[T; N] {
        // SAFETY: construction establishes exactly `N` valid contiguous
        // elements for the parent borrow's lifetime.
        unsafe { &*self.ptr.cast::<[T; N]>() }
    }
}

impl<T, const N: usize, IndexSpace> fmt::Debug for StaticView<'_, T, N, IndexSpace> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticView")
            .field("len", &N)
            .field("contract", &self.contract)
            .finish_non_exhaustive()
    }
}

/// Exclusive fixed-size view derived from a mutably borrowed parent capability.
///
/// This type is intentionally neither `Clone` nor `Copy`. Its private fields
/// and mutable parent borrow prevent safe duplication or forgery of exclusive
/// access.
pub struct StaticViewMut<'parent, T, const N: usize, IndexSpace = Index1D> {
    ptr: *mut T,
    contract: StaticViewContractV1,
    _borrow: PhantomData<&'parent mut [T]>,
    _index_space: PhantomData<fn() -> IndexSpace>,
}

impl<'parent, T, const N: usize, IndexSpace> StaticViewMut<'parent, T, N, IndexSpace> {
    /// Derives a fixed-size exclusive view after validating the parent contract.
    pub fn from_disjoint_slice(
        parent: &'parent mut DisjointSlice<T, IndexSpace>,
        allocation: AllocationSpecV1,
        parent_region: ByteRegionV1,
        start_element: usize,
    ) -> Result<Self, StaticViewError> {
        let (ptr, contract) = checked_parts::<T, N>(
            parent.ptr.cast_const(),
            parent.len,
            allocation,
            parent_region,
            start_element,
            PermissionKindV1::ExclusiveWrite,
        )?;
        Ok(Self {
            ptr: ptr.cast_mut(),
            contract,
            _borrow: PhantomData,
            _index_space: PhantomData,
        })
    }

    pub const fn len(&self) -> usize {
        N
    }

    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Returns the pure contract checked during construction.
    pub const fn contract(&self) -> StaticViewContractV1 {
        self.contract
    }

    #[inline(always)]
    pub fn at_const<const I: usize>(&self, _index: StaticIndex<N, I>) -> &T {
        // SAFETY: construction and the checked index witness establish access.
        unsafe { &*self.ptr.add(I) }
    }

    #[inline(always)]
    pub fn at_const_mut<const I: usize>(&mut self, _index: StaticIndex<N, I>) -> &mut T {
        // SAFETY: construction and the checked index witness establish access;
        // the mutable borrow of this linear view establishes exclusivity.
        unsafe { &mut *self.ptr.add(I) }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= N {
            return None;
        }
        // SAFETY: the branch establishes `index < N`.
        Some(unsafe { &*self.ptr.add(index) })
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= N {
            return None;
        }
        // SAFETY: the branch and mutable view borrow establish access.
        Some(unsafe { &mut *self.ptr.add(index) })
    }

    pub fn as_array(&self) -> &[T; N] {
        // SAFETY: construction establishes `N` valid contiguous elements.
        unsafe { &*self.ptr.cast::<[T; N]>() }
    }

    pub fn as_mut_array(&mut self) -> &mut [T; N] {
        // SAFETY: construction and the mutable view borrow establish access.
        unsafe { &mut *self.ptr.cast::<[T; N]>() }
    }
}

impl<T, const N: usize, IndexSpace> fmt::Debug for StaticViewMut<'_, T, N, IndexSpace> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticViewMut")
            .field("len", &N)
            .field("contract", &self.contract)
            .finish_non_exhaustive()
    }
}

#[allow(clippy::too_many_arguments)]
fn checked_parts<T, const N: usize>(
    parent_ptr: *const T,
    parent_len: usize,
    allocation: AllocationSpecV1,
    parent_region: ByteRegionV1,
    start_element: usize,
    permission: PermissionKindV1,
) -> Result<(*const T, StaticViewContractV1), StaticViewError> {
    if parent_ptr.is_null() {
        return Err(StaticViewError::NullParent);
    }

    let parent_element_count =
        u64::try_from(parent_len).map_err(|_| StaticViewError::ParentLengthNotRepresentable)?;
    let start_element =
        u64::try_from(start_element).map_err(|_| StaticViewError::ParentLengthNotRepresentable)?;
    let element_count =
        u64::try_from(N).map_err(|_| StaticViewError::ParentLengthNotRepresentable)?;
    let element_size = u64::try_from(size_of::<T>())
        .map_err(|_| StaticViewError::ElementLayoutNotRepresentable)?;
    let element_alignment = u64::try_from(align_of::<T>())
        .map_err(|_| StaticViewError::ElementLayoutNotRepresentable)?;

    let parent_bytes = parent_element_count
        .checked_mul(element_size)
        .ok_or(StaticViewError::ParentExtentTooLarge { bytes: u64::MAX })?;
    if parent_bytes > isize::MAX as u64 {
        return Err(StaticViewError::ParentExtentTooLarge {
            bytes: parent_bytes,
        });
    }

    let expected_parent = allocation
        .base_address()
        .checked_add(parent_region.byte_offset())
        .ok_or(StaticViewError::ParentPointerAddressNotRepresentable)?;
    let actual_parent = u64::try_from(parent_ptr.addr())
        .map_err(|_| StaticViewError::ParentPointerAddressNotRepresentable)?;
    if actual_parent != expected_parent {
        return Err(StaticViewError::ParentPointerMismatch {
            expected: expected_parent,
            actual: actual_parent,
        });
    }

    let contract = StaticViewContractV1::new(
        allocation,
        parent_region,
        parent_element_count,
        start_element,
        element_count,
        element_size,
        element_alignment,
        permission,
    )?;
    let start = usize::try_from(contract.start_element())
        .map_err(|_| StaticViewError::ParentLengthNotRepresentable)?;
    // SAFETY: the contract establishes `start + N <= parent_len`; the parent
    // capability establishes pointer validity and provenance.
    Ok((unsafe { parent_ptr.add(start) }, contract))
}

#[cfg(test)]
mod tests {
    use super::{StaticIndex, StaticView, StaticViewError, StaticViewMut};
    use crate::{DisjointSlice, Index1D, Index2D};
    use core::mem::{align_of, size_of};
    use fe2o3_contracts::{
        AddressSpaceIdV1, AllocationProvenanceIdV1, AllocationSpecV1, ByteRegionV1,
        PermissionKindV1, StaticViewContractErrorV1, StaticViewContractV1,
    };

    fn allocation<T>(storage: &[T]) -> (AllocationSpecV1, ByteRegionV1) {
        let base = storage.as_ptr().addr() as u64;
        let bytes = size_of_val(storage) as u64;
        let allocation = AllocationSpecV1::new(
            AllocationProvenanceIdV1::new(7).unwrap(),
            AddressSpaceIdV1::new(1).unwrap(),
            base,
            bytes,
            base.checked_add(bytes).unwrap(),
        )
        .unwrap();
        let region = ByteRegionV1::for_allocation(allocation, 0, bytes).unwrap();
        (allocation, region)
    }

    #[test]
    fn shared_view_checks_once_and_preserves_the_parent_contract() {
        let mut storage = [10_u32, 20, 30, 40, 50, 60];
        let (allocation, parent_region) = allocation(&storage);
        // SAFETY: the array remains live and exclusively owned for the view.
        let parent = unsafe {
            DisjointSlice::<u32, Index1D>::from_raw_parts(storage.as_mut_ptr(), storage.len())
        };
        let view = StaticView::<u32, 3>::from_disjoint_slice(&parent, allocation, parent_region, 2)
            .unwrap();

        assert_eq!(view.len(), 3);
        assert!(!view.is_empty());
        assert_eq!(*view.at_const(StaticIndex::<3, 0>::CHECKED), 30);
        assert_eq!(*view.at_const(StaticIndex::<3, 2>::CHECKED), 50);
        assert_eq!(view.get(3), None);
        assert_eq!(view.as_array(), &[30, 40, 50]);
        assert_eq!(view.contract().permission(), PermissionKindV1::SharedRead);
        assert_eq!(
            view.contract().region().byte_offset(),
            2 * size_of::<u32>() as u64
        );
    }

    #[test]
    fn exclusive_view_is_linear_and_mutates_only_its_fixed_region() {
        let mut storage = [10_u32, 20, 30, 40, 50, 60];
        let (allocation, parent_region) = allocation(&storage);
        // SAFETY: the array remains live and exclusively owned for the view.
        let mut parent = unsafe {
            DisjointSlice::<u32, Index1D>::from_raw_parts(storage.as_mut_ptr(), storage.len())
        };
        {
            let mut view = StaticViewMut::<u32, 3>::from_disjoint_slice(
                &mut parent,
                allocation,
                parent_region,
                1,
            )
            .unwrap();

            *view.at_const_mut(StaticIndex::<3, 0>::CHECKED) = 21;
            *view.get_mut(2).unwrap() = 41;
            view.as_mut_array()[1] = 31;
            assert_eq!(view.at_const(StaticIndex::<3, 2>::CHECKED), &41);
            assert_eq!(view.get(3), None);
            assert_eq!(view.as_array(), &[21, 31, 41]);
            assert_eq!(
                view.contract().permission(),
                PermissionKindV1::ExclusiveWrite
            );
        }
        assert_eq!(storage, [10, 21, 31, 41, 50, 60]);
    }

    #[test]
    fn subregion_parent_retains_root_allocation_provenance() {
        let mut storage = [1_u16, 2, 3, 4, 5, 6];
        let (allocation, _) = allocation(&storage);
        let offset = 2 * size_of::<u16>() as u64;
        let bytes = 4 * size_of::<u16>() as u64;
        let parent_region = ByteRegionV1::for_allocation(allocation, offset, bytes).unwrap();
        // SAFETY: this parent covers the final four elements of `storage`.
        let parent = unsafe {
            DisjointSlice::<u16, Index2D<2>>::from_raw_parts(storage.as_mut_ptr().add(2), 4)
        };
        let view = StaticView::<u16, 2, Index2D<2>>::from_disjoint_slice(
            &parent,
            allocation,
            parent_region,
            1,
        )
        .unwrap();
        assert_eq!(view.as_array(), &[4, 5]);
        assert_eq!(view.contract().allocation().provenance().get(), 7);
        assert_eq!(view.contract().region().byte_offset(), offset + 2);
    }

    #[test]
    fn zero_out_of_range_and_mutated_parent_contracts_fail_closed() {
        let mut storage = [1_u32, 2, 3, 4];
        let (allocation, parent_region) = allocation(&storage);
        // SAFETY: the parent covers `storage` for this test.
        let parent = unsafe {
            DisjointSlice::<u32, Index1D>::from_raw_parts(storage.as_mut_ptr(), storage.len())
        };
        assert_eq!(
            StaticView::<u32, 0>::from_disjoint_slice(&parent, allocation, parent_region, 0,)
                .unwrap_err(),
            StaticViewError::Contract(StaticViewContractErrorV1::EmptyView)
        );
        assert_eq!(
            StaticView::<u32, 3>::from_disjoint_slice(&parent, allocation, parent_region, 2,)
                .unwrap_err(),
            StaticViewError::Contract(StaticViewContractErrorV1::ElementRangeOutsideParent {
                start: 2,
                count: 3,
                parent_count: 4,
            })
        );

        let short_region = ByteRegionV1::for_allocation(allocation, 0, 12).unwrap();
        assert_eq!(
            StaticView::<u32, 2>::from_disjoint_slice(&parent, allocation, short_region, 0)
                .unwrap_err(),
            StaticViewError::Contract(StaticViewContractErrorV1::ParentExtentMismatch {
                expected: 16,
                actual: 12,
            })
        );
    }

    #[test]
    fn pointer_substitution_is_rejected_before_access() {
        let mut storage = [1_u32, 2, 3, 4];
        let (allocation, parent_region) = allocation(&storage);
        let shifted = AllocationSpecV1::new(
            AllocationProvenanceIdV1::new(7).unwrap(),
            AddressSpaceIdV1::new(1).unwrap(),
            allocation.base_address() + 4,
            allocation.byte_length(),
            allocation.address_space_size() + 4,
        )
        .unwrap();
        let shifted_region =
            ByteRegionV1::for_allocation(shifted, 0, shifted.byte_length()).unwrap();
        // SAFETY: the parent itself is valid; only the specification pointer is
        // deliberately substituted.
        let parent = unsafe {
            DisjointSlice::<u32, Index1D>::from_raw_parts(storage.as_mut_ptr(), storage.len())
        };
        assert_eq!(
            StaticView::<u32, 2>::from_disjoint_slice(&parent, shifted, shifted_region, 0)
                .unwrap_err(),
            StaticViewError::ParentPointerMismatch {
                expected: allocation.base_address() + 4,
                actual: allocation.base_address(),
            }
        );
        assert_eq!(parent_region.byte_length(), 16);
    }

    #[test]
    fn type_level_extent_and_index_space_do_not_change_runtime_layout() {
        assert_eq!(
            size_of::<StaticView<'_, u32, 1, Index1D>>(),
            size_of::<StaticView<'_, u32, 64, Index2D<8>>>()
        );
        assert_eq!(
            align_of::<StaticView<'_, u32, 1, Index1D>>(),
            align_of::<StaticView<'_, u32, 64, Index2D<8>>>()
        );
        assert_eq!(
            size_of::<StaticViewMut<'_, u32, 1, Index1D>>(),
            size_of::<StaticViewMut<'_, u32, 64, Index2D<8>>>()
        );
        assert_eq!(
            size_of::<StaticView<'_, u32, 4>>(),
            size_of::<*const u32>() + size_of::<StaticViewContractV1>()
        );
    }
}
