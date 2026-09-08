//! Bounded target-neutral matrix fragments and LDS tile interop.
//!
//! V1 admits one exact profile: a full wave64 cooperates on a 16x16x16 BF16
//! multiply with four FP32 accumulator registers per lane. The managed backend
//! replaces the fail-closed matrix stubs only after binding this reviewed crate
//! compilation, its rustc-observed source ABI, and exact `gfx942:xnack-` policy.
//! This is a build-observation boundary, not cryptographic package-source
//! authentication. LDS method lowering and a complete tiled GEMM remain later
//! frontend increments.

use core::marker::PhantomData;

use crate::{
    Bf16, DynamicLds, ExclusiveReadWrite, Global, LdsElement, LdsInitialized, LdsUninitialized,
    MatrixCapability, MatrixGlobalAccess, NumericalPolicy, PolicyMatrixCapability, ReadOnly,
    Wave64, WaveLane, WaveWidth,
    context::UnbrandedCapability,
    views::{CheckedStridedExtentError, check_strided_2d_extent},
};

pub const MATRIX_CONTRACT_VERSION_V1: u16 = 1;
pub const BF16_F32_MFMA_M: usize = 16;
pub const BF16_F32_MFMA_N: usize = 16;
pub const BF16_F32_MFMA_REDUCTION: usize = 16;
pub const BF16_F32_MFMA_WAVE_LANES: usize = 64;

const TILE_ELEMENTS: usize = 16 * 16;

mod sealed {
    pub trait OperandRole {}
}

/// The sole matrix-instruction profile admitted by V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_mfma_bf16_f32_m16n16k16_profile_v1"]
pub enum Bf16F32M16N16K16 {}

impl Bf16F32M16N16K16 {
    pub const M: usize = BF16_F32_MFMA_M;
    pub const N: usize = BF16_F32_MFMA_N;
    pub const K: usize = BF16_F32_MFMA_REDUCTION;
    pub const WAVE_LANES: usize = BF16_F32_MFMA_WAVE_LANES;
}

/// Left-hand, row-by-reduction operand role for MFMA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_mfma_operand_a_role_v1"]
pub enum MfmaOperandA {}

impl sealed::OperandRole for MfmaOperandA {}

/// Right-hand, reduction-by-column operand role for MFMA.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_mfma_operand_b_role_v1"]
pub enum MfmaOperandB {}

impl sealed::OperandRole for MfmaOperandB {}

/// Canonical four-register lane distribution for the supported 16x16 MFMA tile.
///
/// Global row-major loads and de-swizzled XOR4 LDS reads both produce this
/// register distribution. Storage layout is tracked separately.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_mfma_tile16x16_register_distribution_v1"]
pub enum MfmaRegisterTile16x16 {}

/// XOR4 physical storage layout used by the exact MFMA LDS tile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_mfma_lds_xor4_storage_layout_v1"]
pub enum MfmaLdsXor4 {}

/// Output distribution containing four row-major accumulator elements per lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_mfma_accumulator_row_major_distribution_v1"]
pub enum MfmaAccumulatorRowMajor {}

type Bf16MfmaFragmentContractV1<'wave, Role, Profile, Distribution, Width, Brand> =
    fn(
        &'wave WaveLane<Width, Brand>,
        Role,
        Profile,
        Distribution,
        Brand,
    ) -> &'wave WaveLane<Width, Brand>;

type F32AccumulatorFragmentContractV1<'wave, Profile, Distribution, Width, Brand> =
    fn(
        &'wave WaveLane<Width, Brand>,
        Profile,
        Distribution,
        Brand,
    ) -> &'wave WaveLane<Width, Brand>;

/// A role-, profile-, distribution-, and wave-associated four-value BF16 fragment.
///
/// The payload and constructors are private. Safe code obtains a fragment only
/// from a checked matrix view or a matching published LDS tile. The invariant
/// lifetime retains the borrow of the authenticated wave witness through every
/// fragment use; production admission separately enforces unique acquisition of
/// that witness for the kernel invocation.
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_fragment_v1"]
pub struct Bf16MfmaFragment<
    'wave,
    Role,
    Profile,
    Distribution,
    Width: WaveWidth,
    Brand = UnbrandedCapability,
> {
    values: [Bf16; 4],
    _contract:
        PhantomData<Bf16MfmaFragmentContractV1<'wave, Role, Profile, Distribution, Width, Brand>>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'wave, Role, Profile, Distribution, Width: WaveWidth, Brand>
    Bf16MfmaFragment<'wave, Role, Profile, Distribution, Width, Brand>
{
    fn from_values(_lane: &'wave WaveLane<Width, Brand>, values: [Bf16; 4]) -> Self {
        Self {
            values,
            _contract: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    fn into_array(self) -> [Bf16; 4] {
        self.values
    }

    fn rebrand<OtherBrand>(
        self,
    ) -> Bf16MfmaFragment<'wave, Role, Profile, Distribution, Width, OtherBrand> {
        Bf16MfmaFragment {
            values: self.values,
            _contract: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

/// Canonically distributed A operand for the supported BF16 MFMA profile.
pub type Bf16MfmaAFragment<'wave, Brand = UnbrandedCapability> =
    Bf16MfmaFragment<'wave, MfmaOperandA, Bf16F32M16N16K16, MfmaRegisterTile16x16, Wave64, Brand>;

/// Canonically distributed B operand for the supported BF16 MFMA profile.
pub type Bf16MfmaBFragment<'wave, Brand = UnbrandedCapability> =
    Bf16MfmaFragment<'wave, MfmaOperandB, Bf16F32M16N16K16, MfmaRegisterTile16x16, Wave64, Brand>;

/// Four typed FP32 accumulator values owned by one wave lane.
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_v1"]
pub struct F32AccumulatorFragment<
    'wave,
    Profile = Bf16F32M16N16K16,
    Distribution = MfmaAccumulatorRowMajor,
    Width: WaveWidth = Wave64,
    Brand = UnbrandedCapability,
> {
    values: [f32; 4],
    _contract:
        PhantomData<F32AccumulatorFragmentContractV1<'wave, Profile, Distribution, Width, Brand>>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'wave> F32AccumulatorFragment<'wave> {
    /// Creates the zero accumulator associated with one authenticated wave lane.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_zero_v1"]
    pub fn zero(lane: &'wave WaveLane<Wave64>) -> Self {
        Self::from_values_for_wave(lane, [0.0; 4])
    }
}

impl<'wave, Profile, Distribution, Width: WaveWidth, Brand>
    F32AccumulatorFragment<'wave, Profile, Distribution, Width, Brand>
{
    fn from_values_for_wave(_lane: &'wave WaveLane<Width, Brand>, values: [f32; 4]) -> Self {
        Self {
            values,
            _contract: PhantomData,
            _not_send_sync: PhantomData,
        }
    }

    /// Returns the four FP32 results uniquely owned by this lane.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_into_values_v1"]
    pub fn into_values(self) -> [f32; 4] {
        self.values
    }

    fn rebrand<OtherBrand>(
        self,
    ) -> F32AccumulatorFragment<'wave, Profile, Distribution, Width, OtherBrand> {
        F32AccumulatorFragment {
            values: self.values,
            _contract: PhantomData,
            _not_send_sync: PhantomData,
        }
    }
}

/// Rejection while establishing one checked row-major BF16 matrix view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_matrix_view_error_v1"]
pub enum Bf16MatrixViewError {
    /// A nonempty matrix has a stride smaller than its logical column count.
    InvalidStride,
    /// Offset, extent, or address arithmetic overflowed `usize`.
    ExtentOverflow,
    /// The logical matrix is not fully contained in the supplied allocation.
    OutOfBounds { required: usize, actual: usize },
}

/// Checked row-major BF16 matrix storage carrying its MFMA operand role.
#[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_matrix_view_v1"]
pub struct Bf16MfmaMatrix<'data, Role, Brand = UnbrandedCapability> {
    bits: &'data [u16],
    offset: usize,
    rows: usize,
    columns: usize,
    stride: usize,
    _role: PhantomData<fn() -> Role>,
    _brand: PhantomData<fn(Brand) -> Brand>,
}

/// Checked row-major storage for an MFMA A operand.
pub type Bf16MfmaAMatrix<'data, Brand = UnbrandedCapability> =
    Bf16MfmaMatrix<'data, MfmaOperandA, Brand>;

/// Checked row-major storage for an MFMA B operand.
pub type Bf16MfmaBMatrix<'data, Brand = UnbrandedCapability> =
    Bf16MfmaMatrix<'data, MfmaOperandB, Brand>;

/// A checked Global-backed BF16 matrix retaining root and matrix brands.
///
/// `GlobalBrand` preserves allocation/access identity while `MatrixBrand`
/// preserves subgroup width, workgroup, and epoch identity. Construction is
/// available only through a policy-bound matrix capability whose sealed brand
/// relation connects those identities.
#[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_global_matrix_view_v1"]
pub struct GlobalBf16MfmaMatrix<'view, 'kernel: 'view, Role, MatrixBrand, GlobalBrand> {
    bits: &'view Global<'kernel, u16, ReadOnly, GlobalBrand>,
    offset: usize,
    rows: usize,
    columns: usize,
    stride: usize,
    _role: PhantomData<fn(Role) -> Role>,
    _matrix_brand: PhantomData<fn(MatrixBrand) -> MatrixBrand>,
    _not_send_sync: PhantomData<*mut ()>,
}

/// Global-backed row-major MFMA A operand.
pub type GlobalBf16MfmaAMatrix<'view, 'kernel, MatrixBrand, GlobalBrand> =
    GlobalBf16MfmaMatrix<'view, 'kernel, MfmaOperandA, MatrixBrand, GlobalBrand>;

/// Global-backed row-major MFMA B operand.
pub type GlobalBf16MfmaBMatrix<'view, 'kernel, MatrixBrand, GlobalBrand> =
    GlobalBf16MfmaMatrix<'view, 'kernel, MfmaOperandB, MatrixBrand, GlobalBrand>;

/// Rejection while establishing one checked row-major FP32 output view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_global_matrix_view_error_v1"]
pub enum F32AccumulatorMatrixViewError {
    InvalidStride,
    ExtentOverflow,
    OutOfBounds { required: usize, actual: usize },
}

/// Exclusive Global-backed FP32 accumulator/output matrix.
///
/// The mutable borrow preserves the complete allocation and alias identity.
/// Lane operations expose values only through the matrix capability's exact
/// accumulator distribution and perform checked edge/tail access.
#[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_global_matrix_view_v1"]
pub struct GlobalF32AccumulatorMatrix<'view, 'kernel: 'view, MatrixBrand, GlobalBrand> {
    output: &'view mut Global<'kernel, f32, ExclusiveReadWrite, GlobalBrand>,
    offset: usize,
    rows: usize,
    columns: usize,
    stride: usize,
    _matrix_brand: PhantomData<fn(MatrixBrand) -> MatrixBrand>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl<'data, Role: sealed::OperandRole, Brand> Bf16MfmaMatrix<'data, Role, Brand> {
    fn checked(
        bits: &'data [u16],
        offset: usize,
        rows: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Bf16MatrixViewError> {
        check_strided_2d_extent(offset, rows, columns, stride, bits.len()).map_err(|error| {
            match error {
                CheckedStridedExtentError::InvalidStride => Bf16MatrixViewError::InvalidStride,
                CheckedStridedExtentError::ExtentOverflow => Bf16MatrixViewError::ExtentOverflow,
                CheckedStridedExtentError::OutOfBounds { required, actual } => {
                    Bf16MatrixViewError::OutOfBounds { required, actual }
                }
            }
        })?;
        Ok(Self {
            bits,
            offset,
            rows,
            columns,
            stride,
            _role: PhantomData,
            _brand: PhantomData,
        })
    }

    fn value_or_zero(&self, row: Option<usize>, column: Option<usize>) -> Bf16 {
        let (Some(row), Some(column)) = (row, column) else {
            return Bf16::ZERO;
        };
        if row >= self.rows || column >= self.columns {
            return Bf16::ZERO;
        }
        row.checked_mul(self.stride)
            .and_then(|row_offset| self.offset.checked_add(row_offset))
            .and_then(|index| index.checked_add(column))
            .and_then(|index| self.bits.get(index))
            .copied()
            .map(Bf16::from_bits)
            .unwrap_or(Bf16::ZERO)
    }
}

impl<'data> Bf16MfmaMatrix<'data, MfmaOperandA, UnbrandedCapability> {
    /// Validates a row-major A matrix at `offset` in `bits`.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_matrix_a_row_major_v1"]
    pub fn row_major(
        bits: &'data [u16],
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<Self, Bf16MatrixViewError> {
        Self::checked(bits, offset, rows, reduction, stride)
    }
}

impl<'data, Brand> Bf16MfmaMatrix<'data, MfmaOperandA, Brand> {
    /// Loads this lane's four values from one logical M16xK16 A tile.
    /// Logical out-of-bounds and overflowed coordinates are zero-filled.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_matrix_a_load_zero_filled_v2"]
    pub fn load_m16k16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, Brand>,
        row_base: usize,
        reduction_base: usize,
    ) -> Bf16MfmaAFragment<'wave, Brand> {
        let lane_index = lane.get() as usize;
        let row = row_base.checked_add(lane_index & 15);
        let first_reduction = reduction_base.checked_add((lane_index >> 4) * 4);
        let mut values = [Bf16::ZERO; 4];
        let mut component = 0;
        while component < 4 {
            values[component] = self.value_or_zero(
                row,
                first_reduction.and_then(|base| base.checked_add(component)),
            );
            component += 1;
        }
        Bf16MfmaFragment::from_values(lane, values)
    }
}

impl<'data> Bf16MfmaMatrix<'data, MfmaOperandB, UnbrandedCapability> {
    /// Validates a row-major B matrix at `offset` in `bits`.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_matrix_b_row_major_v1"]
    pub fn row_major(
        bits: &'data [u16],
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Bf16MatrixViewError> {
        Self::checked(bits, offset, reduction, columns, stride)
    }
}

impl<'data, Brand> Bf16MfmaMatrix<'data, MfmaOperandB, Brand> {
    /// Loads this lane's four values from one logical K16xN16 B tile.
    /// Logical out-of-bounds and overflowed coordinates are zero-filled.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_matrix_b_load_zero_filled_v2"]
    pub fn load_k16n16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, Brand>,
        reduction_base: usize,
        column_base: usize,
    ) -> Bf16MfmaBFragment<'wave, Brand> {
        let lane_index = lane.get() as usize;
        let column = column_base.checked_add(lane_index & 15);
        let first_reduction = reduction_base.checked_add((lane_index >> 4) * 4);
        let mut values = [Bf16::ZERO; 4];
        let mut component = 0;
        while component < 4 {
            values[component] = self.value_or_zero(
                first_reduction.and_then(|base| base.checked_add(component)),
                column,
            );
            component += 1;
        }
        Bf16MfmaFragment::from_values(lane, values)
    }
}

impl<'view, 'kernel, Role, MatrixBrand, GlobalBrand>
    GlobalBf16MfmaMatrix<'view, 'kernel, Role, MatrixBrand, GlobalBrand>
where
    Role: sealed::OperandRole,
{
    fn checked(
        bits: &'view Global<'kernel, u16, ReadOnly, GlobalBrand>,
        offset: usize,
        rows: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Bf16MatrixViewError> {
        check_strided_2d_extent(offset, rows, columns, stride, bits.len()).map_err(|error| {
            match error {
                CheckedStridedExtentError::InvalidStride => Bf16MatrixViewError::InvalidStride,
                CheckedStridedExtentError::ExtentOverflow => Bf16MatrixViewError::ExtentOverflow,
                CheckedStridedExtentError::OutOfBounds { required, actual } => {
                    Bf16MatrixViewError::OutOfBounds { required, actual }
                }
            }
        })?;
        Ok(Self {
            bits,
            offset,
            rows,
            columns,
            stride,
            _role: PhantomData,
            _matrix_brand: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    fn value_or_zero(&self, row: Option<usize>, column: Option<usize>) -> Bf16 {
        let (Some(row), Some(column)) = (row, column) else {
            return Bf16::ZERO;
        };
        if row >= self.rows || column >= self.columns {
            return Bf16::ZERO;
        }
        row.checked_mul(self.stride)
            .and_then(|row_offset| self.offset.checked_add(row_offset))
            .and_then(|index| index.checked_add(column))
            .and_then(|index| self.bits.load(index))
            .map(Bf16::from_bits)
            .unwrap_or(Bf16::ZERO)
    }
}

impl<'view, 'kernel, MatrixBrand, GlobalBrand>
    GlobalBf16MfmaMatrix<'view, 'kernel, MfmaOperandA, MatrixBrand, GlobalBrand>
{
    /// Loads this lane's values from one checked M16xK16 Global tile.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_global_matrix_a_load_zero_filled_v1"]
    pub fn load_m16k16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, MatrixBrand>,
        row_base: usize,
        reduction_base: usize,
    ) -> Bf16MfmaAFragment<'wave, MatrixBrand> {
        let lane_index = lane.get() as usize;
        let row = row_base.checked_add(lane_index & 15);
        let first_reduction = reduction_base.checked_add((lane_index >> 4) * 4);
        let mut values = [Bf16::ZERO; 4];
        let mut component = 0;
        while component < 4 {
            values[component] = self.value_or_zero(
                row,
                first_reduction.and_then(|base| base.checked_add(component)),
            );
            component += 1;
        }
        Bf16MfmaFragment::from_values(lane, values)
    }
}

impl<'view, 'kernel, MatrixBrand, GlobalBrand>
    GlobalBf16MfmaMatrix<'view, 'kernel, MfmaOperandB, MatrixBrand, GlobalBrand>
{
    /// Loads this lane's values from one checked K16xN16 Global tile.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_global_matrix_b_load_zero_filled_v1"]
    pub fn load_k16n16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, MatrixBrand>,
        reduction_base: usize,
        column_base: usize,
    ) -> Bf16MfmaBFragment<'wave, MatrixBrand> {
        let lane_index = lane.get() as usize;
        let column = column_base.checked_add(lane_index & 15);
        let first_reduction = reduction_base.checked_add((lane_index >> 4) * 4);
        let mut values = [Bf16::ZERO; 4];
        let mut component = 0;
        while component < 4 {
            values[component] = self.value_or_zero(
                first_reduction.and_then(|base| base.checked_add(component)),
                column,
            );
            component += 1;
        }
        Bf16MfmaFragment::from_values(lane, values)
    }
}

impl<'view, 'kernel, MatrixBrand, GlobalBrand>
    GlobalF32AccumulatorMatrix<'view, 'kernel, MatrixBrand, GlobalBrand>
{
    fn checked(
        output: &'view mut Global<'kernel, f32, ExclusiveReadWrite, GlobalBrand>,
        offset: usize,
        rows: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, F32AccumulatorMatrixViewError> {
        check_strided_2d_extent(offset, rows, columns, stride, output.len()).map_err(|error| {
            match error {
                CheckedStridedExtentError::InvalidStride => {
                    F32AccumulatorMatrixViewError::InvalidStride
                }
                CheckedStridedExtentError::ExtentOverflow => {
                    F32AccumulatorMatrixViewError::ExtentOverflow
                }
                CheckedStridedExtentError::OutOfBounds { required, actual } => {
                    F32AccumulatorMatrixViewError::OutOfBounds { required, actual }
                }
            }
        })?;
        Ok(Self {
            output,
            offset,
            rows,
            columns,
            stride,
            _matrix_brand: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    fn lane_index(
        &self,
        lane: &WaveLane<Wave64, MatrixBrand>,
        row_base: usize,
        column_base: usize,
        component: usize,
    ) -> Option<usize> {
        let lane_index = lane.get() as usize;
        let row = row_base.checked_add(lane_index & 15)?;
        let column = column_base
            .checked_add((lane_index >> 4) * 4)?
            .checked_add(component)?;
        if row >= self.rows || column >= self.columns {
            return None;
        }
        self.offset
            .checked_add(row.checked_mul(self.stride)?)?
            .checked_add(column)
    }

    /// Loads this lane's checked row-major C values, zero-filling logical tails.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_global_matrix_load_lane_v1"]
    pub fn load_lane_values(
        &self,
        lane: &WaveLane<Wave64, MatrixBrand>,
        row_base: usize,
        column_base: usize,
    ) -> [f32; 4] {
        let mut values = [0.0; 4];
        let mut component = 0;
        while component < values.len() {
            values[component] = self
                .lane_index(lane, row_base, column_base, component)
                .and_then(|index| self.output.load(index))
                .unwrap_or(0.0);
            component += 1;
        }
        values
    }

    /// Loads this lane's checked C values as a branded accumulator fragment.
    pub fn load_accumulator<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, MatrixBrand>,
        row_base: usize,
        column_base: usize,
    ) -> F32AccumulatorFragment<'wave, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave64, MatrixBrand>
    {
        F32AccumulatorFragment::from_values_for_wave(
            lane,
            self.load_lane_values(lane, row_base, column_base),
        )
    }

    /// Stores this lane's values to active row-major C coordinates only.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_global_matrix_store_lane_v1"]
    pub fn store_lane_values(
        &mut self,
        lane: &WaveLane<Wave64, MatrixBrand>,
        row_base: usize,
        column_base: usize,
        values: [f32; 4],
    ) -> usize {
        let mut stored = 0;
        for (component, value) in values.into_iter().enumerate() {
            if let Some(index) = self.lane_index(lane, row_base, column_base, component) {
                stored += usize::from(self.output.store(index, value));
            }
        }
        stored
    }

    /// Stores an exact branded accumulator to active C coordinates only.
    pub fn store_accumulator(
        &mut self,
        lane: &WaveLane<Wave64, MatrixBrand>,
        row_base: usize,
        column_base: usize,
        accumulator: F32AccumulatorFragment<
            '_,
            Bf16F32M16N16K16,
            MfmaAccumulatorRowMajor,
            Wave64,
            MatrixBrand,
        >,
    ) -> usize {
        self.store_lane_values(lane, row_base, column_base, accumulator.into_values())
    }
}

/// Compiler-created authority for target-specific matrix instructions.
#[rustc_diagnostic_item = "fe2o3_device_matrix_context_v1"]
pub struct DeviceMatrix {
    _private: (),
    _not_send_sync: PhantomData<*mut ()>,
}

impl DeviceMatrix {
    /// Returns compiler-authenticated authority for matrix operations.
    ///
    /// The compiler proves wave64 mode, gfx942 MFMA support, convergence, and
    /// the V1 floating-point policy. Unsupported lowering and host execution trap.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_matrix_context_current_v1"]
    pub fn current() -> Self {
        unreachable!("DeviceMatrix must be created by provider-bound device lowering")
    }

    /// Performs one full-wave BF16 multiply-accumulate.
    ///
    /// Every active lane must call uniformly with V1-distributed fragments.
    /// Gfx942 maps this to `llvm.amdgcn.mfma.f32.16x16x16bf16.1k` with zero
    /// control immediates. The bounded rustc frontend recognizes this call only
    /// for the reviewed provider and exact observed source ABI; every other path
    /// retains the panic stub.
    ///
    /// The compiler rejects calls unless all 64 lanes of one wave64 execute in
    /// converged control flow with fragments from the same matrix operation.
    #[must_use]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_matrix_mfma_bf16_f32_m16n16k16_v1"]
    pub fn multiply_accumulate<'wave>(
        &self,
        lhs: Bf16MfmaAFragment<'wave>,
        rhs: Bf16MfmaBFragment<'wave>,
        accumulator: F32AccumulatorFragment<'wave>,
    ) -> F32AccumulatorFragment<'wave> {
        let _ = (self, lhs, rhs, accumulator);
        unreachable!("matrix operation requires provider-bound gfx942 wave64 lowering")
    }

    #[cfg(test)]
    pub(crate) fn for_host_test() -> Self {
        Self {
            _private: (),
            _not_send_sync: PhantomData,
        }
    }
}

impl<Brand> MatrixCapability<Brand> {
    /// Validates a branded row-major A matrix at `offset` in `bits`.
    pub fn bf16_a_row_major<'data>(
        &self,
        bits: &'data [u16],
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<Bf16MfmaAMatrix<'data, Brand>, Bf16MatrixViewError> {
        let _ = self;
        Bf16MfmaMatrix::checked(bits, offset, rows, reduction, stride)
    }

    /// Validates a branded row-major B matrix at `offset` in `bits`.
    pub fn bf16_b_row_major<'data>(
        &self,
        bits: &'data [u16],
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Bf16MfmaBMatrix<'data, Brand>, Bf16MatrixViewError> {
        let _ = self;
        Bf16MfmaMatrix::checked(bits, offset, reduction, columns, stride)
    }

    /// Creates a branded zero accumulator for one matching wave lane.
    pub fn bf16_zero_accumulator<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, Brand>,
    ) -> F32AccumulatorFragment<'wave, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave64, Brand>
    {
        let _ = self;
        F32AccumulatorFragment::from_values_for_wave(lane, [0.0; 4])
    }

    /// Performs one full-wave BF16 multiply-accumulate for this exact brand.
    #[must_use]
    pub fn multiply_accumulate<'wave>(
        &self,
        lhs: Bf16MfmaAFragment<'wave, Brand>,
        rhs: Bf16MfmaBFragment<'wave, Brand>,
        accumulator: F32AccumulatorFragment<
            'wave,
            Bf16F32M16N16K16,
            MfmaAccumulatorRowMajor,
            Wave64,
            Brand,
        >,
    ) -> F32AccumulatorFragment<'wave, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave64, Brand>
    {
        DeviceMatrix::multiply_accumulate(
            self,
            lhs.rebrand::<UnbrandedCapability>(),
            rhs.rebrand::<UnbrandedCapability>(),
            accumulator.rebrand::<UnbrandedCapability>(),
        )
        .rebrand::<Brand>()
    }
}

impl<'capability, MatrixBrand, GlobalBrand, Policy>
    PolicyMatrixCapability<'capability, MatrixBrand, GlobalBrand, Policy>
where
    Policy: NumericalPolicy,
    MatrixBrand: MatrixGlobalAccess<GlobalBrand>,
{
    /// Validates a Global-backed row-major BF16 A matrix.
    pub fn bf16_a_global_row_major<'view, 'kernel>(
        &self,
        bits: &'view Global<'kernel, u16, ReadOnly, GlobalBrand>,
        offset: usize,
        rows: usize,
        reduction: usize,
        stride: usize,
    ) -> Result<GlobalBf16MfmaAMatrix<'view, 'kernel, MatrixBrand, GlobalBrand>, Bf16MatrixViewError>
    where
        'kernel: 'view,
    {
        GlobalBf16MfmaMatrix::checked(bits, offset, rows, reduction, stride)
    }

    /// Validates a Global-backed row-major BF16 B matrix.
    pub fn bf16_b_global_row_major<'view, 'kernel>(
        &self,
        bits: &'view Global<'kernel, u16, ReadOnly, GlobalBrand>,
        offset: usize,
        reduction: usize,
        columns: usize,
        stride: usize,
    ) -> Result<GlobalBf16MfmaBMatrix<'view, 'kernel, MatrixBrand, GlobalBrand>, Bf16MatrixViewError>
    where
        'kernel: 'view,
    {
        GlobalBf16MfmaMatrix::checked(bits, offset, reduction, columns, stride)
    }

    /// Validates an exclusive Global-backed row-major FP32 C matrix.
    pub fn f32_accumulator_global_row_major<'view, 'kernel>(
        &self,
        output: &'view mut Global<'kernel, f32, ExclusiveReadWrite, GlobalBrand>,
        offset: usize,
        rows: usize,
        columns: usize,
        stride: usize,
    ) -> Result<
        GlobalF32AccumulatorMatrix<'view, 'kernel, MatrixBrand, GlobalBrand>,
        F32AccumulatorMatrixViewError,
    >
    where
        'kernel: 'view,
    {
        GlobalF32AccumulatorMatrix::checked(output, offset, rows, columns, stride)
    }

    /// Creates a zero accumulator under this exact matrix and policy ownership.
    pub fn bf16_zero_accumulator<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, MatrixBrand>,
    ) -> F32AccumulatorFragment<'wave, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave64, MatrixBrand>
    {
        self.matrix().bf16_zero_accumulator(lane)
    }

    /// Performs the explicit matrix operation under this exact policy owner.
    #[must_use]
    pub fn multiply_accumulate<'wave>(
        &self,
        lhs: Bf16MfmaAFragment<'wave, MatrixBrand>,
        rhs: Bf16MfmaBFragment<'wave, MatrixBrand>,
        accumulator: F32AccumulatorFragment<
            'wave,
            Bf16F32M16N16K16,
            MfmaAccumulatorRowMajor,
            Wave64,
            MatrixBrand,
        >,
    ) -> F32AccumulatorFragment<'wave, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave64, MatrixBrand>
    {
        self.matrix().multiply_accumulate(lhs, rhs, accumulator)
    }
}

/// Row-major 16x16 layout with physical column
/// `column ^ ((row & 3) << 2)`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RowMajorXor4 {}

impl RowMajorXor4 {
    pub const ROWS: usize = 16;
    pub const COLUMNS: usize = 16;
    pub const ELEMENTS: usize = TILE_ELEMENTS;

    pub const fn physical_index(row: usize, column: usize) -> Option<usize> {
        if row >= 16 || column >= 16 {
            return None;
        }
        Some(row * 16 + (column ^ ((row & 3) << 2)))
    }

    pub const fn lane_fragment_origin(lane: usize) -> Option<(usize, usize)> {
        if lane >= 64 {
            return None;
        }
        Some((lane & 15, (lane >> 4) * 4))
    }

    pub const fn lane_fragment_indices(lane: usize) -> Option<[usize; 4]> {
        let Some((row, column)) = Self::lane_fragment_origin(lane) else {
            return None;
        };
        let mut indices = [0; 4];
        let mut component = 0;
        while component < 4 {
            let Some(index) = Self::physical_index(row, column + component) else {
                return None;
            };
            indices[component] = index;
            component += 1;
        }
        Some(indices)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LdsTileShapeError {
    WrongElementCount { expected: usize, actual: usize },
}

/// Exact 16x16 LDS tile retaining the underlying LDS typestate.
pub struct LdsTile16x16<
    'workgroup,
    T: LdsElement,
    State = LdsUninitialized,
    Brand = UnbrandedCapability,
> {
    lds: DynamicLds<'workgroup, T, State, Brand>,
}

/// An exact BF16 LDS tile whose MFMA operand role is part of its type.
///
/// Safe code cannot relabel a tile. The compiler-issued pair fixes the first
/// allocation as A and the second as B, while the publish transition retains
/// both roles and changes only initialization state.
#[rustc_diagnostic_item = "fe2o3_device_mfma_lds_tile16x16_v1"]
pub struct MfmaLdsTile16x16<
    'workgroup,
    Role,
    State = LdsUninitialized,
    StorageLayout = MfmaLdsXor4,
    Brand = UnbrandedCapability,
> {
    tile: LdsTile16x16<'workgroup, Bf16, State, Brand>,
    _role: PhantomData<fn() -> Role>,
    _storage_layout: PhantomData<fn() -> StorageLayout>,
}

impl<'workgroup, Role: sealed::OperandRole, State, Brand>
    MfmaLdsTile16x16<'workgroup, Role, State, MfmaLdsXor4, Brand>
{
    pub const fn len(&self) -> usize {
        TILE_ELEMENTS
    }

    pub const fn is_empty(&self) -> bool {
        false
    }
}

impl<'workgroup, T: LdsElement, State, Brand> LdsTile16x16<'workgroup, T, State, Brand> {
    /// On failure the linear LDS capability is returned.
    pub fn try_from_dynamic(
        lds: DynamicLds<'workgroup, T, State, Brand>,
    ) -> Result<Self, (LdsTileShapeError, DynamicLds<'workgroup, T, State, Brand>)> {
        if lds.len() != TILE_ELEMENTS {
            return Err((
                LdsTileShapeError::WrongElementCount {
                    expected: TILE_ELEMENTS,
                    actual: lds.len(),
                },
                lds,
            ));
        }
        Ok(Self { lds })
    }

    pub fn into_dynamic(self) -> DynamicLds<'workgroup, T, State, Brand> {
        self.lds
    }

    pub const fn len(&self) -> usize {
        TILE_ELEMENTS
    }

    pub const fn is_empty(&self) -> bool {
        false
    }
}

impl<T: LdsElement, Brand> LdsTile16x16<'_, T, LdsUninitialized, Brand> {
    /// Writes only the four elements owned by the current wave64 lane.
    ///
    /// The compiler-issued lane capability maps each wave64 lane to four
    /// disjoint elements during the initialization epoch.
    pub fn write_wave_fragment(&mut self, lane: &WaveLane<Wave64, Brand>, values: [T; 4]) {
        let indices = RowMajorXor4::lane_fragment_indices(lane.get() as usize)
            .expect("authenticated wave64 lane is in range");
        for (index, value) in indices.into_iter().zip(values) {
            debug_assert!(self.lds.write(index, value).is_some());
        }
    }
}

impl<'workgroup, T: LdsElement, Brand> LdsTile16x16<'workgroup, T, LdsUninitialized, Brand> {
    #[cfg(test)]
    unsafe fn assume_init_for_host_test(
        self,
    ) -> LdsTile16x16<'workgroup, T, LdsInitialized, Brand> {
        LdsTile16x16 {
            lds: unsafe { self.lds.assume_init() },
        }
    }
}

impl<T: LdsElement + Copy, Brand> LdsTile16x16<'_, T, LdsInitialized, Brand> {
    /// Reads the four initialized elements owned by an authenticated wave64 lane.
    ///
    /// `WaveLane<Wave64>` proves the lane is in `0..64`, and this tile's private
    /// construction proves it contains exactly 256 initialized elements. The
    /// corresponding XOR4 fragment is therefore always present.
    pub fn read_wave_fragment(&self, lane: &WaveLane<Wave64, Brand>) -> [T; 4] {
        let indices = RowMajorXor4::lane_fragment_indices(lane.get() as usize)
            .expect("authenticated wave64 lane is in range");
        indices.map(|index| *self.lds.get(index).expect("bounded tile index"))
    }
}

impl<'workgroup, Role: sealed::OperandRole, Brand>
    MfmaLdsTile16x16<'workgroup, Role, LdsUninitialized, MfmaLdsXor4, Brand>
{
    /// Writes the four BF16 elements owned by the compiler-issued wave lane.
    #[rustc_diagnostic_item = "fe2o3_device_lds_tile16x16_write_mfma_bf16_v1"]
    pub fn write_mfma_fragment<'wave>(
        &mut self,
        lane: &'wave WaveLane<Wave64, Brand>,
        fragment: Bf16MfmaFragment<
            'wave,
            Role,
            Bf16F32M16N16K16,
            MfmaRegisterTile16x16,
            Wave64,
            Brand,
        >,
    ) {
        self.tile.write_wave_fragment(lane, fragment.into_array())
    }
}

impl<'workgroup, Role: sealed::OperandRole, Brand>
    MfmaLdsTile16x16<'workgroup, Role, LdsInitialized, MfmaLdsXor4, Brand>
{
    #[rustc_diagnostic_item = "fe2o3_device_lds_tile16x16_read_mfma_bf16_v1"]
    pub fn read_mfma_fragment<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, Brand>,
    ) -> Bf16MfmaFragment<'wave, Role, Bf16F32M16N16K16, MfmaRegisterTile16x16, Wave64, Brand> {
        Bf16MfmaFragment::from_values(lane, self.tile.read_wave_fragment(lane))
    }
}

impl<Brand> LdsTile16x16<'_, f32, LdsInitialized, Brand> {
    pub fn read_accumulator_fragment<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64, Brand>,
    ) -> F32AccumulatorFragment<'wave, Bf16F32M16N16K16, MfmaAccumulatorRowMajor, Wave64, Brand>
    {
        F32AccumulatorFragment::from_values_for_wave(lane, self.read_wave_fragment(lane))
    }
}

impl From<Bf16MatrixViewError> for crate::KernelError {
    fn from(_error: Bf16MatrixViewError) -> Self {
        Self::InvalidArgument
    }
}

impl<'wave, Role, Brand> crate::lds::sealed::WorkgroupPipelineSealed
    for Bf16MfmaFragment<'wave, Role, Bf16F32M16N16K16, MfmaRegisterTile16x16, Wave64, Brand>
where
    Role: sealed::OperandRole,
{
}

// SAFETY: this exact fragment representation contains four BF16 values and
// zero-sized lifetime/type brands. It has no runtime reference or destructor.
unsafe impl<'wave, Role, Brand> crate::WorkgroupPipelineElement
    for Bf16MfmaFragment<'wave, Role, Bf16F32M16N16K16, MfmaRegisterTile16x16, Wave64, Brand>
where
    Role: sealed::OperandRole,
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorkgroupLdsScope;
    use core::mem::{align_of, size_of};
    use std::collections::BTreeSet;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::vec::Vec;

    #[test]
    fn profile_and_fragment_layouts_are_exact() {
        enum TestBrand {}

        assert_eq!(MATRIX_CONTRACT_VERSION_V1, 1);
        assert_eq!(
            (
                Bf16F32M16N16K16::M,
                Bf16F32M16N16K16::N,
                Bf16F32M16N16K16::K
            ),
            (16, 16, 16)
        );
        assert_eq!(Bf16F32M16N16K16::WAVE_LANES, 64);
        assert_eq!(size_of::<Bf16MfmaAFragment<'_>>(), 8);
        assert_eq!(size_of::<Bf16MfmaBFragment<'_>>(), 8);
        assert_eq!(align_of::<Bf16MfmaAFragment<'_>>(), align_of::<Bf16>());
        assert_eq!(size_of::<F32AccumulatorFragment<'_>>(), 16);
        assert_eq!(
            size_of::<Bf16MfmaAFragment<'_, TestBrand>>(),
            size_of::<Bf16MfmaAFragment<'_>>()
        );
        assert_eq!(
            size_of::<
                F32AccumulatorFragment<
                    '_,
                    Bf16F32M16N16K16,
                    MfmaAccumulatorRowMajor,
                    Wave64,
                    TestBrand,
                >,
            >(),
            size_of::<F32AccumulatorFragment<'_>>()
        );
        assert_eq!(
            size_of::<Bf16MfmaAMatrix<'_, TestBrand>>(),
            size_of::<Bf16MfmaAMatrix<'_>>()
        );
    }

    #[test]
    fn checked_matrix_views_reject_invalid_storage_contracts() {
        assert_eq!(
            Bf16MfmaAMatrix::row_major(&[0; 6], 0, 2, 3, 2).err(),
            Some(Bf16MatrixViewError::InvalidStride)
        );
        assert_eq!(
            Bf16MfmaBMatrix::row_major(&[0; 5], 0, 2, 3, 3).err(),
            Some(Bf16MatrixViewError::OutOfBounds {
                required: 6,
                actual: 5,
            })
        );
        assert_eq!(
            Bf16MfmaAMatrix::row_major(&[], 1, 2, 1, usize::MAX).err(),
            Some(Bf16MatrixViewError::ExtentOverflow)
        );
    }

    #[test]
    fn direct_matrix_loads_match_the_wave64_distribution() {
        let bits = (0_u16..256).collect::<Vec<_>>();
        let a = Bf16MfmaAMatrix::row_major(&bits, 0, 16, 16, 16).unwrap();
        let b = Bf16MfmaBMatrix::row_major(&bits, 0, 16, 16, 16).unwrap();
        let cases = [
            (0, [0, 1, 2, 3], [0, 16, 32, 48]),
            (15, [240, 241, 242, 243], [15, 31, 47, 63]),
            (16, [4, 5, 6, 7], [64, 80, 96, 112]),
            (63, [252, 253, 254, 255], [207, 223, 239, 255]),
        ];
        for (lane, expected_a, expected_b) in cases {
            let lane = WaveLane::<Wave64>::from_model_snapshot(lane).unwrap();
            assert_eq!(
                a.load_m16k16(&lane, 0, 0).into_array().map(Bf16::to_bits),
                expected_a
            );
            assert_eq!(
                b.load_k16n16(&lane, 0, 0).into_array().map(Bf16::to_bits),
                expected_b
            );
        }
    }

    #[test]
    fn direct_matrix_loads_zero_pad_logical_edges() {
        let bits = [1_u16, 2, 3, 4, 5, 6];
        let a = Bf16MfmaAMatrix::row_major(&bits, 0, 2, 3, 3).unwrap();
        let lane0 = WaveLane::<Wave64>::from_model_snapshot(0).unwrap();
        let lane2 = WaveLane::<Wave64>::from_model_snapshot(2).unwrap();
        assert_eq!(
            a.load_m16k16(&lane0, 0, 0).into_array().map(Bf16::to_bits),
            [1, 2, 3, 0]
        );
        assert_eq!(
            a.load_m16k16(&lane2, 0, 0).into_array().map(Bf16::to_bits),
            [0; 4]
        );
    }

    #[test]
    fn direct_matrix_loads_zero_fill_coordinate_overflow() {
        let bits = [1_u16; 16];
        let a = Bf16MfmaAMatrix::row_major(&bits, 0, 1, 16, 16).unwrap();
        let b = Bf16MfmaBMatrix::row_major(&bits, 0, 16, 1, 1).unwrap();
        let lane = WaveLane::<Wave64>::from_model_snapshot(63).unwrap();
        assert_eq!(
            a.load_m16k16(&lane, usize::MAX, usize::MAX)
                .into_array()
                .map(Bf16::to_bits),
            [0; 4]
        );
        assert_eq!(
            b.load_k16n16(&lane, usize::MAX, usize::MAX)
                .into_array()
                .map(Bf16::to_bits),
            [0; 4]
        );
    }

    #[test]
    fn global_backed_matrix_loads_and_exclusive_c_tails_are_checked() {
        let bits = (0_u16..64).collect::<Vec<_>>();
        let input: Global<'_, u16, ReadOnly, ()> =
            crate::capability_memory::fields::<_, crate::GlobalAddressSpace, _, _>(&bits[..]);
        let a = GlobalBf16MfmaMatrix::<MfmaOperandA, UnbrandedCapability, ()>::checked(
            &input, 0, 3, 16, 16,
        )
        .unwrap();
        let lane = WaveLane::<Wave64>::from_model_snapshot(2).unwrap();
        assert_eq!(
            a.load_m16k16(&lane, 0, 0).into_array().map(Bf16::to_bits),
            [32, 33, 34, 35]
        );

        let mut values = [1.0_f32; 15];
        let mut output: Global<'_, f32, ExclusiveReadWrite, ()> =
            crate::capability_memory::fields::<_, crate::GlobalAddressSpace, _, _>(&mut values[..]);
        let mut c =
            GlobalF32AccumulatorMatrix::<UnbrandedCapability, ()>::checked(&mut output, 0, 3, 1, 5)
                .unwrap();
        assert_eq!(c.load_lane_values(&lane, 0, 0), [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(c.store_lane_values(&lane, 0, 0, [7.0, 8.0, 9.0, 10.0]), 1);
        drop(c);
        drop(output);
        assert_eq!(values[10], 7.0);
        assert_eq!(values[11], 1.0);
    }

    #[test]
    fn xor4_and_wave_mapping_are_bijective() {
        let coordinates = (0..16)
            .flat_map(|row| (0..16).map(move |column| (row, column)))
            .map(|(row, column)| RowMajorXor4::physical_index(row, column).unwrap())
            .collect::<BTreeSet<_>>();
        let wave = (0..64)
            .flat_map(|lane| RowMajorXor4::lane_fragment_indices(lane).unwrap())
            .collect::<BTreeSet<_>>();
        assert_eq!(coordinates, (0..256).collect());
        assert_eq!(wave, coordinates);
        for row in 0..16 {
            for column in 0..16 {
                let physical = RowMajorXor4::physical_index(row, column).unwrap();
                assert_eq!(
                    RowMajorXor4::physical_index(row, physical % 16),
                    Some(row * 16 + column)
                );
            }
        }
        assert_eq!(RowMajorXor4::physical_index(16, 0), None);
        assert_eq!(RowMajorXor4::physical_index(0, 16), None);
        assert_eq!(RowMajorXor4::lane_fragment_indices(64), None);
    }

    #[test]
    fn tile_preserves_typestate_and_fragment_order() {
        let mut storage = [0_u32; 256];
        let mut scope = WorkgroupLdsScope::for_host_test();
        let lds = unsafe {
            DynamicLds::<u32>::from_host_parts_for_test(
                &mut scope,
                storage.as_mut_ptr().cast(),
                size_of::<[u32; 256]>(),
            )
            .unwrap()
        };
        let mut tile = LdsTile16x16::try_from_dynamic(lds).ok().unwrap();
        for lane in 0..64 {
            let witness = WaveLane::<Wave64>::from_model_snapshot(lane).unwrap();
            tile.write_wave_fragment(&witness, [lane; 4]);
        }
        let tile = unsafe { tile.assume_init_for_host_test() };
        for lane in 0..64 {
            let witness = WaveLane::<Wave64>::from_model_snapshot(lane).unwrap();
            assert_eq!(tile.read_wave_fragment(&witness), [lane; 4]);
        }
    }

    #[test]
    fn wrong_extent_returns_the_capability() {
        let mut storage = [0_u32; 255];
        let mut scope = WorkgroupLdsScope::for_host_test();
        let lds = unsafe {
            DynamicLds::<u32>::from_host_parts_for_test(
                &mut scope,
                storage.as_mut_ptr().cast(),
                size_of::<[u32; 255]>(),
            )
            .unwrap()
        };
        let (error, recovered) = match LdsTile16x16::try_from_dynamic(lds) {
            Ok(_) => panic!("wrong tile extent accepted"),
            Err(error) => error,
        };
        assert_eq!(
            error,
            LdsTileShapeError::WrongElementCount {
                expected: 256,
                actual: 255,
            }
        );
        assert_eq!(recovered.len(), 255);
    }

    #[test]
    fn matrix_intrinsic_stub_fails_closed_on_host() {
        assert!(catch_unwind(DeviceMatrix::current).is_err());
        let matrix = DeviceMatrix::for_host_test();
        let bits = [0_u16; TILE_ELEMENTS];
        let a = Bf16MfmaAMatrix::row_major(&bits, 0, 16, 16, 16).unwrap();
        let b = Bf16MfmaBMatrix::row_major(&bits, 0, 16, 16, 16).unwrap();
        let lane = WaveLane::<Wave64>::from_model_snapshot(0).unwrap();
        let lhs = a.load_m16k16(&lane, 0, 0);
        let rhs = b.load_k16n16(&lane, 0, 0);
        let accumulator = F32AccumulatorFragment::zero(&lane);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                matrix.multiply_accumulate(lhs, rhs, accumulator)
            }))
            .is_err()
        );
    }
}
