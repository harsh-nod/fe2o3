// Genuine admitted semantic MIR with a live output, not collected Rust or a
// fabricated source receipt. The ranked row claims bounds, not helper purity.
const FP_UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const FP_VALUE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const FP_POINTER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const FP_CARRIER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const FP_NAME: &str = "f32_scalar_output";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FpRecipe {
    Negate,
    Divide,
    Remainder,
}

impl FpRecipe {
    fn rvalue(self, left: u32, right: u32, left_bits: Option<u32>) -> SemanticRvalueKindV1 {
        let left = left_bits.map_or_else(
            || value(left, FP_VALUE),
            |bits| constant(FP_VALUE, bits.into(), 4),
        );
        match self {
            Self::Negate => SemanticRvalueKindV1::Unary {
                operation: SemanticUnaryOpV1::Negate,
                operand: left,
            },
            Self::Divide | Self::Remainder => SemanticRvalueKindV1::Binary {
                operation: if self == Self::Divide {
                    SemanticBinaryOpV1::Divide
                } else {
                    SemanticBinaryOpV1::Remainder
                },
                left,
                right: value(right, FP_VALUE),
            },
        }
    }
}

fn fp_types(bits: u16) -> Vec<SemanticTypeDeclV1> {
    let pointer_backend = SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(1, 8, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    ));
    let pointer_properties = SemanticTypeAbiPropertiesV1::new(false, false)
        .with_scalar_pointee_info(
            Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
            None,
        );
    let bytes = u64::from(bits / 8);
    vec![
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([1; 32]),
            SemanticLayoutIdentityV1::from_sha256([1; 32]),
            SemanticTypeLayoutV1::with_exact_rustc_layout(
                0,
                1,
                SemanticFieldsShapeV1::arbitrary(vec![], vec![]).unwrap(),
                SemanticRustcVariantsV1::Single { index: 0 },
                SemanticBackendReprV1::memory(true),
                None,
                false,
                None,
                1,
                0,
                SemanticTypeLayoutDetailsV1::None,
            )
            .unwrap(),
            SemanticTypeShapeV1::Unit,
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([2; 32]),
            SemanticLayoutIdentityV1::from_sha256([2; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(bytes),
                bytes,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::float(bits, bytes),
                    SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits }),
        ),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([3; 32]),
            SemanticLayoutIdentityV1::from_sha256([3; 32]),
            SemanticTypeLayoutV1::new_with_backend_repr(Some(8), 8, pointer_backend, false)
                .unwrap(),
            SemanticTypeShapeV1::Pointer(
                SemanticPointerTypeV1::new_with_kind(
                    FP_VALUE,
                    SemanticPointerKindV1::Raw,
                    SemanticMutabilityV1::Mutable,
                    1,
                    64,
                    SemanticPointerMetadataV1::None,
                )
                .unwrap(),
            ),
        )
        .with_rustc_abi_properties(pointer_properties),
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([4; 32]),
            SemanticLayoutIdentityV1::from_sha256([4; 32]),
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(8),
                8,
                pointer_backend,
                false,
                SemanticAggregateLayoutV1::new(vec![0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![FP_POINTER]).unwrap()),
        )
        .with_rustc_abi_properties(pointer_properties),
    ]
}

fn fp_direct(ty: SemanticTypeIdV1) -> SemanticAbiValueV1 {
    SemanticAbiValueV1::new(
        ty,
        SemanticAbiPassModeV1::Direct(
            SemanticAbiValueAttributesV1::new(
                SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                SemanticAbiExtensionV1::None,
                0,
                None,
            )
            .unwrap(),
        ),
    )
}

fn fp_function(tag: u8, root: bool, blocks: Vec<SemanticBasicBlockV1>) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let (arguments, ownership, locals) = if root {
        (
            vec![FP_CARRIER, FP_VALUE, FP_VALUE],
            vec![
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
                SemanticSourceArgumentOwnershipV1::ByValue,
                SemanticSourceArgumentOwnershipV1::ByValue,
            ],
            vec![
                (FP_UNIT, SemanticLocalRoleV1::Return),
                (FP_CARRIER, SemanticLocalRoleV1::Argument(0)),
                (FP_VALUE, SemanticLocalRoleV1::Argument(1)),
                (FP_VALUE, SemanticLocalRoleV1::Argument(2)),
                (FP_POINTER, SemanticLocalRoleV1::Temporary),
                (FP_VALUE, SemanticLocalRoleV1::Temporary),
            ],
        )
    } else {
        (
            vec![FP_VALUE, FP_VALUE],
            vec![SemanticSourceArgumentOwnershipV1::ByValue; 2],
            vec![
                (FP_VALUE, SemanticLocalRoleV1::Return),
                (FP_VALUE, SemanticLocalRoleV1::Argument(0)),
                (FP_VALUE, SemanticLocalRoleV1::Argument(1)),
            ],
        )
    };
    let abi = SemanticFunctionAbiV1::from_rustc(
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
        arguments.len() as u32,
        arguments
            .into_iter()
            .map(|ty| SemanticAbiArgumentV1::source(fp_direct(ty)))
            .collect(),
        if root {
            SemanticAbiValueV1::new(FP_UNIT, SemanticAbiPassModeV1::Ignore)
        } else {
            fp_direct(FP_VALUE)
        },
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([tag; 32]),
        if root {
            SemanticFunctionRoleV1::KernelRoot
        } else {
            SemanticFunctionRoleV1::InternalHelper
        },
        SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
        source,
        abi,
        locals
            .into_iter()
            .enumerate()
            .map(|(index, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + 10 + index as u8; 32]),
                    ty,
                    role,
                    source,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap();
    if root {
        function.with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(FP_NAME.as_bytes().to_vec()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([30; 32]),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(
                        Some(SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap()),
                        Some(SemanticWorkgroupDimensionsV1::new([1, 1, 1]).unwrap()),
                        None,
                    )
                    .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        ))
    } else {
        function
    }
}

fn fp_receipt(
    bits: u16,
    recipe: FpRecipe,
    helper: bool,
) -> ProductionMaterializedRankedModuleReceiptV1 {
    fp_receipt_with_left_bits(bits, recipe, helper, None)
}

fn fp_receipt_with_left_bits(
    bits: u16,
    recipe: FpRecipe,
    helper: bool,
    left_bits: Option<u32>,
) -> ProductionMaterializedRankedModuleReceiptV1 {
    assert!(left_bits.is_none() || bits == 32);
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let pointer = assignment(
        4,
        FP_POINTER,
        SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(1),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), FP_POINTER)
                        .unwrap(),
                ],
                FP_POINTER,
            )
            .unwrap(),
        )),
    );
    let store = SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(4),
                vec![
                    SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, FP_VALUE)
                        .unwrap(),
                ],
                FP_VALUE,
            )
            .unwrap(),
            value(5, FP_VALUE),
            SemanticVolatilityV1::NonVolatile,
            None,
        )),
    );
    let blocks = if helper {
        vec![
            block(
                31,
                vec![pointer],
                SemanticTerminatorKindV1::Call(
                    SemanticDirectCallV1::new(
                        SemanticFunctionIdV1::from_index(1),
                        vec![value(2, FP_VALUE), value(3, FP_VALUE)],
                        Some(SemanticCallDestinationV1::new(
                            place(5, FP_VALUE),
                            edge(SemanticEdgeRoleV1::CallReturn, 1),
                        )),
                        SemanticUnwindActionV1::Unreachable,
                    )
                    .unwrap(),
                ),
            ),
            block(32, vec![store], SemanticTerminatorKindV1::Return),
        ]
    } else {
        vec![block(
            31,
            vec![
                pointer,
                assignment(5, FP_VALUE, recipe.rvalue(2, 3, left_bits)),
                store,
            ],
            SemanticTerminatorKindV1::Return,
        )]
    };
    let mut functions = vec![fp_function(30, true, blocks)];
    if helper {
        functions.push(fp_function(
            90,
            false,
            vec![block(
                91,
                vec![assignment(0, FP_VALUE, recipe.rvalue(1, 2, left_bits))],
                SemanticTerminatorKindV1::Return,
            )],
        ));
    }
    let request = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        fp_types(bits),
        vec![],
        vec![],
        vec![],
        functions,
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap();
    let semantic = ProductionSemanticMirOwnerV1::try_new(
        request
            .admit_current_production(SemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(semantic, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "logical_0",
            [30; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([1, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(FLOOR).unwrap();
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    let layout = source.source_launch().roots()[0].layout();
    let view = ProductionRankedValueIdV1::new(0);
    let index = ProductionRankedValueIdV1::new(1);
    let kernel = ProductionRankedKernelV1::new(
        FP_NAME,
        0,
        vec![ProductionRankedBlockV1::new(
            vec![
                ProductionRankedOperationV1::ExecutionLayout {
                    grid_identity: layout.grid_identity(),
                    global_extents: layout.global_extents(),
                    workgroup_extents: layout.workgroup_extents(),
                    subgroup_size: layout.subgroup_size(),
                    full_physical_workgroups: layout.full_physical_workgroups(),
                },
                ProductionRankedOperationV1::View {
                    result: view,
                    element_width: u32::from(bits),
                    writable: true,
                    shape: vec![1],
                    dynamic_extents: vec![],
                    allocation_origin: 1,
                    noalias_class: 0,
                },
                ProductionRankedOperationV1::IndexConstant {
                    result: index,
                    value: 0,
                },
                ProductionRankedOperationV1::Access {
                    kind: dialect_kernel::AccessKindAttr::Write,
                    view: ProductionRankedValueV1::Local(view),
                    indices: vec![ProductionRankedValueV1::Local(index)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel(FP_NAME, kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    assert!(lowering.all_mandatory_reports_are_clean());
    let root = ProductionRankedSemanticProjectionRootV1::new(
        SemanticFunctionIdV1::from_index(0),
        1,
        lowering,
        "one live scalar output; exact helper value retained by source/N replay".to_owned(),
        vec![ProductionRankedAccessSourceV1::new(
            if helper { 1 } else { 0 },
            Some(if helper { 0 } else { 2 }),
            0,
            0,
            3,
        )],
        vec![],
    );
    ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        source,
        vec![root],
    )
    .unwrap()
}

struct FpPrepared4 {
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    bound: VerifiedCanonicalKernelIrModuleV12,
    checked: fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1,
    floor: usize,
}

fn fp_prepare4(
    recipe: FpRecipe,
    helper: bool,
    profile: Profile,
    mutation: Option<fn(&mut Module)>,
) -> FpPrepared4 {
    fp_prepare_receipt4(fp_receipt(32, recipe, helper), profile, mutation)
}

fn fp_prepare_receipt4(
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    profile: Profile,
    mutation: Option<fn(&mut Module)>,
) -> FpPrepared4 {
    let Prepared {
        receipt,
        bound,
        output,
        source_storage,
        bound_storage,
        ..
    } = prepare(receipt, profile, mutation);
    drop(output);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = FLOOR + source_storage + bound_storage;
    budget.reserve_storage(floor).unwrap();
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, &mut budget)
            .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(checked.forwarding_rows().is_empty());
    let floor = floor + checked.retained_storage();
    FpPrepared4 {
        receipt,
        bound,
        checked,
        floor,
    }
}
