//! Explicit physical pointer types for reviewed device-to-device FFI.
//!
//! These wrappers describe an ABI address space. Their representation alone
//! does not prove that a pointer is valid, live, aligned, non-aliasing, or
//! usable by a particular device invocation. Unsafe construction or compiler
//! ABI admission must establish the complete invariant required by every safe
//! capability subsequently used through the wrapper.

use core::marker::PhantomData;

/// Marker implemented only for V1 scalar and address-space-qualified pointer
/// ABI types accepted by the device FFI macros.
///
/// # Safety
///
/// Implementations must have the exact V1 physical layout encoded by the
/// compiler contract. User implementations are unsupported.
#[doc(hidden)]
pub unsafe trait DeviceFfiAbiTypeV1: Copy + 'static {}

macro_rules! scalar_abi_types {
    ($($ty:ty),* $(,)?) => {$(
        // SAFETY: scalar layout is fixed by the V1 physical ABI grammar.
        unsafe impl DeviceFfiAbiTypeV1 for $ty {}
    )*};
}

scalar_abi_types!(i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);

macro_rules! const_pointer {
    ($name:ident, $diagnostic:literal) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        #[rustc_diagnostic_item = $diagnostic]
        pub struct $name<T> {
            pointer: *const T,
            _element: PhantomData<T>,
        }

        impl<T> $name<T> {
            /// Constructs an address-space-qualified pointer value.
            ///
            /// # Safety
            ///
            /// The pointer must originate in the named device address space
            /// and satisfy the imported or exported function's complete
            /// semantic, lifetime, alignment, and alias contract.
            pub const unsafe fn from_raw(pointer: *const T) -> Self {
                Self {
                    pointer,
                    _element: PhantomData,
                }
            }

            pub const fn as_raw(self) -> *const T {
                self.pointer
            }
        }

        // SAFETY: repr(transparent) preserves the one-pointer physical ABI.
        unsafe impl<T: DeviceFfiAbiTypeV1> DeviceFfiAbiTypeV1 for $name<T> {}
    };
}

macro_rules! mut_pointer {
    ($name:ident, $diagnostic:literal) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(transparent)]
        #[rustc_diagnostic_item = $diagnostic]
        pub struct $name<T> {
            pointer: *mut T,
            _element: PhantomData<T>,
        }

        impl<T> $name<T> {
            /// Constructs an address-space-qualified mutable pointer value.
            ///
            /// # Safety
            ///
            /// The pointer must originate in the named device address space
            /// and satisfy the imported or exported function's complete
            /// semantic, lifetime, alignment, race-freedom, and alias contract.
            /// Safe capabilities on concrete pointer specializations may state
            /// additional parts of this construction invariant.
            pub const unsafe fn from_raw(pointer: *mut T) -> Self {
                Self {
                    pointer,
                    _element: PhantomData,
                }
            }

            pub const fn as_raw(self) -> *mut T {
                self.pointer
            }
        }

        // SAFETY: repr(transparent) preserves the one-pointer physical ABI.
        unsafe impl<T: DeviceFfiAbiTypeV1> DeviceFfiAbiTypeV1 for $name<T> {}
    };
}

const_pointer!(DeviceGlobalConstPtr, "fe2o3_device_ffi_global_const_ptr_v1");
mut_pointer!(DeviceGlobalMutPtr, "fe2o3_device_ffi_global_mut_ptr_v1");
const_pointer!(DeviceConstantPtr, "fe2o3_device_ffi_constant_ptr_v1");
const_pointer!(
    DeviceWorkgroupConstPtr,
    "fe2o3_device_ffi_workgroup_const_ptr_v1"
);
mut_pointer!(
    DeviceWorkgroupMutPtr,
    "fe2o3_device_ffi_workgroup_mut_ptr_v1"
);
const_pointer!(
    DevicePrivateConstPtr,
    "fe2o3_device_ffi_private_const_ptr_v1"
);
mut_pointer!(DevicePrivateMutPtr, "fe2o3_device_ffi_private_mut_ptr_v1");

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    #[test]
    fn address_space_wrappers_have_one_pointer_layout() {
        assert_eq!(
            size_of::<DeviceGlobalConstPtr<u32>>(),
            size_of::<*const u32>()
        );
        assert_eq!(
            align_of::<DeviceGlobalConstPtr<u32>>(),
            align_of::<*const u32>()
        );
        assert_eq!(size_of::<DeviceGlobalMutPtr<u32>>(), size_of::<*mut u32>());
        assert_eq!(size_of::<DeviceConstantPtr<u32>>(), size_of::<*const u32>());
        assert_eq!(
            size_of::<DeviceWorkgroupConstPtr<u32>>(),
            size_of::<*const u32>()
        );
        assert_eq!(
            size_of::<DeviceWorkgroupMutPtr<u32>>(),
            size_of::<*mut u32>()
        );
        assert_eq!(
            size_of::<DevicePrivateConstPtr<u32>>(),
            size_of::<*const u32>()
        );
        assert_eq!(size_of::<DevicePrivateMutPtr<u32>>(), size_of::<*mut u32>());
    }
}
