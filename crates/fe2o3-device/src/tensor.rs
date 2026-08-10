//! Bounded target-neutral matrix fragments and LDS tile interop.
//!
//! V1 admits one exact profile: a full wave64 cooperates on a 16x16x16 BF16
//! multiply with four FP32 accumulator registers per lane. Only authenticated
//! gfx942 lowering may replace the fail-closed device intrinsic stub.

use core::marker::PhantomData;

use crate::{Bf16, DynamicLds, LdsElement, LdsInitialized, LdsUninitialized};

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
    /// # Safety
    ///
    /// Only compiler-generated device code may call this. The compiler must
    /// replace it after proving wave64 mode, gfx942 MFMA support, and the V1
    /// floating-point policy.
    #[doc(hidden)]
    #[inline(never)]
    #[rustc_diagnostic_item = "fe2o3_device_matrix_context_from_compiler_v1"]
    pub unsafe fn from_compiler() -> Self {
        unreachable!("DeviceMatrix must be created by authenticated device lowering")
    }

    /// Performs one full-wave BF16 multiply-accumulate.
    ///
    /// Every active lane must call uniformly with V1-distributed fragments.
    /// Gfx942 maps this to `llvm.amdgcn.mfma.f32.16x16x16bf16.1k` with zero
    /// control immediates.
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
        unreachable!("matrix operation requires authenticated gfx942 wave64 lowering")
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
    pub fn write_wave_fragment(&mut self, lane: usize, values: [T; 4]) -> bool {
        let Some(indices) = RowMajorXor4::lane_fragment_indices(lane) else {
            return false;
        };
        for (index, value) in indices.into_iter().zip(values) {
            debug_assert!(self.lds.write(index, value).is_some());
        }
        true
    }
}

impl<'workgroup, T: LdsElement> LdsTile16x16<'workgroup, T, LdsUninitialized> {
    /// # Safety
    ///
    /// All 256 elements must be initialized and cooperative writes must happen
    /// before subsequent reads.
    pub unsafe fn assume_init(self) -> LdsTile16x16<'workgroup, T, LdsInitialized> {
        LdsTile16x16 {
            lds: unsafe { self.lds.assume_init() },
        }
    }
}

impl<T: LdsElement + Copy> LdsTile16x16<'_, T, LdsInitialized> {
    pub fn read_wave_fragment(&self, lane: usize) -> Option<[T; 4]> {
        let indices = RowMajorXor4::lane_fragment_indices(lane)?;
        Some(indices.map(|index| *self.lds.get(index).expect("bounded tile index")))
    }
}

impl LdsTile16x16<'_, Bf16, LdsUninitialized> {
    pub fn write_mfma_fragment(&mut self, lane: usize, fragment: Bf16MfmaFragment) -> bool {
        self.write_wave_fragment(lane, fragment.to_array())
    }
}

impl LdsTile16x16<'_, Bf16, LdsInitialized> {
    pub fn read_mfma_fragment(&self, lane: usize) -> Option<Bf16MfmaFragment> {
        self.read_wave_fragment(lane).map(Bf16MfmaFragment::new)
    }
}

impl LdsTile16x16<'_, f32, LdsInitialized> {
    pub fn read_accumulator_fragment(&self, lane: usize) -> Option<F32AccumulatorFragment> {
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
            assert!(tile.write_wave_fragment(lane, [lane as u32; 4]));
        }
        assert!(!tile.write_wave_fragment(64, [0; 4]));
        let tile = unsafe { tile.assume_init() };
        for lane in 0..64 {
            assert_eq!(tile.read_wave_fragment(lane), Some([lane as u32; 4]));
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
        let matrix = DeviceMatrix::for_host_test();
        assert!(
            catch_unwind(AssertUnwindSafe(|| matrix.multiply_accumulate(
                Bf16MfmaFragment::ZERO,
                Bf16MfmaFragment::ZERO,
                F32AccumulatorFragment::ZERO,
            )))
            .is_err()
        );
    }
}
