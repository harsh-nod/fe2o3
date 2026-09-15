use super::*;

#[test]
fn raw_fullwave_sum_retains_distinct_nan_payloads() {
    for version in [KirVersion::V9, KirVersion::V10] {
        for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
            let input = (0..width.lanes())
                .map(|lane| 0x7fc0_0100 + lane)
                .collect::<Vec<_>>();
            let execution = admitted(
                version,
                wave_f32_module(width, width.lanes(), WaveF32ReductionKindV1::Sum),
            )
            .simulate(
                &request(&input, 0, u64::from(width.lanes()), width.lanes()),
                TARGET,
                SimulationLimitsV1::default(),
            )
            .unwrap();
            let output = buffer_bits(execution.buffer(1).unwrap());
            assert_eq!(output, input);
            assert_ne!(output[0], output[1]);
        }
    }
}

#[test]
fn raw_float_broadcast_can_select_different_signed_zero_bits_per_lane() {
    for version in [KirVersion::V9, KirVersion::V10] {
        for width in [WaveWidth::Wave32, WaveWidth::Wave64] {
            for tile in [16, width.lanes()] {
                let mut module = wave_f32_module(width, tile, WaveF32ReductionKindV1::Sum);
                let operations =
                    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations;
                let OperationKind::Binary { lhs, .. } = &mut operations[5].kind else {
                    panic!("existing broadcast source-lane mask");
                };
                assert_eq!(*lhs, ValueId(3));
                *lhs = ValueId(13);
                operations.insert(
                    4,
                    effect(
                        13,
                        Type::Scalar(ScalarType::U32),
                        OperationKind::Wave(WaveOperation::full(WaveOperationKind::LaneId, width)),
                    ),
                );
                let input = (0..width.lanes())
                    .map(|lane| if lane % 2 == 0 { 0 } else { 0x8000_0000 })
                    .collect::<Vec<_>>();
                let execution = admitted(version, module)
                    .simulate(
                        &request(&input, 0, u64::from(width.lanes()), width.lanes()),
                        TARGET,
                        SimulationLimitsV1::default(),
                    )
                    .unwrap();
                let output = buffer_bits(execution.buffer(2).unwrap());
                assert_eq!(output, input);
                assert_ne!(output[0], output[1]);
            }
        }
    }
}
