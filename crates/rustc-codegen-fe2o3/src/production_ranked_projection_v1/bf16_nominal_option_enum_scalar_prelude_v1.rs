//! Source-owned Option -> enum -> scalar continuation, stopping before provenance.
//! Earlier checkpoint entries, algorithms and ordinary routes are unchanged.
use super::*;
use crate::production_ranked_projection_v1::bf16_nominal_source_algorithms_v1::RetainedScalarInventoryV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarPhase {
    Fresh,
    Terminal,
    BeforeProvenance,
}

/// The complete earlier owners and scalar partial arrays remain outside the
/// genuine checked-call catch/postflight, with one accepted-credit counter.
struct PendingBeforeProvenanceV1 {
    phase: ScalarPhase,
    earlier: PendingBeforeScalarV1,
    scalar: RetainedScalarInventoryV1,
    scalar_invoked: bool,
    failure: Option<Backend>,
}
impl PendingBeforeProvenanceV1 {
    fn new() -> Self {
        Self {
            phase: ScalarPhase::Fresh,
            earlier: PendingBeforeScalarV1::new(),
            scalar: RetainedScalarInventoryV1::new(),
            scalar_invoked: false,
            failure: None,
        }
    }
    fn prepare(&mut self, source: &Source<'_>, resources: &mut Prep<'_, '_>) -> BResult<()> {
        let fresh =
            self.phase == ScalarPhase::Fresh && !self.scalar_invoked && self.failure.is_none();
        self.phase = ScalarPhase::Terminal;
        if !fresh
            || !resources.is_metered()
            || resources.has_denial()
            || resources.original_ledger_v1() != Some(source.ledger)
        {
            return Err(accounting());
        }
        resources.work(32)?;
        // Private preparation, NOT the earlier factory or its refund boundary.
        self.earlier.prepare(source, resources)?;
        if self.earlier.phase != EnumPhase::BeforeScalar {
            return Err(accounting());
        }
        self.scalar_invoked = true;
        self.scalar.prepare_into(source.function, resources)?;
        self.scalar.completed_for(source.function, resources)?;
        if resources.has_denial() {
            return Err(accounting());
        }
        self.phase = ScalarPhase::BeforeProvenance;
        Ok(())
    }
    fn view<'a>(
        &'a self,
        source: &'a Source<'_>,
        resources: &Prep<'_, '_>,
    ) -> BResult<BeforeProvenanceV1<'a>> {
        if self.phase != ScalarPhase::BeforeProvenance || !self.scalar_invoked {
            return Err(accounting());
        }
        Ok(BeforeProvenanceV1 {
            earlier: self.earlier.view(source, resources)?,
            scalar: self.scalar.completed_for(source.function, resources)?,
            scalar_invoked: self.scalar_invoked,
        })
    }
}

/// Immutable actual source DATA. This is not provenance/allocation/capability
/// completeness and confers no F2 or ordinary-route readiness.
pub(in crate::production_ranked_projection_v1) struct BeforeProvenanceV1<'a> {
    earlier: BeforeScalarV1<'a>,
    scalar: &'a AssertionDefinitionInventoryV1,
    scalar_invoked: bool,
}
impl BeforeProvenanceV1<'_> {
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
        self.scalar
    }
    pub(in crate::production_ranked_projection_v1) fn scalar_api_invoked(&self) -> bool {
        self.scalar_invoked
    }
}

const SCALAR_FRAME_ROWS: usize = 21;
fn scalar_frame_rows<R, F>() -> BResult<[usize; SCALAR_FRAME_ROWS]> {
    Ok([
        size_of::<PendingBeforeProvenanceV1>(),
        size_of::<(
            PendingBeforeScalarV1,
            RetainedScalarInventoryV1,
            ScalarPhase,
            bool,
            Option<Backend>,
        )>(),
        size_of::<(
            &mut PendingBeforeProvenanceV1,
            &Source<'static>,
            &mut Prep<'static, 'static>,
            bool,
            BResult<()>,
        )>(),
        size_of::<(
            BeforeProvenanceV1<'static>,
            BeforeScalarV1<'static>,
            &AssertionDefinitionInventoryV1,
            BResult<&AssertionDefinitionInventoryV1>,
            BResult<BeforeProvenanceV1<'static>>,
        )>(),
        size_of::<(
            &PendingBeforeProvenanceV1,
            &Source<'static>,
            &Prep<'static, 'static>,
            bool,
        )>(),
        size_of::<(
            &BeforeProvenanceV1<'static>,
            &SemanticFunctionDeclV1,
            &[SemanticOptionProducerV1],
            &SemanticOptionDominanceV1,
            &SemanticEnumPayloadDominanceV1,
            &AssertionDefinitionInventoryV1,
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
            &mut PendingBeforeProvenanceV1,
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
            [usize; SCALAR_FRAME_ROWS],
            [usize; SCALAR_FRAME_ROWS],
            BResult<[usize; SCALAR_FRAME_ROWS]>,
            std::array::IntoIter<usize, SCALAR_FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            BResult<usize>,
        )>(),
        size_of::<(BResult<usize>, usize, usize, Option<usize>, &usize)>(),
        size_of::<(
            &mut RetainedScalarInventoryV1,
            &SemanticFunctionDeclV1,
            &mut Prep<'static, 'static>,
            BResult<()>,
        )>(),
        size_of::<(
            &RetainedScalarInventoryV1,
            &SemanticFunctionDeclV1,
            &Prep<'static, 'static>,
            BResult<&AssertionDefinitionInventoryV1>,
        )>(),
        size_of::<(
            &mut PendingBeforeScalarV1,
            &Source<'static>,
            &mut Prep<'static, 'static>,
            BResult<()>,
            &PendingBeforeScalarV1,
            BResult<BeforeScalarV1<'static>>,
        )>(),
    ])
}
fn scalar_frame<R, F>() -> BResult<usize> {
    // Entire existing Option/enum policy is retained. The scalar component pays
    // its own exact 17-row policy and original header inside prepare_into.
    scalar_frame_rows::<R, F>()?
        .into_iter()
        .try_fold(super::enum_frame::<R, F>()?, |sum, row| {
            sum.checked_add(row).ok_or_else(arithmetic)
        })
}

/// A separate authenticated entry. Never nests an externally owned scalar
/// payload inside the older BeforeScalar entry's earlier refund boundary.
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_option_enum_scalar_before_provenance_v1<
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
    F: for<'a, 'b, 'm> FnOnce(BeforeProvenanceV1<'a>, &mut Prep<'b, 'm>) -> BResult<R>,
{
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, move |budget| {
        let before = Custody::take(budget)?;
        let bytes = scalar_frame::<R, F>().map_err(query_error)?;
        let mut owned = 0usize;
        PreparationResourcesV1::new(budget, &mut owned)
            .reserve_storage(bytes)
            .map_err(query_error)?;
        let mut pending = PendingBeforeProvenanceV1::new();
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
                        "Option/enum/scalar checked source identity differs",
                    ));
                }
                let semantic = source_owner.semantic_ssa().source_semantic();
                let function = semantic.functions().get(caller.index() as usize).ok_or(
                    QueryError::Unavailable("Option/enum/scalar source caller absent"),
                )?;
                if semantic.functions().len() != 2
                    || semantic.types().len() > 4096
                    || semantic.callables().len() > 4096
                    || function.blocks().len() > 32
                    || function.locals().len() > 4096
                {
                    return Err(QueryError::Unavailable(
                        "Option/enum/scalar source exceeds closed owner profile",
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
#[path = "bf16_nominal_option_enum_scalar_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
#[path = "bf16_nominal_option_enum_scalar_prelude_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use genuine::observe_option_enum_scalar_before_provenance_for_test_v1;

// Separate provenance continuation; the BeforeProvenance entry is unchanged.
#[path = "bf16_nominal_option_enum_scalar_provenance_prelude_v1.rs"]
mod option_enum_scalar_provenance_prelude;
#[cfg(test)]
pub(crate) use option_enum_scalar_provenance_prelude::observe_option_enum_scalar_provenance_before_allocation_for_test_v1;
#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use option_enum_scalar_provenance_prelude::{
    BeforeAllocationV1, with_nominal_option_enum_scalar_provenance_before_allocation_v1,
};

#[cfg(test)]
pub(crate) use option_enum_scalar_provenance_prelude::observe_option_enum_scalar_provenance_allocation_before_capabilities_for_test_v1;
#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use option_enum_scalar_provenance_prelude::{
    BeforeCapabilitiesV1,
    with_nominal_option_enum_scalar_provenance_allocation_before_capabilities_v1,
};
