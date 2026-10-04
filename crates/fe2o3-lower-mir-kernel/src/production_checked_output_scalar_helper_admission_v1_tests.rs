use super::*;

// Source/N and the compiled ranked global-read component are genuine. The
// private caller result is checked by source/N/B/C/O custody and forwarding,
// not by ranked helper-result normalization or a helper-derived extent proof.
#[derive(Clone, Copy, Debug)]
enum HelperScalar {
    Bool,
    Integer { signed: bool, bits: u16 },
}

impl HelperScalar {
    fn source_type(self) -> SemanticTypeIdV1 {
        match self {
            Self::Bool => BOOL,
            Self::Integer { .. } => SemanticTypeIdV1::from_index(6),
        }
    }

    fn declarations(self) -> Vec<SemanticTypeDeclV1> {
        let mut declarations = types();
        if let Self::Integer { signed, bits } = self {
            let bytes = u64::from(bits / 8);
            declarations.push(SemanticTypeDeclV1::new(
                SemanticTypeIdentityV1::from_sha256([71; 32]),
                SemanticLayoutIdentityV1::from_sha256([72; 32]),
                SemanticTypeLayoutV1::new_with_backend_repr(
                    Some(bytes),
                    bytes,
                    SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                        SemanticBackendPrimitiveV1::integer(signed, bits, bytes),
                        SemanticScalarValidityRangeV1::new(0, (1_u128 << bits) - 1),
                    )),
                    false,
                )
                .unwrap(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }),
            ));
        }
        declarations
    }

    fn abi_value(self, root: bool) -> SemanticAbiValueV1 {
        let extension = match self {
            Self::Bool => SemanticAbiExtensionV1::ZeroExtend,
            Self::Integer { signed: true, bits } if root && bits < 32 => {
                SemanticAbiExtensionV1::SignExtend
            }
            Self::Integer {
                signed: false,
                bits,
            } if root && bits < 32 => SemanticAbiExtensionV1::ZeroExtend,
            Self::Integer { .. } => SemanticAbiExtensionV1::None,
        };
        SemanticAbiValueV1::new(
            self.source_type(),
            SemanticAbiPassModeV1::Direct(
                SemanticAbiValueAttributesV1::new(
                    SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                    extension,
                    0,
                    None,
                )
                .unwrap(),
            ),
        )
    }
}

fn helper_scalar_matrix() -> Vec<HelperScalar> {
    std::iter::once(HelperScalar::Bool)
        .chain([false, true].into_iter().flat_map(|signed| {
            [8, 16, 32, 64]
                .into_iter()
                .map(move |bits| HelperScalar::Integer { signed, bits })
        }))
        .collect()
}

fn scalar_helper_abi(scalar: HelperScalar, tag: u8, root: bool) -> SemanticFunctionAbiV1 {
    let mut arguments = Vec::new();
    let mut ownership = Vec::new();
    if root {
        let first = SemanticAbiValueAttributesV1::new(
            SemanticAbiRegularAttributesV1::new(
                true,
                Some(SemanticAbiPointerCaptureV1::CapturesReadOnly),
                true,
                true,
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
        arguments.push(SemanticAbiArgumentV1::source(SemanticAbiValueV1::new(
            SLICE_REF,
            SemanticAbiPassModeV1::Pair { first, second },
        )));
        ownership.push(SemanticSourceArgumentOwnershipV1::SharedBorrow);
    }
    arguments.extend((0..2).map(|_| SemanticAbiArgumentV1::source(scalar.abi_value(root))));
    ownership.extend([SemanticSourceArgumentOwnershipV1::ByValue; 2]);
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
        arguments.len() as u32,
        arguments,
        if root {
            SemanticAbiValueV1::new(UNIT, SemanticAbiPassModeV1::Ignore)
        } else {
            scalar.abi_value(false)
        },
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap()
}

fn scalar_helper_decl(
    scalar: HelperScalar,
    tag: u8,
    root: bool,
    local_types: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    SemanticFunctionDeclV1::new(
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
        scalar_helper_abi(scalar, tag, root),
        local_types
            .iter()
            .copied()
            .enumerate()
            .map(|(ordinal, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + 10 + ordinal as u8; 32]),
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
}

fn scalar_helper_source(scalar: HelperScalar, changed_body: bool) -> ProductionPreRankedKirOwnerV1 {
    let ty = scalar.source_type();
    let source = SemanticSourceProvenanceV1::unavailable();
    let call = |callee, destination, next, reversed| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(callee),
                if reversed {
                    vec![value(3, ty), value(2, ty)]
                } else {
                    vec![value(2, ty), value(3, ty)]
                },
                Some(SemanticCallDestinationV1::new(
                    place(destination, ty),
                    edge(SemanticEdgeRoleV1::CallReturn, next),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let root = scalar_helper_decl(
        scalar,
        30,
        true,
        &[
            (UNIT, SemanticLocalRoleV1::Return),
            (SLICE_REF, SemanticLocalRoleV1::Argument(0)),
            (ty, SemanticLocalRoleV1::Argument(1)),
            (ty, SemanticLocalRoleV1::Argument(2)),
            (U64, SemanticLocalRoleV1::Temporary),
            (U64, SemanticLocalRoleV1::Temporary),
            (BOOL, SemanticLocalRoleV1::Temporary),
            (U32, SemanticLocalRoleV1::Temporary),
            (ty, SemanticLocalRoleV1::Temporary),
            (ty, SemanticLocalRoleV1::Temporary),
            (ty, SemanticLocalRoleV1::Temporary),
            (ty, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(
                31,
                vec![
                    assignment(
                        4,
                        U64,
                        SemanticRvalueKindV1::Unary {
                            operation: SemanticUnaryOpV1::PointerMetadata,
                            operand: value(1, SLICE_REF),
                        },
                    ),
                    assignment(5, U64, SemanticRvalueKindV1::Use(constant(U64, 0, 8))),
                    assignment(
                        6,
                        BOOL,
                        SemanticRvalueKindV1::Binary {
                            operation: SemanticBinaryOpV1::LessThan,
                            left: value(5, U64),
                            right: value(4, U64),
                        },
                    ),
                ],
                SemanticTerminatorKindV1::Assert {
                    condition: value(6, BOOL),
                    expected: true,
                    message: SemanticAssertMessageV1::BoundsCheck {
                        length: value(4, U64),
                        index: value(5, U64),
                    },
                    target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
            block(
                32,
                vec![assignment(
                    7,
                    U32,
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(
                        SemanticPlaceV1::new(
                            SemanticLocalIdV1::from_index(1),
                            vec![
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Dereference,
                                    SLICE,
                                )
                                .unwrap(),
                                SemanticProjectionV1::new(
                                    SemanticProjectionKindV1::Index(SemanticLocalIdV1::from_index(
                                        5,
                                    )),
                                    U32,
                                )
                                .unwrap(),
                            ],
                            U32,
                        )
                        .unwrap(),
                    )),
                )],
                call(1, 8, 2, false),
            ),
            block(33, vec![], call(2, 9, 3, true)),
            block(
                34,
                vec![
                    SemanticStatementV1::new(
                        source,
                        SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                            place(10, ty),
                            value(8, ty),
                            SemanticVolatilityV1::NonVolatile,
                            None,
                        )),
                    ),
                    assignment(11, ty, SemanticRvalueKindV1::Use(value(10, ty))),
                ],
                SemanticTerminatorKindV1::Return,
            ),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"private_array_relation".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256([30; 32]),
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
    let helper = |tag, operation| {
        scalar_helper_decl(
            scalar,
            tag,
            false,
            &[
                (ty, SemanticLocalRoleV1::Return),
                (ty, SemanticLocalRoleV1::Argument(0)),
                (ty, SemanticLocalRoleV1::Argument(1)),
            ],
            vec![block(
                tag + 1,
                vec![assignment(
                    0,
                    ty,
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left: value(1, ty),
                        right: value(2, ty),
                    },
                )],
                SemanticTerminatorKindV1::Return,
            )],
        )
    };
    let admitted = InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        scalar.declarations(),
        vec![],
        vec![],
        vec![],
        vec![
            root,
            helper(
                90,
                if changed_body {
                    SemanticBinaryOpV1::BitOr
                } else {
                    SemanticBinaryOpV1::BitXor
                },
            ),
            helper(120, SemanticBinaryOpV1::BitAnd),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
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
            "logical_0",
            [30; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
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

fn scalar_helper_receipt(
    scalar: HelperScalar,
    changed_body: bool,
) -> ProductionMaterializedRankedModuleReceiptV1 {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let source = scalar_helper_source(scalar, changed_body);
    let layout = source.source_launch().roots()[0].layout();
    let view = ProductionRankedValueIdV1::new(0);
    let index = ProductionRankedValueIdV1::new(1);
    let kernel = ProductionRankedKernelV1::new(
        "private_array_relation",
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
                    element_width: 32,
                    writable: false,
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
                    kind: dialect_kernel::AccessKindAttr::Read,
                    view: ProductionRankedValueV1::Local(view),
                    indices: vec![ProductionRankedValueV1::Local(index)],
                },
            ],
            ProductionRankedTerminatorV1::Return,
        )],
    )
    .unwrap();
    let lowering = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("private_array_relation", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap();
    assert!(lowering.all_mandatory_reports_are_clean());
    let root = ProductionRankedSemanticProjectionRootV1::new(
        ARRAY_ROOT,
        1,
        lowering,
        "independent source-correlated global read; no ranked helper-result normalization"
            .to_owned(),
        vec![ProductionRankedAccessSourceV1::new(1, Some(0), 0, 0, 3)],
        vec![],
    );
    ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
        source,
        vec![root],
    )
    .unwrap()
}

fn scalar_helper_root(module: &Module) -> &fe2o3_kernel_ir::Function {
    module
        .functions
        .iter()
        .find(|function| function.id == module.kernels[0].entry)
        .unwrap()
}

fn scalar_helper_private_counts(module: &Module) -> (usize, usize, usize) {
    let mut counts = (0, 0, 0);
    for operation in module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        match operation.kind {
            OperationKind::Alloca {
                address_space: AddressSpace::Private,
                ..
            } => counts.0 += 1,
            OperationKind::Store { access, .. }
                if access.address_space == AddressSpace::Private =>
            {
                counts.1 += 1
            }
            OperationKind::Load { access, .. } if access.address_space == AddressSpace::Private => {
                counts.2 += 1
            }
            _ => {}
        }
    }
    counts
}

fn scalar_helper_assert_caller(module: &Module, scalar: HelperScalar, expected_loads: usize) {
    let root = scalar_helper_root(module);
    let operations: Vec<_> = root
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .collect();
    let calls: Vec<_> = operations.iter().filter(|operation| matches!(&operation.kind,
        OperationKind::Call { callee, .. } if module.functions.iter().any(|function|
            function.id == *callee && function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper))).collect();
    assert!(
        !calls.is_empty(),
        "the live scalar helper call remains present"
    );
    for call in calls {
        let [result] = call.results.as_slice() else {
            panic!("one direct helper result")
        };
        match scalar {
            HelperScalar::Bool => assert_eq!(result.ty, Type::BOOL),
            HelperScalar::Integer { signed, bits } => {
                let actual = result.ty.as_scalar().unwrap();
                assert_eq!(actual.is_signed_integer(), signed);
                assert_eq!(actual.bit_width(), Some(bits));
            }
        }
    }
    assert_eq!(scalar_helper_private_counts(module), (1, 1, expected_loads));
}

#[test]
fn general_policy3_scalar_helpers_keep_direct_results_and_actual_global_read() {
    for scalar in helper_scalar_matrix() {
        with_prepared(
            prepare(scalar_helper_receipt(scalar, false), Profile::Gfx942, None),
            |input, budget| {
                scalar_helper_assert_caller(input.bound.module(), scalar, 1);
                let floor = budget.storage();
                let owner = AdmittedOutput::try_admit_general_v1(
                    input.receipt,
                    input.bound,
                    input.output,
                    budget,
                )
                .unwrap();
                scalar_helper_assert_caller(owner.output().module(), scalar, 1);
                let [fact] = owner.kernels()[0].accesses() else {
                    panic!("one independent global read")
                };
                assert_eq!(fact.kind(), fe2o3_kernel_ir::FormalMemoryAccessKind::Read);
                assert_eq!(fact.address_space(), AddressSpace::Global);
                assert_eq!(owner.kernels()[0].bounds_requirements().len(), 1);
                owner.verify_equivalence(budget).unwrap();
                assert_eq!(budget.storage(), floor);
                assert!(!owner.grants_artifact_or_launch_authority());
            },
        );
    }
}

struct ScalarHelperPrepared4 {
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    bound: VerifiedCanonicalKernelIrModuleV12,
    checked: fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1,
    source_storage: usize,
    bound_storage: usize,
}

impl ScalarHelperPrepared4 {
    fn floor(&self) -> usize {
        FLOOR + self.source_storage + self.bound_storage + self.checked.retained_storage()
    }
}

fn scalar_helper_prepare4(
    scalar: HelperScalar,
    changed_body: bool,
    mutation: Option<fn(&mut Module)>,
) -> ScalarHelperPrepared4 {
    let Prepared {
        receipt,
        bound,
        output,
        source_storage,
        bound_storage,
        ..
    } = prepare(
        scalar_helper_receipt(scalar, changed_body),
        Profile::Gfx942,
        mutation,
    );
    drop(output);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = FLOOR + source_storage + bound_storage;
    budget.reserve_storage(floor).unwrap();
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, &mut budget)
            .unwrap();
    // Policy4 forwards ordinary integers, not Bool. Helper admission must not
    // widen the independent memory-rewrite contract.
    let retained_loads = usize::from(matches!(scalar, HelperScalar::Bool));
    assert_eq!(checked.forwarding_rows().len(), 1 - retained_loads);
    assert_eq!(
        scalar_helper_private_counts(checked.intermediate_policy3().owner().module()),
        (1, 1, 1)
    );
    assert_eq!(
        scalar_helper_private_counts(checked.owner().module()),
        (1, 1, retained_loads)
    );
    assert_eq!(
        checked
            .intermediate_policy3()
            .owner()
            .canonical()
            .identity()
            == checked.owner().canonical().identity(),
        retained_loads == 1
    );
    assert_eq!(budget.storage(), floor);
    ScalarHelperPrepared4 {
        receipt,
        bound,
        checked,
        source_storage,
        bound_storage,
    }
}

#[test]
fn general_policy4_scalar_helpers_preserve_bool_and_forward_integer_caller_loads() {
    for scalar in helper_scalar_matrix() {
        let input = scalar_helper_prepare4(scalar, false, None);
        let floor = input.floor();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let owner = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
            input.receipt,
            input.bound,
            input.checked,
            &mut budget,
        )
        .unwrap();
        scalar_helper_assert_caller(
            owner.output().module(),
            scalar,
            usize::from(matches!(scalar, HelperScalar::Bool)),
        );
        let [fact] = owner.kernels()[0].accesses() else {
            panic!("one independent global read")
        };
        assert_eq!(fact.kind(), fe2o3_kernel_ir::FormalMemoryAccessKind::Read);
        assert_eq!(fact.address_space(), AddressSpace::Global);
        assert_eq!(owner.kernels()[0].bounds_requirements().len(), 1);
        owner.verify_equivalence(&mut budget).unwrap();
        assert_eq!(budget.storage(), floor);
        assert!(!owner.grants_artifact_or_launch_authority());
        drop(owner);
        budget.release_storage(floor - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

fn scalar_helper_change_callee(module: &mut Module) {
    let root_id = module.kernels[0].entry.clone();
    let root = module
        .functions
        .iter_mut()
        .find(|function| function.id == root_id)
        .unwrap();
    let calls: Vec<_> = root
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(|operation| match &operation.kind {
            OperationKind::Call { callee, .. } if !operation.results.is_empty() => {
                Some(callee.clone())
            }
            _ => None,
        })
        .collect();
    assert_eq!(calls.len(), 2);
    assert_ne!(calls[0], calls[1]);
    for operation in root
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
    {
        if let OperationKind::Call { callee, .. } = &mut operation.kind
            && *callee == calls[0]
        {
            *callee = calls[1].clone();
            return;
        }
    }
    panic!("first helper call");
}

fn scalar_helper_change_operand(module: &mut Module) {
    let root_id = module.kernels[0].entry.clone();
    let root = module
        .functions
        .iter_mut()
        .find(|function| function.id == root_id)
        .unwrap();
    for operation in root
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
    {
        if let OperationKind::Call { arguments, .. } = &mut operation.kind
            && !operation.results.is_empty()
        {
            assert_eq!(arguments.len(), 2);
            assert_ne!(arguments[0], arguments[1]);
            arguments[0] = arguments[1];
            return;
        }
    }
    panic!("direct scalar helper call");
}

fn scalar_helper_change_stored_result(module: &mut Module) {
    let root_id = module.kernels[0].entry.clone();
    let root = module
        .functions
        .iter_mut()
        .find(|function| function.id == root_id)
        .unwrap();
    let scalar_type = root
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .find(|operation| {
            matches!(operation.kind, OperationKind::Call { .. }) && !operation.results.is_empty()
        })
        .unwrap()
        .results[0]
        .ty
        .clone();
    let source = root
        .signature
        .parameters
        .iter()
        .zip(&root.body.as_ref().unwrap().parameters)
        .find(|(ty, _)| **ty == scalar_type)
        .map(|(_, value)| *value)
        .unwrap();
    for operation in root
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|block| &mut block.operations)
    {
        if let OperationKind::Store { value, access, .. } = &mut operation.kind
            && access.address_space == AddressSpace::Private
        {
            assert_ne!(*value, source);
            *value = source;
            return;
        }
    }
    panic!("live caller private store");
}

fn scalar_helper_change_body(module: &mut Module) {
    for function in &mut module.functions {
        if function.role != fe2o3_kernel_ir::FunctionRole::InternalHelper {
            continue;
        }
        for operation in function
            .body
            .as_mut()
            .unwrap()
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.operations)
        {
            if let OperationKind::Binary { op, .. } = &mut operation.kind
                && *op == BinaryOp::BitXor
            {
                *op = BinaryOp::BitOr;
                return;
            }
        }
    }
    panic!("scalar helper body");
}

#[test]
fn general_policy3_scalar_helper_callee_operand_result_and_body_mutations_fail_exact_join() {
    for scalar in [
        HelperScalar::Bool,
        HelperScalar::Integer {
            signed: false,
            bits: 32,
        },
    ] {
        for mutate in [
            scalar_helper_change_callee as fn(&mut Module),
            scalar_helper_change_operand,
            scalar_helper_change_stored_result,
            scalar_helper_change_body,
        ] {
            with_prepared(
                prepare(
                    scalar_helper_receipt(scalar, false),
                    Profile::Gfx942,
                    Some(mutate),
                ),
                |input, budget| {
                    let floor = budget.storage();
                    // Fresh canonical verification is real, but it cannot replace
                    // exact source/N/B correspondence. This fails before native census.
                    assert!(matches!(
                        AdmittedOutput::try_admit_general_v1(
                            input.receipt,
                            input.bound,
                            input.output,
                            budget
                        ),
                        Err(AdmissionError::Coordinates(_))
                    ));
                    assert_eq!(budget.storage(), floor);
                },
            );
        }
    }
}

#[test]
fn general_policy4_scalar_helper_mutations_do_not_gain_authority_from_forwarding() {
    let scalar = HelperScalar::Integer {
        signed: true,
        bits: 64,
    };
    for mutate in [
        scalar_helper_change_callee as fn(&mut Module),
        scalar_helper_change_operand,
        scalar_helper_change_stored_result,
        scalar_helper_change_body,
    ] {
        let input = scalar_helper_prepare4(scalar, false, Some(mutate));
        let floor = input.floor();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget
            ),
            Err(
                crate::ProductionCheckedOutputAdmissionErrorPolicy4V1::Admission(
                    AdmissionError::Coordinates(_)
                )
            )
        ));
        assert_eq!(budget.storage(), floor);
        budget.release_storage(floor - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
    }
}

#[test]
fn general_policy3_scalar_helper_checked_history_is_not_interchangeable() {
    let scalar = HelperScalar::Integer {
        signed: false,
        bits: 32,
    };
    let mut original = prepare(scalar_helper_receipt(scalar, false), Profile::Gfx942, None);
    let replacement = prepare(scalar_helper_receipt(scalar, true), Profile::Gfx942, None);
    assert_ne!(
        original.bound.canonical().identity(),
        replacement.bound.canonical().identity()
    );
    original.output = replacement.output;
    original.output_storage = replacement.output_storage;
    drop((replacement.receipt, replacement.bound));
    with_prepared(original, |input, budget| {
        let floor = budget.storage();
        assert!(matches!(
            AdmittedOutput::try_admit_general_v1(input.receipt, input.bound, input.output, budget),
            Err(AdmissionError::SourceOutput(
                ProductionSourceOutputErrorV1::InputCustody
            ))
        ));
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn general_policy4_scalar_helper_checked_history_is_not_interchangeable() {
    let scalar = HelperScalar::Bool;
    let original = scalar_helper_prepare4(scalar, false, None);
    let replacement = scalar_helper_prepare4(scalar, true, None);
    assert_ne!(
        original.bound.canonical().identity(),
        replacement.bound.canonical().identity()
    );
    let floor = FLOOR
        + original.source_storage
        + original.bound_storage
        + replacement.checked.retained_storage();
    drop((original.checked, replacement.receipt, replacement.bound));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    assert!(matches!(
        crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
            original.receipt,
            original.bound,
            replacement.checked,
            &mut budget
        ),
        Err(
            crate::ProductionCheckedOutputAdmissionErrorPolicy4V1::Admission(
                AdmissionError::SourceOutput(ProductionSourceOutputErrorV1::InputCustody)
            )
        )
    ));
    assert_eq!(budget.storage(), floor);
    budget.release_storage(floor - FLOOR).unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn general_policy4_scalar_helpers_restore_retained_floor_on_measured_budget_failure() {
    let scalar = HelperScalar::Integer {
        signed: false,
        bits: 64,
    };
    let input = scalar_helper_prepare4(scalar, false, None);
    let floor = input.floor();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(floor).unwrap();
    drop(
        crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
            input.receipt,
            input.bound,
            input.checked,
            &mut budget,
        )
        .unwrap(),
    );
    let used = budget.work();
    let scratch = budget.peak_storage() - floor;
    assert!(used > 8 && scratch > 0);
    assert_eq!(budget.storage(), floor);
    budget.release_storage(floor - FLOOR).unwrap();
    assert_eq!(budget.storage(), FLOOR);
    for short_work in [true, false] {
        let input = scalar_helper_prepare4(scalar, false, None);
        let floor = input.floor();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(if short_work { used - 1 } else { WORK });
        let mut budget = AssertOriginBudgetV1::new(
            &mut work,
            if short_work {
                STORAGE
            } else {
                floor + scratch - 1
            },
        );
        budget.reserve_storage(floor).unwrap();
        assert!(
            crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
        if !short_work {
            assert!(budget.failed_storage().is_some());
        }
        budget.release_storage(floor - FLOOR).unwrap();
        assert_eq!(budget.storage(), FLOOR);
        if short_work {
            assert!(work.failed_work().is_some());
        }
    }
}
