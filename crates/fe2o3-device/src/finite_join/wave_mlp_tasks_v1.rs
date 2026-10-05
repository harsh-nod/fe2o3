//! Engineering-only Norm -> {Gate, Up} -> SwiGLU -> Down finite worker.
//!
//! This separate eleven-root profile does not widen any QKV profile. Five
//! coarse tasks provide a correctness baseline, not a progress/performance
//! guarantee. Down remains FP32; TP reduction and residual addition are external.

use super::{
    CLAIMED, Claim, DONE, DUPLICATE, EPOCH, ERRORS, FiniteJoinWorkerResult, INVALID,
    MISSING_PREDECESSOR, OWNERS, READY, STALE_EPOCH,
};
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
pub const TASK_COUNT: u32 = 5;
pub const WAVE_STATE_WORDS: usize = 6 + TASK_COUNT as usize;
pub const INCOMPLETE_WRITES: u32 = 16;
pub const MAX_ROUNDS: u32 = 32;
const POLLS_PER_ROUND: u32 = 256;
const ALL_TASKS: u32 = 0x1f;
const ARRIVALS: usize = 6;

/// Exact one-shot storage, not a compiler-issued capability or launch authority.
#[must_use]
#[repr(C)]
pub struct WaveMlpTaskStorageV1<'dispatch> {
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

impl<'dispatch> WaveMlpTaskStorageV1<'dispatch> {
    /// Binds engineering storage without issuing production/coherence authority.
    ///
    /// # Safety
    /// All eleven exact array roots must occupy pairwise disjoint, aligned,
    /// live allocations throughout this one-shot dispatch. The five read roots
    /// are initialized and immutable; the five outputs are writable only by
    /// this worker. Gate/up weights are row-major [6144,4096]; down weights are
    /// [4096,6144]. Input is the actual first residual, not its normalization.
    /// Initialize the genuine AtomicU32[11] state to zero except epoch and
    /// ready words0/1, both1, before any invocation. No reset/reuse is permitted
    /// until every invocation quiesces, including rejected/retired execution.
    /// Exactly two WG64 groups participate, one invocation per physical lane.
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
fn predecessor_mask(task: u32) -> u32 {
    if task == 0 {
        0
    } else if task == 1 || task == 2 {
        1
    } else if task == 3 {
        7
    } else {
        15
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
    // Acyclic selector: no leader-only scan loop may surround a later barrier.
    let task = if snapshot & 1 != 0 {
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
    // The final RMW acquires all earlier lanes' writes before successor release.
    if old_arrivals == WAVE_LANES as u32 - 1 {
        let bit = 1 << task;
        let old_done = state[DONE].fetch_or(bit, Ordering::AcqRel);
        if old_done & bit != 0 {
            state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
            return Err(DUPLICATE);
        }
        if task == 0 {
            state[READY].fetch_or(6, Ordering::Release);
        } else if (task == 1 || task == 2) && (old_done | bit) & 7 == 7 {
            state[READY].fetch_or(8, Ordering::Release);
        } else if task == 3 {
            state[READY].fetch_or(16, Ordering::Release);
        }
    }
    Ok(())
}

fn complete_coverage(token: u32, lane: usize, written: usize, valid: bool) -> bool {
    if !valid || token > TASK_COUNT || lane >= WAVE_LANES {
        return false;
    }
    let expected = if token == 1 {
        64
    } else if token == 4 {
        96
    } else if (token == 2 || token == 3) && lane == 0 {
        INTERMEDIATE_ELEMENTS
    } else if token == 5 && lane == 0 {
        OUTPUT_ELEMENTS
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

// Construct and consume each claim in its arm, without an Option/task merge.
#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn execute_task_at_v1<'dispatch>(
    storage: &WaveMlpTaskStorageV1<'dispatch>,
    token: u32,
    lane: usize,
    written: &mut usize,
    valid: &mut bool,
    execute: &mut impl for<'claim> FnMut(WaveMlpTaskV1<'claim, 'dispatch>),
) {
    if !*valid {
        return;
    }
    match token {
        1 => execute(WaveMlpTaskV1::Norm(MlpNormTaskV1 {
            storage,
            lane,
            written,
            valid,
        })),
        2 => execute(WaveMlpTaskV1::Gate(MlpGateTaskV1 {
            storage,
            lane,
            written,
            valid,
        })),
        3 => execute(WaveMlpTaskV1::Up(MlpUpTaskV1 {
            storage,
            lane,
            written,
            valid,
        })),
        4 => execute(WaveMlpTaskV1::SwiGlu(MlpSwiGluTaskV1 {
            storage,
            lane,
            written,
            valid,
        })),
        5 => execute(WaveMlpTaskV1::Down(MlpDownTaskV1 {
            storage,
            lane,
            written,
            valid,
        })),
        _ => {}
    }
}

/// Fixed finite arbitration and uniform three-exchange local rounds.
pub struct WaveMlpWorkerV1<'group, 'dispatch> {
    storage: WaveMlpTaskStorageV1<'dispatch>,
    pipeline: WorkgroupPipeline<'group, u32, 2, 64, 1>,
    lane: usize,
    worker: u32,
    retired: bool,
    result: FiniteJoinWorkerResult,
}

impl<'group, 'dispatch> WaveMlpWorkerV1<'group, 'dispatch> {
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn new(
        storage: WaveMlpTaskStorageV1<'dispatch>,
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
        storage: WaveMlpTaskStorageV1<'dispatch>,
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

    /// Consumes one-shot storage. Idle/retired lanes still finish all32 rounds.
    /// No callback can escape its claim or publish task completion itself.
    ///
    /// ```compile_fail,E0382
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_mlp_tasks_v1::{WaveMlpTaskStorageV1,WaveMlpWorkerV1}};
    /// fn reuse<'g,'d>(s: WaveMlpTaskStorageV1<'d>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = WaveMlpWorkerV1::run(s, scope, |_| {});
    ///     let _ = WaveMlpWorkerV1::run(s, scope, |_| {});
    /// }
    /// ```
    /// ```compile_fail,E0521
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_mlp_tasks_v1::{WaveMlpTaskStorageV1,WaveMlpTaskV1,WaveMlpWorkerV1}};
    /// fn escape<'g,'d,'a>(s: WaveMlpTaskStorageV1<'d>, scope: &mut WorkgroupLdsScope<'g>, saved: &mut Option<WaveMlpTaskV1<'a,'d>>) {
    ///     let _ = WaveMlpWorkerV1::run(s, scope, |task| *saved = Some(task));
    /// }
    /// ```
    /// ```compile_fail,E0599
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_mlp_tasks_v1::{WaveMlpTaskStorageV1,WaveMlpWorkerV1}};
    /// fn finish<'g,'d>(s: WaveMlpTaskStorageV1<'d>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = WaveMlpWorkerV1::run(s, scope, |task| task.finish());
    /// }
    /// ```
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn run(
        storage: WaveMlpTaskStorageV1<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
        mut execute: impl for<'claim> FnMut(WaveMlpTaskV1<'claim, 'dispatch>),
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
            let phase = round_index as usize * 3;
            let mut round = worker.begin_at(phase, lane);
            execute_task_at_v1(
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
    pub fn next_round(&mut self) -> Option<WaveMlpRoundV1<'_, 'group, 'dispatch>> {
        if self.result.rounds == MAX_ROUNDS {
            return None;
        }
        let phase = self.result.rounds as usize * 3;
        Some(self.begin_at(phase, self.lane))
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn begin_at(&mut self, phase: usize, lane: usize) -> WaveMlpRoundV1<'_, 'group, 'dispatch> {
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
        WaveMlpRoundV1 {
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
/// use fe2o3_device::finite_join::wave_mlp_tasks_v1::WaveMlpRoundV1;
/// fn twice(round: WaveMlpRoundV1<'_, '_, '_>) { round.finish(); round.finish(); }
/// ```
#[must_use = "every participating lane must finish each issued round"]
pub struct WaveMlpRoundV1<'worker, 'group, 'dispatch> {
    worker: &'worker mut WaveMlpWorkerV1<'group, 'dispatch>,
    token: u32,
    valid: bool,
    written: usize,
    phase: usize,
}

pub enum WaveMlpTaskV1<'claim, 'dispatch> {
    Norm(MlpNormTaskV1<'claim, 'dispatch>),
    Gate(MlpGateTaskV1<'claim, 'dispatch>),
    Up(MlpUpTaskV1<'claim, 'dispatch>),
    SwiGlu(MlpSwiGluTaskV1<'claim, 'dispatch>),
    Down(MlpDownTaskV1<'claim, 'dispatch>),
}

impl<'dispatch> WaveMlpRoundV1<'_, '_, 'dispatch> {
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn task(&mut self) -> Option<WaveMlpTaskV1<'_, 'dispatch>> {
        self.task_at(self.worker.lane)
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn task_at(&mut self, lane: usize) -> Option<WaveMlpTaskV1<'_, 'dispatch>> {
        if !self.valid {
            return None;
        }
        let storage = &self.worker.storage;
        match self.token {
            1 => Some(WaveMlpTaskV1::Norm(MlpNormTaskV1 {
                storage,
                lane,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            2 => Some(WaveMlpTaskV1::Gate(MlpGateTaskV1 {
                storage,
                lane,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            3 => Some(WaveMlpTaskV1::Up(MlpUpTaskV1 {
                storage,
                lane,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            4 => Some(WaveMlpTaskV1::SwiGlu(MlpSwiGluTaskV1 {
                storage,
                lane,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            5 => Some(WaveMlpTaskV1::Down(MlpDownTaskV1 {
                storage,
                lane,
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
/// use fe2o3_device::finite_join::wave_mlp_tasks_v1::MlpNormTaskV1;
/// fn clone_required<T: Clone>() {}
/// clone_required::<MlpNormTaskV1<'static, 'static>>();
/// ```
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_mlp_tasks_v1::MlpNormTaskV1;
/// let forged = MlpNormTaskV1::from_task(0, 0);
/// ```
pub struct MlpNormTaskV1<'claim, 'dispatch> {
    storage: &'claim WaveMlpTaskStorageV1<'dispatch>,
    lane: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}
impl MlpNormTaskV1<'_, '_> {
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

/// Norm-acquired gate projection; only lane0 writes6144 ordered rows.
pub struct MlpGateTaskV1<'claim, 'dispatch> {
    storage: &'claim WaveMlpTaskStorageV1<'dispatch>,
    lane: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}
impl MlpGateTaskV1<'_, '_> {
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
        if row >= INTERMEDIATE_ELEMENTS || inner >= NORM_ELEMENTS {
            return None;
        }
        Some(unsafe { (*self.storage.gate_weight)[row * NORM_ELEMENTS + inner] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_column(&mut self, row: usize, value: u16) -> bool {
        if !*self.valid || self.lane != 0 || row != *self.written || row >= INTERMEDIATE_ELEMENTS {
            *self.valid = false;
            return false;
        }
        // SAFETY: only this task's lane0 can advance this output cursor.
        unsafe {
            (*self.storage.gate)[row] = value;
        }
        *self.written += 1;
        true
    }
}

/// Norm-acquired up projection, disjoint from gate despite equal geometry.
pub struct MlpUpTaskV1<'claim, 'dispatch> {
    storage: &'claim WaveMlpTaskStorageV1<'dispatch>,
    lane: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}
impl MlpUpTaskV1<'_, '_> {
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
        if row >= INTERMEDIATE_ELEMENTS || inner >= NORM_ELEMENTS {
            return None;
        }
        Some(unsafe { (*self.storage.up_weight)[row * NORM_ELEMENTS + inner] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_column(&mut self, row: usize, value: u16) -> bool {
        if !*self.valid || self.lane != 0 || row != *self.written || row >= INTERMEDIATE_ELEMENTS {
            *self.valid = false;
            return false;
        }
        unsafe {
            (*self.storage.up)[row] = value;
        }
        *self.written += 1;
        true
    }
}

/// Acquires both projections; each lane owns96 activation components.
pub struct MlpSwiGluTaskV1<'claim, 'dispatch> {
    storage: &'claim WaveMlpTaskStorageV1<'dispatch>,
    lane: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}
impl MlpSwiGluTaskV1<'_, '_> {
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

/// Activation-acquired down projection; only lane0 writes ordered FP32 rows.
pub struct MlpDownTaskV1<'claim, 'dispatch> {
    storage: &'claim WaveMlpTaskStorageV1<'dispatch>,
    lane: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}
impl MlpDownTaskV1<'_, '_> {
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
        if row >= OUTPUT_ELEMENTS || inner >= INTERMEDIATE_ELEMENTS {
            return None;
        }
        Some(unsafe { (*self.storage.down_weight)[row * INTERMEDIATE_ELEMENTS + inner] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_output(&mut self, row: usize, value: f32) -> bool {
        if !*self.valid || self.lane != 0 || row != *self.written || row >= OUTPUT_ELEMENTS {
            *self.valid = false;
            return false;
        }
        unsafe {
            (*self.storage.down_partial)[row] = value;
        }
        *self.written += 1;
        true
    }
}

#[cfg(test)]
#[path = "wave_mlp_tasks_v1_tests.rs"]
mod tests;
