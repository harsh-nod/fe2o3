#![no_std]
#![feature(rustc_attrs)]
#![allow(internal_features)]

//! Safe, compiler-issued capabilities for a conservative tiled GEMM.
//!
//! This module is the safe Rust source boundary for the general wave64 GEMM
//! profile. It deliberately does not expose the underlying lane witness, LDS
//! pointers, matrix context, barriers, or arbitrary output indexing. A linear
//! typestate value owns those capabilities conceptually and permits only this
//! sequence for every K phase:
//!
//! `Ready -> Staged -> Published -> Consumed -> Ready`.
//!
//! The accumulator and two distinct XOR4 LDS tiles remain inside that value.
//! Therefore ordinary safe kernel source cannot read an unpublished tile,
//! omit the reuse barrier, reset the accumulator between phases, or select an
//! arbitrary output address through this API.
//!
//! Every operation is currently a fail-closed compiler intrinsic. Host rustc
//! reaches a panic stub. The fe2o3 backend must not replace any stub until it
//! has authenticated the provider crate and kernel, proved the operation's
//! semantic obligations, and selected the exact `gfx942:xnack-` wave64
//! profile. This module does not claim that source import, proof discharge,
//! LLVM lowering, artifact publication, or GPU execution is implemented.

use core::marker::PhantomData;

pub use fe2o3_device::DisjointSlice;

#[cfg(test)]
extern crate std;

/// Version of the safe general tiled-GEMM device contract.
pub const GENERAL_TILED_GEMM_DEVICE_CONTRACT_VERSION_V1: u16 = 1;
/// Logical rows in one output tile.
pub const GENERAL_TILED_GEMM_TILE_M_V1: u32 = 16;
/// Logical columns in one output tile.
pub const GENERAL_TILED_GEMM_TILE_N_V1: u32 = 16;
/// Reduction values staged by one phase.
pub const GENERAL_TILED_GEMM_TILE_K_V1: u32 = 16;
/// Required physical lanes in the one-wave workgroup.
pub const GENERAL_TILED_GEMM_WAVE_LANES_V1: u32 = 64;
/// Bytes reserved for two separate 16x16 BF16 XOR4 LDS tiles.
pub const GENERAL_TILED_GEMM_LDS_BYTES_V1: u32 = 2 * 16 * 16 * 2;

mod sealed {
    pub trait Sealed {}
}

/// Sealed state of one linear tiled-GEMM phase capability.
pub trait GemmPhaseState: sealed::Sealed {}

/// The LDS tiles may be written for the current phase.
#[derive(Debug)]
pub enum GemmReady {}
/// Every lane has written its disjoint A and B fragments for the phase.
#[derive(Debug)]
pub enum GemmStaged {}
/// A convergent publish barrier has made both complete tiles readable.
#[derive(Debug)]
pub enum GemmPublished {}
/// MFMA consumed the published tiles; a reuse barrier is still required.
#[derive(Debug)]
pub enum GemmConsumed {}

impl sealed::Sealed for GemmReady {}
impl sealed::Sealed for GemmStaged {}
impl sealed::Sealed for GemmPublished {}
impl sealed::Sealed for GemmConsumed {}
impl GemmPhaseState for GemmReady {}
impl GemmPhaseState for GemmStaged {}
impl GemmPhaseState for GemmPublished {}
impl GemmPhaseState for GemmConsumed {}

/// Linear authority for one wave64 general tiled-GEMM output tile.
///
/// The value is neither `Copy`, `Clone`, `Send`, nor `Sync`. Its fields are
/// private, its phase states are sealed, and no public unsafe constructor is
/// provided. The hidden accumulator starts at positive zero and is carried by
/// every consuming transition. `tile_row` and `tile_column` are compiler-issued
/// workgroup coordinates; `lane` is the physical wave64 lane identity.
#[must_use = "the tiled-GEMM phase capability must reach a store or its next state"]
pub struct Gfx942TiledGemmWave64V1<State: GemmPhaseState> {
    lane: u32,
    tile_row: u32,
    tile_column: u32,
    epoch: u32,
    phases: u32,
    #[allow(dead_code)] // Carried opaquely until the MFMA intrinsic is lowered.
    accumulator: [f32; 4],
    _state: PhantomData<fn() -> State>,
    _not_send_sync: PhantomData<*mut ()>,
}

impl Gfx942TiledGemmWave64V1<GemmReady> {
    /// Requests the current invocation's safe general-GEMM capability.
    ///
    /// `k` fixes the private phase count to `ceil(k / 16)`. The compiler must
    /// issue exactly one capability per invocation, authenticate a 64x1x1
    /// workgroup, attach the current lane and 2D workgroup coordinates, reserve
    /// two non-overlapping 512-byte LDS allocations, and initialize the private
    /// accumulator to positive zero.
    ///
    /// This function is safe because an ordinary or unsupported compilation
    /// cannot manufacture authority: it panics. Replacing the panic is a
    /// trusted compiler action gated on the obligations above.
    #[inline(always)]
    pub fn from_compiler(k: u32) -> Self {
        // SAFETY: the private intrinsic either fails closed or is replaced by
        // provider-authenticated compiler lowering that establishes its full
        // contract. No caller assertion is accepted as authority.
        unsafe { acquire_gfx942_tiled_gemm_wave64_v1(k) }
    }

    /// Returns the authenticated physical lane in `0..64` as coordinate data.
    pub const fn lane(&self) -> u32 {
        self.lane
    }

    /// Returns the output tile's row coordinate in the launch grid.
    pub const fn tile_row(&self) -> u32 {
        self.tile_row
    }

    /// Returns the output tile's column coordinate in the launch grid.
    pub const fn tile_column(&self) -> u32 {
        self.tile_column
    }

    /// Returns the next K-phase epoch, starting at zero.
    pub const fn phase(&self) -> u32 {
        self.epoch
    }

    /// Reports whether another complete or zero-filled K phase is required.
    pub const fn has_remaining_phases(&self) -> bool {
        self.epoch < self.phases
    }

    /// Stages this lane's four A and four transposed-B BF16 values.
    ///
    /// The compiler maps component `c` of lane `l` to logical staging depth
    /// `4 * (l / 16) + c`. A uses row `l % 16`; B uses column `l % 16` and is
    /// stored transposed in its separate XOR4 tile. The caller supplies zero
    /// bits for any guarded tail element. The private epoch selects the tile's
    /// K origin, so source code cannot write a different phase epoch.
    #[inline(always)]
    pub fn stage(self, a_bits: [u16; 4], b_bits: [u16; 4]) -> Gfx942TiledGemmWave64V1<GemmStaged> {
        // SAFETY: `GemmReady` is linear compiler-issued authority for exactly
        // this lane and epoch. The private intrinsic accepts no addresses.
        unsafe { stage_gfx942_tiled_gemm_wave64_v1(self, a_bits, b_bits) }
    }

    /// Stores this lane's disjoint four-value C fragment with alpha/beta.
    ///
    /// This operation is admitted only after all private K epochs have been
    /// consumed. Component `c` of lane `l` owns output
    /// `(tile_row * 16 + 4 * (l / 16) + c, tile_column * 16 + l % 16)`.
    /// Out-of-domain rows and columns perform no access. Valid coordinates use
    /// checked `row * ldc + column` arithmetic and must be in `c`; a mismatch
    /// traps instead of accessing memory. No arbitrary index enters this API.
    /// Each valid output is assigned `alpha * accumulator + beta * C`.
    #[inline(always)]
    pub fn store_c_fragment(
        self,
        c: &mut DisjointSlice<f32>,
        m: u32,
        n: u32,
        ldc: u32,
        alpha: f32,
        beta: f32,
    ) {
        // SAFETY: the compiler-issued ready token carries the authenticated
        // lane/workgroup partition and private accumulator. The intrinsic must
        // reject an incomplete epoch before deriving any C address.
        unsafe { store_gfx942_tiled_gemm_wave64_v1(self, c, m, n, ldc, alpha, beta) }
    }

    #[cfg(test)]
    fn for_model(lane: u32, tile_row: u32, tile_column: u32, k: u32) -> Option<Self> {
        if lane >= GENERAL_TILED_GEMM_WAVE_LANES_V1 {
            return None;
        }
        Some(Self {
            lane,
            tile_row,
            tile_column,
            epoch: 0,
            phases: phase_count(k),
            accumulator: [0.0; 4],
            _state: PhantomData,
            _not_send_sync: PhantomData,
        })
    }
}

impl Gfx942TiledGemmWave64V1<GemmStaged> {
    /// Executes the convergent LDS publish barrier for the current epoch.
    #[inline(always)]
    pub fn publish(self) -> Gfx942TiledGemmWave64V1<GemmPublished> {
        // SAFETY: the only safe producer of `GemmStaged` performed this lane's
        // complete disjoint writes. Compiler verification establishes that all
        // 64 lanes execute the transition in uniform dynamic order.
        unsafe { publish_gfx942_tiled_gemm_wave64_v1(self) }
    }
}

impl Gfx942TiledGemmWave64V1<GemmPublished> {
    /// Executes one BF16-to-FP32 16x16x16 MFMA and carries the accumulator.
    #[inline(always)]
    pub fn multiply_accumulate(self) -> Gfx942TiledGemmWave64V1<GemmConsumed> {
        // SAFETY: `GemmPublished` proves the publish transition precedes this
        // operation. Compiler verification supplies full-wave convergence and
        // the exact gfx942 MFMA numerical profile.
        unsafe { mfma_gfx942_tiled_gemm_wave64_v1(self) }
    }
}

impl Gfx942TiledGemmWave64V1<GemmConsumed> {
    /// Executes the convergent LDS reuse barrier and advances the private epoch.
    #[inline(always)]
    pub fn reuse(self) -> Gfx942TiledGemmWave64V1<GemmReady> {
        // SAFETY: the only safe producer of `GemmConsumed` has completed this
        // lane's MFMA read. Compiler verification establishes full-wave uniform
        // ordering before the next epoch can overwrite either LDS tile.
        unsafe { reuse_gfx942_tiled_gemm_wave64_v1(self) }
    }
}

const fn phase_count(k: u32) -> u32 {
    k / GENERAL_TILED_GEMM_TILE_K_V1
        + if k.is_multiple_of(GENERAL_TILED_GEMM_TILE_K_V1) {
            0
        } else {
            1
        }
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_wave64_acquire_v1"]
unsafe fn acquire_gfx942_tiled_gemm_wave64_v1(k: u32) -> Gfx942TiledGemmWave64V1<GemmReady> {
    let _ = phase_count(k);
    unreachable!("general tiled-GEMM authority requires authenticated compiler lowering")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_wave64_stage_v1"]
unsafe fn stage_gfx942_tiled_gemm_wave64_v1(
    wave: Gfx942TiledGemmWave64V1<GemmReady>,
    a_bits: [u16; 4],
    b_bits: [u16; 4],
) -> Gfx942TiledGemmWave64V1<GemmStaged> {
    let _ = (wave, a_bits, b_bits);
    unreachable!("general tiled-GEMM staging requires authenticated compiler lowering")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_wave64_publish_v1"]
unsafe fn publish_gfx942_tiled_gemm_wave64_v1(
    wave: Gfx942TiledGemmWave64V1<GemmStaged>,
) -> Gfx942TiledGemmWave64V1<GemmPublished> {
    let _ = wave;
    unreachable!("general tiled-GEMM publish requires authenticated compiler lowering")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_wave64_mfma_v1"]
unsafe fn mfma_gfx942_tiled_gemm_wave64_v1(
    wave: Gfx942TiledGemmWave64V1<GemmPublished>,
) -> Gfx942TiledGemmWave64V1<GemmConsumed> {
    let _ = wave;
    unreachable!("general tiled-GEMM MFMA requires authenticated compiler lowering")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_wave64_reuse_v1"]
unsafe fn reuse_gfx942_tiled_gemm_wave64_v1(
    wave: Gfx942TiledGemmWave64V1<GemmConsumed>,
) -> Gfx942TiledGemmWave64V1<GemmReady> {
    let _ = wave;
    unreachable!("general tiled-GEMM reuse requires authenticated compiler lowering")
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_wave64_store_v1"]
unsafe fn store_gfx942_tiled_gemm_wave64_v1(
    wave: Gfx942TiledGemmWave64V1<GemmReady>,
    c: &mut DisjointSlice<f32>,
    m: u32,
    n: u32,
    ldc: u32,
    alpha: f32,
    beta: f32,
) {
    let _ = (wave, c, m, n, ldc, alpha, beta);
    unreachable!("general tiled-GEMM stores require authenticated compiler lowering")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn profile_constants_and_phase_ceiling_are_exact() {
        assert_eq!(GENERAL_TILED_GEMM_DEVICE_CONTRACT_VERSION_V1, 1);
        assert_eq!(GENERAL_TILED_GEMM_LDS_BYTES_V1, 1024);
        assert_eq!(phase_count(0), 0);
        assert_eq!(phase_count(1), 1);
        assert_eq!(phase_count(16), 1);
        assert_eq!(phase_count(17), 2);
        assert_eq!(phase_count(u32::MAX), 1 << 28);
    }

    #[test]
    fn model_identity_keeps_lane_grid_and_epoch_private() {
        let wave = Gfx942TiledGemmWave64V1::for_model(63, 7, 11, 33).unwrap();
        assert_eq!(wave.lane(), 63);
        assert_eq!(wave.tile_row(), 7);
        assert_eq!(wave.tile_column(), 11);
        assert_eq!(wave.phase(), 0);
        assert!(wave.has_remaining_phases());
        assert_eq!(wave.accumulator.map(f32::to_bits), [0; 4]);
        assert!(Gfx942TiledGemmWave64V1::for_model(64, 0, 0, 0).is_none());
    }

    #[test]
    fn host_acquisition_fails_closed() {
        let failure = catch_unwind(AssertUnwindSafe(|| {
            let _ = Gfx942TiledGemmWave64V1::from_compiler(16);
        }));
        assert!(failure.is_err());
    }
}
