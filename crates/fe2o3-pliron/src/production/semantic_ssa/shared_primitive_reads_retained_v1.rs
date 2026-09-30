//! Source/SSA-bound retained Shared construction. The existing view/query stays unchanged.
use super::*;
use adapter::shared_primitive_v29::{RetainedAliasErrorV1, RetainedSharedEngineV1};
use fe2o3_mir_model::semantic_mir_v1 as model;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}
#[derive(Clone, Copy, Eq, PartialEq)]
struct Snapshot {
    slot: usize,
    ledger: Ledger,
    counter: usize,
    owned: usize,
    storage: usize,
    work: usize,
    peak: usize,
}
fn snapshot(budget: &Budget<'_>, owned: &usize) -> Snapshot {
    Snapshot {
        slot: budget as *const Budget<'_> as usize,
        ledger: budget.work_ledger_identity_v1(),
        counter: owned as *const usize as usize,
        owned: *owned,
        storage: budget.storage(),
        work: budget.work(),
        peak: budget.peak_storage(),
    }
}

/// Move-only preparation owner. Construction retains real owner/plan references;
/// callers retain this object through postflight and drop it before refunding.
pub struct ProductionSemanticSharedReadsPreparationV1<'s> {
    phase: Phase,
    owner: Option<&'s ProductionSemanticSsaOwnerV1>,
    function: Option<SemanticFunctionIdV1>,
    semantic: Option<&'s AdmittedInertSemanticMirV1>,
    declaration: Option<&'s SemanticFunctionDeclV1>,
    plan: Option<&'s SsaConstructionPlanV1>,
    entry: Option<Snapshot>,
    held: Option<Snapshot>,
    failure: Option<Resource>,
    dimensions: Option<Option<(usize, usize)>>,
    engine: Option<RetainedSharedEngineV1<'s>>,
    promoted: Vec<bool>,
    rows: RetainedSharedRowsV1,
    view: Option<ProductionSemanticSharedReadsV1<'s>>,
    scratch: usize,
    engine_start: usize,
    refunded: usize,
}
impl<'s> ProductionSemanticSharedReadsPreparationV1<'s> {
    pub fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            owner: None,
            function: None,
            semantic: None,
            declaration: None,
            plan: None,
            entry: None,
            held: None,
            failure: None,
            dimensions: None,
            engine: None,
            promoted: Vec::new(),
            rows: RetainedSharedRowsV1 {
                rows: Vec::new(),
                transferred: false,
            },
            view: None,
            scratch: 0,
            engine_start: 0,
            refunded: 0,
        }
    }
    /// Fixed operation on the caller's original physical ledger/counter pair.
    /// No admitted semantic owner, plan, source dimensions or row can be supplied separately.
    pub fn prepare_into(
        &mut self,
        owner: &'s ProductionSemanticSsaOwnerV1,
        function: SemanticFunctionIdV1,
        budget: &mut Budget<'_>,
        owned: &mut usize,
    ) -> R<()> {
        if self.phase != Phase::Fresh {
            return Err(self
                .failure
                .map(Failure::Resource)
                .unwrap_or(Failure::Binding));
        }
        self.phase = Phase::Terminal;
        self.owner = Some(owner);
        self.function = Some(function);
        self.entry = Some(snapshot(budget, owned));
        let result = self.construct(owner, function, budget, owned);
        if let Err(Failure::Resource(error)) = &result {
            self.failure.get_or_insert(*error);
        }
        result
    }
    fn construct(
        &mut self,
        owner: &'s ProductionSemanticSsaOwnerV1,
        function: SemanticFunctionIdV1,
        budget: &mut Budget<'_>,
        owned: &mut usize,
    ) -> R<()> {
        if *owned > budget.storage()
            || budget.failed_work().is_some()
            || budget.failed_storage().is_some()
        {
            return Err(Resource::Accounting.into());
        }
        // Original constructor headers still precede its work8. The new typed
        // retained-owner frame is a separate, explicit storage-only prefix.
        reserve(budget, owned, headers()?)?;
        reserve(budget, owned, frame()?)?;
        budget.charge_work(8)?;
        let semantic = owner.source_semantic();
        self.semantic = Some(semantic);
        let declaration = semantic
            .functions()
            .get(function.index() as usize)
            .ok_or(Failure::Binding)?;
        self.declaration = Some(declaration);
        let plan = owner
            .plan_for_function(function)
            .ok_or(Failure::Binding)?
            .plan();
        self.plan = Some(plan);
        let mut meter = Meter {
            budget,
            failure: None,
        };
        let analyzed = (|| {
            if !adapter::shared_primitive_v29::has_candidates(
                declaration,
                semantic.types(),
                &mut meter,
            )? {
                return Ok(None);
            }
            nominal_reference_effects_v29::scan_size(declaration, 0, &mut meter).map(Some)
        })();
        let dimensions = analyzed.map_err(|error| {
            meter
                .failure
                .map(Failure::Resource)
                .unwrap_or(Failure::Analysis(error))
        })?;
        self.dimensions = Some(dimensions);
        let budget = meter.budget;
        if let Some((units, candidates)) = dimensions {
            self.scratch = sum(
                product(
                    sum(
                        sum(128, sum(product(units, 96)?, product(candidates, 64)?)?)?,
                        adapter::shared_primitive_v29::liveness_scratch_words(declaration)
                            .map_err(Failure::Analysis)?,
                    )?,
                    size_of::<usize>(),
                )?,
                product(declaration.locals().len(), size_of::<bool>())?,
            )?;
            self.engine_start = *owned;
            self.engine = Some(RetainedSharedEngineV1::new());
            self.engine
                .as_mut()
                .ok_or(Failure::Binding)?
                .prepare_observer_into(
                    declaration,
                    semantic.types(),
                    self.scratch,
                    units,
                    budget,
                    owned,
                )
                .map_err(engine_error)?;
            self.engine
                .as_mut()
                .ok_or(Failure::Binding)?
                .analyze_into(declaration, semantic.types(), units, budget, owned)
                .map_err(engine_error)?;
            budget.charge_work(sum(
                declaration.locals().len(),
                plan.promoted_variables().len(),
            )?)?;
            // The original base mask credit is inside scratch. Capacity stays
            // attached before either allocation failure or excess-credit denial.
            self.promoted
                .try_reserve_exact(declaration.locals().len())
                .map_err(|_| Resource::Allocation)?;
            let extra = self
                .promoted
                .capacity()
                .checked_sub(declaration.locals().len())
                .ok_or(Resource::Accounting)?;
            if extra != 0 {
                reserve(budget, owned, extra)?;
                self.scratch = sum(self.scratch, extra)?;
            }
            self.promoted.resize(declaration.locals().len(), false);
            for variable in plan.promoted_variables() {
                *self
                    .promoted
                    .get_mut(variable.get() as usize)
                    .ok_or(Failure::Binding)? = true;
            }
            self.engine
                .as_mut()
                .ok_or(Failure::Binding)?
                .finish_rows_into(
                    declaration,
                    semantic.types(),
                    &self.promoted,
                    &mut self.rows,
                    budget,
                    owned,
                )
                .map_err(engine_error)?;
            // Exact split: every post-engine-start credit except physical row
            // capacity is scratch. This includes the old scratch contract,
            // mask excess and all new engine/observer/liveness owner credits.
            let row_bytes = product(self.rows.rows.capacity(), size_of::<Read>())?;
            let growth = owned
                .checked_sub(self.engine_start)
                .ok_or(Resource::Accounting)?;
            let refund = growth.checked_sub(row_bytes).ok_or(Resource::Accounting)?;
            if refund < self.scratch {
                return Err(Resource::Accounting.into());
            }
            let next_owned = owned.checked_sub(refund).ok_or(Resource::Accounting)?;
            self.check_pair(owner, function, budget, owned, false)?;
            if refund > budget.storage() {
                return Err(Resource::Accounting.into());
            }
            // All fallible custody calculations precede these completed-scratch
            // drops. Rows remain attached if the following release ever refuses.
            drop(std::mem::take(&mut self.promoted));
            drop(self.engine.take());
            budget.release_storage(refund)?;
            *owned = next_owned;
            self.refunded = refund;
        }
        self.check_pair(owner, function, budget, owned, false)?;
        let entry = self.entry.ok_or(Resource::Accounting)?;
        let retained = budget
            .storage()
            .checked_sub(entry.storage)
            .ok_or(Resource::Accounting)?;
        let rows = std::mem::take(&mut self.rows.rows);
        self.view = Some(ProductionSemanticSharedReadsV1 {
            owner,
            function,
            rows,
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: entry.storage,
            owned: retained,
            failure: Cell::new(None),
        });
        self.held = Some(snapshot(budget, owned));
        self.phase = Phase::Complete;
        Ok(())
    }
    fn check_pair(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        function: SemanticFunctionIdV1,
        budget: &Budget<'_>,
        owned: &usize,
        allow_consumer_storage: bool,
    ) -> R<()> {
        let entry = self.entry.ok_or(Resource::Accounting)?;
        let now = snapshot(budget, owned);
        let growth = now
            .owned
            .checked_sub(entry.owned)
            .ok_or(Resource::Accounting)?;
        if self.owner.is_none_or(|bound| !std::ptr::eq(bound, owner))
            || self.function != Some(function) || entry.slot != now.slot
            || entry.ledger != now.ledger || entry.counter != now.counter
            // Construction is exact. A completed view owns only its receipt, not
            // later consumer reservations, and never incorporates their excess.
            || (if allow_consumer_storage {
                now.storage < sum(entry.storage, growth)?
            } else {
                now.storage != sum(entry.storage, growth)?
            }) || now.work < entry.work
            || now.peak < entry.peak
        {
            return Err(Resource::Accounting.into());
        }
        if let Some(held) = self.held {
            if now.owned < held.owned
                || now.storage < held.storage
                || now.work < held.work
                || now.peak < held.peak
            {
                return Err(Resource::Accounting.into());
            }
        }
        Ok(())
    }
    /// The actual owner, original pair and successful retained state are checked
    /// before an outer stage treats this view as ready.
    pub fn completed_for(
        &self,
        owner: &ProductionSemanticSsaOwnerV1,
        function: SemanticFunctionIdV1,
        budget: &Budget<'_>,
        owned: &usize,
    ) -> R<&ProductionSemanticSharedReadsV1<'s>> {
        if let Some(error) = self.failure {
            return Err(error.into());
        }
        if let Some(error) = self.view.as_ref().and_then(|view| view.failure.get()) {
            return Err(error.into());
        }
        if self.phase != Phase::Complete {
            return Err(Failure::Binding);
        }
        self.check_pair(owner, function, budget, owned, true)?;
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            return Err(Resource::Accounting.into());
        }
        self.view.as_ref().ok_or(Failure::Binding)
    }
    /// This accessor grants no extra authority: the returned existing view still
    /// enforces exact owner/function/place and ledger binding in every query.
    pub fn view(&self) -> Option<&ProductionSemanticSharedReadsV1<'s>> {
        if self.phase == Phase::Complete {
            self.view.as_ref()
        } else {
            None
        }
    }
}
impl Default for ProductionSemanticSharedReadsPreparationV1<'_> {
    fn default() -> Self {
        Self::new()
    }
}
fn reserve(budget: &mut Budget<'_>, owned: &mut usize, bytes: usize) -> R<()> {
    let next = owned.checked_add(bytes).ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(bytes)?;
    *owned = next;
    Ok(())
}
fn engine_error(error: RetainedAliasErrorV1) -> Failure {
    match error {
        RetainedAliasErrorV1::Resource(error) => Failure::Resource(error),
        RetainedAliasErrorV1::Original(error) => Failure::Analysis(error),
    }
}

fn frame() -> std::result::Result<usize, Resource> {
    // Reached source-level carriers; these are policy envelopes, not native stack sizes.
    let rows = [
        size_of::<ProductionSemanticSharedReadsPreparationV1<'static>>(),
        size_of::<(
            Phase,
            Snapshot,
            Option<Snapshot>,
            Option<Resource>,
            Option<SemanticFunctionIdV1>,
            Option<&ProductionSemanticSsaOwnerV1>,
            Option<&AdmittedInertSemanticMirV1>,
            Option<&SemanticFunctionDeclV1>,
            Option<&SsaConstructionPlanV1>,
        )>(),
        size_of::<(
            Option<Option<(usize, usize)>>,
            Option<(usize, usize)>,
            (usize, usize),
            Option<RetainedSharedEngineV1<'static>>,
            RetainedSharedRowsV1,
            Vec<bool>,
            Option<ProductionSemanticSharedReadsV1<'static>>,
            usize,
            usize,
            usize,
        )>(),
        size_of::<(
            &mut ProductionSemanticSharedReadsPreparationV1<'static>,
            &ProductionSemanticSharedReadsPreparationV1<'static>,
            &ProductionSemanticSsaOwnerV1,
            &AdmittedInertSemanticMirV1,
            &SemanticFunctionDeclV1,
            &SsaConstructionPlanV1,
            &ProductionSemanticSsaFunctionPlanV1,
            &[SemanticFunctionDeclV1],
            &[SemanticTypeDeclV1],
            Option<&SemanticFunctionDeclV1>,
            Option<&ProductionSemanticSsaFunctionPlanV1>,
        )>(),
        size_of::<(
            &Budget<'static>,
            &mut Budget<'static>,
            &usize,
            &mut usize,
            Ledger,
            *const Budget<'static>,
            *const usize,
            &mut Option<Resource>,
            &mut Resource,
            Cell<Option<Resource>>,
        )>(),
        size_of::<(
            R<()>,
            R<usize>,
            R<&ProductionSemanticSharedReadsV1<'static>>,
            R<&SemanticFunctionDeclV1>,
            R<&ProductionSemanticSsaFunctionPlanV1>,
            R<&mut RetainedSharedEngineV1<'static>>,
            R<&mut bool>,
            R<Option<(usize, usize)>>,
            R<&ProductionSemanticSharedReadsV1<'static>>,
            Failure,
            Resource,
            ProductionSemanticSsaErrorV1,
            RetainedAliasErrorV1,
        )>(),
        size_of::<(
            std::result::Result<(), Resource>,
            std::result::Result<usize, Resource>,
            std::result::Result<Snapshot, Resource>,
            std::result::Result<(), RetainedAliasErrorV1>,
            std::result::Result<bool, ProductionSemanticSsaErrorV1>,
            std::result::Result<(usize, usize), ProductionSemanticSsaErrorV1>,
            std::result::Result<Option<(usize, usize)>, ProductionSemanticSsaErrorV1>,
        )>(),
        size_of::<(
            std::result::Result<(), std::collections::TryReserveError>,
            std::collections::TryReserveError,
            Option<usize>,
            Option<&mut bool>,
            Option<&mut RetainedSharedEngineV1<'static>>,
            &mut RetainedSharedEngineV1<'static>,
            Option<&ProductionSemanticSharedReadsV1<'static>>,
            &ProductionSemanticSharedReadsV1<'static>,
        )>(),
        size_of::<(
            Meter<'static, 'static>,
            &mut Meter<'static, 'static>,
            Vec<bool>,
            &mut Vec<bool>,
            &[bool],
            bool,
            &bool,
            &mut bool,
            Vec<Read>,
            &mut Vec<Read>,
            &mut RetainedSharedRowsV1,
        )>(),
        size_of::<(
            &[SsaVariableIdV1],
            std::slice::Iter<'static, SsaVariableIdV1>,
            &SsaVariableIdV1,
            SsaVariableIdV1,
            u32,
            usize,
            usize,
            usize,
            usize,
            usize,
            bool,
        )>(),
        size_of::<(
            &[fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1],
            &[fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1],
            std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
            &[fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1],
            std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1>,
            &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
            &SemanticStatementKindV1,
        )>(),
        size_of::<(
            &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticRvalueV1,
            &SemanticRvalueKindV1,
            &SemanticPlaceV1,
            &[fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1],
            SemanticTypeIdV1,
            SemanticLocalIdV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1,
            Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>,
            Option<(u32, SemanticTypeIdV1, SemanticTypeIdV1)>,
            (u32, SemanticTypeIdV1, SemanticTypeIdV1),
        )>(),
        size_of::<(
            &SemanticTypeDeclV1,
            Option<&SemanticTypeDeclV1>,
            &SemanticTypeShapeV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticPointerTypeV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticPointerKindV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticMutabilityV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticPointerMetadataV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticBorrowKindV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticScalarTypeV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticValidityScalarTypeV1,
        )>(),
        size_of::<(
            &mut Meter<'static, 'static>,
            &SemanticFunctionDeclV1,
            &[SemanticTypeDeclV1],
            &mut ProductionSemanticSharedReadsPreparationV1<'static>,
            &mut dyn FnMut() -> R<()>,
        )>(),
        // Discovery occurs before any engine/observer/liveness frame. These
        // explicit receivers and callback-reference envelopes close that prefix.
        size_of::<(
            &ProductionSemanticMirOwnerV1,
            &&ProductionSemanticSsaFunctionPlanV1,
            &SemanticFunctionIdV1,
            &SsaConstructionPlanV1,
            &mut dyn FnMut(&&ProductionSemanticSsaFunctionPlanV1) -> bool,
        )>(),
        size_of::<(
            &model::SemanticMemoryLoadV1,
            &model::SemanticMemoryStoreV1,
            &model::SemanticAtomicRmwV1,
            &model::SemanticAtomicCompareExchangeV1,
            &SemanticOperandV1,
            &SemanticPlaceV1,
        )>(),
        size_of::<(
            &model::SemanticDirectCallV1,
            &model::SemanticDirectTailCallV1,
            &model::SemanticCallDestinationV1,
            Option<&model::SemanticCallDestinationV1>,
            &model::SemanticTerminatorV1,
            &SemanticTerminatorKindV1,
            &model::SemanticAssertMessageV1,
            &[SemanticOperandV1],
        )>(),
        size_of::<(
            &model::SemanticAggregateRvalueV1,
            &model::SemanticCheckedBinaryRvalueV1,
            &model::SemanticUncheckedBinaryRvalueV1,
            &Box<[SemanticOperandV1]>,
            std::slice::Iter<'static, SemanticOperandV1>,
            &model::SemanticFunctionAbiV1,
            &[SemanticTypeIdV1],
        )>(),
        size_of::<(
            model::SemanticControlFlowEdgeV1,
            &model::SemanticControlFlowEdgeV1,
            model::SemanticUnwindActionV1,
            &model::SemanticUnwindActionV1,
            &model::SemanticSwitchTargetsV1,
            model::SemanticSwitchTargetV1,
            &model::SemanticSwitchTargetV1,
            &[model::SemanticSwitchTargetV1],
            std::slice::Iter<'static, model::SemanticSwitchTargetV1>,
        )>(),
        size_of::<(
            std::convert::Infallible,
            std::result::Result<(), std::convert::Infallible>,
            std::result::Result<usize, ProductionSemanticSsaErrorV1>,
            &mut usize,
            usize,
            Option<usize>,
        )>(),
        // Fat-reference envelopes conservatively cover anonymous capture/reference
        // carriers; no dynamic visitor API or replacement Meter is introduced.
        size_of::<(
            &mut dyn FnMut(
                &SemanticOperandV1,
            ) -> std::result::Result<(), ProductionSemanticSsaErrorV1>,
            &mut dyn FnMut(
                model::SemanticControlFlowEdgeV1,
            ) -> std::result::Result<(), std::convert::Infallible>,
            &mut dyn FnMut(usize) -> Option<usize>,
            &mut Meter<'static, 'static>,
        )>(),
        size_of::<(
            &Vec<ProductionSemanticSsaFunctionPlanV1>,
            &[ProductionSemanticSsaFunctionPlanV1],
            &Vec<SsaVariableIdV1>,
            &Box<[SemanticFunctionDeclV1]>,
            &Box<[SemanticTypeDeclV1]>,
            &Box<[model::SemanticLocalDeclV1]>,
            &Box<[model::SemanticBasicBlockV1]>,
            &Box<[model::SemanticStatementV1]>,
            &Box<[model::SemanticProjectionV1]>,
            &Box<[SemanticTypeIdV1]>,
            &Box<[model::SemanticSwitchTargetV1]>,
        )>(),
        size_of::<(
            [usize; 23],
            std::array::IntoIter<usize, 23>,
            usize,
            usize,
            std::result::Result<usize, Resource>,
        )>(),
    ];
    rows.into_iter().try_fold(0usize, |sum, bytes| {
        sum.checked_add(bytes).ok_or(Resource::Arithmetic)
    })
}
#[cfg(test)]
#[path = "shared_primitive_reads_retained_v1_tests.rs"]
mod tests;
