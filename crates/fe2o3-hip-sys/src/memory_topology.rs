//! Raw, fixed-width HIP managed-memory and VMM ABI.
//!
//! These functions expose native operations only. A successful return does not
//! prove physical residency, peer reachability, synchronization, pointer
//! validity, or permission to launch a kernel.

#[cfg(not(fe2o3_hip_memory_topology))]
use crate::HIP_ERROR_NOT_SUPPORTED;
use crate::{hipError_t, hipStream_t};
use core::ffi::{c_char, c_int, c_uint, c_void};

pub const HIP_CPU_DEVICE_ID: c_int = -1;
pub const HIP_MEMORY_ADVISE_SET_READ_MOSTLY: c_uint = 1;
pub const HIP_MEMORY_ADVISE_UNSET_READ_MOSTLY: c_uint = 2;
pub const HIP_MEMORY_ADVISE_SET_PREFERRED_LOCATION: c_uint = 3;
pub const HIP_MEMORY_ADVISE_UNSET_PREFERRED_LOCATION: c_uint = 4;
pub const HIP_MEMORY_ADVISE_SET_ACCESSED_BY: c_uint = 5;
pub const HIP_MEMORY_ADVISE_UNSET_ACCESSED_BY: c_uint = 6;
pub const HIP_MEMORY_ADVISE_SET_COARSE_GRAIN: c_uint = 7;
pub const HIP_MEMORY_ADVISE_UNSET_COARSE_GRAIN: c_uint = 8;
pub const HIP_VMM_ACCESS_READ: c_uint = 1;
pub const HIP_VMM_ACCESS_READ_WRITE: c_uint = 2;

pub const HIP_MEMORY_TOPOLOGY_AVAILABLE: bool = cfg!(fe2o3_hip_memory_topology);

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Fe2o3HipPhysicalDeviceIdentity {
    pub uuid: [u8; 16],
    pub pci_bus_id: [c_char; 32],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Fe2o3HipMemoryCapabilities {
    pub managed_memory: c_int,
    pub concurrent_managed_access: c_int,
    pub pageable_memory_access: c_int,
    pub virtual_memory_management: c_int,
}

#[cfg(fe2o3_hip_memory_topology)]
unsafe extern "C" {
    pub fn fe2o3HipGetPhysicalDeviceIdentity(
        device_id: c_int,
        identity: *mut Fe2o3HipPhysicalDeviceIdentity,
    ) -> hipError_t;
    pub fn fe2o3HipGetMemoryCapabilities(
        device_id: c_int,
        capabilities: *mut Fe2o3HipMemoryCapabilities,
    ) -> hipError_t;
    pub fn fe2o3HipMallocManaged(pointer: *mut *mut c_void, size: usize) -> hipError_t;
    pub fn fe2o3HipMemPrefetchAsync(
        pointer: *const c_void,
        size: usize,
        device_id: c_int,
        stream: hipStream_t,
    ) -> hipError_t;
    pub fn fe2o3HipMemAdvise(
        pointer: *const c_void,
        size: usize,
        advice: c_uint,
        device_id: c_int,
    ) -> hipError_t;
    pub fn fe2o3HipMemRangeGetLastPrefetchLocation(
        pointer: *const c_void,
        size: usize,
        device_id: *mut c_int,
    ) -> hipError_t;
    pub fn fe2o3HipMemAddressReserve(
        pointer: *mut *mut c_void,
        size: usize,
        alignment: usize,
    ) -> hipError_t;
    pub fn fe2o3HipMemAddressFree(pointer: *mut c_void, size: usize) -> hipError_t;
    pub fn fe2o3HipMemGetAllocationGranularity(
        granularity: *mut usize,
        device_id: c_int,
    ) -> hipError_t;
    pub fn fe2o3HipMemCreate(handle: *mut usize, size: usize, device_id: c_int) -> hipError_t;
    pub fn fe2o3HipMemMap(pointer: *mut c_void, size: usize, handle: usize) -> hipError_t;
    pub fn fe2o3HipMemSetAccess(
        pointer: *mut c_void,
        size: usize,
        device_id: c_int,
        access: c_uint,
    ) -> hipError_t;
    pub fn fe2o3HipMemGetAccess(
        access: *mut c_uint,
        pointer: *mut c_void,
        device_id: c_int,
    ) -> hipError_t;
    pub fn fe2o3HipMemUnmap(pointer: *mut c_void, size: usize) -> hipError_t;
    pub fn fe2o3HipMemRelease(handle: usize) -> hipError_t;
}

macro_rules! unavailable_output {
    (
        $name:ident($($argument:ident : $argument_ty:ty),*),
        clear $output:ident as $output_ty:ty
    ) => {
        #[cfg(not(fe2o3_hip_memory_topology))]
        #[doc = "Fails closed and clears the output when the HIP ABI is unavailable."]
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

unavailable_output!(
    fe2o3HipGetPhysicalDeviceIdentity(
        device_id: c_int,
        identity: *mut Fe2o3HipPhysicalDeviceIdentity
    ),
    clear identity as Fe2o3HipPhysicalDeviceIdentity
);
unavailable_output!(
    fe2o3HipGetMemoryCapabilities(
        device_id: c_int,
        capabilities: *mut Fe2o3HipMemoryCapabilities
    ),
    clear capabilities as Fe2o3HipMemoryCapabilities
);
unavailable_output!(
    fe2o3HipMallocManaged(pointer: *mut *mut c_void, size: usize),
    clear pointer as *mut c_void
);
unavailable_output!(
    fe2o3HipMemRangeGetLastPrefetchLocation(
    pointer: *const c_void,
        size: usize,
        device_id: *mut c_int
    ),
    clear device_id as c_int
);
unavailable_output!(
    fe2o3HipMemAddressReserve(
        pointer: *mut *mut c_void,
        size: usize,
        alignment: usize
    ),
    clear pointer as *mut c_void
);
unavailable_output!(
    fe2o3HipMemGetAllocationGranularity(granularity: *mut usize, device_id: c_int),
    clear granularity as usize
);
unavailable_output!(
    fe2o3HipMemCreate(handle: *mut usize, size: usize, device_id: c_int),
    clear handle as usize
);
unavailable_output!(
    fe2o3HipMemGetAccess(access: *mut c_uint, pointer: *mut c_void, device_id: c_int),
    clear access as c_uint
);

macro_rules! unavailable_mutation {
    ($name:ident($($argument:ident : $argument_ty:ty),*)) => {
        #[cfg(not(fe2o3_hip_memory_topology))]
        #[doc = "Fails closed without dereferencing arguments when the HIP ABI is unavailable."]
        #[doc = ""]
        #[doc = "# Safety"]
        #[doc = "The arguments must satisfy the corresponding raw HIP ABI contract."]
        pub unsafe extern "C" fn $name($($argument: $argument_ty),*) -> hipError_t {
            let _ = ($($argument),*);
            HIP_ERROR_NOT_SUPPORTED
        }
    };
}

unavailable_mutation!(fe2o3HipMemPrefetchAsync(
    pointer: *const c_void,
    size: usize,
    device_id: c_int,
    stream: hipStream_t
));
unavailable_mutation!(fe2o3HipMemAdvise(
    pointer: *const c_void,
    size: usize,
    advice: c_uint,
    device_id: c_int
));
unavailable_mutation!(fe2o3HipMemAddressFree(pointer: *mut c_void, size: usize));
unavailable_mutation!(fe2o3HipMemMap(
    pointer: *mut c_void,
    size: usize,
    handle: usize
));
unavailable_mutation!(fe2o3HipMemSetAccess(
    pointer: *mut c_void,
    size: usize,
    device_id: c_int,
    access: c_uint
));
unavailable_mutation!(fe2o3HipMemUnmap(pointer: *mut c_void, size: usize));
unavailable_mutation!(fe2o3HipMemRelease(handle: usize));

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};

    #[test]
    fn stable_records_have_exact_layouts() {
        assert_eq!(size_of::<Fe2o3HipPhysicalDeviceIdentity>(), 48);
        assert_eq!(align_of::<Fe2o3HipPhysicalDeviceIdentity>(), 1);
        assert_eq!(size_of::<Fe2o3HipMemoryCapabilities>(), 16);
        assert_eq!(align_of::<Fe2o3HipMemoryCapabilities>(), 4);
    }

    #[test]
    fn stable_operation_values_are_disjoint() {
        assert_eq!(HIP_CPU_DEVICE_ID, -1);
        assert_eq!(HIP_MEMORY_ADVISE_SET_READ_MOSTLY, 1);
        assert_eq!(HIP_MEMORY_ADVISE_UNSET_COARSE_GRAIN, 8);
        assert_eq!(HIP_VMM_ACCESS_READ, 1);
        assert_eq!(HIP_VMM_ACCESS_READ_WRITE, 2);
    }

    #[cfg(not(fe2o3_hip_memory_topology))]
    #[test]
    fn unavailable_queries_clear_outputs_and_fail_closed() {
        let mut identity = Fe2o3HipPhysicalDeviceIdentity {
            uuid: [0xff; 16],
            pci_bus_id: [1; 32],
        };
        let mut capabilities = Fe2o3HipMemoryCapabilities {
            managed_memory: 1,
            concurrent_managed_access: 1,
            pageable_memory_access: 1,
            virtual_memory_management: 1,
        };
        let mut pointer = core::ptr::dangling_mut::<c_void>();
        let mut handle = usize::MAX;

        // SAFETY: Every output points to a live writable value; fallbacks call no HIP API.
        unsafe {
            assert_eq!(
                fe2o3HipGetPhysicalDeviceIdentity(0, &mut identity),
                HIP_ERROR_NOT_SUPPORTED
            );
            assert_eq!(
                fe2o3HipGetMemoryCapabilities(0, &mut capabilities),
                HIP_ERROR_NOT_SUPPORTED
            );
            assert_eq!(
                fe2o3HipMallocManaged(&mut pointer, 16),
                HIP_ERROR_NOT_SUPPORTED
            );
            assert_eq!(
                fe2o3HipMemCreate(&mut handle, 4096, 0),
                HIP_ERROR_NOT_SUPPORTED
            );
        }

        assert_eq!(identity, Fe2o3HipPhysicalDeviceIdentity::default());
        assert_eq!(capabilities, Fe2o3HipMemoryCapabilities::default());
        assert!(pointer.is_null());
        assert_eq!(handle, 0);
    }
}
