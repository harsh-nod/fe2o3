use super::*;

fn broadcast(width: WaveWidth, tile: u32) -> Function {
    let mut block = returning(0);
    block.operations = vec![
        constant(2, Constant::U32(tile - 1)),
        result(
            3,
            Type::Scalar(ScalarType::U32),
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs: ValueId(1),
                rhs: ValueId(2),
            },
        ),
        result(
            4,
            Type::F32,
            OperationKind::Wave(WaveOperation::full(
                WaveOperationKind::BroadcastF32 {
                    value: ValueId(0),
                    source_lane: ValueId(3),
                    tile_width: tile,
                },
                width,
            )),
        ),
    ];
    checked_function(vec![Type::F32, Type::Scalar(ScalarType::U32)], vec![block])
}

#[test]
fn raw_float_broadcast_requires_full_tile_and_uniform_source_selector() {
    let variations = [
        Variation::GridUniform,
        Variation::WorkgroupUniform,
        Variation::SubgroupUniform,
        Variation::Varying,
    ];
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for tile in [16, width.lanes()] {
            let function = broadcast(width, tile);
            for value in variations {
                for selector in variations {
                    let report = analyze_function_with_contract(
                        &function,
                        &[value, selector],
                        &BTreeSet::new(),
                        &BTreeSet::new(),
                        None,
                    );
                    let expected = if value.is_uniform_for(SynchronizationScope::Subgroup) {
                        value
                    } else if tile == width.lanes()
                        && selector.is_uniform_for(SynchronizationScope::Subgroup)
                    {
                        Variation::SubgroupUniform
                    } else {
                        Variation::Varying
                    };
                    assert_eq!(
                        report.value(ValueId(4)),
                        expected,
                        "{width:?}/{tile}/{value:?}/{selector:?}"
                    );
                    assert!(report.diagnostics().is_empty());
                }
            }
        }
    }
}

#[test]
fn raw_float_sum_retains_proven_uniform_inputs() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for tile in [1, 16, width.lanes()] {
            let function = parameter_reduction(width, tile, Reduction::Sum);
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

fn require_divergent_branch(function: &Function) {
    let report = analyze_function(function);
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

#[test]
fn raw_float_sum_nan_payload_branch_rejects_convergence() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        require_divergent_branch(&bit_pattern_branch(
            width,
            0x7fc0_0001,
            0xffc0_0042,
            Reduction::Sum,
        ));
    }
}

#[test]
fn raw_float_broadcast_varying_selector_rejects_convergence() {
    for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
        for tile in [16, width.lanes()] {
            let function = bit_pattern_branch(width, 0, 0x8000_0000, Reduction::Maximum);
            let mut blocks = function.body.unwrap().blocks;
            // Every lane selects the input with its own parity, retaining its zero sign.
            blocks[0].operations[8] = result(
                8,
                Type::F32,
                OperationKind::Wave(WaveOperation::full(
                    WaveOperationKind::BroadcastF32 {
                        value: ValueId(7),
                        source_lane: ValueId(2),
                        tile_width: tile,
                    },
                    width,
                )),
            );
            require_divergent_branch(&checked_function(vec![], blocks));
        }
    }
}
