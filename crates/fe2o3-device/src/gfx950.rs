//! Bounded source contracts for gfx950 low-precision matrix operations.
//!
//! This module describes one Wave64 `16x16x128` FP4/FP8 MFMA profile and the
//! matching B4/B8 LDS transpose-read path. The compiler terminals deliberately
//! panic when they are not replaced by authenticated device lowering. Defining
//! these types does not claim that the current rustc backend performs that
//! lowering.

use core::marker::PhantomData;

use crate::{
    Wave64, WaveLane,
    views::{CheckedStridedExtentError, check_strided_2d_extent},
};

type Gfx950WaveContract<'wave, Association> =
    PhantomData<fn(&'wave WaveLane<Wave64>, Association) -> &'wave WaveLane<Wave64>>;

/// Version of the bounded gfx950 low-precision source contract.
pub const GFX950_LOW_PRECISION_CONTRACT_VERSION_V1: u16 = 1;
/// Rows in the exact gfx950 scaled-MFMA output tile.
pub const GFX950_MFMA_M: usize = 16;
/// Columns in the exact gfx950 scaled-MFMA output tile.
pub const GFX950_MFMA_N: usize = 16;
/// Reduction extent in the exact gfx950 scaled-MFMA instruction.
pub const GFX950_MFMA_K: usize = 128;
/// Physical lanes participating in the exact gfx950 scaled-MFMA instruction.
pub const GFX950_MFMA_WAVE_LANES: usize = 64;
/// VGPR dwords consumed by each LLVM scaled-MFMA operand.
pub const GFX950_MFMA_OPERAND_DWORDS: usize = 8;
/// Largest contiguous power-of-two subgroup admitted by gfx950 V1 terminals.
pub const GFX950_SUBGROUP_MAX_WIDTH: u32 = 64;

mod sealed {
    pub trait Format {}
    pub trait OperandRole {}
    pub trait SubgroupWidth {}
    pub trait TransposeState {}
}

/// Type-level gfx950 subgroup width used to reject unsupported widths.
pub struct Gfx950SubgroupWidth<const WIDTH: u32>;

/// A sealed power-of-two subgroup width in `1..=64`.
pub trait Gfx950ValidSubgroupWidth: sealed::SubgroupWidth {}

macro_rules! impl_valid_subgroup_width {
    ($($width:literal),+ $(,)?) => {
        $(
            impl sealed::SubgroupWidth for Gfx950SubgroupWidth<$width> {}
            impl Gfx950ValidSubgroupWidth for Gfx950SubgroupWidth<$width> {}
        )+
    };
}

impl_valid_subgroup_width!(1, 2, 4, 8, 16, 32, 64);

/// A sealed low-precision format admitted by the exact gfx950 MFMA profile.
pub trait Gfx950MfmaFormat: sealed::Format {
    /// Number of meaningful bits in each source byte's format encoding.
    const ENCODING_BITS: usize;
    /// Number of operand dwords containing meaningful packed values.
    const MEANINGFUL_DWORDS: usize;
}

/// OCP E2M1 FP4 format identity for gfx950 scaled MFMA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_gfx950_fp4_e2m1_format_v1"]
pub enum Gfx950Fp4E2M1 {}

impl sealed::Format for Gfx950Fp4E2M1 {}

impl Gfx950MfmaFormat for Gfx950Fp4E2M1 {
    const ENCODING_BITS: usize = 4;
    const MEANINGFUL_DWORDS: usize = 4;
}

/// OCP E4M3 FP8 format identity for gfx950 scaled MFMA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_gfx950_fp8_e4m3_format_v1"]
pub enum Gfx950Fp8E4M3 {}

impl sealed::Format for Gfx950Fp8E4M3 {}

impl Gfx950MfmaFormat for Gfx950Fp8E4M3 {
    const ENCODING_BITS: usize = 8;
    const MEANINGFUL_DWORDS: usize = 8;
}

/// Left-hand, row-by-reduction operand role for gfx950 scaled MFMA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_operand_a_role_v1"]
pub enum Gfx950MfmaOperandA {}

impl sealed::OperandRole for Gfx950MfmaOperandA {}

/// Right-hand, reduction-by-column operand role for gfx950 scaled MFMA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_operand_b_role_v1"]
pub enum Gfx950MfmaOperandB {}

impl sealed::OperandRole for Gfx950MfmaOperandB {}

/// One role- and format-associated gfx950 scaled-MFMA operand fragment.
///
/// LLVM consumes eight i32 values for both FP4 and FP8. FP4 uses only dwords
/// zero through three; safe constructors always keep dwords four through seven
/// zero. The payload is private so source code cannot forge that invariant or
/// exchange A and B roles.
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_fragment_v1"]
pub struct Gfx950MfmaFragment<'wave, Format, Role> {
    registers: [u32; GFX950_MFMA_OPERAND_DWORDS],
    _contract: Gfx950WaveContract<'wave, (Format, Role)>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'wave, Format, Role: sealed::OperandRole> Gfx950MfmaFragment<'wave, Format, Role> {
    fn from_registers(
        _lane: &'wave WaveLane<Wave64>,
        registers: [u32; GFX950_MFMA_OPERAND_DWORDS],
    ) -> Self {
        Self {
            registers,
            _contract: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    fn into_registers(self) -> [u32; GFX950_MFMA_OPERAND_DWORDS] {
        self.registers
    }
}

/// Canonical FP4 A operand fragment.
pub type Gfx950Fp4MfmaAFragment<'wave> =
    Gfx950MfmaFragment<'wave, Gfx950Fp4E2M1, Gfx950MfmaOperandA>;
/// Canonical FP4 B operand fragment.
pub type Gfx950Fp4MfmaBFragment<'wave> =
    Gfx950MfmaFragment<'wave, Gfx950Fp4E2M1, Gfx950MfmaOperandB>;
/// Canonical FP8 A operand fragment.
pub type Gfx950Fp8MfmaAFragment<'wave> =
    Gfx950MfmaFragment<'wave, Gfx950Fp8E4M3, Gfx950MfmaOperandA>;
/// Canonical FP8 B operand fragment.
pub type Gfx950Fp8MfmaBFragment<'wave> =
    Gfx950MfmaFragment<'wave, Gfx950Fp8E4M3, Gfx950MfmaOperandB>;

/// Four FP32 accumulator values associated with one gfx950 input format.
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_gfx950_f32_accumulator_fragment_v1"]
pub struct Gfx950F32AccumulatorFragment<'wave, Format> {
    values: [f32; 4],
    _contract: Gfx950WaveContract<'wave, Format>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'wave, Format: Gfx950MfmaFormat> Gfx950F32AccumulatorFragment<'wave, Format> {
    fn zero_inner(_lane: &'wave WaveLane<Wave64>) -> Self {
        Self {
            values: [0.0; 4],
            _contract: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    fn into_values_inner(self) -> [f32; 4] {
        self.values
    }

    #[cfg(test)]
    fn from_values(_lane: &'wave WaveLane<Wave64>, values: [f32; 4]) -> Self {
        Self {
            values,
            _contract: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

impl<'wave> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1> {
    /// Creates the all-zero FP4 accumulator for one authenticated Wave64 lane.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_fp4_f32_accumulator_zero_v1"]
    pub fn zero(lane: &'wave WaveLane<Wave64>) -> Self {
        Self::zero_inner(lane)
    }

    /// Returns this lane's four row-major FP4 MFMA results.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_fp4_f32_accumulator_into_values_v1"]
    pub fn into_values(self) -> [f32; 4] {
        self.into_values_inner()
    }
}

impl<'wave> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp8E4M3> {
    /// Creates the all-zero FP8 accumulator for one authenticated Wave64 lane.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_f32_accumulator_zero_v1"]
    pub fn zero(lane: &'wave WaveLane<Wave64>) -> Self {
        Self::zero_inner(lane)
    }

    /// Returns this lane's four row-major FP8 MFMA results.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_f32_accumulator_into_values_v1"]
    pub fn into_values(self) -> [f32; 4] {
        self.into_values_inner()
    }
}

/// Rejection while establishing a checked row-major gfx950 matrix view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_view_error_v1"]
pub enum Gfx950MatrixViewError {
    /// A nonempty matrix has a stride smaller than its logical column count.
    InvalidStride,
    /// Offset, extent, or address arithmetic overflowed `usize`.
    ExtentOverflow,
    /// The logical matrix is not fully contained in the supplied allocation.
    OutOfBounds {
        /// Minimum number of bytes required by the view.
        required: usize,
        /// Actual number of bytes in the supplied allocation.
        actual: usize,
    },
}

impl From<Gfx950MatrixViewError> for crate::KernelError {
    fn from(_: Gfx950MatrixViewError) -> Self {
        Self::InvalidArgument
    }
}

#[derive(Clone, Copy)]
struct CheckedByteMatrix<'data> {
    bits: &'data [u8],
    offset: usize,
    rows: usize,
    columns: usize,
    stride: usize,
}

impl<'data> CheckedByteMatrix<'data> {
    fn row_major(
        bits: &'data [u8],
        offset: usize,
        rows: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        check_strided_2d_extent(offset, rows, columns, stride, bits.len()).map_err(|error| {
            match error {
                CheckedStridedExtentError::InvalidStride => Gfx950MatrixViewError::InvalidStride,
                CheckedStridedExtentError::ExtentOverflow => Gfx950MatrixViewError::ExtentOverflow,
                CheckedStridedExtentError::OutOfBounds { required, actual } => {
                    Gfx950MatrixViewError::OutOfBounds { required, actual }
                }
            }
        })?;
        Ok(Self {
            bits,
            offset,
            rows,
            columns,
            stride,
        })
    }

    fn value_or_zero(&self, row: Option<usize>, column: Option<usize>) -> u8 {
        let Some((row, column)) = row.zip(column) else {
            return 0;
        };
        if row >= self.rows || column >= self.columns {
            return 0;
        }
        row.checked_mul(self.stride)
            .and_then(|index| self.offset.checked_add(index))
            .and_then(|index| index.checked_add(column))
            .and_then(|index| self.bits.get(index))
            .copied()
            .unwrap_or(0)
    }
}

/// Checked row-major A matrix carrying its gfx950 low-precision format.
///
/// Each logical value occupies one source byte. FP4 values use the low nibble;
/// fragment loads perform the hardware's dense two-values-per-byte packing.
#[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_a_view_v1"]
pub struct Gfx950MfmaAMatrix<'data, Format> {
    matrix: CheckedByteMatrix<'data>,
    _format: PhantomData<Format>,
}

impl<'data, Format: Gfx950MfmaFormat> Gfx950MfmaAMatrix<'data, Format> {
    fn row_major_inner(
        bits: &'data [u8],
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        Ok(Self {
            matrix: CheckedByteMatrix::row_major(bits, offset, rows, reduction, stride)?,
            _format: PhantomData,
        })
    }
}

impl<'data> Gfx950MfmaAMatrix<'data, Gfx950Fp4E2M1> {
    /// Validates row-major FP4 `rows x reduction` byte storage.
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_a_fp4_row_major_v1"]
    #[inline(never)]
    pub fn row_major(
        bits: &'data [u8],
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        Self::row_major_inner(bits, offset, rows, reduction, stride)
    }

    /// Loads this lane's FP4 values from one logical M16xK128 A tile.
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_a_fp4_load_m16k128_v1"]
    #[inline(never)]
    pub fn load_m16k128<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64>,
        row_base: usize,
        reduction_base: usize,
    ) -> Gfx950Fp4MfmaAFragment<'wave> {
        Gfx950MfmaFragment::from_registers(
            lane,
            pack_fp4_a(&self.matrix, lane.get() as usize, row_base, reduction_base),
        )
    }
}

impl<'data> Gfx950MfmaAMatrix<'data, Gfx950Fp8E4M3> {
    /// Validates row-major FP8 `rows x reduction` byte storage.
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_a_row_major_v1"]
    #[inline(never)]
    pub fn row_major(
        bits: &'data [u8],
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        Self::row_major_inner(bits, offset, rows, reduction, stride)
    }

    /// Loads this lane's FP8 values from one logical M16xK128 A tile.
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_a_fp8_load_m16k128_v1"]
    #[inline(never)]
    pub fn load_m16k128<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64>,
        row_base: usize,
        reduction_base: usize,
    ) -> Gfx950Fp8MfmaAFragment<'wave> {
        Gfx950MfmaFragment::from_registers(
            lane,
            pack_fp8_a(&self.matrix, lane.get() as usize, row_base, reduction_base),
        )
    }
}

/// Checked row-major B matrix carrying its gfx950 low-precision format.
///
/// Each logical value occupies one source byte. FP4 values use the low nibble;
/// fragment loads perform the hardware's dense two-values-per-byte packing.
#[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_b_view_v1"]
pub struct Gfx950MfmaBMatrix<'data, Format> {
    matrix: CheckedByteMatrix<'data>,
    _format: PhantomData<Format>,
}

impl<'data, Format: Gfx950MfmaFormat> Gfx950MfmaBMatrix<'data, Format> {
    fn row_major_inner(
        bits: &'data [u8],
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        Ok(Self {
            matrix: CheckedByteMatrix::row_major(bits, offset, reduction, columns, stride)?,
            _format: PhantomData,
        })
    }
}

impl<'data> Gfx950MfmaBMatrix<'data, Gfx950Fp4E2M1> {
    /// Validates row-major FP4 `reduction x columns` byte storage.
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_b_fp4_row_major_v1"]
    #[inline(never)]
    pub fn row_major(
        bits: &'data [u8],
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        Self::row_major_inner(bits, offset, reduction, columns, stride)
    }

    /// Loads this lane's FP4 values from one logical K128xN16 B tile.
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_b_fp4_load_k128n16_v1"]
    #[inline(never)]
    pub fn load_k128n16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64>,
        reduction_base: usize,
        column_base: usize,
    ) -> Gfx950Fp4MfmaBFragment<'wave> {
        Gfx950MfmaFragment::from_registers(
            lane,
            pack_fp4_b(
                &self.matrix,
                lane.get() as usize,
                reduction_base,
                column_base,
            ),
        )
    }
}

impl<'data> Gfx950MfmaBMatrix<'data, Gfx950Fp8E4M3> {
    /// Validates row-major FP8 `reduction x columns` byte storage.
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_b_row_major_v1"]
    #[inline(never)]
    pub fn row_major(
        bits: &'data [u8],
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        Self::row_major_inner(bits, offset, reduction, columns, stride)
    }

    /// Loads this lane's FP8 values from one logical K128xN16 B tile.
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_b_fp8_load_k128n16_v1"]
    #[inline(never)]
    pub fn load_k128n16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64>,
        reduction_base: usize,
        column_base: usize,
    ) -> Gfx950Fp8MfmaBFragment<'wave> {
        Gfx950MfmaFragment::from_registers(
            lane,
            pack_fp8_b(
                &self.matrix,
                lane.get() as usize,
                reduction_base,
                column_base,
            ),
        )
    }
}

/// Checked row-major FP4 A matrix.
pub type Gfx950Fp4MfmaAMatrix<'data> = Gfx950MfmaAMatrix<'data, Gfx950Fp4E2M1>;
/// Checked row-major FP4 B matrix.
pub type Gfx950Fp4MfmaBMatrix<'data> = Gfx950MfmaBMatrix<'data, Gfx950Fp4E2M1>;
/// Checked row-major FP8 A matrix.
pub type Gfx950Fp8MfmaAMatrix<'data> = Gfx950MfmaAMatrix<'data, Gfx950Fp8E4M3>;
/// Checked row-major FP8 B matrix.
pub type Gfx950Fp8MfmaBMatrix<'data> = Gfx950MfmaBMatrix<'data, Gfx950Fp8E4M3>;

/// Compiler-created authority for the exact gfx950 scaled-MFMA profile.
#[rustc_diagnostic_item = "fe2o3_device_gfx950_matrix_context_v1"]
pub struct Gfx950Matrix {
    _private: (),
    _not_send_sync: PhantomData<*mut ()>,
}

impl Gfx950Matrix {
    /// Acquires gfx950 matrix authority from authenticated compiler lowering.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_matrix_context_current_v1"]
    pub fn current() -> Self {
        unreachable!("Gfx950Matrix must be created by authenticated gfx950 device lowering")
    }

    /// Performs one full-wave FP4 scaled MFMA with identity scales.
    #[must_use]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_fp4_f32_m16n16k128_v1"]
    pub fn multiply_accumulate_fp4<'wave>(
        &self,
        lhs: Gfx950Fp4MfmaAFragment<'wave>,
        rhs: Gfx950Fp4MfmaBFragment<'wave>,
        accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1>,
    ) -> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1> {
        let _ = (
            self,
            lhs.into_registers(),
            rhs.into_registers(),
            accumulator.values,
        );
        unreachable!("gfx950 FP4 MFMA requires authenticated compiler lowering")
    }

    /// Performs one full-wave FP8 scaled MFMA with identity scales.
    #[must_use]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_fp8_f32_m16n16k128_v1"]
    pub fn multiply_accumulate_fp8<'wave>(
        &self,
        lhs: Gfx950Fp8MfmaAFragment<'wave>,
        rhs: Gfx950Fp8MfmaBFragment<'wave>,
        accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp8E4M3>,
    ) -> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp8E4M3> {
        let _ = (
            self,
            lhs.into_registers(),
            rhs.into_registers(),
            accumulator.values,
        );
        unreachable!("gfx950 FP8 MFMA requires authenticated compiler lowering")
    }
}

/// Compiler-created authority for bounded gfx950 Wave64 subgroup operations.
///
/// The capability is move-only and cannot be constructed from caller-provided
/// identity. The sealed width witness admits only powers of two in `1..=64`;
/// authenticated lowering must also prove convergent execution and a native
/// Wave64 target. This source contract does not claim current backend support.
///
/// Unsupported subgroup widths are rejected during type checking:
///
/// ```compile_fail
/// use fe2o3_device::gfx950::Gfx950Subgroup;
///
/// fn invalid_width(subgroup: &Gfx950Subgroup) {
///     let _ = subgroup.reduce_sum_f32::<3>(1.0);
/// }
/// ```
#[rustc_diagnostic_item = "fe2o3_device_gfx950_subgroup_context_v1"]
pub struct Gfx950Subgroup {
    _private: (),
    _not_send_sync: PhantomData<*mut ()>,
}

impl Gfx950Subgroup {
    /// Acquires subgroup authority from authenticated gfx950 device lowering.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_subgroup_current_v1"]
    pub fn current() -> Self {
        unreachable!("gfx950 subgroup authority requires authenticated compiler lowering")
    }

    /// Returns the ordered maximum to every lane in each contiguous subgroup.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_subgroup_reduce_max_f32_v1"]
    pub fn reduce_max_f32<const WIDTH: u32>(&self, value: f32) -> f32
    where
        Gfx950SubgroupWidth<WIDTH>: Gfx950ValidSubgroupWidth,
    {
        let _ = (self, value, WIDTH);
        unreachable!("gfx950 subgroup maximum requires authenticated compiler lowering")
    }

    /// Returns the FP32 sum to every lane in each contiguous subgroup.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_subgroup_reduce_sum_f32_v1"]
    pub fn reduce_sum_f32<const WIDTH: u32>(&self, value: f32) -> f32
    where
        Gfx950SubgroupWidth<WIDTH>: Gfx950ValidSubgroupWidth,
    {
        let _ = (self, value, WIDTH);
        unreachable!("gfx950 subgroup sum requires authenticated compiler lowering")
    }

    /// Broadcasts one value from `source_lane` within each contiguous subgroup.
    ///
    /// Authenticated lowering rejects `source_lane >= WIDTH` and any unsupported
    /// `WIDTH`; it must not silently wrap or clamp either value.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_subgroup_broadcast_f32_v1"]
    pub fn broadcast_f32<const WIDTH: u32>(&self, value: f32, source_lane: u32) -> f32
    where
        Gfx950SubgroupWidth<WIDTH>: Gfx950ValidSubgroupWidth,
    {
        let _ = (self, value, source_lane, WIDTH);
        unreachable!("gfx950 subgroup broadcast requires authenticated compiler lowering")
    }

    #[cfg(test)]
    fn for_host_test() -> Self {
        Self {
            _private: (),
            _not_send_sync: PhantomData,
        }
    }
}

/// Initial state of a typed gfx950 LDS transpose tile.
#[derive(Debug)]
pub enum Gfx950TransposeUninitialized {}
/// State after every lane has staged its format-specific source values.
#[derive(Debug)]
pub enum Gfx950TransposeStaged {}
/// State after a uniform workgroup publish barrier.
#[derive(Debug)]
pub enum Gfx950TransposePublished {}

impl sealed::TransposeState for Gfx950TransposeUninitialized {}
impl sealed::TransposeState for Gfx950TransposeStaged {}
impl sealed::TransposeState for Gfx950TransposePublished {}

/// Move-only capability for one exact gfx950 B4 or B8 LDS transpose tile.
///
/// Safe code can only advance `Uninitialized -> Staged -> Published`, and only
/// a published tile exposes the matching B operand fragment. The backend must
/// replace the terminals with one private address-space-3 allocation, the exact
/// inverse staging permutation, a uniform barrier, and format-specific
/// `ds_read_b64_tr_b4` or `ds_read_b64_tr_b8` instructions.
///
/// The lane lifetime is invariant, so a tile cannot outlive the authenticated
/// wave witness that issued it.
///
/// ```compile_fail
/// use fe2o3_device::{
///     Gfx950Fp4E2M1, Gfx950LdsTransposeTile, Gfx950TransposeUninitialized,
/// };
///
/// fn widen<'wave>(
///     tile: Gfx950LdsTransposeTile<'wave, Gfx950Fp4E2M1, Gfx950TransposeUninitialized>,
/// ) -> Gfx950LdsTransposeTile<'static, Gfx950Fp4E2M1, Gfx950TransposeUninitialized> {
///     tile
/// }
/// ```
///
/// A published tile is also consumed when it is used:
///
/// ```compile_fail
/// use fe2o3_device::{
///     Gfx950Fp4E2M1, Gfx950LdsTransposeTile, Gfx950TransposePublished,
/// };
///
/// fn reuse(tile: Gfx950LdsTransposeTile<'_, Gfx950Fp4E2M1, Gfx950TransposePublished>) {
///     let consumed = tile;
///     let reused = tile;
///     let _ = (consumed, reused);
/// }
/// ```
#[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_tile_v1"]
pub struct Gfx950LdsTransposeTile<'wave, Format, State>
where
    Format: Gfx950MfmaFormat,
    State: sealed::TransposeState,
{
    _contract: Gfx950WaveContract<'wave, (Format, State)>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'wave, Format: Gfx950MfmaFormat>
    Gfx950LdsTransposeTile<'wave, Format, Gfx950TransposeUninitialized>
{
    /// Acquires one compiler-owned transpose tile for the current wave.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_tile_current_v1"]
    pub fn current(_lane: &'wave WaveLane<Wave64>) -> Self {
        unreachable!("gfx950 LDS transpose storage requires authenticated compiler lowering")
    }
}

impl<'wave> Gfx950LdsTransposeTile<'wave, Gfx950Fp4E2M1, Gfx950TransposeUninitialized> {
    /// Stages one token-major FP4 K tile using the inverse B4 permutation.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_stage_b4_v1"]
    pub fn stage_k_transposed(
        self,
        matrix: &Gfx950Fp4MfmaAMatrix<'_>,
        token_base: usize,
        reduction_base: usize,
    ) -> Gfx950LdsTransposeTile<'wave, Gfx950Fp4E2M1, Gfx950TransposeStaged> {
        let _ = (self, matrix, token_base, reduction_base);
        unreachable!("gfx950 B4 inverse transpose staging requires authenticated lowering")
    }
}

impl<'wave> Gfx950LdsTransposeTile<'wave, Gfx950Fp8E4M3, Gfx950TransposeUninitialized> {
    /// Stages one token-major FP8 K tile using the inverse B8 permutation.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_stage_b8_v1"]
    pub fn stage_k_transposed(
        self,
        matrix: &Gfx950Fp8MfmaAMatrix<'_>,
        token_base: usize,
        reduction_base: usize,
    ) -> Gfx950LdsTransposeTile<'wave, Gfx950Fp8E4M3, Gfx950TransposeStaged> {
        let _ = (self, matrix, token_base, reduction_base);
        unreachable!("gfx950 B8 inverse transpose staging requires authenticated lowering")
    }
}

impl<'wave, Format: Gfx950MfmaFormat> Gfx950LdsTransposeTile<'wave, Format, Gfx950TransposeStaged> {
    /// Publishes all staged values through one uniform workgroup barrier.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_publish_v1"]
    pub fn publish(self) -> Gfx950LdsTransposeTile<'wave, Format, Gfx950TransposePublished> {
        let _ = self;
        unreachable!("gfx950 LDS transpose publish requires authenticated compiler lowering")
    }
}

impl<'wave> Gfx950LdsTransposeTile<'wave, Gfx950Fp4E2M1, Gfx950TransposePublished> {
    /// Reads the published tile with two B4 transpose loads.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_read_b4_v1"]
    pub fn read_mfma_fragment(self) -> Gfx950Fp4MfmaBFragment<'wave> {
        let _ = self;
        unreachable!("gfx950 B4 transpose reads require authenticated compiler lowering")
    }
}

impl<'wave> Gfx950LdsTransposeTile<'wave, Gfx950Fp8E4M3, Gfx950TransposePublished> {
    /// Reads the published tile with four B8 transpose loads.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_read_b8_v1"]
    pub fn read_mfma_fragment(self) -> Gfx950Fp8MfmaBFragment<'wave> {
        let _ = self;
        unreachable!("gfx950 B8 transpose reads require authenticated compiler lowering")
    }
}

fn fp8_depth(group: usize, item: usize) -> usize {
    if item < 16 {
        group * 16 + item
    } else {
        64 + group * 16 + (item - 16)
    }
}

fn pack_fp4_a(
    matrix: &CheckedByteMatrix<'_>,
    lane: usize,
    row_base: usize,
    reduction_base: usize,
) -> [u32; GFX950_MFMA_OPERAND_DWORDS] {
    let mut registers = [0; GFX950_MFMA_OPERAND_DWORDS];
    let row = row_base.checked_add(lane & 15);
    let depth_base = reduction_base.checked_add((lane / 16) * 32);
    let mut item = 0;
    while item < 32 {
        let value =
            matrix.value_or_zero(row, depth_base.and_then(|base| base.checked_add(item))) & 15;
        registers[item / 8] |= (value as u32) << ((item % 8) * 4);
        item += 1;
    }
    registers
}

fn pack_fp4_b(
    matrix: &CheckedByteMatrix<'_>,
    lane: usize,
    reduction_base: usize,
    column_base: usize,
) -> [u32; GFX950_MFMA_OPERAND_DWORDS] {
    let mut registers = [0; GFX950_MFMA_OPERAND_DWORDS];
    let column = column_base.checked_add(lane & 15);
    let depth_base = reduction_base.checked_add((lane / 16) * 32);
    let mut item = 0;
    while item < 32 {
        let value =
            matrix.value_or_zero(depth_base.and_then(|base| base.checked_add(item)), column) & 15;
        registers[item / 8] |= (value as u32) << ((item % 8) * 4);
        item += 1;
    }
    registers
}

fn pack_fp8_a(
    matrix: &CheckedByteMatrix<'_>,
    lane: usize,
    row_base: usize,
    reduction_base: usize,
) -> [u32; GFX950_MFMA_OPERAND_DWORDS] {
    let mut registers = [0; GFX950_MFMA_OPERAND_DWORDS];
    let row = row_base.checked_add(lane & 15);
    let group = lane / 16;
    let mut item = 0;
    while item < 32 {
        let depth = reduction_base.checked_add(fp8_depth(group, item));
        let value = matrix.value_or_zero(row, depth);
        registers[item / 4] |= (value as u32) << ((item % 4) * 8);
        item += 1;
    }
    registers
}

fn pack_fp8_b(
    matrix: &CheckedByteMatrix<'_>,
    lane: usize,
    reduction_base: usize,
    column_base: usize,
) -> [u32; GFX950_MFMA_OPERAND_DWORDS] {
    let mut registers = [0; GFX950_MFMA_OPERAND_DWORDS];
    let column = column_base.checked_add(lane & 15);
    let group = lane / 16;
    let mut item = 0;
    while item < 32 {
        let depth = reduction_base.checked_add(fp8_depth(group, item));
        let value = matrix.value_or_zero(depth, column);
        registers[item / 4] |= (value as u32) << ((item % 4) * 8);
        item += 1;
    }
    registers
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::mem::{align_of, size_of};
    use std::panic::{AssertUnwindSafe, catch_unwind};

    fn bytes(registers: [u32; 8]) -> [u8; 32] {
        let mut result = [0; 32];
        let mut index = 0;
        while index < 32 {
            result[index] = (registers[index / 4] >> ((index % 4) * 8)) as u8;
            index += 1;
        }
        result
    }

    #[test]
    fn fragments_have_the_compiler_observed_v8i32_layout() {
        assert_eq!(size_of::<Gfx950Fp4MfmaAFragment<'_>>(), 32);
        assert_eq!(size_of::<Gfx950Fp8MfmaAFragment<'_>>(), 32);
        assert_eq!(align_of::<Gfx950Fp4MfmaAFragment<'_>>(), 4);
        assert_eq!(
            size_of::<Gfx950F32AccumulatorFragment<'_, Gfx950Fp4E2M1>>(),
            16
        );
        assert_eq!(Gfx950Fp4E2M1::MEANINGFUL_DWORDS, 4);
        assert_eq!(Gfx950Fp8E4M3::MEANINGFUL_DWORDS, 8);
    }

    #[test]
    fn fp4_packing_uses_four_dwords_and_zeroes_the_upper_half() {
        let data: [u8; 128] = core::array::from_fn(|index| index as u8 & 15);
        let matrix = CheckedByteMatrix::row_major(&data, 0, 1, 128, 128).unwrap();
        let registers = pack_fp4_a(&matrix, 32, 0, 0);
        assert_eq!(registers[0], 0x7654_3210);
        assert_eq!(registers[1], 0xfedc_ba98);
        assert_eq!(registers[2], 0x7654_3210);
        assert_eq!(registers[3], 0xfedc_ba98);
        assert_eq!(&registers[4..], &[0; 4]);
    }

    #[test]
    fn fp8_packing_uses_the_documented_split_depth_map() {
        let data: [u8; 128] = core::array::from_fn(|index| index as u8);
        let matrix = CheckedByteMatrix::row_major(&data, 0, 1, 128, 128).unwrap();
        let packed = bytes(pack_fp8_a(&matrix, 32, 0, 0));
        let expected: [u8; 32] = core::array::from_fn(|item| {
            if item < 16 {
                32 + item as u8
            } else {
                96 + (item - 16) as u8
            }
        });
        assert_eq!(packed, expected);
    }

    #[test]
    fn b_packing_uses_column_role_and_all_lane_groups() {
        let data: [u8; 128 * 16] =
            core::array::from_fn(|index| ((index / 16) * 3 + index % 16) as u8);
        let matrix = CheckedByteMatrix::row_major(&data, 0, 128, 16, 16).unwrap();

        let fp4 = pack_fp4_b(&matrix, 50, 0, 0);
        let expected_fp4: [u8; 32] =
            core::array::from_fn(|item| (((96 + item) * 3 + 2) & 15) as u8);
        let mut unpacked_fp4 = [0_u8; 32];
        for (item, output) in unpacked_fp4.iter_mut().enumerate() {
            *output = ((fp4[item / 8] >> ((item % 8) * 4)) & 15) as u8;
        }
        assert_eq!(unpacked_fp4, expected_fp4);
        assert_eq!(&fp4[4..], &[0; 4]);

        let fp8 = bytes(pack_fp8_b(&matrix, 50, 0, 0));
        let expected_fp8: [u8; 32] = core::array::from_fn(|item| {
            let depth = if item < 16 {
                48 + item
            } else {
                112 + item - 16
            };
            (depth * 3 + 2) as u8
        });
        assert_eq!(fp8, expected_fp8);
    }

    #[test]
    fn fragment_packing_zero_fills_out_of_tile_edges() {
        let data: [u8; 100 * 16] = core::array::from_fn(|index| index as u8);
        let matrix = CheckedByteMatrix::row_major(&data, 0, 100, 16, 16).unwrap();
        let packed = bytes(pack_fp8_b(&matrix, 48, 0, 0));
        assert!(packed[..16].iter().any(|value| *value != 0));
        assert_eq!(&packed[16..], &[0; 16]);
    }

    #[test]
    fn checked_views_reject_stride_overflow_and_short_storage() {
        assert!(matches!(
            Gfx950Fp8MfmaAMatrix::row_major(&[0; 16], 0, 2, 8, 7),
            Err(Gfx950MatrixViewError::InvalidStride)
        ));
        assert!(matches!(
            Gfx950Fp8MfmaAMatrix::row_major(&[0; 16], usize::MAX, 1, 1, 1),
            Err(Gfx950MatrixViewError::ExtentOverflow)
        ));
        assert!(matches!(
            Gfx950Fp4MfmaBMatrix::row_major(&[0; 15], 0, 2, 8, 8),
            Err(Gfx950MatrixViewError::OutOfBounds {
                required: 16,
                actual: 15
            })
        ));
    }

    #[test]
    fn transpose_typestate_is_move_only_and_format_specific() {
        fn accepts_fp4(_: Gfx950LdsTransposeTile<'_, Gfx950Fp4E2M1, Gfx950TransposePublished>) {}
        fn accepts_fp8(_: Gfx950LdsTransposeTile<'_, Gfx950Fp8E4M3, Gfx950TransposePublished>) {}
        let _ = (accepts_fp4, accepts_fp8);
    }

    #[test]
    fn private_accumulator_constructor_preserves_values() {
        let lane = WaveLane::<Wave64>::from_model_snapshot(0).unwrap();
        let accumulator =
            Gfx950F32AccumulatorFragment::<Gfx950Fp8E4M3>::from_values(&lane, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(accumulator.into_values(), [1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn gfx950_subgroup_terminals_fail_closed_on_host() {
        assert_eq!(GFX950_SUBGROUP_MAX_WIDTH, 64);
        assert!(catch_unwind(Gfx950Subgroup::current).is_err());
        let subgroup = Gfx950Subgroup::for_host_test();
        assert!(catch_unwind(AssertUnwindSafe(|| subgroup.reduce_max_f32::<16>(1.0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| subgroup.reduce_sum_f32::<16>(1.0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| subgroup.broadcast_f32::<16>(1.0, 3))).is_err());
    }
}
