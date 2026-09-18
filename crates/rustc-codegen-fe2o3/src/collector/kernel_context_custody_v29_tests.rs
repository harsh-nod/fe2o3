//! Private custody fixtures, not authenticated Rust providers or executable proofs.
use super::*;

#[path = "kernel_context_custody_v29_tests/scope_tests.rs"]
mod scope_tests;

const UNIT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(0);
const U32: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(1);
const MARKER: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(2);
const CONTEXT: SemanticTypeIdV1 = SemanticTypeIdV1::from_index(3);

fn place(local: u32, ty: SemanticTypeIdV1) -> SemanticPlaceV1 {
    SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap()
}

fn unit_return() -> SemanticStatementV1 {
    SemanticStatementV1::new(
        SemanticSourceProvenanceV1::unavailable(),
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(0, UNIT),
            SemanticRvalueV1::new(
                UNIT,
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(SemanticConstantV1::new(
                    UNIT,
                    SemanticConstantValueV1::ZeroSized,
                ))),
            ),
        )),
    )
}

fn abi(
    tag: u8,
    kernel: bool,
    inputs: &[SemanticTypeIdV1],
    output: SemanticTypeIdV1,
) -> SemanticFunctionAbiV1 {
    let value = |ty| {
        SemanticAbiValueV1::new(
            ty,
            if ty == U32 {
                SemanticAbiPassModeV1::Direct(
                    SemanticAbiValueAttributesV1::new(
                        SemanticAbiRegularAttributesV1::new(false, None, false, false, false, true),
                        SemanticAbiExtensionV1::None,
                        0,
                        None,
                    )
                    .unwrap(),
                )
            } else {
                SemanticAbiPassModeV1::Ignore
            },
        )
    };
    SemanticFunctionAbiV1::from_rustc(
        SemanticAbiIdentityV1::from_sha256([tag; 32]),
        SemanticLayoutIdentityV1::from_sha256([250; 32]),
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
            .iter()
            .copied()
            .map(|ty| SemanticAbiArgumentV1::source(value(ty)))
            .collect(),
        value(output),
    )
    .unwrap()
    .with_source_argument_ownership(vec![
        SemanticSourceArgumentOwnershipV1::ByValue;
        inputs.len()
    ])
    .unwrap()
}

fn block(
    tag: u8,
    statements: Vec<SemanticStatementV1>,
    kind: SemanticTerminatorKindV1,
) -> SemanticBasicBlockV1 {
    SemanticBasicBlockV1::new(
        SemanticBlockIdentityV1::from_sha256([tag; 32]),
        SemanticSourceProvenanceV1::unavailable(),
        statements,
        SemanticTerminatorV1::new(SemanticSourceProvenanceV1::unavailable(), kind),
    )
    .unwrap()
}

fn call(
    callee: u32,
    arguments: Vec<SemanticOperandV1>,
    destination: u32,
    ty: SemanticTypeIdV1,
    target: u32,
) -> SemanticTerminatorKindV1 {
    SemanticTerminatorKindV1::Call(
        SemanticDirectCallV1::new_callable(
            SemanticCallableIdV1::from_index(callee),
            arguments,
            Some(SemanticCallDestinationV1::new(
                place(destination, ty),
                SemanticControlFlowEdgeV1::new(
                    SemanticEdgeRoleV1::CallReturn,
                    SemanticBlockIdV1::from_index(target),
                ),
            )),
            SemanticUnwindActionV1::Unreachable,
        )
        .unwrap(),
    )
}

#[derive(Clone, Copy)]
enum Mutation {
    None,
    Arguments,
    ArgumentKind,
    Issuer,
    ExtraIssue,
    HelperIdentity,
    ContextIdentity,
    LayoutIdentity,
}

fn fixture(mutation: Mutation) -> AdmittedInertSemanticMirV1 {
    fixture_roots(mutation, 1)
}

fn fixture_roots(mutation: Mutation, roots: u8) -> AdmittedInertSemanticMirV1 {
    assert!((1..=8).contains(&roots));
    let aggregate_layout = |fields| {
        SemanticTypeLayoutV1::aggregate_with_backend_repr(
            Some(0),
            1,
            SemanticBackendReprV1::memory(true),
            false,
            SemanticAggregateLayoutV1::new(vec![0; fields], vec![]).unwrap(),
        )
        .unwrap()
    };
    let declaration = |tag, layout, shape| {
        SemanticTypeDeclV1::new(
            SemanticTypeIdentityV1::from_sha256([tag; 32]),
            SemanticLayoutIdentityV1::from_sha256(
                [if tag == 1 && matches!(mutation, Mutation::LayoutIdentity) {
                    99
                } else {
                    tag
                }; 32],
            ),
            layout,
            shape,
        )
    };
    let types = vec![
        declaration(
            1,
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
        declaration(
            2,
            SemanticTypeLayoutV1::new_with_backend_repr(
                Some(4),
                4,
                SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                    SemanticBackendPrimitiveV1::integer(false, 32, 4),
                    SemanticScalarValidityRangeV1::new(0, u32::MAX.into()),
                )),
                false,
            )
            .unwrap(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32,
            }),
        ),
        declaration(
            3,
            aggregate_layout(0),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![]).unwrap()),
        ),
        declaration(
            if matches!(mutation, Mutation::ContextIdentity) {
                5
            } else {
                4
            },
            aggregate_layout(5),
            SemanticTypeShapeV1::Aggregate(SemanticAggregateTypeV1::new(vec![MARKER; 5]).unwrap()),
        )
        .with_rust_type_kind(SemanticRustTypeKindV1::Execution(
            SemanticExecutionRoleV29::KernelContext,
        )),
    ];
    let function = |tag,
                    role,
                    signature: SemanticFunctionAbiV1,
                    locals: &[(SemanticTypeIdV1, SemanticLocalRoleV1)],
                    blocks| {
        SemanticFunctionDeclV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            role,
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            signature,
            locals
                .iter()
                .enumerate()
                .map(|(index, (ty, role))| {
                    SemanticLocalDeclV1::new(
                        SemanticLocalIdentityV1::from_sha256([index as u8 + 1; 32]),
                        *ty,
                        *role,
                        SemanticSourceProvenanceV1::unavailable(),
                    )
                })
                .collect(),
            SemanticBlockIdV1::from_index(0),
            blocks,
        )
        .unwrap()
    };
    let mut arguments = vec![
        SemanticOperandV1::Move(place(3, CONTEXT)),
        SemanticOperandV1::Copy(place(1, U32)),
        SemanticOperandV1::Copy(place(2, U32)),
    ];
    if matches!(mutation, Mutation::Arguments) {
        arguments.swap(1, 2);
    }
    if matches!(mutation, Mutation::ArgumentKind) {
        arguments[1] = SemanticOperandV1::Move(place(1, U32));
    }
    let mut blocks = vec![
        block(
            10,
            vec![],
            call(u32::from(roots) + 1, vec![], 3, CONTEXT, 1),
        ),
        block(11, vec![], call(u32::from(roots), arguments, 4, UNIT, 2)),
        block(12, vec![unit_return()], SemanticTerminatorKindV1::Return),
    ];
    if matches!(mutation, Mutation::ExtraIssue) {
        blocks[2] = block(
            12,
            vec![],
            call(u32::from(roots) + 1, vec![], 3, CONTEXT, 3),
        );
        blocks.push(block(
            13,
            vec![unit_return()],
            SemanticTerminatorKindV1::Return,
        ));
    }
    let mut functions = Vec::new();
    for index in 0..roots {
        let root = function(
            40 + index,
            SemanticFunctionRoleV1::KernelRoot,
            abi(40 + index, true, &[U32, U32], UNIT),
            &[
                (UNIT, SemanticLocalRoleV1::Return),
                (U32, SemanticLocalRoleV1::Argument(0)),
                (U32, SemanticLocalRoleV1::Argument(1)),
                (CONTEXT, SemanticLocalRoleV1::Temporary),
                (UNIT, SemanticLocalRoleV1::Temporary),
            ],
            blocks.clone(),
        )
        .with_kernel_entry(SemanticKernelEntryV1::new(
            SemanticLinkSymbolV1::new(format!("custody_root_{index}").into_bytes()).unwrap(),
            SemanticKernelBindingIdentityV1::from_sha256([60 + index; 32]),
            SemanticKernelSourceContractV1::new(
                Some(
                    SemanticKernelLaunchBoundsV1::new(
                        Some(SemanticWorkgroupDimensionsV1::new([64, 1, 1]).unwrap()),
                        None,
                        None,
                    )
                    .unwrap(),
                ),
                None,
                None,
            )
            .unwrap(),
        ));
        functions.push(root);
    }
    let helper = function(
        if matches!(mutation, Mutation::HelperIdentity) {
            51
        } else {
            50
        },
        SemanticFunctionRoleV1::InternalHelper,
        abi(50, false, &[CONTEXT, U32, U32], UNIT),
        &[
            (UNIT, SemanticLocalRoleV1::Return),
            (CONTEXT, SemanticLocalRoleV1::Argument(0)),
            (U32, SemanticLocalRoleV1::Argument(1)),
            (U32, SemanticLocalRoleV1::Argument(2)),
        ],
        vec![block(
            20,
            vec![unit_return()],
            SemanticTerminatorKindV1::Return,
        )],
    );
    functions.push(helper);
    let mut callables = (0..=u32::from(roots))
        .map(|id| SemanticCallableDeclV1::defined(SemanticFunctionIdV1::from_index(id)))
        .collect::<Vec<_>>();
    let tag = if matches!(mutation, Mutation::Issuer) {
        71
    } else {
        70
    };
    callables.push(SemanticCallableDeclV1::CompilerIntrinsic {
        binding: SemanticNonBodyCallableBindingV1::new(
            SemanticFunctionIdentityV1::from_sha256([tag; 32]),
            SemanticItemDefinitionIdentityV1::from_sha256([tag; 32]),
            SemanticMonomorphizationIdentityV1::from_sha256([tag; 32]),
            SemanticGenericTypeArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticConstGenericArgumentsIdentityV1::from_sha256([tag; 32]),
            SemanticSourceProvenanceV1::unavailable(),
            abi(tag, false, &[], CONTEXT),
        ),
        operation: SemanticCompilerIntrinsicOperationV1::Execution(
            SemanticExecutionOperationV29::ContextIssue { context: CONTEXT },
        ),
        operation_identity: SemanticCompilerIntrinsicIdentityV1::from_sha256([tag; 32]),
    });
    InertSemanticMirRequestV1::new_with_callables(
        SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
        types,
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        (0..u32::from(roots))
            .map(SemanticFunctionIdV1::from_index)
            .collect(),
    )
    .unwrap()
    .admit_exact_v29(SemanticMirLimitsV1::default())
    .unwrap()
}

fn completed() -> CompletedContextEntryV29 {
    let boundary = |block, destination, target| CallBoundaryV29 {
        block: SemanticBlockIdV1::from_index(block),
        statements: 0,
        destination: SemanticLocalIdV1::from_index(destination),
        destination_type: if destination == 3 { CONTEXT } else { UNIT },
        target: SemanticBlockIdV1::from_index(target),
        unwind: SemanticUnwindActionV1::Unreachable,
    };
    CompletedContextEntryV29 {
        function: SemanticFunctionIdV1::from_index(0),
        helper: SemanticFunctionIdV1::from_index(1),
        issuer: SemanticCallableIdV1::from_index(2),
        issuance: boundary(0, 3, 1),
        helper_call: boundary(1, 4, 2),
        helper_argument: SemanticLocalIdV1::from_index(3),
        arguments: vec![
            SemanticOperandV1::Move(place(3, CONTEXT)),
            SemanticOperandV1::Copy(place(1, U32)),
            SemanticOperandV1::Copy(place(2, U32)),
        ],
        root_identity: SemanticFunctionIdentityV1::from_sha256([40; 32]),
        helper_identity: SemanticFunctionIdentityV1::from_sha256([50; 32]),
        issuer_identity: SemanticFunctionIdentityV1::from_sha256([70; 32]),
        context_identity: SemanticTypeIdentityV1::from_sha256([4; 32]),
        _source_commitment: [90; 32],
    }
}

fn retained() -> RetainedContextEntryV29 {
    completed()
        .bind_function(&fixture(Mutation::None).functions()[0], |_| Ok(()))
        .unwrap()
}

fn sealed_roots(semantic: &AdmittedInertSemanticMirV1) -> RetainedContextEntriesV29 {
    let roots = semantic.roots().len() as u32;
    let entries = (0..roots)
        .map(|root| {
            let mut entry = completed();
            entry.function = SemanticFunctionIdV1::from_index(root);
            entry.root_identity = semantic.functions()[root as usize].identity();
            entry.helper = SemanticFunctionIdV1::from_index(roots);
            entry.issuer = SemanticCallableIdV1::from_index(roots + 1);
            entry
                .bind_function(&semantic.functions()[root as usize], |_| Ok(()))
                .unwrap()
        })
        .collect();
    RetainedContextEntriesV29::seal(entries, semantic, |_| Ok(())).unwrap()
}

fn ssa_owner(semantic: AdmittedInertSemanticMirV1) -> fe2o3_pliron::ProductionSemanticSsaOwnerV1 {
    fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
        fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
            semantic,
            fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
        )
        .unwrap(),
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap()
}

fn launch_roster(
    semantic: &AdmittedInertSemanticMirV1,
) -> fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1 {
    use fe2o3_lower_mir_kernel::{
        ProductionSourceLaunchInputV1, ProductionSourceLaunchRootInputV1,
    };
    let names = (0..semantic.roots().len())
        .map(|i| format!("root_{i}"))
        .collect::<Vec<_>>();
    let inputs = semantic
        .roots()
        .iter()
        .zip(&names)
        .map(|(root, name)| {
            ProductionSourceLaunchRootInputV1::new(
                name,
                *semantic.functions()[root.index() as usize]
                    .kernel_entry()
                    .unwrap()
                    .kernel_binding_identity()
                    .as_bytes(),
                ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [2, 1, 1]),
            )
        })
        .collect::<Vec<_>>();
    fe2o3_lower_mir_kernel::ProductionSourceLaunchRosterV1::try_new(semantic, &inputs).unwrap()
}

#[test]
fn borrowed_context_anchors_keep_exact_metadata_and_canonical_order() {
    for count in [1, 4] {
        let semantic = fixture_roots(Mutation::None, count);
        let entries = sealed_roots(&semantic);
        let owner = ssa_owner(semantic);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = VisitBudget::new(&mut work, 7);
        budget.reserve_storage(7).unwrap();
        let mut roots = Vec::new();
        entries
            .visit_root_anchors_v29(owner.source_semantic(), &mut budget, |entry, budget| {
                budget.charge_work(entry.helper_operands().len())?;
                roots.push(entry.root().0);
                assert_eq!(
                    entry.root().1,
                    owner.source_semantic().functions()[entry.function().index() as usize]
                        .identity()
                );
                assert_eq!(
                    entry.helper(),
                    (
                        SemanticFunctionIdV1::from_index(u32::from(count)),
                        SemanticFunctionIdentityV1::from_sha256([50; 32])
                    )
                );
                assert_eq!(
                    entry.issuer(),
                    (
                        SemanticCallableIdV1::from_index(u32::from(count) + 1),
                        SemanticFunctionIdentityV1::from_sha256([70; 32])
                    )
                );
                assert_eq!(
                    entry.context(),
                    (CONTEXT, SemanticTypeIdentityV1::from_sha256([4; 32]))
                );
                assert_eq!(
                    entry.issuance().location(),
                    (SemanticBlockIdV1::from_index(0), 0)
                );
                assert_eq!(
                    entry.issuance().destination(),
                    (SemanticLocalIdV1::from_index(3), CONTEXT)
                );
                assert_eq!(
                    entry.issuance().continuation(),
                    SemanticBlockIdV1::from_index(1)
                );
                assert_eq!(
                    entry.issuance().unwind(),
                    SemanticUnwindActionV1::Unreachable
                );
                assert_eq!(
                    entry.helper_call().location(),
                    (SemanticBlockIdV1::from_index(1), 0)
                );
                assert_eq!(
                    entry.helper_call().destination(),
                    (SemanticLocalIdV1::from_index(4), UNIT)
                );
                assert_eq!(
                    entry.helper_call().continuation(),
                    SemanticBlockIdV1::from_index(2)
                );
                assert_eq!(
                    entry.helper_call().unwind(),
                    SemanticUnwindActionV1::Unreachable
                );
                assert_eq!(entry.helper_argument(), SemanticLocalIdV1::from_index(3));
                assert_eq!(entry.helper_operands(), completed().arguments);
                Ok::<_, VisitResource>(())
            })
            .unwrap();
        assert_eq!(roots, owner.source_semantic().roots());
        assert_eq!(budget.storage(), 7);
        assert_eq!(work.work(), 1 + usize::from(count) * 4);
    }
}

#[test]
fn borrowed_context_visits_prepay_enumeration_and_share_consumer_budget() {
    for count in [1, 4] {
        let semantic = fixture_roots(Mutation::None, count);
        let entries = sealed_roots(&semantic);
        let exact = 1 + usize::from(count);
        for limit in [0, exact - 1, exact] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = VisitBudget::new(&mut work, 0);
            let mut visits = 0;
            let result = entries.visit_root_anchors_v29(&semantic, &mut budget, |_, _| {
                visits += 1;
                Ok::<_, ()>(())
            });
            assert_eq!(result.is_ok(), limit == exact);
            assert_eq!(
                visits,
                if limit == exact {
                    usize::from(count)
                } else {
                    0
                }
            );
            assert_eq!(budget.storage(), 0);
        }
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(exact);
        let mut budget = VisitBudget::new(&mut work, 0);
        assert!(matches!(
            entries
                .visit_root_anchors_v29(&semantic, &mut budget, |_, budget| budget.charge_work(1)),
            Err(ContextRootVisitErrorV29::Consumer(_))
        ));
        assert_eq!(work.work(), exact);
    }
}

#[test]
fn borrowed_context_visits_reject_source_substitution_before_callbacks() {
    let source = fixture(Mutation::None);
    let entries = sealed_roots(&source);
    for mutation in [
        Mutation::Arguments,
        Mutation::ArgumentKind,
        Mutation::Issuer,
        Mutation::ExtraIssue,
        Mutation::HelperIdentity,
        Mutation::ContextIdentity,
        Mutation::LayoutIdentity,
    ] {
        let semantic = fixture(mutation);
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
        let mut budget = VisitBudget::new(&mut work, 0);
        let mut visits = 0;
        assert!(matches!(
            entries.visit_root_anchors_v29(&semantic, &mut budget, |_, _| {
                visits += 1;
                Ok::<_, ()>(())
            }),
            Err(ContextRootVisitErrorV29::Source(_))
        ));
        assert_eq!(visits, 0);
    }
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = VisitBudget::new(&mut work, 0);
    assert!(matches!(
        entries.visit_root_anchors_v29(&source, &mut budget, |_, _| Err("consumer")),
        Err(ContextRootVisitErrorV29::Consumer("consumer"))
    ));
}

#[test]
fn borrowed_context_visits_stop_at_late_consumer_failure_without_resetting_budget() {
    let semantic = fixture_roots(Mutation::None, 4);
    let entries = sealed_roots(&semantic);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100);
    let mut budget = VisitBudget::new(&mut work, 7);
    budget.charge_work(11).unwrap();
    budget.reserve_storage(7).unwrap();
    let mut visits = 0;
    assert!(matches!(
        entries.visit_root_anchors_v29(&semantic, &mut budget, |_, budget| {
            visits += 1;
            budget.charge_work(3).unwrap();
            if visits == 2 { Err("consumer") } else { Ok(()) }
        }),
        Err(ContextRootVisitErrorV29::Consumer("consumer"))
    ));
    assert_eq!(visits, 2);
    assert_eq!(budget.storage(), 7);
    assert_eq!(work.work(), 11 + 1 + 4 + 2 * 3);
}

#[test]
fn production_context_handoff_joins_ssa_and_launch_under_the_shared_budget() {
    use crate::production_pipeline::{ProductionPipelineError, check_context_handoff_v29};
    use fe2o3_lower_mir_kernel::ProductionContextRootErrorV29;
    for count in [1, 4] {
        let semantic = fixture_roots(Mutation::None, count);
        let entries = sealed_roots(&semantic);
        let launch = launch_roster(&semantic);
        let owner = ssa_owner(semantic);
        let exact = {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000);
            let mut budget = VisitBudget::new(&mut work, 0);
            check_context_handoff_v29(&entries, &owner, &launch, &mut budget, |root, _| {
                assert!(std::ptr::eq(root.semantic_ssa(), &owner));
                Ok(())
            })
            .unwrap();
            work.work()
        };
        for prior in [0, 11] {
            for limit in [exact - 1, exact] {
                let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(prior + limit);
                let mut budget = VisitBudget::new(&mut work, 7);
                budget.charge_work(prior).unwrap();
                budget.reserve_storage(7).unwrap();
                let result = check_context_handoff_v29(
                    &entries,
                    &owner,
                    &launch,
                    &mut budget,
                    |_, _| Ok(()),
                );
                if limit == exact {
                    result.unwrap();
                } else {
                    assert!(matches!(
                        result,
                        Err(ProductionPipelineError::ContextHandoff(
                            ProductionContextRootErrorV29::Resource(_)
                        ))
                    ));
                }
                assert_eq!(budget.storage(), 7);
                if limit == exact {
                    assert_eq!(work.work(), prior + exact);
                } else {
                    assert!((prior..=prior + limit).contains(&work.work()));
                }
            }
        }
        let different = fixture_roots(Mutation::Arguments, count);
        let different_launch = launch_roster(&different);
        let different_owner = ssa_owner(different);
        for (ssa, launch, source_mismatch) in [
            (&different_owner, &launch, true),
            (&owner, &different_launch, false),
            (&different_owner, &different_launch, true),
        ] {
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(exact);
            let mut budget = VisitBudget::new(&mut work, 0);
            let error = check_context_handoff_v29(&entries, ssa, launch, &mut budget, |_, _| {
                panic!("substitution must reject before the observer")
            })
            .unwrap_err();
            if source_mismatch {
                assert!(matches!(
                    error,
                    ProductionPipelineError::SemanticImport(
                        crate::collector::ProductionSemanticImportErrorV1::BodyConstruction(_)
                    )
                ));
            } else {
                assert!(matches!(
                    error,
                    ProductionPipelineError::ContextHandoff(ProductionContextRootErrorV29::Launch)
                ));
            }
        }
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(exact);
        let mut budget = VisitBudget::new(&mut work, 0);
        let mut visits = 0;
        let error = check_context_handoff_v29(&entries, &owner, &launch, &mut budget, |_, _| {
            visits += 1;
            Err(ProductionContextRootErrorV29::Arguments)
        })
        .unwrap_err();
        assert_eq!(visits, 1);
        assert!(matches!(
            error,
            ProductionPipelineError::ContextHandoff(ProductionContextRootErrorV29::Arguments)
        ));
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(exact);
        let mut budget = VisitBudget::new(&mut work, 7);
        budget.reserve_storage(7).unwrap();
        let mut visits = 0;
        let error =
            check_context_handoff_v29(&entries, &owner, &launch, &mut budget, |_, budget| {
                visits += 1;
                budget.charge_work(usize::MAX)?;
                Ok(())
            })
            .unwrap_err();
        assert_eq!(visits, 1);
        assert_eq!(budget.storage(), 7);
        assert!(budget.work() > 0);
        assert!(matches!(
            error,
            ProductionPipelineError::ContextHandoff(ProductionContextRootErrorV29::Resource(_))
        ));
    }
}

#[test]
fn retained_context_custody_survives_the_existing_ssa_owner() {
    let semantic = fixture(Mutation::None);
    let retained =
        RetainedContextEntriesV29::seal(vec![retained()], &semantic, |_| Ok(())).unwrap();
    retained.validate_source(&semantic).unwrap();
    let owner = fe2o3_pliron::ProductionSemanticMirOwnerV1::try_new(
        semantic,
        fe2o3_pliron::ProductionSemanticMirLimitsV1::default(),
    )
    .unwrap();
    let ssa = fe2o3_pliron::ProductionSemanticSsaOwnerV1::try_new(
        owner,
        fe2o3_pliron::ProductionSemanticSsaLimitsV1::default(),
    )
    .unwrap();
    retained.validate_source(ssa.source_semantic()).unwrap();
    assert!(
        retained
            .validate_source(&fixture(Mutation::Arguments))
            .is_err()
    );
}

#[test]
fn retained_context_seal_checks_final_operands_identities_and_complete_roster() {
    assert!(
        completed()
            .bind_function(&fixture(Mutation::Arguments).functions()[0], |_| Ok(()))
            .is_err(),
        "same-typed argument substitution before receipt attachment"
    );
    let mut wrong_issuer = completed();
    wrong_issuer.issuer = SemanticCallableIdV1::from_index(1);
    assert!(
        wrong_issuer
            .bind_function(&fixture(Mutation::None).functions()[0], |_| Ok(()))
            .is_err()
    );
    for mutation in [
        Mutation::Arguments,
        Mutation::ArgumentKind,
        Mutation::Issuer,
        Mutation::ExtraIssue,
        Mutation::HelperIdentity,
        Mutation::ContextIdentity,
    ] {
        if matches!(mutation, Mutation::Arguments | Mutation::ArgumentKind) {
            assert!(
                completed()
                    .bind_function(&fixture(mutation).functions()[0], |_| Ok(()))
                    .is_err()
            );
        }
        assert!(
            RetainedContextEntriesV29::seal(vec![retained()], &fixture(mutation), |_| Ok(()))
                .is_err()
        );
    }
    let semantic = fixture(Mutation::None);
    assert!(RetainedContextEntriesV29::seal(vec![], &semantic, |_| Ok(())).is_err());
    assert!(
        RetainedContextEntriesV29::seal(vec![retained(), retained()], &semantic, |_| Ok(()))
            .is_err()
    );
    let mut wrong_root = retained();
    wrong_root.source.function = SemanticFunctionIdV1::from_index(1);
    assert!(RetainedContextEntriesV29::seal(vec![wrong_root], &semantic, |_| Ok(())).is_err());
}

#[test]
fn retained_context_seal_uses_exact_cumulative_work_budget() {
    let semantic = fixture(Mutation::None);
    let mut used = 0;
    RetainedContextEntriesV29::seal(vec![retained()], &semantic, |amount| {
        used += amount;
        Ok(())
    })
    .unwrap();
    for limit in [used - 1, used] {
        let mut remaining = limit;
        let result = RetainedContextEntriesV29::seal(vec![retained()], &semantic, |amount| {
            remaining = remaining.checked_sub(amount).ok_or_else(mismatch)?;
            Ok(())
        });
        assert_eq!(result.is_ok(), limit == used);
    }
}

#[test]
fn completed_context_rechecks_typed_call_boundaries_before_retention() {
    let semantic = fixture(Mutation::None);
    for mutation in 0..15 {
        let corrupt = |entry: &mut CompletedContextEntryV29| {
            if mutation == 14 {
                entry.helper_argument = SemanticLocalIdV1::from_index(2);
                return;
            }
            let call = if mutation < 7 {
                &mut entry.issuance
            } else {
                &mut entry.helper_call
            };
            match mutation % 7 {
                0 => call.destination_type = U32,
                1 => call.destination = SemanticLocalIdV1::from_index(0),
                2 => call.target = SemanticBlockIdV1::from_index(0),
                3 => call.statements = 1,
                4 => call.unwind = SemanticUnwindActionV1::Continue,
                5 => call.block = SemanticBlockIdV1::from_index(2),
                6 => call.block = SemanticBlockIdV1::from_index(u32::MAX),
                _ => unreachable!(),
            }
        };
        let mut entry = completed();
        corrupt(&mut entry);
        assert!(
            matches!(
                entry.bind_function(&semantic.functions()[0], |_| Ok(())),
                Err(Error::IdentityTableMismatch {
                    table: "retained context entry custody"
                })
            ),
            "boundary {mutation}"
        );
        let mut entry = retained();
        corrupt(&mut entry.source);
        assert!(
            matches!(
                RetainedContextEntriesV29::seal(vec![entry], &semantic, |_| Ok(())),
                Err(Error::IdentityTableMismatch {
                    table: "retained context entry custody"
                })
            ),
            "sealed boundary {mutation}"
        );
    }
}
