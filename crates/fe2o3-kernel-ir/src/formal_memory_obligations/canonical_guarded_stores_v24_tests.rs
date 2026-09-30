use super::*;
use crate::{BasicBlock, IntrinsicOperation, Kernel, Signature, StorageLayoutLimitsV1, ValueDef};

include!("canonical_conditional_slice_domains_v26_tests.rs");

#[path = "canonical_generic_effect_census_v26_tests.rs"]
mod generic_effect_census_v26;

#[path = "canonical_trap_effect_census_v26_tests.rs"]
mod trap_effect_census_v26;

fn op(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn fixture(axis: Axis, access: AccessMode, read: bool) -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let pointer = Type::pointer(scalar.clone(), AddressSpace::Global, access);
    let mut entry = BasicBlock::new(BlockId(10));
    entry.operations = vec![
        op(
            2,
            Type::INDEX,
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Global,
                    axis,
                },
                Type::INDEX,
            )),
        ),
        op(
            3,
            Type::INDEX,
            OperationKind::SliceLength { slice: ValueId(0) },
        ),
        op(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: BlockId(20),
        then_arguments: vec![],
        else_target: BlockId(30),
        else_arguments: vec![],
    });
    let mut body = BasicBlock::new(BlockId(20));
    body.operations = vec![
        op(
            5,
            pointer.clone(),
            OperationKind::SliceData { slice: ValueId(0) },
        ),
        op(
            6,
            pointer,
            OperationKind::GetElementPointer {
                base: ValueId(5),
                offset: ValueId(2),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(6),
                value: ValueId(1),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ];
    if read {
        body.operations.push(op(
            7,
            scalar.clone(),
            OperationKind::Load {
                pointer: ValueId(6),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ));
    }
    body.terminator = Some(Terminator::Return { values: vec![] });
    let mut exit = BasicBlock::new(BlockId(30));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("guarded-store-v24");
    module.functions.push(Function::kernel_entry(
        "entry",
        Signature::new(
            vec![
                Type::slice(scalar.clone(), AddressSpace::Global, access),
                scalar,
            ],
            vec![],
        ),
        vec![ValueId(0), ValueId(1)],
        vec![entry, body, exit],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "entry",
        LaunchDomain::D3 {
            x: LaunchExtent::Dynamic,
            y: LaunchExtent::Dynamic,
            z: LaunchExtent::Dynamic,
        },
    ));
    module
}

fn coordinate(operation: u32) -> Coordinate {
    Coordinate {
        block: BlockCoordinate {
            function: FunctionCoordinate(0),
            block: 1,
        },
        operation,
    }
}

fn owner(module: &Module) -> (VerifiedCanonicalKernelIrModuleV18, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = Budget::new(&mut work, 10_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV18::from_module_ref_with_verification_budget_v18(
            module,
            StorageLayoutLimitsV1 {
                rows: 8,
                edges: 16,
                containment_depth: 8,
                object_bytes: 1024,
            },
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, receipt.retained_storage())
}

fn run<T>(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    credit: usize,
    consume: impl for<'s> FnOnce(
        &CheckedCanonicalGuardedGlobalStoresV24<'s, '_>,
        &mut Budget<'_>,
    ) -> Result<T>,
) -> Result<T> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
    let mut budget = Budget::new(&mut work, 100_000_000);
    budget.reserve_storage(credit + 17).unwrap();
    let result =
        with_canonical_guarded_global_stores_v24(owner, Default::default(), &mut budget, consume);
    assert_eq!(budget.storage(), credit + 17);
    result
}

#[test]
fn store_bounds_and_conditional_injectivity_keep_exact_owner_axis_and_domain() {
    for axis in [Axis::X, Axis::Y, Axis::Z] {
        for access in [AccessMode::ReadWrite, AccessMode::WriteOnly] {
            let (graph, credit) = owner(&fixture(axis, access, false));
            run(&graph, credit, |view, budget| {
                assert!(std::ptr::eq(view.owner(budget)?, &graph));
                assert_eq!(view.function_count(budget)?, 1);
                assert_eq!(
                    view.function_effects(FunctionCoordinate(0), budget)?,
                    (1, 0, 0)
                );
                let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
                    view.store_at(coordinate(2), budget)?
                else {
                    panic!("ordinary bounded Store");
                };
                assert!(std::ptr::eq(store.owner(), &graph));
                assert_eq!(store.operation(), coordinate(2));
                assert_eq!(store.domain().allocation().parameter_index(), 0);
                assert_eq!(store.domain().slice(), ValueId(0));
                assert_eq!(store.domain().pointer(), ValueId(6));
                assert_eq!(store.domain().element_bytes(), 4);
                assert_eq!(store.normalized_length_origin(), ValueId(3));
                assert_eq!(store.comparison_operands(), (ValueId(2), ValueId(3)));
                assert_eq!(
                    store.invocation_projection(budget)?,
                    Some((axis, ValueId(2)))
                );
                let mut extents = [1; 3];
                extents[match axis {
                    Axis::X => 0,
                    Axis::Y => 1,
                    Axis::Z => 2,
                }] = 4096;
                let launch = ExplicitLaunchExtent::Exact { rank: 3, extents };
                let proof = store
                    .distinct_invocations(launch, FormalIndexWidth::Bits64, budget)?
                    .unwrap();
                assert!(std::ptr::eq(proof.store(), &store));
                assert_eq!(proof.launch(), launch);
                assert_eq!(proof.index_width(), FormalIndexWidth::Bits64);
                assert_eq!(proof.projection(), (axis, ValueId(2)));
                assert!(view.true_at(coordinate(2), ValueId(4), budget)?.is_some());
                Ok(())
            })
            .unwrap();
        }
    }
}

#[test]
fn store_projection_rejects_orthogonal_invocations_invalid_launch_and_width_overflow() {
    let (graph, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, false));
    run(&graph, credit, |view, budget| {
        let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
            view.store_at(coordinate(2), budget)?
        else {
            panic!();
        };
        for launch in [
            ExplicitLaunchExtent::Unknown,
            ExplicitLaunchExtent::Exact {
                rank: 0,
                extents: [1; 3],
            },
            ExplicitLaunchExtent::Exact {
                rank: 4,
                extents: [1; 3],
            },
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [0, 1, 1],
            },
            ExplicitLaunchExtent::Exact {
                rank: 1,
                extents: [256, 2, 1],
            },
            ExplicitLaunchExtent::Exact {
                rank: 3,
                extents: [256, 2, 1],
            },
            ExplicitLaunchExtent::Exact {
                rank: 3,
                extents: [256, 1, 2],
            },
        ] {
            assert!(
                store
                    .distinct_invocations(launch, FormalIndexWidth::Bits64, budget)?
                    .is_none()
            );
        }
        let exact = ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [1_u64 << 32, 1, 1],
        };
        assert!(
            store
                .distinct_invocations(exact, FormalIndexWidth::Bits32, budget)?
                .is_some()
        );
        assert!(
            store
                .distinct_invocations(exact, FormalIndexWidth::Unknown, budget)?
                .is_none()
        );
        let short_width = ExplicitLaunchExtent::Exact {
            rank: 1,
            extents: [(1_u64 << 32) + 1, 1, 1],
        };
        assert!(
            store
                .distinct_invocations(short_width, FormalIndexWidth::Bits32, budget)?
                .is_none()
        );
        assert!(
            store
                .distinct_invocations(short_width, FormalIndexWidth::Bits64, budget)?
                .is_some()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn store_bounds_do_not_turn_opaque_indices_into_injective_indices() {
    let mut module = fixture(Axis::X, AccessMode::ReadWrite, false);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind =
        OperationKind::Constant(Constant::Index(0));
    let (graph, credit) = owner(&module);
    run(&graph, credit, |view, budget| {
        let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
            view.store_at(coordinate(2), budget)?
        else {
            panic!("constant index is guarded");
        };
        assert_eq!(store.invocation_projection(budget)?, None);
        assert!(
            store
                .distinct_invocations(
                    ExplicitLaunchExtent::Exact {
                        rank: 1,
                        extents: [256, 1, 1]
                    },
                    FormalIndexWidth::Bits64,
                    budget
                )?
                .is_none()
        );
        Ok(())
    })
    .unwrap();
}

#[test]
fn store_bounds_refuse_volatile_false_edge_and_unrelated_length() {
    for mode in 0..3 {
        let mut module = fixture(Axis::X, AccessMode::ReadWrite, false);
        let body = module.functions[0].body.as_mut().unwrap();
        match mode {
            0 => {
                let OperationKind::Store { access, .. } = &mut body.blocks[1].operations[2].kind
                else {
                    panic!();
                };
                access.volatile = true;
            }
            1 => {
                let Some(Terminator::ConditionalBranch {
                    then_target,
                    else_target,
                    ..
                }) = &mut body.blocks[0].terminator
                else {
                    panic!();
                };
                std::mem::swap(then_target, else_target);
            }
            _ => body.blocks[0].operations[1].kind = OperationKind::Constant(Constant::Index(1024)),
        }
        let (graph, credit) = owner(&module);
        run(&graph, credit, |view, budget| {
            assert!(matches!(
                view.store_at(coordinate(2), budget)?,
                CanonicalGuardedGlobalStoreOutcomeV24::NotProved(_)
            ));
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn store_and_read_views_preserve_separate_effect_rosters_and_same_owner_projection() {
    let (graph, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, true));
    run(&graph, credit, |stores, budget| {
        assert_eq!(
            stores.function_effects(FunctionCoordinate(0), budget)?,
            (1, 1, 0)
        );
        assert!(matches!(
            stores.store_at(coordinate(3), budget)?,
            CanonicalGuardedGlobalStoreOutcomeV24::NotProved(
                CanonicalGuardedGlobalStoreReasonV24::NotOrdinaryGlobalStore
            )
        ));
        with_canonical_guarded_global_reads_v18(
            &graph,
            Default::default(),
            budget,
            |reads, budget| {
                assert_eq!(
                    reads.function_effects(FunctionCoordinate(0), budget)?,
                    (1, 1, 0)
                );
                assert!(matches!(
                    reads.read_at(coordinate(2), budget)?,
                    CanonicalGuardedGlobalReadOutcomeV18::NotProved(_)
                ));
                let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
                    reads.read_at(coordinate(3), budget)?
                else {
                    panic!("read remains a read");
                };
                assert_eq!(
                    stores.read_invocation_projection(&read, budget)?,
                    Some((Axis::X, ValueId(2)))
                );
                Ok(())
            },
        )
    })
    .unwrap();
}

#[test]
fn store_foreign_account_query_is_sticky_and_scope_refunds() {
    let (graph, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, false));
    let error = run(&graph, credit, |view, budget| {
        let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
            view.store_at(coordinate(2), budget)?
        else {
            panic!();
        };
        let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(100_000_000);
        let mut foreign = Budget::new(&mut foreign_work, 100_000_000);
        foreign.reserve_storage(budget.storage())?;
        let first = store.invocation_projection(&mut foreign).unwrap_err();
        assert_eq!(store.invocation_projection(budget).unwrap_err(), first);
        Ok(())
    })
    .unwrap_err();
    assert_eq!(error, Failure::Resource(ResourceError::Accounting));
}
#[test]
fn store_read_projection_rejects_equal_bytes_from_a_different_owner() {
    let module = fixture(Axis::X, AccessMode::ReadWrite, true);
    let (graph, credit) = owner(&module);
    let (other, other_credit) = owner(&module);
    let completed = std::cell::Cell::new(false);
    let error = run(&graph, credit + other_credit, |stores, budget| {
        with_canonical_guarded_global_reads_v18(
            &other,
            Default::default(),
            budget,
            |reads, budget| {
                let CanonicalGuardedGlobalReadOutcomeV18::ProvedLocalConditions(read) =
                    reads.read_at(coordinate(3), budget)?
                else {
                    panic!();
                };
                let before = budget.work();
                assert_eq!(
                    stores
                        .read_invocation_projection(&read, budget)
                        .unwrap_err(),
                    Failure::Resource(ResourceError::Accounting)
                );
                assert_eq!(budget.work() - before, 3);
                let after = budget.work();
                assert!(stores.owner(budget).is_err());
                assert_eq!(budget.work(), after);
                completed.set(true);
                Ok(())
            },
        )
    })
    .unwrap_err();
    assert!(completed.get());
    assert_eq!(error, Failure::Resource(ResourceError::Accounting));
}

#[test]
fn store_projection_lookup_has_independent_exact_and_one_short_work_boundaries() {
    let (graph, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, false));
    for short in [false, true] {
        let limit = 10_000_000;
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = Budget::new(&mut work, 10_000_000);
        budget.reserve_storage(credit + 17).unwrap();
        let completed = std::cell::Cell::new(false);
        let result = with_canonical_guarded_global_stores_v24(
            &graph,
            Default::default(),
            &mut budget,
            |view, budget| {
                let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
                    view.store_at(coordinate(2), budget)?
                else {
                    panic!();
                };
                // One scope entry, one binary-search entry, two two-field comparisons.
                let remaining = 6 - usize::from(short);
                budget.charge_work(limit - budget.work() - remaining)?;
                let before = budget.work();
                let query = store.invocation_projection(budget);
                if short {
                    let first = query.unwrap_err();
                    assert!(matches!(first, Failure::Resource(ResourceError::Work(_))));
                    let after = budget.work();
                    assert_eq!(store.invocation_projection(budget).unwrap_err(), first);
                    assert_eq!(budget.work(), after);
                } else {
                    assert_eq!(query?, Some((Axis::X, ValueId(2))));
                    assert_eq!(budget.work() - before, 6);
                }
                completed.set(true);
                Err::<(), _>(Failure::Coordinate(coordinate(2)))
            },
        );
        assert!(result.is_err());
        assert!(completed.get());
        assert_eq!(budget.storage(), credit + 17);
    }
}

#[test]
fn store_scope_rejects_ignored_coordinate_failure_leaks_and_callback_panics() {
    let (graph, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, false));
    for mode in 0..3 {
        let result = run(&graph, credit, |view, budget| {
            match mode {
                0 => {
                    assert!(view.store_at(coordinate(u32::MAX), budget).is_err());
                }
                1 => budget.reserve_storage(13)?,
                _ => panic!("store scope callback"),
            }
            Ok(())
        });
        assert!(matches!(
            (mode, result),
            (0, Err(Failure::Coordinate(_)))
                | (1, Err(Failure::Resource(ResourceError::Accounting)))
                | (2, Err(Failure::Panicked))
        ));
    }
}

fn measured_store(
    graph: &VerifiedCanonicalKernelIrModuleV18,
    credit: usize,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work);
    let mut budget = Budget::new(&mut work, storage);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_guarded_global_stores_v24(
        graph,
        Default::default(),
        &mut budget,
        |view, budget| {
            let CanonicalGuardedGlobalStoreOutcomeV24::ProvedLocalConditions(store) =
                view.store_at(coordinate(2), budget)?
            else {
                panic!();
            };
            assert!(
                store
                    .distinct_invocations(
                        ExplicitLaunchExtent::Exact {
                            rank: 1,
                            extents: [256, 1, 1]
                        },
                        FormalIndexWidth::Bits64,
                        budget
                    )?
                    .is_some()
            );
            Ok(())
        },
    );
    assert_eq!(budget.storage(), credit + 17);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn store_whole_transaction_measured_exact_and_short_resources_preserve_owner_floor() {
    let (graph, credit) = owner(&fixture(Axis::X, AccessMode::ReadWrite, false));
    let (result, work, peak) = measured_store(&graph, credit, 100_000_000, 100_000_000);
    result.unwrap();
    measured_store(&graph, credit, work, peak).0.unwrap();
    assert!(matches!(
        measured_store(&graph, credit, work - 1, peak).0,
        Err(Failure::Resource(ResourceError::Work(_)))
    ));
    assert!(matches!(
        measured_store(&graph, credit, work, peak - 1).0,
        Err(Failure::Resource(ResourceError::Storage { .. }))
    ));
}
