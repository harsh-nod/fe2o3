//! Bounded source contracts for gfx950 low-precision matrix operations.
//!
//! This module describes one Wave64 `16x16x128` FP4/FP8 MFMA profile and the
//! matching B4/B8 LDS transpose-read path. The compiler terminals deliberately
//! panic when they are not replaced by authenticated device lowering. Defining
//! these types does not claim that the current rustc backend performs that
//! lowering.

use core::marker::PhantomData;

use crate::{
    Global, MatrixCapability, MatrixGlobalAccess, NextEpoch, NumericalPolicy,
    PolicyMatrixCapability, ReadOnly, Subgroup, SubgroupBrand, SubgroupWidth64,
    SynchronizationEpoch, Wave64, WaveLane, WorkgroupCapability, WorkgroupEpoch,
    context::UnbrandedCapability,
    views::{CheckedStridedExtentError, check_strided_2d_extent},
};

/// Canonical imports for kernels that explicitly select gfx950 operations.
///
/// Every item is a direct re-export of its branded capability type. This
/// module defines no compatibility aliases and is intentionally separate from
/// the target-neutral [`crate::prelude`].
pub mod prelude;

type Invariant<T> = fn(T) -> T;
type InvariantLifetime<'scope> = fn(&'scope mut ()) -> &'scope mut ();
type Gfx950WaveContract<'wave, Association, Brand> =
    PhantomData<(InvariantLifetime<'wave>, Invariant<(Association, Brand)>)>;
type Gfx950SubgroupBorrow<'operation, 'workgroup, KernelBrand, Epoch> = (
    &'operation Subgroup<'workgroup, SubgroupWidth64, KernelBrand, Epoch>,
    &'operation WorkgroupEpoch<'workgroup, KernelBrand, Epoch>,
);
type Gfx950SubgroupContract<'operation, 'workgroup, KernelBrand, Epoch> = PhantomData<
    fn(
        Gfx950SubgroupBorrow<'operation, 'workgroup, KernelBrand, Epoch>,
    ) -> Gfx950SubgroupBorrow<'operation, 'workgroup, KernelBrand, Epoch>,
>;

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
/// Exact contiguous subgroup partition admitted by gfx950 V1 terminals.
pub const GFX950_WAVE16_WIDTH: u32 = 16;
/// Legacy maximum used by the retired const-generic subgroup surface.
#[deprecated(note = "gfx950 capability operations use the exact GFX950_WAVE16_WIDTH")]
pub const GFX950_SUBGROUP_MAX_WIDTH: u32 = 64;

mod sealed {
    pub trait Format {}
    pub trait OperandRole {}
    pub trait SubgroupWidth {}
    pub trait TransposeState {}
}

/// Type-level gfx950 subgroup width used to reject unsupported widths.
#[deprecated(note = "gfx950 capability operations have an exact Wave16 partition")]
pub struct Gfx950SubgroupWidth<const WIDTH: u32>;

/// A sealed power-of-two subgroup width in `1..=64`.
#[deprecated(note = "gfx950 capability operations have an exact Wave16 partition")]
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
pub struct Gfx950MfmaFragment<'wave, Format, Role, Brand = UnbrandedCapability> {
    registers: [u32; GFX950_MFMA_OPERAND_DWORDS],
    _contract: Gfx950WaveContract<'wave, (Format, Role), Brand>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'wave, Format, Role: sealed::OperandRole, Brand>
    Gfx950MfmaFragment<'wave, Format, Role, Brand>
{
    fn from_registers(
        _lane: &'wave WaveLane<Wave64, Brand>,
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
pub type Gfx950Fp4MfmaAFragment<'wave, Brand = UnbrandedCapability> =
    Gfx950MfmaFragment<'wave, Gfx950Fp4E2M1, Gfx950MfmaOperandA, Brand>;
/// Canonical FP4 B operand fragment.
pub type Gfx950Fp4MfmaBFragment<'wave, Brand = UnbrandedCapability> =
    Gfx950MfmaFragment<'wave, Gfx950Fp4E2M1, Gfx950MfmaOperandB, Brand>;
/// Canonical FP8 A operand fragment.
pub type Gfx950Fp8MfmaAFragment<'wave, Brand = UnbrandedCapability> =
    Gfx950MfmaFragment<'wave, Gfx950Fp8E4M3, Gfx950MfmaOperandA, Brand>;
/// Canonical FP8 B operand fragment.
pub type Gfx950Fp8MfmaBFragment<'wave, Brand = UnbrandedCapability> =
    Gfx950MfmaFragment<'wave, Gfx950Fp8E4M3, Gfx950MfmaOperandB, Brand>;

/// Four FP32 accumulator values associated with one gfx950 input format.
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_gfx950_f32_accumulator_fragment_v1"]
pub struct Gfx950F32AccumulatorFragment<'wave, Format, Brand = UnbrandedCapability> {
    values: [f32; 4],
    _contract: Gfx950WaveContract<'wave, Format, Brand>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'wave, Format: Gfx950MfmaFormat, Brand> Gfx950F32AccumulatorFragment<'wave, Format, Brand> {
    fn zero_inner(_lane: &'wave WaveLane<Wave64, Brand>) -> Self {
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
    fn from_values(_lane: &'wave WaveLane<Wave64, Brand>, values: [f32; 4]) -> Self {
        Self {
            values,
            _contract: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

impl<'wave, Brand> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, Brand> {
    /// Creates the all-zero FP4 accumulator for one authenticated Wave64 lane.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_fp4_f32_accumulator_zero_v1"]
    pub fn zero(lane: &'wave WaveLane<Wave64, Brand>) -> Self {
        Self::zero_inner(lane)
    }

    /// Returns this lane's four row-major FP4 MFMA results.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_fp4_f32_accumulator_into_values_v1"]
    pub fn into_values(self) -> [f32; 4] {
        self.into_values_inner()
    }
}

impl<'wave, Brand> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp8E4M3, Brand> {
    /// Creates the all-zero FP8 accumulator for one authenticated Wave64 lane.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_f32_accumulator_zero_v1"]
    pub fn zero(lane: &'wave WaveLane<Wave64, Brand>) -> Self {
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

trait ByteMatrix {
    fn value_or_zero(&self, row: Option<usize>, column: Option<usize>) -> u8;
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
}

impl ByteMatrix for CheckedByteMatrix<'_> {
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

/// A checked low-precision matrix view backed by branded Global memory.
///
/// `MatrixBrand` retains the issuing subgroup and synchronization epoch while
/// `GlobalBrand` retains the allocation's kernel identity. The only public
/// constructors are policy-bound and require their sealed brand relation.
#[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_global_matrix_view_v1"]
pub struct GlobalGfx950MfmaMatrix<'view, 'kernel: 'view, Format, Role, MatrixBrand, GlobalBrand> {
    bits: &'view Global<'kernel, u8, ReadOnly, GlobalBrand>,
    offset: usize,
    rows: usize,
    columns: usize,
    stride: usize,
    _format: PhantomData<fn(Format) -> Format>,
    _role: PhantomData<fn(Role) -> Role>,
    _matrix_brand: PhantomData<fn(MatrixBrand) -> MatrixBrand>,
    _not_send_sync: PhantomData<*mut ()>,
}

pub type GlobalGfx950Fp4MfmaAMatrix<'view, 'kernel, MatrixBrand, GlobalBrand> =
    GlobalGfx950MfmaMatrix<
        'view,
        'kernel,
        Gfx950Fp4E2M1,
        Gfx950MfmaOperandA,
        MatrixBrand,
        GlobalBrand,
    >;
pub type GlobalGfx950Fp4MfmaBMatrix<'view, 'kernel, MatrixBrand, GlobalBrand> =
    GlobalGfx950MfmaMatrix<
        'view,
        'kernel,
        Gfx950Fp4E2M1,
        Gfx950MfmaOperandB,
        MatrixBrand,
        GlobalBrand,
    >;
pub type GlobalGfx950Fp8MfmaAMatrix<'view, 'kernel, MatrixBrand, GlobalBrand> =
    GlobalGfx950MfmaMatrix<
        'view,
        'kernel,
        Gfx950Fp8E4M3,
        Gfx950MfmaOperandA,
        MatrixBrand,
        GlobalBrand,
    >;
pub type GlobalGfx950Fp8MfmaBMatrix<'view, 'kernel, MatrixBrand, GlobalBrand> =
    GlobalGfx950MfmaMatrix<
        'view,
        'kernel,
        Gfx950Fp8E4M3,
        Gfx950MfmaOperandB,
        MatrixBrand,
        GlobalBrand,
    >;

impl<'view, 'kernel, Format, Role, MatrixBrand, GlobalBrand>
    GlobalGfx950MfmaMatrix<'view, 'kernel, Format, Role, MatrixBrand, GlobalBrand>
where
    Format: Gfx950MfmaFormat,
    Role: sealed::OperandRole,
{
    fn checked(
        bits: &'view Global<'kernel, u8, ReadOnly, GlobalBrand>,
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
            _format: PhantomData,
            _role: PhantomData,
            _matrix_brand: PhantomData,
            _not_send_sync: PhantomData,
        })
    }
}

impl<Format, Role, MatrixBrand, GlobalBrand> ByteMatrix
    for GlobalGfx950MfmaMatrix<'_, '_, Format, Role, MatrixBrand, GlobalBrand>
where
    Format: Gfx950MfmaFormat,
    Role: sealed::OperandRole,
{
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
            .and_then(|index| self.bits.load(index))
            .unwrap_or(0)
    }
}

/// Checked row-major A matrix carrying its gfx950 low-precision format.
///
/// Each logical value occupies one source byte. FP4 values use the low nibble;
/// fragment loads perform the hardware's dense two-values-per-byte packing.
#[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_matrix_a_view_v1"]
pub struct Gfx950MfmaAMatrix<'data, Format, Brand = UnbrandedCapability> {
    matrix: CheckedByteMatrix<'data>,
    _format: PhantomData<Format>,
    _brand: PhantomData<fn(Brand) -> Brand>,
}

impl<'data, Format: Gfx950MfmaFormat, Brand> Gfx950MfmaAMatrix<'data, Format, Brand> {
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
            _brand: PhantomData,
        })
    }
}

impl<'data> Gfx950MfmaAMatrix<'data, Gfx950Fp4E2M1, UnbrandedCapability> {
    /// Validates row-major FP4 `rows x reduction` byte storage.
    #[inline(never)]
    #[deprecated(note = "use PolicyGfx950Matrix with branded Global storage")]
    pub unsafe fn row_major(
        bits: &'data [u8],
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        Self::row_major_inner(bits, offset, rows, reduction, stride)
    }
}

impl<'data, Brand> Gfx950MfmaAMatrix<'data, Gfx950Fp4E2M1, Brand> {
    /// Loads this lane's FP4 values from one logical M16xK128 A tile.
    #[inline(never)]
    #[deprecated(note = "use the Global-backed gfx950 matrix load")]
    pub fn load_m16k128<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, Brand>,
        row_base: usize,
        reduction_base: usize,
    ) -> Gfx950Fp4MfmaAFragment<'wave, Brand> {
        Gfx950MfmaFragment::from_registers(
            lane,
            pack_fp4_a(&self.matrix, lane.get() as usize, row_base, reduction_base),
        )
    }
}

impl<'data> Gfx950MfmaAMatrix<'data, Gfx950Fp8E4M3, UnbrandedCapability> {
    /// Validates row-major FP8 `rows x reduction` byte storage.
    #[inline(never)]
    #[deprecated(note = "use PolicyGfx950Matrix with branded Global storage")]
    pub unsafe fn row_major(
        bits: &'data [u8],
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        Self::row_major_inner(bits, offset, rows, reduction, stride)
    }
}

impl<'data, Brand> Gfx950MfmaAMatrix<'data, Gfx950Fp8E4M3, Brand> {
    /// Loads this lane's FP8 values from one logical M16xK128 A tile.
    #[inline(never)]
    #[deprecated(note = "use the Global-backed gfx950 matrix load")]
    pub fn load_m16k128<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, Brand>,
        row_base: usize,
        reduction_base: usize,
    ) -> Gfx950Fp8MfmaAFragment<'wave, Brand> {
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
pub struct Gfx950MfmaBMatrix<'data, Format, Brand = UnbrandedCapability> {
    matrix: CheckedByteMatrix<'data>,
    _format: PhantomData<Format>,
    _brand: PhantomData<fn(Brand) -> Brand>,
}

impl<'data, Format: Gfx950MfmaFormat, Brand> Gfx950MfmaBMatrix<'data, Format, Brand> {
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
            _brand: PhantomData,
        })
    }
}

impl<'data> Gfx950MfmaBMatrix<'data, Gfx950Fp4E2M1, UnbrandedCapability> {
    /// Validates row-major FP4 `reduction x columns` byte storage.
    #[inline(never)]
    #[deprecated(note = "use PolicyGfx950Matrix with branded Global storage")]
    pub unsafe fn row_major(
        bits: &'data [u8],
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        Self::row_major_inner(bits, offset, reduction, columns, stride)
    }
}

impl<'data, Brand> Gfx950MfmaBMatrix<'data, Gfx950Fp4E2M1, Brand> {
    /// Loads this lane's FP4 values from one logical K128xN16 B tile.
    #[inline(never)]
    #[deprecated(note = "use the Global-backed gfx950 matrix load")]
    pub fn load_k128n16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, Brand>,
        reduction_base: usize,
        column_base: usize,
    ) -> Gfx950Fp4MfmaBFragment<'wave, Brand> {
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

impl<'data> Gfx950MfmaBMatrix<'data, Gfx950Fp8E4M3, UnbrandedCapability> {
    /// Validates row-major FP8 `reduction x columns` byte storage.
    #[inline(never)]
    #[deprecated(note = "use PolicyGfx950Matrix with branded Global storage")]
    pub unsafe fn row_major(
        bits: &'data [u8],
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Gfx950MatrixViewError> {
        Self::row_major_inner(bits, offset, reduction, columns, stride)
    }
}

impl<'data, Brand> Gfx950MfmaBMatrix<'data, Gfx950Fp8E4M3, Brand> {
    /// Loads this lane's FP8 values from one logical K128xN16 B tile.
    #[inline(never)]
    #[deprecated(note = "use the Global-backed gfx950 matrix load")]
    pub fn load_k128n16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, Brand>,
        reduction_base: usize,
        column_base: usize,
    ) -> Gfx950Fp8MfmaBFragment<'wave, Brand> {
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
pub type Gfx950Fp4MfmaAMatrix<'data, Brand = UnbrandedCapability> =
    Gfx950MfmaAMatrix<'data, Gfx950Fp4E2M1, Brand>;
/// Checked row-major FP4 B matrix.
pub type Gfx950Fp4MfmaBMatrix<'data, Brand = UnbrandedCapability> =
    Gfx950MfmaBMatrix<'data, Gfx950Fp4E2M1, Brand>;
/// Checked row-major FP8 A matrix.
pub type Gfx950Fp8MfmaAMatrix<'data, Brand = UnbrandedCapability> =
    Gfx950MfmaAMatrix<'data, Gfx950Fp8E4M3, Brand>;
/// Checked row-major FP8 B matrix.
pub type Gfx950Fp8MfmaBMatrix<'data, Brand = UnbrandedCapability> =
    Gfx950MfmaBMatrix<'data, Gfx950Fp8E4M3, Brand>;

impl<MatrixBrand, GlobalBrand>
    GlobalGfx950MfmaMatrix<'_, '_, Gfx950Fp4E2M1, Gfx950MfmaOperandA, MatrixBrand, GlobalBrand>
{
    /// Loads one lane's FP4 A fragment from branded Global storage.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_global_matrix_a_fp4_load_m16k128_v1"]
    pub fn load_m16k128<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, MatrixBrand>,
        row_base: usize,
        reduction_base: usize,
    ) -> Gfx950Fp4MfmaAFragment<'wave, MatrixBrand> {
        Gfx950MfmaFragment::from_registers(
            lane,
            pack_fp4_a(self, lane.get() as usize, row_base, reduction_base),
        )
    }
}

impl<MatrixBrand, GlobalBrand>
    GlobalGfx950MfmaMatrix<'_, '_, Gfx950Fp4E2M1, Gfx950MfmaOperandB, MatrixBrand, GlobalBrand>
{
    /// Loads one lane's FP4 B fragment from branded Global storage.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_global_matrix_b_fp4_load_k128n16_v1"]
    pub fn load_k128n16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, MatrixBrand>,
        reduction_base: usize,
        column_base: usize,
    ) -> Gfx950Fp4MfmaBFragment<'wave, MatrixBrand> {
        Gfx950MfmaFragment::from_registers(
            lane,
            pack_fp4_b(self, lane.get() as usize, reduction_base, column_base),
        )
    }
}

impl<MatrixBrand, GlobalBrand>
    GlobalGfx950MfmaMatrix<'_, '_, Gfx950Fp8E4M3, Gfx950MfmaOperandA, MatrixBrand, GlobalBrand>
{
    /// Loads one lane's FP8 A fragment from branded Global storage.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_global_matrix_a_fp8_load_m16k128_v1"]
    pub fn load_m16k128<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, MatrixBrand>,
        row_base: usize,
        reduction_base: usize,
    ) -> Gfx950Fp8MfmaAFragment<'wave, MatrixBrand> {
        Gfx950MfmaFragment::from_registers(
            lane,
            pack_fp8_a(self, lane.get() as usize, row_base, reduction_base),
        )
    }
}

impl<MatrixBrand, GlobalBrand>
    GlobalGfx950MfmaMatrix<'_, '_, Gfx950Fp8E4M3, Gfx950MfmaOperandB, MatrixBrand, GlobalBrand>
{
    /// Loads one lane's FP8 B fragment from branded Global storage.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_global_matrix_b_fp8_load_k128n16_v1"]
    pub fn load_k128n16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, MatrixBrand>,
        reduction_base: usize,
        column_base: usize,
    ) -> Gfx950Fp8MfmaBFragment<'wave, MatrixBrand> {
        Gfx950MfmaFragment::from_registers(
            lane,
            pack_fp8_b(self, lane.get() as usize, reduction_base, column_base),
        )
    }
}

/// Compiler-created authority for the exact gfx950 scaled-MFMA profile.
#[rustc_diagnostic_item = "fe2o3_device_gfx950_matrix_context_v1"]
#[deprecated(note = "use PolicyGfx950Matrix for admitted gfx950 operations")]
pub struct Gfx950Matrix<Brand = UnbrandedCapability> {
    _private: (),
    _brand: PhantomData<fn(Brand) -> Brand>,
    _not_send_sync: PhantomData<*mut ()>,
}

/// Policy-bound authority for the exact gfx950 low-precision MFMA profile.
///
/// The wrapper retains the matrix/subgroup brand, root Global brand, and the
/// compiler-issued numerical policy. Only operations on this type carry the
/// diagnostic identities accepted by production import.
#[must_use = "gfx950 matrix authority must retain its numerical policy"]
#[rustc_diagnostic_item = "fe2o3_device_policy_gfx950_matrix_capability_v1"]
pub struct PolicyGfx950Matrix<'capability, MatrixBrand, GlobalBrand, Policy>
where
    Policy: NumericalPolicy,
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    policy_matrix:
        &'capability PolicyMatrixCapability<'capability, MatrixBrand, GlobalBrand, Policy>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'capability, MatrixBrand, GlobalBrand, Policy>
    PolicyMatrixCapability<'capability, MatrixBrand, GlobalBrand, Policy>
where
    Policy: NumericalPolicy,
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    /// Narrows policy-bound matrix authority to the gfx950 profile.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_policy_gfx950_matrix_issue_v1"]
    pub fn gfx950(
        &'capability self,
    ) -> PolicyGfx950Matrix<'capability, MatrixBrand, GlobalBrand, Policy> {
        let _ = self.matrix();
        PolicyGfx950Matrix {
            policy_matrix: self,
            _not_send_sync: PhantomData,
        }
    }
}

impl<MatrixBrand, GlobalBrand, Policy> PolicyGfx950Matrix<'_, MatrixBrand, GlobalBrand, Policy>
where
    Policy: NumericalPolicy,
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    /// Validates a Global-backed row-major FP4 A matrix.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_global_matrix_a_fp4_row_major_v1"]
    pub fn fp4_a_global_row_major<'view, 'kernel>(
        &self,
        bits: &'view Global<'kernel, u8, ReadOnly, GlobalBrand>,
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<
        GlobalGfx950Fp4MfmaAMatrix<'view, 'kernel, MatrixBrand, GlobalBrand>,
        Gfx950MatrixViewError,
    >
    where
        'kernel: 'view,
    {
        let _ = self.policy_matrix;
        GlobalGfx950MfmaMatrix::checked(bits, offset, rows, reduction, stride)
    }

    /// Validates a Global-backed row-major FP4 B matrix.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_global_matrix_b_fp4_row_major_v1"]
    pub fn fp4_b_global_row_major<'view, 'kernel>(
        &self,
        bits: &'view Global<'kernel, u8, ReadOnly, GlobalBrand>,
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<
        GlobalGfx950Fp4MfmaBMatrix<'view, 'kernel, MatrixBrand, GlobalBrand>,
        Gfx950MatrixViewError,
    >
    where
        'kernel: 'view,
    {
        let _ = self.policy_matrix;
        GlobalGfx950MfmaMatrix::checked(bits, offset, reduction, columns, stride)
    }

    /// Validates a Global-backed row-major FP8 A matrix.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_global_matrix_a_fp8_row_major_v1"]
    pub fn fp8_a_global_row_major<'view, 'kernel>(
        &self,
        bits: &'view Global<'kernel, u8, ReadOnly, GlobalBrand>,
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<
        GlobalGfx950Fp8MfmaAMatrix<'view, 'kernel, MatrixBrand, GlobalBrand>,
        Gfx950MatrixViewError,
    >
    where
        'kernel: 'view,
    {
        let _ = self.policy_matrix;
        GlobalGfx950MfmaMatrix::checked(bits, offset, rows, reduction, stride)
    }

    /// Validates a Global-backed row-major FP8 B matrix.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_global_matrix_b_fp8_row_major_v1"]
    pub fn fp8_b_global_row_major<'view, 'kernel>(
        &self,
        bits: &'view Global<'kernel, u8, ReadOnly, GlobalBrand>,
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<
        GlobalGfx950Fp8MfmaBMatrix<'view, 'kernel, MatrixBrand, GlobalBrand>,
        Gfx950MatrixViewError,
    >
    where
        'kernel: 'view,
    {
        let _ = self.policy_matrix;
        GlobalGfx950MfmaMatrix::checked(bits, offset, reduction, columns, stride)
    }

    /// Creates an all-zero FP4 accumulator under this exact policy owner.
    pub fn fp4_zero_accumulator<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, MatrixBrand>,
    ) -> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, MatrixBrand> {
        let _ = self.policy_matrix;
        Gfx950F32AccumulatorFragment::zero_inner(lane)
    }

    /// Creates an all-zero FP8 accumulator under this exact policy owner.
    pub fn fp8_zero_accumulator<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, MatrixBrand>,
    ) -> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp8E4M3, MatrixBrand> {
        let _ = self.policy_matrix;
        Gfx950F32AccumulatorFragment::zero_inner(lane)
    }

    /// Performs one policy-bound full-wave FP4 scaled MFMA.
    #[must_use]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_fp4_f32_m16n16k128_v1"]
    pub fn multiply_accumulate_fp4<'wave>(
        &self,
        lhs: Gfx950Fp4MfmaAFragment<'wave, MatrixBrand>,
        rhs: Gfx950Fp4MfmaBFragment<'wave, MatrixBrand>,
        accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, MatrixBrand>,
    ) -> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, MatrixBrand> {
        let _ = (
            self.policy_matrix,
            lhs.into_registers(),
            rhs.into_registers(),
            accumulator.values,
        );
        unreachable!("policy-bound gfx950 FP4 MFMA requires authenticated lowering")
    }

    /// Performs one policy-bound full-wave mixed FP4xFP8 scaled MFMA.
    #[must_use]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_fp4_fp8_f32_m16n16k128_v1"]
    pub fn multiply_accumulate_fp4_fp8<'wave>(
        &self,
        lhs: Gfx950Fp4MfmaAFragment<'wave, MatrixBrand>,
        rhs: Gfx950Fp8MfmaBFragment<'wave, MatrixBrand>,
        accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, MatrixBrand>,
    ) -> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, MatrixBrand> {
        let _ = (
            self.policy_matrix,
            lhs.into_registers(),
            rhs.into_registers(),
            accumulator.values,
        );
        unreachable!("policy-bound mixed gfx950 MFMA requires authenticated lowering")
    }

    /// Performs one policy-bound full-wave FP8 scaled MFMA.
    #[must_use]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_mfma_fp8_f32_m16n16k128_v1"]
    pub fn multiply_accumulate_fp8<'wave>(
        &self,
        lhs: Gfx950Fp8MfmaAFragment<'wave, MatrixBrand>,
        rhs: Gfx950Fp8MfmaBFragment<'wave, MatrixBrand>,
        accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp8E4M3, MatrixBrand>,
    ) -> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp8E4M3, MatrixBrand> {
        let _ = (
            self.policy_matrix,
            lhs.into_registers(),
            rhs.into_registers(),
            accumulator.values,
        );
        unreachable!("policy-bound gfx950 FP8 MFMA requires authenticated lowering")
    }
}

impl Gfx950Matrix<UnbrandedCapability> {
    /// Acquires gfx950 matrix authority from authenticated compiler lowering.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_matrix_context_current_v1"]
    pub fn current() -> Self {
        unreachable!("Gfx950Matrix must be created by authenticated gfx950 device lowering")
    }
}

impl<Brand> MatrixCapability<Brand> {
    /// Narrows the target-neutral matrix root to the exact gfx950 profile.
    ///
    /// Production target legalization must prove gfx950 support before calls on
    /// the returned capability can lower.
    #[deprecated(note = "use PolicyMatrixCapability::gfx950 for admitted operations")]
    pub unsafe fn gfx950(&self) -> Gfx950Matrix<Brand> {
        Gfx950Matrix::from_matrix_capability(self)
    }
}

impl<Brand> Gfx950Matrix<Brand> {
    fn from_matrix_capability(_matrix: &MatrixCapability<Brand>) -> Self {
        Self {
            _private: (),
            _brand: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    /// Validates branded row-major FP4 A storage.
    #[deprecated(note = "use PolicyGfx950Matrix with branded Global storage")]
    pub unsafe fn fp4_a_row_major<'data>(
        &self,
        bits: &'data [u8],
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<Gfx950Fp4MfmaAMatrix<'data, Brand>, Gfx950MatrixViewError> {
        Gfx950MfmaAMatrix::row_major_inner(bits, offset, rows, reduction, stride)
    }

    /// Validates branded row-major FP4 B storage.
    #[deprecated(note = "use PolicyGfx950Matrix with branded Global storage")]
    pub unsafe fn fp4_b_row_major<'data>(
        &self,
        bits: &'data [u8],
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Gfx950Fp4MfmaBMatrix<'data, Brand>, Gfx950MatrixViewError> {
        Gfx950MfmaBMatrix::row_major_inner(bits, offset, reduction, columns, stride)
    }

    /// Validates branded row-major FP8 A storage.
    #[deprecated(note = "use PolicyGfx950Matrix with branded Global storage")]
    pub unsafe fn fp8_a_row_major<'data>(
        &self,
        bits: &'data [u8],
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<Gfx950Fp8MfmaAMatrix<'data, Brand>, Gfx950MatrixViewError> {
        Gfx950MfmaAMatrix::row_major_inner(bits, offset, rows, reduction, stride)
    }

    /// Validates branded row-major FP8 B storage.
    #[deprecated(note = "use PolicyGfx950Matrix with branded Global storage")]
    pub unsafe fn fp8_b_row_major<'data>(
        &self,
        bits: &'data [u8],
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Gfx950Fp8MfmaBMatrix<'data, Brand>, Gfx950MatrixViewError> {
        Gfx950MfmaBMatrix::row_major_inner(bits, offset, reduction, columns, stride)
    }

    /// Performs one full-wave FP4 scaled MFMA with identity scales.
    #[must_use]
    #[inline(never)]
    #[deprecated(note = "use PolicyGfx950Matrix; bare MFMA has no admitted numerical policy")]
    pub unsafe fn multiply_accumulate_fp4<'wave>(
        &self,
        lhs: Gfx950Fp4MfmaAFragment<'wave, Brand>,
        rhs: Gfx950Fp4MfmaBFragment<'wave, Brand>,
        accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, Brand>,
    ) -> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, Brand> {
        let _ = (
            self,
            lhs.into_registers(),
            rhs.into_registers(),
            accumulator.values,
        );
        unreachable!("gfx950 FP4 MFMA requires authenticated compiler lowering")
    }

    /// Performs one full-wave scaled MFMA with FP4 A and FP8 B operands.
    #[must_use]
    #[inline(never)]
    #[deprecated(note = "use PolicyGfx950Matrix; bare MFMA has no admitted numerical policy")]
    pub unsafe fn multiply_accumulate_fp4_fp8<'wave>(
        &self,
        lhs: Gfx950Fp4MfmaAFragment<'wave, Brand>,
        rhs: Gfx950Fp8MfmaBFragment<'wave, Brand>,
        accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, Brand>,
    ) -> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp4E2M1, Brand> {
        let _ = (
            self,
            lhs.into_registers(),
            rhs.into_registers(),
            accumulator.values,
        );
        unreachable!("gfx950 mixed FP4xFP8 MFMA requires authenticated compiler lowering")
    }

    /// Performs one full-wave FP8 scaled MFMA with identity scales.
    #[must_use]
    #[inline(never)]
    #[deprecated(note = "use PolicyGfx950Matrix; bare MFMA has no admitted numerical policy")]
    pub unsafe fn multiply_accumulate_fp8<'wave>(
        &self,
        lhs: Gfx950Fp8MfmaAFragment<'wave, Brand>,
        rhs: Gfx950Fp8MfmaBFragment<'wave, Brand>,
        accumulator: Gfx950F32AccumulatorFragment<'wave, Gfx950Fp8E4M3, Brand>,
    ) -> Gfx950F32AccumulatorFragment<'wave, Gfx950Fp8E4M3, Brand> {
        let _ = (
            self,
            lhs.into_registers(),
            rhs.into_registers(),
            accumulator.values,
        );
        unreachable!("gfx950 FP8 MFMA requires authenticated compiler lowering")
    }
}

/// A gfx950 Wave16 operation view derived from one exact Wave64 subgroup epoch.
///
/// Wave16 is fixed in the type and the private value retains borrows of both
/// the subgroup and its matching workgroup epoch. Safe source cannot acquire
/// this authority independently or use it after the enclosing epoch advances.
#[rustc_diagnostic_item = "fe2o3_device_gfx950_subgroup_context_v1"]
pub struct Gfx950Subgroup<'operation, 'workgroup, KernelBrand, Epoch>
where
    Epoch: SynchronizationEpoch,
{
    _subgroup: Gfx950SubgroupContract<'operation, 'workgroup, KernelBrand, Epoch>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'workgroup, KernelBrand, Epoch> Subgroup<'workgroup, SubgroupWidth64, KernelBrand, Epoch>
where
    Epoch: SynchronizationEpoch,
{
    /// Narrows this exact subgroup epoch to gfx950 Wave16 collectives.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_subgroup_wave16_v1"]
    pub fn gfx950_wave16<'operation>(
        &'operation self,
        _epoch: &'operation WorkgroupEpoch<'workgroup, KernelBrand, Epoch>,
    ) -> Gfx950Subgroup<'operation, 'workgroup, KernelBrand, Epoch> {
        Gfx950Subgroup {
            _subgroup: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

impl<'operation, 'workgroup, KernelBrand, Epoch>
    Gfx950Subgroup<'operation, 'workgroup, KernelBrand, Epoch>
where
    Epoch: SynchronizationEpoch,
{
    /// Returns the ordered maximum to every lane in each contiguous subgroup.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_subgroup_reduce_max_f32_wave16_v1"]
    pub fn reduce_max_f32(&self, value: f32) -> f32 {
        let _ = (self, value);
        unreachable!("gfx950 Wave16 maximum requires authenticated compiler lowering")
    }

    /// Returns the FP32 sum to every lane in each contiguous subgroup.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_subgroup_reduce_sum_f32_wave16_v1"]
    pub fn reduce_sum_f32(&self, value: f32) -> f32 {
        let _ = (self, value);
        unreachable!("gfx950 Wave16 sum requires authenticated compiler lowering")
    }

    /// Broadcasts from `source_lane` within each contiguous Wave16 partition.
    ///
    /// Authenticated lowering rejects `source_lane >= 16`; it must not silently
    /// wrap or clamp the source lane.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_subgroup_broadcast_f32_wave16_v1"]
    pub fn broadcast_f32(&self, value: f32, source_lane: u32) -> f32 {
        let _ = (self, value, source_lane);
        unreachable!("gfx950 Wave16 broadcast requires authenticated compiler lowering")
    }

    /// Issues one transpose tile for this exact Wave64 subgroup and epoch.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_tile_issue_v1"]
    pub fn transpose_tile<Format>(
        &self,
    ) -> Gfx950LdsTransposeTile<
        'workgroup,
        Format,
        Gfx950TransposeUninitialized,
        SubgroupBrand<'workgroup, SubgroupWidth64, KernelBrand, Epoch>,
    >
    where
        Format: Gfx950MfmaFormat,
    {
        let _ = self;
        unreachable!("gfx950 LDS transpose issuance requires authenticated lowering")
    }

    #[cfg(test)]
    fn for_host_test() -> Self {
        Self {
            _subgroup: PhantomData,
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
/// The workgroup lifetime and execution brand are independently invariant. A
/// tile therefore cannot leave its generative workgroup scope or be relabeled,
/// while the kernel brand does not acquire a false `KernelBrand: 'workgroup`
/// requirement merely because it is nested inside the execution brand.
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
///
/// A kernel may run one to four Wave64s in a one-dimensional workgroup. The production gfx950
/// lowerer assigns every wave a disjoint format-sized LDS tile; `publish` remains a workgroup-wide
/// barrier and therefore must be reached uniformly by every wave in the workgroup.
#[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_tile_v1"]
pub struct Gfx950LdsTransposeTile<'wave, Format, State, Brand = UnbrandedCapability>
where
    Format: Gfx950MfmaFormat,
    State: sealed::TransposeState,
{
    _contract: Gfx950WaveContract<'wave, (Format, State), Brand>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'wave, Format: Gfx950MfmaFormat, Brand>
    Gfx950LdsTransposeTile<'wave, Format, Gfx950TransposeUninitialized, Brand>
{
    /// Acquires one compiler-owned transpose tile for the current wave.
    #[inline(never)]
    #[deprecated(note = "derive a transpose tile from Subgroup::gfx950_wave16")]
    pub unsafe fn current(_lane: &'wave WaveLane<Wave64, Brand>) -> Self {
        unreachable!("gfx950 LDS transpose storage requires authenticated compiler lowering")
    }
}

impl<'wave, Brand>
    Gfx950LdsTransposeTile<'wave, Gfx950Fp4E2M1, Gfx950TransposeUninitialized, Brand>
{
    /// Stages one token-major FP4 K tile using the inverse B4 permutation.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_stage_b4_v1"]
    pub fn stage_k_transposed<GlobalBrand>(
        self,
        matrix: &GlobalGfx950Fp4MfmaAMatrix<'_, '_, Brand, GlobalBrand>,
        token_base: usize,
        reduction_base: usize,
    ) -> Gfx950LdsTransposeTile<'wave, Gfx950Fp4E2M1, Gfx950TransposeStaged, Brand>
    where
        Brand: MatrixGlobalAccess<GlobalBrand>,
    {
        let _ = (self, matrix, token_base, reduction_base);
        unreachable!("gfx950 B4 inverse transpose staging requires authenticated lowering")
    }
}

impl<'wave, Brand>
    Gfx950LdsTransposeTile<'wave, Gfx950Fp8E4M3, Gfx950TransposeUninitialized, Brand>
{
    /// Stages one token-major FP8 K tile using the inverse B8 permutation.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_stage_b8_v1"]
    pub fn stage_k_transposed<GlobalBrand>(
        self,
        matrix: &GlobalGfx950Fp8MfmaAMatrix<'_, '_, Brand, GlobalBrand>,
        token_base: usize,
        reduction_base: usize,
    ) -> Gfx950LdsTransposeTile<'wave, Gfx950Fp8E4M3, Gfx950TransposeStaged, Brand>
    where
        Brand: MatrixGlobalAccess<GlobalBrand>,
    {
        let _ = (self, matrix, token_base, reduction_base);
        unreachable!("gfx950 B8 inverse transpose staging requires authenticated lowering")
    }
}

/// Result of publishing one staged gfx950 transpose tile.
pub type Gfx950TransposePublishTransition<'workgroup, Format, KernelBrand, Epoch> = (
    WorkgroupCapability<'workgroup, KernelBrand, NextEpoch<Epoch>>,
    Gfx950LdsTransposeTile<
        'workgroup,
        Format,
        Gfx950TransposePublished,
        SubgroupBrand<'workgroup, SubgroupWidth64, KernelBrand, NextEpoch<Epoch>>,
    >,
);

impl<'workgroup, Format, KernelBrand, Epoch>
    Gfx950LdsTransposeTile<
        'workgroup,
        Format,
        Gfx950TransposeStaged,
        SubgroupBrand<'workgroup, SubgroupWidth64, KernelBrand, Epoch>,
    >
where
    Format: Gfx950MfmaFormat,
    Epoch: SynchronizationEpoch,
{
    /// Publishes all staged values and advances the owning workgroup epoch.
    ///
    /// Both inputs are consumed. The returned tile carries the same next epoch
    /// as the workgroup, so stale subgroup, matrix, and transpose capabilities
    /// cannot be substituted at later operations.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_publish_v1"]
    pub fn publish(
        self,
        workgroup: WorkgroupCapability<'workgroup, KernelBrand, Epoch>,
    ) -> Gfx950TransposePublishTransition<'workgroup, Format, KernelBrand, Epoch> {
        let _ = (self, workgroup);
        unreachable!("gfx950 LDS transpose publish requires authenticated compiler lowering")
    }
}

impl<'workgroup, KernelBrand, Epoch>
    Gfx950LdsTransposeTile<
        'workgroup,
        Gfx950Fp4E2M1,
        Gfx950TransposePublished,
        SubgroupBrand<'workgroup, SubgroupWidth64, KernelBrand, NextEpoch<Epoch>>,
    >
where
    Epoch: SynchronizationEpoch,
{
    /// Reads the published tile with two B4 transpose loads.
    ///
    /// The matching next-epoch lane reborrows the published tile for one
    /// operation. The fragment therefore cannot outlive that lane borrow or
    /// be relabeled with another kernel, workgroup, width, or epoch.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_read_b4_v1"]
    pub fn read_mfma_fragment<'operation>(
        self,
        lane: &'operation WaveLane<
            Wave64,
            SubgroupBrand<'workgroup, SubgroupWidth64, KernelBrand, NextEpoch<Epoch>>,
        >,
    ) -> Gfx950Fp4MfmaBFragment<
        'operation,
        SubgroupBrand<'workgroup, SubgroupWidth64, KernelBrand, NextEpoch<Epoch>>,
    > {
        let _ = (self, lane);
        unreachable!("gfx950 B4 transpose reads require authenticated compiler lowering")
    }
}

impl<'workgroup, KernelBrand, Epoch>
    Gfx950LdsTransposeTile<
        'workgroup,
        Gfx950Fp8E4M3,
        Gfx950TransposePublished,
        SubgroupBrand<'workgroup, SubgroupWidth64, KernelBrand, NextEpoch<Epoch>>,
    >
where
    Epoch: SynchronizationEpoch,
{
    /// Reads the published tile with four B8 transpose loads.
    ///
    /// The matching next-epoch lane reborrows the published tile for one
    /// operation. The fragment therefore cannot outlive that lane borrow or
    /// be relabeled with another kernel, workgroup, width, or epoch.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_gfx950_lds_transpose_read_b8_v1"]
    pub fn read_mfma_fragment<'operation>(
        self,
        lane: &'operation WaveLane<
            Wave64,
            SubgroupBrand<'workgroup, SubgroupWidth64, KernelBrand, NextEpoch<Epoch>>,
        >,
    ) -> Gfx950Fp8MfmaBFragment<
        'operation,
        SubgroupBrand<'workgroup, SubgroupWidth64, KernelBrand, NextEpoch<Epoch>>,
    > {
        let _ = (self, lane);
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
    matrix: &impl ByteMatrix,
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
    matrix: &impl ByteMatrix,
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
    matrix: &impl ByteMatrix,
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
    matrix: &impl ByteMatrix,
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
        enum TestBrand {}

        assert_eq!(size_of::<Gfx950Fp4MfmaAFragment<'_>>(), 32);
        assert_eq!(size_of::<Gfx950Fp8MfmaAFragment<'_>>(), 32);
        assert_eq!(align_of::<Gfx950Fp4MfmaAFragment<'_>>(), 4);
        assert_eq!(
            size_of::<Gfx950F32AccumulatorFragment<'_, Gfx950Fp4E2M1>>(),
            16
        );
        assert_eq!(
            size_of::<Gfx950Fp4MfmaAFragment<'_, TestBrand>>(),
            size_of::<Gfx950Fp4MfmaAFragment<'_>>()
        );
        assert_eq!(
            size_of::<Gfx950F32AccumulatorFragment<'_, Gfx950Fp4E2M1, TestBrand>>(),
            size_of::<Gfx950F32AccumulatorFragment<'_, Gfx950Fp4E2M1>>()
        );
        assert_eq!(
            size_of::<Gfx950Fp4MfmaAMatrix<'_, TestBrand>>(),
            size_of::<Gfx950Fp4MfmaAMatrix<'_>>()
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
            unsafe { Gfx950Fp8MfmaAMatrix::row_major(&[0; 16], 0, 2, 8, 7) },
            Err(Gfx950MatrixViewError::InvalidStride)
        ));
        assert!(matches!(
            unsafe { Gfx950Fp8MfmaAMatrix::row_major(&[0; 16], usize::MAX, 1, 1, 1) },
            Err(Gfx950MatrixViewError::ExtentOverflow)
        ));
        assert!(matches!(
            unsafe { Gfx950Fp4MfmaBMatrix::row_major(&[0; 15], 0, 2, 8, 8) },
            Err(Gfx950MatrixViewError::OutOfBounds {
                required: 16,
                actual: 15
            })
        ));
    }

    #[test]
    fn global_backed_views_preserve_packing_and_zero_fill() {
        let data: [u8; 128] = core::array::from_fn(|index| index as u8);
        let global: Global<'_, u8, ReadOnly> = crate::capability_memory::fields(&data[..]);
        let matrix = GlobalGfx950Fp8MfmaAMatrix::checked(&global, 0, 1, 128, 128).unwrap();
        let lane = WaveLane::<Wave64>::from_model_snapshot(32).unwrap();
        let packed = bytes(matrix.load_m16k128(&lane, 0, 0).into_registers());
        let expected: [u8; 32] = core::array::from_fn(|item| {
            if item < 16 {
                32 + item as u8
            } else {
                96 + (item - 16) as u8
            }
        });
        assert_eq!(packed, expected);

        let short_global: Global<'_, u8, ReadOnly> = crate::capability_memory::fields(&data[..100]);
        assert!(matches!(
            GlobalGfx950Fp8MfmaAMatrix::<UnbrandedCapability, UnbrandedCapability>::checked(
                &short_global,
                0,
                1,
                128,
                128,
            ),
            Err(Gfx950MatrixViewError::OutOfBounds { .. })
        ));
    }

    #[test]
    fn policy_bound_gfx950_mfma_fails_closed_on_host() {
        let data = [1_u8; 128 * 16];
        let a_global: Global<'_, u8, ReadOnly> = crate::capability_memory::fields(&data[..]);
        let b_global: Global<'_, u8, ReadOnly> = crate::capability_memory::fields(&data[..]);
        let matrix = MatrixCapability::for_host_test();
        let policy = crate::NumericalPolicyCapability::<
            UnbrandedCapability,
            crate::StrictIeee,
        >::for_host_test();
        let policy_matrix = matrix.with_numerical_policy(&policy);
        let gfx950 = policy_matrix.gfx950();
        let lane = WaveLane::<Wave64>::from_model_snapshot(0).unwrap();
        let a = gfx950
            .fp4_a_global_row_major(&a_global, 0, 16, 128, 128)
            .unwrap();
        let b = gfx950
            .fp4_b_global_row_major(&b_global, 0, 128, 16, 16)
            .unwrap();
        let lhs = a.load_m16k128(&lane, 0, 0);
        let rhs = b.load_k128n16(&lane, 0, 0);
        let accumulator = gfx950.fp4_zero_accumulator(&lane);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                gfx950.multiply_accumulate_fp4(lhs, rhs, accumulator)
            }))
            .is_err()
        );
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
        assert_eq!(GFX950_WAVE16_WIDTH, 16);
        let subgroup = Gfx950Subgroup::<'static, 'static, (), crate::InitialEpoch>::for_host_test();
        assert!(catch_unwind(AssertUnwindSafe(|| subgroup.reduce_max_f32(1.0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| subgroup.reduce_sum_f32(1.0))).is_err());
        assert!(catch_unwind(AssertUnwindSafe(|| subgroup.broadcast_f32(1.0, 3))).is_err());
    }
}
