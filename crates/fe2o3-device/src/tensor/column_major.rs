//! Checked column-major global storage for the existing BF16 B register map.

use super::{
    Bf16MatrixViewError, Bf16MfmaBFragment, Bf16MfmaFragment, Bf16MfmaMatrix, MfmaOperandB,
};
use crate::{Bf16, Wave64, WaveLane};

/// Logical B[K,N] backed by column-major BF16 storage, without a transpose copy.
///
/// The private view cannot be substituted for a row-major view. Its loader
/// produces the same logical B lane/register distribution used by MFMA.
///
/// A column-major view is not a row-major view:
/// ```compile_fail
/// use fe2o3_device::{Bf16MfmaBColumnMajorMatrix, Bf16MfmaBMatrix};
/// let bits = [0_u16; 256];
/// let view = Bf16MfmaBColumnMajorMatrix::column_major(&bits, 0, 16, 16, 16).unwrap();
/// let _: Bf16MfmaBMatrix<'_> = view;
/// ```
/// Its B fragment cannot be used as an A operand:
/// ```compile_fail
/// use fe2o3_device::{Bf16MfmaAFragment, Bf16MfmaBColumnMajorMatrix, Wave64, WaveLane};
/// fn swap<'w>(view: &Bf16MfmaBColumnMajorMatrix<'_>, lane: &'w WaveLane<Wave64>) -> Bf16MfmaAFragment<'w> {
///     view.load_k16n16(lane, 0, 0)
/// }
/// ```
#[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_b_column_major_matrix_v1"]
pub struct Bf16MfmaBColumnMajorMatrix<'data> {
    bits: &'data [u16],
    offset: usize,
    reduction: usize,
    columns: usize,
    stride: usize,
}

impl<'data> Bf16MfmaBColumnMajorMatrix<'data> {
    /// Checks B[K,N] storage whose element `(k,n)` is at `offset+n*stride+k`.
    /// Stride, extent overflow, and storage bounds use the row-major view's
    /// existing checks over the physically transposed dimensions N,K.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_b_column_major_v1"]
    pub fn column_major(
        bits: &'data [u16],
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Bf16MatrixViewError> {
        let _ = Bf16MfmaMatrix::<'data, MfmaOperandB>::checked(
            bits, offset, columns, reduction, stride,
        )?;
        Ok(Self {
            bits,
            offset,
            reduction,
            columns,
            stride,
        })
    }

    /// Loads a logical K16xN16 B tile into the existing wave64 B register map.
    /// Logical out-of-bounds and overflowed coordinates are zero-filled.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_b_column_major_load_zero_filled_v1"]
    pub fn load_k16n16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64>,
        reduction_base: usize,
        column_base: usize,
    ) -> Bf16MfmaBFragment<'wave> {
        let lane_index = lane.get() as usize;
        let column = column_base.checked_add(lane_index & 15);
        let first_reduction = reduction_base.checked_add((lane_index >> 4) * 4);
        let mut values = [Bf16::ZERO; 4];
        let mut component = 0;
        while component < 4 {
            let reduction = first_reduction.and_then(|base| base.checked_add(component));
            if let (Some(column), Some(reduction)) = (column, reduction)
                && column < self.columns
                && reduction < self.reduction
            {
                values[component] = column
                    .checked_mul(self.stride)
                    .and_then(|base| self.offset.checked_add(base))
                    .and_then(|base| base.checked_add(reduction))
                    .and_then(|index| self.bits.get(index))
                    .copied()
                    .map(Bf16::from_bits)
                    .unwrap_or(Bf16::ZERO);
            }
            component += 1;
        }
        Bf16MfmaFragment::from_values(lane, values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Bf16MfmaBMatrix;
    use std::vec::Vec;

    #[test]
    fn column_major_b_matches_logical_row_major_for_every_lane_and_component() {
        let (k, n, stride, offset) = (32, 48, 37, 7);
        let mut physical = std::vec![u16::MAX; offset + n * stride];
        let mut logical = std::vec![0; k * n];
        for column in 0..n {
            for reduction in 0..k {
                let value = (1 + column * k + reduction) as u16;
                physical[offset + column * stride + reduction] = value;
                logical[reduction * n + column] = value;
            }
        }
        let column_major =
            Bf16MfmaBColumnMajorMatrix::column_major(&physical, offset, k, n, stride).unwrap();
        let row_major = Bf16MfmaBMatrix::row_major(&logical, 0, k, n, n).unwrap();
        for column_base in [0, 5, 16, 32] {
            for reduction_base in [0, 3, 16] {
                for lane in 0..64 {
                    let lane = WaveLane::<Wave64>::from_model_snapshot(lane).unwrap();
                    assert_eq!(
                        column_major
                            .load_k16n16(&lane, reduction_base, column_base)
                            .into_array()
                            .map(Bf16::to_bits),
                        row_major
                            .load_k16n16(&lane, reduction_base, column_base)
                            .into_array()
                            .map(Bf16::to_bits),
                    );
                }
            }
        }
    }

    #[test]
    fn column_major_b_rejects_bad_stride_extent_and_overflow() {
        assert_eq!(
            Bf16MfmaBColumnMajorMatrix::column_major(&[0; 6], 0, 3, 2, 2).err(),
            Some(Bf16MatrixViewError::InvalidStride)
        );
        assert_eq!(
            Bf16MfmaBColumnMajorMatrix::column_major(&[0; 5], 0, 3, 2, 3).err(),
            Some(Bf16MatrixViewError::OutOfBounds {
                required: 6,
                actual: 5
            })
        );
        assert_eq!(
            Bf16MfmaBColumnMajorMatrix::column_major(&[], 1, 1, 2, usize::MAX).err(),
            Some(Bf16MatrixViewError::ExtentOverflow)
        );
    }

    #[test]
    fn column_major_b_empty_views_check_only_the_offset_and_zero_fill() {
        let bits = [u16::MAX; 3];
        for (reduction, columns) in [(0, 0), (0, usize::MAX), (usize::MAX, 0)] {
            let view =
                Bf16MfmaBColumnMajorMatrix::column_major(&bits, bits.len(), reduction, columns, 0)
                    .unwrap();
            assert_eq!(
                Bf16MfmaBColumnMajorMatrix::column_major(
                    &bits,
                    bits.len() + 1,
                    reduction,
                    columns,
                    0,
                )
                .err(),
                Some(Bf16MatrixViewError::OutOfBounds {
                    required: 4,
                    actual: 3
                }),
            );
            for lane in 0..64 {
                let lane = WaveLane::<Wave64>::from_model_snapshot(lane).unwrap();
                assert_eq!(
                    view.load_k16n16(&lane, 3, 5)
                        .into_array()
                        .map(Bf16::to_bits),
                    [0; 4]
                );
            }
        }
    }

    #[test]
    fn column_major_b_zero_fills_edges_and_overflow_without_padding_reads() {
        let (k, n, stride, offset) = (19, 17, 23, 3);
        let bits = (0..offset + n * stride)
            .map(|index| (index + 1) as u16)
            .collect::<Vec<_>>();
        let view = Bf16MfmaBColumnMajorMatrix::column_major(&bits, offset, k, n, stride).unwrap();
        for lane in 0..64 {
            let wave_lane = WaveLane::<Wave64>::from_model_snapshot(lane).unwrap();
            let actual = view
                .load_k16n16(&wave_lane, 16, 16)
                .into_array()
                .map(Bf16::to_bits);
            for (component, value) in actual.into_iter().enumerate() {
                let column = 16 + lane as usize % 16;
                let reduction = 16 + (lane as usize / 16) * 4 + component;
                let expected = if column < n && reduction < k {
                    bits[offset + column * stride + reduction]
                } else {
                    0
                };
                assert_eq!(value, expected);
            }
            for (reduction, column) in [(usize::MAX, 0), (0, usize::MAX), (usize::MAX, usize::MAX)]
            {
                assert_eq!(
                    view.load_k16n16(&wave_lane, reduction, column)
                        .into_array()
                        .map(Bf16::to_bits),
                    [0; 4]
                );
            }
        }
    }
}
