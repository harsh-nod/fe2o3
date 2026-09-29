//! Paid operation-row consumer. Final report construction is layered below.
use super::*;
use crate::formal_memory_obligations::{
    body_engine_v19, gfx942_inline_u32_v30, report_construction_v18,
};
use crate::{CanonicalEffectErrorV19, CanonicalEffectScopeV19};
use report_construction_v18::{LiveReportMeterV18, ReportMeterV18};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::formal_memory_obligations) enum BodyErrorV19 {
    Source(Failure),
    Effects(CanonicalEffectErrorV19),
    Invocation(RegionValidationError),
    Rejected,
    Panicked,
}
impl From<Failure> for BodyErrorV19 {
    fn from(error: Failure) -> Self {
        Self::Source(error)
    }
}
impl From<ResourceError> for BodyErrorV19 {
    fn from(error: ResourceError) -> Self {
        Self::Source(error.into())
    }
}
impl From<crate::CanonicalKernelIrVerificationResourceErrorV1> for BodyErrorV19 {
    fn from(error: crate::CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Source(error.into())
    }
}
impl From<CanonicalEffectErrorV19> for BodyErrorV19 {
    fn from(error: CanonicalEffectErrorV19) -> Self {
        Self::Effects(error)
    }
}
type BodyResult<T> = std::result::Result<T, BodyErrorV19>;

/// A borrowed complete extraction result, not launch or publication authority.
/// Its lifetime cannot outlive the original owner and the paid report rows.
pub(in crate::formal_memory_obligations) struct ActualOwnerReportViewV19<'view, 'owner> {
    owner: &'owner VerifiedCanonicalKernelIrModuleV18,
    source: &'owner Function,
    root_index: usize,
    launch_extent: ExplicitLaunchExtent,
    interpretation: PhysicalLaunchInterpretationV2,
    report: &'view FormalMemoryObligationAnalysis,
}

impl ActualOwnerReportViewV19<'_, '_> {
    pub(in crate::formal_memory_obligations) fn original_owner(
        &self,
    ) -> &VerifiedCanonicalKernelIrModuleV18 {
        self.owner
    }
    pub(in crate::formal_memory_obligations) fn original_function(&self) -> &Function {
        self.source
    }
    pub(in crate::formal_memory_obligations) fn root_index(&self) -> usize {
        self.root_index
    }
    pub(in crate::formal_memory_obligations) fn launch_extent(&self) -> ExplicitLaunchExtent {
        self.launch_extent
    }
    pub(in crate::formal_memory_obligations) fn interpretation(
        &self,
    ) -> PhysicalLaunchInterpretationV2 {
        self.interpretation
    }
    pub(in crate::formal_memory_obligations) fn analysis(&self) -> &FormalMemoryObligationAnalysis {
        self.report
    }
}

struct Paid<'query, 'pointer, 'borrow, 'affine, 'owner, 'work, 'scope, 'budget, 'budget_work> {
    accesses: &'query mut ActualOwnerAccessesV18<'pointer, 'borrow, 'affine, 'owner, 'work>,
    effects: &'scope CanonicalEffectScopeV19<'owner>,
    budget: &'budget mut Budget<'budget_work>,
    rows: &'query mut Vec<FormalMemoryAccess>,
    reasons: &'query mut Vec<FormalMemoryIncompleteReason>,
    reason_width: usize,
}

impl Paid<'_, '_, '_, '_, '_, '_, '_, '_, '_> {
    fn push_reason(&mut self, reason: FormalMemoryIncompleteReason) -> BodyResult<()> {
        meter::LiveGuardMeter::new(self.budget, usize::MAX, usize::MAX, usize::MAX)
            .push(self.reasons, reason)?;
        Ok(())
    }

    fn invocation_range(
        &mut self,
        launch: ExplicitLaunchExtent,
        interpretation: PhysicalLaunchInterpretationV2,
    ) -> BodyResult<Option<InvocationRange1d>> {
        // Rank is at most three; the shared fixed-result resolver allocates no
        // tree/vector and retains the exact legacy first-reason ordering.
        self.budget.charge_work(64)?;
        match resolve_invocations_value_v19(
            &self.accesses.pointers.slots.affine.root.domain,
            launch,
            interpretation,
        )
        .map_err(BodyErrorV19::Invocation)?
        {
            Ok(range) => Ok(Some(range)),
            Err(reason) => {
                self.push_reason(reason)?;
                Ok(None)
            }
        }
    }

    fn initial_reasons(&mut self, width: FormalIndexWidth) -> BodyResult<()> {
        if width != FormalIndexWidth::Bits64 {
            self.push_reason(FormalMemoryIncompleteReason::UnsupportedIndexWidth { width })?;
        }
        let source = self.accesses.pointers.slots.affine.source;
        let body = source.body.as_ref().ok_or(ResourceError::Accounting)?;
        if !body.blocks[0].parameters.is_empty() {
            self.push_reason(
                FormalMemoryIncompleteReason::UnsupportedEntryBlockParameters {
                    block: body.blocks[0].id,
                },
            )?;
        }
        let owner = self.accesses.pointers.slots.affine.owner;
        let root = self.accesses.pointers.slots.root_index;
        let mut ordinal = 0_usize;
        while let Some((_, escape)) =
            self.accesses
                .pointers
                .slots
                .escape(owner, root, ordinal, self.budget)?
        {
            if let Some((location, pointer)) = escape {
                self.push_reason(FormalMemoryIncompleteReason::UnsupportedPointerDerivation {
                    location,
                    pointer,
                })?;
            }
            ordinal = ordinal.checked_add(1).ok_or(ResourceError::Arithmetic)?;
        }
        Ok(())
    }
}

impl body_engine_v19::State for Paid<'_, '_, '_, '_, '_, '_, '_, '_, '_> {
    type Error = BodyErrorV19;
    fn step(&mut self) -> BodyResult<()> {
        Ok(self.budget.charge_work(32)?)
    }
    fn reachable(&mut self, block: BlockId) -> BodyResult<bool> {
        let affine = &mut self.accesses.pointers.slots.affine;
        Ok(affine
            .context
            .reachable(affine.source, block, self.budget)?)
    }
    fn proven_private(&mut self, pointer: ValueId) -> BodyResult<bool> {
        let root = self.accesses.pointers.slots.root_index;
        let affine = &mut self.accesses.pointers.slots.affine;
        let origin = affine.exact_origin(affine.owner, root, pointer, self.budget)?;
        if let Some(origin) = origin {
            if matches!(affine.context.value_type(affine.source, origin, self.budget)?,
                Some(Type::Pointer(ty)) if ty.address_space == AddressSpace::Private)
            {
                return Ok(true);
            }
        }
        let Some(facts) = self.accesses.guarded.take() else {
            return Ok(false);
        };
        let mut guarded = facts.replace_meter(meter::LiveGuardMeter::new(
            self.budget,
            usize::MAX,
            usize::MAX,
            usize::MAX,
        ));
        let result = guarded.proven_pointer_space_v18(pointer);
        self.accesses.guarded = Some(guarded.replace_meter(()));
        Ok(result? == Some(AddressSpace::Private))
    }
    fn ordered_composition(&self) -> bool {
        false
    }
    fn complete_body_v19(&self) -> bool {
        false
    }
    fn call_pure(&mut self, operation: &Operation) -> BodyResult<bool> {
        let OperationKind::Call { callee, .. } = &operation.kind else {
            return Err(ResourceError::Accounting.into());
        };
        self.budget.check_prior_denials_v1()?;
        if operation.has_complete_effect_summary_with_budget_v1(self.budget)? {
            return Ok(true);
        }
        Ok(self
            .effects
            .function_named(
                self.accesses.pointers.slots.affine.owner,
                callee,
                self.budget,
            )?
            .is_complete_and_pure())
    }
    fn call_reason(
        &mut self,
        location: FunctionOperationLocation,
        callee: &FunctionId,
    ) -> BodyResult<()> {
        let name = callee.as_str();
        let owned = paid_string_v19(name, self.budget)?;
        self.reason_width = self.reason_width.max(
            name.len()
                .checked_add(16)
                .ok_or(ResourceError::Arithmetic)?,
        );
        self.push_reason(FormalMemoryIncompleteReason::CallEffectsUnavailable {
            location,
            callee: FunctionId::new(owned),
        })
    }
    fn reason(&mut self, reason: FormalMemoryIncompleteReason) -> BodyResult<()> {
        if matches!(
            reason,
            FormalMemoryIncompleteReason::CallEffectsUnavailable { .. }
        ) {
            return Err(ResourceError::Accounting.into());
        }
        self.push_reason(reason)
    }
    fn access(
        &mut self,
        location: FunctionOperationLocation,
        _: &Operation,
        invocations: InvocationRange1d,
        conservative: bool,
    ) -> BodyResult<AccessResult> {
        let owner = self.accesses.pointers.slots.affine.owner;
        let root = self.accesses.pointers.slots.root_index;
        Ok(if conservative {
            self.accesses.conservative_guarded_read(
                owner,
                root,
                location,
                invocations,
                self.budget,
            )?
        } else {
            self.accesses
                .access(owner, root, location, invocations, self.budget)?
        })
    }
    fn push(&mut self, access: FormalMemoryAccess) -> BodyResult<()> {
        meter::LiveGuardMeter::new(self.budget, usize::MAX, usize::MAX, usize::MAX)
            .push(self.rows, access)?;
        Ok(())
    }
    fn closed_assembly(&mut self, operation: &Operation) -> BodyResult<bool> {
        let OperationKind::InlineAssembly(assembly) = &operation.kind else {
            return Ok(false);
        };
        // The unchanged closed grammar has at most two scalar inputs and one
        // option. Reject larger envelopes before any input or option scan.
        self.budget.charge_work(128)?;
        if assembly.options.len() != 1 || assembly.operands.len() > 3 {
            return Ok(false);
        }
        let mut types = [(ValueId(0), None); 2];
        let mut count = 0;
        for operand in assembly.operands.iter().skip(1) {
            if let crate::AssemblyOperandKind::Input(value) = operand.kind {
                let affine = &mut self.accesses.pointers.slots.affine;
                let ty = affine
                    .context
                    .value_type(affine.source, value, self.budget)?
                    .and_then(Type::as_scalar);
                types[count] = (value, ty);
                count += 1;
            }
        }
        Ok(gfx942_inline_u32_v30::has_closed_memory_effects_with_types(
            operation,
            |value| {
                types[..count]
                    .iter()
                    .find(|(id, _)| *id == value)
                    .and_then(|(_, ty)| *ty)
            },
        ))
    }
    fn local_effects_empty(&mut self, operation: &Operation) -> BodyResult<bool> {
        let mut empty = true;
        operation.try_visit_local_memory_effects_v1(|_| -> BodyResult<()> {
            self.budget.charge_work(1)?;
            empty = false;
            Ok(())
        })?;
        Ok(empty)
    }
}

fn paid_string_v19(value: &str, budget: &mut Budget<'_>) -> BodyResult<String> {
    budget.charge_work(
        value
            .len()
            .checked_add(2)
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    budget.reserve_storage(value.len())?;
    let mut owned = String::new();
    owned
        .try_reserve_exact(value.len())
        .map_err(|_| ResourceError::Allocation)?;
    budget.reserve_storage(
        owned
            .capacity()
            .checked_sub(value.len())
            .ok_or(ResourceError::Accounting)?,
    )?;
    owned.push_str(value);
    Ok(owned)
}

fn construct_report_v19<'owner>(
    accesses: &mut ActualOwnerAccessesV18<'_, '_, '_, 'owner, '_>,
    effects: &CanonicalEffectScopeV19<'owner>,
    launch: ExplicitLaunchExtent,
    width: FormalIndexWidth,
    interpretation: PhysicalLaunchInterpretationV2,
    budget: &mut Budget<'_>,
) -> BodyResult<FormalMemoryObligationAnalysis> {
    let owner = accesses.pointers.slots.affine.owner;
    let source = accesses.pointers.slots.affine.source;
    let root = accesses.pointers.slots.affine.root;
    // This also checks the exact effect owner, original function, ledger and
    // retained floor when the function contains no Call operations at all.
    effects.function(owner, source, budget)?;
    let mut allocations = Vec::new();
    let body = source.body.as_ref().ok_or(ResourceError::Accounting)?;
    for (ordinal, (&value, ty)) in body
        .parameters
        .iter()
        .zip(&source.signature.parameters)
        .enumerate()
    {
        budget.charge_work(8)?;
        if let Some(allocation) = formal_allocation_parameter(ordinal, value, ty) {
            LiveReportMeterV18 { budget }.push(&mut allocations, allocation)?;
        }
    }
    let mut rows = Vec::new();
    let mut reasons = Vec::new();
    let (invocations, reason_width) = {
        let mut paid = Paid {
            accesses,
            effects,
            budget,
            rows: &mut rows,
            reasons: &mut reasons,
            reason_width: 16,
        };
        paid.initial_reasons(width)?;
        let invocations = paid.invocation_range(launch, interpretation)?;
        body_engine_v19::collect(
            source,
            (width == FormalIndexWidth::Bits64)
                .then_some(invocations)
                .flatten(),
            &mut paid,
        )?;
        (invocations, paid.reason_width)
    };
    let (bounds_requirements, runtime_alias_requirements, inter_invocation_conflicts) = {
        let mut meter = LiveReportMeterV18 { budget };
        let mut overflows = Vec::new();
        let bounds = derive_bounds_requirements_with_meter(&rows, &mut overflows, &mut meter)?;
        for &location in &overflows {
            meter.charge(1)?;
            meter.push(
                &mut reasons,
                FormalMemoryIncompleteReason::AddressArithmeticOverflow { location },
            )?;
        }
        meter.retire(overflows)?;
        let aliases = derive_alias_requirements_with_meter(&rows, &mut meter)?;
        let conflicts = derive_inter_invocation_conflicts_with_meter(&rows, &mut meter)?;
        (bounds, aliases, conflicts)
    };
    crate::verification_index_v1::verification_bounded_sort_by_v1(
        &mut reasons,
        reason_width,
        budget,
        Ord::cmp,
    )?;
    budget.charge_work(
        reasons
            .len()
            .checked_mul(reason_width)
            .ok_or(ResourceError::Arithmetic)?,
    )?;
    // Duplicate owned callee strings are dropped here. Their conservative
    // credit remains until all report storage is retired by the outer scope.
    reasons.dedup();
    let obligations = FormalMemoryObligations {
        kernel: KernelId::new(paid_string_v19(root.id.as_str(), budget)?),
        entry: FunctionId::new(paid_string_v19(root.entry.as_str(), budget)?),
        index_width: width,
        invocations,
        allocations,
        accesses: rows,
        bounds_requirements,
        runtime_alias_requirements,
        inter_invocation_conflicts,
    };
    Ok(if reasons.is_empty() {
        FormalMemoryObligationAnalysis::Complete(obligations)
    } else {
        FormalMemoryObligationAnalysis::Incomplete {
            partial: obligations,
            reasons,
        }
    })
}

// This private boundary accepts trusted compiler callbacks/destructors. Like
// callback execution itself, opaque panic-payload destruction is not metered:
// an adversarial destructor that manufactures another panicking payload can
// fail to terminate. This is not a sandbox for arbitrary Rust callback code.
fn drain_v19<T>(value: T) {
    let mut result = catch_unwind(AssertUnwindSafe(|| drop(value)));
    while let Err(payload) = result {
        result = catch_unwind(AssertUnwindSafe(|| drop(payload)));
    }
}

struct ReportScopeStateV19 {
    slot: usize,
    ledger: crate::CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    required: Option<usize>,
}
impl ReportScopeStateV19 {
    fn same(&self, budget: &Budget<'_>) -> bool {
        self.slot == budget as *const Budget<'_> as usize
            && self.ledger == budget.work_ledger_identity_v1()
    }
}

fn frame_v19<F>() -> BodyResult<usize> {
    [
        size_of::<ReportScopeStateV19>(),
        size_of::<ActualOwnerReportViewV19<'_, '_>>(),
        size_of::<FormalMemoryObligationAnalysis>(),
        size_of::<BodyResult<FormalMemoryObligationAnalysis>>(),
        size_of::<std::thread::Result<BodyResult<()>>>() * 2,
        size_of::<std::thread::Result<()>>(),
        size_of::<Option<BodyErrorV19>>(),
        size_of::<F>(),
        size_of::<Option<F>>(),
        size_of::<Paid<'_, '_, '_, '_, '_, '_, '_, '_, '_>>(),
        size_of::<LiveReportMeterV18<'_, '_>>(),
        size_of::<Option<GuardedAnalysisV1<'_, ()>>>(),
        size_of::<GuardedAnalysisV1<'_, meter::LiveGuardMeter<'_, '_>>>(),
        size_of::<std::result::Result<Option<AddressSpace>, ResourceError>>(),
        size_of::<Vec<FormalAllocationParameter>>(),
        size_of::<Vec<FormalMemoryAccess>>(),
        size_of::<Vec<FormalMemoryIncompleteReason>>(),
        size_of::<Vec<FormalBoundsRequirement>>(),
        size_of::<Vec<FunctionOperationLocation>>(),
        size_of::<Vec<(FormalAllocationIdentity, AllocationEnvelope)>>(),
        size_of::<Vec<RuntimeAliasRequirement>>(),
        size_of::<Vec<InterInvocationConflictRequirement>>(),
        size_of::<String>() * 3,
        size_of::<(InvocationRange1d, Option<InvocationRange1d>, AccessResult)>(),
        // The construction closure itself is measured at its actual call
        // site. This is the outer catch's budget/closure/size borrow envelope.
        size_of::<(&mut Budget<'_>, &mut (), &usize)>(),
    ]
    .into_iter()
    .try_fold(0_usize, |sum, bytes| sum.checked_add(bytes))
    .ok_or_else(|| ResourceError::Arithmetic.into())
}

impl<'pointer, 'borrow, 'affine, 'owner, 'work>
    ActualOwnerAccessesV18<'pointer, 'borrow, 'affine, 'owner, 'work>
{
    /// Consumes the same paid original analyses in the real report algorithms.
    /// Extent/width remain descriptive inputs; no final-admission gate is used.
    pub(in crate::formal_memory_obligations) fn with_report_v19<'budget_work, F>(
        &mut self,
        owner: &VerifiedCanonicalKernelIrModuleV18,
        root_index: usize,
        effects: &CanonicalEffectScopeV19<'owner>,
        launch: ExplicitLaunchExtent,
        width: FormalIndexWidth,
        interpretation: PhysicalLaunchInterpretationV2,
        budget: &mut Budget<'budget_work>,
        consume: F,
    ) -> BodyResult<()>
    where
        F: for<'view> FnOnce(
            &ActualOwnerReportViewV19<'view, 'owner>,
            &mut Budget<'budget_work>,
        ) -> BodyResult<()>,
    {
        // Reject foreign account identity before consulting or changing an
        // existing report failure. Rejected captures cannot mask that result.
        if let Err(error) = self.pointers.slots.affine.identity(budget) {
            drain_v19(consume);
            return Err(error.into());
        }
        if let Some(error) = &self.report_failure {
            let error = error.clone();
            drain_v19(consume);
            return Err(error);
        }
        if let Err(error) = self.check(owner, root_index, budget) {
            drain_v19(consume);
            return Err(error.into());
        }
        let mut pending = Some(consume);
        let mut state = ReportScopeStateV19 {
            slot: budget as *mut Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            required: None,
        };
        let mut construction = |budget: &mut Budget<'budget_work>| -> BodyResult<()> {
            budget.charge_work(4)?;
            let report =
                construct_report_v19(self, effects, launch, width, interpretation, budget)?;
            state.required = Some(budget.storage());
            let view = ActualOwnerReportViewV19 {
                owner: self.pointers.slots.affine.owner,
                source: self.pointers.slots.affine.source,
                root_index,
                launch_extent: launch,
                interpretation,
                report: &report,
            };
            let consume = pending.take().ok_or(ResourceError::Accounting)?;
            let result = consume(&view, budget);
            if result.is_ok() {
                // A consumer can query the same outer effects scope. An
                // ignored denial there cannot complete this report scope.
                self.check(owner, root_index, budget)?;
                effects.function(owner, self.pointers.slots.affine.source, budget)?;
            }
            result
        };
        let construction_bytes = std::mem::size_of_val(&construction);
        let outcome = catch_unwind(AssertUnwindSafe(|| -> BodyResult<()> {
            budget.reserve_storage(
                frame_v19::<F>()?
                    .checked_add(construction_bytes)
                    .ok_or(ResourceError::Arithmetic)?,
            )?;
            construction(budget)
        }));
        drop(construction);
        // The report and all row/string backing were dropped by this point,
        // before either selected errors or resource credit can be returned.
        let selected = if !state.same(budget) {
            Some(ResourceError::Accounting.into())
        } else if let Err(error) = prior_denial_v2(budget) {
            Some(BodyErrorV19::Source(error.into()))
        } else if state
            .required
            .is_some_and(|required| budget.storage() != required)
            || budget.storage() < state.floor
        {
            Some(ResourceError::Accounting.into())
        } else {
            None
        };
        let result = match outcome {
            Ok(result) => result,
            Err(payload) => {
                drain_v19(payload);
                Err(BodyErrorV19::Panicked)
            }
        };
        let mut result = selected.map_or(result, Err);
        drain_v19(pending);
        // Rejected captures are trusted compiler code too. Their destruction
        // cannot authorize success after changing account identity or custody.
        let after_drain = if !state.same(budget) {
            Some(ResourceError::Accounting.into())
        } else if let Err(error) = prior_denial_v2(budget) {
            Some(BodyErrorV19::Source(error.into()))
        } else if state
            .required
            .is_some_and(|required| budget.storage() != required)
            || budget.storage() < state.floor
        {
            Some(ResourceError::Accounting.into())
        } else {
            None
        };
        if result.is_ok() {
            if let Some(error) = after_drain {
                result = Err(error);
            }
        }
        let cleanup = if !state.same(budget) {
            Err(ResourceError::Accounting)
        } else if let Some(required) = state.required {
            if budget.storage() < required {
                Err(ResourceError::Accounting)
            } else {
                // Retire only this scope's credit. Excess callback storage is
                // an error and stays charged, never refunded as report memory.
                budget
                    .release_storage(required - state.floor)
                    .map_err(Into::into)
            }
        } else {
            budget.rollback_storage(state.floor).map_err(Into::into)
        };
        let result = result.and_then(|()| cleanup.map_err(Into::into));
        if let Err(error) = &result {
            self.report_failure = Some(error.clone());
            let source_error = match error {
                BodyErrorV19::Source(error) => error.clone(),
                _ => ResourceError::Accounting.into(),
            };
            let _ = self.pointers.slots.affine.keep::<()>(Err(source_error));
        }
        result
    }
}
