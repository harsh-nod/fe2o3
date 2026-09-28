thread_local! {
    static STATIC_FIELD_OPERAND_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static STATIC_FIELD_FAULT_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static STATIC_FIELD_OBSERVED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static STATIC_FIELD_MUTATED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

include!("production_source_object_lane_v29_tests.rs");

fn static_field_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let base = module_fixture_owner(ModuleFixture::Ordinary);
    let semantic = base.source_semantic();
    let prior = &semantic.functions()[0];
    let mut types = semantic.types().to_vec();
    let pair = declaration(
        &mut types,
        SemanticTypeLayoutV1::aggregate(
            Some(8),
            4,
            SemanticAggregateLayoutV1::new(vec![0, 4], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, U32]).unwrap()),
        None,
    );
    let mut locals = prior.locals().to_vec();
    assert_eq!(locals.len(), 2);
    locals.extend([
        local(70, pair, SemanticLocalRoleV1::Temporary),
        local(71, U32, SemanticLocalRoleV1::Temporary),
        local(72, U32, SemanticLocalRoleV1::Temporary),
    ]);
    let literal = |value| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            U32,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    };
    let field = |index| {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), U32).unwrap()],
            U32,
        )
        .unwrap()
    };
    let operand = match STATIC_FIELD_OPERAND_V29.get() {
        0 => literal(11),
        1 | 3 => SemanticOperandV1::Copy(place(4, U32)),
        2 | 4 => SemanticOperandV1::Move(place(4, U32)),
        _ => panic!("static field operand mode"),
    };
    let statements = vec![
        if STATIC_FIELD_OPERAND_V29.get() >= 3 {
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    place(4, U32),
                    literal(11),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            )
        } else {
            assign(place(4, U32), SemanticRvalueKindV1::Use(literal(11)))
        },
        assign(
            place(2, pair),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Tuple,
                    vec![operand, literal(17)],
                )
                .unwrap(),
            ),
        ),
        // Explicit source memory syntax retains the original aggregate; it is
        // not a synthetic borrow or an altered storage classification.
        SemanticStatementV1::new(
            source(),
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
    ];
    let root = function(
        60,
        SemanticFunctionRoleV1::KernelRoot,
        prior.abi().clone(),
        locals,
        vec![
            block(
                64,
                statements,
                prior.blocks()[0].terminator().kind().clone(),
            ),
            block(65, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(prior.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        types,
        vec![],
        vec![],
        vec![],
        vec![root],
        vec![SemanticCallableDeclV1::defined(ROOT)],
        vec![ROOT],
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

fn static_field_observer_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let lowered = emitted[instances.root().index()].as_mut().unwrap();
    let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
    let mut counts = [0; 3];
    let mut retained_counts = [0; 3];
    for row in &anchors.objects {
        let endpoint = static_field_endpoint_v29(row.role);
        let ScopedObjectIdentityV29::Local {
            instance, local, ..
        } = endpoint.object
        else {
            panic!("original local object");
        };
        assert_eq!(instance, instances.root());
        let counts = if local.index() == 4 {
            assert!(STATIC_FIELD_OPERAND_V29.get() >= 3);
            &mut retained_counts
        } else {
            assert_eq!(local.index(), 2);
            &mut counts
        };
        match row.role {
            ScopedObjectRoleV29::Project { .. } => counts[0] += 1,
            ScopedObjectRoleV29::ReadValue { .. } => counts[1] += 1,
            ScopedObjectRoleV29::WriteValue { .. } => counts[2] += 1,
            _ => panic!("unexpected static field family"),
        }
    }
    assert_eq!(counts, [4, 1, 3]);
    if STATIC_FIELD_OPERAND_V29.get() >= 3 {
        let slot = slots
            .slots
            .iter()
            .find(|slot| {
                slot.instance == instances.root()
                    && slot.origin.identity.original_local() == Some(4)
            })
            .unwrap();
        assert_eq!(
            retained_counts,
            match slot.representation {
                ScopedSlotRepresentationV29::Object { .. } => [0, 1, 1],
                ScopedSlotRepresentationV29::ScalarArray(_) => [0, 0, 0],
            }
        );
        let site = execution_site_v29(SemanticBlockIdV1::from_index(0), Some(1));
        let occurrences = instances.occurrences(instances.root()).unwrap();
        let declaration = instances.instance(instances.root()).unwrap().declaration();
        let mut read_count = 0;
        for (ordinal, row) in anchors.rows.iter().enumerate() {
            let Some((value, read)) = source_object_read_payload_v29(anchors, row, budget)? else {
                continue;
            };
            if read.site != site || read.role != ExecutionOperandV29::RvalueOperand(0) {
                continue;
            }
            read_count += 1;
            assert_eq!(read.ty, U32);
            assert_eq!(read.prefix, 0);
            assert!(matches!(
                read.occurrence,
                ScopedMemoryOccurrenceV29::Retained { .. }
            ));
            let body = lowered
                .function
                .body
                .as_ref()
                .unwrap()
                .blocks
                .iter()
                .find(|block| block.id == row.block)
                .unwrap();
            let actual = &body.operations[row.position];
            assert!(matches!(actual.results.as_slice(), [result] if result.id == value));
            match slot.representation {
                ScopedSlotRepresentationV29::Object { .. } => {
                    let payload = anchors.object_payload(row, budget)?;
                    payload.check_operation(actual, budget)?;
                    anchors.check_object_source(
                        declaration,
                        &occurrences,
                        ordinal,
                        row,
                        payload,
                        budget,
                    )?;
                }
                ScopedSlotRepresentationV29::ScalarArray(_) => {
                    assert!(matches!(row.kind, ScopedMemoryAnchorKindV29::Access { .. }));
                    check_scoped_payload_v29(declaration, &occurrences, row, actual, budget)?;
                }
            }
        }
        assert_eq!(read_count, 1);
    } else {
        assert_eq!(retained_counts, [0; 3]);
    }
    let fault = STATIC_FIELD_FAULT_V29.get();
    if fault != 0 {
        let body = lowered.function.body.as_mut().unwrap();
        let block = body
            .blocks
            .iter_mut()
            .find(|block| {
                block.operations.iter().any(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Storage(ScopedObjectOperationV29::Project { .. })
                    )
                })
            })
            .unwrap();
        match fault {
            1 => {
                let operation = block
                    .operations
                    .iter_mut()
                    .find(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::Storage(ScopedObjectOperationV29::Project {
                                step: ScopedObjectProjectionV29::Field(0),
                                ..
                            })
                        )
                    })
                    .unwrap();
                let OperationKind::Storage(ScopedObjectOperationV29::Project { step, .. }) =
                    &mut operation.kind
                else {
                    unreachable!()
                };
                *step = ScopedObjectProjectionV29::Field(1);
                let result = operation.results[0].id;
                let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
                let payload = anchors
                    .objects
                    .iter_mut()
                    .find(|payload| payload.result == Some(result))
                    .unwrap();
                let OperationKind::Storage(actual) = &operation.kind else {
                    unreachable!()
                };
                payload.operation = *actual;
                let ScopedObjectRoleV29::Project { projected, .. } = payload.role else {
                    unreachable!()
                };
                assert_eq!(projected.path.count, 1);
                let ScopedObjectComponentV29::View { projection, .. } =
                    &mut anchors.object_components[projected.path.first]
                else {
                    unreachable!()
                };
                *projection = ScopedObjectViewProjectionV29::Field(1);
            }
            2 => {
                let value = lowered
                    .scoped_memory_anchors
                    .as_ref()
                    .unwrap()
                    .objects
                    .iter()
                    .find_map(|row| match (row.operation, row.role) {
                        (
                            ScopedObjectOperationV29::WriteValue { value, .. },
                            ScopedObjectRoleV29::WriteValue {
                                destination:
                                    ScopedObjectEndpointV29 {
                                        source:
                                            ScopedObjectSourceV29::AggregateComponent {
                                                operand: 0, ..
                                            },
                                        ..
                                    },
                                ..
                            },
                        ) => Some(value),
                        _ => None,
                    })
                    .unwrap();
                let operation = block.operations.iter_mut().find(|operation|
                    matches!(operation.results.as_slice(), [result] if result.id == value)).unwrap();
                assert!(matches!(
                    operation.kind,
                    OperationKind::Constant(Constant::U32(11))
                ));
                operation.kind = OperationKind::Constant(Constant::U32(12));
            }
            3 => {
                // Every original aggregate component remains mandatory even
                // when an adversary removes every actual projection/effect.
                block
                    .operations
                    .retain(|operation| !matches!(operation.kind, OperationKind::Storage(_)));
                let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
                anchors
                    .rows
                    .retain(|row| !matches!(row.kind, ScopedMemoryAnchorKindV29::Object(_)));
                anchors.objects.clear();
                anchors.object_components.clear();
            }
            4 => {
                let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
                let row = anchors
                    .objects
                    .iter_mut()
                    .find(|row| {
                        matches!(
                            row.role,
                            ScopedObjectRoleV29::WriteValue {
                                destination: ScopedObjectEndpointV29 {
                                    source: ScopedObjectSourceV29::AggregateComponent {
                                        operand: 0,
                                        ..
                                    },
                                    ..
                                },
                                ..
                            }
                        )
                    })
                    .unwrap();
                let ScopedObjectRoleV29::WriteValue { destination, .. } = &mut row.role else {
                    unreachable!()
                };
                let ScopedObjectSourceV29::AggregateComponent { operand, .. } =
                    &mut destination.source
                else {
                    unreachable!()
                };
                *operand = 1;
            }
            5 => {
                let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
                let row = anchors
                    .objects
                    .iter_mut()
                    .find(|row| matches!(row.role, ScopedObjectRoleV29::Project { .. }))
                    .unwrap();
                let ScopedObjectRoleV29::Project { projected, .. } = &mut row.role else {
                    unreachable!()
                };
                let ScopedObjectIdentityV29::Local { generation, .. } = &mut projected.object
                else {
                    unreachable!()
                };
                *generation = u32::MAX;
            }
            6 => {
                let operation = block
                    .operations
                    .iter_mut()
                    .find(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::Storage(ScopedObjectOperationV29::Project { .. })
                        )
                    })
                    .unwrap();
                let Type::Pointer(pointer) = &mut operation.results[0].ty else {
                    unreachable!()
                };
                *pointer.pointee =
                    Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX));
            }
            7 => {
                let first = block
                    .operations
                    .iter()
                    .position(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::Storage(ScopedObjectOperationV29::WriteValue { .. })
                        )
                    })
                    .unwrap();
                let last = block
                    .operations
                    .iter()
                    .position(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. })
                        )
                    })
                    .unwrap();
                block.operations.swap(first, last);
            }
            8 => {
                assert!(STATIC_FIELD_OPERAND_V29.get() >= 3);
                let replacement = block
                    .operations
                    .iter()
                    .find_map(|operation| {
                        matches!(operation.kind, OperationKind::Constant(Constant::U32(17)))
                            .then(|| operation.results[0].id)
                    })
                    .unwrap();
                let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
                let (index, payload) = anchors
                    .objects
                    .iter_mut()
                    .enumerate()
                    .find(|(_, payload)| {
                        matches!(
                            payload.role,
                            ScopedObjectRoleV29::WriteValue {
                                destination: ScopedObjectEndpointV29 {
                                    source: ScopedObjectSourceV29::AggregateComponent {
                                        operand: 0,
                                        ..
                                    },
                                    ..
                                },
                                ..
                            }
                        )
                    })
                    .unwrap();
                let ScopedObjectOperationV29::WriteValue { value, .. } = &mut payload.operation
                else {
                    unreachable!()
                };
                assert_ne!(*value, replacement);
                *value = replacement;
                let row = anchors
                    .rows
                    .iter()
                    .find(|row| row.kind == ScopedMemoryAnchorKindV29::Object(index))
                    .unwrap();
                assert_eq!(row.block, block.id);
                // Coherent operation/payload mutation leaves the original
                // operand read receipt unchanged. Final read-source equality
                // must reject it, not merely operation/payload disagreement.
                block.operations[row.position].kind = OperationKind::Storage(payload.operation);
            }
            _ => panic!("static field fault"),
        }
        if fault == 3 {
            // The unchanged source spans still describe the deleted effects.
            // Assembly rejects this mismatch before any final field proof.
            let original = instances.instance(instances.root()).unwrap();
            assert!(matches!(
                instance_check_source_rows_with_control_v1(
                    instances,
                    instances.root(),
                    original.function(),
                    lowered,
                    budget,
                ),
                Err(InstanceCorrespondenceErrorV1::Source)
            ));
        }
        STATIC_FIELD_MUTATED_V29.set(true);
    }
    STATIC_FIELD_OBSERVED_V29.set(STATIC_FIELD_OBSERVED_V29.get() + 1);
    Ok(())
}

fn static_field_endpoint_v29(role: ScopedObjectRoleV29) -> ScopedObjectEndpointV29 {
    match role {
        ScopedObjectRoleV29::Project { source, .. }
        | ScopedObjectRoleV29::ReadValue { source, .. } => source,
        ScopedObjectRoleV29::WriteValue { destination, .. } => destination,
        _ => panic!("unexpected static field role"),
    }
}

fn run_static_field_admission_v29(
    mode: u8,
    fault: u8,
    work_limit: usize,
    storage_limit: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(u8, u8, Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            STATIC_FIELD_OPERAND_V29.set(self.0);
            STATIC_FIELD_FAULT_V29.set(self.1);
            SCOPED_SLOT_OBSERVER_V29.set(self.2);
        }
    }
    let _restore = Restore(
        STATIC_FIELD_OPERAND_V29.replace(mode),
        STATIC_FIELD_FAULT_V29.replace(fault),
        SCOPED_SLOT_OBSERVER_V29.replace(Some(static_field_observer_v29)),
    );
    STATIC_FIELD_OBSERVED_V29.set(0);
    STATIC_FIELD_MUTATED_V29.set(false);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = ArgumentBudgetV1::new(&mut work, storage_limit);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let mut completed = false;
    let result = (|| {
        let owner = static_field_owner_v29();
        let projection = static_field_owner_v29();
        assert_eq!(owner.identity(), projection.identity());
        assert_eq!(
            owner.source_semantic().semantic_sha256(),
            projection.source_semantic().semantic_sha256()
        );
        let (_, launch) =
            with_module_fixture_view(&owner, ModuleFixture::Ordinary, &mut budget, |_, _| ())?;
        let prepared = with_module_fixture_view(
            &projection,
            ModuleFixture::Ordinary,
            &mut budget,
            |source, budget| {
                ProductionPendingScopedSourceOwnerV29::prepare_source_with_budget_v18(
                    owner,
                    launch,
                    source.input,
                    ProductionSemanticKirLimitsV1::default(),
                    budget,
                )
            },
        )?
        .0?;
        prepared.with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
            source.with_analysis_v18(budget, |scope| scope.with_inventory_v1(|inventory, budget|
                source.with_ranked_correspondence_v18(inventory, budget, |relation, budget|
                    scoped_raw_admission_v29::with_checked_source_memory_v29(relation, 0, None, budget,
                        |physical, budget| -> SourceOwnedResultV18<()> {
                            let root = source.root(0, budget)?.1;
                            let body = inventory.functions()[root].function.body.as_ref().unwrap();
                            let mut counts = [0; 3];
                            for (block, body) in body.blocks.iter().enumerate() {
                                for (ordinal, operation) in body.operations.iter().enumerate() {
                                    let OperationKind::Storage(kind) = operation.kind else { continue; };
                                    let coordinate = fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1 {
                                        block: fe2o3_kernel_ir::CanonicalKirBlockCoordinateV1 {
                                            function: fe2o3_kernel_ir::CanonicalKirFunctionCoordinateV1(root as u32), block: block as u32 },
                                        operation: ordinal as u32 };
                                    let payload = relation.retained_object_payload_v29(0, coordinate, budget)?.unwrap();
                                    if matches!(static_field_endpoint_v29(payload.source.role).object,
                                        ScopedObjectIdentityV29::Local { local, .. } if local.index() == 4) {
                                        assert!(mode >= 3);
                                        assert!(!matches!(kind, ScopedObjectOperationV29::Project { .. }));
                                        continue;
                                    }
                                    let (address, endpoint) = match (kind, payload.source.role) {
                                        (ScopedObjectOperationV29::Project { .. }, ScopedObjectRoleV29::Project { .. }) => {
                                            counts[0] += 1;
                                            continue;
                                        }
                                        (ScopedObjectOperationV29::ReadValue { address, .. }, ScopedObjectRoleV29::ReadValue { source, .. }) => {
                                            counts[1] += 1;
                                            (address, source)
                                        }
                                        (ScopedObjectOperationV29::WriteValue { address, .. }, ScopedObjectRoleV29::WriteValue { destination, .. }) => {
                                            counts[2] += 1;
                                            (address, destination)
                                        }
                                        _ => panic!("actual/source field role mismatch"),
                                    };
                                    let ScopedObjectIdentityV29::Local { instance, local, generation } = endpoint.object
                                        else { panic!("exact direct source field object"); };
                                    assert_eq!((instance.index(), local.index(), generation), (payload.instance, 2, 0));
                                    let access = physical.access(payload.instance, payload.row, coordinate, address, budget)?
                                        .expect("exact gen0 direct field has an invocation activation");
                                    assert_eq!(access.operation_pointer(budget)?, (coordinate, address));
                                    let mut alternatives = 0;
                                    access.visit_alternatives(budget, |actual_instance, actual_local, slot, activation, _| {
                                        assert_eq!((actual_instance, actual_local), (instance.index(), local));
                                        assert!(activation.is_none(), "no StorageLive is present for the original aggregate");
                                        let row = &source.root_row(0)?.source_slots.slots[slot];
                                        assert_eq!(row.instance, instance);
                                        assert_eq!(row.origin.identity, ScopedAllocationIdentityV29::OriginalObject {
                                            local: local.index(), generation });
                                        assert!(matches!(row.representation, ScopedSlotRepresentationV29::Object { schema, .. }
                                            if schema == endpoint.root_schema));
                                        alternatives += 1;
                                        Ok(())
                                    })?;
                                    assert_eq!(alternatives, 1);
                                }
                            }
                            assert_eq!(counts, [4, 1, 3]);
                            completed = true;
                            Ok(())
                        }))))
        })
    })();
    assert_eq!(
        budget.storage(),
        MODULE_FLOOR,
        "mode {mode}, fault {fault}: {result:?}"
    );
    (result, budget.work(), budget.peak_storage(), completed)
}

#[test]
fn original_flat_scalar_fields_reach_final_memory_with_constant_copy_and_move_operands() {
    for mode in 0..=2 {
        let (result, _, _, completed) =
            run_static_field_admission_v29(mode, 0, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap_or_else(|error| panic!("mode {mode}: {error:?}"));
        assert!(completed);
        assert!(
            STATIC_FIELD_OBSERVED_V29.get() >= 2,
            "source admission and reconstruction both execute"
        );
    }
}

#[test]
fn original_flat_scalar_fields_reach_final_memory_with_retained_copy_and_move_reads() {
    for mode in 3..=4 {
        let (result, _, _, completed) =
            run_static_field_admission_v29(mode, 0, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap_or_else(|error| panic!("retained mode {mode}: {error:?}"));
        assert!(completed);
        assert!(STATIC_FIELD_OBSERVED_V29.get() >= 2);
    }
}

#[test]
fn retained_aggregate_field_value_must_equal_its_exact_original_read_result() {
    for mode in 3..=4 {
        let (positive, _, _, completed) =
            run_static_field_admission_v29(mode, 0, MODULE_LIMIT, MODULE_LIMIT);
        positive.unwrap();
        assert!(completed);
        let (result, _, _, completed) =
            run_static_field_admission_v29(mode, 8, MODULE_LIMIT, MODULE_LIMIT);
        assert!(STATIC_FIELD_MUTATED_V29.get());
        assert!(STATIC_FIELD_OBSERVED_V29.get() > 0);
        assert!(!completed);
        assert!(
            matches!(
                result,
                Err(ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::Unsupported {
                            detail: "typed object source payload differs from its actual operation",
                            ..
                        }
                    )
                ))
            ),
            "{result:?}"
        );
    }
}

#[test]
fn original_flat_scalar_fields_reject_same_candidate_field_value_effect_source_generation_schema_and_order_changes()
 {
    for fault in 1..=7 {
        let (positive, _, _, completed) =
            run_static_field_admission_v29(0, 0, MODULE_LIMIT, MODULE_LIMIT);
        positive.unwrap();
        assert!(completed);
        let (result, _, _, completed) =
            run_static_field_admission_v29(0, fault, MODULE_LIMIT, MODULE_LIMIT);
        assert!(STATIC_FIELD_MUTATED_V29.get());
        assert!(
            STATIC_FIELD_OBSERVED_V29.get() > 0,
            "all mutation assertions returned"
        );
        assert!(!completed);
        match result {
            Err(ProductionSourceOwnedViewErrorV18::Binding(
                "aggregate literal differs from its original constant",
            )) if fault == 2 => {}
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        function: 0,
                        block: None,
                        statement: None,
                        detail: "execution call parameters differ from their source instance",
                    },
                ),
            )) if fault == 3 => {}
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported { detail, .. },
                ),
            )) if fault != 2 && fault != 3 => {
                assert!(
                    matches!(
                        detail,
                        "typed object source payload differs from its actual operation"
                            | "original typed aggregate field effect is missing or duplicated"
                            | "source raw address differs from its actual formation or memory history"
                    ),
                    "fault {fault}: {detail}"
                );
            }
            other => panic!("exact same-candidate semantic refusal: fault {fault}: {other:?}"),
        }
    }
}

#[test]
fn original_flat_scalar_field_final_admission_exact_measured_work_and_storage_boundaries() {
    for mode in [0, 3] {
        let (result, work, storage, completed) =
            run_static_field_admission_v29(mode, 0, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap();
        assert!(completed);
        let (result, _, _, completed) = run_static_field_admission_v29(mode, 0, work, storage);
        result.unwrap();
        assert!(completed);
        for (work, storage, work_short) in [(work - 1, storage, true), (work, storage - 1, false)] {
            // A one-short postflight may run after the callback. Only a fully
            // completed consuming scope publishes success, so check its exact error.
            let (result, _, _, _) = run_static_field_admission_v29(mode, 0, work, storage);
            let error = match result {
                Err(ProductionSourceOwnedViewErrorV18::Resource(error)) => error,
                Err(ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
                    ),
                )) => error,
                other => panic!("exact resource refusal: {other:?}"),
            };
            if work_short {
                assert!(matches!(error, ArgumentResourceV1::Work(_)));
            } else {
                assert!(matches!(error, ArgumentResourceV1::Storage(_)));
            }
        }
    }
}
