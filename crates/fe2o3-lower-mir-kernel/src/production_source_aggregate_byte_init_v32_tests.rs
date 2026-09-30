mod aggregate_byte_init_tests_v32 {
    use super::*;

    fn plan() -> AggregateBytePlanV32 {
        let operation = |function| fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(function),
                block: 0,
            },
            operation: 0,
        };
        AggregateBytePlanV32 {
            allocations: vec![operation(0), operation(1)],
            boundaries: vec![
                AggregateByteBoundaryV32 {
                    allocation: 0,
                    offset: 0,
                },
                AggregateByteBoundaryV32 {
                    allocation: 0,
                    offset: 2,
                },
                AggregateByteBoundaryV32 {
                    allocation: 0,
                    offset: 4,
                },
                AggregateByteBoundaryV32 {
                    allocation: 1,
                    offset: 0,
                },
                AggregateByteBoundaryV32 {
                    allocation: 1,
                    offset: 4,
                },
            ],
            allocation_cells: vec![0..2, 3..4],
            leaves: vec![
                AggregateByteLeafV32 {
                    allocation: 0,
                    cells: 0..1,
                },
                AggregateByteLeafV32 {
                    allocation: 0,
                    cells: 1..2,
                },
                AggregateByteLeafV32 {
                    allocation: 0,
                    cells: 0..2,
                },
                AggregateByteLeafV32 {
                    allocation: 1,
                    cells: 3..4,
                },
            ],
        }
    }

    fn flow(rows: Vec<(bool, Vec<AggregateByteEventV32>, Vec<usize>)>) -> AggregateByteFlowV32 {
        let mut flow = AggregateByteFlowV32 {
            blocks: Vec::new(),
            edges: Vec::new(),
            events: Vec::new(),
            allocation_cells: vec![0..2, 3..4],
            leaves: plan().leaves,
        };
        for (entry, events, edges) in rows {
            let operations = flow.events.len()..flow.events.len() + events.len();
            let edge_range = flow.edges.len()..flow.edges.len() + edges.len();
            flow.events.extend(events);
            flow.edges.extend(edges);
            flow.blocks.push(AggregateByteBlockV32 {
                entry,
                operations,
                edges: edge_range,
            });
        }
        flow
    }

    fn probe(
        flow: &AggregateByteFlowV32,
        work_limit: usize,
        storage_limit: usize,
    ) -> (Result<(), ProductionAggregateSourceErrorV30>, usize, usize) {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let result = (|| {
            budget.reserve_storage(aggregate_byte_headers_v32()?)?;
            flow.check(&mut budget)
        })();
        let measured = (budget.work(), budget.peak_storage());
        budget.release_storage(budget.storage() - 17).unwrap();
        assert_eq!(budget.storage(), 17);
        (result, measured.0, measured.1)
    }

    fn succeeds(flow: &AggregateByteFlowV32) {
        probe(flow, 1_000_000, 1_000_000).0.unwrap();
    }

    fn refuses(flow: &AggregateByteFlowV32, detail: &'static str) {
        let result = probe(flow, 1_000_000, 1_000_000).0;
        assert!(
            matches!(result, Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Binding(actual))) if actual == detail)
        );
    }

    #[test]
    fn aggregate_byte_init_interval_order_keeps_full_width_offsets() {
        let offsets = [0, 1, u64::from(u32::MAX), 1u64 << 32, u64::MAX];
        let boundaries = offsets.map(|offset| AggregateByteBoundaryV32 {
            allocation: 3,
            offset,
        });
        for pair in boundaries.windows(2) {
            assert!(pair[0].order() < pair[1].order());
        }
        assert!(
            boundaries.last().unwrap().order()
                < AggregateByteBoundaryV32 {
                    allocation: 4,
                    offset: 0,
                }
                .order()
        );
        let mut shuffled = boundaries;
        shuffled.reverse();
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        private_array_heapsort_v1(
            &mut shuffled,
            |row| row.order(),
            &mut SourceCorrespondenceWorkV18(&mut budget),
            || ArgumentResourceV1::Arithmetic.into(),
        )
        .unwrap();
        assert_eq!(shuffled, boundaries);
    }

    #[test]
    fn aggregate_byte_init_requires_every_byte_not_just_an_overlapping_write() {
        use AggregateByteEventV32::*;
        refuses(
            &flow(vec![(true, vec![Allocate(0), Write(0), Read(2)], vec![])]),
            "private read includes uninitialized bytes",
        );
        succeeds(&flow(vec![(
            true,
            vec![Allocate(0), Write(0), Write(1), Read(2)],
            vec![],
        )]));
        refuses(
            &flow(vec![(true, vec![Allocate(0), Write(1), Read(0)], vec![])]),
            "private read includes uninitialized bytes",
        );
    }

    #[test]
    fn aggregate_byte_init_vector_write_does_not_initialize_alignment_padding() {
        use AggregateByteEventV32::*;
        let vector = fe2o3_kernel_ir::FixedVectorTypeV12::new(
            ScalarType::U16,
            3,
            fe2o3_kernel_ir::VectorLayoutV12::Contiguous,
        );
        let layout = fe2o3_kernel_ir::StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: fe2o3_kernel_ir::StorageLayoutKindV1::Vector(vector),
        };
        let payload =
            aggregate_byte_payload_v32(&layout, AggregateMemoryLeafV31::Vector(vector)).unwrap();
        assert_eq!(payload, 6);
        let mut plan = plan();
        plan.boundaries[1].offset = payload;
        plan.boundaries[2].offset = layout.size;
        let graph = flow(vec![(true, vec![Allocate(0), Write(0), Read(1)], vec![])]);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        budget
            .reserve_storage(aggregate_byte_headers_v32().unwrap())
            .unwrap();
        assert!(matches!(
            graph.check(&mut budget),
            Err(ProductionAggregateSourceErrorV30::Source(
                ProductionSourceOwnedViewErrorV18::Binding(
                    "private read includes uninitialized bytes"
                )
            ))
        ));
        drop((graph, plan));
        budget.release_storage(budget.storage()).unwrap();
        assert_eq!(budget.storage(), 0);
    }

    #[test]
    fn aggregate_byte_init_intersects_all_reachable_parallel_paths() {
        use AggregateByteEventV32::*;
        for missing in [false, true] {
            let graph = flow(vec![
                (true, vec![Allocate(0)], vec![1, 2, 2]),
                (false, vec![Write(2)], vec![3]),
                (
                    false,
                    if missing {
                        vec![]
                    } else {
                        vec![Write(0), Write(1)]
                    },
                    vec![3],
                ),
                (false, vec![Read(2)], vec![]),
            ]);
            if missing {
                refuses(&graph, "private read includes uninitialized bytes");
            } else {
                succeeds(&graph);
            }
        }
    }

    #[test]
    fn aggregate_byte_init_loop_allocation_resets_current_execution_bytes() {
        use AggregateByteEventV32::*;
        let safe = flow(vec![
            (true, vec![Allocate(0), Write(2)], vec![1]),
            (false, vec![Read(2), Allocate(0), Write(2)], vec![1]),
        ]);
        succeeds(&safe);
        let stale = flow(vec![
            (true, vec![Allocate(0), Write(2)], vec![1]),
            (false, vec![Read(2), Allocate(0)], vec![1]),
        ]);
        refuses(&stale, "private read includes uninitialized bytes");
        refuses(
            &flow(vec![(
                true,
                vec![Allocate(0), Write(2), Allocate(0), Read(2)],
                vec![],
            )]),
            "private read includes uninitialized bytes",
        );
    }

    #[test]
    fn aggregate_byte_init_keeps_allocation_function_and_entry_lifetimes_distinct() {
        use AggregateByteEventV32::*;
        refuses(
            &flow(vec![(true, vec![Allocate(0), Write(2), Read(3)], vec![])]),
            "private access precedes dynamic allocation",
        );
        refuses(
            &flow(vec![(true, vec![Project(0)], vec![])]),
            "private projection precedes dynamic allocation",
        );
        refuses(
            &flow(vec![
                (true, vec![Allocate(0), Write(2)], vec![]),
                (true, vec![Read(2)], vec![]),
            ]),
            "private access precedes dynamic allocation",
        );
        succeeds(&flow(vec![
            (true, vec![Allocate(0), Write(2), Read(2)], vec![]),
            (true, vec![Allocate(1), Write(3), Read(3)], vec![]),
        ]));
    }

    #[test]
    fn aggregate_byte_init_unresolved_events_supply_no_initialization() {
        use AggregateByteEventV32::*;
        refuses(
            &flow(vec![(true, vec![Allocate(0), Unresolved, Read(2)], vec![])]),
            "private read includes uninitialized bytes",
        );
        succeeds(&flow(vec![
            (true, vec![], vec![]),
            (false, vec![Read(2)], vec![]),
        ]));
        refuses(
            &flow(vec![
                (true, vec![], vec![1]),
                (false, vec![Read(2)], vec![]),
            ]),
            "private access precedes dynamic allocation",
        );
    }

    #[test]
    fn aggregate_byte_init_exact_and_one_short_resources_restore_floor() {
        use AggregateByteEventV32::*;
        let graph = flow(vec![
            (true, vec![Allocate(0), Write(2)], vec![1]),
            (false, vec![Read(2), Allocate(0), Write(2)], vec![1]),
        ]);
        let (result, work, storage) = probe(&graph, usize::MAX, usize::MAX);
        result.unwrap();
        probe(&graph, work, storage).0.unwrap();
        for (work_limit, storage_limit) in [(work - 1, storage), (work, storage - 1)] {
            assert!(matches!(
                probe(&graph, work_limit, storage_limit).0,
                Err(ProductionAggregateSourceErrorV30::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(_)
                ))
            ));
        }
    }

    fn two_closed_allocation_roots() -> ProductionSemanticSsaOwnerV1 {
        let base = private_entry_phi_owner_v20();
        let source = base.source_semantic();
        // The replacement roots use only the base unit and integer types.
        // The old fixture's address type is outside their exact root closure.
        let mut types = source.types()[..2].to_vec();
        let pair = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([240; 32]),
            SemanticLayoutIdentityV1::from_sha256([240; 32]),
            SemanticTypeLayoutV1::aggregate(
                Some(8),
                4,
                SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, U32]).unwrap()),
        ));
        let field = |index| {
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(2),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), U32).unwrap(),
                ],
                U32,
            )
            .unwrap()
        };
        let functions = source.functions()[..2]
            .iter()
            .enumerate()
            .map(|(root, old)| {
                let tag = 180 + 10 * root as u8;
                function(
                    tag,
                    SemanticFunctionRoleV1::KernelRoot,
                    old.abi().clone(),
                    vec![
                        local(tag + 1, UNIT, SemanticLocalRoleV1::Return),
                        local(tag + 2, U32, SemanticLocalRoleV1::Argument(0)),
                        local(tag + 3, pair, SemanticLocalRoleV1::Temporary),
                        local(tag + 4, U32, SemanticLocalRoleV1::Temporary),
                    ],
                    vec![block(
                        tag + 5,
                        vec![
                            assign(
                                place(2, pair),
                                SemanticRvalueKindV1::Aggregate(
                                    SemanticAggregateRvalueV1::new(
                                        SemanticAggregateKindV1::Tuple,
                                        vec![SemanticOperandV1::Copy(place(1, U32)), literal(17)],
                                    )
                                    .unwrap(),
                                ),
                            ),
                            // Explicit source memory keeps the original object retained
                            // without introducing an escaping address or pointer alias.
                            SemanticStatementV1::new(
                                SemanticSourceProvenanceV1::unavailable(),
                                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                                    field(1),
                                    literal(23),
                                    SemanticVolatilityV1::NonVolatile,
                                    None,
                                )),
                            ),
                            assign(
                                place(3, U32),
                                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                                    field(0),
                                    SemanticVolatilityV1::NonVolatile,
                                    None,
                                )),
                            ),
                            assign(
                                place(0, UNIT),
                                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                                    SemanticConstantV1::new(
                                        UNIT,
                                        SemanticConstantValueV1::ZeroSized,
                                    ),
                                )),
                            ),
                        ],
                        SemanticTerminatorKindV1::Return,
                    )],
                )
                .with_kernel_entry(old.kernel_entry().unwrap().clone())
            })
            .collect();
        let admitted = InertSemanticMirRequestV1::new_with_callables(
            source.target(),
            types,
            vec![],
            vec![],
            vec![],
            functions,
            vec![
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
                SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(1)),
            ],
            source.roots().to_vec(),
        )
        .unwrap()
        .admit_exact_v29(SemanticMirLimitsV1::default())
        .unwrap();
        ProductionSemanticSsaOwnerV1::try_new(
            ProductionSemanticMirOwnerV1::try_new(
                admitted,
                ProductionSemanticMirLimitsV1::default(),
            )
            .unwrap(),
            ProductionSemanticSsaLimitsV1::default(),
        )
        .unwrap()
    }

    #[test]
    fn aggregate_byte_init_real_chain_uses_exact_function_local_domains() {
        with_aggregate_memory_chain_v31(
            two_closed_allocation_roots,
            |source, _, memory, budget| {
                let floor = budget.storage();
                let result = (|| {
                    budget.reserve_storage(aggregate_byte_headers_v32()?)?;
                    let original = source.canonical(budget)?;
                    let (inventory, receipt) =
                        AggregateByteInventoryV32::derive_v18(original, budget)
                            .map_err(ProductionAggregateSourceErrorV30::Inventory)?;
                    budget.reserve_storage(receipt.retained_storage())?;
                    let plan = AggregateBytePlanV32::build(memory, original, budget)?;
                    let allocations = &memory.allocations[memory.endpoints[0].allocations.clone()];
                    let mut cursor = 0;
                    let mut populated_functions = 0;
                    for function in inventory.functions() {
                        let local = aggregate_byte_function_allocations_v32(
                            allocations,
                            &mut cursor,
                            function.coordinate,
                            budget,
                        )?;
                        let local_floor = budget.storage();
                        let flow = AggregateByteFlowV32::derive(
                            &inventory, function, local, memory, 0, &plan, budget,
                        )?;
                        assert_eq!(flow.blocks.len(), function.blocks.len());
                        assert_eq!(flow.edges.len(), function.edges.len());
                        assert_eq!(flow.events.len(), function.operations.len());
                        let expected: Vec<_> = local
                            .iter()
                            .filter(|row| row.closed_uses == Some(true))
                            .collect();
                        assert_eq!(flow.allocation_cells.len(), expected.len());
                        let cells: usize = expected
                            .iter()
                            .map(|row| {
                                let global = plan
                                    .allocations
                                    .iter()
                                    .position(|coordinate| *coordinate == row.original)
                                    .unwrap();
                                plan.allocation_cells[global].len()
                            })
                            .sum();
                        assert_eq!(
                            flow.allocation_cells.last().map_or(0, |range| range.end),
                            cells
                        );
                        assert!(
                            flow.allocation_cells
                                .windows(2)
                                .all(|pair| pair[0].end == pair[1].start)
                        );
                        assert!(
                            flow.leaves
                                .iter()
                                .all(|leaf| leaf.allocation < expected.len())
                        );
                        if !expected.is_empty() {
                            populated_functions += 1;
                            assert!(expected.len() < plan.allocations.len());
                        }
                        flow.check(budget)?;
                        drop(flow);
                        budget.release_storage(budget.storage() - local_floor)?;
                    }
                    assert_eq!(cursor, allocations.len());
                    assert!(populated_functions >= 2);
                    Ok(())
                })();
                budget.release_storage(budget.storage() - floor)?;
                result
            },
        )
        .unwrap();
    }

    fn independent_functions_probe(
        count: usize,
        work_limit: usize,
        storage_limit: usize,
    ) -> (Result<(), ProductionAggregateSourceErrorV30>, usize, usize) {
        use AggregateByteEventV32::*;
        let bindings: Vec<_> = (0..count)
            .map(|function| {
                let original = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                    block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                        function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(
                            function as u32,
                        ),
                        block: 0,
                    },
                    operation: 0,
                };
                AggregateMemoryAllocationBindingV31 {
                    actual: original,
                    original,
                    closed_uses: Some(true),
                }
            })
            .collect();
        let graph = AggregateByteFlowV32 {
            blocks: vec![AggregateByteBlockV32 {
                entry: true,
                operations: 0..3,
                edges: 0..0,
            }],
            edges: vec![],
            events: vec![Allocate(0), Write(0), Read(0)],
            allocation_cells: vec![0..1],
            leaves: vec![AggregateByteLeafV32 {
                allocation: 0,
                cells: 0..1,
            }],
        };
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(17).unwrap();
        let mut cursor = 0;
        let result = (|| {
            for function in 0..count {
                let coordinate = fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(function as u32);
                let local = aggregate_byte_function_allocations_v32(
                    &bindings,
                    &mut cursor,
                    coordinate,
                    &mut budget,
                )?;
                assert_eq!(local.len(), 1);
                assert_eq!(local[0].actual.block.function, coordinate);
                graph.check(&mut budget)?;
                budget.release_storage(budget.storage() - 17)?;
            }
            assert_eq!(cursor, count);
            Ok(())
        })();
        let measured = (budget.work(), budget.peak_storage());
        budget.release_storage(budget.storage() - 17).unwrap();
        assert_eq!(budget.storage(), 17);
        (result, measured.0, measured.1)
    }

    #[test]
    fn aggregate_byte_init_independent_functions_have_linear_work_and_constant_check_scratch() {
        // These are independently enumerated checker/sweep costs. Fixture owners
        // and the outer fixed frame are outside this dynamic-scratch contract.
        let vector_headers = size_of::<Vec<bool>>()
            + 2 * size_of::<Result<Vec<bool>, ProductionSemanticKirErrorV1>>();
        // State, transfer scratch and reached: 2 + 2 + 1 bytes. Queue: two
        // scalar indices, two vector headers, one usize row and one bool mark.
        let bytes = 3 * vector_headers
            + 5
            + 2 * size_of::<usize>()
            + size_of::<Vec<usize>>()
            + size_of::<usize>()
            + size_of::<Vec<bool>>()
            + 1;
        // Three 3-work vectors; queue 2 + 6 + 6 + 2 initialization; 5 state
        // writes; entry 1 + 2 + 5; two pops plus copy + transfer; final check.
        let check_work = 9 + 16 + 5 + 8 + (4 + 2 + 9 + 4) + (1 + 2 + 9);
        assert_eq!(check_work, 69);
        for count in [1, 2, 8, 64] {
            // Per function: two setup visits, its allocation, and the next
            // allocation's stopping comparison, except after the final row.
            let sweep_work = 6 * count - 2;
            let expected_work = count * check_work + sweep_work;
            let expected_storage = 17 + bytes;
            let (result, spent, peak) =
                independent_functions_probe(count, expected_work, expected_storage);
            result.unwrap();
            assert_eq!((spent, peak), (expected_work, expected_storage));
            assert!(matches!(
                independent_functions_probe(count, expected_work - 1, expected_storage).0,
                Err(ProductionAggregateSourceErrorV30::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Work { .. })
                ))
            ));
            assert!(matches!(
                independent_functions_probe(count, expected_work, expected_storage - 1).0,
                Err(ProductionAggregateSourceErrorV30::Source(
                    ProductionSourceOwnedViewErrorV18::Resource(ArgumentResourceV1::Storage { .. })
                ))
            ));
        }
    }
}
