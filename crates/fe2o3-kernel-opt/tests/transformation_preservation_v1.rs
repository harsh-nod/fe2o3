use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, BinaryOp, BlockId, ComparePredicate, Constant, Function,
    FunctionId, Kernel, LaunchDomain, LaunchExtent, MemoryAccess, Module, Operation, OperationKind,
    ScalarType, Signature, Terminator, Type, ValueDef, ValueId, decode_module_v13, verify_module,
};
use fe2o3_kernel_opt::{
    CheckedModuleTransformRelationV1, ExecutionSensitiveMutationPolicyV1,
    KernelIrPlironOptimizationErrorV4, OperationCoordinateLineageV1, OptimizerAnalysisV1,
    ProductionTransformationDispositionV1, ProductionTransformationV1,
    TransformationPreservationErrorV1, TransformationPreservationModeV1,
    check_transformation_preservation_v1, execute_checked_canonical_transformation_v13_v1,
    optimize_production_kernel_ir_module_v4, production_policy_has_only_checked_transformations_v1,
    production_policy_transformations_v1, production_transformation_audit_v1,
};

const ADVANCED_FIXTURES: [(&str, &[u8]); 7] = [
    (
        "workgroup-barrier",
        include_bytes!("fixtures/transformation_preservation_v1/workgroup_barrier.bin"),
    ),
    (
        "atomic-fetch-add",
        include_bytes!("fixtures/transformation_preservation_v1/atomic_fetch_add.bin"),
    ),
    (
        "workgroup-collective",
        include_bytes!("fixtures/transformation_preservation_v1/workgroup_collective.bin"),
    ),
    (
        "matrix-access",
        include_bytes!("fixtures/transformation_preservation_v1/matrix_access.bin"),
    ),
    (
        "async-copy",
        include_bytes!("fixtures/transformation_preservation_v1/async_copy.bin"),
    ),
    (
        "dynamic-private-view",
        include_bytes!("fixtures/transformation_preservation_v1/dynamic_private_view.bin"),
    ),
    (
        "workgroup-view",
        include_bytes!("fixtures/transformation_preservation_v1/workgroup_view.bin"),
    ),
];

fn fixture(bytes: &[u8]) -> Module {
    let module = decode_module_v13(bytes).expect("checked-in fixture is exact KIR V13");
    verify_module(&module).expect("checked-in fixture remains valid");
    module
}

fn scalar_cse_pair() -> (Module, Module) {
    let ty = Type::Scalar(ScalarType::U32);
    let operation = |result| {
        Operation::effect_free(
            ValueDef::new(ValueId(result), ty.clone()),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        )
    };
    let mut before_block = BasicBlock::new(BlockId(0));
    before_block.operations = vec![operation(2), operation(3)];
    before_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(3)],
    });
    let mut before = Module::new("scalar-preservation");
    before.functions.push(Function::internal_helper(
        "scalar",
        Signature::new(vec![ty.clone(), ty.clone()], vec![ty.clone()]),
        vec![ValueId(0), ValueId(1)],
        vec![before_block],
    ));

    let mut after_block = BasicBlock::new(BlockId(0));
    after_block.operations = vec![operation(2)];
    after_block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut after = Module::new("scalar-preservation");
    after.functions.push(Function::internal_helper(
        "scalar",
        Signature::new(vec![ty.clone(), ty.clone()], vec![ty]),
        vec![ValueId(0), ValueId(1)],
        vec![after_block],
    ));
    (before, after)
}

fn private_memory_forwarding_module() -> Module {
    let scalar = Type::Scalar(ScalarType::U32);
    let private_pointer =
        Type::pointer(scalar.clone(), AddressSpace::Private, AccessMode::ReadWrite);
    let output_pointer = Type::pointer(scalar.clone(), AddressSpace::Global, AccessMode::WriteOnly);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(2), private_pointer),
            OperationKind::Alloca {
                element: scalar.clone(),
                count: None,
                address_space: AddressSpace::Private,
                alignment: 4,
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(2),
                value: ValueId(0),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), scalar.clone()),
            OperationKind::Load {
                pointer: ValueId(2),
                access: MemoryAccess::new(AddressSpace::Private, 4),
            },
        ),
        Operation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(1),
                value: ValueId(3),
                access: MemoryAccess::new(AddressSpace::Global, 4),
            },
        ),
    ]);
    entry.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("preserved-memory-forwarding");
    module.functions.push(Function::kernel_entry(
        "kernel_impl",
        Signature::new(vec![scalar, output_pointer], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![entry],
    ));
    module.kernels.push(Kernel::new(
        "kernel",
        "kernel_impl",
        LaunchDomain::D1 {
            x: LaunchExtent::Dynamic,
        },
    ));
    verify_module(&module).unwrap();
    module
}

fn guarded_dynamic_licm_module() -> Module {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            OperationKind::Constant(Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: ValueId(3),
                rhs: ValueId(2),
            },
        ),
    ]);
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(5),
        then_target: BlockId(4),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![ValueId(3)],
    });

    let mut preheader = BasicBlock::new(BlockId(4));
    preheader.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(3)],
    });
    let mut header = BasicBlock::new(BlockId(1));
    header.parameters = vec![ValueDef::new(ValueId(10), Type::INDEX)];
    header.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(11), Type::BOOL),
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: ValueId(10),
            rhs: ValueId(2),
        },
    ));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(11),
        then_target: BlockId(2),
        then_arguments: vec![],
        else_target: BlockId(3),
        else_arguments: vec![ValueId(10)],
    });
    let mut body = BasicBlock::new(BlockId(2));
    body.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(12), Type::BOOL),
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(13), Type::INDEX),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(10),
                rhs: ValueId(4),
            },
        ),
    ]);
    body.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(13)],
    });
    let mut exit = BasicBlock::new(BlockId(3));
    exit.parameters = vec![ValueDef::new(ValueId(20), Type::INDEX)];
    exit.terminator = Some(Terminator::Return {
        values: vec![ValueId(20)],
    });

    let scalar = Type::Scalar(ScalarType::U32);
    let mut module = Module::new("preserved-dynamic-licm");
    module.functions.push(Function::internal_helper(
        "dynamic_licm",
        Signature::new(vec![scalar.clone(), scalar, Type::INDEX], vec![Type::INDEX]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![entry, preheader, header, body, exit],
    ));
    verify_module(&module).unwrap();
    module
}

fn loop_helper() -> Function {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![ValueId(1)],
    });

    let mut loop_block = BasicBlock::new(BlockId(1));
    loop_block
        .parameters
        .push(ValueDef::new(ValueId(2), Type::INDEX));
    loop_block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(3), Type::INDEX),
            OperationKind::Constant(Constant::Index(1)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(4), Type::INDEX),
            OperationKind::Binary {
                op: BinaryOp::Add,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
    ]);
    loop_block.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(0),
        then_target: BlockId(1),
        then_arguments: vec![ValueId(4)],
        else_target: BlockId(2),
        else_arguments: vec![],
    });

    let mut tail = BasicBlock::new(BlockId(2));
    tail.terminator = Some(Terminator::Return { values: vec![] });
    Function::internal_helper(
        "loop_helper",
        Signature::new(vec![Type::BOOL, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![entry, loop_block, tail],
    )
}

fn root(id: &str) -> Function {
    let mut block = BasicBlock::new(BlockId(0));
    block.operations.extend([
        Operation::new(
            vec![],
            OperationKind::Call {
                callee: FunctionId::new("loop_helper"),
                arguments: vec![ValueId(0), ValueId(1)],
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Constant(Constant::Index(7)),
        ),
    ]);
    block.terminator = Some(Terminator::Return { values: vec![] });
    Function::kernel_entry(
        id,
        Signature::new(vec![Type::BOOL, Type::INDEX], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    )
}

fn multi_root_loop_tail_module() -> Module {
    let mut module = Module::new("multiple-roots-loops-and-tails");
    module
        .functions
        .extend([root("first_root"), root("second_root"), loop_helper()]);
    module.kernels.extend([
        Kernel::new(
            "first",
            "first_root",
            LaunchDomain::D1 {
                x: LaunchExtent::Dynamic,
            },
        ),
        Kernel::new(
            "second",
            "second_root",
            LaunchDomain::D2 {
                x: LaunchExtent::Dynamic,
                y: LaunchExtent::Static(4),
            },
        ),
    ]);
    verify_module(&module).expect("multiple-root loop fixture is valid");
    module
}

fn mutate_execution_source(module: &Module) -> Module {
    let mut candidate = module.clone();
    execution_contract_mut(&mut candidate).source.operation[0] ^= 0x5a;
    verify_module(&candidate).expect("source substitution remains structurally valid KIR V13");
    candidate
}

fn next_value_id(module: &Module) -> ValueId {
    let maximum = module
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref())
        .flat_map(|body| {
            body.parameters
                .iter()
                .copied()
                .chain(body.blocks.iter().flat_map(|block| {
                    block.parameters.iter().map(|value| value.id).chain(
                        block
                            .operations
                            .iter()
                            .flat_map(|operation| operation.results.iter().map(|value| value.id)),
                    )
                }))
        })
        .map(|value| value.0)
        .max()
        .unwrap_or(0);
    ValueId(
        maximum
            .checked_add(1)
            .expect("fixture value identity space"),
    )
}

fn with_independent_scalar_before_execution(module: &Module) -> (Module, Module) {
    let mut before = module.clone();
    let anchor = next_value_id(&before);
    let (function_index, block_index, operation_index) = before
        .functions
        .iter()
        .enumerate()
        .filter_map(|(function_index, function)| {
            function.body.as_ref().map(|body| (function_index, body))
        })
        .flat_map(|(function_index, body)| {
            body.blocks
                .iter()
                .enumerate()
                .map(move |(block_index, block)| (function_index, block_index, block))
        })
        .find_map(|(function_index, block_index, block)| {
            block
                .operations
                .iter()
                .position(|operation| {
                    matches!(operation.kind, OperationKind::ExecutionCapability(_))
                })
                .map(|operation_index| (function_index, block_index, operation_index))
        })
        .expect("advanced fixture has an execution capability operation");
    let operations = &mut before.functions[function_index]
        .body
        .as_mut()
        .unwrap()
        .blocks[block_index]
        .operations;
    operations.insert(
        operation_index,
        Operation::effect_free(
            ValueDef::new(anchor, Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(0)),
        ),
    );
    verify_module(&before).expect("independent scalar anchor preserves valid KIR");
    let mut reordered = before.clone();
    reordered.functions[function_index]
        .body
        .as_mut()
        .unwrap()
        .blocks[block_index]
        .operations
        .swap(operation_index, operation_index + 1);
    verify_module(&reordered).expect("independent scalar reorder preserves valid KIR");
    (before, reordered)
}

fn append_canonicalizable_scalar(module: &Module) -> Module {
    let mut module = module.clone();
    let source = next_value_id(&module);
    let result = ValueId(source.0.checked_add(1).unwrap());
    let block = module
        .functions
        .iter_mut()
        .filter_map(|function| function.body.as_mut())
        .flat_map(|body| &mut body.blocks)
        .find(|block| {
            block
                .operations
                .iter()
                .any(|operation| matches!(operation.kind, OperationKind::ExecutionCapability(_)))
        })
        .expect("advanced fixture has an execution capability operation");
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(source, Type::Scalar(ScalarType::U32)),
            OperationKind::Constant(Constant::U32(17)),
        ),
        Operation::effect_free(
            ValueDef::new(result, Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: source,
                rhs: source,
            },
        ),
    ]);
    verify_module(&module).expect("trailing scalar identity preserves valid KIR");
    module
}

fn execution_contract_mut(module: &mut Module) -> &mut fe2o3_kernel_ir::ExecutionCapabilityOpV1 {
    module
        .functions
        .iter_mut()
        .filter_map(|function| function.body.as_mut())
        .flat_map(|body| &mut body.blocks)
        .flat_map(|block| &mut block.operations)
        .rev()
        .find_map(|operation| match &mut operation.kind {
            OperationKind::ExecutionCapability(contract) => Some(contract),
            _ => None,
        })
        .expect("advanced fixture contains an execution-capability operation")
}

#[test]
fn production_transform_audit_is_closed_and_fail_closed_for_remaining_families() {
    let audit = production_transformation_audit_v1();
    assert_eq!(audit.len(), 19);
    assert_eq!(
        audit
            .iter()
            .filter(|entry| {
                entry.disposition()
                    == ProductionTransformationDispositionV1::CheckedCanonicalV13Path
            })
            .count(),
        18
    );
    assert_eq!(
        audit
            .iter()
            .filter(|entry| {
                entry.disposition() == ProductionTransformationDispositionV1::UnavailableFailClosed
            })
            .count(),
        1
    );
    assert!(audit.iter().all(|entry| {
        entry.execution_sensitive_mutation() == match entry.disposition() {
            ProductionTransformationDispositionV1::CheckedCanonicalV13Path => {
                ExecutionSensitiveMutationPolicyV1::ExactProtectedStructureAndAffectedAnalysisReplay
            }
            ProductionTransformationDispositionV1::UnavailableFailClosed => {
                ExecutionSensitiveMutationPolicyV1::TransformUnavailable
            }
        }
    }));

    let module = multi_root_loop_tail_module();
    assert!(matches!(
        check_transformation_preservation_v1(
            ProductionTransformationV1::InstructionScheduling,
            &module,
            &module,
            0,
            0,
        ),
        Err(TransformationPreservationErrorV1::UnavailableFailClosed {
            transformation: ProductionTransformationV1::InstructionScheduling,
        })
    ));

    assert!(production_policy_has_only_checked_transformations_v1());
    let selected = production_policy_transformations_v1();
    assert_eq!(selected.len(), 7);
    assert!(selected.iter().all(|transformation| {
        audit.iter().any(|entry| {
            entry.transformation() == *transformation
                && entry.disposition()
                    == ProductionTransformationDispositionV1::CheckedCanonicalV13Path
        })
    }));
    assert!(audit.iter().all(|entry| {
        entry.disposition() != ProductionTransformationDispositionV1::UnavailableFailClosed
            || !selected.contains(&entry.transformation())
    }));
}

#[test]
fn scalar_mutation_records_exact_invalidation_and_fresh_replay_requirements() {
    let (before, after) = scalar_cse_pair();
    let record = check_transformation_preservation_v1(
        ProductionTransformationV1::LocalPureCommonSubexpressionElimination,
        &before,
        &after,
        19,
        20,
    )
    .unwrap();
    assert!(record.changed());
    assert!(!record.grants_semantic_preservation_authority());
    assert!(record.requires_complete_final_graph_analysis_replay());
    assert_eq!(
        record.mode(),
        TransformationPreservationModeV1::ExactProtectedStructureReplay
    );
    assert_eq!(record.fresh_output_analysis().graph_epoch(), 20);
    assert_eq!(
        record.fresh_output_analysis().graph_identity(),
        record.output_identity()
    );
    assert!(
        record
            .invalidated_analyses()
            .contains(&OptimizerAnalysisV1::UniformityAndConvergence)
    );
    assert!(
        record
            .completed_replays()
            .contains(&OptimizerAnalysisV1::CapabilityPreservation)
    );
    assert!(
        record
            .required_replays()
            .contains(&OptimizerAnalysisV1::SourceOperationRefinement)
    );
    assert!(
        record
            .required_replays()
            .contains(&OptimizerAnalysisV1::ArtifactAndLaunch)
    );

    assert!(matches!(
        check_transformation_preservation_v1(
            ProductionTransformationV1::LocalPureCommonSubexpressionElimination,
            &before,
            &after,
            19,
            19,
        ),
        Err(TransformationPreservationErrorV1::OutputEpochMismatch { .. })
    ));
}

#[test]
fn unchanged_advanced_v13_graphs_receive_exact_identity_records() {
    for (name, bytes) in ADVANCED_FIXTURES {
        let module = fixture(bytes);
        let record = check_transformation_preservation_v1(
            ProductionTransformationV1::DeadCodeElimination,
            &module,
            &module,
            41,
            41,
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(
            record.mode(),
            TransformationPreservationModeV1::ExactCanonicalIdentity,
            "{name}"
        );
        assert!(!record.changed(), "{name}");
        assert!(record.invalidated_analyses().is_empty(), "{name}");
        assert!(record.required_replays().is_empty(), "{name}");

        let optimized = optimize_production_kernel_ir_module_v4(&module);
        if name == "atomic-fetch-add" {
            assert!(matches!(
                optimized,
                Err(KernelIrPlironOptimizationErrorV4::Import(_))
            ));
            continue;
        }
        let optimized = optimized.unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(optimized.module(), &module, "{name}");
        assert!(
            optimized
                .report()
                .capability_pass_replays()
                .iter()
                .all(|pass| pass.preservation().mode()
                    == TransformationPreservationModeV1::ExactCanonicalIdentity)
        );
    }
}

#[test]
fn source_identity_mutation_is_rejected_for_every_advanced_operation_family() {
    for (name, bytes) in ADVANCED_FIXTURES {
        let before = fixture(bytes);
        let after = mutate_execution_source(&before);
        assert!(
            matches!(
                check_transformation_preservation_v1(
                    ProductionTransformationV1::DeadCodeElimination,
                    &before,
                    &after,
                    7,
                    8,
                ),
                Err(TransformationPreservationErrorV1::CapabilityReplay(_))
            ),
            "{name}"
        );
    }
}

#[test]
fn provenance_brand_epoch_extent_layout_and_requirement_mutations_fail_closed() {
    let reject = |before: &Module, after: &Module, case: &str| {
        assert!(
            check_transformation_preservation_v1(
                ProductionTransformationV1::DeadCodeElimination,
                before,
                after,
                13,
                14,
            )
            .is_err(),
            "{case}"
        );
    };

    let barrier = fixture(ADVANCED_FIXTURES[0].1);
    let mut provenance = barrier.clone();
    execution_contract_mut(&mut provenance)
        .provenance
        .frontend_unit[0] ^= 0x11;
    reject(&barrier, &provenance, "provenance");

    let mut brand = barrier.clone();
    execution_contract_mut(&mut brand)
        .workgroup_brand
        .as_mut()
        .unwrap()[0] ^= 0x22;
    reject(&barrier, &brand, "workgroup brand");

    let mut epoch = barrier.clone();
    execution_contract_mut(&mut epoch)
        .epoch_before
        .as_mut()
        .unwrap()[0] ^= 0x33;
    reject(&barrier, &epoch, "workgroup epoch");

    let dynamic = fixture(ADVANCED_FIXTURES[5].1);
    let mut extent = dynamic.clone();
    match &mut execution_contract_mut(&mut extent).operation {
        fe2o3_kernel_ir::ExecutionCapabilityOperationV1::RawMemoryBind { extent, .. } => {
            extent.upper_bound += 1;
        }
        operation => panic!("expected dynamic raw view, found {operation:?}"),
    }
    reject(&dynamic, &extent, "dynamic extent");

    let workgroup = fixture(ADVANCED_FIXTURES[6].1);
    let mut layout = workgroup.clone();
    match &mut execution_contract_mut(&mut layout).operation {
        fe2o3_kernel_ir::ExecutionCapabilityOperationV1::WorkgroupMemoryAllocate {
            layout, ..
        } => layout.byte_alignment *= 2,
        operation => panic!("expected workgroup view, found {operation:?}"),
    }
    reject(&workgroup, &layout, "view layout");

    let mut requirements = barrier.clone();
    requirements.required_capabilities.clear();
    reject(&barrier, &requirements, "target requirement closure");
}

#[test]
fn advanced_capability_fixtures_reject_drop_reorder_and_substitution() {
    for (name, bytes) in ADVANCED_FIXTURES {
        let original = fixture(bytes);
        let (before, reordered) = with_independent_scalar_before_execution(&original);
        assert!(
            check_transformation_preservation_v1(
                ProductionTransformationV1::GeneralTargetIndependentCanonicalization,
                &before,
                &reordered,
                3,
                4,
            )
            .is_err(),
            "{name}: reordered capability operation was admitted"
        );

        let mut dropped = before.clone();
        let (operations, position) = dropped
            .functions
            .iter_mut()
            .filter_map(|function| function.body.as_mut())
            .flat_map(|body| &mut body.blocks)
            .find_map(|block| {
                block
                    .operations
                    .iter()
                    .position(|operation| {
                        matches!(operation.kind, OperationKind::ExecutionCapability(_))
                    })
                    .map(|position| (&mut block.operations, position))
            })
            .expect("advanced fixture has an execution capability operation");
        operations.remove(position);
        assert!(
            check_transformation_preservation_v1(
                ProductionTransformationV1::GeneralTargetIndependentCanonicalization,
                &before,
                &dropped,
                3,
                4,
            )
            .is_err(),
            "{name}: dropped capability operation was admitted"
        );

        let substituted = mutate_execution_source(&original);
        assert!(
            check_transformation_preservation_v1(
                ProductionTransformationV1::GlobalValueNumbering,
                &original,
                &substituted,
                3,
                4,
            )
            .is_err(),
            "{name}: substituted capability operation was admitted"
        );
    }
}

#[test]
fn new_transforms_mutate_real_v13_and_bind_exact_epochs() {
    let advanced = append_canonicalizable_scalar(&fixture(ADVANCED_FIXTURES[0].1));
    let canonicalized = execute_checked_canonical_transformation_v13_v1(
        ProductionTransformationV1::GeneralTargetIndependentCanonicalization,
        &advanced,
        29,
    )
    .unwrap();
    assert!(canonicalized.preservation().changed());
    assert_eq!(canonicalized.preservation().input_epoch(), 29);
    assert_eq!(canonicalized.preservation().output_epoch(), 30);
    assert_eq!(
        canonicalized.preservation().output_identity(),
        canonicalized.canonical().identity()
    );
    assert!(
        canonicalized
            .preservation()
            .requires_complete_final_graph_analysis_replay()
    );
    assert!(!canonicalized.grants_semantic_preservation_authority());

    let (before, _) = scalar_cse_pair();
    let numbered = execute_checked_canonical_transformation_v13_v1(
        ProductionTransformationV1::GlobalValueNumbering,
        &before,
        91,
    )
    .unwrap();
    assert!(numbered.preservation().changed());
    assert_eq!(numbered.preservation().input_epoch(), 91);
    assert_eq!(numbered.preservation().output_epoch(), 92);
}

#[test]
fn checked_v13_transforms_are_deterministic_across_fresh_sessions() {
    let input = append_canonicalizable_scalar(&fixture(ADVANCED_FIXTURES[0].1));
    let first = execute_checked_canonical_transformation_v13_v1(
        ProductionTransformationV1::GeneralTargetIndependentCanonicalization,
        &input,
        11,
    )
    .unwrap();
    for _ in 0..8 {
        let repeated = execute_checked_canonical_transformation_v13_v1(
            ProductionTransformationV1::GeneralTargetIndependentCanonicalization,
            &input,
            11,
        )
        .unwrap();
        assert_eq!(repeated.canonical(), first.canonical());
        assert_eq!(repeated.preservation(), first.preservation());
    }
}

#[test]
fn whole_module_transforms_preserve_advanced_capabilities_or_fail_closed() {
    let transformations = [
        ProductionTransformationV1::ScalarReplacementOfAggregates,
        ProductionTransformationV1::SecondarySsaPromotion,
        ProductionTransformationV1::HelperInlining,
        ProductionTransformationV1::InterproceduralCleanup,
        ProductionTransformationV1::LoopCanonicalization,
        ProductionTransformationV1::InductionVariableSimplification,
        ProductionTransformationV1::LoopInvariantCodeMotion,
        ProductionTransformationV1::MemoryEffectVersioning,
        ProductionTransformationV1::MemorySimplification,
        ProductionTransformationV1::FullLoopUnrolling,
        ProductionTransformationV1::PartialLoopUnrolling,
    ];
    for (name, bytes) in ADVANCED_FIXTURES {
        let input = fixture(bytes);
        for transformation in transformations {
            let output =
                execute_checked_canonical_transformation_v13_v1(transformation, &input, 73)
                    .unwrap_or_else(|error| panic!("{name}/{}: {error}", transformation.name()));
            assert_eq!(
                output.preservation().input_epoch(),
                73,
                "{name}/{}",
                transformation.name()
            );
            assert!(
                !output.grants_semantic_preservation_authority(),
                "{name}/{}",
                transformation.name()
            );

            let substituted = mutate_execution_source(&input);
            assert!(
                check_transformation_preservation_v1(transformation, &input, &substituted, 73, 74,)
                    .is_err(),
                "{name}/{} admitted a source substitution",
                transformation.name()
            );
        }
    }
}

#[test]
fn memory_transform_relation_and_coordinate_lineage_are_replayed_in_preservation() {
    let input = private_memory_forwarding_module();
    let output = execute_checked_canonical_transformation_v13_v1(
        ProductionTransformationV1::MemorySimplification,
        &input,
        100,
    )
    .unwrap();
    let preservation = output.preservation();
    assert!(preservation.changed());
    assert_eq!(preservation.output_epoch(), 101);
    let Some(CheckedModuleTransformRelationV1::LoopMemory(report)) = preservation.module_relation()
    else {
        panic!("expected an independently reconstructed loop/memory relation")
    };
    assert!(report.changed());
    assert!(
        report
            .coordinate_lineage()
            .iter()
            .any(|lineage| matches!(lineage, OperationCoordinateLineageV1::Eliminated { .. }))
    );
    assert!(
        preservation
            .required_replays()
            .contains(&OptimizerAnalysisV1::ProvenanceAliasAndEffects)
    );
    assert!(
        preservation
            .required_replays()
            .contains(&OptimizerAnalysisV1::MemoryInitializationAndVersions)
    );
    assert!(
        preservation
            .required_replays()
            .contains(&OptimizerAnalysisV1::SourceOperationRefinement)
    );
    assert!(!preservation.grants_semantic_preservation_authority());

    let mut unrelated = output.module().clone();
    unrelated.functions[0].body.as_mut().unwrap().blocks[0]
        .operations
        .insert(
            0,
            Operation::effect_free(
                ValueDef::new(ValueId(4), Type::Scalar(ScalarType::U32)),
                OperationKind::Constant(Constant::U32(0)),
            ),
        );
    verify_module(&unrelated).unwrap();
    assert!(matches!(
        check_transformation_preservation_v1(
            ProductionTransformationV1::MemorySimplification,
            &input,
            &unrelated,
            100,
            101,
        ),
        Err(TransformationPreservationErrorV1::LoopMemoryTransform(_))
    ));
}

#[test]
fn dynamic_licm_range_proof_is_replayed_at_the_production_epoch() {
    let input = guarded_dynamic_licm_module();
    let output = execute_checked_canonical_transformation_v13_v1(
        ProductionTransformationV1::LoopInvariantCodeMotion,
        &input,
        55,
    )
    .unwrap();
    let preservation = output.preservation();
    assert!(preservation.changed());
    let [query] = preservation.optimizer_query_replays() else {
        panic!("expected one independently replayed dynamic-trip query")
    };
    assert_eq!(query.graph_epoch(), 55);
    assert_eq!(query.graph_identity(), preservation.input_identity());
    assert!(!query.grants_semantic_authority());
    assert!(!preservation.grants_semantic_preservation_authority());
}

#[test]
fn production_optimizer_records_identity_derived_epochs_and_fresh_analyses() {
    let (before, _) = scalar_cse_pair();
    let optimized = optimize_production_kernel_ir_module_v4(&before).unwrap();
    let mut epoch = optimized.report().initial_epoch();
    let mut identity = optimized.report().input_identity();
    let mut observed_mutation = false;
    for pass in optimized.report().capability_pass_replays() {
        let record = pass.preservation();
        assert_eq!(record.input_epoch(), epoch);
        assert_eq!(record.input_identity(), identity);
        assert_eq!(
            record.fresh_output_analysis().graph_epoch(),
            record.output_epoch()
        );
        assert_eq!(
            record.fresh_output_analysis().graph_identity(),
            record.output_identity()
        );
        if record.changed() {
            observed_mutation = true;
            assert_eq!(record.output_epoch(), epoch + 1);
            assert!(!record.invalidated_analyses().is_empty());
        } else {
            assert_eq!(record.output_epoch(), epoch);
            assert!(record.invalidated_analyses().is_empty());
        }
        epoch = record.output_epoch();
        identity = record.output_identity();
    }
    assert!(observed_mutation);
    assert_eq!(epoch, optimized.report().final_epoch());
    assert_eq!(identity, optimized.report().output_identity());
}
