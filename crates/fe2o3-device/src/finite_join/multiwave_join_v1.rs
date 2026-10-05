//! Engineering-only useful WG128 producer/producer/join source candidate.
//!
//! Both Wave64s execute ordinary payload work. This does not extend the old
//! `FiniteJoinDispatch128` terminal, its six-word state, or its retained image.
//! Checked source import, exact new ABI/profile, machine visibility review and
//! a same-owner runtime binding are still required; no production issuer exists.

use core::{
    marker::PhantomData,
    mem::align_of,
    sync::atomic::{AtomicU32, Ordering},
};

use super::{
    ALL_TASKS, CLAIMED, Claim, DONE, DUPLICATE, EPOCH, ERRORS, FiniteJoinWorkerResult,
    INPUT_ELEMENTS, INVALID, MAX_ROUNDS, MISSING_PREDECESSOR, OWNERS, PAYLOAD_ELEMENTS, READY,
    STALE_EPOCH, TILE_ELEMENTS, claim_ready_task,
};
use crate::{WorkgroupLdsScope, WorkgroupPipeline, thread};

pub const WORKGROUP_LANES: usize = 128;
pub const WORKGROUPS: usize = 2;
pub const MULTIWAVE_STATE_WORDS: usize = 9;
pub const CONTROL_LDS_BYTES: usize = 2 * WORKGROUP_LANES * 4;
const ARRIVALS: usize = 6;
const CONTENDED_TOKEN: u32 = 4;
const RETIRED_TOKEN: u32 = 5;
const ERROR_TOKEN: u32 = 6;

/// Three borrowed roots, not compiler/runtime allocation authority.
///
/// The roots cannot be recovered, cloned, or safely fabricated by callers.
///
/// ```compile_fail,E0599
/// use fe2o3_device::finite_join::multiwave_join_v1::MultiwaveJoinStorageV1;
/// fn duplicate(storage: MultiwaveJoinStorageV1<'_>) { let _ = storage.clone(); }
/// ```
///
/// ```compile_fail,E0616
/// use fe2o3_device::finite_join::multiwave_join_v1::MultiwaveJoinStorageV1;
/// fn pointer(storage: MultiwaveJoinStorageV1<'_>) { let _ = storage.payload; }
/// ```
#[must_use]
#[repr(C)]
pub struct MultiwaveJoinStorageV1<'dispatch> {
    input: *const [f32; INPUT_ELEMENTS],
    payload: *mut [f32; PAYLOAD_ELEMENTS],
    state: *const [AtomicU32; MULTIWAVE_STATE_WORDS],
    _lifetime: PhantomData<(
        &'dispatch [f32; INPUT_ELEMENTS],
        &'dispatch mut [f32; PAYLOAD_ELEMENTS],
        &'dispatch mut [AtomicU32; MULTIWAVE_STATE_WORDS],
    )>,
    _not_send_sync: PhantomData<*mut ()>,
}

fn disjoint_regions(regions: [(usize, usize, usize); 3]) -> bool {
    for (index, &(base, bytes, alignment)) in regions.iter().enumerate() {
        let Some(end) = base.checked_add(bytes) else {
            return false;
        };
        if base == 0 || base % alignment != 0 {
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

impl<'dispatch> MultiwaveJoinStorageV1<'dispatch> {
    /// Binds engineering storage without claiming source or launch admission.
    ///
    /// # Safety
    /// Each pointer designates its exact initialized array in disjoint, aligned,
    /// live storage. Input remains immutable. All nine state cells are actual
    /// `AtomicU32` objects initialized to `[1,3,0,0,0,0,0,0,0]` before launch.
    /// Exactly two full WG128 groups execute, once per physical lane. Each lane
    /// may bind the same dispatch roots, but no other agent may access payload
    /// or alter state until every invocation has quiesced, including failures.
    /// Atomics and ordinary payloads share a coherent system-scope publication
    /// domain. No root/state epoch may be reused before complete native teardown.
    /// Every invocation reaches all three LDS exchanges in all eight rounds.
    /// These engineering premises are not authenticated by this constructor.
    pub unsafe fn from_raw_parts(
        input: *const [f32; INPUT_ELEMENTS],
        payload: *mut [f32; PAYLOAD_ELEMENTS],
        state: *const [AtomicU32; MULTIWAVE_STATE_WORDS],
    ) -> Result<Self, u32> {
        if !disjoint_regions([
            (input as usize, INPUT_ELEMENTS * 4, align_of::<f32>()),
            (payload as usize, PAYLOAD_ELEMENTS * 4, align_of::<f32>()),
            (
                state as usize,
                MULTIWAVE_STATE_WORDS * 4,
                align_of::<AtomicU32>(),
            ),
        ]) {
            return Err(INVALID);
        }
        Ok(Self {
            input,
            payload,
            state,
            _lifetime: PhantomData,
            _not_send_sync: PhantomData,
        })
    }

    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn state(&self) -> &[AtomicU32; MULTIWAVE_STATE_WORDS] {
        // SAFETY: the engineering binding retains genuine initialized atomics.
        unsafe { &*self.state }
    }
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn header_error(state: &[AtomicU32; MULTIWAVE_STATE_WORDS]) -> u32 {
    let mut error = state[ERRORS].load(Ordering::Acquire);
    if state[EPOCH].load(Ordering::Acquire) != 1 {
        error |= STALE_EPOCH;
    }
    if state[READY].load(Ordering::Acquire) & !ALL_TASKS != 0
        || state[DONE].load(Ordering::Acquire) & !ALL_TASKS != 0
        || state[CLAIMED].load(Ordering::Acquire) & !ALL_TASKS != 0
    {
        error |= INVALID;
    }
    let owners = state[OWNERS].load(Ordering::Acquire);
    if owners & !63 != 0 || owners & 3 == 3 || (owners >> 2) & 3 == 3 || (owners >> 4) & 3 == 3 {
        error |= INVALID;
    }
    error
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn claim_task(state: &[AtomicU32; MULTIWAVE_STATE_WORDS], worker: u32) -> Claim {
    let error = header_error(state)
        | if worker >= WORKGROUPS as u32 {
            INVALID
        } else {
            0
        };
    if error != 0 {
        state[ERRORS].fetch_or(error, Ordering::Relaxed);
        return Claim::Rejected(error);
    }
    let claim = claim_ready_task(
        &state[EPOCH],
        &state[READY],
        &state[CLAIMED],
        &state[ERRORS],
        1,
    );
    let Claim::Task(task) = claim else {
        return claim;
    };
    if task == 2 && state[DONE].load(Ordering::Acquire) & 3 != 3 {
        state[ERRORS].fetch_or(MISSING_PREDECESSOR, Ordering::Relaxed);
        return Claim::Rejected(MISSING_PREDECESSOR);
    }
    let shift = 2 * task;
    let old = state[OWNERS].fetch_or((worker + 1) << shift, Ordering::Release);
    if (old >> shift) & 3 != 0 {
        state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
        return Claim::Rejected(DUPLICATE);
    }
    Claim::Task(task)
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn begin_token(state: &[AtomicU32; MULTIWAVE_STATE_WORDS], worker: u32, retired: bool) -> u32 {
    if retired {
        return RETIRED_TOKEN;
    }
    let error = header_error(state);
    if error != 0 {
        state[ERRORS].fetch_or(error, Ordering::Relaxed);
        return ERROR_TOKEN;
    }
    if state[DONE].load(Ordering::Acquire) == ALL_TASKS {
        return RETIRED_TOKEN;
    }
    match claim_task(state, worker) {
        Claim::Task(task) => task + 1,
        Claim::Empty => 0,
        Claim::Contended => CONTENDED_TOKEN,
        Claim::Rejected(_) => ERROR_TOKEN,
    }
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn admission_error(state: &[AtomicU32; MULTIWAVE_STATE_WORDS], token: u32, worker: u32) -> u32 {
    let error = header_error(state);
    if error != 0 {
        return error;
    }
    if token > RETIRED_TOKEN || worker >= WORKGROUPS as u32 {
        return INVALID;
    }
    // Keep the task interval explicit before subtraction and state indexing.
    // The preceding rejection leaves only idle/contended/retired outside it.
    if token < 1 || token > 3 {
        return 0;
    }
    let task = token - 1;
    let bit = 1 << task;
    if state[CLAIMED].load(Ordering::Acquire) & bit == 0
        || (state[OWNERS].load(Ordering::Acquire) >> (2 * task)) & 3 != worker + 1
    {
        return INVALID;
    }
    if state[DONE].load(Ordering::Acquire) & bit != 0
        || state[ARRIVALS + task as usize].load(Ordering::Acquire) != 0
    {
        return DUPLICATE;
    }
    // Every join lane performs this acquire; an LDS wait alone is insufficient.
    if task == 2 && state[DONE].load(Ordering::Acquire) & 3 != 3 {
        return MISSING_PREDECESSOR;
    }
    0
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn complete_lane(state: &[AtomicU32; MULTIWAVE_STATE_WORDS], task: u32) -> Result<(), u32> {
    if task >= 3 {
        state[ERRORS].fetch_or(INVALID, Ordering::Relaxed);
        return Err(INVALID);
    }
    // The AcqRel RMW chain gathers all 128 preceding ordinary stores, not just
    // this wave's stores. Only the unique last arrival can publish completion.
    let old_arrivals = state[ARRIVALS + task as usize].fetch_add(1, Ordering::AcqRel);
    if old_arrivals >= WORKGROUP_LANES as u32 {
        state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
        return Err(DUPLICATE);
    }
    if old_arrivals + 1 == WORKGROUP_LANES as u32 {
        let bit = 1 << task;
        let old_done = state[DONE].fetch_or(bit, Ordering::AcqRel);
        if old_done & bit != 0 {
            state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
            return Err(DUPLICATE);
        }
        if task < 2 && old_done & 3 != 3 && (old_done | bit) & 3 == 3 {
            state[READY].fetch_or(4, Ordering::Release);
        }
    }
    Ok(())
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn execute_cell(storage: &MultiwaveJoinStorageV1<'_>, task: u32, lane: usize) -> Result<(), u32> {
    if task >= 3 || lane >= WORKGROUP_LANES {
        return Err(INVALID);
    }
    // SAFETY: binding retains initialized disjoint roots; the bounds above
    // contain every scalar place. This private helper is called only after
    // all-lane admission in run_lane, once per physical lane of the unique task.
    // Join admission acquires both predecessors. These ownership invariants
    // remain internal to the one-shot run; no whole-array reference is formed.
    unsafe {
        if task < 2 {
            let cell = task as usize * TILE_ELEMENTS + lane;
            (*storage.payload)[cell] = (*storage.input)[cell];
        } else {
            let left = (*storage.payload)[lane];
            let right = (*storage.payload)[TILE_ELEMENTS + lane];
            (*storage.payload)[2 * TILE_ELEMENTS + lane] = left + right;
        }
    }
    Ok(())
}

// Private monomorphized seam: CPU tests exercise this same round body, but their
// host barriers are not evidence that compiler-owned LDS works on a GPU.
trait Exchange128 {
    fn exchange(&mut self, phase: usize, lane: usize, value: u32, leader_only: bool) -> u32;
}

struct NativeExchange<'group> {
    pipeline: WorkgroupPipeline<'group, u32, 2, WORKGROUP_LANES, 1>,
}

impl NativeExchange<'_> {
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    fn exchange_native(&mut self, phase: usize, lane: usize, value: u32, leader_only: bool) -> u32 {
        self.pipeline.stage(phase);
        self.pipeline.write(phase, lane, value);
        self.pipeline.commit(phase);
        self.pipeline.wait(phase);
        self.pipeline.consume(phase);
        let mut result = self.pipeline.read(phase, 0);
        if !leader_only {
            let mut index = 1;
            while index < WORKGROUP_LANES {
                result |= self.pipeline.read(phase, index);
                index += 1;
            }
        }
        self.pipeline.release(phase);
        result
    }
}

impl Exchange128 for NativeExchange<'_> {
    // rustc_force_inline is only legal on the inherent implementation above.
    #[inline(always)]
    fn exchange(&mut self, phase: usize, lane: usize, value: u32, leader_only: bool) -> u32 {
        self.exchange_native(phase, lane, value, leader_only)
    }
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn run_lane(
    storage: &MultiwaveJoinStorageV1<'_>,
    lane: usize,
    worker: u32,
    exchange: &mut impl Exchange128,
) -> FiniteJoinWorkerResult {
    let state = storage.state();
    let mut result = FiniteJoinWorkerResult {
        error: 0,
        executed_tasks: 0,
        rounds: 0,
        empty_probes: 0,
    };
    let mut retired = false;
    let mut round = 0;
    while round < MAX_ROUNDS {
        let phase = round as usize * 3;
        let local_token = if lane == 0 {
            begin_token(state, worker, retired)
        } else {
            0
        };
        let token = exchange.exchange(phase, lane, local_token, true);
        let local_error = admission_error(state, token, worker);
        let admission = exchange.exchange(phase + 1, lane, local_error, false);
        let mut completion_error = admission;
        if admission == 0 && token >= 1 && token <= 3 {
            // Private coordinates come from the exact launch; all lanes admitted
            // the same claim. A rejected cell must not publish an arrival.
            match execute_cell(storage, token - 1, lane) {
                Ok(()) => match complete_lane(state, token - 1) {
                    Ok(()) => result.executed_tasks += 1,
                    Err(error) => completion_error |= error,
                },
                Err(error) => completion_error |= error,
            }
        }
        if completion_error != 0 {
            state[ERRORS].fetch_or(completion_error, Ordering::Relaxed);
        }
        // Every lane retires even idle/rejected rounds, after its global arrival.
        let completion = exchange.exchange(phase + 2, lane, completion_error, false);
        result.error |= completion;
        retired |= completion != 0 || token == RETIRED_TOKEN || token == ERROR_TOKEN;
        result.empty_probes += (token == 0) as u32;
        result.rounds += 1;
        round += 1;
    }
    result
}

#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
fn launch_dimensions_valid() -> bool {
    thread::block_dim_x() == WORKGROUP_LANES as u32
        && thread::block_dim_y() == 1
        && thread::block_dim_z() == 1
        && thread::grid_dim_x() == WORKGROUPS as u32
        && thread::grid_dim_y() == 1
        && thread::grid_dim_z() == 1
        && thread::launch_extent_1d() == WORKGROUP_LANES * WORKGROUPS
}

/// One closed useful two-wave run; no callbacks, resumable rounds or root escape.
pub struct MultiwaveJoinWorkerV1 {
    _private: (),
}

impl MultiwaveJoinWorkerV1 {
    /// Consumes the one-shot storage, with all 24 exchanges on every valid lane.
    /// The returned local observation is not dispatch completion or output access.
    ///
    /// ```compile_fail,E0382
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::multiwave_join_v1::{MultiwaveJoinStorageV1, MultiwaveJoinWorkerV1}};
    /// fn reuse<'g>(storage: MultiwaveJoinStorageV1<'_>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = MultiwaveJoinWorkerV1::run(storage, scope);
    ///     let _ = MultiwaveJoinWorkerV1::run(storage, scope);
    /// }
    /// ```
    ///
    /// ```compile_fail,E0061
    /// use fe2o3_device::{WorkgroupLdsScope, finite_join::multiwave_join_v1::{MultiwaveJoinStorageV1, MultiwaveJoinWorkerV1}};
    /// fn phase<'g>(storage: MultiwaveJoinStorageV1<'_>, scope: &mut WorkgroupLdsScope<'g>) {
    ///     let _ = MultiwaveJoinWorkerV1::run(storage, scope, 7);
    /// }
    /// ```
    #[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
    pub fn run<'group>(
        storage: MultiwaveJoinStorageV1<'_>,
        scope: &mut WorkgroupLdsScope<'group>,
    ) -> Result<FiniteJoinWorkerResult, u32> {
        // Launch dimensions are uniform. No lane-specific rejection skips LDS.
        if !launch_dimensions_valid() {
            return Err(INVALID);
        }
        let global = thread::index_1d().get();
        let mut exchange = NativeExchange {
            pipeline: WorkgroupPipeline::current(scope),
        };
        Ok(run_lane(
            &storage,
            global % WORKGROUP_LANES,
            (global / WORKGROUP_LANES) as u32,
            &mut exchange,
        ))
    }
}

#[cfg(test)]
#[path = "multiwave_join_v1_tests.rs"]
mod tests;
