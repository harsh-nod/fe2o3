//! Inert compiler text demotion from the exact typed composition emitter only.
use super::*;

pub(crate) fn retain_verified_ordered_composition_compiler_module_text_v1(
    owner: &fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1,
    emission: dialect_amdgcn::OrderedProgramCompositionCanonicalEmissionV1,
) -> Result<InertCompilerModuleTextV1, CompilerModuleConstructionError> {
    let canonical = owner.canonical();
    let module = canonical.module();
    if canonical.identity() != emission.canonical_identity() {
        return Err(CompilerModuleConstructionError::OrderedCompositionIdentityMismatchV1);
    }
    // Structural owner retains one actual kernel plus all (including scalar-only)
    // helper definitions. Never truncate this to a one-function physical profile.
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

/// Exact existing descriptor serialization relation; test-only and inert.
#[cfg(test)]
pub(crate) fn exact_ordered_composition_descriptor_extension_v1(
    canonical: &str,
    worker: &str,
    descriptor: &fe2o3_compiler_ffi::CompilerDescriptorSourceV1,
) -> bool {
    if canonical.len() > 4 * 1024 * 1024
        || worker.len() > 8 * 1024 * 1024
        || descriptor.canonical_bytes().len() > 64 * 1024
    {
        return false;
    }
    let mut expected = canonical.to_owned();
    super::append_descriptor_module_assembly(&mut expected, descriptor.canonical_bytes());
    expected == worker
}
#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{BasicBlock, BlockId, Function, Signature, ValueId};
    // Actual owner/emission controls live in the normal-source qualifier. This
    // independent symbol control does not claim source ownership.
    #[test]
    fn composition_symbol_closure_preserves_internal_helpers() {
        let mut module = Module::new("inert-symbol-control");
        let mut block = BasicBlock::new(BlockId(0));
        block.terminator = Some(Terminator::Return {
            values: vec![ValueId(0)],
        });
        module.functions.push(Function::internal_helper(
            "actual_helper",
            Signature::new(
                vec![Type::Scalar(fe2o3_kernel_ir::ScalarType::U32); 3],
                vec![Type::Scalar(fe2o3_kernel_ir::ScalarType::U32)],
            ),
            vec![ValueId(0), ValueId(1), ValueId(2)],
            vec![block],
        ));
        let symbols = compiler_module_symbol_closure_v1(&module);
        let expected = module
            .functions
            .iter()
            .filter(|f| f.role == FunctionRole::InternalHelper)
            .map(|f| f.id.as_str().to_owned())
            .collect::<std::collections::BTreeSet<_>>();
        assert!(!expected.is_empty());
        assert_eq!(
            symbols
                .internal_helpers
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>(),
            expected
        );
    }
}
