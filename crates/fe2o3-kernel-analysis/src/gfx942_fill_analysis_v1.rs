//! Association of one authenticated analysis owner with the complete fill model.

use crate::{
    AuthenticatedPhysicalMachineAnalysisExecutionV1, Gfx942FillErrorV1, Gfx942FillKernelV1,
    PhysicalMachineAnalysisEvidenceV1, PhysicalMachineBranchKindV1 as Branch,
    PhysicalMachineEffectKindV1 as Effect, PhysicalMachineMemoryAccessV1 as Memory,
};
use fe2o3_hsaco::inspect_and_bind_kernel_descriptors;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt};

/// Exact borrowed analyzer execution and inspected fill model, not a launch certificate.
///
/// The execution policy authenticates the analyzer, not a compiler or GPU run.
/// This check associates the complete static trace/effects with the model's exact
/// instruction bytes. ISA interpretation and native dispatch/storage/completion
/// remain separate premises; source-to-machine refinement is not established.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::CheckedGfx942FillAnalysisV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<CheckedGfx942FillAnalysisV1<'static>>();
/// ```
///
/// ```compile_fail,E0505
/// use fe2o3_kernel_analysis::{AuthenticatedPhysicalMachineAnalysisExecutionV1,
///     check_gfx942_fill_analysis_v1};
/// fn release_owner(execution: AuthenticatedPhysicalMachineAnalysisExecutionV1) {
///     let checked = check_gfx942_fill_analysis_v1(&execution, "fill_write_only").unwrap();
///     drop(execution);
///     let _ = checked.kernel();
/// }
/// ```
#[derive(Debug)]
#[must_use]
pub struct CheckedGfx942FillAnalysisV1<'a> {
    execution: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    kernel: Gfx942FillKernelV1<'a>,
}

impl<'a> CheckedGfx942FillAnalysisV1<'a> {
    pub const fn execution(&self) -> &'a AuthenticatedPhysicalMachineAnalysisExecutionV1 {
        self.execution
    }

    pub const fn kernel(&self) -> &Gfx942FillKernelV1<'a> {
        &self.kernel
    }

    pub fn entry_symbol(&self) -> &str {
        self.execution.request().entries()[0].symbol()
    }

    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    pub const fn grants_launch_authority(&self) -> bool {
        false
    }
}

/// Borrows the authenticated owner; no detached record or caller digest can substitute.
pub fn check_gfx942_fill_analysis_v1<'a>(
    execution: &'a AuthenticatedPhysicalMachineAnalysisExecutionV1,
    symbol: &str,
) -> Result<CheckedGfx942FillAnalysisV1<'a>, Gfx942FillAnalysisErrorV1> {
    use Gfx942FillAnalysisErrorV1 as E;
    // Main's canonical request binds the exact payload; its authenticated,
    // request-bound effects carry the decoded machine target.
    if execution.analysis().effects().target()
        != crate::PhysicalMachineTargetV1::Gfx942XnackMinusCov6
    {
        return Err(E::UnsupportedTarget);
    }
    let [requested] = execution.request().entries() else {
        return Err(E::Entry);
    };
    if requested.symbol() != symbol {
        return Err(E::Entry);
    }
    let payload = execution.request().exact_payload_bytes();
    let inspected = inspect_and_bind_kernel_descriptors(payload).map_err(|_| E::Descriptor)?;
    let mut matches = inspected
        .inspection()
        .kernels()
        .iter()
        .enumerate()
        .filter(|(_, kernel)| kernel.name() == symbol);
    let index = matches.next().ok_or(E::Entry)?.0;
    if matches.next().is_some() {
        return Err(E::Entry);
    }
    let kernel = Gfx942FillKernelV1::inspect(payload, index).map_err(E::Kernel)?;
    // Authenticated execution already decoded the bundle against this exact request.
    // Check the closed profile without re-decoding or copying that retained evidence.
    check_profile(execution.analysis(), symbol, &kernel)?;
    Ok(CheckedGfx942FillAnalysisV1 { execution, kernel })
}

fn check_profile(
    analysis: &PhysicalMachineAnalysisEvidenceV1,
    symbol: &str,
    kernel: &Gfx942FillKernelV1<'_>,
) -> Result<(), Gfx942FillAnalysisErrorV1> {
    use Gfx942FillAnalysisErrorV1 as E;
    let binding = kernel.binding();
    let start = binding.entry_file_offset();
    let [entry] = analysis.effects().entry_points() else {
        return Err(E::Entry);
    };
    let [function] = analysis.effects().functions() else {
        return Err(E::Function);
    };
    if entry.symbol() != symbol
        || entry.code_offset() != start
        || entry.code_size() != binding.entry_size()
    {
        return Err(E::Entry);
    }
    if function.symbol() != symbol
        || function.code_offset() != start
        || function.code_size() != binding.entry_size()
        || !function.direct_callees().is_empty()
    {
        return Err(E::Function);
    }
    let descriptor_start =
        usize::try_from(binding.descriptor_file_offset()).map_err(|_| E::Descriptor)?;
    let descriptor_end = descriptor_start.checked_add(64).ok_or(E::Descriptor)?;
    let descriptor = kernel
        .code_object()
        .get(descriptor_start..descriptor_end)
        .ok_or(E::Descriptor)?;
    if entry.descriptor_identity().as_bytes() != <[u8; 32]>::from(Sha256::digest(descriptor)) {
        return Err(E::Descriptor);
    }
    let expected_effects = [
        (0, Effect::GlobalAddress, 8),
        (0, Effect::GlobalRead, 16),
        (56, Effect::GlobalAddress, 8),
        (56, Effect::GlobalWrite, 4),
        (64, Effect::Return, 0),
    ];
    let effects = analysis.effects().effects();
    if effects.len() != expected_effects.len()
        || effects
            .iter()
            .zip(expected_effects)
            .any(|(effect, expected)| {
                effect.entry_symbol() != symbol
                    || effect.function_symbol() != symbol
                    || effect.instruction_offset().checked_sub(start) != Some(expected.0)
                    || effect.kind() != expected.1
                    || effect.byte_width() != expected.2
            })
    {
        return Err(E::Effects);
    }
    let expected_blocks: [(u64, u32, &[u32]); 3] = [(0, 9, &[1, 2]), (40, 4, &[2]), (64, 1, &[])];
    let blocks = analysis.trace().blocks();
    if blocks.len() != expected_blocks.len()
        || blocks.iter().zip(expected_blocks).enumerate().any(
            |(ordinal, (block, (offset, count, successors)))| {
                block.function_symbol() != symbol
                    || block.ordinal() != ordinal as u32
                    || block.first_instruction_offset().checked_sub(start) != Some(offset)
                    || block.instruction_count() != count
                    || block.successors() != successors
            },
        )
    {
        return Err(E::ControlFlow);
    }
    let offsets = [0, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 56, 64];
    let instructions = analysis.trace().instructions();
    if instructions.len() != offsets.len() {
        return Err(E::Instructions);
    }
    for (instruction, offset) in instructions.iter().zip(offsets) {
        let (branch, target) = match offset {
            36 => (Branch::ConditionalDirect, start.checked_add(64)),
            64 => (Branch::Return, None),
            _ => (Branch::None, None),
        };
        let memory = match offset {
            0 => Memory::Read { byte_width: 16 },
            56 => Memory::Write { byte_width: 4 },
            _ => Memory::None,
        };
        if instruction.function_symbol() != symbol
            || instruction.instruction_offset().checked_sub(start) != Some(offset)
            || instruction.branch_kind() != branch
            || instruction.branch_target() != target
            || instruction.memory_access() != memory
            // LLVM's MC Barrier flag marks END's control-flow termination here.
            || instruction.flags().is_barrier() != (offset == 64)
            || instruction.flags().is_predicable()
            || instruction.flags().may_trap()
        {
            return Err(E::Instructions);
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Gfx942FillAnalysisErrorV1 {
    UnsupportedTarget,
    Entry,
    Descriptor,
    Kernel(Gfx942FillErrorV1),
    Function,
    Effects,
    ControlFlow,
    Instructions,
}

impl fmt::Display for Gfx942FillAnalysisErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gfx942 fill analysis association rejected: {self:?}")
    }
}
impl Error for Gfx942FillAnalysisErrorV1 {}

#[cfg(test)]
mod tests;
