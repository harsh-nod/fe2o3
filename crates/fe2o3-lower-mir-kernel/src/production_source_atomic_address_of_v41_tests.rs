use super::*;

fn address_owner_v41(
    fault: Fault41,
    mutability: SemanticMutabilityV1,
    atomic: Option<(SemanticAtomicRmwOpV1, SemanticAtomicOrderingV1)>,
) -> ProductionSemanticSsaOwnerV1 {
    let mut owner = owner41_atomic_formation_uncaptured(fault, atomic, Some(mutability));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    owner
        .try_capture_occurrences_with_budget_v1(&mut ArgumentBudgetV1::new(&mut work, usize::MAX))
        .unwrap();
    owner
}

#[test]
fn atomic_address_of_v41_original_mutable_and_const_formations_preserve_custody() {
    for mutability in [
        SemanticMutabilityV1::Mutable,
        SemanticMutabilityV1::Immutable,
    ] {
        cells_tests::run_cells(
            address_owner_v41(Fault41::None, mutability, None),
            |plan, budget| {
                let rows: Vec<_> = plan
                    .atomic_formations
                    .iter()
                    .filter(|row| row.operation == SourceAtomicViewOperationV41::AddressOf)
                    .collect();
                assert_eq!(rows.len(), 1);
                let row = rows[0];
                assert_eq!(plan.atomic_formations.len(), 5);
                let input = plan.atomic_custody[row.input];
                let output = plan.atomic_custody[row.output];
                assert_eq!(input.parent, output.parent);
                assert_eq!(input.anchor, output.anchor);
                assert_eq!(input.generation, output.generation);
                assert_eq!(input.view.unwrap().depth, 0);
                assert_eq!(output.view.unwrap().depth, 1);
                assert_eq!(output.view.unwrap().current_type(), CELL41);
                assert_eq!(plan.loans[output.parent].effects.address_observations, 0);
                assert_eq!(plan.loans[output.parent].effects.referent_writes, 0);
                assert_eq!(plan.loans[output.parent].effects.payload_writes, 0);
                assert!(plan.raw_origins.is_empty());
                let function = plan
                    .instances
                    .instance(row.site.instance)
                    .unwrap()
                    .declaration();
                let site = ExecutionSiteV29::Statement {
                    block: SsaBlockIdV1::new(row.site.block.index()),
                    statement: row.site.statement.unwrap() as u32,
                };
                let SemanticStatementKindV1::Assign(assignment) =
                    scoped_source_statement_v29(function, site).unwrap()
                else {
                    panic!("assignment");
                };
                assert_eq!(
                    plan.atomic_view_formation_v41(
                        row.site.instance,
                        site,
                        assignment.value(),
                        budget,
                    )?,
                    Some(*row)
                );
                Ok(())
            },
        )
        .unwrap();
    }
}

#[test]
fn atomic_address_of_v41_foreign_source_and_wrong_site_do_not_consume_receipts() {
    for wrong_site in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result = cells_tests::run_cells(
            address_owner_v41(Fault41::None, SemanticMutabilityV1::Mutable, None),
            |plan, budget| {
                reached.set(true);
                let row = plan
                    .atomic_formations
                    .iter()
                    .find(|row| row.operation == SourceAtomicViewOperationV41::AddressOf)
                    .unwrap();
                let function = plan
                    .instances
                    .instance(row.site.instance)
                    .unwrap()
                    .declaration();
                let site = ExecutionSiteV29::Statement {
                    block: SsaBlockIdV1::new(row.site.block.index()),
                    statement: row.site.statement.unwrap() as u32,
                };
                let SemanticStatementKindV1::Assign(assignment) =
                    scoped_source_statement_v29(function, site).unwrap()
                else {
                    panic!("assignment");
                };
                let copy = assignment.value().clone();
                let query_site = if wrong_site {
                    ExecutionSiteV29::Statement {
                        block: SsaBlockIdV1::new(row.site.block.index()),
                        statement: (row.site.statement.unwrap() as u32) + 1,
                    }
                } else {
                    site
                };
                let denied = plan.atomic_view_formation_v41(
                    row.site.instance,
                    query_site,
                    if wrong_site {
                        assignment.value()
                    } else {
                        &copy
                    },
                    budget,
                );
                assert!(denied.is_err());
                denied.map(|_| ())
            },
        );
        assert!(reached.get());
        assert!(result.is_err());
    }
}

#[test]
fn atomic_address_of_v41_does_not_grant_ordinary_access_or_relax_owner_lifetimes() {
    for fault in [
        Fault41::ByValue,
        Fault41::DeadRoot,
        Fault41::OrdinaryLoad,
        Fault41::OrdinaryStore,
        Fault41::MutableBorrow,
        Fault41::ForeignCast,
        Fault41::ReverseCast,
        Fault41::Offset,
        Fault41::Expose,
        Fault41::BaseAliasRead,
        Fault41::PreViewCopy,
        Fault41::PreViewMove,
    ] {
        assert!(
            cells_tests::run_cells(
                address_owner_v41(fault, SemanticMutabilityV1::Mutable, None),
                |_, _| Ok(()),
            )
            .is_err(),
            "{fault:?}"
        );
    }
}

#[test]
fn atomic_address_of_v41_original_emission_claims_every_formation_and_atomic() {
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
                address_owner_v41(
                    Fault41::None,
                    SemanticMutabilityV1::Mutable,
                    Some((kind, order)),
                ),
                |plan, emission, emitted, budget| {
                    assert_eq!(plan.atomic_formations.len(), 5);
                    assert_eq!(plan.atomic_uses.len(), 1);
                    assert_eq!(emission.atomic_receipts.len(), 7);
                    assert!(
                        emission
                            .atomic_receipts
                            .iter()
                            .all(|row| row.get().is_some())
                    );
                    let mut count = 0;
                    let scalar = if matches!(kind, SignedMinimum | SignedMaximum) {
                        ScalarType::I32
                    } else {
                        ScalarType::U32
                    };
                    for (_, row) in &emitted {
                        for operation in row
                            .function
                            .body
                            .as_ref()
                            .unwrap()
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                        {
                            match &operation.kind {
                                OperationKind::Atomic(atomic) => {
                                    count += 1;
                                    assert_eq!(
                                        atomic.kind,
                                        lower_atomic_rmw_kind(kind, scalar).unwrap()
                                    );
                                    assert_eq!(atomic.ordering, lower_atomic_ordering(order));
                                    assert_eq!(atomic.scope, SynchronizationScope::System);
                                }
                                OperationKind::Load { .. } | OperationKind::Store { .. } => {
                                    panic!("ordinary memory operation is not atomic permission")
                                }
                                _ => {}
                            }
                        }
                    }
                    assert_eq!(count, 1);
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
fn atomic_address_of_v41_complete_pending_root_uses_original_formation_and_final_graph() {
    for mutability in [
        SemanticMutabilityV1::Mutable,
        SemanticMutabilityV1::Immutable,
    ] {
        let owner = owner41_atomic_formation_uncaptured(
            Fault41::None,
            Some((
                SemanticAtomicRmwOpV1::Add,
                SemanticAtomicOrderingV1::Relaxed,
            )),
            Some(mutability),
        );
        assert!(owner.occurrence_storage().is_none());
        let semantic = owner.source_semantic();
        let root = semantic.roots()[0];
        let entry = semantic.functions()[root.index() as usize]
            .kernel_entry()
            .unwrap();
        let launch = crate::ProductionSourceLaunchRosterV1::try_new(
            semantic,
            &[crate::ProductionSourceLaunchRootInputV1::new(
                std::str::from_utf8(entry.export_symbol().as_bytes()).unwrap(),
                *entry.kernel_binding_identity().as_bytes(),
                crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )],
        )
        .unwrap();
        let identity = *owner.source_semantic_sha256();
        let classes =
            vec![crate::ProductionScopeCallableCandidateV29::Ordinary; semantic.callables().len()];
        let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 20_000_000);
        let prepared =
            crate::ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                owner,
                launch,
                crate::ProductionExecutionSourceInputV29 {
                    semantic_sha256: &identity,
                    roots: &[],
                    classes: &classes,
                    events: &[],
                },
                ProductionSemanticKirLimitsV1::default(),
                &mut budget,
            )
            .unwrap();
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| {
                let module = source.canonical(budget)?.module();
                let atomics: Vec<_> = module
                    .functions
                    .iter()
                    .filter_map(|f| f.body.as_ref())
                    .flat_map(|body| &body.blocks)
                    .flat_map(|block| &block.operations)
                    .filter_map(|op| match &op.kind {
                        OperationKind::Atomic(a) => Some(a),
                        _ => None,
                    })
                    .collect();
                assert_eq!(atomics.len(), 1);
                assert_eq!(atomics[0].kind, AtomicKind::Add);
                assert_eq!(atomics[0].scope, SynchronizationScope::System);
                assert_eq!(atomics[0].ordering, MemoryOrdering::Relaxed);
                Ok::<(), crate::ProductionSourceOwnedViewErrorV18>(())
            })
            .unwrap();
    }
}
