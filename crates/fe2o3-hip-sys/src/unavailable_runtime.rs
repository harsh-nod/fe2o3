//! Fail-closed core HIP ABI used when no runtime library is available.

use crate::{
    HIP_ERROR_NOT_SUPPORTED, hipDeviceptr_t, hipError_t, hipEvent_t, hipFunction_t, hipMemcpyKind,
    hipModule_t, hipStream_t,
};
use core::ffi::{c_char, c_int, c_uint, c_void};

static HIP_RUNTIME_UNAVAILABLE: &[u8] = b"HIP runtime unavailable\0";

macro_rules! unavailable_output {
    (
        $name:ident($($argument:ident : $argument_ty:ty),*),
        clear $output:ident as $output_ty:ty
    ) => {
        #[doc = "Fails closed and clears the output when the HIP runtime is unavailable."]
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = "Every non-null output pointer must identify one writable value."]
        pub unsafe extern "C" fn $name($($argument: $argument_ty),*) -> hipError_t {
            let _ = ($(&$argument),*);
            if !$output.is_null() {
                // SAFETY: The caller promises that a non-null output is writable.
                unsafe { $output.write(<$output_ty>::default()) };
            }
            HIP_ERROR_NOT_SUPPORTED
        }
    };
}

macro_rules! unavailable_mutation {
    ($name:ident($($argument:ident : $argument_ty:ty),*)) => {
        #[allow(clippy::too_many_arguments)]
        #[doc = "Fails closed without dereferencing arguments when the HIP runtime is unavailable."]
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = "The arguments must satisfy the corresponding raw HIP ABI contract."]
        pub unsafe extern "C" fn $name($($argument: $argument_ty),*) -> hipError_t {
            let _ = ($($argument),*);
            HIP_ERROR_NOT_SUPPORTED
        }
    };
}

unavailable_mutation!(hipInit(flags: c_uint));
unavailable_output!(hipGetDeviceCount(count: *mut c_int), clear count as c_int);
unavailable_mutation!(hipSetDevice(device_id: c_int));

/// Returns a stable diagnostic for the unavailable runtime.
///
/// # Safety
///
/// The error code must follow the raw HIP ABI contract.
pub unsafe extern "C" fn hipGetErrorString(_error: hipError_t) -> *const c_char {
    HIP_RUNTIME_UNAVAILABLE.as_ptr().cast()
}

unavailable_output!(hipStreamCreate(stream: *mut hipStream_t), clear stream as hipStream_t);
unavailable_mutation!(hipStreamDestroy(stream: hipStream_t));
unavailable_mutation!(hipStreamSynchronize(stream: hipStream_t));

unavailable_output!(
    hipEventCreateWithFlags(event: *mut hipEvent_t, flags: c_uint),
    clear event as hipEvent_t
);
unavailable_mutation!(hipEventDestroy(event: hipEvent_t));
unavailable_mutation!(hipEventRecord(event: hipEvent_t, stream: hipStream_t));
unavailable_mutation!(hipEventSynchronize(event: hipEvent_t));
unavailable_mutation!(hipEventQuery(event: hipEvent_t));
unavailable_output!(
    hipEventElapsedTime(milliseconds: *mut f32, start: hipEvent_t, stop: hipEvent_t),
    clear milliseconds as f32
);

unavailable_output!(hipMalloc(ptr: *mut hipDeviceptr_t, size: usize), clear ptr as hipDeviceptr_t);
unavailable_mutation!(hipFree(ptr: hipDeviceptr_t));
unavailable_output!(
    hipHostMalloc(ptr: *mut *mut c_void, size: usize, flags: c_uint),
    clear ptr as *mut c_void
);
unavailable_mutation!(hipHostFree(ptr: *mut c_void));
unavailable_mutation!(hipMemcpyAsync(
    dst: *mut c_void,
    src: *const c_void,
    size_bytes: usize,
    kind: hipMemcpyKind,
    stream: hipStream_t
));
unavailable_mutation!(hipMemsetAsync(
    dst: *mut c_void,
    value: c_int,
    size_bytes: usize,
    stream: hipStream_t
));

unavailable_output!(
    hipModuleLoad(module: *mut hipModule_t, file_name: *const c_char),
    clear module as hipModule_t
);
unavailable_output!(
    hipModuleLoadData(module: *mut hipModule_t, image: *const c_void),
    clear module as hipModule_t
);
unavailable_mutation!(hipModuleUnload(module: hipModule_t));
unavailable_output!(
    hipModuleGetFunction(
        function: *mut hipFunction_t,
        module: hipModule_t,
        kernel_name: *const c_char
    ),
    clear function as hipFunction_t
);
unavailable_mutation!(hipModuleLaunchKernel(
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
    extra: *mut *mut c_void
));

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::CStr;
    use core::ptr;

    #[test]
    fn unavailable_runtime_clears_outputs_and_returns_stable_errors() {
        let dangling = ptr::dangling_mut::<c_void>();
        let mut count = 7;
        let mut stream = dangling;
        let mut event = dangling;
        let mut elapsed = 1.0;
        let mut device_pointer = dangling;
        let mut host_pointer = dangling;
        let mut module = dangling;
        let mut function = dangling;

        // SAFETY: Every output points to one writable value; fallbacks dereference no inputs.
        unsafe {
            assert_eq!(hipGetDeviceCount(&mut count), HIP_ERROR_NOT_SUPPORTED);
            assert_eq!(hipStreamCreate(&mut stream), HIP_ERROR_NOT_SUPPORTED);
            assert_eq!(
                hipEventCreateWithFlags(&mut event, 0),
                HIP_ERROR_NOT_SUPPORTED
            );
            assert_eq!(
                hipEventElapsedTime(&mut elapsed, ptr::null_mut(), ptr::null_mut()),
                HIP_ERROR_NOT_SUPPORTED
            );
            assert_eq!(hipMalloc(&mut device_pointer, 16), HIP_ERROR_NOT_SUPPORTED);
            assert_eq!(
                hipHostMalloc(&mut host_pointer, 16, 0),
                HIP_ERROR_NOT_SUPPORTED
            );
            assert_eq!(
                hipModuleLoadData(&mut module, ptr::null()),
                HIP_ERROR_NOT_SUPPORTED
            );
            assert_eq!(
                hipModuleGetFunction(&mut function, ptr::null_mut(), ptr::null()),
                HIP_ERROR_NOT_SUPPORTED
            );
        }

        assert_eq!(count, 0);
        assert!(stream.is_null());
        assert!(event.is_null());
        assert_eq!(elapsed, 0.0);
        assert!(device_pointer.is_null());
        assert!(host_pointer.is_null());
        assert!(module.is_null());
        assert!(function.is_null());

        // SAFETY: The fallback returns a process-lifetime NUL-terminated static string.
        let error = unsafe { CStr::from_ptr(hipGetErrorString(HIP_ERROR_NOT_SUPPORTED)) };
        assert_eq!(error.to_bytes(), b"HIP runtime unavailable");
    }
}
