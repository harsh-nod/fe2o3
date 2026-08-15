use fe2o3_wave64_collectives_v1::{
    CollectiveOutputV1, OracleErrorV1, WAVE64_LANES_V1, compare_wave64_collectives_v1,
    wave64_collectives_oracle_v1,
};

fn corpus() -> [f32; WAVE64_LANES_V1] {
    core::array::from_fn(|lane| ((lane * 5 + 3) % 17) as f32 - 8.0)
}

fn run(mask: u64) -> ([f32; 64], [f32; 64], [f32; 64]) {
    let input = corpus();
    let mut reduction = [f32::NAN; WAVE64_LANES_V1];
    let mut inclusive = [f32::NAN; WAVE64_LANES_V1];
    let mut exclusive = [f32::NAN; WAVE64_LANES_V1];
    let state =
        wave64_collectives_oracle_v1(&input, mask, &mut reduction, &mut inclusive, &mut exclusive)
            .unwrap();
    assert_eq!(state.active_mask, mask);
    assert_eq!(state.active_lanes, mask.count_ones());
    compare_wave64_collectives_v1(&input, mask, &reduction, &inclusive, &exclusive).unwrap();
    (reduction, inclusive, exclusive)
}

fn assert_exact_mask_semantics(mask: u64) {
    let input = corpus();
    let (reduction, inclusive, exclusive) = run(mask);
    let total: f32 = input
        .iter()
        .copied()
        .enumerate()
        .filter(|(lane, _)| mask & (1_u64 << lane) != 0)
        .map(|(_, value)| value)
        .sum();
    let mut prefix = 0.0_f32;
    for lane in 0..WAVE64_LANES_V1 {
        if mask & (1_u64 << lane) == 0 {
            assert_eq!(reduction[lane].to_bits(), 0.0_f32.to_bits());
            assert_eq!(inclusive[lane].to_bits(), 0.0_f32.to_bits());
            assert_eq!(exclusive[lane].to_bits(), 0.0_f32.to_bits());
        } else {
            assert_eq!(reduction[lane], total);
            assert_eq!(exclusive[lane], prefix);
            prefix += input[lane];
            assert_eq!(inclusive[lane], prefix);
        }
    }
}

#[test]
fn empty_mask_is_accepted_and_publishes_positive_zero() {
    assert_exact_mask_semantics(0);
}

#[test]
fn partial_contiguous_mask_has_exact_prefixes() {
    assert_exact_mask_semantics((1_u64 << 19) - 1);
}

#[test]
fn sparse_mask_has_exact_physical_lane_order() {
    assert_exact_mask_semantics(
        (1_u64 << 0) | (1_u64 << 3) | (1_u64 << 8) | (1_u64 << 34) | (1_u64 << 63),
    );
}

#[test]
fn alternating_mask_has_exact_physical_lane_order() {
    assert_exact_mask_semantics(0xaaaa_aaaa_aaaa_aaaa);
}

#[test]
fn full_wave_has_exact_reduction_and_scans() {
    assert_exact_mask_semantics(u64::MAX);
}

#[test]
fn signed_zero_and_corpus_endpoints_remain_exact() {
    let mut input = [0.0_f32; WAVE64_LANES_V1];
    input[0] = -0.0;
    input[1] = -1024.0;
    input[63] = 1024.0;
    let mask = (1_u64 << 0) | (1_u64 << 1) | (1_u64 << 63);
    let mut reduction = [1.0; 64];
    let mut inclusive = [1.0; 64];
    let mut exclusive = [1.0; 64];
    let state =
        wave64_collectives_oracle_v1(&input, mask, &mut reduction, &mut inclusive, &mut exclusive)
            .unwrap();
    assert_eq!(state.reduction, 0.0);
    assert_eq!(exclusive[0].to_bits(), 0.0_f32.to_bits());
    assert_eq!(inclusive[1], -1024.0);
    assert_eq!(exclusive[63], -1024.0);
    assert_eq!(inclusive[63], 0.0);
}

#[test]
fn each_invalid_input_preserves_every_output() {
    let sentinel_reduction = [11.0_f32; 64];
    let sentinel_inclusive = [22.0_f32; 64];
    let sentinel_exclusive = [33.0_f32; 64];

    for (input, expected) in [
        (
            vec![0.0; 63],
            OracleErrorV1::WrongInputLength { actual: 63 },
        ),
        (
            {
                let mut values = corpus().to_vec();
                values[62] = f32::INFINITY;
                values
            },
            OracleErrorV1::NonFiniteInput { lane: 62 },
        ),
        (
            {
                let mut values = corpus().to_vec();
                values[63] = 0.5;
                values
            },
            OracleErrorV1::OutsideExactCorpus {
                lane: 63,
                value: 0.5,
            },
        ),
        (
            {
                let mut values = corpus().to_vec();
                values[63] = 1025.0;
                values
            },
            OracleErrorV1::OutsideExactCorpus {
                lane: 63,
                value: 1025.0,
            },
        ),
    ] {
        let mut reduction = sentinel_reduction;
        let mut inclusive = sentinel_inclusive;
        let mut exclusive = sentinel_exclusive;
        assert_eq!(
            wave64_collectives_oracle_v1(
                &input,
                u64::MAX,
                &mut reduction,
                &mut inclusive,
                &mut exclusive,
            ),
            Err(expected)
        );
        assert_eq!(reduction, sentinel_reduction);
        assert_eq!(inclusive, sentinel_inclusive);
        assert_eq!(exclusive, sentinel_exclusive);
    }
}

#[test]
fn each_invalid_output_width_preserves_all_supplied_outputs() {
    for rejected in [
        CollectiveOutputV1::Reduction,
        CollectiveOutputV1::Inclusive,
        CollectiveOutputV1::Exclusive,
    ] {
        let mut reduction = vec![
            11.0_f32;
            if rejected == CollectiveOutputV1::Reduction {
                63
            } else {
                64
            }
        ];
        let mut inclusive = vec![
            22.0_f32;
            if rejected == CollectiveOutputV1::Inclusive {
                65
            } else {
                64
            }
        ];
        let mut exclusive = vec![
            33.0_f32;
            if rejected == CollectiveOutputV1::Exclusive {
                0
            } else {
                64
            }
        ];
        let before_reduction = reduction.clone();
        let before_inclusive = inclusive.clone();
        let before_exclusive = exclusive.clone();
        assert!(matches!(
            wave64_collectives_oracle_v1(
                &corpus(),
                u64::MAX,
                &mut reduction,
                &mut inclusive,
                &mut exclusive,
            ),
            Err(OracleErrorV1::WrongOutputLength { output, .. }) if output == rejected
        ));
        assert_eq!(reduction, before_reduction);
        assert_eq!(inclusive, before_inclusive);
        assert_eq!(exclusive, before_exclusive);
    }
}
