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

use crate::{Bf16, DynamicLds, LdsElement, LdsInitialized, LdsUninitialized, Wave64, WaveLane};

pub const MATRIX_CONTRACT_VERSION_V1: u16 = 1;
pub const BF16_F32_MFMA_M: usize = 16;
pub const BF16_F32_MFMA_N: usize = 16;
pub const BF16_F32_MFMA_REDUCTION: usize = 16;
pub const BF16_F32_MFMA_WAVE_LANES: usize = 64;

const TILE_ELEMENTS: usize = 16 * 16;

/// The sole matrix-instruction profile admitted by V1.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Bf16F32M16N16K16 {}

impl Bf16F32M16N16K16 {
    pub const M: usize = BF16_F32_MFMA_M;
    pub const N: usize = BF16_F32_MFMA_N;
    pub const K: usize = BF16_F32_MFMA_REDUCTION;
    pub const WAVE_LANES: usize = BF16_F32_MFMA_WAVE_LANES;
}

/// Four BF16 values consumed by one lane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_fragment_v1"]
pub struct Bf16MfmaFragment([Bf16; 4]);

impl Bf16MfmaFragment {
    pub const ZERO: Self = Self([Bf16::ZERO; 4]);

    pub const fn new(values: [Bf16; 4]) -> Self {
        Self(values)
    }

    /// Reinterprets four physical `u16` values as one BF16 MFMA fragment.
    ///
    /// The collected tiled GEMM profile admits this only as a bit-preserving
    /// source-to-Kernel-IR bridge. It performs no numeric conversion.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_bf16_mfma_fragment_from_bits_v1"]
    pub fn from_bits(bits: [u16; 4]) -> Self {
        Self(bits.map(Bf16::from_bits))
    }

    pub const fn to_array(self) -> [Bf16; 4] {
        self.0
    }
}

/// Four FP32 accumulator values owned by one lane.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
#[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_v1"]
pub struct F32AccumulatorFragment([f32; 4]);

impl F32AccumulatorFragment {
    pub const ZERO: Self = Self([0.0; 4]);

    pub const fn new(values: [f32; 4]) -> Self {
        Self(values)
    }

    /// Creates one lane's exact four-value FP32 accumulator fragment.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_from_values_v1"]
    pub fn from_values(values: [f32; 4]) -> Self {
        Self(values)
    }

    /// Returns the four FP32 results uniquely owned by this lane.
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_f32_accumulator_fragment_into_values_v1"]
    pub fn into_values(self) -> [f32; 4] {
        self.0
    }

    pub const fn to_array(self) -> [f32; 4] {
        self.0
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
    pub fn multiply_accumulate(
        &self,
        lhs: Bf16MfmaFragment,
        rhs: Bf16MfmaFragment,
        accumulator: F32AccumulatorFragment,
    ) -> F32AccumulatorFragment {
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
    LdsTile16x16<'workgroup, Bf16>,
    LdsTile16x16<'workgroup, Bf16>,
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
    pub fn read_wave_fragment(&self, lane: &WaveLane<Wave64>) -> Option<[T; 4]> {
        let indices = RowMajorXor4::lane_fragment_indices(lane.get() as usize)
            .expect("authenticated wave64 lane is in range");
        Some(indices.map(|index| *self.lds.get(index).expect("bounded tile index")))
    }
}

impl LdsTile16x16<'_, Bf16, LdsUninitialized> {
    /// Writes the four BF16 elements owned by the compiler-issued wave lane.
    #[rustc_diagnostic_item = "fe2o3_device_lds_tile16x16_write_mfma_bf16_v1"]
    pub fn write_mfma_fragment(&mut self, lane: &WaveLane<Wave64>, fragment: Bf16MfmaFragment) {
        self.write_wave_fragment(lane, fragment.to_array())
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
    lhs: LdsTile16x16<'workgroup, Bf16, LdsUninitialized>,
    rhs: LdsTile16x16<'workgroup, Bf16, LdsUninitialized>,
) -> (
    LdsTile16x16<'workgroup, Bf16, LdsInitialized>,
    LdsTile16x16<'workgroup, Bf16, LdsInitialized>,
) {
    let _ = (lhs, rhs);
    unreachable!("initialized BF16 LDS tile pairs must be published by authenticated lowering")
}

impl LdsTile16x16<'_, Bf16, LdsInitialized> {
    #[rustc_diagnostic_item = "fe2o3_device_lds_tile16x16_read_mfma_bf16_v1"]
    pub fn read_mfma_fragment(&self, lane: &WaveLane<Wave64>) -> Option<Bf16MfmaFragment> {
        self.read_wave_fragment(lane).map(Bf16MfmaFragment::new)
    }
}

impl LdsTile16x16<'_, f32, LdsInitialized> {
    pub fn read_accumulator_fragment(
        &self,
        lane: &WaveLane<Wave64>,
    ) -> Option<F32AccumulatorFragment> {
        self.read_wave_fragment(lane)
            .map(F32AccumulatorFragment::new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WorkgroupLdsScope;
    use core::mem::{align_of, size_of};
    use std::collections::BTreeSet;
    use std::panic::{AssertUnwindSafe, catch_unwind};

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
        assert_eq!(size_of::<Bf16MfmaFragment>(), 8);
        assert_eq!(align_of::<Bf16MfmaFragment>(), align_of::<Bf16>());
        assert_eq!(size_of::<F32AccumulatorFragment>(), 16);
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
            assert_eq!(tile.read_wave_fragment(&witness), Some([lane; 4]));
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
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                matrix.multiply_accumulate(
                    Bf16MfmaFragment::ZERO,
                    Bf16MfmaFragment::ZERO,
                    F32AccumulatorFragment::ZERO,
                )
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
        let lhs = LdsTile16x16::try_from_dynamic(lhs).ok().unwrap();
        let rhs = LdsTile16x16::try_from_dynamic(rhs).ok().unwrap();
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                gfx942_publish_lds_bf16_tile_pair_m16x16_v1(lhs, rhs)
            }))
            .is_err()
        );
    }
}
