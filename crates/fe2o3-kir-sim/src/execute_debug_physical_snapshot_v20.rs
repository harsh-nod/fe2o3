//! Allocation and traversal accounting for snapshots of the actual Engine.
//! This is a logical payload/capacity ledger, not a process-RSS or allocator cap.
use super::*;

pub(super) enum Failure {
    Resource(Resource),
    Unavailable,
}
impl From<Resource> for Failure {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}

pub(super) fn vector<T>(length: usize, budget: &mut Budget<'_>) -> Result<Vec<T>, Resource> {
    let requested = length
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut result = Vec::new();
    if result.try_reserve_exact(length).is_err() {
        budget.release_storage(requested)?;
        return Err(Resource::Allocation);
    }
    // Match the exact-capacity discipline of the canonical owner's Reader.
    // Reject allocator-reported excess before population or retention; do not
    // retroactively accept unprepaid capacity. This is not an allocator/RSS cap.
    if result.capacity() != length {
        drop(result);
        budget.release_storage(requested)?;
        return Err(Resource::Allocation);
    }
    Ok(result)
}
fn bytes<T>(values: &Vec<T>) -> Result<usize, Resource> {
    values
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)
}
fn sort_work(length: usize, budget: &mut Budget<'_>) -> Result<(), Resource> {
    // Conservative quadratic comparison allowance, charged before sorting.
    budget.charge_work(length.checked_mul(length).ok_or(Resource::Arithmetic)?)
}
fn project(value: &RuntimeValue) -> Result<SimulationDebugValueV1, Failure> {
    match value {
        RuntimeValue::PhysicalEntry(value) => PhysicalEntryDebugSymbolicV20::from_runtime(value)
            .map(SimulationDebugValueV1::PhysicalEntrySymbolicV20)
            .ok_or(Failure::Unavailable),
        _ => debug_value(value).ok_or(Failure::Unavailable),
    }
}

pub(super) fn capture(
    frames: &[RuntimeFrame<'_>],
    indices: &[usize],
    memory: &Memory,
    limits: SimulationDebugCaptureLimitsV1,
    phase: SimulationDebugCheckpointPhaseV1,
    budget: &mut Budget<'_>,
) -> Result<SimulationDebugRecordKindV1, Failure> {
    let floor = budget.storage();
    let result = (|| {
        budget.charge_work(
            frames
                .len()
                .checked_add(
                    memory
                        .allocations
                        .capacity()
                        .checked_mul(2)
                        .ok_or(Resource::Arithmetic)?,
                )
                .and_then(|n| n.checked_add(memory.allocations.len()))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if frames.len() > limits.max_frames_per_checkpoint()
            || memory.allocations.len() > limits.max_allocations_per_checkpoint()
        {
            return Err(Failure::Unavailable);
        }
        let values = frames
            .iter()
            .try_fold(0usize, |n, f| n.checked_add(f.values.len()))
            .ok_or(Resource::Arithmetic)?;
        let payload = memory
            .allocations
            .values()
            .try_fold(0usize, |n, a| {
                n.checked_add(a.bytes.len())?
                    .checked_add(a.initialized.len())
            })
            .ok_or(Resource::Arithmetic)?;
        if values > limits.max_values_per_checkpoint()
            || payload > limits.max_memory_bytes_per_checkpoint()
        {
            return Err(Failure::Unavailable);
        }
        // Stack-resident scratch headers and projection temporaries coexist
        // with all returned frames. Heap scratch is charged by vector() below.
        let scratch = size_of::<SimulationDebugRecordKindV1>()
            + 4 * size_of::<Vec<usize>>()
            + size_of::<SimulationDebugValueV1>();
        budget.reserve_storage(scratch)?;
        let mut captured_frames = vector(frames.len(), budget)?;
        for (depth, frame) in frames.iter().enumerate() {
            budget.charge_work(
                frame
                    .values
                    .capacity()
                    .checked_add(frame.values.len())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let mut ordered = vector(frame.values.len(), budget)?;
            ordered.extend(frame.values.iter());
            sort_work(ordered.len(), budget)?;
            ordered.sort_unstable_by_key(|(id, _)| **id);
            let mut bindings = vector(ordered.len(), budget)?;
            for (id, value) in &ordered {
                bindings.push(SimulationDebugBindingV1 {
                    value: **id,
                    observed: project(value)?,
                });
            }
            let scratch_bytes = bytes(&ordered)?;
            drop(ordered);
            budget.release_storage(scratch_bytes)?;
            let function_ordinal = *indices
                .get(frame.function_index)
                .ok_or(Failure::Unavailable)?;
            captured_frames.push(SimulationDebugFrameV1 {
                depth: u32::try_from(depth).map_err(|_| Failure::Unavailable)?,
                function_ordinal,
                block: frame.current,
                next_operation: frame
                    .function
                    .body
                    .as_ref()
                    .and_then(|body| body.blocks.get(frame.current_index))
                    .and_then(|block| block.operations.get(frame.operation))
                    .and_then(|_| u32::try_from(frame.operation).ok()),
                values: SimulationDebugCollectionV1::Captured(bindings),
            });
        }
        let mut ordered = vector(memory.allocations.len(), budget)?;
        ordered.extend(memory.allocations.iter());
        sort_work(ordered.len(), budget)?;
        ordered.sort_unstable_by_key(|(id, _)| **id);
        let mut captured_memory = vector(ordered.len(), budget)?;
        for (id, allocation) in &ordered {
            budget.charge_work(
                allocation
                    .bytes
                    .len()
                    .checked_add(allocation.initialized.len())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let mut copied = vector(allocation.bytes.len(), budget)?;
            copied.extend_from_slice(&allocation.bytes);
            let mut initialized = vector(allocation.initialized.len(), budget)?;
            initialized.extend(allocation.initialized.iter().copied());
            captured_memory.push(SimulationDebugAllocationV1 {
                allocation: **id,
                address_space: allocation.address_space,
                access: allocation.access,
                alignment: allocation.alignment,
                bytes: copied,
                initialized,
            });
        }
        let scratch_bytes = bytes(&ordered)?;
        drop(ordered);
        budget.release_storage(scratch_bytes)?;
        budget.release_storage(scratch)?;
        Ok(SimulationDebugRecordKindV1::Checkpoint {
            phase,
            stack: SimulationDebugCollectionV1::Captured(captured_frames),
            memory: SimulationDebugCollectionV1::Captured(captured_memory),
        })
    })();
    if result.is_err() {
        // All temporary owners in the closure have already dropped.
        budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
    }
    result
}

/// Only called for a freshly produced record when retention is denied.
pub(super) fn heap_bytes(kind: &SimulationDebugRecordKindV1) -> Option<usize> {
    match kind {
        SimulationDebugRecordKindV1::Checkpoint {
            stack: SimulationDebugCollectionV1::Captured(frames),
            memory: SimulationDebugCollectionV1::Captured(memory),
            ..
        } => {
            let mut n = frames
                .capacity()
                .checked_mul(size_of::<SimulationDebugFrameV1>())?;
            n = n.checked_add(
                memory
                    .capacity()
                    .checked_mul(size_of::<SimulationDebugAllocationV1>())?,
            )?;
            for frame in frames {
                let SimulationDebugCollectionV1::Captured(values) = &frame.values else {
                    return None;
                };
                n = n.checked_add(
                    values
                        .capacity()
                        .checked_mul(size_of::<SimulationDebugBindingV1>())?,
                )?;
            }
            for allocation in memory {
                n = n
                    .checked_add(allocation.bytes.capacity())?
                    .checked_add(allocation.initialized.capacity())?;
            }
            Some(n)
        }
        SimulationDebugRecordKindV1::Memory { .. }
        | SimulationDebugRecordKindV1::WorkgroupBarrier { .. }
        | SimulationDebugRecordKindV1::Fence { .. } => Some(0),
        _ => None,
    }
}
