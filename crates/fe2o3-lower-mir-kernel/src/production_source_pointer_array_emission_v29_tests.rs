use super::*;

mod index_producer_tests {
    include!("production_source_array_component_index_v29_tests.rs");
}

thread_local! {
    static POINTER_STORAGE_TRANSPORT_COMPLETED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn observe_original_pointer_storage_transport_v29(
    transport: &ScopedStorageTransportV29,
    map: &ProductionInstanceCorrespondenceV1<'_, '_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), CallInstanceEmissionErrorV1> {
    let first = transport.rows.iter().find(|row| {
        row.inputs
            .iter()
            .flatten()
            .any(|(_, ty)| matches!(ty, ScopedStorageTypeV29::OriginalPointer(..)))
    });
    let Some(first) = first else {
        return Ok(());
    };
    let instance = first.instance;
    let function = &emitted[instance.index()].as_ref().unwrap().function;
    let input = transport
        .rows
        .iter()
        .filter(|row| row.instance == instance)
        .flat_map(|row| row.inputs.iter().flatten())
        .find_map(|(value, ty)| {
            matches!(ty, ScopedStorageTypeV29::OriginalPointer(..)).then_some((*value, *ty))
        })
        .unwrap();
    let result = transport
        .rows
        .iter()
        .filter(|row| row.instance == instance)
        .find_map(|row| {
            let ty = row.result?;
            if !matches!(ty, ScopedStorageTypeV29::OriginalPointer(..)) {
                return None;
            }
            let ScopedStoragePayloadV59::Object(ScopedObjectPayloadV29 {
                operation: ScopedObjectOperationV29::ReadValue { .. },
                ..
            }) = row.payload
            else {
                panic!("original pointer result must be a value read");
            };
            let (block, operation) = scoped_storage_mapped_point_v29(
                &map.spans.rows[row.span],
                row.offset,
                row.call_offset,
            )
            .unwrap();
            let operation = &function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|row| row.id == block)
                .unwrap()
                .operations[operation as usize];
            let [value] = operation.results.as_slice() else {
                panic!("exact original pointer read result");
            };
            Some((value.id, ty))
        })
        .unwrap();
    for (selected, original) in [input, result] {
        assert_eq!(
            original,
            ScopedStorageTypeV29::OriginalPointer(
                ScalarType::U64,
                AddressSpace::Generic,
                AccessMode::ReadOnly,
            )
        );
        for fault in 0..6 {
            let mut changed = function.clone();
            let replacement = match fault {
                0 => Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Generic,
                    AccessMode::ReadOnly,
                ),
                1 => Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Generic,
                    AccessMode::ReadOnly,
                ),
                2 => Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                3 => Type::pointer(
                    Type::Scalar(ScalarType::U64),
                    AddressSpace::Generic,
                    AccessMode::ReadWrite,
                ),
                4 => Type::pointer(
                    Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(0)),
                    AddressSpace::Generic,
                    AccessMode::ReadOnly,
                ),
                _ => Type::Scalar(ScalarType::U64),
            };
            let mut definitions = 0;
            let body = changed.body.as_mut().unwrap();
            for (id, ty) in body
                .parameters
                .iter()
                .zip(&mut changed.signature.parameters)
            {
                if *id == selected {
                    *ty = replacement.clone();
                    definitions += 1;
                }
            }
            for block in &mut body.blocks {
                for value in block.parameters.iter_mut().chain(
                    block
                        .operations
                        .iter_mut()
                        .flat_map(|operation| &mut operation.results),
                ) {
                    if value.id == selected {
                        value.ty = replacement.clone();
                        definitions += 1;
                    }
                }
            }
            assert_eq!(definitions, 1);
            let floor = budget.storage();
            with_canonical_call_scratch_v1(budget, |budget| {
                let mut scratch = 0;
                let index = call_splice_index_v1(&changed, budget, &mut scratch)
                    .map_err(source_address_call_error_v29)?;
                let source = ScopedStorageCalleeSourceV29 {
                    transport,
                    map,
                    child: instance,
                };
                let checked = source.permit(&changed, &index, budget, &mut scratch);
                if fault == 0 {
                    checked
                        .map_err(source_address_call_error_v29)?
                        .check(&changed, budget)
                        .map_err(source_address_call_error_v29)?;
                } else {
                    assert!(matches!(
                        checked,
                        Err(CallInstanceEmissionErrorV1::StorageTransport)
                    ));
                }
                Ok(())
            })
            .map_err(scoped_storage_error_v29)?;
            assert_eq!(budget.storage(), floor);
        }
    }
    POINTER_STORAGE_TRANSPORT_COMPLETED_V29.set(POINTER_STORAGE_TRANSPORT_COMPLETED_V29.get() + 1);
    Ok(())
}

#[test]
fn original_pointer_storage_transport_authenticates_input_and_result_types_across_extents() {
    struct Restore(Option<ScopedStorageObserverV29>, usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_STORAGE_OBSERVER_V29.set(self.0);
            POINTER_STORAGE_TRANSPORT_COMPLETED_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SCOPED_STORAGE_OBSERVER_V29.replace(Some(observe_original_pointer_storage_transport_v29)),
        POINTER_STORAGE_TRANSPORT_COMPLETED_V29.replace(0),
    );
    for length in [1, 3, 5] {
        let before = POINTER_STORAGE_TRANSPORT_COMPLETED_V29.get();
        check_actual_argument_pointer_array_v29(length);
        assert!(POINTER_STORAGE_TRANSPORT_COMPLETED_V29.get() > before);
    }
}

#[test]
fn original_pointer_storage_values_are_distinct_from_selected_storage_addresses() {
    let schema = fe2o3_kernel_ir::StorageLayoutIdV1(7);
    let value = ScopedStorageTypeV29::OriginalPointer(
        ScalarType::U64,
        AddressSpace::Generic,
        AccessMode::ReadOnly,
    );
    let address =
        ScopedStorageTypeV29::Pointer(schema, AddressSpace::Generic, AccessMode::ReadOnly);
    let scalar_pointer = Type::pointer(
        Type::Scalar(ScalarType::U64),
        AddressSpace::Generic,
        AccessMode::ReadOnly,
    );
    let storage_pointer = Type::pointer(
        Type::StorageObject(schema),
        AddressSpace::Generic,
        AccessMode::ReadOnly,
    );
    assert!(value.matches(&scalar_pointer));
    assert!(!value.matches(&storage_pointer));
    assert!(address.matches(&storage_pointer));
    assert!(!address.matches(&scalar_pointer));
}

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
        abi(61, true, &[pointer])
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::RawPointer])
            .unwrap(),
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
        abi(211, false, &[pointer])
            .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::RawPointer])
            .unwrap(),
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
    let mut generations = BTreeSet::new();
    let (_, length, _) = SOURCE_ARRAY_CASE_V29.get();
    for slot in &slots.slots {
        let ScopedAllocationIdentityV29::OriginalObject {
            local: 2,
            generation,
        } = slot.origin.identity
        else {
            continue;
        };
        let ScopedSlotRepresentationV29::Object {
            schema,
            bytes,
            alignment,
        } = slot.representation
        else {
            panic!("pointer arrays require typed object representation");
        };
        assert!(matches!(slot.origin.source,
            ScopedAllocationSourceV29::OriginalObject { schema: original, .. }
                if original == schema));
        assert_eq!((bytes, alignment), (length * 8, 8));
        assert!(slot.scalar_array().is_err());
        helpers.insert(slot.instance.index());
        assert!(generations.insert((slot.instance.index(), generation)));
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
        assert!(matches!(&actual.kind, OperationKind::Alloca {
                element: Type::StorageObject(actual), count: None, ..
            } if *actual == schema));
        for mutation in 0..5 {
            let mut forged = actual.clone();
            let OperationKind::Alloca {
                element,
                count,
                alignment,
                ..
            } = &mut forged.kind
            else {
                unreachable!();
            };
            match mutation {
                0 => {
                    let changed = fe2o3_kernel_ir::StorageLayoutIdV1(schema.0 + 1);
                    *element = Type::StorageObject(changed);
                    forged.results[0].ty = Type::pointer(
                        Type::StorageObject(changed),
                        AddressSpace::Private,
                        AccessMode::ReadWrite,
                    );
                }
                1 => {
                    forged.results[0].ty = Type::pointer(
                        Type::StorageObject(schema),
                        AddressSpace::Global,
                        AccessMode::ReadWrite,
                    )
                }
                2 => {
                    forged.results[0].ty = Type::pointer(
                        Type::StorageObject(schema),
                        AddressSpace::Private,
                        AccessMode::ReadOnly,
                    )
                }
                3 => *count = Some(slot.origin.pointer),
                4 => *alignment = 4,
                _ => unreachable!(),
            }
            assert!(matches!(
                check_scoped_slot_alloca_v29(slot, &forged, budget),
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "typed allocation identity or representation requires its exact source contract",
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
    assert_eq!(
        generations.len(),
        3 * helpers.len(),
        "entry and both StorageLive generations"
    );
    for helper in &helpers {
        assert!(generations.contains(&(*helper, 0)));
        let lowered = emitted[*helper].as_ref().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        let instance = ProductionCallInstanceIdV1(*helper);
        let original = instances.instance(instance).unwrap().declaration();
        let occurrences = instances.occurrences(instance).unwrap();
        let mut components = BTreeSet::new();
        let mut projects = BTreeSet::new();
        for (ordinal, row) in anchors.rows.iter().enumerate() {
            let ScopedMemoryAnchorKindV29::Object(_) = row.kind else {
                continue;
            };
            let payload = anchors.object_payload(row, budget)?;
            let endpoint = match payload.role {
                ScopedObjectRoleV29::Project { projected, .. } => projected,
                ScopedObjectRoleV29::WriteValue { destination, .. } => destination,
                _ => continue,
            };
            let ScopedObjectSourceV29::AggregateComponent {
                site,
                operand,
                destination,
                variant: None,
            } = endpoint.source
            else {
                continue;
            };
            assert_eq!(destination.index(), 2);
            assert!(u64::from(operand) < length);
            anchors.check_object_source(original, &occurrences, ordinal, row, payload, budget)?;
            assert!(matches!(
                anchors.object_path(endpoint.path, budget)?,
                [ScopedObjectComponentV29::View {
                    projection: ScopedObjectViewProjectionV29::ArrayElement,
                    ..
                }]
            ));
            match payload.operation {
                ScopedObjectOperationV29::Project {
                    step: ScopedObjectProjectionV29::ArrayIndex(index),
                    ..
                } => {
                    assert!(projects.insert((scoped_memory_site_key_v29(site), operand)));
                    let constants: Vec<_> = lowered
                        .function
                        .body
                        .as_ref()
                        .unwrap()
                        .blocks
                        .iter()
                        .flat_map(|block| &block.operations)
                        .filter(|operation| {
                            operation.results.iter().any(|result| result.id == index)
                        })
                        .collect();
                    assert_eq!(constants.len(), 1);
                    assert_eq!(
                        constants[0].kind,
                        OperationKind::Constant(Constant::Index(u64::from(operand)))
                    );
                }
                ScopedObjectOperationV29::WriteValue { .. } => {
                    assert!(components.insert((scoped_memory_site_key_v29(site), operand)));
                    for mutation in 0..3 {
                        let mut forged = *payload;
                        let ScopedObjectRoleV29::WriteValue { destination, .. } = &mut forged.role
                        else {
                            unreachable!();
                        };
                        match mutation {
                            0 => {
                                destination.source = ScopedObjectSourceV29::AggregateComponent {
                                    site,
                                    operand: u32::MAX,
                                    destination: SemanticLocalIdV1::from_index(2),
                                    variant: None,
                                }
                            }
                            1 => {
                                destination.source = ScopedObjectSourceV29::AggregateComponent {
                                    site,
                                    operand,
                                    destination: SemanticLocalIdV1::from_index(3),
                                    variant: None,
                                }
                            }
                            2 => destination.projected_type = UNIT,
                            _ => unreachable!(),
                        }
                        assert!(
                            anchors
                                .check_object_source(
                                    original,
                                    &occurrences,
                                    ordinal,
                                    row,
                                    &forged,
                                    budget
                                )
                                .is_err()
                        );
                    }
                }
                _ => panic!("array component changed its emitted operation family"),
            }
        }
        assert_eq!(components.len(), 2 * length as usize);
        assert_eq!(projects, components);
        assert!(
            anchors
                .objects
                .iter()
                .any(|row| matches!(row.operation, ScopedObjectOperationV29::ReadValue { .. }))
        );
        assert!(
            anchors
                .objects
                .iter()
                .any(|row| matches!(row.operation, ScopedObjectOperationV29::WriteValue { .. }))
        );
    }
    SOURCE_ARRAY_OBSERVED_V29.set(SOURCE_ARRAY_OBSERVED_V29.get() + helpers.len());
    Ok(())
}

#[test]
fn original_pointer_array_leaf_types_authenticate_full_layout_and_sticky_refusal() {
    struct Restore((u16, u64, bool), SourceArrayModeV29);
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_ARRAY_CASE_V29.set(self.0);
            SOURCE_ARRAY_MODE_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SOURCE_ARRAY_CASE_V29.replace((64, 3, false)),
        SOURCE_ARRAY_MODE_V29.replace(SourceArrayModeV29::ThinPointer),
    );
    for mutation in 0..11 {
        let mut completed = false;
        let mut selected = None;
        let result = with_original_array_plan_from_v29(
            original_argument_pointer_array_owner_v29,
            |plan, budget| {
                let cell = plan
                    .cells
                    .rows
                    .iter()
                    .find(|row| row.local.index() == 2)
                    .unwrap();
                let SourceBackingKindV29::Object(array_schema) = cell.kind else {
                    panic!("array object");
                };
                let owner = plan.instances.owner();
                let SemanticTypeShapeV1::Array { element: ty, .. } =
                    owner.source_semantic().types()[cell.ty.index() as usize].shape()
                else {
                    panic!("array type");
                };
                let layouts = plan
                    .storage_root
                    .as_ref()
                    .unwrap()
                    .source_layouts(plan.instances, budget)?;
                let schema = {
                    let rows = layouts.rows(owner, budget)?;
                    let fe2o3_kernel_ir::StorageLayoutKindV1::Array { element, .. } =
                        rows[array_schema.0 as usize].kind
                    else {
                        panic!("array schema");
                    };
                    element
                };
                assert_eq!(
                    source_object_original_leaf_type_v29(plan, *ty, schema, budget)?,
                    Type::pointer(
                        Type::Scalar(ScalarType::U64),
                        AddressSpace::Generic,
                        AccessMode::ReadOnly
                    )
                );
                if mutation == 0 {
                    completed = true;
                    return Ok(());
                }
                if mutation >= 9 {
                    let storage_filler = if mutation == 10 {
                        let filler = MODULE_LIMIT - budget.storage();
                        budget.reserve_storage(filler)?;
                        filler
                    } else {
                        budget.charge_work(MODULE_LIMIT - budget.work())?;
                        0
                    };
                    let first = source_object_original_leaf_type_v29(plan, *ty, schema, budget)
                        .unwrap_err();
                    assert!(
                        matches!(
                            &first,
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            ) if mutation == 9
                        ) || matches!(
                            &first,
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Storage(_)
                            ) if mutation == 10
                        )
                    );
                    budget.release_storage(storage_filler)?;
                    let before = (budget.work(), budget.storage());
                    let retry = source_object_original_leaf_type_v29(plan, *ty, schema, budget)
                        .unwrap_err();
                    assert_eq!(format!("{first:?}"), format!("{retry:?}"));
                    assert_eq!((budget.work(), budget.storage()), before);
                    selected = Some(format!("{first:?}"));
                    completed = true;
                    return Err(first);
                }
                if mutation < 8 {
                    layouts.mutate_row_for_test_v29(schema, |row| {
                        let fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(pointer) = &mut row.kind
                        else {
                            panic!("pointer row");
                        };
                        match mutation {
                            1 => pointer.encoded_space = AddressSpace::Private,
                            2 => pointer.value_space = AddressSpace::Private,
                            3 => pointer.stored_bits = 32,
                            4 => pointer.access = AccessMode::ReadWrite,
                            5 => pointer.pointee = schema,
                            6 => row.size = 4,
                            7 => row.alignment = 4,
                            _ => unreachable!(),
                        }
                    });
                }
                let bad_schema = if mutation == 8 { array_schema } else { schema };
                let first = source_object_original_leaf_type_v29(plan, *ty, bad_schema, budget)
                    .unwrap_err();
                let expected = if matches!(mutation, 5 | 8) {
                    "selected storage child differs from its original source component"
                } else {
                    "typed allocation identity or representation requires its exact source contract"
                };
                assert!(matches!(&first,
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0, block: None, statement: None, detail
                    } if *detail == expected));
                let before = (budget.work(), budget.storage());
                let retry = source_object_original_leaf_type_v29(plan, *ty, schema, budget);
                if mutation == 8 {
                    assert_eq!(
                        retry.unwrap(),
                        Type::pointer(
                            Type::Scalar(ScalarType::U64),
                            AddressSpace::Generic,
                            AccessMode::ReadOnly
                        )
                    );
                } else {
                    assert_eq!(format!("{first:?}"), format!("{:?}", retry.unwrap_err()));
                }
                // Semantic refusal is not a shared resource denial. The next
                // query must revalidate and pay for its actual work and scratch.
                assert!(budget.work() > before.0);
                assert!(budget.storage() > before.1);
                assert_eq!(budget.failed_work(), None);
                assert_eq!(budget.failed_storage(), None);
                selected = Some(format!("{first:?}"));
                completed = true;
                Err(first)
            },
        );
        assert!(
            completed,
            "mutation {mutation}: callback assertions did not complete"
        );
        assert_eq!(
            result.as_ref().err().map(|error| format!("{error:?}")),
            selected
        );
    }
}

#[test]
fn original_pointer_leaf_query_does_not_admit_selected_private_address_representations() {
    struct Restore((u16, u64, bool), SourceArrayModeV29);
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_ARRAY_CASE_V29.set(self.0);
            SOURCE_ARRAY_MODE_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SOURCE_ARRAY_CASE_V29.replace((64, 3, false)),
        SOURCE_ARRAY_MODE_V29.replace(SourceArrayModeV29::PointerAddresses),
    );
    let mut completed = false;
    let mut selected = None;
    let result = with_original_scalar_array_plan_v29(|plan, budget| {
        let cell = plan
            .cells
            .rows
            .iter()
            .find(|row| row.local.index() == 2 && row.generation != 0)
            .unwrap();
        let SourceBackingKindV29::Object(array_schema) = cell.kind else {
            panic!("array object");
        };
        let owner = plan.instances.owner();
        let SemanticTypeShapeV1::Array { element: ty, .. } =
            owner.source_semantic().types()[cell.ty.index() as usize].shape()
        else {
            panic!("array type");
        };
        let layouts = plan
            .storage_root
            .as_ref()
            .unwrap()
            .source_layouts(plan.instances, budget)?;
        let rows = layouts.rows(owner, budget)?;
        let fe2o3_kernel_ir::StorageLayoutKindV1::Array { element, .. } =
            rows[array_schema.0 as usize].kind
        else {
            panic!("array schema");
        };
        let fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(pointer) = rows[element.0 as usize].kind
        else {
            panic!("pointer schema");
        };
        assert_eq!(
            (pointer.encoded_space, pointer.value_space),
            (AddressSpace::Generic, AddressSpace::Private)
        );
        assert_ne!(layouts.original_schema(owner, *ty, budget)?, Some(element));
        let selected_type =
            ScopedStorageTypeV29::Pointer(pointer.pointee, pointer.value_space, pointer.access);
        assert!(selected_type.matches(&Type::pointer(
            Type::StorageObject(pointer.pointee),
            pointer.value_space,
            pointer.access,
        )));
        assert!(!selected_type.matches(&Type::pointer(
            Type::Scalar(ScalarType::U64),
            pointer.value_space,
            pointer.access,
        )));
        drop(rows);
        let first = source_object_original_leaf_type_v29(plan, *ty, element, budget).unwrap_err();
        assert!(matches!(
            &first,
            ProductionSemanticKirErrorV1::Unsupported {
                function: 0,
                block: None,
                statement: None,
                detail: "typed allocation identity or representation requires its exact source contract"
            }
        ));
        let before = (budget.work(), budget.storage());
        let retry = source_object_original_leaf_type_v29(plan, *ty, element, budget).unwrap_err();
        assert_eq!(format!("{first:?}"), format!("{retry:?}"));
        assert!(budget.work() > before.0);
        assert!(budget.storage() > before.1);
        assert_eq!(budget.failed_work(), None);
        assert_eq!(budget.failed_storage(), None);
        selected = Some(format!("{first:?}"));
        completed = true;
        Err(first)
    });
    assert!(
        completed,
        "selected-address callback assertions did not complete"
    );
    assert_eq!(
        result.as_ref().err().map(|error| format!("{error:?}")),
        selected
    );
}

#[test]
fn original_pointer_leaf_query_refuses_foreign_and_alternate_custody_before_scratch() {
    struct Alternate<'a, 'work>(&'a mut ArgumentBudgetV1<'work>);
    impl SemanticEmissionBudgetV1 for Alternate<'_, '_> {
        fn work_ledger_identity_v1(
            &self,
        ) -> fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1 {
            self.0.work_ledger_identity_v1()
        }
        fn charge_work(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
            self.0.charge_work(amount).map_err(Into::into)
        }
        fn reserve_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
            self.0.reserve_storage(amount).map_err(Into::into)
        }
        fn release_storage(&mut self, amount: usize) -> Result<(), ProductionSemanticKirErrorV1> {
            self.0.release_storage(amount).map_err(Into::into)
        }
        fn storage(&self) -> usize {
            self.0.storage()
        }
    }
    for alternate in [false, true] {
        let mut entered = false;
        let result = with_original_object_access_builder(|builder, budget| {
            let before = (budget.work(), budget.storage());
            let mut foreign_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut foreign = ArgumentBudgetV1::new(&mut foreign_work, MODULE_LIMIT);
            let query = if alternate {
                Alternate(budget).source_object_original_leaf_type_v29(
                    &builder.plan,
                    UNIT,
                    fe2o3_kernel_ir::StorageLayoutIdV1(0),
                )
            } else {
                source_object_original_leaf_type_v29(
                    &builder.plan,
                    UNIT,
                    fe2o3_kernel_ir::StorageLayoutIdV1(0),
                    &mut foreign,
                )
            };
            assert!(matches!(
                query,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Accounting
                    )
                )
            ));
            assert_eq!((budget.work(), budget.storage()), before);
            assert_eq!((foreign.work(), foreign.storage()), (0, 0));
            assert!(
                source_object_original_leaf_type_v29(
                    &builder.plan,
                    UNIT,
                    fe2o3_kernel_ir::StorageLayoutIdV1(0),
                    budget
                )
                .is_err()
            );
            assert_eq!((budget.work(), budget.storage()), before);
            entered = true;
            Ok(())
        });
        assert!(entered && result.is_err());
    }
}

#[test]
fn actual_argument_pointer_arrays_keep_original_shapes_generations_and_exact_physical_checks() {
    check_actual_argument_pointer_array_v29(3);
}

#[test]
fn actual_argument_pointer_array_object_routing_is_not_specific_to_one_extent() {
    for length in [1, 5] {
        check_actual_argument_pointer_array_v29(length);
    }
}

fn check_actual_argument_pointer_array_v29(length: u64) {
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
        SOURCE_ARRAY_CASE_V29.replace((64, length, false)),
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
                                    ScopedAllocationSourceV29::OriginalObject { .. }
                                ) {
                                    let ScopedAllocationIdentityV29::OriginalObject {
                                        local: 2,
                                        ..
                                    } = allocation.slot.origin.identity
                                    else {
                                        continue;
                                    };
                                    let ScopedSlotRepresentationV29::Object {
                                        schema,
                                        bytes,
                                        alignment,
                                    } = allocation.slot.representation
                                    else {
                                        panic!("source pointer array must stay object");
                                    };
                                    assert_eq!((bytes, alignment), (length * 8, 8));
                                    let layouts = &inventory.owner().module().storage_layouts;
                                    let fe2o3_kernel_ir::StorageLayoutKindV1::Array {
                                        element,
                                        length: actual,
                                        stride,
                                    } = layouts[schema.0 as usize].kind
                                    else {
                                        panic!("actual original array schema");
                                    };
                                    assert_eq!((actual, stride), (length, 8));
                                    let fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(pointer) =
                                        &layouts[element.0 as usize].kind
                                    else {
                                        panic!("actual original pointer schema");
                                    };
                                    assert_eq!(
                                        (
                                            pointer.encoded_space,
                                            pointer.value_space,
                                            pointer.stored_bits,
                                            pointer.access
                                        ),
                                        (
                                            AddressSpace::Generic,
                                            AddressSpace::Generic,
                                            64,
                                            AccessMode::ReadOnly
                                        )
                                    );
                                    assert!(matches!(
                                        layouts[pointer.pointee.0 as usize].kind,
                                        fe2o3_kernel_ir::StorageLayoutKindV1::Scalar(
                                            ScalarType::U64
                                        )
                                    ));
                                    count += 1;
                                }
                            }
                        }
                        assert_eq!(
                            count, 6,
                            "two helpers retain three distinct source generations"
                        );
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

#[test]
fn pointer_array_layout_compatibility_does_not_grant_scalar_array_payload_admission() {
    struct RestoreCase((u16, u64, bool));
    impl Drop for RestoreCase {
        fn drop(&mut self) {
            SOURCE_ARRAY_CASE_V29.set(self.0);
        }
    }
    for mode in [SourceArrayModeV29::Scalar, SourceArrayModeV29::ThinPointer] {
        let _restore = RestoreSourceArrayModeV29(SOURCE_ARRAY_MODE_V29.replace(mode));
        let _case = RestoreCase(SOURCE_ARRAY_CASE_V29.replace((64, 3, false)));
        with_original_scalar_array_plan_v29(|plan, budget| {
            let eligible = source_array_eligibility_v29(plan, budget)?;
            let mut checked = 0;
            for (cell, row) in plan.cells.rows.iter().enumerate() {
                if row.local.index() != 2 {
                    continue;
                }
                let (schema, facts) = source_array_cell_facts_v29(plan, cell, budget)?.unwrap();
                assert_eq!(facts.length, 3);
                if mode == SourceArrayModeV29::Scalar {
                    assert!(matches!(
                        facts.element.element,
                        PrivateRetainedElementFactsV1::Scalar(_)
                    ));
                    assert_eq!(eligible[cell], Some(schema));
                } else {
                    assert!(matches!(
                        facts.element.element,
                        PrivateRetainedElementFactsV1::ThinPointer { .. }
                    ));
                    assert_eq!(eligible[cell], None);
                }
                checked += 1;
            }
            assert!(
                checked >= 4,
                "both helper activations keep original layout facts"
            );
            Ok(())
        })
        .unwrap();
    }
}
