use super::*;

fn original_argument_pointer_array_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let template = original_scalar_array_owner_v29();
    let semantic = template.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let array = functions[2].locals()[2].ty();
    let pointer = functions[2].locals()[3].ty();
    let SemanticTypeShapeV1::Array { length, .. } =
        semantic.types()[array.index() as usize].shape()
    else {
        panic!("actual array declaration");
    };
    let call = |tag, target| {
        block(
            tag,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(2),
                    vec![SemanticOperandV1::Copy(place(1, pointer))],
                    Some(SemanticCallDestinationV1::new(
                        place(0, UNIT),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(target),
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        )
    };
    functions[0] = function(
        60,
        SemanticFunctionRoleV1::KernelRoot,
        abi(61, true, &[pointer]),
        vec![
            local(62, UNIT, SemanticLocalRoleV1::Return),
            local(63, pointer, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![
            call(201, 1),
            call(202, 2),
            block(203, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(semantic.functions()[0].kernel_entry().unwrap().clone());
    let indexed = || {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: length - 1,
                        minimum_length: *length,
                        from_end: false,
                    },
                    pointer,
                )
                .unwrap(),
            ],
            pointer,
        )
        .unwrap()
    };
    let mut statements = Vec::new();
    for _ in 0..2 {
        statements.push(SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
        ));
        statements.push(assign(
            place(2, array),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Array,
                    (0..*length)
                        .map(|_| SemanticOperandV1::Copy(place(1, pointer)))
                        .collect(),
                )
                .unwrap(),
            ),
        ));
        statements.push(assign(
            indexed(),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(1, pointer))),
        ));
        statements.push(assign(
            place(3, pointer),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                indexed(),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ));
        statements.push(SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(2)),
        ));
    }
    functions[2] = function(
        210,
        SemanticFunctionRoleV1::InternalHelper,
        abi(211, false, &[pointer]),
        vec![
            local(212, UNIT, SemanticLocalRoleV1::Return),
            local(213, pointer, SemanticLocalRoleV1::Argument(0)),
            local(219, array, SemanticLocalRoleV1::Temporary),
            local(220, pointer, SemanticLocalRoleV1::Temporary),
        ],
        vec![block(214, statements, SemanticTerminatorKindV1::Return)],
    );
    let missing_pointer_abi = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions.clone(),
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap();
    assert!(matches!(
        missing_pointer_abi.admit_exact_v29(SemanticMirLimitsV1::default()),
        Err(fe2o3_mir_model::semantic_mir_v1::SemanticMirErrorV1::InvalidFunctionAbi)
    ));
    // The template pointer was local-only. Direct ABI use also requires its
    // exact raw pointee facts; these grant no alignment or dereference promise.
    let mut types = semantic.types().to_vec();
    let declaration = &types[pointer.index() as usize];
    types[pointer.index() as usize] = declaration.clone().with_rustc_abi_properties(
        declaration.abi_properties().with_scalar_pointee_info(
            Some(
                fe2o3_mir_model::semantic_mir_v1::SemanticAbiPointeeInfoV1::new(
                    fe2o3_mir_model::semantic_mir_v1::SemanticAbiPointeeKindV1::Raw,
                    0,
                    1,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        semantic.callables().to_vec(),
        semantic.roots().to_vec(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn observe_original_pointer_array_emission_v29(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut helpers = BTreeSet::new();
    let expected = Type::pointer(
        Type::Scalar(ScalarType::U64),
        AddressSpace::Generic,
        AccessMode::ReadOnly,
    );
    for slot in &slots.slots {
        let ScopedAllocationSourceV29::OriginalArray { .. } = slot.origin.source else {
            continue;
        };
        let array = slot.scalar_array()?;
        assert_eq!(array.element.element.into_owned_type(), expected);
        assert_eq!((array.length, array.bytes, array.element.size), (3, 24, 8));
        assert!(array.count.is_some());
        assert!(helpers.insert(slot.instance.index()));
        let original = instances.instance(slot.instance).unwrap();
        assert_eq!(original.function().index(), 2);
        assert_eq!(
            original.declaration().blocks()[0].statements().iter().filter(|statement|
                matches!(statement.kind(), SemanticStatementKindV1::StorageLive(local) if local.index() == 2)).count(),
            2,
        );
        let lowered = emitted[slot.instance.index()].as_ref().unwrap();
        let actual = &lowered.function.body.as_ref().unwrap().blocks[slot.allocation.block_ordinal]
            .operations[slot.allocation.operation];
        check_scoped_slot_alloca_v29(slot, actual, budget)?;
        assert!(
            matches!(&actual.kind, OperationKind::Alloca { element, count: Some(_), .. } if element == &expected)
        );
        for changed in [
            Type::pointer(
                Type::Scalar(ScalarType::U64),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            Type::pointer(
                Type::Scalar(ScalarType::U64),
                AddressSpace::Generic,
                AccessMode::ReadWrite,
            ),
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Generic,
                AccessMode::ReadOnly,
            ),
        ] {
            let mut forged = actual.clone();
            let OperationKind::Alloca { element, .. } = &mut forged.kind else {
                unreachable!();
            };
            *element = changed.clone();
            forged.results[0].ty =
                Type::pointer(changed, AddressSpace::Private, AccessMode::ReadWrite);
            assert!(matches!(
                check_scoped_slot_alloca_v29(slot, &forged, budget),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "scoped source-slot allocation census is incomplete or mismatched",
                    ..
                })
            ));
        }
    }
    let root = instances
        .instance(instances.root())
        .unwrap()
        .function()
        .index();
    assert_eq!(
        helpers.len(),
        if root == 0 {
            2
        } else {
            assert_eq!(root, 1);
            0
        }
    );
    SOURCE_ARRAY_OBSERVED_V29.set(SOURCE_ARRAY_OBSERVED_V29.get() + helpers.len());
    Ok(())
}

#[test]
fn actual_argument_pointer_arrays_keep_original_shapes_generations_and_exact_physical_checks() {
    struct Restore(
        (u16, u64, bool),
        SourceArrayModeV29,
        Option<ScopedSlotObserverV29>,
        usize,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_ARRAY_CASE_V29.set(self.0);
            SOURCE_ARRAY_MODE_V29.set(self.1);
            SCOPED_SLOT_OBSERVER_V29.set(self.2);
            SOURCE_ARRAY_OBSERVED_V29.set(self.3);
        }
    }
    let _restore = Restore(
        SOURCE_ARRAY_CASE_V29.replace((64, 3, false)),
        SOURCE_ARRAY_MODE_V29.replace(SourceArrayModeV29::ThinPointer),
        SCOPED_SLOT_OBSERVER_V29.get(),
        SOURCE_ARRAY_OBSERVED_V29.replace(0),
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared =
        scalar_payload_prepared_from_v18(original_argument_pointer_array_owner_v29, &mut budget);
    SCOPED_SLOT_OBSERVER_V29.set(Some(observe_original_pointer_array_emission_v29));
    let reached = std::cell::Cell::new(false);
    prepared
        .with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| {
                scope.with_inventory_v1(|inventory, budget| {
                    source.with_ranked_correspondence_v18(inventory, budget, |relation, budget| {
                        let mut count = 0;
                        for root in 0..source.root_count(budget)? {
                            let (_, function) = source.root(root, budget)?;
                            for operation in &inventory.operations()
                                [inventory.functions()[function].operations.clone()]
                            {
                                if let Some(allocation) = relation.retained_allocation(
                                    root,
                                    operation.coordinate,
                                    budget,
                                )? && matches!(
                                    allocation.slot.origin.source,
                                    ScopedAllocationSourceV29::OriginalArray { .. }
                                ) {
                                    assert_eq!(
                                        allocation
                                            .slot
                                            .scalar_array()
                                            .unwrap()
                                            .element
                                            .element
                                            .into_owned_type(),
                                        Type::pointer(
                                            Type::Scalar(ScalarType::U64),
                                            AddressSpace::Generic,
                                            AccessMode::ReadOnly
                                        )
                                    );
                                    count += 1;
                                }
                            }
                        }
                        assert_eq!(count, 2);
                        reached.set(true);
                        Ok(())
                    })
                })
            })
        })
        .unwrap();
    assert!(reached.get());
    assert_eq!(SOURCE_ARRAY_OBSERVED_V29.get(), 3 * 2);
    assert_eq!(budget.storage(), MODULE_FLOOR);
}
