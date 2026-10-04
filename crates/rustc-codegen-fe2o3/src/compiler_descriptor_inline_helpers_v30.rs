//! Exact source-owned helper ISA requirements at the runtime-descriptor boundary.
//! This does not erase canonical capabilities or admit general extension strings.
//! The fixed runtime descriptor carries execution requirements, not source custody.
use super::{CompilerDescriptorError, Module, TargetCapability};
use crate::production_pipeline::RetainedProductionTargetV30;
use fe2o3_kernel_ir::{
    AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE, AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME,
    AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE,
    AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME, AssemblyOption, Function, FunctionRole,
    InlineAssemblyTarget, OperationKind, ScalarType, Type, WaveWidth,
    gfx942_inline_assembly_instruction_v1,
};
use fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1;
use std::collections::BTreeSet;

type Error = CompilerDescriptorError;

/// Borrowed only after source/ranked/formal replay and exact fixed-output custody.
/// Private fields prevent raw module/capability callers from making admission.
/// Neither this value nor the resulting inert descriptor grants launch authority.
pub(super) struct InlineHelperDescriptorAdmissionV30<'a> {
    module: &'a Module,
    // Keep the actual replayed owner alive throughout descriptor construction.
    _formal: &'a ProductionFormalMemoryOwnerV1,
    _optimized: &'a RetainedProductionTargetV30,
}

impl InlineHelperDescriptorAdmissionV30<'_> {
    pub(super) fn admits(&self, module: &Module, capability: &TargetCapability) -> bool {
        std::ptr::eq(self.module, module) && exact_inline(capability)
    }
}

fn mismatch(detail: &'static str) -> Error {
    Error::ProductionDescriptorMismatch(detail)
}
fn exact_inline(capability: &TargetCapability) -> bool {
    matches!(capability, TargetCapability::Extension { namespace, name }
        if namespace == AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAMESPACE
            && name == AMDGPU_GFX942_INLINE_ASSEMBLY_CAPABILITY_NAME)
}
fn declares_inline(capabilities: &BTreeSet<TargetCapability>) -> bool {
    capabilities.iter().any(exact_inline)
}
fn contains_inline(function: &Function) -> bool {
    function.body.as_ref().is_some_and(|body| {
        body.blocks
            .iter()
            .flat_map(|block| &block.operations)
            .any(|operation| matches!(operation.kind, OperationKind::InlineAssembly(_)))
    })
}
fn needs_inline(module: &Module) -> bool {
    declares_inline(&module.required_capabilities)
        || module
            .kernels
            .iter()
            .any(|kernel| declares_inline(&kernel.required_capabilities))
        || module.functions.iter().any(|function| {
            declares_inline(&function.required_capabilities) || contains_inline(function)
        })
}

// Linear bounded-owner scans only: no temporary sets, cloned graphs or alternate
// semantics. Optimizer CFG/SSA changes are admitted by retained fixed-policy
// custody, not by comparing the optimized body with pre-optimization blocks.
// This does not upgrade structural optimizer evidence to semantic preservation.
fn validate_module_join(source: &Module, target: &Module, device: &str) -> Result<(), Error> {
    if device != "gfx942:xnack-"
        || source.id != target.id
        || source.functions.len() != target.functions.len()
        || source.kernels.is_empty()
        || source.kernels.len() != target.kernels.len()
        || declares_inline(&source.required_capabilities)
            != declares_inline(&target.required_capabilities)
    {
        return Err(mismatch("inline helper exact source/target module"));
    }
    target_scope(&target.required_capabilities, true)?;
    for (source, target) in source.kernels.iter().zip(&target.kernels) {
        if source.id != target.id
            || source.entry != target.entry
            || source.domain != target.domain
            || source.workgroup_size != target.workgroup_size
            || declares_inline(&source.required_capabilities)
                != declares_inline(&target.required_capabilities)
        {
            return Err(mismatch("inline helper source launch/capability roster"));
        }
        target_scope(&target.required_capabilities, true)?;
    }
    let mut saw_source_helper = false;
    let mut saw_target_helper = false;
    for (source, target) in source.functions.iter().zip(&target.functions) {
        if source.id != target.id
            || source.role != target.role
            || source.signature != target.signature
            || declares_inline(&source.required_capabilities)
                != declares_inline(&target.required_capabilities)
        {
            return Err(mismatch(
                "inline helper source function/ABI/capability roster",
            ));
        }
        target_scope(
            &target.required_capabilities,
            target.role == FunctionRole::KernelEntry,
        )?;
        saw_source_helper |= validate_helper_profile(source)?;
        saw_target_helper |= validate_helper_profile(target)?;
    }
    if !saw_source_helper || !saw_target_helper {
        return Err(mismatch(
            "inline helper capability has no retained helper instruction",
        ));
    }
    Ok(())
}

fn validate_helper_profile(function: &Function) -> Result<bool, Error> {
    let mut saw_helper = false;
    for operation in function
        .body
        .iter()
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        let OperationKind::InlineAssembly(assembly) = &operation.kind else {
            continue;
        };
        if function.role != FunctionRole::InternalHelper {
            return Err(mismatch("inline helper requires internal helper ownership"));
        }
        // The complete source replay and retained fixed optimizer already check
        // operand/result constraints and source occurrence custody. This only
        // narrows descriptor permission to the six source-authored NoMemory ops.
        if assembly.target != InlineAssemblyTarget::AmdGpuGfx942
            || !assembly.source.is_complete()
            || !gfx942_inline_assembly_instruction_v1(&assembly.mnemonic)
                .is_some_and(|instruction| instruction.has_source_macro())
            || assembly.options.len() != 1
            || !assembly.options.contains(&AssemblyOption::NoMemory)
            || !assembly.declared_effects.is_empty()
            || !matches!(operation.results.as_slice(), [result] if result.ty == Type::Scalar(ScalarType::U32))
        {
            return Err(mismatch("inline helper exact six-u32 NoMemory profile"));
        }
        saw_helper = true;
    }
    Ok(saw_helper)
}

fn target_scope(capabilities: &BTreeSet<TargetCapability>, required: bool) -> Result<(), Error> {
    let mut target = false;
    let mut wave = false;
    for capability in capabilities {
        match capability {
            TargetCapability::Extension { namespace, name }
                if namespace == AMDGPU_EXACT_TARGET_CAPABILITY_NAMESPACE =>
            {
                if name != AMDGPU_GFX942_XNACK_MINUS_TARGET_CAPABILITY_NAME {
                    return Err(mismatch("inline helper exact gfx942 target capability"));
                }
                target = true;
            }
            TargetCapability::WaveWidth(width) => {
                if *width != WaveWidth::Wave64 {
                    return Err(mismatch("inline helper exact Wave64 capability"));
                }
                wave = true;
            }
            _ => {}
        }
    }
    if required && !(target && wave) {
        return Err(mismatch("inline helper missing exact target/Wave64 scope"));
    }
    Ok(())
}

/// Replaces, rather than bypasses or duplicates, the existing formal replay at
/// the normal descriptor constructor. Ordinary modules retain the same replay.
/// New helper admission additionally requires the complete mandatory check roster.
pub(super) fn verify_and_admit<'a>(
    formal: &'a ProductionFormalMemoryOwnerV1,
    optimized: &'a RetainedProductionTargetV30,
    module: &'a Module,
    device: &str,
) -> Result<Option<InlineHelperDescriptorAdmissionV30<'a>>, Error> {
    formal
        .verify_equivalence()
        .map_err(Error::ProductionFormalMemory)?;
    if !optimized.joins(formal, module, device) {
        return Err(mismatch(
            "inline helper retained fixed target/source custody",
        ));
    }
    let source = formal.semantic_kir();
    if !needs_inline(source.module()) && !needs_inline(module) {
        return Ok(None);
    }
    if !source.retains_mandatory_generic_checks()
        || source.mir_pliron_translation_validations().len() != source.module().kernels.len()
        || source
            .mir_pliron_translation_validations()
            .zip(&source.module().kernels)
            .any(|((name, _), kernel)| name != kernel.id.as_str())
    {
        return Err(mismatch(
            "inline helper complete mandatory ranked check roster",
        ));
    }
    validate_module_join(source.module(), module, device)?;
    Ok(Some(InlineHelperDescriptorAdmissionV30 {
        module,
        _formal: formal,
        _optimized: optimized,
    }))
}

#[cfg(test)]
#[path = "compiler_descriptor_inline_helpers_v30_tests.rs"]
mod tests;
