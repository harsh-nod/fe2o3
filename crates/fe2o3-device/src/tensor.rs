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
    Bf16, DynamicLds, LdsElement, LdsInitialized, LdsUninitialized, Wave64, WaveLane, WaveWidth,
};

pub const MATRIX_CONTRACT_VERSION_V1: u16 = 1;
pub const BF16_F32_MFMA_M: usize = 16;
pub const BF16_F32_MFMA_N: usize = 16;
pub const BF16_F32_MFMA_REDUCTION: usize = 16;
pub const BF16_F32_MFMA_WAVE_LANES: usize = 64;

const TILE_ELEMENTS: usize = 16 * 16;

mod sealed {
    pub trait OperandRole {}
    pub trait OperandDistribution {}
    pub trait CompatibleDistribution<Rhs> {}
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

/// Lane distribution loaded directly from a logical row-major matrix.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_mfma_row_major_distribution_v1"]
pub enum MfmaRowMajor {}

impl sealed::OperandDistribution for MfmaRowMajor {}

/// Lane distribution read from a published XOR4 LDS tile.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_mfma_xor4_distribution_v1"]
pub enum MfmaRowMajorXor4 {}

impl sealed::OperandDistribution for MfmaRowMajorXor4 {}

/// Output distribution containing four row-major accumulator elements per lane.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[rustc_diagnostic_item = "fe2o3_device_mfma_accumulator_row_major_distribution_v1"]
pub enum MfmaAccumulatorRowMajor {}

/// Sealed relation between operand distributions accepted by one MFMA.
pub trait CompatibleMfmaDistribution<Rhs>: sealed::CompatibleDistribution<Rhs> {}

impl sealed::CompatibleDistribution<MfmaRowMajor> for MfmaRowMajor {}
impl CompatibleMfmaDistribution<MfmaRowMajor> for MfmaRowMajor {}
impl sealed::CompatibleDistribution<MfmaRowMajorXor4> for MfmaRowMajorXor4 {}
impl CompatibleMfmaDistribution<MfmaRowMajorXor4> for MfmaRowMajorXor4 {}

/// A role-, profile-, distribution-, and wave-associated four-value BF16 fragment.
///
/// The payload and constructors are private. Safe code obtains a fragment only
/// from a checked matrix view or a matching published LDS tile. The invariant
/// lifetime retains the borrow of the authenticated wave witness through every
/// fragment use; production admission separately enforces unique acquisition of
/// that witness for the kernel invocation.
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_fragment_v1"]
pub struct Bf16MfmaFragment<'wave, Role, Profile, Distribution, Width: WaveWidth> {
    values: [Bf16; 4],
    _contract: PhantomData<
        fn(&'wave WaveLane<Width>, Role, Profile, Distribution) -> &'wave WaveLane<Width>,
    >,
}

impl<'wave, Role, Profile, Distribution, Width: WaveWidth>
    Bf16MfmaFragment<'wave, Role, Profile, Distribution, Width>
{
    fn from_values(_lane: &'wave WaveLane<Width>, values: [Bf16; 4]) -> Self {
        Self {
            values,
            _contract: PhantomData,
        }
    }

    fn into_array(self) -> [Bf16; 4] {
        self.values
    }
}

/// Direct row-major A operand for the supported BF16 MFMA profile.
pub type Bf16MfmaAFragment<'wave, Distribution> =
    Bf16MfmaFragment<'wave, MfmaOperandA, Bf16F32M16N16K16, Distribution, Wave64>;

/// Direct row-major B operand for the supported BF16 MFMA profile.
pub type Bf16MfmaBFragment<'wave, Distribution> =
    Bf16MfmaFragment<'wave, MfmaOperandB, Bf16F32M16N16K16, Distribution, Wave64>;

/// Four typed FP32 accumulator values owned by one wave lane.
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_v1"]
pub struct F32AccumulatorFragment<
    'wave,
    Profile = Bf16F32M16N16K16,
    Distribution = MfmaAccumulatorRowMajor,
    Width: WaveWidth = Wave64,
> {
    values: [f32; 4],
    _contract:
        PhantomData<fn(&'wave WaveLane<Width>, Profile, Distribution) -> &'wave WaveLane<Width>>,
}

impl<'wave> F32AccumulatorFragment<'wave> {
    /// Creates the zero accumulator associated with one authenticated wave lane.
    #[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_zero_v1"]
    pub fn zero(lane: &'wave WaveLane<Wave64>) -> Self {
        Self::from_values_for_wave(lane, [0.0; 4])
    }
}

impl<'wave, Profile, Distribution, Width: WaveWidth>
    F32AccumulatorFragment<'wave, Profile, Distribution, Width>
{
    fn from_values_for_wave(_lane: &'wave WaveLane<Width>, values: [f32; 4]) -> Self {
        Self {
            values,
            _contract: PhantomData,
        }
    }

    /// Returns the four FP32 results uniquely owned by this lane.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_into_values_v1"]
    pub fn into_values(self) -> [f32; 4] {
        self.values
    }
}

/// Rejection while establishing one checked row-major BF16 matrix view.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
pub struct Bf16MfmaMatrix<'data, Role> {
    bits: &'data [u16],
    offset: usize,
    rows: usize,
    columns: usize,
    stride: usize,
    _role: PhantomData<fn() -> Role>,
}

/// Checked row-major storage for an MFMA A operand.
pub type Bf16MfmaAMatrix<'data> = Bf16MfmaMatrix<'data, MfmaOperandA>;

/// Checked row-major storage for an MFMA B operand.
pub type Bf16MfmaBMatrix<'data> = Bf16MfmaMatrix<'data, MfmaOperandB>;

impl<'data, Role: sealed::OperandRole> Bf16MfmaMatrix<'data, Role> {
    fn checked(
        bits: &'data [u16],
        offset: usize,
        rows: usize,
        columns: usize,
        stride: usize,
    ) -> Result<Self, Bf16MatrixViewError> {
        if rows != 0 && columns != 0 && stride < columns {
            return Err(Bf16MatrixViewError::InvalidStride);
        }
        let required = if rows == 0 || columns == 0 {
            offset
        } else {
            offset
                .checked_add(
                    (rows - 1)
                        .checked_mul(stride)
                        .and_then(|value| value.checked_add(columns))
                        .ok_or(Bf16MatrixViewError::ExtentOverflow)?,
                )
                .ok_or(Bf16MatrixViewError::ExtentOverflow)?
        };
        if required > bits.len() {
            return Err(Bf16MatrixViewError::OutOfBounds {
                required,
                actual: bits.len(),
            });
        }
        Ok(Self {
            bits,
            offset,
            rows,
            columns,
            stride,
            _role: PhantomData,
        })
    }

    fn value_or_zero(&self, row: usize, column: usize) -> Option<Bf16> {
        if row >= self.rows || column >= self.columns {
            return Some(Bf16::ZERO);
        }
        let index = self
            .offset
            .checked_add(row.checked_mul(self.stride)?)?
            .checked_add(column)?;
        self.bits.get(index).copied().map(Bf16::from_bits)
    }
}

impl<'data> Bf16MfmaMatrix<'data, MfmaOperandA> {
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

    /// Loads this lane's four values from one logical M16xK16 A tile.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_matrix_a_load_v1"]
    pub fn load_m16k16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64>,
        row_base: usize,
        reduction_base: usize,
    ) -> Option<Bf16MfmaAFragment<'wave, MfmaRowMajor>> {
        let lane_index = lane.get() as usize;
        let row = row_base.checked_add(lane_index & 15)?;
        let first_reduction = reduction_base.checked_add((lane_index >> 4) * 4)?;
        let mut values = [Bf16::ZERO; 4];
        let mut component = 0;
        while component < 4 {
            values[component] = self.value_or_zero(row, first_reduction.checked_add(component)?)?;
            component += 1;
        }
        Some(Bf16MfmaFragment::from_values(lane, values))
    }
}

impl<'data> Bf16MfmaMatrix<'data, MfmaOperandB> {
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

    /// Loads this lane's four values from one logical K16xN16 B tile.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_matrix_b_load_v1"]
    pub fn load_k16n16<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64>,
        reduction_base: usize,
        column_base: usize,
    ) -> Option<Bf16MfmaBFragment<'wave, MfmaRowMajor>> {
        let lane_index = lane.get() as usize;
        let column = column_base.checked_add(lane_index & 15)?;
        let first_reduction = reduction_base.checked_add((lane_index >> 4) * 4)?;
        let mut values = [Bf16::ZERO; 4];
        let mut component = 0;
        while component < 4 {
            values[component] =
                self.value_or_zero(first_reduction.checked_add(component)?, column)?;
            component += 1;
        }
        Some(Bf16MfmaFragment::from_values(lane, values))
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
    pub fn multiply_accumulate<'wave, ADistribution, BDistribution>(
        &self,
        lhs: Bf16MfmaAFragment<'wave, ADistribution>,
        rhs: Bf16MfmaBFragment<'wave, BDistribution>,
        accumulator: F32AccumulatorFragment<'wave>,
    ) -> F32AccumulatorFragment<'wave>
    where
        ADistribution: sealed::OperandDistribution + CompatibleMfmaDistribution<BDistribution>,
        BDistribution: sealed::OperandDistribution,
    {
        let _ = (self, lhs, rhs, accumulator);
        unreachable!("matrix operation requires provider-bound gfx942 wave64 lowering")
    }

    #[cfg(test)]
    fn for_host_test() -> Self {
        Self {
            _private: (),
            _not_send_sync: PhantomData,
        }
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
pub struct LdsTile16x16<'workgroup, T: LdsElement, State = LdsUninitialized> {
    lds: DynamicLds<'workgroup, T, State>,
}

/// An exact BF16 LDS tile whose MFMA operand role is part of its type.
///
/// Safe code cannot relabel a tile. The compiler-issued pair fixes the first
/// allocation as A and the second as B, while the publish transition retains
/// both roles and changes only initialization state.
#[rustc_diagnostic_item = "fe2o3_device_mfma_lds_tile16x16_v1"]
pub struct MfmaLdsTile16x16<'workgroup, Role, State = LdsUninitialized> {
    tile: LdsTile16x16<'workgroup, Bf16, State>,
    _role: PhantomData<fn() -> Role>,
}

impl<'workgroup, Role: sealed::OperandRole, State> MfmaLdsTile16x16<'workgroup, Role, State> {
    #[cfg(test)]
    fn from_tile(tile: LdsTile16x16<'workgroup, Bf16, State>) -> Self {
        Self {
            tile,
            _role: PhantomData,
        }
    }

    pub const fn len(&self) -> usize {
        TILE_ELEMENTS
    }

    pub const fn is_empty(&self) -> bool {
        false
    }
}

/// Issues the exact pair of static BF16 LDS tiles used by the bounded gfx942
/// tiled-GEMM Slice 1 profile.
///
/// This is a compiler intrinsic, not a general allocator. The fe2o3 compiler
/// may recognize it only in the authenticated `gfx942:xnack-`, WG64 Slice 1
/// source profile. Recognition creates two distinct 512-byte, 16-byte-aligned
/// workgroup allocations. Unsupported compilation and host execution trap.
///
/// The compiler issues this pair only for the exact authenticated Slice 1
/// kernel. The returned linear capabilities cannot be duplicated in safe Rust.
#[doc(hidden)]
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_gfx942_lds_bf16_tile_pair_m16x16_v1"]
pub fn gfx942_lds_bf16_tile_pair_m16x16_v1<'workgroup>() -> (
    MfmaLdsTile16x16<'workgroup, MfmaOperandA>,
    MfmaLdsTile16x16<'workgroup, MfmaOperandB>,
) {
    unreachable!("static BF16 LDS tile pairs must be issued by the fe2o3 compiler")
}

impl<'workgroup, T: LdsElement, State> LdsTile16x16<'workgroup, T, State> {
    /// On failure the linear LDS capability is returned.
    pub fn try_from_dynamic(
        lds: DynamicLds<'workgroup, T, State>,
    ) -> Result<Self, (LdsTileShapeError, DynamicLds<'workgroup, T, State>)> {
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

    pub fn into_dynamic(self) -> DynamicLds<'workgroup, T, State> {
        self.lds
    }

    pub const fn len(&self) -> usize {
        TILE_ELEMENTS
    }

    pub const fn is_empty(&self) -> bool {
        false
    }
}

impl<T: LdsElement> LdsTile16x16<'_, T, LdsUninitialized> {
    /// Writes only the four elements owned by the current wave64 lane.
    ///
    /// The compiler-issued lane capability maps each wave64 lane to four
    /// disjoint elements during the initialization epoch.
    pub fn write_wave_fragment(&mut self, lane: &WaveLane<Wave64>, values: [T; 4]) {
        let indices = RowMajorXor4::lane_fragment_indices(lane.get() as usize)
            .expect("authenticated wave64 lane is in range");
        for (index, value) in indices.into_iter().zip(values) {
            debug_assert!(self.lds.write(index, value).is_some());
        }
    }
}

impl<'workgroup, T: LdsElement> LdsTile16x16<'workgroup, T, LdsUninitialized> {
    #[cfg(test)]
    unsafe fn assume_init_for_host_test(self) -> LdsTile16x16<'workgroup, T, LdsInitialized> {
        LdsTile16x16 {
            lds: unsafe { self.lds.assume_init() },
        }
    }
}

impl<T: LdsElement + Copy> LdsTile16x16<'_, T, LdsInitialized> {
    /// Reads the four initialized elements owned by an authenticated wave64 lane.
    ///
    /// `WaveLane<Wave64>` proves the lane is in `0..64`, and this tile's private
    /// construction proves it contains exactly 256 initialized elements. The
    /// corresponding XOR4 fragment is therefore always present.
    pub fn read_wave_fragment(&self, lane: &WaveLane<Wave64>) -> [T; 4] {
        let indices = RowMajorXor4::lane_fragment_indices(lane.get() as usize)
            .expect("authenticated wave64 lane is in range");
        indices.map(|index| *self.lds.get(index).expect("bounded tile index"))
    }
}

impl<'workgroup, Role: sealed::OperandRole> MfmaLdsTile16x16<'workgroup, Role, LdsUninitialized> {
    /// Writes the four BF16 elements owned by the compiler-issued wave lane.
    #[rustc_diagnostic_item = "fe2o3_device_lds_tile16x16_write_mfma_bf16_v1"]
    pub fn write_mfma_fragment<'wave>(
        &mut self,
        lane: &'wave WaveLane<Wave64>,
        fragment: Bf16MfmaFragment<'wave, Role, Bf16F32M16N16K16, MfmaRowMajor, Wave64>,
    ) {
        self.tile.write_wave_fragment(lane, fragment.into_array())
    }
}

/// Publishes a completely initialized BF16 LDS tile pair.
///
/// Authenticated lowering proves that every lane wrote its four disjoint
/// elements in both tiles and inserts the required workgroup barrier. The
/// consuming typestate transition makes initialized reads unavailable before
/// that proof. Unsupported lowering and host execution trap.
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_gfx942_lds_bf16_tile_pair_publish_v1"]
pub fn gfx942_publish_lds_bf16_tile_pair_m16x16_v1<'workgroup>(
    lhs: MfmaLdsTile16x16<'workgroup, MfmaOperandA, LdsUninitialized>,
    rhs: MfmaLdsTile16x16<'workgroup, MfmaOperandB, LdsUninitialized>,
) -> (
    MfmaLdsTile16x16<'workgroup, MfmaOperandA, LdsInitialized>,
    MfmaLdsTile16x16<'workgroup, MfmaOperandB, LdsInitialized>,
) {
    let _ = (lhs, rhs);
    unreachable!("initialized BF16 LDS tile pairs must be published by authenticated lowering")
}

impl<'workgroup, Role: sealed::OperandRole> MfmaLdsTile16x16<'workgroup, Role, LdsInitialized> {
    #[rustc_diagnostic_item = "fe2o3_device_lds_tile16x16_read_mfma_bf16_v1"]
    pub fn read_mfma_fragment<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64>,
    ) -> Bf16MfmaFragment<'wave, Role, Bf16F32M16N16K16, MfmaRowMajorXor4, Wave64> {
        Bf16MfmaFragment::from_values(lane, self.tile.read_wave_fragment(lane))
    }
}

impl LdsTile16x16<'_, f32, LdsInitialized> {
    pub fn read_accumulator_fragment<'wave>(
        &self,
        lane: &'wave WaveLane<Wave64>,
    ) -> F32AccumulatorFragment<'wave> {
        F32AccumulatorFragment::from_values_for_wave(lane, self.read_wave_fragment(lane))
    }
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
        assert_eq!(size_of::<Bf16MfmaAFragment<'_, MfmaRowMajor>>(), 8);
        assert_eq!(size_of::<Bf16MfmaBFragment<'_, MfmaRowMajor>>(), 8);
        assert_eq!(size_of::<Bf16MfmaAFragment<'_, MfmaRowMajorXor4>>(), 8);
        assert_eq!(
            align_of::<Bf16MfmaAFragment<'_, MfmaRowMajor>>(),
            align_of::<Bf16>()
        );
        assert_eq!(size_of::<F32AccumulatorFragment<'_>>(), 16);
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
                a.load_m16k16(&lane, 0, 0)
                    .unwrap()
                    .into_array()
                    .map(Bf16::to_bits),
                expected_a
            );
            assert_eq!(
                b.load_k16n16(&lane, 0, 0)
                    .unwrap()
                    .into_array()
                    .map(Bf16::to_bits),
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
            a.load_m16k16(&lane0, 0, 0)
                .unwrap()
                .into_array()
                .map(Bf16::to_bits),
            [1, 2, 3, 0]
        );
        assert_eq!(
            a.load_m16k16(&lane2, 0, 0)
                .unwrap()
                .into_array()
                .map(Bf16::to_bits),
            [0; 4]
        );
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
    fn intrinsic_stub_fails_closed_on_host() {
        assert!(catch_unwind(DeviceMatrix::current).is_err());
        assert!(catch_unwind(gfx942_lds_bf16_tile_pair_m16x16_v1).is_err());
        let matrix = DeviceMatrix::for_host_test();
        let bits = [0_u16; TILE_ELEMENTS];
        let a = Bf16MfmaAMatrix::row_major(&bits, 0, 16, 16, 16).unwrap();
        let b = Bf16MfmaBMatrix::row_major(&bits, 0, 16, 16, 16).unwrap();
        let lane = WaveLane::<Wave64>::from_model_snapshot(0).unwrap();
        let lhs = a.load_m16k16(&lane, 0, 0).unwrap();
        let rhs = b.load_k16n16(&lane, 0, 0).unwrap();
        let accumulator = F32AccumulatorFragment::zero(&lane);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                matrix.multiply_accumulate(lhs, rhs, accumulator)
            }))
            .is_err()
        );
    }

    #[test]
    fn lds_tile_publish_fails_closed_on_host() {
        let mut lhs_storage = [Bf16::ZERO; TILE_ELEMENTS];
        let mut rhs_storage = [Bf16::ZERO; TILE_ELEMENTS];
        let mut lhs_scope = WorkgroupLdsScope::for_host_test();
        let mut rhs_scope = WorkgroupLdsScope::for_host_test();
        let lhs = unsafe {
            DynamicLds::<Bf16>::from_host_parts_for_test(
                &mut lhs_scope,
                lhs_storage.as_mut_ptr().cast(),
                size_of::<[Bf16; TILE_ELEMENTS]>(),
            )
            .unwrap()
        };
        let rhs = unsafe {
            DynamicLds::<Bf16>::from_host_parts_for_test(
                &mut rhs_scope,
                rhs_storage.as_mut_ptr().cast(),
                size_of::<[Bf16; TILE_ELEMENTS]>(),
            )
            .unwrap()
        };
        let lhs = MfmaLdsTile16x16::<MfmaOperandA>::from_tile(
            LdsTile16x16::try_from_dynamic(lhs).ok().unwrap(),
        );
        let rhs = MfmaLdsTile16x16::<MfmaOperandB>::from_tile(
            LdsTile16x16::try_from_dynamic(rhs).ok().unwrap(),
        );
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                gfx942_publish_lds_bf16_tile_pair_m16x16_v1(lhs, rhs)
            }))
            .is_err()
        );
    }
}
