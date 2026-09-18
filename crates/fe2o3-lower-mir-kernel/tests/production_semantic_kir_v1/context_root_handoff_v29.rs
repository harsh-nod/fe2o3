//! Inert root-handoff agreement tests, not source authentication or scope execution.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget, CanonicalKernelIrWorkBudgetV1,
};
use fe2o3_lower_mir_kernel::{
    ProductionContextCallBoundaryV29 as Boundary, ProductionContextRootErrorV29 as Error,
    ProductionContextRootInputV29 as Input, ProductionSourceLaunchInputV1,
    ProductionSourceLaunchRootInputV1, ProductionSourceLaunchRosterV1,
    with_checked_context_root_v29,
};
use fe2o3_pliron::{ProductionSemanticSsaLimitsV1, ProductionSemanticSsaOwnerV1};

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);
const WRAPPER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(4);
const HELPER: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(0);
const ROOT: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(1);
const ISSUER: SemanticCallableIdV1 = SemanticCallableIdV1::from_index(2);
const WORK: usize = 10_000;
const FLOOR: usize = 19;

#[derive(Clone, Copy, Debug)]
enum Transport {
    Copy,
    Move,
    ZeroSized,
    Projected,
    Bytes,
    Scalar,
    CopyContext,
}

fn zst(tag: u8, fields: Vec<SemanticTypeIdV1>) -> SemanticTypeDeclV1 {
    SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256(bytes(tag)),
        SemanticLayoutIdentityV1::from_sha256(bytes(tag)),
        SemanticTypeLayoutV1::aggregate(
            Some(0),
            1,
            SemanticAggregateLayoutV1::new(vec![0; fields.len()], vec![]).unwrap(),
        )
        .unwrap(),
        SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(fields).unwrap()),
    )
}

fn fixture(transport: Transport, binding: u8) -> ProductionSemanticSsaOwnerV1 {
    let source = SemanticSourceProvenanceV1::unavailable();
    let local = |tag, ty, role| {
        SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256(bytes(tag)),
            ty,
            role,
            source,
        )
    };
    let ignored = |ty| SemanticAbiValueV1::new(ty, SemanticAbiPassModeV1::Ignore);
    let abi = |tag, kernel, inputs: Vec<SemanticAbiValueV1>| {
        SemanticFunctionAbiV1::from_rustc(
            SemanticAbiIdentityV1::from_sha256(bytes(tag)),
            SemanticLayoutIdentityV1::from_sha256(bytes(250)),
            if kernel {
                SemanticCanonAbiV1::GpuKernel
            } else {
                SemanticCanonAbiV1::Rust
            },
            if kernel {
                SemanticExternAbiV1::GpuKernel
            } else {
                SemanticExternAbiV1::Rust
            },
            false,
            false,
            inputs.len() as u32,
            inputs
                .into_iter()
                .map(SemanticAbiArgumentV1::source)
                .collect(),
            ignored(UNIT),
        )
        .unwrap()
    };
    let function = |tag, role, abi, locals, blocks| {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256(bytes(tag)),
            role,
            SemanticItemDefinitionIdentityV1::from_sha256(bytes(tag)),
            SemanticMonomorphizationIdentityV1::from_sha256(bytes(tag)),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256(bytes(tag)),
            SemanticConstGenericArgumentsIdentityV1::from_sha256(bytes(tag)),
            source,
            abi,
            locals,
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    };
    let call = |callee, arguments, destination, target| {
        SemanticTerminatorKindV1::Call(
            SemanticDirectCallV1::new_callable(
                callee,
                arguments,
                Some(SemanticCallDestinationV1::new(
                    destination,
                    SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::CallReturn,
                        SemanticBlockIdV1::from_index(target),
                    ),
                )),
                SemanticUnwindActionV1::Unreachable,
            )
            .unwrap(),
        )
    };
    let marker_operand = match transport {
        Transport::Move => SemanticOperandV1::Move(local_place(3, MARKER)),
        Transport::ZeroSized => SemanticOperandV1::Constant(SemanticConstantV1::new(
            MARKER,
            SemanticConstantValueV1::ZeroSized,
        )),
        Transport::Projected => SemanticOperandV1::Copy(
            SemanticPlaceV1::new(
                SemanticLocalIdV1::from_index(3),
                vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(0), UNIT).unwrap()],
                UNIT,
            )
            .unwrap(),
        ),
        Transport::Bytes => SemanticOperandV1::Constant(SemanticConstantV1::new(
            MARKER,
            SemanticConstantValueV1::Bytes(SemanticConstantBytesV1::new(vec![]).unwrap()),
        )),
        _ => SemanticOperandV1::Copy(local_place(3, MARKER)),
    };
    let last_type = marker_operand.ty();
    let root_marker_type = if matches!(transport, Transport::Projected) {
        WRAPPER
    } else {
        MARKER
    };
    let context_operand = if matches!(transport, Transport::CopyContext) {
        SemanticOperandV1::Copy(local_place(4, CONTEXT))
    } else {
        SemanticOperandV1::Move(local_place(4, CONTEXT))
    };
    let first_scalar = if matches!(transport, Transport::Scalar) {
        scalar_constant(U32, 7, 4)
    } else {
        SemanticOperandV1::Copy(local_place(1, U32))
    };
    let root = function(
        60,
        SemanticFunctionRoleV1::KernelRoot,
        abi(
            60,
            true,
            vec![
                direct_abi_value(U32),
                direct_abi_value(U32),
                ignored(root_marker_type),
            ],
        ),
        vec![
            local(61, UNIT, SemanticLocalRoleV1::Return),
            local(62, U32, SemanticLocalRoleV1::Argument(0)),
            local(63, U32, SemanticLocalRoleV1::Argument(1)),
            local(64, root_marker_type, SemanticLocalRoleV1::Argument(2)),
            local(65, CONTEXT, SemanticLocalRoleV1::Temporary),
        ],
        vec![
            block(80, vec![], call(ISSUER, vec![], local_place(4, CONTEXT), 1)),
            block(
                81,
                vec![],
                call(
                    SemanticCallableIdV1::from_index(0),
                    vec![
                        context_operand,
                        first_scalar,
                        SemanticOperandV1::Copy(local_place(2, U32)),
                        marker_operand,
                    ],
                    local_place(0, UNIT),
                    2,
                ),
            ),
            block(82, vec![], SemanticTerminatorKindV1::Return),
        ],
    )
    .with_kernel_entry(SemanticKernelEntryV1::new(
        SemanticLinkSymbolV1::new(b"context_root_handoff".to_vec()).unwrap(),
        SemanticKernelBindingIdentityV1::from_sha256(bytes(binding)),
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
    let helper = function(
        70,
        SemanticFunctionRoleV1::InternalHelper,
        abi(
            70,
            false,
            vec![
                ignored(CONTEXT),
                direct_abi_value(U32),
                direct_abi_value(U32),
                ignored(last_type),
            ],
        ),
        vec![
            local(71, UNIT, SemanticLocalRoleV1::Return),
            local(72, CONTEXT, SemanticLocalRoleV1::Argument(0)),
            local(73, U32, SemanticLocalRoleV1::Argument(1)),
            local(74, U32, SemanticLocalRoleV1::Argument(2)),
            local(75, last_type, SemanticLocalRoleV1::Argument(3)),
        ],
        vec![block(83, vec![], SemanticTerminatorKindV1::Return)],
    );
    let admitted = InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(bytes(250))),
        vec![
            unit_type(),
            zst(10, vec![]),
            zst(11, vec![MARKER; 5]).with_rust_type_kind(SemanticRustTypeKindV1::Execution(
                SemanticExecutionRoleV29::KernelContext,
            )),
            scalar_type(
                12,
                SemanticScalarTypeV1::Integer {
                    signed: false,
                    bits: 32,
                },
            ),
            zst(13, vec![UNIT]),
        ],
        vec![],
        vec![],
        vec![],
        vec![helper, root],
        vec![
            SemanticCallableDeclV1::defined(HELPER),
            SemanticCallableDeclV1::defined(ROOT),
            compiler_intrinsic_callable(
                90,
                vec![],
                ignored(CONTEXT),
                SemanticCompilerIntrinsicOperationV1::Execution(
                    SemanticExecutionOperationV29::ContextIssue { context: CONTEXT },
                ),
            ),
        ],
        vec![ROOT],
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap();
    let owner =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default()).unwrap()
}

fn launch(ssa: &ProductionSemanticSsaOwnerV1) -> ProductionSourceLaunchRosterV1 {
    let semantic = ssa.source_semantic();
    ProductionSourceLaunchRosterV1::try_new(
        semantic,
        &[ProductionSourceLaunchRootInputV1::new(
            "logical_context",
            *semantic.functions()[1]
                .kernel_entry()
                .unwrap()
                .kernel_binding_identity()
                .as_bytes(),
            ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
        )],
    )
    .unwrap()
}

fn input(ssa: &ProductionSemanticSsaOwnerV1) -> Input<'_> {
    let semantic = ssa.source_semantic();
    let SemanticTerminatorKindV1::Call(helper_call) =
        semantic.functions()[1].blocks()[1].terminator().kind()
    else {
        panic!("fixture helper call")
    };
    Input {
        semantic_sha256: ssa.source_semantic_sha256(),
        root: ROOT,
        root_identity: SemanticFunctionIdentityV1::from_sha256(bytes(60)),
        helper: HELPER,
        helper_identity: SemanticFunctionIdentityV1::from_sha256(bytes(70)),
        issuer: ISSUER,
        issuer_identity: SemanticFunctionIdentityV1::from_sha256(bytes(90)),
        context_type: CONTEXT,
        context_identity: SemanticTypeIdentityV1::from_sha256(bytes(11)),
        issuance: Boundary {
            block: SemanticBlockIdV1::from_index(0),
            statement_count: 0,
            destination: SemanticLocalIdV1::from_index(4),
            destination_type: CONTEXT,
            target: SemanticBlockIdV1::from_index(1),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
        helper_call: Boundary {
            block: SemanticBlockIdV1::from_index(1),
            statement_count: 0,
            destination: SemanticLocalIdV1::from_index(0),
            destination_type: UNIT,
            target: SemanticBlockIdV1::from_index(2),
            unwind: SemanticUnwindActionV1::Unreachable,
        },
        helper_context_local: SemanticLocalIdV1::from_index(4),
        helper_arguments: helper_call.arguments(),
    }
}

fn rejects(
    ssa: &ProductionSemanticSsaOwnerV1,
    roster: &ProductionSourceLaunchRosterV1,
    input: Input<'_>,
    expected: Error,
) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = Budget::new(&mut work, FLOOR);
    budget.reserve_storage(FLOOR).unwrap();
    let mut invoked = false;
    let result = with_checked_context_root_v29(ssa, roster, input, &mut budget, |_, _| {
        invoked = true;
        Ok(())
    });
    assert_eq!(result, Err(expected));
    assert!(!invoked);
    assert_eq!(budget.storage(), FLOOR);
    assert!(budget.failed_storage().is_none());
}

#[test]
fn exact_v29_handoff_borrows_source_and_retains_defined_callable_identity() {
    for transport in [Transport::Copy, Transport::Move, Transport::ZeroSized] {
        let ssa = fixture(transport, 100);
        let roster = launch(&ssa);
        let detached = input(&ssa);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        let answer =
            with_checked_context_root_v29(&ssa, &roster, detached, &mut budget, |checked, _| {
                assert_eq!(checked.root_id(), ROOT);
                assert_eq!(checked.helper_id(), HELPER);
                assert_eq!(checked.context_type(), CONTEXT);
                assert_eq!(checked.helper_call().callee().index(), 0);
                assert_eq!(
                    checked.helper_call().callee().index(),
                    checked.helper_id().index()
                );
                assert!(std::ptr::eq(checked.semantic_ssa(), &ssa));
                assert!(std::ptr::eq(
                    checked.root(),
                    &ssa.source_semantic().functions()[1]
                ));
                assert!(std::ptr::eq(
                    checked.helper(),
                    &ssa.source_semantic().functions()[0]
                ));
                assert!(std::ptr::eq(
                    checked.root_plan(),
                    ssa.plan_for_function(ROOT).unwrap()
                ));
                assert!(std::ptr::eq(
                    checked.helper_plan(),
                    ssa.plan_for_function(HELPER).unwrap()
                ));
                assert!(std::ptr::eq(checked.launch(), &roster.roots()[0]));
                assert!(std::ptr::eq(
                    checked.helper_call().arguments(),
                    detached.helper_arguments
                ));
                assert_eq!(checked.issuance().callee(), ISSUER);
                assert_eq!(checked.issuance_boundary(), detached.issuance);
                assert_eq!(checked.helper_call_boundary(), detached.helper_call);
                assert!(!checked.semantic_ssa().grants_proof_or_artifact_authority());
                Ok(7_u32)
            })
            .unwrap();
        assert_eq!(answer, 7);
        assert_eq!(budget.storage(), FLOOR);
        assert!(!roster.grants_artifact_or_launch_authority());
        assert!(matches!(
            ProductionSemanticKirOwnerV1::try_lower(
                ssa.into_source_owner().unwrap(),
                ProductionSemanticKirLimitsV1::default(),
            ),
            Err(ProductionSemanticKirErrorV1::Unsupported { detail, .. })
                if detail == "execution capabilities require checked canonical KIR materialization"
        ));
    }
}

#[test]
fn permuted_defined_callable_prefix_is_rejected_before_handoff() {
    let ssa = fixture(Transport::Copy, 100);
    let semantic = ssa.source_semantic();
    let mut callables = semantic.callables().to_vec();
    callables.swap(0, 1);
    let request = InertSemanticMirRequestV1::new_with_callables(
        semantic.target(),
        semantic.types().to_vec(),
        semantic.allocations().to_vec(),
        semantic.statics().to_vec(),
        semantic.vtables().to_vec(),
        semantic.functions().to_vec(),
        callables,
        semantic.roots().to_vec(),
    )
    .unwrap();
    assert_eq!(
        request
            .admit_exact_v29(SemanticMirLimitsV1::default())
            .unwrap_err(),
        SemanticMirErrorV1::InvalidFunctionAbi,
    );
}

#[test]
fn detached_identity_and_argument_substitutions_never_reach_the_callback() {
    let ssa = fixture(Transport::Copy, 100);
    let roster = launch(&ssa);
    let exact = input(&ssa);
    let wrong = bytes(255);
    let cases = [
        (
            Input {
                semantic_sha256: &wrong,
                ..exact
            },
            Error::Source,
        ),
        (
            Input {
                root: SemanticFunctionIdV1::from_index(u32::MAX),
                ..exact
            },
            Error::Root,
        ),
        (
            Input {
                root: HELPER,
                ..exact
            },
            Error::Root,
        ),
        (
            Input {
                root_identity: SemanticFunctionIdentityV1::from_sha256(wrong),
                ..exact
            },
            Error::Root,
        ),
        (
            Input {
                helper: SemanticFunctionIdV1::from_index(u32::MAX),
                ..exact
            },
            Error::Helper,
        ),
        (
            Input {
                helper: ROOT,
                ..exact
            },
            Error::Helper,
        ),
        (
            Input {
                helper_identity: SemanticFunctionIdentityV1::from_sha256(wrong),
                ..exact
            },
            Error::Helper,
        ),
        (
            Input {
                issuer: SemanticCallableIdV1::from_index(u32::MAX),
                ..exact
            },
            Error::Issuer,
        ),
        (
            Input {
                issuer: SemanticCallableIdV1::from_index(0),
                ..exact
            },
            Error::Issuer,
        ),
        (
            Input {
                issuer_identity: SemanticFunctionIdentityV1::from_sha256(wrong),
                ..exact
            },
            Error::Issuer,
        ),
        (
            Input {
                context_type: SemanticTypeIdV1::from_index(u32::MAX),
                ..exact
            },
            Error::ContextType,
        ),
        (
            Input {
                context_type: MARKER,
                ..exact
            },
            Error::ContextType,
        ),
        (
            Input {
                context_identity: SemanticTypeIdentityV1::from_sha256(wrong),
                ..exact
            },
            Error::ContextType,
        ),
        (
            Input {
                helper_context_local: SemanticLocalIdV1::from_index(3),
                ..exact
            },
            Error::Arguments,
        ),
        (
            Input {
                helper_arguments: &[],
                ..exact
            },
            Error::Arguments,
        ),
    ];
    for (changed, error) in cases {
        rejects(&ssa, &roster, changed, error);
    }
    let mut arguments = exact.helper_arguments.to_vec();
    arguments.swap(1, 2);
    rejects(
        &ssa,
        &roster,
        Input {
            helper_arguments: &arguments,
            ..exact
        },
        Error::Arguments,
    );
    arguments = exact.helper_arguments.to_vec();
    arguments[0] = SemanticOperandV1::Copy(local_place(4, CONTEXT));
    rejects(
        &ssa,
        &roster,
        Input {
            helper_arguments: &arguments,
            ..exact
        },
        Error::Arguments,
    );
    arguments = exact.helper_arguments.to_vec();
    arguments[1] = SemanticOperandV1::Move(local_place(1, U32));
    rejects(
        &ssa,
        &roster,
        Input {
            helper_arguments: &arguments,
            ..exact
        },
        Error::Arguments,
    );
    arguments = exact.helper_arguments.to_vec();
    arguments.push(SemanticOperandV1::Copy(local_place(3, MARKER)));
    rejects(
        &ssa,
        &roster,
        Input {
            helper_arguments: &arguments,
            ..exact
        },
        Error::Arguments,
    );

    let other = fixture(Transport::Copy, 101);
    rejects(&ssa, &launch(&other), exact, Error::Launch);
    let old = ProductionSemanticSsaOwnerV1::try_new(
        checked_arithmetic_owner(),
        ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    rejects(
        &old,
        &roster,
        Input {
            semantic_sha256: old.source_semantic_sha256(),
            ..exact
        },
        Error::Source,
    );
}

#[test]
fn every_call_boundary_anchor_is_checked_before_visiting() {
    let ssa = fixture(Transport::Copy, 100);
    let roster = launch(&ssa);
    let exact = input(&ssa);
    for issuance in [true, false] {
        let original = if issuance {
            exact.issuance
        } else {
            exact.helper_call
        };
        for changed in [
            Boundary {
                block: SemanticBlockIdV1::from_index(u32::MAX),
                ..original
            },
            Boundary {
                block: SemanticBlockIdV1::from_index(2),
                ..original
            },
            Boundary {
                statement_count: 1,
                ..original
            },
            Boundary {
                destination: SemanticLocalIdV1::from_index(1),
                ..original
            },
            Boundary {
                destination_type: U32,
                ..original
            },
            Boundary {
                target: SemanticBlockIdV1::from_index(0),
                ..original
            },
            Boundary {
                unwind: SemanticUnwindActionV1::Continue,
                ..original
            },
        ] {
            let changed = if issuance {
                Input {
                    issuance: changed,
                    ..exact
                }
            } else {
                Input {
                    helper_call: changed,
                    ..exact
                }
            };
            rejects(&ssa, &roster, changed, Error::CallBoundary);
        }
    }
}

#[test]
fn matching_inert_source_does_not_admit_other_root_operand_grammars() {
    for transport in [
        Transport::Projected,
        Transport::Bytes,
        Transport::Scalar,
        Transport::CopyContext,
    ] {
        let ssa = fixture(transport, 100);
        rejects(&ssa, &launch(&ssa), input(&ssa), Error::Arguments);
    }
}

#[test]
fn work_budget_is_exact_cumulative_and_preserves_the_storage_floor() {
    let ssa = fixture(Transport::Copy, 100);
    let roster = launch(&ssa);
    let measure = {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        with_checked_context_root_v29(&ssa, &roster, input(&ssa), &mut budget, |_, _| Ok(()))
            .unwrap();
        assert_eq!(budget.storage(), FLOOR);
        budget.work()
    };
    assert!(measure > 0);
    for available in [0, measure - 1, measure] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(available + 3);
        let mut budget = Budget::new(&mut work, FLOOR);
        budget.reserve_storage(FLOOR).unwrap();
        budget.charge_work(3).unwrap();
        let mut invoked = false;
        let result =
            with_checked_context_root_v29(&ssa, &roster, input(&ssa), &mut budget, |_, _| {
                invoked = true;
                Ok(())
            });
        if available == measure {
            assert_eq!(result, Ok(()));
            assert!(invoked);
            assert_eq!(budget.work(), 3 + measure);
        } else {
            assert!(matches!(result, Err(Error::Resource(_))));
            assert!(!invoked);
        }
        assert_eq!(budget.storage(), FLOOR);
        assert!(budget.failed_storage().is_none());
    }
    for fail in [false, true] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(measure + 5);
        let mut budget = Budget::new(&mut work, FLOOR + 7);
        budget.reserve_storage(FLOOR).unwrap();
        let result =
            with_checked_context_root_v29(&ssa, &roster, input(&ssa), &mut budget, |_, shared| {
                shared.charge_work(5)?;
                shared.reserve_storage(7)?;
                if fail { Err(Error::Arguments) } else { Ok(()) }
            });
        assert_eq!(result, if fail { Err(Error::Arguments) } else { Ok(()) });
        assert_eq!(budget.work(), measure + 5);
        assert_eq!(budget.storage(), FLOOR + 7);
    }
}
