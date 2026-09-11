use fe2o3_kernel_analysis::{Diagnostic, Variation, analyze_function};
use fe2o3_kernel_ir::{
    AddressSpace, Axis, Barrier, BarrierSemantics, BasicBlock, BlockId, CastKind, ComparePredicate,
    Constant, Function, IndexKind, IntrinsicKind, IntrinsicOperation, MemoryOrdering, Module,
    Operation, OperationKind, ScalarType, Signature, SynchronizationScope, Terminator, Type,
    ValueDef, ValueId, WaveOperation, WaveOperationKind, WaveWidth, verify_module_ref,
};

fn wave(id: u32, kind: WaveOperationKind, width: WaveWidth) -> Operation {
    Operation::effect_free(
        ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
        OperationKind::Wave(WaveOperation::full(kind, width)),
    )
}

fn returning(id: u32) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

fn shuffle_module(
    width: WaveWidth,
    tile_width: u32,
    value: ValueId,
    source_lane: ValueId,
) -> Module {
    let u32_ty = Type::Scalar(ScalarType::U32);
    let i32_ty = Type::Scalar(ScalarType::I32);
    let mut entry = returning(0);
    entry.operations = vec![
        wave(0, WaveOperationKind::LaneId, width),
        Operation::effect_free(
            ValueDef::new(ValueId(1), u32_ty.clone()),
            OperationKind::Constant(Constant::U32(0)),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), Type::INDEX),
            OperationKind::Intrinsic(IntrinsicOperation::new(
                IntrinsicKind::InvocationIndex {
                    kind: IndexKind::Workgroup,
                    axis: Axis::X,
                },
                Type::INDEX,
            )),
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(8), Type::Scalar(ScalarType::U64)),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(2),
                to: Type::Scalar(ScalarType::U64),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(3), u32_ty.clone()),
            OperationKind::Cast {
                kind: CastKind::Truncate,
                value: ValueId(8),
                to: u32_ty.clone(),
            },
        ),
        wave(
            4,
            WaveOperationKind::ShuffleIndex {
                value: ValueId(0),
                source_lane: ValueId(1),
                tile_width: width.lanes(),
            },
            width,
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(5), i32_ty.clone()),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(0),
                to: i32_ty.clone(),
            },
        ),
        Operation::effect_free(
            ValueDef::new(
                ValueId(6),
                if value == ValueId(5) { i32_ty } else { u32_ty },
            ),
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::ShuffleIndex {
                    value,
                    source_lane,
                    tile_width,
                },
                width,
            )),
        ),
    ];
    let mut module = Module::new("shuffle_uniformity");
    module.functions.push(Function::definition(
        "shuffle",
        Signature::new(vec![], vec![]),
        vec![],
        vec![entry],
    ));
    module
}

#[test]
fn partial_tiles_do_not_claim_physical_subgroup_uniformity() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for value in [ValueId(0), ValueId(5)] {
            let mut tile_width = 1;
            while tile_width < width.lanes() {
                let module = shuffle_module(width, tile_width, value, ValueId(1));
                verify_module_ref(&module).unwrap();
                let report = analyze_function(&module.functions[0]);
                assert_eq!(report.value(value), Variation::Varying);
                assert_eq!(report.value(ValueId(6)), Variation::Varying);
                assert!(report.diagnostics().is_empty());
                tile_width *= 2;
            }
        }
    }
}

#[test]
fn full_width_uniform_source_promotes_only_to_subgroup() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for value in [ValueId(0), ValueId(5)] {
            let module = shuffle_module(width, width.lanes(), value, ValueId(1));
            verify_module_ref(&module).unwrap();
            let report = analyze_function(&module.functions[0]);
            assert_eq!(report.value(value), Variation::Varying);
            assert_eq!(report.value(ValueId(6)), Variation::SubgroupUniform);
            assert!(
                !report
                    .value(ValueId(6))
                    .is_uniform_for(SynchronizationScope::Workgroup)
            );
            assert!(report.diagnostics().is_empty());
        }
    }
}

#[test]
fn varying_source_keeps_varying_input_varying() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for value in [ValueId(0), ValueId(5)] {
            let module = shuffle_module(width, width.lanes(), value, ValueId(0));
            verify_module_ref(&module).unwrap();
            let report = analyze_function(&module.functions[0]);
            assert_eq!(report.value(ValueId(0)), Variation::Varying);
            assert_eq!(report.value(ValueId(6)), Variation::Varying);
            assert!(report.diagnostics().is_empty());
        }
    }
}

#[test]
fn uniform_inputs_preserve_all_lattice_levels_across_tiles() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for (value, expected) in [
            (ValueId(1), Variation::GridUniform),
            (ValueId(3), Variation::WorkgroupUniform),
            (ValueId(4), Variation::SubgroupUniform),
        ] {
            let mut tile_width = 1;
            while tile_width <= width.lanes() {
                let module = shuffle_module(width, tile_width, value, ValueId(1));
                verify_module_ref(&module).unwrap();
                let report = analyze_function(&module.functions[0]);
                assert_eq!(report.value(value), expected);
                assert_eq!(report.value(ValueId(6)), expected);
                assert!(report.diagnostics().is_empty());
                tile_width *= 2;
            }
            let module = shuffle_module(width, width.lanes(), value, ValueId(0));
            verify_module_ref(&module).unwrap();
            let report = analyze_function(&module.functions[0]);
            assert_eq!(report.value(ValueId(6)), expected);
        }
    }
}

#[test]
fn partial_tile_control_rejects_subgroup_barrier() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for tile_width in [1, width.lanes() / 2, width.lanes()] {
            let mut module = shuffle_module(width, tile_width, ValueId(0), ValueId(1));
            let body = module.functions[0].body.as_mut().unwrap();
            body.blocks[0].operations.push(Operation::effect_free(
                ValueDef::new(ValueId(7), Type::BOOL),
                OperationKind::Compare {
                    predicate: ComparePredicate::Equal,
                    lhs: ValueId(6),
                    rhs: ValueId(1),
                },
            ));
            body.blocks[0].terminator = Some(Terminator::ConditionalBranch {
                condition: ValueId(7),
                then_target: BlockId(1),
                then_arguments: vec![],
                else_target: BlockId(2),
                else_arguments: vec![],
            });
            let mut guarded = returning(1);
            guarded.operations.push(Operation::new(
                vec![],
                OperationKind::Barrier(Barrier {
                    execution_scope: SynchronizationScope::Subgroup,
                    memory_scope: SynchronizationScope::Workgroup,
                    semantics: BarrierSemantics::new(
                        MemoryOrdering::AcquireRelease,
                        [AddressSpace::Workgroup],
                    ),
                }),
            ));
            body.blocks.extend([guarded, returning(2)]);
            verify_module_ref(&module).unwrap();
            let report = analyze_function(&module.functions[0]);
            if tile_width == width.lanes() {
                assert_eq!(report.block_control(BlockId(1)), Variation::SubgroupUniform);
                assert!(report.diagnostics().is_empty());
            } else {
                assert_eq!(report.block_control(BlockId(1)), Variation::Varying);
                assert_eq!(
                    report.diagnostics(),
                    &[Diagnostic::DivergentBarrier {
                        block: BlockId(1),
                        operation_index: 0,
                        execution_scope: SynchronizationScope::Subgroup,
                        control: Variation::Varying,
                    }]
                );
            }
        }
    }
}
