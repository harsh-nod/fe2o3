//! Fixed-size views with explicit runtime authority boundaries.
//!
//! [`StaticView`] can be derived safely only from a valid shared Rust slice and
//! never provides mutation. [`StaticViewMut`] requires an unsafe constructor
//! because a per-invocation Rust borrow does not establish GPU-wide partition
//! authority. [`DisjointStaticTileMut`] instead borrows an existing
//! [`crate::DisjointSlice`] authority and an exclusive [`crate::GridLeader`]
//! capability, then checks one parent-region-relative fixed extent before
//! granting unchecked constant-index accesses.
//!
//! None of these types represents artifact, launch, or compiler-refinement
//! authority. In particular, the static tile preserves the parent
//! `DisjointSlice` contract; it does not authenticate how that parent was
//! constructed.

use core::fmt;
use core::marker::PhantomData;
use core::mem::size_of;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CheckedStridedExtentError {
    InvalidStride,
    ExtentOverflow,
    OutOfBounds { required: usize, actual: usize },
}

pub(crate) fn check_strided_2d_extent(
    offset: usize,
    rows: usize,
    columns: usize,
    row_stride: usize,
    actual: usize,
) -> Result<(), CheckedStridedExtentError> {
    if rows != 0 && columns != 0 && row_stride < columns {
        return Err(CheckedStridedExtentError::InvalidStride);
    }
    let required = if rows == 0 || columns == 0 {
        offset
    } else {
        offset
            .checked_add(
                (rows - 1)
                    .checked_mul(row_stride)
                    .and_then(|value| value.checked_add(columns))
                    .ok_or(CheckedStridedExtentError::ExtentOverflow)?,
            )
            .ok_or(CheckedStridedExtentError::ExtentOverflow)?
    };
    if required > actual {
        return Err(CheckedStridedExtentError::OutOfBounds { required, actual });
    }
    Ok(())
}

/// Zero-sized witness that constant `I` lies within fixed extent `N`.
///
/// Safe code can construct this witness only through [`Self::CHECKED`], whose
/// assertion is evaluated by rustc at the use site. Passing the witness adds no
/// runtime data or bounds branch.
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

/// Failure to derive a fixed-size view from a Rust slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum StaticViewError {
    EmptyView,
    ZeroSizedElement,
    ElementRangeOverflow,
    ElementRangeOutsideParent {
        start: usize,
        count: usize,
        parent_count: usize,
    },
}

/// Failure to derive a checked row-major strided read view from a shared slice.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
#[rustc_diagnostic_item = "fe2o3_device_strided_read_view_2d_error_v1"]
pub enum StridedReadView2DError {
    /// The element type has no addressable storage.
    ZeroSizedElement,
    /// A nonempty view has a row stride smaller than its logical column count.
    InvalidStride,
    /// Offset or extent arithmetic overflowed `usize`.
    ExtentOverflow,
    /// The complete logical view is not contained in the supplied slice.
    OutOfBounds { required: usize, actual: usize },
}

impl fmt::Display for StridedReadView2DError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid strided 2-D read view: {self:?}")
    }
}

type InvariantSharedBorrow<'parent, T> = fn(&'parent [T]) -> &'parent [T];

/// Checked read-only row-major 2-D view with a runtime row stride.
///
/// Safe construction is available only from an ordinary shared Rust slice.
/// The private slice and invariant lifetime brand keep the resulting view tied
/// to that exact borrow; callers cannot forge a view from a raw pointer or
/// extend its lifetime. Logical out-of-bounds reads are explicit and total:
/// [`Self::load_or`] returns the supplied fallback without accessing storage.
#[rustc_diagnostic_item = "fe2o3_device_strided_read_view_2d_v1"]
pub struct StridedReadView2D<'parent, T> {
    data: &'parent [T],
    offset: usize,
    rows: usize,
    columns: usize,
    row_stride: usize,
    _borrow: PhantomData<InvariantSharedBorrow<'parent, T>>,
}

impl<T> Copy for StridedReadView2D<'_, T> {}

impl<T> Clone for StridedReadView2D<'_, T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<'parent, T> StridedReadView2D<'parent, T> {
    /// Checks one row-major strided view rooted at `offset` in `data`.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_strided_read_view_2d_from_shared_slice_v1"]
    pub fn from_shared_slice(
        data: &'parent [T],
        offset: usize,
        rows: usize,
        columns: usize,
        row_stride: usize,
    ) -> Result<Self, StridedReadView2DError> {
        if size_of::<T>() == 0 {
            return Err(StridedReadView2DError::ZeroSizedElement);
        }
        check_strided_2d_extent(offset, rows, columns, row_stride, data.len()).map_err(
            |error| match error {
                CheckedStridedExtentError::InvalidStride => StridedReadView2DError::InvalidStride,
                CheckedStridedExtentError::ExtentOverflow => StridedReadView2DError::ExtentOverflow,
                CheckedStridedExtentError::OutOfBounds { required, actual } => {
                    StridedReadView2DError::OutOfBounds { required, actual }
                }
            },
        )?;
        Ok(Self {
            data,
            offset,
            rows,
            columns,
            row_stride,
            _borrow: PhantomData,
        })
    }

    pub const fn rows(&self) -> usize {
        self.rows
    }

    pub const fn columns(&self) -> usize {
        self.columns
    }

    pub const fn row_stride(&self) -> usize {
        self.row_stride
    }

    /// Reads one logical element or returns `fallback` without accessing
    /// storage when either coordinate lies outside the logical view.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_strided_read_view_2d_load_or_v1"]
    pub fn load_or(&self, row: usize, column: usize, fallback: T) -> T
    where
        T: Copy,
    {
        if row >= self.rows || column >= self.columns {
            return fallback;
        }
        let Some(index) = row
            .checked_mul(self.row_stride)
            .and_then(|value| self.offset.checked_add(value))
            .and_then(|value| value.checked_add(column))
        else {
            return fallback;
        };
        self.data.get(index).copied().unwrap_or(fallback)
    }
}

impl<T> fmt::Debug for StridedReadView2D<'_, T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StridedReadView2D")
            .field("rows", &self.rows)
            .field("columns", &self.columns)
            .field("row_stride", &self.row_stride)
            .finish_non_exhaustive()
    }
}

impl fmt::Display for StaticViewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid static view: {self:?}")
    }
}

/// Non-forgeable identity and extent witness embedded in one checked tile.
///
/// The private fields retain the exact pointer and length representation of the
/// parent [`crate::DisjointSlice`], plus the checked element offset. The
/// invariant lifetime brand ties the witness to that exact mutable parent
/// borrow. Safe code can inspect but cannot construct, clone, extract, replace,
/// or substitute this witness into another tile.
type InvariantDisjointBorrow<'parent, T, IndexSpace> =
    fn(
        &'parent mut crate::DisjointSlice<T, IndexSpace>,
    ) -> &'parent mut crate::DisjointSlice<T, IndexSpace>;

pub struct StaticTileRegionWitness<'parent, T, IndexSpace, const N: usize> {
    parent_ptr: *mut T,
    parent_len: usize,
    start_element: usize,
    _parent: PhantomData<InvariantDisjointBorrow<'parent, T, IndexSpace>>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<T, IndexSpace, const N: usize> StaticTileRegionWitness<'_, T, IndexSpace, N> {
    /// Element offset relative to the checked parent `DisjointSlice` region.
    pub const fn start_element(&self) -> usize {
        self.start_element
    }

    /// Element extent of the exact parent `DisjointSlice` region.
    pub const fn parent_region_len(&self) -> usize {
        self.parent_len
    }

    pub const fn tile_len(&self) -> usize {
        N
    }
}

impl<T, IndexSpace, const N: usize> fmt::Debug for StaticTileRegionWitness<'_, T, IndexSpace, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticTileRegionWitness")
            .field("parent_region_len", &self.parent_len)
            .field("start_element", &self.start_element)
            .field("tile_len", &N)
            .finish_non_exhaustive()
    }
}

/// Mutable fixed-size tile checked once against one `DisjointSlice` region.
///
/// The embedded [`StaticTileRegionWitness`] is the proof-carrying portion of
/// the view. Constant-index accessors use its already-checked extent and emit
/// no dynamic bounds branch. This type is intentionally neither `Clone`,
/// `Copy`, `Send`, nor `Sync`.
pub struct DisjointStaticTileMut<'parent, T, IndexSpace, const N: usize> {
    region: StaticTileRegionWitness<'parent, T, IndexSpace, N>,
}

impl<'parent, T, IndexSpace, const N: usize> DisjointStaticTileMut<'parent, T, IndexSpace, N> {
    pub(crate) fn from_disjoint_region(
        _parent: &'parent mut crate::DisjointSlice<T, IndexSpace>,
        parent_ptr: *mut T,
        parent_len: usize,
        start_element: usize,
    ) -> Result<Self, StaticViewError> {
        let start_element = checked_start::<T, N>(parent_len, start_element)?;
        Ok(Self {
            region: StaticTileRegionWitness {
                parent_ptr,
                parent_len,
                start_element,
                _parent: PhantomData,
                _not_send_sync: PhantomData,
            },
        })
    }

    pub const fn len(&self) -> usize {
        N
    }

    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Borrows the inseparable identity and extent witness for this tile.
    pub const fn region_witness(&self) -> &StaticTileRegionWitness<'parent, T, IndexSpace, N> {
        &self.region
    }

    /// Reads an element selected by a compile-time checked tile index.
    #[inline(always)]
    pub fn at_const<const I: usize>(&self, _index: StaticIndex<N, I>) -> &T {
        // SAFETY: tile construction checked `start + N <= parent_len`; the
        // static index establishes `I < N`, and the embedded witness retains
        // the exact parent pointer, extent, and borrow.
        unsafe { &*self.region.parent_ptr.add(self.region.start_element + I) }
    }

    /// Mutates an element selected by a compile-time checked tile index.
    #[inline(always)]
    pub fn at_const_mut<const I: usize>(&mut self, _index: StaticIndex<N, I>) -> &mut T {
        // SAFETY: the checked embedded witness establishes the element extent;
        // this mutable tile borrow preserves the parent `DisjointSlice`'s
        // exclusive partition.
        unsafe { &mut *self.region.parent_ptr.add(self.region.start_element + I) }
    }

    pub fn as_array(&self) -> &[T; N] {
        // SAFETY: construction checked a contiguous `N`-element region and the
        // parent `DisjointSlice` contract establishes validity.
        unsafe {
            &*self
                .region
                .parent_ptr
                .add(self.region.start_element)
                .cast::<[T; N]>()
        }
    }

    pub fn as_mut_array(&mut self) -> &mut [T; N] {
        // SAFETY: the mutable tile borrow additionally establishes exclusive
        // access within the parent partition.
        unsafe {
            &mut *self
                .region
                .parent_ptr
                .add(self.region.start_element)
                .cast::<[T; N]>()
        }
    }
}

impl<T, IndexSpace, const N: usize> fmt::Debug for DisjointStaticTileMut<'_, T, IndexSpace, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DisjointStaticTileMut")
            .field("region", &self.region)
            .finish_non_exhaustive()
    }
}

/// Shared fixed-size view derived from an ordinary shared Rust slice.
///
/// The source slice supplies real Rust lifetime and shared-access authority.
/// Safe construction does not accept [`crate::DisjointSlice`], raw pointers,
/// symbolic provenance, or a caller-declared permission. The type is pinned to
/// the current invocation and is intentionally neither `Send` nor `Sync`.
pub struct StaticView<'parent, T, const N: usize> {
    ptr: *const T,
    _borrow: PhantomData<&'parent [T]>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'parent, T, const N: usize> StaticView<'parent, T, N> {
    /// Derives a nonempty fixed-size view from a valid shared slice.
    ///
    /// Rust's shared-reference rules, including the absence of mutation for
    /// the borrow's lifetime, remain the source of authority. On a GPU, the
    /// code that originally creates `parent` must uphold those rules across
    /// every invocation and host/device alias; this safe function does not
    /// manufacture shared authority from a pointer or invocation-local token.
    pub fn from_shared_slice(
        parent: &'parent [T],
        start_element: usize,
    ) -> Result<Self, StaticViewError> {
        let start = checked_start::<T, N>(parent.len(), start_element)?;
        // SAFETY: `checked_start` establishes `start + N <= parent.len()` and
        // the shared slice carries pointer validity and lifetime.
        let ptr = unsafe { parent.as_ptr().add(start) };
        Ok(Self {
            ptr,
            _borrow: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    pub const fn len(&self) -> usize {
        N
    }

    pub const fn is_empty(&self) -> bool {
        false
    }

    /// Returns an element selected by a compile-time checked index.
    #[inline(always)]
    pub fn at_const<const I: usize>(&self, _index: StaticIndex<N, I>) -> &T {
        // SAFETY: construction establishes `N` valid elements and `StaticIndex`
        // can be created safely only after proving `I < N`.
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
        // SAFETY: construction establishes `N` valid contiguous elements for
        // the source slice's shared lifetime.
        unsafe { &*self.ptr.cast::<[T; N]>() }
    }
}

impl<T, const N: usize> fmt::Debug for StaticView<'_, T, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticView")
            .field("len", &N)
            .finish_non_exhaustive()
    }
}

/// Exclusive fixed-size view whose constructor requires device-wide authority.
///
/// The type is intentionally neither `Clone`, `Copy`, `Send`, nor `Sync`.
/// Private fields prevent safe forgery. Its safe accessors rely on the complete
/// unsafe construction contract, not on symbolic metadata or an ordinary
/// per-invocation borrow being treated as a global partition proof.
pub struct StaticViewMut<'parent, T, const N: usize> {
    ptr: *mut T,
    _borrow: PhantomData<&'parent mut [T]>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'parent, T, const N: usize> StaticViewMut<'parent, T, N> {
    /// Derives a fixed-size mutable view from a globally exclusive region.
    ///
    /// This constructor performs only extent checks. It does not inspect other
    /// GPU invocations, aliases, queues, or synchronization epochs.
    ///
    /// # Safety
    ///
    /// In addition to `parent` being a valid initialized Rust slice, the bytes
    /// selected by `start_element..start_element + N` must be globally
    /// exclusive for the complete lifetime of the returned view:
    ///
    /// - no other GPU invocation may read or write them through any pointer,
    ///   slice, view, allocation handle, or FFI alias;
    /// - no host thread, queue, device, or concurrent dispatch may access them;
    /// - the exclusivity must hold across every synchronization epoch in which
    ///   the view or references derived from it remain live; and
    /// - any transition that transfers ownership must happen before this
    ///   constructor and must not make another alias live until the view ends.
    ///
    /// Creating a separate `&mut [T]` in each invocation over overlapping
    /// global bytes does not satisfy these requirements. A future safe
    /// constructor must consume a branded proven-region token that establishes
    /// these global facts.
    pub unsafe fn from_globally_exclusive_slice(
        parent: &'parent mut [T],
        start_element: usize,
    ) -> Result<Self, StaticViewError> {
        let start = checked_start::<T, N>(parent.len(), start_element)?;
        // SAFETY: the caller establishes global exclusivity and the checked
        // range is contained in the valid mutable slice.
        let ptr = unsafe { parent.as_mut_ptr().add(start) };
        Ok(Self {
            ptr,
            _borrow: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    pub const fn len(&self) -> usize {
        N
    }

    pub const fn is_empty(&self) -> bool {
        false
    }

    #[inline(always)]
    pub fn at_const<const I: usize>(&self, _index: StaticIndex<N, I>) -> &T {
        // SAFETY: the constructor contract and checked index establish access.
        unsafe { &*self.ptr.add(I) }
    }

    #[inline(always)]
    pub fn at_const_mut<const I: usize>(&mut self, _index: StaticIndex<N, I>) -> &mut T {
        // SAFETY: the constructor contract, checked index, and mutable view
        // borrow establish access and exclusivity.
        unsafe { &mut *self.ptr.add(I) }
    }

    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= N {
            return None;
        }
        // SAFETY: the constructor contract and branch establish access.
        Some(unsafe { &*self.ptr.add(index) })
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= N {
            return None;
        }
        // SAFETY: the constructor contract, branch, and mutable borrow establish access.
        Some(unsafe { &mut *self.ptr.add(index) })
    }

    pub fn as_array(&self) -> &[T; N] {
        // SAFETY: the constructor contract establishes `N` initialized values.
        unsafe { &*self.ptr.cast::<[T; N]>() }
    }

    pub fn as_mut_array(&mut self) -> &mut [T; N] {
        // SAFETY: the constructor contract and mutable borrow establish access.
        unsafe { &mut *self.ptr.cast::<[T; N]>() }
    }
}

impl<T, const N: usize> fmt::Debug for StaticViewMut<'_, T, N> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("StaticViewMut")
            .field("len", &N)
            .finish_non_exhaustive()
    }
}

fn checked_start<T, const N: usize>(
    parent_len: usize,
    start_element: usize,
) -> Result<usize, StaticViewError> {
    if N == 0 {
        return Err(StaticViewError::EmptyView);
    }
    if size_of::<T>() == 0 {
        return Err(StaticViewError::ZeroSizedElement);
    }
    let end = start_element
        .checked_add(N)
        .ok_or(StaticViewError::ElementRangeOverflow)?;
    if end > parent_len {
        return Err(StaticViewError::ElementRangeOutsideParent {
            start: start_element,
            count: N,
            parent_count: parent_len,
        });
    }
    Ok(start_element)
}

impl From<StridedReadView2DError> for crate::KernelError {
    fn from(_error: StridedReadView2DError) -> Self {
        Self::InvalidArgument
    }
}

#[cfg(test)]
mod tests {
    use super::{
        StaticIndex, StaticView, StaticViewError, StaticViewMut, StridedReadView2D,
        StridedReadView2DError,
    };
    use core::mem::{align_of, size_of};

    #[test]
    fn shared_view_comes_only_from_shared_slice_authority() {
        let storage = [10_u32, 20, 30, 40, 50, 60];
        let view = StaticView::<u32, 3>::from_shared_slice(&storage, 2).unwrap();
        assert_eq!(view.len(), 3);
        assert!(!view.is_empty());
        assert_eq!(*view.at_const(StaticIndex::<3, 0>::CHECKED), 30);
        assert_eq!(*view.at_const(StaticIndex::<3, 2>::CHECKED), 50);
        assert_eq!(view.get(3), None);
        assert_eq!(view.as_array(), &[30, 40, 50]);
    }

    #[test]
    fn unsafe_global_exclusivity_boundary_enables_safe_mutable_accessors() {
        let mut storage = [10_u32, 20, 30, 40, 50, 60];
        {
            // SAFETY: this single-threaded host test owns the complete array;
            // no other invocation, thread, queue, or epoch can access it.
            let mut view = unsafe {
                StaticViewMut::<u32, 3>::from_globally_exclusive_slice(&mut storage, 1).unwrap()
            };
            *view.at_const_mut(StaticIndex::<3, 0>::CHECKED) = 21;
            *view.get_mut(2).unwrap() = 41;
            view.as_mut_array()[1] = 31;
            assert_eq!(view.at_const(StaticIndex::<3, 2>::CHECKED), &41);
            assert_eq!(view.get(3), None);
            assert_eq!(view.as_array(), &[21, 31, 41]);
        }
        assert_eq!(storage, [10, 21, 31, 41, 50, 60]);
    }

    #[test]
    fn zero_overflow_and_out_of_range_extents_fail_closed() {
        let storage = [1_u32, 2, 3, 4];
        assert_eq!(
            StaticView::<u32, 0>::from_shared_slice(&storage, 0).unwrap_err(),
            StaticViewError::EmptyView
        );
        assert_eq!(
            StaticView::<u32, 3>::from_shared_slice(&storage, 2).unwrap_err(),
            StaticViewError::ElementRangeOutsideParent {
                start: 2,
                count: 3,
                parent_count: 4,
            }
        );
        assert_eq!(
            StaticView::<u32, 2>::from_shared_slice(&storage, usize::MAX).unwrap_err(),
            StaticViewError::ElementRangeOverflow
        );
        assert_eq!(
            StaticView::<(), 1>::from_shared_slice(&[()], 0).unwrap_err(),
            StaticViewError::ZeroSizedElement
        );
    }

    #[test]
    fn explicit_auto_trait_markers_do_not_change_runtime_layout() {
        assert_eq!(
            size_of::<StaticView<'_, u32, 1>>(),
            size_of::<StaticView<'_, u32, 64>>()
        );
        assert_eq!(
            align_of::<StaticView<'_, u32, 1>>(),
            align_of::<StaticView<'_, u32, 64>>()
        );
        assert_eq!(size_of::<StaticView<'_, u32, 4>>(), size_of::<*const u32>());
        assert_eq!(
            size_of::<StaticViewMut<'_, u32, 4>>(),
            size_of::<*mut u32>()
        );
        assert_eq!(size_of::<StaticIndex<4, 2>>(), 0);
    }

    #[test]
    fn strided_read_view_uses_explicit_non_speculative_fallbacks() {
        let storage = [10_u32, 11, 99, 20, 21, 98, 30, 31];
        let view = StridedReadView2D::from_shared_slice(&storage, 0, 3, 2, 3).unwrap();
        assert_eq!(view.rows(), 3);
        assert_eq!(view.columns(), 2);
        assert_eq!(view.row_stride(), 3);
        assert_eq!(view.load_or(0, 1, 777), 11);
        assert_eq!(view.load_or(2, 1, 777), 31);
        assert_eq!(view.load_or(3, 0, 777), 777);
        assert_eq!(view.load_or(1, 2, 888), 888);
    }

    #[test]
    fn strided_read_view_checks_layout_extent_and_overflow() {
        let storage = [0_u32; 8];
        assert_eq!(
            StridedReadView2D::from_shared_slice(&storage, 0, 2, 3, 2).unwrap_err(),
            StridedReadView2DError::InvalidStride
        );
        assert_eq!(
            StridedReadView2D::from_shared_slice(&storage, 1, 3, 2, 3).unwrap_err(),
            StridedReadView2DError::OutOfBounds {
                required: 9,
                actual: 8,
            }
        );
        assert_eq!(
            StridedReadView2D::from_shared_slice(&storage, usize::MAX, 0, 0, 0).unwrap_err(),
            StridedReadView2DError::OutOfBounds {
                required: usize::MAX,
                actual: 8,
            }
        );
        assert_eq!(
            StridedReadView2D::from_shared_slice(&storage, 0, usize::MAX, 2, usize::MAX)
                .unwrap_err(),
            StridedReadView2DError::ExtentOverflow
        );
        assert_eq!(
            StridedReadView2D::from_shared_slice(&[()], 0, 1, 1, 1).unwrap_err(),
            StridedReadView2DError::ZeroSizedElement
        );
    }

    #[test]
    fn empty_strided_read_view_is_valid_and_never_reads() {
        let storage = [5_u32, 6];
        let view = StridedReadView2D::from_shared_slice(&storage, 2, 0, 7, 0).unwrap();
        assert_eq!(view.load_or(0, 0, 42), 42);
    }

    #[test]
    fn strided_read_view_preserves_shared_reference_auto_traits() {
        fn assert_send_sync<T: Send + Sync>() {}
        fn assert_copy_clone<T: Copy + Clone>() {}
        assert_send_sync::<StridedReadView2D<'static, u32>>();
        assert_copy_clone::<StridedReadView2D<'static, u32>>();
    }
}
