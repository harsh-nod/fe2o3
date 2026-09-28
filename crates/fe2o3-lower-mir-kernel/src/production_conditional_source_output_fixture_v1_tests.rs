//! Admitted semantic MIR for `if global_x < output.len() { output[global_x] = stored; }`.
//! This is not a rustc capture or protected source-origin receipt.

use super::*;

const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const WITNESS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);

pub(super) fn output_source(stored: u32) -> ProductionPreRankedKirOwnerV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let mut catalog = types();
    let old = &catalog[SLICE_REF.index() as usize];
    catalog[SLICE_REF.index() as usize] = SemanticTypeDeclV1::new(
        old.identity(),
        old.layout_identity(),
        old.layout().clone(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                SLICE,
                SemanticPointerKindV1::Reference,
                SemanticMutabilityV1::Mutable,
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
                    SemanticAbiPointeeKindV1::MutableReference { unpin: true },
                    0,
                    4,
                )
                .unwrap(),
            ),
            None,
        ),
    );
    catalog.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([7; 32]),
        SemanticLayoutIdentityV1::from_sha256([7; 32]),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    catalog.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([8; 32]),
        SemanticLayoutIdentityV1::from_sha256([8; 32]),
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 64, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![U64, MARKER]).unwrap()),
    ));
    let pointer_attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(true, None, true, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        Some(4),
    )
    .unwrap();
    let length_attributes = SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
        SemanticAbiExtensionV1::None,
        0,
        None,
    )
    .unwrap();
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([30; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SLICE_REF,
            SemanticAbiPassModeV1::Pair {
                first: pointer_attributes,
                second: length_attributes,
            },
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::UniqueBorrow])
    .unwrap();
    let element = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(1),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, SLICE).unwrap(),
            SemanticProjectionV1::new(
                SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(3)),
                U32,
            )
            .unwrap(),
        ],
        U32,
    )
    .unwrap();
    let blocks = vec![
        block(
            31,
            vec![],
            SemanticTerminatorKindV1::Call(
                SemanticDirectCallV1::new_callable(
                    SemanticCallableIdV1::from_index(1),
                    vec![],
                    Some(SemanticCallDestinationV1::new(
                        place(5, WITNESS),
                        edge(SemanticEdgeRoleV1::CallReturn, 1),
                    )),
                    SemanticUnwindActionV1::Unreachable,
                )
                .unwrap(),
            ),
        ),
        block(
            32,
            vec![
                assignment(
                    2,
                    U64,
                    SemanticRvalueKindV1::Unary {
                        operation: SemanticUnaryOpV1::PointerMetadata,
                        operand: value(1, SLICE_REF),
                    },
                ),
                assignment(
                    3,
                    U64,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(5),
                            vec![
                                SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), U64)
                                    .unwrap(),
                            ],
                            U64,
                        )
                        .unwrap(),
                    )),
                ),
                assignment(
                    4,
                    BOOL,
                    SemanticRvalueKindV1::Binary {
                        operation: SemanticBinaryOpV1::LessThan,
                        left: value(3, U64),
                        right: value(2, U64),
                    },
                ),
            ],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: value(4, BOOL),
                targets: SemanticSwitchTargetsV1::new(
                    vec![SemanticSwitchTargetV1::new(
                        1,
                        edge(SemanticEdgeRoleV1::SwitchValue, 2),
                    )],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 3),
                )
                .unwrap(),
            },
        ),
        block(
            33,
            vec![SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    element,
                    SemanticRvalueV1::new(
                        U32,
                        SemanticRvalueKindV1::Use(constant(U32, u128::from(stored), 4)),
                    ),
                )),
            )],
            SemanticTerminatorKindV1::Return,
        ),
        block(34, vec![], SemanticTerminatorKindV1::Return),
    ];
    let dimensions = SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([30; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([30; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([30; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([30; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([30; 32]),
        source,
        abi,
        [
            (UNIT, SemanticLocalRoleV1::Return),
            (SLICE_REF, SemanticLocalRoleV1::Argument(0)),
            (U64, SemanticLocalRoleV1::Temporary),
            (U64, SemanticLocalRoleV1::Temporary),
            (BOOL, SemanticLocalRoleV1::Temporary),
            (WITNESS, SemanticLocalRoleV1::Temporary),
        ]
        .into_iter()
        .enumerate()
        .map(|(ordinal, (ty, role))| {
            SemanticLocalDeclV1::new(
                SemanticLocalIdentityV1::from_sha256([40 + ordinal as u8; 32]),
                ty,
                role,
                source,
            )
        })
        .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"conditional_output".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([30; 32]),
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
    let intrinsic = SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([220; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([220; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([220; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([220; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([220; 32]),
            source,
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([220; 32]),
                SemanticLayoutIdentityV1::from_sha256([220; 32]),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                vec![],
                SemanticAbiValueV1::new(WITNESS, SemanticAbiPassModeV1::Direct(length_attributes)),
            )
            .unwrap(),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
            index_witness: WITNESS,
            raw_index: U64,
        },
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([220; 32]),
    };
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        catalog,
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(0)),
            intrinsic,
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    let ssa = ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "logical_output",
            [30; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [u32::MAX, 1, 1]),
        )],
    )
    .unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
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
