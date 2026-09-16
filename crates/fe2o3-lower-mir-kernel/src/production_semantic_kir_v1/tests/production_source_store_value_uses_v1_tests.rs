// Included inside the existing admitted_source_transport_v1 test module.

fn source_store_test_scope_v1() -> SourceStoreScopeV1 {
    SourceStoreScopeV1 {
        owner: SemanticFunctionIdV1::from_index(0),
        function: SemanticFunctionIdV1::from_index(0),
        block: SemanticBlockIdV1::from_index(2),
        kernel_ir_block: BlockId(2),
        statement: 0,
        operand: SemanticKirSourceStoreOperandV1::AssignmentRvalue,
        source_type: U32,
    }
}

fn source_store_test_emission_v1(work: usize, storage: usize) -> SourceStoreEmissionV1 {
    let mut emission = SourceStoreEmissionV1::new(work);
    emission.work.row_limit = storage;
    emission
}

fn source_store_test_block_v1() -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(2));
    block.operations.push(Operation::new(
        vec![],
        OperationKind::Store {
            pointer: ValueId(99),
            value: ValueId(7),
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    block
}

fn source_store_test_capture_v1(
    emission: &mut SourceStoreEmissionV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mut capture = SourceStoreFunctionCaptureV1::new(Some(emission));
    capture.active = Some(source_store_test_scope_v1());
    capture.record(0, ValueId(7), None)?;
    capture.active = None;
    capture.finish(
        &[ValueId(7)],
        &[Type::Scalar(ScalarType::U32)],
        &[source_store_test_block_v1()],
    )
}

#[test]
fn source_store_capture_has_independent_exact_work_and_capacity_boundaries() {
    // Numeric component only, not source admission: record 4+3+1 = 8.
    // Finish: entry1 + formal(3+1) + block1 + op1 + sort1 + row1
    // + binary-search3 + final reserve(3+1) = 16. Three coexisting rows.
    for limit in [24, 23] {
        let mut emission = source_store_test_emission_v1(limit, 3);
        let result = source_store_test_capture_v1(&mut emission);
        if limit == 24 {
            result.unwrap();
            assert_eq!(emission.work.work.work(), 24);
            assert_eq!(emission.rows.len(), 1);
            assert_eq!(emission.rows.capacity(), 1);
            assert_eq!(emission.work.peak_rows, 3);
            assert_eq!(
                emission.rows[0].definition(),
                SemanticKirSourceStoreDefinitionV1::FunctionArgument(0)
            );
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: 24,
                    limit: 23,
                })
            ));
            assert_eq!(emission.work.work.work(), 23);
            assert_eq!(emission.work.work.failed_work(), Some(24));
            assert!(emission.rows.is_empty());
            assert_eq!(emission.rows.capacity(), 0);
            assert!(source_store_test_capture_v1(&mut emission).is_err());
            assert_eq!(emission.work.work.failed_work(), Some(24));
        }
    }
    let mut emission = source_store_test_emission_v1(1000, 2);
    assert!(matches!(
        source_store_test_capture_v1(&mut emission),
        Err(ProductionSemanticKirErrorV1::ResourceLimit {
            resource: ProductionSemanticKirResourceV1::AnalysisStorage,
            actual: 3,
            limit: 2,
        })
    ));
    assert_eq!(emission.work.work.work(), 23);
    assert!(emission.rows.is_empty());
    assert_eq!(emission.rows.capacity(), 0);
    // Scratch has dropped. A storage-only refusal does not reset accepted work.
    emission.work.row_limit = 3;
    source_store_test_capture_v1(&mut emission).unwrap();
    assert_eq!(emission.work.work.work(), 47);
}

#[test]
fn source_store_first_reservation_refuses_before_allocating() {
    for (work_limit, row_limit) in [(7, 1), (100, 0)] {
        let mut emission = source_store_test_emission_v1(work_limit, row_limit);
        let mut capture = SourceStoreFunctionCaptureV1::new(Some(&mut emission));
        capture.active = Some(source_store_test_scope_v1());
        let result = capture.record(0, ValueId(7), None);
        assert!(result.is_err());
        assert_eq!(capture.pending.capacity(), 0);
        assert!(capture.pending.is_empty());
        let emission = capture.emission.as_ref().unwrap();
        assert_eq!(emission.work.work.work(), 7);
        if work_limit == 7 {
            assert_eq!(emission.work.work.failed_work(), Some(8));
        } else {
            assert_eq!(emission.work.work.failed_work(), None);
        }
    }
}

#[test]
fn source_store_capture_uses_one_counter_across_functions_and_keeps_history() {
    // Prior7 + first24 + second25 = 56. Second final growth relocates one row.
    for limit in [56, 55] {
        let mut emission = source_store_test_emission_v1(limit, 4);
        emission.work.charge(7).unwrap();
        source_store_test_capture_v1(&mut emission).unwrap();
        let result = source_store_test_capture_v1(&mut emission);
        if limit == 56 {
            result.unwrap();
            assert_eq!(emission.rows.len(), 2);
            assert_eq!(emission.work.work.work(), 56);
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: 56,
                    limit: 55,
                })
            ));
            assert_eq!(emission.rows.len(), 1);
            assert_eq!(emission.work.work.work(), 54);
            assert_eq!(emission.work.work.failed_work(), Some(56));
        }
    }
}

#[test]
fn source_store_partial_seal_failure_drops_scratch_and_rolls_back_rows() {
    let mut emission = source_store_test_emission_v1(1000, 100);
    source_store_test_capture_v1(&mut emission).unwrap();
    let retained = emission.rows.clone();
    {
        let mut capture = SourceStoreFunctionCaptureV1::new(Some(&mut emission));
        capture.active = Some(source_store_test_scope_v1());
        capture.record(0, ValueId(7), None).unwrap();
        capture.component = 1;
        capture.record(1, ValueId(8), None).unwrap();
        capture.active = None;
        assert!(matches!(
            capture.finish(
                &[ValueId(7)],
                &[Type::Scalar(ScalarType::U32)],
                &[source_store_test_block_v1()],
            ),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    }
    assert_eq!(emission.rows, retained);
    let work = emission.work.work.work();
    let capacity = emission.rows.capacity();
    source_store_test_capture_v1(&mut emission).unwrap();
    assert!(emission.work.work.work() > work);
    assert_eq!(emission.rows.len(), 2);
    assert_eq!(emission.rows.capacity(), capacity);
}

#[test]
fn source_store_capture_does_not_record_unscoped_staging_or_guarded_operations() {
    let mut emission = source_store_test_emission_v1(4, 1);
    {
        let mut capture = SourceStoreFunctionCaptureV1::new(Some(&mut emission));
        capture.record(0, ValueId(7), None).unwrap();
        capture.record(1, ValueId(7), Some(ValueId(8))).unwrap();
        capture.finish(&[], &[], &[]).unwrap();
        assert!(capture.pending.is_empty());
        capture.active = Some(source_store_test_scope_v1());
        assert!(matches!(
            capture.record(2, ValueId(7), Some(ValueId(8))),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
        assert!(capture.pending.is_empty());
    }
    assert!(emission.rows.is_empty());
    assert_eq!(emission.work.work.work(), 4);
    assert_eq!(emission.work.peak_rows, 0);
}

#[test]
fn actual_payload_and_prepared_call_emission_do_not_impersonate_source_rhs_stores() {
    // Lowering-component regression, not a fabricated admitted enum program.
    let source_owner = noop_semantic_owner(&["source_store_internal_component"]);
    let source = source_owner.semantic();
    let mut emission = source_store_test_emission_v1(0, 0);
    {
        let mut lowering = SemanticFunctionLoweringV1::new(
            source.types(),
            source.callables(),
            &source.functions()[0],
            SemanticParameterBindingsV1 {
                declarations: &[],
                values: &[],
                types: &[],
                local_bindings: None,
            },
            None,
            None,
            BTreeSet::new(),
            1,
            false,
            1000,
        )
        .unwrap();
        lowering.source_stores = SourceStoreFunctionCaptureV1::new(Some(&mut emission));
        lowering.enum_payload_storage.insert(
            (0, 0, 0),
            SemanticEnumPayloadFieldStorageV1 {
                semantic_type: UNIT,
                exact_enum_variant: None,
                compiler_issued_binding: None,
                components: vec![SemanticEnumPayloadComponentStorageV1 {
                    pointer: ValueId(99),
                    kernel_type: Type::Scalar(ScalarType::U32),
                    alignment: 4,
                }]
                .into_boxed_slice(),
            },
        );
        let mut operations = Vec::new();
        let scalar = || SemanticValueBindingV1::Value {
            id: ValueId(7),
            ty: Type::Scalar(ScalarType::U32),
        };
        // Even a surrounding source assignment scope cannot label the internal
        // enum payload emitter's Store as an ordinary source RHS Store.
        lowering.source_stores.active = Some(source_store_test_scope_v1());
        lowering
            .store_enum_payload_v1(
                SemanticBlockIdV1::from_index(0),
                Some(0),
                SemanticLocalIdV1::from_index(0),
                &SemanticValueBindingV1::Enum {
                    discriminant: ValueId(6),
                    discriminant_ty: Type::Scalar(ScalarType::U32),
                    semantic_type: UNIT,
                    variant: Some(0),
                    payloads: BTreeMap::from([(0, vec![scalar()])]),
                },
                &mut operations,
            )
            .unwrap();
        assert_eq!(operations.len(), 1);
        assert!(matches!(operations[0].kind, OperationKind::Store { .. }));
        assert!(lowering.source_stores.pending.is_empty());
        lowering.source_stores.active = None;
        lowering
            .finish_call_destination_v1(
                SemanticBlockIdV1::from_index(0),
                &place(0, UNIT),
                PreparedSemanticCallDestinationV1::Memory {
                    pointer: ValueId(99),
                    value_type: Type::Scalar(ScalarType::U32),
                    access: MemoryAccess::new(AddressSpace::Private, 4),
                },
                scalar(),
                None,
                &mut operations,
            )
            .unwrap();
        assert_eq!(operations.len(), 2);
        assert!(matches!(operations[1].kind, OperationKind::Store { .. }));
        assert!(lowering.source_stores.pending.is_empty());
    }
    assert!(emission.rows.is_empty());
    assert_eq!(emission.work.work.work(), 0);
}

fn source_store_same_typed_tuple_owner_v1() -> ProductionSemanticKirOwnerV1 {
    const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
    let mut catalog = types();
    let SemanticBackendReprV1::Scalar(component) =
        catalog[U32.index() as usize].layout().backend_repr()
    else {
        panic!()
    };
    catalog.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([51; 32]),
        SemanticLayoutIdentityV1::from_sha256([52; 32]),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            8,
            4,
            SemanticFieldsShapeV1::arbitrary(vec![4, 0], vec![1, 0]).unwrap(),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::scalar_pair(*component, *component),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::Aggregate(
                SemanticAggregateLayoutV1::new(vec![4, 0], vec![]).unwrap(),
            ),
        )
        .unwrap(),
        SemanticTypeShapeV1::Tuple(SemanticAggregateTypeV1::new(vec![U32, U32]).unwrap()),
    ));
    let index = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(2),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::ConstantIndex {
                    offset: 0,
                    minimum_length: 2,
                    from_end: false,
                },
                U32,
            )
            .unwrap(),
        ],
        U32,
    )
    .unwrap();
    let chain = forwarding_chain(
        PAIR,
        SemanticAbiPassModeV1::Pair {
            first: initialized_scalar_attributes(),
            second: initialized_scalar_attributes(),
        },
        vec![
            local(207, UNIT, SemanticLocalRoleV1::Return),
            local(208, PAIR, SemanticLocalRoleV1::Temporary),
            local(209, ARRAY, SemanticLocalRoleV1::Temporary),
            local(210, TUPLE, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            aggregate(
                place(2, ARRAY),
                SemanticAggregateKindV1::Array,
                vec![
                    SemanticOperandV1::Copy(field(1, 0, U32)),
                    SemanticOperandV1::Copy(field(1, 1, U32)),
                ],
            ),
            assign(index, SemanticRvalueKindV1::Use(scalar(U32, 7))),
        ],
        vec![local(217, PAIR, SemanticLocalRoleV1::Return)],
        vec![block(
            226,
            vec![aggregate(
                place(0, PAIR),
                SemanticAggregateKindV1::Tuple,
                vec![scalar(U32, 11), scalar(U32, 29)],
            )],
            SemanticTerminatorKindV1::Return,
        )],
    );
    lower(catalog, chain)
}

#[test]
fn admitted_source_tuple_fields_capture_distinct_same_typed_store_components() {
    let owner = source_store_same_typed_tuple_owner_v1();
    let rows = &owner.correspondence.source_store_value_uses;
    assert_eq!(rows.len(), 3);
    assert_eq!(
        rows[0].source_statement(),
        (SemanticBlockIdV1::from_index(1), 0)
    );
    assert_eq!(rows[1].source_statement(), rows[0].source_statement());
    assert_eq!((rows[0].component(), rows[1].component()), (0, 1));
    assert_eq!(rows[0].source_type(), ARRAY);
    assert_eq!(rows[0].scalar(), ScalarType::U32);
    assert_eq!(rows[1].scalar(), ScalarType::U32);
    assert_ne!(rows[0].value(), rows[1].value());
    assert_ne!(rows[0].definition(), rows[1].definition());
    let function = &owner.module().functions[0];
    for row in rows {
        let (block, operation) = row.store();
        let body = function.body.as_ref().unwrap();
        let block = body
            .blocks
            .iter()
            .find(|candidate| candidate.id == block)
            .unwrap();
        assert!(matches!(&block.operations[operation as usize].kind,
            OperationKind::Store { value, .. } if *value == row.value()));
    }
    owner.verify_equivalence().unwrap();
}

#[test]
fn replay_rejects_row_mutation_without_changing_source_identity() {
    for mutation in 0..10 {
        let mut owner = source_store_same_typed_tuple_owner_v1();
        let identity = owner.correspondence.semantic_sha256;
        let (call_block, call_operation, call) = owner.module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| {
                block
                    .operations
                    .iter()
                    .enumerate()
                    .map(move |(ordinal, operation)| (block.id, ordinal, operation))
            })
            .find(|(_, _, operation)| matches!(operation.kind, OperationKind::Call { .. }))
            .unwrap();
        assert_eq!(call.results.len(), 2);
        assert_eq!(call.results[0].ty, call.results[1].ty);
        let wrong_result = (
            call.results[1].id,
            SemanticKirSourceStoreDefinitionV1::OperationResult {
                block: call_block,
                operation: call_operation as u32,
                result: 1,
            },
        );
        let rows = &mut owner.correspondence.source_store_value_uses;
        match mutation {
            0 => rows[0].component = 1,
            1 => {
                rows[0].value = rows[1].value;
                rows[0].definition = rows[1].definition;
            }
            2 => rows[0].semantic_function = SemanticFunctionIdV1::from_index(1),
            3 => rows[0].operation = rows[1].operation,
            4 => {
                rows.remove(0);
            }
            5 => rows.push(rows[0]),
            6 => rows.swap(0, 1),
            7 => rows[0].operand = SemanticKirSourceStoreOperandV1::StoreValue,
            8 => rows[0].statement = 1,
            9 => {
                rows[0].value = wrong_result.0;
                rows[0].definition = wrong_result.1;
            }
            _ => unreachable!(),
        }
        assert_eq!(owner.correspondence.semantic_sha256, identity);
        assert!(matches!(
            owner.verify_equivalence(),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    }
}

const SOURCE_STORE_ROOTS_V1: [&str; 2] = ["store_z", "store_a"];

fn with_source_store_two_root_owner_v1(body: impl FnOnce(&mut ProductionPreRankedKirOwnerV1)) {
    const PAIR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
    let single = source_store_same_typed_tuple_owner_v1();
    let semantic = single.semantic_ssa.source_semantic();
    let old_root = &semantic.functions()[0];
    let templates = noop_semantic_owner(&SOURCE_STORE_ROOTS_V1);
    let mut functions = templates
        .semantic()
        .functions()
        .iter()
        .map(|template| {
            rebuild(
                template,
                old_root.locals().to_vec(),
                vec![
                    block(208, vec![], call_to(2, place(1, PAIR))),
                    block(
                        209,
                        old_root.blocks()[1].statements().to_vec(),
                        SemanticTerminatorKindV1::Return,
                    ),
                ],
            )
        })
        .collect::<Vec<_>>();
    let forwarding = &semantic.functions()[1];
    functions.push(rebuild(
        forwarding,
        forwarding.locals().to_vec(),
        vec![
            block(218, vec![], call_to(3, place(0, PAIR))),
            block(219, vec![], SemanticTerminatorKindV1::Return),
        ],
    ));
    functions.push(semantic.functions()[2].clone());
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        semantic.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        vec![
            SemanticFunctionIdV1::from_index(0),
            SemanticFunctionIdV1::from_index(1),
        ],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let source =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        source,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let inputs = ssa
        .source_semantic()
        .roots()
        .iter()
        .zip(SOURCE_STORE_ROOTS_V1)
        .map(|(root, name)| {
            let entry = ssa.source_semantic().functions()[root.index() as usize]
                .kernel_entry()
                .unwrap();
            let workgroup = entry
                .source_contract()
                .launch()
                .unwrap()
                .required()
                .unwrap()
                .as_array();
            crate::ProductionSourceLaunchRootInputV1::new(
                name,
                *entry.kernel_binding_identity().as_bytes(),
                crate::ProductionSourceLaunchInputV1::new(1, Some(workgroup), [1, 1, 1]),
            )
        })
        .collect::<Vec<_>>();
    let launch =
        crate::ProductionSourceLaunchRosterV1::try_new(ssa.source_semantic(), &inputs).unwrap();
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1::new(
        &mut work,
        1_000_000_000,
    );
    let mut owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), 0);
    let retained = owner.executable_storage().retained_storage()
        + owner.assert_origin_storage().payload_storage();
    budget.reserve_storage(retained).unwrap();
    owner.replay_source_store_value_uses_v1().unwrap();
    verify_module(owner.executable().module()).unwrap();
    body(&mut owner);
    drop(owner);
    budget.release_storage(retained).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn admitted_two_root_store_capture_keeps_reused_local_coordinates_root_qualified() {
    with_source_store_two_root_owner_v1(|owner| {
        let semantic = owner.semantic_ssa.source_semantic();
        assert_eq!(
            semantic.functions()[0].locals(),
            semantic.functions()[1].locals()
        );
        assert_eq!(semantic.functions().len(), 4);
        assert_eq!(
            owner
                .executable()
                .module()
                .functions
                .iter()
                .filter(|function| function.body.is_some())
                .count(),
            4,
        );
        assert_eq!(
            owner
                .executable()
                .module()
                .kernels
                .iter()
                .map(|kernel| kernel.id.as_str())
                .collect::<Vec<_>>(),
            SOURCE_STORE_ROOTS_V1,
        );
        let aliases = owner.correspondence.lowered_functions();
        assert_eq!(aliases.len(), 6);
        for helper in [2, 3] {
            let shared = aliases
                .iter()
                .filter(|row| row.semantic_function() == SemanticFunctionIdV1::from_index(helper))
                .collect::<Vec<_>>();
            assert_eq!(shared.len(), 2);
            assert_ne!(
                shared[0].correspondence_owner(),
                shared[1].correspondence_owner()
            );
            assert_eq!(
                shared[0].kernel_ir_function(),
                shared[1].kernel_ir_function()
            );
        }
        let rows = &owner.correspondence.source_store_value_uses;
        assert_eq!(rows.len(), 6);
        for (root, root_rows) in rows.chunks_exact(3).enumerate() {
            let root = SemanticFunctionIdV1::from_index(root as u32);
            let alias = aliases
                .iter()
                .find(|alias| {
                    alias.correspondence_owner() == root && alias.semantic_function() == root
                })
                .unwrap();
            let function = owner
                .executable()
                .module()
                .functions
                .iter()
                .find(|function| &function.id == alias.kernel_ir_function())
                .unwrap();
            let body = function.body.as_ref().unwrap();
            let shared = aliases
                .iter()
                .find(|alias| {
                    alias.correspondence_owner() == root
                        && alias.semantic_function() == SemanticFunctionIdV1::from_index(2)
                })
                .unwrap();
            assert_eq!(
                calls(function)
                    .into_iter()
                    .filter(
                        |call| matches!(&call.kind, OperationKind::Call { callee, .. }
                if callee == shared.kernel_ir_function())
                    )
                    .count(),
                1
            );
            for row in root_rows {
                assert_eq!(row.correspondence_owner(), root);
                assert_eq!(row.semantic_function(), root);
                let (block, operation) = row.store();
                let block = body
                    .blocks
                    .iter()
                    .find(|candidate| candidate.id == block)
                    .unwrap();
                assert!(matches!(&block.operations[operation as usize].kind,
                OperationKind::Store { value, .. } if *value == row.value()));
            }
        }
        for (left, right) in rows[..3].iter().zip(&rows[3..]) {
            assert_eq!(left.source_statement(), right.source_statement());
            assert_eq!(left.component(), right.component());
            assert_eq!(left.store(), right.store());
            assert_eq!(left.value(), right.value());
            assert_eq!(left.definition(), right.definition());
            assert_ne!(left.correspondence_owner(), right.correspondence_owner());
        }
        owner.replay_source_store_value_uses_v1().unwrap();
    });
}

#[test]
fn admitted_two_root_store_replay_rejects_only_the_wrong_owner_with_same_source() {
    with_source_store_two_root_owner_v1(|owner| {
        let source_identity = owner.correspondence.semantic_sha256;
        let original = owner.correspondence.source_store_value_uses[0];
        owner.correspondence.source_store_value_uses[0].correspondence_owner =
            SemanticFunctionIdV1::from_index(1);
        let changed = owner.correspondence.source_store_value_uses[0];
        assert_eq!(changed.semantic_function(), original.semantic_function());
        assert_eq!(changed.source_statement(), original.source_statement());
        assert_eq!(changed.store(), original.store());
        assert_eq!(changed.value(), original.value());
        assert_eq!(changed.definition(), original.definition());
        assert_eq!(owner.correspondence.semantic_sha256, source_identity);
        assert!(matches!(
            owner.replay_source_store_value_uses_v1(),
            Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
        ));
    });
}

#[test]
fn actual_two_root_lowering_continues_one_capture_meter_and_retained_row_prefix() {
    // White-box continuation of real source lowering, not an independently
    // exact public-constructor budget. Public materialization/replay is tested above.
    with_source_store_two_root_owner_v1(|owner| {
        let limits = ProductionSemanticKirLimitsV1::default();
        let mut closure = ReachableClosureBlockBudgetV1::new(limits.max_blocks);
        let mut result_work =
            fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limits.max_operations);
        let mut capture = SourceStoreEmissionV1::new(limits.max_operations);
        capture.work.charge(7).unwrap();
        let mut private = PrivateArrayLazyBudgetV1::new(2, limits.max_operations);
        let launches = &owner.launch_roots;
        assert_eq!(launches.len(), 2);
        let mut prefix = Vec::new();
        let mut previous_work = 7;
        for launch in launches.iter().copied() {
            let (module, _, _) = lower_single_root_module(
                &owner.semantic_ssa,
                limits,
                launch.selected_root,
                Some(launch),
                &mut closure,
                &mut result_work,
                &mut capture,
                false,
                None,
                &mut private,
                None,
            )
            .unwrap();
            verify_module(&module).unwrap();
            assert_eq!(&capture.rows[..prefix.len()], prefix.as_slice());
            assert_eq!(capture.rows.len(), prefix.len() + 3);
            assert!(
                capture.rows[prefix.len()..]
                    .iter()
                    .all(|row| row.correspondence_owner() == launch.selected_root)
            );
            assert!(capture.work.work.work() > previous_work);
            assert_eq!(capture.work.work.failed_work(), None);
            previous_work = capture.work.work.work();
            prefix.clone_from(&capture.rows);
        }
        assert_eq!(capture.rows, owner.correspondence.source_store_value_uses);
        assert!(capture.work.work.work() > 7);
    });
}
