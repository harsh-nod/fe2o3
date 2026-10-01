use super::*;

#[path = "adapter_emission_v1.rs"]
pub(super) mod emission_v1;

#[path = "adapter_prepared_v1.rs"]
pub(super) mod prepared_v1;

#[path = "adapter_shared_primitive_v29.rs"]
pub(super) mod shared_primitive_v29;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct SemanticTransparentBorrowSiteV1 {
    block: u32,
    statement: u32,
}

#[cfg(test)]
impl SemanticTransparentBorrowSiteV1 {
    pub(super) fn test_coordinates(&self) -> (u32, u32) {
        (self.block, self.statement)
    }
}

#[derive(Clone, Copy)]
enum SemanticBorrowOriginV1 {
    Statement(SemanticTransparentBorrowSiteV1),
    Copy(SemanticTransparentBorrowSiteV1),
    Parameter(u32),
}

#[derive(Clone, Copy)]
struct SemanticBorrowCandidateV1 {
    origin: SemanticBorrowOriginV1,
    reference_local: u32,
    reference_type: SemanticTypeIdV1,
    source_local: u32,
    source_type: SemanticTypeIdV1,
    source_reference: Option<u32>,
    parent: Option<usize>,
    shared: bool,
    valid: bool,
    consumers: usize,
    closed_consumers: usize,
}

pub(super) fn transparent_borrow_sites_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
) -> BTreeSet<SemanticTransparentBorrowSiteV1> {
    analyze_borrow_uses_v29(function, callables, &[], None).0
}

pub(super) fn borrow_graph_resources_v41(
    function: &SemanticFunctionDeclV1,
    roots: usize,
) -> Result<SemanticSsaAuxiliaryResourcesV1, ProductionSemanticSsaErrorV1> {
    let overflow = || ProductionSemanticSsaErrorV1::ResourceOverflow;
    let mut statements = 0_usize;
    let mut borrows = roots;
    let mut copies = 0_usize;
    for block in function.blocks() {
        statements = statements
            .checked_add(block.statements().len())
            .ok_or_else(overflow)?;
        for statement in block.statements() {
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
                match assignment.value().kind() {
                    SemanticRvalueKindV1::Borrow { .. } => {
                        borrows = borrows.checked_add(1).ok_or_else(overflow)?
                    }
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(_)) => {
                        copies = copies.checked_add(1).ok_or_else(overflow)?
                    }
                    _ => {}
                }
            }
        }
    }
    if borrows == 0 {
        return Ok(SemanticSsaAuxiliaryResourcesV1::default());
    }
    let candidates = borrows.checked_add(copies).ok_or_else(overflow)?;
    let height = (usize::BITS
        - candidates
            .checked_add(function.locals().len())
            .ok_or_else(overflow)?
            .max(1)
            .leading_zeros()) as usize
        + 1;
    // Includes type nomination, alias/occurrence tree nodes and the concurrent
    // completion, grounding and path buffers. Every graph edge is visited once
    // per closure phase; the tree factor is for collection and publication.
    let work_units = statements
        .checked_add(function.blocks().len())
        .and_then(|value| value.checked_add(candidates))
        .and_then(|value| value.checked_mul(32))
        .and_then(|value| value.checked_mul(height))
        .and_then(|value| value.checked_add(candidates.checked_mul(32)?))
        .ok_or_else(overflow)?;
    let storage_words = candidates
        .checked_mul(64)
        .and_then(|value| value.checked_add(64))
        .ok_or_else(overflow)?;
    Ok(SemanticSsaAuxiliaryResourcesV1 {
        storage_words,
        work_units,
    })
}

pub(super) fn analyze_borrow_uses_v29(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    parameters: &[Option<super::nominal_reference_effects_v29::NominalReferenceParameterV29>],
    effects: Option<&super::nominal_reference_effects_v29::NominalReferenceEffectsV29>,
) -> (BTreeSet<SemanticTransparentBorrowSiteV1>, BTreeSet<u32>) {
    let mut candidates = Vec::new();
    let mut candidate_by_reference = BTreeMap::<u32, usize>::new();
    let mut duplicate_references = BTreeSet::new();
    let mut reference_types = BTreeSet::new();
    for parameter in parameters.iter().flatten() {
        reference_types.insert(parameter.reference_type);
        candidate_by_reference.insert(parameter.local, candidates.len());
        candidates.push(SemanticBorrowCandidateV1 {
            origin: SemanticBorrowOriginV1::Parameter(parameter.ordinal),
            reference_local: parameter.local,
            reference_type: parameter.reference_type,
            source_local: parameter.local,
            source_type: parameter.pointee,
            source_reference: None,
            parent: None,
            shared: function.abi().source_argument_ownership().get(parameter.ordinal as usize)
                == Some(&fe2o3_mir_model::semantic_mir_v1::SemanticSourceArgumentOwnershipV1::SharedBorrow),
            valid: true,
            consumers: 0,
            closed_consumers: 0,
        });
    }
    let parameter_count = candidates.len();
    for block in function.blocks() {
        for statement in block.statements() {
            if let SemanticStatementKindV1::Assign(assignment) = statement.kind()
                && matches!(
                    assignment.value().kind(),
                    SemanticRvalueKindV1::Borrow { .. }
                )
                && assignment.destination().projections().is_empty()
            {
                reference_types.insert(assignment.destination().ty());
            }
        }
    }
    if reference_types.is_empty() {
        return (BTreeSet::new(), BTreeSet::new());
    }
    for (block_index, block) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() {
                continue;
            }
            let site = SemanticTransparentBorrowSiteV1 {
                block: block_index as u32,
                statement: statement_index as u32,
            };
            let (place, origin, source_reference, shared) = match assignment.value().kind() {
                SemanticRvalueKindV1::Borrow { kind, place } => {
                    let source_reference = match place.projections() {
                        [] => None,
                        [projection]
                            if projection.kind() == SemanticProjectionKindV1::Dereference =>
                        {
                            Some(place.local().index())
                        }
                        _ => continue,
                    };
                    (
                        place,
                        SemanticBorrowOriginV1::Statement(site),
                        source_reference,
                        *kind == fe2o3_mir_model::semantic_mir_v1::SemanticBorrowKindV1::Shared,
                    )
                }
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place))
                    if place.projections().is_empty()
                        && place.ty() == assignment.destination().ty()
                        && place.ty() == assignment.value().result_type()
                        && reference_types.contains(&place.ty()) =>
                {
                    (
                        place,
                        SemanticBorrowOriginV1::Copy(site),
                        Some(place.local().index()),
                        false,
                    )
                }
                _ => continue,
            };
            let reference_local = assignment.destination().local().index();
            let candidate = SemanticBorrowCandidateV1 {
                origin,
                reference_local,
                reference_type: assignment.destination().ty(),
                source_local: place.local().index(),
                source_type: place.ty(),
                source_reference,
                parent: None,
                shared,
                valid: true,
                consumers: 0,
                closed_consumers: 0,
            };
            let index = candidates.len();
            candidates.push(candidate);
            if !duplicate_references.contains(&reference_local)
                && let Some(previous) = candidate_by_reference.insert(reference_local, index)
            {
                if matches!(
                    candidates[previous].origin,
                    SemanticBorrowOriginV1::Parameter(_)
                ) {
                    candidates[previous].valid = false;
                }
                candidate_by_reference.remove(&reference_local);
                duplicate_references.insert(reference_local);
            }
        }
    }
    if candidates.is_empty() {
        return (BTreeSet::new(), BTreeSet::new());
    }
    let return_local = (!matches!(
        function.abi().return_value().mode(),
        SemanticAbiPassModeV1::Ignore,
    ))
    .then(|| {
        function
            .locals()
            .iter()
            .position(|local| matches!(local.role(), SemanticLocalRoleV1::Return))
    })
    .flatten();
    let mut unscoped_references = BTreeSet::new();
    let mut events = Vec::new();
    let mut next_candidate = parameter_count;
    // Repeated MIR temporaries are indexed by their definition occurrence within
    // a block. A use without a local definition disqualifies every occurrence
    // of that temporary; this is not a cross-block reference alias analysis.
    for (block_index, block) in function.blocks().iter().enumerate() {
        let first_candidate = next_candidate;
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let site = SemanticTransparentBorrowSiteV1 {
                block: block_index as u32,
                statement: statement_index as u32,
            };
            events.clear();
            if !duplicate_references.is_empty() {
                append_statement_events_v1(statement.kind(), &mut events);
                record_unscoped_reference_uses_v1(
                    &events,
                    &duplicate_references,
                    &candidate_by_reference,
                    &mut unscoped_references,
                );
            }
            let definition = candidates
                .get(next_candidate)
                .filter(|candidate| {
                    matches!(candidate.origin, SemanticBorrowOriginV1::Statement(origin) | SemanticBorrowOriginV1::Copy(origin) if origin == site)
                })
                .map(|candidate| candidate.reference_local);
            if let Some(reference) = definition {
                if duplicate_references.contains(&reference) {
                    candidate_by_reference.insert(reference, next_candidate);
                }
                next_candidate += 1;
            }
            invalidate_reference_uses_in_statement_v1(
                statement.kind(),
                site,
                &candidate_by_reference,
                &mut candidates,
            );
            for event in &events {
                let reference = event.variable().get();
                if duplicate_references.contains(&reference)
                    && (matches!(event, SsaEventV1::Kill(_))
                        || (matches!(event, SsaEventV1::Define(_))
                            && definition != Some(reference)))
                {
                    candidate_by_reference.remove(&reference);
                }
            }
        }
        events.clear();
        if !duplicate_references.is_empty() {
            append_terminator_events_v1(block.terminator().kind(), return_local, &mut events);
            record_unscoped_reference_uses_v1(
                &events,
                &duplicate_references,
                &candidate_by_reference,
                &mut unscoped_references,
            );
        }
        validate_reference_uses_in_terminator_v1(
            block.terminator().kind(),
            return_local,
            callables,
            effects,
            &candidate_by_reference,
            &mut candidates,
        );
        for candidate in &candidates[first_candidate..next_candidate] {
            if duplicate_references.contains(&candidate.reference_local) {
                candidate_by_reference.remove(&candidate.reference_local);
            }
        }
    }
    for candidate in &mut candidates {
        if unscoped_references.contains(&candidate.reference_local) {
            candidate.valid = false;
        }
    }

    // A shared copy may fan out, but every outgoing use must close at an
    // authenticated consumer. A successful sibling cannot hide an escape.
    let mut complete = vec![false; candidates.len()];
    let mut pending = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        if candidate.valid
            && candidate.consumers != 0
            && (candidate.shared || candidate.consumers == 1)
            && candidate.closed_consumers == candidate.consumers
        {
            pending.push(index);
        }
    }
    while let Some(index) = pending.pop() {
        if complete[index] {
            continue;
        }
        complete[index] = true;
        if let Some(parent_index) = candidates[index].parent {
            let parent = &mut candidates[parent_index];
            parent.closed_consumers = parent.closed_consumers.saturating_add(1);
            if parent.valid
                && parent.consumers != 0
                && (parent.shared || parent.consumers == 1)
                && parent.closed_consumers == parent.consumers
            {
                pending.push(parent_index);
            }
        }
    }
    // Memoize whether a completed node is grounded at an original borrow or
    // parameter. Shared ancestors are not re-walked for each fan-out leaf.
    let mut grounded = vec![0_u8; candidates.len()];
    let mut chain = Vec::new();
    for terminal in 0..candidates.len() {
        if grounded[terminal] != 0 {
            continue;
        }
        chain.clear();
        let mut current = terminal;
        let accepted = loop {
            if grounded[current] != 0 {
                break grounded[current] == 2;
            }
            let candidate = candidates[current];
            if !complete[current] {
                break false;
            }
            grounded[current] = 1;
            chain.push(current);
            if candidate.source_reference.is_none() {
                break true;
            }
            let Some(parent) = candidate.parent else {
                break false;
            };
            current = parent;
        };
        for index in &chain {
            grounded[*index] = if accepted { 2 } else { 3 };
        }
    }
    // Parameter roots describe body effects only. They must never become
    // fabricated statement sites or SSA entry definitions.
    let mut sites = BTreeSet::new();
    let mut closed = BTreeSet::new();
    for (candidate, status) in candidates.iter().zip(grounded) {
        if status != 2 {
            continue;
        }
        match candidate.origin {
            SemanticBorrowOriginV1::Statement(site) => {
                sites.insert(site);
            }
            SemanticBorrowOriginV1::Parameter(ordinal) => {
                closed.insert(ordinal);
            }
            SemanticBorrowOriginV1::Copy(_) => {}
        }
    }
    for candidate in &candidates[..parameter_count] {
        if candidate.valid
            && candidate.consumers == 0
            && let SemanticBorrowOriginV1::Parameter(ordinal) = candidate.origin
        {
            closed.insert(ordinal);
        }
    }
    (sites, closed)
}

fn record_unscoped_reference_uses_v1(
    events: &[SsaEventV1],
    duplicate_references: &BTreeSet<u32>,
    candidate_by_reference: &BTreeMap<u32, usize>,
    unscoped_references: &mut BTreeSet<u32>,
) {
    for event in events {
        let reference = event.variable().get();
        if matches!(event, SsaEventV1::Use(_))
            && duplicate_references.contains(&reference)
            && !candidate_by_reference.contains_key(&reference)
        {
            unscoped_references.insert(reference);
        }
    }
}

fn invalidate_reference_place_v1(
    place: &SemanticPlaceV1,
    candidate_by_reference: &BTreeMap<u32, usize>,
    candidates: &mut [SemanticBorrowCandidateV1],
) {
    if let Some(candidate) = candidate_by_reference.get(&place.local().index()) {
        candidates[*candidate].valid = false;
    }
    for projection in place.projections() {
        if let SemanticProjectionKindV1::Index(local) = projection.kind()
            && let Some(candidate) = candidate_by_reference.get(&local.index())
        {
            candidates[*candidate].valid = false;
        }
    }
}

fn invalidate_reference_operand_v1(
    operand: &SemanticOperandV1,
    candidate_by_reference: &BTreeMap<u32, usize>,
    candidates: &mut [SemanticBorrowCandidateV1],
) {
    if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand {
        invalidate_reference_place_v1(place, candidate_by_reference, candidates);
    }
}

fn invalidate_reference_rvalue_v1(
    value: &SemanticRvalueKindV1,
    candidate_by_reference: &BTreeMap<u32, usize>,
    candidates: &mut [SemanticBorrowCandidateV1],
) {
    value
        .try_visit_operands::<std::convert::Infallible>(|operand| {
            invalidate_reference_operand_v1(operand, candidate_by_reference, candidates);
            Ok(())
        })
        .expect("infallible reference-use scan");
    match value {
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. }
        | SemanticRvalueKindV1::Length(place)
        | SemanticRvalueKindV1::Discriminant(place) => {
            invalidate_reference_place_v1(place, candidate_by_reference, candidates)
        }
        SemanticRvalueKindV1::Load(load) => {
            invalidate_reference_place_v1(load.source(), candidate_by_reference, candidates)
        }
        SemanticRvalueKindV1::Use(_)
        | SemanticRvalueKindV1::Unary { .. }
        | SemanticRvalueKindV1::Binary { .. }
        | SemanticRvalueKindV1::CheckedBinary(_)
        | SemanticRvalueKindV1::UncheckedBinary(_)
        | SemanticRvalueKindV1::Cast { .. }
        | SemanticRvalueKindV1::Aggregate(_) => {}
    }
}

fn invalidate_reference_uses_in_statement_v1(
    statement: &SemanticStatementKindV1,
    site: SemanticTransparentBorrowSiteV1,
    candidate_by_reference: &BTreeMap<u32, usize>,
    candidates: &mut [SemanticBorrowCandidateV1],
) {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            let candidate_definition = candidate_by_reference
                .get(&assignment.destination().local().index())
                .copied()
                .filter(|candidate| {
                    matches!(candidates[*candidate].origin, SemanticBorrowOriginV1::Statement(origin) | SemanticBorrowOriginV1::Copy(origin) if origin == site)
                });
            if let Some(candidate) = candidate_by_reference
                .get(&assignment.destination().local().index())
                .copied()
                && candidate_definition != Some(candidate)
            {
                candidates[candidate].valid = false;
            }
            if let Some(candidate) = candidate_definition {
                if let Some(source_reference) = candidates[candidate].source_reference {
                    match candidate_by_reference.get(&source_reference).copied() {
                        Some(parent) if parent != candidate => {
                            if matches!(
                                candidates[candidate].origin,
                                SemanticBorrowOriginV1::Copy(_)
                            ) {
                                let defined_before = match candidates[parent].origin {
                                    SemanticBorrowOriginV1::Statement(origin)
                                    | SemanticBorrowOriginV1::Copy(origin) => {
                                        origin.block != site.block
                                            || origin.statement < site.statement
                                    }
                                    SemanticBorrowOriginV1::Parameter(_) => true,
                                };
                                candidates[candidate].valid &= candidates[parent].shared
                                    && defined_before
                                    && candidates[candidate].reference_type
                                        == candidates[parent].reference_type;
                                candidates[candidate].source_type = candidates[parent].source_type;
                                candidates[candidate].shared = candidates[parent].shared;
                            }
                            candidates[candidate].parent = Some(parent);
                            candidates[parent].consumers =
                                candidates[parent].consumers.saturating_add(1);
                        }
                        Some(_) | None => candidates[candidate].valid = false,
                    }
                } else if let Some(parent) = candidate_by_reference
                    .get(&candidates[candidate].source_local)
                    .copied()
                {
                    candidates[parent].valid = false;
                    candidates[candidate].valid = false;
                }
            } else {
                invalidate_reference_place_v1(
                    assignment.destination(),
                    candidate_by_reference,
                    candidates,
                );
                invalidate_reference_rvalue_v1(
                    assignment.value().kind(),
                    candidate_by_reference,
                    candidates,
                );
            }
        }
        SemanticStatementKindV1::Store(store) => {
            invalidate_reference_place_v1(store.destination(), candidate_by_reference, candidates);
            invalidate_reference_operand_v1(store.value(), candidate_by_reference, candidates);
        }
        SemanticStatementKindV1::AtomicRmw(operation) => {
            invalidate_reference_place_v1(
                operation.destination(),
                candidate_by_reference,
                candidates,
            );
            invalidate_reference_place_v1(operation.address(), candidate_by_reference, candidates);
            invalidate_reference_operand_v1(operation.value(), candidate_by_reference, candidates);
        }
        SemanticStatementKindV1::AtomicCompareExchange(operation) => {
            invalidate_reference_place_v1(
                operation.destination(),
                candidate_by_reference,
                candidates,
            );
            invalidate_reference_place_v1(operation.address(), candidate_by_reference, candidates);
            invalidate_reference_operand_v1(
                operation.expected(),
                candidate_by_reference,
                candidates,
            );
            invalidate_reference_operand_v1(
                operation.replacement(),
                candidate_by_reference,
                candidates,
            );
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. }
        | SemanticStatementKindV1::Deinitialize(place) => {
            invalidate_reference_place_v1(place, candidate_by_reference, candidates)
        }
        SemanticStatementKindV1::Assume(condition) => {
            invalidate_reference_operand_v1(condition, candidate_by_reference, candidates)
        }
        SemanticStatementKindV1::StorageLive(_)
        | SemanticStatementKindV1::StorageDead(_)
        | SemanticStatementKindV1::Nop => {}
    }
}

fn validate_reference_uses_in_terminator_v1(
    terminator: &SemanticTerminatorKindV1,
    return_local: Option<usize>,
    callables: &[SemanticCallableDeclV1],
    effects: Option<&super::nominal_reference_effects_v29::NominalReferenceEffectsV29>,
    candidate_by_reference: &BTreeMap<u32, usize>,
    candidates: &mut [SemanticBorrowCandidateV1],
) {
    match terminator {
        SemanticTerminatorKindV1::Call(call) => {
            if let Some(destination) = call.destination() {
                invalidate_reference_place_v1(
                    destination.place(),
                    candidate_by_reference,
                    candidates,
                );
            }
            let callable = callables.get(call.callee().index() as usize);
            for (argument_index, argument) in call.arguments().iter().enumerate() {
                let place = match argument {
                    SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)
                        if place.projections().is_empty() =>
                    {
                        place
                    }
                    _ => {
                        invalidate_reference_operand_v1(
                            argument,
                            candidate_by_reference,
                            candidates,
                        );
                        continue;
                    }
                };
                let Some(candidate_index) =
                    candidate_by_reference.get(&place.local().index()).copied()
                else {
                    continue;
                };
                let candidate = candidates[candidate_index];
                let accepted = place.ty() == candidate.reference_type
                    && match callable {
                        Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) => {
                            compiler_intrinsic_accepts_transparent_borrow_v1(
                                operation,
                                argument_index,
                                candidate.source_type,
                            )
                        }
                        Some(SemanticCallableDeclV1::Defined { function }) => {
                            effects.is_some_and(|effects| {
                                place.ty() == candidate.reference_type
                                    && effects.accepts(
                                        *function,
                                        argument_index,
                                        candidate.reference_type,
                                        candidate.source_type,
                                    )
                            })
                        }
                        Some(SemanticCallableDeclV1::DeviceFfiImport { .. }) | None => false,
                    };
                if accepted {
                    candidates[candidate_index].consumers =
                        candidates[candidate_index].consumers.saturating_add(1);
                    candidates[candidate_index].closed_consumers = candidates[candidate_index]
                        .closed_consumers
                        .saturating_add(1);
                } else {
                    candidates[candidate_index].valid = false;
                }
            }
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for argument in call.arguments() {
                invalidate_reference_operand_v1(argument, candidate_by_reference, candidates);
            }
        }
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            invalidate_reference_operand_v1(discriminant, candidate_by_reference, candidates)
        }
        SemanticTerminatorKindV1::Drop { place, .. } => {
            invalidate_reference_place_v1(place, candidate_by_reference, candidates);
        }
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            invalidate_reference_operand_v1(condition, candidate_by_reference, candidates);
            invalidate_reference_assert_message_v1(message, candidate_by_reference, candidates);
        }
        SemanticTerminatorKindV1::Return => {
            // The ABI can make Return read a local without an explicit operand.
            if let Some(local) = return_local
                && let Some(candidate) = candidate_by_reference.get(&(local as u32))
            {
                candidates[*candidate].valid = false;
            }
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => {}
    }
}

fn invalidate_reference_assert_message_v1(
    message: &SemanticAssertMessageV1,
    candidate_by_reference: &BTreeMap<u32, usize>,
    candidates: &mut [SemanticBorrowCandidateV1],
) {
    let mut invalidate =
        |operand| invalidate_reference_operand_v1(operand, candidate_by_reference, candidates);
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => {
            invalidate(length);
            invalidate(index);
        }
        SemanticAssertMessageV1::Overflow { left, right, .. } => {
            invalidate(left);
            invalidate(right);
        }
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => invalidate(operand),
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => {
            invalidate(required_alignment);
            invalidate(found_alignment);
        }
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => {}
    }
}

fn compiler_intrinsic_accepts_transparent_borrow_v1(
    operation: &SemanticCompilerIntrinsicOperationV1,
    argument: usize,
    source_type: SemanticTypeIdV1,
) -> bool {
    match operation {
        SemanticCompilerIntrinsicOperationV1::Execution(
            fe2o3_mir_model::semantic_mir_v1::SemanticExecutionOperationV29::WorkgroupDerive {
                context,
                ..
            },
        ) => argument == 0 && source_type == *context,
        SemanticCompilerIntrinsicOperationV1::Execution(
            fe2o3_mir_model::semantic_mir_v1::SemanticExecutionOperationV29::MaskedTileLoadU32 {
                workgroup,
                ..
            },
        ) => argument == 0 && source_type == *workgroup,
        SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent { scope, .. }
        | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { scope, .. } => {
            argument == 0 && source_type == *scope
        }
        SemanticCompilerIntrinsicOperationV1::DynamicLdsIntoCollectiveRawParts {
            dynamic_lds,
            ..
        } => argument == 0 && source_type == *dynamic_lds,
        SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupReduceSum {
            context,
            dynamic_lds,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::NeutralWorkgroupScanSum {
            context,
            dynamic_lds,
            ..
        } => {
            (argument == 0 && source_type == *context)
                || (argument == 1 && source_type == *dynamic_lds)
        }
        SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent { pipeline, .. }
        | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite { pipeline, .. }
        | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead { pipeline, .. } => {
            argument == 0 && source_type == *pipeline
        }
        SemanticCompilerIntrinsicOperationV1::MathF32 { context, .. }
        | SemanticCompilerIntrinsicOperationV1::SubgroupReduceF32 { context, .. }
        | SemanticCompilerIntrinsicOperationV1::Gfx950SubgroupReduceF32 { context, .. }
        | SemanticCompilerIntrinsicOperationV1::SubgroupBroadcastF32 { context, .. } => {
            argument == 0 && source_type == *context
        }
        SemanticCompilerIntrinsicOperationV1::WorkgroupReduceSum {
            context, scratch, ..
        } => {
            (argument == 0 && source_type == *context) || (argument == 1 && source_type == *scratch)
        }
        SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoad { view, lane, .. }
        | SemanticCompilerIntrinsicOperationV1::Bf16MatrixLoadZeroFilledV2 { view, lane, .. }
        | SemanticCompilerIntrinsicOperationV1::Gfx950Fp4MatrixLoadM16K128 { view, lane, .. }
        | SemanticCompilerIntrinsicOperationV1::Gfx950Fp8MatrixLoadM16K128 { view, lane, .. } => {
            (argument == 0 && source_type == *view) || (argument == 1 && source_type == *lane)
        }
        SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeCurrent { lane, .. } => {
            argument == 0 && source_type == *lane
        }
        SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeStage {
            input_tile, view, ..
        } => {
            (argument == 0 && source_type == *input_tile) || (argument == 1 && source_type == *view)
        }
        SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposePublish { input_tile, .. }
        | SemanticCompilerIntrinsicOperationV1::Gfx950LdsTransposeRead {
            tile: input_tile, ..
        } => argument == 0 && source_type == *input_tile,
        SemanticCompilerIntrinsicOperationV1::StridedReadView2DLoadOr { view, .. } => {
            argument == 0 && source_type == *view
        }
        SemanticCompilerIntrinsicOperationV1::F32MatrixAccumulatorZero { lane, .. } => {
            argument == 0 && source_type == *lane
        }
        SemanticCompilerIntrinsicOperationV1::MatrixMultiplyAccumulate { context, .. } => {
            argument == 0 && source_type == *context
        }
        SemanticCompilerIntrinsicOperationV1::ThreadIndexGet { index_witness, .. }
        | SemanticCompilerIntrinsicOperationV1::DisjointIndexGet { index_witness, .. } => {
            argument == 0 && source_type == *index_witness
        }
        SemanticCompilerIntrinsicOperationV1::DisjointBlockComponentIndex {
            block_witness, ..
        } => argument == 0 && source_type == *block_witness,
        SemanticCompilerIntrinsicOperationV1::DisjointSliceLen { disjoint_slice, .. }
        | SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceLen {
            disjoint_slice, ..
        } => argument == 0 && source_type == *disjoint_slice,
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
            disjoint_slice,
            index_witness,
            ..
        }
        | SemanticCompilerIntrinsicOperationV1::DisjointSliceGetDisjointMut {
            disjoint_slice,
            index_witness,
            ..
        } => {
            (argument == 0 && source_type == *disjoint_slice)
                || (argument == 1 && source_type == *index_witness)
        }
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMutExclusive {
            disjoint_slice,
            grid_leader,
            ..
        } => {
            (argument == 0 && source_type == *disjoint_slice)
                || (argument == 1 && source_type == *grid_leader)
        }
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetBlockMut {
            disjoint_slice,
            block_witness,
            ..
        } => {
            (argument == 0 && source_type == *disjoint_slice)
                || (argument == 1 && source_type == *block_witness)
        }
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetTiled2dMut {
            disjoint_slice,
            tile_witness,
            ..
        } => {
            (argument == 0 && source_type == *disjoint_slice)
                || (argument == 1 && source_type == *tile_witness)
        }
        SemanticCompilerIntrinsicOperationV1::DisjointSliceGetRowStriped2dMut {
            disjoint_slice,
            stripe_witness,
            ..
        } => {
            (argument == 0 && source_type == *disjoint_slice)
                || (argument == 1 && source_type == *stripe_witness)
        }
        SemanticCompilerIntrinsicOperationV1::WriteOnlyDisjointSliceWrite {
            disjoint_slice,
            witness,
            ..
        } => {
            (argument == 0 && source_type == *disjoint_slice)
                || (argument == 1 && source_type == *witness)
        }
        _ => false,
    }
}

pub(super) fn semantic_function_ssa_input_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    callables: &[SemanticCallableDeclV1],
    transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
) -> (SsaConstructionInputV1, Vec<SsaVariableIdV1>, usize) {
    emission_v1::infallible_v1(semantic_function_ssa_input_with_observer_v1(
        function,
        types,
        callables,
        transparent_borrows,
        &mut emission_v1::NoSemanticSsaEmissionObserverV1,
    ))
}

pub(super) fn semantic_function_ssa_input_with_observer_v1<
    O: emission_v1::SemanticSsaEmissionObserverV1,
>(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    callables: &[SemanticCallableDeclV1],
    transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
    observer: &mut O,
) -> Result<(SsaConstructionInputV1, Vec<SsaVariableIdV1>, usize), O::Error> {
    prepared_v1::prepare_semantic_ssa_adapter_with_observer_v1(
        function,
        types,
        callables,
        transparent_borrows,
        observer,
    )?
    .into_entries(observer)?
    .finish(observer)
}

fn authenticated_elided_grid_leader_borrow_sites_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    callables: &[SemanticCallableDeclV1],
    transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
) -> (BTreeSet<SemanticTransparentBorrowSiteV1>, usize) {
    let Some(types) = types else {
        return (BTreeSet::new(), 0);
    };
    let candidates = transparent_borrows
        .iter()
        .filter_map(|site| {
            let SemanticStatementKindV1::Assign(assignment) = function
                .blocks()
                .get(site.block as usize)?
                .statements()
                .get(site.statement as usize)?
                .kind()
            else {
                return None;
            };
            let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind() else {
                return None;
            };
            if !place.projections().is_empty() {
                return None;
            }
            let declaration = function.locals().get(place.local().index() as usize)?;
            let ty = types.get(place.ty().index() as usize)?;
            if declaration.role() != SemanticLocalRoleV1::Temporary
                || declaration.ty() != place.ty()
                || ty.layout().size_bytes() != Some(0)
                || ty.layout().is_uninhabited()
                || local_has_direct_definition_or_lifetime_event_v1(function, place.local())
            {
                return None;
            }
            Some((*site, place.ty()))
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return (BTreeSet::new(), 0);
    }

    let Ok(option_producers) = semantic_option_producers_v1(function, callables) else {
        return (BTreeSet::new(), 0);
    };
    let Ok(option_dominance) = SemanticOptionDominanceV1::analyze(function, &option_producers)
    else {
        return (BTreeSet::new(), 0);
    };
    let grid_leader_producers = function
        .blocks()
        .iter()
        .filter_map(|block| {
            let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
                return None;
            };
            let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::GridLeaderCurrent { grid_leader },
                ..
            }) = callables.get(call.callee().index() as usize)
            else {
                return None;
            };
            let destination = call.destination()?;
            let availability = option_dominance.availability(destination.place().local())?;
            Some((*grid_leader, availability))
        })
        .collect::<Vec<_>>();
    let sites = candidates
        .into_iter()
        .filter_map(|(site, ty)| {
            let block = SemanticBlockIdV1::from_index(site.block);
            let mut matching =
                grid_leader_producers
                    .iter()
                    .filter_map(|(grid_leader, availability)| {
                        (*grid_leader == ty && option_dominance.allows(*availability, block))
                            .then_some(*availability)
                    });
            matching.next()?;
            matching.next().is_none().then_some(site)
        })
        .collect();
    (sites, option_dominance.work_units())
}

fn local_has_direct_definition_or_lifetime_event_v1(
    function: &SemanticFunctionDeclV1,
    local: SemanticLocalIdV1,
) -> bool {
    let is_direct =
        |place: &SemanticPlaceV1| place.local() == local && place.projections().is_empty();
    function.blocks().iter().any(|block| {
        block
            .statements()
            .iter()
            .any(|statement| match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => is_direct(assignment.destination()),
                SemanticStatementKindV1::Store(store) => is_direct(store.destination()),
                SemanticStatementKindV1::AtomicRmw(atomic) => is_direct(atomic.destination()),
                SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                    is_direct(atomic.destination())
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => is_direct(place),
                SemanticStatementKindV1::StorageLive(candidate)
                | SemanticStatementKindV1::StorageDead(candidate) => *candidate == local,
                SemanticStatementKindV1::Assume(_) | SemanticStatementKindV1::Nop => false,
            })
            || matches!(
                block.terminator().kind(),
                SemanticTerminatorKindV1::Call(call)
                    if call.destination().is_some_and(|destination| is_direct(destination.place()))
            )
    })
}

fn authenticated_implicit_entry_variables_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    callables: &[SemanticCallableDeclV1],
    transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
    promotable: &[bool],
    blocks: &[SsaBlockInputV1],
) -> Vec<SsaVariableIdV1> {
    let Some(types) = types else {
        return Vec::new();
    };
    let mut expected_uses = BTreeMap::<u32, usize>::new();
    for site in transparent_borrows {
        let Some(SemanticStatementKindV1::Assign(assignment)) = function
            .blocks()
            .get(site.block as usize)
            .and_then(|block| block.statements().get(site.statement as usize))
            .map(|statement| statement.kind())
        else {
            continue;
        };
        let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind() else {
            continue;
        };
        if !place.projections().is_empty()
            || !authenticated_ambient_workgroup_lds_scope_zst_v1(types, callables, place.ty())
        {
            continue;
        }
        *expected_uses.entry(place.local().index()).or_default() += 1;
    }

    let mut actual_uses = vec![0_usize; function.locals().len()];
    let mut disqualified = vec![false; function.locals().len()];
    for block in blocks {
        for event in block.events() {
            let local = event.variable().get() as usize;
            match event {
                SsaEventV1::Use(_) => {
                    actual_uses[local] = actual_uses[local].saturating_add(1);
                }
                SsaEventV1::Define(_) | SsaEventV1::Kill(_) => disqualified[local] = true,
            }
        }
        for edge in block.edges() {
            for variable in edge.definitions() {
                disqualified[variable.get() as usize] = true;
            }
        }
    }

    expected_uses
        .into_iter()
        .filter_map(|(local, expected)| {
            let declaration = function.locals().get(local as usize)?;
            if declaration.role() != SemanticLocalRoleV1::Temporary
                || !promotable.get(local as usize).copied().unwrap_or(false)
                || disqualified[local as usize]
            {
                return None;
            }
            (actual_uses[local as usize] == expected).then_some(SsaVariableIdV1::new(local))
        })
        .collect()
}

/// Recognizes the exact ambient, idempotent workgroup-LDS scope capability.
///
/// Its safe `current()` acquisition is observationally pure and has no
/// destructor, so optimized MIR may erase the producer while retaining later
/// borrows. No other zero-sized capability is reconstructed by this rule.
pub fn authenticated_ambient_workgroup_lds_scope_zst_v1(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    ty: SemanticTypeIdV1,
) -> bool {
    let issued = callables.iter().any(|callable| {
        matches!(
            callable,
            SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }
                if matches!(
                    operation,
                    SemanticCompilerIntrinsicOperationV1::DynamicLdsExactCurrent { scope, .. }
                        | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { scope, .. }
                            if *scope == ty
                )
        )
    });
    if !issued {
        return false;
    }
    let Some(declaration) = types.get(ty.index() as usize) else {
        return false;
    };
    let SemanticTypeShapeV1::Aggregate(aggregate) = declaration.shape() else {
        return false;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return false;
    };
    declaration.layout().size_bytes() == Some(0)
        && !declaration.layout().is_uninhabited()
        && matches!(
            declaration.layout().backend_repr(),
            SemanticBackendReprV1::Memory { sized: true }
        )
        && aggregate.fields().len() == layout.field_offsets().len()
        && layout.field_offsets().iter().all(|offset| *offset == 0)
        && layout.padding().is_empty()
        && aggregate.fields().iter().all(|field| {
            types.get(field.index() as usize).is_some_and(|field| {
                field.layout().size_bytes() == Some(0) && !field.layout().is_uninhabited()
            })
        })
}

fn classify_storage_observable_locals_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
    promotable: &mut [bool],
) -> (usize, usize) {
    let mut projection_work = 0;
    let mut field_updates = 0_usize;
    for (block_index, block) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in block.statements().iter().enumerate() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    if static_field_assignment_v1(
                        function,
                        types,
                        assignment.destination(),
                        &mut projection_work,
                    ) {
                        field_updates = field_updates.saturating_add(1);
                    } else if !assignment.destination().projections().is_empty() {
                        mark_local_storage_observable_v1(
                            assignment.destination(),
                            promotable,
                            &mut projection_work,
                        );
                    }
                    classify_rvalue_storage_v1(
                        assignment.value().kind(),
                        transparent_borrows.contains(&SemanticTransparentBorrowSiteV1 {
                            block: block_index as u32,
                            statement: statement_index as u32,
                        }),
                        promotable,
                        &mut projection_work,
                    );
                }
                SemanticStatementKindV1::Store(store) => {
                    mark_local_storage_observable_v1(
                        store.destination(),
                        promotable,
                        &mut projection_work,
                    );
                }
                SemanticStatementKindV1::AtomicRmw(operation) => {
                    if !operation.destination().projections().is_empty() {
                        mark_local_storage_observable_v1(
                            operation.destination(),
                            promotable,
                            &mut projection_work,
                        );
                    }
                    mark_local_storage_observable_v1(
                        operation.address(),
                        promotable,
                        &mut projection_work,
                    );
                }
                SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                    if !operation.destination().projections().is_empty() {
                        mark_local_storage_observable_v1(
                            operation.destination(),
                            promotable,
                            &mut projection_work,
                        );
                    }
                    mark_local_storage_observable_v1(
                        operation.address(),
                        promotable,
                        &mut projection_work,
                    );
                }
                SemanticStatementKindV1::Deinitialize(place)
                    if super::partial_moves::is_static_local_deinitialize_v1(place) =>
                {
                    // Removing a logical value does not expose its backing.
                    // The partial-state certificate validates the exact path
                    // and every later use, including joins and reinitialization.
                    projection_work = projection_work.saturating_add(place.projections().len());
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => {
                    mark_local_storage_observable_v1(place, promotable, &mut projection_work);
                }
                SemanticStatementKindV1::Assume(_) => {}
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Call(call) => {
                if let Some(destination) = call.destination()
                    && !destination.place().projections().is_empty()
                {
                    mark_local_storage_observable_v1(
                        destination.place(),
                        promotable,
                        &mut projection_work,
                    );
                }
            }
            SemanticTerminatorKindV1::TailCall(_) | SemanticTerminatorKindV1::SwitchInt { .. } => {}
            SemanticTerminatorKindV1::Drop { place, .. } => {
                mark_local_storage_observable_v1(place, promotable, &mut projection_work);
            }
            SemanticTerminatorKindV1::Assert { .. } => {}
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
    }
    (projection_work, field_updates)
}

fn static_field_assignment_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    destination: &SemanticPlaceV1,
    work: &mut usize,
) -> bool {
    if destination.projections().is_empty() {
        return false;
    }
    let Some(types) = types else { return false };
    *work = work.saturating_add(3);
    let Some(local) = function.locals().get(destination.local().index() as usize) else {
        return false;
    };
    let mut ty = local.ty();
    for projection in destination.projections() {
        *work = work.saturating_add(4);
        let SemanticProjectionKindV1::Field(field) = projection.kind() else {
            return false;
        };
        let Some(SemanticTypeShapeV1::Tuple(fields) | SemanticTypeShapeV1::Aggregate(fields)) =
            types
                .get(ty.index() as usize)
                .map(SemanticTypeDeclV1::shape)
        else {
            return false;
        };
        let Some(next) = fields.fields().get(field as usize).copied() else {
            return false;
        };
        if next != projection.result_type() {
            return false;
        }
        ty = next;
    }
    ty == destination.ty()
}

fn classify_rvalue_storage_v1(
    value: &SemanticRvalueKindV1,
    transparent_borrow: bool,
    promotable: &mut [bool],
    projection_work: &mut usize,
) {
    match value {
        SemanticRvalueKindV1::Borrow { .. } if transparent_borrow => {}
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. } => {
            mark_local_storage_observable_v1(place, promotable, projection_work);
        }
        SemanticRvalueKindV1::Load(load) => {
            mark_local_storage_observable_v1(load.source(), promotable, projection_work);
        }
        SemanticRvalueKindV1::Use(_)
        | SemanticRvalueKindV1::Unary { .. }
        | SemanticRvalueKindV1::Binary { .. }
        | SemanticRvalueKindV1::CheckedBinary(_)
        | SemanticRvalueKindV1::UncheckedBinary(_)
        | SemanticRvalueKindV1::Cast { .. }
        | SemanticRvalueKindV1::Aggregate(_)
        | SemanticRvalueKindV1::Length(_)
        | SemanticRvalueKindV1::Discriminant(_) => {}
    }
}

fn mark_local_storage_observable_v1(
    place: &SemanticPlaceV1,
    promotable: &mut [bool],
    projection_work: &mut usize,
) {
    // A dereference after aggregate projections still crosses the root allocation.
    // Charge the full possible scan; saturation fails the checked auxiliary sum.
    *projection_work = projection_work.saturating_add(place.projections().len());
    let rooted_behind_pointer = place
        .projections()
        .iter()
        .any(|projection| projection.kind() == SemanticProjectionKindV1::Dereference);
    if !rooted_behind_pointer
        && let Some(value) = promotable.get_mut(place.local().index() as usize)
    {
        *value = false;
    }
}

fn append_statement_events_v1(statement: &SemanticStatementKindV1, events: &mut Vec<SsaEventV1>) {
    emission_v1::infallible_v1(emission_v1::emit_statement_events_v1(
        statement,
        false,
        emission_v1::SemanticSsaEmissionSiteV1::Auxiliary,
        events,
        &mut emission_v1::NoSemanticSsaEmissionObserverV1,
    ));
}

fn append_terminator_events_v1(
    terminator: &SemanticTerminatorKindV1,
    return_local: Option<usize>,
    events: &mut Vec<SsaEventV1>,
) {
    emission_v1::infallible_v1(emission_v1::emit_terminator_events_v1(
        terminator,
        return_local,
        emission_v1::SemanticSsaEmissionSiteV1::Auxiliary,
        events,
        &mut emission_v1::NoSemanticSsaEmissionObserverV1,
    ));
}

fn call_edge_definition_v1(
    terminator: &SemanticTerminatorKindV1,
    edge: SemanticControlFlowEdgeV1,
) -> Option<SsaVariableIdV1> {
    let SemanticTerminatorKindV1::Call(call) = terminator else {
        return None;
    };
    let destination = call.destination()?;
    (destination.edge() == edge && destination.place().projections().is_empty())
        .then(|| SsaVariableIdV1::new(destination.place().local().index()))
}

pub(super) fn semantic_edge_role_v1(role: SemanticEdgeRoleV1) -> u16 {
    match role {
        SemanticEdgeRoleV1::Goto => 1,
        SemanticEdgeRoleV1::SwitchValue => 2,
        SemanticEdgeRoleV1::SwitchOtherwise => 3,
        SemanticEdgeRoleV1::CallReturn => 4,
        SemanticEdgeRoleV1::CallUnwind => 5,
        SemanticEdgeRoleV1::TailCallUnwind => 6,
        SemanticEdgeRoleV1::DropReturn => 7,
        SemanticEdgeRoleV1::DropUnwind => 8,
        SemanticEdgeRoleV1::AssertSuccess => 9,
        SemanticEdgeRoleV1::AssertUnwind => 10,
        SemanticEdgeRoleV1::FalseEdgeReal => 11,
        SemanticEdgeRoleV1::FalseEdgeImaginary => 12,
    }
}
