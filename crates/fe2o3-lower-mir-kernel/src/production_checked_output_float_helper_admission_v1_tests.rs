use super::*;

// The global read is independently source/ranked-correlated. Helper returns
// are checked by source/N/B/C/O transport, not a ranked return/extent summary.
const FLOAT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(6);

fn float_types(bits: u16) -> Vec<SemanticTypeDeclV1> {
    let mut declarations = types();
    let bytes = u64::from(bits / 8);
    declarations.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([71; 32]),
        SemanticLayoutIdentityV1::from_sha256([72; 32]),
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
    ));
    declarations
}

fn float_abi(tag: u8, root: bool) -> SemanticFunctionAbiV1 {
    let direct = || {
        SemanticAbiValueV1::new(
            FLOAT,
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
    };
    let mut arguments = vec![];
    let mut ownership = vec![];
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
    arguments.extend((0..2).map(|_| SemanticAbiArgumentV1::source(direct())));
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
            direct()
        },
    )
    .unwrap()
    .with_source_argument_ownership(ownership)
    .unwrap()
}

fn float_decl(
    tag: u8,
    root: bool,
    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let provenance = SemanticSourceProvenanceV1::unavailable();
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
        provenance,
        float_abi(tag, root),
        locals
            .iter()
            .enumerate()
            .map(|(i, (ty, role))| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([tag + 10 + i as u8; 32]),
                    *ty,
                    *role,
                    provenance,
                )
            })
            .collect(),
        SemanticBlockIdV1::from_index(0),
        blocks,
    )
    .unwrap()
}

fn float_request(bits: u16, op: SemanticBinaryOpV1) -> InertSemanticMirRequestV1 {
    let call = |callee, left, right, destination, next| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new(
                SemanticFunctionIdV1::from_index(callee),
                vec![value(left, FLOAT), value(right, FLOAT)],
                Some(SemanticCallDestinationV1::new(
                    place(destination, FLOAT),
                    edge(SemanticEdgeRoleV1::CallReturn, next),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let root = float_decl(
        30,
        true,
        &[
            (UNIT, SemanticLocalRoleV1::Return),
            (SLICE_REF, SemanticLocalRoleV1::Argument(0)),
            (FLOAT, SemanticLocalRoleV1::Argument(1)),
            (FLOAT, SemanticLocalRoleV1::Argument(2)),
            (U64, SemanticLocalRoleV1::Temporary),
            (U64, SemanticLocalRoleV1::Temporary),
            (BOOL, SemanticLocalRoleV1::Temporary),
            (U32, SemanticLocalRoleV1::Temporary),
            (FLOAT, SemanticLocalRoleV1::Temporary),
            (FLOAT, SemanticLocalRoleV1::Temporary),
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
                call(1, 2, 3, 8, 2),
            ),
            // The second retained call consumes the exact first helper result.
            block(33, vec![], call(2, 8, 3, 9, 3)),
            block(34, vec![], SemanticTerminatorKindV1::Return),
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
        float_decl(
            tag,
            false,
            &[
                (FLOAT, SemanticLocalRoleV1::Return),
                (FLOAT, SemanticLocalRoleV1::Argument(0)),
                (FLOAT, SemanticLocalRoleV1::Argument(1)),
            ],
            vec![block(
                tag + 1,
                vec![assignment(
                    0,
                    FLOAT,
                    SemanticRvalueKindV1::Binary {
                        operation,
                        left: value(1, FLOAT),
                        right: value(2, FLOAT),
                    },
                )],
                SemanticTerminatorKindV1::Return,
            )],
        )
    };
    InertSemanticMirRequestV1::new(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        float_types(bits),
        vec![],
        vec![],
        vec![],
        vec![
            root,
            helper(90, op),
            helper(120, SemanticBinaryOpV1::Multiply),
        ],
        vec![SemanticFunctionIdV1::from_index(0)],
    )
    .unwrap()
}

fn float_source(bits: u16, op: SemanticBinaryOpV1) -> ProductionPreRankedKirOwnerV1 {
    let admitted = float_request(bits, op)
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
    let source = ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), FLOOR);
    source
}

fn float_receipt(bits: u16, op: SemanticBinaryOpV1) -> ProductionMaterializedRankedModuleReceiptV1 {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let source = float_source(bits, op);
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
        "independent source global read; retained float calls have no ranked range summary"
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

fn assert_float_calls(module: &Module, bits: u16) {
    let root = module
        .functions
        .iter()
        .find(|f| f.id == module.kernels[0].entry)
        .unwrap();
    let calls: Vec<_> = root
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|b| &b.operations)
        .filter(|op| matches!(op.kind, OperationKind::Call { .. }) && !op.results.is_empty())
        .collect();
    assert_eq!(calls.len(), 2);
    for call in &calls {
        assert_eq!(call.results.len(), 1);
        assert_eq!(
            call.results[0].ty,
            if bits == 32 { Type::F32 } else { Type::F64 }
        );
    }
    let OperationKind::Call { arguments, .. } = &calls[1].kind else {
        unreachable!()
    };
    let first_result = calls[0].results[0].id;
    let mut transported = arguments[0];
    // The materializer may carry the returned value through exact block
    // parameters; this fixture is a finite single-predecessor call chain.
    let blocks = &root.body.as_ref().unwrap().blocks;
    for _ in 0..=blocks.len() {
        if transported == first_result {
            break;
        }
        let (block, ordinal) = blocks
            .iter()
            .find_map(|block| {
                block
                    .parameters
                    .iter()
                    .position(|p| p.id == transported)
                    .map(|ordinal| (block, ordinal))
            })
            .unwrap();
        let incoming: Vec<_> = blocks
            .iter()
            .filter_map(|predecessor| match &predecessor.terminator {
                Some(fe2o3_kernel_ir::Terminator::Branch { target, arguments })
                    if *target == block.id =>
                {
                    Some(arguments[ordinal])
                }
                _ => None,
            })
            .collect();
        assert_eq!(incoming.len(), 1);
        transported = incoming[0];
    }
    assert_eq!(transported, first_result);
    for function in &module.functions {
        if let Some(body) = &function.body {
            for operation in body.blocks.iter().flat_map(|b| &b.operations) {
                assert!(!matches!(operation.kind, OperationKind::Alloca { .. }));
                assert!(!matches!(operation.kind, OperationKind::Store { .. }));
                if function.role == fe2o3_kernel_ir::FunctionRole::InternalHelper {
                    assert!(!matches!(operation.kind, OperationKind::Load { .. }));
                }
            }
        }
    }
}

#[test]
fn general_policy3_strict_float_helpers_keep_exact_direct_result_transport() {
    for bits in [32, 64] {
        for op in [
            SemanticBinaryOpV1::Add,
            SemanticBinaryOpV1::Subtract,
            SemanticBinaryOpV1::Multiply,
        ] {
            with_prepared(
                prepare(float_receipt(bits, op), Profile::Gfx942, None),
                |input, budget| {
                    assert_float_calls(input.bound.module(), bits);
                    let floor = budget.storage();
                    let owner = AdmittedOutput::try_admit_general_v1(
                        input.receipt,
                        input.bound,
                        input.output,
                        budget,
                    )
                    .unwrap();
                    assert_float_calls(owner.output().module(), bits);
                    assert_eq!(owner.kernels()[0].accesses().len(), 1);
                    owner.verify_equivalence(budget).unwrap();
                    assert_eq!(budget.storage(), floor);
                    assert!(!owner.grants_artifact_or_launch_authority());
                },
            );
        }
    }
}

struct FloatPrepared4 {
    receipt: ProductionMaterializedRankedModuleReceiptV1,
    bound: VerifiedCanonicalKernelIrModuleV12,
    checked: fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy4V1,
    floor: usize,
}

fn float_prepare4(bits: u16, op: SemanticBinaryOpV1) -> FloatPrepared4 {
    let Prepared {
        receipt,
        bound,
        output,
        source_storage,
        bound_storage,
        ..
    } = prepare(float_receipt(bits, op), Profile::Gfx942, None);
    drop(output);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = FLOOR + source_storage + bound_storage;
    budget.reserve_storage(floor).unwrap();
    let checked =
        fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, &mut budget)
            .unwrap();
    assert!(checked.forwarding_rows().is_empty());
    assert_float_calls(checked.owner().module(), bits);
    assert_eq!(budget.storage(), floor);
    let floor = floor + checked.retained_storage();
    FloatPrepared4 {
        receipt,
        bound,
        checked,
        floor,
    }
}

#[test]
fn general_policy4_rechecks_actual_float_call_output_without_forwarding_permission() {
    for bits in [32, 64] {
        for op in [
            SemanticBinaryOpV1::Add,
            SemanticBinaryOpV1::Subtract,
            SemanticBinaryOpV1::Multiply,
        ] {
            let input = float_prepare4(bits, op);
            let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
            let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
            budget.reserve_storage(input.floor).unwrap();
            let owner = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget,
            )
            .unwrap();
            assert_float_calls(owner.output().module(), bits);
            owner.verify_equivalence(&mut budget).unwrap();
            assert_eq!(budget.storage(), input.floor);
            assert!(!owner.grants_artifact_or_launch_authority());
        }
    }
}

fn float_change_body(module: &mut Module) {
    let operation = module
        .functions
        .iter_mut()
        .filter(|f| f.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
        .filter_map(|f| f.body.as_mut())
        .flat_map(|body| &mut body.blocks)
        .flat_map(|b| &mut b.operations)
        .find(|op| {
            matches!(
                op.kind,
                OperationKind::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            )
        })
        .unwrap();
    let OperationKind::Binary { op, .. } = &mut operation.kind else {
        unreachable!()
    };
    assert_eq!(*op, BinaryOp::Add);
    *op = BinaryOp::Subtract;
}

fn float_change_result_input(module: &mut Module) {
    let entry = module.kernels[0].entry.clone();
    let root = module.functions.iter_mut().find(|f| f.id == entry).unwrap();
    let mut calls = root
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|b| &mut b.operations)
        .filter(|op| matches!(op.kind, OperationKind::Call { .. }) && !op.results.is_empty());
    let first = calls.next().unwrap();
    let OperationKind::Call {
        arguments: first_arguments,
        ..
    } = &first.kind
    else {
        unreachable!()
    };
    let replacement = first_arguments[0];
    let second = calls.next().unwrap();
    let OperationKind::Call { arguments, .. } = &mut second.kind else {
        unreachable!()
    };
    assert_ne!(arguments[0], replacement);
    arguments[0] = replacement;
}

fn float_swap_call_operands(module: &mut Module) {
    let entry = module.kernels[0].entry.clone();
    let root = module.functions.iter_mut().find(|f| f.id == entry).unwrap();
    let operation = root
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|b| &mut b.operations)
        .find(|op| matches!(op.kind, OperationKind::Call { .. }) && !op.results.is_empty())
        .unwrap();
    let OperationKind::Call { arguments, .. } = &mut operation.kind else {
        unreachable!()
    };
    assert_ne!(arguments[0], arguments[1]);
    arguments.swap(0, 1);
}

fn float_change_callee(module: &mut Module) {
    let entry = module.kernels[0].entry.clone();
    let root = module.functions.iter_mut().find(|f| f.id == entry).unwrap();
    let mut calls: Vec<_> = root
        .body
        .as_mut()
        .unwrap()
        .blocks
        .iter_mut()
        .flat_map(|b| &mut b.operations)
        .filter(|op| matches!(op.kind, OperationKind::Call { .. }) && !op.results.is_empty())
        .collect();
    let OperationKind::Call { callee: second, .. } = &calls[1].kind else {
        unreachable!()
    };
    let replacement = second.clone();
    let OperationKind::Call { callee, .. } = &mut calls[0].kind else {
        unreachable!()
    };
    assert_ne!(*callee, replacement);
    *callee = replacement;
}

#[test]
fn fresh_verified_float_graph_mutations_do_not_replace_source_correspondence() {
    for bits in [32, 64] {
        for mutation in [
            float_change_body as fn(&mut Module),
            float_change_result_input,
            float_swap_call_operands,
            float_change_callee,
        ] {
            with_prepared(
                prepare(
                    float_receipt(bits, SemanticBinaryOpV1::Add),
                    Profile::Gfx942,
                    Some(mutation),
                ),
                |input, budget| {
                    let floor = budget.storage();
                    let result = AdmittedOutput::try_admit_general_v1(
                        input.receipt,
                        input.bound,
                        input.output,
                        budget,
                    );
                    assert!(matches!(result, Err(AdmissionError::Coordinates(_))));
                    assert_eq!(budget.storage(), floor);
                },
            );
        }
    }
}

#[test]
fn float_checked_history_cannot_be_transplanted_between_source_bodies() {
    let source = float_prepare4(32, SemanticBinaryOpV1::Add);
    let changed = float_prepare4(32, SemanticBinaryOpV1::Subtract);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    let floor = source.floor + changed.floor;
    budget.reserve_storage(floor).unwrap();
    let result = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
        source.receipt,
        source.bound,
        changed.checked,
        &mut budget,
    );
    assert!(result.is_err());
    assert_eq!(budget.storage(), floor);
}

#[test]
fn admitted_source_float_division_is_not_a_raw_empty_policy3_recipe() {
    for bits in [32, 64] {
        // This first proves ordinary semantic MIR admission, not helper policy
        // admission. The general source census must still refuse division.
        assert!(
            float_request(bits, SemanticBinaryOpV1::Divide)
                .admit_current_production(SemanticMirLimitsV1::default())
                .is_ok()
        );
        with_prepared(
            prepare(
                float_receipt(bits, SemanticBinaryOpV1::Divide),
                Profile::Gfx942,
                None,
            ),
            |input, budget| {
                let floor = budget.storage();
                let result = AdmittedOutput::try_admit_general_v1(
                    input.receipt,
                    input.bound,
                    input.output,
                    budget,
                );
                assert!(matches!(
                    result,
                    Err(AdmissionError::Unsupported {
                        phase: "source",
                        detail: "total scalar/global recipe; no unchecked arithmetic",
                    })
                ));
                assert_eq!(budget.storage(), floor);
            },
        );
    }
}

#[test]
fn actual_float_policy4_admission_restores_floor_at_exact_and_one_short_resources() {
    let input = float_prepare4(64, SemanticBinaryOpV1::Multiply);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    budget.reserve_storage(input.floor).unwrap();
    drop(
        crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
            input.receipt,
            input.bound,
            input.checked,
            &mut budget,
        )
        .unwrap(),
    );
    let exact_work = budget.work();
    let exact_storage = budget.peak_storage();
    assert_eq!(budget.storage(), input.floor);
    for (work_limit, storage_limit, success) in [
        (exact_work, exact_storage, true),
        (exact_work - 1, exact_storage, false),
        (exact_work, exact_storage - 1, false),
    ] {
        let input = float_prepare4(64, SemanticBinaryOpV1::Multiply);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(input.floor).unwrap();
        {
            let result = crate::ProductionCheckedOutputOwnerPolicy4V1::try_admit_v1(
                input.receipt,
                input.bound,
                input.checked,
                &mut budget,
            );
            if success {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err());
            }
        }
        assert_eq!(budget.storage(), input.floor);
        if success {
            assert_eq!(budget.work(), exact_work);
            assert_eq!(budget.peak_storage(), exact_storage);
        } else if work_limit < exact_work {
            assert!(work.failed_work().is_some());
        } else {
            assert!(budget.failed_storage().is_some());
        }
    }
}
