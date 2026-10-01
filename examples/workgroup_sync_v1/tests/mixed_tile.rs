use fe2o3_workgroup_sync_v1::mixed_tile_oracle::{
    MixedTileConfigV1, MixedTileErrorV1, MixedTileOrderV1, mixed_tile_oracle_v1,
};

const CANARY: u32 = 0xDAD0_0057;
const ORDERS: [MixedTileOrderV1; 2] = [MixedTileOrderV1::Blocked, MixedTileOrderV1::Striped];

fn config(order: MixedTileOrderV1) -> MixedTileConfigV1 {
    MixedTileConfigV1 {
        base: 0,
        workgroups: 1,
        order,
    }
}

#[test]
fn independently_calculated_weighted_sums_distinguish_distributions() {
    let input: Vec<u32> = (1..=129).collect();
    let mut blocked = [CANARY; 64];
    let mut striped = [CANARY; 64];
    let mut blocked_ready = [false; 64];
    let mut striped_ready = [false; 64];
    mixed_tile_oracle_v1(&input, config(ORDERS[0]), &mut blocked, &mut blocked_ready).unwrap();
    mixed_tile_oracle_v1(&input, config(ORDERS[1]), &mut striped, &mut striped_ready).unwrap();
    assert_eq!(blocked[0], 75);
    assert_eq!(blocked[1], 120);
    assert_eq!(blocked[42], 1965);
    assert_eq!(blocked[43], CANARY);
    assert!(!blocked_ready[43]);
    assert_eq!(striped[0], 1272);
    assert_eq!(striped[1], 360);
    assert_eq!(striped[63], 856);
    assert!(striped_ready.into_iter().all(|ready| ready));
    assert_ne!(blocked, striped);
}

#[test]
fn wrapping_and_partial_tiles_have_literal_anchors() {
    for (input, order, first, second) in [
        (vec![u32::MAX; 3], ORDERS[0], 26, CANARY),
        (vec![u32::MAX; 2], ORDERS[0], 16, CANARY),
        (vec![u32::MAX; 1], ORDERS[0], 8, CANARY),
        (vec![u32::MAX; 65], ORDERS[1], 16, 8),
        (vec![u32::MAX; 129], ORDERS[1], 26, 16),
    ] {
        let mut output = [CANARY; 64];
        let mut initialized = [false; 64];
        mixed_tile_oracle_v1(&input, config(order), &mut output, &mut initialized).unwrap();
        assert_eq!((output[0], output[1]), (first, second));
        assert!(initialized[0]);
        assert_eq!(initialized[1], second != CANARY);
    }
}

#[test]
fn all_tail_boundaries_preserve_inactive_bytes_and_initialization() {
    for length in [0_usize, 1, 2, 3, 63, 64, 65, 127, 128, 129, 191, 192, 193] {
        for order in ORDERS {
            let mut output = [CANARY; 64];
            let mut initialized = [false; 64];
            mixed_tile_oracle_v1(
                &vec![0; length],
                config(order),
                &mut output,
                &mut initialized,
            )
            .unwrap();
            let written = match order {
                MixedTileOrderV1::Blocked => length.min(192).div_ceil(3),
                MixedTileOrderV1::Striped => length.min(64),
            };
            assert!(initialized[..written].iter().all(|&ready| ready));
            assert!(initialized[written..].iter().all(|&ready| !ready));
            assert!(output[written..].iter().all(|&value| value == CANARY));
            assert!(output[..written].iter().all(|&value| value != CANARY));
        }
    }
}

#[test]
fn two_groups_short_sinks_and_guard_regions_are_independent() {
    for order in ORDERS {
        for length in [0_usize, 1, 63, 64, 65, 127, 128, 129] {
            let input = [1; 193];
            let before = input;
            let mut backing = vec![CANARY; length + 2];
            let mut initialized = vec![false; length + 2];
            initialized[0] = true;
            initialized[length + 1] = true;
            mixed_tile_oracle_v1(
                &input,
                MixedTileConfigV1 {
                    workgroups: 2,
                    ..config(order)
                },
                &mut backing[1..length + 1],
                &mut initialized[1..length + 1],
            )
            .unwrap();
            let written = length.min(128);
            assert_eq!(&backing[1..written + 1], vec![56; written]);
            assert!(initialized[1..written + 1].iter().all(|&ready| ready));
            assert!(backing[written + 1..].iter().all(|&value| value == CANARY));
            assert!(
                initialized[written + 1..length + 1]
                    .iter()
                    .all(|&ready| !ready)
            );
            assert_eq!(backing[0], CANARY);
            assert!(initialized[0] && initialized[length + 1]);
            assert_eq!(input, before);
        }
    }
}

#[test]
fn shifted_and_finite_width_overflowing_bases_do_not_wrap() {
    for order in ORDERS {
        let input: Vec<u32> = (1..=65).collect();
        let mut output = [CANARY; 64];
        let mut initialized = [false; 64];
        mixed_tile_oracle_v1(
            &input,
            MixedTileConfigV1 {
                base: 64,
                ..config(order)
            },
            &mut output,
            &mut initialized,
        )
        .unwrap();
        assert_eq!(output[0], 206);
        assert!(initialized[0]);
        assert!(output[1..].iter().all(|&value| value == CANARY));
        assert!(initialized[1..].iter().all(|&ready| !ready));
        for base in [65, u64::MAX - 1, u64::MAX] {
            let mut output = [CANARY; 64];
            let mut initialized = [false; 64];
            initialized[17] = true;
            let before = initialized;
            mixed_tile_oracle_v1(
                &input,
                MixedTileConfigV1 {
                    base,
                    ..config(order)
                },
                &mut output,
                &mut initialized,
            )
            .unwrap();
            assert_eq!(output, [CANARY; 64]);
            assert_eq!(initialized, before);
        }
    }
}

#[test]
fn invalid_oracle_arguments_fail_before_mutation() {
    for (workgroups, mask_len, expected) in [
        (0, 2, MixedTileErrorV1::Launch),
        (u32::MAX, 2, MixedTileErrorV1::Launch),
        (1, 1, MixedTileErrorV1::Initialization),
    ] {
        let mut output = [CANARY; 2];
        let mut initialized = vec![false; mask_len];
        assert_eq!(
            mixed_tile_oracle_v1(
                &[1, 2, 3],
                MixedTileConfigV1 {
                    workgroups,
                    ..config(ORDERS[0])
                },
                &mut output,
                &mut initialized,
            ),
            Err(expected),
        );
        assert_eq!(output, [CANARY; 2]);
        assert!(initialized.iter().all(|&ready| !ready));
    }
}
