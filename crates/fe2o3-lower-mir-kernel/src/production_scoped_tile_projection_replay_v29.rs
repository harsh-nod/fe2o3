use super::{
    TileAttachmentLocationV29 as Location, TileAttachmentOutputV29 as Target,
    TileScalarPieceV29 as Piece, TileScalarSourceV29 as Source, TileScalarStageV29 as Stage,
};
use fe2o3_kernel_ir::{
    BasicBlock, CanonicalKirBlockCoordinateV1 as BlockCoordinate,
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgumentCoordinate,
    CanonicalKirEdgeCoordinateV1 as EdgeCoordinate,
    CanonicalKirFunctionCoordinateV1 as FunctionCoordinate,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate,
    CanonicalKirUseCoordinateV1 as UseCoordinate, FunctionBody,
};

type R<T> = Result<T, ScopedTileFailureKindV29>;

fn require(valid: bool) -> R<()> {
    if valid {
        Ok(())
    } else {
        Err(ScopedTileFailureKindV29::ReplayMismatch)
    }
}

fn body(graph: &Module, function: usize) -> R<&FunctionBody> {
    graph
        .functions
        .get(function)
        .and_then(|f| f.body.as_ref())
        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
}

fn block(graph: &Module, function: usize, ordinal: usize) -> R<&BasicBlock> {
    body(graph, function)?
        .blocks
        .get(ordinal)
        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
}

fn operation(graph: &Module, point: TileScalarPointV29) -> R<&Operation> {
    block(graph, point.function, point.block)?
        .operations
        .get(point.operation)
        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
}

fn terminator(graph: &Module, function: usize, ordinal: usize) -> R<&Terminator> {
    block(graph, function, ordinal)?
        .terminator
        .as_ref()
        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
}

fn edge_arguments<'a>(
    graph: &'a Module,
    function: usize,
    block: usize,
    selected: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<&'a [ValueId]> {
    let mut ordinal = 0usize;
    let mut found = None;
    terminator(graph, function, block)?.try_visit_edges_v1(|_, arguments| {
        budget.charge_work(1)?;
        if ordinal == selected {
            found = Some(arguments);
        }
        ordinal = ordinal
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok::<_, ScopedTileFailureKindV29>(())
    })?;
    found.ok_or(ScopedTileFailureKindV29::ReplayMismatch)
}

fn validate_source(graph: &Module, source: Source, budget: &mut ArgumentBudgetV1<'_>) -> R<()> {
    budget.charge_work(1)?;
    match source {
        Source::FunctionParameter {
            function,
            parameter,
        } => require(parameter < body(graph, function)?.parameters.len()),
        Source::Block {
            function,
            block: ordinal,
        } => {
            block(graph, function, ordinal)?;
            Ok(())
        }
        Source::BlockParameter {
            function,
            block: ordinal,
            parameter,
        } => require(parameter < block(graph, function, ordinal)?.parameters.len()),
        Source::Operation(point) => {
            operation(graph, point)?;
            Ok(())
        }
        Source::Result {
            operation: point,
            result,
        } => require(result < operation(graph, point)?.results.len()),
        Source::Terminator { function, block } => {
            terminator(graph, function, block)?;
            Ok(())
        }
        Source::Edge {
            function,
            block,
            edge,
        } => {
            edge_arguments(graph, function, block, edge, budget)?;
            Ok(())
        }
    }
}

fn validate_piece(output: &Module, piece: Piece, budget: &mut ArgumentBudgetV1<'_>) -> R<()> {
    let source = match piece {
        Piece::FunctionParameter {
            function,
            parameter,
        } => Source::FunctionParameter {
            function,
            parameter,
        },
        Piece::Block { function, block } => Source::Block { function, block },
        Piece::BlockParameter {
            function,
            block,
            parameter,
        } => Source::BlockParameter {
            function,
            block,
            parameter,
        },
        Piece::Operation(point) => Source::Operation(point),
        Piece::Result { operation, result } => Source::Result { operation, result },
        Piece::Terminator { function, block } => Source::Terminator { function, block },
        Piece::Edge {
            function,
            block,
            edge,
        } => Source::Edge {
            function,
            block,
            edge,
        },
        Piece::Anchor(point) => {
            budget.charge_work(1)?;
            return require(
                point.operation <= block(output, point.function, point.block)?.operations.len(),
            );
        }
        Piece::ErasedValue(_) => {
            // The preceding independent graph replay checked the erased source ID.
            budget.charge_work(1)?;
            return Ok(());
        }
    };
    validate_source(output, source, budget)
}

// Full scans deliberately differ from the producer's structural-key lookup.
fn origin<'a>(
    original: &Module,
    output: &Module,
    relations: &'a TileScalarRelationsV29,
    source: Source,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<&'a TileScalarOriginV29> {
    validate_source(original, source, budget)?;
    let mut found = None;
    for row in &relations.origins {
        budget.charge_work(1)?;
        if row.source == source {
            require(found.replace(row).is_none())?;
        }
    }
    let row = found.ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
    require(row.count != 0)?;
    let end = row
        .first
        .checked_add(row.count)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let pieces = relations
        .pieces
        .get(row.first..end)
        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
    for piece in pieces {
        validate_piece(output, piece.piece, budget)?;
    }
    Ok(row)
}

fn preserved(
    original: &Module,
    output: &Module,
    relations: &TileScalarRelationsV29,
    source: Source,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<Piece> {
    let row = origin(original, output, relations, source, budget)?;
    require(row.count == 1)?;
    let piece = relations
        .pieces
        .get(row.first)
        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
    require(piece.stage == Stage::Preserved && piece.component.is_none())?;
    Ok(piece.piece)
}

fn block_ordinals(coordinate: BlockCoordinate) -> R<(usize, usize)> {
    Ok((
        usize::try_from(coordinate.function.0).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        usize::try_from(coordinate.block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    ))
}

fn operation_point(coordinate: OperationCoordinate) -> R<TileScalarPointV29> {
    let (function, block) = block_ordinals(coordinate.block)?;
    Ok(TileScalarPointV29 {
        function,
        block,
        operation: usize::try_from(coordinate.operation)
            .map_err(|_| ArgumentResourceV1::Arithmetic)?,
    })
}

fn block_coordinate(function: usize, block: usize) -> R<BlockCoordinate> {
    Ok(BlockCoordinate {
        function: FunctionCoordinate(
            u32::try_from(function).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        ),
        block: u32::try_from(block).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    })
}

fn operation_coordinate(point: TileScalarPointV29) -> R<OperationCoordinate> {
    Ok(OperationCoordinate {
        block: block_coordinate(point.function, point.block)?,
        operation: u32::try_from(point.operation).map_err(|_| ArgumentResourceV1::Arithmetic)?,
    })
}

fn operand(
    graph: &Module,
    coordinate: UseCoordinate,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<ValueId> {
    let selected = match coordinate {
        UseCoordinate::OperationOperand { operand, .. }
        | UseCoordinate::TerminatorOperand { operand, .. } => {
            usize::try_from(operand).map_err(|_| ArgumentResourceV1::Arithmetic)?
        }
    };
    let mut ordinal = 0usize;
    let mut found = None;
    let mut visit = |value| {
        budget.charge_work(1)?;
        if ordinal == selected {
            found = Some(value);
        }
        ordinal = ordinal
            .checked_add(1)
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok::<_, ScopedTileFailureKindV29>(())
    };
    match coordinate {
        UseCoordinate::OperationOperand {
            operation: coordinate,
            ..
        } => operation(graph, operation_point(coordinate)?)?
            .kind
            .try_visit_operands(&mut visit)?,
        UseCoordinate::TerminatorOperand {
            block: coordinate, ..
        } => {
            let (function, block) = block_ordinals(coordinate)?;
            terminator(graph, function, block)?.try_visit_operands(&mut visit)?;
        }
    }
    found.ok_or(ScopedTileFailureKindV29::ReplayMismatch)
}

fn edge_argument(
    graph: &Module,
    coordinate: EdgeArgumentCoordinate,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<ValueId> {
    let (function, block) = block_ordinals(coordinate.edge.source)?;
    let edge =
        usize::try_from(coordinate.edge.successor).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    let argument =
        usize::try_from(coordinate.argument).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    budget.charge_work(1)?;
    edge_arguments(graph, function, block, edge, budget)?
        .get(argument)
        .copied()
        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)
}

fn gap(
    original: &Module,
    output: &Module,
    relations: &TileScalarRelationsV29,
    point: TileScalarPointV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<TileScalarPointV29> {
    budget.charge_work(1)?;
    let count = block(original, point.function, point.block)?
        .operations
        .len();
    require(point.operation <= count)?;
    if point.operation == count {
        let piece = preserved(
            original,
            output,
            relations,
            Source::Terminator {
                function: point.function,
                block: point.block,
            },
            budget,
        )?;
        let Piece::Terminator {
            function,
            block: ordinal,
        } = piece
        else {
            return Err(ScopedTileFailureKindV29::ReplayMismatch);
        };
        return Ok(TileScalarPointV29 {
            function,
            block: ordinal,
            operation: block(output, function, ordinal)?.operations.len(),
        });
    }
    let row = origin(
        original,
        output,
        relations,
        Source::Operation(point),
        budget,
    )?;
    let end = row
        .first
        .checked_add(row.count)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    let pieces = relations
        .pieces
        .get(row.first..end)
        .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
    for piece in pieces {
        budget.charge_work(1)?;
        if let Piece::Operation(point) | Piece::Anchor(point) = piece.piece {
            return Ok(point);
        }
    }
    Err(ScopedTileFailureKindV29::ReplayMismatch)
}

fn map_use(
    original: &Module,
    output: &Module,
    relations: &TileScalarRelationsV29,
    coordinate: UseCoordinate,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<UseCoordinate> {
    let value = operand(original, coordinate, budget)?;
    let mapped = match coordinate {
        UseCoordinate::OperationOperand {
            operation: coordinate,
            operand,
        } => {
            let piece = preserved(
                original,
                output,
                relations,
                Source::Operation(operation_point(coordinate)?),
                budget,
            )?;
            let Piece::Operation(point) = piece else {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            };
            UseCoordinate::OperationOperand {
                operation: operation_coordinate(point)?,
                operand,
            }
        }
        UseCoordinate::TerminatorOperand {
            block: coordinate,
            operand,
        } => {
            let (function, block) = block_ordinals(coordinate)?;
            let piece = preserved(
                original,
                output,
                relations,
                Source::Terminator { function, block },
                budget,
            )?;
            let Piece::Terminator { function, block } = piece else {
                return Err(ScopedTileFailureKindV29::ReplayMismatch);
            };
            UseCoordinate::TerminatorOperand {
                block: block_coordinate(function, block)?,
                operand,
            }
        }
    };
    require(value == operand(output, mapped, budget)?)?;
    Ok(mapped)
}

fn map_edge_argument(
    original: &Module,
    output: &Module,
    relations: &TileScalarRelationsV29,
    coordinate: EdgeArgumentCoordinate,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<EdgeArgumentCoordinate> {
    let value = edge_argument(original, coordinate, budget)?;
    let (function, block) = block_ordinals(coordinate.edge.source)?;
    let edge =
        usize::try_from(coordinate.edge.successor).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    let piece = preserved(
        original,
        output,
        relations,
        Source::Edge {
            function,
            block,
            edge,
        },
        budget,
    )?;
    let Piece::Edge {
        function,
        block,
        edge,
    } = piece
    else {
        return Err(ScopedTileFailureKindV29::ReplayMismatch);
    };
    let mapped = EdgeArgumentCoordinate {
        edge: EdgeCoordinate {
            source: block_coordinate(function, block)?,
            successor: u32::try_from(edge).map_err(|_| ArgumentResourceV1::Arithmetic)?,
        },
        argument: coordinate.argument,
    };
    require(value == edge_argument(output, mapped, budget)?)?;
    Ok(mapped)
}

fn map_location(
    original: &Module,
    output: &Module,
    relations: &TileScalarRelationsV29,
    location: &Location,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<Target> {
    budget.charge_work(1)?;
    match location {
        Location::Origin(source) => {
            let row = origin(original, output, relations, *source, budget)?;
            Ok(Target::Origin {
                first: row.first,
                count: row.count,
            })
        }
        Location::Gap(point) => Ok(Target::Gap(gap(
            original, output, relations, *point, budget,
        )?)),
        Location::Use(coordinate) => Ok(Target::Use(map_use(
            original,
            output,
            relations,
            *coordinate,
            budget,
        )?)),
        Location::EdgeArgument(coordinate) => Ok(Target::EdgeArgument(map_edge_argument(
            original,
            output,
            relations,
            *coordinate,
            budget,
        )?)),
        Location::Tombstone => Ok(Target::Tombstone),
        Location::NoOutput => Ok(Target::NoOutput),
    }
}

// Must follow independent graph replay; shared visitation reads only retained
// source metadata. No producer V->O lookup or mapping helper is used here.
pub(super) fn replay_scoped_tile_attachments_v29(
    input: &PreparedScopedTileSourceV29,
    output: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18,
    relations: &TileScalarRelationsV29,
    projections: &TileScalarProjectionsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> R<()> {
    let original = input.pending.inner.pending.graph.module();
    let mut next = 0usize;
    visit_tile_attachment_sources_v29(input, budget, |key, location, budget| {
        budget.charge_work(1)?;
        let row = projections
            .rows
            .get(next)
            .ok_or(ScopedTileFailureKindV29::ReplayMismatch)?;
        require(row.key == key && row.source == location)?;
        let expected = map_location(original, output.module(), relations, &location, budget)?;
        budget.charge_work(1)?;
        require(row.target == expected)?;
        next = next.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
        Ok(())
    })?;
    budget.charge_work(1)?;
    require(next == projections.rows.len())
}

#[cfg(test)]
pub(super) use projection_test_oracles_v29::{
    reject_assertion_coordinate_domain_shifts_v29, reject_kill_after_lifecycle_insertion_v29,
    reject_span_slot_and_tombstone_substitutions_v29,
};

#[cfg(test)]
mod projection_test_oracles_v29 {
    use super::*;

    fn reject_projection_subject_v29(
        candidate: &ScopedTileScalarCandidateV29,
        rows: Vec<TileScalarProjectionV29>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) {
        // Caller-owned adversarial table, never adopted as source or candidate custody.
        let subject = TileScalarProjectionsV29 { rows };
        let floor = budget.storage();
        let result = scoped_tile_attempt_v29(budget, |budget| {
            replay_scoped_tile_attachments_v29(
                &candidate.input,
                &candidate.output,
                &candidate.relations,
                &subject,
                budget,
            )
        });
        drop(subject);
        assert_eq!(budget.storage(), floor);
        assert_eq!(result, Err(ScopedTileFailureKindV29::ReplayMismatch));
    }

    pub(in super::super) fn reject_span_slot_and_tombstone_substitutions_v29(
        candidate: &ScopedTileScalarCandidateV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        candidate.replay_with_budget(budget).unwrap();
        let original = candidate.input.pending.pending_module();
        let rows = &candidate.projections.rows;
        let roots = &candidate.input.pending.inner.pending.roots;

        // Extend one real span at its boundary with another source's actual event.
        let mut widened = None;
        'roots: for (r, root) in roots.iter().enumerate() {
            let body = original.functions[root.function_ordinal]
                .body
                .as_ref()
                .unwrap();
            for (span_row, span) in root.coordinates.spans.rows.iter().enumerate() {
                for insertion in &root.insertions {
                    if insertion.source_span == span_row
                        || !span.segments.iter().flatten().any(|s| {
                            s.block == insertion.before.block
                                && s.first <= insertion.before.first
                                && insertion.before.first <= s.first.checked_add(s.count).unwrap()
                        })
                    {
                        continue;
                    }
                    let block = body
                        .blocks
                        .iter()
                        .position(|b| b.id == insertion.after.block)
                        .unwrap();
                    let point = TileScalarPointV29 {
                        function: root.function_ordinal,
                        block,
                        operation: insertion.after.first as usize,
                    };
                    assert!(matches!(
                        body.blocks[block].operations[point.operation].kind,
                        OperationKind::Execution(_)
                    ));
                    let last = rows
                        .iter()
                        .rposition(|row| {
                            row.key.root == r
                                && row.key.family == Family::InstanceSpans
                                && row.key.instance == span.instance.index()
                                && row.key.row == span_row
                        })
                        .unwrap();
                    let foreign = candidate
                        .relations
                        .origins
                        .iter()
                        .find(|row| row.source == Source::Operation(point))
                        .unwrap();
                    let mut extra = rows[last];
                    extra.key.part = extra.key.part.checked_add(1).unwrap();
                    extra.source = Location::Origin(Source::Operation(point));
                    extra.target = Target::Origin {
                        first: foreign.first,
                        count: foreign.count,
                    };
                    widened = Some((last + 1, extra));
                    break 'roots;
                }
            }
        }
        let (at, extra) =
            widened.expect("a real span boundary contains a foreign lifecycle insertion");
        let mut wrong = rows.clone();
        wrong.insert(at, extra);
        reject_projection_subject_v29(candidate, wrong, budget);

        // A historical removed call is not a genuine empty gap from another span.
        let tombstone = rows
            .iter()
            .position(|row| {
                row.key.family == Family::InstanceSpans && row.source == Location::Tombstone
            })
            .expect("genuine removed-call span");
        let gap = rows
            .iter()
            .find(|row| {
                row.key.family == Family::InstanceSpans
                    && matches!((row.source, row.target), (Location::Gap(_), Target::Gap(_)))
            })
            .expect("genuine empty span");
        let mut wrong = rows.clone();
        wrong[tombstone].source = gap.source;
        wrong[tombstone].target = gap.target;
        reject_projection_subject_v29(candidate, wrong, budget);

        // A moved allocation cannot use a live ordinary body span's coordinates.
        let allocation = rows
            .iter()
            .position(|row| {
                row.key.family == Family::SourceSlot && row.key.field == Field::SlotAllocation
            })
            .expect("shifted source allocation");
        let Location::Origin(Source::Operation(alloca)) = rows[allocation].source else {
            panic!("allocation source operation");
        };
        assert_eq!(alloca.block, 0);
        assert!(matches!(
            operation(original, alloca).unwrap().kind,
            OperationKind::Alloca { .. }
        ));
        assert!(
            roots[rows[allocation].key.root]
                .slot_relocation
                .as_ref()
                .unwrap()
                .storage()
                .allocations
                > 0
        );
        let ordinary = rows
            .iter()
            .find(|row| {
                if row.key.root != rows[allocation].key.root
                    || row.key.family != Family::InstanceSpans
                {
                    return false;
                }
                let Location::Origin(Source::Operation(point)) = row.source else {
                    return false;
                };
                point != alloca
                    && matches!(row.target, Target::Origin { .. })
                    && !matches!(
                        operation(original, point).unwrap().kind,
                        OperationKind::Execution(_) | OperationKind::Alloca { .. }
                    )
            })
            .expect("retained ordinary body span");
        let mut wrong = rows.clone();
        wrong[allocation].source = ordinary.source;
        wrong[allocation].target = ordinary.target;
        reject_projection_subject_v29(candidate, wrong, budget);
        candidate.replay_with_budget(budget).unwrap();
    }

    pub(in super::super) fn reject_kill_after_lifecycle_insertion_v29(
        candidate: &ScopedTileScalarCandidateV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) {
        use TileAttachmentFamilyV29 as Family;
        use TileAttachmentFieldV29 as Field;
        candidate.replay_with_budget(budget).unwrap();
        let original = candidate.input.pending.pending_module();
        let mut witness = None;
        for (ordinal, row) in candidate.projections.rows.iter().enumerate() {
            if row.key.family != Family::MemoryAnchor || row.key.field != Field::MemoryPosition {
                continue;
            }
            let (Location::Gap(before), Target::Gap(_)) = (row.source, row.target) else {
                continue;
            };
            let root = &candidate.input.pending.inner.pending.roots[row.key.root];
            let anchor = &root.sidecars.rows.iter()
                .find(|sidecar| sidecar.source_call_instance.unwrap().index() == row.key.instance)
                .unwrap()
                .scoped_memory_anchors
                .as_ref()
                .unwrap()
                .rows[row.key.row];
            if !matches!(anchor.kind, ScopedMemoryAnchorKindV29::Kill { .. }) {
                continue;
            }
            let body = original.functions[before.function].body.as_ref().unwrap();
            if !root.insertions.iter().any(|insertion| {
                insertion.after.block == body.blocks[before.block].id
                    && insertion.after.first as usize == before.operation
            }) {
                continue;
            }
            if !matches!(
                operation(original, before).unwrap().kind,
                OperationKind::Execution(
                    fe2o3_kernel_ir::ExecutionOperationV15::TileIntoFragmentU32 { .. }
                )
            ) {
                continue;
            }
            let erased = candidate
                .relations
                .origins
                .iter()
                .find(|origin| origin.source == Source::Operation(before))
                .unwrap();
            assert_eq!(erased.count, 1);
            let Piece::Anchor(output) = candidate.relations.pieces[erased.first].piece else {
                panic!("Fragment erasure has an actual zero-extent anchor");
            };
            let after = TileScalarPointV29 {
                operation: before.operation.checked_add(1).unwrap(),
                ..before
            };
            assert_ne!(before, after, "B and A remain distinct original V gaps");
            assert!(after.operation <= body.blocks[after.block].operations.len());
            assert!(
                output.operation
                    <= candidate.output.module().functions[output.function]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[output.block]
                        .operations
                        .len()
            );
            assert_eq!(
                row.target,
                Target::Gap(output),
                "erasure may coalesce output gaps, not original source identities"
            );
            witness = Some((ordinal, Location::Gap(after), Target::Gap(output)));
            break;
        }
        let (ordinal, source, target) =
            witness.expect("genuine Kill immediately before a Fragment insertion");
        let mut wrong = candidate.projections.rows.clone();
        wrong[ordinal].source = source;
        wrong[ordinal].target = target;
        reject_projection_subject_v29(candidate, wrong, budget);
        candidate.replay_with_budget(budget).unwrap();
    }

    pub(in super::super) fn reject_assertion_coordinate_domain_shifts_v29(
        candidate: &mut ScopedTileScalarCandidateV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) {
        reject_failure_read_pointer_claims_v29(candidate, budget);
        let root = &candidate.input.pending.inner.pending.roots[0];
        assert!(root.slot_relocation.is_some());
        let root_instance = root.source_slots.instances[0].instance;
        let moved: usize = root
            .source_slots
            .slots
            .iter()
            .filter(|slot| slot.instance != root_instance)
            .map(|slot| 1 + usize::from(slot.representation.count().is_some()))
            .sum();
        assert!(moved > 0, "a genuine relocated helper allocation prefix");
        let function = root.function_ordinal;
        let graph = candidate.input.pending.pending_module();
        let body = graph.functions[function].body.as_ref().unwrap();
        let mut pre_relocation_witnesses = 0;
        for index in 0..candidate.projections.rows.len() {
            let original = candidate.projections.rows[index];
            if original.key.family != TileAttachmentFamilyV29::Assertion
                || original.key.field != TileAttachmentFieldV29::AssertConditionDefinition
            {
                continue;
            }
            let Location::Origin(Source::Result {
                operation: point,
                result,
            }) = original.source
            else {
                panic!("dynamic assertion result");
            };
            if point.function != function || point.block != 0 {
                continue;
            }
            let operations = &body.blocks[point.block].operations;
            for (pre_relocation, shifted) in [
                (true, point.operation.checked_sub(moved)),
                (false, point.operation.checked_add(moved)),
            ] {
                let Some(shifted) = shifted else { continue };
                let Some(value) = operations
                    .get(shifted)
                    .and_then(|op| op.results.get(result))
                else {
                    continue;
                };
                assert_ne!(value.id, operations[point.operation].results[result].id);
                let source = Source::Result {
                    operation: TileScalarPointV29 {
                        operation: shifted,
                        ..point
                    },
                    result,
                };
                let origin = candidate
                    .relations
                    .origins
                    .iter()
                    .find(|origin| origin.source == source)
                    .unwrap();
                assert!(origin.count > 0);
                // Both coordinates are real: this substitutes E for V, or reapplies
                // relocation only when that second shift still names a valid result.
                candidate.projections.rows[index] = TileScalarProjectionV29 {
                    source: Location::Origin(source),
                    target: Target::Origin {
                        first: origin.first,
                        count: origin.count,
                    },
                    ..original
                };
                let floor = budget.storage();
                let refusal = scoped_tile_attempt_v29(budget, |budget| {
                    replay_scoped_tile_attachments_v29(
                        &candidate.input,
                        &candidate.output,
                        &candidate.relations,
                        &candidate.projections,
                        budget,
                    )
                });
                candidate.projections.rows[index] = original;
                assert_eq!(budget.storage(), floor);
                assert_eq!(refusal, Err(ScopedTileFailureKindV29::ReplayMismatch));
                pre_relocation_witnesses += usize::from(pre_relocation);
            }
        }
        assert!(
            pre_relocation_witnesses > 0,
            "the pre-relocation coordinate must be valid but wrongly labeled V"
        );
    }

    fn reject_failure_read_pointer_claims_v29(
        candidate: &ScopedTileScalarCandidateV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) {
        let rows = &candidate.projections.rows;
        let actual_pointer = rows
            .iter()
            .find(|row| {
                row.key.family == TileAttachmentFamilyV29::MemoryAnchor
                    && row.key.field == TileAttachmentFieldV29::MemoryPointer
                    && matches!(row.source, Location::Origin(_))
            })
            .expect("genuine physical memory pointer");
        let mut witnessed = 0;
        for (index, row) in rows.iter().enumerate() {
            if row.key.family != TileAttachmentFamilyV29::MemoryAnchor
                || row.key.field != TileAttachmentFieldV29::MemoryPointer
            {
                continue;
            }
            let root = &candidate.input.pending.inner.pending.roots[row.key.root];
            let anchor = &root.sidecars.rows.iter()
                .find(|sidecar| sidecar.source_call_instance.unwrap().index() == row.key.instance)
                .unwrap()
                .scoped_memory_anchors
                .as_ref()
                .unwrap()
                .rows[row.key.row];
            if !matches!(anchor.kind, ScopedMemoryAnchorKindV29::FailureRead { .. }) {
                continue;
            }
            witnessed += 1;
            let mut wrong = rows.clone();
            wrong[index].source = actual_pointer.source;
            wrong[index].target = actual_pointer.target;
            reject_projection_subject_v29(candidate, wrong, budget);
            let mut wrong = rows.clone();
            wrong.remove(index);
            reject_projection_subject_v29(candidate, wrong, budget);
            let mut wrong = rows.clone();
            wrong.insert(index, *row);
            reject_projection_subject_v29(candidate, wrong, budget);
        }
        assert!(witnessed > 0, "source-derived failure-only receipt");
        candidate.replay_with_budget(budget).unwrap();
    }
}
