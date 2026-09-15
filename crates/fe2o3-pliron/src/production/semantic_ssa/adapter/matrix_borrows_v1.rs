//! Address transparency for replay-checked Matrix reference flows only.
//! No Matrix issuance, policy authority or ambient ZST is authorized here.
use super::*;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticDefinedCapabilityContractV1, SemanticMutabilityV1, SemanticPointerKindV1,
    SemanticPointerMetadataV1,
};

mod scoped_captures;

#[derive(Default)]
pub(super) struct MatrixBorrowSitesV1<'a> {
    pub(super) pairs: BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    scoped: scoped_captures::ScopedCaptures<'a>,
    read_source: Option<(&'a [SemanticTypeDeclV1], &'a SemanticExpandedRootV1)>,
    pub(super) work_units: usize,
}

impl<'a> MatrixBorrowSitesV1<'a> {
    pub(super) fn new(
        source: &'a AdmittedInertSemanticMirV1,
        expansion: &SemanticCallExpansionV1,
        view: &'a SemanticExpandedRootV1,
        bindings: &[SemanticExpandedDefinedCapabilityV1],
        max_work: usize,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let required = bindings
            .len()
            .checked_mul(32)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        if required > max_work {
            return Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                required,
                limit: max_work,
            });
        }
        let mismatch = || ProductionSemanticSsaErrorV1::ReplayMismatch;
        if !expansion
            .root(view.root())
            .is_some_and(|root| std::ptr::eq(root, view))
        {
            return Err(mismatch());
        }
        let mut result = Self::default();
        for binding in bindings {
            if binding.root() != view.root() {
                continue;
            }
            if matches!(binding.contract(), SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
                | SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_))
            {
                result.scoped.register(
                    source, expansion, view, binding, &mut result.pairs, max_work - required,
                )?;
                continue;
            }
            let SemanticDefinedCapabilityContractV1::KernelMatrixDerive(record) =
                binding.contract()
            else {
                continue;
            };
            let types = record.types();
            let [receiver] = binding.callee_arguments() else {
                return Err(mismatch());
            };
            if binding.root_identity() != view.identity()
                || binding.expansion_identity() != expansion.identity()
                || binding.arguments().len() != 1
                || binding.arguments()[0].ty() != types.context_reference
                || view
                    .body()
                    .locals()
                    .get(receiver.index() as usize)
                    .is_none_or(|local| local.ty() != types.context_reference)
                || view
                    .local_origins()
                    .get(receiver.index() as usize)
                    .is_none_or(|origin| {
                        origin.instance() != binding.callee_instance()
                            || origin.function() != record.function()
                    })
                || !matches!(source.types().get(types.context_reference.index() as usize)
                    .map(SemanticTypeDeclV1::shape), Some(SemanticTypeShapeV1::Pointer(pointer))
                    if pointer.kind() == SemanticPointerKindV1::Reference
                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                        && pointer.metadata() == SemanticPointerMetadataV1::None
                        && pointer.pointee() == types.context
                        && pointer.address_space() == 0 && pointer.pointer_width_bits() == 64)
                || types.context_reference == types.context
                || result
                    .pairs
                    .insert(types.context_reference, types.context)
                    .is_some_and(|old| old != types.context)
            {
                return Err(mismatch());
            }
        }
        result.scoped.register_wrapper_references(
            source.types(), &mut result.pairs, max_work - required,
        )?;
        if !result.scoped.wrapper_types().is_empty() {
            result.read_source = Some((source.types(), view));
        }
        // New registration work is paid from the existing shared flow budget.
        // The prior getter accounting remains unchanged.
        result.work_units = result.scoped.work_units();
        Ok(result)
    }

    pub(super) fn capture_cursor(
        &self,
        charge: &mut impl FnMut(usize) -> Result<(), ProductionSemanticSsaErrorV1>,
    ) -> Result<scoped_captures::CaptureCursor<'_, 'a>, ProductionSemanticSsaErrorV1> {
        self.scoped.cursor(charge)
    }

    pub(super) fn copied_matrix_field(
        &self,
        site: SemanticTransparentBorrowSiteV1,
        kind: &SemanticStatementKindV1,
        charge: &mut impl FnMut(usize) -> Result<(), ProductionSemanticSsaErrorV1>,
    ) -> Result<Option<[u32; 1]>, ProductionSemanticSsaErrorV1> {
        let Some((types, view)) = self.read_source else { return Ok(None); };
        self.scoped.copied_matrix_field(types, view, site, kind, charge)
    }

    pub(super) fn captured(
        &self,
        site: SemanticTransparentBorrowSiteV1,
        kind: &SemanticStatementKindV1,
        charge: &mut impl FnMut(usize) -> Result<(), ProductionSemanticSsaErrorV1>,
    ) -> Result<Option<&[u32]>, ProductionSemanticSsaErrorV1> {
        self.scoped.captured(site, kind, charge)
    }

    pub(super) fn carrier_leaves(&self) -> &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1> {
        self.scoped.matrix_leaves()
    }

    pub(super) fn policy_carrier_leaves(&self) -> &BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1> {
        self.scoped.policy_leaves()
    }

    pub(super) fn carrier_barriers(&self) -> &BTreeSet<SemanticTypeIdV1> {
        self.scoped.wrapper_types()
    }
}

#[cfg(test)]
mod tests;
