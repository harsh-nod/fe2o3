use super::*;
use std::mem::size_of;

thread_local! {
    static STORAGE_OBSERVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn probe(
    transport: &ScopedStorageTransportV29,
    map: &ProductionInstanceCorrespondenceV1<'_, '_>,
    child: ProductionCallInstanceIdV1,
    function: &Function,
    expected_refusal: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = with_canonical_call_scratch_v1(budget, |budget| {
        let mut scratch = 0;
        let index = call_splice_index_v1(function, budget, &mut scratch)
            .map_err(source_address_call_error_v29)?;
        let source = ScopedStorageCalleeSourceV29 {
            transport,
            map,
            child,
        };
        let result = source.permit(function, &index, budget, &mut scratch);
        if expected_refusal {
            assert!(
                matches!(result, Err(CallInstanceEmissionErrorV1::StorageTransport)),
                "the exact source-census refusal is required, not resource exhaustion"
            );
            return Ok(());
        }
        let permit = result.map_err(source_address_call_error_v29)?;
        permit
            .check(function, budget)
            .map_err(source_address_call_error_v29)?;
        let equal_clone = function.clone();
        assert_eq!(
            permit.check(&equal_clone, budget),
            Err(CallInstanceEmissionErrorV1::StorageTransport)
        );
        Ok(())
    });
    assert_eq!(budget.storage(), floor, "{result:?}");
    result
}

fn inspect_transport(
    transport: &ScopedStorageTransportV29,
    map: &ProductionInstanceCorrespondenceV1<'_, '_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), CallInstanceEmissionErrorV1> {
    let root = map.plan.root();
    assert_eq!(transport.chains[root.index()], ScopedStorageChainV29::default());
    assert!(transport.rows.iter().all(|row| row.instance != root));
    let expected: usize = emitted.iter().enumerate()
        .filter(|(ordinal, _)| *ordinal != root.index())
        .filter_map(|(_, lowered)| lowered.as_ref())
        .filter_map(|lowered| lowered.scoped_memory_anchors.as_ref())
        .map(|anchors| anchors.objects.len()).sum();
    assert_eq!(transport.rows.len(), expected);
    let root_lowered = emitted[root.index()].as_ref().unwrap();
    assert!(root_lowered.function.body.as_ref().unwrap().blocks.iter()
        .flat_map(|block| &block.operations)
        .all(|operation| !matches!(operation.kind, OperationKind::Execution(_))),
        "the root probe is before deferred lifecycle operations are inserted");
    probe(transport, map, root, &root_lowered.function, true, budget)
        .map_err(scoped_storage_error_v29)?;
    let (ordinal, function) = emitted
        .iter()
        .enumerate()
        .filter(|(ordinal, _)| *ordinal != root.index())
        .find_map(|(ordinal, lowered)| {
            (transport.chains[ordinal].count != 0)
                .then(|| lowered.as_ref().map(|lowered| (ordinal, &lowered.function)))
                .flatten()
        })
        .unwrap();
    let child = map.plan.id_at(ordinal).unwrap();
    let parent = map.plan.incoming(child).unwrap().occurrence().caller;
    assert!(map.plan.incoming(parent).is_some(),
        "the authentic storage-bearing callback must be merged into a helper before that helper is merged again");
    probe(transport, map, child, function, false, budget).map_err(scoped_storage_error_v29)?;
    let operation = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| matches!(operation.kind, OperationKind::Storage(_)))
        .unwrap();
    assert_eq!(
        call_splice_check_callee_operation_v1(&operation.kind),
        Err(CallInstanceEmissionErrorV1::StorageTransport)
    );
    for fault in 0..5 {
        let mut changed = function.clone();
        let block = changed
            .body
            .as_mut()
            .unwrap()
            .blocks
            .iter_mut()
            .find(|block| {
                block.operations.iter().any(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. })
                    )
                })
            })
            .unwrap();
        let position = block
            .operations
            .iter()
            .position(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. })
                )
            })
            .unwrap();
        match fault {
            0 => block.operations[position].kind = OperationKind::Constant(Constant::U32(0)),
            1 => {
                let extra = block.operations[position].clone();
                block.operations.push(extra);
            }
            2 => {
                let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { access, .. }) =
                    &mut block.operations[position].kind
                else {
                    unreachable!()
                };
                access.alignment = access.alignment.checked_add(1).unwrap();
            }
            3 => {
                let OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                    address,
                    value,
                    ..
                }) = &mut block.operations[position].kind
                else {
                    unreachable!()
                };
                *value = *address;
            }
            4 => block.operations[position].results.push(ValueDef::new(
                ValueId(u32::MAX),
                Type::Scalar(ScalarType::U32),
            )),
            _ => unreachable!(),
        }
        probe(transport, map, child, &changed, true, budget).map_err(scoped_storage_error_v29)?;
    }
    // Changing a type with identical syntax/ValueIds is not an original schema.
    let mut changed = function.clone();
    let row = &transport.rows[transport.chains[ordinal].first.unwrap()];
    let address = row.inputs[0].unwrap().0;
    let definition = changed
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
        .flat_map(|operation| &mut operation.results)
        .find(|value| value.id == address)
        .unwrap();
    definition.ty = Type::pointer(
        Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX)),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    probe(transport, map, child, &changed, true, budget).map_err(scoped_storage_error_v29)?;
    for fault in 0..4 {
        let mut changed = transport.clone();
        match fault {
            0 => changed.source_plan ^= 1,
            1 => changed.chains[ordinal].count = 0,
            2 => changed.rows[changed.chains[ordinal].first.unwrap()].instance = map.plan.root(),
            3 => changed.rows[changed.chains[ordinal].first.unwrap()].offset = u32::MAX,
            _ => unreachable!(),
        }
        probe(&changed, map, child, function, true, budget).map_err(scoped_storage_error_v29)?;
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let foreign = ArgumentBudgetV1::new(&mut work, 0);
    assert_eq!(
        transport.check_custody(&foreign),
        Err(CallInstanceEmissionErrorV1::Resource(
            ArgumentResourceV1::Accounting
        ))
    );
    assert_eq!((foreign.work(), foreign.storage()), (0, 0));
    let mut other_slot = transport.clone();
    other_slot.slot ^= 1;
    let before = (budget.work(), budget.storage());
    assert_eq!(
        other_slot.check_custody(budget),
        Err(CallInstanceEmissionErrorV1::Resource(
            ArgumentResourceV1::Accounting
        ))
    );
    assert_eq!((budget.work(), budget.storage()), before);
    probe(transport, map, child, function, false, budget).map_err(scoped_storage_error_v29)?;
    STORAGE_OBSERVED.set(STORAGE_OBSERVED.get() + 1);
    Ok(())
}

struct RestoreStorageObserver(Option<ScopedStorageObserverV29>);
impl Drop for RestoreStorageObserver {
    fn drop(&mut self) {
        SCOPED_STORAGE_OBSERVER_V29.set(self.0);
    }
}

#[test]
fn scoped_storage_transport_requires_actual_source_census_then_complete_memory_admission() {
    for retained_address in [false, true] {
        STORAGE_OBSERVED.set(0);
        let _restore =
            RestoreStorageObserver(SCOPED_STORAGE_OBSERVER_V29.replace(Some(inspect_transport)));
        let fixture = ScopedFixture::CallDestinations {
            projected: true,
            retained_address,
            indexed: false,
        };
        let (result, _, _) = run(
            false,
            fixture,
            typed_call_results::inspect_and_continue,
            LIMIT,
            LIMIT,
        );
        result.unwrap();
        assert_eq!(
            STORAGE_OBSERVED.get(),
            4,
            "inspect initial transport plus admission, reconstruction, and consuming source replay"
        );
    }
}

fn mapped(count: u32) -> InstanceMappedSpanV1 {
    InstanceMappedSpanV1 {
        instance: ProductionCallInstanceIdV1(3),
        source: InstanceSpanSourceV1::Terminator(SemanticKirTerminatorOperationSpanV1 {
            correspondence_owner: SemanticFunctionIdV1::from_index(0),
            semantic_function: SemanticFunctionIdV1::from_index(1),
            semantic_block: SemanticBlockIdV1::from_index(2),
            kernel_ir_block: BlockId(20),
            first_operation_ordinal: 7,
            operation_count: count,
        }),
        segments: [
            Some(InstancePhysicalSpanV1 {
                block: BlockId(20),
                first: 7,
                count,
            }),
            None,
        ],
        removed_call: None,
    }
}

#[test]
fn scoped_storage_coordinates_preserve_prefix_suffix_and_packed_trailing_span() {
    for count in 1..17 {
        for call in 0..count {
            let original = mapped(count);
            let occurrence = ProductionCallOccurrenceV1 {
                caller: original.instance,
                block: SemanticBlockIdV1::from_index(2),
            };
            let moved = original
                .after_splice(
                    occurrence,
                    CallInstanceSplitV1 {
                        call: FunctionOperationLocation {
                            block: BlockId(20),
                            operation_index: (7 + call) as usize,
                        },
                        entry: BlockId(88),
                        callee_entry: BlockId(89),
                        continuation: BlockId(90),
                        callee_blocks: 1,
                        returns: 1,
                        result_components: 1,
                    },
                )
                .unwrap();
            for offset in 0..count {
                assert_eq!(
                    scoped_storage_mapped_point_v29(&original, offset, Some(call)),
                    Ok((BlockId(20), 7 + offset))
                );
                let expected = if offset == call {
                    Err(CallInstanceEmissionErrorV1::StorageTransport)
                } else if offset < call {
                    Ok((BlockId(20), 7 + offset))
                } else {
                    Ok((BlockId(90), offset - call - 1))
                };
                assert_eq!(
                    scoped_storage_mapped_point_v29(&moved, offset, Some(call)),
                    expected
                );
            }
            assert_eq!(
                scoped_storage_mapped_point_v29(&moved, count, Some(call)),
                Err(CallInstanceEmissionErrorV1::StorageTransport)
            );
            assert_eq!(
                scoped_storage_mapped_point_v29(&moved, 0, None),
                Err(CallInstanceEmissionErrorV1::StorageTransport)
            );
        }
    }
    let mut row = mapped(2);
    row.segments.swap(0, 1);
    assert_eq!(
        scoped_storage_mapped_point_v29(&row, 0, None),
        Err(CallInstanceEmissionErrorV1::StorageTransport)
    );
    row = mapped(2);
    row.removed_call = Some(ProductionCallOccurrenceV1 {
        caller: ProductionCallInstanceIdV1(4),
        block: SemanticBlockIdV1::from_index(2),
    });
    assert_eq!(
        scoped_storage_mapped_point_v29(&row, 0, Some(1)),
        Err(CallInstanceEmissionErrorV1::StorageTransport)
    );
    row.removed_call = Some(ProductionCallOccurrenceV1 {
        caller: row.instance,
        block: SemanticBlockIdV1::from_index(99),
    });
    assert_eq!(
        scoped_storage_mapped_point_v29(&row, 0, Some(1)),
        Err(CallInstanceEmissionErrorV1::StorageTransport)
    );
}

#[test]
fn scoped_storage_source_span_index_rejects_duplicate_original_coverage() {
    let rows = [mapped(2), mapped(2)];
    let mut work = CanonicalKernelIrWorkBudgetV1::new(10_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 10_000);
    budget.reserve_storage(43).unwrap();
    let mut completed = false;
    with_canonical_call_scratch_v1(&mut budget, |budget| {
        let mut scratch = 0;
        assert!(matches!(
            scoped_storage_span_index_v29(&rows, ProductionCallInstanceIdV1(99), budget, &mut scratch),
            Err(CallInstanceEmissionErrorV1::StorageTransport)
        ));
        completed = true;
        Ok(())
    })
    .unwrap();
    assert!(completed);
    assert_eq!(budget.storage(), 43);
}

fn span_resources(
    count: usize,
    root_count: usize,
    work_limit: usize,
    storage_limit: usize,
) -> (Result<(), ProductionSemanticKirErrorV1>, usize, usize, bool) {
    const FLOOR: usize = 43;
    let root = ProductionCallInstanceIdV1(count + 17);
    let mut rows: Vec<_> = (0..root_count).map(|ordinal| {
        let mut row = mapped(3);
        row.instance = root;
        let InstanceSpanSourceV1::Terminator(source) = &mut row.source else { unreachable!() };
        source.first_operation_ordinal = 3 * ordinal as u32;
        row.segments[0].as_mut().unwrap().first = 3 * ordinal as u32;
        row
    }).collect();
    rows.extend((0..count)
        .rev()
        .map(|ordinal| {
            let mut row = mapped(3);
            row.instance = ProductionCallInstanceIdV1(ordinal);
            row
        })
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(FLOOR).unwrap();
    let mut completed = false;
    let result = with_canonical_call_scratch_v1(&mut budget, |budget| {
        let mut scratch = 0;
        let index = scoped_storage_span_index_v29(&rows, root, budget, &mut scratch)
            .map_err(source_address_call_error_v29)?;
        assert_eq!(index.len(), count);
        for (ordinal, row) in index.iter().enumerate() {
            assert_eq!(*row, ((ordinal, BlockId(20), 7), root_count + count - ordinal - 1, None));
        }
        let bytes = size_of::<Vec<((usize, BlockId, u32), usize, Option<u32>)>>()
            + count * size_of::<((usize, BlockId, u32), usize, Option<u32>)>();
        let search_bound = usize::BITS as usize - count.leading_zeros() as usize + 1;
        assert_eq!(
            scratch, bytes,
            "both the Vec header and every key slot are prepaid"
        );
        assert_eq!(budget.storage(), FLOOR + bytes);
        assert_eq!(budget.work(), 2 + 2 * (root_count + count) + count + 4 * count * search_bound);
        drop(index);
        completed = true;
        Ok(())
    });
    assert_eq!(budget.storage(), FLOOR, "{result:?}");
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn scoped_storage_source_span_index_has_independent_exact_and_one_short_resources() {
    for count in [0usize, 1, 2, 31, 32, 64, 128] {
        let bound = usize::BITS as usize - count.leading_zeros() as usize + 1;
        let work = 2 + 3 * count + 4 * count * bound;
        let bytes = 43
            + size_of::<Vec<((usize, BlockId, u32), usize, Option<u32>)>>()
            + count * size_of::<((usize, BlockId, u32), usize, Option<u32>)>();
        let (result, actual_work, peak, completed) = span_resources(count, 0, work, bytes);
        result.unwrap();
        assert!(completed);
        assert_eq!((actual_work, peak), (work, bytes));
        for short_work in [false, true] {
            let (result, _, _, completed) = span_resources(
                count,
                0,
                work - usize::from(short_work),
                bytes - usize::from(!short_work),
            );
            assert!(!completed);
            let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) = result
            else {
                panic!("exact one-short resource refusal required");
            };
            if short_work {
                assert!(matches!(error, ArgumentResourceV1::Work { .. }));
            } else {
                assert!(matches!(error, ArgumentResourceV1::Storage { .. }));
            }
        }
    }
}

#[test]
fn scoped_storage_large_root_small_helper_index_retains_only_moved_source_spans() {
    for root_count in [1usize, 64, 1024] {
        for count in [0usize, 1, 3] {
            let bound = usize::BITS as usize - count.leading_zeros() as usize + 1;
            let work = 2 + 2 * (root_count + count) + count + 4 * count * bound;
            let bytes = 43 + size_of::<Vec<((usize, BlockId, u32), usize, Option<u32>)>>()
                + count * size_of::<((usize, BlockId, u32), usize, Option<u32>)>();
            let (result, actual_work, peak, completed) = span_resources(count, root_count, work, bytes);
            result.unwrap();
            assert!(completed);
            assert_eq!((actual_work, peak), (work, bytes));
            for short_work in [false, true] {
                let (short, _, _, completed) = span_resources(count, root_count,
                    work - usize::from(short_work), bytes - usize::from(!short_work));
                assert!(!completed);
                let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) = short else {
                    panic!("exact one-short resource refusal required");
                };
                if short_work {
                    assert!(matches!(error, ArgumentResourceV1::Work { .. }));
                } else {
                    assert!(matches!(error, ArgumentResourceV1::Storage { .. }));
                }
            }
        }
    }
}
