//! Engineering-only RMSNorm -> QKV -> head norm/RoPE/append -> paged attention.
//!
//! This distinct thirteen-root profile leaves Key V1, QKV V2 and Post V3 unchanged.
//! It requires a new authenticated compiler/layout/descriptor/runtime route;
//! storage construction alone grants no production launch or coherence authority.
//! One post task acquires every projection before exposing any raw QKV reads.
//! One attention task acquires Post before reading query and the completed cache.

use core::{
    marker::PhantomData,
    mem::align_of,
    sync::atomic::{AtomicU32, Ordering},
};

use super::{
    CLAIMED, Claim, DONE, DUPLICATE, EPOCH, ERRORS, FiniteJoinWorkerResult, INVALID,
    MISSING_PREDECESSOR, OWNERS, READY, STALE_EPOCH,
};
use crate::{WorkgroupLdsScope, WorkgroupPipeline, thread};

pub const WAVE_LANES: usize = 64;
pub const NORM_ELEMENTS: usize = 4096;
pub const QKV_COLUMNS: usize = 3072;
pub const QKV_CHUNK: usize = 256;
pub const QKV_ELEMENTS: usize = NORM_ELEMENTS * QKV_COLUMNS;
pub const TASK_COUNT: u32 = 15;
pub const HEAD_WIDTH: usize = 128;
pub const QUERY_ELEMENTS: usize = 2048;
pub const ATTENTION_ELEMENTS: usize = QUERY_ELEMENTS;
pub const HEAD_WEIGHT_ELEMENTS: usize = 256;
pub const ROTARY_ELEMENTS: usize = 128;
pub const CACHE_CAPACITY: usize = 2304;
pub const PAGE_COUNT: usize = 144;
pub const CACHE_METADATA_WORDS: usize = 1 + PAGE_COUNT;
pub const CACHE_ELEMENTS: usize = CACHE_CAPACITY * 512;
pub const WAVE_STATE_WORDS: usize = 6 + TASK_COUNT as usize;
pub const INCOMPLETE_WRITES: u32 = 16;
// These are bounded polling opportunities, not a cross-workgroup progress
// guarantee. A lone scheduled worker needs fifteen successful rounds; the
// remaining rounds permit idle/contention retries without permanent retirement.
pub const MAX_ROUNDS: u32 = 32;
const POLLS_PER_ROUND: u32 = 256;
const ALL_TASKS: u32 = (1 << TASK_COUNT) - 1;
const QKV_PREDECESSORS: u32 = 0x1fff;
const PROJECTION_TASKS: u32 = 0x1ffe;
const POST_TASK: u32 = 13;
const POST_BIT: u32 = 1 << POST_TASK;
const ATTENTION_TASK: u32 = 14;
const ATTENTION_BIT: u32 = 1 << ATTENTION_TASK;
const ATTENTION_PREDECESSORS: u32 = QKV_PREDECESSORS | POST_BIT;
const ARRIVALS: usize = 6;

/// A thirteen-root one-shot storage binding, not a compiler-issued capability.
///
/// No field, pointer getter, Clone, or Copy implementation is public. Even a
/// correctly sized value grants no protected/runtime coherence permission.
#[must_use]
#[repr(C)]
pub struct WaveQkvAttentionTaskStorageV4<'dispatch> {
    input: *const [u16; NORM_ELEMENTS],
    norm_weight: *const [u16; NORM_ELEMENTS],
    qkv_weight: *const [u16; QKV_ELEMENTS],
    head_norm_weight: *const [u16; HEAD_WEIGHT_ELEMENTS],
    rotary: *const [f32; ROTARY_ELEMENTS],
    cache_metadata: *const [u32; CACHE_METADATA_WORDS],
    normalized: *mut [u16; NORM_ELEMENTS],
    qkv_output: *mut [u16; QKV_COLUMNS],
    query: *mut [u16; QUERY_ELEMENTS],
    key_cache: *mut [u16; CACHE_ELEMENTS],
    value_cache: *mut [u16; CACHE_ELEMENTS],
    attention_output: *mut [u16; ATTENTION_ELEMENTS],
    state: *const [AtomicU32; WAVE_STATE_WORDS],
    _lifetime: PhantomData<(
        &'dispatch [u16],
        &'dispatch mut [u16],
        &'dispatch [AtomicU32],
    )>,
    _not_send_sync: PhantomData<*mut ()>,
}

fn disjoint_regions(regions: [(usize, usize, usize); 13]) -> bool {
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

impl<'dispatch> WaveQkvAttentionTaskStorageV4<'dispatch> {
    /// Binds exact engineering storage without claiming a production issuer.
    ///
    /// # Safety
    /// Every root must designate its exact array in one live allocation. All
    /// thirteen byte ranges must be disjoint and valid for the complete dispatch.
    /// Read roots are initialized and immutable; output roots are writable.
    /// Packed weights/output use contiguous rows `[Q2048; K512; V512]` for
    /// one TP2 rank. Head weights are Q128 then K128; rotary is cos64 then
    /// sin64. Metadata is position then a permutation of 144 page IDs for
    /// 16-token pages. Caches contain 2304 rows of 512 BF16 words. All cache
    /// words are initialized; prior history is immutable and only this dispatch's
    /// selected row is writable by Post. Attention reads only causal rows after
    /// Post acquisition and writes sixteen contiguous 128-word query heads.
    /// Before any invocation starts, initialize the 21-word state
    /// to zero except epoch word0 and ready word1, which must both equal1,
    /// with initialization happening-before all invocations. Do not reinitialize
    /// or reuse storage/epoch until all invocations quiesce, including errors.
    /// Exactly two full WG64 groups participate, with one invocation per physical
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
        normalized: *mut [u16; NORM_ELEMENTS],
        qkv_output: *mut [u16; QKV_COLUMNS],
        query: *mut [u16; QUERY_ELEMENTS],
        key_cache: *mut [u16; CACHE_ELEMENTS],
        value_cache: *mut [u16; CACHE_ELEMENTS],
        attention_output: *mut [u16; ATTENTION_ELEMENTS],
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
            normalized,
            qkv_output,
            query,
            key_cache,
            value_cache,
            attention_output,
            state,
            _lifetime: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn cache_slot(&self) -> Option<usize> {
        self.cache_slot_impl::<false>()
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
fn predecessor_mask(task: u32) -> u32 {
    if task == 0 {
        0
    } else if task == POST_TASK {
        QKV_PREDECESSORS
    } else if task == ATTENTION_TASK {
        ATTENTION_PREDECESSORS
    } else {
        1
    }
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn claim_fanout(state: &[AtomicU32; WAVE_STATE_WORDS], worker: u32) -> Claim {
    if worker > 1 {
        state[ERRORS].fetch_or(INVALID, Ordering::Relaxed);
        return Claim::Rejected(INVALID);
    }
    if state[EPOCH].load(Ordering::Acquire) != 1 {
        state[ERRORS].fetch_or(STALE_EPOCH, Ordering::Relaxed);
        return Claim::Rejected(STALE_EPOCH);
    }
    let snapshot = state[READY].load(Ordering::Acquire);
    if snapshot & !ALL_TASKS != 0 {
        state[ERRORS].fetch_or(INVALID, Ordering::Relaxed);
        return Claim::Rejected(INVALID);
    }
    if snapshot == 0 {
        return Claim::Empty;
    }
    // Only the leader selects a task. Keep this region acyclic so all lanes
    // reconverge before the later workgroup exchange. The guards above make
    // bit 14 the only possible remaining bit in the final branch.
    let task = if snapshot & 1 != 0 {
        0
    } else if snapshot & (1 << 1) != 0 {
        1
    } else if snapshot & (1 << 2) != 0 {
        2
    } else if snapshot & (1 << 3) != 0 {
        3
    } else if snapshot & (1 << 4) != 0 {
        4
    } else if snapshot & (1 << 5) != 0 {
        5
    } else if snapshot & (1 << 6) != 0 {
        6
    } else if snapshot & (1 << 7) != 0 {
        7
    } else if snapshot & (1 << 8) != 0 {
        8
    } else if snapshot & (1 << 9) != 0 {
        9
    } else if snapshot & (1 << 10) != 0 {
        10
    } else if snapshot & (1 << 11) != 0 {
        11
    } else if snapshot & (1 << 12) != 0 {
        12
    } else if snapshot & (1 << 13) != 0 {
        13
    } else {
        14
    };
    let bit = 1 << task;
    if state[READY].fetch_and(!bit, Ordering::AcqRel) & bit == 0 {
        return Claim::Contended;
    }
    if state[CLAIMED].fetch_or(bit, Ordering::AcqRel) & bit != 0 {
        state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
        return Claim::Rejected(DUPLICATE);
    }
    let predecessors = predecessor_mask(task);
    if state[DONE].load(Ordering::Acquire) & predecessors != predecessors {
        state[ERRORS].fetch_or(MISSING_PREDECESSOR, Ordering::Relaxed);
        return Claim::Rejected(MISSING_PREDECESSOR);
    }
    state[OWNERS].fetch_or((worker + 1) << (2 * task), Ordering::Relaxed);
    Claim::Task(task)
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn lane_admitted(state: &[AtomicU32; WAVE_STATE_WORDS], token: u32, worker: u32) -> bool {
    if token == 0 {
        return true;
    }
    if token > TASK_COUNT || worker > 1 {
        return false;
    }
    let task = token - 1;
    state[EPOCH].load(Ordering::Acquire) == 1
        && state[CLAIMED].load(Ordering::Acquire) & (1 << task) != 0
        && (state[OWNERS].load(Ordering::Relaxed) >> (2 * task)) & 3 == worker + 1
        && state[DONE].load(Ordering::Acquire) & predecessor_mask(task) == predecessor_mask(task)
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn complete_lane(state: &[AtomicU32; WAVE_STATE_WORDS], task: u32) -> Result<(), u32> {
    if task >= TASK_COUNT {
        return Err(INVALID);
    }
    let old_arrivals = state[ARRIVALS + task as usize].fetch_add(1, Ordering::AcqRel);
    if old_arrivals >= WAVE_LANES as u32 {
        state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
        return Err(DUPLICATE);
    }
    // The final arriving lane, not necessarily lane0, acquires the entire RMW
    // chain before releasing successors. Every payload access precedes arrival.
    if old_arrivals == WAVE_LANES as u32 - 1 {
        let bit = 1 << task;
        let old_done = state[DONE].fetch_or(bit, Ordering::AcqRel);
        if old_done & bit != 0 {
            state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
            return Err(DUPLICATE);
        }
        if task == 0 {
            state[READY].fetch_or(PROJECTION_TASKS, Ordering::Release);
        } else if task < POST_TASK && (old_done | bit) & QKV_PREDECESSORS == QKV_PREDECESSORS {
            state[READY].fetch_or(POST_BIT, Ordering::Release);
        } else if task == POST_TASK {
            state[READY].fetch_or(ATTENTION_BIT, Ordering::Release);
        }
    }
    Ok(())
}

fn complete_coverage(token: u32, lane: usize, written: usize, valid: bool) -> bool {
    if !valid || token > TASK_COUNT || lane >= WAVE_LANES {
        return false;
    }
    let expected = if token == 1 {
        WAVE_LANES
    } else if token == 14 {
        48
    } else if token == 15 {
        32
    } else if token >= 2 && lane == 0 {
        QKV_CHUNK
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
        && thread::grid_dim_x() == 2
        && thread::grid_dim_y() == 1
        && thread::grid_dim_z() == 1
        && thread::launch_extent_1d() == 128
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
    // Inactive slots perform no atomics. This preserves the attempt budget
    // while giving checked lowering one fixed induction header and exit.
    while polls < POLLS_PER_ROUND {
        if lane == 0 && !*retired && token == 0 {
            let errors = state[ERRORS].load(Ordering::Acquire);
            if errors != 0 {
                result.error |= errors;
                *retired = true;
            } else if state[DONE].load(Ordering::Acquire) == ALL_TASKS {
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
    token
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
    } else if token != 0 {
        if let Err(error) = complete_lane(state, token - 1) {
            result.error |= error;
            *retired = true;
        } else {
            result.executed_tasks += 1;
        }
    }
}

// Keep task construction and callback invocation in the same arm. In
// particular, Post metadata validation must not put a loop before a shared
// Option<task> merge that the callback then has to rediscriminate.
#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn execute_task_at_v4<'dispatch, const WAVE: bool>(
    storage: &WaveQkvAttentionTaskStorageV4<'dispatch>,
    token: u32,
    lane: usize,
    written: &mut usize,
    valid: &mut bool,
    execute: &mut impl for<'claim> FnMut(WaveQkvAttentionTaskV4<'claim, 'dispatch>),
) {
    if !*valid {
        return;
    }
    match token {
        1 => execute(WaveQkvAttentionTaskV4::Norm(QkvAttentionNormTaskV4 {
            storage,
            lane,
            written,
            valid,
        })),
        2..=13 => execute(WaveQkvAttentionTaskV4::Projection(
            QkvAttentionProjectionTaskV4 {
                storage,
                lane,
                base: (token as usize - 2) * QKV_CHUNK,
                written,
                valid,
            },
        )),
        14 => {
            if let Some(slot) = storage.cache_slot_impl::<WAVE>() {
                execute(WaveQkvAttentionTaskV4::Post(AttentionReadyTaskV4 {
                    storage,
                    lane,
                    slot,
                    written,
                    valid,
                }));
            } else {
                *valid = false;
            }
        }
        15 => {
            // Position and validation are full-wave only on run's private path.
            // A scalar task() call never acquires hidden subgroup collectives.
            let position = storage.metadata_word::<WAVE>(0) as usize;
            if position < CACHE_CAPACITY && matches!(storage.cache_slot_impl::<WAVE>(), Some(_)) {
                execute(WaveQkvAttentionTaskV4::Attention(AttentionTaskV4 {
                    storage,
                    lane,
                    position,
                    written,
                    valid,
                }));
            } else {
                *valid = false;
            }
        }
        _ => {}
    }
}

/// Uniform local rounds around this profile's bounded global arbitration.
/// No API turns a user-provided task number into a write witness.
pub struct WaveQkvAttentionWorkerV4<'group, 'dispatch> {
    storage: WaveQkvAttentionTaskStorageV4<'dispatch>,
    pipeline: WorkgroupPipeline<'group, u32, 2, 64, 1>,
    lane: usize,
    worker: u32,
    retired: bool,
    result: FiniteJoinWorkerResult,
}

impl<'group, 'dispatch> WaveQkvAttentionWorkerV4<'group, 'dispatch> {
    /// Uses real physical lane coordinates only to select lane participation,
    /// never as a RowStriped2D/Index1D tensor write capability.
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn new(
        storage: WaveQkvAttentionTaskStorageV4<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
    ) -> Result<Self, u32> {
        if !launch_dimensions_valid() {
            return Err(INVALID);
        }
        let global = thread::index_1d().get();
        if global >= 128 {
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
        storage: WaveQkvAttentionTaskStorageV4<'dispatch>,
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

    /// Consumes fresh dispatch storage and runs every bounded round, including
    /// idle and retired rounds. A callback receives only a claim-borrowed task;
    /// it cannot retain the view or take over the round's completion.
    ///
    /// This is not a resume operation on an existing worker. Storage retains
    /// the one-shot construction and workgroup convergence obligations above.
    ///
    /// ```compile_fail,E0382
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_qkv_attention_tasks_v4::{WaveQkvAttentionTaskStorageV4, WaveQkvAttentionWorkerV4}};
    /// fn reuse<'g, 'd>(storage: WaveQkvAttentionTaskStorageV4<'d>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = WaveQkvAttentionWorkerV4::run(storage, scope, |_| {});
    ///     let _ = WaveQkvAttentionWorkerV4::run(storage, scope, |_| {});
    /// }
    /// ```
    ///
    /// ```compile_fail,E0599
    /// use fe2o3_device::finite_join::wave_qkv_attention_tasks_v4::WaveQkvAttentionWorkerV4;
    /// fn restart(worker: &mut WaveQkvAttentionWorkerV4<'_, '_>) {
    ///     let _ = worker.run(|_| {});
    /// }
    /// ```
    ///
    /// ```compile_fail,E0521
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_qkv_attention_tasks_v4::{WaveQkvAttentionTaskStorageV4, WaveQkvAttentionTaskV4, WaveQkvAttentionWorkerV4}};
    /// fn escape<'g, 'd, 'saved>(storage: WaveQkvAttentionTaskStorageV4<'d>, scope: &mut WorkgroupLdsScope<'g>, saved: &mut Option<WaveQkvAttentionTaskV4<'saved, 'd>>) {
    ///     let _ = WaveQkvAttentionWorkerV4::run(storage, scope, |task| *saved = Some(task));
    /// }
    /// ```
    ///
    /// ```compile_fail,E0061
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_qkv_attention_tasks_v4::{WaveQkvAttentionTaskStorageV4, WaveQkvAttentionWorkerV4}};
    /// fn phase<'g, 'd>(storage: WaveQkvAttentionTaskStorageV4<'d>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = WaveQkvAttentionWorkerV4::run(storage, scope, |_| {}, 7usize);
    /// }
    /// ```
    ///
    /// ```compile_fail,E0624
    /// use fe2o3_device::finite_join::wave_qkv_attention_tasks_v4::WaveQkvAttentionWorkerV4;
    /// fn private_phase(worker: &mut WaveQkvAttentionWorkerV4<'_, '_>) {
    ///     let _ = worker.begin_at(7, 0);
    /// }
    /// ```
    ///
    /// ```compile_fail,E0599
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_qkv_attention_tasks_v4::{WaveQkvAttentionTaskStorageV4, WaveQkvAttentionWorkerV4}};
    /// fn finish<'g, 'd>(storage: WaveQkvAttentionTaskStorageV4<'d>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = WaveQkvAttentionWorkerV4::run(storage, scope, |task| task.finish());
    /// }
    /// ```
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn run(
        storage: WaveQkvAttentionTaskStorageV4<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
        mut execute: impl for<'claim> FnMut(WaveQkvAttentionTaskV4<'claim, 'dispatch>),
    ) -> Result<FiniteJoinWorkerResult, u32> {
        if !launch_dimensions_valid() {
            return Err(INVALID);
        }
        let global = thread::index_1d().get();
        if global >= 128 {
            return Err(INVALID);
        }
        let lane = global % 64;
        let mut worker = Self::from_coordinates(storage, scope, lane, (global / 64) as u32);
        let mut round_index = 0u32;
        while round_index < MAX_ROUNDS {
            let phase = (round_index as usize) * 3;
            let mut round = worker.begin_at(phase, lane);
            // begin_at broadcasts token and admission validity to all 64 lanes;
            // each valid Post/Attention dispatch enters metadata collectives
            // with a full wave. Different workgroups may choose different tasks.
            execute_task_at_v4::<true>(
                &round.worker.storage,
                round.token,
                lane,
                &mut round.written,
                &mut round.valid,
                &mut execute,
            );
            round.finish_at(phase, lane);
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
            let mut lane = 1;
            while lane < WAVE_LANES {
                result |= self.pipeline.read(phase, lane);
                lane += 1;
            }
        }
        self.pipeline.release(phase);
        // Return one full-wave value while preserving every integer payload bit.
        let subgroup = crate::gfx950::Gfx950Subgroup::current();
        subgroup
            .broadcast_f32::<64>(f32::from_bits(result), 0)
            .to_bits()
    }

    /// Every lane calls this and finishes the returned round, even when idle.
    /// The fixed round bound makes termination workgroup-uniform. A locally
    /// retired leader still participates in all remaining pipeline phases.
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn next_round(&mut self) -> Option<WaveQkvAttentionRoundV4<'_, 'group, 'dispatch>> {
        if self.result.rounds == MAX_ROUNDS {
            return None;
        }
        let phase = self.result.rounds as usize * 3;
        Some(self.begin_at(phase, self.lane))
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn begin_at(
        &mut self,
        phase: usize,
        lane: usize,
    ) -> WaveQkvAttentionRoundV4<'_, 'group, 'dispatch> {
        let token = begin_round_claim(
            self.storage.state(),
            lane,
            self.worker,
            &mut self.retired,
            &mut self.result,
        );
        let token = self.exchange(phase, lane, token, true);
        let invalid = (!lane_admitted(self.storage.state(), token, self.worker)) as u32;
        // A stale dependency observation must reject uniformly BEFORE any
        // numerical subgroup collective. Do not let just one lane skip it.
        let valid = self.exchange(phase + 1, lane, invalid, false) == 0;
        if !valid {
            self.storage.state()[ERRORS].fetch_or(MISSING_PREDECESSOR, Ordering::Relaxed);
        }
        WaveQkvAttentionRoundV4 {
            worker: self,
            token,
            valid,
            written: 0,
            phase,
        }
    }

    /// Local observations only, not output access or dispatch completion.
    pub fn observations(&self) -> FiniteJoinWorkerResult {
        self.result
    }
}

/// Move-only round; finishing consumes the value and ends every view borrow.
///
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_qkv_attention_tasks_v4::WaveQkvAttentionRoundV4;
/// fn twice(round: WaveQkvAttentionRoundV4<'_, '_, '_>) { round.finish(); round.finish(); }
/// ```
#[must_use = "all participating lanes must finish every issued round"]
pub struct WaveQkvAttentionRoundV4<'worker, 'group, 'dispatch> {
    worker: &'worker mut WaveQkvAttentionWorkerV4<'group, 'dispatch>,
    token: u32,
    valid: bool,
    written: usize,
    phase: usize,
}

pub enum WaveQkvAttentionTaskV4<'claim, 'dispatch> {
    Norm(QkvAttentionNormTaskV4<'claim, 'dispatch>),
    Projection(QkvAttentionProjectionTaskV4<'claim, 'dispatch>),
    Post(AttentionReadyTaskV4<'claim, 'dispatch>),
    Attention(AttentionTaskV4<'claim, 'dispatch>),
}

impl<'dispatch> WaveQkvAttentionRoundV4<'_, '_, 'dispatch> {
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn task(&mut self) -> Option<WaveQkvAttentionTaskV4<'_, 'dispatch>> {
        self.task_at(self.worker.lane)
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn task_at(&mut self, lane: usize) -> Option<WaveQkvAttentionTaskV4<'_, 'dispatch>> {
        if !self.valid {
            return None;
        }
        let storage = &self.worker.storage;
        match self.token {
            1 => Some(WaveQkvAttentionTaskV4::Norm(QkvAttentionNormTaskV4 {
                storage,
                lane,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            2..=13 => Some(WaveQkvAttentionTaskV4::Projection(
                QkvAttentionProjectionTaskV4 {
                    storage,
                    lane,
                    base: (self.token as usize - 2) * QKV_CHUNK,
                    written: &mut self.written,
                    valid: &mut self.valid,
                },
            )),
            14 => {
                let Some(slot) = storage.cache_slot() else {
                    self.valid = false;
                    return None;
                };
                Some(WaveQkvAttentionTaskV4::Post(AttentionReadyTaskV4 {
                    storage,
                    lane,
                    slot,
                    written: &mut self.written,
                    valid: &mut self.valid,
                }))
            }
            15 => {
                let position = storage.metadata_word::<false>(0) as usize;
                if position >= CACHE_CAPACITY || matches!(storage.cache_slot(), None) {
                    self.valid = false;
                    return None;
                }
                Some(WaveQkvAttentionTaskV4::Attention(AttentionTaskV4 {
                    storage,
                    lane,
                    position,
                    written: &mut self.written,
                    valid: &mut self.valid,
                }))
            }
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

/// Claim-borrowed norm view: exactly lane+64*component, no arbitrary write index.
///
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_qkv_attention_tasks_v4::QkvAttentionNormTaskV4;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<QkvAttentionNormTaskV4<'static, 'static>>();
/// ```
///
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_qkv_attention_tasks_v4::QkvAttentionNormTaskV4;
/// fn escape<'d>(task: QkvAttentionNormTaskV4<'_, 'd>) -> QkvAttentionNormTaskV4<'static, 'd> { task }
/// ```
///
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_qkv_attention_tasks_v4::QkvAttentionNormTaskV4;
/// let forged = QkvAttentionNormTaskV4::from_task(0, 0);
/// ```
pub struct QkvAttentionNormTaskV4<'claim, 'dispatch> {
    storage: &'claim WaveQkvAttentionTaskStorageV4<'dispatch>,
    lane: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl QkvAttentionNormTaskV4<'_, '_> {
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

/// Dependency-acquired projection; lane0 owns one of twelve 256-column tiles.
pub struct QkvAttentionProjectionTaskV4<'claim, 'dispatch> {
    storage: &'claim WaveQkvAttentionTaskStorageV4<'dispatch>,
    lane: usize,
    base: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl QkvAttentionProjectionTaskV4<'_, '_> {
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
        if local_column >= QKV_CHUNK || inner >= NORM_ELEMENTS {
            return None;
        }
        let index = (self.base + local_column) * NORM_ELEMENTS + inner;
        // SAFETY: private base is a 256-row tile in the exact 3072-row root.
        Some(unsafe { (*self.storage.qkv_weight)[index] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_column(&mut self, local_column: usize, value: u16) -> bool {
        if !*self.valid
            || self.lane != 0
            || local_column != *self.written
            || local_column >= QKV_CHUNK
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
pub struct AttentionReadyTaskV4<'claim, 'dispatch> {
    storage: &'claim WaveQkvAttentionTaskStorageV4<'dispatch>,
    lane: usize,
    slot: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl AttentionReadyTaskV4<'_, '_> {
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
        // SAFETY: all thirteen predecessors were acquired before view issuance.
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
pub struct AttentionTaskV4<'claim, 'dispatch> {
    storage: &'claim WaveQkvAttentionTaskStorageV4<'dispatch>,
    lane: usize,
    position: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl AttentionTaskV4<'_, '_> {
    pub fn lane(&self) -> usize {
        self.lane
    }

    pub fn position(&self) -> usize {
        self.position
    }

    pub fn reject(&mut self) {
        *self.valid = false;
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn query(&self, head: usize, half: usize) -> Option<u16> {
        if !*self.valid || self.lane >= WAVE_LANES || head >= 16 || half >= 2 {
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
        if !*self.valid || self.lane >= WAVE_LANES || head >= 16 || *self.written != 2 * head {
            *self.valid = false;
            return false;
        }
        let index = head * HEAD_WIDTH + self.lane;
        // SAFETY: one Attention claim plus sequential heads and physical lane
        // coordinates cover every output element exactly once, without aliases.
        unsafe {
            (*self.storage.attention_output)[index] = low;
            (*self.storage.attention_output)[index + WAVE_LANES] = high;
        }
        *self.written += 2;
        true
    }
}

#[cfg(test)]
#[path = "wave_qkv_attention_tasks_v4_tests.rs"]
mod tests;
