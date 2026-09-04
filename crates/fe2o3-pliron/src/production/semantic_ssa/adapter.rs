use super::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct SemanticTransparentBorrowSiteV1 {
    block: u32,
    statement: u32,
}

#[derive(Clone, Copy)]
struct SemanticBorrowCandidateV1 {
    site: SemanticTransparentBorrowSiteV1,
    source_type: SemanticTypeIdV1,
    valid: bool,
    consumers: u32,
}

pub(super) fn transparent_borrow_sites_v1(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
) -> BTreeSet<SemanticTransparentBorrowSiteV1> {
    let mut candidates = Vec::new();
    let mut candidate_by_reference = BTreeMap::<u32, usize>::new();
    let mut duplicate_references = BTreeSet::new();
    for (block_index, block) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::Borrow { place, .. } = assignment.value().kind() else {
                continue;
            };
            if !assignment.destination().projections().is_empty() || !place.projections().is_empty()
            {
                continue;
            }
            let reference_local = assignment.destination().local().index();
            let candidate = SemanticBorrowCandidateV1 {
                site: SemanticTransparentBorrowSiteV1 {
                    block: block_index as u32,
                    statement: statement_index as u32,
                },
                source_type: place.ty(),
                valid: true,
                consumers: 0,
            };
            let index = candidates.len();
            candidates.push(candidate);
            if duplicate_references.contains(&reference_local) {
                candidates[index].valid = false;
            } else if let Some(previous) = candidate_by_reference.insert(reference_local, index) {
                candidates[previous].valid = false;
                candidates[index].valid = false;
                candidate_by_reference.remove(&reference_local);
                duplicate_references.insert(reference_local);
            }
        }
    }
    if candidates.is_empty() {
        return BTreeSet::new();
    }
    for (block_index, block) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let site = SemanticTransparentBorrowSiteV1 {
                block: block_index as u32,
                statement: statement_index as u32,
            };
            invalidate_reference_uses_in_statement_v1(
                statement.kind(),
                site,
                &candidate_by_reference,
                &mut candidates,
            );
        }
        validate_reference_uses_in_terminator_v1(
            block.terminator().kind(),
            callables,
            &candidate_by_reference,
            &mut candidates,
        );
    }

    candidates
        .into_iter()
        .filter_map(|candidate| {
            (candidate.valid && candidate.consumers == 1).then_some(candidate.site)
        })
        .collect()
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
            if let Some(candidate) = candidate_by_reference
                .get(&assignment.destination().local().index())
                .copied()
                && candidates[candidate].site != site
            {
                candidates[candidate].valid = false;
            }
            let is_candidate_definition = candidate_by_reference
                .get(&assignment.destination().local().index())
                .is_some_and(|candidate| candidates[*candidate].site == site);
            if !is_candidate_definition {
                invalidate_reference_place_v1(
                    assignment.destination(),
                    candidate_by_reference,
                    candidates,
                );
            }
            invalidate_reference_rvalue_v1(
                assignment.value().kind(),
                candidate_by_reference,
                candidates,
            );
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
    callables: &[SemanticCallableDeclV1],
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
            let operation = callables
                .get(call.callee().index() as usize)
                .and_then(|callable| match callable {
                    SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } => Some(operation),
                    SemanticCallableDeclV1::Defined { .. }
                    | SemanticCallableDeclV1::DeviceFfiImport { .. } => None,
                });
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
                let accepted = operation.is_some_and(|operation| {
                    compiler_intrinsic_accepts_transparent_borrow_v1(
                        operation,
                        argument_index,
                        candidates[candidate_index].source_type,
                    )
                });
                if accepted {
                    candidates[candidate_index].consumers =
                        candidates[candidate_index].consumers.saturating_add(1);
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
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
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
) -> (SsaConstructionInputV1, Vec<SsaVariableIdV1>) {
    let mut promotable = vec![true; function.locals().len()];
    classify_storage_observable_locals_v1(function, transparent_borrows, &mut promotable);
    let return_local = (!matches!(
        function.abi().return_value().mode(),
        SemanticAbiPassModeV1::Ignore,
    ))
    .then(|| {
        function
            .locals()
            .iter()
            .position(|declaration| matches!(declaration.role(), SemanticLocalRoleV1::Return))
    })
    .flatten();
    let blocks = function
        .blocks()
        .iter()
        .map(|block| {
            let mut events = Vec::new();
            for statement in block.statements() {
                append_statement_events_v1(statement.kind(), &mut events);
            }
            append_terminator_events_v1(block.terminator().kind(), return_local, &mut events);
            let mut edges = Vec::with_capacity(block.terminator().kind().edge_count());
            block
                .terminator()
                .kind()
                .try_for_each_edge::<std::convert::Infallible>(|edge| {
                    let definitions = call_edge_definitions_v1(block.terminator().kind(), edge);
                    edges.push(SsaEdgeInputV1::new(
                        SsaEdgeRoleV1::new(semantic_edge_role_v1(edge.role())),
                        SsaBlockIdV1::new(edge.target().index()),
                        definitions,
                    ));
                    Ok(())
                })
                .expect("infallible semantic edge collection");
            SsaBlockInputV1::new(events, edges)
        })
        .collect::<Vec<_>>();
    let implicit_entry_variables = authenticated_implicit_entry_variables_v1(
        function,
        types,
        callables,
        transparent_borrows,
        &promotable,
        &blocks,
    );
    let implicit = implicit_entry_variables
        .iter()
        .map(|variable| variable.get())
        .collect::<BTreeSet<_>>();
    let entry_definitions = function
        .locals()
        .iter()
        .enumerate()
        .filter_map(|(local, declaration)| {
            (matches!(declaration.role(), SemanticLocalRoleV1::Argument(_))
                || implicit.contains(&(local as u32)))
            .then_some(SsaVariableIdV1::new(local as u32))
        })
        .collect();
    (
        SsaConstructionInputV1::new(
            SsaBlockIdV1::new(function.entry().index()),
            function.locals().len() as u32,
            promotable,
            entry_definitions,
            blocks,
        ),
        implicit_entry_variables,
    )
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
    transparent_borrows: &BTreeSet<SemanticTransparentBorrowSiteV1>,
    promotable: &mut [bool],
) {
    for (block_index, block) in function.blocks().iter().enumerate() {
        for (statement_index, statement) in block.statements().iter().enumerate() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    if !assignment.destination().projections().is_empty() {
                        mark_local_storage_observable_v1(assignment.destination(), promotable);
                    }
                    classify_rvalue_storage_v1(
                        assignment.value().kind(),
                        transparent_borrows.contains(&SemanticTransparentBorrowSiteV1 {
                            block: block_index as u32,
                            statement: statement_index as u32,
                        }),
                        promotable,
                    );
                }
                SemanticStatementKindV1::Store(store) => {
                    mark_local_storage_observable_v1(store.destination(), promotable);
                }
                SemanticStatementKindV1::AtomicRmw(operation) => {
                    if !operation.destination().projections().is_empty() {
                        mark_local_storage_observable_v1(operation.destination(), promotable);
                    }
                    mark_local_storage_observable_v1(operation.address(), promotable);
                }
                SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                    if !operation.destination().projections().is_empty() {
                        mark_local_storage_observable_v1(operation.destination(), promotable);
                    }
                    mark_local_storage_observable_v1(operation.address(), promotable);
                }
                SemanticStatementKindV1::SetDiscriminant { place, .. }
                | SemanticStatementKindV1::Deinitialize(place) => {
                    mark_local_storage_observable_v1(place, promotable);
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
                    mark_local_storage_observable_v1(destination.place(), promotable);
                }
            }
            SemanticTerminatorKindV1::TailCall(_) | SemanticTerminatorKindV1::SwitchInt { .. } => {}
            SemanticTerminatorKindV1::Drop { place, .. } => {
                mark_local_storage_observable_v1(place, promotable);
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
}

fn classify_rvalue_storage_v1(
    value: &SemanticRvalueKindV1,
    transparent_borrow: bool,
    promotable: &mut [bool],
) {
    match value {
        SemanticRvalueKindV1::Borrow { .. } if transparent_borrow => {}
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. } => {
            mark_local_storage_observable_v1(place, promotable);
        }
        SemanticRvalueKindV1::Load(load) => {
            mark_local_storage_observable_v1(load.source(), promotable);
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

fn mark_local_storage_observable_v1(place: &SemanticPlaceV1, promotable: &mut [bool]) {
    let rooted_behind_pointer = matches!(
        place
            .projections()
            .first()
            .map(|projection| projection.kind()),
        Some(SemanticProjectionKindV1::Dereference),
    );
    if !rooted_behind_pointer
        && let Some(value) = promotable.get_mut(place.local().index() as usize)
    {
        *value = false;
    }
}

fn append_statement_events_v1(statement: &SemanticStatementKindV1, events: &mut Vec<SsaEventV1>) {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            append_rvalue_events_v1(assignment.value().kind(), events);
            append_place_definition_v1(assignment.destination(), events);
        }
        SemanticStatementKindV1::Store(store) => {
            append_operand_events_v1(store.value(), events);
            append_place_use_v1(store.destination(), events);
        }
        SemanticStatementKindV1::AtomicRmw(operation) => {
            append_place_use_v1(operation.address(), events);
            append_operand_events_v1(operation.value(), events);
            append_place_definition_v1(operation.destination(), events);
        }
        SemanticStatementKindV1::AtomicCompareExchange(operation) => {
            append_place_use_v1(operation.address(), events);
            append_operand_events_v1(operation.expected(), events);
            append_operand_events_v1(operation.replacement(), events);
            append_place_definition_v1(operation.destination(), events);
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. }
        | SemanticStatementKindV1::Deinitialize(place) => append_place_use_v1(place, events),
        SemanticStatementKindV1::Assume(condition) => append_operand_events_v1(condition, events),
        SemanticStatementKindV1::StorageLive(local)
        | SemanticStatementKindV1::StorageDead(local) => {
            events.push(SsaEventV1::Kill(SsaVariableIdV1::new(local.index())))
        }
        SemanticStatementKindV1::Nop => {}
    }
}

fn append_rvalue_events_v1(value: &SemanticRvalueKindV1, events: &mut Vec<SsaEventV1>) {
    match value {
        SemanticRvalueKindV1::Use(operand)
        | SemanticRvalueKindV1::Unary { operand, .. }
        | SemanticRvalueKindV1::Cast { operand, .. } => append_operand_events_v1(operand, events),
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            append_operand_events_v1(left, events);
            append_operand_events_v1(right, events);
        }
        SemanticRvalueKindV1::CheckedBinary(operation) => {
            append_operand_events_v1(operation.left(), events);
            append_operand_events_v1(operation.right(), events);
        }
        SemanticRvalueKindV1::UncheckedBinary(operation) => {
            append_operand_events_v1(operation.left(), events);
            append_operand_events_v1(operation.right(), events);
        }
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. }
        | SemanticRvalueKindV1::Length(place)
        | SemanticRvalueKindV1::Discriminant(place) => append_place_use_v1(place, events),
        SemanticRvalueKindV1::Aggregate(aggregate) => {
            for operand in aggregate.operands() {
                append_operand_events_v1(operand, events);
            }
        }
        SemanticRvalueKindV1::Load(load) => append_place_use_v1(load.source(), events),
    }
}

fn append_operand_events_v1(operand: &SemanticOperandV1, events: &mut Vec<SsaEventV1>) {
    match operand {
        SemanticOperandV1::Copy(place) => append_place_use_v1(place, events),
        SemanticOperandV1::Move(place) => {
            append_place_use_v1(place, events);
            if place.projections().is_empty() {
                events.push(SsaEventV1::Kill(SsaVariableIdV1::new(
                    place.local().index(),
                )));
            }
        }
        SemanticOperandV1::Constant(_) => {}
    }
}

fn append_place_use_v1(place: &SemanticPlaceV1, events: &mut Vec<SsaEventV1>) {
    events.push(SsaEventV1::Use(SsaVariableIdV1::new(place.local().index())));
    for projection in place.projections() {
        if let SemanticProjectionKindV1::Index(local) = projection.kind() {
            events.push(SsaEventV1::Use(SsaVariableIdV1::new(local.index())));
        }
    }
}

fn append_place_definition_v1(place: &SemanticPlaceV1, events: &mut Vec<SsaEventV1>) {
    if place.projections().is_empty() {
        events.push(SsaEventV1::Define(SsaVariableIdV1::new(
            place.local().index(),
        )));
    } else {
        append_place_use_v1(place, events);
    }
}

fn append_terminator_events_v1(
    terminator: &SemanticTerminatorKindV1,
    return_local: Option<usize>,
    events: &mut Vec<SsaEventV1>,
) {
    match terminator {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            append_operand_events_v1(discriminant, events);
        }
        SemanticTerminatorKindV1::Call(call) => {
            for argument in call.arguments() {
                append_operand_events_v1(argument, events);
            }
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for argument in call.arguments() {
                append_operand_events_v1(argument, events);
            }
        }
        SemanticTerminatorKindV1::Drop { place, .. } => append_place_use_v1(place, events),
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            append_operand_events_v1(condition, events);
            append_assert_message_events_v1(message, events);
        }
        SemanticTerminatorKindV1::Return => {
            if let Some(local) = return_local {
                events.push(SsaEventV1::Use(SsaVariableIdV1::new(local as u32)));
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

fn append_assert_message_events_v1(
    message: &SemanticAssertMessageV1,
    events: &mut Vec<SsaEventV1>,
) {
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => {
            append_operand_events_v1(length, events);
            append_operand_events_v1(index, events);
        }
        SemanticAssertMessageV1::Overflow { left, right, .. } => {
            append_operand_events_v1(left, events);
            append_operand_events_v1(right, events);
        }
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => {
            append_operand_events_v1(operand, events);
        }
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => {
            append_operand_events_v1(required_alignment, events);
            append_operand_events_v1(found_alignment, events);
        }
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => {}
    }
}

fn call_edge_definitions_v1(
    terminator: &SemanticTerminatorKindV1,
    edge: SemanticControlFlowEdgeV1,
) -> Vec<SsaVariableIdV1> {
    let SemanticTerminatorKindV1::Call(call) = terminator else {
        return vec![];
    };
    let Some(destination) = call.destination() else {
        return vec![];
    };
    if destination.edge() == edge && destination.place().projections().is_empty() {
        vec![SsaVariableIdV1::new(destination.place().local().index())]
    } else {
        vec![]
    }
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
