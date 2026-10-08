use super::*;

fn descendant_owner_v41(
    fault: Fault41,
    kind: SemanticAtomicRmwOpV1,
    order: SemanticAtomicOrderingV1,
    captured: bool,
) -> ProductionSemanticSsaOwnerV1 {
    // The original source has AddressOf to const cell, then one typed pointer
    // Cast directly to mutable scalar. No intermediate cast is emitted.
    let mut owner = owner41_atomic_path_uncaptured(
        fault,
        Some((kind, order)),
        Some(SemanticMutabilityV1::Immutable),
        true,
    );
    if captured {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
        owner
            .try_capture_occurrences_with_budget_v1(&mut ArgumentBudgetV1::new(
                &mut work,
                usize::MAX,
            ))
            .unwrap();
    }
    owner
}

#[test]
fn atomic_descendant_v41_original_depth_one_to_three_emits_and_replays_exact_formation() {
    use SemanticAtomicOrderingV1::*;
    use SemanticAtomicRmwOpV1::*;
    for kind in [
        Exchange,
        Add,
        Subtract,
        BitAnd,
        BitOr,
        BitXor,
        UnsignedMinimum,
        UnsignedMaximum,
        SignedMinimum,
        SignedMaximum,
    ] {
        for order in [
            Relaxed,
            Release,
            Acquire,
            AcquireRelease,
            SequentiallyConsistent,
        ] {
            let reached = std::cell::Cell::new(false);
            with_atomic_source_lowered(
                descendant_owner_v41(Fault41::None, kind, order, true),
                |plan, emission, emitted, budget| {
                    assert_eq!(plan.atomic_formations.len(), 4);
                    assert_eq!(emission.atomic_receipts.len(), 6);
                    assert!(
                        emission
                            .atomic_receipts
                            .iter()
                            .all(|row| row.get().is_some())
                    );
                    let rows: Vec<_> = plan
                        .atomic_formations
                        .iter()
                        .filter(|row| {
                            row.operation == SourceAtomicViewOperationV41::Cast
                                && plan.atomic_custody[row.input]
                                    .view
                                    .is_some_and(|path| path.depth == 1)
                                && plan.atomic_custody[row.output]
                                    .view
                                    .is_some_and(|path| path.depth == 3)
                        })
                        .collect();
                    assert_eq!(rows.len(), 1);
                    let row = rows[0];
                    let declaration = plan
                        .instances
                        .instance(row.site.instance)
                        .unwrap()
                        .declaration();
                    let site = ExecutionSiteV29::Statement {
                        block: SsaBlockIdV1::new(row.site.block.index()),
                        statement: row.site.statement.unwrap() as u32,
                    };
                    let SemanticStatementKindV1::Assign(original) =
                        scoped_source_statement_v29(declaration, site).unwrap()
                    else {
                        panic!("Cast");
                    };
                    assert!(matches!(
                        original.value().kind(),
                        SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::Pointer,
                            ..
                        }
                    ));
                    assert_eq!(
                        plan.atomic_view_formation_v41(
                            row.site.instance,
                            site,
                            original.value(),
                            budget,
                        )?,
                        Some(*row)
                    );
                    assert_eq!(plan.loans[0].effects.address_observations, 0);
                    assert_eq!(plan.loans[0].effects.referent_writes, 0);
                    let scalar = if matches!(kind, SignedMinimum | SignedMaximum) {
                        ScalarType::I32
                    } else {
                        ScalarType::U32
                    };
                    let mut atomics = 0;
                    for (_, function) in &emitted {
                        for operation in function
                            .function
                            .body
                            .as_ref()
                            .unwrap()
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                        {
                            match &operation.kind {
                                OperationKind::Atomic(value) => {
                                    atomics += 1;
                                    assert_eq!(
                                        value.kind,
                                        lower_atomic_rmw_kind(kind, scalar).unwrap()
                                    );
                                    assert_eq!(value.ordering, lower_atomic_ordering(order));
                                    assert_eq!(value.scope, SynchronizationScope::System);
                                }
                                OperationKind::Load { .. } | OperationKind::Store { .. } => {
                                    panic!("ordinary access never inherits atomic permission")
                                }
                                _ => {}
                            }
                        }
                    }
                    assert_eq!(atomics, 1);
                    emission.finish_source_claims_v29(budget)?;
                    reached.set(true);
                    Ok(())
                },
            )
            .unwrap_or_else(|error| panic!("{kind:?}/{order:?}: {error:?}"));
            assert!(reached.get());
        }
    }
}

#[test]
fn atomic_descendant_v41_original_foreign_reverse_lifetime_and_ordinary_accesses_refuse() {
    for fault in [
        Fault41::ForeignCast,
        Fault41::ReverseCast,
        Fault41::ByValue,
        Fault41::DeadRoot,
        Fault41::OrdinaryLoad,
        Fault41::OrdinaryStore,
        Fault41::MutableBorrow,
        Fault41::Offset,
        Fault41::Expose,
        Fault41::BaseAliasRead,
        Fault41::PreViewCopy,
        Fault41::PreViewMove,
    ] {
        let entered = std::cell::Cell::new(false);
        let result = cells_tests::run_cells(
            descendant_owner_v41(
                fault,
                SemanticAtomicRmwOpV1::Add,
                SemanticAtomicOrderingV1::Relaxed,
                true,
            ),
            |_, _| {
                entered.set(true);
                Ok(())
            },
        );
        assert!(result.is_err(), "{fault:?}");
        assert!(!entered.get(), "{fault:?}");
    }
}

#[test]
fn atomic_descendant_v41_complete_root_accepts_original_ten_kinds_and_five_orders() {
    use SemanticAtomicOrderingV1::*;
    use SemanticAtomicRmwOpV1::*;
    for kind in [
        Exchange,
        Add,
        Subtract,
        BitAnd,
        BitOr,
        BitXor,
        UnsignedMinimum,
        UnsignedMaximum,
        SignedMinimum,
        SignedMaximum,
    ] {
        for order in [
            Relaxed,
            Release,
            Acquire,
            AcquireRelease,
            SequentiallyConsistent,
        ] {
            let (result, observed) = with_final_atomic_fault_v41(0, || {
                prepare_final_atomic_owner_v41(
                    descendant_owner_v41(Fault41::None, kind, order, false),
                    kind,
                    order,
                )
            });
            assert!(
                observed,
                "original full root not reached: {kind:?}/{order:?}: {result:?}"
            );
            result.unwrap_or_else(|error| panic!("{kind:?}/{order:?}: {error:?}"));
        }
    }
}

#[test]
fn atomic_descendant_v41_full_root_relocation_payload_and_original_source_mutations_refuse() {
    for fault in 1..=14 {
        let (result, observed) = with_final_atomic_fault_v41(fault, || {
            prepare_final_atomic_owner_v41(
                descendant_owner_v41(
                    Fault41::None,
                    SemanticAtomicRmwOpV1::Add,
                    SemanticAtomicOrderingV1::Relaxed,
                    false,
                ),
                SemanticAtomicRmwOpV1::Add,
                SemanticAtomicOrderingV1::Relaxed,
            )
        });
        assert!(observed, "hostile mutation {fault} not reached: {result:?}");
        assert!(result.is_err(), "hostile mutation {fault} was accepted");
    }
}
