//! Experimental source implementation of a finite ordinary-f32 task join.
//!
//! This module has no trusted compiler identity or production admission. Its
//! unsafe contract is a prerequisite for CPU protocol tests, not a substitute
//! for source/ranked/KIR ownership, visibility, and fresh-epoch evidence.
//! Only the claiming leader accesses payloads in the original WG128 recipe.
//! The separate engineering wave proposal requires additional all-lane proof.

use core::sync::atomic::{AtomicU32, Ordering};

use crate::{DisjointSlice, thread};

#[path = "finite_join_dispatch.rs"]
mod dispatch;

pub mod multiwave_join_v1;
pub mod wave_mlp_tasks_v1;
pub mod wave_mlp_tiles_v2;
pub mod wave_qkv_attention_output_tasks_v5;
pub mod wave_qkv_attention_output_tiles_v6;
pub mod wave_qkv_attention_tasks_v4;
pub mod wave_qkv_post_tasks_v3;
pub mod wave_qkv_tasks_v2;
pub mod wave_tasks;

pub use dispatch::FiniteJoinDispatch128;

pub const TILE_ELEMENTS: usize = 128;
pub const INPUT_ELEMENTS: usize = 2 * TILE_ELEMENTS;
pub const PAYLOAD_ELEMENTS: usize = 3 * TILE_ELEMENTS;
pub const STATE_WORDS: usize = 6;
pub const MAX_ROUNDS: u32 = 8;
pub const MAX_EMPTY_PROBES: u32 = 4;

const EPOCH: usize = 0;
const READY: usize = 1;
const DONE: usize = 2;
const CLAIMED: usize = 3;
const OWNERS: usize = 4;
const ERRORS: usize = 5;
const ALL_TASKS: u32 = 7;
pub const INVALID: u32 = 1;
pub const STALE_EPOCH: u32 = 2;
pub const DUPLICATE: u32 = 4;
pub const MISSING_PREDECESSOR: u32 = 8;

/// Local execution observations, not permission to read the shared payload.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(C)]
pub struct FiniteJoinWorkerResult {
    pub error: u32,
    pub executed_tasks: u32,
    pub rounds: u32,
    pub empty_probes: u32,
}

/// Copies two input tiles and adds them through the finite DAG `{0,1} -> 2`.
///
/// The intended launch is two WG128 workgroups. Only their leaders execute;
/// no worker waits for residency of another worker. A returning worker does
/// not imply that every worker has returned or that the join has completed.
/// Wrong slice lengths or launch extent return `INVALID` without memory effects.
/// This experimental helper is not supported by checked production lowering.
///
/// # Safety
/// The three allocations must be disjoint, aligned, initialized, and live until
/// all invocations return. Input is immutable. The host must establish a fresh
/// epoch with state `[expected_epoch, 3, 0, 0, 0, 0]`, where the epoch is nonzero,
/// with initialization happening-before both leaders. There may be at most
/// one active leader for each worker ID, zero or one.
/// No outside agent may access payloads or mutate state during the dispatch;
/// no allocation or epoch may be reused before all workers quiesce. On a GPU,
/// allocations and the atomic scope must support this publication protocol.
/// Merely observing a matching epoch or completion mask proves none of this.
pub unsafe fn finite_join_128(
    input: DisjointSlice<f32>,
    payload: DisjointSlice<f32>,
    state: &[AtomicU32],
    expected_epoch: u32,
) -> FiniteJoinWorkerResult {
    if input.len() != INPUT_ELEMENTS
        || payload.len() != PAYLOAD_ELEMENTS
        || state.len() != STATE_WORDS
        || thread::launch_extent_1d() != 256
    {
        return FiniteJoinWorkerResult {
            error: INVALID,
            ..Default::default()
        };
    }
    let global = thread::index_1d().get();
    if global >= 256 {
        return FiniteJoinWorkerResult {
            error: INVALID,
            ..Default::default()
        };
    }
    if global & 127 != 0 {
        return FiniteJoinWorkerResult::default();
    }
    // SAFETY: the caller owns a fresh, disjoint dispatch epoch; only the two
    // selected leaders can claim tasks, and claims order every payload access.
    unsafe {
        run_leader(
            input.ptr,
            payload.ptr,
            state,
            expected_epoch,
            (global / 128) as u32,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Claim {
    Task(u32),
    Empty,
    Contended,
    Rejected(u32),
}

// Keep error publication in the caller's ordinary atomic body.
#[inline(always)]
fn reject(errors: &AtomicU32, error: u32) -> Claim {
    errors.fetch_or(error, Ordering::Relaxed);
    Claim::Rejected(error)
}

// Both state layouts share arbitration without coercing fixed arrays to slices.
// The pinned nightly's AMD MIR importer needs these atomic accesses in callers;
// inline(always) alone remains cost-limited in rustc's MIR inliner.
#[cfg_attr(target_arch = "amdgpu", rustc_force_inline)]
#[cfg_attr(not(target_arch = "amdgpu"), inline(always))]
fn claim_ready_task(
    epoch: &AtomicU32,
    ready: &AtomicU32,
    claimed: &AtomicU32,
    errors: &AtomicU32,
    expected_epoch: u32,
) -> Claim {
    if expected_epoch == 0 || epoch.load(Ordering::Acquire) != expected_epoch {
        return reject(errors, STALE_EPOCH);
    }
    let snapshot = ready.load(Ordering::Acquire);
    if snapshot & !ALL_TASKS != 0 {
        return reject(errors, INVALID);
    }
    if snapshot == 0 {
        return Claim::Empty;
    }
    let task = if snapshot & 1 != 0 {
        0
    } else if snapshot & 2 != 0 {
        1
    } else {
        2
    };
    let bit = 1 << task;
    if ready.fetch_and(!bit, Ordering::AcqRel) & bit == 0 {
        return Claim::Contended;
    }
    if claimed.fetch_or(bit, Ordering::AcqRel) & bit != 0 {
        return reject(errors, DUPLICATE);
    }
    Claim::Task(task)
}

fn claim_task(state: &[AtomicU32], expected_epoch: u32, worker: u32) -> Claim {
    let claimed = claim_ready_task(
        &state[EPOCH],
        &state[READY],
        &state[CLAIMED],
        &state[ERRORS],
        expected_epoch,
    );
    let Claim::Task(task) = claimed else {
        return claimed;
    };
    if task == 2 && state[DONE].load(Ordering::Acquire) & 3 != 3 {
        return reject(&state[ERRORS], MISSING_PREDECESSOR);
    }
    state[OWNERS].fetch_or((worker + 1) << (2 * task), Ordering::Relaxed);
    Claim::Task(task)
}

// This function must follow that task's last ordinary payload access.
fn complete_task(state: &[AtomicU32], task: u32) -> Result<(), u32> {
    let bit = 1 << task;
    let old_done = state[DONE].fetch_or(bit, Ordering::AcqRel);
    if old_done & bit != 0 {
        state[ERRORS].fetch_or(DUPLICATE, Ordering::Relaxed);
        return Err(DUPLICATE);
    }
    // The returned RMW value, never a subsequent load, identifies the unique
    // last producer. Its acquire gathers both producers' ordinary writes.
    if task < 2 && old_done & 3 != 3 && (old_done | bit) & 3 == 3 {
        state[READY].fetch_or(4, Ordering::Release);
    }
    Ok(())
}

unsafe fn execute_task(input: *const f32, payload: *mut f32, task: u32) {
    let mut cell = 0;
    while cell < TILE_ELEMENTS {
        // SAFETY: the private caller holds the unique claim. Producer rows are
        // disjoint; the join's acquire claim follows both producers' releases.
        unsafe {
            if task < 2 {
                let offset = task as usize * TILE_ELEMENTS + cell;
                payload.add(offset).write(input.add(offset).read());
            } else {
                let left = payload.add(cell).read();
                let right = payload.add(TILE_ELEMENTS + cell).read();
                payload.add(2 * TILE_ELEMENTS + cell).write(left + right);
            }
        }
        cell += 1;
    }
}

unsafe fn run_leader(
    input: *const f32,
    payload: *mut f32,
    state: &[AtomicU32],
    expected_epoch: u32,
    worker: u32,
) -> FiniteJoinWorkerResult {
    let mut result = FiniteJoinWorkerResult::default();
    while result.rounds < MAX_ROUNDS {
        result.rounds += 1;
        match claim_task(state, expected_epoch, worker) {
            Claim::Task(task) => {
                // SAFETY: run_leader inherits the allocation/epoch contract;
                // the successful private claim precedes all ordinary accesses.
                unsafe { execute_task(input, payload, task) };
                if let Err(error) = complete_task(state, task) {
                    result.error = error;
                    break;
                }
                result.executed_tasks += 1;
            }
            Claim::Empty => {
                result.empty_probes += 1;
                if result.empty_probes == MAX_EMPTY_PROBES {
                    break;
                }
            }
            Claim::Contended => {}
            Claim::Rejected(error) => {
                result.error = error;
                break;
            }
        }
        if state[DONE].load(Ordering::Acquire) == ALL_TASKS {
            break;
        }
    }
    result
}

#[cfg(test)]
#[path = "finite_join_tests.rs"]
mod tests;
