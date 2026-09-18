use super::*;

#[path = "production_checked_output_private_memory_policy3_v1_tests.rs"]
mod private_memory_tests;

fn general_control_source(looping: bool) -> ProductionPreRankedKirOwnerV1 {
    general_control_and_read_source(looping, false)
}

fn general_control_and_read_source(looping: bool, read: bool) -> ProductionPreRankedKirOwnerV1 {
    let (ssa, launch) = fixture_with_blocks_and_symbol(
        if looping {
            Fixture::Literal(true)
        } else {
            Fixture::ElidedBounds
        },
        false,
        |_, _| {
            if looping {
                vec![block(
                    31,
                    vec![assignment(
                        1,
                        BOOL,
                        SemanticRvalueKindV1::Use(constant(BOOL, 1, 1)),
                    )],
                    SemanticTerminatorKindV1::Goto(edge(SemanticEdgeRoleV1::Goto, 0)),
                )]
            } else {
                vec![
                    block(
                        31,
                        vec![
                            assignment(
                                2,
                                U64,
                                SemanticRvalueKindV1::Unary {
                                    operation: SemanticUnaryOpV1::PointerMetadata,
                                    operand: value(1, SLICE_REF),
                                },
                            ),
                            assignment(3, U64, SemanticRvalueKindV1::Use(constant(U64, 0, 8))),
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
                        SemanticTerminatorKindV1::Assert {
                            condition: value(4, BOOL),
                            expected: true,
                            message: SemanticAssertMessageV1::BoundsCheck {
                                length: value(2, U64),
                                index: value(3, U64),
                            },
                            target: edge(SemanticEdgeRoleV1::AssertSuccess, 1),
                            unwind: SemanticUnwindActionV1::Unreachable,
                        },
                    ),
                    block(
                        32,
                        if read {
                            vec![assignment(
                                5,
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
                                                SemanticProjectionKindV1::Index(
                                                    SemanticLocalIdV1::from_index(3),
                                                ),
                                                U32,
                                            )
                                            .unwrap(),
                                        ],
                                        U32,
                                    )
                                    .unwrap(),
                                )),
                            )]
                        } else {
                            vec![]
                        },
                        SemanticTerminatorKindV1::Return,
                    ),
                ]
            }
        },
        |_| "private_array_relation".to_owned(),
        if looping {
            &[BOOL]
        } else if read {
            &[U32]
        } else {
            &[]
        },
    );
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
    ProductionPreRankedKirOwnerV1::try_materialize_with_budget(
        ssa,
        launch,
        ProductionSemanticKirLimitsV1::default(),
        &mut budget,
    )
    .unwrap()
}

#[test]
fn general_policy3_binds_actual_global_load_to_consumed_source_and_fresh_output_fact() {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let source = general_control_and_read_source(false, true);
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
    let root = ProductionRankedSemanticProjectionRootV1::new(
        ARRAY_ROOT,
        1,
        lowering,
        "source-correlated independently compiled read component, not the backend projector"
            .to_owned(),
        vec![ProductionRankedAccessSourceV1::new(1, Some(0), 0, 0, 3)],
        vec![],
    );
    let receipt =
        ProductionMaterializedRankedModuleReceiptV1::from_unvalidated_projection_roster_candidate(
            source,
            vec![root],
        )
        .unwrap();
    with_prepared(prepare(receipt, Profile::Gfx942, None), |input, budget| {
        let owner =
            AdmittedOutput::try_admit_general_v1(input.receipt, input.bound, input.output, budget)
                .unwrap();
        let [fact] = owner.kernels()[0].accesses() else {
            panic!("one actual global read")
        };
        assert_eq!(fact.kind(), fe2o3_kernel_ir::FormalMemoryAccessKind::Read);
        assert_eq!(fact.address_space(), AddressSpace::Global);
        let operation = &owner.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .find(|block| block.id == fact.location().block)
            .unwrap()
            .operations[fact.location().operation_index];
        assert!(matches!(
            operation.kind,
            OperationKind::Load { .. } | OperationKind::GuardedLoad { .. }
        ));
        assert_eq!(owner.kernels()[0].bounds_requirements().len(), 1);
        owner.verify_equivalence(budget).unwrap();
    });
}

#[test]
fn general_policy3_accepts_real_scalar_and_unused_borrowed_roots_without_fallback() {
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        for borrowed in [false, true] {
            let input = prepare(
                array_output_ranked_receipt_v1(scalar_source(false, borrowed)),
                profile,
                None,
            );
            with_prepared(input, |input, budget| {
                let expected = *input.output.owner().canonical().identity();
                let minimum = input.source_storage + input.output_storage;
                let owner = AdmittedOutput::try_admit_general_v1(
                    input.receipt,
                    input.bound,
                    input.output,
                    budget,
                )
                .unwrap();
                assert_eq!(*owner.output().canonical().identity(), expected);
                assert!(std::ptr::eq(owner.output(), owner.checked_output().owner()));
                assert_eq!(owner.retained_input_storage_floor_v1().unwrap(), minimum);
                assert!(owner.kernels()[0].accesses().is_empty());
                assert!(!owner.grants_artifact_or_launch_authority());
                owner.verify_equivalence(budget).unwrap();
            });
        }
    }
}

#[test]
fn general_policy3_retains_actual_runtime_assertion_and_failure_trap() {
    let input = prepare(
        array_output_ranked_receipt_v1(general_control_source(false)),
        Profile::Gfx942,
        None,
    );
    with_prepared(input, |input, budget| {
        let owner =
            AdmittedOutput::try_admit_general_v1(input.receipt, input.bound, input.output, budget)
                .unwrap();
        let blocks = &owner.output().module().functions[0]
            .body
            .as_ref()
            .unwrap()
            .blocks;
        assert!(
            blocks.iter().any(|block| matches!(
                block.terminator,
                Some(Terminator::ConditionalBranch { .. })
            ))
        );
        assert!(blocks.iter().any(|block| block.operations.iter().any(|operation| {
            matches!(&operation.kind, OperationKind::Call { callee, arguments }
                if matches!(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::from_intrinsic_call(callee, arguments),
                    Some(fe2o3_kernel_ir::AmdGpuDiagnosticOperation::Trap)))
        })));
        owner.verify_equivalence(budget).unwrap();
    });
}

#[test]
fn general_policy3_cannot_bypass_existing_ranked_nontermination_refusal() {
    use fe2o3_pliron::{
        ProductionConstructionV1, ProductionRankedBlockV1, ProductionRankedKernelV1,
        ProductionRankedTerminatorV1, ProductionSessionLimitsV1,
        compile_ranked_kernel_for_lowering_v1,
    };
    let source = general_control_source(true);
    let layout = source.source_launch().roots()[0].layout();
    let kernel = ProductionRankedKernelV1::new(
        "private_array_relation",
        0,
        vec![ProductionRankedBlockV1::new(
            vec![ProductionRankedOperationV1::ExecutionLayout {
                grid_identity: layout.grid_identity(),
                global_extents: layout.global_extents(),
                workgroup_extents: layout.workgroup_extents(),
                subgroup_size: layout.subgroup_size(),
                full_physical_workgroups: layout.full_physical_workgroups(),
            }],
            ProductionRankedTerminatorV1::Branch { target: 0 },
        )],
    )
    .unwrap();
    let error = compile_ranked_kernel_for_lowering_v1(
        ProductionConstructionV1::ranked_kernel("private_array_relation", kernel).unwrap(),
        ProductionSessionLimitsV1::default(),
    )
    .unwrap_err();
    let fe2o3_pliron::ProductionRankedCompileErrorV1::Session(
        fe2o3_pliron::ProductionSessionErrorV1::RankedSemantic(error),
    ) = error
    else {
        panic!("unexpected loop rejection: {error:?}");
    };
    assert!(matches!(error.report().progress().findings(),
        [fe2o3_pliron::PlironProgressFindingV1::NonTerminatingCycle { blocks, .. }]
        if blocks == &[0]));
    // A source owner alone cannot replace the refused ranked receipt.
    assert_eq!(source.executable().module().kernels.len(), 1);
}

#[test]
fn general_policy3_rejects_changed_assertion_branches_and_removed_trap_at_exact_nb_join() {
    let mutations: [fn(&mut Module); 3] = [
        |module| {
            let blocks = &mut module.functions[0].body.as_mut().unwrap().blocks;
            let control = blocks
                .iter_mut()
                .find_map(|b| match b.terminator.as_mut() {
                    Some(Terminator::ConditionalBranch {
                        then_target,
                        else_target,
                        ..
                    }) => Some((then_target, else_target)),
                    _ => None,
                })
                .unwrap();
            std::mem::swap(control.0, control.1);
        },
        |module| {
            for block in &mut module.functions[0].body.as_mut().unwrap().blocks {
                block
                    .operations
                    .retain(|operation| !matches!(operation.kind, OperationKind::Call { .. }));
            }
        },
        |module| {
            let block = module.functions[0]
                .body
                .as_mut()
                .unwrap()
                .blocks
                .iter_mut()
                .find(|b| matches!(b.terminator, Some(Terminator::ConditionalBranch { .. })))
                .unwrap();
            let Some(Terminator::ConditionalBranch {
                then_target,
                then_arguments,
                ..
            }) = &block.terminator
            else {
                unreachable!()
            };
            block.terminator = Some(Terminator::Branch {
                target: *then_target,
                arguments: then_arguments.clone(),
            });
        },
    ];
    for mutate in mutations {
        let input = prepare(
            array_output_ranked_receipt_v1(general_control_source(false)),
            Profile::Gfx942,
            Some(mutate),
        );
        with_prepared(input, |input, budget| {
            assert!(matches!(
                AdmittedOutput::try_admit_general_v1(
                    input.receipt,
                    input.bound,
                    input.output,
                    budget
                ),
                Err(AdmissionError::Coordinates(_))
            ));
        });
    }
}

#[test]
fn general_policy3_admits_completely_initialized_exact_private_addresses() {
    let input = prepare(
        array_output_ranked_receipt_v1(array_owner(ArrayCase::Initializer {
            values: [11; 8],
            repetitions: 1,
            float: false,
        })),
        Profile::Gfx942,
        None,
    );
    with_prepared(input, |input, budget| {
        let owner =
            AdmittedOutput::try_admit_general_v1(input.receipt, input.bound, input.output, budget)
                .unwrap();
        assert!(owner.kernels()[0].accesses().is_empty());
        owner.verify_equivalence(budget).unwrap();
    });
}

#[test]
fn general_policy3_restores_caller_floor_after_partial_workspace_failure() {
    let input = prepare(
        array_output_ranked_receipt_v1(scalar_source(false, false)),
        Profile::Gfx942,
        None,
    );
    let retained = FLOOR + input.source_storage + input.bound_storage + input.output_storage;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(WORK);
    let mut budget = AssertOriginBudgetV1::new(&mut work, retained + 1);
    budget.reserve_storage(retained).unwrap();
    assert!(
        AdmittedOutput::try_admit_general_v1(input.receipt, input.bound, input.output, &mut budget)
            .is_err()
    );
    assert_eq!(budget.storage(), retained);
    assert!(budget.work() > 8);
}

#[test]
fn general_policy3_entry_work_and_floor_precedence_are_unchanged() {
    for limit in [7, 8] {
        let input = prepare(
            array_output_ranked_receipt_v1(scalar_source(false, false)),
            Profile::Gfx942,
            None,
        );
        let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = AssertOriginBudgetV1::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let error = AdmittedOutput::try_admit_general_v1(
            input.receipt,
            input.bound,
            input.output,
            &mut budget,
        )
        .unwrap_err();
        if limit == 7 {
            assert!(matches!(
                error,
                AdmissionError::Resource(AssertOriginResourceV1::Work(_))
            ));
        } else {
            assert!(matches!(
                error,
                AdmissionError::Resource(AssertOriginResourceV1::Accounting)
            ));
        }
        assert_eq!(budget.storage(), FLOOR);
    }
}
