use super::*;
use ProductionTileOccurrenceCoordinateV29 as Coordinate;
use ProductionTilePendingKindV29 as PendingKind;
use ProductionTileScalarOrderV29 as Order;
use ProductionTileScalarTransportErrorV29 as Error;
use ProductionTileScalarTransportOwnerV29 as Owner;
use scoped_tile_transport_v29 as transport_impl;

#[path = "production_scoped_tile_transport_hostile_v29_tests.rs"]
mod hostile;
#[path = "production_scoped_tile_transport_resources_v29_tests.rs"]
mod resources;

fn owner_from_pending(
    pending: ProductionPendingScopedSourceOwnerV29,
    order: Order,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Owner {
    let mut donor = Some(pending);
    let owner = Owner::try_from_pending_with_budget_v29(&mut donor, order, budget).unwrap();
    assert!(donor.is_none());
    owner
}
fn tile_owner(case: SourceCase, order: Order, budget: &mut ArgumentBudgetV1<'_>) -> Owner {
    let (pending, capture) = pending_source(case, false, None, budget);
    assert_eq!(capture, 0);
    owner_from_pending(pending, order, budget)
}
fn drop_owner(owner: Owner, budget: &mut ArgumentBudgetV1<'_>) {
    let retained = owner.adopted_storage();
    drop(owner);
    budget.release_storage(retained).unwrap();
}
fn check_complete(owner: &Owner, budget: &mut ArgumentBudgetV1<'_>) {
    let before = budget.storage();
    owner.with_checked_transport_v29(budget, |checked, budget| {
        let inventory = checked.current_inventory(budget)?;
        let original = checked.pending_ancestor(budget)?.pending_module();
        let _: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV18 = inventory.owner();
        assert!(std::ptr::eq(inventory.owner(), &owner.candidate_for_test_v29().output));
        assert_eq!(inventory.identity_v18(), checked.current_identity(budget)?);
        assert_eq!(inventory.owner().module().storage_layouts, original.storage_layouts);
        assert_eq!(checked.pending_identity(budget)?, owner.candidate_for_test_v29().input.pending.pending_identity());
        assert_eq!(inventory.functions().len(), original.functions.len());
        assert_eq!(inventory.kernels().len(), original.kernels.len());
        for (actual, original) in inventory.functions().iter().zip(&original.functions) {
            assert_eq!(actual.function.id, original.id);
            assert_eq!(actual.function.signature, original.signature);
            assert_eq!(actual.function.role, original.role);
            assert_eq!(actual.function.body.is_some(), original.body.is_some());
        }
        assert_eq!(checked.operation_count(budget)?, inventory.operations().len());
        assert_eq!(checked.definition_count(budget)?, inventory.definitions().len());
        assert_eq!(checked.use_site_count(budget)?, inventory.uses().len());
        assert_eq!(checked.block_count(budget)?, inventory.blocks().len());
        assert_eq!(checked.terminator_count(budget)?, inventory.blocks().len());
        assert_eq!(checked.edge_count(budget)?, inventory.edges().len());
        assert_eq!(checked.edge_argument_count(budget)?, inventory.edge_arguments().len());
        for (i, actual) in inventory.operations().iter().enumerate() {
            let row = checked.operation(i, budget)?;
            assert_eq!(row.coordinate(), Coordinate::Operation(actual.coordinate));
            assert!(!row.piece_aliases().is_empty());
            for alias in row.piece_aliases() {
                let piece = checked.piece(checked.piece_alias(alias, budget)?, budget)?;
                assert_eq!(piece.target(), ProductionTileTargetCoordinateV29::Operation(actual.coordinate));
            }
        }
        let mut multiple_aliases = 0;
        for (i, actual) in inventory.definitions().iter().enumerate() {
            let row = checked.definition(i, budget)?;
            assert_eq!(row.coordinate(), Coordinate::Definition(actual.coordinate));
            assert_eq!(row.value(), actual.value);
            assert_eq!(row.definition(), Some(i));
            if row.piece_aliases().len() > 1 { multiple_aliases += 1; }
            for alias in row.piece_aliases() {
                let piece = checked.piece_alias(alias, budget)?;
                assert!(piece < checked.piece_count(budget)?);
            }
        }
        assert!(multiple_aliases > 0, "defining-operation and source-result associations both survive");
        for (i, actual) in inventory.uses().iter().enumerate() {
            let row = checked.use_site(i, budget)?;
            assert_eq!(row.coordinate(), Coordinate::Use(actual.coordinate));
            assert_eq!(row.definition(), Some(actual.definition));
            assert_eq!(row.value(), Some(actual.value));
        }
        for (i, actual) in inventory.blocks().iter().enumerate() {
            assert_eq!(checked.block(i, budget)?.coordinate(), Coordinate::Block(actual.coordinate));
            assert_eq!(checked.terminator(i, budget)?.coordinate(), Coordinate::Terminator(actual.coordinate));
        }
        for (i, actual) in inventory.edges().iter().enumerate() {
            assert_eq!(checked.edge(i, budget)?.coordinate(), Coordinate::Edge(actual.coordinate));
        }
        for (i, actual) in inventory.edge_arguments().iter().enumerate() {
            let row = checked.edge_argument(i, budget)?;
            assert_eq!(row.coordinate(), Coordinate::EdgeArgument(actual.coordinate));
            assert_eq!(row.definition(), Some(actual.incoming_definition));
            assert_eq!(row.target_definition(), Some(actual.target_definition));
            assert_eq!(row.value(), Some(actual.value));
        }
        let mut expected_reads = 0;
        let mut parts = 0;
        for function in &original.functions {
            if let Some(body) = &function.body {
                for block in &body.blocks {
                    for operation in &block.operations {
                        match operation.kind {
                            OperationKind::Execution(fe2o3_kernel_ir::ExecutionOperationV15::MaskedTileLoadU32 { elements, .. }) => expected_reads += usize::from(elements),
                            OperationKind::Execution(fe2o3_kernel_ir::ExecutionOperationV15::FragmentIntoPartsU32 { .. }) => parts += operation.results.len(),
                            _ => {},
                        }
                    }
                }
            }
        }
        let mut kinds = [0; 5];
        let mut reads = std::collections::BTreeSet::new();
        for i in 0..checked.pending_obligation_count(budget)? {
            let row = checked.pending_obligation(i, budget)?;
            let slot = match row.kind() {
                PendingKind::Source { alias } => {
                    let source = checked.source_alias(alias, budget)?;
                    assert!(!source.attachments().is_empty());
                    0
                },
                PendingKind::CollectiveLifecycle { .. } => 1,
                PendingKind::LaunchGeometry { root } => {
                    assert_eq!(checked.root(root, budget)?.launch().selected_root(),
                        checked.source_semantic(budget)?.roots()[root]);
                    2
                },
                PendingKind::GlobalRead => {
                    let read = row.global_read().expect("typed generated read");
                    assert!(reads.insert(read.output()));
                    assert_eq!(inventory.operations()[read.operation()].coordinate, read.output());
                    assert!(matches!(inventory.operations()[read.operation()].operation.kind,
                        OperationKind::Load { access, .. } if access.address_space == fe2o3_kernel_ir::AddressSpace::Global));
                    assert_eq!(checked.origin(read.origin(), budget)?.source(), ProductionTileSourceCoordinateV29::Operation(read.source()));
                    assert!(checked.origin(read.origin(), budget)?.pieces().contains(&read.piece()));
                    assert_eq!(checked.source_alias(read.source_alias(), budget)?.instance(), read.instance());
                    assert_eq!(checked.root(read.root(), budget)?.function(), read.output().block.function);
                    assert!(read.component() < u32::from(read.elements()));
                    3
                },
                PendingKind::RetainedAttachment { attachment } => { checked.attachment(attachment, budget)?; 4 },
            };
            kinds[slot] += 1;
        }
        assert_eq!(kinds[0], checked.source_alias_count(budget)?);
        assert_eq!(kinds[2], checked.root_count(budget)?);
        assert_eq!(kinds[3], expected_reads);
        assert_eq!(kinds[4], checked.attachment_count(budget)?);
        assert!(kinds.iter().all(|count| *count > 0));
        let mut actual_parts = 0;
        for i in 0..checked.piece_count(budget)? {
            let piece = checked.piece(i, budget)?;
            if piece.stage() == ProductionTileExpansionStageV29::Parts
                && matches!(piece.target(), ProductionTileTargetCoordinateV29::Result { .. }) { actual_parts += 1; }
        }
        assert_eq!(actual_parts, parts);
        Ok(())
    }).unwrap();
    assert_eq!(budget.storage(), before);
}
#[test]
fn both_schedules_preserve_complete_genuine_source_occurrences_and_aliases() {
    for order in [Order::Blocked, Order::Striped] {
        for case in [
            SourceCase::Repeated,
            SourceCase::Slots,
            SourceCase::Shifted,
            SourceCase::Discard,
            SourceCase::BranchParts,
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
            budget.reserve_storage(SCHEDULE_FLOOR).unwrap();
            let owner = tile_owner(case, order, &mut budget);
            check_complete(&owner, &mut budget);
            drop_owner(owner, &mut budget);
            assert_eq!(budget.storage(), SCHEDULE_FLOOR);
        }
    }
}
#[test]
fn actual_geometry_and_overflow_boundaries_preserve_every_pending_read() {
    for order in [Order::Blocked, Order::Striped] {
        for (lanes, base) in [
            (3, 0),
            (128, 0),
            (64, 7),
            (64, 65),
            (64, u64::MAX),
            (64, u64::MAX - 1),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
            let pending = scalar_pending_from_source_v29(
                || configured_source_owner(SourceCase::Shifted, lanes, base),
                lanes,
                &mut budget,
            )
            .unwrap();
            let owner = owner_from_pending(pending, order, &mut budget);
            check_complete(&owner, &mut budget);
            drop_owner(owner, &mut budget);
            assert_eq!(budget.storage(), 0);
        }
    }
}
#[test]
fn full_cyclic_bodyless_non_topological_and_multi_root_roster_is_not_filtered() {
    for order in [Order::Blocked, Order::Striped] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        let prepared = preservation_prepared(
            match order {
                Order::Blocked => ScopedTileOrderV29::Blocked,
                Order::Striped => ScopedTileOrderV29::Striped,
            },
            &mut budget,
        );
        let bytes = prepared.adopted_storage();
        let PreparedScopedTileSourceV29 {
            pending,
            selections,
            ..
        } = prepared;
        drop(selections);
        budget
            .release_storage(bytes - pending.adopted_storage())
            .unwrap();
        let owner = owner_from_pending(pending, order, &mut budget);
        check_complete(&owner, &mut budget);
        owner
            .with_checked_transport_v29(&mut budget, |view, budget| {
                let actual = view.current_inventory(budget)?;
                assert_eq!(actual.functions().len(), 5);
                assert_eq!(actual.kernels().len(), 4);
                assert!(actual.functions()[4].function.body.is_none());
                assert_eq!(
                    actual.functions()[1].function.id.as_str(),
                    "z_ordinary_cycle"
                );
                assert_eq!(
                    actual.functions()[2].function.id.as_str(),
                    "a_ordinary_assert"
                );
                Ok(())
            })
            .unwrap();
        drop_owner(owner, &mut budget);
        assert_eq!(budget.storage(), 0);
    }
}
#[test]
fn no_output_source_aliases_and_ignored_units_remain_pending() {
    for variant in [
        ExtraSourceWitnessV29::IgnoredUnit,
        ExtraSourceWitnessV29::ElidedBounds,
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
        let pending =
            scalar_pending_from_source_v29(|| extra_source_owner_v29(variant), 64, &mut budget)
                .unwrap();
        let owner = owner_from_pending(pending, Order::Blocked, &mut budget);
        check_complete(&owner, &mut budget);
        owner
            .with_checked_transport_v29(&mut budget, |view, budget| {
                let mut logical = 0;
                for i in 0..view.attachment_count(budget)? {
                    if matches!(
                        view.attachment(i, budget)?.target(),
                        ProductionTileAttachmentTargetV29::Tombstone
                            | ProductionTileAttachmentTargetV29::NoOutput
                    ) {
                        logical += 1;
                    }
                }
                assert!(logical > 0);
                Ok(())
            })
            .unwrap();
        drop_owner(owner, &mut budget);
    }
}

#[test]
fn public_call_coordinate_wrappers_preserve_fields_and_layout() {
    use std::mem::align_of;
    assert_eq!(
        size_of::<ProductionTileCallInstanceV29>(),
        size_of::<ProductionCallInstanceIdV1>()
    );
    assert_eq!(
        align_of::<ProductionTileCallInstanceV29>(),
        align_of::<ProductionCallInstanceIdV1>()
    );
    assert_eq!(
        size_of::<ProductionTileCallOccurrenceV29>(),
        size_of::<ProductionCallOccurrenceV1>()
    );
    assert_eq!(
        align_of::<ProductionTileCallOccurrenceV29>(),
        align_of::<ProductionCallOccurrenceV1>()
    );
    assert_eq!(
        size_of::<Option<ProductionTileCallOccurrenceV29>>(),
        size_of::<Option<ProductionCallOccurrenceV1>>()
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
    let (pending, _) = pending_source(SourceCase::Repeated, false, None, &mut budget);
    let mut instances = Vec::new();
    let mut aliases = Vec::new();
    for (root, retained) in pending.inner.pending.roots.iter().enumerate() {
        for row in &retained.coordinates.sources.rows {
            instances.push((
                root,
                row.instance.index(),
                row.function,
                row.identity,
                row.incoming.map(|call| (call.caller.index(), call.block)),
            ));
        }
        for row in &retained.coordinates.spans.rows {
            aliases.push((
                root,
                row.instance.index(),
                row.removed_call
                    .map(|call| (call.caller.index(), call.block)),
            ));
        }
    }
    assert!(instances.iter().any(|row| row.4.is_some()));
    assert!(instances.iter().any(|row| row.4.is_none()));
    assert!(aliases.iter().any(|row| row.2.is_some()));
    assert!(aliases.iter().any(|row| row.2.is_none()));
    let owner = owner_from_pending(pending, Order::Blocked, &mut budget);
    owner
        .with_checked_transport_v29(&mut budget, |view, budget| {
            assert_eq!(view.instance_count(budget)?, instances.len());
            for (ordinal, expected) in instances.iter().enumerate() {
                let row = view.instance(ordinal, budget)?;
                assert_eq!(
                    (
                        row.root(),
                        row.instance().index(),
                        row.function(),
                        row.identity(),
                        row.incoming()
                            .map(|call| (call.caller().index(), call.block()))
                    ),
                    *expected
                );
            }
            assert_eq!(view.source_alias_count(budget)?, aliases.len());
            for (ordinal, expected) in aliases.iter().enumerate() {
                let row = view.source_alias(ordinal, budget)?;
                assert_eq!(
                    (
                        row.root(),
                        row.instance().index(),
                        row.removed_call()
                            .map(|call| (call.caller().index(), call.block()))
                    ),
                    *expected
                );
            }
            Ok(())
        })
        .unwrap();
    drop_owner(owner, &mut budget);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn generated_read_subjects_match_original_loads_and_schedule() {
    use fe2o3_kernel_ir::{
        CanonicalKirBlockCoordinateV1, CanonicalKirFunctionCoordinateV1,
        CanonicalKirOperationCoordinateV1, ExecutionOperationV15,
    };
    struct ExpectedRead {
        source: CanonicalKirOperationCoordinateV1,
        root: usize,
        instance: usize,
        selection: usize,
        alias: usize,
        input: ValueId,
        base: ValueId,
        workgroup: SemanticExecutionIdentityV29,
        lanes: u16,
        elements: u16,
    }
    for order in [Order::Blocked, Order::Striped] {
        for (case, lanes, base) in [
            (SourceCase::Repeated, 64, 0),
            (SourceCase::Slots, 64, 0),
            (SourceCase::Shifted, 3, 7),
            (SourceCase::Shifted, 128, u64::MAX),
            (SourceCase::BranchParts, 64, 65),
        ] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(SCHEDULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, SCHEDULE_LIMIT);
            let pending = scalar_pending_from_source_v29(
                || configured_source_owner(case, lanes, base),
                lanes,
                &mut budget,
            )
            .unwrap();
            let request = ScopedTileScheduleInputV29 {
                order: match order {
                    Order::Blocked => ScopedTileOrderV29::Blocked,
                    Order::Striped => ScopedTileOrderV29::Striped,
                },
            };
            let mut donor = Some((pending, request));
            let prepared = prepare_scoped_tile_source_v29(&mut donor, &mut budget).unwrap();
            assert!(donor.is_none());
            // Capture the oracle before transport exists, from original Load operands
            // and the separately prepared source schedule and semantic type roster.
            let mut expected = Vec::new();
            for (selection, selected) in prepared.selections.iter().enumerate() {
                let roots = &prepared.pending.inner.pending.roots;
                let root = &roots[selected.root];
                let body = prepared.pending.pending_module().functions[root.function_ordinal]
                    .body
                    .as_ref()
                    .unwrap();
                let block = body
                    .blocks
                    .iter()
                    .position(|block| block.id == selected.witness.after.block)
                    .unwrap();
                let operation =
                    &body.blocks[block].operations[selected.witness.after.first as usize];
                let OperationKind::Execution(ExecutionOperationV15::MaskedTileLoadU32 {
                    workgroup: value,
                    input,
                    base,
                    lanes,
                    elements,
                }) = operation.kind
                else {
                    panic!("scheduled original Load")
                };
                let DeferredTileInputV29::Load { workgroup, .. } = selected.tile.input else {
                    panic!("scheduled source Load")
                };
                assert_eq!(workgroup.value, value);
                let semantic = prepared.pending.inner.source.owner.source_semantic();
                assert_eq!(
                    workgroup.type_identity,
                    semantic.types()[workgroup.semantic_type.index() as usize].identity()
                );
                assert!(
                    root.coordinates
                        .sources
                        .rows
                        .iter()
                        .any(|row| row.instance == workgroup.producer.caller)
                );
                assert_eq!(
                    root.coordinates.spans.rows[selected.witness.source_span].instance,
                    selected.witness.instance
                );
                expected.push(ExpectedRead {
                    source: CanonicalKirOperationCoordinateV1 {
                        block: CanonicalKirBlockCoordinateV1 {
                            function: CanonicalKirFunctionCoordinateV1(
                                u32::try_from(root.function_ordinal).unwrap(),
                            ),
                            block: u32::try_from(block).unwrap(),
                        },
                        operation: selected.witness.after.first,
                    },
                    root: selected.root,
                    instance: selected.witness.instance.index(),
                    selection,
                    alias: roots[..selected.root]
                        .iter()
                        .map(|root| root.coordinates.spans.rows.len())
                        .sum::<usize>()
                        + selected.witness.source_span,
                    input,
                    base,
                    workgroup,
                    lanes,
                    elements,
                });
            }
            assert!(!expected.is_empty());
            let retained = prepared.adopted_storage();
            let PreparedScopedTileSourceV29 {
                pending,
                selections,
                ..
            } = prepared;
            drop(selections);
            budget
                .release_storage(retained - pending.adopted_storage())
                .unwrap();
            let owner = owner_from_pending(pending, order, &mut budget);
            owner
                .with_checked_transport_v29(&mut budget, |view, budget| {
                    let mut components = std::collections::BTreeSet::new();
                    for ordinal in 0..view.pending_obligation_count(budget)? {
                        let row = view.pending_obligation(ordinal, budget)?;
                        let Some(read) = row.global_read() else {
                            continue;
                        };
                        let expected = expected
                            .iter()
                            .find(|expected| expected.source == read.source())
                            .expect("read belongs to independently enumerated source Load");
                        assert_eq!(row.kind(), PendingKind::GlobalRead);
                        assert_eq!(read.root(), expected.root);
                        assert_eq!(read.instance().index(), expected.instance);
                        assert_eq!(read.selection(), expected.selection);
                        assert_eq!(read.source_alias(), expected.alias);
                        assert_eq!(read.input(), expected.input);
                        assert_eq!(read.base(), expected.base);
                        assert_eq!(read.lanes(), expected.lanes);
                        assert_eq!(read.elements(), expected.elements);
                        let workgroup = read.workgroup();
                        assert_eq!(workgroup.value(), expected.workgroup.value);
                        assert_eq!(workgroup.semantic_type(), expected.workgroup.semantic_type);
                        assert_eq!(workgroup.type_identity(), expected.workgroup.type_identity);
                        assert_eq!(
                            workgroup.producer().caller().index(),
                            expected.workgroup.producer.caller.index()
                        );
                        assert_eq!(
                            workgroup.producer().block(),
                            expected.workgroup.producer.block
                        );
                        assert!(read.component() < u32::from(expected.elements));
                        assert!(
                            components.insert((expected.selection, read.component())),
                            "duplicate source component"
                        );
                    }
                    let count: usize = expected.iter().map(|row| usize::from(row.elements)).sum();
                    assert_eq!(components.len(), count);
                    for row in &expected {
                        for component in 0..u32::from(row.elements) {
                            assert!(
                                components.contains(&(row.selection, component)),
                                "missing source component"
                            );
                        }
                    }
                    Ok(())
                })
                .unwrap();
            drop_owner(owner, &mut budget);
            assert_eq!(budget.storage(), 0);
        }
    }
}
