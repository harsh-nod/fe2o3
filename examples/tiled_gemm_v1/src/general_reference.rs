//! Executable tiled CPU model for the conservative general GEMM plan.
//!
//! This is an oracle-side schedule model, not device execution authority. It
//! stages guarded `16x16` BF16 tiles through the same XOR4 physical mapping,
//! carries FP32 accumulators across every K phase, and applies the recorded
//! `alpha/beta` epilogue only to in-bounds outputs.

use core::fmt;
use std::hint::black_box;

use fe2o3_device::RowMajorXor4;

use crate::general_plan::GeneralGemmPlanV1;
use crate::numerical_contract::{NumericalOperand, widen_bf16_bits};

const TILE: usize = 16;
const TILE_ELEMENTS: usize = TILE * TILE;

/// Structural observations from one complete tiled reference execution.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct GeneralReferenceTraceV1 {
    workgroups: u64,
    reduction_phases: u64,
    publish_barriers: u64,
    reuse_barriers: u64,
    a_zero_fills: u64,
    b_zero_fills: u64,
    c_predicated_stores: u64,
    output_stores: u64,
}

impl GeneralReferenceTraceV1 {
    /// Returns the number of output-tile workgroups modeled.
    pub const fn workgroups(self) -> u64 {
        self.workgroups
    }

    /// Returns the total number of workgroup K phases modeled.
    pub const fn reduction_phases(self) -> u64 {
        self.reduction_phases
    }

    /// Returns the number of unconditional LDS publication barriers.
    pub const fn publish_barriers(self) -> u64 {
        self.publish_barriers
    }

    /// Returns the number of unconditional LDS reuse barriers.
    pub const fn reuse_barriers(self) -> u64 {
        self.reuse_barriers
    }

    /// Returns the number of predicated-off A tile elements zero-filled.
    pub const fn a_zero_fills(self) -> u64 {
        self.a_zero_fills
    }

    /// Returns the number of predicated-off B tile elements zero-filled.
    pub const fn b_zero_fills(self) -> u64 {
        self.b_zero_fills
    }

    /// Returns the number of out-of-bounds C tile stores skipped.
    pub const fn c_predicated_stores(self) -> u64 {
        self.c_predicated_stores
    }

    /// Returns the number of logical C values produced exactly once.
    pub const fn output_stores(self) -> u64 {
        self.output_stores
    }
}

/// Compact logical outputs and schedule observations.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneralReferenceResultV1 {
    output: Vec<f32>,
    trace: GeneralReferenceTraceV1,
}

impl GeneralReferenceResultV1 {
    /// Returns compact row-major logical outputs without C padding.
    pub fn output(&self) -> &[f32] {
        &self.output
    }

    /// Consumes the result and returns compact logical outputs.
    pub fn into_output(self) -> Vec<f32> {
        self.output
    }

    /// Returns structural schedule observations.
    pub const fn trace(&self) -> GeneralReferenceTraceV1 {
        self.trace
    }
}

/// Reference execution rejected host storage before arithmetic.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralReferenceErrorV1 {
    /// A matrix slice does not exactly match its checked accessed extent.
    WrongLength {
        /// Rejected matrix.
        operand: NumericalOperand,
        /// Required element count.
        expected: usize,
        /// Supplied element count.
        actual: usize,
    },
    /// The plan's private invariants disagreed with the XOR4 mapping.
    InvalidPlanInvariant,
}

impl fmt::Display for GeneralReferenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength {
                operand,
                expected,
                actual,
            } => write!(
                formatter,
                "{operand} requires exactly {expected} accessed elements, got {actual}"
            ),
            Self::InvalidPlanInvariant => {
                formatter.write_str("checked general GEMM plan has an invalid tiled invariant")
            }
        }
    }
}

impl std::error::Error for GeneralReferenceErrorV1 {}

#[inline(never)]
fn fp32_product(left: f32, right: f32) -> f32 {
    black_box(black_box(left) * black_box(right))
}

#[inline(never)]
fn fp32_sum(left: f32, right: f32) -> f32 {
    black_box(black_box(left) + black_box(right))
}

fn validate_lengths(
    plan: &GeneralGemmPlanV1,
    a_bits: &[u16],
    b_bits: &[u16],
    c: &[f32],
) -> Result<(), GeneralReferenceErrorV1> {
    let expected = plan.storage().elements();
    for (operand, expected, actual) in [
        (NumericalOperand::A, expected[0], a_bits.len()),
        (NumericalOperand::B, expected[1], b_bits.len()),
        (NumericalOperand::C, expected[2], c.len()),
    ] {
        if expected != actual {
            return Err(GeneralReferenceErrorV1::WrongLength {
                operand,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

/// Executes the plan's conservative tiled schedule on the CPU.
///
/// The returned output is compact `M*N`; input padding is read only where the
/// checked logical matrix requires it and is never copied into the result.
pub fn execute_general_reference_v1(
    plan: &GeneralGemmPlanV1,
    a_bits: &[u16],
    b_bits: &[u16],
    c: &[f32],
) -> Result<GeneralReferenceResultV1, GeneralReferenceErrorV1> {
    validate_lengths(plan, a_bits, b_bits, c)?;
    if !plan.requires_dispatch() {
        return Ok(GeneralReferenceResultV1 {
            output: Vec::new(),
            trace: GeneralReferenceTraceV1::default(),
        });
    }

    let request = plan.request();
    let [m, n, k] = request.dimensions().map(|value| value as usize);
    let [lda, ldb, ldc] = request.strides().map(|value| value as usize);
    let [tile_columns, tile_rows, _] = plan.block_counts();
    let mut output = vec![0.0_f32; m * n];
    let mut trace = GeneralReferenceTraceV1 {
        workgroups: plan.total_workgroups(),
        ..GeneralReferenceTraceV1::default()
    };

    for group_y in 0..tile_rows as usize {
        for group_x in 0..tile_columns as usize {
            let mut accumulators = [0.0_f32; TILE_ELEMENTS];
            for phase in 0..plan.reduction_phases() as usize {
                let mut a_lds = [0.0_f32; TILE_ELEMENTS];
                let mut b_lds = [0.0_f32; TILE_ELEMENTS];

                for tile_row in 0..TILE {
                    for tile_depth in 0..TILE {
                        let physical = RowMajorXor4::physical_index(tile_row, tile_depth)
                            .ok_or(GeneralReferenceErrorV1::InvalidPlanInvariant)?;
                        let row = group_y * TILE + tile_row;
                        let depth = phase * TILE + tile_depth;
                        if row < m && depth < k {
                            a_lds[physical] = widen_bf16_bits(a_bits[row * lda + depth]);
                        } else {
                            trace.a_zero_fills += 1;
                        }
                    }
                }
                for tile_depth in 0..TILE {
                    for tile_column in 0..TILE {
                        let physical = RowMajorXor4::physical_index(tile_column, tile_depth)
                            .ok_or(GeneralReferenceErrorV1::InvalidPlanInvariant)?;
                        let depth = phase * TILE + tile_depth;
                        let column = group_x * TILE + tile_column;
                        if depth < k && column < n {
                            b_lds[physical] = widen_bf16_bits(b_bits[depth * ldb + column]);
                        } else {
                            trace.b_zero_fills += 1;
                        }
                    }
                }

                trace.reduction_phases += 1;
                trace.publish_barriers += 1;
                for tile_row in 0..TILE {
                    for tile_column in 0..TILE {
                        let output_index = tile_row * TILE + tile_column;
                        for tile_depth in 0..TILE {
                            let a_physical = RowMajorXor4::physical_index(tile_row, tile_depth)
                                .ok_or(GeneralReferenceErrorV1::InvalidPlanInvariant)?;
                            let b_physical = RowMajorXor4::physical_index(tile_column, tile_depth)
                                .ok_or(GeneralReferenceErrorV1::InvalidPlanInvariant)?;
                            let product = fp32_product(a_lds[a_physical], b_lds[b_physical]);
                            accumulators[output_index] =
                                fp32_sum(accumulators[output_index], product);
                        }
                    }
                }
                trace.reuse_barriers += 1;
            }

            for tile_row in 0..TILE {
                for tile_column in 0..TILE {
                    let row = group_y * TILE + tile_row;
                    let column = group_x * TILE + tile_column;
                    if row >= m || column >= n {
                        trace.c_predicated_stores += 1;
                        continue;
                    }
                    let accumulator = accumulators[tile_row * TILE + tile_column];
                    let product = fp32_product(f32::from_bits(request.alpha_bits()), accumulator);
                    let initial =
                        fp32_product(f32::from_bits(request.beta_bits()), c[row * ldc + column]);
                    output[row * n + column] = fp32_sum(product, initial);
                    trace.output_stores += 1;
                }
            }
        }
    }

    Ok(GeneralReferenceResultV1 { output, trace })
}
