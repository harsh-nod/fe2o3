// Independent source/native replay. Candidate rows are locators, never facts.
// This rule checks the complete affected closure, not unrelated root semantics.
// Suffix noninterference is diagnostic in this snapshot and remains rejecting
// until sealed coverage and the existing whole-source gate compose it.

#[derive(Clone, Copy, Debug)]
enum BorrowedReplayValueV1 {
    Uninitialized,
    Unit,
    Native(ValueId),
    Owner(usize),
    Reference(usize),
}

struct BorrowedReplayReferenceV1 {
    group: usize,
    owner: usize,
    access: AccessMode,
    values: Vec<(usize, ValueId)>,
}

struct BorrowedReplaySpanV1<'a> {
    block: &'a BasicBlock,
    next: usize,
    end: usize,
}

impl BorrowedReplaySpanV1<'_> {
    fn operation(
        &mut self,
        kind: OperationKind,
        ty: Option<Type>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<ValueId>, ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        if self.next >= self.end {
            return Err(borrowed_replay_mismatch_v1());
        }
        let operation = &self.block.operations[self.next];
        self.next += 1;
        if operation.kind != kind {
            return Err(borrowed_replay_mismatch_v1());
        }
        match (ty, operation.results.as_slice()) {
            (None, []) => Ok(None),
            (Some(ty), [value]) if value.ty == ty => Ok(Some(value.id)),
            _ => Err(borrowed_replay_mismatch_v1()),
        }
    }

    fn value(
        &mut self,
        kind: OperationKind,
        ty: Type,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        self.operation(kind, Some(ty), budget)?
            .ok_or_else(borrowed_replay_mismatch_v1)
    }

    fn restrict_pointer(
        &mut self,
        value: ValueId,
        expected: &Type,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<ValueId, ProductionSemanticKirErrorV1> {
        budget.charge_work(12)?;
        if !matches!(expected, Type::Pointer(pointer)
            if matches!(pointer.pointee.as_ref(), Type::Scalar(_)))
        {
            return Err(borrowed_replay_unsupported_v1());
        }
        if self.next >= self.end {
            return Err(borrowed_replay_mismatch_v1());
        }
        let operation = &self.block.operations[self.next];
        if !matches!(&operation.kind, OperationKind::Cast {
            kind: CastKind::RestrictPointerAccess, value: actual, to,
        } if *actual == value && to == expected)
        {
            return Err(borrowed_replay_mismatch_v1());
        }
        let [result] = operation.results.as_slice() else {
            return Err(borrowed_replay_mismatch_v1());
        };
        if &result.ty != expected {
            return Err(borrowed_replay_mismatch_v1());
        }
        self.next += 1;
        Ok(result.id)
    }

    fn call(
        &mut self,
        callee: &FunctionId,
        arguments: &[ValueId],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        if self.next >= self.end {
            return Err(borrowed_replay_mismatch_v1());
        }
        let operation = &self.block.operations[self.next];
        let OperationKind::Call {
            callee: actual_callee,
            arguments: actual_arguments,
        } = &operation.kind
        else {
            return Err(borrowed_replay_mismatch_v1());
        };
        budget.charge_work(argument_sum_v1(&[
            callee.as_str().len().min(actual_callee.as_str().len()),
            arguments.len().min(actual_arguments.len()),
        ])?)?;
        if actual_callee != callee || actual_arguments != arguments || !operation.results.is_empty()
        {
            return Err(borrowed_replay_mismatch_v1());
        }
        self.next += 1;
        Ok(())
    }

    fn finish(&self) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.next != self.end {
            return Err(borrowed_replay_mismatch_v1());
        }
        Ok(())
    }
}

fn borrowed_replay_mismatch_v1() -> ProductionSemanticKirErrorV1 {
    ProductionSemanticKirErrorV1::CorrespondenceMismatch
}

fn borrowed_replay_effect_error_v1(
    error: fe2o3_kernel_analysis::CanonicalKirCallEffectErrorV1,
) -> ProductionSemanticKirErrorV1 {
    match error {
        fe2o3_kernel_analysis::CanonicalKirCallEffectErrorV1::Resource(error) => error.into(),
        _ => borrowed_replay_mismatch_v1(),
    }
}

fn borrowed_replay_unsupported_v1() -> ProductionSemanticKirErrorV1 {
    unsupported(
        0,
        None,
        None,
        "borrowed aggregate is outside the checked straight-line source subset",
    )
}

struct BorrowedReplayV1<'a, 'i> {
    subject: CanonicalCallSubjectV1<'a>,
    inventory: &'i CanonicalKirInventoryV1<'a>,
    candidates: BorrowedAggregateReplayCandidatesV1<'a>,
    groups: &'i [CanonicalCallGroupV1<'a>],
    calls: &'i [CanonicalCallBindingV1<'a>],
    root: SemanticFunctionIdV1,
    initialized: Vec<bool>,
    allocated: Vec<bool>,
    call_claims: Vec<bool>,
    function_claims: Vec<bool>,
    active: Vec<bool>,
    references: Vec<BorrowedReplayReferenceV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct BorrowedAggregateFunctionCoverageV1 {
    root: SemanticFunctionIdV1,
    function: SemanticFunctionIdV1,
    physical: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1,
    role: SemanticKirFunctionRoleV1,
}

// Move-only rows for attachment beside the exact checked source/correspondence
// and executable, like SealedUnitLocalSourceV1. Identities are attachment
// cross-checks, not a detached admission API or a substitute for replay.
#[derive(Debug)]
struct SealedBorrowedAggregateReplayV1 {
    source_identity: ProductionSemanticSsaIdentityV1,
    graph_identity: fe2o3_kernel_ir::VerifiedCanonicalKernelIrIdentityV12,
    functions: Vec<BorrowedAggregateFunctionCoverageV1>,
    storage: usize,
}

impl SealedBorrowedAggregateReplayV1 {
    fn functions(&self) -> &[BorrowedAggregateFunctionCoverageV1] {
        &self.functions
    }

    fn retained_storage(&self) -> usize {
        self.storage
    }
}

// Root construction, equivalence and source/output replay must all invoke this
// gate on the same final subject. This function neither lowers nor copies KIR.
// Success retains the returned seal's charge; refusal/unwind restores scratch.
fn check_borrowed_aggregate_replay_v1(
    subject: CanonicalCallSubjectV1<'_>,
    inventory: &CanonicalKirInventoryV1<'_>,
    candidates: BorrowedAggregateReplayCandidatesV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<SealedBorrowedAggregateReplayV1, ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    let ledger = budget.work_ledger_identity_v1();
    let floor = budget.storage();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        budget.charge_work(8)?;
        if !inventory.belongs_to(subject.executable)
            || candidates.fields.is_empty()
            || candidates.calls.is_empty()
        {
            return Err(borrowed_replay_mismatch_v1());
        }
        let (groups, calls) = build_canonical_call_index_v1(subject, inventory, budget)?;
        let (effects, storage) =
            fe2o3_kernel_analysis::CanonicalKirCallEffectsV1::derive(inventory, budget)
                .map_err(borrowed_replay_effect_error_v1)?;
        budget.reserve_storage(storage.retained_storage())?;
        budget.charge_work(groups.len())?;
        let mut roots = groups.iter().enumerate().filter(|(_, group)| {
            group.function.source.role == SemanticKirFunctionRoleV1::KernelEntry
        });
        let (root_group, root) = roots.next().ok_or_else(borrowed_replay_mismatch_v1)?;
        if roots.next().is_some() || groups.len() != inventory.functions().len() {
            return Err(borrowed_replay_unsupported_v1());
        }
        let root = root.function.source.correspondence_owner;
        for group in &groups {
            budget.charge_work(2)?;
            if group.function.source.correspondence_owner != root
                || effects
                    .decision(group.function.canonical.coordinate, budget)
                    .map_err(borrowed_replay_effect_error_v1)?
                    == fe2o3_kernel_analysis::CanonicalKirCallEffectDecisionV1::Incomplete
            {
                return Err(borrowed_replay_unsupported_v1());
            }
        }
        budget.reserve_storage(std::mem::size_of::<BorrowedReplayV1<'_, '_>>())?;
        let mut replay = BorrowedReplayV1 {
            subject,
            inventory,
            candidates,
            groups: &groups,
            calls: &calls,
            root,
            initialized: unit_local_filled_v1(candidates.fields.len(), false, budget)?,
            allocated: unit_local_filled_v1(candidates.fields.len(), false, budget)?,
            call_claims: unit_local_filled_v1(candidates.calls.len(), false, budget)?,
            function_claims: unit_local_filled_v1(groups.len(), false, budget)?,
            active: unit_local_filled_v1(groups.len(), false, budget)?,
            references: Vec::new(),
        };
        replay.check_fields(root_group, budget)?;
        replay.function(root_group, None, 0, budget)?;
        budget.charge_work(argument_sum_v1(&[
            replay.initialized.len(),
            replay.allocated.len(),
            replay.call_claims.len(),
            replay.function_claims.len(),
            replay.active.len(),
        ])?)?;
        if replay.initialized.iter().any(|v| !v)
            || replay.allocated.iter().any(|v| !v)
            || replay.call_claims.iter().any(|v| !v)
            || replay.function_claims.iter().any(|v| !v)
            || replay.active.iter().any(|v| *v)
        {
            return Err(borrowed_replay_mismatch_v1());
        }
        budget.reserve_storage(std::mem::size_of::<SealedBorrowedAggregateReplayV1>())?;
        let mut functions = unit_local_vec_v1(groups.len(), budget)?;
        let mut covered = unit_local_filled_v1(inventory.functions().len(), false, budget)?;
        for group in &groups {
            budget.charge_work(8)?;
            let source = group.function.source;
            let canonical = group.function.canonical;
            let declaration = subject
                .semantic_ssa
                .source_semantic()
                .functions()
                .get(source.semantic_function.index() as usize)
                .ok_or_else(borrowed_replay_mismatch_v1)?;
            let seen = covered
                .get_mut(canonical.coordinate.0 as usize)
                .ok_or_else(borrowed_replay_mismatch_v1)?;
            if *seen {
                return Err(borrowed_replay_mismatch_v1());
            }
            *seen = true;
            match (source.role, declaration.role(), canonical.function.role) {
                (
                    SemanticKirFunctionRoleV1::KernelEntry,
                    SemanticFunctionRoleV1::KernelRoot,
                    fe2o3_kernel_ir::FunctionRole::KernelEntry,
                ) if source.semantic_function == root => {}
                (
                    SemanticKirFunctionRoleV1::InternalHelper,
                    SemanticFunctionRoleV1::InternalHelper,
                    fe2o3_kernel_ir::FunctionRole::InternalHelper,
                ) => {}
                _ => return Err(borrowed_replay_mismatch_v1()),
            }
            unit_local_push_v1(
                &mut functions,
                BorrowedAggregateFunctionCoverageV1 {
                    root,
                    function: source.semantic_function,
                    physical: canonical.coordinate,
                    role: source.role,
                },
                budget,
            )?;
        }
        budget.charge_work(covered.len())?;
        if covered.iter().any(|v| !v) {
            return Err(borrowed_replay_mismatch_v1());
        }
        let storage = argument_sum_v1(&[
            std::mem::size_of::<SealedBorrowedAggregateReplayV1>(),
            argument_product_v1(
                functions.capacity(),
                std::mem::size_of::<BorrowedAggregateFunctionCoverageV1>(),
            )?,
        ])?;
        Ok(SealedBorrowedAggregateReplayV1 {
            source_identity: subject.semantic_ssa.identity(),
            graph_identity: *subject.executable.canonical().identity(),
            functions,
            storage,
        })
    }));
    if ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let retained = match &result {
        Ok(Ok(sealed)) => sealed.retained_storage(),
        _ => 0,
    };
    budget.release_storage(
        budget
            .storage()
            .checked_sub(floor)
            .and_then(|live| live.checked_sub(retained))
            .ok_or(ArgumentResourceV1::Accounting)?,
    )?;
    match result {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

include!("production_borrowed_aggregate_replay_control_v1.rs");
include!("production_borrowed_aggregate_replay_values_v1.rs");

fn borrowed_replay_native_v1(
    value: BorrowedReplayValueV1,
) -> Result<ValueId, ProductionSemanticKirErrorV1> {
    match value {
        BorrowedReplayValueV1::Native(value) => Ok(value),
        _ => Err(borrowed_replay_unsupported_v1()),
    }
}

fn borrowed_replay_statement_supported_v1(kind: &SemanticStatementKindV1) -> bool {
    match kind {
        SemanticStatementKindV1::Assign(assignment) => matches!(
            assignment.value().kind(),
            SemanticRvalueKindV1::Use(_)
                | SemanticRvalueKindV1::Borrow { .. }
                | SemanticRvalueKindV1::Aggregate(_)
                | SemanticRvalueKindV1::Length(_)
        ),
        SemanticStatementKindV1::StorageLive(_)
        | SemanticStatementKindV1::StorageDead(_)
        | SemanticStatementKindV1::Nop => true,
        _ => false,
    }
}

fn borrowed_replay_suffix_local_v1(
    local: SemanticLocalIdV1,
    forbidden: &[bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    if *forbidden
        .get(local.index() as usize)
        .ok_or_else(borrowed_replay_mismatch_v1)?
    {
        return Err(borrowed_replay_unsupported_v1());
    }
    Ok(())
}

fn borrowed_replay_suffix_place_v1(
    place: &SemanticPlaceV1,
    forbidden: &[bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    borrowed_replay_suffix_local_v1(place.local(), forbidden, budget)?;
    for projection in place.projections() {
        budget.charge_work(1)?;
        if let SemanticProjectionKindV1::Index(local) = projection.kind() {
            borrowed_replay_suffix_local_v1(local, forbidden, budget)?;
        }
    }
    Ok(())
}

fn borrowed_replay_suffix_operand_v1(
    operand: &SemanticOperandV1,
    forbidden: &[bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            borrowed_replay_suffix_place_v1(place, forbidden, budget)
        }
        SemanticOperandV1::Constant(_) => Ok(()),
    }
}

fn borrowed_replay_suffix_statement_v1(
    kind: &SemanticStatementKindV1,
    forbidden: &[bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    match kind {
        SemanticStatementKindV1::Assign(assignment) => {
            borrowed_replay_suffix_place_v1(assignment.destination(), forbidden, budget)?;
            let value = assignment.value().kind();
            value.try_visit_operands(|operand| {
                borrowed_replay_suffix_operand_v1(operand, forbidden, budget)
            })?;
            match value {
                SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. }
                | SemanticRvalueKindV1::Length(place)
                | SemanticRvalueKindV1::Discriminant(place) => {
                    borrowed_replay_suffix_place_v1(place, forbidden, budget)
                }
                SemanticRvalueKindV1::Load(load) => {
                    borrowed_replay_suffix_place_v1(load.source(), forbidden, budget)
                }
                SemanticRvalueKindV1::Use(_)
                | SemanticRvalueKindV1::Unary { .. }
                | SemanticRvalueKindV1::Binary { .. }
                | SemanticRvalueKindV1::CheckedBinary(_)
                | SemanticRvalueKindV1::UncheckedBinary(_)
                | SemanticRvalueKindV1::Cast { .. }
                | SemanticRvalueKindV1::Aggregate(_) => Ok(()),
            }
        }
        SemanticStatementKindV1::Store(store) => {
            borrowed_replay_suffix_place_v1(store.destination(), forbidden, budget)?;
            borrowed_replay_suffix_operand_v1(store.value(), forbidden, budget)
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. }
        | SemanticStatementKindV1::Deinitialize(place) => {
            borrowed_replay_suffix_place_v1(place, forbidden, budget)
        }
        SemanticStatementKindV1::StorageLive(local) => {
            borrowed_replay_suffix_local_v1(*local, forbidden, budget)
        }
        // No alias is removed here. Every surviving source alias and physical
        // carrier remains forbidden throughout the complete suffix census.
        SemanticStatementKindV1::StorageDead(_) | SemanticStatementKindV1::Nop => Ok(()),
        SemanticStatementKindV1::Assume(value) => {
            borrowed_replay_suffix_operand_v1(value, forbidden, budget)
        }
        SemanticStatementKindV1::AtomicRmw(_)
        | SemanticStatementKindV1::AtomicCompareExchange(_) => {
            Err(borrowed_replay_unsupported_v1())
        }
    }
}

fn borrowed_replay_suffix_terminator_v1(
    kind: &SemanticTerminatorKindV1,
    callables: &[SemanticCallableDeclV1],
    forbidden: &[bool],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticAssertMessageV1;
    budget.charge_work(2)?;
    match kind {
        SemanticTerminatorKindV1::Call(call) => {
            if !matches!(
                callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic { .. })
            ) || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
            {
                return Err(borrowed_replay_unsupported_v1());
            }
            for argument in call.arguments() {
                borrowed_replay_suffix_operand_v1(argument, forbidden, budget)?;
            }
            if let Some(destination) = call.destination() {
                borrowed_replay_suffix_place_v1(destination.place(), forbidden, budget)?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            borrowed_replay_suffix_operand_v1(discriminant, forbidden, budget)
        }
        SemanticTerminatorKindV1::Assert {
            condition,
            message,
            unwind,
            ..
        } => {
            if !matches!(unwind, SemanticUnwindActionV1::Unreachable) {
                return Err(borrowed_replay_unsupported_v1());
            }
            borrowed_replay_suffix_operand_v1(condition, forbidden, budget)?;
            match message {
                SemanticAssertMessageV1::BoundsCheck {
                    length: left,
                    index: right,
                }
                | SemanticAssertMessageV1::Overflow { left, right, .. }
                | SemanticAssertMessageV1::MisalignedPointerDereference {
                    required_alignment: left,
                    found_alignment: right,
                } => {
                    borrowed_replay_suffix_operand_v1(left, forbidden, budget)?;
                    borrowed_replay_suffix_operand_v1(right, forbidden, budget)
                }
                SemanticAssertMessageV1::DivisionByZero(value)
                | SemanticAssertMessageV1::RemainderByZero(value) => {
                    borrowed_replay_suffix_operand_v1(value, forbidden, budget)
                }
                SemanticAssertMessageV1::NullPointerDereference
                | SemanticAssertMessageV1::ResumedAfterReturn
                | SemanticAssertMessageV1::ResumedAfterPanic => Ok(()),
            }
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::Unreachable
        | SemanticTerminatorKindV1::Abort => Ok(()),
        SemanticTerminatorKindV1::TailCall(_)
        | SemanticTerminatorKindV1::Drop { .. }
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate => Err(borrowed_replay_unsupported_v1()),
    }
}

#[cfg(test)]
#[path = "production_borrowed_aggregate_replay_v1_tests.rs"]
mod production_borrowed_aggregate_replay_v1_tests;
