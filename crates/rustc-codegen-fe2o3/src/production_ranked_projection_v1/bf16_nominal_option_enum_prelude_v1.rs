//! Additive source-owned enum continuation. Stops before scalar inventory.
//! The original BeforeEnum entry, its debits and old routes remain unchanged.
use super::*;
use fe2o3_mir_model::SemanticEnumPayloadDominancePreparationV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnumPhase {
    Fresh,
    Terminal,
    BeforeScalar,
}

/// Both actual model owners exist outside the checked-call catch/postflight.
struct PendingBeforeScalarV1 {
    phase: EnumPhase,
    options: PendingOptionFirstPreludeV1,
    enumeration: SemanticEnumPayloadDominancePreparationV1,
    enum_invoked: bool,
    failure: Option<Backend>,
}
impl PendingBeforeScalarV1 {
    fn new() -> Self {
        Self {
            phase: EnumPhase::Fresh,
            options: PendingOptionFirstPreludeV1::new(),
            enumeration: SemanticEnumPayloadDominancePreparationV1::new(),
            enum_invoked: false,
            failure: None,
        }
    }
    fn prepare(&mut self, source: &Source<'_>, resources: &mut Prep<'_, '_>) -> BResult<()> {
        let fresh = self.phase == EnumPhase::Fresh && !self.enum_invoked && self.failure.is_none();
        self.phase = EnumPhase::Terminal;
        if !fresh
            || !resources.is_metered()
            || resources.has_denial()
            || resources.original_ledger_v1() != Some(source.ledger)
        {
            return Err(accounting());
        }
        // New wrapper vertices only. The existing private Option preparation
        // retains exactly its original internal call/debit/error order.
        resources.work(32)?;
        self.options.prepare(source, resources)?;
        if self.options.phase != Phase::BeforeEnum {
            return Err(accounting());
        }
        self.enum_invoked = true;
        self.enumeration
            .prepare_into(source.function, source.types, &mut ModelMeter(resources))
            .map_err(model_error)?;
        if resources.has_denial() || self.enumeration.completed().is_none() {
            return Err(accounting());
        }
        self.phase = EnumPhase::BeforeScalar;
        Ok(())
    }
    fn view<'a>(
        &'a self,
        source: &'a Source<'_>,
        resources: &Prep<'_, '_>,
    ) -> BResult<BeforeScalarV1<'a>> {
        if self.phase != EnumPhase::BeforeScalar || !self.enum_invoked {
            return Err(accounting());
        }
        Ok(BeforeScalarV1 {
            options: self.options.view(source, resources)?,
            enumeration: self.enumeration.completed().ok_or_else(accounting)?,
            enum_invoked: self.enum_invoked,
        })
    }
}

/// Immutable same-owner DATA, not scalar/provenance/capability/F2 readiness.
pub(in crate::production_ranked_projection_v1) struct BeforeScalarV1<'a> {
    options: BeforeEnumV1<'a>,
    enumeration: &'a SemanticEnumPayloadDominanceV1,
    enum_invoked: bool,
}
impl BeforeScalarV1<'_> {
    pub(in crate::production_ranked_projection_v1) fn function(&self) -> &SemanticFunctionDeclV1 {
        self.options.function()
    }
    pub(in crate::production_ranked_projection_v1) fn option_producers(
        &self,
    ) -> &[SemanticOptionProducerV1] {
        self.options.producers()
    }
    pub(in crate::production_ranked_projection_v1) fn option_dominance(
        &self,
    ) -> &SemanticOptionDominanceV1 {
        self.options.dominance()
    }
    pub(in crate::production_ranked_projection_v1) fn enum_dominance(
        &self,
    ) -> &SemanticEnumPayloadDominanceV1 {
        self.enumeration
    }
    pub(in crate::production_ranked_projection_v1) fn enum_api_invoked(&self) -> bool {
        self.enum_invoked
    }
}

const ENUM_FRAME_ROWS: usize = 19;
fn enum_frame_rows<R, F>() -> BResult<[usize; ENUM_FRAME_ROWS]> {
    Ok([
        size_of::<PendingBeforeScalarV1>(),
        size_of::<(
            SemanticEnumPayloadDominancePreparationV1,
            EnumPhase,
            bool,
            Option<Backend>,
        )>(),
        size_of::<(
            &mut PendingBeforeScalarV1,
            &Source<'static>,
            &mut Prep<'static, 'static>,
            bool,
            BResult<()>,
        )>(),
        size_of::<(
            BeforeScalarV1<'static>,
            BeforeEnumV1<'static>,
            Option<&SemanticEnumPayloadDominanceV1>,
            &SemanticEnumPayloadDominanceV1,
            BResult<BeforeScalarV1<'static>>,
        )>(),
        size_of::<(
            &PendingBeforeScalarV1,
            &Source<'static>,
            &Prep<'static, 'static>,
            bool,
        )>(),
        size_of::<(
            &BeforeScalarV1<'static>,
            &SemanticFunctionDeclV1,
            &[SemanticOptionProducerV1],
            &SemanticOptionDominanceV1,
            &SemanticEnumPayloadDominanceV1,
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
            &mut PendingBeforeScalarV1,
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
        size_of::<(
            Prep<'static, 'static>,
            ModelMeter<'static, 'static, 'static>,
            &mut usize,
            &mut Budget<'static>,
        )>(),
        size_of::<(F, F, R, R)>(),
        size_of::<(Result<R>, Result<R>, BResult<R>, BResult<R>, BResult<()>)>(),
        size_of::<(Custody, Custody, usize, usize, bool, Result<()>)>(),
        size_of::<(
            Backend,
            Option<Backend>,
            &Backend,
            QueryError,
            SemanticEnumPayloadMeteredErrorV1<Backend>,
        )>(),
        size_of::<(PanicPayload, std::result::Result<Result<R>, PanicPayload>)>(),
        size_of::<(
            [usize; ENUM_FRAME_ROWS],
            [usize; ENUM_FRAME_ROWS],
            BResult<[usize; ENUM_FRAME_ROWS]>,
            std::array::IntoIter<usize, ENUM_FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            BResult<usize>,
        )>(),
        size_of::<(BResult<usize>, usize, usize, Option<usize>, &usize)>(),
        size_of::<(
            &mut SemanticEnumPayloadDominancePreparationV1,
            &SemanticFunctionDeclV1,
            &[SemanticTypeDeclV1],
            &mut ModelMeter<'static, 'static, 'static>,
            BResult<()>,
        )>(),
    ])
}
fn enum_frame<R, F>() -> BResult<usize> {
    // The complete unchanged parent policy pays all reached private Option
    // preparation/getter/custody callees. Every new vertex is additive above.
    enum_frame_rows::<R, F>()?
        .into_iter()
        .try_fold(super::frame::<R, F>()?, |sum, row| {
            sum.checked_add(row).ok_or_else(arithmetic)
        })
}

/// A separate authenticated entry. Never nests an externally owned enum
/// payload inside the older BeforeEnum entry's earlier refund boundary.
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_option_enum_before_scalar_v1<
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
    F: for<'a, 'b, 'm> FnOnce(BeforeScalarV1<'a>, &mut Prep<'b, 'm>) -> BResult<R>,
{
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, move |budget| {
        let before = Custody::take(budget)?;
        let bytes = enum_frame::<R, F>().map_err(query_error)?;
        let mut owned = 0usize;
        PreparationResourcesV1::new(budget, &mut owned)
            .reserve_storage(bytes)
            .map_err(query_error)?;
        let mut pending = PendingBeforeScalarV1::new();
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
                        "Option/enum checked source identity differs",
                    ));
                }
                let semantic = source_owner.semantic_ssa().source_semantic();
                let function = semantic
                    .functions()
                    .get(caller.index() as usize)
                    .ok_or(QueryError::Unavailable("Option/enum source caller absent"))?;
                if semantic.functions().len() != 2
                    || semantic.types().len() > 4096
                    || semantic.callables().len() > 4096
                    || function.blocks().len() > 32
                    || function.locals().len() > 4096
                {
                    return Err(QueryError::Unavailable(
                        "Option/enum source exceeds closed owner profile",
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
        // unchanged boundary. Both preparation owners and saved errors outlive it.
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
#[path = "bf16_nominal_option_enum_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
#[path = "bf16_nominal_option_enum_prelude_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use genuine::observe_option_enum_before_scalar_for_test_v1;
