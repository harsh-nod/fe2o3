//! Closed reference sinks for retained Math bodies; never issuer evidence.
use super::*;
use fe2o3_mir_model::semantic_direct_call_expansion_v1::SemanticExpandedDefinedCapabilityV1;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticDefinedCapabilityContractV1, SemanticDirectCallV1,
    SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
    SemanticSourceArgumentOwnershipV1,
};

#[derive(Default)]
pub(super) struct MathBorrowSitesV1 {
    pub(super) pairs: BTreeMap<SemanticTypeIdV1, SemanticTypeIdV1>,
    binds: BTreeMap<SemanticTransparentBorrowSiteV1, (SemanticAssignmentV1, [u32; 2])>,
}

impl MathBorrowSitesV1 {
    pub(super) fn new(
        source: &AdmittedInertSemanticMirV1,
        view: &SemanticExpandedRootV1,
        bindings: &[SemanticExpandedDefinedCapabilityV1],
        max_work: usize,
    ) -> Result<Self, ProductionSemanticSsaErrorV1> {
        let required = bindings.len().saturating_mul(32);
        if required > max_work {
            return Err(ProductionSemanticSsaErrorV1::AggregateResourceLimit {
                resource: SsaPlannerResourceV1::WorkUnits,
                required,
                limit: max_work,
            });
        }
        let mut result = Self::default();
        for binding in bindings {
            if binding.root() != view.root() {
                continue;
            }
            if binding.root_identity() != view.identity() {
                return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
            }
            match binding.contract() {
                SemanticDefinedCapabilityContractV1::WorkgroupEpochProjection(_)
                | SemanticDefinedCapabilityContractV1::KernelMatrixDerive(_)
                | SemanticDefinedCapabilityContractV1::ReusableLdsConversion(_)
                | SemanticDefinedCapabilityContractV1::GuardedGridLeader(_)
                | SemanticDefinedCapabilityContractV1::ReusablePhase(_)
                | SemanticDefinedCapabilityContractV1::PolicyMatrixBind(_)
                | SemanticDefinedCapabilityContractV1::PolicyGfx950Narrow(_) => (),
                SemanticDefinedCapabilityContractV1::KernelMathDerive(record) => {
                    result.pair(
                        source.types(),
                        record.types().context_reference,
                        record.types().context,
                    )?;
                }
                SemanticDefinedCapabilityContractV1::PolicyMathBind(record) => {
                    let ids = record.types();
                    result.pair(source.types(), ids.math_reference, ids.math)?;
                    result.pair(source.types(), ids.policy_reference, ids.capability)?;
                    let site = SemanticTransparentBorrowSiteV1 {
                        block: binding.expanded_entry_block().index(),
                        statement: 0,
                    };
                    let origin = view
                        .block_origins()
                        .get(site.block as usize)
                        .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
                    let SemanticStatementKindV1::Assign(assignment) = view.body().blocks()
                        [site.block as usize]
                        .statements()
                        .first()
                        .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?
                        .kind()
                    else {
                        return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                    };
                    let SemanticRvalueKindV1::Aggregate(aggregate) = assignment.value().kind()
                    else {
                        return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                    };
                    let [math, policy] = binding.callee_arguments() else {
                        return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                    };
                    if origin.instance() != binding.callee_instance() || origin.function() != record.function()
                        || origin.statements().first() != Some(&SemanticExpandedStatementOriginV1::Source { statement: 0 })
                        || assignment.destination().local() != binding.callee_return()
                        || !assignment.destination().projections().is_empty() || assignment.destination().ty() != ids.bound
                        || assignment.value().result_type() != ids.bound || aggregate.operands().len() != 3
                        || !aggregate.operands()[..2].iter().zip([(*math, ids.math_reference), (*policy, ids.policy_reference)])
                            .all(|(operand, (local, ty))| matches!(operand, SemanticOperandV1::Copy(place)
                                if place.local() == local && place.ty() == ty && place.projections().is_empty()))
                    {
                        return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                    }
                    // The replayed common binding already checks the complete
                    // original aggregate, including its third marker operand.
                    if result
                        .binds
                        .insert(site, (assignment.clone(), [math.index(), policy.index()]))
                        .is_some()
                    {
                        return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
                    }
                }
            }
        }
        Ok(result)
    }

    fn pair(
        &mut self,
        types: &[SemanticTypeDeclV1],
        reference: SemanticTypeIdV1,
        owned: SemanticTypeIdV1,
    ) -> Result<(), ProductionSemanticSsaErrorV1> {
        if !shared(types, reference, owned)
            || self
                .pairs
                .insert(reference, owned)
                .is_some_and(|old| old != owned)
        {
            return Err(ProductionSemanticSsaErrorV1::ReplayMismatch);
        }
        Ok(())
    }

    pub(super) fn captured(
        &self,
        site: SemanticTransparentBorrowSiteV1,
        kind: &SemanticStatementKindV1,
    ) -> Option<[u32; 2]> {
        let (expected, locals) = self.binds.get(&site)?;
        matches!(kind, SemanticStatementKindV1::Assign(actual) if actual == expected)
            .then_some(*locals)
    }
}

fn shared(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    owned: SemanticTypeIdV1,
) -> bool {
    reference != owned
        && matches!(types.get(reference.index() as usize).map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Pointer(p)) if p.kind() == SemanticPointerKindV1::Reference
            && p.mutability() == SemanticMutabilityV1::Immutable && p.metadata() == SemanticPointerMetadataV1::None && p.pointee() == owned)
}

#[derive(Clone, Copy)]
pub(super) struct MathConsumerBorrowV1<'a> {
    abi: &'a fe2o3_mir_model::semantic_mir_v1::SemanticFunctionAbiV1,
    pair: (SemanticTypeIdV1, SemanticTypeIdV1),
}

impl<'a> MathConsumerBorrowV1<'a> {
    pub(super) fn for_callable(
        types: &[SemanticTypeDeclV1],
        callable: &'a SemanticCallableDeclV1,
    ) -> Option<Self> {
        let SemanticCallableDeclV1::CompilerIntrinsic {
            binding,
            operation: SemanticCompilerIntrinsicOperationV1::PolicyMathF32 { contract },
            ..
        } = callable
        else {
            return None;
        };
        let abi = binding.abi();
        let pair = (contract.types().bound_reference, contract.types().bound);
        (binding.identity() == contract.source_identity()
            && shared(types, pair.0, pair.1)
            && abi.source_argument_ownership().first()
                == Some(&SemanticSourceArgumentOwnershipV1::SharedBorrow)
            && !abi.c_variadic()
            && abi.hidden_arguments().is_empty()
            && abi
                .source_input_types()
                .iter()
                .copied()
                .eq(contract.signature().arguments())
            && abi.source_output_type() == contract.types().element)
            .then_some(Self { abi, pair })
    }
    pub(super) fn pair(self) -> (SemanticTypeIdV1, SemanticTypeIdV1) {
        self.pair
    }
    pub(super) fn accepts(
        self,
        call: &SemanticDirectCallV1,
        argument: usize,
        owned: SemanticTypeIdV1,
    ) -> bool {
        argument == 0
            && owned == self.pair.1
            && call.variadic_argument_abis().is_empty()
            && call.arguments().iter().map(SemanticOperandV1::ty).eq(self
                .abi
                .source_input_types()
                .iter()
                .copied())
            && call.destination().is_some_and(|d| {
                d.place().projections().is_empty()
                    && d.place().ty() == self.abi.source_output_type()
            })
    }
}

#[cfg(test)]
#[path = "math_borrows_v1/tests.rs"]
mod tests;
