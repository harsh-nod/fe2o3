use super::*;

fn copied_atomic_receiver_access_v41(
    original: &SourceReferenceAccessRecordV29,
) -> SourceReferenceAccessRecordV29 {
    SourceReferenceAccessRecordV29 {
        key: original.key,
        source_local: original.source_local,
        ty: original.ty,
        instance: original.instance,
        local: original.local,
        generation: original.generation,
        projections: original.projections.clone(),
        loan: original.loan,
        traversed: original.traversed.clone(),
        shared_path: original.shared_path,
        checked_enum_read: original.checked_enum_read,
    }
}

#[test]
fn atomic_receiver_v41_exact_original_capture_uses_existing_root_representation() {
    let entered = std::cell::Cell::new(false);
    cells_tests::run_cells(
        owner41_atomic(
            Fault41::None,
            Some((
                SemanticAtomicRmwOpV1::Exchange,
                SemanticAtomicOrderingV1::Relaxed,
            )),
        ),
        |plan, budget| {
            assert_eq!(plan.atomic_captures.len(), 1);
            let capture = &plan.atomic_captures[0];
            let original = plan
                .accesses
                .iter()
                .find(|row| row.key.site == capture.site && row.key.source == capture.source)
                .unwrap();
            assert!(source_atomic_receiver_capture_read_v41(
                plan, original, budget
            )?);
            let rows = source_existing_receiver_rows_v29(plan, budget)?;
            assert!(source_existing_receiver_v29(
                &rows,
                plan.root,
                SemanticLocalIdV1::from_index(1),
                budget
            )?);
            assert!(
                !plan
                    .cells
                    .rows
                    .iter()
                    .any(|cell| cell.instance == plan.root && cell.local.index() == 1)
            );
            let mut demands = 0;
            for source in &plan.representation_demands {
                if source.instance == plan.root && source.local.index() == 1 {
                    assert!(source_atomic_receiver_value_v41(plan, source, budget)?);
                    demands += 1;
                }
            }
            // Actual original source only borrows/captures its entry root; it
            // does not replace that aggregate, so no root demand is installed.
            assert_eq!(demands, 0);
            entered.set(true);
            Ok(())
        },
    )
    .unwrap();
    assert!(entered.get());
}

#[test]
fn atomic_receiver_v41_changed_or_non_capture_accesses_keep_storage_obligations() {
    cells_tests::run_cells(
        owner41_atomic(
            Fault41::None,
            Some((
                SemanticAtomicRmwOpV1::Exchange,
                SemanticAtomicOrderingV1::Relaxed,
            )),
        ),
        |plan, budget| {
            let capture = &plan.atomic_captures[0];
            let original = plan
                .accesses
                .iter()
                .find(|row| row.key.site == capture.site && row.key.source == capture.source)
                .unwrap();
            for fault in 0..11 {
                let mut changed = copied_atomic_receiver_access_v41(original);
                match fault {
                    0 => changed.key.access = SourceReferenceAccessV29::Write,
                    1 => changed.key.access = SourceReferenceAccessV29::Address,
                    2 => changed.key.source = 0,
                    3 => changed.key.site.statement = None,
                    4 => changed.local = SemanticLocalIdV1::from_index(2),
                    5 => changed.generation = 1,
                    6 => changed.loan = None,
                    7 => changed.projections = 0..0,
                    8 => changed.traversed = 0..0,
                    9 => changed.shared_path = false,
                    10 => changed.source_local = SemanticLocalIdV1::from_index(1),
                    _ => unreachable!(),
                }
                assert!(
                    !source_atomic_receiver_capture_read_v41(plan, &changed, budget)?,
                    "fault {fault}"
                );
            }
            let borrowed = plan
                .accesses
                .iter()
                .find(|row| matches!(row.key.access, SourceReferenceAccessV29::Borrow(_)))
                .unwrap();
            assert!(!source_atomic_receiver_capture_read_v41(
                plan, borrowed, budget
            )?);
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn atomic_receiver_v41_direct_value_predicate_and_restart_keep_entry_obligations() {
    cells_tests::run_cells(
        owner41_atomic(
            Fault41::None,
            Some((
                SemanticAtomicRmwOpV1::Exchange,
                SemanticAtomicOrderingV1::Relaxed,
            )),
        ),
        |plan, budget| {
            assert!(
                !plan
                    .representation_demands
                    .iter()
                    .any(|source| source.instance == plan.root && source.local.index() == 1)
            );
            // Direct private-predicate coverage only: this is NOT an installed
            // or retained source demand and confers no source authority. The
            // original fixture has no replacement. Use its authentic root node
            // to exercise the closed candidate-value checks and hostile inputs.
            let custody = plan.atomic_custody[plan.atomic_captures[0].custody];
            let node = plan
                .nodes
                .iter()
                .position(|node| {
                    node.ty == custody.anchor.ty
                        && node.atomic_custody.is_none()
                        && node.descriptor.is_none()
                        && matches!(node.kind, SourceReferenceNodeKindV29::Plain(Some(anchor))
                    if anchor == custody.anchor)
                })
                .unwrap();
            let original = SourceReferenceRepresentationDemandV29 {
                instance: custody.instance,
                local: custody.local,
                generation: custody.generation,
                projections: 0..0,
                selector_source: None,
                node,
            };
            assert!(source_atomic_receiver_value_v41(plan, &original, budget)?);
            for fault in 0..4 {
                let mut changed = SourceReferenceRepresentationDemandV29 {
                    instance: original.instance,
                    local: original.local,
                    generation: original.generation,
                    projections: original.projections.clone(),
                    selector_source: original.selector_source,
                    node: original.node,
                };
                match fault {
                    0 => changed.generation = 1,
                    1 => changed.local = SemanticLocalIdV1::from_index(2),
                    2 => changed.node = plan.atomic_captures[0].node,
                    3 => {
                        changed.selector_source =
                            Some((plan.atomic_captures[0].site, plan.atomic_captures[0].source))
                    }
                    _ => unreachable!(),
                }
                assert!(
                    !source_atomic_receiver_value_v41(plan, &changed, budget)?,
                    "fault {fault}"
                );
            }
            let rows = source_existing_receiver_rows_v29(plan, budget)?;
            let entry = SourceReferenceStorageActivationV29 {
                instance: plan.root,
                local: SemanticLocalIdV1::from_index(1),
                generation: 0,
                origin: SourceReferenceActivationOriginV29::Entry,
            };
            assert!(source_existing_receiver_entry_v29(
                plan, &rows, entry, budget
            )?);
            assert!(!source_existing_receiver_entry_v29(
                plan,
                &rows,
                SourceReferenceStorageActivationV29 {
                    generation: 1,
                    ..entry
                },
                budget
            )?);
            assert!(!source_existing_receiver_entry_v29(
                plan,
                &rows,
                SourceReferenceStorageActivationV29 {
                    origin: SourceReferenceActivationOriginV29::StorageLive(
                        plan.atomic_captures[0].site
                    ),
                    ..entry
                },
                budget
            )?);
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn atomic_receiver_v41_original_alias_move_and_dead_owner_refusals_remain() {
    for fault in [
        Fault41::ByValue,
        Fault41::DeadRoot,
        Fault41::OrdinaryLoad,
        Fault41::OrdinaryStore,
        Fault41::Offset,
        Fault41::Expose,
        Fault41::PreViewCopy,
        Fault41::PreViewMove,
    ] {
        let entered = std::cell::Cell::new(false);
        let result = cells_tests::run_cells(
            owner41_atomic(
                fault,
                Some((
                    SemanticAtomicRmwOpV1::Exchange,
                    SemanticAtomicOrderingV1::Relaxed,
                )),
            ),
            |_, _| {
                entered.set(true);
                Ok(())
            },
        );
        assert!(result.is_err(), "original source fault {fault:?}");
        assert!(!entered.get(), "original source fault {fault:?}");
    }
}
