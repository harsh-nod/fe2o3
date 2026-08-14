mod support;

use std::collections::BTreeSet;
use std::fmt::Write;

use fe2o3_device::RowMajorXor4;
use fe2o3_tiled_gemm_v1::{
    AMD_MATRIX_CALCULATOR_A_CSV_SHA256_V1, AMD_MATRIX_CALCULATOR_ARCHITECTURE_V1,
    AMD_MATRIX_CALCULATOR_B_CSV_SHA256_V1, AMD_MATRIX_CALCULATOR_C_CSV_SHA256_V1,
    AMD_MATRIX_CALCULATOR_COMMIT_V1, AMD_MATRIX_CALCULATOR_D_CSV_SHA256_V1,
    AMD_MATRIX_CALCULATOR_INSTRUCTION_V1, AMD_MATRIX_CALCULATOR_REPOSITORY_V1,
    ARegisterCoordinateV1, ARegisterLayoutV1, AccumulatorCoordinateV1, AccumulatorRegisterLayoutV1,
    BRegisterCoordinateV1, BRegisterLayoutV1, LdsLogicalCoordinateV1, MFMA_LAYOUT_COMPONENTS_V1,
    MFMA_LAYOUT_EXTENT_V1, MFMA_LAYOUT_LANES_V1, RowMajorXor4StagingV1,
};

const OFFICIAL_A_CSV_SHA256: &str =
    "0b81297df0a554684c8631e9266d9282d911bbf74518fba8e990ac9a3c41355d";
const OFFICIAL_B_CSV_SHA256: &str =
    "b39f7eed0eab2c7b207d79bd63bb57d005638cf2a9f87f250e2bc6dc611be377";
const OFFICIAL_C_CSV_SHA256: &str =
    "87b308afdee4ab2182c640a3a7ed0fb84c5555c7311ec3630d21e80969c944be";
const OFFICIAL_D_CSV_SHA256: &str =
    "dd015ae356fd034cb6f48902bf24d097426ecc3a7d8ac6942b12552bf597d836";

#[derive(Clone, Copy)]
enum OfficialMatrix {
    A,
    B,
    C,
    D,
}

fn official_calculator_csv(matrix: OfficialMatrix) -> String {
    let mut csv = String::new();
    writeln!(
        csv,
        "Architecture: {}",
        AMD_MATRIX_CALCULATOR_ARCHITECTURE_V1.to_ascii_uppercase()
    )
    .unwrap();
    writeln!(
        csv,
        "Instruction: {}",
        AMD_MATRIX_CALCULATOR_INSTRUCTION_V1.to_ascii_uppercase()
    )
    .unwrap();
    match matrix {
        OfficialMatrix::A | OfficialMatrix::B => {
            csv.push_str("lane,v0.[15:0],v0.[31:16],v1.[15:0],v1.[31:16]\n");
        }
        OfficialMatrix::C | OfficialMatrix::D => csv.push_str("lane,v0,v1,v2,v3\n"),
    }

    for lane in 0..MFMA_LAYOUT_LANES_V1 {
        write!(csv, "{lane}").unwrap();
        for component in 0..MFMA_LAYOUT_COMPONENTS_V1 {
            match matrix {
                OfficialMatrix::A => {
                    let coordinate = ARegisterLayoutV1::coordinate(lane, component).unwrap();
                    write!(csv, ",A[{}][{}]", coordinate.row, coordinate.depth).unwrap();
                }
                OfficialMatrix::B => {
                    let coordinate = BRegisterLayoutV1::coordinate(lane, component).unwrap();
                    write!(csv, ",B[{}][{}]", coordinate.depth, coordinate.column).unwrap();
                }
                OfficialMatrix::C | OfficialMatrix::D => {
                    let name = if matches!(matrix, OfficialMatrix::C) {
                        'C'
                    } else {
                        'D'
                    };
                    let coordinate =
                        AccumulatorRegisterLayoutV1::coordinate(lane, component).unwrap();
                    write!(csv, ",{name}[{}][{}]", coordinate.row, coordinate.column).unwrap();
                }
            }
        }
        csv.push('\n');
    }
    csv
}

#[test]
fn provenance_is_pinned_to_the_reviewed_amd_calculator_commit() {
    assert_eq!(
        AMD_MATRIX_CALCULATOR_REPOSITORY_V1,
        "https://github.com/ROCm/amd_matrix_instruction_calculator"
    );
    assert_eq!(
        AMD_MATRIX_CALCULATOR_COMMIT_V1,
        "2ef91896bcdc4d26624f952e5c905c787cd9bc9e"
    );
    assert_eq!(AMD_MATRIX_CALCULATOR_ARCHITECTURE_V1, "cdna3");
    assert_eq!(
        AMD_MATRIX_CALCULATOR_INSTRUCTION_V1,
        "v_mfma_f32_16x16x16_bf16"
    );
    assert_eq!(AMD_MATRIX_CALCULATOR_A_CSV_SHA256_V1, OFFICIAL_A_CSV_SHA256);
    assert_eq!(AMD_MATRIX_CALCULATOR_B_CSV_SHA256_V1, OFFICIAL_B_CSV_SHA256);
    assert_eq!(AMD_MATRIX_CALCULATOR_C_CSV_SHA256_V1, OFFICIAL_C_CSV_SHA256);
    assert_eq!(AMD_MATRIX_CALCULATOR_D_CSV_SHA256_V1, OFFICIAL_D_CSV_SHA256);
}

#[test]
fn all_official_matrix_layout_csv_digests_match_the_pinned_calculator() {
    for (matrix, expected) in [
        (OfficialMatrix::A, OFFICIAL_A_CSV_SHA256),
        (OfficialMatrix::B, OFFICIAL_B_CSV_SHA256),
        (OfficialMatrix::C, OFFICIAL_C_CSV_SHA256),
        (OfficialMatrix::D, OFFICIAL_D_CSV_SHA256),
    ] {
        assert_eq!(
            support::hex(support::sha256(official_calculator_csv(matrix).as_bytes())),
            expected
        );
    }
}

#[test]
fn all_lanes_and_components_have_the_three_distinct_official_register_maps() {
    let mut a_coordinates = BTreeSet::new();
    let mut b_coordinates = BTreeSet::new();
    let mut accumulator_coordinates = BTreeSet::new();

    for lane in 0..MFMA_LAYOUT_LANES_V1 {
        for component in 0..MFMA_LAYOUT_COMPONENTS_V1 {
            let a = ARegisterLayoutV1::coordinate(lane, component).unwrap();
            let b = BRegisterLayoutV1::coordinate(lane, component).unwrap();
            let accumulator = AccumulatorRegisterLayoutV1::coordinate(lane, component).unwrap();
            assert_eq!(
                a,
                ARegisterCoordinateV1 {
                    row: lane % 16,
                    depth: 4 * (lane / 16) + component,
                }
            );
            assert_eq!(
                b,
                BRegisterCoordinateV1 {
                    depth: 4 * (lane / 16) + component,
                    column: lane % 16,
                }
            );
            assert_eq!(
                accumulator,
                AccumulatorCoordinateV1 {
                    row: 4 * (lane / 16) + component,
                    column: lane % 16,
                }
            );
            assert!(a_coordinates.insert((a.row, a.depth)));
            assert!(b_coordinates.insert((b.depth, b.column)));
            assert!(accumulator_coordinates.insert((accumulator.row, accumulator.column)));
        }
    }

    let complete_tile = (0..MFMA_LAYOUT_EXTENT_V1)
        .flat_map(|row| (0..MFMA_LAYOUT_EXTENT_V1).map(move |column| (row, column)))
        .collect::<BTreeSet<_>>();
    assert_eq!(a_coordinates, complete_tile);
    assert_eq!(b_coordinates, complete_tile);
    assert_eq!(accumulator_coordinates, complete_tile);
}

#[test]
fn xor4_staging_is_separate_bounded_and_b_is_transposed() {
    let mut a_physical = BTreeSet::new();
    let mut b_physical = BTreeSet::new();

    for lane in 0..MFMA_LAYOUT_LANES_V1 {
        for component in 0..MFMA_LAYOUT_COMPONENTS_V1 {
            let a = ARegisterLayoutV1::coordinate(lane, component).unwrap();
            let b = BRegisterLayoutV1::coordinate(lane, component).unwrap();
            let a_staging = RowMajorXor4StagingV1::a_coordinate(lane, component).unwrap();
            let b_staging =
                RowMajorXor4StagingV1::b_transposed_coordinate(lane, component).unwrap();

            assert_eq!(
                a_staging.logical,
                LdsLogicalCoordinateV1 {
                    row: a.row,
                    column: a.depth,
                }
            );
            assert_eq!(
                b_staging.logical,
                LdsLogicalCoordinateV1 {
                    row: b.column,
                    column: b.depth,
                }
            );
            assert_eq!(
                a_staging.physical_index,
                RowMajorXor4::physical_index(a.row, a.depth).unwrap()
            );
            assert_eq!(
                b_staging.physical_index,
                RowMajorXor4::physical_index(b.column, b.depth).unwrap()
            );
            assert!(a_staging.physical_index < 256);
            assert!(b_staging.physical_index < 256);
            assert!(a_physical.insert(a_staging.physical_index));
            assert!(b_physical.insert(b_staging.physical_index));
        }
    }

    assert_eq!(a_physical, (0..256).collect());
    assert_eq!(b_physical, (0..256).collect());
}

#[test]
fn every_layout_rejects_out_of_domain_lanes_and_components() {
    for component in 0..=MFMA_LAYOUT_COMPONENTS_V1 {
        assert_eq!(ARegisterLayoutV1::coordinate(64, component), None);
        assert_eq!(BRegisterLayoutV1::coordinate(64, component), None);
        assert_eq!(AccumulatorRegisterLayoutV1::coordinate(64, component), None);
        assert_eq!(RowMajorXor4StagingV1::a_coordinate(64, component), None);
        assert_eq!(
            RowMajorXor4StagingV1::b_transposed_coordinate(64, component),
            None
        );
    }
    for lane in 0..=MFMA_LAYOUT_LANES_V1 {
        assert_eq!(ARegisterLayoutV1::coordinate(lane, 4), None);
        assert_eq!(BRegisterLayoutV1::coordinate(lane, 4), None);
        assert_eq!(AccumulatorRegisterLayoutV1::coordinate(lane, 4), None);
        assert_eq!(RowMajorXor4StagingV1::a_coordinate(lane, 4), None);
        assert_eq!(
            RowMajorXor4StagingV1::b_transposed_coordinate(lane, 4),
            None
        );
    }
    assert_eq!(
        RowMajorXor4StagingV1::physical(LdsLogicalCoordinateV1 { row: 16, column: 0 }),
        None
    );
    assert_eq!(
        RowMajorXor4StagingV1::physical(LdsLogicalCoordinateV1 { row: 0, column: 16 }),
        None
    );
}
