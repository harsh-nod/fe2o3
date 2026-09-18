use super::*;

const LOCAL_BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const LOCAL_U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);

#[derive(Clone, Copy, Debug)]
pub(super) enum UnitCase {
    Initializer,
    ArrayLength,
    ReadThenWrite,
    ScalarSlot,
    Reinitialize(ScalarKill),
    Killed(ScalarKill),
    CastAssert { expected: bool },
    RetainedBoolAssert,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ScalarKill {
    Move,
    Storage,
    Deinitialize,
}

fn local_abi(tag: u8, root: bool) -> SemanticFunctionAbiV1 {
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        if root {
            SemanticCanonAbiV1::GpuKernel
        } else {
            SemanticCanonAbiV1::Rust
        },
        if root {
            SemanticExternAbiV1::GpuKernel
        } else {
            SemanticExternAbiV1::Rust
        },
        false,
        false,
        0,
        vec![],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![])
    .unwrap()
}

fn indexed(local: u32) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(local)),
                ARRAY_SCALAR,
            )
            .unwrap(),
        ],
        ARRAY_SCALAR,
    )
    .unwrap()
}

fn write_index(local: u32, value: SemanticOperandV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            indexed(local),
            SemanticRvalueV1::new(ARRAY_SCALAR, SemanticRvalueKindV1::Use(value)),
        )),
    )
}

fn store_scalar(value: SemanticOperandV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            place(2, ARRAY_SCALAR),
            value,
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    )
}

fn compare_index(expected: bool) -> SemanticStatementV1 {
    assignment(
        5,
        LOCAL_BOOL,
        SemanticRvalueKindV1::Binary {
            operation: if expected {
                SemanticBinaryOpV1::LessThan
            } else {
                SemanticBinaryOpV1::GreaterOrEqual
            },
            left: value(4, LOCAL_U64),
            right: constant(LOCAL_U64, 8, 8),
        },
    )
}

fn bounds_assert(expected: bool, target: u32) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Assert {
        condition: value(5, LOCAL_BOOL),
        expected,
        message: SemanticAssertMessageV1::BoundsCheck {
            length: constant(LOCAL_U64, 8, 8),
            index: value(4, LOCAL_U64),
        },
        target: edge(SemanticEdgeRoleV1::AssertSuccess, target),
        unwind: SemanticUnwindActionV1::Unreachable,
    }
}

// The seed supplies admitted scalar/array layouts and source syntax only. All
// functions, calls, semantic admission and SSA below are fresh, before the
// resource-measured constructor. No candidate owner or proof row is mutated.
fn unit_source(
    case: UnitCase,
    calls_per_root: &[usize],
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    assert!((1..=2).contains(&calls_per_root.len()));
    assert!(calls_per_root.iter().all(|count| *count <= 2));
    assert!(calls_per_root.iter().any(|count| *count != 0));
    let seed = array_owner(ArrayCase::RetainedValueRead);
    let semantic = seed.semantic_ssa().source_semantic();
    let original = &semantic.functions()[0];
    let mut source_types = semantic.types().to_vec();
    let scalar_types = types();
    for (tag, declaration) in [(203, &scalar_types[1]), (204, &scalar_types[3])] {
        source_types.push(SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256([tag; 32]),
            declaration.layout().clone(),
            declaration.shape().clone(),
        ));
    }
    let statements = original.blocks()[0].statements();
    let helper_blocks = match case {
        UnitCase::Initializer => vec![block(
            220,
            statements[..3].to_vec(),
            SemanticTerminatorKindV1::Return,
        )],
        UnitCase::ArrayLength => {
            let mut body = statements[..3].to_vec();
            body.push(assignment(
                4,
                LOCAL_U64,
                SemanticRvalueKindV1::Length(place(1, ARRAY_TYPE)),
            ));
            vec![block(220, body, SemanticTerminatorKindV1::Return)]
        }
        UnitCase::ReadThenWrite => {
            let mut body = statements.to_vec();
            body.push(write_index(2, value(3, ARRAY_SCALAR)));
            vec![block(220, body, SemanticTerminatorKindV1::Return)]
        }
        UnitCase::ScalarSlot => {
            let mut body = statements.to_vec();
            body[0] = store_scalar(constant(ARRAY_SCALAR, 0, 4));
            body.insert(
                1,
                assignment(
                    3,
                    ARRAY_SCALAR,
                    SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                        place(2, ARRAY_SCALAR),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ),
            );
            body.push(write_index(2, value(3, ARRAY_SCALAR)));
            vec![block(220, body, SemanticTerminatorKindV1::Return)]
        }
        UnitCase::Reinitialize(kill) | UnitCase::Killed(kill) => {
            let mut body = vec![store_scalar(constant(ARRAY_SCALAR, 0, 4))];
            match kill {
                ScalarKill::Move => body.push(assignment(
                    3,
                    ARRAY_SCALAR,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Move(place(2, ARRAY_SCALAR))),
                )),
                ScalarKill::Storage => {
                    body.push(SemanticStatementV1::new(
                        SemanticSourceProvenanceV1::unavailable(),
                        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(2)),
                    ));
                    body.push(SemanticStatementV1::new(
                        SemanticSourceProvenanceV1::unavailable(),
                        SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(2)),
                    ));
                }
                ScalarKill::Deinitialize => body.push(SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Deinitialize(place(2, ARRAY_SCALAR)),
                )),
            }
            if matches!(case, UnitCase::Killed(_)) {
                body.push(assignment(
                    3,
                    ARRAY_SCALAR,
                    SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
                        place(2, ARRAY_SCALAR),
                        SemanticVolatilityV1::NonVolatile,
                        None,
                    )),
                ));
                body.push(statements[1].clone());
                body.push(SemanticStatementV1::new(
                    SemanticSourceProvenanceV1::unavailable(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        array_place(false),
                        SemanticRvalueV1::new(
                            ARRAY_SCALAR,
                            SemanticRvalueKindV1::Use(value(3, ARRAY_SCALAR)),
                        ),
                    )),
                ));
            } else {
                body.push(store_scalar(constant(ARRAY_SCALAR, 0, 4)));
                body.extend_from_slice(&statements[1..]);
                body.push(write_index(2, value(3, ARRAY_SCALAR)));
            }
            vec![block(220, body, SemanticTerminatorKindV1::Return)]
        }
        UnitCase::CastAssert { expected } => vec![
            block(
                220,
                vec![
                    store_scalar(constant(ARRAY_SCALAR, 0, 4)),
                    statements[1].clone(),
                    assignment(
                        4,
                        LOCAL_U64,
                        SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::Integer,
                            operand: value(2, ARRAY_SCALAR),
                        },
                    ),
                    compare_index(expected),
                ],
                bounds_assert(expected, 1),
            ),
            block(
                221,
                vec![
                    write_index(4, constant(ARRAY_SCALAR, 99, 4)),
                    compare_index(expected),
                ],
                bounds_assert(expected, 2),
            ),
            block(
                222,
                vec![
                    assignment(
                        3,
                        ARRAY_SCALAR,
                        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(indexed(4))),
                    ),
                    assignment(
                        4,
                        LOCAL_U64,
                        SemanticRvalueKindV1::Cast {
                            kind: SemanticCastKindV1::Integer,
                            operand: constant(ARRAY_SCALAR, 1, 4),
                        },
                    ),
                    compare_index(expected),
                ],
                bounds_assert(expected, 3),
            ),
            block(
                223,
                vec![write_index(4, value(3, ARRAY_SCALAR))],
                SemanticTerminatorKindV1::Return,
            ),
        ],
        UnitCase::RetainedBoolAssert => {
            let mut before_assert = statements[..3].to_vec();
            before_assert.push(SemanticStatementV1::new(
                SemanticSourceProvenanceV1::unavailable(),
                SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                    place(5, LOCAL_BOOL),
                    constant(LOCAL_BOOL, 1, 1),
                    SemanticVolatilityV1::NonVolatile,
                    None,
                )),
            ));
            vec![
                block(
                    220,
                    before_assert,
                    SemanticTerminatorKindV1::Assert {
                        condition: value(5, LOCAL_BOOL),
                        expected: true,
                        message: SemanticAssertMessageV1::BoundsCheck {
                            length: constant(LOCAL_U64, 8, 8),
                            index: constant(LOCAL_U64, 0, 8),
                        },
                        target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                        unwind: SemanticUnwindActionV1::Unreachable,
                    },
                ),
                block(221, vec![], SemanticTerminatorKindV1::Return),
            ]
        }
    };
    let source = SemanticSourceProvenanceV1::unavailable();
    let helper_id = SemanticFunctionIdV1::from_index(calls_per_root.len() as u32);
    let mut functions = Vec::new();
    let mut launches = Vec::new();
    for (root, count) in calls_per_root.iter().copied().enumerate() {
        let tag = 180 + root as u8;
        let mut blocks = Vec::new();
        for call in 0..count {
            blocks.push(block(
                210 + call as u8,
                vec![],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        helper_id,
                        vec![],
                        Some(SemanticCallDestinationV1::new(
                            place(0, UNIT),
                            edge(SemanticEdgeRoleV1::CallReturn, call as u32 + 1),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ));
        }
        blocks.push(block(215, vec![], SemanticTerminatorKindV1::Return));
        let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
        let function = SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticFunctionRoleV1::KernelRoot,
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            source,
            local_abi(tag, true),
            vec![SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([tag + 10; 32]),
                UNIT,
                SemanticLocalRoleV1::Return,
                source,
            )],
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(format!("unit_local_{root}").into_bytes()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([tag; 32]),
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
        functions.push(function);
        launches.push(crate::ProductionSourceLaunchRootInputV1::new(
            if root == 0 { "unit_zero" } else { "unit_one" },
            [tag; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        ));
    }
    functions.push(
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([200; 32]),
            SemanticFunctionRoleV1::InternalHelper,
            SemanticItemDefinitionIdentityV1::from_sha256([200; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([200; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([200; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([200; 32]),
            source,
            local_abi(200, false),
            [
                UNIT,
                ARRAY_TYPE,
                ARRAY_SCALAR,
                ARRAY_SCALAR,
                LOCAL_U64,
                LOCAL_BOOL,
            ]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([230 + index as u8; 32]),
                    ty,
                    if index == 0 {
                        SemanticLocalRoleV1::Return
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    source,
                )
            })
            .collect(),
            SemanticBlockIdV1::from_index(0),
            helper_blocks,
        )
        .unwrap(),
    );
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        source_types,
        vec![],
        vec![],
        vec![],
        functions,
        (0..calls_per_root.len())
            .map(|root| SemanticFunctionIdV1::from_index(root as u32))
            .collect(),
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
    let launch =
        crate::ProductionSourceLaunchRosterV1::try_new(ssa.source_semantic(), &launches).unwrap();
    (ssa, launch)
}

pub(super) fn unit_owner(
    case: UnitCase,
    calls_per_root: &[usize],
) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = unit_source(case, calls_per_root);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    assert_eq!(
        owner.helper_source_policy_v1(),
        ProductionHelperSourcePolicyV1::UnitLocal
    );
    owner
}

fn with_pending_candidate(
    case: UnitCase,
    mutate: impl FnOnce(&mut Module),
    next: impl FnOnce(CanonicalCallSubjectV1<'_>, &SealedAssertOriginsV1, &mut ArgumentBudgetV1<'_>),
) {
    let (mut ssa, launch) = unit_source(case, &[1]);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let launch_roots = materialization_launch_roots_v1(&ssa, &launch).unwrap();
    let mut emission = AssertOriginEmissionV1::new(&mut budget);
    let PendingHelperSourceLoweringV1 {
        mut module,
        correspondence,
        requires_source,
        requires_borrowed,
    } = lower_pending_module_with_assert_origins_v1(
        &ssa,
        ProductionSemanticKirLimitsV1::default(),
        &launch_roots,
        &mut emission,
    )
    .unwrap();
    assert!(requires_source);
    assert!(!requires_borrowed);
    mutate(&mut module);
    // Hostile operands must pass real module verification before source checking.
    let (executable, graph_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            emission.budget,
        )
        .unwrap();
    emission
        .budget
        .reserve_storage(graph_storage.retained_storage())
        .unwrap();
    drop(module);
    let origins = emission.seal(&ssa, &correspondence, &executable).unwrap();
    let origin_storage = origins.storage.payload_storage();
    let floor = budget.storage();
    next(
        CanonicalCallSubjectV1 {
            semantic_ssa: &ssa,
            executable: &executable,
            correspondence: &correspondence,
        },
        &origins,
        &mut budget,
    );
    assert_eq!(budget.storage(), floor);
    drop(origins);
    budget.release_storage(origin_storage).unwrap();
    drop(correspondence);
    drop(executable);
    budget
        .release_storage(graph_storage.retained_storage())
        .unwrap();
    drop(ssa);
    budget.release_storage(capture.retained_storage()).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

pub(super) fn initializer_helper_source_relation() {
    for promoted in [false, true] {
        let mut owner = array_owner_at_body(
            if promoted {
                ArrayCase::ValueRead { local_index: false }
            } else {
                ArrayCase::Initializer {
                    values: [11; 8],
                    repetitions: 1,
                    float: false,
                }
            },
            true,
        )
        .unwrap();
        assert_eq!(
            owner.helper_source_policy_v1(),
            if promoted {
                ProductionHelperSourcePolicyV1::RawEmpty
            } else {
                ProductionHelperSourcePolicyV1::UnitLocal
            }
        );
        let body = SemanticFunctionIdV1::from_index(1);
        let site = Site::Statement {
            block: SsaBlockIdV1::new(0),
            statement: 1,
        };
        let run = |owner: &ProductionPreRankedKirOwnerV1, requested_body| {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = ArgumentBudgetV1::new(&mut work, STORAGE);
            let floor = FLOOR + retained(owner);
            budget.reserve_storage(floor).unwrap();
            let result = owner.materialized_private_array_initializer_count(
                ARRAY_ROOT,
                requested_body,
                site,
                &mut budget,
            );
            assert_eq!((budget.storage(), budget.peak_storage()), (floor, floor));
            result
        };
        assert_eq!(run(&owner, body), Ok(if promoted { None } else { Some(8) }));
        assert_eq!(
            run(&owner, SemanticFunctionIdV1::from_index(2)),
            Err(SemanticKirPrivateArrayQueryErrorV1::InvalidSource(
                "initializer body is absent from this owner"
            ))
        );
        if !promoted {
            assert_eq!(owner.correspondence.private_arrays.instances.len(), 1);
            owner.correspondence.private_arrays.instances[0].module_function_ordinal = 0;
            assert_eq!(
                run(&owner, body),
                Err(SemanticKirPrivateArrayQueryErrorV1::Mismatch(
                    "private initializer instance coordinates changed"
                ))
            );
        }
    }
}

#[test]
fn normal_source_constructor_keeps_two_roots_and_both_calls_per_root() {
    let owner = unit_owner(UnitCase::ReadThenWrite, &[2, 2]);
    let physical_helpers = owner
        .correspondence
        .lowered_functions
        .iter()
        .filter(|row| row.role == SemanticKirFunctionRoleV1::InternalHelper)
        .map(|row| &row.kernel_ir_function)
        .collect::<Vec<_>>();
    assert_eq!(physical_helpers.len(), 2);
    assert_eq!(physical_helpers[0], physical_helpers[1]);
    let source = &owner.helper_memory.unit_source;
    assert_eq!(source.bodies.len(), 1);
    assert_eq!(source.associations.len(), 2);
    assert_eq!(source.calls.len(), 4);
    assert_eq!(owner.empty_effect_helpers().iter().count(), 0);
    for (ordinal, association) in source.associations.iter().enumerate() {
        assert_eq!(association.key.root.index() as usize, ordinal);
        assert_eq!(association.key.function.index(), 2);
        assert_eq!(association.body, 0);
        assert_eq!(association.call_count, 2);
        assert_eq!(
            source
                .calls
                .iter()
                .filter(|call| call.callee_association == ordinal)
                .count(),
            2
        );
    }
    for calls in source.calls.chunks_exact(2) {
        assert_ne!(calls[0].call, calls[1].call);
        assert_ne!(calls[0].source_block, calls[1].source_block);
        assert_eq!(calls[0].return_control, calls[1].return_control);
    }
    let associations = &source.associations;
    assert_eq!(associations[0].key.physical, associations[1].key.physical);
    assert!(associations[0].values.1 <= associations[1].values.0);
    assert!(associations[0].memory.1 <= associations[1].memory.0);
}

#[path = "production_local_helper_control_call_v1_tests.rs"]
mod control_call_tests;
#[path = "production_local_helper_memory_v1_tests.rs"]
mod memory_tests;
#[path = "production_local_helper_resource_v1_tests.rs"]
mod resource_tests;
#[path = "production_local_helper_ranked_v1_tests.rs"]
mod ranked_tests;
#[path = "production_local_helper_ranked_stage_v1_tests.rs"]
mod ranked_stage_tests;
