use super::*;

#[path = "tests/stage_gradient.rs"]
mod stage_gradient;

fn inputs() -> (Vec<f32>, Vec<f32>) {
    (
        (0..ELEMENTS).map(|i| i as f32 * 0.25).collect(),
        (0..ELEMENTS).map(|i| 1.0 - i as f32 * 0.125).collect(),
    )
}

fn bits(values: &[f32]) -> Vec<u32> {
    values.iter().map(|value| value.to_bits()).collect()
}

const SENTINEL: f32 = -12345.25;

#[test]
fn combine_point_matches_original_cpu_reference_on_complete_domain() {
    let (rank0, rank1) = inputs();
    let expected = crate::reference::combine_expert_ranks_reference(&rank0, &rank1);
    let mut actual = vec![SENTINEL; ELEMENTS];
    for point in 0..ELEMENTS {
        combine_expert_ranks_point_v1(point, &rank0, &rank1, &mut actual);
    }
    assert_eq!(bits(&actual), bits(&expected));
}

#[test]
fn combine_point_preserves_unexecuted_frame_for_partial_launches() {
    let (rank0, rank1) = inputs();
    let expected = crate::reference::combine_expert_ranks_reference(&rank0, &rank1);
    for launched in [0, 1, 256, 512, 768] {
        let mut actual = vec![SENTINEL; ELEMENTS];
        for point in 0..launched {
            combine_expert_ranks_point_v1(point, &rank0, &rank1, &mut actual);
        }
        assert_eq!(bits(&actual[..launched]), bits(&expected[..launched]));
        assert!(
            actual[launched..]
                .iter()
                .all(|v| v.to_bits() == SENTINEL.to_bits())
        );
        assert_ne!(
            bits(&actual),
            bits(&expected),
            "partial launch is not complete coverage"
        );
    }
}

#[test]
fn combine_point_changes_only_its_exact_coordinate() {
    let (rank0, rank1) = inputs();
    let expected = crate::reference::combine_expert_ranks_reference(&rank0, &rank1);
    for point in [0, 15, 16, 255, 256, ELEMENTS - 1] {
        let mut actual = vec![SENTINEL; ELEMENTS];
        combine_expert_ranks_point_v1(point, &rank0, &rank1, &mut actual);
        for (index, value) in actual.iter().enumerate() {
            assert_eq!(
                value.to_bits(),
                if index == point {
                    expected[index]
                } else {
                    SENTINEL
                }
                .to_bits()
            );
        }
    }
}

#[test]
fn combine_point_invalid_extents_and_coordinates_preserve_complete_frame() {
    for wrong_argument in 0..3 {
        for length in [0, ELEMENTS - 1, ELEMENTS + 1] {
            let mut lengths = [ELEMENTS; 3];
            lengths[wrong_argument] = length;
            let rank0 = vec![1.0; lengths[0]];
            let rank1 = vec![2.0; lengths[1]];
            let mut actual = vec![SENTINEL; lengths[2]];
            for point in [0, ELEMENTS - 1, ELEMENTS, usize::MAX] {
                combine_expert_ranks_point_v1(point, &rank0, &rank1, &mut actual);
                assert!(actual.iter().all(|v| v.to_bits() == SENTINEL.to_bits()));
            }
        }
    }
    let (rank0, rank1) = inputs();
    let mut actual = vec![SENTINEL; ELEMENTS];
    for point in [ELEMENTS, ELEMENTS + 1, usize::MAX] {
        combine_expert_ranks_point_v1(point, &rank0, &rank1, &mut actual);
        assert!(actual.iter().all(|v| v.to_bits() == SENTINEL.to_bits()));
    }
}
