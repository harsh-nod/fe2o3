use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1 as Work,
    VerifiedCanonicalKernelIrModuleV12 as Native,
};
use fe2o3_mir_model::semantic_mir_v1::*;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const U64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const WITNESS: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const RAW: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(5);
const SLICE: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);
const RECEIVER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(7);
const ELEMENT_REF: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(8);
const OPTION: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(9);
const I64: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(10);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
const WORK: usize = 200_000_000;
const STORAGE: usize = 64 * 1024 * 1024;
const PREFIX: usize = 37;

fn declaration(
    tag: u8,
    layout: SemanticTypeLayoutV1,
    shape: SemanticTypeShapeV1,
) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([tag; 32]),
        layout,
        shape,
    )
}

fn pointer_scalar(reference: bool) -> SemanticBackendScalarV1 {
    SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(u128::from(reference), u64::MAX.into()),
    )
}

fn pointer_type(
    tag: u8,
    pointee: SemanticTypeIdV1,
    reference: bool,
    size: u64,
    alignment: u64,
) -> SemanticTypeDeclV1 {
    declaration(
        tag,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(pointer_scalar(reference)),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                pointee,
                if reference {
                    SemanticPointerKindV1::Reference
                } else {
                    SemanticPointerKindV1::Raw
                },
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    )
    .with_rustc_abi_properties(
        SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
            Some(
                SemanticAbiPointeeInfoV1::new(
                    if reference {
                        SemanticAbiPointeeKindV1::MutableReference { unpin: true }
                    } else {
                        SemanticAbiPointeeKindV1::Raw
                    },
                    size,
                    alignment,
                )
                .unwrap(),
            ),
            None,
        ),
    )
}

fn types(signed: bool) -> Vec<SemanticTypeDeclV1> {
    let seed = resource_tests::scalar_transmute_semantic_owner();
    let mut types = seed.semantic().types()[..2].to_vec();
    let integer = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::integer(false, 64, 8),
        SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
    );
    types.push(declaration(
        147,
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(integer),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
            signed: false,
            bits: 64,
        }),
    ));
    types.push(declaration(
        148,
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
    ));
    types.push(declaration(
        149,
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(integer),
            false,
            SemanticAggregateLayoutV1::new(vec![0, 0], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![U64, MARKER]).unwrap()),
    ));
    types.push(pointer_type(150, U32, false, 0, 1));
    types.push(
        declaration(
            151,
            SemanticTypeLayoutV1::aggregate_with_backend_repr(
                Some(16),
                8,
                SemanticBackendReprV1::ScalarPair {
                    first: pointer_scalar(false),
                    second: integer,
                },
                false,
                SemanticAggregateLayoutV1::new(vec![0, 8, 0], vec![]).unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::Aggregate(
                SemanticAggregateTypeV1::new(vec![RAW, U64, MARKER]).unwrap(),
            ),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(SemanticAbiPointeeInfoV1::new(SemanticAbiPointeeKindV1::Raw, 0, 1).unwrap()),
                None,
            ),
        ),
    );
    types.push(pointer_type(152, SLICE, true, 16, 8));
    types.push(pointer_type(153, U32, true, 4, 4));
    let niche = SemanticLayoutNicheV1::new(
        0,
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, u64::MAX.into()),
    )
    .unwrap();
    let nullable = SemanticBackendScalarV1::initialized(
        SemanticBackendPrimitiveV1::pointer(0, 8, 8),
        SemanticScalarValidityRangeV1::new(1, 0),
    );
    let variant = |index, offsets: Vec<u64>, repr, niche| {
        SemanticEnumVariantLayoutV1::from_rustc(
            index,
            8,
            8,
            SemanticFieldsShapeV1::arbitrary(offsets.clone(), (0..offsets.len() as u32).collect())
                .unwrap(),
            repr,
            niche,
            false,
            None,
            8,
            u64::from(index),
            SemanticAggregateLayoutV1::new(offsets, vec![]).unwrap(),
        )
        .unwrap()
    };
    types.push(
        declaration(
            154,
            SemanticTypeLayoutV1::enum_layout_with_backend_repr(
                8,
                8,
                SemanticBackendReprV1::scalar(nullable),
                false,
                SemanticEnumLayoutV1::new(
                    vec![
                        variant(0, vec![], SemanticBackendReprV1::memory(true), None),
                        variant(
                            1,
                            vec![0],
                            SemanticBackendReprV1::scalar(pointer_scalar(true)),
                            Some(niche),
                        ),
                    ],
                    SemanticEnumEncodingV1::Niche(
                        SemanticNicheEnumEncodingV1::new(
                            0,
                            SemanticNicheSourceV1::new(
                                vec![SemanticNichePathComponentV1::Field(0)],
                                0,
                            )
                            .unwrap(),
                            niche,
                            nullable,
                            1,
                            0,
                            0,
                            0,
                        )
                        .unwrap(),
                    ),
                )
                .unwrap(),
            )
            .unwrap(),
            SemanticTypeShapeV1::enum_type(
                if signed { I64 } else { U64 },
                vec![
                    SemanticEnumVariantV1::new(0, SemanticAggregateTypeV1::new(vec![]).unwrap()),
                    SemanticEnumVariantV1::new(
                        1,
                        SemanticAggregateTypeV1::new(vec![ELEMENT_REF]).unwrap(),
                    ),
                ],
            )
            .unwrap(),
        )
        .with_rustc_abi_properties(
            SemanticTypeAbiPropertiesV1::new(false, false).with_scalar_pointee_info(
                Some(
                    SemanticAbiPointeeInfoV1::new(
                        SemanticAbiPointeeKindV1::MutableReference { unpin: false },
                        0,
                        4,
                    )
                    .unwrap(),
                ),
                None,
            ),
        ),
    );
    if signed {
        types.push(declaration(
            155,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(8),
                8,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(true, 64, 8),
                    SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: true,
                bits: 64,
            }),
        ));
    }
    types
}

fn attributes(reference: bool, size: u64, alignment: Option<u64>) -> SemanticAbiValueAttributesV1 {
    SemanticAbiValueAttributesV1::new(
        SemanticAbiRegularAttributesV1::new(reference, None, reference, false, false, true),
        SemanticAbiExtensionV1::None,
        size,
        alignment,
    )
    .unwrap()
}
fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}
fn edge(role: SemanticEdgeRoleV1, target: u32) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(role, SemanticBlockIdV1::from_index(target))
}
fn statement(destination: SemanticPlaceV1, value: SemanticRvalueKindV1) -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination.clone(),
            SemanticRvalueV1::new(destination.ty(), value),
        )),
    )
}
fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    terminator: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), terminator),
    )
    .unwrap()
}

// Real admitted factory, not rustc provenance. All SSA identities and source/N
// locators below are produced by the current planner/capture/materializer.
fn source(value: u32, signed: bool, copies: bool, stores: usize) -> ProductionSemanticSsaOwnerV1 {
    let discriminator = if signed { I64 } else { U64 };
    let call = |callee, arguments, destination, ty, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                SemanticCallableIdV1::from_index(callee),
                arguments,
                Some(SemanticCallDestinationV1::new(
                    place(destination, ty),
                    edge(SemanticEdgeRoleV1::CallReturn, target),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let payload = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(4),
        vec![
            SemanticProjectionV1::new(SemanticProjectionKindV1::Downcast(1), OPTION).unwrap(),
            SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), ELEMENT_REF).unwrap(),
        ],
        ELEMENT_REF,
    )
    .unwrap();
    let pointer = SemanticPlaceV1::new(
        SemanticLocalIdV1::from_index(6),
        vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Dereference, U32).unwrap()],
        U32,
    )
    .unwrap();
    let constant = SemanticOperandV1::Constant(SemanticConstantV1::new(
        U32,
        SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(value.into(), 4).unwrap()),
    ));
    let mut writes = vec![statement(
        place(6, ELEMENT_REF),
        SemanticRvalueKindV1::Use(SemanticOperandV1::Move(payload)),
    )];
    if copies {
        writes.push(statement(
            place(7, U32),
            SemanticRvalueKindV1::Use(constant.clone()),
        ));
    }
    for _ in 0..stores {
        writes.push(statement(
            pointer.clone(),
            SemanticRvalueKindV1::Use(if copies {
                SemanticOperandV1::Copy(place(7, U32))
            } else {
                constant.clone()
            }),
        ));
    }
    let blocks = vec![
        block(170, vec![], call(1, vec![], 2, WITNESS, 1)),
        block(
            171,
            vec![statement(
                place(3, RECEIVER),
                SemanticRvalueKindV1::Borrow {
                    kind: SemanticBorrowKindV1::Mutable,
                    place: place(1, SLICE),
                },
            )],
            call(
                2,
                vec![
                    SemanticOperandV1::Move(place(3, RECEIVER)),
                    SemanticOperandV1::Move(place(2, WITNESS)),
                ],
                4,
                OPTION,
                2,
            ),
        ),
        block(
            172,
            vec![statement(
                place(5, discriminator),
                SemanticRvalueKindV1::Discriminant(place(4, OPTION)),
            )],
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: SemanticOperandV1::Copy(place(5, discriminator)),
                targets: SemanticSwitchTargetsV1::new(
                    vec![
                        SemanticSwitchTargetV1::new(0, edge(SemanticEdgeRoleV1::SwitchValue, 4)),
                        SemanticSwitchTargetV1::new(1, edge(SemanticEdgeRoleV1::SwitchValue, 3)),
                    ],
                    edge(SemanticEdgeRoleV1::SwitchOtherwise, 5),
                )
                .unwrap(),
            },
        ),
        block(
            173,
            writes,
            SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 4)),
        ),
        block(174, vec![], SemanticTerminatorKindV1::Return),
        block(175, vec![], SemanticTerminatorKindV1::Unreachable),
    ];
    let mut local_types = vec![
        (UNIT, SemanticLocalRoleV1::Return),
        (SLICE, SemanticLocalRoleV1::Argument(0)),
        (WITNESS, SemanticLocalRoleV1::Temporary),
        (RECEIVER, SemanticLocalRoleV1::Temporary),
        (OPTION, SemanticLocalRoleV1::Temporary),
        (discriminator, SemanticLocalRoleV1::Temporary),
        (ELEMENT_REF, SemanticLocalRoleV1::Temporary),
    ];
    if copies {
        local_types.push((U32, SemanticLocalRoleV1::Temporary));
    }
    let abi = SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([180; 32]),
        SemanticLayoutIdentityV1::from_sha256([180; 32]),
        SemanticCanonAbiV1::GpuKernel,
        SemanticExternAbiV1::GpuKernel,
        false,
        false,
        1,
        vec![SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SLICE,
            SemanticAbiPassModeV1::Pair {
                first: attributes(false, 0, None),
                second: attributes(false, 0, None),
            },
        ))],
        SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore),
    )
    .unwrap()
    .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ExclusiveOwner])
    .unwrap();
    let unavailable = SemanticSourceProvenanceV1::unavailable();
    let function = SemanticFunctionDeclV1::new(
        SemanticFunctionIdentityV1::from_sha256([181; 32]),
        SemanticFunctionRoleV1::KernelRoot,
        SemanticItemDefinitionIdentityV1::from_sha256([182; 32]),
        SemanticMonomorphizationIdentityV1::from_sha256([183; 32]),
        SemanticGenericTypeArgumentsIdentityV1::from_sha256([184; 32]),
        SemanticConstGenericArgumentsIdentityV1::from_sha256([185; 32]),
        unavailable,
        abi,
        local_types
            .into_iter()
            .enumerate()
            .map(|(index, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([190 + index as u8; 32]),
                    ty,
                    role,
                    unavailable,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"source_preservation".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([210; 32]),
        SemanticKernelSourceContractV1::new(
            Some(
                SemanticKernelLaunchBoundsV1::new(
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                    None,
                )
                .unwrap(),
            ),
            None,
            None,
        )
        .unwrap(),
    ));
    let intrinsic = |tag, inputs, output, operation| SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            unavailable,
            SemanticFunctionAbiV1::new(
                SemanticAbiIdentityV1::from_sha256([tag; 32]),
                SemanticLayoutIdentityV1::from_sha256([tag; 32]),
                SemanticCanonAbiV1::Rust,
                false,
                false,
                inputs,
                output,
            )
            .unwrap(),
        ),
        operation,
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    };
    let direct = |ty| {
        SemanticAbiValueV1::new(
            ty,
            SemanticAbiPassModeV1::Direct(attributes(false, 0, None)),
        )
    };
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types(signed),
        vec![],
        vec![],
        vec![],
        vec![function],
        vec![
            SemanticCallableDeclV1::defined(ROOT),
            intrinsic(
                220,
                vec![],
                direct(WITNESS),
                SemanticCompilerIntrinsicOperationV1::ThreadIndex1d {
                    index_witness: WITNESS,
                    raw_index: U64,
                },
            ),
            intrinsic(
                221,
                vec![
                    SemanticAbiValueV1::new(
                        RECEIVER,
                        SemanticAbiPassModeV1::Direct(attributes(true, 16, Some(8))),
                    ),
                    direct(WITNESS),
                ],
                SemanticAbiValueV1::new(
                    OPTION,
                    SemanticAbiPassModeV1::Direct(attributes(false, 0, Some(4))),
                ),
                SemanticCompilerIntrinsicOperationV1::DisjointSliceGetMut {
                    disjoint_slice: SLICE,
                    index_witness: WITNESS,
                    element: U32,
                    raw_index: U64,
                },
            ),
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_current_production(SemanticMirLimitsV1::default())
    .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(
        ProductionSemanticMirOwnerV1::try_new(
            admitted,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn with_fixture(
    value: u32,
    signed: bool,
    copies: bool,
    stores: usize,
    next: impl FnOnce(&ProductionPreRankedKirOwnerV1, &mut Budget<'_>),
) {
    let mut work = Work::new(WORK);
    let mut budget = Budget::new(&mut work, STORAGE);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(PREFIX).unwrap();
    {
        let mut ssa = source(value, signed, copies, stores);
        let capture = ssa
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(capture.retained_storage()).unwrap();
        let launch = crate::ProductionSourceLaunchRosterV1::try_new(
            ssa.source_semantic(),
            &[crate::ProductionSourceLaunchRootInputV1::new(
                "source_preservation",
                [210; 32],
                crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
            )],
        )
        .unwrap();
        let owner = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget,
        )
        .unwrap();
        budget
            .reserve_storage(owner.executable_storage().retained_storage())
            .unwrap();
        budget
            .reserve_storage(owner.assert_origin_storage().payload_storage())
            .unwrap();
        next(&owner, &mut budget);
    }
    budget.release_storage(budget.storage() - PREFIX).unwrap();
    assert_eq!(budget.storage(), PREFIX);
}

fn check(
    owner: &ProductionPreRankedKirOwnerV1,
    candidate: &Native,
    budget: &mut Budget<'_>,
) -> Result<usize, ProductionSourceOutputErrorV1> {
    let floor = budget.storage();
    let result = {
        source_preservation_root_v1(owner, candidate.module(), ROOT, budget)
            .map(|checked| checked.checked_rule_count())
    };
    budget.release_storage(budget.storage() - floor).unwrap();
    result
}

#[test]
fn independently_checked_source_n_rules_cover_values_copies_signedness_and_multiple_stores() {
    for value in [17, 29] {
        for signed in [false, true] {
            for copies in [false, true] {
                for stores in [1, 2] {
                    with_fixture(value, signed, copies, stores, |owner, budget| {
                        let before = budget.work();
                        let floor = budget.storage();
                        assert_eq!(
                            check(owner, owner.executable(), budget).unwrap(),
                            9 + stores + usize::from(copies)
                        );
                        assert_eq!(budget.storage(), floor);
                        assert!(budget.work() > before);
                    });
                }
            }
        }
    }
}

#[test]
fn separately_admitted_hostile_n_reaches_exact_independent_semantic_rules() {
    for signed in [false, true] {
        with_fixture(17, signed, false, 1, |owner, budget| {
            assert_eq!(check(owner, owner.executable(), budget).unwrap(), 10);
            for case in 0..9 {
                let mut proposed = owner.executable().module().clone();
                let body = proposed.functions[0].body.as_mut().unwrap();
                let expected = match case {
                    0 => {
                        let operation = body
                            .blocks
                            .iter_mut()
                            .flat_map(|block| &mut block.operations)
                            .find(|operation| {
                                matches!(operation.kind, OperationKind::Constant(Constant::U32(17)))
                            })
                            .unwrap();
                        operation.kind = OperationKind::Constant(Constant::U32(29));
                        "source preservation scalar value rule differs"
                    }
                    1 => {
                        let data = body
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .find(|operation| {
                                matches!(operation.kind, OperationKind::SliceData { .. })
                            })
                            .unwrap()
                            .results[0]
                            .id;
                        let store = body
                            .blocks
                            .iter_mut()
                            .flat_map(|block| &mut block.operations)
                            .find(|operation| matches!(operation.kind, OperationKind::Store { .. }))
                            .unwrap();
                        let OperationKind::Store { pointer, .. } = &mut store.kind else {
                            unreachable!()
                        };
                        *pointer = data;
                        "source preservation own Store value/address rule differs"
                    }
                    2 => {
                        let operation = body
                            .blocks
                            .iter_mut()
                            .flat_map(|block| &mut block.operations)
                            .find(|operation| matches!(operation.kind, OperationKind::Intrinsic(_)))
                            .unwrap();
                        operation.kind = OperationKind::Constant(Constant::Index(0));
                        "source preservation Global-X rule differs"
                    }
                    3 => {
                        let operation = body
                            .blocks
                            .iter_mut()
                            .flat_map(|block| &mut block.operations)
                            .find(|operation| {
                                matches!(operation.kind, OperationKind::Compare { .. })
                            })
                            .unwrap();
                        let OperationKind::Compare { predicate, .. } = &mut operation.kind else {
                            unreachable!()
                        };
                        *predicate = ComparePredicate::LessThanOrEqual;
                        "source preservation getter rule differs"
                    }
                    4 => {
                        let terminator = body
                            .blocks
                            .iter_mut()
                            .filter_map(|block| block.terminator.as_mut())
                            .find(|term| {
                                matches!(
                                    term,
                                    Terminator::Switch { .. } | Terminator::IntegerSwitch { .. }
                                )
                            })
                            .unwrap();
                        match terminator {
                            Terminator::Switch { cases, .. } => {
                                let target = cases[0].target;
                                cases[0].target = cases[1].target;
                                cases[1].target = target;
                            }
                            Terminator::IntegerSwitch { cases, .. } => {
                                let target = cases[0].target;
                                cases[0].target = cases[1].target;
                                cases[1].target = target;
                            }
                            _ => unreachable!(),
                        }
                        "source preservation Option polarity rule differs"
                    }
                    5 => {
                        let target = body
                            .blocks
                            .iter()
                            .find(|block| {
                                matches!(block.terminator, Some(Terminator::Return { .. }))
                            })
                            .unwrap()
                            .id;
                        let terminator = body
                            .blocks
                            .iter_mut()
                            .filter_map(|block| block.terminator.as_mut())
                            .find(|term| {
                                matches!(
                                    term,
                                    Terminator::Switch { .. } | Terminator::IntegerSwitch { .. }
                                )
                            })
                            .unwrap();
                        match terminator {
                            Terminator::Switch { default_target, .. }
                            | Terminator::IntegerSwitch { default_target, .. } => {
                                *default_target = target
                            }
                            _ => unreachable!(),
                        }
                        "source preservation Option default rule differs"
                    }
                    6 => {
                        let block = body
                            .blocks
                            .iter_mut()
                            .find(|block| {
                                block
                                    .operations
                                    .iter()
                                    .any(|op| matches!(op.kind, OperationKind::Store { .. }))
                            })
                            .unwrap();
                        let store = block
                            .operations
                            .iter()
                            .find(|op| matches!(op.kind, OperationKind::Store { .. }))
                            .unwrap()
                            .clone();
                        block.operations.push(store);
                        "source preservation complete operation census differs"
                    }
                    7 => {
                        let next = body
                            .blocks
                            .iter()
                            .flat_map(|block| &block.operations)
                            .flat_map(|operation| &operation.results)
                            .map(|value| value.id.0)
                            .max()
                            .unwrap()
                            .checked_add(1)
                            .unwrap();
                        let block = body
                            .blocks
                            .iter_mut()
                            .find(|block| {
                                matches!(block.terminator, Some(Terminator::Return { .. }))
                            })
                            .unwrap();
                        block.operations.push(Operation::new(
                            vec![ValueDef::new(ValueId(next), Type::Scalar(ScalarType::U32))],
                            OperationKind::Constant(Constant::U32(9)),
                        ));
                        "source preservation complete operation census differs"
                    }
                    8 => {
                        let to = Type::Scalar(if signed {
                            ScalarType::U64
                        } else {
                            ScalarType::I64
                        });
                        let cast = body
                            .blocks
                            .iter_mut()
                            .flat_map(|block| &mut block.operations)
                            .find(|operation| matches!(operation.kind, OperationKind::Cast { .. }))
                            .unwrap();
                        cast.results[0].ty = to.clone();
                        let OperationKind::Cast { to: actual, .. } = &mut cast.kind else {
                            unreachable!()
                        };
                        *actual = to;
                        let term = body
                            .blocks
                            .iter_mut()
                            .filter_map(|block| block.terminator.as_mut())
                            .find(|term| {
                                matches!(
                                    term,
                                    Terminator::Switch { .. } | Terminator::IntegerSwitch { .. }
                                )
                            })
                            .unwrap();
                        let value = |value: u64| {
                            if signed {
                                Constant::U64(value)
                            } else {
                                Constant::I64(value as i64)
                            }
                        };
                        *term = match term.clone() {
                            Terminator::Switch {
                                selector,
                                cases,
                                default_target,
                                default_arguments,
                            } => Terminator::IntegerSwitch {
                                selector,
                                cases: cases
                                    .into_iter()
                                    .map(|case| fe2o3_kernel_ir::IntegerSwitchCase {
                                        value: value(case.value),
                                        target: case.target,
                                        arguments: case.arguments,
                                    })
                                    .collect(),
                                default_target,
                                default_arguments,
                            },
                            Terminator::IntegerSwitch {
                                selector,
                                cases,
                                default_target,
                                default_arguments,
                            } => Terminator::IntegerSwitch {
                                selector,
                                cases: cases
                                    .into_iter()
                                    .map(|case| {
                                        let number = match case.value {
                                            Constant::I64(v) => u64::try_from(v).unwrap(),
                                            Constant::U64(v) => v,
                                            _ => unreachable!(),
                                        };
                                        fe2o3_kernel_ir::IntegerSwitchCase {
                                            value: value(number),
                                            target: case.target,
                                            arguments: case.arguments,
                                        }
                                    })
                                    .collect(),
                                default_target,
                                default_arguments,
                            },
                            _ => unreachable!(),
                        };
                        "source preservation Option discriminator rule differs"
                    }
                    _ => unreachable!(),
                };
                let floor = budget.storage();
                {
                    let (candidate, receipt) =
                        Native::from_module_ref_with_verification_budget_v12(&proposed, budget)
                            .unwrap_or_else(|error| {
                                panic!("candidate admission case={case} signed={signed}: {error:?}")
                            });
                    budget.reserve_storage(receipt.retained_storage()).unwrap();
                    assert_ne!(
                        candidate.canonical().identity(),
                        owner.executable().canonical().identity()
                    );
                    let result = check(owner, &candidate, budget);
                    assert!(
                        matches!(result, Err(ProductionSourceOutputErrorV1::Invalid(reason)) if reason == expected),
                        "new independent checker case={case} signed={signed}: {result:?}"
                    );
                }
                budget.release_storage(budget.storage() - floor).unwrap();
                check(owner, owner.executable(), budget).unwrap();
            }
        });
    }
}

fn owner_floor(owner: &ProductionPreRankedKirOwnerV1) -> usize {
    owner.executable_storage().retained_storage()
        + owner.assert_origin_storage().payload_storage()
        + owner
            .semantic_ssa()
            .occurrence_storage()
            .unwrap()
            .retained_storage()
}

fn before_first_header_work(owner: &ProductionPreRankedKirOwnerV1) -> usize {
    let semantic = owner.semantic_ssa().source_semantic();
    let correspondence = owner.correspondence.lowered_functions();
    assert_eq!(correspondence.len(), 1);
    7 + 8
        + semantic.roots().len()
        + 8
        + 4 * correspondence.len()
        + owner
            .executable()
            .module()
            .functions
            .iter()
            .map(|function| {
                3 + function.id.as_str().len()
                    + correspondence[0].kernel_ir_function().as_str().len()
            })
            .sum::<usize>()
        + 4 * semantic.functions()[ROOT.index() as usize].blocks().len()
}

#[test]
fn private_checker_entry_work_and_source_floor_refuse_before_allocation() {
    with_fixture(17, false, false, 1, |owner, _outer| {
        let floor = owner_floor(owner);
        let mut work = Work::new(14);
        {
            let mut budget = Budget::new(&mut work, STORAGE);
            budget.charge_work(7).unwrap();
            budget.reserve_storage(floor).unwrap();
            assert!(matches!(check(owner, owner.executable(), &mut budget),
                Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Work(limit)))
                if limit.actual() == 15 && limit.limit() == 14));
            assert_eq!(budget.work(), 7);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor);
        }
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor - 1).unwrap();
        assert!(matches!(
            check(owner, owner.executable(), &mut budget),
            Err(ProductionSourceOutputErrorV1::Resource(
                AssertOriginResourceV1::Accounting
            ))
        ));
        assert_eq!(budget.work(), 15);
        assert_eq!(budget.storage(), floor - 1);
        assert_eq!(budget.peak_storage(), floor - 1);
    });
}

#[test]
fn private_checker_first_header_denial_has_derived_prefix() {
    with_fixture(17, false, false, 1, |owner, _outer| {
        let floor = owner_floor(owner);
        let header = std::mem::size_of::<ProductionSourcePreservationRootV1>()
            - std::mem::size_of::<SourceOutputInvocationSourceIndexV1>();
        let mut work = Work::new(WORK);
        let mut budget = Budget::new(&mut work, floor + header - 1);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(check(owner, owner.executable(), &mut budget),
            Err(ProductionSourceOutputErrorV1::Resource(AssertOriginResourceV1::Storage(limit)))
            if limit.actual() == floor + header && limit.limit() == floor + header - 1));
        assert_eq!(budget.work(), before_first_header_work(owner));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
        assert_eq!(budget.failed_storage(), Some(floor + header));
    });
}

#[test]
fn private_checker_work_denial_after_first_actual_vector_allocation_preserves_floor() {
    with_fixture(17, false, false, 1, |owner, _outer| {
        let captured = owner
            .semantic_ssa()
            .occurrences_v1()
            .unwrap()
            .function(ROOT)
            .unwrap();
        let event = captured.events().first().unwrap();
        let key = source_output_invocation_event_key_v1(event).unwrap();
        let row = (key, 0usize);
        // First event charge + first capacity-growth charge are accepted; the
        // push charge is denied after the actual allocation is reconciled.
        let limit = before_first_header_work(owner) + 8 + 8 + 1;
        let floor = owner_floor(owner);
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, STORAGE);
        budget.charge_work(7).unwrap();
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(check(owner, owner.executable(), &mut budget),
            Err(ProductionSourceOutputErrorV1::SourceOrigin(SemanticKirAssertOriginErrorV1::Resource(
                AssertOriginResourceV1::Work(failure))))
            if failure.actual() == limit + 1 && failure.limit() == limit));
        assert_eq!(budget.work(), limit);
        assert_eq!(budget.storage(), floor);
        assert!(
            budget.peak_storage()
                >= floor
                    + std::mem::size_of::<ProductionSourcePreservationRootV1>()
                    + 4 * std::mem::size_of_val(&row)
        );
    });
}

fn vector_bytes<T>(rows: &Vec<T>) -> usize {
    rows.capacity() * std::mem::size_of::<T>()
}

#[test]
fn private_checked_row_retains_exact_capacity_bytes_and_named_rule_census() {
    with_fixture(29, true, true, 2, |owner, budget| {
        let floor = budget.storage();
        {
            let row = source_preservation_root_v1(owner, owner.executable().module(), ROOT, budget)
                .unwrap();
            let index = &row.source_index;
            let retained = std::mem::size_of::<ProductionSourcePreservationRootV1>()
                + vector_bytes(&row.coverage)
                + vector_bytes(&row.source_blocks)
                + vector_bytes(&row.neutral_blocks)
                + vector_bytes(&row.incoming)
                + vector_bytes(&row.ready)
                + vector_bytes(&row.source_facts.stores)
                + vector_bytes(&row.source_facts.some_region)
                + vector_bytes(&index.events)
                + vector_bytes(&index.definitions)
                + vector_bytes(&index.incoming)
                + vector_bytes(&index.transports)
                + vector_bytes(&index.statements)
                + vector_bytes(&index.terminators)
                + vector_bytes(&index.blocks)
                + vector_bytes(&index.values);
            assert_eq!(budget.storage(), floor + retained);
            assert_eq!(row.source_facts.stores.len(), 2);
            assert!(
                row.source_facts
                    .stores
                    .windows(2)
                    .all(|pair| pair[0].site < pair[1].site)
            );
            assert_eq!(row.selected_root(), ROOT);
            assert_eq!(row.selected_body(), ROOT);
            assert!(
                row.preconditions()
                    .requires_valid_exclusive_global_u32_slice()
            );
            assert!(
                row.preconditions()
                    .requires_representable_invocation_address()
            );
            let mut names = Vec::new();
            for ordinal in 0..row.checked_rule_count() {
                let before = budget.work();
                names.push(row.checked_rule_v1(ordinal, budget).unwrap().unwrap().4);
                assert_eq!(budget.work() - before, 8);
            }
            assert_eq!(names.iter().filter(|name| **name == "store").count(), 2);
            assert_eq!(names.iter().filter(|name| **name == "scalar").count(), 1);
            for name in [
                "borrow",
                "payload",
                "discriminant",
                "invocation",
                "getter",
                "goto",
                "switch",
                "return",
                "unreachable",
            ] {
                assert_eq!(
                    names.iter().filter(|actual| **actual == name).count(),
                    1,
                    "{name}"
                );
            }
            assert!(
                row.checked_rule_v1(row.checked_rule_count(), budget)
                    .unwrap()
                    .is_none()
            );
        }
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn canonical_store_index_missing_exact_site_is_rejected_by_own_store_rule() {
    with_fixture(17, false, false, 2, |owner, budget| {
        let floor = budget.storage();
        {
            let mut row =
                source_preservation_root_v1(owner, owner.executable().module(), ROOT, budget)
                    .unwrap();
            let site = row.source_facts.stores[0].site;
            assert!(
                row.source_facts
                    .stores
                    .windows(2)
                    .all(|pair| pair[0].site < pair[1].site)
            );
            let coverage = row
                .coverage
                .iter()
                .find(|coverage| {
                    coverage.source_block == site.0 && coverage.source_statement == Some(site.1)
                })
                .unwrap();
            let body = owner.executable().module().functions[0]
                .body
                .as_ref()
                .unwrap();
            let operations = &body.blocks[coverage.neutral_block as usize].operations
                [coverage.first_operation..coverage.end_operation];
            let source = &owner.semantic_ssa().source_semantic().functions()[0];
            let statement = source.blocks()[site.0 as usize].statements()[site.1 as usize].kind();
            assert!(matches!(
                source_preservation_statement_v1(
                    owner, &row, body, statement, site, operations, budget
                ),
                Ok(SourcePreservationRuleV1::Store)
            ));
            // Private hostile source-fact component, not a fabricated public
            // owner. Removal keeps the canonical strict ordering premise.
            row.source_facts.stores.remove(0);
            assert!(matches!(
                source_preservation_statement_v1(
                    owner, &row, body, statement, site, operations, budget
                ),
                Err(ProductionSourceOutputErrorV1::Invalid(
                    "source preservation own Store value/address rule differs"
                ))
            ));
        }
        budget.release_storage(budget.storage() - floor).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}
