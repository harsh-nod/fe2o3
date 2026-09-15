use super::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const USIZE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const BOOL: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const REFERENCE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn copy(local: u32, ty: SemanticTypeIdV1) -> SemanticOperandV1 {
    SemanticOperandV1::Copy(place(local, ty))
}

fn scalar(value: u128, ty: SemanticTypeIdV1, bytes: u8) -> SemanticOperandV1 {
    SemanticOperandV1::Constant(SemanticConstantV1::new(
        ty,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value, bytes).unwrap()),
    ))
}

fn assign(local: u32, ty: SemanticTypeIdV1, kind: SemanticRvalueKindV1) -> SemanticStatementV1 {
    statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
        place(local, ty),
        SemanticRvalueV1::new(ty, kind),
    )))
}

fn types(write: bool) -> Vec<SemanticTypeDeclV1> {
    let old = neutral_semantic_types_v1();
    let boolean = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(230)),
        SemanticLayoutIdentityV1::from_sha256(bytes(230)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    );
    // The existing indexed-slice fixture's actual unsized layout and pair ABI.
    let slice = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(231)),
        SemanticLayoutIdentityV1::from_sha256(bytes(231)),
        SemanticTypeLayoutV1::with_exact_rustc_layout(
            0,
            4,
            SemanticFieldsShapeV1::array(4, 0),
            SemanticRustcVariantsV1::Single { index: 0 },
            SemanticBackendReprV1::memory(false),
            None,
            false,
            None,
            4,
            0,
            SemanticTypeLayoutDetailsV1::None,
        )
        .unwrap(),
        SemanticTypeShapeV1::Slice { element: U32 },
    );
    let reference = SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(232)),
        SemanticLayoutIdentityV1::from_sha256(bytes(232)),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(16),
            8,
            SemanticBackendReprV1::scalar_pair(
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                    SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
                ),
                SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                ),
            ),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SLICE,
                SemanticPointerKindV1::Reference,
                if write {
                    SemanticMutabilityV1::Mutable
                } else {
                    SemanticMutabilityV1::Immutable
                },
                0,
                64,
                SemanticPointerMetadataV1::SliceLength,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    if write {
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                    } else {
                        SemanticAbiPointeeKindV1::SharedReference { frozen: true }
                    },
                    0,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    vec![
        old[0].clone(),
        old[3].clone(),
        old[5].clone(),
        boolean,
        slice,
        reference,
    ]
}

fn source(
    write: bool,
    max_grid: u32,
) -> (
    ProductionSemanticSsaOwnerV1,
    Vec<ProductionRankedRootInputV1>,
) {
    let first = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(
            true,
            if write {
                None
            } else {
                Some(SemanticAbiPointerCaptureV1::CapturesReadOnly)
            },
            true,
            !write,
            false,
            true,
        ),
        SemanticAbiExtensionV1::None,
        0,
        Some(4),
    )
    .unwrap();
    let second = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let ownership = if write {
        SemanticSourceArgumentOwnershipV1::UniqueBorrow
    } else {
        SemanticSourceArgumentOwnershipV1::SharedBorrow
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256(bytes(234)),
        SemanticLayoutIdentityV1::from_sha256(bytes(250)),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            REFERENCE,
            SemanticAbiPassModeV1::Pair { first, second },
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![ownership])
    .unwrap();
    let element = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(2)),
                U32,
            )
            .unwrap(),
        ],
        U32,
    )
    .unwrap();
    let effect = if write {
        statement(SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            element,
            SemanticRvalueV1::new(U32, SemanticRvalueKindV1::Use(scalar(1, U32, 4))),
        )))
    } else {
        assign(
            5,
            U32,
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(element)),
        )
    };
    let dimensions = SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap();
    let name = if write {
        "coordinate_free_write"
    } else {
        "coordinate_free_read"
    };
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256(bytes(234)),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256(bytes(234)),
        SemanticMonomorphizationIdentityV1::from_sha256(bytes(234)),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(234)),
        SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(234)),
        SemanticSourceProvenanceV1::unavailable(),
        abi,
        [UNIT, REFERENCE, USIZE, USIZE, BOOL, U32]
            .into_iter()
            .enumerate()
            .map(|(index, ty)| {
                local(
                    240 + index as u8,
                    ty,
                    match index {
                        0 => SemanticLocalRoleV1::Return,
                        1 => SemanticLocalRoleV1::Argument(0),
                        _ => SemanticLocalRoleV1::Temporary,
                    },
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        vec![
            block(
                250,
                vec![
                    assign(2, USIZE, SemanticRvalueKindV1::Use(scalar(0, USIZE, 8))),
                    assign(
                        3,
                        USIZE,
                        SemanticRvalueKindV1::Unary {
                            operation: SemanticUnaryOpV1::PointerMetadata,
                            operand: copy(1, REFERENCE),
                        },
                    ),
                    assign(
                        4,
                        BOOL,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: copy(2, USIZE),
                            right: copy(3, USIZE),
                        },
                    ),
                ],
                SemanticTerminatorKindV1::Assert {
                    condition: copy(4, BOOL),
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length: copy(3, USIZE),
                        index: copy(2, USIZE),
                    },
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        SemanticBlockIdV1::from_index(1),
                    ),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(251, vec![effect], SemanticTerminatorKindV1::Return),
        ],
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(name.as_bytes().to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(233)),
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
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        types(write),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    assert_eq!(
        admitted.functions()[0].abi().source_argument_ownership(),
        &[ownership]
    );
    let mir = ProductionSemanticMirOwnerV1::try_new(
        admitted,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        mir,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    ssa.verify_replay().unwrap();
    let launch = LaunchContract::new(
        1,
        BlockSize::Exact(fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap()),
        fe2o3_artifacts::Dimensions::new(max_grid, 1, 1).unwrap(),
        0,
        0,
    )
    .unwrap();
    (
        ssa,
        vec![ProductionRankedRootInputV1::new(name, bytes(233), &launch)],
    )
}

#[test]
fn admitted_coordinate_free_read_keeps_dynamic_domain_and_mandatory_checks() {
    let (ssa, inputs) = source(false, u32::MAX);
    let expected = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs)
        .unwrap()
        .roots()[0]
        .layout();
    assert_eq!(expected.global_extents(), [0, 1, 1]);
    assert_eq!(expected.workgroup_extents(), [1, 1, 1]);
    let materialized = materialize_ranked_fixture_v1(ssa, &inputs).unwrap();
    let program = project_and_verify_ranked_materialized_semantic_mir_v1(
        materialized,
        &inputs,
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(vec![]),
    )
    .expect("a real coordinate-free read must reach clean mandatory analysis");
    assert_eq!(program.roots.len(), 1);
    let root = &program.roots[0];
    assert!(root.lowering.all_mandatory_reports_are_clean());
    assert!(projected_execution_domain_matches_v1(
        expected,
        root.lowering.kernel().blocks()[0].operations()
    ));
    assert!(
        !root
            .lowering
            .kernel()
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
            .any(|operation| matches!(
                operation,
                ProductionRankedOperationV1::InvocationIndex { .. }
            ))
    );
    assert_eq!(root.access_sources.len(), 1);
    let source = root.access_sources[0];
    assert_eq!(source.semantic_block(), 1);
    assert_eq!(source.semantic_statement(), Some(0));
    assert_eq!(source.semantic_access_ordinal(), 0);
    let operations = root.lowering.kernel().blocks()[source.ranked_block() as usize].operations();
    assert!(matches!(
        operations[source.ranked_operation() as usize],
        ProductionRankedOperationV1::Access {
            kind: AccessKindAttr::Read,
            ..
        }
    ));
    assert!(
        root.lowering
            .kernel()
            .blocks()
            .iter()
            .flat_map(|block| block.operations())
            .any(|operation| matches!(
                operation,
                ProductionRankedOperationV1::ViewInSpace {
                    memory_space: MemorySpaceAttr::Global,
                    writable: false,
                    ..
                }
            ))
    );
    program.materialized.semantic_ssa().verify_replay().unwrap();
}

#[test]
fn admitted_coordinate_free_nonexclusive_write_reaches_mandatory_race_refusal() {
    let (ssa, inputs) = source(true, 2);
    let expected = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs)
        .unwrap()
        .roots()[0]
        .layout();
    assert_eq!(expected.global_extents(), [2, 1, 1]);
    let materialized = materialize_ranked_fixture_v1(ssa, &inputs).unwrap();
    let error = match project_and_verify_ranked_materialized_semantic_mir_v1(
        materialized,
        &inputs,
        &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1::new(vec![]),
    ) {
        Ok(_) => panic!("constant-address writes are not invocation-exclusive"),
        Err(error) => error,
    };
    let ProductionRankedProjectionErrorV1::Compile {
        error,
        ranked_ir,
        access_sources,
    } = error
    else {
        panic!("the actual write must reach mandatory compilation: {error:?}")
    };
    let ProductionRankedCompileErrorV1::Session(
        fe2o3_pliron::ProductionSessionErrorV1::RankedRace(race),
    ) = *error
    else {
        panic!("the write must be refused by mandatory race analysis")
    };
    assert!(!ranked_ir.contains("kernel.invocation_index"));
    assert_eq!(access_sources.len(), 1);
    assert_eq!(access_sources[0].access, AccessKindAttr::Write);
    assert_eq!(access_sources[0].memory_space, MemorySpaceAttr::Global);
    // The existing slice-bounds path projects the source index as unknown.
    // This is a mandatory refusal, not an enumerated conflict witness.
    let [
        fe2o3_pliron::RankedRaceFindingV1::UnresolvedIndex {
            block,
            operation,
            dimension: 0,
            ..
        },
    ] = race.report().findings()
    else {
        panic!("the existing slice-index limitation must remain explicit: {race:?}")
    };
    assert_eq!(
        (*block, *operation),
        (access_sources[0].block, access_sources[0].operation)
    );
}

#[test]
fn execution_domain_requires_the_exact_source_layout_not_a_coordinate() {
    let (ssa, inputs) = source(false, u32::MAX);
    let expected = source_launch_roster_for_ranked_inputs_v1(&ssa, &inputs)
        .unwrap()
        .roots()[0]
        .layout();
    let exact = ranked_execution_layout_v1(expected);
    assert!(projected_execution_domain_matches_v1(
        expected,
        std::slice::from_ref(&exact)
    ));
    assert!(!projected_execution_domain_matches_v1(expected, &[]));
    let coordinate = ProductionRankedOperationV1::InvocationIndex {
        result: ProductionRankedValueIdV1::new(0),
        dimension: 0,
        launch_extent: 0,
    };
    assert!(!projected_execution_domain_matches_v1(
        expected,
        std::slice::from_ref(&coordinate)
    ));
    assert!(!projected_execution_domain_matches_v1(
        expected,
        &[coordinate, exact.clone()]
    ));
    for field in 0..10 {
        let mut changed = exact.clone();
        let ProductionRankedOperationV1::ExecutionLayout {
            grid_identity,
            global_extents,
            workgroup_extents,
            subgroup_size,
            full_physical_workgroups,
        } = &mut changed
        else {
            unreachable!()
        };
        match field {
            0 => *grid_identity ^= 1,
            1..=3 => global_extents[field - 1] = 2,
            4..=6 => workgroup_extents[field - 4] = 2,
            7 => *subgroup_size = 32,
            8 => *full_physical_workgroups = false,
            9 => global_extents[0] = 1,
            _ => unreachable!(),
        }
        assert!(
            !projected_execution_domain_matches_v1(expected, &[changed]),
            "field {field}"
        );
    }
    // Duplicate layouts are still rejected by the ordinary recipe constructor.
    assert!(matches!(
        ProductionRankedKernelV1::new(
            "duplicate_layout",
            0,
            vec![ProductionRankedBlockV1::new(
                vec![exact.clone(), exact],
                ProductionRankedTerminatorV1::Return
            )],
        ),
        Err(ProductionRankedKernelErrorV1::InvalidExecutionLayout)
    ));
}

include!("checked_output_domain_v1_tests.rs");
