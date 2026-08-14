//! Exact register and LDS layouts for gfx942 `V_MFMA_F32_16X16X16_BF16`.
//!
//! Register mappings are pinned to AMD's matrix instruction calculator at
//! commit [`2ef91896bcdc4d26624f952e5c905c787cd9bc9e`]. The A, B, and C/D
//! coordinate types are intentionally distinct. XOR4 is a separate physical
//! LDS storage transformation and is never used as a register-layout model.
//!
//! [`2ef91896bcdc4d26624f952e5c905c787cd9bc9e`]: https://github.com/ROCm/amd_matrix_instruction_calculator/tree/2ef91896bcdc4d26624f952e5c905c787cd9bc9e

use fe2o3_device::RowMajorXor4;

/// Pinned AMD matrix instruction calculator repository.
pub const AMD_MATRIX_CALCULATOR_REPOSITORY_V1: &str =
    "https://github.com/ROCm/amd_matrix_instruction_calculator";
/// Pinned AMD matrix instruction calculator commit.
pub const AMD_MATRIX_CALCULATOR_COMMIT_V1: &str = "2ef91896bcdc4d26624f952e5c905c787cd9bc9e";
/// Calculator architecture selecting gfx942/CDNA3 semantics.
pub const AMD_MATRIX_CALCULATOR_ARCHITECTURE_V1: &str = "cdna3";
/// Exact instruction name passed to the calculator.
pub const AMD_MATRIX_CALCULATOR_INSTRUCTION_V1: &str = "v_mfma_f32_16x16x16_bf16";

/// SHA-256 of the calculator's exact A `--matrix-layout --csv` output.
pub const AMD_MATRIX_CALCULATOR_A_CSV_SHA256_V1: &str =
    "0b81297df0a554684c8631e9266d9282d911bbf74518fba8e990ac9a3c41355d";
/// SHA-256 of the calculator's exact B `--matrix-layout --csv` output.
pub const AMD_MATRIX_CALCULATOR_B_CSV_SHA256_V1: &str =
    "b39f7eed0eab2c7b207d79bd63bb57d005638cf2a9f87f250e2bc6dc611be377";
/// SHA-256 of the calculator's exact C `--matrix-layout --csv` output.
pub const AMD_MATRIX_CALCULATOR_C_CSV_SHA256_V1: &str =
    "87b308afdee4ab2182c640a3a7ed0fb84c5555c7311ec3630d21e80969c944be";
/// SHA-256 of the calculator's exact D `--matrix-layout --csv` output.
pub const AMD_MATRIX_CALCULATOR_D_CSV_SHA256_V1: &str =
    "dd015ae356fd034cb6f48902bf24d097426ecc3a7d8ac6942b12552bf597d836";

/// Number of lanes participating in the exact MFMA operation.
pub const MFMA_LAYOUT_LANES_V1: usize = 64;
/// Number of logical matrix values held by each lane for each operand.
pub const MFMA_LAYOUT_COMPONENTS_V1: usize = 4;
/// Row, column, and reduction dimension of each exact matrix tile.
pub const MFMA_LAYOUT_EXTENT_V1: usize = 16;

/// One A-register matrix coordinate `A[row][depth]`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ARegisterCoordinateV1 {
    /// Matrix row in `0..16`.
    pub row: usize,
    /// Reduction coordinate in `0..16`.
    pub depth: usize,
}

/// One B-register matrix coordinate `B[depth][column]`.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct BRegisterCoordinateV1 {
    /// Reduction coordinate in `0..16`.
    pub depth: usize,
    /// Matrix column in `0..16`.
    pub column: usize,
}

/// One C-input or D-output accumulator coordinate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AccumulatorCoordinateV1 {
    /// Matrix row in `0..16`.
    pub row: usize,
    /// Matrix column in `0..16`.
    pub column: usize,
}

/// A logical row-major coordinate before any LDS swizzle.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LdsLogicalCoordinateV1 {
    /// Logical row in `0..16`.
    pub row: usize,
    /// Logical column in `0..16`.
    pub column: usize,
}

/// One logical coordinate together with its XOR4 physical LDS index.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct LdsPhysicalCoordinateV1 {
    /// Logical row-major coordinate before swizzling.
    pub logical: LdsLogicalCoordinateV1,
    /// Physical element index in the 256-element XOR4 LDS tile.
    pub physical_index: usize,
}

const fn checked_lane_component_v1(lane: usize, component: usize) -> bool {
    lane < MFMA_LAYOUT_LANES_V1 && component < MFMA_LAYOUT_COMPONENTS_V1
}

/// Exact Src0/A register layout for `V_MFMA_F32_16X16X16_BF16`.
pub enum ARegisterLayoutV1 {}

impl ARegisterLayoutV1 {
    /// Maps one lane and packed BF16 component to `A[row][depth]`.
    ///
    /// The components are `v0[15:0]`, `v0[31:16]`, `v1[15:0]`, and
    /// `v1[31:16]`, in that order.
    pub const fn coordinate(lane: usize, component: usize) -> Option<ARegisterCoordinateV1> {
        if !checked_lane_component_v1(lane, component) {
            return None;
        }
        Some(ARegisterCoordinateV1 {
            row: lane % 16,
            depth: 4 * (lane / 16) + component,
        })
    }
}

/// Exact Src1/B register layout for `V_MFMA_F32_16X16X16_BF16`.
pub enum BRegisterLayoutV1 {}

impl BRegisterLayoutV1 {
    /// Maps one lane and packed BF16 component to `B[depth][column]`.
    ///
    /// The components are `v0[15:0]`, `v0[31:16]`, `v1[15:0]`, and
    /// `v1[31:16]`, in that order.
    pub const fn coordinate(lane: usize, component: usize) -> Option<BRegisterCoordinateV1> {
        if !checked_lane_component_v1(lane, component) {
            return None;
        }
        Some(BRegisterCoordinateV1 {
            depth: 4 * (lane / 16) + component,
            column: lane % 16,
        })
    }
}

/// Exact Src2/C and Vdst/D accumulator register layout.
pub enum AccumulatorRegisterLayoutV1 {}

impl AccumulatorRegisterLayoutV1 {
    /// Maps one lane and FP32 register component to `C/D[row][column]`.
    ///
    /// Components zero through three correspond to registers `v0` through
    /// `v3`, respectively.
    pub const fn coordinate(lane: usize, component: usize) -> Option<AccumulatorCoordinateV1> {
        if !checked_lane_component_v1(lane, component) {
            return None;
        }
        Some(AccumulatorCoordinateV1 {
            row: 4 * (lane / 16) + component,
            column: lane % 16,
        })
    }
}

/// Separate row-major XOR4 physical storage transformation for 16x16 LDS.
pub enum RowMajorXor4StagingV1 {}

impl RowMajorXor4StagingV1 {
    /// Maps one bounded logical LDS coordinate to its physical element index.
    pub const fn physical(logical: LdsLogicalCoordinateV1) -> Option<LdsPhysicalCoordinateV1> {
        let Some(physical_index) = RowMajorXor4::physical_index(logical.row, logical.column) else {
            return None;
        };
        Some(LdsPhysicalCoordinateV1 {
            logical,
            physical_index,
        })
    }

    /// Stages A in logical `(row, depth)` order before applying XOR4.
    pub const fn a_coordinate(lane: usize, component: usize) -> Option<LdsPhysicalCoordinateV1> {
        let Some(register) = ARegisterLayoutV1::coordinate(lane, component) else {
            return None;
        };
        Self::physical(LdsLogicalCoordinateV1 {
            row: register.row,
            column: register.depth,
        })
    }

    /// Stages B transposed as logical `(column, depth)` before applying XOR4.
    pub const fn b_transposed_coordinate(
        lane: usize,
        component: usize,
    ) -> Option<LdsPhysicalCoordinateV1> {
        let Some(register) = BRegisterLayoutV1::coordinate(lane, component) else {
            return None;
        };
        Self::physical(LdsLogicalCoordinateV1 {
            row: register.column,
            column: register.depth,
        })
    }
}
