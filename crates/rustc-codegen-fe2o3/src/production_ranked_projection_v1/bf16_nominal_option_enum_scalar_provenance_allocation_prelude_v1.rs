//! Source-owned Option -> enum -> scalar -> provenance -> allocation continuation.
//! Stops before capability preparation; prior entries and ordinary routes are unchanged.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::RetainedAllocationContractsV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AllocationPhase {
    Fresh,
    Terminal,
    BeforeCapabilities,
}
struct PendingBeforeCapabilitiesV1 {
    phase: AllocationPhase,
    earlier: PendingBeforeAllocationV1,
    allocation: RetainedAllocationContractsV1,
    allocation_invoked: bool,
    failure: Option<Backend>,
}
impl PendingBeforeCapabilitiesV1 {
    fn new() -> Self {
        Self {
            phase: AllocationPhase::Fresh,
            earlier: PendingBeforeAllocationV1::new(),
            allocation: RetainedAllocationContractsV1::new(),
            allocation_invoked: false,
            failure: None,
        }
    }
    fn prepare(&mut self, source: &Source<'_>, resources: &mut Prep<'_, '_>) -> BResult<()> {
        let fresh = self.phase == AllocationPhase::Fresh
            && !self.allocation_invoked
            && self.failure.is_none();
        self.phase = AllocationPhase::Terminal;
        if !fresh
            || !resources.is_metered()
            || resources.has_denial()
            || resources.original_ledger_v1() != Some(source.ledger)
        {
            return Err(accounting());
        }
        resources.work(32)?;
        self.earlier.prepare(source, resources)?;
        if self.earlier.phase != ProvenancePhase::BeforeAllocation {
            return Err(accounting());
        }
        // These exact origin rows stay owned by the preceding provenance stage;
        // both owners and the actual source loan outlive true checked postflight.
        let earlier = self.earlier.view(source, resources)?;
        let provenance = earlier.provenance();
        self.allocation_invoked = true;
        self.allocation.prepare_into(
            source.types,
            source.function,
            &provenance.allocation_origins,
            resources,
        )?;
        self.allocation.completed_for(
            source.types,
            source.function,
            &provenance.allocation_origins,
            resources,
        )?;
        if resources.has_denial() {
            return Err(accounting());
        }
        self.phase = AllocationPhase::BeforeCapabilities;
        Ok(())
    }
    fn view<'a>(
        &'a self,
        source: &'a Source<'_>,
        resources: &Prep<'_, '_>,
    ) -> BResult<BeforeCapabilitiesV1<'a>> {
        if self.phase != AllocationPhase::BeforeCapabilities || !self.allocation_invoked {
            return Err(accounting());
        }
        let earlier = self.earlier.view(source, resources)?;
        let provenance = earlier.provenance();
        let allocation = self.allocation.completed_for(
            source.types,
            source.function,
            &provenance.allocation_origins,
            resources,
        )?;
        Ok(BeforeCapabilitiesV1 {
            earlier,
            allocation,
            allocation_invoked: self.allocation_invoked,
        })
    }
}

/// Immutable completed pre-capability DATA, not capability authority, later
/// argument writers, F2, or ordinary-route readiness.
pub(in crate::production_ranked_projection_v1) struct BeforeCapabilitiesV1<'a> {
    earlier: BeforeAllocationV1<'a>,
    allocation: &'a [Option<AllocationContractV1>],
    allocation_invoked: bool,
}
impl BeforeCapabilitiesV1<'_> {
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
        self.earlier.provenance()
    }
    pub(in crate::production_ranked_projection_v1) fn provenance_api_invoked(&self) -> bool {
        self.earlier.provenance_api_invoked()
    }
    pub(in crate::production_ranked_projection_v1) fn allocation_contracts(
        &self,
    ) -> &[Option<AllocationContractV1>] {
        self.allocation
    }
    pub(in crate::production_ranked_projection_v1) fn allocation_api_invoked(&self) -> bool {
        self.allocation_invoked
    }
}

const ALLOCATION_FRAME_ROWS: usize = 23;
fn allocation_frame_rows<R, F>() -> BResult<[usize; ALLOCATION_FRAME_ROWS]> {
    Ok([
        size_of::<PendingBeforeCapabilitiesV1>(),
        size_of::<(
            PendingBeforeAllocationV1,
            RetainedAllocationContractsV1,
            AllocationPhase,
            bool,
            Option<Backend>,
        )>(),
        size_of::<(
            &mut PendingBeforeCapabilitiesV1,
            &Source<'static>,
            &mut Prep<'static, 'static>,
            bool,
            BResult<()>,
        )>(),
        size_of::<(
            BeforeCapabilitiesV1<'static>,
            BeforeAllocationV1<'static>,
            &LocalProvenanceV1,
            &[Option<AllocationContractV1>],
            BResult<&[Option<AllocationContractV1>]>,
            BResult<BeforeCapabilitiesV1<'static>>,
        )>(),
        size_of::<(
            &PendingBeforeCapabilitiesV1,
            &Source<'static>,
            &Prep<'static, 'static>,
            bool,
        )>(),
        size_of::<(
            &BeforeCapabilitiesV1<'static>,
            &SemanticFunctionDeclV1,
            &[SemanticOptionProducerV1],
            &SemanticOptionDominanceV1,
            &SemanticEnumPayloadDominanceV1,
            &AssertionDefinitionInventoryV1,
            &LocalProvenanceV1,
            &[Option<AllocationContractV1>],
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
            &mut PendingBeforeCapabilitiesV1,
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
            [usize; ALLOCATION_FRAME_ROWS],
            [usize; ALLOCATION_FRAME_ROWS],
            BResult<[usize; ALLOCATION_FRAME_ROWS]>,
            std::array::IntoIter<usize, ALLOCATION_FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            BResult<usize>,
        )>(),
        size_of::<(BResult<usize>, usize, usize, Option<usize>, &usize)>(),
        size_of::<(
            &mut RetainedAllocationContractsV1,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &Vec<Option<u32>>,
            &[Option<u32>],
            &mut Prep<'static, 'static>,
            BResult<()>,
        )>(),
        size_of::<(
            &RetainedAllocationContractsV1,
            &[SemanticTypeDeclV1],
            &SemanticFunctionDeclV1,
            &Vec<Option<u32>>,
            &[Option<u32>],
            &Prep<'static, 'static>,
            BResult<&[Option<AllocationContractV1>]>,
        )>(),
        size_of::<(
            &mut PendingBeforeAllocationV1,
            &Source<'static>,
            &mut Prep<'static, 'static>,
            BResult<()>,
            &PendingBeforeAllocationV1,
            BResult<BeforeAllocationV1<'static>>,
        )>(),
        size_of::<(
            BeforeAllocationV1<'static>,
            &BeforeAllocationV1<'static>,
            &LocalProvenanceV1,
            &Vec<Option<u32>>,
            &[Option<u32>],
        )>(),
        size_of::<(
            BeforeAllocationV1<'static>,
            &BeforeAllocationV1<'static>,
            &LocalProvenanceV1,
            &Vec<Option<u32>>,
            &[Option<u32>],
            &[Option<AllocationContractV1>],
        )>(),
    ])
}
fn allocation_frame<R, F>() -> BResult<usize> {
    // Preserve prior checkpoint policy; the retained allocation component charges
    // its own additional work/frame and complete unchanged donor policy in place.
    allocation_frame_rows::<R, F>()?
        .into_iter()
        .try_fold(super::provenance_frame::<R, F>()?, |sum, row| {
            sum.checked_add(row).ok_or_else(arithmetic)
        })
}

/// A separate authenticated entry. Never nests an externally owned allocation
/// payload inside the older BeforeAllocation entry's earlier refund boundary.
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_option_enum_scalar_provenance_allocation_before_capabilities_v1<
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
    F: for<'a, 'b, 'm> FnOnce(BeforeCapabilitiesV1<'a>, &mut Prep<'b, 'm>) -> BResult<R>,
{
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, move |budget| {
        let before = Custody::take(budget)?;
        let bytes = allocation_frame::<R, F>().map_err(query_error)?;
        let mut owned = 0usize;
        PreparationResourcesV1::new(budget, &mut owned)
            .reserve_storage(bytes)
            .map_err(query_error)?;
        let mut pending = PendingBeforeCapabilitiesV1::new();
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
                        "Option/enum/scalar/provenance/allocation checked source identity differs",
                    ));
                }
                let semantic = source_owner.semantic_ssa().source_semantic();
                let function = semantic
                    .functions()
                    .get(caller.index() as usize)
                    .ok_or(QueryError::Unavailable("Option/enum/scalar/provenance/allocation source caller absent"))?;
                if semantic.functions().len() != 2
                    || semantic.types().len() > 4096
                    || semantic.callables().len() > 4096
                    || function.blocks().len() > 32
                    || function.locals().len() > 4096
                {
                    return Err(QueryError::Unavailable(
                        "Option/enum/scalar/provenance/allocation source exceeds closed owner profile",
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
#[path = "bf16_nominal_option_enum_scalar_provenance_allocation_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
#[path = "bf16_nominal_option_enum_scalar_provenance_allocation_prelude_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use genuine::observe_option_enum_scalar_provenance_allocation_before_capabilities_for_test_v1;

#[cfg(test)]
pub(crate) use genuine::observe_actual_capability_prefix_for_test_v1;
