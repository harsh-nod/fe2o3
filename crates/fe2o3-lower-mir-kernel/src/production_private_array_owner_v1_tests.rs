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
    Write { sparse: bool },
    ValueRead { local_index: bool },
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
    let old_types = types();
    let source_types = vec![
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
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut statements = vec![assignment(
        2,
        ARRAY_SCALAR,
        SemanticRvalueKindV1::Use(constant(ARRAY_SCALAR, 0, 4)),
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
        [UNIT, ARRAY_TYPE, ARRAY_SCALAR, ARRAY_SCALAR]
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
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        source_types,
        vec![],
        vec![],
        vec![],
        vec![function],
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
    )
    .unwrap();
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
    // variables; selected row lookup prefix142 plus exact U32 Store relation136.
    assert_eq!(
        owner.executable().module().functions[0].id.as_str(),
        "private_array_relation"
    );
    for (limit, accepted, succeeds) in [(278, 278, true), (277, 275, false)] {
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
                if error.actual() == 278 && error.limit() == 277)
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
    // The unchanged bool query is 142 selection + 136 relation units. The
    // constant-index facade prepays one extraction unit: 1 + 278 = 279.
    for (limit, accepted, attempted) in [(279, 279, None), (278, 276, Some(279)), (0, 0, Some(1))] {
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

#[path = "production_private_array_caller_v1_tests.rs"]
mod private_array_caller_v1_tests;

#[path = "production_private_array_facts_v1_tests.rs"]
mod private_array_facts_v1_tests;

#[path = "production_private_array_output_v1_tests.rs"]
mod private_array_output_v1_tests;
