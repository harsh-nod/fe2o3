//! Descriptor-derived locations for the scratch-free gfx942 entry profile.
//!
//! Setup order follows LLVM's AMDHSA Initial Kernel Execution State ABI:
//! https://llvm.org/docs/AMDGPUUsage.html#initial-kernel-execution-state
//! This is descriptive layout, not a register-value or execution proof.

use super::{AmdhsaKernelDescriptor, InspectedKernelBindings};
use core::fmt;

/// Register locations derived from one inspected gfx942 descriptor.
///
/// Pointer pairs are low word first. Only requested inputs have locations.
/// The profile excludes private inputs, scratch, preload and surplus user SGPRs.
/// It does not establish pointer values, memory validity, launch geometry, EXEC,
/// compiler refinement, or load/launch authority. A composing checker must bind
/// the originating inspection to its exact retained code object.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Gfx942InitialRegisterLayoutV1 {
    user_pairs: [Option<[u8; 2]>; 4],
    workgroup_ids: [Option<u8>; 3],
    workgroup_info: Option<u8>,
    workitem_ids: [Option<u8>; 3],
    user_sgpr_count: u8,
    initialized_sgpr_count: u8,
    initialized_vgpr_count: u8,
}

impl Gfx942InitialRegisterLayoutV1 {
    pub const fn dispatch_pointer_sgprs(self) -> Option<[u8; 2]> {
        self.user_pairs[0]
    }

    pub const fn queue_pointer_sgprs(self) -> Option<[u8; 2]> {
        self.user_pairs[1]
    }

    pub const fn kernarg_pointer_sgprs(self) -> Option<[u8; 2]> {
        self.user_pairs[2]
    }

    pub const fn dispatch_id_sgprs(self) -> Option<[u8; 2]> {
        self.user_pairs[3]
    }

    /// X, Y, Z respectively; workgroup dimensions can be independently absent.
    pub const fn workgroup_id_sgprs(self) -> [Option<u8>; 3] {
        self.workgroup_ids
    }

    pub const fn workgroup_info_sgpr(self) -> Option<u8> {
        self.workgroup_info
    }

    /// Unpacked X, Y, Z respectively. X is always initialized on gfx942.
    pub const fn workitem_id_vgprs(self) -> [Option<u8>; 3] {
        self.workitem_ids
    }

    pub const fn user_sgpr_count(self) -> u8 {
        self.user_sgpr_count
    }

    /// Dense initialized prefix, not the kernel's complete register allocation.
    pub const fn initialized_sgpr_count(self) -> u8 {
        self.initialized_sgpr_count
    }

    pub const fn initialized_vgpr_count(self) -> u8 {
        self.initialized_vgpr_count
    }
}

/// Unsupported layout profiles are not necessarily malformed AMDHSA objects.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942InitialRegisterLayoutErrorV1 {
    UnsupportedProcessor,
    KernelIndexOutOfBounds,
    PrivateInputsOrScratch,
    KernargPreload,
    WavefrontSize,
    UserSgprCount { expected: u8, actual: u8 },
    WorkitemIdMode,
    InsufficientSgprCapacity,
    InsufficientVgprCapacity,
}

impl fmt::Display for Gfx942InitialRegisterLayoutErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "unsupported gfx942 initial-register layout: {self:?}"
        )
    }
}

impl std::error::Error for Gfx942InitialRegisterLayoutErrorV1 {}

impl InspectedKernelBindings {
    /// Derives locations from this inspection's target and selected descriptor.
    ///
    /// A greater-than-required USER_SGPR_COUNT is legal AMDHSA but unsupported
    /// here: accepting it as a dense initialized prefix would place system
    /// registers incorrectly. Existing general descriptor inspection is unchanged.
    pub fn gfx942_initial_register_layout_v1(
        &self,
        kernel_index: usize,
    ) -> Result<Gfx942InitialRegisterLayoutV1, Gfx942InitialRegisterLayoutErrorV1> {
        if self.inspection().target().processor() != "gfx942" {
            return Err(Gfx942InitialRegisterLayoutErrorV1::UnsupportedProcessor);
        }
        let binding = self
            .bindings()
            .get(kernel_index)
            .ok_or(Gfx942InitialRegisterLayoutErrorV1::KernelIndexOutOfBounds)?;
        derive(binding.descriptor())
    }
}

fn derive(
    descriptor: AmdhsaKernelDescriptor,
) -> Result<Gfx942InitialRegisterLayoutV1, Gfx942InitialRegisterLayoutErrorV1> {
    use Gfx942InitialRegisterLayoutErrorV1 as E;
    let properties = descriptor.kernel_code_properties();
    if properties & ((1 << 0) | (1 << 5) | (1 << 6)) != 0
        || descriptor.private_segment_enabled()
        || descriptor.private_segment_fixed_size() != 0
        || descriptor.uses_dynamic_stack()
    {
        return Err(E::PrivateInputsOrScratch);
    }
    if descriptor.kernarg_preload() != 0 {
        return Err(E::KernargPreload);
    }
    if descriptor.wavefront_size() != 64 {
        return Err(E::WavefrontSize);
    }
    let mut user_pairs = [None; 4];
    let mut next = 0;
    for (index, pair) in user_pairs.iter_mut().enumerate() {
        if properties & (1 << (index + 1)) != 0 {
            *pair = Some([next, next + 1]);
            next += 2;
        }
    }
    let user_sgpr_count = next;
    let rsrc2 = descriptor.compute_pgm_rsrc2();
    let actual = ((rsrc2 >> 1) & 0x1f) as u8;
    if actual != user_sgpr_count {
        return Err(E::UserSgprCount {
            expected: user_sgpr_count,
            actual,
        });
    }
    let mut workgroup_ids = [None; 3];
    for (index, register) in workgroup_ids.iter_mut().enumerate() {
        if rsrc2 & (1 << (index + 7)) != 0 {
            *register = Some(next);
            next += 1;
        }
    }
    let workgroup_info = if rsrc2 & (1 << 10) != 0 {
        let register = next;
        next += 1;
        Some(register)
    } else {
        None
    };
    // gfx942 uses architected flat scratch, not a legacy scratch-offset SGPR.
    let workitem_mode = ((rsrc2 >> 11) & 3) as u8;
    if workitem_mode == 3 {
        return Err(E::WorkitemIdMode);
    }
    let initialized_vgpr_count = workitem_mode + 1;
    let mut workitem_ids = [None; 3];
    for index in 0..initialized_vgpr_count {
        workitem_ids[usize::from(index)] = Some(index);
    }
    let rsrc1 = descriptor.compute_pgm_rsrc1();
    let sgpr_capacity = (((rsrc1 >> 6) & 0xf) + 1) * 8;
    let vgpr_capacity = ((rsrc1 & 0x3f) + 1) * 8;
    if u32::from(next) > sgpr_capacity {
        return Err(E::InsufficientSgprCapacity);
    }
    if u32::from(initialized_vgpr_count) > vgpr_capacity {
        return Err(E::InsufficientVgprCapacity);
    }
    Ok(Gfx942InitialRegisterLayoutV1 {
        user_pairs,
        workgroup_ids,
        workgroup_info,
        workitem_ids,
        user_sgpr_count,
        initialized_sgpr_count: next,
        initialized_vgpr_count,
    })
}

#[cfg(test)]
mod tests;
