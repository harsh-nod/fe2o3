use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, BasicBlock, DebugSourceMapBindingV1, DebugSourceMapDocumentV2,
    DebugSourceMapFileV1, DebugSourceMapKirSiteV1, DebugSourceMapSiteV1, DebugSourceMapSpanV1,
    DebugSourceScopeV2, DebugSourceVariableBindingV2, DebugSourceVariableFallbackV2,
    DebugSourceVariableLocationV2, DebugSourceVariableV2, Function, Kernel, LaunchDomain,
    LaunchExtent, MemoryAccess, Module, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, ValueDef, ValueId, VerifiedCanonicalKernelIrV13, verify_module,
};
use fe2o3_kernel_opt::{
    LoopMemoryTransformLimitsV1, LoopMemoryTransformV1, ProductionTransformationV1,
    SourceCoordinateLineageErrorV1, SourceCoordinateLineageLimitsV1,
    apply_checked_loop_source_coordinate_lineage_v1,
    execute_checked_loop_transformation_with_source_lineage_v1, execute_loop_memory_transform_v1,
};

fn module() -> Module {
    let private = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Private,
        AccessMode::ReadWrite,
    );
    let output = Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::WriteOnly,
    );
    let mut block = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    block.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(2), private),
            OperationKind::Alloca {
                element: Type::Scalar(ScalarType::U32),
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
            ValueDef::new(ValueId(3), Type::Scalar(ScalarType::U32)),
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
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("source-coordinate-lineage-v1");
    module.functions.push(Function::kernel_entry(
        "kernel_impl",
        Signature::new(vec![Type::Scalar(ScalarType::U32), output], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
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

fn source_map(module: &Module, operation_variable: bool) -> DebugSourceMapDocumentV2 {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let file = DebugSourceMapFileV1::new([3; 32], 1_000_000, "/src/kernel.rs".into()).unwrap();
    let sites = module
        .functions
        .iter()
        .enumerate()
        .flat_map(|(function, definition)| {
            definition.body.as_ref().into_iter().flat_map(move |body| {
                body.blocks
                    .iter()
                    .enumerate()
                    .flat_map(move |(block, block_value)| {
                        block_value
                            .operations
                            .iter()
                            .enumerate()
                            .map(move |(operation, _)| {
                                let ordinal =
                                    u64::try_from(function * 1_024 + block * 256 + operation)
                                        .unwrap();
                                let start = 4 + ordinal * 8;
                                DebugSourceMapSiteV1::new(
                                    DebugSourceMapKirSiteV1::operation(
                                        u64::try_from(function).unwrap(),
                                        u64::try_from(block).unwrap(),
                                        u64::try_from(operation).unwrap(),
                                    ),
                                    vec![
                                        DebugSourceMapSpanV1::new([3; 32], start, start + 4, 2, 3)
                                            .unwrap(),
                                    ],
                                )
                                .unwrap()
                            })
                    })
            })
        })
        .collect();
    let (scopes, variables) = if operation_variable {
        let span = DebugSourceMapSpanV1::new([3; 32], 0, 4, 1, 1).unwrap();
        let scope = DebugSourceScopeV2::new([4; 32], 0, None, 0, span).unwrap();
        let variable = DebugSourceVariableV2::new(
            [5; 32],
            "value".into(),
            0,
            [4; 32],
            DebugSourceVariableFallbackV2::OptimizedOut,
            vec![
                DebugSourceVariableLocationV2::new(
                    0,
                    0,
                    1,
                    DebugSourceVariableBindingV2::Captured { value_ordinal: 0 },
                )
                .unwrap(),
            ],
        )
        .unwrap();
        (vec![scope], vec![variable])
    } else {
        (Vec::new(), Vec::new())
    };
    DebugSourceMapDocumentV2::new(
        DebugSourceMapBindingV1::new(
            [1; 32],
            *canonical.identity().digest(),
            canonical.identity().canonical_length(),
        )
        .unwrap(),
        vec![file],
        sites,
        Vec::new(),
        scopes,
        variables,
    )
    .unwrap()
}

fn output_binding(module: &Module) -> DebugSourceMapBindingV1 {
    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    DebugSourceMapBindingV1::new(
        [2; 32],
        *canonical.identity().digest(),
        canonical.identity().canonical_length(),
    )
    .unwrap()
}

fn static_loop_module() -> Module {
    let mut entry = BasicBlock::new(fe2o3_kernel_ir::BlockId(0));
    entry.operations.extend([
        Operation::effect_free(
            ValueDef::new(ValueId(0), Type::INDEX),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::Index(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(1), Type::INDEX),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::Index(1)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Constant(fe2o3_kernel_ir::Constant::Index(3)),
        ),
    ]);
    entry.terminator = Some(Terminator::Branch {
        target: fe2o3_kernel_ir::BlockId(1),
        arguments: vec![ValueId(0)],
    });
    let mut header = BasicBlock::new(fe2o3_kernel_ir::BlockId(1));
    header
        .parameters
        .push(ValueDef::new(ValueId(3), Type::INDEX));
    header.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(4), Type::BOOL),
        OperationKind::Compare {
            predicate: fe2o3_kernel_ir::ComparePredicate::LessThan,
            lhs: ValueId(3),
            rhs: ValueId(2),
        },
    ));
    header.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(4),
        then_target: fe2o3_kernel_ir::BlockId(2),
        then_arguments: vec![],
        else_target: fe2o3_kernel_ir::BlockId(3),
        else_arguments: vec![],
    });
    let mut loop_body = BasicBlock::new(fe2o3_kernel_ir::BlockId(2));
    loop_body.operations.push(Operation::effect_free(
        ValueDef::new(ValueId(5), Type::INDEX),
        OperationKind::Binary {
            op: fe2o3_kernel_ir::BinaryOp::Add,
            lhs: ValueId(3),
            rhs: ValueId(1),
        },
    ));
    loop_body.terminator = Some(Terminator::Branch {
        target: fe2o3_kernel_ir::BlockId(1),
        arguments: vec![ValueId(5)],
    });
    let mut exit = BasicBlock::new(fe2o3_kernel_ir::BlockId(3));
    exit.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("source-coordinate-loop-lineage-v1");
    module.functions.push(Function::kernel_entry(
        "kernel_impl",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry, header, loop_body, exit],
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

#[test]
fn checked_lineage_rebinds_exact_output_and_accounts_for_elimination() {
    let input = module();
    let map = source_map(&input, false);
    let (output, _) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    let checked = apply_checked_loop_source_coordinate_lineage_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &input,
        &output,
        &map,
        output_binding(&output),
        LoopMemoryTransformLimitsV1::default(),
        SourceCoordinateLineageLimitsV1::default(),
    )
    .unwrap();
    let output_identity = VerifiedCanonicalKernelIrV13::from_module(output.clone()).unwrap();
    assert_eq!(
        checked.document().binding().canonical_kir().digest(),
        *output_identity.identity().digest()
    );
    assert_eq!(checked.document().sites().len(), 3);
    assert_eq!(checked.document().eliminated().len(), 1);
    assert_eq!(checked.mapped_coordinates(), 3);
    assert_eq!(checked.eliminated_coordinates(), 1);
    assert!(!checked.grants_semantic_authority());

    let transaction = execute_checked_loop_transformation_with_source_lineage_v1(
        ProductionTransformationV1::MemorySimplification,
        &input,
        7,
        &map,
        output_binding(&output),
        SourceCoordinateLineageLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        transaction.transformation().preservation().output_epoch(),
        8
    );
    assert_eq!(
        transaction.source_lineage().output_identity(),
        transaction.transformation().canonical().identity()
    );
    assert!(!transaction.grants_semantic_authority());
}

#[test]
fn checked_lineage_maps_full_unroll_duplication() {
    let input = static_loop_module();
    let map = source_map(&input, false);
    let (output, report) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::FullLoopUnrolling,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    assert!(report.changed());
    let checked = apply_checked_loop_source_coordinate_lineage_v1(
        LoopMemoryTransformV1::FullLoopUnrolling,
        &input,
        &output,
        &map,
        output_binding(&output),
        LoopMemoryTransformLimitsV1::default(),
        SourceCoordinateLineageLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(
        checked.document().sites().len(),
        output.functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .len()
    );
    assert!(checked.mapped_coordinates() > map.sites().len());
}

#[test]
fn lineage_fails_closed_on_binding_resources_and_operation_variable_state() {
    let input = module();
    let (output, _) = execute_loop_memory_transform_v1(
        LoopMemoryTransformV1::MemorySimplification,
        &input,
        LoopMemoryTransformLimitsV1::default(),
    )
    .unwrap();
    let mut wrong = source_map(&input, false).to_canonical_json_bytes().unwrap();
    let input_digest = input_binding_digest_hex(&input);
    let position = wrong
        .windows(64)
        .position(|bytes| bytes == input_digest.as_bytes())
        .unwrap();
    wrong[position] = if wrong[position] == b'a' { b'b' } else { b'a' };
    let wrong = DebugSourceMapDocumentV2::from_canonical_json_bytes(&wrong).unwrap();
    assert!(matches!(
        apply_checked_loop_source_coordinate_lineage_v1(
            LoopMemoryTransformV1::MemorySimplification,
            &input,
            &output,
            &wrong,
            output_binding(&output),
            LoopMemoryTransformLimitsV1::default(),
            SourceCoordinateLineageLimitsV1::default(),
        ),
        Err(SourceCoordinateLineageErrorV1::InputBindingMismatch)
    ));

    assert!(matches!(
        apply_checked_loop_source_coordinate_lineage_v1(
            LoopMemoryTransformV1::MemorySimplification,
            &input,
            &output,
            &source_map(&input, false),
            output_binding(&input),
            LoopMemoryTransformLimitsV1::default(),
            SourceCoordinateLineageLimitsV1::default(),
        ),
        Err(SourceCoordinateLineageErrorV1::OutputBindingMismatch)
    ));

    let limits = SourceCoordinateLineageLimitsV1::new(1).unwrap();
    assert!(matches!(
        apply_checked_loop_source_coordinate_lineage_v1(
            LoopMemoryTransformV1::MemorySimplification,
            &input,
            &output,
            &source_map(&input, false),
            output_binding(&output),
            LoopMemoryTransformLimitsV1::default(),
            limits,
        ),
        Err(SourceCoordinateLineageErrorV1::ResourceLimit { .. })
    ));
    assert!(matches!(
        apply_checked_loop_source_coordinate_lineage_v1(
            LoopMemoryTransformV1::MemorySimplification,
            &input,
            &output,
            &source_map(&input, true),
            output_binding(&output),
            LoopMemoryTransformLimitsV1::default(),
            SourceCoordinateLineageLimitsV1::default(),
        ),
        Err(SourceCoordinateLineageErrorV1::OperationVariableLineageUnsupported)
    ));
}

fn input_binding_digest_hex(module: &Module) -> String {
    VerifiedCanonicalKernelIrV13::from_module(module.clone())
        .unwrap()
        .identity()
        .digest()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
