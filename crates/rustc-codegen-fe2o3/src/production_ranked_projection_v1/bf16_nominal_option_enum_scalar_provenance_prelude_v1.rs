//! Source-owned Option -> enum -> scalar -> provenance continuation.
//! Stops before allocation contracts; prior entries and ordinary routes are unchanged.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::RetainedLocalProvenanceV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ProvenancePhase {
    Fresh,
    Terminal,
    BeforeAllocation,
}

struct PendingBeforeAllocationV1 {
    phase: ProvenancePhase,
    earlier: PendingBeforeProvenanceV1,
    provenance: RetainedLocalProvenanceV1,
    provenance_invoked: bool,
    failure: Option<Backend>,
}
impl PendingBeforeAllocationV1 {
    fn new() -> Self {
        Self {
            phase: ProvenancePhase::Fresh,
            earlier: PendingBeforeProvenanceV1::new(),
            provenance: RetainedLocalProvenanceV1::new(),
            provenance_invoked: false,
            failure: None,
        }
    }
    fn prepare(&mut self, source: &Source<'_>, resources: &mut Prep<'_, '_>) -> BResult<()> {
        let fresh = self.phase == ProvenancePhase::Fresh
            && !self.provenance_invoked
            && self.failure.is_none();
        self.phase = ProvenancePhase::Terminal;
        if !fresh
            || !resources.is_metered()
            || resources.has_denial()
            || resources.original_ledger_v1() != Some(source.ledger)
        {
            return Err(accounting());
        }
        resources.work(32)?;
        self.earlier.prepare(source, resources)?;
        if self.earlier.phase != ScalarPhase::BeforeProvenance {
            return Err(accounting());
        }
        // These exact tables are physically owned by the earlier scalar stage;
        // the original source loan and both owners outlive checked postflight.
        let scalar = self
            .earlier
            .scalar
            .completed_for(source.function, resources)?;
        self.provenance_invoked = true;
        self.provenance.prepare_into(
            source.callables,
            source.types,
            source.function,
            &scalar.counts,
            &scalar.address_escaped,
            resources,
        )?;
        self.provenance.completed_for(
            source.callables,
            source.types,
            source.function,
            &scalar.counts,
            &scalar.address_escaped,
            resources,
        )?;
        if resources.has_denial() {
            return Err(accounting());
        }
        self.phase = ProvenancePhase::BeforeAllocation;
        Ok(())
    }
    fn view<'a>(
        &'a self,
        source: &'a Source<'_>,
        resources: &Prep<'_, '_>,
    ) -> BResult<BeforeAllocationV1<'a>> {
        if self.phase != ProvenancePhase::BeforeAllocation || !self.provenance_invoked {
            return Err(accounting());
        }
        let earlier = self.earlier.view(source, resources)?;
        let scalar = earlier.scalar_inventory();
        let provenance = self.provenance.completed_for(
            source.callables,
            source.types,
            source.function,
            &scalar.counts,
            &scalar.address_escaped,
            resources,
        )?;
        Ok(BeforeAllocationV1 {
            earlier,
            provenance,
            provenance_invoked: self.provenance_invoked,
        })
    }
}

/// Immutable actual source and completed pre-allocation DATA, not capabilities,
/// allocation contracts, later argument writers, F2, or ordinary-route readiness.
pub(in crate::production_ranked_projection_v1) struct BeforeAllocationV1<'a> {
    earlier: BeforeProvenanceV1<'a>,
    provenance: &'a LocalProvenanceV1,
    provenance_invoked: bool,
}
impl BeforeAllocationV1<'_> {
    pub(in crate::production_ranked_projection_v1) fn function(&self) -> &SemanticFunctionDeclV1 {
        self.earlier.function()
    }
    pub(in crate::production_ranked_projection_v1) fn option_producers(
        &self,
    ) -> &[SemanticOptionProducerV1] {
        self.earlier.option_producers()
    }
    pub(in crate::production_ranked_projection_v1) fn option_dominance(
        &self,
    ) -> &SemanticOptionDominanceV1 {
        self.earlier.option_dominance()
    }
    pub(in crate::production_ranked_projection_v1) fn enum_dominance(
        &self,
    ) -> &SemanticEnumPayloadDominanceV1 {
        self.earlier.enum_dominance()
    }
    pub(in crate::production_ranked_projection_v1) fn enum_api_invoked(&self) -> bool {
        self.earlier.enum_api_invoked()
    }
    pub(in crate::production_ranked_projection_v1) fn scalar_inventory(
        &self,
    ) -> &AssertionDefinitionInventoryV1 {
        self.earlier.scalar_inventory()
    }
    pub(in crate::production_ranked_projection_v1) fn scalar_api_invoked(&self) -> bool {
        self.earlier.scalar_api_invoked()
    }
    pub(in crate::production_ranked_projection_v1) fn provenance(&self) -> &LocalProvenanceV1 {
        self.provenance
    }
    pub(in crate::production_ranked_projection_v1) fn provenance_api_invoked(&self) -> bool {
        self.provenance_invoked
    }
}

const PROVENANCE_FRAME_ROWS: usize = 23;
fn provenance_frame_rows<R, F>() -> BResult<[usize; PROVENANCE_FRAME_ROWS]> {
    Ok([
        size_of::<PendingBeforeAllocationV1>(),
        size_of::<(
            PendingBeforeProvenanceV1,
            RetainedLocalProvenanceV1,
            ProvenancePhase,
            bool,
            Option<Backend>,
        )>(),
        size_of::<(
            &mut PendingBeforeAllocationV1,
            &Source<'static>,
            &mut Prep<'static, 'static>,
            bool,
            BResult<()>,
        )>(),
        size_of::<(
            BeforeAllocationV1<'static>,
            BeforeProvenanceV1<'static>,
            &AssertionDefinitionInventoryV1,
            &LocalProvenanceV1,
            BResult<&LocalProvenanceV1>,
            BResult<BeforeAllocationV1<'static>>,
        )>(),
        size_of::<(
            &PendingBeforeAllocationV1,
            &Source<'static>,
            &Prep<'static, 'static>,
            bool,
        )>(),
        size_of::<(
            &BeforeAllocationV1<'static>,
            &SemanticFunctionDeclV1,
            &[SemanticOptionProducerV1],
            &SemanticOptionDominanceV1,
            &SemanticEnumPayloadDominanceV1,
            &AssertionDefinitionInventoryV1,
            &LocalProvenanceV1,
            bool,
        )>(),
        size_of::<(
            &ProductionPreRankedKirOwnerV1,
            &CanonicalKirInventoryV1<'static>,
            SemanticFunctionIdV1,
            SemanticFunctionIdV1,
            SemanticBlockIdV1,
            &SemanticDirectCallV1,
            &mut Budget<'static>,
            F,
        )>(),
        size_of::<(
            &mut PendingBeforeAllocationV1,
            &mut usize,
            &ProductionPreRankedKirOwnerV1,
            &CanonicalKirInventoryV1<'static>,
            SemanticFunctionIdV1,
            &SemanticDirectCallV1,
            F,
        )>(),
        size_of::<(
            &CheckedBf16NominalCallV1<'static>,
            &mut Budget<'static>,
            &ProductionPreRankedKirOwnerV1,
            &AdmittedInertSemanticMirV1,
            Option<&SemanticFunctionDeclV1>,
            &SemanticFunctionDeclV1,
            &[SemanticCallableDeclV1],
            &[SemanticTypeDeclV1],
            usize,
            bool,
        )>(),
        size_of::<(Source<'static>, SourceIdentity, Ledger, Option<Ledger>)>(),
        size_of::<(Prep<'static, 'static>, &mut usize, &mut Budget<'static>)>(),
        size_of::<(F, F, R, R)>(),
        size_of::<(Result<R>, Result<R>, BResult<R>, BResult<R>, BResult<()>)>(),
        size_of::<(Custody, Custody, usize, usize, bool, Result<()>)>(),
        size_of::<(Backend, Option<Backend>, &Backend, QueryError)>(),
        size_of::<(PanicPayload, std::result::Result<Result<R>, PanicPayload>)>(),
        size_of::<(
            [usize; PROVENANCE_FRAME_ROWS],
            [usize; PROVENANCE_FRAME_ROWS],
            BResult<[usize; PROVENANCE_FRAME_ROWS]>,
            std::array::IntoIter<usize, PROVENANCE_FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            BResult<usize>,
        )>(),
        size_of::<(BResult<usize>, usize, usize, Option<usize>, &usize)>(),
        size_of::<(
            &mut RetainedLocalProvenanceV1,
            &[SemanticCallableDeclV1],
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &Vec<u8>,
            &[u8],
            &Vec<bool>,
            &[bool],
            &mut Prep<'static, 'static>,
            BResult<()>,
        )>(),
        size_of::<(
            &RetainedLocalProvenanceV1,
            &[SemanticCallableDeclV1],
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &Vec<u8>,
            &[u8],
            &Vec<bool>,
            &[bool],
            &Prep<'static, 'static>,
            BResult<&LocalProvenanceV1>,
        )>(),
        size_of::<(
            &mut PendingBeforeProvenanceV1,
            &Source<'static>,
            &mut Prep<'static, 'static>,
            BResult<()>,
            &PendingBeforeProvenanceV1,
            BResult<BeforeProvenanceV1<'static>>,
        )>(),
        size_of::<(
            &RetainedScalarInventoryV1,
            &SemanticFunctionDeclV1,
            &Prep<'static, 'static>,
            &AssertionDefinitionInventoryV1,
            BResult<&AssertionDefinitionInventoryV1>,
        )>(),
        size_of::<(
            BeforeProvenanceV1<'static>,
            &BeforeProvenanceV1<'static>,
            &AssertionDefinitionInventoryV1,
            &LocalProvenanceV1,
            &Vec<u8>,
            &[u8],
            &Vec<bool>,
            &[bool],
        )>(),
    ])
}
fn provenance_frame<R, F>() -> BResult<usize> {
    // Preserve the complete prior checkpoint policy. Retained provenance,
    // carrier and FIFO components charge their own unchanged policies in place.
    provenance_frame_rows::<R, F>()?
        .into_iter()
        .try_fold(super::scalar_frame::<R, F>()?, |sum, row| {
            sum.checked_add(row).ok_or_else(arithmetic)
        })
}

/// A separate authenticated entry. Never nests an externally owned provenance
/// payload inside the older BeforeProvenance entry's earlier refund boundary.
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_option_enum_scalar_provenance_before_allocation_v1<
    'w,
    R,
    F,
>(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'a, 'b, 'm> FnOnce(BeforeAllocationV1<'a>, &mut Prep<'b, 'm>) -> BResult<R>,
{
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, move |budget| {
        let before = Custody::take(budget)?;
        let bytes = provenance_frame::<R, F>().map_err(query_error)?;
        let mut owned = 0usize;
        PreparationResourcesV1::new(budget, &mut owned)
            .reserve_storage(bytes)
            .map_err(query_error)?;
        let mut pending = PendingBeforeAllocationV1::new();
        let result = owner.with_checked_bf16_nominal_call_v1(
            inventory,
            root,
            caller,
            block,
            call,
            budget,
            |checked, budget| {
                let source_owner = checked.emission().owner();
                if !std::ptr::eq(source_owner, owner)
                    || !checked.belongs_to(inventory)
                    || !std::ptr::eq(checked.source_call(), call)
                {
                    return Err(QueryError::Unavailable(
                        "Option/enum/scalar/provenance checked source identity differs",
                    ));
                }
                let semantic = source_owner.semantic_ssa().source_semantic();
                let function = semantic.functions().get(caller.index() as usize).ok_or(
                    QueryError::Unavailable("Option/enum/scalar/provenance source caller absent"),
                )?;
                if semantic.functions().len() != 2
                    || semantic.types().len() > 4096
                    || semantic.callables().len() > 4096
                    || function.blocks().len() > 32
                    || function.locals().len() > 4096
                {
                    return Err(QueryError::Unavailable(
                        "Option/enum/scalar/provenance source exceeds closed owner profile",
                    ));
                }
                let source = Source {
                    function,
                    callables: semantic.callables(),
                    types: semantic.types(),
                    ledger: (
                        budget as *const Budget<'_> as usize,
                        budget.work_ledger_identity_v1(),
                    ),
                };
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                let entered = pending.prepare(&source, &mut resources);
                let observed = match entered {
                    Ok(()) => match pending.view(&source, &resources) {
                        Ok(view) => inspect(view, &mut resources),
                        Err(error) => Err(error),
                    },
                    Err(error) => Err(error),
                };
                match observed {
                    Ok(value) => Ok(value),
                    Err(error) => {
                        let mapped = saved_query_error(&error);
                        pending.failure = Some(error);
                        Err(mapped)
                    }
                }
            },
        );
        // The checked wrapper still converts/drops panic payloads at its own
        // unchanged boundary. All preparation owners and saved errors outlive it.
        let custody = before.check(budget, owned);
        let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
        drop(pending);
        custody?;
        budget.release_storage(owned)?;
        match result {
            Ok(_) if denied => Err(Resource::Accounting.into()),
            other => other,
        }
    })
}

#[cfg(test)]
#[path = "bf16_nominal_option_enum_scalar_provenance_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
#[path = "bf16_nominal_option_enum_scalar_provenance_prelude_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use genuine::observe_option_enum_scalar_provenance_before_allocation_for_test_v1;

// Separate allocation continuation; the BeforeAllocation entry is unchanged.
#[path = "bf16_nominal_option_enum_scalar_provenance_allocation_prelude_v1.rs"]
mod option_enum_scalar_provenance_allocation_prelude;
#[cfg(test)]
pub(crate) use option_enum_scalar_provenance_allocation_prelude::observe_option_enum_scalar_provenance_allocation_before_capabilities_for_test_v1;
#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use option_enum_scalar_provenance_allocation_prelude::{BeforeCapabilitiesV1, with_nominal_option_enum_scalar_provenance_allocation_before_capabilities_v1};

#[cfg(test)]
pub(crate) use option_enum_scalar_provenance_allocation_prelude::observe_actual_capability_prefix_for_test_v1;
