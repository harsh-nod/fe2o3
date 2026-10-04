// Private source-bound plans. These rows never stand in for source SSA/use
// admission or a source/native correspondence check.

use fe2o3_mir_model::{SemanticMaskedShiftFactV1, SemanticMaskedShiftLimitsV1};

struct MaskedAssertionPlanV1<'source> {
    source: &'source AdmittedInertSemanticMirV1,
    plan: &'source LoweredFunctionPlanV1,
    correspondence_owner: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    bounds: BTreeSet<u32>,
    rows: Vec<Option<SemanticMaskedShiftFactV1<'source>>>,
}

enum InfallibleAssertDecisionsV1<'source> {
    Legacy(BTreeSet<u32>),
    LegacyBorrowed(&'source BTreeSet<u32>),
    Source(&'source MaskedAssertionPlanV1<'source>),
}

impl From<BTreeSet<u32>> for InfallibleAssertDecisionsV1<'_> {
    fn from(bounds: BTreeSet<u32>) -> Self {
        Self::Legacy(bounds)
    }
}

impl InfallibleAssertDecisionsV1<'_> {
    // Keep the original decision owner independent of the emitter's budget borrow.
    fn borrowed(&self) -> InfallibleAssertDecisionsV1<'_> {
        match self {
            Self::Legacy(bounds) => InfallibleAssertDecisionsV1::LegacyBorrowed(bounds),
            Self::LegacyBorrowed(bounds) => InfallibleAssertDecisionsV1::LegacyBorrowed(bounds),
            Self::Source(plan) => InfallibleAssertDecisionsV1::Source(plan),
        }
    }

    fn contains(&self, block: &u32) -> bool {
        match self {
            Self::Legacy(bounds) => bounds.contains(block),
            Self::LegacyBorrowed(bounds) => bounds.contains(block),
            Self::Source(plan) => {
                plan.bounds.contains(block)
                    || plan.rows.get(*block as usize).is_some_and(Option::is_some)
            }
        }
    }

    fn require_source(
        &self,
        source: &AdmittedInertSemanticMirV1,
        plan: &LoweredFunctionPlanV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if let Self::Source(facts) = self
            && (!std::ptr::eq(facts.source, source)
                || !std::ptr::eq(facts.plan, plan)
                || facts.correspondence_owner != plan.correspondence_owner
                || facts.function != plan.semantic_function)
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }

    fn require_parts(
        &self,
        types: &[SemanticTypeDeclV1],
        callables: &[SemanticCallableDeclV1],
        function: &SemanticFunctionDeclV1,
        correspondence_owner: SemanticFunctionIdV1,
        semantic_function: SemanticFunctionIdV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if let Self::Source(facts) = self
            && (facts.correspondence_owner != correspondence_owner
                || facts.function != semantic_function
                || !std::ptr::eq(facts.source.types(), types)
                || !std::ptr::eq(facts.source.callables(), callables)
                || !facts
                    .source
                    .functions()
                    .get(semantic_function.index() as usize)
                    .is_some_and(|actual| std::ptr::eq(actual, function)))
        {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    }
}

struct MaskedPlanStorageV1 {
    slot: usize,
    ledger: ArgumentLedgerV1,
    floor: usize,
    accepted: usize,
    poisoned: bool,
}

impl MaskedPlanStorageV1 {
    fn new(budget: &ArgumentBudgetV1<'_>) -> Self {
        Self {
            slot: budget as *const ArgumentBudgetV1<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            accepted: 0,
            poisoned: false,
        }
    }

    fn check(&mut self, budget: &ArgumentBudgetV1<'_>) -> Result<(), ArgumentResourceV1> {
        if self.slot != budget as *const ArgumentBudgetV1<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || self
                .floor
                .checked_add(self.accepted)
                .is_none_or(|minimum| budget.storage() < minimum)
        {
            self.poisoned = true;
        }
        if self.poisoned {
            Err(ArgumentResourceV1::Accounting)
        } else {
            Ok(())
        }
    }

    fn reserve(
        &mut self,
        bytes: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ArgumentResourceV1> {
        self.check(budget)?;
        let accepted = self
            .accepted
            .checked_add(bytes)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        budget.reserve_storage(bytes)?;
        self.accepted = accepted;
        Ok(())
    }

    fn table<T>(
        &mut self,
        count: usize,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Vec<Option<T>>, ProductionSemanticKirErrorV1> {
        self.check(budget)?;
        budget.charge_work(count)?;
        let width = std::mem::size_of::<Option<T>>();
        self.reserve(argument_product_v1(count, width)?, budget)?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| ArgumentResourceV1::Allocation)?;
        let excess = rows
            .capacity()
            .checked_sub(count)
            .ok_or(ArgumentResourceV1::Accounting)?;
        self.reserve(argument_product_v1(excess, width)?, budget)?;
        rows.resize_with(count, || None);
        Ok(rows)
    }
}

fn with_masked_assertion_plans_v1<'source, 'work, T>(
    owner: &'source ProductionSemanticSsaOwnerV1,
    plans: &'source [LoweredFunctionPlanV1],
    entry_bounds: BTreeSet<u32>,
    budget: &mut ArgumentBudgetV1<'work>,
    run: impl for<'scope> FnOnce(
        &'scope [Option<MaskedAssertionPlanV1<'source>>],
        &mut ArgumentBudgetV1<'work>,
    ) -> Result<T, ProductionSemanticKirErrorV1>,
) -> Result<T, ProductionSemanticKirErrorV1> {
    let source = owner.source_semantic();
    let mut storage = MaskedPlanStorageV1::new(budget);
    let mut tables = Vec::new();
    let mut deferred_panic = None;
    let mut result = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        tables = storage.table(plans.len(), budget)?;
        let mut entry_bounds = Some(entry_bounds);
        for (index, plan) in plans.iter().enumerate() {
            storage.check(budget)?;
            budget.charge_work(4)?;
            let function = source
                .functions()
                .get(plan.semantic_function.index() as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            owner
                .plan_for_function(plan.semantic_function)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            // Four bounded decision reads per source block (two sizing walks,
            // terminator emission, and origin recording), plus owner bindings.
            let decision_work = argument_product_v1(function.blocks().len(), 4)?
                .checked_add(16)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            budget.charge_work(decision_work)?;
            budget.charge_work(function.blocks().len())?;
            let has_shift_assertion = function.blocks().iter().any(|block| {
                matches!(
                    block.terminator().kind(),
                    SemanticTerminatorKindV1::Assert {
                        message: SemanticAssertMessageV1::Overflow {
                            operation: SemanticBinaryOpV1::ShiftLeft
                                | SemanticBinaryOpV1::ShiftRight,
                            ..
                        },
                        ..
                    }
                )
            });
            let mut rows = storage.table(
                if has_shift_assertion {
                    function.blocks().len()
                } else {
                    0
                },
                budget,
            )?;
            // Output storage already coexists at the query scope's entry floor.
            if has_shift_assertion {
                crate::with_production_semantic_masked_shift_query_v1(
                    source,
                    plan.semantic_function,
                    SemanticMaskedShiftLimitsV1::default(),
                    budget,
                    |query, budget| {
                        for (block, row) in rows.iter_mut().enumerate() {
                            budget.charge_work(1)?;
                            *row = query
                                .assertion(SemanticBlockIdV1::from_index(block as u32), budget)?;
                        }
                        Ok(())
                    },
                )
                .map_err(ProductionSemanticKirErrorV1::MaskedAssertionQuery)?;
            }
            tables[index] = Some(MaskedAssertionPlanV1 {
                source,
                plan,
                correspondence_owner: plan.correspondence_owner,
                function: plan.semantic_function,
                bounds: if index == 0 {
                    entry_bounds.take().unwrap()
                } else {
                    BTreeSet::new()
                },
                rows,
            });
        }
        run(&tables, budget)
    })) {
        Ok(result) => result,
        Err(payload) => {
            deferred_panic = Some(payload);
            Err(ProductionSemanticKirErrorV1::MaskedAssertionQuery(
                crate::ProductionSemanticMaskedShiftQueryErrorV1::Panicked,
            ))
        }
    };
    if let Err(error) = storage.check(budget) {
        if let Err(payload) =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(result)))
        {
            deferred_panic = Some(payload);
        }
        result = Err(error.into());
    }
    drop(tables);
    if !storage.poisoned {
        // Caller outputs may coexist here. Release this scope's capacity only.
        if let Err(error) = budget.release_storage(storage.accepted) {
            drop(result);
            drop(deferred_panic);
            return Err(error.into());
        }
    }
    drop(deferred_panic);
    result
}

fn masked_decisions_for_plan_v1<'scope>(
    source: &AdmittedInertSemanticMirV1,
    plan: &LoweredFunctionPlanV1,
    facts: &'scope Option<MaskedAssertionPlanV1<'_>>,
) -> Result<InfallibleAssertDecisionsV1<'scope>, ProductionSemanticKirErrorV1> {
    let decision = InfallibleAssertDecisionsV1::Source(
        facts
            .as_ref()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?,
    );
    decision.require_source(source, plan)?;
    Ok(decision)
}
