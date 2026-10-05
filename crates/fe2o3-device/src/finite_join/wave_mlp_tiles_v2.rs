//! Engineering-only row-tiled Norm -> {Gate, Up} -> SwiGLU -> Down worker.
//!
//! This separate eleven-root profile does not widen any QKV profile. Five
//! stages contain 258 distinct claims. Gate, Up and Down own 64 rows per tile;
//! each row retains one Wave64's arithmetic order. This is not a progress or
//! performance guarantee. TP reduction and residual addition remain external.

use super::{Claim, DUPLICATE, FiniteJoinWorkerResult, INVALID, MISSING_PREDECESSOR, STALE_EPOCH};
use crate::{WorkgroupLdsScope, WorkgroupPipeline, thread};
use core::{
    marker::PhantomData,
    mem::align_of,
    sync::atomic::{AtomicU32, Ordering},
};

pub const WAVE_LANES: usize = 64;
pub const NORM_ELEMENTS: usize = 4096;
pub const INTERMEDIATE_ELEMENTS: usize = 6144;
pub const WEIGHT_ELEMENTS: usize = NORM_ELEMENTS * INTERMEDIATE_ELEMENTS;
pub const OUTPUT_ELEMENTS: usize = 4096;
pub const WORKGROUPS: u32 = 64;
pub const TILE_ROWS: usize = 64;
pub const STAGE_COUNTS: [u32; 5] = [1, 96, 96, 1, 64];
pub const STAGE_STARTS: [u32; 5] = [0, 1, 97, 193, 194];
pub const TASK_COUNT: u32 = 258;
pub const WAVE_STATE_WORDS: usize = 548;
pub const INCOMPLETE_WRITES: u32 = 16;
pub const MAX_ROUNDS: u32 = 512;
const POLLS_PER_ROUND: u32 = 256;
const ALL_STAGES: u32 = 0x1f;
const EPOCH: usize = 0;
const ERRORS: usize = 1;
const READY: usize = 2;
const DONE: usize = 3;
const NEXT: usize = 4;
const COMPLETED: usize = 9;
const CLAIMED: usize = 14;
const DONE_TILES: usize = 23;
const OWNERS: usize = 32;
const ARRIVALS: usize = 290;
const STOP: u32 = TASK_COUNT + 1;

/// Initialization bytes only, not permission to reset a live dispatch.
pub const fn initial_state_words() -> [u32; WAVE_STATE_WORDS] {
    let mut words = [0; WAVE_STATE_WORDS];
    words[EPOCH] = 1;
    words[READY] = 1;
    words
}

/// Checks an externally acquired, quiescent snapshot; confers no authority.
pub fn terminal_snapshot(words: &[u32; WAVE_STATE_WORDS]) -> bool {
    if words[EPOCH] != 1 || words[ERRORS] != 0 || words[READY] != 0 || words[DONE] != ALL_STAGES {
        return false;
    }
    for stage in 0..5 {
        if words[NEXT + stage] != STAGE_COUNTS[stage]
            || words[COMPLETED + stage] != STAGE_COUNTS[stage]
        {
            return false;
        }
    }
    for word in 0..9 {
        let expected = if word == 8 { 3 } else { u32::MAX };
        if words[CLAIMED + word] != expected || words[DONE_TILES + word] != expected {
            return false;
        }
    }
    for tile in 0..TASK_COUNT as usize {
        if words[OWNERS + tile] == 0
            || words[OWNERS + tile] > WORKGROUPS
            || words[ARRIVALS + tile] != WAVE_LANES as u32
        {
            return false;
        }
    }
    true
}

/// Exact one-shot storage, not a compiler-issued capability or launch authority.
#[must_use]
#[repr(C)]
pub struct WaveMlpTileStorageV2<'dispatch> {
    input: *const [u16; NORM_ELEMENTS],
    norm_weight: *const [u16; NORM_ELEMENTS],
    gate_weight: *const [u16; WEIGHT_ELEMENTS],
    up_weight: *const [u16; WEIGHT_ELEMENTS],
    down_weight: *const [u16; WEIGHT_ELEMENTS],
    normalized: *mut [u16; NORM_ELEMENTS],
    gate: *mut [u16; INTERMEDIATE_ELEMENTS],
    up: *mut [u16; INTERMEDIATE_ELEMENTS],
    activation: *mut [u16; INTERMEDIATE_ELEMENTS],
    down_partial: *mut [f32; OUTPUT_ELEMENTS],
    state: *const [AtomicU32; WAVE_STATE_WORDS],
    _lifetime: PhantomData<(
        &'dispatch [u16],
        &'dispatch mut [u16],
        &'dispatch mut [f32],
        &'dispatch [AtomicU32],
    )>,
    _not_send_sync: PhantomData<*mut ()>,
}

fn disjoint_regions(regions: [(usize, usize, usize); 11]) -> bool {
    for (index, &(base, bytes, alignment)) in regions.iter().enumerate() {
        let Some(end) = base.checked_add(bytes) else {
            return false;
        };
        if base == 0 || bytes == 0 || base % alignment != 0 {
            return false;
        }
        for &(other, other_bytes, _) in &regions[..index] {
            let Some(other_end) = other.checked_add(other_bytes) else {
                return false;
            };
            if base < other_end && other < end {
                return false;
            }
        }
    }
    true
}

impl<'dispatch> WaveMlpTileStorageV2<'dispatch> {
    /// Binds engineering storage without issuing production/coherence authority.
    ///
    /// # Safety
    /// All eleven exact array roots must occupy pairwise disjoint, aligned,
    /// live allocations throughout this one-shot dispatch. The five read roots
    /// are initialized and immutable; the five outputs are writable only by
    /// this worker. Gate/up weights are row-major [6144,4096]; down weights are
    /// [4096,6144]. Input is the actual first residual, not its normalization.
    /// Initialize genuine AtomicU32[548] state from `initial_state_words()`
    /// before any invocation. No reset/reuse is permitted
    /// until every invocation quiesces, including rejected/retired execution.
    /// Exactly 64 WG64 groups participate, one invocation per physical lane.
    /// System atomics and payloads require one coherent publication domain;
    /// external access during execution is forbidden. Every lane reaches each
    /// issued round's finish and the callback's same numerical collectives.
    /// Bounded polling does not guarantee task completion: final acquired state
    /// and all numerical/readback checks remain mandatory outside this API.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn from_raw_parts(
        input: *const [u16; NORM_ELEMENTS],
        norm_weight: *const [u16; NORM_ELEMENTS],
        gate_weight: *const [u16; WEIGHT_ELEMENTS],
        up_weight: *const [u16; WEIGHT_ELEMENTS],
        down_weight: *const [u16; WEIGHT_ELEMENTS],
        normalized: *mut [u16; NORM_ELEMENTS],
        gate: *mut [u16; INTERMEDIATE_ELEMENTS],
        up: *mut [u16; INTERMEDIATE_ELEMENTS],
        activation: *mut [u16; INTERMEDIATE_ELEMENTS],
        down_partial: *mut [f32; OUTPUT_ELEMENTS],
        state: *const [AtomicU32; WAVE_STATE_WORDS],
    ) -> Result<Self, u32> {
        if !disjoint_regions([
            (input as usize, NORM_ELEMENTS * 2, align_of::<u16>()),
            (norm_weight as usize, NORM_ELEMENTS * 2, align_of::<u16>()),
            (gate_weight as usize, WEIGHT_ELEMENTS * 2, align_of::<u16>()),
            (up_weight as usize, WEIGHT_ELEMENTS * 2, align_of::<u16>()),
            (down_weight as usize, WEIGHT_ELEMENTS * 2, align_of::<u16>()),
            (normalized as usize, NORM_ELEMENTS * 2, align_of::<u16>()),
            (gate as usize, INTERMEDIATE_ELEMENTS * 2, align_of::<u16>()),
            (up as usize, INTERMEDIATE_ELEMENTS * 2, align_of::<u16>()),
            (
                activation as usize,
                INTERMEDIATE_ELEMENTS * 2,
                align_of::<u16>(),
            ),
            (
                down_partial as usize,
                OUTPUT_ELEMENTS * 4,
                align_of::<f32>(),
            ),
            (
                state as usize,
                WAVE_STATE_WORDS * 4,
                align_of::<AtomicU32>(),
            ),
        ]) {
            return Err(INVALID);
        }
        Ok(Self {
            input,
            norm_weight,
            gate_weight,
            up_weight,
            down_weight,
            normalized,
            gate,
            up,
            activation,
            down_partial,
            state,
            _lifetime: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    fn state(&self) -> &[AtomicU32; WAVE_STATE_WORDS] {
        // SAFETY: construction retains genuine initialized atomic storage.
        unsafe { &*self.state }
    }
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn predecessor_mask(stage: usize) -> u32 {
    if stage == 0 {
        0
    } else if stage == 1 || stage == 2 {
        1
    } else if stage == 3 {
        7
    } else {
        15
    }
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn tile_stage(tile: u32) -> usize {
    if tile == 0 {
        0
    } else if tile < 97 {
        1
    } else if tile < 193 {
        2
    } else if tile == 193 {
        3
    } else {
        4
    }
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn reject(state: &[AtomicU32; WAVE_STATE_WORDS], error: u32) -> Claim {
    state[ERRORS].fetch_or(error, Ordering::Relaxed);
    Claim::Rejected(error)
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn claim_fanout(state: &[AtomicU32; WAVE_STATE_WORDS], worker: u32) -> Claim {
    if worker >= WORKGROUPS {
        return reject(state, INVALID);
    }
    if state[EPOCH].load(Ordering::Acquire) != 1 {
        return reject(state, STALE_EPOCH);
    }
    let errors = state[ERRORS].load(Ordering::Acquire);
    if errors != 0 {
        return Claim::Rejected(errors);
    }
    let snapshot = state[READY].load(Ordering::Acquire);
    if snapshot & !ALL_STAGES != 0 {
        return reject(state, INVALID);
    }
    if snapshot == 0 {
        return Claim::Empty;
    }
    // Acyclic selector: no leader-only scan loop may surround a later barrier.
    let stage = if snapshot & 1 != 0 {
        0
    } else if snapshot & 2 != 0 {
        1
    } else if snapshot & 4 != 0 {
        2
    } else if snapshot & 8 != 0 {
        3
    } else {
        4
    };
    let cursor = state[NEXT + stage].load(Ordering::Acquire);
    let count = STAGE_COUNTS[stage];
    if cursor > count {
        return reject(state, INVALID);
    }
    // One bounded CAS attempt. A delayed final claimant may not have cleared
    // READY yet; exhausted contenders must not overshoot the exact cursor.
    if cursor == count {
        return Claim::Contended;
    }
    if state[NEXT + stage]
        .compare_exchange(cursor, cursor + 1, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Claim::Contended;
    }
    if cursor + 1 == count {
        state[READY].fetch_and(!(1 << stage), Ordering::AcqRel);
    }
    let tile = STAGE_STARTS[stage] + cursor;
    let word = tile as usize / 32;
    let bit = 1 << (tile % 32);
    if state[CLAIMED + word].fetch_or(bit, Ordering::AcqRel) & bit != 0 {
        return reject(state, DUPLICATE);
    }
    let predecessors = predecessor_mask(stage);
    if state[DONE].load(Ordering::Acquire) & predecessors != predecessors {
        return reject(state, MISSING_PREDECESSOR);
    }
    if state[OWNERS + tile as usize]
        .compare_exchange(0, worker + 1, Ordering::Release, Ordering::Relaxed)
        .is_err()
    {
        return reject(state, DUPLICATE);
    }
    Claim::Task(tile)
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn lane_admitted(state: &[AtomicU32; WAVE_STATE_WORDS], token: u32, worker: u32) -> bool {
    if token == 0 || token == STOP {
        return true;
    }
    if token > TASK_COUNT || worker >= WORKGROUPS {
        return false;
    }
    let tile = token - 1;
    let predecessors = predecessor_mask(tile_stage(tile));
    state[EPOCH].load(Ordering::Acquire) == 1
        && state[ERRORS].load(Ordering::Acquire) == 0
        && state[CLAIMED + tile as usize / 32].load(Ordering::Acquire) & (1 << (tile % 32)) != 0
        && state[OWNERS + tile as usize].load(Ordering::Acquire) == worker + 1
        && state[DONE].load(Ordering::Acquire) & predecessors == predecessors
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn complete_lane(state: &[AtomicU32; WAVE_STATE_WORDS], tile: u32) -> Result<(), u32> {
    if tile >= TASK_COUNT {
        state[ERRORS].fetch_or(INVALID, Ordering::Relaxed);
        return Err(INVALID);
    }
    let word = tile as usize / 32;
    let bit = 1 << (tile % 32);
    let stage = tile_stage(tile);
    if state[EPOCH].load(Ordering::Acquire) != 1
        || state[CLAIMED + word].load(Ordering::Acquire) & bit == 0
        || state[DONE].load(Ordering::Acquire) & predecessor_mask(stage) != predecessor_mask(stage)
    {
        state[ERRORS].fetch_or(MISSING_PREDECESSOR, Ordering::Relaxed);
        return Err(MISSING_PREDECESSOR);
    }
    let old_arrivals = state[ARRIVALS + tile as usize].fetch_add(1, Ordering::AcqRel);
    if old_arrivals >= WAVE_LANES as u32 {
        state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
        return Err(DUPLICATE);
    }
    // The final RMW acquires all earlier lanes' writes before successor release.
    if old_arrivals == WAVE_LANES as u32 - 1 {
        if state[DONE_TILES + word].fetch_or(bit, Ordering::AcqRel) & bit != 0 {
            state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
            return Err(DUPLICATE);
        }
        // This second RMW chain acquires every completed tile's lane fan-in.
        let completed = state[COMPLETED + stage].fetch_add(1, Ordering::AcqRel);
        if completed >= STAGE_COUNTS[stage] {
            state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
            return Err(DUPLICATE);
        }
        if completed + 1 != STAGE_COUNTS[stage] {
            return Ok(());
        }
        let stage_bit = 1 << stage;
        let old_done = state[DONE].fetch_or(stage_bit, Ordering::AcqRel);
        if old_done & stage_bit != 0 {
            state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
            return Err(DUPLICATE);
        }
        if stage == 0 {
            state[READY].fetch_or(6, Ordering::Release);
        } else if (stage == 1 || stage == 2) && (old_done | stage_bit) & 7 == 7 {
            state[READY].fetch_or(8, Ordering::Release);
        } else if stage == 3 {
            state[READY].fetch_or(16, Ordering::Release);
        }
    }
    Ok(())
}

fn complete_coverage(token: u32, lane: usize, written: usize, valid: bool) -> bool {
    if !valid || token > STOP || lane >= WAVE_LANES {
        return false;
    }
    let expected = if token == 1 {
        64
    } else if token == 194 {
        96
    } else if token >= 2 && token <= TASK_COUNT && lane == 0 {
        TILE_ROWS
    } else {
        0
    };
    written == expected
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn launch_dimensions_valid() -> bool {
    thread::block_dim_x() == 64
        && thread::block_dim_y() == 1
        && thread::block_dim_z() == 1
        && thread::grid_dim_x() == WORKGROUPS
        && thread::grid_dim_y() == 1
        && thread::grid_dim_z() == 1
        && thread::launch_extent_1d() == WORKGROUPS as usize * WAVE_LANES
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn begin_round_claim(
    state: &[AtomicU32; WAVE_STATE_WORDS],
    lane: usize,
    worker: u32,
    retired: &mut bool,
    result: &mut FiniteJoinWorkerResult,
) -> u32 {
    result.rounds += 1;
    let mut token = 0;
    let mut polls = 0;
    while polls < POLLS_PER_ROUND {
        if lane == 0 && !*retired && token == 0 {
            let errors = state[ERRORS].load(Ordering::Acquire);
            if errors != 0 {
                result.error |= errors;
                *retired = true;
            } else if state[DONE].load(Ordering::Acquire) == ALL_STAGES {
                *retired = true;
            } else {
                match claim_fanout(state, worker) {
                    Claim::Task(task) => token = task + 1,
                    Claim::Empty => {
                        result.empty_probes += 1;
                    }
                    Claim::Contended => {}
                    Claim::Rejected(error) => {
                        result.error |= error;
                        *retired = true;
                    }
                }
            }
        }
        polls += 1;
    }
    if lane == 0 && *retired { STOP } else { token }
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn finish_round_completion(
    state: &[AtomicU32; WAVE_STATE_WORDS],
    token: u32,
    all_valid: bool,
    retired: &mut bool,
    result: &mut FiniteJoinWorkerResult,
) {
    if !all_valid {
        state[ERRORS].fetch_or(INCOMPLETE_WRITES, Ordering::Relaxed);
        result.error |= INCOMPLETE_WRITES;
        *retired = true;
    } else if token != 0 && token != STOP {
        if let Err(error) = complete_lane(state, token - 1) {
            result.error |= error;
            *retired = true;
        } else {
            result.executed_tasks += 1;
        }
    }
}

// Construct and consume each claim in its arm, without an Option/task merge.
#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn execute_task_at_v2<'dispatch>(
    storage: &WaveMlpTileStorageV2<'dispatch>,
    token: u32,
    lane: usize,
    written: &mut usize,
    valid: &mut bool,
    execute: &mut impl for<'claim> FnMut(WaveMlpTileTaskV2<'claim, 'dispatch>),
) {
    if !*valid {
        return;
    }
    match token {
        1 => execute(WaveMlpTileTaskV2::Norm(MlpNormTileV2 {
            storage,
            lane,
            written,
            valid,
        })),
        2..=97 => execute(WaveMlpTileTaskV2::Gate(MlpGateTileV2 {
            storage,
            lane,
            row_base: (token - 2) as usize * TILE_ROWS,
            written,
            valid,
        })),
        98..=193 => execute(WaveMlpTileTaskV2::Up(MlpUpTileV2 {
            storage,
            lane,
            row_base: (token - 98) as usize * TILE_ROWS,
            written,
            valid,
        })),
        194 => execute(WaveMlpTileTaskV2::SwiGlu(MlpSwiGluTileV2 {
            storage,
            lane,
            written,
            valid,
        })),
        195..=258 => execute(WaveMlpTileTaskV2::Down(MlpDownTileV2 {
            storage,
            lane,
            row_base: (token - 195) as usize * TILE_ROWS,
            written,
            valid,
        })),
        _ => {}
    }
}

/// Fixed finite arbitration and uniform three-exchange local rounds.
pub struct WaveMlpTileWorkerV2<'group, 'dispatch> {
    storage: WaveMlpTileStorageV2<'dispatch>,
    pipeline: WorkgroupPipeline<'group, u32, 2, 64, 1>,
    lane: usize,
    worker: u32,
    retired: bool,
    result: FiniteJoinWorkerResult,
}

impl<'group, 'dispatch> WaveMlpTileWorkerV2<'group, 'dispatch> {
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn new(
        storage: WaveMlpTileStorageV2<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
    ) -> Result<Self, u32> {
        if !launch_dimensions_valid() {
            return Err(INVALID);
        }
        let global = thread::index_1d().get();
        if global >= WORKGROUPS as usize * WAVE_LANES {
            return Err(INVALID);
        }
        Ok(Self::from_coordinates(
            storage,
            scope,
            global % 64,
            (global / 64) as u32,
        ))
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn from_coordinates(
        storage: WaveMlpTileStorageV2<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
        lane: usize,
        worker: u32,
    ) -> Self {
        Self {
            storage,
            pipeline: WorkgroupPipeline::current(scope),
            lane,
            worker,
            retired: false,
            result: FiniteJoinWorkerResult {
                error: 0,
                executed_tasks: 0,
                rounds: 0,
                empty_probes: 0,
            },
        }
    }

    /// Consumes one-shot storage, bounded by 512 uniform local rounds. A
    /// broadcast stop token retires only after all lanes finish that round.
    /// No callback can escape its claim or publish task completion itself.
    ///
    /// ```compile_fail,E0382
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_mlp_tiles_v2::{WaveMlpTileStorageV2,WaveMlpTileWorkerV2}};
    /// fn reuse<'g,'d>(s: WaveMlpTileStorageV2<'d>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = WaveMlpTileWorkerV2::run(s, scope, |_| {});
    ///     let _ = WaveMlpTileWorkerV2::run(s, scope, |_| {});
    /// }
    /// ```
    /// ```compile_fail,E0521
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_mlp_tiles_v2::{WaveMlpTileStorageV2,WaveMlpTileTaskV2,WaveMlpTileWorkerV2}};
    /// fn escape<'g,'d,'a>(s: WaveMlpTileStorageV2<'d>, scope: &mut WorkgroupLdsScope<'g>, saved: &mut Option<WaveMlpTileTaskV2<'a,'d>>) {
    ///     let _ = WaveMlpTileWorkerV2::run(s, scope, |task| *saved = Some(task));
    /// }
    /// ```
    /// ```compile_fail,E0599
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_mlp_tiles_v2::{WaveMlpTileStorageV2,WaveMlpTileWorkerV2}};
    /// fn finish<'g,'d>(s: WaveMlpTileStorageV2<'d>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = WaveMlpTileWorkerV2::run(s, scope, |task| task.finish());
    /// }
    /// ```
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn run(
        storage: WaveMlpTileStorageV2<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
        execute: impl for<'claim> FnMut(WaveMlpTileTaskV2<'claim, 'dispatch>),
    ) -> Result<FiniteJoinWorkerResult, u32> {
        Self::run_with_stop_policy::<true>(storage, scope, execute)
    }

    /// Runs exactly 512 rounds after valid launch admission, including all
    /// three local exchanges in retired rounds. Unlike [`Self::run`], STOP
    /// suppresses further tasks but does not exit the outer loop early.
    ///
    /// This source-compatibility path adds LDS work and is not timing-neutral.
    /// Retired padding leaves shared state and payload unchanged; completion
    /// still requires the external acquired-state and numerical checks.
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn run_fixed_rounds(
        storage: WaveMlpTileStorageV2<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
        execute: impl for<'claim> FnMut(WaveMlpTileTaskV2<'claim, 'dispatch>),
    ) -> Result<FiniteJoinWorkerResult, u32> {
        Self::run_with_stop_policy::<false>(storage, scope, execute)
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn run_with_stop_policy<const STOP_EARLY: bool>(
        storage: WaveMlpTileStorageV2<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
        mut execute: impl for<'claim> FnMut(WaveMlpTileTaskV2<'claim, 'dispatch>),
    ) -> Result<FiniteJoinWorkerResult, u32> {
        if !launch_dimensions_valid() {
            return Err(INVALID);
        }
        let global = thread::index_1d().get();
        if global >= WORKGROUPS as usize * WAVE_LANES {
            return Err(INVALID);
        }
        let lane = global % 64;
        let mut worker = Self::from_coordinates(storage, scope, lane, (global / 64) as u32);
        let mut round_index = 0u32;
        while round_index < MAX_ROUNDS {
            let phase = round_index as usize * 3;
            let mut round = worker.begin_at(phase, lane);
            let stop = round.token == STOP;
            execute_task_at_v2(
                &round.worker.storage,
                round.token,
                lane,
                &mut round.written,
                &mut round.valid,
                &mut execute,
            );
            round.finish_at(phase, lane);
            if STOP_EARLY && stop {
                break;
            }
            round_index += 1;
        }
        Ok(worker.observations())
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn exchange(&mut self, phase: usize, lane: usize, value: u32, leader_only: bool) -> u32 {
        self.pipeline.stage(phase);
        self.pipeline.write(phase, lane, value);
        self.pipeline.commit(phase);
        self.pipeline.wait(phase);
        self.pipeline.consume(phase);
        let mut result = self.pipeline.read(phase, 0);
        if !leader_only {
            let mut cursor = 1;
            while cursor < WAVE_LANES {
                result |= self.pipeline.read(phase, cursor);
                cursor += 1;
            }
        }
        self.pipeline.release(phase);
        crate::gfx950::Gfx950Subgroup::current()
            .broadcast_f32::<64>(f32::from_bits(result), 0)
            .to_bits()
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn next_round(&mut self) -> Option<WaveMlpTileRoundV2<'_, 'group, 'dispatch>> {
        if self.result.rounds == MAX_ROUNDS {
            return None;
        }
        let phase = self.result.rounds as usize * 3;
        Some(self.begin_at(phase, self.lane))
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn begin_at(&mut self, phase: usize, lane: usize) -> WaveMlpTileRoundV2<'_, 'group, 'dispatch> {
        let token = begin_round_claim(
            self.storage.state(),
            lane,
            self.worker,
            &mut self.retired,
            &mut self.result,
        );
        let token = self.exchange(phase, lane, token, true);
        let invalid = (!lane_admitted(self.storage.state(), token, self.worker)) as u32;
        let valid = self.exchange(phase + 1, lane, invalid, false) == 0;
        if !valid {
            self.storage.state()[ERRORS].fetch_or(MISSING_PREDECESSOR, Ordering::Relaxed);
        }
        WaveMlpTileRoundV2 {
            worker: self,
            token,
            valid,
            written: 0,
            phase,
        }
    }

    /// Local observations are not a terminal graph-completion certificate.
    pub fn observations(&self) -> FiniteJoinWorkerResult {
        self.result
    }
}

/// Move-only round; every lane must consume it exactly once.
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_mlp_tiles_v2::WaveMlpTileRoundV2;
/// fn twice(round: WaveMlpTileRoundV2<'_, '_, '_>) { round.finish(); round.finish(); }
/// ```
#[must_use = "every participating lane must finish each issued round"]
pub struct WaveMlpTileRoundV2<'worker, 'group, 'dispatch> {
    worker: &'worker mut WaveMlpTileWorkerV2<'group, 'dispatch>,
    token: u32,
    valid: bool,
    written: usize,
    phase: usize,
}

pub enum WaveMlpTileTaskV2<'claim, 'dispatch> {
    Norm(MlpNormTileV2<'claim, 'dispatch>),
    Gate(MlpGateTileV2<'claim, 'dispatch>),
    Up(MlpUpTileV2<'claim, 'dispatch>),
    SwiGlu(MlpSwiGluTileV2<'claim, 'dispatch>),
    Down(MlpDownTileV2<'claim, 'dispatch>),
}

impl<'dispatch> WaveMlpTileRoundV2<'_, '_, 'dispatch> {
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn task(&mut self) -> Option<WaveMlpTileTaskV2<'_, 'dispatch>> {
        self.task_at(self.worker.lane)
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn task_at(&mut self, lane: usize) -> Option<WaveMlpTileTaskV2<'_, 'dispatch>> {
        if !self.valid {
            return None;
        }
        let storage = &self.worker.storage;
        match self.token {
            1 => Some(WaveMlpTileTaskV2::Norm(MlpNormTileV2 {
                storage,
                lane,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            2..=97 => Some(WaveMlpTileTaskV2::Gate(MlpGateTileV2 {
                storage,
                lane,
                row_base: (self.token - 2) as usize * TILE_ROWS,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            98..=193 => Some(WaveMlpTileTaskV2::Up(MlpUpTileV2 {
                storage,
                lane,
                row_base: (self.token - 98) as usize * TILE_ROWS,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            194 => Some(WaveMlpTileTaskV2::SwiGlu(MlpSwiGluTileV2 {
                storage,
                lane,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            195..=258 => Some(WaveMlpTileTaskV2::Down(MlpDownTileV2 {
                storage,
                lane,
                row_base: (self.token - 195) as usize * TILE_ROWS,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            _ => None,
        }
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn finish(self) {
        let phase = self.phase;
        let lane = self.worker.lane;
        self.finish_at(phase, lane);
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn finish_at(self, phase: usize, lane: usize) {
        let bad = (!complete_coverage(self.token, lane, self.written, self.valid)) as u32;
        let all_valid = self.worker.exchange(phase + 2, lane, bad, false) == 0;
        finish_round_completion(
            self.worker.storage.state(),
            self.token,
            all_valid,
            &mut self.worker.retired,
            &mut self.worker.result,
        );
    }
}

/// Claim-borrowed norm writes exactly lane+64*component in order.
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_mlp_tiles_v2::MlpNormTileV2;
/// fn clone_required<T: Clone>() {}
/// clone_required::<MlpNormTileV2<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_mlp_tiles_v2::MlpNormTileV2;
/// let forged = MlpNormTileV2::from_task(0, 0);
/// ```
pub struct MlpNormTileV2<'claim, 'dispatch> {
    storage: &'claim WaveMlpTileStorageV2<'dispatch>,
    lane: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}
impl MlpNormTileV2<'_, '_> {
    pub fn lane(&self) -> usize {
        self.lane
    }
    pub fn reject(&mut self) {
        *self.valid = false;
    }
    pub fn input(&self, column: usize) -> Option<u16> {
        if column >= NORM_ELEMENTS {
            return None;
        }
        // SAFETY: this initialized immutable read root lives through dispatch.
        Some(unsafe { (*self.storage.input)[column] })
    }
    pub fn weight(&self, column: usize) -> Option<u16> {
        if column >= NORM_ELEMENTS {
            return None;
        }
        Some(unsafe { (*self.storage.norm_weight)[column] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_component(&mut self, component: usize, value: u16) -> bool {
        if !*self.valid || component != *self.written || component >= 64 || self.lane >= 64 {
            *self.valid = false;
            return false;
        }
        // SAFETY: the injective lane/component map is fixed by private issuance.
        unsafe {
            (*self.storage.normalized)[self.lane + 64 * component] = value;
        }
        *self.written += 1;
        true
    }
}

/// Norm-acquired gate tile; only lane0 writes its 64 ordered rows.
pub struct MlpGateTileV2<'claim, 'dispatch> {
    storage: &'claim WaveMlpTileStorageV2<'dispatch>,
    lane: usize,
    row_base: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}
impl MlpGateTileV2<'_, '_> {
    pub fn lane(&self) -> usize {
        self.lane
    }
    pub fn reject(&mut self) {
        *self.valid = false;
    }
    pub fn input(&self, inner: usize) -> Option<u16> {
        if !*self.valid || inner >= NORM_ELEMENTS {
            return None;
        }
        // SAFETY: this lane acquired Norm DONE before the claim was issued.
        Some(unsafe { (*self.storage.normalized)[inner] })
    }
    pub fn weight(&self, row: usize, inner: usize) -> Option<u16> {
        if row >= TILE_ROWS || inner >= NORM_ELEMENTS {
            return None;
        }
        Some(unsafe { (*self.storage.gate_weight)[(self.row_base + row) * NORM_ELEMENTS + inner] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_column(&mut self, row: usize, value: u16) -> bool {
        if !*self.valid || self.lane != 0 || row != *self.written || row >= TILE_ROWS {
            *self.valid = false;
            return false;
        }
        // SAFETY: only this task's lane0 can advance this output cursor.
        unsafe {
            (*self.storage.gate)[self.row_base + row] = value;
        }
        *self.written += 1;
        true
    }
}

/// Norm-acquired up projection, disjoint from gate despite equal geometry.
pub struct MlpUpTileV2<'claim, 'dispatch> {
    storage: &'claim WaveMlpTileStorageV2<'dispatch>,
    lane: usize,
    row_base: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}
impl MlpUpTileV2<'_, '_> {
    pub fn lane(&self) -> usize {
        self.lane
    }
    pub fn reject(&mut self) {
        *self.valid = false;
    }
    pub fn input(&self, inner: usize) -> Option<u16> {
        if !*self.valid || inner >= NORM_ELEMENTS {
            return None;
        }
        Some(unsafe { (*self.storage.normalized)[inner] })
    }
    pub fn weight(&self, row: usize, inner: usize) -> Option<u16> {
        if row >= TILE_ROWS || inner >= NORM_ELEMENTS {
            return None;
        }
        Some(unsafe { (*self.storage.up_weight)[(self.row_base + row) * NORM_ELEMENTS + inner] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_column(&mut self, row: usize, value: u16) -> bool {
        if !*self.valid || self.lane != 0 || row != *self.written || row >= TILE_ROWS {
            *self.valid = false;
            return false;
        }
        unsafe {
            (*self.storage.up)[self.row_base + row] = value;
        }
        *self.written += 1;
        true
    }
}

/// Acquires both projections; each lane owns96 activation components.
pub struct MlpSwiGluTileV2<'claim, 'dispatch> {
    storage: &'claim WaveMlpTileStorageV2<'dispatch>,
    lane: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}
impl MlpSwiGluTileV2<'_, '_> {
    pub fn lane(&self) -> usize {
        self.lane
    }
    pub fn reject(&mut self) {
        *self.valid = false;
    }
    pub fn gate(&self, column: usize) -> Option<u16> {
        if !*self.valid || column >= INTERMEDIATE_ELEMENTS {
            return None;
        }
        Some(unsafe { (*self.storage.gate)[column] })
    }
    pub fn up(&self, column: usize) -> Option<u16> {
        if !*self.valid || column >= INTERMEDIATE_ELEMENTS {
            return None;
        }
        Some(unsafe { (*self.storage.up)[column] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_component(&mut self, component: usize, value: u16) -> bool {
        if !*self.valid || component != *self.written || component >= 96 || self.lane >= 64 {
            *self.valid = false;
            return false;
        }
        unsafe {
            (*self.storage.activation)[self.lane + 64 * component] = value;
        }
        *self.written += 1;
        true
    }
}

/// Activation-acquired down tile; only lane0 writes its 64 ordered FP32 rows.
pub struct MlpDownTileV2<'claim, 'dispatch> {
    storage: &'claim WaveMlpTileStorageV2<'dispatch>,
    lane: usize,
    row_base: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}
impl MlpDownTileV2<'_, '_> {
    pub fn lane(&self) -> usize {
        self.lane
    }
    pub fn reject(&mut self) {
        *self.valid = false;
    }
    pub fn input(&self, inner: usize) -> Option<u16> {
        if !*self.valid || inner >= INTERMEDIATE_ELEMENTS {
            return None;
        }
        Some(unsafe { (*self.storage.activation)[inner] })
    }
    pub fn weight(&self, row: usize, inner: usize) -> Option<u16> {
        if row >= TILE_ROWS || inner >= INTERMEDIATE_ELEMENTS {
            return None;
        }
        Some(unsafe {
            (*self.storage.down_weight)[(self.row_base + row) * INTERMEDIATE_ELEMENTS + inner]
        })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_output(&mut self, row: usize, value: f32) -> bool {
        if !*self.valid || self.lane != 0 || row != *self.written || row >= TILE_ROWS {
            *self.valid = false;
            return false;
        }
        unsafe {
            (*self.storage.down_partial)[self.row_base + row] = value;
        }
        *self.written += 1;
        true
    }
}

#[cfg(test)]
#[path = "wave_mlp_tiles_v2_tests.rs"]
mod tests;
