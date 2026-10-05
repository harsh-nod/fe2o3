use super::*;

// origin: 0 exact private allocation, 1 exact global slice, 2 unresolved
// Generic parameter. Every case has one genuine guarded global effect.
fn generic_fixture(read: bool, origin: u8, guarded: bool, volatile: bool) -> Module {
    let mut module = fixture(Axis::X, AccessMode::ReadWrite, read);
    let function = &mut module.functions[0];
    let body = function.body.as_mut().unwrap();
    if read {
        body.blocks[1].operations.remove(2);
    }
    let scalar = Type::Scalar(ScalarType::U32);
    let generic = Type::pointer(scalar.clone(), AddressSpace::Generic, AccessMode::ReadWrite);
    match origin {
        0 => {
            body.blocks[0].operations.extend([
                op(
                    20,
                    Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite),
                    OperationKind::Alloca {
                        element: scalar.clone(),
                        count: None,
                        address_space: AddressSpace::Private,
                        alignment: 4,
                    },
                ),
                Operation::new(
                    vec![],
                    OperationKind::Store {
                        pointer: ValueId(20),
                        value: ValueId(1),
                        access: MemoryAccess::new(AddressSpace::Private, 4),
                    },
                ),
                op(
                    21,
                    generic.clone(),
                    OperationKind::Cast {
                        kind: crate::CastKind::PointerToGeneric,
                        value: ValueId(20),
                        to: generic,
                    },
                ),
            ]);
        }
        1 => body.blocks[1].operations.push(op(
            21,
            generic.clone(),
            OperationKind::Cast {
                kind: crate::CastKind::PointerToGeneric,
                value: ValueId(6),
                to: generic,
            },
        )),
        2 => {
            function.signature.parameters.push(generic);
            body.parameters.push(ValueId(21));
        }
        _ => unreachable!(),
    }
    let mut access = MemoryAccess::new(AddressSpace::Generic, 4);
    access.volatile = volatile;
    body.blocks[1].operations.push(if read {
        op(
            22,
            scalar,
            if guarded {
                OperationKind::GuardedLoad {
                    pointer: ValueId(21),
                    predicate: ValueId(4),
                    fallback: ValueId(1),
                    access,
                }
            } else {
                OperationKind::Load {
                    pointer: ValueId(21),
                    access,
                }
            },
        )
    } else {
        Operation::new(
            vec![],
            if guarded {
                OperationKind::GuardedStore {
                    pointer: ValueId(21),
                    predicate: ValueId(4),
                    value: ValueId(1),
                    access,
                }
            } else {
                OperationKind::Store {
                    pointer: ValueId(21),
                    value: ValueId(1),
                    access,
                }
            },
        )
    });
    module
}

#[test]
fn opposite_profile_generic_effects_require_exact_pointer_origin() {
    for read in [false, true] {
        for origin in 0..3 {
            for (guarded, volatile) in [(false, false), (true, false), (false, true)] {
                let (graph, credit) = owner(&generic_fixture(read, origin, guarded, volatile));
                run(&graph, credit, |stores, budget| {
                    with_canonical_guarded_global_reads_v18(
                        &graph,
                        Default::default(),
                        budget,
                        |reads, budget| {
                            let count = if origin == 0 { 1 } else { 2 };
                            let expected_reads = if read { (count, 0, 0) } else { (0, count, 0) };
                            let expected_stores = if read { (0, count, 0) } else { (count, 0, 0) };
                            assert_eq!(
                                reads.function_effects(FunctionCoordinate(0), budget)?,
                                expected_reads,
                                "read={read} origin={origin} guarded={guarded} volatile={volatile}"
                            );
                            assert_eq!(
                                stores.function_effects(FunctionCoordinate(0), budget)?,
                                expected_stores,
                                "read={read} origin={origin} guarded={guarded} volatile={volatile}"
                            );
                            Ok(())
                        },
                    )
                })
                .unwrap();
            }
        }
    }
}

fn measure_generic_batch(
    read: bool,
    origin: u8,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<Option<usize>>, usize, usize) {
    let (graph, credit) = owner(&generic_fixture(read, origin, false, false));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(&mut work, storage_limit);
    budget.reserve_storage(credit + 17).unwrap();
    let result = with_canonical_guarded_global_reads_v18(
        &graph,
        Default::default(),
        &mut budget,
        |reads, budget| {
            with_canonical_guarded_global_stores_v24(
                &graph,
                Default::default(),
                budget,
                |stores, budget| {
                    crate::with_canonical_conditional_slice_domains_v26(
                        reads,
                        stores,
                        &[ExplicitLaunchExtent::Exact {
                            rank: 3,
                            extents: [64, 1, 1],
                        }],
                        FormalIndexWidth::Bits64,
                        budget,
                        |batch, budget| {
                            assert!(!batch.runtime_requirements_are_discharged());
                            batch.access_count(budget)
                        },
                    )
                },
            )
        },
    );
    assert_eq!(budget.storage(), credit + 17);
    (result, budget.work(), budget.peak_storage())
}

#[test]
fn mixed_read_only_and_store_only_batches_leave_proven_private_effects_unclaimed() {
    for read in [false, true] {
        assert_eq!(
            measure_generic_batch(read, 0, 100_000_000, 100_000_000)
                .0
                .unwrap(),
            Some(1)
        );
        assert_eq!(
            measure_generic_batch(read, 2, 100_000_000, 100_000_000)
                .0
                .unwrap(),
            None
        );
    }
}

#[test]
fn opposite_profile_provenance_work_and_storage_are_exactly_budgeted() {
    for read in [false, true] {
        let (result, work, peak) = measure_generic_batch(read, 0, 100_000_000, 100_000_000);
        assert_eq!(result.unwrap(), Some(1));
        let (result, exact_work, exact_peak) = measure_generic_batch(read, 0, work, peak);
        assert_eq!(result.unwrap(), Some(1));
        assert_eq!((exact_work, exact_peak), (work, peak));
        assert!(matches!(
            measure_generic_batch(read, 0, work - 1, peak).0,
            Err(Failure::Resource(ResourceError::Work(_)))
        ));
        assert!(matches!(
            measure_generic_batch(read, 0, work, peak - 1).0,
            Err(Failure::Resource(ResourceError::Storage { .. }))
        ));
    }
}
