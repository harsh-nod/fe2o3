#[test]
fn aggregate_memory_key_order_preserves_full_width_offsets_and_coordinates() {
    let key = ProductionAggregateMemoryKeyV31 {
        allocation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
            block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(1),
                block: 2,
            },
            operation: 3,
        },
        layout: fe2o3_kernel_ir::StorageLayoutIdV1(4),
        offset: 0,
        ty: AggregateMemoryLeafV31::Scalar(fe2o3_kernel_ir::ScalarType::U32),
    };
    let offsets = [0, 1, u64::from(u32::MAX), 1u64 << 32, u64::MAX];
    let keys = offsets.map(|offset| ProductionAggregateMemoryKeyV31 { offset, ..key });
    for pair in keys.windows(2) {
        assert!(pair[0].order() < pair[1].order());
    }
    for field in 0..4 {
        let mut next = key;
        match field {
            0 => next.allocation.block.function.0 += 1,
            1 => next.allocation.block.block += 1,
            2 => next.allocation.operation += 1,
            _ => next.layout.0 += 1,
        }
        assert!(keys.last().unwrap().order() < next.order());
    }
}

fn with_aggregate_memory_chain_v31(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    observe: impl FnOnce(
        &ProductionSourceOwnedViewV18<'_>,
        &fe2o3_kernel_opt::OwnedAggregateFixedpointV18,
        &ProductionAggregateMemoryChainV31,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionAggregateSourceErrorV30>,
) -> Result<(), ProductionAggregateSourceErrorV30> {
    with_aggregate_source_owner_v30(factory, |source, abi, budget| {
        let chain = source.aggregate_output_v30(abi, budget)?;
        let floor = budget.storage();
        let result = scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| {
            let headers = aggregate_memory_headers_v31()?;
            budget.reserve_storage(headers)?;
            let mut memory = fold_aggregate_source_stages_v30(
                &chain,
                |stage, prior: Option<ProductionAggregateMemoryChainV31>, budget| {
                    let memory = match prior {
                        Some(value) => value,
                        None => ProductionAggregateMemoryChainV31::seed(stage, budget)?,
                    };
                    memory.advance(stage, budget)
                },
                budget,
            )?;
            let result = source.retain_aggregate_result_v30((|| {
                let output = chain.output(budget)?;
                memory.finish(source, output, budget)?;
                observe(source, output, &memory, budget)
            })());
            let credit = memory.retained_storage()?;
            drop(memory);
            let settled = if source.cleanup.is_denied() {
                Ok(())
            } else {
                budget.release_storage(argument_sum_v1(&[headers, credit])?)
            };
            result?;
            settled?;
            Ok::<_, ProductionAggregateSourceErrorV30>(())
        });
        let settled = chain.discard(budget);
        result?;
        settled?;
        Ok(())
    })
}

#[test]
fn aggregate_memory_chain_binds_every_exact_endpoint_and_promoted_original_slot() {
    with_aggregate_memory_chain_v31(
        private_entry_phi_owner_v20,
        |source, chain, memory, budget| {
            assert_eq!(memory.endpoint_count(), chain.rounds().len() * 2 + 1);
            assert!(memory.key_count() > 0 && memory.promotion_count() > 0);
            assert!(!memory.grants_authority());
            let endpoints = std::iter::once(source.canonical(budget)?).chain(
                chain
                    .rounds()
                    .iter()
                    .flat_map(|round| [round.scalar().owner(), round.aggregate().output()]),
            );
            for (boundary, owner) in endpoints.enumerate() {
                assert!(memory.endpoint_belongs_to(boundary, owner));
                assert!(!memory.endpoint_belongs_to(
                    boundary,
                    if boundary == 0 {
                        chain.owner()
                    } else {
                        source.canonical(budget)?
                    }
                ));
                let mut operation = 0;
                for (function, row) in owner.module().functions.iter().enumerate() {
                    let Some(body) = &row.body else { continue };
                    for (block, row) in body.blocks.iter().enumerate() {
                        for (at, actual) in row.operations.iter().enumerate() {
                            let (coordinate, event) = memory.endpoint_event(boundary, operation).unwrap();
                            assert_eq!((coordinate.block.function.0 as usize, coordinate.block.block as usize, coordinate.operation as usize), (function, block, at));
                            match event {
                                ProductionAggregateMemoryEventV31::Unmodeled => (),
                                ProductionAggregateMemoryEventV31::Allocate { original } => {
                                    assert!(matches!(actual.kind, OperationKind::Alloca { .. }));
                                    assert!(memory.allocations[memory.endpoints[boundary].allocations.clone()].iter().any(|row| row.actual == coordinate && row.original == original));
                                }
                                ProductionAggregateMemoryEventV31::Project { .. } => assert!(matches!(actual.kind, OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::Project { .. }))),
                                ProductionAggregateMemoryEventV31::Read { leaf, output } => {
                                    assert!(matches!(actual.kind, OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::ReadValue { .. })));
                                    assert_eq!(output, actual.results[0].id);
                                    assert!(memory.key_at(leaf).is_some());
                                }
                                ProductionAggregateMemoryEventV31::Write { leaf, value } => {
                                    assert!(matches!(actual.kind, OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::WriteValue { value: actual, .. }) if actual == value));
                                    assert!(memory.key_at(leaf).is_some());
                                }
                            }
                            operation += 1;
                        }
                    }
                }
                assert_eq!(memory.endpoint_event_count(boundary), Some(operation));
                assert_eq!(memory.endpoint_event(boundary, operation), None);
                for leaf in 0..memory.endpoint_leaf_count(boundary).unwrap() {
                    let (global, alignment) = memory.endpoint_leaf(boundary, leaf).unwrap();
                    assert!(alignment.is_power_of_two());
                    let key = memory.key_at(global).unwrap();
                    let original = source.canonical(budget)?.module();
                    let allocation = &original.functions
                        [key.allocation().block.function.0 as usize]
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks[key.allocation().block.block as usize]
                        .operations[key.allocation().operation as usize];
                    assert!(matches!(allocation.kind, OperationKind::Alloca { .. }));
                    assert_eq!(memory.keys[global], key);
                }
            }
            let mut expected = 0;
            for (round, row) in chain.rounds().iter().enumerate() {
                for (slot, selected) in row.aggregate().witness().memory_slots().iter().enumerate()
                {
                    if selected.is_none() {
                        continue;
                    }
                    let (stage, actual_slot, global) = memory.promotion(expected).unwrap();
                    assert_eq!((stage, actual_slot), (round * 2 + 1, slot));
                    let selected = selected.unwrap();
                    let key = memory.key_at(global).unwrap();
                    assert_eq!(
                        (key.layout(), key.offset(), key.ty()),
                        (selected.layout, selected.offset, selected.ty)
                    );
                    expected += 1;
                }
            }
            assert_eq!(memory.promotion_count(), expected);
            Ok(())
        },
    )
    .unwrap();
}

#[test]
fn aggregate_memory_chain_refuses_missing_reordered_foreign_and_changed_allocation_state() {
    for fault in 0..5 {
        let result =
            with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
                let chain = source.aggregate_output_v30(abi, budget)?;
                let floor = budget.storage();
                let result = scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| {
                    let headers = aggregate_memory_headers_v31()?;
                    budget.reserve_storage(headers)?;
                    let mut memory = fold_aggregate_source_stages_v30(
                        &chain,
                        |stage, prior: Option<ProductionAggregateMemoryChainV31>, budget| {
                            let mut memory = match prior {
                                Some(value) => value,
                                None => ProductionAggregateMemoryChainV31::seed(stage, budget)?,
                            };
                            if stage.ordinal == 0 && fault < 3 {
                                match fault {
                                    0 => memory.next_stage = 1,
                                    1 => {
                                        memory.final_owner =
                                            std::ptr::from_ref(stage.output.owner()) as usize
                                    }
                                    _ => {
                                        let at = memory
                                            .current
                                            .iter()
                                            .position(Option::is_some)
                                            .unwrap();
                                        memory.current[at] = None;
                                    }
                                }
                            }
                            memory.advance(stage, budget)
                        },
                        budget,
                    )?;
                    if fault == 3 {
                        memory.endpoints.pop();
                    } else if fault == 4 {
                        memory.endpoints.swap(0, 1);
                    }
                    let result = source.retain_aggregate_result_v30(memory.finish(
                        source,
                        chain.output(budget)?,
                        budget,
                    ));
                    let credit = memory.retained_storage()?;
                    drop(memory);
                    let settled = if source.cleanup.is_denied() {
                        Ok(())
                    } else {
                        budget.release_storage(argument_sum_v1(&[headers, credit])?)
                    };
                    result?;
                    settled?;
                    Ok::<_, ProductionAggregateSourceErrorV30>(())
                });
                let settled = chain.discard(budget);
                result?;
                settled?;
                Ok(())
            });
        let expected = match fault {
            2 => "aggregate memory current allocation binding differs",
            3 => "aggregate memory final chain identity",
            4 => "aggregate memory final endpoint order",
            _ => "aggregate memory stage owner or order",
        };
        assert!(
            matches!(result, Err(ProductionAggregateSourceErrorV30::Source(
            ProductionSourceOwnedViewErrorV18::Binding(actual))) if actual == expected)
        );
    }
}

#[test]
fn aggregate_memory_chain_keeps_unmodeled_events_explicit_and_keys_function_qualified() {
    with_aggregate_memory_chain_v31(private_entry_phi_owner_v20, |_, _, memory, _| {
        assert!(
            memory
                .endpoints
                .iter()
                .any(|endpoint| endpoint.unmodeled_operations != 0)
        );
        for pair in memory.keys.windows(2) {
            assert!(pair[0].order() < pair[1].order());
        }
        assert!(
            memory.keys.iter().any(
                |key| key.allocation.block.function != memory.keys[0].allocation.block.function
            )
        );
        for endpoint in &memory.endpoints {
            for binding in &memory.allocations[endpoint.allocations.clone()] {
                assert_eq!(
                    binding.actual.block.function,
                    binding.original.block.function
                );
            }
        }
        assert!(!memory.grants_authority());
        Ok(())
    })
    .unwrap();
}

#[test]
fn aggregate_memory_chain_unwind_drops_retained_rows_before_scope_refund() {
    struct Tracked<'a> {
        memory: ProductionAggregateMemoryChainV31,
        dropped: &'a std::cell::Cell<usize>,
    }
    impl AggregateStageStateV30 for Tracked<'_> {
        fn retained_storage(&self) -> Result<usize, ArgumentResourceV1> {
            self.memory.retained_storage()
        }
    }
    impl Drop for Tracked<'_> {
        fn drop(&mut self) {
            self.dropped.set(self.dropped.get() + 1);
        }
    }
    with_aggregate_source_owner_v30(private_entry_phi_owner_v20, |source, abi, budget| {
        let chain = source.aggregate_output_v30(abi, budget)?;
        let floor = budget.storage();
        let dropped = std::cell::Cell::new(0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            scoped_source_attempt_v29(source.cleanup, budget, floor, |budget| {
                budget.reserve_storage(aggregate_memory_headers_v31()?)?;
                fold_aggregate_source_stages_v30(
                    &chain,
                    |stage, prior: Option<Tracked<'_>>, budget| {
                        if stage.ordinal == 1 {
                            assert!(prior.is_some());
                            panic!("aggregate memory unwind sentinel");
                        }
                        assert_eq!(stage.ordinal, 0);
                        assert!(prior.is_none());
                        let memory = ProductionAggregateMemoryChainV31::seed(stage, budget)?
                            .advance(stage, budget)?;
                        assert!(memory.events.len() > 0 && memory.allocations.len() > 0);
                        Ok(Tracked {
                            memory,
                            dropped: &dropped,
                        })
                    },
                    budget,
                )
            })
        }));
        let payload = match caught {
            Err(payload) => payload,
            Ok(_) => panic!("memory panic did not propagate"),
        };
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"aggregate memory unwind sentinel")
        );
        assert_eq!(dropped.get(), 1);
        assert_eq!(budget.storage(), floor);
        chain.discard(budget)?;
        Ok(())
    })
    .unwrap();
}

fn aggregate_memory_header_oracle_v31() -> usize {
    use std::mem::{align_of, size_of};
    #[allow(dead_code)]
    struct Allocation {
        actual: AggregateOperationV30,
        original: AggregateOperationV30,
        closed_uses: Option<bool>,
    }
    #[allow(dead_code)]
    struct Event {
        actual: AggregateOperationV30,
        event: fe2o3_kernel_analysis::CanonicalKirAggregateMemoryEventV31,
        allocation: Option<AggregateOperationV30>,
    }
    #[allow(dead_code)]
    struct Endpoint {
        owner: usize,
        allocations: std::ops::Range<usize>,
        leaves: std::ops::Range<usize>,
        events: std::ops::Range<usize>,
        unmodeled_operations: usize,
        nonclosed_allocations: usize,
    }
    #[allow(dead_code)]
    struct Leaf {
        original: ProductionAggregateMemoryKeyV31,
        global: usize,
        alignment: u32,
    }
    #[allow(dead_code)]
    struct Promotion {
        stage: usize,
        witness_slot: usize,
        original: ProductionAggregateMemoryKeyV31,
        global: usize,
    }
    for (actual, fields) in [
        (
            (
                size_of::<AggregateMemoryAllocationBindingV31>(),
                align_of::<AggregateMemoryAllocationBindingV31>(),
            ),
            (size_of::<Allocation>(), align_of::<Allocation>()),
        ),
        (
            (
                size_of::<AggregateMemoryEventBindingV31>(),
                align_of::<AggregateMemoryEventBindingV31>(),
            ),
            (size_of::<Event>(), align_of::<Event>()),
        ),
        (
            (
                size_of::<AggregateMemoryEndpointV31>(),
                align_of::<AggregateMemoryEndpointV31>(),
            ),
            (size_of::<Endpoint>(), align_of::<Endpoint>()),
        ),
        (
            (
                size_of::<AggregateMemoryLeafBindingV31>(),
                align_of::<AggregateMemoryLeafBindingV31>(),
            ),
            (size_of::<Leaf>(), align_of::<Leaf>()),
        ),
        (
            (
                size_of::<AggregateMemoryPromotionV31>(),
                align_of::<AggregateMemoryPromotionV31>(),
            ),
            (size_of::<Promotion>(), align_of::<Promotion>()),
        ),
    ] {
        assert_eq!(actual, fields);
    }
    #[allow(dead_code)]
    struct Owner {
        initial_owner: usize,
        final_owner: usize,
        next_stage: usize,
        endpoints: Vec<AggregateMemoryEndpointV31>,
        allocations: Vec<AggregateMemoryAllocationBindingV31>,
        leaves: Vec<AggregateMemoryLeafBindingV31>,
        events: Vec<AggregateMemoryEventBindingV31>,
        promotions: Vec<AggregateMemoryPromotionV31>,
        keys: Vec<ProductionAggregateMemoryKeyV31>,
        current: Vec<Option<AggregateOperationV30>>,
        finalized: bool,
    }
    assert_eq!(
        size_of::<Owner>(),
        size_of::<ProductionAggregateMemoryChainV31>()
    );
    assert_eq!(
        align_of::<Owner>(),
        align_of::<ProductionAggregateMemoryChainV31>()
    );
    type Frame<'a, 'w> = (
        Owner,
        Result<Owner, ProductionAggregateSourceErrorV30>,
        AggregateMemoryCensusV31<'a, 'a>,
        fe2o3_kernel_analysis::CanonicalKirAggregateMemoryStorageV31,
        Result<
            (
                AggregateMemoryCensusV31<'a, 'a>,
                fe2o3_kernel_analysis::CanonicalKirAggregateMemoryStorageV31,
            ),
            fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18,
        >,
        &'a mut Owner,
        &'a AggregateSourceStageV30<'a>,
        &'a fe2o3_kernel_analysis::CanonicalKirInventoryV18<'a>,
        &'a AggregateMemoryCensusV31<'a, 'a>,
        &'a ProductionSourceOwnedViewV18<'a>,
        &'a mut ArgumentBudgetV1<'w>,
        Vec<Option<AggregateOperationV30>>,
        Vec<bool>,
        [Result<(), ProductionAggregateSourceErrorV30>; 3],
        Result<(), ArgumentResourceV1>,
        [usize; 16],
        [ProductionAggregateMemoryKeyV31; 3],
        Endpoint,
        Allocation,
        Event,
        Option<(AggregateOperationV30, ProductionAggregateMemoryEventV31)>,
        Leaf,
        Promotion,
        Option<fe2o3_kernel_analysis::CanonicalKirAggregateMemoryAllocationV31>,
        Option<fe2o3_kernel_analysis::CanonicalKirAggregateSsaMemorySlotV18>,
        Option<fe2o3_kernel_analysis::CanonicalKirAggregateMemoryEventV31>,
        Result<
            Option<fe2o3_kernel_analysis::CanonicalKirAggregateOperationTransportV30>,
            fe2o3_kernel_analysis::CanonicalKirAggregateSsaErrorV18,
        >,
        SourceOwnedResultV18<ProductionAggregateMemoryKeyV31>,
        SourceOwnedResultV18<&'a Owner>,
    );
    size_of::<Frame<'_, '_>>() + align_of::<Frame<'_, '_>>()
}

#[test]
fn aggregate_memory_chain_header_and_capacity_oracle_are_exact() {
    assert_eq!(
        aggregate_memory_headers_v31().unwrap(),
        aggregate_memory_header_oracle_v31()
    );
    with_aggregate_memory_chain_v31(private_entry_phi_owner_v20, |_, _, memory, _| {
        fn credit<T>(rows: &Vec<T>) -> usize {
            source_reference_emission_headers_v29::<Vec<T>>().unwrap()
                + rows.capacity() * size_of::<T>()
        }
        let expected = credit(&memory.endpoints)
            + credit(&memory.allocations)
            + credit(&memory.leaves)
            + credit(&memory.events)
            + credit(&memory.promotions)
            + credit(&memory.keys)
            + credit(&memory.current);
        assert_eq!(memory.retained_storage()?, expected);
        Ok(())
    })
    .unwrap();
}

include!("production_source_aggregate_byte_init_v32_tests.rs");
include!("production_source_aggregate_memory_visit_v31_tests.rs");
