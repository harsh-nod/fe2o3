use super::{
    Gfx942FillKernelV1, Gfx942FillWaveExecutionV1, Gfx942FillWaveStateV1, decode_kernarg,
    execute_wave, pair,
};
use std::{error::Error, fmt};

include!("dispatch_body.rs");

#[derive(Clone, Copy, Debug)]
struct DispatchInput {
    kernarg: [u8; 16],
    kernarg_address: u64,
    kernarg_bytes: u64,
    output_base: u64,
    output_bytes: u64,
    grid: [u32; 3],
    workgroup: [u32; 3],
}

/// Complete full-wave dispatch projection over one retained inspected code object.
///
/// Kernarg bytes are a stable snapshot of the explicit prefix, not an authenticated
/// native read. Bounds cover the complete descriptor-required storage, but do not
/// establish allocation ownership or implicit-tail contents/initialization.
/// Entry construction models the stated descriptor/AMDHSA contract; it does not
/// observe actual GPU registers.
/// Ordinary wave64 mode, disabled VSKIP/GPR indexing, and 64-bit global addressing
/// remain premises; initializing the projected registers does not establish them.
/// No ISA interpretation, scheduling, visibility or launch authority is granted.
#[derive(Debug)]
#[must_use]
pub struct Gfx942FillDispatchV1<'kernel, 'code> {
    kernel: &'kernel Gfx942FillKernelV1<'code>,
    input: DispatchInput,
}

impl<'code> Gfx942FillKernelV1<'code> {
    /// Checks exact bounds and full64 geometry without rounding or iterating the grid.
    pub fn check_dispatch<'kernel>(
        &'kernel self,
        kernarg: [u8; 16],
        kernarg_address: u64,
        output_base: u64,
        output_bytes: u64,
        grid: [u32; 3],
        workgroup: [u32; 3],
    ) -> Result<Gfx942FillDispatchV1<'kernel, 'code>, Gfx942FillDispatchErrorV1> {
        let input = DispatchInput {
            kernarg,
            kernarg_address,
            kernarg_bytes: self.kernarg_storage_bytes(),
            output_base,
            output_bytes,
            grid,
            workgroup,
        };
        if !valid_dispatch(&input) {
            return Err(Gfx942FillDispatchErrorV1::Input);
        }
        Ok(Gfx942FillDispatchV1 {
            kernel: self,
            input,
        })
    }
}

impl<'code> Gfx942FillDispatchV1<'_, 'code> {
    pub const fn kernel(&self) -> &Gfx942FillKernelV1<'code> {
        self.kernel
    }
    pub const fn kernarg(&self) -> &[u8; 16] {
        &self.input.kernarg
    }
    pub const fn workgroup_count(&self) -> u32 {
        self.input.grid[0] / 64
    }

    /// Initializes the descriptor-selected registers, then executes the shared wave body.
    /// `seed` supplies arbitrary *other* registers, not an actual hardware entry snapshot.
    /// Repeating a group models it again; this API does not submit or schedule work.
    pub fn execute_group(
        &self,
        group: u32,
        seed: Gfx942FillWaveStateV1,
    ) -> Result<Gfx942FillWaveExecutionV1, Gfx942FillDispatchErrorV1> {
        execute_group(&self.input, group, seed).ok_or(Gfx942FillDispatchErrorV1::Group)
    }

    /// Observes the actual sparse stores of the unique group that can write this byte.
    /// Bounded by one 64-lane execution, independent of output length or grid size.
    pub fn byte_after(&self, address: u64, before: u8) -> u8 {
        dispatch_byte_after(&self.input, address, before)
    }
}

fn valid_dispatch(input: &DispatchInput) -> bool {
    gfx942_fill_dispatch_valid_body_v1!(input)
}
fn initialize_entry(
    mut state: Gfx942FillWaveStateV1,
    address: u64,
    group: u32,
) -> Gfx942FillWaveStateV1 {
    gfx942_fill_entry_body_v1!(exec_expr, state, address, group, lane, [])
}
fn execute_group(
    input: &DispatchInput,
    group: u32,
    seed: Gfx942FillWaveStateV1,
) -> Option<Gfx942FillWaveExecutionV1> {
    gfx942_fill_group_body_v1!(exec_expr, input, group, seed, entry, [])
}
fn dispatch_byte_after(input: &DispatchInput, address: u64, before: u8) -> u8 {
    gfx942_fill_dispatch_byte_body_v1!(
        exec_expr,
        input,
        address,
        before,
        index,
        group,
        entry,
        execution,
        [],
        []
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942FillDispatchErrorV1 {
    Input,
    Group,
}
impl fmt::Display for Gfx942FillDispatchErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gfx942 fill dispatch projection rejected: {self:?}")
    }
}
impl Error for Gfx942FillDispatchErrorV1 {}

#[cfg(test)]
mod tests;
