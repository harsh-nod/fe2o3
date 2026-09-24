//! Consuming exact canonical emission into bounded INERT compiler text.
use super::*;
pub(crate) fn retain_verified_physical_entry_compiler_module_text_v20(
    canonical: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV20,
    emission: dialect_amdgcn::Gfx942PhysicalEntryCanonicalEmissionV20,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    let module = canonical.module();
    if canonical.identity() != emission.canonical_identity()
        || module.functions.len() != 1
        || module.kernels.len() != 1
        || module.kernels[0].entry != module.functions[0].id
        || module.kernels[0].id.as_str() != module.functions[0].id.as_str()
    {
        return Err(CompilerModuleConstructionError::PhysicalEntryIdentityMismatchV20);
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
