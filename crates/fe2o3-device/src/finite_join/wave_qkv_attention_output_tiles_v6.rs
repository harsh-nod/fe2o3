//! Engineering-only row-tiled RMSNorm -> QKV -> post/RoPE/KV -> attention -> O.
//!
//! This distinct fifteen-root profile leaves V5 and all MLP profiles unchanged.
//! There are 130 claims: one norm, 48 QKV tiles, one post, 16 attention heads,
//! and 64 O tiles. Row tiles own 64 rows while every row retains Wave64 order.
//! Head claims retain the original causal token recurrence, not a token split.
//! No compiler, runtime, convergence, scheduling or numerical authority is issued.

use super::{Claim, DUPLICATE, FiniteJoinWorkerResult, INVALID, MISSING_PREDECESSOR, STALE_EPOCH};
use crate::{WorkgroupLdsScope, WorkgroupPipeline, thread};
use core::{
    marker::PhantomData,
    mem::align_of,
    sync::atomic::{AtomicU32, Ordering},
};

pub const WAVE_LANES: usize = 64;
pub const NORM_ELEMENTS: usize = 4096;
pub const QKV_COLUMNS: usize = 3072;
pub const QKV_ELEMENTS: usize = NORM_ELEMENTS * QKV_COLUMNS;
pub const HEAD_WIDTH: usize = 128;
pub const QUERY_ELEMENTS: usize = 2048;
pub const ATTENTION_ELEMENTS: usize = QUERY_ELEMENTS;
pub const OUTPUT_ELEMENTS: usize = 4096;
pub const OUTPUT_WEIGHT_ELEMENTS: usize = OUTPUT_ELEMENTS * ATTENTION_ELEMENTS;
pub const HEAD_WEIGHT_ELEMENTS: usize = 256;
pub const ROTARY_ELEMENTS: usize = 128;
pub const CACHE_CAPACITY: usize = 2304;
pub const PAGE_COUNT: usize = 144;
pub const CACHE_METADATA_WORDS: usize = 1 + PAGE_COUNT;
pub const CACHE_ELEMENTS: usize = CACHE_CAPACITY * 512;
pub const WORKGROUPS: u32 = 64;
pub const TILE_ROWS: usize = 64;
pub const STAGE_COUNTS: [u32; 5] = [1, 48, 1, 16, 64];
pub const STAGE_STARTS: [u32; 5] = [0, 1, 49, 50, 66];
pub const TASK_COUNT: u32 = 130;
pub const WAVE_STATE_WORDS: usize = 284;
pub const INCOMPLETE_WRITES: u32 = 16;
/// Fixed local opportunities, not a cross-workgroup progress guarantee.
pub const MAX_ROUNDS: u32 = 256;
const POLLS_PER_ROUND: u32 = 256;
const ALL_STAGES: u32 = 0x1f;
const EPOCH: usize = 0;
const ERRORS: usize = 1;
const READY: usize = 2;
const DONE: usize = 3;
const NEXT: usize = 4;
const COMPLETED: usize = 9;
const CLAIMED: usize = 14;
const DONE_TILES: usize = 19;
const OWNERS: usize = 24;
const ARRIVALS: usize = 154;
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
    for word in 0..5 {
        let expected = if word == 4 { 3 } else { u32::MAX };
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

/// A fifteen-root one-shot storage binding, not a compiler-issued capability.
///
/// No field, pointer getter, Clone, or Copy implementation is public. Even a
/// correctly sized value grants no protected/runtime coherence permission.
#[must_use]
#[repr(C)]
pub struct WaveQkvAttentionOutputTileStorageV6<'dispatch> {
    input: *const [u16; NORM_ELEMENTS],
    norm_weight: *const [u16; NORM_ELEMENTS],
    qkv_weight: *const [u16; QKV_ELEMENTS],
    head_norm_weight: *const [u16; HEAD_WEIGHT_ELEMENTS],
    rotary: *const [f32; ROTARY_ELEMENTS],
    cache_metadata: *const [u32; CACHE_METADATA_WORDS],
    output_weight: *const [u16; OUTPUT_WEIGHT_ELEMENTS],
    normalized: *mut [u16; NORM_ELEMENTS],
    qkv_output: *mut [u16; QKV_COLUMNS],
    query: *mut [u16; QUERY_ELEMENTS],
    key_cache: *mut [u16; CACHE_ELEMENTS],
    value_cache: *mut [u16; CACHE_ELEMENTS],
    attention_output: *mut [u16; ATTENTION_ELEMENTS],
    output_partial: *mut [f32; OUTPUT_ELEMENTS],
    state: *const [AtomicU32; WAVE_STATE_WORDS],
    _lifetime: PhantomData<(
        &'dispatch [u16],
        &'dispatch mut [u16],
        &'dispatch mut [f32],
        &'dispatch [AtomicU32],
    )>,
    _not_send_sync: PhantomData<*mut ()>,
}

fn disjoint_regions(regions: [(usize, usize, usize); 15]) -> bool {
    for (index, &(base, bytes, alignment)) in regions.iter().enumerate() {
        let Some(end) = base.checked_add(bytes) else {
            return false;
        };
        if base == 0 || base % alignment != 0 || bytes == 0 {
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

impl<'dispatch> WaveQkvAttentionOutputTileStorageV6<'dispatch> {
    /// Binds exact engineering storage without claiming a production issuer.
    ///
    /// # Safety
    /// Every root must designate its exact array in one live allocation. All
    /// fifteen byte ranges must be disjoint and valid for the complete dispatch.
    /// Read roots are initialized and immutable; output roots are writable.
    /// Packed weights/output use contiguous rows `[Q2048; K512; V512]` for
    /// one TP2 rank. Head weights are Q128 then K128; rotary is cos64 then
    /// sin64. Metadata is position then a permutation of 144 page IDs for
    /// 16-token pages. Caches contain 2304 rows of 512 BF16 words. All cache
    /// words are initialized; prior history is immutable and only this dispatch's
    /// selected row is writable by Post. Attention reads only causal rows after
    /// Post acquisition and writes sixteen contiguous 128-word query heads.
    /// Output weights are row-major `[4096, 2048]`, formed by selecting this
    /// rank's 2048 columns from every row of the full `[4096, 4096]` O matrix.
    /// Output tiles read completed attention and jointly write 4096 FP32 partials;
    /// these must not be narrowed before the later cross-rank sum.
    /// Before any invocation starts, construct genuine AtomicU32[284] from
    /// initial_state_words(), with initialization happening-before all invocations. Do not reinitialize
    /// or reuse storage/epoch until all invocations quiesce, including errors.
    /// Exactly 64 full WG64 groups participate, with one invocation per physical
    /// lane. System-scope atomics and ordinary payloads must share a coherent
    /// publication domain. No external access is permitted during the dispatch.
    /// Every lane must execute the same numerical collectives and each issued
    /// round's finish exactly once, including idle and rejected rounds. The
    /// current source/compiler/runtime does not authenticate these obligations.
    pub unsafe fn from_raw_parts(
        input: *const [u16; NORM_ELEMENTS],
        norm_weight: *const [u16; NORM_ELEMENTS],
        qkv_weight: *const [u16; QKV_ELEMENTS],
        head_norm_weight: *const [u16; HEAD_WEIGHT_ELEMENTS],
        rotary: *const [f32; ROTARY_ELEMENTS],
        cache_metadata: *const [u32; CACHE_METADATA_WORDS],
        output_weight: *const [u16; OUTPUT_WEIGHT_ELEMENTS],
        normalized: *mut [u16; NORM_ELEMENTS],
        qkv_output: *mut [u16; QKV_COLUMNS],
        query: *mut [u16; QUERY_ELEMENTS],
        key_cache: *mut [u16; CACHE_ELEMENTS],
        value_cache: *mut [u16; CACHE_ELEMENTS],
        attention_output: *mut [u16; ATTENTION_ELEMENTS],
        output_partial: *mut [f32; OUTPUT_ELEMENTS],
        state: *const [AtomicU32; WAVE_STATE_WORDS],
    ) -> Result<Self, u32> {
        if !disjoint_regions([
            (input as usize, NORM_ELEMENTS * 2, align_of::<u16>()),
            (norm_weight as usize, NORM_ELEMENTS * 2, align_of::<u16>()),
            (qkv_weight as usize, QKV_ELEMENTS * 2, align_of::<u16>()),
            (
                head_norm_weight as usize,
                HEAD_WEIGHT_ELEMENTS * 2,
                align_of::<u16>(),
            ),
            (rotary as usize, ROTARY_ELEMENTS * 4, align_of::<f32>()),
            (
                cache_metadata as usize,
                CACHE_METADATA_WORDS * 4,
                align_of::<u32>(),
            ),
            (
                output_weight as usize,
                OUTPUT_WEIGHT_ELEMENTS * 2,
                align_of::<u16>(),
            ),
            (normalized as usize, NORM_ELEMENTS * 2, align_of::<u16>()),
            (qkv_output as usize, QKV_COLUMNS * 2, align_of::<u16>()),
            (query as usize, QUERY_ELEMENTS * 2, align_of::<u16>()),
            (key_cache as usize, CACHE_ELEMENTS * 2, align_of::<u16>()),
            (value_cache as usize, CACHE_ELEMENTS * 2, align_of::<u16>()),
            (
                attention_output as usize,
                ATTENTION_ELEMENTS * 2,
                align_of::<u16>(),
            ),
            (
                output_partial as usize,
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
            qkv_weight,
            head_norm_weight,
            rotary,
            cache_metadata,
            output_weight,
            normalized,
            qkv_output,
            query,
            key_cache,
            value_cache,
            attention_output,
            output_partial,
            state,
            _lifetime: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    // WAVE=true is private to run's full-wave admitted Post dispatch. Public
    // task access remains scalar and never acquires hidden collectives.
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn metadata_word<const WAVE: bool>(&self, index: usize) -> u32 {
        // SAFETY: construction retains initialized, immutable metadata;
        // array indexing still checks the supplied index.
        let word = unsafe { (*self.cache_metadata)[index] };
        if WAVE {
            crate::gfx950::Gfx950Subgroup::current()
                .broadcast_f32::<64>(f32::from_bits(word), 0)
                .to_bits()
        } else {
            word
        }
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn cache_slot_impl<const WAVE: bool>(&self) -> Option<usize> {
        // Validate the complete immutable page permutation before any payload
        // write. Five scalar bitsets avoid hidden private-array ABI assumptions.
        let position = self.metadata_word::<WAVE>(0) as usize;
        if position >= CACHE_CAPACITY {
            return None;
        }
        let (mut a, mut b, mut c, mut d, mut e) = (0u32, 0u32, 0u32, 0u32, 0u32);
        let mut invalid = false;
        let mut logical = 0;
        while logical < PAGE_COUNT {
            let page = self.metadata_word::<WAVE>(1 + logical);
            let bit = 1u32 << (page & 31);
            let duplicate = if page < 32 {
                let old = a;
                a |= bit;
                old & bit != 0
            } else if page < 64 {
                let old = b;
                b |= bit;
                old & bit != 0
            } else if page < 96 {
                let old = c;
                c |= bit;
                old & bit != 0
            } else if page < 128 {
                let old = d;
                d |= bit;
                old & bit != 0
            } else if page < PAGE_COUNT as u32 {
                let old = e;
                e |= bit;
                old & bit != 0
            } else {
                true
            };
            invalid |= duplicate;
            logical += 1;
        }
        if invalid {
            return None;
        }
        let physical = self.metadata_word::<WAVE>(1 + position / 16) as usize;
        Some(physical * 16 + position % 16)
    }

    fn state(&self) -> &[AtomicU32; WAVE_STATE_WORDS] {
        // SAFETY: engineering construction retains aligned, initialized atomic
        // storage. This reference never aliases an ordinary payload root.
        unsafe { &*self.state }
    }
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn predecessor_mask(stage: usize) -> u32 {
    if stage == 0 {
        0
    } else if stage == 1 {
        1
    } else if stage == 2 {
        3
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
    } else if tile < 49 {
        1
    } else if tile == 49 {
        2
    } else if tile < 66 {
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
            state[READY].fetch_or(2, Ordering::Release);
        } else if stage == 1 {
            state[READY].fetch_or(4, Ordering::Release);
        } else if stage == 2 {
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
    } else if token == 50 {
        48
    } else if token >= 51 && token <= 66 {
        2
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

// Construct and consume each view in its own arm. Metadata validation is
// private to the full-wave path; CPU model calls explicitly select scalar reads.
#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn execute_task_at_v6<'dispatch, const WAVE: bool>(
    storage: &WaveQkvAttentionOutputTileStorageV6<'dispatch>,
    token: u32,
    lane: usize,
    written: &mut usize,
    valid: &mut bool,
    execute: &mut impl for<'claim> FnMut(WaveQkvAttentionOutputTileTaskV6<'claim, 'dispatch>),
) {
    if !*valid {
        return;
    }
    match token {
        1 => execute(WaveQkvAttentionOutputTileTaskV6::Norm(
            QkvAttentionOutputNormTileV6 {
                storage,
                lane,
                written,
                valid,
            },
        )),
        2..=49 => execute(WaveQkvAttentionOutputTileTaskV6::Projection(
            QkvAttentionOutputProjectionTileV6 {
                storage,
                lane,
                base: (token - 2) as usize * TILE_ROWS,
                written,
                valid,
            },
        )),
        50 => {
            if let Some(slot) = storage.cache_slot_impl::<WAVE>() {
                execute(WaveQkvAttentionOutputTileTaskV6::Post(
                    AttentionReadyTileV6 {
                        storage,
                        lane,
                        slot,
                        written,
                        valid,
                    },
                ));
            } else {
                *valid = false;
            }
        }
        51..=66 => {
            let position = storage.metadata_word::<WAVE>(0) as usize;
            if position < CACHE_CAPACITY && matches!(storage.cache_slot_impl::<WAVE>(), Some(_)) {
                execute(WaveQkvAttentionOutputTileTaskV6::Attention(
                    AttentionHeadTileV6 {
                        storage,
                        lane,
                        position,
                        head: (token - 51) as usize,
                        written,
                        valid,
                    },
                ));
            } else {
                *valid = false;
            }
        }
        67..=130 => execute(WaveQkvAttentionOutputTileTaskV6::OutputProjection(
            OutputProjectionTileV6 {
                storage,
                lane,
                base: (token - 67) as usize * TILE_ROWS,
                written,
                valid,
            },
        )),
        _ => {}
    }
}

pub enum WaveQkvAttentionOutputTileTaskV6<'claim, 'dispatch> {
    Norm(QkvAttentionOutputNormTileV6<'claim, 'dispatch>),
    Projection(QkvAttentionOutputProjectionTileV6<'claim, 'dispatch>),
    Post(AttentionReadyTileV6<'claim, 'dispatch>),
    Attention(AttentionHeadTileV6<'claim, 'dispatch>),
    OutputProjection(OutputProjectionTileV6<'claim, 'dispatch>),
}

struct Worker<'group, 'dispatch> {
    storage: WaveQkvAttentionOutputTileStorageV6<'dispatch>,
    pipeline: WorkgroupPipeline<'group, u32, 2, 64, 1>,
}
impl Worker<'_, '_> {
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
}

/// One consuming entry; no rounds, pipeline phases or completion methods escape.
pub struct WaveQkvAttentionOutputTileWorkerV6;
impl WaveQkvAttentionOutputTileWorkerV6 {
    /// Exactly 256 local rounds after valid launch admission, each with three
    /// exchanges over two 64-u32 LDS buffers (512 bytes). STOP suppresses tasks
    /// but never exits the loop. Retired padding is state/payload inert while
    /// still doing LDS work. This is not a graph-completion or progress promise.
    /// Every numerical callback must retain its row's original Wave64 reduction
    /// and every attention head's ascending causal-token recurrence.
    ///
    /// ```compile_fail,E0382
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_qkv_attention_output_tiles_v6::{WaveQkvAttentionOutputTileStorageV6 as Storage,WaveQkvAttentionOutputTileWorkerV6 as Worker}};
    /// fn reuse<'g,'d>(s:Storage<'d>, scope:&mut WorkgroupLdsScope<'g>) {
    ///     let _ = Worker::run(s,scope,|_|{});
    ///     let _ = Worker::run(s,scope,|_|{});
    /// }
    /// ```
    /// ```compile_fail,E0521
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_qkv_attention_output_tiles_v6::{WaveQkvAttentionOutputTileStorageV6 as Storage,WaveQkvAttentionOutputTileWorkerV6 as Worker,WaveQkvAttentionOutputTileTaskV6 as Task}};
    /// fn escape<'g,'d,'a>(s:Storage<'d>, scope:&mut WorkgroupLdsScope<'g>, saved:&mut Option<Task<'a,'d>>) {
    ///     let _ = Worker::run(s,scope,|task| *saved = Some(task));
    /// }
    /// ```
    /// ```compile_fail,E0599
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_qkv_attention_output_tiles_v6::{WaveQkvAttentionOutputTileStorageV6 as Storage,WaveQkvAttentionOutputTileWorkerV6 as Worker}};
    /// fn finish<'g,'d>(s:Storage<'d>, scope:&mut WorkgroupLdsScope<'g>) {
    ///     let _ = Worker::run(s,scope,|task| task.finish());
    /// }
    /// ```
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn run<'group, 'dispatch>(
        storage: WaveQkvAttentionOutputTileStorageV6<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
        mut execute: impl for<'claim> FnMut(WaveQkvAttentionOutputTileTaskV6<'claim, 'dispatch>),
    ) -> Result<FiniteJoinWorkerResult, u32> {
        if !launch_dimensions_valid() {
            return Err(INVALID);
        }
        let global = thread::index_1d().get();
        if global >= WORKGROUPS as usize * WAVE_LANES {
            return Err(INVALID);
        }
        let lane = global % WAVE_LANES;
        let owner = (global / WAVE_LANES) as u32;
        let mut worker = Worker {
            storage,
            pipeline: WorkgroupPipeline::current(scope),
        };
        let mut retired = false;
        let mut result = FiniteJoinWorkerResult {
            error: 0,
            executed_tasks: 0,
            rounds: 0,
            empty_probes: 0,
        };
        let mut round = 0u32;
        while round < MAX_ROUNDS {
            let phase = round as usize * 3;
            let token = begin_round_claim(
                worker.storage.state(),
                lane,
                owner,
                &mut retired,
                &mut result,
            );
            let token = worker.exchange(phase, lane, token, true);
            let invalid = (!lane_admitted(worker.storage.state(), token, owner)) as u32;
            let mut valid = worker.exchange(phase + 1, lane, invalid, false) == 0;
            if !valid {
                worker.storage.state()[ERRORS].fetch_or(MISSING_PREDECESSOR, Ordering::Relaxed);
            }
            let mut written = 0;
            execute_task_at_v6::<true>(
                &worker.storage,
                token,
                lane,
                &mut written,
                &mut valid,
                &mut execute,
            );
            let bad = (!complete_coverage(token, lane, written, valid)) as u32;
            let all_valid = worker.exchange(phase + 2, lane, bad, false) == 0;
            finish_round_completion(
                worker.storage.state(),
                token,
                all_valid,
                &mut retired,
                &mut result,
            );
            round += 1;
        }
        Ok(result)
    }
}

pub struct QkvAttentionOutputNormTileV6<'claim, 'dispatch> {
    storage: &'claim WaveQkvAttentionOutputTileStorageV6<'dispatch>,
    lane: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl QkvAttentionOutputNormTileV6<'_, '_> {
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
        // SAFETY: immutable exact input root is retained by this live claim.
        Some(unsafe { (*self.storage.input)[column] })
    }
    pub fn weight(&self, column: usize) -> Option<u16> {
        if column >= NORM_ELEMENTS {
            return None;
        }
        // SAFETY: immutable exact norm-weight root is retained by this claim.
        Some(unsafe { (*self.storage.norm_weight)[column] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_component(&mut self, component: usize, value: u16) -> bool {
        if !*self.valid
            || component != *self.written
            || component >= WAVE_LANES
            || self.lane >= WAVE_LANES
        {
            *self.valid = false;
            return false;
        }
        let column = self.lane + component * WAVE_LANES;
        // SAFETY: private claim issuance fixes the owning task/physical lane.
        // The injective lane/component map and sequential cursor exclude aliases.
        unsafe {
            (*self.storage.normalized)[column] = value;
        }
        *self.written += 1;
        true
    }
}

/// Dependency-acquired projection; lane0 owns one of 48 disjoint 64-row tiles.
pub struct QkvAttentionOutputProjectionTileV6<'claim, 'dispatch> {
    storage: &'claim WaveQkvAttentionOutputTileStorageV6<'dispatch>,
    lane: usize,
    base: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl QkvAttentionOutputProjectionTileV6<'_, '_> {
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
        // SAFETY: every lane's positive Acquire of Norm DONE precedes issuance.
        // No immutable Rust slice was formed while the norm task was writing.
        Some(unsafe { (*self.storage.normalized)[inner] })
    }
    pub fn weight(&self, local_column: usize, inner: usize) -> Option<u16> {
        if local_column >= TILE_ROWS || inner >= NORM_ELEMENTS {
            return None;
        }
        let index = (self.base + local_column) * NORM_ELEMENTS + inner;
        // SAFETY: private base is a 64-row tile in the exact 3072-row root.
        Some(unsafe { (*self.storage.qkv_weight)[index] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_column(&mut self, local_column: usize, value: u16) -> bool {
        if !*self.valid
            || self.lane != 0
            || local_column != *self.written
            || local_column >= TILE_ROWS
        {
            *self.valid = false;
            return false;
        }
        // SAFETY: task-bound tiles are disjoint and only their lane0 writes.
        unsafe {
            (*self.storage.qkv_output)[self.base + local_column] = value;
        }
        *self.written += 1;
        true
    }
}

/// One dependency-acquired post task. All Q/K/V reads are from immutable raw
/// projection results; private lane/head cursors issue disjoint output writes.
pub struct AttentionReadyTileV6<'claim, 'dispatch> {
    storage: &'claim WaveQkvAttentionOutputTileStorageV6<'dispatch>,
    lane: usize,
    slot: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl AttentionReadyTileV6<'_, '_> {
    pub fn lane(&self) -> usize {
        self.lane
    }
    pub fn reject(&mut self) {
        *self.valid = false;
    }

    pub fn head_input(&self, head: usize, column: usize) -> Option<u16> {
        if !*self.valid || head >= 20 || column >= HEAD_WIDTH {
            return None;
        }
        // SAFETY: the completed Norm and all 48 QKV tiles were acquired before view issuance.
        Some(unsafe { (*self.storage.qkv_output)[head * HEAD_WIDTH + column] })
    }

    pub fn head_weight(&self, head: usize, column: usize) -> Option<u16> {
        if !*self.valid || head >= 20 || column >= HEAD_WIDTH {
            return None;
        }
        let offset = if head < 16 { 0 } else { HEAD_WIDTH };
        Some(unsafe { (*self.storage.head_norm_weight)[offset + column] })
    }

    pub fn rotary(&self, half: usize) -> Option<f32> {
        if !*self.valid || self.lane >= WAVE_LANES || half >= 2 {
            return None;
        }
        Some(unsafe { (*self.storage.rotary)[half * WAVE_LANES + self.lane] })
    }

    pub fn value(&self, head: usize, half: usize) -> Option<u16> {
        if !*self.valid || self.lane >= WAVE_LANES || head >= 4 || half >= 2 {
            return None;
        }
        Some(unsafe {
            (*self.storage.qkv_output)[2560 + head * HEAD_WIDTH + self.lane + half * WAVE_LANES]
        })
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_query_head(&mut self, head: usize, low: u16, high: u16) -> bool {
        if !*self.valid || self.lane >= WAVE_LANES || head >= 16 || *self.written != 2 * head {
            *self.valid = false;
            return false;
        }
        let index = head * HEAD_WIDTH + self.lane;
        // SAFETY: a unique post claim, sequential heads and physical lanes give
        // each query element exactly one writer. Check all guards before stores.
        unsafe {
            (*self.storage.query)[index] = low;
            (*self.storage.query)[index + WAVE_LANES] = high;
        }
        *self.written += 2;
        true
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_key_value_head(
        &mut self,
        head: usize,
        key_low: u16,
        key_high: u16,
        value_low: u16,
        value_high: u16,
    ) -> bool {
        if !*self.valid
            || self.lane >= WAVE_LANES
            || self.slot >= CACHE_CAPACITY
            || head >= 4
            || *self.written != 32 + 4 * head
        {
            *self.valid = false;
            return false;
        }
        let index = self.slot * 512 + head * HEAD_WIDTH + self.lane;
        // SAFETY: the validated permutation fixes a unique logical cache row.
        // Separate K/V roots and the injective head/lane map exclude aliasing.
        unsafe {
            (*self.storage.key_cache)[index] = key_low;
            (*self.storage.key_cache)[index + WAVE_LANES] = key_high;
            (*self.storage.value_cache)[index] = value_low;
            (*self.storage.value_cache)[index + WAVE_LANES] = value_high;
        }
        *self.written += 4;
        true
    }
}

/// Claim-borrowed causal query/cache reader and sequential attention writer.
/// Accessors are scalar. A numerical wrapper must explicitly broadcast page
/// IDs before subgroup collectives; this view does not hide a GPU collective.
pub struct AttentionHeadTileV6<'claim, 'dispatch> {
    storage: &'claim WaveQkvAttentionOutputTileStorageV6<'dispatch>,
    lane: usize,
    position: usize,
    head: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl AttentionHeadTileV6<'_, '_> {
    pub fn lane(&self) -> usize {
        self.lane
    }

    /// This claim owns only this one query head, never another head's output.
    pub fn head(&self) -> usize {
        self.head
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn reject(&mut self) {
        *self.valid = false;
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn query(&self, head: usize, half: usize) -> Option<u16> {
        if !*self.valid || self.lane >= WAVE_LANES || head >= 16 || head != self.head || half >= 2 {
            return None;
        }
        Some(unsafe { (*self.storage.query)[head * HEAD_WIDTH + self.lane + half * WAVE_LANES] })
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn page(&self, token: usize) -> Option<u32> {
        if !*self.valid
            || self.lane >= WAVE_LANES
            || token > self.position
            || token >= CACHE_CAPACITY
        {
            return None;
        }
        // SAFETY: immutable initialized metadata is retained for this claim.
        Some(unsafe { (*self.storage.cache_metadata)[1 + token / 16] })
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn cache_index(
        &self,
        head: usize,
        token: usize,
        physical_page: u32,
        half: usize,
    ) -> Option<usize> {
        if !*self.valid
            || self.lane >= WAVE_LANES
            || head >= 16
            || head != self.head
            || half >= 2
            || token > self.position
            || token >= CACHE_CAPACITY
            || physical_page >= PAGE_COUNT as u32
        {
            return None;
        }
        // Bind the caller's explicitly broadcast page to this logical token.
        // No caller-selected physical row can bypass the immutable page table.
        if unsafe { (*self.storage.cache_metadata)[1 + token / 16] } != physical_page {
            return None;
        }
        Some(
            (physical_page as usize * 16 + token % 16) * 512
                + (head / 4) * HEAD_WIDTH
                + self.lane
                + half * WAVE_LANES,
        )
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn key(&self, head: usize, token: usize, physical_page: u32, half: usize) -> Option<u16> {
        let index = match self.cache_index(head, token, physical_page, half) {
            Some(index) => index,
            None => return None,
        };
        // SAFETY: Post and all projections were acquired before this view;
        // cache_index proves a causal initialized element within this root.
        Some(unsafe { (*self.storage.key_cache)[index] })
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn value(&self, head: usize, token: usize, physical_page: u32, half: usize) -> Option<u16> {
        let index = match self.cache_index(head, token, physical_page, half) {
            Some(index) => index,
            None => return None,
        };
        Some(unsafe { (*self.storage.value_cache)[index] })
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_head(&mut self, head: usize, low: u16, high: u16) -> bool {
        if !*self.valid
            || self.lane >= WAVE_LANES
            || head >= 16
            || head != self.head
            || *self.written != 0
        {
            *self.valid = false;
            return false;
        }
        let index = head * HEAD_WIDTH + self.lane;
        // SAFETY: one private head claim and injective lane/half coordinates
        // cover that head exactly once; no claim can write a different head.
        unsafe {
            (*self.storage.attention_output)[index] = low;
            (*self.storage.attention_output)[index + WAVE_LANES] = high;
        }
        *self.written += 2;
        true
    }
}

/// Dependency-acquired attention reader and rank-local FP32 O-partial writer.
/// Only lane0 writes, in row order; every lane still participates in every dot.
/// The private claim prevents reads before Attention's publication is acquired.
pub struct OutputProjectionTileV6<'claim, 'dispatch> {
    storage: &'claim WaveQkvAttentionOutputTileStorageV6<'dispatch>,
    lane: usize,
    base: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl OutputProjectionTileV6<'_, '_> {
    pub fn lane(&self) -> usize {
        self.lane
    }

    pub fn reject(&mut self) {
        *self.valid = false;
    }

    pub fn input(&self, inner: usize) -> Option<u16> {
        if !*self.valid || inner >= ATTENTION_ELEMENTS {
            return None;
        }
        // SAFETY: positive Acquire of Attention DONE precedes view issuance.
        Some(unsafe { (*self.storage.attention_output)[inner] })
    }

    pub fn weight(&self, column: usize, inner: usize) -> Option<u16> {
        if !*self.valid || column >= TILE_ROWS || inner >= ATTENTION_ELEMENTS {
            return None;
        }
        // SAFETY: bounded row/column indices into the immutable rank shard.
        Some(unsafe {
            (*self.storage.output_weight)[(self.base + column) * ATTENTION_ELEMENTS + inner]
        })
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_output(&mut self, column: usize, value: f32) -> bool {
        if !*self.valid || self.lane != 0 || column != *self.written || column >= TILE_ROWS {
            *self.valid = false;
            return false;
        }
        // SAFETY: disjoint 64-row tiles, lane0 and monotonic cursors exclude aliases.
        // Keep the FP32 partial intact for the later TP sum, without narrowing.
        unsafe {
            (*self.storage.output_partial)[self.base + column] = value;
        }
        *self.written += 1;
        true
    }
}

#[cfg(test)]
#[path = "wave_qkv_attention_output_tiles_v6_tests.rs"]
mod tests;
