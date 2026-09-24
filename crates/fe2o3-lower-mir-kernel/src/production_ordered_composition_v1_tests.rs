//! Inert fixtures qualify same-source lowering/custody only, not rustc source authentication.
use super::*;
include!("production_ordered_composition_resource_v1_tests.rs");
const HELPER: SemanticFunctionIdV1 = SemanticFunctionIdV1::from_index(1);

#[derive(Clone, Copy)]
enum CompositionCase {
    Root,
    TwoRoot,
    Helper,
    Twice,
    RootHelper,
    ScalarHelper,
    Wrapping,
}
fn rebuild(
    original: &SemanticFunctionDeclV1,
    blocks: Vec<SemanticBasicBlockV1>,
) -> SemanticFunctionDeclV1 {
    let mut result = SemanticFunctionDeclV1::new(
        original.identity(),
        original.role(),
        original.item_definition_identity(),
        original.monomorphization_identity(),
        original.generic_type_arguments_identity(),
        original.const_generic_arguments_identity(),
        original.source(),
        original.abi().clone(),
        original.locals().to_vec(),
        original.entry(),
        blocks,
    )
    .unwrap();
    if let Some(entry) = original.kernel_entry() {
        result = result.with_kernel_entry(entry.clone());
    }
    result
}
fn marker(
    callable: u32,
    function: SemanticFunctionIdentityV1,
    site: u8,
    destination: u32,
    next: u32,
    first: u32,
) -> SemanticDirectCallV1 {
    let mut args = vec![input(first), input(2), input(3)];
    args.extend([32, 33, 34, 35, 36].into_iter().map(literal));
    SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(callable),
        args,
        Some(SemanticCallDestinationV1::new(
            place(destination, U32),
            edge(SemanticEdgeRoleV1::CallReturn, next),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
    .with_ordered_program_source_v32(
        SemanticOrderedProgramSourceV32::new([30; 32], function, [31; 32], [site; 32]).unwrap(),
    )
}
fn helper_call(next: u32, first: u32) -> SemanticDirectCallV1 {
    SemanticDirectCallV1::new_callable(
        SemanticCallableIdV1::from_index(1),
        vec![input(first), input(2), input(3)],
        Some(SemanticCallDestinationV1::new(
            place(4, U32),
            edge(SemanticEdgeRoleV1::CallReturn, next),
        )),
        SemanticUnwindActionV1::Unreachable,
    )
    .unwrap()
}
fn composition_request(case: CompositionCase) -> InertSemanticMirRequestV1 {
    let admitted = Fixture::default()
        .request()
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let root = &admitted.functions()[0];
    let with_helper = !matches!(case, CompositionCase::Root | CompositionCase::TwoRoot);
    let root_marker = matches!(
        case,
        CompositionCase::Root
            | CompositionCase::TwoRoot
            | CompositionCase::RootHelper
            | CompositionCase::ScalarHelper
    );
    let mut root_blocks = vec![];
    if root_marker {
        root_blocks.push(block(
            0,
            vec![],
            SemanticTerminatorKindV1::Call(marker(
                if with_helper { 2 } else { 1 },
                function_identity(),
                32,
                4,
                1,
                1,
            )),
        ));
    }
    if matches!(case, CompositionCase::TwoRoot) {
        root_blocks.push(block(
            1,
            vec![statement(SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::BitXor,
                left: input(4),
                right: input(3),
            })],
            SemanticTerminatorKindV1::Call(marker(1, function_identity(), 33, 4, 2, 5)),
        ));
    }
    if with_helper {
        let next = root_blocks.len() as u32 + 1;
        root_blocks.push(block(
            root_blocks.len() as u8,
            vec![],
            SemanticTerminatorKindV1::Call(helper_call(next, if root_marker { 4 } else { 1 })),
        ));
        if matches!(case, CompositionCase::Twice) {
            let next = root_blocks.len() as u32 + 1;
            root_blocks.push(block(
                root_blocks.len() as u8,
                vec![],
                SemanticTerminatorKindV1::Call(helper_call(next, 4)),
            ));
        }
    }
    root_blocks.push(block(
        root_blocks.len() as u8,
        vec![],
        SemanticTerminatorKindV1::Return,
    ));
    let mut functions = vec![rebuild(root, root_blocks)];
    let mut callables = admitted.callables().to_vec();
    if with_helper {
        let identity = SemanticFunctionIdentityV1::from_sha256([120; 32]);
        let abi = SemanticFunctionAbiV1::new(
            SemanticAbiIdentityV1::from_sha256([140; 32]),
            SemanticLayoutIdentityV1::from_sha256([141; 32]),
            SemanticCanonAbiV1::Rust,
            false,
            false,
            vec![abi_value(U32); 3],
            abi_value(U32),
        )
        .unwrap()
        .with_source_argument_ownership(vec![SemanticSourceArgumentOwnershipV1::ByValue; 3])
        .unwrap();
        let locals = (0..5)
            .map(|i| {
                SemanticLocalDeclV1::new(
                    SemanticLocalIdentityV1::from_sha256([150 + i as u8; 32]),
                    U32,
                    if i == 0 {
                        SemanticLocalRoleV1::Return
                    } else if i <= 3 {
                        SemanticLocalRoleV1::Argument(i - 1)
                    } else {
                        SemanticLocalRoleV1::Temporary
                    },
                    provenance(),
                )
            })
            .collect();
        let blocks = if matches!(case, CompositionCase::ScalarHelper) {
            vec![block(
                20,
                vec![SemanticStatementV1::new(
                    provenance(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(0, U32),
                        SemanticRvalueV1::new(
                            U32,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::BitXor,
                                left: input(1),
                                right: input(2),
                            },
                        ),
                    )),
                )],
                SemanticTerminatorKindV1::Return,
            )]
        } else {
            let statements = if matches!(case, CompositionCase::Wrapping) {
                vec![SemanticStatementV1::new(
                    provenance(),
                    SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                        place(4, U32),
                        SemanticRvalueV1::new(
                            U32,
                            SemanticRvalueKindV1::Binary {
                                operation: SemanticBinaryOpV1::Add,
                                left: input(1),
                                right: input(2),
                            },
                        ),
                    )),
                )]
            } else {
                vec![]
            };
            vec![
                block(
                    20,
                    statements,
                    SemanticTerminatorKindV1::Call(marker(
                        2,
                        identity,
                        132,
                        0,
                        1,
                        if matches!(case, CompositionCase::Wrapping) {
                            4
                        } else {
                            1
                        },
                    )),
                ),
                block(21, vec![], SemanticTerminatorKindV1::Return),
            ]
        };
        functions.push(
            SemanticFunctionDeclV1::new(
                identity,
                SemanticFunctionRoleV1::InternalHelper,
                SemanticItemDefinitionIdentityV1::from_sha256([121; 32]),
                SemanticMonomorphizationIdentityV1::from_sha256([122; 32]),
                SemanticGenericTypeArgumentsIdentityV1::from_sha256([123; 32]),
                SemanticConstGenericArgumentsIdentityV1::from_sha256([124; 32]),
                provenance(),
                abi,
                locals,
                SemanticBlockIdV1::from_index(0),
                blocks,
            )
            .unwrap(),
        );
        // Canonical callable order is every Defined body first, then intrinsics.
        callables.insert(1, SemanticCallableDeclV1::defined(HELPER));
    }
    InertSemanticMirRequestV1::new_with_callables(
        admitted.target(),
        admitted.types().to_vec(),
        vec![],
        vec![],
        vec![],
        functions,
        callables,
        vec![ROOT],
    )
    .unwrap()
}
fn composition_owners(
    request: InertSemanticMirRequestV1,
) -> (
    ProductionSemanticSsaOwnerV1,
    crate::ProductionSourceLaunchRosterV1,
) {
    let admitted = request
        .admit_current_production(SemanticMirLimitsV1::default())
        .unwrap();
    let owner =
        ProductionSemanticMirOwnerV1::try_new(admitted, ProductionSemanticMirLimitsV1::default())
            .unwrap();
    let ssa =
        ProductionSemanticSsaOwnerV1::try_new(owner, ProductionSemanticSsaLimitsV1::default())
            .unwrap();
    let launch = crate::ProductionSourceLaunchRosterV1::try_new(
        ssa.source_semantic(),
        &[crate::ProductionSourceLaunchRootInputV1::new(
            "ordered_program_root",
            [15; 32],
            crate::ProductionSourceLaunchInputV1::new(1, Some([64, 1, 1]), [1, 1, 1]),
        )],
    )
    .unwrap();
    (ssa, launch)
}
fn with_composition(
    case: CompositionCase,
    use_owner: impl FnOnce(
        &mut ProductionOrderedCompositionPreRankedKirOwnerV1,
        &mut ArgumentBudgetV1<'_>,
    ),
) {
    let (mut ssa, launch) = composition_owners(composition_request(case));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 64 * 1024 * 1024);
    budget.reserve_storage(73).unwrap();
    let capture = ssa
        .try_capture_occurrences_with_budget_v1(&mut budget)
        .unwrap();
    budget.reserve_storage(capture.retained_storage()).unwrap();
    let floor = budget.storage();
    let mut owner = ProductionOrderedCompositionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap();
    assert_eq!(budget.storage(), floor);
    budget
        .reserve_storage(owner.retained_storage().retained_storage())
        .unwrap();
    let live = budget.storage();
    owner.verify_equivalence(&mut budget).unwrap();
    assert_eq!(budget.storage(), live);
    use_owner(&mut owner, &mut budget);
    assert_eq!(budget.storage(), live);
}
#[test]
fn ordered_composition_root_and_helpers_keep_complete_graph_and_real_occurrences() {
    for (case, defs, calls, occurrences) in [
        (CompositionCase::Root, 1, 0, 1),
        (CompositionCase::TwoRoot, 2, 0, 2),
        (CompositionCase::Helper, 1, 1, 1),
        (CompositionCase::Twice, 1, 2, 2),
        (CompositionCase::RootHelper, 2, 1, 2),
        (CompositionCase::ScalarHelper, 1, 1, 1),
        (CompositionCase::Wrapping, 1, 1, 1),
    ] {
        with_composition(case, |owner, budget| {
            assert_eq!(owner.definitions().count(), defs);
            assert_eq!(owner.composition().calls().len(), calls);
            assert_eq!(owner.occurrences().count(), occurrences);
            assert!(!owner.grants_artifact_or_launch_authority());
            owner
                .with_checked_canonical_calls_v17(budget, |view, budget| {
                    assert!(view.belongs_to(owner));
                    assert_eq!(view.call_count(), calls);
                    for key in view.sites() {
                        view.with_call(key, budget, |call| {
                            assert_eq!(call.source().arguments().len(), 3);
                            assert_eq!(call.result_count(), 1);
                            assert!(!call.result_is_aggregate());
                            assert!(matches!(call.operation().kind, OperationKind::Call { .. }));
                            Ok(())
                        })?;
                    }
                    Ok(())
                })
                .unwrap();
        });
    }
}
#[test]
fn ordered_composition_two_helper_calls_share_definition_not_source_instance() {
    with_composition(CompositionCase::Twice, |owner, _| {
        let rows = owner.occurrences().collect::<Vec<_>>();
        assert_eq!(rows[0].definition(), rows[1].definition());
        assert_ne!(
            rows[0].call_instance_ordinal(),
            rows[1].call_instance_ordinal()
        );
    });
}
#[test]
fn ordered_composition_wrapping_scalar_remains_checked_not_trapping() {
    with_composition(CompositionCase::Wrapping, |owner, _| {
        let helper = owner
            .executable()
            .module()
            .functions
            .iter()
            .find(|f| f.role == fe2o3_kernel_ir::FunctionRole::InternalHelper)
            .unwrap();
        let operations = helper
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|b| &b.operations)
            .collect::<Vec<_>>();
        assert!(operations.iter().any(|op| matches!(
            op.kind,
            OperationKind::Binary {
                op: BinaryOp::Checked(_),
                ..
            }
        )));
        assert!(!operations.iter().any(|op| matches!(
            op.kind,
            OperationKind::Binary {
                op: BinaryOp::Add,
                ..
            }
        )));
    });
}
#[test]
fn ordered_composition_inspection_joins_exact_owner_and_refuses_foreign_identity() {
    with_composition(CompositionCase::RootHelper, |owner, budget| {
        let key = owner.composition().definitions()[1].key();
        with_ordered_composition_inspection_v1(owner, budget, |view, budget| {
            let selected = view.definition(
                key,
                owner.executable().identity(),
                owner.correspondence().semantic_sha256(),
                budget,
            )?;
            assert_eq!(selected.semantic_function(), HELPER);
            assert!(matches!(
                selected.operation().kind,
                OperationKind::Gfx942OrderedProgram(_)
            ));
            let mut bad = *owner.correspondence().semantic_sha256();
            bad[0] ^= 1;
            assert!(
                view.definition(key, owner.executable().identity(), &bad, budget)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
    });
}
#[test]
fn ordered_composition_missing_occurrence_capture_refuses_without_floor_loss() {
    let (ssa, launch) = composition_owners(composition_request(CompositionCase::Helper));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(19).unwrap();
    assert!(
        ProductionOrderedCompositionPreRankedKirOwnerV1::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), 19);
}
#[test]
fn ordered_composition_old_singleton_keeps_helper_refusal() {
    let (ssa, launch) = composition_owners(composition_request(CompositionCase::Helper));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
    assert!(
        ProductionOrderedProgramPreRankedKirOwnerV17::try_materialize_with_budget(
            ssa,
            launch,
            ProductionSemanticKirLimitsV1::default(),
            &mut budget
        )
        .is_err()
    );
    assert_eq!(budget.storage(), 0);
}
#[test]
fn ordered_composition_old_conditional_and_backedge_refusals_remain() {
    for flow in [Flow::Conditional, Flow::Backedge] {
        let fixture = Fixture {
            flow,
            ..Fixture::default()
        };
        let (mut ssa, launch) = fixture.owners();
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = ArgumentBudgetV1::new(&mut work, 1_000_000);
        let captured = ssa
            .try_capture_occurrences_with_budget_v1(&mut budget)
            .unwrap();
        budget.reserve_storage(captured.retained_storage()).unwrap();
        let floor = budget.storage();
        assert!(
            ProductionOrderedCompositionPreRankedKirOwnerV1::try_materialize_with_budget(
                ssa,
                launch,
                ProductionSemanticKirLimitsV1::default(),
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.storage(), floor);
    }
}
#[test]
fn ordered_composition_distinct_ssa_substitution_fails_full_replay() {
    with_composition(CompositionCase::Root, |owner, budget| {
        let mut module = owner.executable().module().clone();
        let function = module
            .functions
            .iter_mut()
            .find(|f| f.role == fe2o3_kernel_ir::FunctionRole::KernelEntry)
            .unwrap();
        let operation = function
            .body
            .as_mut()
            .unwrap()
            .blocks
            .iter_mut()
            .flat_map(|b| &mut b.operations)
            .find(|op| matches!(op.kind, OperationKind::Gfx942OrderedProgram(_)))
            .unwrap();
        let OperationKind::Gfx942OrderedProgram(program) = &operation.kind else {
            unreachable!()
        };
        let mut inputs = *program.inputs();
        // Distinct SSA inputs may be numerically equal at runtime. They still
        // cannot be substituted in a source-preserving compilation.
        inputs[0] = inputs[1];
        operation.kind = OperationKind::Gfx942OrderedProgram(
            fe2o3_kernel_ir::Gfx942OrderedProgramV1::new(
                program.source(),
                program.registers(),
                inputs,
                program.program().clone(),
            )
            .unwrap(),
        );
        let (canonical,receipt)=fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV17::from_module_ref_with_verification_budget_v17(&module,budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let (composition, storage) =
            fe2o3_kernel_ir::VerifiedOrderedProgramCompositionV1::try_from_canonical_v17(
                canonical, budget,
            )
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let original = std::mem::replace(&mut owner.composition, composition);
        assert!(owner.verify_equivalence(budget).is_err());
        drop(std::mem::replace(&mut owner.composition, original));
        budget
            .release_storage(storage.retained_storage() + receipt.retained_storage())
            .unwrap();
    });
}
#[test]
fn ordered_composition_call_and_occurrence_substitutions_fail_replay() {
    with_composition(CompositionCase::Twice, |owner, budget| {
        let original = owner.sources.occurrences[1];
        owner.sources.occurrences[1] = owner.sources.occurrences[0];
        assert!(owner.verify_equivalence(budget).is_err());
        owner.sources.occurrences[1] = original;
        owner.verify_equivalence(budget).unwrap();
    });
}

#[path = "production_ordered_composition_checked_v1_tests.rs"]
mod checked_v1;
