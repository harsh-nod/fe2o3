use super::*;
use fe2o3_mir_model::SsaBlockIdV1;
use fe2o3_pliron::{
    ProductionSemanticSsaOccurrenceSiteV1 as Site, ProductionSemanticSsaOperandRoleV1 as Role,
};

const ARRAY_SCALAR: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const ARRAY_TYPE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const ARRAY_ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);

#[derive(Clone, Copy)]
enum ArrayCase {
    Write {
        sparse: bool,
    },
    ValueRead {
        local_index: bool,
    },
    RetainedValueRead,
    Initializer {
        values: [u32; 8],
        repetitions: usize,
        float: bool,
    },
}

fn array_place(local_index: bool) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                if local_index {
                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2))
                } else {
                    SemanticProjectionKindV1::ConstantIndex {
                        offset: 0,
                        minimum_length: 8,
                        from_end: false,
                    }
                },
                ARRAY_SCALAR,
            )
            .unwrap(),
        ],
        ARRAY_SCALAR,
    )
    .unwrap()
}

// The Unit/U32 declarations reuse the qualified source fixture. The array is
// freshly admitted with the exact fixed layout, never installed into an owner.
fn array_owner(case: ArrayCase) -> ProductionPreRankedKirOwnerV1 {
    array_owner_at_body(case, false).unwrap()
}

fn array_owner_at_body(
    case: ArrayCase,
    helper: bool,
) -> Result<ProductionPreRankedKirOwnerV1, ProductionPreRankedKirErrorV1> {
    let old_types = types();
    let mut source_types = vec![
        old_types[0].clone(),
        old_types[2].clone(),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([201; 32]),
            SemanticLayoutIdentityV1::from_sha256([201; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                32,
                4,
                SemanticFieldsShapeV1::array(4, 8),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                4,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Array {
                element: ARRAY_SCALAR,
                length: 8,
            },
        ),
    ];
    let float = matches!(case, ArrayCase::Initializer { float: true, .. });
    let index_type = if float {
        source_types[1] = SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([2; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::float(32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits: 32 }),
        );
        source_types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([203; 32]),
            SemanticLayoutIdentityV1::from_sha256([203; 32]),
            old_types[2].layout().clone(),
            old_types[2].shape().clone(),
        ));
        SemanticTypeIdV1::from_index(3)
    } else {
        ARRAY_SCALAR
    };
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut statements = vec![assignment(
        2,
        index_type,
        SemanticRvalueKindV1::Use(constant(index_type, 0, 4)),
    )];
    match case {
        ArrayCase::Write { .. } => statements.push(SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                array_place(true),
                SemanticRvalueV1::new(
                    ARRAY_SCALAR,
                    SemanticRvalueKindV1::Use(constant(ARRAY_SCALAR, 99, 4)),
                ),
            )),
        )),
        ArrayCase::ValueRead { local_index } => {
            statements.push(assignment(
                1,
                ARRAY_TYPE,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Array,
                        vec![constant(ARRAY_SCALAR, 11, 4); 8],
                    )
                    .unwrap(),
                ),
            ));
            statements.push(assignment(
                3,
                ARRAY_SCALAR,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(array_place(local_index))),
            ));
        }
        ArrayCase::RetainedValueRead => {
            statements.push(assignment(
                1,
                ARRAY_TYPE,
                SemanticRvalueKindV1::Aggregate(
                    SemanticAggregateRvalueV1::new(
                        SemanticAggregateKindV1::Array,
                        vec![constant(ARRAY_SCALAR, 11, 4); 8],
                    )
                    .unwrap(),
                ),
            ));
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    array_place(true),
                    SemanticRvalueV1::new(
                        ARRAY_SCALAR,
                        SemanticRvalueKindV1::Use(constant(ARRAY_SCALAR, 99, 4)),
                    ),
                )),
            ));
            statements.push(assignment(
                3,
                ARRAY_SCALAR,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(array_place(true))),
            ));
        }
        ArrayCase::Initializer {
            values,
            repetitions,
            ..
        } => {
            for _ in 0..repetitions {
                statements.push(assignment(
                    1,
                    ARRAY_TYPE,
                    SemanticRvalueKindV1::Aggregate(
                        SemanticAggregateRvalueV1::new(
                            SemanticAggregateKindV1::Array,
                            values
                                .into_iter()
                                .map(|value| constant(ARRAY_SCALAR, value.into(), 4))
                                .collect(),
                        )
                        .unwrap(),
                    ),
                ));
            }
            statements.push(SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    array_place(true),
                    SemanticRvalueV1::new(
                        ARRAY_SCALAR,
                        SemanticRvalueKindV1::Use(constant(ARRAY_SCALAR, 99, 4)),
                    ),
                )),
            ));
        }
    }
    let blocks = if matches!(case, ArrayCase::Write { sparse: true }) {
        vec![
            block(
                211,
                vec![],
                SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 2)),
            ),
            block(212, vec![], SemanticTerminatorKindV1::Return),
            block(213, statements, SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![block(211, statements, SemanticTerminatorKindV1::Return)]
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([202; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![])
    .unwrap();
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([202; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([202; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([202; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([202; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([202; 32]),
        source,
        abi,
        [UNIT, ARRAY_TYPE, index_type, ARRAY_SCALAR]
            .into_iter()
            .enumerate()
            .map(|(i, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([220 + i as u8; 32]),
                    ty,
                    if i == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"private_array_relation".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([202; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(Some(dimensions), Some(dimensions), None)
                    .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let functions = if helper {
        let helper_abi = SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256([230; 32]),
            SemanticLayoutIdentityV1::from_sha256([250; 32]),
            SemanticCanonAbiV1::Rust,
            SemanticExternAbiV1::Rust,
            false,
            false,
            0,
            vec![],
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
        )
        .unwrap()
        .with_source_argument_ownership(vec![])
        .unwrap();
        let helper_function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([230; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([230; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([230; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([230; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([230; 32]),
            source,
            helper_abi,
            function
                .locals()
                .iter()
                .enumerate()
                .map(|(index, old)| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([234 + index as u8; 32]),
                        old.ty(),
                        old.role(),
                        old.source(),
                    )
                })
                .collect(),
            function.entry(),
            function.blocks().to_vec(),
        )
        .unwrap();
        let root = SemanticFunctionDeclV1::new(
            function.identity(),
            function.role(),
            function.item_definition_identity(),
            function.monomorphization_identity(),
            function.generic_type_arguments_identity(),
            function.const_generic_arguments_identity(),
            source,
            function.abi().clone(),
            vec![function.locals()[0].clone()],
            SemanticBlockIdV1::from_index(0),
            vec![
                block(
                    240,
                    vec![],
                    SemanticTerminatorKindV1::Call(
                        SemanticDirectCallV1::new(
                            SemanticFunctionIdV1::from_index(1),
                            vec![],
                            Some(SemanticCallDestinationV1::new(
                                place(0, UNIT),
                                edge(SemanticEdgeRoleV1::CallReturn, 1),
                            )),
                            SemanticUnwindActionV1::Unreachable,
                        )
                        .unwrap(),
                    ),
                ),
                block(241, vec![], SemanticTerminatorKindV1::Return),
            ],
        )
        .unwrap()
        .with_kernel_entry(function.kernel_entry().unwrap().clone());
        vec![root, helper_function]
    } else {
        vec![function]
    };
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        source_types,
        vec![],
        vec![],
        vec![],
        functions,
        vec![ARRAY_ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let semantic =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "array_relation",
            [202; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    );
    assert_eq!(budget.storage(), FLOOR);
    owner
}

fn query(
    owner: &ProductionPreRankedKirOwnerV1,
    block: u32,
    statement: u32,
    role: Role,
) -> Result<bool, SemanticKirPrivateArrayQueryErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    let floor = FLOOR + retained(owner);
    budget.reserve_storage(floor).unwrap();
    let result = owner.has_materialized_private_array_access(
        ARRAY_ROOT,
        ARRAY_ROOT,
        Site::Statement {
            block: SsaBlockIdV1::new(block),
            statement,
        },
        role,
        &mut budget,
    );
    assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
    result
}

#[test]
fn private_array_admitted_write_records_actual_operations_and_survives_owner_move() {
    let owner = array_owner(ArrayCase::Write { sparse: false });
    let rows = &owner.correspondence.private_arrays;
    assert!(rows.active);
    assert_eq!(
        (
            rows.recorded_instance_operations,
            rows.instances.len(),
            rows.slots.len(),
            rows.effects.len()
        ),
        (7, 1, 1, 1)
    );
    let slot = rows.slots[0];
    let effect = rows.effects[0];
    assert_eq!(
        (
            slot.local,
            slot.length,
            slot.count_location.operation,
            slot.alloca_location.operation
        ),
        (1, 8, 0, 1)
    );
    assert_eq!(
        (
            effect.semantic_block,
            effect.semantic_statement,
            effect.role
        ),
        (0, 1, Role::Destination)
    );
    assert_eq!(
        (
            effect.source_first_operation,
            effect.gep_location.operation,
            effect.memory_location.operation,
            effect.source_end_operation
        ),
        (3, 5, 6, 7)
    );
    assert_eq!(query(&owner, 0, 1, Role::Destination), Ok(true));
    let graph = owner.executable().module().functions.as_ptr();
    let rows_address = rows.effects.as_ptr();
    let moved = Box::new(owner);
    assert_eq!(moved.executable().module().functions.as_ptr(), graph);
    assert_eq!(
        moved.correspondence.private_arrays.effects.as_ptr(),
        rows_address
    );
    moved.semantic_ssa().verify_replay().unwrap();
    assert_eq!(query(&moved, 0, 1, Role::Destination), Ok(true));
    assert!(!moved.grants_artifact_or_launch_authority());
}

#[test]
fn private_array_required_missing_instance_and_slot_are_not_false() {
    for remove_instance in [true, false] {
        let mut owner = array_owner(ArrayCase::Write { sparse: false });
        if remove_instance {
            owner.correspondence.private_arrays.instances.clear();
        } else {
            owner.correspondence.private_arrays.slots.clear();
            owner.correspondence.private_arrays.instances[0].slot_end = 0;
        }
        assert_eq!(
            query(&owner, 0, 1, Role::Destination),
            Err(SemanticKirPrivateArrayQueryErrorV1::Mismatch(
                "required retained array instance or slot is absent",
            ))
        );
    }
}

#[test]
fn private_array_promoted_fixed_read_is_false_but_local_index_absence_is_incomplete() {
    let owner = array_owner(ArrayCase::ValueRead { local_index: false });
    assert!(!owner.correspondence.private_arrays.active);
    assert!(
        owner
            .semantic_ssa()
            .plan_for_function(ARRAY_ROOT)
            .unwrap()
            .plan()
            .promoted_variables()
            .iter()
            .any(|v| v.get() == 1)
    );
    assert_eq!(query(&owner, 0, 2, Role::RvalueOperand(0)), Ok(false));
    let owner = array_owner(ArrayCase::ValueRead { local_index: true });
    assert!(!owner.correspondence.private_arrays.active);
    assert_eq!(
        query(&owner, 0, 2, Role::RvalueOperand(0)),
        Err(SemanticKirPrivateArrayQueryErrorV1::Incomplete(
            "unretained array index lacks a bounded constant-index witness",
        ))
    );
}

#[test]
fn private_array_sparse_source_block_uses_distinct_physical_block_ordinal() {
    let mut owner = array_owner(ArrayCase::Write { sparse: true });
    let effect = owner.correspondence.private_arrays.effects[0];
    assert_eq!(
        (
            effect.semantic_block,
            effect.memory_location.block,
            effect.memory_location.block_ordinal
        ),
        (2, BlockId(2), 1)
    );
    assert_eq!(query(&owner, 2, 1, Role::Destination), Ok(true));
    owner.correspondence.private_arrays.effects[0]
        .memory_location
        .block_ordinal = 2;
    assert!(matches!(
        query(&owner, 2, 1, Role::Destination),
        Err(SemanticKirPrivateArrayQueryErrorV1::Mismatch(_))
    ));
}

#[test]
fn private_array_source_role_and_original_index_identity_are_not_interchangeable() {
    for hostile in 0..5 {
        let mut owner = array_owner(ArrayCase::Write { sparse: false });
        let effect = &mut owner.correspondence.private_arrays.effects[0];
        match hostile {
            0 => effect.role = Role::RvalueOperand(0),
            1 => effect.semantic_statement = 0,
            2 => effect.local = 3,
            3 => effect.gep_location.operation = effect.memory_location.operation,
            4 => {
                let PrivateArrayIndexV1::Local { original, .. } = &mut effect.original_index else {
                    panic!("actual local index");
                };
                *original = effect.offset;
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            query(&owner, 0, 1, Role::Destination),
            Err(SemanticKirPrivateArrayQueryErrorV1::Mismatch(_)
                | SemanticKirPrivateArrayQueryErrorV1::Incomplete(_))
        ));
    }
}

#[test]
fn private_array_query_rejects_foreign_root_body_and_underreserved_floor() {
    let owner = array_owner(ArrayCase::Write { sparse: false });
    let site = Site::Statement {
        block: SsaBlockIdV1::new(0),
        statement: 1,
    };
    for (root, body, detail) in [
        (1, 0, "selected root is absent from this owner"),
        (
            0,
            1,
            "requested body differs from the constructor-selected entry",
        ),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(retained(&owner)).unwrap();
        assert_eq!(
            owner.has_materialized_private_array_access(
                SemanticFunctionIdV1::from_index(root),
                SemanticFunctionIdV1::from_index(body),
                site,
                Role::Destination,
                &mut budget,
            ),
            Err(SemanticKirPrivateArrayQueryErrorV1::InvalidSource(detail))
        );
    }
    let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(retained(&owner) - 1).unwrap();
    assert!(matches!(
        owner.has_materialized_private_array_access(
            ARRAY_ROOT,
            ARRAY_ROOT,
            site,
            Role::Destination,
            &mut budget,
        ),
        Err(SemanticKirPrivateArrayQueryErrorV1::Resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
        ))
    ));
    assert_eq!(budget.work(), 3);
}

#[test]
fn private_array_actual_query_has_independently_derived_exact_and_one_under_work() {
    let owner = array_owner(ArrayCase::Write { sparse: false });
    // Source-derived: exact export has 22 bytes; promoted [0,2,3], no cross-edge
    // variables; selected row lookup prefix143 (one extra component-key
    // comparison) plus the unchanged exact U32 Store relation136.
    assert_eq!(
        owner.executable().module().functions[0].id.as_str(),
        "private_array_relation"
    );
    for (limit, accepted, succeeds) in [(279, 279, true), (278, 276, false)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = FLOOR + retained(&owner);
        budget.reserve_storage(floor).unwrap();
        let result = owner.has_materialized_private_array_access(
            ARRAY_ROOT,
            ARRAY_ROOT,
            Site::Statement {
                block: SsaBlockIdV1::new(0),
                statement: 1,
            },
            Role::Destination,
            &mut budget,
        );
        if succeeds {
            assert_eq!(result, Ok(true));
        } else {
            assert!(
                matches!(result, Err(SemanticKirPrivateArrayQueryErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error)))
                if error.actual() == 279 && error.limit() == 278)
            );
        }
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (accepted, floor, floor)
        );
    }
    for (remove_instance, expected) in [(true, 116), (false, 127)] {
        let mut owner = array_owner(ArrayCase::Write { sparse: false });
        if remove_instance {
            owner.correspondence.private_arrays.instances.clear();
        } else {
            owner.correspondence.private_arrays.slots.clear();
            owner.correspondence.private_arrays.instances[0].slot_end = 0;
        }
        let mut work = CanonicalKernelIrWorkBudgetV1::new(expected);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(retained(&owner)).unwrap();
        assert_eq!(
            owner.has_materialized_private_array_access(
                ARRAY_ROOT,
                ARRAY_ROOT,
                Site::Statement {
                    block: SsaBlockIdV1::new(0),
                    statement: 1
                },
                Role::Destination,
                &mut budget
            ),
            Err(SemanticKirPrivateArrayQueryErrorV1::Mismatch(
                "required retained array instance or slot is absent"
            ))
        );
        assert_eq!(budget.work(), expected);
    }
}

#[test]
fn private_array_constant_index_query_preserves_exact_work_and_floor() {
    let owner = array_owner(ArrayCase::Write { sparse: false });
    // The bool query is 143 selection + 136 relation units after the explicit
    // component-key comparison. The facade prepays one more unit: 1 + 279 = 280.
    for (limit, accepted, attempted) in [(280, 280, None), (279, 277, Some(280)), (0, 0, Some(1))] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = FLOOR + retained(&owner);
        budget.reserve_storage(floor).unwrap();
        let result = owner.materialized_private_array_constant_index(
            ARRAY_ROOT,
            ARRAY_ROOT,
            Site::Statement {
                block: SsaBlockIdV1::new(0),
                statement: 1,
            },
            Role::Destination,
            &mut budget,
        );
        if let Some(attempted) = attempted {
            assert!(
                matches!(result, Err(SemanticKirPrivateArrayQueryErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error)))
                if error.actual() == attempted && error.limit() == limit)
            );
        } else {
            assert_eq!(result, Ok(Some(0)));
        }
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (accepted, floor, floor)
        );
    }
}

#[test]
fn private_array_constant_index_query_preserves_supported_absence_and_errors() {
    for local_index in [false, true] {
        let owner = array_owner(ArrayCase::ValueRead { local_index });
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = FLOOR + retained(&owner);
        budget.reserve_storage(floor).unwrap();
        let result = owner.materialized_private_array_constant_index(
            ARRAY_ROOT,
            ARRAY_ROOT,
            Site::Statement {
                block: SsaBlockIdV1::new(0),
                statement: 2,
            },
            Role::RvalueOperand(0),
            &mut budget,
        );
        if local_index {
            assert_eq!(
                result,
                Err(SemanticKirPrivateArrayQueryErrorV1::Incomplete(
                    "unretained array index lacks a bounded constant-index witness",
                ))
            );
        } else {
            assert_eq!(result, Ok(None));
        }
        assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
    }
}

fn initializer_query(
    owner: &ProductionPreRankedKirOwnerV1,
    statement: u32,
) -> Result<Option<u64>, SemanticKirPrivateArrayQueryErrorV1> {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
    let floor = FLOOR + retained(owner);
    budget.reserve_storage(floor).unwrap();
    let result = owner.materialized_private_array_initializer_count(
        ARRAY_ROOT,
        ARRAY_ROOT,
        Site::Statement {
            block: SsaBlockIdV1::new(0),
            statement,
        },
        &mut budget,
    );
    assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
    result
}

#[test]
fn private_array_initializer_records_dense_values_and_repeated_aggregate_recipes() {
    for (values, float) in [
        ([0, 1, 2, 3, 7, 31, 255, u32::MAX], false),
        ([11; 8], false),
        (
            [
                0,
                0x8000_0000,
                0x3f80_0000,
                0x7f80_0000,
                0xff80_0000,
                0x7fc0_0001,
                0x7fc0_0002,
                1,
            ],
            true,
        ),
    ] {
        let owner = array_owner(ArrayCase::Initializer {
            values,
            repetitions: 2,
            float,
        });
        let rows = &owner.correspondence.private_arrays;
        assert_eq!((rows.slots.len(), rows.effects.len()), (1, 17));
        for statement in [1, 2] {
            assert_eq!(initializer_query(&owner, statement), Ok(Some(8)));
            let effects: Vec<_> = rows
                .effects
                .iter()
                .filter(|effect| effect.semantic_statement == statement)
                .collect();
            assert_eq!(effects.len(), 8);
            for (component, effect) in effects.into_iter().enumerate() {
                let PrivateArrayIndexV1::InitializerElement {
                    component: actual,
                    value: PrivateArrayInitializerValueV1::LiteralScalar { value, definition },
                } = effect.original_index
                else {
                    panic!("explicit initializer component");
                };
                assert_eq!(actual as usize, component);
                let body = owner.executable().module().functions[0]
                    .body
                    .as_ref()
                    .unwrap();
                let operation =
                    &body.blocks[definition.block_ordinal].operations[definition.operation];
                assert_eq!(operation.results[0].id, value);
                assert_eq!(
                    operation.kind,
                    OperationKind::Constant(if float {
                        Constant::F32Bits(values[component])
                    } else {
                        Constant::U32(values[component])
                    })
                );
                assert!(matches!(body.blocks[effect.memory_location.block_ordinal]
                    .operations[effect.memory_location.operation].kind,
                    OperationKind::Store { value: actual, .. } if actual == value));
            }
        }
        assert_eq!(query(&owner, 0, 3, Role::Destination), Ok(true));
        assert!(!owner.grants_artifact_or_launch_authority());
    }
}

#[test]
fn private_array_initializer_promoted_recipe_is_none_not_a_retained_effect() {
    let owner = array_owner(ArrayCase::ValueRead { local_index: false });
    assert!(!owner.correspondence.private_arrays.active);
    assert_eq!(initializer_query(&owner, 1), Ok(None));
}

#[test]
fn private_array_initializer_census_rejects_missing_duplicate_reordered_and_foreign_rows() {
    for hostile in 0..7 {
        let mut owner = array_owner(ArrayCase::Initializer {
            values: [0, 1, 2, 3, 4, 5, 6, 7],
            repetitions: 1,
            float: false,
        });
        let rows = &mut owner.correspondence.private_arrays;
        let expected = match hostile {
            0 => {
                rows.effects.remove(3);
                rows.instances[0].effect_end -= 1;
                "private initializer component census is incomplete"
            }
            1 => {
                rows.effects[3] = rows.effects[2];
                "private initializer components are not exact and ordered"
            }
            2 => {
                rows.effects.swap(2, 3);
                "private initializer components are not exact and ordered"
            }
            3 => {
                rows.effects[2].local = 3;
                "private initializer components are not exact and ordered"
            }
            4 => {
                rows.slots[0].length = 7;
                "private array layout facts changed"
            }
            5 => {
                rows.effects[2].owner = SemanticFunctionIdV1::from_index(1);
                "private array owner, function, or local changed"
            }
            6 => {
                rows.effects[2].gep_location = rows.effects[2].memory_location;
                "private array effect is outside its exact source operation span"
            }
            _ => unreachable!(),
        };
        assert_eq!(
            initializer_query(&owner, 1),
            Err(SemanticKirPrivateArrayQueryErrorV1::Mismatch(expected))
        );
    }
}

#[test]
fn private_array_initializer_value_relation_checks_actual_bits_definition_and_store() {
    let owner = array_owner(ArrayCase::Initializer {
        values: [0, 1, 2, 3, 4, 5, 6, 7],
        repetitions: 1,
        float: false,
    });
    let slot = owner.correspondence.private_arrays.slots[0];
    let effect = owner.correspondence.private_arrays.effects[2];
    let source =
        owner.semantic_ssa().source_semantic().functions()[0].blocks()[0].statements()[1].kind();
    for hostile in 0..5 {
        let mut body = owner.executable().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .clone();
        let PrivateArrayIndexV1::InitializerElement {
            component,
            mut value,
        } = effect.original_index
        else {
            panic!("initializer");
        };
        let PrivateArrayInitializerValueV1::LiteralScalar {
            value: actual,
            definition,
        } = value;
        let expected = match hostile {
            0 => {
                body.blocks[definition.block_ordinal].operations[definition.operation].kind =
                    OperationKind::Constant(Constant::U32(99));
                "private initializer value differs from its exact source literal"
            }
            1 => {
                body.blocks[definition.block_ordinal].operations[definition.operation].results[0]
                    .ty = Type::Scalar(ScalarType::F32);
                "private initializer value differs from its exact source literal"
            }
            2 => {
                value = PrivateArrayInitializerValueV1::LiteralScalar {
                    value: effect.offset,
                    definition,
                };
                "private initializer value differs from its exact source literal"
            }
            3 => {
                value = PrivateArrayInitializerValueV1::LiteralScalar {
                    value: actual,
                    definition: effect.memory_location,
                };
                "private initializer definition is outside its exact source recipe"
            }
            4 => {
                let OperationKind::Store { value, .. } = &mut body.blocks
                    [effect.memory_location.block_ordinal]
                    .operations[effect.memory_location.operation]
                    .kind
                else {
                    panic!("actual Store");
                };
                *value = effect.offset;
                "private initializer Store uses a different value"
            }
            _ => unreachable!(),
        };
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(retained(&owner)).unwrap();
        let result = private_array_initializer_value_v1(
            source,
            &body,
            &slot,
            &effect,
            component,
            value,
            &mut PrivateArrayQueryWorkV1 {
                budget: &mut budget,
            },
        )
        .map_err(private_array_query_error_v1);
        assert_eq!(
            result,
            Err(SemanticKirPrivateArrayQueryErrorV1::Mismatch(expected))
        );
    }
}

#[test]
fn private_array_initializer_value_component_exact_work_and_reservation_denial() {
    let owner = array_owner(ArrayCase::Initializer {
        values: [11; 8],
        repetitions: 1,
        float: false,
    });
    let slot = owner.correspondence.private_arrays.slots[0];
    let effect = owner.correspondence.private_arrays.effects[0];
    let PrivateArrayIndexV1::InitializerElement { component, value } = effect.original_index else {
        panic!("initializer");
    };
    let source =
        owner.semantic_ssa().source_semantic().functions()[0].blocks()[0].statements()[1].kind();
    let body = owner.executable().module().functions[0]
        .body
        .as_ref()
        .unwrap();
    // Fixed recipe12 + scalar6 + decode/coordinate12 + definition3+5 + Store3+2.
    for (limit, accepted, success) in [(43, 43, true), (42, 41, false)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        let floor = FLOOR + retained(&owner);
        budget.reserve_storage(floor).unwrap();
        let result = private_array_initializer_value_v1(
            source,
            body,
            &slot,
            &effect,
            component,
            value,
            &mut PrivateArrayQueryWorkV1 {
                budget: &mut budget,
            },
        )
        .map_err(private_array_query_error_v1);
        if success {
            assert_eq!(result, Ok(0));
        } else {
            assert!(
                matches!(result, Err(SemanticKirPrivateArrayQueryErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error)))
                if error.actual() == 43 && error.limit() == 42)
            );
        }
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (accepted, floor, floor)
        );
    }
    // Recorder-buffer component only: reserve17, push frontier2, insertion1.
    for (limit, accepted, success) in [(20, 20, true), (19, 19, false)] {
        let mut work = PrivateArrayRecorderBudgetV1::new(1, limit).unwrap();
        let mut buffer = PrivateArrayBufferV1::new(1);
        buffer.reserve(1, 1, &mut work).unwrap();
        assert_eq!(work.work.work(), 17);
        assert!(buffer.rows.capacity() >= 1);
        let result = buffer.push(effect, &mut work);
        if success {
            result.unwrap();
        } else {
            assert!(matches!(
                result,
                Err(ProductionSemanticKirErrorV1::ResourceLimit {
                    resource: ProductionSemanticKirResourceV1::AnalysisWork,
                    actual: 20,
                    limit: 19,
                })
            ));
            assert!(buffer.rows.is_empty());
        }
        assert_eq!(work.work.work(), accepted);
    }
}

#[test]
fn private_array_initializer_query_floor_source_and_initial_work_prefix_are_exact() {
    let owner = array_owner(ArrayCase::Initializer {
        values: [1; 8],
        repetitions: 1,
        float: false,
    });
    let site = Site::Statement {
        block: SsaBlockIdV1::new(0),
        statement: 1,
    };
    for (limit, floor) in [(2, retained(&owner)), (3, retained(&owner) - 1)] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let result = owner.materialized_private_array_initializer_count(
            ARRAY_ROOT,
            ARRAY_ROOT,
            site,
            &mut budget,
        );
        if limit == 2 {
            assert!(
                matches!(result, Err(SemanticKirPrivateArrayQueryErrorV1::Resource(
                fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(error)))
                if error.actual() == 3 && error.limit() == 2)
            );
            assert_eq!(budget.work(), 0);
        } else {
            assert_eq!(
                result,
                Err(SemanticKirPrivateArrayQueryErrorV1::Resource(
                    fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting
                ))
            );
            assert_eq!(budget.work(), 3);
        }
        assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
    }
    for (root, body, detail) in [
        (1, 0, "selected root is absent from this owner"),
        (0, 1, "initializer body is absent from this owner"),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(retained(&owner)).unwrap();
        assert_eq!(
            owner.materialized_private_array_initializer_count(
                SemanticFunctionIdV1::from_index(root),
                SemanticFunctionIdV1::from_index(body),
                site,
                &mut budget
            ),
            Err(SemanticKirPrivateArrayQueryErrorV1::InvalidSource(detail))
        );
    }
}

#[test]
fn private_array_initializer_retained_helper_uses_checked_source_relation() {
    local_helper_source_v1_tests::initializer_helper_source_relation();
}

#[test]
fn private_array_initializer_root_query_rejects_foreign_instance_coordinates() {
    for hostile in 0..3 {
        let mut owner = array_owner(ArrayCase::Initializer {
            values: [11; 8],
            repetitions: 1,
            float: false,
        });
        assert_eq!(initializer_query(&owner, 1), Ok(Some(8)));
        let instance = &mut owner.correspondence.private_arrays.instances[0];
        let expected = match hostile {
            0 => {
                instance.module_function_ordinal = 1;
                "private initializer instance coordinates changed"
            }
            1 => {
                instance.lowered_function_ordinal = usize::MAX;
                "private initializer correspondence function is absent"
            }
            _ => {
                instance.function = SemanticFunctionIdV1::from_index(1);
                "retained initializer instance is absent"
            }
        };
        assert_eq!(
            initializer_query(&owner, 1),
            Err(SemanticKirPrivateArrayQueryErrorV1::Mismatch(expected))
        );
    }
}

#[path = "production_private_array_caller_v1_tests.rs"]
mod private_array_caller_v1_tests;

#[path = "production_private_array_facts_v1_tests.rs"]
mod private_array_facts_v1_tests;

#[path = "production_private_array_output_v1_tests.rs"]
mod private_array_output_v1_tests;

#[path = "production_local_helper_source_v1_tests.rs"]
mod local_helper_source_v1_tests;
