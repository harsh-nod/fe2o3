//! Explicit single-root ordinary compiler selection, not a source authority.
//! The caller owns the original prepaid account and keeps it around the returned
//! program. Existing source preparation and structural finish remain mandatory.
use super::*;

type CompileResult = Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1>;
type CompileCallback<'a> =
    dyn FnMut(ProductionConstructionV1, Vec<u64>) -> Option<CompileResult> + 'a;

/// Private routing only. Paid never falls back to legacy on refusal or reuse.
pub(super) enum Selection<'a> {
    Legacy,
    Paid(&'a mut CompileCallback<'a>),
}

impl Selection<'_> {
    pub(super) fn is_legacy(&self) -> bool {
        matches!(self, Self::Legacy)
    }

    pub(super) fn compile(
        &mut self,
        construction: ProductionConstructionV1,
        coherent: Vec<u64>,
        ranked_ir: String,
        sources: Vec<ProjectedAccessSourceV1>,
    ) -> Result<ProductionRankedKernelLoweringInputV1, ProductionRankedProjectionErrorV1> {
        let result = match self {
            Self::Legacy => compile_ranked_kernel_for_gfx942_lowering_v1(
                construction,
                ProductionSessionLimitsV1::default(),
                coherent,
            ),
            Self::Paid(callback) => callback(construction, coherent).ok_or(
                ProductionRankedProjectionErrorV1::Incomplete(
                    "paid ordinary compiler was already consumed",
                ),
            )?,
        };
        result.map_err(|error| ProductionRankedProjectionErrorV1::Compile {
            error: Box::new(error),
            ranked_ir,
            access_sources: sources,
        })
    }
}

pub(super) fn require_single_ordinary(
    root_count: usize,
    reference_count: usize,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    if root_count != 1 || reference_count != 0 {
        return Err(ProductionRankedProjectionErrorV1::Unsupported(
            "paid ordinary verifier requires one root and no reference-effect bindings",
        ));
    }
    Ok(())
}

/// Runs the exact materialized-source pipeline with one consumed compile call.
/// This function does not mint a paid permit: only the private retained owner
/// supplies its closure after original-account admission.
pub(crate) fn project_owned_with_one_compile(
    materialized: fe2o3_lower_mir_kernel::ProductionPreRankedKirOwnerV1,
    root_inputs: &[ProductionRankedRootInputV1],
    reference_bindings: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
    compile: impl FnOnce(ProductionConstructionV1, Vec<u64>) -> CompileResult,
) -> Result<ProductionRankedSemanticProgramV1, ProductionRankedProjectionErrorV1> {
    require_single_ordinary(root_inputs.len(), reference_bindings.as_slice().len())?;
    let mut compile = Some(compile);
    let result = {
        let mut callback = |construction, coherent| {
            compile
                .take()
                .map(|selected| selected(construction, coherent))
        };
        let mut selection = Selection::Paid(&mut callback);
        project_and_verify_ranked_materialized_with_compile_v1(
            materialized,
            root_inputs,
            reference_bindings,
            &mut selection,
        )
    };
    if result.is_ok() && compile.is_some() {
        drop(result);
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "paid ordinary verifier did not consume its compiler",
        ));
    }
    result
}

#[cfg(test)]
#[path = "paid_ranked_compile_v1_tests.rs"]
mod tests;
