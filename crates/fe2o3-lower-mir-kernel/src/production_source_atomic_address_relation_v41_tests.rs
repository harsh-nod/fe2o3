use super::*;

thread_local! {
    static ATOMIC_FINAL_FAULT_V41: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static ATOMIC_FINAL_VISITED_V41: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn final_atomic_observer_v41(
    pending: &mut PendingScopedRootEmissionV29,
    _: &ExecutionInstancesV29<'_>,
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_eq!(plan.atomic_uses.len(), 1);
    let fault = ATOMIC_FINAL_FAULT_V41.get();
    let body = pending.function.body.as_mut().unwrap();
    let position = body
        .blocks
        .iter()
        .enumerate()
        .find_map(|(b, block)| {
            block
                .operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::Atomic(_)))
                .map(|o| (b, o))
        })
        .expect("the original emitter reached its actual Atomic");
    let mut changed = false;
    match fault {
        0 => {}
        1..=5 | 7 | 8 => {
            let operation = &mut body.blocks[position.0].operations[position.1];
            let OperationKind::Atomic(atomic) = &mut operation.kind else {
                unreachable!()
            };
            match fault {
                1 => atomic.kind = AtomicKind::Subtract,
                2 => atomic.ordering = MemoryOrdering::SequentiallyConsistent,
                3 => atomic.scope = SynchronizationScope::Device,
                4 => atomic.access.alignment = 1,
                5 => atomic.access.volatile = true,
                7 => operation.results[0].id = ValueId(u32::MAX),
                8 => atomic.pointer = ValueId(u32::MAX),
                _ => unreachable!(),
            }
            changed = true;
        }
        6 => {
            let OperationKind::Atomic(atomic) =
                &body.blocks[position.0].operations[position.1].kind
            else {
                unreachable!()
            };
            let rhs = atomic.value.unwrap();
            let operation = body.blocks.iter_mut().flat_map(|block| &mut block.operations)
                .find(|operation| matches!(operation.results.as_slice(), [result] if result.id == rhs))
                .unwrap();
            let OperationKind::Constant(Constant::U32(value)) = &mut operation.kind else {
                panic!("the original fixture RHS is a genuine u32 constant");
            };
            *value = value.wrapping_add(1);
            changed = true;
        }
        9..=13 => {
            let (sidecar_index, anchor_index) = pending
                .sidecars
                .rows
                .iter()
                .enumerate()
                .find_map(|(s, sidecar)| {
                    sidecar
                        .scoped_memory_anchors
                        .as_ref()
                        .unwrap()
                        .rows
                        .iter()
                        .position(|row| {
                            matches!(
                                row.kind,
                                ScopedMemoryAnchorKindV29::Access {
                                    payload: Some(ScopedMemoryPayloadV29::AtomicRmw { .. }),
                                    ..
                                }
                            )
                        })
                        .map(|a| (s, a))
                })
                .unwrap();
            let anchors = &mut pending.sidecars.rows[sidecar_index]
                .scoped_memory_anchors
                .as_mut()
                .unwrap()
                .rows;
            match fault {
                9 => {
                    let ScopedMemoryAnchorKindV29::Access {
                        payload: Some(ScopedMemoryPayloadV29::AtomicRmw { source, .. }),
                        ..
                    } = &mut anchors[anchor_index].kind
                    else {
                        unreachable!()
                    };
                    let ScopedMemoryStoreSourceV29::Operand { role, .. } = source else {
                        unreachable!()
                    };
                    *role = ExecutionOperandV29::StoreValue;
                }
                10 => anchors[anchor_index].position += 1,
                11 => {
                    anchors.remove(anchor_index);
                }
                12 => {
                    let duplicate = anchors[anchor_index];
                    emission_push_v1(anchors, duplicate, budget)?;
                }
                13 => {
                    let source = anchors[anchor_index].source.as_mut().unwrap();
                    let ExecutionSiteV29::Statement { block, statement } = source.site else {
                        unreachable!()
                    };
                    source.site = ExecutionSiteV29::Statement {
                        block,
                        statement: statement.checked_sub(1).unwrap(),
                    };
                }
                _ => unreachable!(),
            }
            changed = true;
        }
        14 => {
            let cast = body
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.operations)
                .find(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Cast {
                            kind: CastKind::PointerToGeneric,
                            ..
                        }
                    )
                })
                .unwrap();
            let OperationKind::Cast { kind, .. } = &mut cast.kind else {
                unreachable!()
            };
            *kind = CastKind::RestrictPointerAccess;
            changed = true;
        }
        _ => unreachable!(),
    }
    assert_eq!(
        changed,
        fault != 0,
        "the hostile original output mutation must actually occur"
    );
    ATOMIC_FINAL_VISITED_V41.set(true);
    Ok(())
}

fn with_final_atomic_fault_v41<T>(fault: u8, run: impl FnOnce() -> T) -> (T, bool) {
    struct Restore(Option<RootExecutionArchiveObserverV29>, u8, bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.set(self.0);
            ATOMIC_FINAL_FAULT_V41.set(self.1);
            ATOMIC_FINAL_VISITED_V41.set(self.2);
        }
    }
    let _restore = Restore(
        ROOT_EXECUTION_ARCHIVE_OBSERVER_V29.replace(Some(final_atomic_observer_v41)),
        ATOMIC_FINAL_FAULT_V41.replace(fault),
        ATOMIC_FINAL_VISITED_V41.replace(false),
    );
    let result = run();
    (result, ATOMIC_FINAL_VISITED_V41.get())
}

// This goes through real pending source capture and with_original_source_root,
// not the per-function component harness. Its admitted semantic fixture is not
// a replacement for the separate original Rust callback.
fn prepare_final_atomic_v41(
    kind: SemanticAtomicRmwOpV1,
    ordering: SemanticAtomicOrderingV1,
) -> Result<(), crate::ProductionSourceOwnedViewErrorV18> {
    prepare_final_atomic_owner_v41(
        owner41_atomic_uncaptured(Fault41::None, Some((kind, ordering))),
        kind,
        ordering,
    )
}

fn prepare_final_atomic_owner_v41(
    owner: ProductionSemanticSsaOwnerV1,
    kind: SemanticAtomicRmwOpV1,
    ordering: SemanticAtomicOrderingV1,
) -> Result<(), crate::ProductionSourceOwnedViewErrorV18> {
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
    let prepared = crate::ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
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
    )?;
    prepared.with_source_consumer_v18(&mut budget, |source, budget| {
        let original = source.source_semantic(budget)?;
        assert_eq!(
            original.wire_version(),
            fe2o3_mir_model::semantic_mir_v1::SemanticMirWireVersionV1::V41
        );
        assert_eq!(source.root_count(budget)?, 1);
        let module = source.canonical(budget)?.module();
        let atomics: Vec<_> = module
            .functions
            .iter()
            .filter_map(|f| f.body.as_ref())
            .flat_map(|b| &b.blocks)
            .flat_map(|b| &b.operations)
            .filter_map(|op| match &op.kind {
                OperationKind::Atomic(atomic) => Some((op, atomic)),
                _ => None,
            })
            .collect();
        assert_eq!(atomics.len(), 1);
        let scalar = if matches!(
            kind,
            SemanticAtomicRmwOpV1::SignedMinimum | SemanticAtomicRmwOpV1::SignedMaximum
        ) {
            ScalarType::I32
        } else {
            ScalarType::U32
        };
        assert_eq!(
            atomics[0].1.kind,
            lower_atomic_rmw_kind(kind, scalar).unwrap()
        );
        assert_eq!(atomics[0].1.ordering, lower_atomic_ordering(ordering));
        assert_eq!(atomics[0].1.scope, SynchronizationScope::System);
        assert_eq!(
            atomics[0].1.access,
            MemoryAccess::new(AddressSpace::Generic, 4)
        );
        assert_eq!(atomics[0].0.results[0].ty, Type::Scalar(scalar));
        Ok(())
    })
}

#[test]
fn atomic_final_v41_complete_original_root_accepts_ten_kinds_five_orders() {
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
            let (result, observed) =
                with_final_atomic_fault_v41(0, || prepare_final_atomic_v41(kind, order));
            assert!(
                observed,
                "must reach the genuine final root: {kind:?}/{order:?}: {result:?}"
            );
            result.unwrap_or_else(|e| panic!("{kind:?}/{order:?}: {e:?}"));
        }
    }
}

#[test]
fn atomic_final_v41_exact_original_payload_root_and_relocation_mutations_refuse() {
    for fault in 1..=14 {
        let (result, observed) = with_final_atomic_fault_v41(fault, || {
            prepare_final_atomic_v41(
                SemanticAtomicRmwOpV1::Add,
                SemanticAtomicOrderingV1::Relaxed,
            )
        });
        assert!(
            observed,
            "fault {fault} must mutate genuine emitted output: {result:?}"
        );
        assert!(
            result.is_err(),
            "mutated original final root {fault} was accepted"
        );
    }
}

#[path = "production_source_atomic_descendant_cast_v41_tests.rs"]
mod atomic_descendant_cast_v41_tests;
