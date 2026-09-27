//! Closed source-expression adapter used by the source-owned consumer.
//! It reuses the original resolver and helper-template validation. Private
//! scalar names never constitute a ranked or memory proof.

use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use fe2o3_lower_mir_kernel::{
    ProductionSourceOwnedViewErrorV18 as SourceError,
    ProductionSourceScalarArgumentV18 as Argument,
    ProductionSourceScalarLeavesV18 as Leaves,
    ProductionSourceScalarInputV18 as Input,
    ProductionSourceScalarStoreV18 as StoreInput,
    ProductionOptimizedSourceScalarStoreV18 as OptimizedStoreInput,
    ProductionSourceEntryWriteV18 as EntryWriteInput,
};
use std::{cell::{Cell, RefCell}, mem::size_of};

impl From<SourceError> for ProductionRankedProjectionErrorV1 {
    fn from(error: SourceError) -> Self { canonical_source_facts_v18::source_error(error) }
}

// This trait is private to the compiler module. The sole production adapter
// below borrows the checked original-source table and original budget.
pub(super) trait SourceScalarResolverLeavesV18 {
    fn work(&self, amount: usize) -> Result<(), &'static str>;
    fn reserve(&self, bytes: usize) -> Result<(), &'static str>;
    fn resource_refusal(&self, error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1)
        -> &'static str;
    fn place(&self, function: &SemanticFunctionDeclV1, place: &SemanticPlaceV1)
        -> Result<Option<ProductionSemanticExpressionV2>, &'static str>;
    fn argument(&self, function: &SemanticFunctionDeclV1, argument: u32,
        scalar: ProductionSemanticScalarTypeV2, depth: usize)
        -> Result<ProductionSemanticExpressionV2, &'static str>;
}

pub(super) enum ResolverBorrowedLocalsV18<'a> {
    Legacy(HashSet<u32>),
    Source { rows: Vec<bool>, source: &'a dyn SourceScalarResolverLeavesV18 },
}

impl ResolverBorrowedLocalsV18<'_> {
    pub(super) fn contains(&self, local: &u32) -> bool {
        match self {
            Self::Legacy(rows) => rows.contains(local),
            Self::Source { rows, source } => source.work(1).is_err()
                || rows.get(*local as usize).copied().unwrap_or(true),
        }
    }

    pub(super) fn insert(&mut self, local: u32) -> bool {
        match self {
            Self::Legacy(rows) => rows.insert(local),
            Self::Source { rows, .. } => match rows.get_mut(local as usize) {
                Some(row) => { let old = *row; *row = true; !old }
                None => false,
            },
        }
    }
}

const ACTIVE: usize = fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_DEPTH_V2 + 1;
type ActiveKey = (u32, usize, usize);

pub(super) enum ResolverActiveDefinitionsV18<'a> {
    Legacy(HashSet<ActiveKey>),
    Source {
        rows: [Option<ActiveKey>; ACTIVE],
        source: &'a dyn SourceScalarResolverLeavesV18,
    },
}

impl<'a> ResolverActiveDefinitionsV18<'a> {
    pub(super) fn source(source: &'a dyn SourceScalarResolverLeavesV18) -> Result<Self, &'static str> {
        source.work(ACTIVE)?;
        Ok(Self::Source { rows: [None; ACTIVE], source })
    }

    pub(super) fn insert(&mut self, key: ActiveKey) -> bool {
        match self {
            Self::Legacy(rows) => rows.insert(key),
            Self::Source { rows, source } => {
                let mut vacant = None;
                for (index, row) in rows.iter().enumerate() {
                    if source.work(1).is_err() { return false; }
                    if *row == Some(key) { return false; }
                    if row.is_none() && vacant.is_none() { vacant = Some(index); }
                }
                match vacant {
                    Some(index) => { rows[index] = Some(key); true }
                    None => false,
                }
            }
        }
    }

    pub(super) fn remove(&mut self, key: &ActiveKey) -> bool {
        match self {
            Self::Legacy(rows) => rows.remove(key),
            Self::Source { rows, source } => {
                for row in rows {
                    if source.work(1).is_err() { return false; }
                    if row.as_ref() == Some(key) { *row = None; return true; }
                }
                false
            }
        }
    }

    #[cfg(test)]
    pub(super) fn is_empty(&self) -> bool {
        match self {
            Self::Legacy(rows) => rows.is_empty(),
            Self::Source { rows, .. } => rows.iter().all(Option::is_none),
        }
    }
}

struct SourceExpressionSessionV18<'v, 'scope, 'b, 'w> {
    semantic: &'v AdmittedInertSemanticMirV1,
    leaves: &'v Leaves<'scope>,
    budget: RefCell<&'b mut Budget<'w>>,
    first: RefCell<Option<ProductionRankedProjectionErrorV1>>,
    incoming: usize,
    live_floor: Cell<usize>,
}

impl SourceExpressionSessionV18<'_, '_, '_, '_> {
    fn select(&self, error: ProductionRankedProjectionErrorV1) {
        let mut first = self.first.borrow_mut();
        if first.is_none() { *first = Some(error); }
    }

    fn observe(&self) {
        let valid = self.budget.try_borrow().is_ok_and(|budget|
            budget.storage() >= self.live_floor.get() && self.leaves.observe_custody(&budget).is_ok());
        if !valid {
            self.leaves.deny_refund();
            self.select(source_ranked_consumer_resources_v18::resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting));
        }
    }

    fn capture<T>(&self, result: Result<T, ProductionRankedProjectionErrorV1>) -> Result<T, &'static str> {
        result.map_err(|error| {
            self.select(error);
            self.observe();
            "source scalar original resolver refused"
        })
    }

    fn query<T>(&self, action: impl FnOnce(&mut Budget<'_>) -> Result<T, SourceError>)
        -> Result<T, &'static str>
    {
        if self.first.borrow().is_some() { return Err("source scalar caller ledger already refused"); }
        let result = match self.budget.try_borrow_mut() {
            Ok(mut budget) if budget.storage() >= self.live_floor.get() =>
                self.leaves.check(&mut budget).and_then(|()| action(&mut budget)),
            Ok(_) => {
                self.leaves.deny_refund();
                Err(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting.into())
            }
            Err(_) => Err(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting.into()),
        };
        result.map_err(|error| {
            self.select(canonical_source_facts_v18::source_error(error));
            "source scalar caller ledger or source identity refused"
        })
    }

    fn work(&self, amount: usize) -> Result<(), &'static str> {
        self.query(|budget| budget.charge_work(amount).map_err(Into::into))
    }

    fn reserve(&self, bytes: usize) -> Result<(), &'static str> {
        self.query(|budget| {
            let next = self.live_floor.get().checked_add(bytes)
                .ok_or(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic)?;
            budget.reserve_storage(bytes)?;
            self.live_floor.set(next);
            Ok(())
        })
    }

    fn release(&self, bytes: usize) -> Result<(), &'static str> {
        self.query(|budget| {
            let next = self.live_floor.get().checked_sub(bytes).filter(|next| *next >= self.incoming)
                .ok_or(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting)?;
            budget.release_storage(bytes)?;
            self.live_floor.set(next);
            Ok(())
        })
    }
}

impl Drop for SourceExpressionSessionV18<'_, '_, '_, '_> {
    fn drop(&mut self) { self.observe(); }
}

struct SourceScalarInstanceV18<'a, 'v, 'scope, 'b, 'w> {
    session: &'a SourceExpressionSessionV18<'v, 'scope, 'b, 'w>,
    instance: usize,
}

struct SourceExpressionHelperMeterV18<'a, 'v, 'scope, 'b, 'w> {
    session: &'a SourceExpressionSessionV18<'v, 'scope, 'b, 'w>,
}

impl helper_value_template_v1::Meter for SourceExpressionHelperMeterV18<'_, '_, '_, '_, '_> {
    fn work(&mut self, amount: usize) -> Result<(), &'static str> { self.session.work(amount) }
    fn reserve(&mut self, amount: usize) -> Result<(), &'static str> { self.session.reserve(amount) }
    fn release(&mut self, amount: usize) -> Result<(), &'static str> {
        self.session.release(amount)
    }
    fn exhausted(&self) -> bool { self.session.first.borrow().is_some() }
    fn storage(&self) -> Result<usize, &'static str> { self.session.query(|budget| Ok(budget.storage())) }
    fn identity(&mut self) -> Result<helper_value_template_v1::Ledger, &'static str> {
        self.session.query(|budget| Ok(helper_value_template_v1::Ledger {
            slot: budget as *const Budget<'_> as usize, work: budget.work_ledger_identity_v1(),
        }))
    }
}

impl SourceScalarInstanceV18<'_, '_, '_, '_, '_> {
    fn with_resolver(
        &self,
        action: impl for<'query> FnOnce(&mut GpuSemanticExpressionResolverV2<'query>)
            -> Result<ProductionSemanticExpressionV2, &'static str>,
    ) -> Result<ProductionSemanticExpressionV2, &'static str> {
        let function = self.session.query(|budget| self.session.leaves.original_function(self.instance, budget))?;
        let semantic = self.session.semantic;
        self.session.work(semantic.functions().len())?;
        let index = semantic.functions().iter().position(|candidate| std::ptr::eq(candidate, function))
            .ok_or("source scalar original declaration is foreign to semantic owner")?;
        self.session.reserve(fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2.checked_mul(
                size_of::<ProductionSemanticExpressionV2>()
                    .checked_add(size_of::<Vec<fe2o3_mir_model::semantic_mir_v1::SemanticProjectionV1>>())
                    .ok_or_else(|| self.resource_refusal(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic))?
            ).ok_or_else(|| self.resource_refusal(fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Arithmetic))?)?;
        let mut meter = SourceExpressionHelperMeterV18 { session: self.session };
        source_helper_value_context_v1::with_source_helper_values(semantic, index, &mut meter, |helpers, meter| {
            let mut resolver = self.session.capture(GpuSemanticExpressionResolverV2::new_with_source_v18(
                semantic.types(), function, Some(self),
            ))?;
            resolver.helper_semantic = Some(semantic);
            resolver.helper_values = Some(helpers);
            resolver.helper_meter = Some(meter);
            let mut resolver = self.session.capture(resolver.with_gfx942_inline_callables_v30(semantic.callables())
                .and_then(|resolver| resolver.with_scalar_callables_v1(semantic.callables())))?;
            self.session.capture(action(&mut resolver).map_err(ProductionRankedProjectionErrorV1::Incomplete))
        })
    }
}

impl SourceScalarResolverLeavesV18 for SourceScalarInstanceV18<'_, '_, '_, '_, '_> {
    fn work(&self, amount: usize) -> Result<(), &'static str> { self.session.work(amount) }
    fn reserve(&self, bytes: usize) -> Result<(), &'static str> { self.session.reserve(bytes) }
    fn resource_refusal(&self, error: fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1)
        -> &'static str
    {
        self.session.select(source_ranked_consumer_resources_v18::resource(error));
        self.session.observe();
        "source scalar allocation or arithmetic refused"
    }
    fn place(&self, function: &SemanticFunctionDeclV1, place: &SemanticPlaceV1)
        -> Result<Option<ProductionSemanticExpressionV2>, &'static str>
    {
        self.session.query(|budget| self.session.leaves.original_place(self.instance, function, place, budget))
    }
    fn argument(&self, function: &SemanticFunctionDeclV1, argument: u32,
        scalar: ProductionSemanticScalarTypeV2, depth: usize)
        -> Result<ProductionSemanticExpressionV2, &'static str>
    {
        GpuSemanticExpressionResolverV2::require_depth_v2(depth)?;
        match self.session.query(|budget| self.session.leaves.original_argument(self.instance, function, argument, budget))? {
            Argument::Root { argument } => {
                let symbol = crate::reference_effect_v1::kernel_scalar_symbol_v2(argument)
                    .ok_or("source scalar root argument namespace exhausted")?;
                Ok(ProductionSemanticExpressionV2::Symbol { symbol, scalar })
            }
            Argument::Caller { instance, function: caller, block, operand } => {
                let parent = SourceScalarInstanceV18 { session: self.session, instance };
                parent.with_resolver(|resolver| {
                    if !std::ptr::eq(resolver.function, caller) {
                        return Err("source scalar argument substituted caller declaration");
                    }
                    let body = resolver.function.blocks().get(block.index() as usize)
                        .ok_or("source scalar argument original call block missing")?;
                    let SemanticTerminatorKindV1::Call(call) = body.terminator().kind() else {
                        return Err("source scalar argument original caller is not a call");
                    };
                    let actual = call.arguments().get(argument as usize)
                        .filter(|actual| std::ptr::eq(*actual, operand))
                        .ok_or("source scalar argument operand changed")?;
                    resolver.use_site = Some(ScalarAssignmentSiteV1 {
                        block: block.index() as usize, statement: body.statements().len(),
                    });
                    let expression = resolver.resolve_operand_v2(actual,
                        depth.checked_add(1).ok_or("source scalar argument recursion overflow")?)?;
                    if expression.scalar() != scalar {
                        return Err("source scalar argument transport type changed");
                    }
                    Ok(expression)
                })
            }
        }
    }
}

// The reconstructed tree stays inside the same private leaf scope as physical
// normalization. It never becomes a public ranked recipe or memory receipt.
pub(super) fn check_source_scalar_store_v18(
    semantic: &AdmittedInertSemanticMirV1, leaves: &Leaves<'_>, request: &StoreInput<'_>,
    budget: &mut Budget<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    check_source_scalar_store_endpoint_v18(semantic, leaves,
        SourceScalarStoreEndpointV18::Original(request), budget)
}

pub(super) fn check_optimized_source_scalar_store_v18(
    semantic: &AdmittedInertSemanticMirV1, leaves: &Leaves<'_>,
    request: &OptimizedStoreInput<'_>, budget: &mut Budget<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    check_source_scalar_store_endpoint_v18(semantic, leaves,
        SourceScalarStoreEndpointV18::Optimized(request), budget)
}

enum SourceScalarStoreEndpointV18<'a> {
    Original(&'a StoreInput<'a>),
    Optimized(&'a OptimizedStoreInput<'a>),
    TypedEntry(&'a EntryWriteInput<'a>),
}

impl SourceScalarStoreEndpointV18<'_> {
    fn original(&self, budget: &mut Budget<'_>) -> Result<(usize, &SemanticFunctionDeclV1), SourceError> {
        match self {
            Self::Original(request) => request.original(budget),
            Self::Optimized(request) => request.original(budget),
            Self::TypedEntry(request) => request.original(budget),
        }
    }
    fn scalar(&self, budget: &mut Budget<'_>) -> Result<ProductionSemanticScalarTypeV2, SourceError> {
        match self {
            Self::Original(request) => request.scalar(budget),
            Self::Optimized(request) => request.scalar(budget),
            Self::TypedEntry(request) => request.scalar(budget),
        }
    }
    fn input_for<'f>(&self, function: &'f SemanticFunctionDeclV1, budget: &mut Budget<'_>)
        -> Result<Input<'f>, SourceError>
    {
        match self {
            Self::Original(request) => request.input_for(function, budget),
            Self::Optimized(request) => request.input_for(function, budget),
            Self::TypedEntry(request) => request.input_for(function, budget),
        }
    }
    fn check_expression(&self, expression: &ProductionSemanticExpressionV2, budget: &mut Budget<'_>)
        -> Result<(), SourceError>
    {
        match self {
            Self::Original(request) => request.check_expression(expression, budget),
            Self::Optimized(request) => request.check_expression(expression, budget),
            Self::TypedEntry(request) => request.check_expression(expression, budget),
        }
    }
}

pub(super) fn check_source_entry_write_v18(
    semantic: &AdmittedInertSemanticMirV1, leaves: &Leaves<'_>,
    request: &EntryWriteInput<'_>, budget: &mut Budget<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    check_source_scalar_store_endpoint_v18(semantic, leaves,
        SourceScalarStoreEndpointV18::TypedEntry(request), budget)
}

fn check_source_scalar_store_endpoint_v18(
    semantic: &AdmittedInertSemanticMirV1, leaves: &Leaves<'_>,
    request: SourceScalarStoreEndpointV18<'_>, budget: &mut Budget<'_>,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    let (instance, original) = request.original(budget).map_err(canonical_source_facts_v18::source_error)?;
    let scalar = request.scalar(budget).map_err(canonical_source_facts_v18::source_error)?;
    let floor = budget.storage();
    budget.reserve_storage(size_of::<SourceExpressionSessionV18<'_, '_, '_, '_>>()
        + size_of::<SourceScalarInstanceV18<'_, '_, '_, '_, '_>>()
        + size_of::<SourceExpressionHelperMeterV18<'_, '_, '_, '_, '_>>())
        .map_err(source_ranked_consumer_resources_v18::resource)?;
    let incoming = budget.storage();
    let session = SourceExpressionSessionV18 { semantic, leaves, budget: RefCell::new(budget),
        first: RefCell::new(None), incoming, live_floor: Cell::new(incoming) };
    let source = SourceScalarInstanceV18 { session: &session, instance };
    let expression = source.with_resolver(|resolver| {
        if !std::ptr::eq(resolver.function, original) {
            return Err("source scalar Store substituted original function");
        }
        let input = session.query(|budget| request.input_for(resolver.function, budget))?;
        match input {
            Input::Operand { block, statement, operand } => {
                let body = resolver.function.blocks().get(block.index() as usize)
                    .ok_or("source scalar Store input block absent")?;
                resolver.use_site = Some(ScalarAssignmentSiteV1 {
                    block: block.index() as usize,
                    statement: statement.map(|index| index as usize).unwrap_or(body.statements().len()),
                });
                resolver.resolve_operand_v2(operand, 0)
            }
            Input::Assignment { block, statement, assignment } => resolver.resolve_store_v2(assignment,
                ScalarAssignmentSiteV1 { block: block.index() as usize, statement: statement as usize }),
            Input::CallResult { block, call } => {
                resolver.use_site = Some(ScalarAssignmentSiteV1 {
                    block: block.index() as usize,
                    statement: resolver.function.blocks().get(block.index() as usize)
                        .ok_or("source scalar Store call block absent")?.statements().len(),
                });
                match semantic.callables().get(call.callee().index() as usize) {
                    Some(SemanticCallableDeclV1::Defined { .. }) =>
                        resolver.resolve_defined_call_v1(block.index() as usize, call, 0),
                    Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::Gfx942InlineU32(_), ..
                    }) => {
                        let destination = call.destination()
                            .ok_or("source scalar inline result has no normal destination")?;
                        // A CallResult capture is the successful result, not a
                        // read before the call. Keep the existing decoder's
                        // normal-edge dominance and source occurrence checks.
                        resolver.use_site = Some(ScalarAssignmentSiteV1 {
                            block: destination.edge().target().index() as usize, statement: 0,
                        });
                        resolver.resolve_gfx942_inline_call_result_v30(destination.place(), 0)
                    }
                    _ => resolver.resolve_saturating_call_v2(call, 0),
                }
            }
            Input::EntryArgument { argument } =>
                source.argument(resolver.function, argument, scalar, 0),
        }
    });
    let result = match expression {
        Ok(expression) => {
            let result = session.query(|budget| request.check_expression(&expression, budget));
            drop(expression);
            result
        }
        Err(error) => Err(error),
    };
    session.observe();
    let selected = session.first.borrow_mut().take();
    let retained = session.live_floor.get().checked_sub(floor)
        .ok_or_else(|| source_ranked_consumer_resources_v18::resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting))?;
    drop(source);
    drop(session);
    match selected {
        Some(error) => Err(error),
        None => {
            result.map_err(ProductionRankedProjectionErrorV1::Incomplete)?;
            budget.release_storage(retained).map_err(source_ranked_consumer_resources_v18::resource)
        }
    }
}

pub(super) fn check_source_scalar_stores_v18(
    semantic: &AdmittedInertSemanticMirV1,
    leaves: &Leaves<'_>, budget: &mut Budget<'_>,
) -> Result<usize, ProductionRankedProjectionErrorV1> {
    leaves.visit_store_inputs(budget, |request, budget| {
        check_source_scalar_store_v18(semantic, leaves, request, budget)
    })
}
