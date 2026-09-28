//! Caller-ledger bridge for native scalar helper-value correspondence.
use super::native_helper_value_context_v1::{NativeHelperValues, with_native_helper_values};
use super::native_helper_value_template_v1::{Ledger, Meter};
use super::*;

struct NativeValueMeter<'a, 'w> {
    budget: &'a mut ArgumentBudgetV1<'w>,
    failed: bool,
}

impl NativeValueMeter<'_, '_> {
    fn resource<T>(&mut self, result: Result<T, ArgumentResourceV1>) -> Result<T, &'static str> {
        result.map_err(|_| {
            self.failed = true;
            "native helper caller resource ledger exhausted"
        })
    }
}

impl Meter for NativeValueMeter<'_, '_> {
    fn work(&mut self, amount: usize) -> Result<(), &'static str> {
        let result = self.budget.charge_work(amount);
        self.resource(result)
    }
    fn reserve(&mut self, amount: usize) -> Result<(), &'static str> {
        let result = self.budget.reserve_storage(amount);
        self.resource(result)
    }
    fn release(&mut self, amount: usize) -> Result<(), &'static str> {
        let result = self.budget.release_storage(amount);
        self.resource(result)
    }
    fn exhausted(&self) -> bool {
        self.failed
    }
    fn storage(&self) -> Result<usize, &'static str> {
        Ok(self.budget.storage())
    }
    fn identity(&mut self) -> Result<Ledger, &'static str> {
        Ok(Ledger {
            slot: self.budget as *const ArgumentBudgetV1<'_> as usize,
            work: self.budget.work_ledger_identity_v1(),
        })
    }
}

struct SourceValueMeterV18<'q, 'b, 'w, 'c> {
    ledger: &'q CorrelationLedgerV18<'b, 'w, 'c>,
}

impl Meter for SourceValueMeterV18<'_, '_, '_, '_> {
    fn work(&mut self, amount: usize) -> Result<(), &'static str> {
        self.ledger
            .with_budget(|budget| budget.charge_work(amount))
            .map_err(|_| "source translation caller work ledger refused")
    }
    fn reserve(&mut self, amount: usize) -> Result<(), &'static str> {
        self.ledger
            .with_budget(|budget| budget.reserve_storage(amount))
            .map_err(|_| "source translation caller storage ledger refused")
    }
    fn release(&mut self, amount: usize) -> Result<(), &'static str> {
        self.ledger
            .with_budget(|budget| budget.release_storage(amount))
            .map_err(|_| "source translation caller storage custody refused")
    }
    fn exhausted(&self) -> bool {
        self.ledger.failure.get().is_some() || self.ledger.inconsistent_inventory.get()
    }
    fn storage(&self) -> Result<usize, &'static str> {
        self.ledger
            .with_budget(|budget| Ok(budget.storage()))
            .map_err(|_| "source translation caller storage custody refused")
    }
    fn identity(&mut self) -> Result<Ledger, &'static str> {
        self.ledger
            .with_budget(|budget| {
                Ok(Ledger {
                    slot: budget as *const ArgumentBudgetV1<'_> as usize,
                    work: budget.work_ledger_identity_v1(),
                })
            })
            .map_err(|_| "source translation caller ledger custody refused")
    }
}

// The source-owned root has already expanded each original defined call into
// its exact retained instance. This supplies accounting, not a legacy helper
// certificate; retained executable calls still need their own source relation.
pub(super) fn with_source_value_expansion_v18<T>(
    ledger: &CorrelationLedgerV18<'_, '_, '_>,
    action: impl FnOnce(
        &mut NativeValueExpansion<'_, '_>,
    ) -> Result<T, ProductionMirPlironTranslationErrorV1>,
) -> Result<T, ProductionMirPlironTranslationErrorV1> {
    let headers = std::mem::size_of::<SourceValueMeterV18<'_, '_, '_, '_>>()
        .checked_add(std::mem::size_of::<NativeValueExpansion<'_, '_>>())
        .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    ledger
        .with_budget(|budget| budget.reserve_storage(headers))
        .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    let result = {
        let mut meter = SourceValueMeterV18 { ledger };
        run(None, &mut meter, |expansion| {
            expansion.with_argument_allowance(action)
        })
    };
    let value = result?;
    ledger
        .with_budget(|budget| budget.release_storage(headers))
        .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    Ok(value)
}

pub(super) struct NativeValueExpansion<'a, 'm> {
    helpers: Option<&'a NativeHelperValues<'a>>,
    meter: &'m mut dyn Meter,
    reserved: usize,
    temporary_nodes_remaining: Option<usize>,
}

/// Standalone normalizer tests have no semantic owner or helper authority.
#[cfg(test)]
pub(super) fn with_no_helpers_for_test_v1<R>(
    budget: &mut ArgumentBudgetV1<'_>,
    action: impl FnOnce(&mut NativeValueExpansion<'_, '_>) -> R,
) -> R {
    let mut meter = NativeValueMeter {
        budget,
        failed: false,
    };
    let mut expansion = NativeValueExpansion {
        helpers: None,
        meter: &mut meter,
        reserved: 0,
        temporary_nodes_remaining: None,
    };
    action(&mut expansion)
}

impl NativeValueExpansion<'_, '_> {
    pub(super) fn charge_normalization_node_v1(&mut self) -> Option<()> {
        if let Some(remaining) = &mut self.temporary_nodes_remaining {
            *remaining = remaining.checked_sub(1)?;
        }
        Some(())
    }

    fn with_argument_allowance<T>(&mut self, action: impl FnOnce(&mut Self) -> T) -> T {
        let outer = self
            .temporary_nodes_remaining
            .replace(fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| action(self)));
        self.temporary_nodes_remaining = outer;
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn argument(
        &mut self,
        function: &Function,
        kir: &dyn KirCorrelationGraphV18,
        sites: &dyn SemanticAccessQueriesV18,
        value: ValueId,
        depth: usize,
        visiting: &mut dyn ScalarValueVisitingV18,
        budget: &mut dyn CorrelationChargeV18,
    ) -> Option<NormalizedScalarExpressionV1> {
        self.with_argument_allowance(|expansion| {
            normalize_kir_expression_with_visiting_v18(
                function, kir, sites, value, depth, visiting, budget, expansion,
            )
        })
    }

    /// The immutable source/N replay is still mandatory. This only composes a
    /// complete independently interpreted native helper with exact actual call
    /// arguments; no caller operation, Call, or effect is removed or hoisted.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn call(
        &mut self,
        function: &Function,
        kir: &dyn KirCorrelationGraphV18,
        sites: &dyn SemanticAccessQueriesV18,
        location: FunctionOperationLocation,
        operation: &Operation,
        arguments: &[ValueId],
        depth: usize,
        visiting: &mut dyn ScalarValueVisitingV18,
        budget: &mut dyn CorrelationChargeV18,
    ) -> Option<NormalizedScalarExpressionV1> {
        let context = self.helpers?;
        let template = context
            .root_call(function, location, operation, self.meter)
            .ok()?;
        // The historical correlation/depth limits remain mandatory. Each new
        // temporary argument also has a construction-time node allowance,
        // including nested helper results, matching this prepaid payload.
        let nodes = arguments
            .len()
            .checked_mul(fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2)?;
        let trees = nodes.checked_mul(std::mem::size_of::<NormalizedScalarExpressionV1>())?;
        self.meter.work(nodes).ok()?;
        self.meter.reserve(trees).ok()?;
        let (mut resolved, storage) =
            match native_helper_value_template_v1::vector(arguments.len(), self.meter) {
                Ok(value) => value,
                Err(_) => {
                    let _ = self.meter.release(trees);
                    return None;
                }
            };
        let result = (|| {
            for value in arguments {
                resolved
                    .push(self.argument(function, kir, sites, *value, depth, visiting, budget)?);
            }
            // A Call entry already consumed one enclosing node. Replace that
            // placeholder with the exact expanded result, checking before emit.
            let limit = match self.temporary_nodes_remaining {
                Some(remaining) => remaining.checked_add(1)?,
                None => fe2o3_pliron::MAX_PRODUCTION_SEMANTIC_EXPRESSION_NODES_V2,
            };
            let (expression, bytes, nodes) = template
                .instantiate_with_node_limit(&resolved, limit, self.meter)
                .ok()?;
            if let Some(remaining) = &mut self.temporary_nodes_remaining {
                let Some(next) = nodes
                    .checked_sub(1)
                    .and_then(|nodes| remaining.checked_sub(nodes))
                else {
                    drop(expression);
                    let _ = self.meter.release(bytes);
                    return None;
                };
                *remaining = next;
            }
            if let Some(total) = self.reserved.checked_add(bytes) {
                self.reserved = total;
                Some(expression)
            } else {
                drop(expression);
                let _ = self.meter.release(bytes);
                None
            }
        })();
        drop(resolved);
        self.meter.release(storage).ok()?;
        self.meter.release(trees).ok()?;
        result
    }
}

fn run<'a, T>(
    helpers: Option<&'a NativeHelperValues<'a>>,
    meter: &mut dyn Meter,
    action: impl FnOnce(
        &mut NativeValueExpansion<'_, '_>,
    ) -> Result<T, ProductionMirPlironTranslationErrorV1>,
) -> Result<T, ProductionMirPlironTranslationErrorV1> {
    let ledger = meter
        .identity()
        .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    let mut expansion = NativeValueExpansion {
        helpers,
        meter,
        reserved: 0,
        temporary_nodes_remaining: None,
    };
    let result = action(&mut expansion);
    if expansion.meter.identity().ok() != Some(ledger) {
        return Err(ProductionMirPlironTranslationErrorV1::ResourceLimit);
    }
    expansion
        .meter
        .release(expansion.reserved)
        .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    if expansion.meter.exhausted() {
        return Err(ProductionMirPlironTranslationErrorV1::ResourceLimit);
    }
    result
}

/// Shared entry for the additive caller-budgeted validator. Full existing
/// source lowering, original Module/correspondence equality and mandatory
/// effect checks remain outside this expression-only scope and unchanged.
pub(super) fn with_native_value_expansion_v1(
    semantic: Option<&AdmittedInertSemanticMirV1>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    kernel: &str,
    budget: &mut ArgumentBudgetV1<'_>,
    action: impl FnOnce(
        &mut NativeValueExpansion<'_, '_>,
    ) -> Result<
        ProductionMirPlironTranslationValidationV1,
        ProductionMirPlironTranslationErrorV1,
    >,
) -> Result<ProductionMirPlironTranslationValidationV1, ProductionMirPlironTranslationErrorV1> {
    let mut meter = NativeValueMeter {
        budget,
        failed: false,
    };
    let Some(semantic) = semantic else {
        return run(None, &mut meter, action);
    };
    let work = module
        .kernels
        .len()
        .checked_add(module.functions.len())
        .and_then(|n| n.checked_add(correspondence.lowered_functions.len()))
        .ok_or(ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    meter
        .work(work)
        .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    let entry = module
        .kernels
        .iter()
        .find(|candidate| candidate.id.as_str() == kernel)
        .and_then(|kernel| {
            module
                .functions
                .iter()
                .find(|function| function.id == kernel.entry)
        })
        .ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?;
    let root = correspondence
        .lowered_functions
        .iter()
        .find(|row| {
            row.role == SemanticKirFunctionRoleV1::KernelEntry && row.kernel_ir_function == entry.id
        })
        .ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?
        .correspondence_owner;
    let mut needed = false;
    for block in entry
        .body
        .as_ref()
        .ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?
        .blocks
        .iter()
    {
        for operation in &block.operations {
            meter
                .work(1)
                .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
            if matches!(operation.kind, OperationKind::Call { .. }) {
                needed = true;
                break;
            }
        }
        if needed {
            break;
        }
    }
    if !needed {
        return run(None, &mut meter, action);
    }
    let mut result = None;
    let checked = with_native_helper_values(
        semantic,
        module,
        correspondence,
        root,
        entry,
        &mut meter,
        |context, meter| {
            result = Some(run(Some(context), meter, action));
            Ok(())
        },
    );
    if meter.failed {
        return Err(ProductionMirPlironTranslationErrorV1::ResourceLimit);
    }
    checked.map_err(|_| ProductionMirPlironTranslationErrorV1::KernelShape)?;
    result.ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?
}

#[cfg(test)]
#[path = "native_helper_value_node_cap_v1_tests.rs"]
mod node_cap_tests;
