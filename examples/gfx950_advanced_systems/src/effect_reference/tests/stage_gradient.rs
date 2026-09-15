use super::{SENTINEL, bits, stage_gradient_shard_point_v1};
use crate::{MUON_ELEMENTS, SYSTEM_BATCHES};

const ELEMENTS: usize = SYSTEM_BATCHES * MUON_ELEMENTS;
const INVOCATIONS: usize = SYSTEM_BATCHES * 64;

fn input() -> Vec<f32> {
    let patterns = [
        0,
        0x8000_0000,
        1,
        0x007f_ffff,
        0x3f80_0000,
        0xbf80_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_1234,
        0x7f80_0001,
    ];
    (0..ELEMENTS)
        .map(|index| f32::from_bits(patterns[index % patterns.len()]))
        .collect()
}

#[test]
fn complete_domain_matches_original_cpu_copy_bit_for_bit() {
    let input = input();
    let expected = crate::reference::stage_gradient_shard_reference(&input);
    let mut output = vec![SENTINEL; ELEMENTS];
    for point in 0..INVOCATIONS {
        stage_gradient_shard_point_v1(point, &input, &mut output);
    }
    assert_eq!(bits(&output), bits(&expected));
}

#[test]
fn each_invocation_changes_only_its_compact_coordinate() {
    let input = input();
    for point in [
        0,
        15,
        16,
        63,
        64,
        79,
        80,
        INVOCATIONS - 1,
        INVOCATIONS,
        usize::MAX,
    ] {
        let mut output = vec![SENTINEL; ELEMENTS];
        stage_gradient_shard_point_v1(point, &input, &mut output);
        let coordinate = (point / 64 < SYSTEM_BATCHES && point % 64 < MUON_ELEMENTS)
            .then(|| point / 64 * MUON_ELEMENTS + point % 64);
        for (index, value) in output.iter().enumerate() {
            let expected = if coordinate == Some(index) {
                input[index]
            } else {
                SENTINEL
            };
            assert_eq!(
                value.to_bits(),
                expected.to_bits(),
                "point={point}, index={index}"
            );
        }
    }
}

#[test]
fn partial_launch_preserves_every_unexecuted_coordinate() {
    let input = input();
    for launched in [0, 1, 16, 64, 256, 512, 768] {
        let mut output = vec![SENTINEL; ELEMENTS];
        for point in 0..launched {
            stage_gradient_shard_point_v1(point, &input, &mut output);
        }
        for (index, value) in output.iter().enumerate() {
            let owner = index / MUON_ELEMENTS * 64 + index % MUON_ELEMENTS;
            let expected = if owner < launched {
                input[index]
            } else {
                SENTINEL
            };
            assert_eq!(
                value.to_bits(),
                expected.to_bits(),
                "launched={launched}, index={index}"
            );
        }
        assert_ne!(bits(&output), bits(&input));
    }
}

#[test]
fn invalid_extents_preserve_the_complete_output() {
    for wrong_argument in 0..2 {
        for length in [0, ELEMENTS - 1, ELEMENTS + 1] {
            let mut lengths = [ELEMENTS; 2];
            lengths[wrong_argument] = length;
            let input = vec![1.0; lengths[0]];
            let mut output = vec![SENTINEL; lengths[1]];
            for point in (0..=INVOCATIONS).chain([usize::MAX]) {
                stage_gradient_shard_point_v1(point, &input, &mut output);
                assert!(
                    output
                        .iter()
                        .all(|value| value.to_bits() == SENTINEL.to_bits())
                );
            }
        }
    }
}
