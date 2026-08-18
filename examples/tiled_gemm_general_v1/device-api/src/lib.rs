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
//!
//! [`ProofSensitiveGeneralGemmWave64V1`] is a separate production-candidate
//! frontend surface. Its safe calls name proof obligations without using
//! typestate to reject invalid order locally. That lets attributed safe-Rust
//! negative sources reach compiler semantic analysis. The context remains
//! sealed and every unsupported or host call still fails closed.

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

/// One issue #138 semantic mutation category in canonical issue order.
///
/// This vocabulary mirrors the ordinary-Rust semantic corpus without making
/// that corpus a dependency of this isolated device-surface crate.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GemmSemanticCategoryV1 {
    /// A global A load omits its M/K tail predicate.
    UnguardedATailLoad,
    /// A global B load omits its K/N tail predicate.
    UnguardedBTailLoad,
    /// A C store selects a coordinate outside the M/N domain.
    UnguardedCTailStore,
    /// Two lanes select the same C coordinate.
    DuplicateLaneCWrite,
    /// Two workgroups select overlapping C tiles.
    OverlappingWorkgroupCTile,
    /// Two lanes select the same LDS staging slot.
    DuplicateLdsWrite,
    /// An LDS value is read before complete initialization.
    LdsReadBeforeInitialization,
    /// MFMA consumes LDS without the publish transition.
    MissingPublishBarrier,
    /// A barrier is reached through lane-varying control flow.
    DivergentBarrier,
    /// A later phase overwrites LDS without the reuse transition.
    MissingReuseBarrier,
    /// Source attempts to consume an already expired phase capability.
    ExpiredLdsEpoch,
    /// Source reads an asynchronous stage before its admitted wait.
    StagedReadBeforeWait,
    /// Source resets the carried accumulator between phases.
    AccumulatorReset,
    /// An out-of-domain K-tail component is not positive BF16 zero.
    IncorrectKTailZeroFill,
    /// The output does not implement `alpha * AB + beta * C`.
    IncorrectAlphaBetaEpilogue,
}

impl GemmSemanticCategoryV1 {
    /// Returns the stable semantic-corpus mutation ID.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnguardedATailLoad => "unguarded_a_tail_load",
            Self::UnguardedBTailLoad => "unguarded_b_tail_load",
            Self::UnguardedCTailStore => "unguarded_c_tail_store",
            Self::DuplicateLaneCWrite => "duplicate_lane_c_write",
            Self::OverlappingWorkgroupCTile => "overlapping_workgroup_c_tile",
            Self::DuplicateLdsWrite => "duplicate_lds_write",
            Self::LdsReadBeforeInitialization => "lds_read_before_initialization",
            Self::MissingPublishBarrier => "missing_publish_barrier",
            Self::DivergentBarrier => "divergent_barrier",
            Self::MissingReuseBarrier => "missing_reuse_barrier",
            Self::ExpiredLdsEpoch => "expired_lds_epoch",
            Self::StagedReadBeforeWait => "staged_read_before_wait",
            Self::AccumulatorReset => "accumulator_reset",
            Self::IncorrectKTailZeroFill => "incorrect_k_tail_zero_fill",
            Self::IncorrectAlphaBetaEpilogue => "incorrect_alpha_beta_epilogue",
        }
    }
}

/// Strongest honest source-enforcement owner for one semantic category.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GemmSourceEnforcementV1 {
    /// Rust move/typestate rules reject the invalid local lifecycle directly.
    RustTypestate,
    /// The sealed surface rejects direct address/state selection, but dynamic
    /// or cross-invocation correctness still requires fe2o3 verification.
    SealedSurfaceAndVerifier,
    /// Well-typed safe Rust can express the mutation; MIR/Pliron must reject it.
    SemanticVerifier,
}

impl GemmSourceEnforcementV1 {
    /// Returns the stable enforcement-owner spelling.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RustTypestate => "rust_typestate",
            Self::SealedSurfaceAndVerifier => "sealed_surface_and_verifier",
            Self::SemanticVerifier => "semantic_verifier",
        }
    }
}

/// Enforcement boundary for one issue #138 semantic category.
///
/// `rust_ui_fixture` names a compile-fail attempt against this sealed API. It
/// is deliberately absent when safe Rust can express the real mutation. A UI
/// failure establishes only the stated local surface restriction; it is not a
/// substitute for the well-typed semantic fixture or its proof obligation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GemmSemanticEnforcementV1 {
    category: GemmSemanticCategoryV1,
    owner: GemmSourceEnforcementV1,
    rust_ui_fixture: Option<&'static str>,
    verifier_requirement: &'static str,
}

impl GemmSemanticEnforcementV1 {
    const fn new(
        category: GemmSemanticCategoryV1,
        owner: GemmSourceEnforcementV1,
        rust_ui_fixture: Option<&'static str>,
        verifier_requirement: &'static str,
    ) -> Self {
        Self {
            category,
            owner,
            rust_ui_fixture,
            verifier_requirement,
        }
    }

    /// Returns the semantic mutation category.
    pub const fn category(self) -> GemmSemanticCategoryV1 {
        self.category
    }

    /// Returns the strongest honest source-enforcement owner.
    pub const fn owner(self) -> GemmSourceEnforcementV1 {
        self.owner
    }

    /// Returns the standalone-crate-relative compile-fail fixture, when meaningful.
    pub const fn rust_ui_fixture(self) -> Option<&'static str> {
        self.rust_ui_fixture
    }

    /// Returns the remaining semantic-verifier responsibility, or an empty
    /// string when local Rust typestate fully owns the source misuse.
    pub const fn verifier_requirement(self) -> &'static str {
        self.verifier_requirement
    }
}

const fn enforcement(
    category: GemmSemanticCategoryV1,
    owner: GemmSourceEnforcementV1,
    rust_ui_fixture: Option<&'static str>,
    verifier_requirement: &'static str,
) -> GemmSemanticEnforcementV1 {
    GemmSemanticEnforcementV1::new(category, owner, rust_ui_fixture, verifier_requirement)
}

/// Complete issue #138 source-enforcement boundary in canonical issue order.
///
/// The five `SemanticVerifier` entries intentionally have no trybuild fixture:
/// their ordinary-Rust mutations must continue to typecheck and reach the
/// proof-required compiler. Seven hybrid entries have API escape tests but
/// retain the stated dynamic verifier obligation. Only three local lifecycle
/// errors are fully owned by Rust typestate.
pub const GENERAL_GEMM_SEMANTIC_ENFORCEMENT_V1: [GemmSemanticEnforcementV1; 15] = [
    enforcement(
        GemmSemanticCategoryV1::UnguardedATailLoad,
        GemmSourceEnforcementV1::SemanticVerifier,
        None,
        "prove the dynamic A region and M/K tail predicate",
    ),
    enforcement(
        GemmSemanticCategoryV1::UnguardedBTailLoad,
        GemmSourceEnforcementV1::SemanticVerifier,
        None,
        "prove the dynamic B region and K/N tail predicate",
    ),
    enforcement(
        GemmSemanticCategoryV1::UnguardedCTailStore,
        GemmSourceEnforcementV1::SealedSurfaceAndVerifier,
        Some("tests/ui/fail/semantic_unguarded_c_tail_store.rs"),
        "prove dynamic C bounds under compiler-issued lane and tile identities",
    ),
    enforcement(
        GemmSemanticCategoryV1::DuplicateLaneCWrite,
        GemmSourceEnforcementV1::SealedSurfaceAndVerifier,
        Some("tests/ui/fail/semantic_duplicate_lane_c_write.rs"),
        "prove lane/component output injectivity",
    ),
    enforcement(
        GemmSemanticCategoryV1::OverlappingWorkgroupCTile,
        GemmSourceEnforcementV1::SealedSurfaceAndVerifier,
        Some("tests/ui/fail/semantic_overlapping_workgroup_c_tile.rs"),
        "prove workgroup output-tile disjointness",
    ),
    enforcement(
        GemmSemanticCategoryV1::DuplicateLdsWrite,
        GemmSourceEnforcementV1::SealedSurfaceAndVerifier,
        Some("tests/ui/fail/semantic_duplicate_lds_write.rs"),
        "prove the compiler-lowered XOR4 lane-to-slot bijection",
    ),
    enforcement(
        GemmSemanticCategoryV1::LdsReadBeforeInitialization,
        GemmSourceEnforcementV1::SealedSurfaceAndVerifier,
        Some("tests/ui/fail/semantic_lds_read_before_initialization.rs"),
        "prove every lane completes its disjoint stage before publication",
    ),
    enforcement(
        GemmSemanticCategoryV1::MissingPublishBarrier,
        GemmSourceEnforcementV1::RustTypestate,
        Some("tests/ui/fail/semantic_missing_publish_barrier.rs"),
        "",
    ),
    enforcement(
        GemmSemanticCategoryV1::DivergentBarrier,
        GemmSourceEnforcementV1::SemanticVerifier,
        None,
        "prove barrier convergence over the reachable MIR control-flow graph",
    ),
    enforcement(
        GemmSemanticCategoryV1::MissingReuseBarrier,
        GemmSourceEnforcementV1::RustTypestate,
        Some("tests/ui/fail/semantic_missing_reuse_barrier.rs"),
        "",
    ),
    enforcement(
        GemmSemanticCategoryV1::ExpiredLdsEpoch,
        GemmSourceEnforcementV1::RustTypestate,
        Some("tests/ui/fail/semantic_expired_lds_epoch.rs"),
        "",
    ),
    enforcement(
        GemmSemanticCategoryV1::StagedReadBeforeWait,
        GemmSourceEnforcementV1::SealedSurfaceAndVerifier,
        Some("tests/ui/fail/semantic_staged_read_before_wait.rs"),
        "the conservative profile has no async stage; a future profile must prove wait epochs",
    ),
    enforcement(
        GemmSemanticCategoryV1::AccumulatorReset,
        GemmSourceEnforcementV1::SealedSurfaceAndVerifier,
        Some("tests/ui/fail/semantic_accumulator_reset.rs"),
        "prove unique compiler issuance and accumulator carry across all dynamic phases",
    ),
    enforcement(
        GemmSemanticCategoryV1::IncorrectKTailZeroFill,
        GemmSourceEnforcementV1::SemanticVerifier,
        None,
        "prove each out-of-domain staged component is positive BF16 zero",
    ),
    enforcement(
        GemmSemanticCategoryV1::IncorrectAlphaBetaEpilogue,
        GemmSourceEnforcementV1::SemanticVerifier,
        None,
        "prove the runtime alpha/beta argument binding and exact epilogue expression",
    ),
];

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
    #[inline(always)]
    pub const fn lane(&self) -> u32 {
        self.lane
    }

    /// Returns the output tile's row coordinate in the launch grid.
    #[inline(always)]
    pub const fn tile_row(&self) -> u32 {
        self.tile_row
    }

    /// Returns the output tile's column coordinate in the launch grid.
    #[inline(always)]
    pub const fn tile_column(&self) -> u32 {
        self.tile_column
    }

    /// Returns the next K-phase epoch, starting at zero.
    #[inline(always)]
    pub const fn phase(&self) -> u32 {
        self.epoch
    }

    /// Reports whether another complete or zero-filled K phase is required.
    #[inline(always)]
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

/// Sealed safe source context whose calls create general-GEMM proof obligations.
///
/// Unlike [`Gfx942TiledGemmWave64V1`], this surface deliberately does not use
/// Rust typestate to enforce phase order. Safe attributed source can therefore
/// express missing barriers or duplicate stores, and the compiler must derive
/// and reject those schedules before artifact creation. Private fields prevent
/// source from forging the context or selecting lane, workgroup, LDS, or
/// accumulator state. The value is neither `Copy`, `Clone`, `Send`, nor `Sync`.
///
/// This is a production-candidate frontend contract, not execution authority.
/// Its diagnostic-item terminals are panic stubs until authenticated MIR
/// import, runtime plan binding, proof discharge, lowering, and publication
/// are joined.
#[must_use = "the proof-sensitive GEMM context must reach semantic compiler analysis"]
pub struct ProofSensitiveGeneralGemmWave64V1 {
    _sealed: (),
    _not_send_sync: PhantomData<*mut ()>,
}

impl ProofSensitiveGeneralGemmWave64V1 {
    /// Requests one compiler-issued proof-sensitive wave64 context.
    #[inline(always)]
    pub fn from_compiler(k: u32) -> Self {
        proof_acquire_gfx942_tiled_gemm_wave64_v1(k)
    }

    /// Names complete guarded, zero-filled A/B staging obligations.
    #[inline(always)]
    pub fn stage(&mut self, a_bits: [u16; 4], b_bits: [u16; 4]) {
        proof_stage_gfx942_tiled_gemm_wave64_v1(self, a_bits, b_bits)
    }

    /// Names a convergent publish-barrier obligation for the current phase.
    #[inline(always)]
    pub fn publish(&mut self) {
        proof_publish_gfx942_tiled_gemm_wave64_v1(self)
    }

    /// Names current-epoch LDS reads and one carried MFMA update obligation.
    #[inline(always)]
    pub fn multiply_accumulate(&mut self) {
        proof_mfma_gfx942_tiled_gemm_wave64_v1(self)
    }

    /// Names a convergent LDS reuse-barrier obligation for the current phase.
    #[inline(always)]
    pub fn reuse(&mut self) {
        proof_reuse_gfx942_tiled_gemm_wave64_v1(self)
    }

    /// Names guarded, disjoint `alpha * AB + beta * C` store obligations.
    #[inline(always)]
    pub fn store_c_fragment(
        &mut self,
        c: &mut DisjointSlice<f32>,
        m: u32,
        n: u32,
        ldc: u32,
        alpha: f32,
        beta: f32,
    ) {
        proof_store_gfx942_tiled_gemm_wave64_v1(self, c, m, n, ldc, alpha, beta)
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

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_acquire_v1"]
fn proof_acquire_gfx942_tiled_gemm_wave64_v1(k: u32) -> ProofSensitiveGeneralGemmWave64V1 {
    let _ = phase_count(k);
    unreachable!("proof-sensitive GEMM authority requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_stage_v1"]
fn proof_stage_gfx942_tiled_gemm_wave64_v1(
    context: &mut ProofSensitiveGeneralGemmWave64V1,
    a_bits: [u16; 4],
    b_bits: [u16; 4],
) {
    let _ = (context, a_bits, b_bits);
    unreachable!("proof-sensitive GEMM staging requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_publish_v1"]
fn proof_publish_gfx942_tiled_gemm_wave64_v1(context: &mut ProofSensitiveGeneralGemmWave64V1) {
    let _ = context;
    unreachable!("proof-sensitive GEMM publish requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_mfma_v1"]
fn proof_mfma_gfx942_tiled_gemm_wave64_v1(context: &mut ProofSensitiveGeneralGemmWave64V1) {
    let _ = context;
    unreachable!("proof-sensitive GEMM MFMA requires authenticated compiler analysis")
}

#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_reuse_v1"]
fn proof_reuse_gfx942_tiled_gemm_wave64_v1(context: &mut ProofSensitiveGeneralGemmWave64V1) {
    let _ = context;
    unreachable!("proof-sensitive GEMM reuse requires authenticated compiler analysis")
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
#[rustc_diagnostic_item = "fe2o3_device_general_tiled_gemm_proof_store_v1"]
fn proof_store_gfx942_tiled_gemm_wave64_v1(
    context: &mut ProofSensitiveGeneralGemmWave64V1,
    c: &mut DisjointSlice<f32>,
    m: u32,
    n: u32,
    ldc: u32,
    alpha: f32,
    beta: f32,
) {
    let _ = (context, c, m, n, ldc, alpha, beta);
    unreachable!("proof-sensitive GEMM stores require authenticated compiler analysis")
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

    #[test]
    fn proof_sensitive_host_acquisition_fails_closed() {
        let failure = catch_unwind(AssertUnwindSafe(|| {
            let _ = ProofSensitiveGeneralGemmWave64V1::from_compiler(16);
        }));
        assert!(failure.is_err());
    }
}
