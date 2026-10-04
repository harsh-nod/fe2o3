//! Compiler-session seed for optional source publication. No decode constructor.
//! Captured only after the composition factory's existing source-work precharge.
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticFunctionDeclV1, SemanticFunctionIdV1,
};
use rustc_hir::Body;
use rustc_middle::ty::{Instance, TyCtxt};

use crate::rustc_semantic_adapter_v1::{
    CanonicalFunctionIdentitiesV1, canonical_function_identities_v1,
};
use crate::rustc_semantic_plan_v1::ProductionSemanticPreflightPlanV1;

const MAX_FUNCTIONS: usize = 3;

/// Not externally constructible, cloned, serialized, or recovered from a digest.
/// The actual HIR query storage is borrowed and is not claimed as owned payload.
pub(crate) struct OrderedCompositionSourceSeedV1<'tcx> {
    tcx: TyCtxt<'tcx>,
    functions: [Option<FunctionSeedV1<'tcx>>; MAX_FUNCTIONS],
    function_count: usize,
}

/// Only successful import plus exact completed source census constructs this.
/// Retaining it alone does not grant publication: the publisher must additionally
/// join the borrowed lowerer inspection, the selected actual HIR expansion and
/// the current retained source file. Dropping it forfeits that edit seam.
pub(crate) struct AuthenticatedOrderedCompositionSourceSeedV1<'tcx> {
    captured: OrderedCompositionSourceSeedV1<'tcx>,
    semantic_sha256: [u8; 32],
}

struct FunctionSeedV1<'tcx> {
    instance: Instance<'tcx>,
    identities: CanonicalFunctionIdentitiesV1,
    hir_body: Option<&'tcx Body<'tcx>>,
}

impl<'tcx> OrderedCompositionSourceSeedV1<'tcx> {
    pub(super) fn capture_precharged(
        tcx: TyCtxt<'tcx>,
        plan: &ProductionSemanticPreflightPlanV1<'tcx>,
    ) -> Result<Self, &'static str> {
        let producers = plan.function_producers();
        if !(1..=MAX_FUNCTIONS).contains(&producers.len()) {
            return Err("ordered composition source seed roster bound differs");
        }
        let mut functions = [const { None }; MAX_FUNCTIONS];
        for (index, producer) in producers.iter().enumerate() {
            if canonical_function_identities_v1(tcx, producer.instance) != producer.identities {
                return Err("ordered composition source seed actual Instance differs");
            }
            // A non-local body can still be valid composition input. It cannot
            // be selected by the source publisher. This is not HIR from metadata.
            let hir_body = producer
                .instance
                .def_id()
                .as_local()
                .and_then(|local| tcx.hir_maybe_body_owned_by(local));
            functions[index] = Some(FunctionSeedV1 {
                instance: producer.instance,
                identities: producer.identities,
                hir_body,
            });
        }
        Ok(Self {
            tcx,
            functions,
            function_count: producers.len(),
        })
    }

    pub(super) fn complete(
        self,
        semantic: &AdmittedInertSemanticMirV1,
    ) -> Result<AuthenticatedOrderedCompositionSourceSeedV1<'tcx>, &'static str> {
        if semantic.functions().len() != self.function_count {
            return Err("ordered composition source seed semantic roster differs");
        }
        for (index, function) in semantic.functions().iter().enumerate() {
            let captured = self.functions[index]
                .as_ref()
                .ok_or("ordered composition source seed function is absent")?;
            if !same_function(captured.identities, function) {
                return Err("ordered composition source seed semantic function differs");
            }
        }
        if self.functions[self.function_count..]
            .iter()
            .any(Option::is_some)
        {
            return Err("ordered composition source seed contains trailing functions");
        }
        Ok(AuthenticatedOrderedCompositionSourceSeedV1 {
            captured: self,
            semantic_sha256: *semantic.semantic_sha256().as_bytes(),
        })
    }
}

impl<'tcx> AuthenticatedOrderedCompositionSourceSeedV1<'tcx> {
    pub(crate) fn semantic_sha256(&self) -> &[u8; 32] {
        &self.semantic_sha256
    }

    /// Inline retained payload only. HIR/TyCtxt storage remains rustc-owned.
    pub(crate) const fn retained_storage_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
    }

    pub(super) fn require_root_publication(
        &self,
        function: &SemanticFunctionDeclV1,
    ) -> Result<(), &'static str> {
        if self.captured.function_count >= MAX_FUNCTIONS {
            return Err("publisher would exceed the admitted helper count");
        }
        if function.role() != fe2o3_mir_model::semantic_mir_v1::SemanticFunctionRoleV1::KernelRoot {
            return Err("publisher refuses nested helper promotion");
        }
        Ok(())
    }

    /// Not an identity-to-owner factory: the receiver retains actual session
    /// custody. The digest and complete function identity are selectors checked
    /// against that live owner, not credentials accepted on their own.
    pub(super) fn selected(
        &self,
        expected_semantic: &[u8; 32],
        function_id: SemanticFunctionIdV1,
        function: &SemanticFunctionDeclV1,
    ) -> Result<(TyCtxt<'tcx>, Instance<'tcx>, &'tcx Body<'tcx>), &'static str> {
        if expected_semantic != &self.semantic_sha256 {
            return Err("ordered composition source seed semantic identity differs");
        }
        let index = function_id.index() as usize;
        if index >= self.captured.function_count {
            return Err("ordered composition source seed selected function is absent");
        }
        let captured = self.captured.functions[index]
            .as_ref()
            .ok_or("ordered composition source seed selected function is absent")?;
        if !same_function(captured.identities, function)
            || canonical_function_identities_v1(self.captured.tcx, captured.instance)
                != captured.identities
        {
            return Err("ordered composition source seed selected function differs");
        }
        let body = captured
            .hir_body
            .ok_or("ordered composition source publishing requires local HIR")?;
        Ok((self.captured.tcx, captured.instance, body))
    }
}

fn same_function(
    captured: CanonicalFunctionIdentitiesV1,
    function: &SemanticFunctionDeclV1,
) -> bool {
    exact_axes(
        [
            *captured.function().as_bytes(),
            *captured.item_definition().as_bytes(),
            *captured.monomorphization().as_bytes(),
            *captured.generic_type_arguments().as_bytes(),
            *captured.const_generic_arguments().as_bytes(),
        ],
        [
            *function.identity().as_bytes(),
            *function.item_definition_identity().as_bytes(),
            *function.monomorphization_identity().as_bytes(),
            *function.generic_type_arguments_identity().as_bytes(),
            *function.const_generic_arguments_identity().as_bytes(),
        ],
    )
}

fn exact_axes(actual: [[u8; 32]; 5], expected: [[u8; 32]; 5]) -> bool {
    actual == expected
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_function_identity_axis_is_required() {
        let expected = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]];
        assert!(exact_axes(expected, expected));
        for axis in 0..5 {
            let mut changed = expected;
            changed[axis][17] ^= 1;
            assert!(!exact_axes(changed, expected));
        }
    }

    #[test]
    fn same_item_different_monomorphization_is_not_the_same_function() {
        let actual = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]];
        let mut other = actual;
        other[0] = [6; 32];
        other[2] = [7; 32];
        other[4] = [8; 32];
        assert_eq!(actual[1], other[1]);
        assert!(!exact_axes(actual, other));
    }

    #[test]
    fn function_axes_are_not_a_sorted_or_unordered_set() {
        let actual = [[1; 32], [2; 32], [3; 32], [4; 32], [5; 32]];
        let mut swapped = actual;
        swapped.swap(0, 2);
        assert!(!exact_axes(actual, swapped));
    }

    #[test]
    fn source_seed_inline_payload_is_finitely_bounded() {
        // A layout bound, not a fake rustc session or an RSS assertion.
        assert!(
            std::mem::size_of::<AuthenticatedOrderedCompositionSourceSeedV1<'static>>() <= 4096
        );
    }
}
