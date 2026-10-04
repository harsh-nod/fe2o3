//! Caller-ledger bridge for native scalar helper-value correspondence.
use super::native_helper_value_context_v1::{
    NativeHelperCallQuery, NativeHelperMeter, NativeHelperValues, with_native_helper_values,
};
use super::native_helper_value_template_v1::{Ledger, Meter};
use super::*;

/// One legacy translation-phase allowance shared across every root in that
/// phase. It carries no source authority and owns no alternate resource ledger.
/// All accepted work/storage charges still enter the original caller Budget.
pub(super) struct TranslationAllowanceV1 {
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    work: usize,
    work_limit: usize,
    storage_limit: usize,
    failed: bool,
}

impl TranslationAllowanceV1 {
    pub(super) fn new(
        budget: &ArgumentBudgetV1<'_>,
        work_limit: usize,
        storage_limit: usize,
    ) -> Self {
        Self {
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            work: 0,
            work_limit,
            storage_limit,
            failed: false,
        }
    }

    /// Sole private nominal phase: its header was just paid on this same Budget.
    /// This seeds only actual already-paid work; it does not refill an allowance.
    pub(super) fn with_prepaid_header_v1(
        budget: &ArgumentBudgetV1<'_>,
        incoming_floor: usize,
        work_limit: usize,
        storage_limit: usize,
    ) -> Result<Self, ProductionMirPlironTranslationErrorV1> {
        let header = std::mem::size_of::<Self>();
        if header > work_limit
            || header > storage_limit
            || incoming_floor.checked_add(header) != Some(budget.storage())
        {
            return Err(ProductionMirPlironTranslationErrorV1::ResourceLimit);
        }
        Ok(Self {
            ledger: budget.work_ledger_identity_v1(),
            floor: incoming_floor,
            work: header,
            work_limit,
            storage_limit,
            failed: false,
        })
    }

    pub(super) fn refused_v1(&self) -> bool {
        self.failed
    }

    fn refuse<T>(&mut self) -> Result<T, &'static str> {
        self.failed = true;
        Err("native helper local translation allowance exhausted")
    }

    fn live(&mut self, budget: &ArgumentBudgetV1<'_>) -> Result<usize, &'static str> {
        if self.failed || budget.work_ledger_identity_v1() != self.ledger {
            return self.refuse();
        }
        match budget.storage().checked_sub(self.floor) {
            Some(live) if live <= self.storage_limit => Ok(live),
            _ => self.refuse(),
        }
    }

    fn work(
        &mut self,
        budget: &ArgumentBudgetV1<'_>,
        amount: usize,
    ) -> Result<usize, &'static str> {
        self.live(budget)?;
        match self.work.checked_add(amount) {
            Some(next) if next <= self.work_limit => Ok(next),
            _ => self.refuse(),
        }
    }

    fn reserve(
        &mut self,
        budget: &ArgumentBudgetV1<'_>,
        amount: usize,
    ) -> Result<(), &'static str> {
        let live = self.live(budget)?;
        match live.checked_add(amount) {
            Some(next) if next <= self.storage_limit => Ok(()),
            _ => self.refuse(),
        }
    }

    // Cleanup remains permitted after denial, but never releases the caller's
    // incoming floor or charges/resets a replacement ledger.
    fn release(
        &mut self,
        budget: &ArgumentBudgetV1<'_>,
        amount: usize,
    ) -> Result<(), &'static str> {
        if budget.work_ledger_identity_v1() != self.ledger
            || budget
                .storage()
                .checked_sub(self.floor)
                .is_none_or(|live| amount > live)
        {
            return self.refuse();
        }
        Ok(())
    }
}

struct NativeValueMeter<'a, 'w> {
    budget: &'a mut ArgumentBudgetV1<'w>,
    allowance: Option<&'a mut TranslationAllowanceV1>,
    failed: bool,
    resource_error: Option<ArgumentResourceV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NativeTranslationErrorV1 {
    Resource(ArgumentResourceV1),
    Translation(ProductionMirPlironTranslationErrorV1),
}

impl NativeTranslationErrorV1 {
    pub(super) fn into_translation(self) -> ProductionMirPlironTranslationErrorV1 {
        match self {
            Self::Resource(_) => ProductionMirPlironTranslationErrorV1::ResourceLimit,
            Self::Translation(error) => error,
        }
    }

    pub(super) fn into_semantic(self) -> ProductionSemanticKirErrorV1 {
        match self {
            Self::Resource(error) => error.into(),
            Self::Translation(error) => ProductionSemanticKirErrorV1::MirPlironTranslation(error),
        }
    }
}

impl NativeHelperMeter for NativeValueMeter<'_, '_> {
    fn check_call(&mut self, query: NativeHelperCallQuery<'_>) -> Result<bool, &'static str> {
        self.check_original_bounded(|budget| query.check(budget))
    }
}

impl NativeValueMeter<'_, '_> {
    fn check_original_bounded(
        &mut self,
        check: impl FnOnce(&mut ArgumentBudgetV1<'_>) -> Result<(), ProductionSemanticKirErrorV1>,
    ) -> Result<bool, &'static str> {
        let ledger = self.identity()?;
        let floor = self.budget.storage();
        let start = self.budget.work();
        let (work, storage) = if let Some(allowance) = &mut self.allowance {
            let live = allowance.live(self.budget)?;
            (
                allowance.work_limit.checked_sub(allowance.work),
                allowance.storage_limit.checked_sub(live),
            )
        } else {
            (
                usize::MAX.checked_sub(start),
                self.budget.storage_limit().checked_sub(floor),
            )
        };
        let (Some(work), Some(storage)) = (work, storage) else {
            self.failed = true;
            return Err("native helper checked-call allowance accounting");
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.budget.with_bounded_scratch_v1(work, storage, check)
        }));
        let consumed = self.budget.work().checked_sub(start);
        let intact = self.identity()? == ledger && self.budget.storage() == floor;
        let accounted = if let (Some(allowance), Some(consumed)) = (&mut self.allowance, consumed) {
            match allowance.work.checked_add(consumed) {
                Some(next) if next <= allowance.work_limit => {
                    allowance.work = next;
                    true
                }
                _ => false,
            }
        } else {
            consumed.is_some()
        };
        if !intact || !accounted {
            self.failed = true;
            if let Some(allowance) = &mut self.allowance {
                allowance.failed = true;
            }
        }
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        };
        if self.exhausted() {
            return Err("native helper checked-call ledger changed");
        }
        match result {
            Ok(()) => Ok(true),
            Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(resource)) => {
                // The scoped cap may be the phase allowance, not the parent ledger.
                // Ties retain the same local-first policy as work() and reserve().
                let local = self.allowance.is_some()
                    && match resource {
                        ArgumentResourceV1::Work(error) => {
                            start.checked_add(work) == Some(error.limit())
                        }
                        ArgumentResourceV1::Storage(error) => {
                            floor.checked_add(storage) == Some(error.limit())
                        }
                        _ => false,
                    };
                if local {
                    self.allowance
                        .as_deref_mut()
                        .expect("checked local allowance")
                        .refuse()
                } else {
                    self.resource(Err(resource))
                }
            }
            Err(_) => Ok(false),
        }
    }

    fn resource<T>(&mut self, result: Result<T, ArgumentResourceV1>) -> Result<T, &'static str> {
        result.map_err(|error| {
            self.resource_error.get_or_insert(error);
            self.failed = true;
            if let Some(allowance) = &mut self.allowance {
                allowance.failed = true;
            }
            "native helper caller resource ledger exhausted"
        })
    }
}

impl Meter for NativeValueMeter<'_, '_> {
    fn work(&mut self, amount: usize) -> Result<(), &'static str> {
        let next = match &mut self.allowance {
            Some(allowance) => Some(allowance.work(self.budget, amount)?),
            None => None,
        };
        let result = self.budget.charge_work(amount);
        self.resource(result)?;
        if let (Some(allowance), Some(next)) = (&mut self.allowance, next) {
            allowance.work = next;
        }
        Ok(())
    }
    fn reserve(&mut self, amount: usize) -> Result<(), &'static str> {
        if let Some(allowance) = &mut self.allowance {
            allowance.reserve(self.budget, amount)?;
        }
        let result = self.budget.reserve_storage(amount);
        self.resource(result)
    }
    fn release(&mut self, amount: usize) -> Result<(), &'static str> {
        if let Some(allowance) = &mut self.allowance {
            allowance.release(self.budget, amount)?;
        }
        let result = self.budget.release_storage(amount);
        self.resource(result)
    }
    fn exhausted(&self) -> bool {
        self.failed
            || self
                .allowance
                .as_ref()
                .is_some_and(|allowance| allowance.failed)
    }
    fn storage(&self) -> Result<usize, &'static str> {
        Ok(self.budget.storage())
    }
    fn identity(&mut self) -> Result<Ledger, &'static str> {
        // Identity must remain readable after refusal so existing cleanup can
        // release its exact owned scratch. This does not revive the allowance.
        Ok(Ledger {
            slot: self.budget as *const ArgumentBudgetV1<'_> as usize,
            work: self.budget.work_ledger_identity_v1(),
        })
    }
}

#[derive(Clone, Copy)]
enum ValueSource<'a> {
    None,
    Native(&'a NativeHelperValues<'a>),
    Nominal(&'a bf16_nominal_translation_context_v1::Context<'a>),
}

pub(super) struct NativeValueExpansion<'a, 'm> {
    source: ValueSource<'a>,
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
        allowance: None,
        failed: false,
        resource_error: None,
    };
    let mut expansion = NativeValueExpansion {
        source: ValueSource::None,
        meter: &mut meter,
        reserved: 0,
        temporary_nodes_remaining: None,
    };
    action(&mut expansion)
}

impl NativeValueExpansion<'_, '_> {
    pub(super) fn nominal_tensors(
        &mut self,
        function: &Function,
        recipe: &fe2o3_pliron::ProductionRankedKernelV1,
    ) -> Result<Option<usize>, ProductionMirPlironTranslationErrorV1> {
        let ValueSource::Nominal(context) = self.source else {
            return Ok(None);
        };
        context
            .tensors(function, recipe, self.meter)
            .map(Some)
            .map_err(|_| {
                if self.meter.exhausted() {
                    ProductionMirPlironTranslationErrorV1::ResourceLimit
                } else {
                    ProductionMirPlironTranslationErrorV1::TensorContractMismatch
                }
            })
    }

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
        kir: &KirCorrelationIndexV1<'_>,
        sites: &BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>,
        value: ValueId,
        depth: usize,
        visiting: &mut BTreeSet<ValueId>,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<NormalizedScalarExpressionV1> {
        self.with_argument_allowance(|expansion| {
            normalize_kir_expression_v1(
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
        kir: &KirCorrelationIndexV1<'_>,
        sites: &BTreeMap<(FunctionOperationLocation, u32), SemanticAccessSiteV1>,
        location: FunctionOperationLocation,
        operation: &Operation,
        arguments: &[ValueId],
        depth: usize,
        visiting: &mut BTreeSet<ValueId>,
        budget: &mut UnsupportedIndexCorrelationBudgetV1,
    ) -> Option<NormalizedScalarExpressionV1> {
        let context = match self.source {
            ValueSource::Native(context) => context,
            // The checked four-component nominal Call is not a scalar native
            // template. Any requested index/value normalization still refuses.
            ValueSource::None | ValueSource::Nominal(_) => return None,
        };
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

fn run<'a>(
    source: ValueSource<'a>,
    meter: &mut dyn Meter,
    action: impl FnOnce(
        &mut NativeValueExpansion<'_, '_>,
    ) -> Result<
        ProductionMirPlironTranslationValidationV1,
        ProductionMirPlironTranslationErrorV1,
    >,
) -> Result<ProductionMirPlironTranslationValidationV1, ProductionMirPlironTranslationErrorV1> {
    let ledger = meter
        .identity()
        .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
    let mut expansion = NativeValueExpansion {
        source,
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
#[allow(dead_code)] // Preserve the existing private compatibility entry.
pub(super) fn with_native_value_expansion_v1(
    semantic: Option<&ProductionSemanticSsaOwnerV1>,
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
    with_native_value_expansion_and_allowance_v1(
        semantic,
        module,
        correspondence,
        kernel,
        budget,
        None,
        action,
    )
}

#[allow(clippy::too_many_arguments)]
pub(super) fn with_native_value_expansion_and_allowance_v1(
    semantic: Option<&ProductionSemanticSsaOwnerV1>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    kernel: &str,
    budget: &mut ArgumentBudgetV1<'_>,
    allowance: Option<&mut TranslationAllowanceV1>,
    action: impl FnOnce(
        &mut NativeValueExpansion<'_, '_>,
    ) -> Result<
        ProductionMirPlironTranslationValidationV1,
        ProductionMirPlironTranslationErrorV1,
    >,
) -> Result<ProductionMirPlironTranslationValidationV1, ProductionMirPlironTranslationErrorV1> {
    with_native_value_expansion_and_allowance_resources_v1(
        semantic,
        module,
        correspondence,
        kernel,
        budget,
        allowance,
        action,
    )
    .map_err(NativeTranslationErrorV1::into_translation)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn with_native_value_expansion_and_allowance_resources_v1(
    semantic: Option<&ProductionSemanticSsaOwnerV1>,
    module: &Module,
    correspondence: &SemanticKirCorrespondenceV1,
    kernel: &str,
    budget: &mut ArgumentBudgetV1<'_>,
    allowance: Option<&mut TranslationAllowanceV1>,
    action: impl FnOnce(
        &mut NativeValueExpansion<'_, '_>,
    ) -> Result<
        ProductionMirPlironTranslationValidationV1,
        ProductionMirPlironTranslationErrorV1,
    >,
) -> Result<ProductionMirPlironTranslationValidationV1, NativeTranslationErrorV1> {
    let mut meter = NativeValueMeter {
        budget,
        allowance,
        failed: false,
        resource_error: None,
    };
    let result = (|| {
        let Some(semantic) = semantic else {
            return run(ValueSource::None, &mut meter, action);
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
                row.role == SemanticKirFunctionRoleV1::KernelEntry
                    && row.kernel_ir_function == entry.id
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
            return run(ValueSource::None, &mut meter, action);
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
                result = Some(run(ValueSource::Native(context), meter, action));
                Ok(())
            },
        );
        if meter.exhausted() {
            return Err(ProductionMirPlironTranslationErrorV1::ResourceLimit);
        }
        checked.map_err(|_| ProductionMirPlironTranslationErrorV1::KernelShape)?;
        result.ok_or(ProductionMirPlironTranslationErrorV1::KernelShape)?
    })();
    match meter.resource_error {
        Some(error) => Err(NativeTranslationErrorV1::Resource(error)),
        None => result.map_err(NativeTranslationErrorV1::Translation),
    }
}

#[cfg(test)]
#[path = "native_helper_value_node_cap_v1_tests.rs"]
mod node_cap_tests;

#[cfg(test)]
#[path = "native_helper_translation_allowance_v1_tests.rs"]
mod allowance_tests;

#[cfg(test)]
#[path = "native_helper_checked_allowance_v1_tests.rs"]
mod checked_allowance_tests;

#[cfg(test)]
#[path = "native_helper_resource_error_v1_tests.rs"]
mod resource_error_tests;

#[cfg(test)]
#[path = "native_helper_checked_caps_v1_tests.rs"]
mod checked_caps_tests;

fn nominal_frame_bytes<F>() -> Result<usize, ArgumentResourceV1> {
    [
        BF16_CALL_QUERY_SCRATCH_V1,
        std::mem::size_of::<bf16_nominal_translation_context_v1::Rows<'static>>(),
        std::mem::size_of::<bf16_nominal_translation_context_v1::Context<'static>>(),
        std::mem::size_of::<NativeValueExpansion<'static, 'static>>(),
        std::mem::size_of::<NativeValueMeter<'static, 'static>>(),
        std::mem::size_of::<ProductionMirPlironTranslationValidationV1>(),
        std::mem::size_of::<NativeTranslationErrorV1>(),
        std::mem::size_of::<F>(),
    ]
    .into_iter()
    .try_fold(0usize, |sum, bytes| {
        bytes
            .checked_mul(2)
            .and_then(|n| sum.checked_add(n))
            .ok_or(ArgumentResourceV1::Arithmetic)
    })
}

// The original Budget and existing local translation allowance cover the new
// selected context before construction. Only this frame's exact receipt is
// refunded. Callback-owned surplus is never reset to an entry snapshot.
fn with_nominal_meter<F>(
    budget: &mut ArgumentBudgetV1<'_>,
    allowance: Option<&mut TranslationAllowanceV1>,
    action: F,
) -> Result<ProductionMirPlironTranslationValidationV1, NativeTranslationErrorV1>
where
    F: FnOnce(
        &mut NativeValueMeter<'_, '_>,
    ) -> Result<
        ProductionMirPlironTranslationValidationV1,
        ProductionMirPlironTranslationErrorV1,
    >,
{
    let frame = nominal_frame_bytes::<F>().map_err(NativeTranslationErrorV1::Resource)?;
    let mut meter = NativeValueMeter {
        budget,
        allowance,
        failed: false,
        resource_error: None,
    };
    let selected = (|| {
        let ledger = meter
            .identity()
            .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
        meter
            .reserve(frame)
            .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
        let protected = meter.budget.storage();
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            meter
                .work(frame)
                .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
            action(&mut meter)
        }));
        if meter.identity().ok() != Some(ledger) || meter.budget.storage() < protected {
            drop(outcome);
            meter
                .resource_error
                .get_or_insert(ArgumentResourceV1::Accounting);
            return Err(ProductionMirPlironTranslationErrorV1::ResourceLimit);
        }
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(ProductionMirPlironTranslationErrorV1::KernelShape)
            }
        };
        // Keep existing local-first denial classification. A denial hidden by a
        // callback which never entered the meter still prevents acceptance.
        if !meter.exhausted() {
            let prior = meter.budget.check_prior_denials_v1();
            let _ = meter.resource(prior);
        }
        meter
            .release(frame)
            .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
        if meter.exhausted() {
            return Err(ProductionMirPlironTranslationErrorV1::ResourceLimit);
        }
        result
    })();
    match meter.resource_error {
        Some(error) => Err(NativeTranslationErrorV1::Resource(error)),
        None => selected.map_err(NativeTranslationErrorV1::Translation),
    }
}

/// Private nominal sibling. The scalar helper path and all public attachment
/// gates remain unchanged. Nominal Call components have no scalar expansion.
pub(super) fn with_nominal_value_expansion_and_allowance_resources_v1(
    owner: &ProductionPreRankedKirOwnerV1,
    kernel: &str,
    budget: &mut ArgumentBudgetV1<'_>,
    allowance: Option<&mut TranslationAllowanceV1>,
    action: impl FnOnce(
        &mut NativeValueExpansion<'_, '_>,
    ) -> Result<
        ProductionMirPlironTranslationValidationV1,
        ProductionMirPlironTranslationErrorV1,
    >,
) -> Result<ProductionMirPlironTranslationValidationV1, NativeTranslationErrorV1> {
    with_nominal_meter(budget, allowance, |meter| {
        let rows = bf16_nominal_translation_context_v1::Rows::resolve(owner, kernel, meter)
            .map_err(|_| {
                if meter.exhausted() {
                    ProductionMirPlironTranslationErrorV1::ResourceLimit
                } else {
                    ProductionMirPlironTranslationErrorV1::KernelShape
                }
            })?;
        let checked = meter
            .check_original_bounded(|budget| rows.check_components(budget))
            .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
        if !checked {
            return Err(ProductionMirPlironTranslationErrorV1::KernelShape);
        }
        let context = rows
            .bind(meter)
            .map_err(|_| ProductionMirPlironTranslationErrorV1::ResourceLimit)?;
        run(ValueSource::Nominal(&context), meter, action)
    })
}

#[cfg(test)]
#[path = "bf16_nominal_translation_v1_tests.rs"]
mod nominal_translation_tests;
