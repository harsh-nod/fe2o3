//! Exact frame copies are exclusive call reborrows, not ordinary mutable copies.

use super::*;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedTerminatorOriginV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticLocalRoleV1, SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
    SemanticSourceArgumentOwnershipV1, SemanticTypeShapeV1,
};

// All lookups below are indexed and projection lists are tested for emptiness,
// never cloned or walked. Charge a fixed conservative number of logical steps.
const COPY_CHECK_WORK: usize = 96;

pub(super) struct CheckedTransfers<'a> {
    source: &'a AdmittedInertSemanticMirV1,
    view: &'a SemanticExpandedRootV1,
}

impl<'a> CheckedTransfers<'a> {
    // execution_sites has already replayed this expansion when obtaining its
    // defined-capability bindings. No arbitrary statement/site roster enters.
    pub(super) fn new(
        source: &'a AdmittedInertSemanticMirV1,
        expansion: &SemanticCallExpansionV1,
        view: &'a SemanticExpandedRootV1,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        if expansion.source_semantic_sha256() != source.semantic_sha256().as_bytes()
            || !expansion
                .root(view.root())
                .is_some_and(|expected| std::ptr::eq(expected, view))
        {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        Ok(Self { source, view })
    }

    pub(super) fn accepts(
        &self,
        function: &SemanticFunctionDeclV1,
        candidate: SemanticBorrowCandidateV1,
        budget: &mut Budget,
    ) -> Result<bool, ProductionSemanticSsaErrorV1> {
        budget.charge(COPY_CHECK_WORK)?;
        Ok(std::ptr::eq(function, self.view.body()) && self.matches(candidate).is_some())
    }

    fn matches(&self, candidate: SemanticBorrowCandidateV1) -> Option<()> {
        if !candidate.value_alias {
            return None;
        }
        let site = candidate.site;
        let body = self.view.body().blocks().get(site.block as usize)?;
        let origin = self.view.block_origins().get(site.block as usize)?;
        let SemanticExpandedStatementOriginV1::ParameterTransfer { callee, argument } =
            *origin.statements().get(site.statement as usize)?
        else {
            return None;
        };
        if origin.terminator() != (SemanticExpandedTerminatorOriginV1::CallEntry { callee }) {
            return None;
        }
        let child = self.view.instances().get(callee.index() as usize)?;
        let parent = self
            .view
            .instances()
            .get(origin.instance().index() as usize)?;
        let caller = self
            .source
            .functions()
            .get(origin.function().index() as usize)?;
        let target = self
            .source
            .functions()
            .get(child.function().index() as usize)?;
        if child.parent() != Some(origin.instance())
            || child.call_block() != Some(origin.block())
            || parent.function() != origin.function()
            || parent.function_identity() != caller.identity()
            || child.function_identity() != target.identity()
            || parent.block_start().checked_add(origin.block().index()) != Some(site.block)
        {
            return None;
        }
        let source_block = caller.blocks().get(origin.block().index() as usize)?;
        let SemanticTerminatorKindV1::Call(call) = source_block.terminator().kind() else {
            return None;
        };
        if self
            .source
            .callables()
            .get(call.callee().index() as usize)?
            != &SemanticCallableDeclV1::defined(child.function())
            || target
                .abi()
                .source_argument_ownership()
                .get(argument as usize)
                != Some(&SemanticSourceArgumentOwnershipV1::UniqueBorrow)
        {
            return None;
        }
        let SemanticOperandV1::Copy(original) = call.arguments().get(argument as usize)? else {
            return None;
        };
        let item = body.statements().get(site.statement as usize)?;
        let SemanticStatementKindV1::Assign(assignment) = item.kind() else {
            return None;
        };
        let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(source)) = assignment.value().kind()
        else {
            return None;
        };
        let destination = assignment.destination();
        let reference = source.ty();
        if !original.projections().is_empty()
            || !source.projections().is_empty()
            || !destination.projections().is_empty()
            || original.ty() != reference
            || assignment.value().result_type() != reference
            || destination.ty() != reference
            || target.abi().source_input_types().get(argument as usize) != Some(&reference)
            || candidate.source_reference != Some(source.local().index())
            || item.source() != source_block.terminator().source()
        {
            return None;
        }
        let SemanticTypeShapeV1::Pointer(pointer) =
            self.source.types().get(reference.index() as usize)?.shape()
        else {
            return None;
        };
        if pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.mutability() != SemanticMutabilityV1::Mutable
            || pointer.pointee() != candidate.source_type
            || pointer.metadata() != SemanticPointerMetadataV1::None
        {
            return None;
        }
        let from = self
            .view
            .local_origins()
            .get(source.local().index() as usize)?;
        let to = self
            .view
            .local_origins()
            .get(destination.local().index() as usize)?;
        let parameter = target.locals().get(to.local().index() as usize)?;
        if from.instance() != origin.instance()
            || from.function() != origin.function()
            || from.local() != original.local()
            || to.instance() != callee
            || to.function() != child.function()
            || parameter.role() != SemanticLocalRoleV1::Argument(argument)
            || parameter.ty() != reference
            || caller.locals().get(original.local().index() as usize)?.ty() != reference
            || parent.local_start().checked_add(original.local().index())
                != Some(source.local().index())
            || child.local_start().checked_add(to.local().index())
                != Some(destination.local().index())
        {
            return None;
        }
        let SemanticTerminatorKindV1::Goto(edge) = body.terminator().kind() else {
            return None;
        };
        (child.block_start().checked_add(target.entry().index()) == Some(edge.target().index()))
            .then_some(())
    }
}
