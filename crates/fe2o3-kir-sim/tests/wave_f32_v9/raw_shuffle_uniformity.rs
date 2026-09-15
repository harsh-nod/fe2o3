use super::*;

#[test]
fn raw_partial_shuffle_with_uniform_selector_keeps_partition_distinct_values() {
    for version in [KirVersion::V9, KirVersion::V10] {
        for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
            let tile = 16;
            let mut module = wave_f32_module(width, tile, WaveF32ReductionKindV1::Sum);
            let operations = &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
            assert!(matches!(
                operations[6].kind,
                OperationKind::Wave(WaveOperation {
                    kind: WaveOperationKind::BroadcastF32 {
                        value: ValueId(6),
                        source_lane: ValueId(9),
                        ..
                    },
                    ..
                })
            ));
            operations.splice(
                6..7,
                [
                    effect(
                        13,
                        Type::Scalar(ScalarType::U32),
                        OperationKind::Cast {
                            kind: fe2o3_kernel_ir::CastKind::Bitcast,
                            value: ValueId(6),
                            to: Type::Scalar(ScalarType::U32),
                        },
                    ),
                    effect(
                        14,
                        Type::Scalar(ScalarType::U32),
                        OperationKind::Wave(WaveOperation::full(
                            WaveOperationKind::ShuffleIndex {
                                value: ValueId(13),
                                source_lane: ValueId(9),
                                tile_width: tile,
                            },
                            width,
                        )),
                    ),
                    effect(
                        10,
                        Type::F32,
                        OperationKind::Cast {
                            kind: fe2o3_kernel_ir::CastKind::Bitcast,
                            value: ValueId(14),
                            to: Type::F32,
                        },
                    ),
                ],
            );
            let input = (0..width.lanes())
                .map(|lane| (lane as f32).to_bits())
                .collect::<Vec<_>>();
            let execution = admitted(version, module)
                .simulate(
                    &request(&input, 0, u64::from(width.lanes()), width.lanes()),
                    TARGET,
                    SimulationLimitsV1::default(),
                )
                .unwrap();
            let output = buffer_bits(execution.buffer(2).unwrap());
            let expected = (0..width.lanes())
                .map(|lane| input[(lane / tile * tile) as usize])
                .collect::<Vec<_>>();
            assert_eq!(output, expected);
            assert_ne!(output[0], output[tile as usize]);
        }
    }
}
