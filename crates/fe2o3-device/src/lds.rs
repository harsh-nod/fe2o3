//! Typed access to launch-sized workgroup memory.
//!
//! This module describes the device-side ownership and initialization
//! contract. The backend still has to lower the constructors to the launch's
//! dynamic LDS base and byte extent; these APIs do not allocate memory or
//! authorize a launch.

use core::fmt;
use core::marker::PhantomData;
use core::mem::{MaybeUninit, align_of, size_of};
use core::ptr::NonNull;
use core::slice;

/// Largest element alignment admitted by the dynamic LDS contract.
///
/// The generated backend must provide a base with at least this alignment when
/// it instantiates a view whose element requires it.
pub const MAX_DYNAMIC_LDS_ALIGNMENT: usize = 16;

mod sealed {
    pub trait Sealed {}
}

/// A plain scalar or scalar array that can reside in dynamic LDS.
///
/// This trait is sealed because accepting user-defined types would require the
/// compiler to prove that they contain no references, invalid padding state,
/// destructor, or unsupported target layout. Dynamic LDS starts uninitialized,
/// so implementing this trait alone never permits reading arbitrary bytes as a
/// value.
///
/// # Safety
///
/// Implementations must have a stable device layout, contain no references or
/// destructor, and be valid to move by copying their initialized bytes. The
/// implementation must also remain valid in the AMDGPU workgroup address
/// space. Only this crate can implement the trait.
pub unsafe trait LdsElement: sealed::Sealed {}

macro_rules! lds_scalars {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl sealed::Sealed for $ty {}

            // SAFETY: these scalar types have no references or destructor and
            // use target-defined scalar layouts supported by the backend.
            unsafe impl LdsElement for $ty {}
        )+
    };
}

lds_scalars!(u8, i8, u16, i16, u32, i32, u64, i64, usize, isize, f32, f64);

lds_scalars!(crate::F16, crate::Bf16, crate::Bf16x2);

impl<T: LdsElement, const N: usize> sealed::Sealed for [T; N] {}

// SAFETY: arrays preserve the layout and byte-movability requirements of their
// element. Zero-length arrays are rejected when a view is constructed.
unsafe impl<T: LdsElement, const N: usize> LdsElement for [T; N] {}

/// Error returned when a raw launch-sized LDS region cannot represent `T`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum DynamicLdsError {
    /// The dynamic LDS base was null, including for an empty view.
    NullBase,
    /// The element type occupies no bytes.
    ZeroSizedElement,
    /// The element's alignment exceeds the bounded device contract.
    UnsupportedElementAlignment { required: usize, maximum: usize },
    /// The provided base does not meet the element's alignment.
    MisalignedBase { required: usize },
    /// The byte extent does not contain a whole number of elements.
    PartialElement { bytes: usize, element_size: usize },
    /// Rust slices cannot represent an allocation larger than `isize::MAX`.
    ExtentTooLarge { bytes: usize },
    /// Adding the byte extent to the base address overflowed `usize`.
    AddressOverflow,
}

/// Compiler-created, non-duplicable identity for one workgroup's LDS lifetime.
///
/// The type is neither `Send` nor `Sync`. Its private fields and unsafe
/// constructor prevent safe code from inventing workgroup identity. Borrowing
/// it for [`DynamicLds::from_raw_parts`] ties the view to the scope and permits
/// only one root view; disjoint views must be derived with
/// [`DynamicLds::split_at`].
#[rustc_diagnostic_item = "fe2o3_device_workgroup_lds_scope"]
pub struct WorkgroupLdsScope<'workgroup> {
    _brand: PhantomData<&'workgroup mut &'workgroup ()>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'workgroup> WorkgroupLdsScope<'workgroup> {
    /// Creates the scope identity consumed by dynamic LDS lowering.
    ///
    /// # Safety
    ///
    /// This function may be called only by compiler-generated device code,
    /// once for the active workgroup execution. The chosen lifetime must not
    /// outlive that workgroup, and the value must not cross an invocation or a
    /// host thread. The current backend does not lower this constructor yet.
    #[doc(hidden)]
    #[rustc_diagnostic_item = "fe2o3_device_workgroup_lds_scope_from_compiler"]
    pub unsafe fn from_compiler() -> Self {
        Self::new_identity()
    }

    fn new_identity() -> Self {
        Self {
            _brand: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    #[cfg(test)]
    fn for_host_test() -> Self {
        Self::new_identity()
    }
}

impl fmt::Debug for WorkgroupLdsScope<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("WorkgroupLdsScope")
    }
}

/// Typestate for a dynamic LDS region whose elements may be uninitialized.
#[derive(Debug)]
pub enum LdsUninitialized {}

/// Typestate for a dynamic LDS region whose elements are all initialized.
#[derive(Debug)]
pub enum LdsInitialized {}

/// An exclusive typed view over launch-sized workgroup memory.
///
/// `State` tracks whether every element is initialized. New views start in
/// [`LdsUninitialized`], where safe access returns [`MaybeUninit<T>`]. A caller
/// that establishes initialization for the complete region can make the
/// unsafe transition to [`LdsInitialized`] and then use ordinary safe slices.
///
/// A view is intentionally neither `Clone`, `Copy`, `Send`, nor `Sync`. It has
/// no allocation or launch authority. Compiler lowering remains responsible
/// for supplying the AMDGPU workgroup-address-space base and exact launch byte
/// extent.
#[rustc_diagnostic_item = "fe2o3_device_dynamic_lds"]
pub struct DynamicLds<'workgroup, T: LdsElement, State = LdsUninitialized> {
    ptr: NonNull<MaybeUninit<T>>,
    len: usize,
    byte_len: usize,
    _borrow: PhantomData<&'workgroup mut [MaybeUninit<T>]>,
    _state: PhantomData<fn() -> State>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'workgroup, T: LdsElement> DynamicLds<'workgroup, T, LdsUninitialized> {
    /// Constructs a typed view from the launch's dynamic LDS allocation.
    ///
    /// Validation rejects null or misaligned bases, zero-sized or over-aligned
    /// elements, partial elements, slice-incompatible extents, and address
    /// arithmetic overflow. A zero-byte region is allowed but still requires a
    /// non-null, aligned base as required by Rust slices.
    ///
    /// # Safety
    ///
    /// On success, `base` must identify `allocated_bytes` live bytes in the
    /// current workgroup's AMDGPU workgroup address space for the entire
    /// `'workgroup` lifetime. That byte region must be exclusively owned by
    /// this capability and may not overlap any other live Rust reference or LDS
    /// capability. `scope` must be the compiler-created identity for the same
    /// workgroup. The bytes may be uninitialized.
    ///
    /// Invalid null, alignment, and integer-extent inputs may be passed for
    /// validation and return `Err`; the provenance, address-space, lifetime,
    /// and exclusivity requirements apply whenever this function returns `Ok`.
    /// The current backend does not lower this constructor yet.
    #[doc(hidden)]
    #[rustc_diagnostic_item = "fe2o3_device_dynamic_lds_from_raw_parts"]
    pub unsafe fn from_raw_parts(
        _scope: &'workgroup mut WorkgroupLdsScope<'workgroup>,
        base: *mut u8,
        allocated_bytes: usize,
    ) -> Result<Self, DynamicLdsError> {
        // SAFETY: the caller establishes device provenance, lifetime, and
        // exclusivity; the helper enforces the checkable representation facts.
        unsafe { Self::from_checked_raw_parts(base, allocated_bytes) }
    }

    unsafe fn from_checked_raw_parts(
        base: *mut u8,
        allocated_bytes: usize,
    ) -> Result<Self, DynamicLdsError> {
        let len = validate_layout(
            base as usize,
            allocated_bytes,
            size_of::<T>(),
            align_of::<T>(),
        )?;
        let ptr = NonNull::new(base.cast::<MaybeUninit<T>>()).ok_or(DynamicLdsError::NullBase)?;

        Ok(Self {
            ptr,
            len,
            byte_len: allocated_bytes,
            _borrow: PhantomData,
            _state: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    #[cfg(test)]
    unsafe fn from_host_parts_for_test(
        _scope: &'workgroup mut WorkgroupLdsScope<'workgroup>,
        base: *mut u8,
        allocated_bytes: usize,
    ) -> Result<Self, DynamicLdsError> {
        // SAFETY: test callers establish host allocation validity, lifetime,
        // and exclusivity; no AMDGPU address-space claim is made.
        unsafe { Self::from_checked_raw_parts(base, allocated_bytes) }
    }

    /// Returns an element without asserting that it has been initialized.
    pub fn get_uninit(&self, index: usize) -> Option<&MaybeUninit<T>> {
        if index >= self.len {
            return None;
        }
        // SAFETY: construction validates the extent and provenance. Shared
        // access is derived from the exclusive capability.
        Some(unsafe { &*self.ptr.as_ptr().add(index) })
    }

    /// Returns exclusive access to an element without asserting initialization.
    pub fn get_uninit_mut(&mut self, index: usize) -> Option<&mut MaybeUninit<T>> {
        if index >= self.len {
            return None;
        }
        // SAFETY: construction validates the extent and provenance, while the
        // mutable borrow of the linear capability establishes exclusivity.
        Some(unsafe { &mut *self.ptr.as_ptr().add(index) })
    }

    /// Returns the complete region without asserting initialization.
    pub fn as_uninit_slice(&self) -> &[MaybeUninit<T>] {
        // SAFETY: the constructor validates all slice requirements.
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    /// Returns the complete region exclusively without asserting initialization.
    pub fn as_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<T>] {
        // SAFETY: the constructor validates all slice requirements and the
        // mutable borrow of the linear capability establishes exclusivity.
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    /// Initializes one element and returns exclusive access to the new value.
    pub fn write(&mut self, index: usize, value: T) -> Option<&mut T> {
        self.get_uninit_mut(index).map(|slot| slot.write(value))
    }

    /// Marks every element in this region initialized.
    ///
    /// # Safety
    ///
    /// Every element must hold a valid initialized `T`, and all writes that
    /// established initialization must happen before subsequent reads under
    /// the kernel's synchronization model.
    pub unsafe fn assume_init(self) -> DynamicLds<'workgroup, T, LdsInitialized> {
        DynamicLds {
            ptr: self.ptr,
            len: self.len,
            byte_len: self.byte_len,
            _borrow: PhantomData,
            _state: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

impl<T: LdsElement> DynamicLds<'_, T, LdsInitialized> {
    pub fn get(&self, index: usize) -> Option<&T> {
        if index >= self.len {
            return None;
        }
        // SAFETY: this typestate is created only by `assume_init`.
        Some(unsafe { &*self.ptr.as_ptr().add(index).cast::<T>() })
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut T> {
        if index >= self.len {
            return None;
        }
        // SAFETY: this typestate is created only by `assume_init`, and the
        // mutable capability borrow establishes exclusivity.
        Some(unsafe { &mut *self.ptr.as_ptr().add(index).cast::<T>() })
    }

    pub fn as_slice(&self) -> &[T] {
        // SAFETY: the constructor validated the slice and this typestate is
        // created only after the caller establishes complete initialization.
        unsafe { slice::from_raw_parts(self.ptr.as_ptr().cast::<T>(), self.len) }
    }

    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: initialization is established by the typestate and the
        // mutable capability borrow establishes exclusivity.
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr().cast::<T>(), self.len) }
    }
}

impl<T: LdsElement, State> DynamicLds<'_, T, State> {
    pub const fn len(&self) -> usize {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn byte_len(&self) -> usize {
        self.byte_len
    }

    /// Consumes this capability and returns two disjoint capabilities.
    ///
    /// On failure the original capability is returned unchanged. Consuming
    /// `self` prevents duplication, and the two resulting extents cannot
    /// overlap.
    pub fn split_at(self, mid: usize) -> Result<(Self, Self), Self> {
        if mid > self.len {
            return Err(self);
        }

        let right_len = self.len - mid;
        let left_bytes = mid * size_of::<T>();
        let right_bytes = self.byte_len - left_bytes;
        // SAFETY: `mid <= len`; one-past-the-end is valid for an empty right
        // slice, and non-zero element size was checked at construction.
        let right_ptr = unsafe { NonNull::new_unchecked(self.ptr.as_ptr().add(mid)) };
        let left = Self {
            ptr: self.ptr,
            len: mid,
            byte_len: left_bytes,
            _borrow: PhantomData,
            _state: PhantomData,
            _not_send_sync: PhantomData,
        };
        let right = Self {
            ptr: right_ptr,
            len: right_len,
            byte_len: right_bytes,
            _borrow: PhantomData,
            _state: PhantomData,
            _not_send_sync: PhantomData,
        };
        Ok((left, right))
    }
}

impl<T: LdsElement, State> fmt::Debug for DynamicLds<'_, T, State> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("DynamicLds")
            .field("len", &self.len)
            .field("byte_len", &self.byte_len)
            .field("element_size", &size_of::<T>())
            .field("element_alignment", &align_of::<T>())
            .finish_non_exhaustive()
    }
}

fn validate_layout(
    base_address: usize,
    allocated_bytes: usize,
    element_size: usize,
    element_alignment: usize,
) -> Result<usize, DynamicLdsError> {
    if element_size == 0 {
        return Err(DynamicLdsError::ZeroSizedElement);
    }
    if element_alignment > MAX_DYNAMIC_LDS_ALIGNMENT {
        return Err(DynamicLdsError::UnsupportedElementAlignment {
            required: element_alignment,
            maximum: MAX_DYNAMIC_LDS_ALIGNMENT,
        });
    }
    if allocated_bytes > isize::MAX as usize {
        return Err(DynamicLdsError::ExtentTooLarge {
            bytes: allocated_bytes,
        });
    }
    if !allocated_bytes.is_multiple_of(element_size) {
        return Err(DynamicLdsError::PartialElement {
            bytes: allocated_bytes,
            element_size,
        });
    }
    if base_address == 0 {
        return Err(DynamicLdsError::NullBase);
    }
    if !base_address.is_multiple_of(element_alignment) {
        return Err(DynamicLdsError::MisalignedBase {
            required: element_alignment,
        });
    }
    if base_address.checked_add(allocated_bytes).is_none() {
        return Err(DynamicLdsError::AddressOverflow);
    }
    Ok(allocated_bytes / element_size)
}

#[cfg(test)]
mod tests {
    use super::{
        DynamicLds, DynamicLdsError, LdsInitialized, MAX_DYNAMIC_LDS_ALIGNMENT, WorkgroupLdsScope,
        validate_layout,
    };
    use core::mem::{MaybeUninit, align_of, size_of};

    #[test]
    fn layout_validation_is_exact_and_bounded() {
        assert_eq!(validate_layout(16, 32, 4, 4), Ok(8));
        assert_eq!(validate_layout(16, 0, 4, 4), Ok(0));
        assert_eq!(
            validate_layout(16, 1, 0, 1),
            Err(DynamicLdsError::ZeroSizedElement)
        );
        assert_eq!(
            validate_layout(32, 32, 4, 32),
            Err(DynamicLdsError::UnsupportedElementAlignment {
                required: 32,
                maximum: MAX_DYNAMIC_LDS_ALIGNMENT,
            })
        );
        assert_eq!(
            validate_layout(18, 32, 4, 4),
            Err(DynamicLdsError::MisalignedBase { required: 4 })
        );
        assert_eq!(
            validate_layout(16, 10, 4, 4),
            Err(DynamicLdsError::PartialElement {
                bytes: 10,
                element_size: 4,
            })
        );
        assert_eq!(validate_layout(0, 0, 4, 4), Err(DynamicLdsError::NullBase));
        assert_eq!(
            validate_layout(usize::MAX - 7, 8, 1, 1),
            Err(DynamicLdsError::AddressOverflow)
        );
        assert_eq!(
            validate_layout(16, isize::MAX as usize + 1, 1, 1),
            Err(DynamicLdsError::ExtentTooLarge {
                bytes: isize::MAX as usize + 1,
            })
        );
    }

    #[test]
    fn zero_length_supported_array_is_rejected_at_construction() {
        let mut scope = WorkgroupLdsScope::for_host_test();
        let result = unsafe {
            DynamicLds::<[u8; 0]>::from_host_parts_for_test(
                &mut scope,
                core::ptr::NonNull::<u8>::dangling().as_ptr(),
                0,
            )
        };
        assert_eq!(result.unwrap_err(), DynamicLdsError::ZeroSizedElement);
    }

    #[test]
    fn uninitialized_access_and_transition_are_explicit() {
        let mut storage = [MaybeUninit::<u32>::uninit(); 4];
        // SAFETY: this host-side test supplies a unique scope and allocation
        // whose lifetime and alignment cover the view.
        let mut scope = WorkgroupLdsScope::for_host_test();
        let mut lds = unsafe {
            DynamicLds::<u32>::from_host_parts_for_test(
                &mut scope,
                storage.as_mut_ptr().cast::<u8>(),
                size_of_val(&storage),
            )
            .unwrap()
        };

        assert_eq!(lds.len(), 4);
        assert_eq!(lds.byte_len(), 16);
        assert!(!lds.is_empty());
        assert!(lds.get_uninit(4).is_none());
        assert!(lds.get_uninit_mut(4).is_none());
        assert_eq!(lds.as_uninit_slice().len(), 4);
        assert_eq!(lds.as_uninit_slice_mut().len(), 4);
        for (index, value) in [10, 20, 30, 40].into_iter().enumerate() {
            assert_eq!(lds.write(index, value).map(|item| *item), Some(value));
        }

        // SAFETY: every element was initialized by `write` above.
        let mut lds: DynamicLds<'_, u32, LdsInitialized> = unsafe { lds.assume_init() };
        assert_eq!(lds.as_slice(), &[10, 20, 30, 40]);
        *lds.get_mut(2).unwrap() = 31;
        assert_eq!(lds.get(2), Some(&31));
        lds.as_mut_slice()[3] = 41;
        assert_eq!(lds.as_slice(), &[10, 20, 31, 41]);
    }

    #[test]
    fn consuming_split_produces_disjoint_extents() {
        let mut storage = [MaybeUninit::<u32>::uninit(); 4];
        // SAFETY: this host-side test supplies a unique scope and allocation.
        let mut scope = WorkgroupLdsScope::for_host_test();
        let lds = unsafe {
            DynamicLds::<u32>::from_host_parts_for_test(
                &mut scope,
                storage.as_mut_ptr().cast::<u8>(),
                size_of_val(&storage),
            )
            .unwrap()
        };
        let (mut left, mut right) = lds.split_at(1).unwrap();
        assert_eq!((left.len(), left.byte_len()), (1, 4));
        assert_eq!((right.len(), right.byte_len()), (3, 12));
        left.write(0, 7).unwrap();
        right.write(0, 11).unwrap();
        assert_eq!(unsafe { storage[0].assume_init() }, 7);
        assert_eq!(unsafe { storage[1].assume_init() }, 11);
    }

    #[test]
    fn failed_split_returns_the_original_capability() {
        let mut storage = [MaybeUninit::<u16>::uninit(); 2];
        // SAFETY: this host-side test supplies a unique scope and allocation.
        let mut scope = WorkgroupLdsScope::for_host_test();
        let lds = unsafe {
            DynamicLds::<u16>::from_host_parts_for_test(
                &mut scope,
                storage.as_mut_ptr().cast::<u8>(),
                size_of_val(&storage),
            )
            .unwrap()
        };
        let original = lds.split_at(3).unwrap_err();
        assert_eq!((original.len(), original.byte_len()), (2, 4));
    }

    #[test]
    fn scalar_and_array_layouts_are_supported() {
        assert_eq!(size_of::<[u32; 4]>(), 16);
        assert_eq!(align_of::<[u32; 4]>(), align_of::<u32>());
        assert!(align_of::<[u64; 2]>() <= MAX_DYNAMIC_LDS_ALIGNMENT);
    }
}
