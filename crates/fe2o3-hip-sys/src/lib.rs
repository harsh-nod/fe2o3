#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use core::ffi::{c_char, c_int, c_uint, c_void};

mod cooperative_peer;

pub use cooperative_peer::*;

pub type hipError_t = c_int;
pub type hipStream_t = *mut c_void;
pub type hipEvent_t = *mut c_void;
pub type hipModule_t = *mut c_void;
pub type hipFunction_t = *mut c_void;
pub type hipDeviceptr_t = *mut c_void;
pub type hipMemcpyKind = c_uint;

pub const HIP_SUCCESS: hipError_t = 0;
pub const HIP_ERROR_NOT_READY: hipError_t = 600;
pub const HIP_ERROR_PEER_ACCESS_ALREADY_ENABLED: hipError_t = 704;
pub const HIP_ERROR_PEER_ACCESS_NOT_ENABLED: hipError_t = 705;
pub const HIP_ERROR_COOPERATIVE_LAUNCH_TOO_LARGE: hipError_t = 720;
pub const HIP_ERROR_NOT_SUPPORTED: hipError_t = 801;

pub const HIP_DEVICE_ARCH_HAS_GLOBAL_INT32_ATOMICS: u64 = 1 << 0;
pub const HIP_DEVICE_ARCH_HAS_SHARED_INT32_ATOMICS: u64 = 1 << 1;
pub const HIP_DEVICE_ARCH_HAS_GLOBAL_INT64_ATOMICS: u64 = 1 << 2;
pub const HIP_DEVICE_ARCH_HAS_SHARED_INT64_ATOMICS: u64 = 1 << 3;
pub const HIP_DEVICE_ARCH_HAS_WARP_VOTE: u64 = 1 << 4;
pub const HIP_DEVICE_ARCH_HAS_WARP_BALLOT: u64 = 1 << 5;
pub const HIP_DEVICE_ARCH_HAS_WARP_SHUFFLE: u64 = 1 << 6;

/// Whether the build found HIP headers and compiled device-property discovery.
pub const HIP_DEVICE_PROPERTIES_AVAILABLE: bool = cfg!(fe2o3_hip_device_properties);

/// Whether the build found a HIP runtime library to link.
pub const HIP_RUNTIME_AVAILABLE: bool = cfg!(fe2o3_hip_runtime);

/// Stable subset of `hipDeviceProp_t` used by fe2o3 runtime discovery.
///
/// The native shim owns compatibility with the versioned HIP structure and its
/// C bitfields. All sizes use fixed-width types at this ABI boundary.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fe2o3HipDeviceProperties {
    pub gcn_arch_name: [c_char; 256],
    pub warp_size: i32,
    pub max_threads_per_block: i32,
    pub max_block_dim: [i32; 3],
    pub max_grid_dim: [i32; 3],
    pub shared_mem_per_block: u64,
    pub shared_mem_per_block_optin: u64,
    pub architecture_features: u64,
}

impl Default for Fe2o3HipDeviceProperties {
    fn default() -> Self {
        Self {
            gcn_arch_name: [0; 256],
            warp_size: 0,
            max_threads_per_block: 0,
            max_block_dim: [0; 3],
            max_grid_dim: [0; 3],
            shared_mem_per_block: 0,
            shared_mem_per_block_optin: 0,
            architecture_features: 0,
        }
    }
}

// Values from ROCm 7.2 hip_runtime_api.h. Default events use active
// synchronization and record timing information.
pub const HIP_EVENT_DEFAULT: c_uint = 0x0;
pub const HIP_EVENT_BLOCKING_SYNC: c_uint = 0x1;
pub const HIP_EVENT_DISABLE_TIMING: c_uint = 0x2;

// Value from ROCm 7.2 hip_runtime_api.h.
pub const HIP_HOST_MALLOC_DEFAULT: c_uint = 0x0;

pub const HIP_MEMCPY_HOST_TO_HOST: hipMemcpyKind = 0;
pub const HIP_MEMCPY_HOST_TO_DEVICE: hipMemcpyKind = 1;
pub const HIP_MEMCPY_DEVICE_TO_HOST: hipMemcpyKind = 2;
pub const HIP_MEMCPY_DEVICE_TO_DEVICE: hipMemcpyKind = 3;
pub const HIP_MEMCPY_DEFAULT: hipMemcpyKind = 4;

unsafe extern "C" {
    pub fn hipInit(flags: c_uint) -> hipError_t;
    pub fn hipGetDeviceCount(count: *mut c_int) -> hipError_t;
    pub fn hipSetDevice(device_id: c_int) -> hipError_t;
    pub fn hipGetErrorString(error: hipError_t) -> *const c_char;

    #[cfg(fe2o3_hip_device_properties)]
    /// Queries the stable fe2o3 subset of HIP properties for `device_id`.
    ///
    /// # Safety
    ///
    /// `properties` must be non-null, aligned, and valid for one writable
    /// `Fe2o3HipDeviceProperties` value. The HIP runtime must be initialized.
    pub fn fe2o3HipGetDeviceProperties(
        device_id: c_int,
        properties: *mut Fe2o3HipDeviceProperties,
    ) -> hipError_t;

    pub fn hipStreamCreate(stream: *mut hipStream_t) -> hipError_t;
    pub fn hipStreamDestroy(stream: hipStream_t) -> hipError_t;
    pub fn hipStreamSynchronize(stream: hipStream_t) -> hipError_t;

    pub fn hipEventCreateWithFlags(event: *mut hipEvent_t, flags: c_uint) -> hipError_t;
    pub fn hipEventDestroy(event: hipEvent_t) -> hipError_t;
    pub fn hipEventRecord(event: hipEvent_t, stream: hipStream_t) -> hipError_t;
    pub fn hipEventSynchronize(event: hipEvent_t) -> hipError_t;
    pub fn hipEventQuery(event: hipEvent_t) -> hipError_t;
    pub fn hipEventElapsedTime(
        milliseconds: *mut f32,
        start: hipEvent_t,
        stop: hipEvent_t,
    ) -> hipError_t;

    pub fn hipMalloc(ptr: *mut *mut c_void, size: usize) -> hipError_t;
    pub fn hipFree(ptr: *mut c_void) -> hipError_t;
    pub fn hipHostMalloc(ptr: *mut *mut c_void, size: usize, flags: c_uint) -> hipError_t;
    pub fn hipHostFree(ptr: *mut c_void) -> hipError_t;
    pub fn hipMemcpyAsync(
        dst: *mut c_void,
        src: *const c_void,
        size_bytes: usize,
        kind: hipMemcpyKind,
        stream: hipStream_t,
    ) -> hipError_t;
    pub fn hipMemsetAsync(
        dst: *mut c_void,
        value: c_int,
        size_bytes: usize,
        stream: hipStream_t,
    ) -> hipError_t;

    pub fn hipModuleLoad(module: *mut hipModule_t, file_name: *const c_char) -> hipError_t;
    pub fn hipModuleLoadData(module: *mut hipModule_t, image: *const c_void) -> hipError_t;
    pub fn hipModuleUnload(module: hipModule_t) -> hipError_t;
    pub fn hipModuleGetFunction(
        function: *mut hipFunction_t,
        module: hipModule_t,
        kernel_name: *const c_char,
    ) -> hipError_t;
    pub fn hipModuleLaunchKernel(
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
        extra: *mut *mut c_void,
    ) -> hipError_t;
}

/// Fail-closed implementation used when this crate was built without HIP headers.
///
/// # Safety
///
/// If `properties` is non-null, it must be valid and aligned for one writable
/// `Fe2o3HipDeviceProperties` value.
#[cfg(not(fe2o3_hip_device_properties))]
pub unsafe extern "C" fn fe2o3HipGetDeviceProperties(
    _device_id: c_int,
    properties: *mut Fe2o3HipDeviceProperties,
) -> hipError_t {
    if !properties.is_null() {
        // SAFETY: The caller guarantees that a non-null output pointer is writable.
        unsafe { properties.write(Fe2o3HipDeviceProperties::default()) };
    }
    HIP_ERROR_NOT_SUPPORTED
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};

    #[test]
    fn device_property_abi_is_stable() {
        assert_eq!(size_of::<Fe2o3HipDeviceProperties>(), 312);
        assert_eq!(align_of::<Fe2o3HipDeviceProperties>(), 8);
        assert_eq!(offset_of!(Fe2o3HipDeviceProperties, gcn_arch_name), 0);
        assert_eq!(offset_of!(Fe2o3HipDeviceProperties, warp_size), 256);
        assert_eq!(
            offset_of!(Fe2o3HipDeviceProperties, max_threads_per_block),
            260
        );
        assert_eq!(offset_of!(Fe2o3HipDeviceProperties, max_block_dim), 264);
        assert_eq!(offset_of!(Fe2o3HipDeviceProperties, max_grid_dim), 276);
        assert_eq!(
            offset_of!(Fe2o3HipDeviceProperties, shared_mem_per_block),
            288
        );
        assert_eq!(
            offset_of!(Fe2o3HipDeviceProperties, shared_mem_per_block_optin),
            296
        );
        assert_eq!(
            offset_of!(Fe2o3HipDeviceProperties, architecture_features),
            304
        );
    }

    #[test]
    fn device_architecture_feature_bits_are_disjoint() {
        let features = [
            HIP_DEVICE_ARCH_HAS_GLOBAL_INT32_ATOMICS,
            HIP_DEVICE_ARCH_HAS_SHARED_INT32_ATOMICS,
            HIP_DEVICE_ARCH_HAS_GLOBAL_INT64_ATOMICS,
            HIP_DEVICE_ARCH_HAS_SHARED_INT64_ATOMICS,
            HIP_DEVICE_ARCH_HAS_WARP_VOTE,
            HIP_DEVICE_ARCH_HAS_WARP_BALLOT,
            HIP_DEVICE_ARCH_HAS_WARP_SHUFFLE,
        ];

        assert_eq!(features, [1, 2, 4, 8, 16, 32, 64]);
        assert_eq!(
            features.into_iter().reduce(|left, right| left | right),
            Some(0x7f)
        );
    }

    #[cfg(not(fe2o3_hip_device_properties))]
    #[test]
    fn unavailable_device_properties_fail_closed_and_clear_output() {
        let mut properties = Fe2o3HipDeviceProperties {
            warp_size: 64,
            architecture_features: u64::MAX,
            ..Fe2o3HipDeviceProperties::default()
        };

        // SAFETY: `properties` is a live writable value.
        let status = unsafe { fe2o3HipGetDeviceProperties(0, &mut properties) };

        assert_eq!(status, HIP_ERROR_NOT_SUPPORTED);
        assert_eq!(properties, Fe2o3HipDeviceProperties::default());
    }

    #[cfg(fe2o3_hip_device_properties)]
    #[test]
    #[ignore = "requires a configured HIP runtime and an AMD GPU"]
    fn queries_device_properties_from_hip() {
        // SAFETY: HIP initialization has no additional caller obligations.
        assert_eq!(unsafe { hipInit(0) }, HIP_SUCCESS);

        let mut properties = Fe2o3HipDeviceProperties::default();
        // SAFETY: `properties` is a live writable value and device zero is
        // validated by HIP.
        let status = unsafe { fe2o3HipGetDeviceProperties(0, &mut properties) };

        assert_eq!(status, HIP_SUCCESS);
        assert!(properties.gcn_arch_name.contains(&0));
        assert!(properties.gcn_arch_name.starts_with(&[
            b'g' as c_char,
            b'f' as c_char,
            b'x' as c_char
        ]));
        assert!(matches!(properties.warp_size, 32 | 64));
        assert!(properties.max_threads_per_block > 0);
        assert!(properties.max_block_dim.into_iter().all(|dim| dim > 0));
        assert!(properties.max_grid_dim.into_iter().all(|dim| dim > 0));
        assert!(properties.shared_mem_per_block > 0);
        assert!(
            properties.shared_mem_per_block_optin == 0
                || properties.shared_mem_per_block_optin >= properties.shared_mem_per_block
        );
        assert_ne!(properties.architecture_features, 0);
        assert_eq!(properties.architecture_features & !0x7f, 0);
    }
}
