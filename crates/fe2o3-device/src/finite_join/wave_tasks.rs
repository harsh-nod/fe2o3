//! Engineering-only Wave64 fan-out on the existing bounded finite scheduler.
//!
//! This source proposal has no checked source import, grouped ABI binder,
//! atomic-coherence issuer, or production launch admission. It does not extend
//! the existing WG128 recipe's proof. Unsafe storage construction is explicit.
//!
//! AMD-only force-inlining uses the pinned nightly's checked MIR inliner to
//! retain these ordinary task bodies in the kernel; it grants no admission.

use core::{
    marker::PhantomData,
    mem::align_of,
    sync::atomic::{AtomicU32, Ordering},
};

use super::{
    ALL_TASKS, CLAIMED, Claim, DONE, DUPLICATE, EPOCH, ERRORS, FiniteJoinWorkerResult, INVALID,
    MAX_EMPTY_PROBES, MAX_ROUNDS, MISSING_PREDECESSOR, OWNERS, READY, claim_ready_task,
};
use crate::{WorkgroupLdsScope, WorkgroupPipeline, thread};

pub const WAVE_LANES: usize = 64;
pub const NORM_ELEMENTS: usize = 4096;
pub const KEY_COLUMNS: usize = 512;
pub const KEY_CHUNK: usize = 256;
pub const KEY_ELEMENTS: usize = NORM_ELEMENTS * KEY_COLUMNS;
pub const WAVE_STATE_WORDS: usize = 9;
pub const INCOMPLETE_WRITES: u32 = 16;
const ARRIVALS: usize = 6;

/// A six-root one-shot storage binding, not a compiler-issued capability.
///
/// No field, pointer getter, Clone, or Copy implementation is public. Even a
/// correctly sized value grants no protected/runtime coherence permission.
#[must_use]
#[repr(C)]
pub struct WaveTaskStorageV1<'dispatch> {
    input: *const [u16; NORM_ELEMENTS],
    norm_weight: *const [u16; NORM_ELEMENTS],
    key_weight: *const [u16; KEY_ELEMENTS],
    normalized: *mut [u16; NORM_ELEMENTS],
    key_output: *mut [u16; KEY_COLUMNS],
    state: *const [AtomicU32; WAVE_STATE_WORDS],
    _lifetime: PhantomData<(
        &'dispatch [u16],
        &'dispatch mut [u16],
        &'dispatch [AtomicU32],
    )>,
    _not_send_sync: PhantomData<*mut ()>,
}

fn disjoint_regions(regions: [(usize, usize, usize); 6]) -> bool {
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

impl<'dispatch> WaveTaskStorageV1<'dispatch> {
    /// Binds exact engineering storage without claiming a production issuer.
    ///
    /// # Safety
    /// Every root must designate its exact array in one live allocation. All
    /// six byte ranges must be disjoint and valid for the complete dispatch.
    /// Read roots are initialized and immutable; output roots are writable.
    /// Before any invocation starts, initialize state to `[1,1,0,0,0,0,0,0,0]`
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
        key_weight: *const [u16; KEY_ELEMENTS],
        normalized: *mut [u16; NORM_ELEMENTS],
        key_output: *mut [u16; KEY_COLUMNS],
        state: *const [AtomicU32; WAVE_STATE_WORDS],
    ) -> Result<Self, u32> {
        if !disjoint_regions([
            (input as usize, NORM_ELEMENTS * 2, align_of::<u16>()),
            (norm_weight as usize, NORM_ELEMENTS * 2, align_of::<u16>()),
            (key_weight as usize, KEY_ELEMENTS * 2, align_of::<u16>()),
            (normalized as usize, NORM_ELEMENTS * 2, align_of::<u16>()),
            (key_output as usize, KEY_COLUMNS * 2, align_of::<u16>()),
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
            key_weight,
            normalized,
            key_output,
            state,
            _lifetime: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    fn state(&self) -> &[AtomicU32; WAVE_STATE_WORDS] {
        // SAFETY: engineering construction retains aligned, initialized atomic
        // storage. This reference never aliases an ordinary payload root.
        unsafe { &*self.state }
    }
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn claim_fanout(state: &[AtomicU32; WAVE_STATE_WORDS], worker: u32) -> Claim {
    let claimed = claim_ready_task(
        &state[EPOCH],
        &state[READY],
        &state[CLAIMED],
        &state[ERRORS],
        1,
    );
    let Claim::Task(task) = claimed else {
        return claimed;
    };
    if task != 0 && state[DONE].load(Ordering::Acquire) & 1 == 0 {
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
    if token > 3 || worker > 1 {
        return false;
    }
    let task = token - 1;
    state[EPOCH].load(Ordering::Acquire) == 1
        && state[CLAIMED].load(Ordering::Acquire) & (1 << task) != 0
        && (state[OWNERS].load(Ordering::Relaxed) >> (2 * task)) & 3 == worker + 1
        && (task == 0 || state[DONE].load(Ordering::Acquire) & 1 != 0)
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn complete_lane(state: &[AtomicU32; WAVE_STATE_WORDS], task: u32) -> Result<(), u32> {
    if task >= 3 {
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
            state[READY].fetch_or(6, Ordering::Release);
        }
    }
    Ok(())
}

fn complete_coverage(token: u32, lane: usize, written: usize, valid: bool) -> bool {
    if !valid || token > 3 || lane >= WAVE_LANES {
        return false;
    }
    let expected = if token == 1 {
        WAVE_LANES
    } else if (token == 2 || token == 3) && lane == 0 {
        KEY_CHUNK
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
    if lane == 0 && !*retired {
        if state[DONE].load(Ordering::Acquire) == ALL_TASKS {
            *retired = true;
        } else {
            match claim_fanout(state, worker) {
                Claim::Task(task) => token = task + 1,
                Claim::Empty => {
                    result.empty_probes += 1;
                    *retired = result.empty_probes == MAX_EMPTY_PROBES;
                }
                Claim::Contended => {}
                Claim::Rejected(error) => {
                    result.error |= error;
                    *retired = true;
                }
            }
        }
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

/// Uniform local rounds around the existing bounded global arbitration.
/// No API turns a user-provided task number into a write witness.
pub struct WaveWorkerV1<'group, 'dispatch> {
    storage: WaveTaskStorageV1<'dispatch>,
    pipeline: WorkgroupPipeline<'group, u32, 2, 64, 1>,
    lane: usize,
    worker: u32,
    retired: bool,
    result: FiniteJoinWorkerResult,
}

impl<'group, 'dispatch> WaveWorkerV1<'group, 'dispatch> {
    /// Uses real physical lane coordinates only to select lane participation,
    /// never as a RowStriped2D/Index1D tensor write capability.
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn new(
        storage: WaveTaskStorageV1<'dispatch>,
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
        storage: WaveTaskStorageV1<'dispatch>,
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
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_tasks::{WaveTaskStorageV1, WaveWorkerV1}};
    /// fn reuse<'g, 'd>(storage: WaveTaskStorageV1<'d>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = WaveWorkerV1::run(storage, scope, |_| {});
    ///     let _ = WaveWorkerV1::run(storage, scope, |_| {});
    /// }
    /// ```
    ///
    /// ```compile_fail,E0599
    /// use fe2o3_device::finite_join::wave_tasks::WaveWorkerV1;
    /// fn restart(worker: &mut WaveWorkerV1<'_, '_>) {
    ///     let _ = worker.run(|_| {});
    /// }
    /// ```
    ///
    /// ```compile_fail,E0521
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_tasks::{WaveTaskStorageV1, WaveTaskV1, WaveWorkerV1}};
    /// fn escape<'g, 'd, 'saved>(storage: WaveTaskStorageV1<'d>, scope: &mut WorkgroupLdsScope<'g>, saved: &mut Option<WaveTaskV1<'saved, 'd>>) {
    ///     let _ = WaveWorkerV1::run(storage, scope, |task| *saved = Some(task));
    /// }
    /// ```
    ///
    /// ```compile_fail,E0061
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_tasks::{WaveTaskStorageV1, WaveWorkerV1}};
    /// fn phase<'g, 'd>(storage: WaveTaskStorageV1<'d>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = WaveWorkerV1::run(storage, scope, |_| {}, 7usize);
    /// }
    /// ```
    ///
    /// ```compile_fail,E0624
    /// use fe2o3_device::finite_join::wave_tasks::WaveWorkerV1;
    /// fn private_phase(worker: &mut WaveWorkerV1<'_, '_>) {
    ///     let _ = worker.begin_at(7, 0);
    /// }
    /// ```
    ///
    /// ```compile_fail,E0599
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::wave_tasks::{WaveTaskStorageV1, WaveWorkerV1}};
    /// fn finish<'g, 'd>(storage: WaveTaskStorageV1<'d>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = WaveWorkerV1::run(storage, scope, |task| task.finish());
    /// }
    /// ```
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn run(
        storage: WaveTaskStorageV1<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
        mut execute: impl for<'claim> FnMut(WaveTaskV1<'claim, 'dispatch>),
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
            if let Some(task) = round.task_at(lane) {
                execute(task);
            }
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
    pub fn next_round(&mut self) -> Option<WaveRoundV1<'_, 'group, 'dispatch>> {
        if self.result.rounds == MAX_ROUNDS {
            return None;
        }
        let phase = self.result.rounds as usize * 3;
        Some(self.begin_at(phase, self.lane))
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn begin_at(&mut self, phase: usize, lane: usize) -> WaveRoundV1<'_, 'group, 'dispatch> {
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
        WaveRoundV1 {
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
/// use fe2o3_device::finite_join::wave_tasks::WaveRoundV1;
/// fn twice(round: WaveRoundV1<'_, '_, '_>) { round.finish(); round.finish(); }
/// ```
#[must_use = "all participating lanes must finish every issued round"]
pub struct WaveRoundV1<'worker, 'group, 'dispatch> {
    worker: &'worker mut WaveWorkerV1<'group, 'dispatch>,
    token: u32,
    valid: bool,
    written: usize,
    phase: usize,
}

pub enum WaveTaskV1<'claim, 'dispatch> {
    Norm(NormTaskV1<'claim, 'dispatch>),
    Key(KeyTaskV1<'claim, 'dispatch>),
}

impl<'dispatch> WaveRoundV1<'_, '_, 'dispatch> {
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn task(&mut self) -> Option<WaveTaskV1<'_, 'dispatch>> {
        self.task_at(self.worker.lane)
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn task_at(&mut self, lane: usize) -> Option<WaveTaskV1<'_, 'dispatch>> {
        if !self.valid {
            return None;
        }
        let storage = &self.worker.storage;
        match self.token {
            1 => Some(WaveTaskV1::Norm(NormTaskV1 {
                storage,
                lane,
                written: &mut self.written,
                valid: &mut self.valid,
            })),
            2 | 3 => Some(WaveTaskV1::Key(KeyTaskV1 {
                storage,
                lane,
                base: (self.token as usize - 2) * KEY_CHUNK,
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

/// Claim-borrowed norm view: exactly lane+64*component, no arbitrary write index.
///
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_tasks::NormTaskV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<NormTaskV1<'static, 'static>>();
/// ```
///
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_tasks::NormTaskV1;
/// fn escape<'d>(task: NormTaskV1<'_, 'd>) -> NormTaskV1<'static, 'd> { task }
/// ```
///
/// ```compile_fail
/// use fe2o3_device::finite_join::wave_tasks::NormTaskV1;
/// let forged = NormTaskV1::from_task(0, 0);
/// ```
pub struct NormTaskV1<'claim, 'dispatch> {
    storage: &'claim WaveTaskStorageV1<'dispatch>,
    lane: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl NormTaskV1<'_, '_> {
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

/// Dependency-acquired key view; only lane0 owns this task's 256 output columns.
pub struct KeyTaskV1<'claim, 'dispatch> {
    storage: &'claim WaveTaskStorageV1<'dispatch>,
    lane: usize,
    base: usize,
    written: &'claim mut usize,
    valid: &'claim mut bool,
}

impl KeyTaskV1<'_, '_> {
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
        if local_column >= KEY_CHUNK || inner >= NORM_ELEMENTS {
            return None;
        }
        let index = (self.base + local_column) * NORM_ELEMENTS + inner;
        // SAFETY: private base is0 or256, so index is inside the exact K root.
        Some(unsafe { (*self.storage.key_weight)[index] })
    }
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn write_column(&mut self, local_column: usize, value: u16) -> bool {
        if !*self.valid
            || self.lane != 0
            || local_column != *self.written
            || local_column >= KEY_CHUNK
        {
            *self.valid = false;
            return false;
        }
        // SAFETY: the task-bound halves are disjoint and only their lane0 writes.
        unsafe {
            (*self.storage.key_output)[self.base + local_column] = value;
        }
        *self.written += 1;
        true
    }
}

#[cfg(test)]
#[path = "wave_tasks_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "wave_tasks_scheduler_tests.rs"]
mod scheduler_tests;

#[cfg(test)]
#[path = "wave_tasks_run_tests.rs"]
mod run_tests;
