thread_local! {
    static MIXED_MEMORY_FAULT_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static MIXED_MEMORY_CHANGED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn mixed_original_array_object_owner_v29(whole: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = initialized_literal_array_owner_v29(whole);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let raw = declaration(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                U32,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
        None,
    );
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[3];
    let mut locals = helper.locals().to_vec();
    assert_eq!(locals.len(), 3);
    locals.push(local(136, U32, SemanticLocalRoleV1::Temporary));
    locals.push(local(137, raw, SemanticLocalRoleV1::Temporary));
    locals.push(local(138, U32, SemanticLocalRoleV1::Temporary));
    let mut initial = helper.blocks()[0].statements().to_vec();
    initial.push(assign(place(3, U32), SemanticRvalueKindV1::Use(
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
        )),
    )));
    initial.push(assign(
        place(4, raw),
        SemanticRvalueKindV1::AddressOf {
            place: place(3, U32),
            mutability: SemanticMutabilityV1::Mutable,
        },
    ));
    initial.push(assign(
        place(5, U32),
        SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(4),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap(),
                ],
                U32,
            )
            .unwrap(),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    ));
    let mut blocks = helper.blocks().to_vec();
    blocks[0] = block(140, initial, blocks[0].terminator().kind().clone());
    functions[3] = function(130, helper.role(), helper.abi().clone(), locals, blocks);
    super::super::fixtures::build(types, functions, semantic.callables().to_vec())
}

fn mixed_selected_array_object_owner_v29(branches: bool) -> ProductionSemanticSsaOwnerV1 {
    let original = super::super::fixtures::array_owner(branches);
    let semantic = original.source_semantic();
    let mut types = semantic.types().to_vec();
    let raw = declaration(
        &mut types,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                U32,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
        None,
    );
    let mut functions = semantic.functions().to_vec();
    for (index, tag, entry) in [(1, 100, 90), (2, 110, 115)] {
        let helper = &functions[index];
        let mut locals = helper.locals().to_vec();
        let object = locals.len() as u32;
        let pointer = object + 1;
        let value = object + 2;
        locals.push(local(142, U32, SemanticLocalRoleV1::Temporary));
        locals.push(local(143, raw, SemanticLocalRoleV1::Temporary));
        locals.push(local(144, U32, SemanticLocalRoleV1::Temporary));
        let mut initial = helper.blocks()[0].statements().to_vec();
        let SemanticStatementKindV1::Assign(selected) = initial[2].kind() else {
            panic!("original selected array write");
        };
        let SemanticProjectionKindV1::Index(index_local) =
            selected.destination().projections()[0].kind()
        else {
            panic!("original index local");
        };
        let index_place = place(index_local.index(), U32);
        initial.splice(
            2..2,
            [
                // A genuine source Store retains the index holder. Its later Load
                // must read through the disjoint typed object write below using
                // the complete, shared MemorySSA graph.
                SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        index_place.clone(),
                        SemanticOperandV1::Copy(index_place),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
                assign(place(object, U32), SemanticRvalueKindV1::Use(
                    SemanticOperandV1::Constant(SemanticConstantV1::new(
                        U32,
                        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(7, 4).unwrap()),
                    )),
                )),
                assign(
                    place(pointer, raw),
                    SemanticRvalueKindV1::AddressOf {
                        place: place(object, U32),
                        mutability: SemanticMutabilityV1::Mutable,
                    },
                ),
                assign(
                    place(value, U32),
                    SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(pointer),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Dereference,
                                    U32,
                                )
                                .unwrap(),
                            ],
                            U32,
                        )
                        .unwrap(),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
            ],
        );
        let mut blocks = helper.blocks().to_vec();
        blocks[0] = block(entry, initial, blocks[0].terminator().kind().clone());
        functions[index] = function(tag, helper.role(), helper.abi().clone(), locals, blocks);
    }
    super::super::fixtures::build(types, functions, semantic.callables().to_vec())
}

fn inspect_mixed_selected_memory_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    let mut domains = [0usize; 3];
    for slot in &slots.slots {
        let instance = instances.instance(slot.instance).unwrap();
        assert!(matches!(instance.function().index(), 1 | 2));
        let lowered = emitted[slot.instance.index()].as_ref().unwrap();
        match slot.representation {
            ScopedSlotRepresentationV29::ScalarArray(scalar) => {
                assert_eq!(scalar.length, 1);
                if matches!(
                    slot.origin.source,
                    ScopedAllocationSourceV29::OriginalArray { .. }
                ) {
                    assert!(lowered.function.body.as_ref().unwrap().blocks.iter().flat_map(|block| &block.operations)
                        .any(|operation| matches!(operation.kind, OperationKind::GetElementPointer { base, .. }
                            if base == slot.origin.pointer)));
                    domains[0] += 1;
                } else {
                    assert!(
                        lowered
                            .scoped_memory_anchors
                            .as_ref()
                            .unwrap()
                            .rows
                            .iter()
                            .any(|row| matches!(
                                row.kind,
                                ScopedMemoryAnchorKindV29::Access {
                                    payload: Some(ScopedMemoryPayloadV29::IndexLoad { .. }),
                                    ..
                                }
                            ))
                    );
                    domains[2] += 1;
                }
            }
            ScopedSlotRepresentationV29::Object { .. } => {
                assert!(
                    matches!(slot.origin.identity, ScopedAllocationIdentityV29::OriginalObject { local, .. }
                    if local as usize == instance.declaration().locals().len() - 3)
                );
                domains[1] += 1;
            }
        }
    }
    assert_eq!(domains, [2, 2, 2]);
    Ok(())
}

fn inspect_mixed_original_memory_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    OBSERVED.set(OBSERVED.get() + 1);
    let mut arrays = 0;
    let mut objects = 0;
    let mut changed = 0;
    for slot in &slots.slots {
        let original = instances.instance(slot.instance).unwrap().declaration();
        assert_eq!(
            original.identity(),
            SemanticFunctionIdentityV1::from_sha256([130; 32])
        );
        let lowered = emitted[slot.instance.index()].as_mut().unwrap();
        match slot.representation {
            ScopedSlotRepresentationV29::Object { .. } => {
                objects += 1;
                assert!(matches!(
                    slot.origin.identity,
                    ScopedAllocationIdentityV29::OriginalObject { local: 3, .. }
                ));
                let body = lowered.function.body.as_ref().unwrap();
                assert!(body.blocks.iter().flat_map(|block| &block.operations).any(|operation|
                    matches!(operation.kind, OperationKind::Storage(ScopedObjectOperationV29::WriteValue { address, .. })
                        if address == slot.origin.pointer)));
                assert!(body.blocks.iter().flat_map(|block| &block.operations).any(
                    |operation| matches!(
                        operation.kind,
                        OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. })
                    )
                ));
            }
            ScopedSlotRepresentationV29::ScalarArray(scalar) => {
                arrays += 1;
                assert_eq!(scalar.length, 2);
                assert!(matches!(
                    slot.origin.source,
                    ScopedAllocationSourceV29::OriginalArray { .. }
                ));
                assert_eq!(slot.origin.semantic_type, original.locals()[2].ty());
                if MIXED_MEMORY_FAULT_V29.get() {
                    let block_id = lowered
                        .blocks
                        .iter()
                        .find(|row| row.semantic_block.index() == 1)
                        .unwrap()
                        .kernel_ir_block;
                    let body = lowered.function.body.as_mut().unwrap();
                    let block = body
                        .blocks
                        .iter_mut()
                        .find(|row| row.id == block_id)
                        .unwrap();
                    let pointer = block
                        .operations
                        .iter()
                        .find_map(|operation| {
                            if let OperationKind::Load { pointer, .. } = operation.kind {
                                Some(pointer)
                            } else {
                                None
                            }
                        })
                        .unwrap();
                    let address = block
                        .operations
                        .iter()
                        .find(|operation| {
                            operation.results.iter().any(|result| result.id == pointer)
                        })
                        .unwrap();
                    let OperationKind::GetElementPointer { base, offset } = address.kind else {
                        panic!("actual literal array address");
                    };
                    assert_eq!(base, slot.origin.pointer);
                    let offset = block
                        .operations
                        .iter_mut()
                        .find(|operation| {
                            operation.results.iter().any(|result| result.id == offset)
                        })
                        .unwrap();
                    assert_eq!(offset.kind, OperationKind::Constant(Constant::Index(0)));
                    offset.kind = OperationKind::Constant(Constant::Index(1));
                    changed += 1;
                }
            }
        }
    }
    assert_eq!(
        (arrays, objects),
        (2, 2),
        "both repeated original instances retain both domains"
    );
    if MIXED_MEMORY_FAULT_V29.get() {
        assert_eq!(changed, 2);
    }
    MIXED_MEMORY_CHANGED_V29.set(MIXED_MEMORY_CHANGED_V29.get() + changed);
    Ok(())
}

#[test]
fn mixed_original_arrays_and_objects_finish_the_same_source_candidate() {
    for whole in [false, true] {
        MIXED_MEMORY_FAULT_V29.set(false);
        let (result, work, peak, completed) = run_original_repeated_source_v29(
            || mixed_original_array_object_owner_v29(whole),
            inspect_mixed_original_memory_v29,
            10_000_000,
            10_000_000,
        );
        assert!(result.is_ok() && completed, "whole={whole}: {result:?}");
        assert_eq!(OBSERVED.get(), 3);
        let (exact, _, _, completed) = run_original_repeated_source_v29(
            || mixed_original_array_object_owner_v29(whole),
            inspect_mixed_original_memory_v29,
            work,
            peak,
        );
        assert!(exact.is_ok() && completed, "{exact:?}");
        for (limit, storage, work_failure) in [(work - 1, peak, true), (work, peak - 1, false)] {
            let (refused, _, _, completed) = run_original_repeated_source_v29(
                || mixed_original_array_object_owner_v29(whole),
                inspect_mixed_original_memory_v29,
                limit,
                storage,
            );
            assert!(!completed);
            let resource = original_repeated_source_resource_v29(refused.unwrap_err());
            assert!(matches!(
                (work_failure, resource),
                (true, ArgumentResourceV1::Work(_)) | (false, ArgumentResourceV1::Storage(_))
            ));
        }
    }
}

#[test]
fn mixed_original_selected_arrays_keep_shared_memory_versions_with_object_effects() {
    for branches in [false, true] {
        let (result, work, peak, completed) = run_original_source_fixture_v29(
            || mixed_selected_array_object_owner_v29(branches),
            branches,
            false,
            1,
            inspect_mixed_selected_memory_v29,
            10_000_000,
            10_000_000,
        );
        assert!(
            result.is_ok() && completed,
            "branches={branches}: {result:?}"
        );
        assert_eq!(OBSERVED.get(), 3);
        let (exact, _, _, completed) = run_original_source_fixture_v29(
            || mixed_selected_array_object_owner_v29(branches),
            branches, false, 1, inspect_mixed_selected_memory_v29, work, peak,
        );
        assert!(exact.is_ok() && completed, "branches={branches}: {exact:?}");
        assert_eq!(OBSERVED.get(), 3);
        for (limit, storage, work_failure) in [(work - 1, peak, true), (work, peak - 1, false)] {
            let (refused, _, _, completed) = run_original_source_fixture_v29(
                || mixed_selected_array_object_owner_v29(branches),
                branches, false, 1, inspect_mixed_selected_memory_v29, limit, storage,
            );
            assert!(!completed);
            let resource = original_repeated_source_resource_v29(refused.unwrap_err());
            match (work_failure, resource) {
                (true, ArgumentResourceV1::Work(error)) => {
                    assert_eq!(error.limit(), limit);
                    assert!(error.actual() > limit, "{error:?}");
                }
                (false, ArgumentResourceV1::Storage(error)) => {
                    assert_eq!(error.limit(), storage);
                    assert!(error.actual() > storage, "{error:?}");
                }
                (_, error) => panic!("wrong resource refusal: {error:?}"),
            }
        }
    }
}

#[test]
fn mixed_original_object_writes_cannot_initialize_an_untouched_array_element() {
    MIXED_MEMORY_FAULT_V29.set(false);
    let (positive, _, _, completed) = run_original_repeated_source_v29(
        || mixed_original_array_object_owner_v29(false),
        inspect_mixed_original_memory_v29,
        10_000_000,
        10_000_000,
    );
    assert!(positive.is_ok() && completed, "{positive:?}");
    MIXED_MEMORY_FAULT_V29.set(true);
    MIXED_MEMORY_CHANGED_V29.set(0);
    let (refused, _, _, completed) = run_original_repeated_source_v29(
        || mixed_original_array_object_owner_v29(false),
        inspect_mixed_original_memory_v29,
        10_000_000,
        10_000_000,
    );
    MIXED_MEMORY_FAULT_V29.set(false);
    assert!(!completed);
    // Emission, immutable source reconstruction and ranked source replay all
    // preserve the same mutation. The final physical history rejects it only
    // after those three passes, each containing two original helper instances.
    assert_eq!(OBSERVED.get(), 3);
    assert_eq!(MIXED_MEMORY_CHANGED_V29.get(), 3 * 2);
    assert!(
        matches!(
            refused,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        detail: "scoped slot read is not initialized in its fresh physical activation",
                        ..
                    }
                )
            ))
        ),
        "{refused:?}"
    );
}
