//! Fixed production source/target join before any formal-memory candidate.

use super::*;

fn check_root_binding(
    entry: &fe2o3_mir_model::semantic_mir_v1::SemanticKernelEntryV1,
    kernel: &fe2o3_kernel_ir::Kernel,
    descriptor_binding: [u8; 32],
    descriptor_symbol: &str,
) -> Result<(), ProductionPipelineError> {
    if entry.kernel_binding_identity().as_bytes() != &descriptor_binding
        || entry.export_symbol().as_bytes() != descriptor_symbol.as_bytes()
        || kernel.id.as_str() != descriptor_symbol
        || kernel.entry.as_str() != descriptor_symbol
    {
        return Err(ProductionPipelineError::Geometry(
            crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
        ));
    }
    Ok(())
}

pub(super) fn authenticated_envelopes(
    lowered: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    bindings: &AuthenticatedProductionBindings,
) -> Result<Vec<[u64; 3]>, ProductionPipelineError> {
    let semantic = lowered.semantic().semantic();
    let module = lowered.module();
    let closure = || {
        ProductionPipelineError::Geometry(
            crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure,
        )
    };
    if semantic.roots().is_empty()
        || semantic.roots().len() != bindings.typed_descriptor_roots.len()
        || semantic.roots().len() != module.kernels.len()
    {
        return Err(closure());
    }
    let mut envelopes = Vec::with_capacity(semantic.roots().len());
    for ((typed_root, semantic_root), kernel) in bindings
        .typed_descriptor_roots
        .iter()
        .zip(semantic.roots())
        .zip(&module.kernels)
    {
        let function = semantic
            .functions()
            .get(semantic_root.index() as usize)
            .ok_or_else(closure)?;
        let entry = function.kernel_entry().ok_or_else(closure)?;
        check_root_binding(
            entry,
            kernel,
            typed_root.kernel_binding_bytes(),
            typed_root.entry_symbol(),
        )?;
        let launch = typed_root
            .source_launch()
            .ok_or(ProductionPipelineError::Geometry(
            crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
        ))?;
        // This is the retained authenticated rustc target, not a caller-authored
        // Bits64 flag or device-name string. Both admitted profiles use the
        // same checked i64 logical-global-ID lowering contract.
        let geometry = crate::production_geometry_v1::derive_production_geometry_v1(
            module,
            typed_root.entry_symbol(),
            function,
            launch,
            bindings.rustc_target.profile().device_target(),
        )
        .map_err(ProductionPipelineError::Geometry)?;
        envelopes.push(
            geometry
                .formal_coordinate_envelope_v2()
                .map_err(ProductionPipelineError::Geometry)?,
        );
    }
    Ok(envelopes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_mir_model::semantic_mir_v1::{
        SemanticKernelBindingIdentityV1, SemanticKernelEntryV1, SemanticKernelSourceContractV1,
        SemanticLinkSymbolV1,
    };

    fn source_entry(binding: [u8; 32], symbol: &str) -> SemanticKernelEntryV1 {
        SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(symbol.as_bytes().to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256(binding),
            SemanticKernelSourceContractV1::new(None, None, None).unwrap(),
        )
    }

    fn physical_kernel(id: &str, entry: &str) -> fe2o3_kernel_ir::Kernel {
        fe2o3_kernel_ir::Kernel::new(
            id,
            entry,
            fe2o3_kernel_ir::LaunchDomain::D1 {
                x: fe2o3_kernel_ir::LaunchExtent::Dynamic,
            },
        )
    }

    #[test]
    fn production_root_binding_check_accepts_exact_semantic_descriptor_and_physical_pairs() {
        for (binding, symbol) in [([1; 32], "first_entry"), ([2; 32], "second_entry")] {
            check_root_binding(
                &source_entry(binding, symbol),
                &physical_kernel(symbol, symbol),
                binding,
                symbol,
            )
            .unwrap();
        }
    }

    #[test]
    fn production_root_binding_check_rejects_each_binding_or_symbol_substitution() {
        let entry = source_entry([1; 32], "entry");
        for (kernel, binding, symbol) in [
            (physical_kernel("entry", "entry"), [2; 32], "entry"),
            (physical_kernel("entry", "entry"), [1; 32], "other"),
            (physical_kernel("other", "entry"), [1; 32], "entry"),
            (physical_kernel("entry", "other"), [1; 32], "entry"),
        ] {
            assert!(matches!(
                check_root_binding(&entry, &kernel, binding, symbol),
                Err(ProductionPipelineError::Geometry(
                    crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure
                ))
            ));
        }
        assert!(matches!(
            check_root_binding(
                &source_entry([1; 32], "other"),
                &physical_kernel("entry", "entry"),
                [1; 32],
                "entry",
            ),
            Err(ProductionPipelineError::Geometry(
                crate::production_geometry_v1::ProductionGeometryErrorV1::KernelClosure
            ))
        ));
    }

    #[test]
    fn authenticated_geometry_precedes_fresh_formal_candidate_without_fallback() {
        let source = include_str!("../production_pipeline.rs");
        let body = source
            .split_once("    fn admit_formal_memory(\n")
            .unwrap()
            .1
            .split_once("\nimpl FormalMemoryAdmittedProductionCompilation")
            .unwrap()
            .0;
        assert!(
            body.find("authenticated_envelopes(&lowered, &bindings)?")
                .unwrap()
                < body.find("::try_admit_for_launch_envelopes_v2(").unwrap()
        );
        assert!(!body.contains("::try_admit("));
        assert!(!body.contains(".or_else("));
        assert!(!body.contains("unwrap_or"));
        let join = include_str!("formal_envelope_preflight_v2.rs")
            .split_once("\n#[cfg(test)]")
            .unwrap()
            .0;
        assert!(join.contains("bindings.rustc_target.profile().device_target()"));
        assert!(join.contains("entry.kernel_binding_identity().as_bytes()"));
        assert!(join.contains("typed_root.kernel_binding_bytes()"));
        assert!(join.contains("typed_root.entry_symbol()"));
        assert!(join.contains(".source_launch()"));
        assert!(
            join.find("check_root_binding(\n            entry,")
                .unwrap()
                < join.find("let launch = typed_root").unwrap()
        );
    }
}
