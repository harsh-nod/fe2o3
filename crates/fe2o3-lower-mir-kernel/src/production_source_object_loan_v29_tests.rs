#[derive(Clone, Copy, Debug)]
struct ObjectLoanCaseV29 {
    mutable: bool,
    wrapped: bool,
    temporary: bool,
    validity: bool,
}

thread_local! {
    static OBJECT_LOAN_CASE_V29: std::cell::Cell<ObjectLoanCaseV29> = const {
        std::cell::Cell::new(ObjectLoanCaseV29 { mutable: false, wrapped: false, temporary: false, validity: false })
    };
    static OBJECT_LOAN_FAULT_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static OBJECT_LOAN_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OBJECT_LOAN_MUTATIONS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OBJECT_LOAN_QUERY_MODE_V29: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static OBJECT_LOAN_QUERY_COMPLETED_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    static OBJECT_LOAN_FORCE_BACKING_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(true) };
    static OBJECT_LOAN_STRATEGY_VISITS_V29: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static OBJECT_LOAN_UNBACKED_KINDS_V29: std::cell::Cell<(usize, usize)> = const { std::cell::Cell::new((0, 0)) };
    static OBJECT_LOAN_LEGACY_CELLS_V29: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn object_loan_original_owner_v29(case: SuffixCase) -> ProductionSemanticSsaOwnerV1 {
    try_object_loan_original_owner_v29(case).unwrap()
}

fn try_object_loan_original_owner_v29(
    case: SuffixCase,
) -> Result<ProductionSemanticSsaOwnerV1, ProductionSemanticSsaErrorV1> {
    let config = OBJECT_LOAN_CASE_V29.get();
    let original = if OBJECT_LOAN_FORCE_BACKING_V29.get() {
        scalar_entry_original_owner_v29(case)
    } else {
        suffix_owner(case)
    };
    let source = original.source_semantic();
    let mut types = source.types().to_vec();
    let scalar = if config.validity {
        assert!(config.temporary);
        declaration(
            &mut types,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(1, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::ValidityScalar(
                SemanticValidityScalarTypeV1::new(
                    SemanticScalarTypeV1::Integer {
                        signed: false,
                        bits: 32,
                    },
                    vec![SemanticScalarValidityRangeV1::new(1, u32::MAX.into())],
                )
                .unwrap(),
            ),
            None,
        )
    } else {
        U32
    };
    let reference_type = reference(
        &mut types,
        scalar,
        if config.mutable {
            SemanticMutabilityV1::Mutable
        } else {
            SemanticMutabilityV1::Immutable
        },
        false,
    );
    let raw_type = (config.temporary && OBJECT_LOAN_FORCE_BACKING_V29.get())
        .then(|| reference(&mut types, scalar, SemanticMutabilityV1::Immutable, true));
    let holder_type = if config.wrapped {
        declaration(
            &mut types,
            SemanticTypeLayoutV1::aggregate(
                Some(16),
                8,
                SemanticAggregateLayoutV1::new(vec![0, 8], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Tuple(
                SemanticAggregateTypeV1::new(vec![U32, reference_type]).unwrap(),
            ),
            None,
        )
    } else {
        reference_type
    };
    let constant = |value| {
        SemanticOperandV1::Constant(SemanticConstantV1::new(
            scalar,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, 4).unwrap()),
        ))
    };
    let mut functions = source.functions().to_vec();
    for index in [0usize, 3] {
        let old = &functions[index];
        let mut locals = old.locals().to_vec();
        let mut statements = old.blocks()[0].statements().to_vec();
        if OBJECT_LOAN_LEGACY_CELLS_V29.get() && index == 3 {
            assert_eq!(case, SuffixCase::Cells);
            assert!(!OBJECT_LOAN_FORCE_BACKING_V29.get());
            assert_eq!(locals.len(), 6);
            // The original helper already writes through both mutable loans.
            // End those holders before the later borrow of argument 1.
            for local in [4, 5] {
                statements.push(SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local)),
                ));
            }
        }
        let mut append = |ty| {
            let id = locals.len() as u32;
            locals.push(local(220 + id as u8, ty, SemanticLocalRoleV1::Temporary));
            id
        };
        let referent = if config.temporary {
            let value = append(scalar);
            let raw = raw_type.map(|ty| (append(ty), ty));
            statements.push(assign(
                place(value, scalar),
                SemanticRvalueKindV1::Use(constant(17)),
            ));
            if let Some((raw, ty)) = raw {
                statements.push(assign(
                    place(raw, ty),
                    SemanticRvalueKindV1::AddressOf {
                        place: place(value, scalar),
                        mutability: SemanticMutabilityV1::Immutable,
                    },
                ));
            }
            value
        } else {
            1
        };
        let borrowed = append(reference_type);
        let holder = if config.wrapped {
            append(holder_type)
        } else {
            borrowed
        };
        let output = append(scalar);
        if OBJECT_LOAN_FAULT_V29.get() == 11 {
            statements.push(SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Deinitialize(place(referent, scalar)),
            ));
        }
        statements.push(assign(
            place(borrowed, reference_type),
            SemanticRvalueKindV1::Borrow {
                kind: if config.mutable {
                    SemanticBorrowKindV1::Mutable
                } else {
                    SemanticBorrowKindV1::Shared
                },
                place: place(referent, scalar),
            },
        ));
        if OBJECT_LOAN_FAULT_V29.get() == 10 {
            statements.push(SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(referent)),
            ));
            statements.push(SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(referent)),
            ));
            statements.push(assign(
                place(referent, scalar),
                SemanticRvalueKindV1::Use(constant(31)),
            ));
        }
        if config.wrapped {
            statements.push(assign(
                place(holder, holder_type),
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Tuple,
                        vec![
                            literal(9),
                            SemanticOperandV1::Move(place(borrowed, reference_type)),
                        ],
                    )
                    .unwrap(),
                ),
            ));
        }
        let mut projections = Vec::new();
        if config.wrapped {
            projections.push(
                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(1), reference_type)
                    .unwrap(),
            );
        }
        projections.push(
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, scalar).unwrap(),
        );
        let dereference =
            SemanticPlaceV1::new(SemanticLocalIdV1::from_index(holder), projections, scalar)
                .unwrap();
        statements.push(assign(place(output, scalar), load(dereference.clone())));
        if config.mutable || OBJECT_LOAN_FAULT_V29.get() == 12 {
            statements.push(store(dereference.clone(), constant(29)));
            statements.push(assign(place(output, scalar), load(dereference)));
        }
        statements.push(SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(holder)),
        ));
        let mut blocks = old.blocks().to_vec();
        blocks[0] = SemanticBasicBlockV1::new(
            blocks[0].identity(),
            blocks[0].source(),
            statements,
            blocks[0].terminator().clone(),
        )
        .unwrap();
        let mut rebuilt = SemanticFunctionDeclV1::new(
            old.identity(),
            old.role(),
            old.item_definition_identity(),
            old.monomorphization_identity(),
            old.generic_type_arguments_identity(),
            old.const_generic_arguments_identity(),
            old.source(),
            old.abi().clone(),
            locals,
            old.entry(),
            blocks,
        )
        .unwrap();
        if let Some(kernel) = old.kernel_entry() {
            rebuilt = rebuilt.with_kernel_entry(kernel.clone());
        }
        assert_eq!(rebuilt.identity(), old.identity());
        assert_eq!(rebuilt.abi(), old.abi());
        assert_eq!(rebuilt.kernel_entry(), old.kernel_entry());
        assert!(
            rebuilt
                .locals()
                .windows(2)
                .all(|pair| pair[0].identity().as_bytes() < pair[1].identity().as_bytes())
        );
        for (before, after) in old.blocks().iter().zip(rebuilt.blocks()) {
            assert_eq!(before.identity(), after.identity());
            assert_eq!(before.source(), after.source());
            assert_eq!(before.terminator(), after.terminator());
        }
        functions[index] = rebuilt;
    }
    scoped_root_tests::fixtures::try_build(types, functions, source.callables().to_vec())
}

fn inspect_object_loans_v29(
    _: &ExecutionLifecycleSourceV29<'_>,
    instances: &ExecutionInstancesV29<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    slots: &OwnedScopedSourceSlotsV29,
    _: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let config = OBJECT_LOAN_CASE_V29.get();
    let fault = OBJECT_LOAN_FAULT_V29.get();
    let mut roots = 0;
    let mut helpers = 0;
    for owner in &slots.instances {
        let declaration = instances.instance(owner.instance).unwrap();
        if !matches!(declaration.function().index(), 0 | 3) {
            continue;
        }
        if declaration.function().index() == 0 {
            roots += 1;
        } else {
            helpers += 1;
        }
        let lowered = emitted[owner.instance.index()].as_mut().unwrap();
        let anchors = lowered.scoped_memory_anchors.as_mut().unwrap();
        let mut reads = 0;
        let mut writes = 0;
        for row in &mut anchors.rows {
            let ScopedMemoryAnchorKindV29::Object(payload_index) = row.kind else {
                continue;
            };
            let payload = &mut anchors.objects[payload_index];
            let (endpoint, read) = match &mut payload.role {
                ScopedObjectRoleV29::ReadValue { source, .. } => (source, true),
                ScopedObjectRoleV29::WriteValue { destination, .. } => (destination, false),
                _ => continue,
            };
            let ScopedObjectIdentityV29::Reference {
                instance,
                site,
                role,
                dereference_prefix,
            } = endpoint.object
            else {
                continue;
            };
            assert_eq!(instance, owner.instance);
            assert_eq!(site, row.source.unwrap().site);
            assert_eq!(dereference_prefix, if config.wrapped { 2 } else { 1 });
            let original =
                scoped_object_original_place_v29(declaration.declaration(), site, role).unwrap();
            assert_eq!(original.projections().len() as u32, dereference_prefix);
            assert_eq!(
                original.projections().last().unwrap().kind(),
                SemanticProjectionKindV1::Dereference
            );
            assert_eq!(original.ty(), endpoint.root_type);
            assert_eq!(endpoint.root_type, endpoint.projected_type);
            assert_eq!(endpoint.root_schema, endpoint.projected_schema);
            assert_eq!(endpoint.path.count, 0);
            assert_eq!(endpoint.source_path.count, dereference_prefix as usize);
            let block = lowered
                .function
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .find(|block| block.id == row.block)
                .unwrap();
            let operation = &mut block.operations[row.position];
            assert_eq!(
                operation.kind,
                OperationKind::Storage(payload.operation.clone())
            );
            let (pointer, access) = match payload.operation {
                ScopedObjectOperationV29::ReadValue { address, access } => {
                    assert!(read);
                    reads += 1;
                    (address, access)
                }
                ScopedObjectOperationV29::WriteValue {
                    address, access, ..
                } => {
                    assert!(!read);
                    writes += 1;
                    (address, access)
                }
                _ => panic!("whole primitive safe loan"),
            };
            assert_eq!(access, MemoryAccess::new(AddressSpace::Private, 1));
            if fault == 1 {
                // Change actual access and its receipt coherently; immutable
                // original effect replay still requires nonvolatile access.
                match &mut payload.operation {
                    ScopedObjectOperationV29::ReadValue { access, .. }
                    | ScopedObjectOperationV29::WriteValue { access, .. } => access.volatile = true,
                    _ => unreachable!(),
                }
                operation.kind = OperationKind::Storage(payload.operation.clone());
            } else if fault == 2 {
                endpoint.projected_schema = fe2o3_kernel_ir::StorageLayoutIdV1(u32::MAX);
            } else if fault == 3 && declaration.function().index() == 3 {
                let alternate = slots.slots[owner.slots.clone()]
                    .iter()
                    .find(|slot| {
                        matches!(
                            slot.origin.identity,
                            ScopedAllocationIdentityV29::OriginalObject {
                                local: 2,
                                generation: 0
                            }
                        )
                    })
                    .unwrap();
                assert_ne!(alternate.origin.pointer, pointer);
                match &mut payload.operation {
                    ScopedObjectOperationV29::ReadValue { address, .. }
                    | ScopedObjectOperationV29::WriteValue { address, .. } => {
                        *address = alternate.origin.pointer
                    }
                    _ => unreachable!(),
                }
                operation.kind = OperationKind::Storage(payload.operation.clone());
            } else if fault == 4 {
                let ScopedObjectSourceV29::Place { prefix, .. } = &mut endpoint.source else {
                    unreachable!()
                };
                *prefix = 1;
                let ScopedObjectIdentityV29::Reference {
                    dereference_prefix, ..
                } = &mut endpoint.object
                else {
                    unreachable!()
                };
                *dereference_prefix = 1;
            }
            if fault != 0 && (fault != 3 || declaration.function().index() == 3) {
                OBJECT_LOAN_MUTATIONS_V29.set(OBJECT_LOAN_MUTATIONS_V29.get() + 1);
            }
        }
        assert_eq!(reads, if config.mutable { 2 } else { 1 });
        assert_eq!(writes, usize::from(config.mutable));
    }
    assert_eq!(roots, 1);
    let expected_helpers = instances
        .instances()
        .iter()
        .filter(|row| row.function().index() == 3)
        .count();
    assert!(expected_helpers >= 1);
    assert_eq!(helpers, expected_helpers);
    OBJECT_LOAN_VISITS_V29.set(OBJECT_LOAN_VISITS_V29.get() + 1);
    Ok(())
}

fn run_object_loans_v29(
    config: ObjectLoanCaseV29,
    case: SuffixCase,
    fault: u8,
    work: usize,
    storage: usize,
) -> (SourceOwnedResultV18<()>, usize, usize, bool) {
    struct Restore(ObjectLoanCaseV29, u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            OBJECT_LOAN_CASE_V29.set(self.0);
            OBJECT_LOAN_FAULT_V29.set(self.1);
        }
    }
    let _restore = Restore(
        OBJECT_LOAN_CASE_V29.replace(config),
        OBJECT_LOAN_FAULT_V29.replace(fault),
    );
    OBJECT_LOAN_VISITS_V29.set(0);
    OBJECT_LOAN_MUTATIONS_V29.set(0);
    run_original_source_fixture_v29(
        || object_loan_original_owner_v29(case),
        false,
        true,
        3,
        inspect_object_loans_v29,
        work,
        storage,
    )
}

#[test]
fn original_object_safe_loans_read_and_write_root_and_repeated_helpers() {
    for mutable in [false, true] {
        for wrapped in [false, true] {
            for case in [SuffixCase::Root, SuffixCase::Loop, SuffixCase::Branch] {
                let config = ObjectLoanCaseV29 {
                    mutable,
                    wrapped,
                    temporary: false,
                    validity: false,
                };
                let (result, _, _, completed) = run_object_loans_v29(config, case, 0, LIMIT, LIMIT);
                assert!(result.is_ok(), "{config:?}/{case:?}: {result:?}");
                assert!(completed);
                assert_eq!(OBJECT_LOAN_VISITS_V29.get(), 3);
                assert_eq!(OBJECT_LOAN_MUTATIONS_V29.get(), 0);
            }
        }
    }
}

#[test]
fn original_object_same_block_scalar_and_validity_initialization_precedes_safe_borrow() {
    for validity in [false, true] {
        for mutable in [false, true] {
            let config = ObjectLoanCaseV29 {
                mutable,
                wrapped: true,
                temporary: true,
                validity,
            };
            let (result, _, _, completed) =
                run_object_loans_v29(config, SuffixCase::Root, 0, LIMIT, LIMIT);
            assert!(result.is_ok(), "{config:?}: {result:?}");
            assert!(completed);
            assert_eq!(OBJECT_LOAN_VISITS_V29.get(), 3);
        }
    }
}

#[test]
fn original_object_safe_loan_transaction_has_exact_and_one_short_resources() {
    for mutable in [false, true] {
        let config = ObjectLoanCaseV29 {
            mutable,
            wrapped: true,
            temporary: false,
            validity: false,
        };
        let (result, work, storage, completed) =
            run_object_loans_v29(config, SuffixCase::Root, 0, LIMIT, LIMIT);
        result.unwrap();
        assert!(completed);
        let exact = run_object_loans_v29(config, SuffixCase::Root, 0, work, storage);
        exact.0.unwrap();
        assert!(exact.3);
        assert_eq!((exact.1, exact.2), (work, storage));
        for (work_limit, storage_limit, is_work) in
            [(work - 1, storage, true), (work, storage - 1, false)]
        {
            let short =
                run_object_loans_v29(config, SuffixCase::Root, 0, work_limit, storage_limit);
            assert!(!short.3);
            match (
                is_work,
                original_repeated_source_resource_v29(short.0.unwrap_err()),
            ) {
                (true, ArgumentResourceV1::Work(error)) => {
                    assert_eq!(error.limit(), work_limit);
                    assert!(error.actual() > work_limit);
                }
                (false, ArgumentResourceV1::Storage(error)) => {
                    assert_eq!(error.limit(), storage_limit);
                    assert!(error.actual() > storage_limit);
                }
                other => panic!("safe-loan exact resource: {other:?}"),
            }
        }
    }
}

#[test]
fn original_object_safe_loan_same_candidate_effect_schema_and_pointer_mutations_refuse() {
    let config = ObjectLoanCaseV29 {
        mutable: true,
        wrapped: true,
        temporary: false,
        validity: false,
    };
    for fault in 1..=4 {
        let (positive, _, _, completed) =
            run_object_loans_v29(config, SuffixCase::Root, 0, LIMIT, LIMIT);
        positive.unwrap();
        assert!(completed);
        let (refused, _, _, completed) =
            run_object_loans_v29(config, SuffixCase::Root, fault, LIMIT, LIMIT);
        assert!(!completed);
        assert!(OBJECT_LOAN_VISITS_V29.get() >= 1);
        assert!(OBJECT_LOAN_MUTATIONS_V29.get() >= 1);
        let expected = match fault {
            1 => "scoped memory anchors differ from their source instance",
            // The helper movement permit independently checks each endpoint's
            // selected source schema before the final typed-memory callback.
            2 => "execution call parameters differ from their source instance",
            4 => "typed object source payload differs from its actual operation",
            3 => "source raw address differs from its actual formation or memory history",
            _ => unreachable!(),
        };
        assert!(
            matches!(refused, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported {
                detail, ..
            }))) if detail == expected),
            "fault {fault}: {refused:?}"
        );
    }
}

#[test]
fn original_object_safe_loan_cannot_cross_storage_restart_even_with_a_new_value() {
    let config = ObjectLoanCaseV29 {
        mutable: false,
        wrapped: true,
        temporary: false,
        validity: false,
    };
    let (positive, _, _, completed) =
        run_object_loans_v29(config, SuffixCase::Root, 0, LIMIT, LIMIT);
    positive.unwrap();
    assert!(completed);
    let (refused, _, _, completed) =
        run_object_loans_v29(config, SuffixCase::Root, 10, LIMIT, LIMIT);
    assert!(!completed);
    assert_eq!(OBJECT_LOAN_VISITS_V29.get(), 0);
    assert_eq!(OBJECT_LOAN_MUTATIONS_V29.get(), 0);
    assert!(
        matches!(
            refused,
            Err(ProductionSourceOwnedViewErrorV18::Source(
                ProductionPendingScopedSourceErrorV29::Source(
                    ProductionSemanticKirErrorV1::Unsupported {
                        detail: "source reference referent storage dies with a live loan",
                        ..
                    }
                )
            ))
        ),
        "{refused:?}"
    );
}

#[test]
fn original_object_safe_loan_requires_initialized_referent_and_original_mutability() {
    let config = ObjectLoanCaseV29 {
        mutable: false,
        wrapped: true,
        temporary: true,
        validity: false,
    };
    for fault in [11, 12] {
        let (positive, _, _, completed) =
            run_object_loans_v29(config, SuffixCase::Root, 0, LIMIT, LIMIT);
        positive.unwrap();
        assert!(completed);
        if fault == 11 {
            struct Restore(ObjectLoanCaseV29, u8);
            impl Drop for Restore {
                fn drop(&mut self) {
                    OBJECT_LOAN_CASE_V29.set(self.0);
                    OBJECT_LOAN_FAULT_V29.set(self.1);
                }
            }
            let _restore = Restore(
                OBJECT_LOAN_CASE_V29.replace(config),
                OBJECT_LOAN_FAULT_V29.replace(fault),
            );
            OBJECT_LOAN_VISITS_V29.set(0);
            OBJECT_LOAN_MUTATIONS_V29.set(0);
            let refused = try_object_loan_original_owner_v29(SuffixCase::Root)
                .err()
                .expect("borrow of a deinitialized original local must fail SSA admission");
            assert!(
                matches!(refused, ProductionSemanticSsaErrorV1::PartialMove {
                function, block: 0, statement: Some(5), local: 4,
                violation: fe2o3_pliron::SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            } if function == SemanticFunctionIdV1::from_index(0)),
                "{refused:?}"
            );
            assert_eq!(OBJECT_LOAN_VISITS_V29.get(), 0);
            assert_eq!(OBJECT_LOAN_MUTATIONS_V29.get(), 0);
            continue;
        }
        let (refused, _, _, completed) =
            run_object_loans_v29(config, SuffixCase::Root, fault, LIMIT, LIMIT);
        assert!(!completed);
        assert_eq!(OBJECT_LOAN_VISITS_V29.get(), 0);
        assert_eq!(OBJECT_LOAN_MUTATIONS_V29.get(), 0);
        assert!(
            matches!(refused, Err(ProductionSourceOwnedViewErrorV18::Source(
            ProductionPendingScopedSourceErrorV29::Source(ProductionSemanticKirErrorV1::Unsupported { detail, .. })))
            if detail == "source reference traversal widens mutability"),
            "fault {fault}: {refused:?}"
        );
    }
}

fn object_loan_holder_headers_v29() -> usize {
    fn h<T>() -> usize {
        std::mem::size_of::<T>()
            + 2 * std::mem::size_of::<Result<T, ProductionSemanticKirErrorV1>>()
    }
    h::<Option<SemanticTypeIdV1>>()
        + h::<Option<&production_call_instances_v1::ProductionCallInstanceV1<'_>>>()
        + h::<&production_call_instances_v1::ProductionCallInstanceV1<'_>>()
        + h::<Option<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>>()
        + h::<&fe2o3_mir_model::semantic_mir_v1::SemanticLocalDeclV1>()
        + h::<Option<&SemanticTypeDeclV1>>()
        + h::<Option<&SemanticTypeShapeV1>>()
        + h::<Option<&SemanticTypeIdV1>>()
        + h::<&SemanticTypeIdV1>()
        + h::<Option<&SemanticProjectionV1>>()
        + h::<&SemanticProjectionV1>()
        + h::<std::slice::Iter<'_, SemanticProjectionV1>>()
}

fn inspect_object_loan_query_v29(
    instances: &ExecutionInstancesV29<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = references.unwrap().plan;
    let endpoint = emitted
        .iter()
        .flatten()
        .find_map(|lowered| {
            lowered
                .scoped_memory_anchors
                .as_ref()?
                .objects
                .iter()
                .find_map(|row| {
                    let ScopedObjectRoleV29::ReadValue { source, .. } = row.role else {
                        return None;
                    };
                    matches!(source.object, ScopedObjectIdentityV29::Reference { .. })
                        .then_some(source)
                })
        })
        .unwrap();
    let ScopedObjectIdentityV29::Reference {
        instance,
        site,
        role,
        dereference_prefix,
    } = endpoint.object
    else {
        unreachable!()
    };
    let place = scoped_object_original_place_v29(
        instances.instance(instance).unwrap().declaration(),
        site,
        role,
    )
    .unwrap();
    let original_site = SourceReferenceSiteV29 {
        instance,
        block: SemanticBlockIdV1::from_index(match site {
            ExecutionSiteV29::Statement { block, .. } | ExecutionSiteV29::Terminator { block } => {
                block.get()
            }
        }),
        statement: match site {
            ExecutionSiteV29::Statement { statement, .. } => Some(statement as usize),
            ExecutionSiteV29::Terminator { .. } => None,
        },
    };
    assert_eq!(dereference_prefix, 2);
    let pointer_type = place.projections()[0].result_type();
    let headers = object_loan_holder_headers_v29();
    // Initial owner, twelve prepaid locals/return envelopes and the charged
    // join's owner: fourteen five-work guards plus 8 + 4*path_length.
    let work = 14 * 5 + 8 + 4 * place.projections().len();
    match OBJECT_LOAN_QUERY_MODE_V29.get() {
        0 => {
            let before = (budget.work(), budget.storage());
            for _ in 0..16 {
                with_canonical_call_scratch_v1(budget, |budget| {
                    let floor = budget.storage();
                    assert_eq!(
                        source_object_loan_holder_type_v29(plan, original_site, place, budget)?,
                        Some(pointer_type)
                    );
                    assert_eq!(budget.storage() - floor, headers);
                    Ok(())
                })?;
                assert_eq!(budget.storage(), before.1);
            }
            assert_eq!(budget.work() - before.0, 16 * (work + 2));
            with_canonical_call_scratch_v1(budget, |budget| {
                let checked = source_object_loan_access_v29(
                    plan,
                    original_site,
                    place,
                    SourceReferenceAccessV29::Read,
                    budget,
                )?
                .unwrap();
                assert_eq!(plan.cells.rows[checked.cell], checked.original);
                assert_eq!(
                    checked.original.kind,
                    SourceBackingKindV29::Object(endpoint.root_schema)
                );
                let clone = place.clone();
                let refused = source_object_loan_access_v29(
                    plan,
                    original_site,
                    &clone,
                    SourceReferenceAccessV29::Read,
                    budget,
                );
                assert!(
                    matches!(
                        refused,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "source reference access occurrence is not retained",
                            ..
                        })
                    ),
                    "cloned locator: {refused:?}"
                );
                // The old source-plan query contract latches resource failures,
                // not semantic refusal. The authentic occurrence still works.
                let wrong_role = source_object_loan_access_v29(
                    plan, original_site, place, SourceReferenceAccessV29::Write, budget,
                );
                assert!(matches!(wrong_role, Err(ProductionSemanticKirErrorV1::Unsupported {
                    detail: "source reference access occurrence is not retained", ..
                })), "primitive role mismatch: {wrong_role:?}");
                assert!(
                    source_object_loan_access_v29(
                        plan,
                        original_site,
                        place,
                        SourceReferenceAccessV29::Read,
                        budget
                    )?
                    .is_some()
                );
                let legacy = plan.scalar_cell(checked.loan, budget);
                assert!(
                    matches!(
                        legacy,
                        Err(ProductionSemanticKirErrorV1::Unsupported {
                            detail: "typed object backing is not a scalar cell",
                            ..
                        })
                    ),
                    "legacy scalar gate: {legacy:?}"
                );
                Ok(())
            })?;
            OBJECT_LOAN_QUERY_COMPLETED_V29.set(true);
            Ok(())
        }
        mode => {
            let first = match mode {
                1 => {
                    let before = (budget.work(), budget.storage());
                    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
                    let mut foreign = ArgumentBudgetV1::new(&mut work, 37);
                    foreign.reserve_storage(37)?;
                    let refused = source_object_loan_holder_type_v29(
                        plan,
                        original_site,
                        place,
                        &mut foreign,
                    );
                    assert!(matches!(
                        refused,
                        Err(
                            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(
                                ArgumentResourceV1::Accounting
                            )
                        )
                    ));
                    assert_eq!((foreign.work(), foreign.storage()), (0, 37));
                    assert_eq!((budget.work(), budget.storage()), before);
                    refused
                }
                2 => {
                    budget.charge_work(LIMIT - budget.work() - (work - 1))?;
                    let refused =
                        source_object_loan_holder_type_v29(plan, original_site, place, budget);
                    assert!(
                        matches!(refused, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Work(error)))
                        if error.limit() == LIMIT && error.actual() > LIMIT)
                    );
                    refused
                }
                3 => {
                    let padding = LIMIT - budget.storage() - (headers - 1);
                    budget.reserve_storage(padding)?;
                    let refused =
                        source_object_loan_holder_type_v29(plan, original_site, place, budget);
                    assert!(
                        matches!(refused, Err(ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(ArgumentResourceV1::Storage(error)))
                        if error.limit() == LIMIT && error.actual() > LIMIT)
                    );
                    budget.release_storage(padding)?;
                    budget.charge_work(LIMIT - budget.work())?;
                    refused
                }
                _ => unreachable!(),
            };
            let before = (budget.work(), budget.storage());
            let replay = source_object_loan_holder_type_v29(plan, original_site, place, budget);
            assert_eq!(format!("{first:?}"), format!("{replay:?}"));
            assert_eq!((budget.work(), budget.storage()), before);
            OBJECT_LOAN_QUERY_COMPLETED_V29.set(true);
            first.map(|_| ())
        }
    }
}

#[test]
fn original_object_safe_loan_query_has_exact_headers_work_owner_and_first_failure() {
    struct Restore(Option<ScopedSlotCustodyObserverV29>, u8);
    impl Drop for Restore {
        fn drop(&mut self) {
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.0);
            OBJECT_LOAN_QUERY_MODE_V29.set(self.1);
        }
    }
    let _restore = Restore(
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(inspect_object_loan_query_v29)),
        OBJECT_LOAN_QUERY_MODE_V29.get(),
    );
    let config = ObjectLoanCaseV29 {
        mutable: true,
        wrapped: true,
        temporary: false,
        validity: false,
    };
    for mode in 0..=3 {
        OBJECT_LOAN_QUERY_MODE_V29.set(mode);
        OBJECT_LOAN_QUERY_COMPLETED_V29.set(false);
        let (result, _, _, completed) =
            run_object_loans_v29(config, SuffixCase::Root, 0, LIMIT, LIMIT);
        assert!(
            OBJECT_LOAN_QUERY_COMPLETED_V29.get(),
            "inner assertions: {result:?}"
        );
        if mode == 0 {
            result.unwrap();
            assert!(completed);
        } else {
            assert!(!completed);
            match (
                mode,
                original_repeated_source_resource_v29(result.unwrap_err()),
            ) {
                (1, ArgumentResourceV1::Accounting) => {}
                (2, ArgumentResourceV1::Work(error)) => assert_eq!(error.limit(), LIMIT),
                (3, ArgumentResourceV1::Storage(error)) => assert_eq!(error.limit(), LIMIT),
                other => panic!("safe-loan query failure: {other:?}"),
            }
        }
    }
}

fn inspect_object_loan_strategy_v29(
    instances: &ExecutionInstancesV29<'_>,
    _: &[Option<LoweredFunctionResultV1>],
    _: &mut OwnedScopedSourceSlotsV29,
    references: Option<&SourceReferenceEmissionV29<'_, '_>>,
    _: Option<&ExecutionIdentityPlanV1<'_, '_>>,
    _: usize,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    with_canonical_call_scratch_v1(budget, |budget| {
        let references = references.unwrap();
        references.check(budget)?;
        let plan = references.plan;
        let types = instances.owner().source_semantic().types();
        let mut checked = 0;
        for (loan, record) in plan.loans.iter().enumerate() {
            let origin = &plan.origins[record.origin];
            if record.kind != SemanticBorrowKindV1::Shared
                || !origin.projections.is_empty()
                || !matches!(
                    types[origin.ty.index() as usize].shape(),
                    SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
                )
            {
                continue;
            }
            if OBJECT_LOAN_FORCE_BACKING_V29.get() {
                let SourceReferenceCellStrategyV29::Object(cell) = plan.cells.strategies[loan]
                else {
                    panic!("an independently selected Object cannot retain a scalar-snapshot loan");
                };
                let row = plan.cells.rows[cell];
                assert_eq!(
                    (row.instance, row.local, row.generation, row.ty),
                    (origin.instance, origin.local, origin.generation, origin.ty)
                );
                let SourceBackingKindV29::Object(pointee) = row.kind else {
                    unreachable!()
                };
                let pointer = budget.source_reference_object_pointer_type_v29(plan, loan)?;
                assert!(matches!(pointer, Type::Pointer(pointer)
                    if pointer.pointee.as_ref() == &Type::StorageObject(pointee)
                        && pointer.address_space == AddressSpace::Private
                        && pointer.access == AccessMode::ReadOnly));
                for (node, value) in plan.nodes.iter().enumerate() {
                    if value.kind != SourceReferenceNodeKindV29::Loan(loan) {
                        continue;
                    }
                    if let Some(schema) = plan.selected_storage[node] {
                        let layouts = plan
                            .storage_root
                            .as_ref()
                            .unwrap()
                            .source_layouts(instances, budget)?;
                        let layouts = layouts.rows(instances.owner(), budget)?;
                        assert!(matches!(&layouts[schema.0 as usize].kind,
                            fe2o3_kernel_ir::StorageLayoutKindV1::Pointer(pointer)
                                if pointer.pointee == pointee && pointer.value_space == AddressSpace::Private
                                    && pointer.access == AccessMode::ReadOnly));
                    }
                }
            } else if OBJECT_LOAN_LEGACY_CELLS_V29.get()
                && instances
                    .instance(origin.instance)
                    .unwrap()
                    .function()
                    .index()
                    == 3
            {
                let SourceReferenceCellStrategyV29::Scalar(index) = plan.cells.strategies[loan]
                else {
                    panic!("the existing addressable-loan group must retain its Scalar strategy");
                };
                let (queried_index, cell) = plan.scalar_cell(loan, budget)?.unwrap();
                assert_eq!(queried_index, index);
                assert_eq!(plan.cells.rows[index], cell);
                assert_eq!(cell.kind, SourceBackingKindV29::Scalar);
                assert_eq!(
                    (cell.instance, cell.local, cell.generation, cell.ty),
                    (origin.instance, origin.local, origin.generation, origin.ty)
                );
                let (absent, scalar) = OBJECT_LOAN_UNBACKED_KINDS_V29.get();
                OBJECT_LOAN_UNBACKED_KINDS_V29.set((absent, scalar + 1));
            } else {
                assert_eq!(
                    plan.cells.strategies[loan],
                    SourceReferenceCellStrategyV29::Promoted
                );
                assert!(plan.scalar_cell(loan, budget)?.is_none());
                let mut kinds = OBJECT_LOAN_UNBACKED_KINDS_V29.get();
                match plan.cells.rows.iter().find(|cell| {
                    (cell.instance, cell.local, cell.generation)
                        == (origin.instance, origin.local, origin.generation)
                }) {
                    None => kinds.0 += 1,
                    Some(cell) => {
                        assert_eq!(cell.kind, SourceBackingKindV29::Scalar);
                        kinds.1 += 1;
                    }
                }
                OBJECT_LOAN_UNBACKED_KINDS_V29.set(kinds);
            }
            checked += 1;
        }
        assert!(checked >= 2, "original root and helper loan roster");
        OBJECT_LOAN_STRATEGY_VISITS_V29.set(OBJECT_LOAN_STRATEGY_VISITS_V29.get() + 1);
        Ok(())
    })
}

#[test]
fn original_shared_primitive_loan_strategy_matches_only_its_existing_object_backing() {
    struct Restore(
        ObjectLoanCaseV29,
        u8,
        bool,
        Option<ScopedSlotCustodyObserverV29>,
        bool,
    );
    impl Drop for Restore {
        fn drop(&mut self) {
            OBJECT_LOAN_CASE_V29.set(self.0);
            OBJECT_LOAN_FAULT_V29.set(self.1);
            OBJECT_LOAN_FORCE_BACKING_V29.set(self.2);
            SCOPED_SLOT_CUSTODY_OBSERVER_V29.set(self.3);
            OBJECT_LOAN_LEGACY_CELLS_V29.set(self.4);
        }
    }
    let config = ObjectLoanCaseV29 {
        mutable: false,
        wrapped: true,
        temporary: false,
        validity: false,
    };
    let _restore = Restore(
        OBJECT_LOAN_CASE_V29.replace(config),
        OBJECT_LOAN_FAULT_V29.replace(0),
        OBJECT_LOAN_FORCE_BACKING_V29.replace(true),
        SCOPED_SLOT_CUSTODY_OBSERVER_V29.replace(Some(inspect_object_loan_strategy_v29)),
        OBJECT_LOAN_LEGACY_CELLS_V29.replace(false),
    );
    OBJECT_LOAN_UNBACKED_KINDS_V29.set((0, 0));
    for case in [SuffixCase::Root, SuffixCase::Loop] {
        for backed in [true, false] {
            OBJECT_LOAN_FORCE_BACKING_V29.set(backed);
            OBJECT_LOAN_STRATEGY_VISITS_V29.set(0);
            let (result, _, _, completed) = run_original_source_fixture_v29(
                || object_loan_original_owner_v29(case),
                false,
                true,
                3,
                |_, _, _, _, _| Ok(()),
                LIMIT,
                LIMIT,
            );
            assert!(
                result.is_ok() && completed,
                "backed={backed}/{case:?}: {result:?}"
            );
            assert_eq!(OBJECT_LOAN_STRATEGY_VISITS_V29.get(), 3);
        }
    }
    OBJECT_LOAN_FORCE_BACKING_V29.set(false);
    OBJECT_LOAN_LEGACY_CELLS_V29.set(true);
    OBJECT_LOAN_STRATEGY_VISITS_V29.set(0);
    let (result, _, _, completed) = run_original_source_fixture_v29(
        || object_loan_original_owner_v29(SuffixCase::Cells),
        false,
        true,
        3,
        |_, _, _, _, _| Ok(()),
        LIMIT,
        LIMIT,
    );
    assert!(
        result.is_ok() && completed,
        "genuine Scalar demand: {result:?}"
    );
    assert_eq!(OBJECT_LOAN_STRATEGY_VISITS_V29.get(), 3);
    let before = OBJECT_LOAN_UNBACKED_KINDS_V29.get();
    OBJECT_LOAN_LEGACY_CELLS_V29.set(false);
    OBJECT_LOAN_CASE_V29.set(ObjectLoanCaseV29 { temporary: true, ..config });
    OBJECT_LOAN_STRATEGY_VISITS_V29.set(0);
    let (result, _, _, completed) = run_original_source_fixture_v29(
        || object_loan_original_owner_v29(SuffixCase::Root),
        false,
        true,
        3,
        |_, _, _, _, _| Ok(()),
        LIMIT,
        LIMIT,
    );
    assert!(result.is_ok() && completed, "genuine no-cell temporary: {result:?}");
    assert_eq!(OBJECT_LOAN_STRATEGY_VISITS_V29.get(), 3);
    let (absent, scalar) = OBJECT_LOAN_UNBACKED_KINDS_V29.get();
    assert!(absent > before.0, "new original temporary must remain unbacked");
    assert!(
        absent > 0 && scalar > 0,
        "no-cell and legacy-scalar dispositions: absent={absent}, scalar={scalar}"
    );
}

include!("production_source_scalar_loan_access_v29_tests.rs");
include!("production_source_object_loan_candidate_v29_tests.rs");

include!("production_source_primitive_reborrow_v29_tests.rs");
