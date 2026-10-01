use fe2o3_mir_model::semantic_mir_v1::*;

#[test]
fn object_operand_locator_keeps_exact_copy_move_store_roles_and_original_place_identity() {
    let owners = [
        selected_pointer_test_owner_v29(entrance_control_owner(false), false, 1),
        projected_pointer_test_owner_v29(entrance_control_owner(false), false, 0),
    ];
    let (mut copies, mut moves, mut stores, mut projections) = (0, 0, 0, 0);
    for owner in owners {
        for function in owner.source_semantic().functions() {
            for (block, body) in function.blocks().iter().enumerate() {
                for (ordinal, statement) in body.statements().iter().enumerate() {
                    let (role, operand) = match statement.kind() {
                        SemanticStatementKindV1::Assign(assignment) => {
                            match assignment.value().kind() {
                                SemanticRvalueKindV1::Use(operand) => {
                                    (ExecutionOperandV29::RvalueOperand(0), operand)
                                }
                                _ => continue,
                            }
                        }
                        SemanticStatementKindV1::Store(store) => {
                            (ExecutionOperandV29::StoreValue, store.value())
                        }
                        _ => continue,
                    };
                    let (SemanticOperandV1::Copy(original) | SemanticOperandV1::Move(original)) =
                        operand
                    else {
                        continue;
                    };
                    let site = execution_site_v29(
                        SemanticBlockIdV1::from_index(block as u32),
                        Some(ordinal as u32),
                    );
                    let found = scoped_object_original_place_v29(function, site, role).unwrap();
                    assert!(std::ptr::eq(found, original));
                    assert!(
                        scoped_source_place_v29(function, site, role).is_none(),
                        "a statement-place-only query cannot identify a Copy/Move operand"
                    );
                    let cloned = original.clone();
                    assert_eq!(&cloned, original);
                    assert!(
                        !std::ptr::eq(found, &cloned),
                        "equal metadata is not source location identity"
                    );
                    let wrong_role = if role == ExecutionOperandV29::StoreValue {
                        ExecutionOperandV29::StoreDestination
                    } else {
                        ExecutionOperandV29::Destination
                    };
                    assert!(
                        !scoped_object_original_place_v29(function, site, wrong_role)
                            .is_some_and(|place| std::ptr::eq(place, original))
                    );
                    let wrong_site = execution_site_v29(
                        SemanticBlockIdV1::from_index(block as u32),
                        Some(ordinal as u32 + 1),
                    );
                    assert!(
                        !scoped_object_original_place_v29(function, wrong_site, role)
                            .is_some_and(|place| std::ptr::eq(place, original))
                    );
                    if matches!(operand, SemanticOperandV1::Move(_)) {
                        moves += 1;
                    } else {
                        copies += 1;
                    }
                    stores += usize::from(role == ExecutionOperandV29::StoreValue);
                    projections += usize::from(!original.projections().is_empty());
                }
            }
        }
    }
    assert!(copies > 1 && moves > 0 && stores > 0 && projections > 0);
}

const SELECTED_POINTER_STOP: &str = "selected pointer producer and transport observed";
#[test]
fn source_array_assignment_indices_are_exact_bounded_and_allocation_free() {
    let element = SemanticTypeIdV1::from_index(2);
    for length in [1, 2, 31, 32, 64, 128, u64::MAX] {
        for from_end in [false, true] {
            let projection = SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: if from_end { 1 } else { length - 1 },
                    minimum_length: length,
                    from_end,
                },
                element,
            )
            .unwrap();
            for short in [0, 1] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(6 - short);
                let mut budget = ArgumentBudgetV1::new(&mut work, 0);
                let result = source_reference_array_assignment_index_v29(
                    element,
                    length,
                    projection,
                    &mut budget,
                );
                if short == 0 {
                    assert_eq!(
                        result.unwrap(),
                        (
                            usize::try_from(length - 1).unwrap(),
                            usize::try_from(length).unwrap()
                        )
                    );
                    assert_eq!(budget.work(), 6);
                } else {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                }
                assert_eq!((budget.storage(), budget.peak_storage()), (0, 0));
            }
        }
    }
    for (length, offset, minimum_length, from_end, result_type) in [
        (0, 0, 1, false, element),
        (2, 2, 3, false, element),
        (2, 3, 3, true, element),
        (2, 1, 3, false, element),
        (2, 1, 2, false, SemanticTypeIdV1::from_index(3)),
    ] {
        let projection = SemanticProjectionV1::new(
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length,
                from_end,
            },
            result_type,
        )
        .unwrap();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(6);
        let mut budget = ArgumentBudgetV1::new(&mut work, 0);
        assert!(matches!(
            source_reference_array_assignment_index_v29(element, length, projection, &mut budget),
            Err(ProductionSemanticKirErrorV1::Unsupported {
                detail: "source array assignment element differs from its declaration",
                ..
            })
        ));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (6, 0, 0)
        );
    }
}

#[test]
fn original_array_element_writes_retain_immutable_pointer_snapshots() {
    for immutable in [false, true] {
        for mode in [2, 3, 5] {
            let mut reached = false;
            let result = with_selected_pointer_test_plan_v29(
                projected_pointer_test_owner_v29(entrance_control_owner(false), immutable, mode),
                |plan, _| {
                    let mut initial = false;
                    let mut updated = false;
                    let array = plan
                        .instances
                        .instance(plan.root)
                        .unwrap()
                        .declaration()
                        .locals()[13]
                        .ty();
                    for (root, row) in plan.nodes.iter().enumerate() {
                        if row.ty != array {
                            continue;
                        }
                        let SourceReferenceNodeKindV29::Aggregate { first, count: 2 } = row.kind
                        else {
                            continue;
                        };
                        let targets = [first, first + 1].map(|index| {
                            let child = plan.children[index];
                            assert!(child < root);
                            let SourceReferenceNodeKindV29::Address(set) = plan.nodes[child].kind
                            else {
                                return None;
                            };
                            let set = plan.raw_sets[set];
                            assert_eq!(set.count, 1);
                            let origin = plan.raw_choices[set.first].origin;
                            Some(plan.raw_origins[origin].local.index())
                        });
                        // The first pointer addresses local 2, not argument 1
                        // whose scalar value was copied into that local.
                        initial |= targets == [Some(2), Some(9)];
                        updated |= targets == [Some(2), Some(2)];
                    }
                    assert!(
                        initial && updated,
                        "mode {mode}, immutable {immutable}: distinct original snapshots"
                    );
                    reached = true;
                    Ok(())
                },
            );
            assert!(
                result.is_ok() && reached,
                "mode {mode}, immutable {immutable}: {result:?}"
            );
        }
    }
}

#[test]
fn original_array_element_writes_do_not_initialize_untouched_pointer_siblings() {
    let original_owner = projected_pointer_test_owner_v29(entrance_control_owner(false), false, 2);
    let source = original_owner.source_semantic();
    let original = &source.functions()[0];
    let statements = original.blocks()[0]
        .statements()
        .iter()
        .filter(|statement| {
            let destination = match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => assignment.destination(),
                SemanticStatementKindV1::Store(store) => store.destination(),
                _ => return true,
            };
            let sibling = destination.local().index() == 13
                && destination.projections().iter().any(|projection| {
                    matches!(
                        projection.kind(),
                        SemanticProjectionKindV1::ConstantIndex { offset: 1, .. }
                    )
                });
            let alias_write =
                destination.local().index() == 4 && !destination.projections().is_empty();
            !sibling && !alias_write
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        original.blocks()[0].statements().len() - statements.len(),
        3
    );
    let function = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        vec![
            SemanticBasicBlockV1::new(
                original.blocks()[0].identity(),
                original.blocks()[0].source(),
                statements,
                original.blocks()[0].terminator().clone(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
    .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        source.target(),
        source.types().to_vec(),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticCallableDeclV1::defined(
            SemanticFunctionIdV1::from_index(0),
        )],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let owner = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let mut reached = false;
    let result = with_selected_pointer_test_plan_v29(owner, |_, _| {
        reached = true;
        Ok(())
    });
    assert!(
        !reached && result.is_err(),
        "an untouched pointer sibling has no original read authority: {result:?}"
    );
}

thread_local! {
    static SELECTED_POINTER_FIXTURE: std::cell::Cell<(bool, u8)> = const { std::cell::Cell::new((false, 0)) };
    static SELECTED_POINTER_OBSERVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static SELECTED_POINTER_EXIT: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static SELECTED_POINTER_LAYOUTS: std::cell::RefCell<Vec<fe2o3_kernel_ir::StorageLayoutV1>> = const { std::cell::RefCell::new(Vec::new()) };
}

fn selected_pointer_emission_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let (immutable, graph) = SELECTED_POINTER_FIXTURE.get();
    selected_pointer_test_owner_v29(entrance_control_owner(false), immutable, graph)
}

fn selected_pointer_structural_layouts_v29() {
    pointer_structural_layouts_v29(selected_pointer_emission_owner_v29);
}

fn pointer_structural_layouts_v29(factory: fn() -> ProductionSemanticSsaOwnerV1) {
    with_selected_pointer_test_plan_v29(factory(), |plan, budget| {
        let layouts = plan
            .storage_root
            .as_ref()
            .unwrap()
            .source_layouts(plan.instances, budget)?;
        SELECTED_POINTER_LAYOUTS.set(layouts.rows(plan.instances.owner(), budget)?.to_vec());
        Ok(())
    })
    .unwrap_or_else(|error| {
        panic!(
            "selected {:?}, projected {:?}: {error:?}",
            SELECTED_POINTER_FIXTURE.get(),
            PROJECTED_POINTER_FIXTURE.get()
        )
    });
}

const PROJECTED_POINTER_STOP: &str = "projected pointer producer observed";
thread_local! {
    static PROJECTED_POINTER_FIXTURE: std::cell::Cell<(bool, u8)> = const { std::cell::Cell::new((false, 0)) };
    static PROJECTED_POINTER_OBSERVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static PROJECTED_POINTER_MUTATION: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

fn projected_pointer_emission_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let (immutable, mode) = PROJECTED_POINTER_FIXTURE.get();
    projected_pointer_test_owner_v29(entrance_control_owner(false), immutable, mode)
}

#[test]
fn selected_and_projected_pointer_fixture_abis_admit_before_emission() {
    for immutable in [false, true] {
        for graph in 0..4 {
            let owner =
                selected_pointer_test_owner_v29(entrance_control_owner(false), immutable, graph);
            assert_eq!(owner.source_semantic().types().len(), 4);
            assert_eq!(
                owner.source_semantic().functions().len(),
                if graph == 3 { 1 } else { 2 }
            );
        }
        for mode in 0..6 {
            let owner =
                projected_pointer_test_owner_v29(entrance_control_owner(false), immutable, mode);
            assert_eq!(owner.source_semantic().types().len(), 7);
        }
    }
}

fn projected_pointer_verify_v29(module: &Module) -> bool {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    let checked = fe2o3_kernel_ir::check_module_storage_v1(
        module,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )
    .unwrap();
    let valid =
        fe2o3_kernel_ir::verify_storage_module_ref_with_budget_v1(checked, None, &mut budget)
            .is_ok();
    assert_eq!(budget.storage(), 0);
    valid
}

fn observe_projected_pointer_emission_v29(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let root = emitted[instances.root().index()].as_ref().unwrap();
    let body = root.function.body.as_ref().unwrap();
    assert_eq!(body.blocks.len(), 1);
    let block = &body.blocks[0];
    let mut objects = std::collections::BTreeMap::new();
    for slot in &slots.slots {
        let ScopedSlotRepresentationV29::Object { schema, .. } = slot.representation else {
            continue;
        };
        let ScopedAllocationIdentityV29::OriginalObject { local, generation } =
            slot.origin.identity
        else {
            panic!("original generation");
        };
        assert_eq!(generation, 0);
        check_scoped_slot_alloca_v29(
            slot,
            &body.blocks[slot.allocation.block_ordinal].operations[slot.allocation.operation],
            budget,
        )?;
        assert!(
            objects
                .insert(local, (slot.origin.pointer, schema))
                .is_none()
        );
    }
    assert_eq!(objects.keys().copied().collect::<Vec<_>>(), [2, 9, 11, 13]);
    let mut types = std::collections::BTreeMap::new();
    for (&id, ty) in body
        .parameters
        .iter()
        .zip(&root.function.signature.parameters)
    {
        types.insert(id, ty.clone());
    }
    for value in &block.parameters {
        types.insert(value.id, value.ty.clone());
    }
    for operation in &block.operations {
        for result in &operation.results {
            assert!(types.insert(result.id, result.ty.clone()).is_none());
        }
    }
    let layouts = SELECTED_POINTER_LAYOUTS.with(|rows| rows.borrow().clone());
    let mut projections = 0;
    let mut pointer_reads = 0;
    let mut pointer_writes = Vec::new();
    let mut aliases = 0;
    for (ordinal, operation) in block.operations.iter().enumerate() {
        assert!(!matches!(
            operation.kind,
            OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                ..
            }
        ));
        match operation.kind {
            OperationKind::Storage(ScopedObjectOperationV29::Project { base, step }) => {
                assert_eq!(operation.results.len(), 1);
                let Type::Pointer(source) = &types[&base] else {
                    panic!("typed project base");
                };
                let Type::Pointer(result) = &operation.results[0].ty else {
                    panic!("typed project result");
                };
                assert_eq!(
                    (source.address_space, source.access),
                    (result.address_space, result.access)
                );
                assert!(
                    matches!(*source.pointee, Type::StorageObject(_))
                        && matches!(*result.pointee, Type::StorageObject(_))
                );
                if let ScopedObjectProjectionV29::ArrayIndex(index) = step {
                    assert_eq!(types[&index], Type::INDEX);
                    assert!(block.operations[..ordinal].iter().any(|operation| matches!(
                        operation.kind,
                        OperationKind::Constant(Constant::Index(0 | 1))
                    )
                        && operation.results[0].id == index));
                }
                projections += 1;
            }
            OperationKind::Storage(ScopedObjectOperationV29::ReadValue { .. })
                if operation
                    .results
                    .first()
                    .is_some_and(|result| matches!(result.ty, Type::Pointer(_))) =>
            {
                pointer_reads += 1
            }
            OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address,
                value,
                access,
            }) => {
                let Type::Pointer(destination) = &types[&address] else {
                    panic!("typed write address");
                };
                let Type::StorageObject(schema) = *destination.pointee else {
                    panic!("selected leaf schema");
                };
                assert_eq!(destination.access, AccessMode::ReadWrite);
                assert_eq!(access.address_space, AddressSpace::Private);
                if let fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(pointer) =
                    layouts[schema.0 as usize].kind
                {
                    assert_eq!(
                        types[&value],
                        Type::pointer(
                            Type::StorageObject(pointer.pointee),
                            pointer.value_space,
                            pointer.access
                        )
                    );
                    pointer_writes.push(ordinal);
                }
            }
            OperationKind::Select {
                condition,
                true_value,
                false_value,
            } => {
                assert_eq!(true_value, false_value);
                assert_ne!(operation.results[0].id, true_value);
                assert_eq!(
                    block.operations[ordinal - 1].kind,
                    OperationKind::Constant(Constant::Bool(true))
                );
                assert_eq!(block.operations[ordinal - 1].results[0].id, condition);
                aliases += 1;
            }
            OperationKind::Cast {
                kind: CastKind::RestrictPointerAccess,
                value,
                ..
            } => {
                assert_ne!(operation.results[0].id, value);
                aliases += 1;
            }
            _ => {}
        }
    }
    assert!(projections >= 5 && pointer_reads >= 1 && pointer_writes.len() >= 5 && aliases >= 6);
    let anchors = root.scoped_memory_anchors.as_ref().unwrap();
    assert_eq!(
        anchors.objects.len(),
        block
            .operations
            .iter()
            .filter(|operation| matches!(operation.kind, OperationKind::Storage(_)))
            .count()
    );
    let mut original_projects = 0;
    let mut explicit_stores = 0;
    for row in &anchors.objects {
        if let ScopedObjectRoleV29::Project { source, projected } = row.role {
            assert_eq!(source.object, projected.object);
            assert_eq!(projected.path.count, source.path.count + 1);
            let ScopedObjectSourceV29::Place {
                site,
                role,
                local,
                prefix,
            } = projected.source
            else {
                panic!("original projected place");
            };
            let original = scoped_object_original_place_v29(
                instances.instance(instances.root()).unwrap().declaration(),
                site,
                role,
            )
            .unwrap();
            assert_eq!(local, original.local());
            let path = anchors.object_path(projected.source_path, budget)?;
            assert_eq!(path.len(), prefix as usize);
            for (component, projection) in
                path.iter().zip(&original.projections()[..prefix as usize])
            {
                assert!(
                    matches!(component, ScopedObjectComponentV29::Original { projection: actual, selector: None } if actual == projection)
                );
            }
            original_projects += 1;
        }
        if matches!(
            row.role,
            ScopedObjectRoleV29::WriteValue {
                value: ScopedObjectValueOriginV29::Original(ScopedMemoryStoreSourceV29::Operand {
                    role: ExecutionOperandV29::StoreValue,
                    ..
                }),
                ..
            }
        ) {
            explicit_stores += 1;
        }
    }
    assert_eq!(original_projects, projections);
    assert_eq!(explicit_stores, 1);
    let mut module = Module::new("projected-pointer-component");
    module.storage_layouts = layouts;
    module.functions = emitted
        .iter()
        .flatten()
        .map(|lowered| lowered.function.clone())
        .collect();
    module.kernels.push(fe2o3_kernel_ir::Kernel::new(
        "projected-pointer-component",
        root.function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    assert!(
        projected_pointer_verify_v29(&module),
        "unchanged actual bodies must independently verify"
    );
    let mutation = PROJECTED_POINTER_MUTATION.get();
    if mutation != 0 {
        let function = &mut module.functions[0];
        let body = function.body.as_mut().unwrap();
        let ordinal = pointer_writes[0];
        if mutation == 1 {
            let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { value, .. }) =
                &mut body.blocks[0].operations[ordinal].kind
            else {
                unreachable!()
            };
            *value = body.parameters[0];
        } else {
            let OperationKind::Storage(ScopedObjectOperationV29::WriteValue { address, .. }) =
                body.blocks[0].operations[ordinal].kind
            else {
                unreachable!()
            };
            let Type::Pointer(pointer) = &types[&address] else {
                unreachable!()
            };
            let read_only = Type::pointer(
                *pointer.pointee.clone(),
                pointer.address_space,
                AccessMode::ReadOnly,
            );
            let id = ValueId(types.keys().map(|id| id.0).max().unwrap() + 1);
            let OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                address: destination,
                ..
            }) = &mut body.blocks[0].operations[ordinal].kind
            else {
                unreachable!()
            };
            *destination = id;
            body.blocks[0].operations.insert(
                ordinal,
                Operation::new(
                    vec![ValueDef::new(id, read_only.clone())],
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        value: address,
                        to: read_only,
                    },
                ),
            );
        }
        assert!(
            !projected_pointer_verify_v29(&module),
            "wrong selected value type or read-only store address must refuse"
        );
    }
    PROJECTED_POINTER_OBSERVED.set(PROJECTED_POINTER_OBSERVED.get() + 1);
    Err(unsupported(0, None, None, PROJECTED_POINTER_STOP))
}

#[test]
fn projected_raw_pointer_stores_and_address_of_reach_original_observer() {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_projected_pointer_emission_v29)));
    for immutable in [false, true] {
        for mode in 0..9 {
            PROJECTED_POINTER_FIXTURE.set((immutable, mode));
            PROJECTED_POINTER_MUTATION.set(0);
            PROJECTED_POINTER_OBSERVED.set(0);
            pointer_structural_layouts_v29(projected_pointer_emission_owner_v29);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared =
                scalar_payload_prepared_from_v18(projected_pointer_emission_owner_v29, &mut budget);
            let result = prepared.with_source_consumer_v18(
                &mut budget,
                |_, _| -> SourceOwnedResultV18<()> {
                    panic!("final completion remains gated");
                },
            );
            assert_eq!(
                PROJECTED_POINTER_OBSERVED.get(),
                1,
                "immutable {immutable}, mode {mode}: {result:?}"
            );
            assert!(format!("{:?}", result.unwrap_err()).contains(PROJECTED_POINTER_STOP));
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn projected_pointer_actual_candidate_refuses_wrong_value_type_and_readonly_store() {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_projected_pointer_emission_v29)));
    for mutation in [1, 2] {
        PROJECTED_POINTER_FIXTURE.set((false, 1));
        PROJECTED_POINTER_MUTATION.set(mutation);
        PROJECTED_POINTER_OBSERVED.set(0);
        pointer_structural_layouts_v29(projected_pointer_emission_owner_v29);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared =
            scalar_payload_prepared_from_v18(projected_pointer_emission_owner_v29, &mut budget);
        let result =
            prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
                panic!("final completion remains gated");
            });
        assert_eq!(PROJECTED_POINTER_OBSERVED.get(), 1, "{result:?}");
        assert!(format!("{:?}", result.unwrap_err()).contains(PROJECTED_POINTER_STOP));
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

fn observe_selected_pointer_emission_v29(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    assert_eq!(
        slots.retained_storage,
        slots.instances.capacity() * std::mem::size_of::<ScopedSourceSlotInstanceV29>()
            + slots.slots.capacity() * std::mem::size_of::<ScopedSourceSlotV29>(),
        "completed source-slot inventory owns vectors, not dead object-query envelopes",
    );
    let (immutable, graph) = SELECTED_POINTER_FIXTURE.get();
    let mut objects = std::collections::BTreeMap::new();
    for slot in &slots.slots {
        let ScopedSlotRepresentationV29::Object { schema, .. } = slot.representation else {
            continue;
        };
        let ScopedAllocationIdentityV29::OriginalObject { local, generation } =
            slot.origin.identity
        else {
            panic!("original allocation");
        };
        assert_eq!(slot.instance, instances.root());
        assert!(matches!(local, 2 | 3));
        assert_eq!(generation, 0);
        let lowered = emitted[slot.instance.index()].as_ref().unwrap();
        check_scoped_slot_alloca_v29(
            slot,
            &lowered.function.body.as_ref().unwrap().blocks[slot.allocation.block_ordinal]
                .operations[slot.allocation.operation],
            budget,
        )?;
        assert!(
            objects
                .insert(local, (slot.origin.pointer, schema))
                .is_none()
        );
    }
    assert_eq!(objects.len(), if graph == 3 { 1 } else { 2 });
    let scalar_pointer = Type::pointer(
        Type::StorageObject(objects[&2].1),
        AddressSpace::Private,
        if immutable {
            AccessMode::ReadOnly
        } else {
            AccessMode::ReadWrite
        },
    );
    let mut helper_ids = std::collections::BTreeSet::new();
    let mut calls = Vec::new();
    let mut aliases = Vec::new();
    for (instance, lowered) in emitted
        .iter()
        .enumerate()
        .filter_map(|(index, value)| value.as_ref().map(|value| (index, value)))
    {
        let function = &lowered.function;
        let body = function.body.as_ref().unwrap();
        let mut types = std::collections::BTreeMap::new();
        for (&id, ty) in body.parameters.iter().zip(&function.signature.parameters) {
            assert!(types.insert(id, ty).is_none());
        }
        for block in &body.blocks {
            for value in block.parameters.iter().chain(
                block
                    .operations
                    .iter()
                    .flat_map(|operation| &operation.results),
            ) {
                assert!(
                    types.insert(value.id, &value.ty).is_none(),
                    "one physical definition per SSA ID"
                );
            }
        }
        if instance != instances.root().index() {
            assert_eq!(
                instances
                    .instance(ProductionCallInstanceIdV1(instance))
                    .unwrap()
                    .function()
                    .index(),
                1
            );
            assert_eq!(function.signature.parameters, [scalar_pointer.clone()]);
            assert_eq!(function.signature.results, [scalar_pointer.clone()]);
            assert!(helper_ids.insert(function.id.clone()));
        }
        for block in &body.blocks {
            for (ordinal, operation) in block.operations.iter().enumerate() {
                assert!(
                    !matches!(
                        operation.kind,
                        OperationKind::Cast {
                            kind: CastKind::PointerToGeneric,
                            ..
                        }
                    ),
                    "selected representation must not be recovered from a Generic cast"
                );
                match &operation.kind {
                    OperationKind::Call { callee, arguments } => {
                        let target = emitted
                            .iter()
                            .flatten()
                            .find(|lowered| &lowered.function.id == callee)
                            .unwrap();
                        assert_eq!(arguments.len(), target.function.signature.parameters.len());
                        assert_eq!(
                            operation.results.len(),
                            target.function.signature.results.len()
                        );
                        for (argument, ty) in
                            arguments.iter().zip(&target.function.signature.parameters)
                        {
                            assert_eq!(types[argument], ty);
                        }
                        for (result, ty) in operation
                            .results
                            .iter()
                            .zip(&target.function.signature.results)
                        {
                            assert_eq!(&result.ty, ty);
                        }
                        assert_eq!(operation.results.len(), 1);
                        assert_ne!(operation.results[0].id, arguments[0]);
                        calls.push((callee.clone(), arguments[0], operation.results[0].id));
                    }
                    OperationKind::Select {
                        condition,
                        true_value,
                        false_value,
                    } if objects.values().any(|(pointer, _)| pointer == true_value) => {
                        assert_eq!(instance, instances.root().index());
                        assert_eq!(true_value, false_value);
                        assert_eq!(operation.results.len(), 1);
                        assert_ne!(operation.results[0].id, *true_value);
                        let previous = &block.operations[ordinal.checked_sub(1).unwrap()];
                        assert_eq!(previous.kind, OperationKind::Constant(Constant::Bool(true)));
                        assert_eq!(
                            previous.results,
                            [ValueDef::new(*condition, Type::Scalar(ScalarType::Bool))]
                        );
                        assert_eq!(types[true_value], &operation.results[0].ty);
                        aliases.push((
                            block.id,
                            ordinal - 1,
                            ordinal + 1,
                            *true_value,
                            operation.results[0].id,
                        ));
                    }
                    OperationKind::Cast {
                        kind: CastKind::RestrictPointerAccess,
                        value,
                        to,
                    } if objects.values().any(|(pointer, _)| pointer == value) => {
                        assert!(immutable);
                        assert_eq!(*value, objects[&2].0);
                        assert_eq!(to, &scalar_pointer);
                        assert_eq!(operation.results.len(), 1);
                        assert_ne!(operation.results[0].id, *value);
                        aliases.push((
                            block.id,
                            ordinal,
                            ordinal + 1,
                            *value,
                            operation.results[0].id,
                        ));
                    }
                    _ => {}
                }
            }
            let edge = |target: BlockId, arguments: &[ValueId]| {
                let target = body.blocks.iter().find(|block| block.id == target).unwrap();
                assert_eq!(arguments.len(), target.parameters.len());
                for (argument, parameter) in arguments.iter().zip(&target.parameters) {
                    assert_eq!(types[argument], &parameter.ty);
                }
            };
            match block.terminator.as_ref().expect("complete emitted block") {
                Terminator::Branch { target, arguments } => edge(*target, arguments),
                Terminator::ConditionalBranch {
                    then_target,
                    then_arguments,
                    else_target,
                    else_arguments,
                    ..
                } => {
                    edge(*then_target, then_arguments);
                    edge(*else_target, else_arguments);
                }
                Terminator::Switch {
                    cases,
                    default_target,
                    default_arguments,
                    ..
                } => {
                    for case in cases {
                        edge(case.target, &case.arguments);
                    }
                    edge(*default_target, default_arguments);
                }
                Terminator::IntegerSwitch {
                    cases,
                    default_target,
                    default_arguments,
                    ..
                } => {
                    for case in cases {
                        edge(case.target, &case.arguments);
                    }
                    edge(*default_target, default_arguments);
                }
                Terminator::Return { values } => {
                    assert_eq!(values.len(), function.signature.results.len());
                    for (value, ty) in values.iter().zip(&function.signature.results) {
                        assert_eq!(types[value], ty);
                    }
                }
                Terminator::Unreachable => panic!("fixture has no unreachable source branch"),
            }
        }
    }
    assert_eq!(helper_ids.len(), if graph == 3 { 0 } else { 2 });
    assert_eq!(calls.len(), helper_ids.len());
    if calls.len() == 2 {
        assert_ne!(calls[0].0, calls[1].0);
        assert_ne!(calls[0].2, calls[1].2);
    }
    assert_eq!(aliases.len(), objects.len());
    let root = emitted[instances.root().index()].as_ref().unwrap();
    for &(block, first, end, base, result) in &aliases {
        let spans: Vec<_> = root
            .statement_operation_spans
            .iter()
            .filter(|span| {
                span.kernel_ir_block == block
                    && span.first_operation_ordinal as usize <= first
                    && span.first_operation_ordinal as usize + span.operation_count as usize >= end
            })
            .collect();
        assert_eq!(
            spans.len(),
            1,
            "complete fresh-alias interval belongs to one original statement"
        );
        let source = instances.instance(instances.root()).unwrap().declaration();
        let SemanticStatementKindV1::Assign(assignment) = source.blocks()
            [spans[0].semantic_block.index() as usize]
            .statements()[spans[0].statement_ordinal as usize]
            .kind()
        else {
            panic!("original address assignment");
        };
        let SemanticRvalueKindV1::AddressOf { place, .. } = assignment.value().kind() else {
            panic!("original AddressOf");
        };
        assert_eq!(objects[&place.local().index()].0, base);
        assert_ne!(base, result);
    }
    let anchors = root.scoped_memory_anchors.as_ref().unwrap();
    let mut holder_reads = 0;
    let mut indirect_reads = 0;
    for row in &anchors.objects {
        let endpoint = match row.role {
            ScopedObjectRoleV29::ReadValue { source, .. } => {
                if matches!(source.object, ScopedObjectIdentityV29::Local { local, .. } if local.index() == 3)
                {
                    holder_reads += 1;
                }
                if matches!(source.object, ScopedObjectIdentityV29::Reference { .. }) {
                    indirect_reads += 1;
                }
                source
            }
            ScopedObjectRoleV29::WriteValue { destination, .. } => destination,
            _ => panic!("fixture only has scalar/pointer object effects"),
        };
        if let ScopedObjectIdentityV29::Local {
            instance,
            local,
            generation,
        } = endpoint.object
        {
            assert_eq!(instance, instances.root());
            assert_eq!(generation, 0);
            assert_eq!(endpoint.root_schema, objects[&local.index()].1);
        }
    }
    if graph != 3 {
        assert!(holder_reads >= 2);
        assert!(indirect_reads >= 2);
    }
    // This independent physical module uses unchanged emitted bodies. Its
    // freshly selected inert layout table and launch domain confer no source
    // custody, alias lifetime, currentness, or final production authority.
    let mut module = Module::new("selected-pointer-component-structure");
    module.storage_layouts = SELECTED_POINTER_LAYOUTS.with(|rows| rows.borrow().clone());
    module.functions = emitted
        .iter()
        .flatten()
        .map(|lowered| lowered.function.clone())
        .collect();
    module.kernels.push(fe2o3_kernel_ir::Kernel::new(
        "selected-pointer-component",
        root.function.id.clone(),
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    let mut structural_work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut structural_budget = ArgumentBudgetV1::new(&mut structural_work, MODULE_LIMIT);
    let checked = fe2o3_kernel_ir::check_module_storage_v1(
        &module,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut structural_budget,
    )
    .unwrap();
    fe2o3_kernel_ir::verify_storage_module_ref_with_budget_v1(
        checked,
        None,
        &mut structural_budget,
    )
    .unwrap();
    assert_eq!(structural_budget.storage(), 0);
    SELECTED_POINTER_OBSERVED.set(SELECTED_POINTER_OBSERVED.get() + 1);
    match SELECTED_POINTER_EXIT.get() {
        0 => Err(unsupported(0, None, None, SELECTED_POINTER_STOP)),
        1 => Ok(()),
        2 => panic!("selected pointer observer panic"),
        _ => unreachable!(),
    }
}

#[test]
fn selected_raw_pointer_real_producer_holder_cfg_and_helper_transport_reach_observer() {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_selected_pointer_emission_v29)));
    for immutable in [false, true] {
        for graph in [0, 1, 2, 3] {
            SELECTED_POINTER_FIXTURE.set((immutable, graph));
            SELECTED_POINTER_OBSERVED.set(0);
            SELECTED_POINTER_EXIT.set(0);
            selected_pointer_structural_layouts_v29();
            let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
            let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
            budget.reserve_storage(MODULE_FLOOR).unwrap();
            let prepared =
                scalar_payload_prepared_from_v18(selected_pointer_emission_owner_v29, &mut budget);
            let result = prepared.with_source_consumer_v18(
                &mut budget,
                |_, _| -> SourceOwnedResultV18<()> {
                    panic!("final object completion remains gated");
                },
            );
            assert_eq!(
                SELECTED_POINTER_OBSERVED.get(),
                1,
                "immutable {immutable}, graph {graph}: {result:?}"
            );
            assert!(format!("{:?}", result.unwrap_err()).contains(SELECTED_POINTER_STOP));
            assert_eq!(budget.storage(), MODULE_FLOOR);
        }
    }
}

#[test]
fn selected_raw_pointer_observer_error_panic_and_pending_guard_restore_caller_floor() {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    let _restore =
        Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_selected_pointer_emission_v29)));
    for mode in [0, 1, 2] {
        SELECTED_POINTER_FIXTURE.set((false, 0));
        SELECTED_POINTER_OBSERVED.set(0);
        SELECTED_POINTER_EXIT.set(mode);
        selected_pointer_structural_layouts_v29();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared =
            scalar_payload_prepared_from_v18(selected_pointer_emission_owner_v29, &mut budget);
        let completed = std::cell::Cell::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_source_consumer_v18(
                &mut budget,
                |source, budget| -> SourceOwnedResultV18<()> {
                    assert_eq!(
                        mode, 1,
                        "error and panic observers must stop before the consumer"
                    );
                    assert_eq!(source.root_count(budget)?, 1);
                    source.with_analysis_v18(budget, |scope| {
                        scope.with_inventory_v1(|inventory, budget| {
                            source.with_ranked_correspondence_v18(
                                inventory,
                                budget,
                                |relation, budget| {
                                    scoped_raw_admission_v29::with_checked_source_memory_v29(
                                        relation,
                                        0,
                                        None,
                                        budget,
                                        |_, _| -> SourceOwnedResultV18<()> {
                                            completed.set(true);
                                            Ok(())
                                        },
                                    )
                                },
                            )
                        })
                    })
                },
            )
        }));
        // Successful admission, reconstruction and consumer replay each emit
        // the unchanged source; observer errors and panics stop on first entry.
        assert_eq!(
            SELECTED_POINTER_OBSERVED.get(),
            if mode == 1 { 3 } else { 1 }
        );
        assert_eq!(completed.get(), mode == 1);
        if mode == 1 {
            result.unwrap().unwrap();
        } else if mode == 2 {
            let refused =
                result.expect("the source callback boundary must contain observer panics");
            assert!(
                matches!(
                    &refused,
                    Err(ProductionSourceOwnedViewErrorV18::Source(
                        ProductionPendingScopedSourceErrorV29::Source(
                            ProductionSemanticKirErrorV1::Unsupported {
                                function: 0,
                                block: None,
                                statement: None,
                                detail: "source reference callback panicked",
                            }
                        )
                    ))
                ),
                "observer panic must keep its exact typed refusal: {refused:?}"
            );
        } else {
            let error = format!("{:?}", result.unwrap().unwrap_err());
            assert!(error.contains(SELECTED_POINTER_STOP), "{error}");
        }
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

const TAG_EMISSION_STOP: &str = "test stopped after original typed allocation and tag census";
thread_local! {
    static TAG_EMISSION_OBSERVED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static TAG_EMISSION_CONTINUE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static OBJECT_ORIGIN_MUTATION_CASE: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OBJECT_ORIGIN_MUTATION_REACHED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn original_tag_emission_owner() -> ProductionSemanticSsaOwnerV1 {
    let base = entrance_control_owner(false);
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let enumeration = SemanticTypeIdV1::from_index(types.len() as u32);
    let tag = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 8, 1),
        SemanticScalarValidityRangeV1::new(0, 255),
    );
    let variants = (0..3)
        .map(|index| {
            SemanticEnumVariantLayoutV1::from_rustc(
                index,
                1,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                720 + u64::from(index),
                SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
            )
            .unwrap()
        })
        .collect();
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([172; 32]),
        SemanticLayoutIdentityV1::from_sha256([172; 32]),
        SemanticTypeLayoutV1::enum_layout_with_backend_repr(
            1,
            1,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticEnumLayoutV1::new(
                variants,
                SemanticEnumEncodingV1::Direct(SemanticDirectEnumEncodingV1::new(0, 0, tag)),
            )
            .unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::enum_type(
            U32,
            [3, 17, 250]
                .into_iter()
                .map(|value| {
                    SemanticEnumVariantV1::new(value, SemanticAggregateTypeV1::new(vec![]).unwrap())
                })
                .collect(),
        )
        .unwrap(),
    ));
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let mut locals = helper.locals().to_vec();
    assert_eq!(locals.len(), 2);
    locals.push(local(218, enumeration, SemanticLocalRoleV1::Temporary));
    let mut statements = Vec::new();
    for variant in [0, 2] {
        statements.extend([
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
            ),
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::SetDiscriminant {
                    place: place(2, enumeration),
                    variant_index: variant,
                },
            ),
            assign(
                place(1, U32),
                SemanticRvalueKindV1::Discriminant(place(2, enumeration)),
            ),
            SemanticStatementV1::new(
                source(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(2)),
            ),
        ]);
    }
    functions[2] = function(
        210,
        helper.role(),
        helper.abi().clone(),
        locals,
        vec![block(214, statements, SemanticTerminatorKindV1::Return)],
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

fn observe_original_tag_emission(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut helpers = std::collections::BTreeSet::new();
    let mut allocations = std::collections::BTreeMap::new();
    for slot in &slots.slots {
        let ScopedSlotRepresentationV29::Object {
            schema,
            bytes,
            alignment,
        } = slot.representation
        else {
            continue;
        };
        let ScopedAllocationIdentityV29::OriginalObject { local, generation } =
            slot.origin.identity
        else {
            panic!("original object allocation");
        };
        let source = instances.instance(slot.instance).unwrap();
        assert_eq!(source.function().index(), 2);
        assert_eq!((local, bytes, alignment), (2, 1, 1));
        assert!(matches!(generation, 0 | 1 | 5));
        assert!(
            matches!(slot.origin.source, ScopedAllocationSourceV29::OriginalObject { schema: original, .. } if original == schema)
        );
        let lowered = emitted[slot.instance.index()].as_ref().unwrap();
        let operation = &lowered.function.body.as_ref().unwrap().blocks
            [slot.allocation.block_ordinal]
            .operations[slot.allocation.operation];
        check_scoped_slot_alloca_v29(slot, operation, budget)?;
        assert!(
            allocations
                .insert((slot.instance.index(), generation), slot.origin.pointer)
                .is_none()
        );
        helpers.insert(slot.instance.index());
    }
    assert!(
        !helpers.is_empty(),
        "actual source helper must emit typed allocations"
    );
    assert_eq!(
        allocations.len(),
        helpers.len() * 3,
        "entry and both original activation sites"
    );
    for instance in helpers {
        let lowered = emitted[instance].as_ref().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_ref().unwrap();
        let mut tags = Vec::new();
        for row in &anchors.objects {
            let (endpoint, read, variant, address) = match (row.role, row.operation) {
                (
                    ScopedObjectRoleV29::SetDiscriminant {
                        destination,
                        variant,
                        origin: ScopedObjectTagOriginV29::Statement(_),
                    },
                    ScopedObjectOperationV29::SetDiscriminant {
                        address,
                        variant: actual,
                        access,
                    },
                ) => {
                    assert_eq!(variant, actual);
                    assert_eq!(access, MemoryAccess::new(AddressSpace::Private, 1));
                    (destination, false, Some(variant), address)
                }
                (
                    ScopedObjectRoleV29::ReadDiscriminant {
                        source,
                        origin: ScopedObjectTagOriginV29::Statement(_),
                    },
                    ScopedObjectOperationV29::ReadDiscriminant { address, access },
                ) => {
                    assert_eq!(access, MemoryAccess::new(AddressSpace::Private, 1));
                    (source, true, None, address)
                }
                _ => panic!("only original ordered tag effects are expected"),
            };
            let ScopedObjectIdentityV29::Local {
                instance: original,
                local,
                generation,
            } = endpoint.object
            else {
                panic!("original local identity");
            };
            assert_eq!((original.index(), local.index()), (instance, 2));
            assert_eq!(Some(&address), allocations.get(&(instance, generation)));
            assert_eq!(endpoint.source_path.count, 0);
            assert_eq!(endpoint.path.count, 0);
            tags.push((generation, read, variant));
        }
        assert_eq!(
            tags,
            [
                (1, false, Some(0)),
                (1, true, None),
                (5, false, Some(2)),
                (5, true, None)
            ]
        );
        TAG_EMISSION_OBSERVED.set(TAG_EMISSION_OBSERVED.get() + 1);
    }
    if TAG_EMISSION_CONTINUE.get() {
        Ok(())
    } else {
        Err(unsupported(0, None, None, TAG_EMISSION_STOP))
    }
}

#[test]
fn prepared_source_emits_original_object_allocations_and_ordered_tag_effects_before_completion() {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    TAG_EMISSION_OBSERVED.set(0);
    let _restore = Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_original_tag_emission)));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(original_tag_emission_owner, &mut budget);
    let result =
        prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
            panic!("typed final completion is intentionally still pending");
        });
    assert!(
        TAG_EMISSION_OBSERVED.get() > 0,
        "real prepared source did not reach allocation/tag observer: {result:?}"
    );
    assert!(format!("{:?}", result.unwrap_err()).contains(TAG_EMISSION_STOP));
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

fn with_original_object_access_builder(
    consume: impl FnOnce(
        &mut SourceReferenceBuilderV29<'_, '_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut owner = original_tag_emission_owner();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = ArgumentBudgetV1::new(&mut work, usize::MAX);
    budget.reserve_storage(97)?;
    assert!(
        owner.occurrence_storage().is_none(),
        "the prepared-source factory does not pre-capture under a foreign budget"
    );
    let captured = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(
        budget.storage(),
        97,
        "capture transfers its attachment out of the caller ledger"
    );
    let capture = owner
        .occurrence_storage()
        .expect("original occurrence capture");
    assert_eq!(capture.retained_storage(), captured.retained_storage());
    budget.reserve_storage(capture.retained_storage())?;
    assert_eq!(
        budget.storage(),
        97 + capture.retained_storage(),
        "capture storage is owned and charged exactly once"
    );
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )?;
    let floor = budget.storage();
    let persistent = layouts.persistent_storage_for_test();
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        SemanticFunctionIdV1::from_index(0),
        &mut budget,
        |instances, budget| {
            let lens = demands.root_lens(&owner, 0, budget).unwrap();
            let result = with_source_reference_descriptor_demands_scope_v29(
                instances,
                SourceReferenceStorageV29::ScalarCells,
                Some(&mut layouts),
                None,
                Some(lens),
                budget,
                |plan, root, budget| {
                    let mut builder = SourceReferenceBuilderV29::new_with_descriptor_demands(
                        plan.instances,
                        SourceReferenceStorageV29::ScalarCells,
                        root,
                        None,
                        plan.storage_demands,
                        budget,
                    )?;
                    builder.collect_storage_selectors(budget)?;
                    builder.function(builder.plan.root, None, budget)?;
                    consume(&mut builder, budget).map_err(Into::into)
                },
            );
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(result)
        },
    )
    .unwrap();
    let growth = layouts.persistent_storage_for_test() - persistent;
    let scratch = budget.storage() - floor - growth;
    assert!(layouts.permits_root_emission_refund(&owner, scratch, &budget));
    budget.release_storage(scratch)?;
    let cleanup = layouts.release(&mut budget);
    let demand_cleanup = demands.discard(&mut budget);
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    assert_eq!(budget.storage(), 97);
    result.and(cleanup).and(demand_cleanup)
}

fn original_object_access_replay<'source>(
    builder: &SourceReferenceBuilderV29<'_, '_, 'source>,
) -> (
    SourceReferenceSiteV29,
    &'source SemanticPlaceV1,
    SourceReferencePlaceV29,
) {
    let row = builder
        .plan
        .accesses
        .last()
        .expect("actual original tag access");
    assert_eq!(row.key.access, SourceReferenceAccessV29::ReadDiscriminant);
    let declaration = builder
        .plan
        .instances
        .instance(row.key.site.instance)
        .unwrap()
        .declaration();
    let statement = &declaration.blocks()[row.key.site.block.index() as usize].statements()
        [row.key.site.statement.unwrap()];
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        panic!("tag read assignment");
    };
    let SemanticRvalueKindV1::Discriminant(source) = assignment.value().kind() else {
        panic!("original tag read");
    };
    assert_eq!(row.key.source, source as *const SemanticPlaceV1 as usize);
    assert_eq!((row.local.index(), row.generation), (2, 5));
    assert!(row.loan.is_none() && row.projections.is_empty() && row.traversed.is_empty());
    // These three node fields are deliberately not payload facts: the locator
    // producer only consumes the original resolved identity/path retained above.
    let resolved = SourceReferencePlaceV29 {
        instance: row.instance,
        local: row.local,
        generation: row.generation,
        value: usize::MAX,
        representation_root: usize::MAX,
        node: usize::MAX,
        projections: builder.plan.projections[row.projections.clone()].to_vec(),
        selector_source: None,
        anchor: None,
        loan: row.loan,
        shared_path: row.shared_path,
        traversed: builder.plan.access_loans[row.traversed.clone()].to_vec(),
    };
    (row.key.site, source, resolved)
}

fn original_object_locator_filter_work(
    builder: &SourceReferenceBuilderV29<'_, '_, '_>,
    resolved: &SourceReferencePlaceV29,
) -> usize {
    let requests = builder.storage_requests.unwrap();
    let ordinal = requests
        .iter()
        .position(|row| (row.instance, row.local) == (resolved.instance, resolved.local))
        .unwrap();
    // Independent balanced interval tree, not the producer's binary search or
    // a measured successful run. Leaves name the lower-bound insertion points.
    let mut levels = std::collections::VecDeque::from([(0, requests.len(), 0)]);
    let comparisons = loop {
        let (first, end, depth) = levels.pop_front().unwrap();
        if first == end {
            if first == ordinal {
                break depth;
            }
        } else {
            let middle = first + (end - first) / 2;
            levels.push_back((first, middle, depth + 1));
            levels.push_back((middle + 1, end, depth + 1));
        }
    };
    // Owner5 + lens3+4 + demand identity6/path3 + slice identity1 + binary
    // comparison2/final1 + original declaration/type classification3.
    17 + 2 * comparisons
        + requests
            .iter()
            .map(|row| 6 + 3 * row.path.len())
            .sum::<usize>()
}

#[test]
fn original_local_object_access_revisit_preserves_exact_generation_and_custody() {
    for mode in 0..7 {
        let reached = std::cell::Cell::new(false);
        let result = with_original_object_access_builder(|builder, budget| {
            let (site, source, mut resolved) = original_object_access_replay(builder);
            let before = (
                builder.plan.accesses.len(),
                builder.plan.access_sites.len(),
                builder.plan.projections.len(),
                builder.plan.access_loans.len(),
            );
            match mode {
                1 => resolved.generation += 1,
                2 => resolved.shared_path = !resolved.shared_path,
                3 => builder.storage_requests = None,
                4 => builder.plan.slot ^= 1,
                5 => builder.plan.source[0] ^= 1,
                _ => {}
            }
            let result = if mode == 6 {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
                let mut foreign = ArgumentBudgetV1::new(&mut work, usize::MAX);
                foreign.reserve_storage(budget.storage())?;
                builder.retain_reference_access(
                    site,
                    source,
                    SourceReferenceAccessV29::ReadDiscriminant,
                    &resolved,
                    &mut foreign,
                )
            } else {
                builder.retain_reference_access(
                    site,
                    source,
                    SourceReferenceAccessV29::ReadDiscriminant,
                    &resolved,
                    budget,
                )
            };
            assert_eq!(result.is_ok(), mode == 0);
            assert_eq!(
                (
                    builder.plan.accesses.len(),
                    builder.plan.access_sites.len(),
                    builder.plan.projections.len(),
                    builder.plan.access_loans.len()
                ),
                before
            );
            reached.set(true);
            Ok(())
        });
        assert!(
            reached.get(),
            "actual source must reach hostile locator replay"
        );
        assert_eq!(
            result.is_err(),
            matches!(mode, 4..=6),
            "owner failures remain sticky"
        );
    }
}

#[test]
fn original_local_object_access_publication_has_independent_work_and_storage_boundaries() {
    for mode in 0..4 {
        let reached = std::cell::Cell::new(false);
        let result = with_original_object_access_builder(|builder, budget| {
            let (site, source, resolved) = original_object_access_replay(builder);
            let removed = builder.plan.accesses.pop().unwrap();
            let key = source_reference_access_key_v29(
                site,
                source,
                SourceReferenceAccessV29::ReadDiscriminant,
            );
            assert_eq!(
                builder.plan.access_sites.remove(&key),
                Some(builder.plan.accesses.len())
            );
            builder.plan.projections.truncate(removed.projections.start);
            builder.plan.access_loans.truncate(removed.traversed.start);
            assert!(builder.plan.accesses.len() < builder.plan.accesses.capacity());
            let before = (
                builder.plan.accesses.len(),
                builder.plan.access_sites.len(),
                builder.plan.projections.len(),
                builder.plan.access_loans.len(),
            );
            let capacities = (
                builder.plan.accesses.capacity(),
                builder.plan.projections.capacity(),
                builder.plan.access_loans.capacity(),
            );
            let levels = builder.plan.access_sites.len().checked_ilog2().unwrap_or(0) as usize + 2;
            let exact_work =
                original_object_locator_filter_work(builder, &resolved) + 2 * levels * 16 + 7;
            let exact_storage = levels
                * 32
                * std::mem::size_of::<(Box<SourceReferenceAccessIndexKeyV29>, usize, usize)>()
                + std::mem::size_of::<SourceReferenceAccessIndexKeyV29>();
            let filler = if mode >= 2 {
                let filler = usize::MAX - budget.storage() - exact_storage + usize::from(mode == 3);
                budget.reserve_storage(filler)?;
                filler
            } else {
                budget.charge_work(
                    usize::MAX - budget.work() - exact_work + usize::from(mode == 1),
                )?;
                0
            };
            let work_before = budget.work();
            let storage_before = budget.storage();
            let result = builder.retain_reference_access(
                site,
                source,
                SourceReferenceAccessV29::ReadDiscriminant,
                &resolved,
                budget,
            );
            let short = matches!(mode, 1 | 3);
            assert_eq!(result.is_err(), short);
            assert_eq!(
                (
                    builder.plan.accesses.len(),
                    builder.plan.access_sites.len(),
                    builder.plan.projections.len(),
                    builder.plan.access_loans.len()
                ),
                (
                    before.0 + usize::from(!short),
                    before.1 + usize::from(!short),
                    before.2,
                    before.3
                )
            );
            assert_eq!(
                (
                    builder.plan.accesses.capacity(),
                    builder.plan.projections.capacity(),
                    builder.plan.access_loans.capacity()
                ),
                capacities
            );
            assert_eq!(
                budget.storage() - storage_before,
                if mode == 3 { 0 } else { exact_storage }
            );
            assert_eq!(
                budget.work() - work_before,
                exact_work
                    - match mode {
                        1 => 1,
                        3 => 7,
                        _ => 0,
                    }
            );
            if !short {
                assert_eq!(builder.plan.access_sites.get(&key), Some(&before.0));
                assert_eq!(builder.plan.accesses[before.0].generation, 5);
            }
            if filler != 0 {
                budget.release_storage(filler)?;
            }
            reached.set(true);
            Err(source_reference_error_v29(
                "finished original locator resource boundary",
            ))
        });
        assert!(
            reached.get(),
            "independent resource assertions must execute"
        );
        assert!(result.is_err());
    }
}

#[test]
fn boxed_access_index_prepayment_has_independent_exact_and_one_short_boundaries() {
    for count in [0usize, 1, 2, 31, 32, 63, 64, 127, 128] {
        let levels = count.checked_ilog2().unwrap_or(0) as usize + 2;
        let exact_work = 16 * levels;
        let exact_storage = levels
            * 32
            * std::mem::size_of::<(Box<SourceReferenceAccessIndexKeyV29>, usize, usize)>()
            + std::mem::size_of::<SourceReferenceAccessIndexKeyV29>();
        for mode in 0..3 {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work - usize::from(mode == 1));
            let mut budget =
                ArgumentBudgetV1::new(&mut work, 47 + exact_storage - usize::from(mode == 2));
            budget.reserve_storage(47).unwrap();
            let result = reserve_source_reference_access_index_v29(count, &mut budget);
            match mode {
                0 => result.unwrap(),
                1 => assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Work(_)
                        )
                    )
                )),
                2 => assert!(matches!(
                    result,
                    Err(
                        ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                            ArgumentResourceV1::Storage(_)
                        )
                    )
                )),
                _ => unreachable!(),
            }
            assert_eq!(budget.work(), if mode == 1 { 0 } else { exact_work });
            assert_eq!(
                budget.storage(),
                47 + if mode == 0 { exact_storage } else { 0 }
            );
        }
    }
}

#[test]
fn boxed_access_index_scaling_preserves_exact_borrowed_keys_and_stable_owned_storage() {
    // These inert keys test the representation only. Original-source publication
    // and custody remain covered by the builder boundary and hostile tests above.
    for count in [0usize, 1, 2, 32, 64, 128] {
        let levels = (0..count)
            .map(|index| index.checked_ilog2().unwrap_or(0) as usize + 2)
            .sum::<usize>();
        let exact_work = 16 * levels;
        let exact_storage = 32
            * levels
            * std::mem::size_of::<(Box<SourceReferenceAccessIndexKeyV29>, usize, usize)>()
            + count * std::mem::size_of::<SourceReferenceAccessIndexKeyV29>();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(exact_work);
        let mut budget = ArgumentBudgetV1::new(&mut work, 47 + exact_storage);
        budget.reserve_storage(47).unwrap();
        let mut index = BTreeMap::<Box<SourceReferenceAccessIndexKeyV29>, usize>::new();
        let mut originals = Vec::new();
        for ordinal in 0..count {
            let key: SourceReferenceAccessIndexKeyV29 = (
                ordinal / 16,
                (ordinal % 16) as u32,
                if ordinal % 2 == 0 {
                    None
                } else {
                    Some(ordinal / 2)
                },
                4096 + ordinal,
                (ordinal % 7) as u8,
            );
            reserve_source_reference_access_index_v29(index.len(), &mut budget).unwrap();
            assert!(index.insert(Box::new(key), ordinal).is_none());
            let (owned, value) = index.get_key_value(&key).unwrap();
            assert_eq!(*value, ordinal);
            originals.push((
                key,
                owned.as_ref() as *const SourceReferenceAccessIndexKeyV29 as usize,
            ));
        }
        for (ordinal, (key, pointer)) in originals.iter().enumerate() {
            let (owned, value) = index.get_key_value(key).unwrap();
            assert_eq!(owned.as_ref(), key);
            assert_eq!(*value, ordinal);
            assert_eq!(
                owned.as_ref() as *const SourceReferenceAccessIndexKeyV29 as usize,
                *pointer
            );
            for coordinate in 0..5 {
                let mut changed = *key;
                match coordinate {
                    0 => changed.0 += 1,
                    1 => changed.1 += 1,
                    2 => changed.2 = if changed.2.is_none() { Some(0) } else { None },
                    3 => changed.3 += count + 1,
                    4 => changed.4 = (changed.4 + 1) % 7,
                    _ => unreachable!(),
                }
                assert_eq!(
                    index.get(&changed),
                    None,
                    "the complete original occurrence key is retained"
                );
            }
        }
        assert_eq!(budget.work(), exact_work);
        assert_eq!(
            (budget.storage(), budget.peak_storage()),
            (47 + exact_storage, 47 + exact_storage)
        );
        if count >= 32 {
            let inline = 32
                * levels
                * std::mem::size_of::<(SourceReferenceAccessIndexKeyV29, usize, usize)>();
            assert!(
                2 * exact_storage < inline,
                "unused BTree slots must not duplicate complete source keys"
            );
        }
        drop(index);
        budget.release_storage(exact_storage).unwrap();
        assert_eq!(budget.storage(), 47);
    }
}

#[test]
fn original_object_storage_query_requires_the_same_cell_generation_and_schema() {
    for mode in 0..9 {
        let reached = std::cell::Cell::new(false);
        let result = with_original_object_access_builder(|builder, budget| {
            builder.plan_scalar_cells(budget)?;
            let (mut ordinal, cell) = builder
                .plan
                .cells
                .rows
                .iter()
                .copied()
                .enumerate()
                .find(|(_, cell)| cell.local.index() == 2 && cell.generation == 5)
                .unwrap();
            let SourceBackingKindV29::Object(mut schema) = cell.kind else {
                panic!("actual selected object");
            };
            let mut instance = cell.instance;
            let mut local = cell.local;
            let mut generation = cell.generation;
            match mode {
                1 => ordinal = usize::MAX,
                2 => instance = builder.plan.root,
                3 => local = SemanticLocalIdV1::from_index(1),
                4 => generation = u32::MAX,
                5 => schema = fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX),
                6 => builder.plan.cells.rows[ordinal].kind = SourceBackingKindV29::Scalar,
                7 => builder.plan.cells.rows[ordinal].ty = U32,
                8 => builder.plan.has_storage_demands = false,
                _ => {}
            }
            let queried = budget.source_object_storage_v29(
                &builder.plan,
                ordinal,
                instance,
                local,
                generation,
                schema,
            );
            assert_eq!(queried.is_ok(), mode == 0);
            reached.set(true);
            match queried {
                Ok(storage) => {
                    assert!(matches!(storage, SemanticRetainedStorageV29::Object {
                        cell: original, schema: selected, bytes: 1, alignment: 1,
                    } if original == ordinal && selected == schema));
                    Ok(())
                }
                Err(error) => Err(error),
            }
        });
        assert!(
            reached.get(),
            "actual source must select object backing before query mutation"
        );
        assert_eq!(result.is_err(), mode != 0);
    }
}

#[test]
fn original_object_storage_query_prepays_independent_value_and_return_headers() {
    for short in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result = with_original_object_access_builder(|builder, budget| {
            builder.plan_scalar_cells(budget)?;
            let (ordinal, cell) = builder
                .plan
                .cells
                .rows
                .iter()
                .copied()
                .enumerate()
                .find(|(_, cell)| cell.local.index() == 2 && cell.generation == 5)
                .unwrap();
            let SourceBackingKindV29::Object(schema) = cell.kind else {
                panic!("selected object");
            };
            let header = std::mem::size_of::<SemanticRetainedStorageV29>()
                + 2 * std::mem::size_of::<
                    Result<SemanticRetainedStorageV29, ProductionSemanticKirErrorV1>,
                >();
            let filler = usize::MAX - budget.storage() - header + usize::from(short);
            budget.reserve_storage(filler)?;
            let before = (budget.work(), budget.storage());
            let query = budget.source_object_storage_v29(
                &builder.plan,
                ordinal,
                cell.instance,
                cell.local,
                cell.generation,
                schema,
            );
            assert_eq!(query.is_err(), short);
            assert_eq!(budget.storage() - before.1, if short { 0 } else { header });
            if short {
                assert_eq!(budget.work() - before.0, 10);
            }
            budget.release_storage(filler)?;
            reached.set(true);
            Ok(())
        });
        assert!(reached.get());
        assert_eq!(result.is_err(), short);
    }
}

#[test]
fn original_object_slot_query_scope_keeps_exact_work_peak_and_first_denial() {
    const STOP: &str = "exact object slot query completed at the work ceiling";
    for mode in 0..7 {
        let completed = std::cell::Cell::new(false);
        let result = with_original_object_access_builder(|builder, budget| {
            builder.plan_scalar_cells(budget)?;
            let (ordinal, cell) = builder
                .plan
                .cells
                .rows
                .iter()
                .copied()
                .enumerate()
                .find(|(_, cell)| cell.local.index() == 2 && cell.generation == 5)
                .unwrap();
            let SourceBackingKindV29::Object(schema) = cell.kind else {
                panic!("selected object");
            };
            let header = std::mem::size_of::<SemanticRetainedStorageV29>()
                + 2 * std::mem::size_of::<
                    Result<SemanticRetainedStorageV29, ProductionSemanticKirErrorV1>,
                >();
            // Entry custody 5; scratch entry 2; source custody 5+5; cell identity 12;
            // layout lens/owner/schema/rows 1+1+1+1; physical shape 3.
            const WORK: usize = 36;
            if mode == 1 || mode == 2 {
                budget.charge_work(usize::MAX - budget.work() - WORK + usize::from(mode == 2))?;
            }
            if mode == 6 {
                budget.charge_work(usize::MAX - budget.work() - 6)?;
            }
            let filler = if mode == 3 || mode == 4 {
                usize::MAX - budget.storage() - header + usize::from(mode == 4)
            } else {
                0
            };
            budget.reserve_storage(filler)?;
            let before = (budget.work(), budget.storage(), budget.peak_storage());
            let query = scoped_object_slot_representation_v29(
                &builder.plan,
                ordinal,
                cell.instance,
                cell.local,
                if mode == 5 { u32::MAX } else { cell.generation },
                schema,
                budget,
            );
            assert_eq!(
                budget.storage(),
                before.1,
                "query values die before scratch refund"
            );
            assert_eq!(
                budget.peak_storage(),
                before.2.max(if mode == 4 || mode == 6 {
                    before.1
                } else {
                    before.1 + header
                })
            );
            let expected_work = match mode {
                2 => 33,
                4 => 17,
                5 => 29,
                6 => 5,
                _ => WORK,
            };
            assert_eq!(budget.work() - before.0, expected_work);
            match mode {
                0 | 1 | 3 => assert!(matches!(query,
                    Ok(ScopedSlotRepresentationV29::Object { schema: found, bytes: 1, alignment: 1 })
                    if found == schema)),
                2 | 4 | 6 => {
                    let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(resource)) =
                        &query
                    else {
                        panic!("expected exact resource refusal: {query:?}");
                    };
                    assert!(matches!(
                        (mode, resource),
                        (2 | 6, ArgumentResourceV1::Work(_)) | (4, ArgumentResourceV1::Storage(_))
                    ));
                    let first = *resource;
                    if mode == 4 {
                        budget.charge_work(usize::MAX - budget.work())?;
                    }
                    let failed = (budget.work(), budget.storage());
                    assert!(matches!(scoped_object_slot_representation_v29(
                        &builder.plan, ordinal, cell.instance, cell.local, cell.generation, schema, budget,
                    ),
                        Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error)) if error == first));
                    assert_eq!((budget.work(), budget.storage()), failed);
                }
                5 => assert!(matches!(
                    query,
                    Err(ProductionSemanticKirErrorV1::Unsupported {
                        detail: "typed allocation identity or representation requires its exact source contract",
                        ..
                    })
                )),
                _ => unreachable!(),
            }
            budget.release_storage(filler)?;
            if mode == 0 {
                for _ in 0..3 {
                    let before = (budget.work(), budget.storage());
                    assert!(matches!(
                        scoped_object_slot_representation_v29(
                            &builder.plan,
                            ordinal,
                            cell.instance,
                            cell.local,
                            cell.generation,
                            schema,
                            budget,
                        )?,
                        ScopedSlotRepresentationV29::Object {
                            bytes: 1,
                            alignment: 1,
                            ..
                        }
                    ));
                    assert_eq!(
                        (budget.work() - before.0, budget.storage()),
                        (WORK, before.1)
                    );
                }
            }
            completed.set(true);
            if mode == 1 {
                Err(source_reference_error_v29(STOP))
            } else {
                query.map(|_| ())
            }
        });
        assert!(
            completed.get(),
            "all scope assertions must finish: {result:?}"
        );
        match mode {
            0 | 3 => result.unwrap(),
            1 => assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported { detail: STOP, .. })
            )),
            2 | 6 => assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Work(_)
                    )
                )
            )),
            4 => assert!(matches!(
                result,
                Err(
                    ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                        ArgumentResourceV1::Storage(_)
                    )
                )
            )),
            5 => assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "typed allocation identity or representation requires its exact source contract",
                    ..
                })
            )),
            _ => unreachable!(),
        }
    }
}

#[test]
fn original_object_slot_query_refuses_foreign_custody_before_scratch_debits() {
    let completed = std::cell::Cell::new(false);
    let result = with_original_object_access_builder(|builder, budget| {
        builder.plan_scalar_cells(budget)?;
        let (ordinal, cell) = builder
            .plan
            .cells
            .rows
            .iter()
            .copied()
            .enumerate()
            .find(|(_, cell)| cell.local.index() == 2 && cell.generation == 5)
            .unwrap();
        let SourceBackingKindV29::Object(schema) = cell.kind else {
            panic!("selected object");
        };
        let before = (budget.work(), budget.storage());
        let mut work = CanonicalKernelIrWorkBudgetV1::new(64);
        let mut foreign = ArgumentBudgetV1::new(&mut work, 83);
        foreign.reserve_storage(83)?;
        let query = scoped_object_slot_representation_v29(
            &builder.plan,
            ordinal,
            cell.instance,
            cell.local,
            cell.generation,
            schema,
            &mut foreign,
        );
        assert!(matches!(
            query,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((foreign.work(), foreign.storage()), (0, 83));
        assert_eq!((budget.work(), budget.storage()), before);
        assert!(matches!(
            scoped_object_slot_representation_v29(
                &builder.plan,
                ordinal,
                cell.instance,
                cell.local,
                cell.generation,
                schema,
                budget,
            ),
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((budget.work(), budget.storage()), before);
        completed.set(true);
        query.map(|_| ())
    });
    assert!(
        completed.get(),
        "foreign-custody assertions must finish: {result:?}"
    );
    assert!(matches!(
        result,
        Err(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Accounting
            )
        )
    ));
}

#[test]
fn alternate_emission_meter_cannot_lend_original_object_storage_authority() {
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
    let reached = std::cell::Cell::new(false);
    let result = with_original_object_access_builder(|builder, budget| {
        let before = (budget.work(), budget.storage());
        let root = builder.plan.root;
        let query = Alternate(budget).source_object_storage_v29(
            &builder.plan,
            0,
            root,
            SemanticLocalIdV1::from_index(0),
            0,
            fe2o3_kernel_ir::StorageLayoutIdV1(0),
        );
        assert!(matches!(
            query,
            Err(
                ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Accounting
                )
            )
        ));
        assert_eq!((budget.work(), budget.storage()), before);
        assert!(
            builder
                .plan
                .check_owner(builder.plan.instances, budget)
                .is_err()
        );
        reached.set(true);
        Ok(())
    });
    assert!(reached.get());
    assert!(
        result.is_err(),
        "default trait refusal must remain in the original sticky owner"
    );
}

fn forge_original_object_origins(
    original: &[ScopedSlotOriginV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<Vec<ScopedSlotOriginV29>>, ProductionSemanticKirErrorV1> {
    if !original.iter().any(|row| {
        matches!(
            row.identity,
            ScopedAllocationIdentityV29::OriginalObject { .. }
        )
    }) {
        return Ok(None);
    }
    assert_eq!(
        original.len(),
        3,
        "entry and two actual original live sites"
    );
    let case = OBJECT_ORIGIN_MUTATION_CASE.get();
    let before = (budget.work(), budget.storage());
    type Mutation = Option<Vec<ScopedSlotOriginV29>>;
    let headers = std::mem::size_of::<Mutation>()
        + 2 * std::mem::size_of::<Result<Mutation, ProductionSemanticKirErrorV1>>();
    budget.reserve_storage(headers)?;
    let mut forged = emission_vec_v1(original.len() + usize::from(case == 1), budget)?;
    budget.charge_work(2 * original.len())?;
    forged.extend_from_slice(original);
    assert_eq!(budget.work() - before.0, 3 + 2 * original.len());
    assert_eq!(
        budget.storage() - before.1,
        headers + forged.capacity() * std::mem::size_of::<ScopedSlotOriginV29>()
    );
    OBJECT_ORIGIN_MUTATION_REACHED.set(OBJECT_ORIGIN_MUTATION_REACHED.get() + 1);
    match case {
        0 => {
            forged.remove(0);
        }
        1 => {
            forged.insert(1, forged[0]);
        }
        2 => {
            if let ScopedAllocationIdentityV29::OriginalObject { generation, .. } =
                &mut forged[0].identity
            {
                *generation = u32::MAX;
            }
        }
        3 => {
            if let ScopedAllocationSourceV29::OriginalObject { schema, .. } = &mut forged[0].source
            {
                *schema = fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX);
            }
        }
        4 => forged[0].pointer = ValueId(u32::MAX),
        5 => forged[0].source = ScopedAllocationSourceV29::Legacy,
        6 => {
            forged[0].identity = ScopedAllocationIdentityV29::OriginalObject {
                local: 1,
                generation: 0,
            }
        }
        7 => {
            if let ScopedAllocationSourceV29::OriginalObject { cell, .. } = &mut forged[0].source {
                *cell = usize::MAX;
            }
        }
        8 => panic!("original object roster test panic after paid clone"),
        9 => {
            return Err(source_reference_error_v29(
                "original object roster test error after paid clone",
            ));
        }
        _ => unreachable!(),
    }
    Ok(Some(forged))
}

#[test]
fn prepared_source_rejects_forged_object_allocation_rosters_and_releases_test_clones() {
    struct Restore {
        mutation: Option<ScopedObjectOriginMutationV29>,
        observer: Option<ScopedSlotObserverV29>,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_OBJECT_ORIGIN_MUTATION_V29.set(self.mutation);
            SCOPED_SLOT_OBSERVER_V29.set(self.observer);
        }
    }
    let _restore = Restore {
        mutation: SCOPED_OBJECT_ORIGIN_MUTATION_V29.replace(Some(forge_original_object_origins)),
        observer: SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_original_tag_emission)),
    };
    for case in 0..10 {
        OBJECT_ORIGIN_MUTATION_CASE.set(case);
        OBJECT_ORIGIN_MUTATION_REACHED.set(0);
        TAG_EMISSION_OBSERVED.set(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared = scalar_payload_prepared_from_v18(original_tag_emission_owner, &mut budget);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
                panic!("forged object census reached final completion");
            })
        }));
        assert_eq!(
            OBJECT_ORIGIN_MUTATION_REACHED.get(),
            1,
            "case {case} must reach the actual emitted census"
        );
        assert_eq!(
            TAG_EMISSION_OBSERVED.get(),
            0,
            "case {case} must fail production census validation"
        );
        match result {
            Ok(result) => assert!(result.is_err(), "case {case} was accepted"),
            Err(_) => assert_eq!(case, 8, "unexpected panic in case {case}"),
        }
        assert_eq!(
            budget.storage(),
            MODULE_FLOOR,
            "case {case} leaked original scope or test clone storage"
        );
    }
}

#[test]
fn observed_tag_only_object_emission_cannot_admit_a_final_source_artifact() {
    struct Restore(Option<ScopedSlotObserverV29>, bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
            TAG_EMISSION_CONTINUE.set(self.1);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_original_tag_emission)),
        TAG_EMISSION_CONTINUE.replace(true),
    );
    TAG_EMISSION_OBSERVED.set(0);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(MODULE_FLOOR).unwrap();
    let prepared = scalar_payload_prepared_from_v18(original_tag_emission_owner, &mut budget);
    let result =
        prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
            panic!("unqualified typed memory reached final artifact admission");
        });
    assert!(
        TAG_EMISSION_OBSERVED.get() > 0,
        "actual source must first emit typed allocations and tags"
    );
    assert!(
        result.is_err(),
        "allocation/tag observation is not final currentness qualification"
    );
    assert_eq!(budget.storage(), MODULE_FLOOR);
}

mod pointer_array_representation_tests {
    include!("production_source_pointer_array_emission_v29_tests.rs");
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum SourceArrayModeV29 {
    Scalar,
    ThinPointer,
    PointerAddresses,
    StoredBorrow,
    ExpiredRaw,
}

thread_local! {
    static SOURCE_ARRAY_CASE_V29: std::cell::Cell<(u16, u64, bool)> = const { std::cell::Cell::new((32, 2, false)) };
    static SOURCE_ARRAY_MODE_V29: std::cell::Cell<SourceArrayModeV29> = const { std::cell::Cell::new(SourceArrayModeV29::Scalar) };
    static SOURCE_ARRAY_OBSERVED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn original_scalar_array_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let (bits, length, raw) = SOURCE_ARRAY_CASE_V29.get();
    let mode = SOURCE_ARRAY_MODE_V29.get();
    let base = entrance_control_owner(false);
    let semantic = base.source_semantic();
    let mut types = semantic.types().to_vec();
    let bytes = u64::from(bits / 8);
    let mut element = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([174; 32]),
        SemanticLayoutIdentityV1::from_sha256([174; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, bits, bytes),
                SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits,
        }),
    ));
    let pointee_type = element;
    if matches!(
        mode,
        SourceArrayModeV29::ThinPointer | SourceArrayModeV29::PointerAddresses
    ) {
        assert_eq!(bits, 64);
        let pointee = element;
        element = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([175; 32]),
            SemanticLayoutIdentityV1::from_sha256([175; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(0, u128::from(u64::MAX)),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    pointee,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
    }
    let array = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([176; 32]),
        SemanticLayoutIdentityV1::from_sha256([176; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            bytes * length,
            bytes,
            SemanticFieldsShapeV1::array(bytes, length),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(true),
            None,
            false,
            None,
            bytes,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Array { element, length },
    ));
    let mut functions = semantic.functions().to_vec();
    let helper = &functions[2];
    let mut locals = helper.locals().to_vec();
    assert_eq!(locals.len(), 2);
    locals.push(local(219, array, SemanticLocalRoleV1::Temporary));
    locals.push(local(220, element, SemanticLocalRoleV1::Temporary));
    if mode == SourceArrayModeV29::PointerAddresses {
        assert!(!raw);
        locals.push(local(223, pointee_type, SemanticLocalRoleV1::Temporary));
    }
    let reference = mode == SourceArrayModeV29::StoredBorrow;
    let raw_type = if raw || reference {
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([177; 32]),
            SemanticLayoutIdentityV1::from_sha256([177; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(u128::from(reference), u128::from(u64::MAX)),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    element,
                    if reference {
                        SemanticPointerKindV1::Reference
                    } else {
                        SemanticPointerKindV1::Raw
                    },
                    SemanticMutabilityV1::Immutable,
                    0,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        ));
        locals.push(local(221, ty, SemanticLocalRoleV1::Temporary));
        Some(ty)
    } else {
        None
    };
    let holder = reference.then(|| {
        let ty = SemanticTypeIdV1::from_index(types.len() as u32);
        types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([178; 32]),
            SemanticLayoutIdentityV1::from_sha256([178; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                8,
                8,
                SemanticFieldsShapeV1::array(8, 1),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                8,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: raw_type.unwrap(),
                length: 1,
            },
        ));
        locals.push(local(222, ty, SemanticLocalRoleV1::Temporary));
        ty
    });
    let literal = |value| {
        if mode == SourceArrayModeV29::PointerAddresses {
            SemanticOperandV1::Copy(place(3, element))
        } else {
            SemanticOperandV1::Constant(SemanticConstantV1::new(
                element,
                if mode == SourceArrayModeV29::ThinPointer {
                    SemanticConstantValueV1::Pointer(SemanticPointerValueV1::new(
                        0,
                        SemanticPointerProvenanceV1::ExposedAddress,
                    ))
                } else {
                    SemanticConstantValueV1::Scalar(
                        SemanticScalarValueV1::new(value, bytes as u8).unwrap(),
                    )
                },
            ))
        }
    };
    let indexed = || {
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(2),
            vec![
                SemanticProjectionV1::new(
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: length - 1,
                        minimum_length: length,
                        from_end: false,
                    },
                    element,
                )
                .unwrap(),
            ],
            element,
        )
        .unwrap()
    };
    let mut statements = Vec::new();
    if mode == SourceArrayModeV29::PointerAddresses {
        statements.push(assign(
            place(4, pointee_type),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                pointee_type,
                SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(19, 8).unwrap()),
            ))),
        ));
        statements.push(assign(
            place(3, element),
            SemanticRvalueKindV1::AddressOf {
                mutability: SemanticMutabilityV1::Immutable,
                place: place(4, pointee_type),
            },
        ));
    }
    for generation in 0..2 {
        statements.push(SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
        ));
        statements.push(assign(
            place(2, array),
            SemanticRvalueKindV1::Aggregate(
                SemanticAggregateRvalueV1::new(
                    SemanticAggregateKindV1::Array,
                    (0..length)
                        .map(|index| literal(u128::from(index + 11)))
                        .collect(),
                )
                .unwrap(),
            ),
        ));
        statements.push(assign(
            indexed(),
            SemanticRvalueKindV1::Use(literal(31 + generation)),
        ));
        statements.push(assign(
            place(3, element),
            SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                indexed(),
                SemanticVolatilityV1::NonVolatile,
                None,
            )),
        ));
        if generation == 0
            && let Some(raw_type) = raw_type
        {
            statements.push(assign(
                place(4, raw_type),
                if reference {
                    SemanticRvalueKindV1::Borrow {
                        kind: SemanticBorrowKindV1::Shared,
                        place: indexed(),
                    }
                } else {
                    SemanticRvalueKindV1::AddressOf {
                        mutability: SemanticMutabilityV1::Immutable,
                        place: indexed(),
                    }
                },
            ));
            if let Some(holder) = holder {
                statements.push(assign(
                    place(5, holder),
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Array,
                            vec![SemanticOperandV1::Copy(place(4, raw_type))],
                        )
                        .unwrap(),
                    ),
                ));
                // Make the loan an actual memory payload, not a dead SSA aggregate.
                let destination = SemanticPlaceV1::new(
                    SemanticLocalIdV1::from_index(5),
                    vec![
                        SemanticProjectionV1::new(
                            SemanticProjectionKindV1::ConstantIndex {
                                offset: 0,
                                minimum_length: 1,
                                from_end: false,
                            },
                            raw_type,
                        )
                        .unwrap(),
                    ],
                    raw_type,
                )
                .unwrap();
                statements.push(SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                        destination,
                        SemanticOperandV1::Copy(place(4, raw_type)),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ));
                statements.push(SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(5)),
                ));
                statements.push(SemanticStatementV1::new(
                    source(),
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(4)),
                ));
            }
        }
        if generation == 1 && mode == SourceArrayModeV29::ExpiredRaw {
            let dereference = SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(4),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, element)
                        .unwrap(),
                ],
                element,
            )
            .unwrap();
            statements.push(assign(
                place(3, element),
                SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                    dereference,
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            ));
        }
        statements.push(SemanticStatementV1::new(
            source(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(2)),
        ));
    }
    functions[2] = function(
        210,
        helper.role(),
        helper.abi().clone(),
        locals,
        vec![block(214, statements, SemanticTerminatorKindV1::Return)],
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

#[test]
fn pointer_array_fixture_type_rosters_admit_in_every_supported_mode() {
    struct Restore((u16, u64, bool), SourceArrayModeV29);
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_ARRAY_CASE_V29.set(self.0);
            SOURCE_ARRAY_MODE_V29.set(self.1);
        }
    }
    let _restore = Restore(SOURCE_ARRAY_CASE_V29.get(), SOURCE_ARRAY_MODE_V29.get());
    for (mode, raw) in [
        (SourceArrayModeV29::ThinPointer, false),
        (SourceArrayModeV29::ThinPointer, true),
        (SourceArrayModeV29::PointerAddresses, false),
        (SourceArrayModeV29::StoredBorrow, false),
    ] {
        SOURCE_ARRAY_CASE_V29.set((64, 2, raw));
        SOURCE_ARRAY_MODE_V29.set(mode);
        let owner = original_scalar_array_owner_v29();
        assert!(owner.source_semantic().types().len() >= 5);
    }
}

fn with_original_scalar_array_plan_v29(
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_original_array_plan_from_v29(original_scalar_array_owner_v29, consume)
}

#[test]
fn original_object_lane_keeps_exact_legacy_arrays_ordinary() {
    struct Restore((u16, u64, bool), SourceArrayModeV29);
    impl Drop for Restore {
        fn drop(&mut self) {
            SOURCE_ARRAY_CASE_V29.set(self.0);
            SOURCE_ARRAY_MODE_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SOURCE_ARRAY_CASE_V29.replace((32, 2, false)),
        SOURCE_ARRAY_MODE_V29.replace(SourceArrayModeV29::Scalar),
    );
    let mut completed = false;
    with_original_scalar_array_plan_v29(|plan, budget| {
        let objects: Vec<_> = plan.cells.rows.iter().filter(|row| matches!(row.kind, SourceBackingKindV29::Object(_))).collect();
        let helpers: BTreeSet<_> = objects.iter().map(|row| row.instance.index()).collect();
        assert_eq!(helpers.len(), 2);
        assert!(objects.len() >= 4, "both original activation generations of both helper instances remain selected");
        for row in &objects {
            assert_eq!(row.local.index(), 2);
            let original = plan.instances.instance(row.instance).unwrap();
            assert_eq!(original.function().index(), 2);
            assert_eq!(original.declaration().blocks()[0].statements().iter().filter(|statement|
                matches!(statement.kind(), SemanticStatementKindV1::StorageLive(local) if local.index() == 2)).count(), 2);
        }
        let storage = budget.storage();
        assert_eq!(source_physical_object_count_v29(plan, budget)?, 0);
        assert_eq!(budget.storage(), storage);
        scoped_raw_admission_v29::require_original_zero_raw_v29(plan.instances, Some(plan), budget)?;
        assert_eq!(budget.storage(), storage);
        completed = true;
        Ok(())
    }).unwrap();
    assert!(completed);
}

fn with_original_array_plan_from_v29(
    factory: fn() -> ProductionSemanticSsaOwnerV1,
    consume: impl FnOnce(
        &SourceReferencePlanV29<'_, '_>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut owner = factory();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
    let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
    budget.reserve_storage(101)?;
    let capture = owner
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), 101);
    budget.reserve_storage(capture.retained_storage())?;
    let demands =
        source_storage_demands_v29::SourceStorageDemandsV29::collect(&owner, &mut budget)?;
    let mut layouts = source_storage_v29::SourceStorageLayoutsV29::new_with_limits(
        &owner,
        demands.types(&owner, &mut budget)?,
        ProductionSemanticKirLimitsV1::default().storage_layout_limits(),
        &mut budget,
    )?;
    let credit = layouts.capture_emission_credit(&owner, &mut budget)?;
    let result = production_call_instances_v1::with_production_call_instances_v1(
        &owner,
        SemanticFunctionIdV1::from_index(0),
        &mut budget,
        |instances, budget| {
            let lens = demands.root_lens(&owner, 0, budget).unwrap();
            Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                with_source_reference_descriptor_demands_scope_v29(
                    instances,
                    SourceReferenceStorageV29::ScalarCells,
                    Some(&mut layouts),
                    None,
                    Some(lens),
                    budget,
                    |plan, _, budget| consume(plan, budget).map_err(Into::into),
                ),
            )
        },
    )
    .unwrap();
    let scratch = credit.into_root_credit(&layouts, &budget).unwrap();
    assert!(layouts.permits_root_emission_refund(&owner, scratch, &budget));
    budget.release_storage(scratch)?;
    let cleanup = layouts.release(&mut budget);
    let demand_cleanup = demands.discard(&mut budget);
    drop(owner);
    budget.release_storage(capture.retained_storage())?;
    assert_eq!(budget.storage(), 101);
    result.and(cleanup).and(demand_cleanup)
}

fn observe_original_array_emission_v29(
    _source: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let (bits, length, raw) = SOURCE_ARRAY_CASE_V29.get();
    assert!(!raw);
    let mut helpers = BTreeSet::new();
    for slot in &slots.slots {
        let ScopedAllocationSourceV29::OriginalArray { .. } = slot.origin.source else {
            continue;
        };
        let scalar = slot.scalar_array()?;
        assert_eq!(
            slot.origin.identity,
            ScopedAllocationIdentityV29::LegacyLocal(2)
        );
        assert_eq!(
            (scalar.length, scalar.bytes, scalar.element.size),
            (length, length * u64::from(bits / 8), u64::from(bits / 8))
        );
        assert!(scalar.count.is_some());
        assert!(helpers.insert(slot.instance.index()));
        let original = instances.instance(slot.instance).unwrap();
        assert_eq!(original.function().index(), 2);
        assert_eq!(original.declaration().blocks()[0].statements().iter().filter(|statement|
            matches!(statement.kind(), SemanticStatementKindV1::StorageLive(local) if local.index() == 2)).count(), 2);
        let lowered = emitted[slot.instance.index()].as_ref().unwrap();
        let actual = &lowered.function.body.as_ref().unwrap().blocks[slot.allocation.block_ordinal]
            .operations[slot.allocation.operation];
        check_scoped_slot_alloca_v29(slot, actual, budget)?;
        assert!(matches!(
            actual.kind,
            OperationKind::Alloca {
                count: Some(_),
                element: Type::Scalar(_),
                ..
            }
        ));
    }
    let root = instances
        .instance(instances.root())
        .unwrap()
        .function()
        .index();
    let expected = match root {
        0 => 2,
        1 => 0,
        _ => panic!("unexpected source root {root}"),
    };
    assert_eq!(
        helpers.len(),
        expected,
        "only the calling root owns the two helper arrays"
    );
    SOURCE_ARRAY_OBSERVED_V29.set(SOURCE_ARRAY_OBSERVED_V29.get() + helpers.len());
    Ok(())
}

#[test]
fn actual_source_arrays_keep_all_generations_and_use_exact_scalar_allocations() {
    struct Restore(Option<ScopedSlotObserverV29>);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_OBSERVER_V29.set(self.0);
        }
    }
    for (bits, length) in [(8, 2), (32, 5), (64, 3)] {
        SOURCE_ARRAY_CASE_V29.set((bits, length, false));
        SOURCE_ARRAY_OBSERVED_V29.set(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared =
            scalar_payload_prepared_from_v18(original_scalar_array_owner_v29, &mut budget);
        let _restore =
            Restore(SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_original_array_emission_v29)));
        let reached = std::cell::Cell::new(false);
        prepared
            .with_source_consumer_v18(&mut budget, |source, budget| -> SourceOwnedResultV18<()> {
                source.with_analysis_v18(budget, |scope| {
                    scope.with_inventory_v1(|inventory, budget| {
                        source.with_ranked_correspondence_v18(
                            inventory,
                            budget,
                            |relation, budget| {
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
                                                allocation.slot.scalar_array().unwrap().length,
                                                length
                                            );
                                            count += 1;
                                        }
                                    }
                                }
                                assert_eq!(count, 2);
                                reached.set(true);
                                Ok(())
                            },
                        )
                    })
                })
            })
            .unwrap();
        assert!(reached.get());
        // Admission, reconstruction and consumer replay each emit two helpers.
        assert_eq!(SOURCE_ARRAY_OBSERVED_V29.get(), 3 * 2);
        assert_eq!(budget.storage(), MODULE_FLOOR);
    }
}

#[test]
fn original_array_dispositions_preserve_cells_and_intersect_raw_generations() {
    for raw in [false, true] {
        SOURCE_ARRAY_CASE_V29.set((32, 3, raw));
        with_original_scalar_array_plan_v29(|plan, budget| {
            with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
                let mut cells = 0;
                let mut helpers = BTreeSet::new();
                for (ordinal, cell) in plan.cells.rows.iter().enumerate() {
                    if cell.local.index() != 2
                        || !matches!(cell.kind, SourceBackingKindV29::Object(_))
                    {
                        continue;
                    }
                    cells += 1;
                    helpers.insert(cell.instance.index());
                    let view = SourceFunctionBackingViewV29 {
                        layouts,
                        instance: cell.instance,
                    };
                    assert_eq!(
                        view.cell(cell.local, cell.generation, budget)?.unwrap().0,
                        ordinal
                    );
                    let schema = view.array_schema(cell.local.index(), budget)?;
                    assert_eq!(
                        schema.is_some(),
                        !raw,
                        "one address-bearing generation keeps the entire local typed"
                    );
                    assert!(
                        matches!(cell.kind, SourceBackingKindV29::Object(_)),
                        "the original cell kind never becomes a scalar loan cell"
                    );
                }
                assert_eq!(helpers.len(), 2);
                assert!(
                    cells >= 4,
                    "all original activation generations remain in the census"
                );
                Ok(())
            })
        })
        .unwrap();
    }
}

fn original_array_coverage_rows_v29(
    plan: &SourceReferencePlanV29<'_, '_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(Vec<ScopedSourceSlotV29>, Vec<bool>), ProductionSemanticKirErrorV1> {
    let mut order = BTreeMap::new();
    source_reference_owned_prepay_v29::<BTreeMap<(usize, u32), ScopedSourceSlotV29>>(plan, budget)?;
    let mut seen = emission_vec_v1(plan.cells.rows.len(), budget)?;
    for (cell, row) in plan.cells.rows.iter().enumerate() {
        let Some((schema, facts)) = source_array_cell_facts_v29(plan, cell, budget)? else {
            seen.push(matches!(row.kind, SourceBackingKindV29::Object(_)));
            continue;
        };
        seen.push(false);
        let key = (row.instance.index(), row.local.index());
        charge_execution_cfg_lookup_v29(order.len(), budget)?;
        if order.contains_key(&key) {
            continue;
        }
        reserve_execution_cfg_map_entry_v29::<(usize, u32), ScopedSourceSlotV29>(
            order.len(),
            budget,
        )?;
        let value = u32::try_from(order.len()).unwrap();
        let location = PrivateArrayPhysicalLocationV1 {
            block_ordinal: 0,
            block: BlockId(0),
            operation: value as usize,
        };
        order.insert(
            key,
            ScopedSourceSlotV29 {
                instance: row.instance,
                origin: ScopedSlotOriginV29 {
                    identity: ScopedAllocationIdentityV29::LegacyLocal(row.local.index()),
                    source: ScopedAllocationSourceV29::OriginalArray { schema },
                    semantic_type: row.ty,
                    pointer: ValueId(value),
                },
                representation: ScopedSlotRepresentationV29::ScalarArray(
                    ScopedScalarArraySlotV29 {
                        element_type: facts.element_type,
                        element: facts.element,
                        length: facts.length,
                        bytes: facts.element.size * facts.length,
                        count: Some((ValueId(value + 100), location)),
                    },
                ),
                allocation: location,
            },
        );
    }
    let mut slots = emission_vec_v1(order.len() + 1, budget)?;
    slots.extend(order.into_values());
    Ok((slots, seen))
}

#[test]
fn original_array_cell_coverage_rejects_missing_mixed_and_changed_source_geometry() {
    SOURCE_ARRAY_CASE_V29.set((32, 3, false));
    for fault in 0..16 {
        with_original_scalar_array_plan_v29(|plan, budget| {
            // These rows test only the original-cell join. The separate prepared
            // source test supplies the actual operation/currentness qualification.
            let (mut slots, mut seen) = original_array_coverage_rows_v29(plan, budget)?;
            assert_eq!(slots.len(), 2);
            match fault {
                0 => {}
                1 => {
                    slots.pop();
                }
                2 => slots[0].origin.source = ScopedAllocationSourceV29::Legacy,
                3 => {
                    slots[0].origin.source = ScopedAllocationSourceV29::OriginalArray {
                        schema: fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX),
                    }
                }
                4 => slots[0].origin.semantic_type = UNIT,
                5 => {
                    if let ScopedSlotRepresentationV29::ScalarArray(row) =
                        &mut slots[0].representation
                    {
                        row.length += 1;
                    }
                }
                6 => {
                    if let ScopedSlotRepresentationV29::ScalarArray(row) =
                        &mut slots[0].representation
                    {
                        row.bytes += 1;
                    }
                }
                7 => {
                    if let ScopedSlotRepresentationV29::ScalarArray(row) =
                        &mut slots[0].representation
                    {
                        row.element.size += 1;
                    }
                }
                8 => {
                    if let ScopedSlotRepresentationV29::ScalarArray(row) =
                        &mut slots[0].representation
                    {
                        row.element.alignment *= 2;
                    }
                }
                9 => {
                    if let ScopedSlotRepresentationV29::ScalarArray(row) =
                        &mut slots[0].representation
                    {
                        row.element_type = UNIT;
                    }
                }
                10 => slots[0].origin.identity = ScopedAllocationIdentityV29::LegacyLocal(u32::MAX),
                11 => slots[0].instance = plan.root,
                12 => slots.push(slots[0]),
                13 => {
                    let cell = plan
                        .cells
                        .rows
                        .iter()
                        .position(|row| {
                            row.instance == slots[0].instance
                                && row.local.index() == slots[0].legacy_local().unwrap()
                        })
                        .unwrap();
                    seen[cell] = true;
                }
                14 => {
                    let mut extra = *slots.last().unwrap();
                    extra.origin.identity = ScopedAllocationIdentityV29::LegacyLocal(u32::MAX);
                    slots.push(extra);
                }
                15 => {
                    if let ScopedSlotRepresentationV29::ScalarArray(row) =
                        &mut slots[0].representation
                    {
                        row.count = None;
                    }
                }
                _ => unreachable!(),
            }
            let mut original_seen = emission_vec_v1(seen.len(), budget)?;
            original_seen.extend_from_slice(&seen);
            let storage = budget.storage();
            let result = check_source_array_cell_coverage_v29(plan, &slots, &mut seen, budget);
            assert_eq!(result.is_ok(), fault == 0, "fault {fault}");
            assert_eq!(
                budget.storage(),
                storage,
                "coverage owns no retained source or schema rows"
            );
            if fault != 0 {
                assert_eq!(
                    seen, original_seen,
                    "failed validation cannot publish coverage fault {fault}"
                );
            } else {
                for (cell, covered) in plan.cells.rows.iter().zip(&seen) {
                    assert_eq!(
                        *covered,
                        matches!(cell.kind, SourceBackingKindV29::Object(_))
                    );
                }
            }
            Ok(())
        })
        .unwrap();
    }
}

#[test]
fn original_array_backing_locator_requires_eligible_exact_source_and_unique_representation() {
    SOURCE_ARRAY_CASE_V29.set((32, 3, false));
    for fault in 0..8 {
        with_original_scalar_array_plan_v29(|plan, budget| {
            let eligibility = source_array_eligibility_v29(plan, budget)?;
            let (mut slots, _) = original_array_coverage_rows_v29(plan, budget)?;
            let cell = plan.cells.rows.iter().position(|row|
                row.instance == slots[0].instance && row.local.index() == slots[0].legacy_local().unwrap()
            ).unwrap();
            let mut eligible = eligibility[cell];
            assert!(eligible.is_some());
            match fault {
                0 => {},
                1 => eligible = None,
                2 => eligible = Some(fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX)),
                3 => if let ScopedSlotRepresentationV29::ScalarArray(row) = &mut slots[0].representation { row.count = None; },
                4 => if let ScopedSlotRepresentationV29::ScalarArray(row) = &mut slots[0].representation { row.length += 1; },
                5 => if let ScopedSlotRepresentationV29::ScalarArray(row) = &mut slots[0].representation { row.bytes += 1; },
                6 => slots[0].origin.semantic_type = SemanticTypeIdV1::from_index(u32::MAX),
                7 => {
                    let mut duplicate = slots[0];
                    duplicate.origin.identity = ScopedAllocationIdentityV29::OriginalObject {
                        local: plan.cells.rows[cell].local.index(),
                        generation: plan.cells.rows[cell].generation,
                    };
                    slots.push(duplicate);
                    slots.sort_unstable_by_key(|slot| (slot.instance.index(), slot.origin.identity));
                },
                _ => unreachable!(),
            }
            let result = source_array_cell_slot_v29(plan, cell, eligible, &slots, budget);
            if fault == 0 {
                assert_eq!(result?, Some(0));
                let original = plan.cells.rows[cell];
                let mut generations = 0;
                for (ordinal, row) in plan.cells.rows.iter().enumerate() {
                    if row.instance != original.instance || row.local != original.local { continue; }
                    assert_eq!(source_array_cell_slot_v29(plan, ordinal, eligibility[ordinal], &slots, budget)?, Some(0));
                    assert!(matches!(row.kind, SourceBackingKindV29::Object(_)));
                    generations += 1;
                }
                assert!(generations >= 2, "the allocation is shared, not the original lifetime identity");
            } else {
                assert!(matches!(result, Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "typed allocation identity or representation requires its exact source contract", ..
                })), "fault {fault}: {result:?}");
            }
            Ok(())
        }).unwrap();
    }
}

#[test]
fn original_array_generation_intersection_has_independent_work_and_header_boundaries() {
    SOURCE_ARRAY_CASE_V29.set((32, 3, false));
    for storage_cut in [false, true] {
        for slack in [0, 1] {
            let result = with_original_scalar_array_plan_v29(|plan, budget| {
                type Key = (usize, u32, u32);
                type Map = BTreeMap<Key, SourceBackingAllocationV29>;
                let schema = plan
                    .cells
                    .rows
                    .iter()
                    .find_map(|row| {
                        if let SourceBackingKindV29::Object(schema) = row.kind {
                            Some(schema)
                        } else {
                            None
                        }
                    })
                    .unwrap();
                source_reference_owned_prepay_v29::<Map>(plan, budget)?;
                let mut map = Map::new();
                for generation in [0, 1, 5] {
                    let n = map.len();
                    let entry = (n.checked_ilog2().unwrap_or(0) as usize + 2)
                        * 32
                        * std::mem::size_of::<(Key, SourceBackingAllocationV29, usize)>();
                    let before = budget.storage();
                    reserve_execution_cfg_map_entry_v29::<Key, SourceBackingAllocationV29>(
                        n, budget,
                    )?;
                    assert_eq!(
                        budget.storage() - before,
                        entry,
                        "actual enlarged map value is prepaid"
                    );
                    map.insert(
                        (1, 2, generation),
                        SourceBackingAllocationV29 {
                            cell: n,
                            array: Some(schema),
                        },
                    );
                }
                let header = std::mem::size_of::<Option<Key>>()
                    + 2 * std::mem::size_of::<Result<Option<Key>, ProductionSemanticKirErrorV1>>();
                // Original owner5, three independent tree lookups at n=3 (48
                // each), and three two-comparison generation rows: 155.
                let work = 155;
                if storage_cut {
                    budget.reserve_storage(MODULE_LIMIT - budget.storage() - (header - slack))?;
                } else {
                    budget.charge_work(MODULE_LIMIT - budget.work() - (work - slack))?;
                }
                let before_work = budget.work();
                let before_storage = budget.storage();
                let result = intersect_source_array_generations_v29(plan, &mut map, budget);
                assert_eq!(result.is_ok(), slack == 0);
                assert!(map.values().all(|row| row.array == Some(schema)));
                if slack == 0 {
                    assert_eq!(budget.work() - before_work, work);
                    assert_eq!(budget.storage() - before_storage, header);
                } else if storage_cut {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Storage(_)
                            )
                        )
                    ));
                    assert_eq!(budget.work() - before_work, 5);
                    assert_eq!(budget.storage(), before_storage);
                } else {
                    assert!(matches!(
                        result,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Work(_)
                            )
                        )
                    ));
                    assert_eq!(
                        budget.work() - before_work,
                        107,
                        "the final48-unit lookup is rejected atomically"
                    );
                    assert_eq!(budget.storage() - before_storage, header);
                }
                Ok(())
            });
            assert_eq!(
                result.is_ok(),
                slack == 0,
                "the original source owner retains resource failure"
            );
        }
    }
}

#[test]
fn original_array_eligibility_prepays_capacity_and_retains_no_partial_publication() {
    SOURCE_ARRAY_CASE_V29.set((32, 3, false));
    for short in [false, true] {
        let result = with_original_scalar_array_plan_v29(|plan, budget| {
            type Row = Option<fe2o3_kernel_ir::StorageLayoutIdV1>;
            let header = std::mem::size_of::<Vec<Row>>()
                + 2 * std::mem::size_of::<Result<Vec<Row>, ProductionSemanticKirErrorV1>>();
            let capacity = plan.cells.rows.len() * std::mem::size_of::<Row>();
            assert!(capacity > 0);
            let before_rows = plan.cells.rows.len();
            budget.reserve_storage(
                MODULE_LIMIT - budget.storage() - header - capacity + usize::from(short),
            )?;
            let before_storage = budget.storage();
            let before_work = budget.work();
            let result = source_array_eligibility_v29(plan, budget);
            let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                ArgumentResourceV1::Storage(error),
            )) = result
            else {
                panic!(
                    "capacity/probe-header denial must not publish a successful disposition vector"
                );
            };
            assert_eq!(error.limit(), MODULE_LIMIT);
            assert_eq!(plan.cells.rows.len(), before_rows);
            if short {
                assert_eq!(error.actual(), MODULE_LIMIT + 1);
                assert_eq!(budget.storage() - before_storage, header);
                assert_eq!(
                    budget.work() - before_work,
                    13,
                    "two owner checks and one vector prepayment"
                );
            } else {
                assert!(
                    error.actual() > MODULE_LIMIT,
                    "the next per-cell query envelope, not capacity, is denied"
                );
                assert_eq!(budget.storage() - before_storage, header + capacity);
                assert_eq!(
                    budget.work() - before_work,
                    23,
                    "exact capacity proceeds to both per-cell owner checks"
                );
            }
            Ok(())
        });
        assert!(
            result.is_err(),
            "the original failure owner retains the storage denial"
        );
    }
}

#[test]
fn original_array_disposition_paid_queries_settle_callback_error_and_panic_credit() {
    SOURCE_ARRAY_CASE_V29.set((32, 3, false));
    for panic_callback in [false, true] {
        let reached = std::cell::Cell::new(false);
        let result = with_original_scalar_array_plan_v29(|plan, budget| {
            with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
                let before = budget.storage();
                let eligible = source_array_eligibility_v29(plan, budget)?;
                assert!(eligible.iter().any(Option::is_some));
                let (&(instance, local, _), _) = layouts
                    .backing
                    .iter()
                    .find(|(_, row)| row.array.is_some())
                    .unwrap();
                let view = SourceFunctionBackingViewV29 {
                    layouts,
                    instance: plan.instances.id_at(instance).unwrap(),
                };
                assert!(view.array_schema(local, budget)?.is_some());
                assert!(budget.storage() > before);
                reached.set(true);
                if panic_callback {
                    panic!("original array callback credit probe");
                }
                Err(source_reference_error_v29(
                    "original array callback credit refusal",
                ))
            })
        });
        assert!(reached.get());
        assert!(
            result.is_err(),
            "bounded source callback error/panic stays a refusal"
        );
    }
}

thread_local! {
    static ORIGINAL_ARRAY_FAULT_COMPLETED_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

fn forge_original_array_origins_v29(
    origins: &[ScopedSlotOriginV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Option<Vec<ScopedSlotOriginV29>>, ProductionSemanticKirErrorV1> {
    let Some(index) = origins
        .iter()
        .position(|row| matches!(row.source, ScopedAllocationSourceV29::OriginalArray { .. }))
    else {
        return Ok(None);
    };
    OBJECT_ORIGIN_MUTATION_REACHED.set(OBJECT_ORIGIN_MUTATION_REACHED.get() + 1);
    source_reference_emission_prepay_v29::<Vec<ScopedSlotOriginV29>>(budget)?;
    let mut changed = emission_vec_v1(origins.len() + 1, budget)?;
    budget.charge_work(origins.len())?;
    changed.extend_from_slice(origins);
    match OBJECT_ORIGIN_MUTATION_CASE.get() {
        0 => changed[index].source = ScopedAllocationSourceV29::Legacy,
        1 => {
            changed[index].source = ScopedAllocationSourceV29::OriginalArray {
                schema: fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX),
            }
        }
        2 => {
            changed[index].identity = ScopedAllocationIdentityV29::OriginalObject {
                local: 2,
                generation: 0,
            }
        }
        3 => changed[index].semantic_type = UNIT,
        4 => {
            changed.remove(index);
        }
        5 => changed.push(changed[index]),
        6 => {
            ORIGINAL_ARRAY_FAULT_COMPLETED_V29.set(ORIGINAL_ARRAY_FAULT_COMPLETED_V29.get() + 1);
            panic!("array origin paid clone panic probe");
        }
        7 => {
            ORIGINAL_ARRAY_FAULT_COMPLETED_V29.set(ORIGINAL_ARRAY_FAULT_COMPLETED_V29.get() + 1);
            return Err(source_reference_error_v29(
                "array origin paid clone refusal",
            ));
        }
        _ => unreachable!(),
    }
    ORIGINAL_ARRAY_FAULT_COMPLETED_V29.set(ORIGINAL_ARRAY_FAULT_COMPLETED_V29.get() + 1);
    Ok(Some(changed))
}

#[test]
fn actual_source_array_census_rejects_forged_provenance_and_reclaims_test_clones() {
    struct Restore(
        Option<ScopedObjectOriginMutationV29>,
        Option<ScopedSlotObserverV29>,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_OBJECT_ORIGIN_MUTATION_V29.set(self.0);
            SCOPED_SLOT_OBSERVER_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SCOPED_OBJECT_ORIGIN_MUTATION_V29.replace(Some(forge_original_array_origins_v29)),
        SCOPED_SLOT_OBSERVER_V29.replace(Some(observe_original_array_emission_v29)),
    );
    SOURCE_ARRAY_CASE_V29.set((32, 3, false));
    for fault in 0..8 {
        OBJECT_ORIGIN_MUTATION_CASE.set(fault);
        OBJECT_ORIGIN_MUTATION_REACHED.set(0);
        ORIGINAL_ARRAY_FAULT_COMPLETED_V29.set(0);
        SOURCE_ARRAY_OBSERVED_V29.set(0);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(MODULE_LIMIT);
        let mut budget = ArgumentBudgetV1::new(&mut work, MODULE_LIMIT);
        budget.reserve_storage(MODULE_FLOOR).unwrap();
        let prepared =
            scalar_payload_prepared_from_v18(original_scalar_array_owner_v29, &mut budget);
        let final_consumer_reached = std::cell::Cell::new(false);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            prepared.with_source_consumer_v18(&mut budget, |_, _| -> SourceOwnedResultV18<()> {
                final_consumer_reached.set(true);
                Err(ProductionSourceOwnedViewErrorV18::Binding(
                    "forged original array reached final source admission",
                ))
            })
        }));
        let expected_captures = if fault <= 1 { 2 } else { 1 };
        assert_eq!(
            OBJECT_ORIGIN_MUTATION_REACHED.get(),
            expected_captures,
            "fault {fault}: provenance/schema are checked at the complete two-helper join; malformed rows fail earlier"
        );
        assert_eq!(
            ORIGINAL_ARRAY_FAULT_COMPLETED_V29.get(),
            expected_captures,
            "fault {fault} must complete the paid mutation, not fail during fixture setup"
        );
        assert!(
            !final_consumer_reached.get(),
            "fault {fault} reached final source admission"
        );
        assert_eq!(SOURCE_ARRAY_OBSERVED_V29.get(), 0);
        match result {
            Ok(result) => {
                let expected = match fault {
                    0..=2 => {
                        "typed allocation identity or representation requires its exact source contract"
                    }
                    3..=5 => "scoped source-slot allocation census is incomplete or mismatched",
                    6 => "source reference callback panicked",
                    7 => "array origin paid clone refusal",
                    _ => unreachable!(),
                };
                assert!(
                    matches!(&result, Err(ProductionSourceOwnedViewErrorV18::Source(
                    ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
                        function: 0, block: None, statement: None, detail,
                    }))) if *detail == expected),
                    "fault {fault} requires its exact semantic refusal: {result:?}"
                );
            }
            Err(payload) => {
                assert_eq!(fault, 6, "only the intentional paid-clone panic may unwind");
                assert_eq!(
                    payload.downcast_ref::<&'static str>(),
                    Some(&"array origin paid clone panic probe")
                );
            }
        }
        assert_eq!(
            budget.storage(),
            MODULE_FLOOR,
            "fault {fault} owns neither source nor clone storage after refusal"
        );
    }
}

struct RestoreSourceArrayModeV29(SourceArrayModeV29);
impl Drop for RestoreSourceArrayModeV29 {
    fn drop(&mut self) {
        SOURCE_ARRAY_MODE_V29.set(self.0);
    }
}

#[test]
fn original_thin_pointer_array_schema_is_exact_and_selected_address_facts_stay_object() {
    use fe2o3_kernel_ir::StorageLayoutKindV1 as Kind;
    for mode in [
        SourceArrayModeV29::ThinPointer,
        SourceArrayModeV29::PointerAddresses,
    ] {
        let _restore = RestoreSourceArrayModeV29(SOURCE_ARRAY_MODE_V29.replace(mode));
        SOURCE_ARRAY_CASE_V29.set((64, 3, false));
        with_original_scalar_array_plan_v29(|plan, budget| {
            let layouts = plan
                .storage_root
                .as_ref()
                .unwrap()
                .source_layouts(plan.instances, budget)?;
            let mut checked = 0;
            for (cell, row) in plan.cells.rows.iter().enumerate() {
                if row.local.index() != 2 {
                    continue;
                }
                let SourceBackingKindV29::Object(schema) = row.kind else {
                    panic!("original array stays Object in the backing census");
                };
                let facts = source_array_cell_facts_v29(plan, cell, budget)?;
                let original = layouts
                    .original_schema(plan.instances.owner(), row.ty, budget)?
                    .unwrap();
                let rows = layouts.rows(plan.instances.owner(), budget)?;
                let Kind::Array {
                    element,
                    length: 3,
                    stride: 8,
                } = rows[schema.0 as usize].kind
                else {
                    panic!("exact selected pointer array");
                };
                let Kind::Pointer(pointer) = &rows[element.0 as usize].kind else {
                    panic!("exact selected pointer field");
                };
                assert_eq!(
                    pointer.encoded_space,
                    AddressSpace::Generic,
                    "original source encoding never follows selected value provenance"
                );
                assert_eq!(pointer.stored_bits, 64);
                assert_eq!(pointer.access, AccessMode::ReadOnly);
                if mode == SourceArrayModeV29::ThinPointer {
                    assert_eq!(schema, original);
                    let (actual, facts) = facts.unwrap();
                    assert_eq!(actual, schema);
                    assert!(matches!(
                        facts.element.element,
                        PrivateRetainedElementFactsV1::ThinPointer {
                            element: ScalarType::U64,
                            space: AddressSpace::Generic,
                            access: AccessMode::ReadOnly,
                        }
                    ));
                    assert_eq!(pointer.value_space, AddressSpace::Generic);
                } else {
                    // Entry activation has no assigned value. Written generations
                    // retain the actual private address without an AS0 reload.
                    if row.generation != 0 {
                        assert_ne!(schema, original);
                        assert_eq!(pointer.value_space, AddressSpace::Private);
                        assert!(facts.is_none());
                    }
                }
                checked += 1;
            }
            assert!(checked >= 4);
            with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
                let mut original_instances = BTreeSet::new();
                for cell in &plan.cells.rows {
                    if cell.local.index() == 2 {
                        original_instances.insert(cell.instance.index());
                        let view = SourceFunctionBackingViewV29 {
                            layouts,
                            instance: cell.instance,
                        };
                        // Exact pointer layout facts remain available, but
                        // pointer payloads require typed object admission.
                        assert!(view.array_schema(2, budget)?.is_none());
                    }
                }
                assert_eq!(original_instances.len(), 2);
                Ok(())
            })
        })
        .unwrap();
    }
}

#[test]
fn stored_loan_array_requirements_keep_every_generation_on_the_object_route() {
    let _restore =
        RestoreSourceArrayModeV29(SOURCE_ARRAY_MODE_V29.replace(SourceArrayModeV29::StoredBorrow));
    SOURCE_ARRAY_CASE_V29.set((32, 3, false));
    with_original_scalar_array_plan_v29(|plan, budget| {
        let mut required = 0;
        for strategy in &plan.cells.strategies {
            if let SourceReferenceCellStrategyV29::Object(cell) = *strategy
                && plan.cells.rows[cell].local.index() == 2
            {
                required += 1;
            }
        }
        assert_eq!(
            required, 2,
            "both original helper invocations store an actual array-payload loan"
        );
        with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
            let mut generations = 0;
            for (ordinal, cell) in plan.cells.rows.iter().enumerate() {
                if cell.local.index() != 2 {
                    continue;
                }
                let view = SourceFunctionBackingViewV29 {
                    layouts,
                    instance: cell.instance,
                };
                assert_eq!(
                    view.cell(cell.local, cell.generation, budget)?.unwrap().0,
                    ordinal
                );
                assert!(view.array_schema(2, budget)?.is_none());
                generations += 1;
            }
            assert!(generations >= 4);
            Ok(())
        })
    })
    .unwrap();
}

#[test]
fn original_array_storage_reactivation_does_not_resurrect_a_raw_payload_address() {
    let _restore =
        RestoreSourceArrayModeV29(SOURCE_ARRAY_MODE_V29.replace(SourceArrayModeV29::ExpiredRaw));
    SOURCE_ARRAY_CASE_V29.set((32, 3, true));
    let reached = std::cell::Cell::new(false);
    let error = with_original_scalar_array_plan_v29(|_, _| {
        reached.set(true);
        Ok(())
    })
    .unwrap_err();
    assert!(
        !reached.get(),
        "the actual source activation relation rejects the expired address before representation selection"
    );
    assert!(
        format!("{error:?}").contains("source raw pointer outlived its storage activation"),
        "{error:?}"
    );
}

fn original_entry_array_owner_v29() -> ProductionSemanticSsaOwnerV1 {
    let template = original_scalar_array_owner_v29();
    let semantic = template.source_semantic();
    let mut functions = semantic.functions().to_vec();
    let array = functions[2].locals()[2].ty();
    let SemanticTypeShapeV1::Array { element, length } =
        *semantic.types()[array.index() as usize].shape()
    else {
        unreachable!()
    };
    let layout = semantic.types()[array.index() as usize].layout();
    let literal = || {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            element,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(37, 4).unwrap()),
        ))
    };
    let original = &functions[0];
    let mut locals = original.locals().to_vec();
    let root_array = locals.len() as u32;
    locals.push(local(224, array, SemanticLocalRoleV1::Temporary));
    let mut blocks = Vec::new();
    for (ordinal, old) in original.blocks().iter().enumerate() {
        let statements = if ordinal < 2 {
            vec![assign(
                place(root_array, array),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Array,
                        (0..length).map(|_| literal()).collect(),
                    )
                    .unwrap(),
                ),
            )]
        } else {
            vec![]
        };
        let terminator = if ordinal < 2 {
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(2),
                    vec![SemanticOperandV1::Copy(place(root_array, array))],
                    Some(SemanticCallDestinationV1::new(
                        place(0, UNIT),
                        SemanticControlFlowEdgeV1::new(
                            SemanticEdgeRoleV1::CallReturn,
                            SemanticBlockIdV1::from_index(ordinal as u32 + 1),
                        ),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            )
        } else {
            old.terminator().kind().clone()
        };
        blocks.push(block(225 + ordinal as u8, statements, terminator));
    }
    functions[0] = function(60, original.role(), original.abi().clone(), locals, blocks)
        .with_kernel_entry(original.kernel_entry().unwrap().clone());
    let mode = SemanticAbiPassModeV1::Indirect {
        attributes: SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesNone),
                true,
                false,
                false,
                true,
            ),
            SemanticAbiExtensionV1::None,
            layout.size_bytes().unwrap(),
            Some(layout.alignment_bytes()),
        )
        .unwrap(),
        metadata_attributes: None,
        on_stack: false,
    };
    let helper_abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([228; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::Rust,
        SemanticExternAbiV1::Rust,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            array, mode,
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue])
    .unwrap();
    let indexed = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: length - 1,
                    minimum_length: length,
                    from_end: false,
                },
                element,
            )
            .unwrap(),
        ],
        element,
    )
    .unwrap();
    functions[2] = function(
        210,
        SemanticFunctionRoleV1::InternalHelper,
        helper_abi,
        vec![
            local(212, UNIT, SemanticLocalRoleV1::Return),
            local(213, array, SemanticLocalRoleV1::Argument(0)),
        ],
        vec![block(
            214,
            vec![assign(indexed, SemanticRvalueKindV1::Use(literal()))],
            SemanticTerminatorKindV1::Return,
        )],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
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

#[test]
fn original_entry_array_keeps_its_entry_materialization_obligation() {
    SOURCE_ARRAY_CASE_V29.set((32, 3, false));
    with_original_array_plan_from_v29(original_entry_array_owner_v29, |plan, budget| {
        with_execution_instance_layouts_v29(plan, budget, |layouts, budget| {
            let mut checked = 0;
            for (ordinal, cell) in plan.cells.rows.iter().enumerate() {
                let instance = plan.instances.instance(cell.instance).unwrap();
                if instance.function().index() != 2 || cell.local.index() != 1 {
                    continue;
                }
                assert!(matches!(cell.kind, SourceBackingKindV29::Object(_)));
                assert!(
                    instance.declaration().locals()[1]
                        .role()
                        .is_entry_argument()
                );
                assert!(source_array_cell_facts_v29(plan, ordinal, budget)?.is_none());
                let view = SourceFunctionBackingViewV29 {
                    layouts,
                    instance: cell.instance,
                };
                assert_eq!(
                    view.cell(cell.local, cell.generation, budget)?.unwrap().0,
                    ordinal
                );
                assert!(view.array_schema(1, budget)?.is_none());
                checked += 1;
            }
            assert_eq!(checked, 2, "each actual call retains its own entry object");
            Ok(())
        })
    })
    .unwrap();
}

#[test]
fn original_array_slot_counter_storage_has_independent_exact_and_short_boundaries() {
    SOURCE_ARRAY_CASE_V29.set((32, 3, false));
    for short in [false, true] {
        let result = with_original_scalar_array_plan_v29(|plan, budget| {
            let (slots, _) = original_array_coverage_rows_v29(plan, budget)?;
            let count = slots.len();
            assert_eq!(count, 2);
            let header = std::mem::size_of::<Vec<usize>>()
                + 2 * std::mem::size_of::<Result<Vec<usize>, ProductionSemanticKirErrorV1>>();
            let capacity = count * std::mem::size_of::<usize>();
            budget.reserve_storage(
                MODULE_LIMIT - budget.storage() - header - capacity + usize::from(short),
            )?;
            let storage = budget.storage();
            let work = budget.work();
            let result = source_array_coverage_counts_v29(plan, count, budget);
            if short {
                let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Storage(error),
                )) = result
                else {
                    panic!("one-short counter capacity must fail before publication");
                };
                assert_eq!(
                    (error.actual(), error.limit()),
                    (MODULE_LIMIT + 1, MODULE_LIMIT)
                );
                assert_eq!(budget.storage() - storage, header);
                assert_eq!(
                    budget.work() - work,
                    8,
                    "original owner5 plus vector3, no zero-fill on denial"
                );
            } else {
                assert_eq!(result.unwrap(), [0, 0]);
                assert_eq!(budget.storage() - storage, header + capacity);
                assert_eq!(budget.work() - work, 8 + count);
            }
            Ok(())
        });
        assert_eq!(result.is_err(), short);
    }
}

#[test]
fn original_array_final_mark_batch_is_atomic_at_its_independent_work_boundary() {
    SOURCE_ARRAY_CASE_V29.set((32, 3, false));
    for short in [false, true] {
        let result = with_original_scalar_array_plan_v29(|plan, budget| {
            // Only exercise the publication phase here. The full validation
            // tests establish the original source/cell/slot join before it.
            let (slots, mut seen) = original_array_coverage_rows_v29(plan, budget)?;
            let mut before = emission_vec_v1(seen.len(), budget)?;
            before.extend_from_slice(&seen);
            let counts = [1_usize, 1];
            assert_eq!(slots.len(), counts.len());
            let prefix = 5 + 4 + 2 * slots.len();
            let required = prefix + plan.cells.rows.len();
            budget.charge_work(MODULE_LIMIT - budget.work() - required + usize::from(short))?;
            let work = budget.work();
            let storage = budget.storage();
            let result =
                publish_source_array_cell_coverage_v29(plan, &slots, &counts, &mut seen, budget);
            assert_eq!(result.is_err(), short);
            assert_eq!(budget.storage(), storage);
            if short {
                let Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                    ArgumentResourceV1::Work(error),
                )) = result
                else {
                    panic!("the final all-cell batch must fail atomically");
                };
                assert_eq!(
                    (error.actual(), error.limit()),
                    (MODULE_LIMIT + 1, MODULE_LIMIT)
                );
                assert_eq!(budget.work() - work, prefix);
                assert_eq!(seen, before);
            } else {
                assert_eq!(budget.work() - work, required);
                for (row, covered) in plan.cells.rows.iter().zip(&seen) {
                    assert_eq!(
                        *covered,
                        matches!(row.kind, SourceBackingKindV29::Object(_))
                    );
                }
            }
            Ok(())
        });
        assert_eq!(result.is_err(), short);
    }
}

include!("production_source_object_abi_claims_v29_tests.rs");
include!("production_source_object_query_scratch_v29_tests.rs");
include!("production_source_object_storage_check_scratch_v29_tests.rs");
include!("production_source_partial_array_read_v29_tests.rs");

#[path = "production_source_static_raw_holders_v42_tests.rs"]
mod static_raw_holders_v42_tests;
