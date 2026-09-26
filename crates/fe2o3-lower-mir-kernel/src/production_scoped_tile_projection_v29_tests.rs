// Source-derived coordinate oracles are independent of projection construction.
use super::*;

#[path = "production_scoped_tile_attachment_census_v29_tests.rs"]
mod sidecar_census_tests;

use TileAttachmentFamilyV29 as Family;
use TileAttachmentFieldV29 as Field;
use TileAttachmentLocationV29 as Location;
use TileAttachmentOutputV29 as Target;
use TileScalarPieceV29 as Piece;
use TileScalarSourceV29 as Source;
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition, CanonicalKirUseCoordinateV1 as UseCoordinate,
};

fn with_projection_candidate(
    case: SourceCase,
    check: impl FnOnce(&mut ScopedTileScalarCandidateV29, &mut ArgumentBudgetV1<'_>),
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let mut candidate = scalar_candidate(case, ScopedTileOrderV29::Blocked, &mut budget);
    check(&mut candidate, &mut budget);
    candidate.replay_with_budget(&mut budget).unwrap();
    drop_scalar_candidate(candidate, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

fn projection_key(
    root: usize,
    family: Family,
    instance: usize,
    row: usize,
    field: Field,
) -> TileAttachmentKeyV29 {
    TileAttachmentKeyV29 {
        root,
        family,
        instance,
        row,
        field,
        component: 0,
        part: 0,
    }
}

fn push_keys(
    keys: &mut Vec<TileAttachmentKeyV29>,
    key: TileAttachmentKeyV29,
    components: usize,
    parts: usize,
) {
    for component in 0..components.max(1) {
        for part in 0..parts.max(1) {
            keys.push(TileAttachmentKeyV29 {
                component,
                part,
                ..key
            });
        }
    }
}

fn projected_row(
    candidate: &ScopedTileScalarCandidateV29,
    key: TileAttachmentKeyV29,
) -> &TileScalarProjectionV29 {
    let mut rows = candidate
        .projections
        .rows
        .iter()
        .filter(|row| row.key == key);
    let row = rows
        .next()
        .expect("required independently enumerated source key");
    assert!(rows.next().is_none(), "duplicate source key {key:?}");
    row
}

fn original_block(root: &ScopedModuleRootV29, graph: &Module, id: BlockId) -> usize {
    graph.functions[root.function_ordinal]
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .position(|block| block.id == id)
        .unwrap()
}

fn canonical_point(
    root: &ScopedModuleRootV29,
    graph: &Module,
    id: BlockId,
    position: u32,
    gap: bool,
) -> TileScalarPointV29 {
    // B(p) is left-sticky; A(p) includes all insertions at p.
    let inserted = root
        .insertions
        .iter()
        .filter(|row| {
            row.before.block == id
                && if gap {
                    row.before.first < position
                } else {
                    row.before.first <= position
                }
        })
        .count();
    TileScalarPointV29 {
        function: root.function_ordinal,
        block: original_block(root, graph, id),
        operation: usize::try_from(position).unwrap() + inserted,
    }
}

fn expected_span_sources(root: &ScopedModuleRootV29, graph: &Module, row: usize) -> Vec<Location> {
    let span = &root.coordinates.spans.rows[row];
    if span.segments == [None, None] {
        return vec![if span.removed_call.is_some() {
            Location::Tombstone
        } else {
            Location::NoOutput
        }];
    }
    let mut expected = Vec::new();
    for segment in span.segments.into_iter().flatten() {
        let end = segment.first.checked_add(segment.count).unwrap();
        if segment.count == 0 {
            expected.push(Location::Gap(canonical_point(
                root,
                graph,
                segment.block,
                segment.first,
                true,
            )));
        }
        // Independent set/order oracle: ordinary positions and only source-owned
        // lifecycle positions, ordered in final canonical coordinates.
        let mut positions: Vec<_> = (segment.first..end)
            .map(|p| canonical_point(root, graph, segment.block, p, false))
            .collect();
        positions.extend(
            root.insertions
                .iter()
                .filter(|i| {
                    i.source_span == row
                        && i.before.block == segment.block
                        && i.before.first >= segment.first
                        && i.before.first <= end
                })
                .map(|i| TileScalarPointV29 {
                    function: root.function_ordinal,
                    block: original_block(root, graph, i.after.block),
                    operation: i.after.first as usize,
                }),
        );
        positions.sort_by_key(|p| (p.block, p.operation));
        expected.extend(
            positions
                .into_iter()
                .map(|p| Location::Origin(Source::Operation(p))),
        );
    }
    expected
}

fn core_key_census(candidate: &ScopedTileScalarCandidateV29) -> Vec<TileAttachmentKeyV29> {
    let graph = candidate.input.pending.pending_module();
    let mut expected = Vec::new();
    for (r, root) in candidate
        .input
        .pending
        .inner
        .pending
        .roots
        .iter()
        .enumerate()
    {
        let coordinates = &root.coordinates;
        for (row, span) in coordinates.spans.rows.iter().enumerate() {
            let key = projection_key(
                r,
                Family::InstanceSpans,
                span.instance.index(),
                row,
                Field::Span,
            );
            push_keys(
                &mut expected,
                key,
                1,
                expected_span_sources(root, graph, row).len(),
            );
        }
        for (row, seed) in coordinates.seeds.rows.iter().enumerate() {
            push_keys(
                &mut expected,
                projection_key(
                    r,
                    Family::InstanceSeeds,
                    seed.instance.index(),
                    row,
                    Field::Parameters,
                ),
                seed.parameters.len(),
                1,
            );
        }
        for (row, control) in coordinates.controls.rows.iter().enumerate() {
            let key = |field| {
                projection_key(
                    r,
                    Family::InstanceControls,
                    control.instance.index(),
                    row,
                    field,
                )
            };
            for field in [
                Field::PhysicalBlock,
                Field::Terminator,
                Field::ExpectedTarget,
            ] {
                push_keys(&mut expected, key(field), 1, 1);
            }
            let body = graph.functions[root.function_ordinal]
                .body
                .as_ref()
                .unwrap();
            let block = body
                .blocks
                .iter()
                .find(|block| block.id == control.physical_block)
                .unwrap();
            let mut edge_count = 0;
            let mut argument_count = 0;
            block
                .terminator
                .as_ref()
                .unwrap()
                .try_visit_edges_v1(|_, args| {
                    edge_count += 1;
                    argument_count += args.len();
                    Ok::<_, std::convert::Infallible>(())
                })
                .unwrap();
            push_keys(&mut expected, key(Field::Edge), edge_count, 1);
            push_keys(&mut expected, key(Field::EdgeArgument), argument_count, 1);
            for field in [Field::ReturnDefinition, Field::ReturnUse] {
                push_keys(
                    &mut expected,
                    key(field),
                    control
                        .return_values
                        .as_ref()
                        .map_or(0, |range| range.len()),
                    1,
                );
            }
            push_keys(
                &mut expected,
                key(Field::ExpectedArgument),
                control
                    .expected_branch
                    .as_ref()
                    .map_or(0, |(_, range)| range.len()),
                1,
            );
        }
        for (row, call) in coordinates.anchors.rows.iter().enumerate() {
            let key =
                |field| projection_key(r, Family::InstanceCalls, call.instance.index(), row, field);
            let SemanticKirCallReturnKindV1::Call {
                arguments_first,
                call_operation,
                destination_end,
                transport,
                ..
            } = call.source.kind
            else {
                panic!("call source");
            };
            let source_span = coordinates
                .spans
                .rows
                .iter()
                .find_map(|span| match span.source {
                    InstanceSpanSourceV1::Terminator(source)
                        if span.instance == call.instance
                            && source.semantic_function == call.source.semantic_function
                            && source.semantic_block == call.source.semantic_block =>
                    {
                        Some(source)
                    }
                    _ => None,
                })
                .unwrap();
            push_keys(
                &mut expected,
                key(Field::ArgumentPreparation),
                1,
                (call_operation - arguments_first) as usize,
            );
            push_keys(
                &mut expected,
                key(Field::DestinationPreparation),
                1,
                (arguments_first - source_span.first_operation_ordinal) as usize,
            );
            push_keys(
                &mut expected,
                key(Field::DestinationRange),
                1,
                (destination_end - call_operation - 1) as usize,
            );
            for field in [Field::CallSite, Field::DestinationPointer] {
                push_keys(&mut expected, key(field), 1, 1);
            }
            for field in [Field::ArgumentDefinition, Field::ArgumentUse] {
                push_keys(&mut expected, key(field), call.arguments.len(), 1);
            }
            push_keys(
                &mut expected,
                key(Field::ResultDefinition),
                call.results.len(),
                1,
            );
            for field in [
                Field::TransportComponentConversion,
                Field::TransportComponentOutput,
                Field::TransportComponentUse,
            ] {
                push_keys(
                    &mut expected,
                    projection_key(
                        r,
                        Family::InstanceReturns,
                        call.instance.index(),
                        row,
                        field,
                    ),
                    transport.range().unwrap().len(),
                    1,
                );
            }
        }
        for (row, ret) in coordinates.returns.rows.iter().enumerate() {
            let SemanticKirCallReturnKindV1::Return { components } = ret.source.kind else {
                panic!("return source");
            };
            let key = |field| {
                projection_key(r, Family::InstanceReturns, ret.instance.index(), row, field)
            };
            push_keys(&mut expected, key(Field::ReturnSite), 1, 1);
            for field in [
                Field::ReturnComponentInput,
                Field::ReturnComponentConversion,
                Field::ReturnComponentOutput,
                Field::ReturnComponentUse,
            ] {
                push_keys(
                    &mut expected,
                    key(field),
                    components.range().unwrap().len(),
                    1,
                );
            }
        }
    }
    // R5 orders retained source keys, not the producer's projection rows.
    expected.sort_by_key(|key| {
        let sources = &candidate.input.pending.inner.pending.roots[key.root]
            .coordinates
            .sources
            .rows;
        let instance = sources
            .iter()
            .position(|source| source.instance.index() == key.instance)
            .expect("retained source instance");
        (
            key.root,
            key.family as usize,
            instance,
            key.row,
            key.field as usize,
            key.component,
            key.part,
        )
    });
    expected
}

fn is_core(family: Family) -> bool {
    matches!(
        family,
        Family::InstanceSpans
            | Family::InstanceSeeds
            | Family::InstanceControls
            | Family::InstanceCalls
            | Family::InstanceReturns
    )
}

#[test]
fn source_core_field_keys_have_independent_exact_census() {
    for case in [
        SourceCase::Repeated,
        SourceCase::Slots,
        SourceCase::Shifted,
        SourceCase::Discard,
        SourceCase::BranchParts,
    ] {
        with_projection_candidate(case, |candidate, _| {
            let expected = core_key_census(candidate);
            let actual: Vec<_> = candidate
                .projections
                .rows
                .iter()
                .filter(|row| is_core(row.key.family))
                .map(|row| row.key)
                .collect();
            assert_eq!(actual, expected, "ordered retained field census: {case:?}");
            for (index, row) in candidate.projections.rows.iter().enumerate() {
                assert!(
                    !candidate.projections.rows[..index]
                        .iter()
                        .any(|prior| prior.key == row.key),
                    "duplicate full source key"
                );
            }
        });
    }
}

#[test]
fn mapped_spans_include_only_their_own_lifecycle_insertions() {
    let mut seen_empty_owner = false;
    let mut seen_tombstone = false;
    let mut seen_empty_gap = false;
    for case in [
        SourceCase::Repeated,
        SourceCase::Shifted,
        SourceCase::Discard,
        SourceCase::BranchParts,
    ] {
        with_projection_candidate(case, |candidate, _| {
            let graph = candidate.input.pending.pending_module();
            for (r, root) in candidate
                .input
                .pending
                .inner
                .pending
                .roots
                .iter()
                .enumerate()
            {
                for (ordinal, span) in root.coordinates.spans.rows.iter().enumerate() {
                    let expected = expected_span_sources(root, graph, ordinal);
                    let actual: Vec<_> = candidate
                        .projections
                        .rows
                        .iter()
                        .filter(|row| {
                            row.key.root == r
                                && row.key.family == Family::InstanceSpans
                                && row.key.instance == span.instance.index()
                                && row.key.row == ordinal
                        })
                        .collect();
                    assert_eq!(actual.len(), expected.len());
                    for (part, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                        assert_eq!(actual.key.field, Field::Span);
                        assert_eq!((actual.key.component, actual.key.part), (0, part));
                        assert_eq!(actual.source, expected);
                        match expected {
                            Location::Tombstone => {
                                seen_tombstone = true;
                                assert_eq!(actual.target, Target::Tombstone);
                            }
                            Location::Gap(_) => {
                                seen_empty_gap = true;
                                assert!(matches!(actual.target, Target::Gap(_)));
                            }
                            _ => {}
                        }
                    }
                    let originally_empty = span
                        .segments
                        .iter()
                        .flatten()
                        .all(|segment| segment.count == 0)
                        && span.segments.iter().any(Option::is_some);
                    if originally_empty && root.insertions.iter().any(|i| i.source_span == ordinal)
                    {
                        seen_empty_owner = true;
                        assert!(
                            actual
                                .iter()
                                .any(|row| matches!(row.source, Location::Gap(_)))
                        );
                        assert!(actual.iter().any(|row| matches!(
                            row.source,
                            Location::Origin(Source::Operation(_))
                        )));
                    }
                }
            }
        });
    }
    assert!(
        seen_empty_owner,
        "genuine lifecycle-only source spans must be exercised"
    );
    assert!(
        seen_empty_gap && seen_tombstone,
        "zero width is not a removed call"
    );
}

fn projection_replay(
    candidate: &ScopedTileScalarCandidateV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ScopedTileFailureKindV29> {
    let floor = budget.storage();
    let result = scoped_tile_attempt_v29(budget, |budget| {
        replay_scoped_tile_attachments_v29(
            &candidate.input,
            &candidate.output,
            &candidate.relations,
            &candidate.projections,
            budget,
        )
    });
    assert_eq!(budget.storage(), floor);
    result
}

#[test]
fn attachment_replay_rejects_each_mutable_key_coordinate_and_target() {
    with_projection_candidate(SourceCase::Shifted, |candidate, budget| {
        let index = candidate
            .projections
            .rows
            .iter()
            .position(|row| matches!(row.target, Target::Origin { .. }))
            .unwrap();
        let original = candidate.projections.rows[index];
        let mut changes = Vec::new();
        for field in 0..7 {
            let mut changed = original;
            match field {
                0 => changed.key.root = usize::MAX,
                1 => changed.key.family = Family::Assertion,
                2 => changed.key.instance = usize::MAX,
                3 => changed.key.row = usize::MAX,
                4 => changed.key.field = Field::AssertConditionUse,
                5 => changed.key.component = usize::MAX,
                6 => changed.key.part = usize::MAX,
                _ => unreachable!(),
            }
            changes.push(changed);
        }
        for source in [
            Location::NoOutput,
            Location::Tombstone,
            Location::Origin(Source::Operation(TileScalarPointV29 {
                function: usize::MAX,
                block: 0,
                operation: 0,
            })),
        ] {
            changes.push(TileScalarProjectionV29 { source, ..original });
        }
        for target in [
            Target::NoOutput,
            Target::Tombstone,
            Target::Origin {
                first: usize::MAX,
                count: 2,
            },
            Target::Origin {
                first: 0,
                count: usize::MAX,
            },
            Target::Origin { first: 0, count: 0 },
        ] {
            changes.push(TileScalarProjectionV29 { target, ..original });
        }
        for changed in changes {
            candidate.projections.rows[index] = changed;
            assert_eq!(
                projection_replay(candidate, budget),
                Err(ScopedTileFailureKindV29::ReplayMismatch)
            );
        }
        candidate.projections.rows[index] = original;
        let last = candidate.projections.rows.pop().unwrap();
        assert_eq!(
            projection_replay(candidate, budget),
            Err(ScopedTileFailureKindV29::ReplayMismatch)
        );
        candidate.projections.rows.push(last);
        candidate.projections.rows.swap(0, 1);
        assert_eq!(
            projection_replay(candidate, budget),
            Err(ScopedTileFailureKindV29::ReplayMismatch)
        );
        candidate.projections.rows.swap(0, 1);
        let original_last = *candidate.projections.rows.last().unwrap();
        *candidate.projections.rows.last_mut().unwrap() = candidate.projections.rows[0];
        assert_eq!(
            projection_replay(candidate, budget),
            Err(ScopedTileFailureKindV29::ReplayMismatch)
        );
        *candidate.projections.rows.last_mut().unwrap() = original_last;
        projection_replay(candidate, budget).unwrap();
    });
}

fn use_value(graph: &Module, coordinate: UseCoordinate) -> ValueId {
    let mut values = Vec::new();
    let selected = match coordinate {
        UseCoordinate::OperationOperand { operation, operand } => {
            graph.functions[operation.block.function.0 as usize]
                .body
                .as_ref()
                .unwrap()
                .blocks[operation.block.block as usize]
                .operations[operation.operation as usize]
                .kind
                .visit_operands(|value| values.push(value));
            operand
        }
        UseCoordinate::TerminatorOperand { block, operand } => {
            graph.functions[block.function.0 as usize]
                .body
                .as_ref()
                .unwrap()
                .blocks[block.block as usize]
                .terminator
                .as_ref()
                .unwrap()
                .visit_operands(|value| values.push(value));
            operand
        }
    };
    values[selected as usize]
}

fn edge_value(graph: &Module, coordinate: kir::CanonicalKirEdgeArgumentCoordinateV1) -> ValueId {
    let source = coordinate.edge.source;
    let term = graph.functions[source.function.0 as usize]
        .body
        .as_ref()
        .unwrap()
        .blocks[source.block as usize]
        .terminator
        .as_ref()
        .unwrap();
    let mut edges = Vec::new();
    term.try_visit_edges_v1(|_, arguments| {
        edges.push(arguments);
        Ok::<_, std::convert::Infallible>(())
    })
    .unwrap();
    edges[coordinate.edge.successor as usize][coordinate.argument as usize]
}

#[test]
fn genuine_return_and_transport_uses_preserve_operand_occurrence() {
    with_projection_candidate(SourceCase::BranchParts, |candidate, budget| {
        let mut arguments = 0;
        for row in &candidate.projections.rows {
            match (row.source, row.target) {
                (Location::Use(source), Target::Use(target)) => {
                    assert_eq!(
                        use_value(candidate.input.pending.pending_module(), source),
                        use_value(candidate.output.module(), target)
                    );
                }
                (Location::EdgeArgument(source), Target::EdgeArgument(target)) => {
                    arguments += 1;
                    assert_eq!(source.argument, target.argument);
                    assert_eq!(
                        edge_value(candidate.input.pending.pending_module(), source),
                        edge_value(candidate.output.module(), target)
                    );
                }
                _ => {}
            }
        }
        assert!(arguments > 0, "exercise genuine transported edge arguments");
        let mut checked = Vec::new();
        for index in 0..candidate.projections.rows.len() {
            let original = candidate.projections.rows[index];
            let kind = std::mem::discriminant(&original.target);
            if checked.contains(&kind) {
                continue;
            }
            checked.push(kind);
            let changed = match original.target {
                Target::Use(UseCoordinate::OperationOperand { operation, .. }) => {
                    Some(Target::Use(UseCoordinate::OperationOperand {
                        operation,
                        operand: u32::MAX,
                    }))
                }
                Target::Use(UseCoordinate::TerminatorOperand { block, .. }) => {
                    Some(Target::Use(UseCoordinate::TerminatorOperand {
                        block,
                        operand: u32::MAX,
                    }))
                }
                Target::EdgeArgument(mut coordinate) => {
                    coordinate.argument = u32::MAX;
                    Some(Target::EdgeArgument(coordinate))
                }
                Target::Gap(_) | Target::NoOutput => Some(Target::Tombstone),
                Target::Tombstone => Some(Target::NoOutput),
                _ => None,
            };
            if let Some(target) = changed {
                candidate.projections.rows[index].target = target;
                assert_eq!(
                    projection_replay(candidate, budget),
                    Err(ScopedTileFailureKindV29::ReplayMismatch)
                );
                candidate.projections.rows[index] = original;
            }
        }
    });
}

#[test]
fn shifted_allocations_remain_allocation_operations_not_shifted_body_spans() {
    with_projection_candidate(SourceCase::Shifted, |candidate, budget| {
        scoped_tile_projection_replay_v29::reject_span_slot_and_tombstone_substitutions_v29(
            candidate, budget,
        );
        let graph = candidate.input.pending.pending_module();
        let mut allocations = 0;
        for (r, root) in candidate
            .input
            .pending
            .inner
            .pending
            .roots
            .iter()
            .enumerate()
        {
            let storage = root.slot_relocation.as_ref().unwrap().storage();
            assert_eq!(
                (
                    storage.allocations,
                    storage.payload_bytes,
                    storage.alignment
                ),
                (2, 8, 4)
            );
            for (ordinal, slot) in root.source_slots.slots.iter().enumerate() {
                let instance = slot.instance.index();
                let compact = root.source_slots.instances.iter()
                    .position(|frame| frame.instance == slot.instance).unwrap();
                assert_eq!(root.sidecars.rows[compact].source_call_instance, Some(slot.instance));
                let origin_count = root.sidecars.rows[compact]
                    .scoped_slot_origins
                    .as_ref()
                    .map_or(0, Vec::len);
                let local = ordinal - root.source_slots.instances[compact].slots.start;
                let key = projection_key(
                    r,
                    Family::SourceSlot,
                    instance,
                    origin_count + local,
                    Field::SlotAllocation,
                );
                let projected = projected_row(candidate, key);
                let Location::Origin(Source::Operation(source)) = projected.source else {
                    panic!("allocation source");
                };
                let source_body = graph.functions[source.function].body.as_ref().unwrap();
                assert_eq!(source.block, 0, "hoisted prefix is in entry");
                assert!(matches!(
                    source_body.blocks[source.block].operations[source.operation].kind,
                    OperationKind::Alloca { .. }
                ));
                let Target::Origin { first, count: 1 } = projected.target else {
                    panic!("preserved allocation");
                };
                let Piece::Operation(target) = candidate.relations.pieces[first].piece else {
                    panic!("allocation operation");
                };
                assert_eq!(
                    candidate.output.module().functions[target.function]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[target.block]
                        .operations[target.operation],
                    source_body.blocks[source.block].operations[source.operation]
                );
                allocations += 1;
                for field in [Field::SlotCount, Field::SlotCountLocation] {
                    let row = projected_row(candidate, TileAttachmentKeyV29 { field, ..key });
                    if slot.representation.count().is_none() {
                        assert_eq!(
                            (row.source, row.target),
                            (Location::NoOutput, Target::NoOutput)
                        );
                    } else {
                        assert!(matches!(row.source, Location::Origin(_)));
                    }
                }
            }
        }
        assert_eq!(allocations, 2);
    });
}

fn source_definition(coordinate: Definition) -> Source {
    match coordinate {
        Definition::FunctionArgument { function, argument } => Source::FunctionParameter {
            function: function.0 as usize,
            parameter: argument as usize,
        },
        Definition::BlockArgument { block, argument } => Source::BlockParameter {
            function: block.function.0 as usize,
            block: block.block as usize,
            parameter: argument as usize,
        },
        Definition::Result { operation, result } => Source::Result {
            operation: TileScalarPointV29 {
                function: operation.block.function.0 as usize,
                block: operation.block.block as usize,
                operation: operation.operation as usize,
            },
            result: result as usize,
        },
    }
}

#[test]
fn promoted_failure_diagnostics_coexist_with_real_private_helper_memory() {
    for factory in [
        (|| projection_source_owner(ProjectionSourceVariant::Assertion)) as fn() -> _,
        || extra_source_owner_v29(ExtraSourceWitnessV29::ElidedBounds),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let candidate = scalar_candidate_from_source_v29(factory, ScopedTileOrderV29::Blocked, 64, &mut budget);
        let root = &candidate.input.pending.inner.pending.roots[0];
        assert!(!root.source_slots.slots.is_empty(), "a genuinely nonempty private domain");
        assert!(root.source_slots.pending_memory.is_some(), "completed original memory census");
        let diagnostics = root.sidecars.rows.iter().filter_map(|sidecar| sidecar.scoped_memory_anchors.as_ref())
            .flat_map(|anchors| &anchors.rows).filter(|row| matches!(row.kind, ScopedMemoryAnchorKindV29::FailureRead { .. })).count();
        assert!(diagnostics > 0, "original failure-only reads must not disappear");
        candidate.replay_with_budget(&mut budget).unwrap();
        drop_scalar_candidate(candidate, &mut budget);
        assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    }
}

fn retained_failure_diagnostic_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let base = projection_source_owner(ProjectionSourceVariant::Assertion);
    let semantic = base.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let root = &functions[0];
    let mut locals = root.locals().to_vec();
    let diagnostic = locals.len() as u32;
    locals.push(local(249, U32, SemanticLocalRoleV1::Temporary));
    let mut blocks = root.blocks().to_vec();
    let mut statements = blocks[0].statements().to_vec();
    let condition = statements.pop().expect("original assertion condition");
    let SemanticStatementKindV1::Assign(condition) = condition.kind() else { panic!("condition assignment"); };
    statements.push(assign(place(diagnostic, U32), SemanticRvalueKindV1::Use(scalar(7))));
    statements.push(SemanticStatementV1::new(source(), SemanticStatementKindV1::Store(
        fe2o3_mir_model::semantic_mir_v1::SemanticMemoryStoreV1::new(place(diagnostic, U32), scalar(7),
            fe2o3_mir_model::semantic_mir_v1::SemanticVolatilityV1::NonVolatile, None))));
    statements.push(assign(condition.destination().clone(), SemanticRvalueKindV1::Binary {
        operation: SemanticBinaryOpV1::NotEqual,
        left: SemanticOperandV1::Copy(place(diagnostic, U32)), right: scalar(0),
    }));
    let mut terminator = blocks[0].terminator().kind().clone();
    let SemanticTerminatorKindV1::Assert { message, .. } = &mut terminator else { panic!("source assertion"); };
    *message = SemanticAssertMessageV1::DivisionByZero(SemanticOperandV1::Copy(place(diagnostic, U32)));
    blocks[0] = SemanticBasicBlockV1::new(blocks[0].identity(), blocks[0].source(), statements,
        SemanticTerminatorV1::new(blocks[0].terminator().source(), terminator)).unwrap();
    functions[0] = replace_extra_function_v29(root, root.abi().clone(), locals, blocks);
    let admitted = InertSemanticMirRequestV1::new_with_callables(semantic.target(), semantic.types().to_vec(),
        semantic.allocations().to_vec(), semantic.statics().to_vec(), semantic.vtables().to_vec(), functions,
        semantic.callables().to_vec(), semantic.roots().to_vec()).unwrap()
        .admit_exact_v29(SemanticMirLimitsV1::default()).unwrap();
    ProductionSemanticSsaOwnerV1::try_new(ProductionSemanticMirOwnerV1::try_new(admitted,
        ProductionSemanticMirLimitsV1::default()).unwrap(), ProductionSemanticSsaLimitsV1::default()).unwrap()
}

#[test]
fn retained_assertion_diagnostic_still_requires_its_private_failure_history() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let mut candidate = scalar_candidate_from_source_v29(retained_failure_diagnostic_owner_v29,
        ScopedTileOrderV29::Blocked, 64, &mut budget);
    candidate.replay_with_budget(&mut budget).unwrap();
    let (sidecar, ordinal, original) = {
        let root = &candidate.input.pending.inner.pending.roots[0];
        assert!(!root.source_slots.slots.is_empty());
        assert!(root.source_slots.pending_memory.is_some());
        root.sidecars.rows.iter().enumerate().find_map(|(sidecar, output)| {
            let anchors = output.scoped_memory_anchors.as_ref()?;
            anchors.rows.iter().enumerate().find_map(|(ordinal, anchor)| {
                let ScopedMemoryAnchorKindV29::FailureRead { local, .. } = anchor.kind else { return None; };
                root.source_slots.slots.iter().any(|slot| slot.instance == anchors.subject.instance
                    && slot.origin.identity.original_local() == Some(local))
                    .then_some((sidecar, ordinal, *anchor))
            })
        }).expect("an original diagnostic read with retained private backing")
    };
    let floor = budget.storage();
    // Neither the original graph nor its scalarized successor changes. Replay
    // must still require the complete source-bound failure history.
    let memory = candidate.input.pending.inner.pending.roots[0].source_slots.pending_memory.take();
    assert_eq!(candidate.replay_with_budget(&mut budget), Err(ScopedTileFailureKindV29::Source));
    assert_eq!(budget.storage(), floor);
    candidate.input.pending.inner.pending.roots[0].source_slots.pending_memory = memory;
    candidate.replay_with_budget(&mut budget).unwrap();

    candidate.input.pending.inner.pending.roots[0].sidecars.rows[sidecar]
        .scoped_memory_anchors.as_mut().unwrap().rows.remove(ordinal);
    assert_eq!(candidate.replay_with_budget(&mut budget), Err(ScopedTileFailureKindV29::Source));
    assert_eq!(budget.storage(), floor);
    candidate.input.pending.inner.pending.roots[0].sidecars.rows[sidecar]
        .scoped_memory_anchors.as_mut().unwrap().rows.insert(ordinal, original);
    candidate.replay_with_budget(&mut budget).unwrap();

    let ScopedMemoryAnchorKindV29::FailureRead { event, local } = original.kind else { unreachable!() };
    candidate.input.pending.inner.pending.roots[0].sidecars.rows[sidecar]
        .scoped_memory_anchors.as_mut().unwrap().rows[ordinal].kind =
        ScopedMemoryAnchorKindV29::FailureRead { event, local: local + 1 };
    assert_eq!(candidate.replay_with_budget(&mut budget), Err(ScopedTileFailureKindV29::Source));
    assert_eq!(budget.storage(), floor);
    candidate.input.pending.inner.pending.roots[0].sidecars.rows[sidecar]
        .scoped_memory_anchors.as_mut().unwrap().rows[ordinal] = original;
    candidate.replay_with_budget(&mut budget).unwrap();
    drop_scalar_candidate(candidate, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
}

#[test]
fn canonical_assertion_coordinates_are_not_normalized_twice() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let mut candidate = variant_candidate(ProjectionSourceVariant::Assertion, &mut budget);
    let assertions = &candidate.input.pending.inner.assertions;
    assert_eq!(assertions.len(), 2, "two genuine root source sites");
    assert_ne!(assertions[0].site, assertions[1].site);
    let root = &candidate.input.pending.inner.pending.roots[0];
    assert_eq!(root.coordinates.sources.rows[0].function, ROOT);
    let capture = root.sidecars.rows[0]
        .instance_assert_origins
        .as_ref()
        .unwrap();
    assert_eq!(capture.records.len(), 2);
    for assertion in assertions {
        assert_eq!(assertion.instance, capture.instance);
        let record = capture
            .records
            .iter()
            .position(|row| row.site == assertion.site)
            .unwrap();
        let SemanticKirAssertConditionOutcomeV1::Emitted {
            condition_use,
            definition,
            success_edge,
            failure_edge,
        } = assertion.binding.outcome()
        else {
            panic!("dynamic scalar assertion");
        };
        for (field, source) in [
            (Field::AssertConditionUse, Location::Use(condition_use)),
            (
                Field::AssertConditionDefinition,
                Location::Origin(source_definition(definition)),
            ),
            (
                Field::AssertSuccessEdge,
                Location::Origin(Source::Edge {
                    function: success_edge.source.function.0 as usize,
                    block: success_edge.source.block as usize,
                    edge: success_edge.successor as usize,
                }),
            ),
            (
                Field::AssertFailureEdge,
                Location::Origin(Source::Edge {
                    function: failure_edge.source.function.0 as usize,
                    block: failure_edge.source.block as usize,
                    edge: failure_edge.successor as usize,
                }),
            ),
        ] {
            let row = projected_row(
                &candidate,
                projection_key(0, Family::Assertion, 0, record, field),
            );
            assert_eq!(
                row.source, source,
                "canonical V is not raw E or pre-insertion C"
            );
            if let Location::Use(original) = source {
                let Target::Use(target) = row.target else {
                    panic!("live assertion use");
                };
                assert_eq!(
                    use_value(candidate.input.pending.pending_module(), original),
                    use_value(candidate.output.module(), target)
                );
            }
        }
    }
    scoped_tile_projection_replay_v29::reject_assertion_coordinate_domain_shifts_v29(
        &mut candidate,
        &mut budget,
    );
    projection_replay(&candidate, &mut budget).unwrap();
    drop_scalar_candidate(candidate, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

#[test]
fn independent_storage_kills_at_one_gap_remain_left_sticky_and_distinct() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let candidate = variant_candidate(ProjectionSourceVariant::LifetimeKills, &mut budget);
    scoped_tile_projection_replay_v29::reject_kill_after_lifecycle_insertion_v29(
        &candidate,
        &mut budget,
    );
    let graph = candidate.input.pending.pending_module();
    let mut kills = 0;
    for (r, root) in candidate
        .input
        .pending
        .inner
        .pending
        .roots
        .iter()
        .enumerate()
    {
        for (instance, sidecar) in root.sidecars.rows.iter().enumerate() {
            let Some(anchors) = &sidecar.scoped_memory_anchors else {
                continue;
            };
            for (row, anchor) in anchors.rows.iter().enumerate() {
                let ScopedMemoryAnchorKindV29::Kill {
                    local: 7, cause, ..
                } = anchor.kind
                else {
                    continue;
                };
                if !matches!(
                    cause,
                    ScopedMemoryKillV29::StorageDead | ScopedMemoryKillV29::StorageLive
                ) {
                    continue;
                }
                let spans: Vec<_> = root
                    .coordinates
                    .spans
                    .rows
                    .iter()
                    .filter(|span| {
                        let original = span.source.coordinates().2;
                        span.instance.index() == instance
                            && original.block == anchor.block
                            && original.first as usize == anchor.position
                            && original.count == 0
                    })
                    .collect();
                assert!(!spans.is_empty());
                let mut expected = None;
                for span in spans {
                    let [Some(segment), None] = span.segments else {
                        panic!("ordinary zero-width kill span");
                    };
                    assert_eq!(segment.count, 0);
                    let point = canonical_point(root, graph, segment.block, segment.first, true);
                    if let Some(previous) = expected {
                        assert_eq!(previous, point);
                    } else {
                        expected = Some(point);
                    }
                }
                let key = projection_key(
                    r,
                    Family::MemoryAnchor,
                    instance,
                    row,
                    Field::MemoryPosition,
                );
                let projected = projected_row(&candidate, key);
                assert_eq!(projected.source, Location::Gap(expected.unwrap()));
                assert!(matches!(projected.target, Target::Gap(_)));
                let pointer = projected_row(
                    &candidate,
                    TileAttachmentKeyV29 {
                        field: Field::MemoryPointer,
                        ..key
                    },
                );
                assert_eq!(
                    (pointer.source, pointer.target),
                    (Location::NoOutput, Target::NoOutput)
                );
                kills += 1;
            }
        }
    }
    assert_eq!(
        kills, 4,
        "two separate kills in each of two inlined instances"
    );
    projection_replay(&candidate, &mut budget).unwrap();
    drop_scalar_candidate(candidate, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}

#[test]
fn genuine_ignored_argument_and_elided_assertion_have_complete_projection_census() {
    for variant in [
        ExtraSourceWitnessV29::IgnoredUnit,
        ExtraSourceWitnessV29::ElidedBounds,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
        let candidate = scalar_candidate_from_source_v29(
            || extra_source_owner_v29(variant),
            ScopedTileOrderV29::Blocked,
            64,
            &mut budget,
        );
        candidate.replay_with_budget(&mut budget).unwrap();
        let witnessed = sidecar_census_tests::assert_sidecar_census(&candidate);
        match variant {
            ExtraSourceWitnessV29::IgnoredUnit => {
                assert_eq!(witnessed.raw_ignored_parameters, 2);
                assert_eq!(witnessed.assertions_elided, 0);
                let original = source_owner(SourceCase::Slots);
                let first = original.source_semantic().functions()[0].locals().len();
                let root = &candidate.input.pending.inner.pending.roots[0];
                let mut ignored = root
                    .sidecars
                    .rows
                    .iter()
                    .flat_map(|sidecar| &sidecar.ignored_parameter_bindings);
                for local in first..first + 2 {
                    let row = ignored.next().expect("one row per root source argument");
                    assert_eq!(row.semantic_function, ROOT);
                    assert_eq!(
                        row.semantic_local,
                        SemanticLocalIdV1::from_index(local as u32),
                    );
                    assert_eq!(row.semantic_type, UNIT);
                }
                assert!(ignored.next().is_none());
            }
            ExtraSourceWitnessV29::ElidedBounds => {
                assert_eq!(witnessed.assertions_elided, 1);
                assert_eq!(witnessed.assertions_emitted, 0);
                let rows: Vec<_> = candidate
                    .projections
                    .rows
                    .iter()
                    .filter(|row| row.key.field == Field::AssertConditionUse)
                    .collect();
                assert_eq!(rows.len(), 1);
                assert_eq!(
                    (rows[0].source, rows[0].target),
                    (Location::NoOutput, Target::NoOutput)
                );
            }
        }
        drop_scalar_candidate(candidate, &mut budget);
        assert_eq!(budget.storage(), SCHEDULE_FLOOR);
        budget.release_storage(SCHEDULE_FLOOR).unwrap();
    }
}

#[test]
fn genuine_slice_metadata_returns_project_two_nonempty_conversions() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
    let candidate = scalar_candidate_from_source_v29(
        return_conversion_source_owner_v29,
        ScopedTileOrderV29::Blocked,
        64,
        &mut budget,
    );
    candidate.replay_with_budget(&mut budget).unwrap();
    let witnessed = sidecar_census_tests::assert_sidecar_census(&candidate);
    let graph = candidate.input.pending.pending_module();
    let mut conversions = 0;
    for (r, root) in candidate
        .input
        .pending
        .inner
        .pending
        .roots
        .iter()
        .enumerate()
    {
        assert_eq!(
            root.sidecars.rows.len(),
            root.coordinates.sources.rows.len()
        );
        let mut instances = Vec::new();
        for (instance, sidecar) in root.sidecars.rows.iter().enumerate() {
            let source = &root.coordinates.sources.rows[instance];
            if source.function != METADATA_HELPER_V29 {
                continue;
            }
            assert!(!instances.contains(&source.instance));
            instances.push(source.instance);
            assert_eq!(sidecar.source_call_instance, Some(source.instance));
            let sites = &sidecar.call_returns.sites.rows;
            assert_eq!(sites.len(), 1);
            assert_eq!(sites[0].semantic_function, METADATA_HELPER_V29);
            let SemanticKirCallReturnKindV1::Return { components } = sites[0].kind else {
                panic!("metadata helper returns directly");
            };
            assert!(matches!(
                &sidecar.call_returns.components.rows[components.range().unwrap()],
                [CallResultComponentV1::Return {
                    conversion: Some(_),
                    ..
                }]
            ));
            // Locate the raw key directly from its retained table ordinal,
            // independently of the production attachment source visitor.
            let row = sidecar.blocks.len()
                + sidecar.statement_operation_spans.len()
                + sidecar.terminator_operation_spans.len()
                + sidecar.generated_terminator_values.len();
            let projected = projected_row(
                &candidate,
                projection_key(
                    r,
                    Family::RawSidecar,
                    instance,
                    row,
                    Field::RawReturnConversion,
                ),
            );
            let Location::Origin(Source::Operation(point)) = projected.source else {
                panic!("nonempty conversion maps an original operation");
            };
            assert_eq!(point.function, root.function_ordinal);
            let body = graph.functions[point.function].body.as_ref().unwrap();
            let operation = &body.blocks[point.block].operations[point.operation];
            let OperationKind::Cast {
                kind: CastKind::Bitcast,
                value,
                to,
            } = &operation.kind
            else {
                panic!("real return conversion must be a bitcast");
            };
            assert_eq!(*to, Type::Scalar(ScalarType::U64));
            let definitions: Vec<_> = body
                .blocks
                .iter()
                .flat_map(|block| &block.operations)
                .filter(|op| op.results.iter().any(|result| result.id == *value))
                .collect();
            assert_eq!(definitions.len(), 1);
            assert!(matches!(
                definitions[0].kind,
                OperationKind::SliceLength { .. }
            ));
            assert_eq!(definitions[0].results.len(), 1);
            assert_eq!(definitions[0].results[0].ty, Type::INDEX);
            assert!(matches!(projected.target, Target::Origin { count: 1, .. }));
            conversions += 1;
        }
        assert_eq!(instances.len(), 2);
    }
    assert_eq!(
        conversions, 2,
        "two genuine helper instances, not duplicated metadata"
    );
    assert_eq!(witnessed.raw_conversions_present, conversions);
    drop_scalar_candidate(candidate, &mut budget);
    assert_eq!(budget.storage(), SCHEDULE_FLOOR);
    budget.release_storage(SCHEDULE_FLOOR).unwrap();
}
