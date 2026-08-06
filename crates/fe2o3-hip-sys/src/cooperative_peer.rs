//! Raw cooperative-launch and peer-access HIP ABI.
//!
//! These bindings report only what the HIP runtime returns. They do not prove
//! occupancy, establish peer mappings, authorize a launch, or make any memory
//! operation safe.

#[cfg(not(fe2o3_hip_cooperative_peer))]
use crate::HIP_ERROR_NOT_SUPPORTED;
use crate::{hipError_t, hipFunction_t, hipStream_t};
use core::ffi::{c_int, c_uint, c_void};

pub type hipDeviceAttribute_t = c_int;

/// Minimum HIP header major version accepted by the C ABI probe.
pub const HIP_COOPERATIVE_PEER_MIN_MAJOR: u32 = 5;

/// Whether compatible HIP headers and a runtime library were found at build time.
pub const HIP_COOPERATIVE_PEER_AVAILABLE: bool = cfg!(fe2o3_hip_cooperative_peer);

pub const HIP_DEVICE_ATTRIBUTE_COOPERATIVE_LAUNCH: hipDeviceAttribute_t = 10;
pub const HIP_DEVICE_ATTRIBUTE_COOPERATIVE_MULTI_DEVICE_LAUNCH: hipDeviceAttribute_t = 11;

/// The only currently valid flags value for `hipDeviceEnablePeerAccess`.
pub const HIP_PEER_ACCESS_DEFAULT: c_uint = 0;

pub const HIP_COOPERATIVE_LAUNCH_MULTI_DEVICE_NO_PRE_SYNC: c_uint = 0x01;
pub const HIP_COOPERATIVE_LAUNCH_MULTI_DEVICE_NO_POST_SYNC: c_uint = 0x02;

/// Raw HIP launch dimensions, ABI-compatible with C `dim3`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct hipDim3 {
    pub x: c_uint,
    pub y: c_uint,
    pub z: c_uint,
}

impl hipDim3 {
    pub const fn new(x: c_uint, y: c_uint, z: c_uint) -> Self {
        Self { x, y, z }
    }
}

#[cfg(fe2o3_hip_cooperative_peer)]
unsafe extern "C" {
    pub fn hipDeviceGetAttribute(
        value: *mut c_int,
        attribute: hipDeviceAttribute_t,
        device_id: c_int,
    ) -> hipError_t;

    pub fn hipDeviceCanAccessPeer(
        can_access_peer: *mut c_int,
        device_id: c_int,
        peer_device_id: c_int,
    ) -> hipError_t;

    pub fn hipDeviceEnablePeerAccess(peer_device_id: c_int, flags: c_uint) -> hipError_t;
    pub fn hipDeviceDisablePeerAccess(peer_device_id: c_int) -> hipError_t;

    pub fn hipModuleLaunchCooperativeKernel(
        function: hipFunction_t,
        grid_dim_x: c_uint,
        grid_dim_y: c_uint,
        grid_dim_z: c_uint,
        block_dim_x: c_uint,
        block_dim_y: c_uint,
        block_dim_z: c_uint,
        shared_mem_bytes: c_uint,
        stream: hipStream_t,
        kernel_params: *mut *mut c_void,
    ) -> hipError_t;

    pub fn hipLaunchCooperativeKernel(
        function: *const c_void,
        grid_dim: hipDim3,
        block_dim: hipDim3,
        kernel_params: *mut *mut c_void,
        shared_mem_bytes: c_uint,
        stream: hipStream_t,
    ) -> hipError_t;
}

/// Fail-closed fallback when compatible HIP headers were unavailable.
///
/// # Safety
///
/// A non-null `value` must point to one writable `c_int`.
#[cfg(not(fe2o3_hip_cooperative_peer))]
pub unsafe extern "C" fn hipDeviceGetAttribute(
    value: *mut c_int,
    _attribute: hipDeviceAttribute_t,
    _device_id: c_int,
) -> hipError_t {
    clear_output(value);
    HIP_ERROR_NOT_SUPPORTED
}

/// Fail-closed fallback when compatible HIP headers were unavailable.
///
/// # Safety
///
/// A non-null `can_access_peer` must point to one writable `c_int`.
#[cfg(not(fe2o3_hip_cooperative_peer))]
pub unsafe extern "C" fn hipDeviceCanAccessPeer(
    can_access_peer: *mut c_int,
    _device_id: c_int,
    _peer_device_id: c_int,
) -> hipError_t {
    clear_output(can_access_peer);
    HIP_ERROR_NOT_SUPPORTED
}

/// Fail-closed fallback when compatible HIP headers were unavailable.
///
/// # Safety
///
/// This fallback does not dereference any pointer or call HIP.
#[cfg(not(fe2o3_hip_cooperative_peer))]
pub unsafe extern "C" fn hipDeviceEnablePeerAccess(
    _peer_device_id: c_int,
    _flags: c_uint,
) -> hipError_t {
    HIP_ERROR_NOT_SUPPORTED
}

/// Fail-closed fallback when compatible HIP headers were unavailable.
///
/// # Safety
///
/// This fallback does not dereference any pointer or call HIP.
#[cfg(not(fe2o3_hip_cooperative_peer))]
pub unsafe extern "C" fn hipDeviceDisablePeerAccess(_peer_device_id: c_int) -> hipError_t {
    HIP_ERROR_NOT_SUPPORTED
}

/// Fail-closed fallback when compatible HIP headers were unavailable.
///
/// # Safety
///
/// This fallback does not dereference any pointer or call HIP.
#[cfg(not(fe2o3_hip_cooperative_peer))]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn hipModuleLaunchCooperativeKernel(
    _function: hipFunction_t,
    _grid_dim_x: c_uint,
    _grid_dim_y: c_uint,
    _grid_dim_z: c_uint,
    _block_dim_x: c_uint,
    _block_dim_y: c_uint,
    _block_dim_z: c_uint,
    _shared_mem_bytes: c_uint,
    _stream: hipStream_t,
    _kernel_params: *mut *mut c_void,
) -> hipError_t {
    HIP_ERROR_NOT_SUPPORTED
}

/// Fail-closed fallback when compatible HIP headers were unavailable.
///
/// # Safety
///
/// This fallback does not dereference any pointer or call HIP.
#[cfg(not(fe2o3_hip_cooperative_peer))]
pub unsafe extern "C" fn hipLaunchCooperativeKernel(
    _function: *const c_void,
    _grid_dim: hipDim3,
    _block_dim: hipDim3,
    _kernel_params: *mut *mut c_void,
    _shared_mem_bytes: c_uint,
    _stream: hipStream_t,
) -> hipError_t {
    HIP_ERROR_NOT_SUPPORTED
}

#[cfg(not(fe2o3_hip_cooperative_peer))]
fn clear_output(output: *mut c_int) {
    if !output.is_null() {
        // SAFETY: Each fallback's contract requires a writable non-null output.
        unsafe { output.write(0) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    #[test]
    fn dim3_abi_is_stable() {
        assert_eq!(size_of::<hipDim3>(), 12);
        assert_eq!(align_of::<hipDim3>(), 4);
        assert_eq!(hipDim3::new(2, 3, 5), hipDim3 { x: 2, y: 3, z: 5 });
    }

    #[test]
    fn constants_match_the_gated_c_abi() {
        assert_eq!(HIP_COOPERATIVE_PEER_MIN_MAJOR, 5);
        assert_eq!(HIP_DEVICE_ATTRIBUTE_COOPERATIVE_LAUNCH, 10);
        assert_eq!(HIP_DEVICE_ATTRIBUTE_COOPERATIVE_MULTI_DEVICE_LAUNCH, 11);
        assert_eq!(HIP_PEER_ACCESS_DEFAULT, 0);
        assert_eq!(HIP_COOPERATIVE_LAUNCH_MULTI_DEVICE_NO_PRE_SYNC, 1);
        assert_eq!(HIP_COOPERATIVE_LAUNCH_MULTI_DEVICE_NO_POST_SYNC, 2);
        assert_eq!(crate::HIP_ERROR_PEER_ACCESS_ALREADY_ENABLED, 704);
        assert_eq!(crate::HIP_ERROR_PEER_ACCESS_NOT_ENABLED, 705);
        assert_eq!(crate::HIP_ERROR_COOPERATIVE_LAUNCH_TOO_LARGE, 720);
    }

    #[test]
    fn raw_function_signatures_are_stable() {
        let _: unsafe extern "C" fn(*mut c_int, hipDeviceAttribute_t, c_int) -> hipError_t =
            hipDeviceGetAttribute;
        let _: unsafe extern "C" fn(*mut c_int, c_int, c_int) -> hipError_t =
            hipDeviceCanAccessPeer;
        let _: unsafe extern "C" fn(c_int, c_uint) -> hipError_t = hipDeviceEnablePeerAccess;
        let _: unsafe extern "C" fn(c_int) -> hipError_t = hipDeviceDisablePeerAccess;
        let _: unsafe extern "C" fn(
            hipFunction_t,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            c_uint,
            hipStream_t,
            *mut *mut c_void,
        ) -> hipError_t = hipModuleLaunchCooperativeKernel;
        let _: unsafe extern "C" fn(
            *const c_void,
            hipDim3,
            hipDim3,
            *mut *mut c_void,
            c_uint,
            hipStream_t,
        ) -> hipError_t = hipLaunchCooperativeKernel;
    }

    #[cfg(not(fe2o3_hip_cooperative_peer))]
    #[test]
    fn unavailable_queries_fail_closed_and_clear_outputs() {
        const {
            assert!(!crate::HIP_RUNTIME_AVAILABLE);
            assert!(!HIP_COOPERATIVE_PEER_AVAILABLE);
        }

        let mut cooperative = 1;
        let mut peer = 1;

        // SAFETY: Both outputs are live writable integers; no HIP call occurs.
        let cooperative_status = unsafe {
            hipDeviceGetAttribute(&mut cooperative, HIP_DEVICE_ATTRIBUTE_COOPERATIVE_LAUNCH, 0)
        };
        // SAFETY: `peer` is a live writable integer; no HIP call occurs.
        let peer_status = unsafe { hipDeviceCanAccessPeer(&mut peer, 0, 1) };

        assert_eq!(cooperative_status, HIP_ERROR_NOT_SUPPORTED);
        assert_eq!(peer_status, HIP_ERROR_NOT_SUPPORTED);
        assert_eq!(cooperative, 0);
        assert_eq!(peer, 0);
    }

    #[cfg(not(fe2o3_hip_cooperative_peer))]
    #[test]
    fn unavailable_mutating_and_launch_entries_fail_closed() {
        // SAFETY: The unavailable-path functions do not call HIP or dereference pointers.
        unsafe {
            assert_eq!(
                hipDeviceEnablePeerAccess(1, HIP_PEER_ACCESS_DEFAULT),
                HIP_ERROR_NOT_SUPPORTED
            );
            assert_eq!(hipDeviceDisablePeerAccess(1), HIP_ERROR_NOT_SUPPORTED);
            assert_eq!(
                hipModuleLaunchCooperativeKernel(
                    core::ptr::null_mut(),
                    1,
                    1,
                    1,
                    1,
                    1,
                    1,
                    0,
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                ),
                HIP_ERROR_NOT_SUPPORTED
            );
            assert_eq!(
                hipLaunchCooperativeKernel(
                    core::ptr::null(),
                    hipDim3::new(1, 1, 1),
                    hipDim3::new(1, 1, 1),
                    core::ptr::null_mut(),
                    0,
                    core::ptr::null_mut(),
                ),
                HIP_ERROR_NOT_SUPPORTED
            );
        }
    }
}
