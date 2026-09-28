//! Source-owned Option-first checkpoint. No enum/intrinsic/bounds/F2 readiness.
//! The old dense-first rich factory and all ordinary routes are unchanged.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
use fe2o3_lower_mir_kernel::CheckedBf16NominalCallV1;
use fe2o3_mir_model::{
    SemanticOptionDominancePreparationV1, SemanticOptionProducerPreparationV1,
    SemanticOptionProducerV1,
};

type Prep<'b, 'w> = PreparationResourcesV1<'b, 'w>;
type Backend = ProductionRankedProjectionErrorV1;
type BResult<T> = std::result::Result<T, Backend>;
type Ledger = (usize, CanonicalKernelIrWorkLedgerIdentityV1);
type PanicPayload = Box<dyn std::any::Any + Send>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    BeforeEnum,
}

/// Original intrinsic destination order. These remain empty destinations, not
/// completed producers; their physical initial allocations survive every exit.
struct InitialDestinations {
    indices: Vec<Option<ProjectedDisjointIndexV1>>,
    leaders: Vec<Option<ProjectedGridLeaderV1>>,
    predicates: Vec<Option<GuardPredicateV1>>,
    edges: Vec<Vec<CapabilityEdgeV1>>,
    stores: Vec<PendingEnumPayloadStoreV1>,
    loads: Vec<PendingEnumPayloadLoadV1>,
}
impl InitialDestinations {
    const fn new() -> Self {
        Self {
            indices: Vec::new(),
            leaders: Vec::new(),
            predicates: Vec::new(),
            edges: Vec::new(),
            stores: Vec::new(),
            loads: Vec::new(),
        }
    }
    fn prepare(&mut self, locals: usize, resources: &mut Prep<'_, '_>) -> BResult<()> {
        // Exact original ordering after retired-intrinsic rejection. Initial
        // payloads are attached before reserve/resize, including partial failure.
        fill(&mut self.indices, locals, None, resources)?;
        fill(&mut self.leaders, locals, None, resources)?;
        fill(&mut self.predicates, locals, None, resources)?;
        resources.work(locals)?;
        resources.reserve(&mut self.edges, locals)?;
        self.edges.resize_with(locals, Vec::new);
        Ok(())
    }
}
fn fill<T: Clone>(
    rows: &mut Vec<T>,
    count: usize,
    value: T,
    resources: &mut Prep<'_, '_>,
) -> BResult<()> {
    resources.work(count)?;
    resources.reserve(rows, count)?;
    rows.resize(count, value);
    Ok(())
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct SourceIdentity {
    function: usize,
    callables: (usize, usize),
    types: (usize, usize),
}
struct Source<'a> {
    function: &'a SemanticFunctionDeclV1,
    callables: &'a [SemanticCallableDeclV1],
    types: &'a [SemanticTypeDeclV1],
    ledger: Ledger,
}
impl Source<'_> {
    fn identity(&self) -> SourceIdentity {
        SourceIdentity {
            function: self.function as *const SemanticFunctionDeclV1 as usize,
            callables: (self.callables.as_ptr() as usize, self.callables.len()),
            types: (self.types.as_ptr() as usize, self.types.len()),
        }
    }
}

/// Not publicly constructible, resumable, replaceable or transferable as proof.
struct PendingOptionFirstPreludeV1 {
    phase: Phase,
    source: Option<SourceIdentity>,
    ledger: Option<Ledger>,
    initial: InitialDestinations,
    producers: SemanticOptionProducerPreparationV1,
    dominance: SemanticOptionDominancePreparationV1,
    failure: Option<Backend>,
}
impl PendingOptionFirstPreludeV1 {
    fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            source: None,
            ledger: None,
            initial: InitialDestinations::new(),
            producers: SemanticOptionProducerPreparationV1::new(),
            dominance: SemanticOptionDominancePreparationV1::new(),
            failure: None,
        }
    }
    fn prepare(&mut self, source: &Source<'_>, resources: &mut Prep<'_, '_>) -> BResult<()> {
        let fresh = self.phase == Phase::Fresh
            && self.source.is_none()
            && self.ledger.is_none()
            && self.failure.is_none();
        self.phase = Phase::Terminal;
        if !fresh
            || !resources.is_metered()
            || resources.has_denial()
            || resources.original_ledger_v1() != Some(source.ledger)
        {
            return Err(resource(Resource::Accounting));
        }
        // New bounded metadata/getter/result/cleanup prefix, before source scans.
        resources.work(64)?;
        self.source = Some(source.identity());
        self.ledger = Some(source.ledger);
        // Two original finite roster scans, before any Option/enum analysis.
        resources.work(
            source
                .callables
                .len()
                .checked_mul(2)
                .ok_or_else(arithmetic)?,
        )?;
        reject_retired_production_intrinsics_v1(source.callables)?;
        self.initial
            .prepare(source.function.locals().len(), resources)?;
        self.producers
            .prepare_into(
                source.function,
                source.callables,
                &mut ModelMeter(resources),
            )
            .map_err(model_error)?;
        let producers = self.producers.completed().ok_or_else(accounting)?;
        self.dominance
            .prepare_into(source.function, producers, &mut ModelMeter(resources))
            .map_err(model_error)?;
        if resources.has_denial() {
            return Err(accounting());
        }
        self.phase = Phase::BeforeEnum;
        Ok(())
    }
    fn view<'a>(
        &'a self,
        source: &'a Source<'_>,
        resources: &Prep<'_, '_>,
    ) -> BResult<BeforeEnumV1<'a>> {
        if self.phase != Phase::BeforeEnum
            || self.source != Some(source.identity())
            || self.ledger != Some(source.ledger)
            || resources.original_ledger_v1() != self.ledger
            || resources.has_denial()
        {
            return Err(accounting());
        }
        Ok(BeforeEnumV1 {
            function: source.function,
            callables: source.callables,
            types: source.types,
            ledger: source.ledger,
            producers: self.producers.completed().ok_or_else(accounting)?,
            dominance: self.dominance.completed().ok_or_else(accounting)?,
        })
    }
}

/// A lexical DATA view, deliberately not a rich source table, F2 token or recipe.
pub(in crate::production_ranked_projection_v1) struct BeforeEnumV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    callables: &'a [SemanticCallableDeclV1],
    types: &'a [SemanticTypeDeclV1],
    ledger: Ledger,
    producers: &'a [SemanticOptionProducerV1],
    dominance: &'a SemanticOptionDominanceV1,
}
impl BeforeEnumV1<'_> {
    pub(in crate::production_ranked_projection_v1) fn function(&self) -> &SemanticFunctionDeclV1 {
        self.function
    }
    pub(in crate::production_ranked_projection_v1) fn producers(
        &self,
    ) -> &[SemanticOptionProducerV1] {
        self.producers
    }
    pub(in crate::production_ranked_projection_v1) fn dominance(
        &self,
    ) -> &SemanticOptionDominanceV1 {
        self.dominance
    }
}

#[derive(Clone, Copy)]
struct Custody {
    ledger: Ledger,
    floor: usize,
    work: usize,
    peak: usize,
}
impl Custody {
    fn take(budget: &Budget<'_>) -> Result<Self> {
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            return Err(Resource::Accounting.into());
        }
        Ok(Self {
            ledger: (
                budget as *const Budget<'_> as usize,
                budget.work_ledger_identity_v1(),
            ),
            floor: budget.storage(),
            work: budget.work(),
            peak: budget.peak_storage(),
        })
    }
    fn check(self, budget: &Budget<'_>, owned: usize) -> Result<()> {
        let floor = self.floor.checked_add(owned).ok_or(Resource::Arithmetic)?;
        if self.ledger
            != (
                budget as *const Budget<'_> as usize,
                budget.work_ledger_identity_v1(),
            )
            || budget.storage() < floor
            || budget.work() < self.work
            || budget.peak_storage() < self.peak
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}
fn accounting() -> Backend {
    resource(Resource::Accounting)
}
fn arithmetic() -> Backend {
    resource(Resource::Arithmetic)
}
fn saved_query_error(error: &Backend) -> QueryError {
    match error {
        Backend::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error)) => {
            QueryError::Resource(*error)
        }
        Backend::Unsupported(detail) | Backend::Incomplete(detail) => {
            QueryError::Unavailable(detail)
        }
        _ => QueryError::Unavailable("Option-first callback returned an owning backend error"),
    }
}

const FRAME_ROWS: usize = 24;
fn frame_rows<R, F>() -> BResult<[usize; FRAME_ROWS]> {
    Ok([
        // Every new source-owned vertex is additive to retained model headers.
        size_of::<PendingOptionFirstPreludeV1>(),
        size_of::<(
            InitialDestinations,
            SemanticOptionProducerPreparationV1,
            SemanticOptionDominancePreparationV1,
        )>(),
        size_of::<(F, F)>(),
        size_of::<(R, R)>(),
        size_of::<(Result<R>, Result<R>)>(),
        size_of::<(BResult<R>, BResult<R>)>(),
        size_of::<(
            Custody,
            Custody,
            usize,
            usize,
            bool,
            Option<usize>,
            Result<()>,
        )>(),
        size_of::<(
            Source<'static>,
            SourceIdentity,
            Option<SourceIdentity>,
            Ledger,
            Option<Ledger>,
        )>(),
        size_of::<(
            BeforeEnumV1<'static>,
            &PendingOptionFirstPreludeV1,
            &Source<'static>,
            &Prep<'static, 'static>,
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
            &mut PendingOptionFirstPreludeV1,
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
        size_of::<(
            Prep<'static, 'static>,
            ModelMeter<'static, 'static, 'static>,
            &mut usize,
            &mut Budget<'static>,
        )>(),
        size_of::<(
            &mut PendingOptionFirstPreludeV1,
            &Source<'static>,
            &mut Prep<'static, 'static>,
            bool,
            BResult<()>,
        )>(),
        size_of::<(
            &mut InitialDestinations,
            usize,
            &mut Prep<'static, 'static>,
            BResult<()>,
        )>(),
        fill_frame::<Option<ProjectedDisjointIndexV1>>(),
        fill_frame::<Option<ProjectedGridLeaderV1>>(),
        fill_frame::<Option<GuardPredicateV1>>(),
        size_of::<(
            &mut Vec<Vec<CapabilityEdgeV1>>,
            usize,
            &mut Prep<'static, 'static>,
            Vec<CapabilityEdgeV1>,
            Option<usize>,
            BResult<()>,
        )>(),
        size_of::<(
            Option<&[SemanticOptionProducerV1]>,
            &[SemanticOptionProducerV1],
            Option<&SemanticOptionDominanceV1>,
            &SemanticOptionDominanceV1,
            BResult<BeforeEnumV1<'static>>,
        )>(),
        size_of::<(
            Backend,
            Option<Backend>,
            &Backend,
            QueryError,
            SemanticEnumPayloadMeteredErrorV1<Backend>,
            BResult<()>,
        )>(),
        size_of::<(PanicPayload, std::result::Result<Result<R>, PanicPayload>)>(),
        size_of::<(
            &[SemanticCallableDeclV1],
            std::slice::Iter<'static, SemanticCallableDeclV1>,
            bool,
            usize,
            Option<usize>,
            BResult<()>,
        )>(),
        size_of::<(
            [usize; FRAME_ROWS],
            [usize; FRAME_ROWS],
            BResult<[usize; FRAME_ROWS]>,
            std::array::IntoIter<usize, FRAME_ROWS>,
            usize,
            usize,
            Option<usize>,
            BResult<usize>,
        )>(),
    ])
}
fn fill_frame<T>() -> usize {
    // Includes the unchanged PreparationResources::reserve instantiation and
    // fallible resize admission; not allocator internals or native stack bytes.
    size_of::<(
        &mut Vec<T>,
        usize,
        T,
        &mut Prep<'static, 'static>,
        usize,
        Option<usize>,
        BResult<()>,
        BResult<()>,
    )>()
}
fn frame<R, F>() -> BResult<usize> {
    frame_rows::<R, F>()?
        .into_iter()
        .try_fold(0usize, |sum, row| {
            sum.checked_add(row).ok_or_else(arithmetic)
        })
}

/// Separate authenticated entry. No caller can construct Source or the pending
/// owner, and the old dense-first factories are never invoked here. Callback
/// results cannot carry a borrowed source, proof, or owning payload out.
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_option_first_before_enum_v1<
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
    F: for<'a, 'b, 'm> FnOnce(BeforeEnumV1<'a>, &mut Prep<'b, 'm>) -> BResult<R>,
{
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, move |budget| {
        let before = Custody::take(budget)?;
        let bytes = frame::<R, F>().map_err(query_error)?;
        let mut owned = 0usize;
        PreparationResourcesV1::new(budget, &mut owned)
            .reserve_storage(bytes)
            .map_err(query_error)?;
        // All model owners remain OUTSIDE the checked query's catch/postflight.
        let mut pending = PendingOptionFirstPreludeV1::new();
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
                        "Option-first checked source identity differs",
                    ));
                }
                let semantic = source_owner.semantic_ssa().source_semantic();
                let function = semantic
                    .functions()
                    .get(caller.index() as usize)
                    .ok_or(QueryError::Unavailable("Option-first source caller absent"))?;
                if semantic.functions().len() != 2
                    || semantic.types().len() > 4096
                    || semantic.callables().len() > 4096
                    || function.blocks().len() > 32
                    || function.locals().len() > 4096
                {
                    return Err(QueryError::Unavailable(
                        "Option-first source exceeds closed owner profile",
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
        // The unchanged checked wrapper converts/drops a panic payload before
        // its own refund. Our actual model/destination/error owners survive that
        // postflight. This does not claim physical panic-payload retention.
        let custody = before.check(budget, owned);
        let denied = budget.failed_work().is_some() || budget.failed_storage().is_some();
        drop(pending);
        custody?;
        // Valid-custody cleanup also refunds accepted credits after sticky
        // denial, without resetting the original first-denial/work history.
        budget.release_storage(owned)?;
        match result {
            Ok(_) if denied => Err(Resource::Accounting.into()),
            other => other,
        }
    })
}

#[cfg(test)]
#[path = "bf16_nominal_option_first_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
#[path = "bf16_nominal_option_first_prelude_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use genuine::observe_option_first_before_enum_for_test_v1;

// Separate source-ordered enum continuation; the BeforeEnum entry is unchanged.
#[path = "bf16_nominal_option_enum_prelude_v1.rs"]
mod option_enum_prelude;
#[cfg(test)]
pub(crate) use option_enum_prelude::observe_option_enum_before_scalar_for_test_v1;
#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use option_enum_prelude::{
    BeforeScalarV1, with_nominal_option_enum_before_scalar_v1,
};

#[cfg(test)]
pub(crate) use option_enum_prelude::observe_option_enum_scalar_before_provenance_for_test_v1;
#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use option_enum_prelude::{
    BeforeProvenanceV1, with_nominal_option_enum_scalar_before_provenance_v1,
};

#[cfg(test)]
pub(crate) use option_enum_prelude::observe_option_enum_scalar_provenance_before_allocation_for_test_v1;
#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use option_enum_prelude::{
    BeforeAllocationV1, with_nominal_option_enum_scalar_provenance_before_allocation_v1,
};
