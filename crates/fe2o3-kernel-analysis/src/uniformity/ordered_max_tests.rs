use super::*;
use fe2o3_kernel_ir::{
    Barrier, BarrierSemantics, MemoryOrdering, Signature, SynchronizationScope, ValueDef,
    WaveF32ReductionKindV1 as Reduction, WaveOperation, WaveWidth,
};

#[path = "ordered_max_tests/wave_float_tests.rs"]
mod wave_float_tests;

#[path = "ordered_max_tests/collective_uniformity_tests.rs"]
mod collective_uniformity_tests;

fn result(id: u32, ty: Type, kind: OperationKind) -> Operation {
    Operation::effect_free(ValueDef::new(ValueId(id), ty), kind)
}

fn constant(id: u32, value: Constant) -> Operation {
    result(id, value.ty(), OperationKind::Constant(value))
}

fn reduction(id: u32, input: u32, width: WaveWidth, tile_width: u32, kind: Reduction) -> Operation {
    result(
        id,
        Type::F32,
        OperationKind::Wave(WaveOperation::full(
            WaveOperationKind::ReduceF32 {
                value: ValueId(input),
                tile_width,
                kind,
            },
            width,
        )),
    )
}

fn checked_function(parameters: Vec<Type>, blocks: Vec<BasicBlock>) -> Function {
    let values = (0..parameters.len()).map(|id| ValueId(id as u32)).collect();
    let mut function = Function::definition(
        "ordered_max_uniformity",
        Signature::new(parameters, vec![]),
        values,
        blocks,
    );
    function.required_capabilities = function
        .body
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .flat_map(Operation::required_capabilities)
        .collect();
    let mut module = Module::new("ordered_max_uniformity");
    module.required_capabilities = function.required_capabilities.clone();
    module.functions.push(function.clone());
    fe2o3_kernel_ir::verify_module(&module).unwrap();
    function
}

fn returning(id: u32) -> BasicBlock {
    let mut block = BasicBlock::new(BlockId(id));
    block.terminator = Some(Terminator::Return { values: vec![] });
    block
}

fn parameter_reduction(width: WaveWidth, tile: u32, kind: Reduction) -> Function {
    let mut block = returning(0);
    block.operations.push(reduction(1, 0, width, tile, kind));
    checked_function(vec![Type::F32], vec![block])
}

#[test]
fn ordered_max_kind_and_width_do_not_manufacture_uniformity() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for tile in [1, 2, 4, 8, 16, 32, 64]
            .into_iter()
            .filter(|tile| *tile <= width.lanes())
        {
            for kind in [Reduction::Sum, Reduction::Maximum] {
                let report = analyze_function(&parameter_reduction(width, tile, kind));
                assert_eq!(
                    report.value(ValueId(1)),
                    Variation::Varying,
                    "{width:?}/{tile}/{kind:?}"
                );
                assert!(report.diagnostics().is_empty());
            }
        }
    }
}

#[test]
fn ordered_max_retains_proven_input_uniformity() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for tile in [1, 16, width.lanes()] {
            let function = parameter_reduction(width, tile, Reduction::Maximum);
            for input in [
                Variation::GridUniform,
                Variation::WorkgroupUniform,
                Variation::SubgroupUniform,
            ] {
                let report = analyze_function_with_contract(
                    &function,
                    &[input],
                    &BTreeSet::new(),
                    &BTreeSet::new(),
                    None,
                );
                assert_eq!(
                    report.value(ValueId(1)),
                    input,
                    "{width:?}/{tile}/{input:?}"
                );
                assert!(report.diagnostics().is_empty());
            }
        }
    }
}

fn bit_pattern_branch(width: WaveWidth, even: u32, odd: u32, kind: Reduction) -> Function {
    let mut entry = BasicBlock::new(BlockId(0));
    entry.operations = vec![
        result(
            0,
            Type::Scalar(ScalarType::U32),
            OperationKind::Wave(WaveOperation::full(WaveOperationKind::LaneId, width)),
        ),
        constant(1, Constant::U32(1)),
        result(
            2,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(0),
                rhs: ValueId(1),
            },
        ),
        constant(3, Constant::U32(0)),
        result(
            4,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(2),
                rhs: ValueId(3),
            },
        ),
        constant(5, Constant::F32Bits(even)),
        constant(6, Constant::F32Bits(odd)),
        result(
            7,
            Type::F32,
            OperationKind::Select {
                condition: ValueId(4),
                true_value: ValueId(5),
                false_value: ValueId(6),
            },
        ),
        reduction(8, 7, width, width.lanes(), kind),
        result(
            9,
            Type::Scalar(ScalarType::U32),
            OperationKind::Cast {
                kind: CastKind::Bitcast,
                value: ValueId(8),
                to: Type::Scalar(ScalarType::U32),
            },
        ),
        constant(10, Constant::U32(even)),
        result(
            11,
            Type::BOOL,
            OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs: ValueId(9),
                rhs: ValueId(10),
            },
        ),
    ];
    entry.terminator = Some(Terminator::ConditionalBranch {
        condition: ValueId(11),
        then_target: BlockId(1),
        then_arguments: vec![],
        else_target: BlockId(2),
        else_arguments: vec![],
    });
    let mut conditional = returning(1);
    conditional.operations.push(Operation::new(
        vec![],
        OperationKind::Barrier(Barrier {
            execution_scope: SynchronizationScope::Subgroup,
            memory_scope: SynchronizationScope::Subgroup,
            semantics: BarrierSemantics::new(
                MemoryOrdering::AcquireRelease,
                [AddressSpace::Workgroup],
            ),
        }),
    ));
    checked_function(vec![], vec![entry, conditional, returning(2)])
}

// A bit-preserving counterexample oracle for the specified ordered XOR tree, not proof evidence.
fn ordered_tree(mut lanes: Vec<u32>) -> Vec<u32> {
    for stage in 0..lanes.len().trailing_zeros() {
        let previous = lanes.clone();
        for (lane, output) in lanes.iter_mut().enumerate() {
            let lhs = previous[lane];
            let rhs = previous[lane ^ (1 << stage)];
            *output = if f32::from_bits(lhs) < f32::from_bits(rhs) {
                rhs
            } else {
                lhs
            };
        }
    }
    lanes
}

fn assert_lane_bits_reject_convergence(even: u32, odd: u32) {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        let input = (0..width.lanes())
            .map(|lane| if lane % 2 == 0 { even } else { odd })
            .collect::<Vec<_>>();
        assert_ne!(even, odd);
        assert_eq!(ordered_tree(input.clone()), input);
        let report = analyze_function(&bit_pattern_branch(width, even, odd, Reduction::Maximum));
        assert_eq!(report.value(ValueId(8)), Variation::Varying);
        assert_eq!(report.value(ValueId(11)), Variation::Varying);
        assert_eq!(report.block_control(BlockId(1)), Variation::Varying);
        assert!(
            matches!(
                report.diagnostics(),
                [Diagnostic::DivergentBarrier {
                    block: BlockId(1),
                    operation_index: 0,
                    execution_scope: SynchronizationScope::Subgroup,
                    control: Variation::Varying,
                }]
            ),
            "{:?}",
            report.diagnostics()
        );
    }
}

#[test]
fn ordered_max_signed_zero_bits_reject_subgroup_convergence() {
    assert_lane_bits_reject_convergence(0, 0x8000_0000);
}

#[test]
fn ordered_max_nan_payload_bits_reject_subgroup_convergence() {
    assert_lane_bits_reject_convergence(0x7fc0_0001, 0xffc0_0042);
    assert_lane_bits_reject_convergence(0x7f80_0001, 0xff80_0042);
}

#[test]
fn ordered_max_nan_barriers_can_leave_different_finite_results() {
    for width in [32, 64] {
        let mut input = vec![0x7fc0_0100; width];
        input[0] = 1.0_f32.to_bits();
        input[1] = 2.0_f32.to_bits();
        input[3] = 3.0_f32.to_bits();
        let output = ordered_tree(input);
        assert_eq!(output[0], 2.0_f32.to_bits());
        assert_eq!(output[1], 3.0_f32.to_bits());
    }
}
