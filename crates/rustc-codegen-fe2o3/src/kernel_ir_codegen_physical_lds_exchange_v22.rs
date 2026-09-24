//! Consuming exact canonical emission into bounded INERT compiler text.
use super::*;
pub(crate) fn retain_verified_physical_lds_exchange_compiler_module_text_v22(
    canonical: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV22,
    emission: dialect_amdgcn::Gfx942PhysicalLdsExchangeCanonicalEmissionV22,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    let module = canonical.module();
    if canonical.identity() != emission.canonical_identity()
        || fe2o3_kernel_ir::gfx942_physical_lds_exchange_declaration_v22(canonical)
            .is_none_or(|declaration| declaration.lds_frame != *emission.lds_frame())
        || module.functions.len() != 1
        || module.kernels.len() != 1
        || module.kernels[0].entry != module.functions[0].id
        || module.kernels[0].id.as_str() != module.functions[0].id.as_str()
    {
        return Err(CompilerModuleConstructionError::PhysicalLdsExchangeIdentityMismatchV22);
    }
    enforce_compiler_module_bounds(module)?;
    enforce_llvm_text_bound(emission.llvm_ir())?;
    let symbols = compiler_module_symbol_closure_v1(module);
    Ok(InertCompilerModuleTextV1 {
        llvm_ir: emission.into_llvm_ir(),
        kernel_entries: symbols.kernel_entries,
        device_definitions: symbols.device_definitions,
        internal_helpers: symbols.internal_helpers,
        device_ffi_exports: symbols.device_ffi_exports,
        external_declarations: symbols.external_declarations,
        descriptor_source_identity: None,
    })
}

/// Test-only byte comparison of the existing production binder's exact inert
/// relation. No decoded handoff or descriptor becomes a checked source owner.
#[cfg(test)]
pub(crate) fn exact_inert_descriptor_extension_v22(
    canonical: &str,
    worker: &str,
    descriptor: &fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
) -> bool {
    if canonical.len() > dialect_amdgcn::GFX942_PHYSICAL_LDS_EXCHANGE_LLVM_BYTES_V22
        || descriptor.canonical_bytes().len() > 64 * 1024
        || worker.len() > 1024 * 1024
    {
        return false;
    }
    let mut expected = canonical.to_owned();
    super::append_descriptor_module_assembly(&mut expected, descriptor.canonical_bytes());
    expected == worker
}
