//! Private x86_64 little-endian ROCr-compatible debugger rendezvous layouts.
//!
//! This leaf does not enable a runtime, install a trap handler, advertise an
//! active ABI version, own code, or confer pointer authority. The consuming
//! no-queue owner derives every address for its private syscall adapter.
//! Source revision pins are retained in the preparation delivery provenance.
//! These are independently written ABI descriptions, not ROCr implementation code.

use core::mem::{align_of, offset_of, size_of};

/// A prospective version, not a statement about any live runtime.
pub(super) const REQUIRED_ROCR_DEBUG_VERSION_V11: i32 = 11;
pub(super) const RT_CONSISTENT_V1: i32 = 0;
pub(super) const RT_ADD_V1: i32 = 1;
pub(super) const RT_DELETE_V1: i32 = 2;

/// The host ELF rendezvous, not glibc's extended loader-namespace structure.
/// Explicit reserved words occupy the C ABI's padding; initialize them to zero.
#[repr(C)]
pub(super) struct RDebugAbiV11 {
    pub(super) version: i32,
    pub(super) reserved0: u32,
    pub(super) map: u64,
    pub(super) breakpoint: u64,
    pub(super) state: i32,
    pub(super) reserved1: u32,
    pub(super) loader_base: u64,
}

/// ELF link_map's five debugger-visible words. Address values remain inert;
/// there is deliberately no constructor accepting caller-provided pointers.
#[repr(C)]
pub(super) struct LinkMapAbiV1 {
    pub(super) load_bias: u64,
    pub(super) name: u64,
    pub(super) dynamic: u64,
    pub(super) next: u64,
    pub(super) previous: u64,
}

/// SET_TRAP_HANDLER input shape only; no public address constructor.
#[repr(C)]
pub(in super::super) struct SetTrapHandlerAbiV1 {
    pub(in super::super) trap_base: u64,
    pub(in super::super) trap_memory: u64,
    pub(in super::super) gpu_id: u32,
    pub(in super::super) reserved: u32,
}

/// Private debug-metadata runtime profile; leaves the public plain ABI unchanged.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub(in super::super) struct RuntimeDebugEnableAbiV1 {
    pub(in super::super) r_debug: u64,
    pub(in super::super) mode_mask: u32,
    pub(in super::super) capabilities_mask: u32,
}

#[cfg(test)]
pub(super) const AQL_READ_DISPATCH_ID_V1: usize = 0x80;
#[cfg(test)]
pub(super) const AQL_COMPUTE_TMPRING_SIZE_V1: usize = 0x8c;
#[cfg(test)]
pub(super) const AQL_SCRATCH_BACKING_ADDRESS_V1: usize = 0xa0;

const _: () = {
    assert!(size_of::<usize>() == 8);
    assert!(size_of::<RDebugAbiV11>() == 40);
    assert!(align_of::<RDebugAbiV11>() == 8);
    assert!(offset_of!(RDebugAbiV11, version) == 0);
    assert!(offset_of!(RDebugAbiV11, map) == 8);
    assert!(offset_of!(RDebugAbiV11, breakpoint) == 16);
    assert!(offset_of!(RDebugAbiV11, state) == 24);
    assert!(offset_of!(RDebugAbiV11, loader_base) == 32);
    assert!(size_of::<LinkMapAbiV1>() == 40);
    assert!(align_of::<LinkMapAbiV1>() == 8);
    assert!(offset_of!(LinkMapAbiV1, load_bias) == 0);
    assert!(offset_of!(LinkMapAbiV1, name) == 8);
    assert!(offset_of!(LinkMapAbiV1, dynamic) == 16);
    assert!(offset_of!(LinkMapAbiV1, next) == 24);
    assert!(offset_of!(LinkMapAbiV1, previous) == 32);
};

const _: () = {
    assert!(size_of::<RuntimeDebugEnableAbiV1>() == 16);
    assert!(align_of::<RuntimeDebugEnableAbiV1>() == 8);
    assert!(offset_of!(RuntimeDebugEnableAbiV1, r_debug) == 0);
    assert!(offset_of!(RuntimeDebugEnableAbiV1, mode_mask) == 8);
    assert!(offset_of!(RuntimeDebugEnableAbiV1, capabilities_mask) == 12);
    assert!(size_of::<SetTrapHandlerAbiV1>() == 24);
    assert!(align_of::<SetTrapHandlerAbiV1>() == 8);
    assert!(offset_of!(SetTrapHandlerAbiV1, trap_base) == 0);
    assert!(offset_of!(SetTrapHandlerAbiV1, trap_memory) == 8);
    assert!(offset_of!(SetTrapHandlerAbiV1, gpu_id) == 16);
    assert!(offset_of!(SetTrapHandlerAbiV1, reserved) == 20);
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_metadata_layout_and_states_are_exact() {
        assert_eq!(size_of::<RDebugAbiV11>(), 40);
        assert_eq!(size_of::<LinkMapAbiV1>(), 40);
        assert_eq!([RT_CONSISTENT_V1, RT_ADD_V1, RT_DELETE_V1], [0, 1, 2]);
        assert_eq!(REQUIRED_ROCR_DEBUG_VERSION_V11, 11);
    }

    #[test]
    fn trap_registration_wire_has_no_size_or_agent_pointer() {
        assert_eq!(size_of::<SetTrapHandlerAbiV1>(), 24);
        assert_eq!(offset_of!(SetTrapHandlerAbiV1, gpu_id), 16);
        assert_eq!(offset_of!(SetTrapHandlerAbiV1, reserved), 20);
    }

    #[test]
    fn existing_scratch_free_control_page_supplies_the_dbgapi_fields() {
        #[repr(align(8))]
        struct Aligned([u8; 4096]);
        let mut page = Aligned([0xff; 4096]);
        crate::queue::submit::initialize_amd_aql_control(&mut page.0).unwrap();
        assert_eq!(
            crate::queue_resources::AMD_AQL_READ_DISPATCH_ID_OFFSET_V1,
            AQL_READ_DISPATCH_ID_V1
        );
        assert_eq!(AQL_COMPUTE_TMPRING_SIZE_V1 - AQL_READ_DISPATCH_ID_V1, 12);
        assert_eq!(AQL_SCRATCH_BACKING_ADDRESS_V1 - AQL_READ_DISPATCH_ID_V1, 32);
        assert_eq!(&page.0[AQL_COMPUTE_TMPRING_SIZE_V1..0x90], &[0; 4]);
        assert_eq!(&page.0[AQL_SCRATCH_BACKING_ADDRESS_V1..0xa8], &[0; 8]);
        assert_eq!(&page.0[0x88..0x8c], &0x80_u32.to_le_bytes());
    }
}
