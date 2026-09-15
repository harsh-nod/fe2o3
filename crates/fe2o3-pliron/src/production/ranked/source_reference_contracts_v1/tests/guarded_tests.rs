use super::*;

fn guarded_kernel(
    limit: u64,
    read: bool,
) -> (
    ProductionRankedKernelV1,
    ProductionEffectRefinementContractV2,
    FunctionalRefinementSubjectsV2,
) {
    let subjects = FunctionalRefinementSubjectsV2::new(
        SafeReferenceKindV2::Mir,
        digest(1),
        DigestV1::ZERO,
        digest(2),
        digest(3),
        digest(4),
    )
    .unwrap();
    let effect = ProductionEffectRefinementContractV2::new(
        91,
        ProductionGpuWriteSiteV2::new(1, 2),
        ProductionReferenceOutputSiteV2::new(2, 3, 4),
        local(0),
        vec![local(1)],
        vec![local(2)],
        vec![local(2)],
        local(3),
        local(3),
        local(3),
        local(3),
        local(5),
        local(6),
    )
    .unwrap();
    let expression = || {
        if read {
            X::Load(crate::production::ProductionSemanticLoadV2 {
                block: 3,
                operation: 0,
                scalar: ProductionSemanticScalarTypeV2::Float { bits: 32 },
                read_mode: crate::production::ProductionSemanticReadModeV2::UnorderedNonVolatile,
                allocation_origin: 7,
                view: local(0),
                indices: vec![local(1)].into_boxed_slice(),
            })
        } else {
            f32_constant(1.0)
        }
    };
    let numerical = ProductionNumericalContractV2::exact_for_expression(&expression());
    let mut blocks = vec![
        ProductionRankedBlockV1::new(
            vec![
                O::ExecutionLayout {
                    grid_identity: 1,
                    global_extents: [8, 1, 1],
                    workgroup_extents: [1, 1, 1],
                    subgroup_size: 1,
                    full_physical_workgroups: true,
                },
                O::ViewInSpace {
                    result: id(0),
                    element_width: 32,
                    writable: true,
                    shape: vec![7],
                    dynamic_extents: vec![],
                    memory_space: MemorySpaceAttr::Global,
                    allocation_origin: 7,
                    noalias_class: 9,
                },
                O::InvocationIndex {
                    result: id(1),
                    dimension: 0,
                    launch_extent: 8,
                },
                O::SemanticExpression {
                    result: id(2),
                    expression: X::Symbol {
                        symbol: 0,
                        scalar: ProductionSemanticScalarTypeV2::Integer {
                            signed: false,
                            bits: 64,
                        },
                    },
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                O::SemanticExpression {
                    result: id(3),
                    expression: X::Constant {
                        scalar: ProductionSemanticScalarTypeV2::Bool,
                        bits: 1,
                    },
                    numerical_contract:
                        ProductionNumericalContractV2::ExactBitVectorOperatorCongruence,
                },
                O::IndexConstant {
                    result: id(4),
                    value: limit,
                },
                O::OwnershipContract {
                    view: local(0),
                    coverage: OwnershipCoverageAttr::TotalView,
                    partition: OwnershipPartitionAttr::ExactSets,
                },
            ],
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs: local(1),
                rhs: local(4),
                true_block: if read { 3 } else { 1 },
                false_block: 2,
            },
        ),
        ProductionRankedBlockV1::new(
            vec![
                O::SemanticExpression {
                    result: id(5),
                    expression: expression(),
                    numerical_contract: numerical,
                },
                O::SemanticExpression {
                    result: id(6),
                    expression: expression(),
                    numerical_contract: numerical,
                },
                O::ValueAccess {
                    kind: AccessKindAttr::Write,
                    view: local(0),
                    indices: vec![local(1)],
                    value: local(5),
                },
                O::RequestEffectRefinement {
                    contract: effect.clone(),
                    subjects,
                },
            ],
            ProductionRankedTerminatorV1::Branch { target: 2 },
        ),
        ProductionRankedBlockV1::new(vec![], ProductionRankedTerminatorV1::Return),
    ];
    if read {
        // Physically later, but this original read dominates both RHS roots.
        blocks.push(ProductionRankedBlockV1::new(
            vec![O::Access {
                kind: AccessKindAttr::Read,
                view: local(0),
                indices: vec![local(1)],
            }],
            ProductionRankedTerminatorV1::Branch { target: 1 },
        ));
    }
    (
        ProductionRankedKernelV1::new("guarded_output", 0, blocks).unwrap(),
        effect,
        subjects,
    )
}

#[test]
fn guarded_tail_exports_exact_nonentry_write_with_nonvacuous_coverage() {
    for read in [false, true] {
        let (kernel, effect, subjects) = guarded_kernel(7, read);
        let input = compile_fixture(kernel, 1, 3, &effect, subjects).unwrap();
        let exported = input.export_live_source_reference_contracts_v1().unwrap();
        assert_eq!(exported.outputs().len(), 1);
        assert_eq!(exported.outputs()[0].effect(), &effect);
        assert_eq!(
            exported.source_kernel().blocks().len(),
            if read { 4 } else { 3 }
        );
        let report = &input.production_pipeline_report;
        let ownership = report.ownership();
        assert!(ownership.all_total_view_contracts_are_proved());
        assert_eq!(ownership.coverage_summary().total_view_declared(), 1);
        assert_eq!(ownership.coverage_summary().total_view_proved(), 1);
        let effects = report.semantics().effect_refinement();
        assert!(effects.all_declared_effects_are_proved());
        assert_eq!(effects.contract_count(), 1);
        assert_eq!(effects.proved_contract_count(), 1);
        assert!(!exported.grants_compiler_refinement_authority());
        assert!(!exported.grants_artifact_or_launch_authority());
        if read {
            let X::Load(load) = exported.outputs()[0].reference_rhs() else {
                panic!("CPU read")
            };
            assert_eq!((load.block, load.operation), (3, 0));
            assert_eq!(load.indices.as_ref(), [local(1)]);
            assert_eq!(load.allocation_origin, 7);
        }
    }
}

#[test]
fn matching_guard_hole_fails_whole_output_coverage() {
    for read in [false, true] {
        let (mut kernel, effect, subjects) = guarded_kernel(6, read);
        // Supply the exact bounds guard too, so rejection tests coverage rather
        // than the bounds checker's narrower syntactic guard matching.
        let extra_guard = kernel.blocks.len() as u32;
        let ProductionRankedTerminatorV1::IndexLessThan { true_block, .. } =
            &mut kernel.blocks[0].terminator
        else {
            panic!("entry guard")
        };
        let original_target = *true_block;
        *true_block = extra_guard;
        kernel.blocks.push(ProductionRankedBlockV1::new(
            vec![O::IndexConstant {
                result: id(7),
                value: 7,
            }],
            ProductionRankedTerminatorV1::IndexLessThan {
                lhs: local(1),
                rhs: local(7),
                true_block: original_target,
                false_block: 2,
            },
        ));
        let error = compile_fixture(kernel, 1, 3, &effect, subjects).unwrap_err();
        let ProductionRankedCompileErrorV2::Pipeline(ProductionRankedCompileErrorV1::Session(
            ProductionSessionErrorV1::RankedOwnership(error),
        )) = error
        else {
            panic!("{error}")
        };
        assert!(error.report().findings().iter().any(|finding| matches!(finding,
            fe2o3_kernel_analysis::HierarchicalOwnershipFindingV1::CoverageHole { coordinate, extents, .. }
                if coordinate == &[6] && extents == &[7])), "{error}");
    }
}

#[test]
fn load_free_finite_loop_before_guarded_output_uses_existing_progress_proof() {
    for advances in [true, false] {
        let (mut kernel, effect, subjects) = guarded_kernel(7, false);
        kernel.blocks[0].terminator = ProductionRankedTerminatorV1::Branch { target: 5 };
        let header = ProductionRankedValueV1::BlockArgument {
            block: 3,
            argument: 0,
        };
        let body = ProductionRankedValueV1::BlockArgument {
            block: 4,
            argument: 0,
        };
        kernel
            .blocks
            .push(ProductionRankedBlockV1::with_index_arguments(
                1,
                vec![],
                ProductionRankedTerminatorV1::IndexLessThanArgs {
                    lhs: header,
                    rhs: local(9),
                    true_block: 4,
                    false_block: 1,
                    true_arguments: vec![header],
                    false_arguments: vec![],
                },
            ));
        let preheader = ProductionRankedBlockV1::new(
            vec![
                O::IndexConstant {
                    result: id(7),
                    value: 0,
                },
                O::IndexConstant {
                    result: id(8),
                    value: 1,
                },
                O::IndexConstant {
                    result: id(9),
                    value: 2,
                },
            ],
            ProductionRankedTerminatorV1::IndexLessThanArgs {
                lhs: local(1),
                rhs: local(4),
                true_block: 3,
                false_block: 2,
                true_arguments: vec![local(7)],
                false_arguments: vec![],
            },
        );
        kernel
            .blocks
            .push(ProductionRankedBlockV1::with_index_arguments(
                1,
                vec![],
                if advances {
                    ProductionRankedTerminatorV1::BranchArgsAdd {
                        value: body,
                        step: local(8),
                        target: 3,
                    }
                } else {
                    ProductionRankedTerminatorV1::BranchArgs {
                        arguments: vec![body],
                        target: 3,
                    }
                },
            ));
        kernel.blocks.push(preheader);
        let compiled = compile_fixture(kernel, 1, 3, &effect, subjects);
        if advances {
            let input = compiled.unwrap();
            let exported = input.export_live_source_reference_contracts_v1().unwrap();
            assert_eq!(exported.outputs()[0].effect(), &effect);
            assert!(
                input
                    .production_pipeline_report
                    .ownership()
                    .all_total_view_contracts_are_proved()
            );
        } else {
            assert!(
                compiled.is_err(),
                "a non-advancing loop must not prove total output"
            );
        }
    }
}

#[test]
fn unrepresented_cross_block_read_does_not_export() {
    let (mut kernel, effect, subjects) = guarded_kernel(7, false);
    let ProductionRankedTerminatorV1::IndexLessThan { true_block, .. } =
        &mut kernel.blocks[0].terminator
    else {
        panic!("entry guard")
    };
    *true_block = 3;
    kernel.blocks.push(ProductionRankedBlockV1::new(
        vec![O::Access {
            kind: AccessKindAttr::Read,
            view: local(0),
            indices: vec![local(1)],
        }],
        ProductionRankedTerminatorV1::Branch { target: 1 },
    ));
    let input = compile_fixture(kernel, 1, 3, &effect, subjects).unwrap();
    assert_eq!(
        input
            .export_live_source_reference_contracts_v1()
            .unwrap_err(),
        E::LiveMemoryRejected
    );
}

#[test]
fn live_guard_mutation_invalidates_epoch_and_complete_graph_replay() {
    for restore in [false, true] {
        let (kernel, effect, subjects) = guarded_kernel(7, false);
        let mut input = compile_fixture(kernel, 1, 3, &effect, subjects).unwrap();
        let context = &input._session.inner.context;
        let function = FuncOp::from_operation(input.source_contract_function.operation);
        let pointer = function
            .get_entry_block(context)
            .deref(context)
            .iter(context)
            .find(|op| Operation::is_op::<dialect_kernel::IndexConstantOp>(*op, context))
            .unwrap();
        let guard = dialect_kernel::IndexConstantOp::from_operation(pointer);
        assert_eq!(guard.value(context), Some(7));
        guard.set_attr_kernel_index_value(context, dialect_kernel::IndexValueAttr(6));
        if restore {
            guard.set_attr_kernel_index_value(context, dialect_kernel::IndexValueAttr(7));
        }
        assert_eq!(
            input
                .export_live_source_reference_contracts_v1()
                .unwrap_err(),
            E::LiveOwnerChanged
        );
        if !restore {
            // Test-only epoch substitution must still fail exact graph replay.
            input.source_contract_mutation_epoch =
                Some(context.ir_mutation_attempt_epoch().unwrap().value());
            assert_eq!(
                input
                    .export_live_source_reference_contracts_v1()
                    .unwrap_err(),
                E::LiveGraphChanged
            );
        }
    }
}

#[test]
fn guarded_graph_and_contract_changes_cannot_reuse_staging() {
    for change in 0..5 {
        let (kernel, effect, subjects) = guarded_kernel(7, false);
        let mut input = compile_fixture(kernel, 1, 3, &effect, subjects).unwrap();
        match change {
            0 => {
                input.kernel.blocks[1].terminator =
                    ProductionRankedTerminatorV1::Branch { target: 1 }
            }
            1 => {
                let O::ValueAccess { value, .. } = &mut input.kernel.blocks[1].operations[2] else {
                    panic!("write")
                };
                *value = local(6);
            }
            2 => {
                let O::ViewInSpace {
                    allocation_origin, ..
                } = &mut input.kernel.blocks[0].operations[1]
                else {
                    panic!("view")
                };
                *allocation_origin += 1;
            }
            3 => input.kernel.blocks[1].operations.swap(0, 1),
            _ => {
                let O::SemanticExpression { expression, .. } =
                    &mut input.kernel.blocks[1].operations[1]
                else {
                    panic!("CPU root")
                };
                *expression = f32_constant(2.0);
            }
        }
        assert!(
            input.export_live_source_reference_contracts_v1().is_err(),
            "change {change}"
        );
    }
}
