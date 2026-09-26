//! Object copies snapshot initialization and symbolic relocations before commit.

use super::*;

pub(super) struct StorageSnapshotV1 {
    pub(super) bytes: Vec<u8>,
    pub(super) initialized: Vec<bool>,
    pub(super) relocations: Vec<StorageRelocationV1>,
    headers: usize,
}

pub(super) fn storage_snapshot_headers_v1() -> usize {
    // Constructor, snapshot builder, caller, and consuming release slots; both
    // constructor/builder Kind results and caller's mapped Execution results.
    4 * size_of::<StorageSnapshotV1>()
        + 4 * size_of::<Result<StorageSnapshotV1, SimulationExecutionErrorKindV1>>()
        + 2 * size_of::<Result<StorageSnapshotV1, SimulationExecutionErrorV1>>()
        + 2 * size_of::<Result<(), SimulationExecutionErrorKindV1>>()
        + 2 * size_of::<Result<(), SimulationExecutionErrorV1>>()
}

impl StorageSnapshotV1 {
    pub(super) fn new(
        accounting: &StorageAccountingV1,
    ) -> Result<Self, SimulationExecutionErrorKindV1> {
        let headers = storage_snapshot_headers_v1();
        accounting.hold(headers)?;
        Ok(Self {
            bytes: Vec::new(),
            initialized: Vec::new(),
            relocations: Vec::new(),
            headers,
        })
    }

    pub(super) fn release(self, accounting: &StorageAccountingV1) {
        let bytes = self.headers
            + self.bytes.capacity() * size_of::<u8>()
            + self.initialized.capacity() * size_of::<bool>()
            + self.relocations.capacity() * size_of::<StorageRelocationV1>();
        drop(self);
        accounting.release(bytes);
    }

    pub(super) fn width(&self) -> usize {
        self.bytes.len()
    }
}

impl Memory {
    pub(super) fn storage_snapshot_v1(
        &self,
        source: &StorageAddressV1,
        width: usize,
        invocation: SimulationInvocationV1,
    ) -> Result<StorageSnapshotV1, SimulationExecutionErrorKindV1> {
        let allocation = self.allocation(&source.pointer)?;
        let start = source.pointer.byte_offset;
        let end = start
            .checked_add(width)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        if end > allocation.bytes.len() {
            return Err(out_of_bounds_error(
                &source.pointer,
                width,
                allocation.bytes.len(),
            ));
        }
        self.storage_accounting
            .charge(allocation.storage.relocations.len())?;
        let mut complete = 0_usize;
        for relocation in &allocation.storage.relocations {
            if storage_overlap_v1(start, end, relocation.start, relocation.end) {
                if relocation.start < start || relocation.end > end {
                    return Err(storage_violation_v1(
                        "copy cuts through a symbolic pointer representation",
                    ));
                }
                self.storage_validate_pointer_v1(
                    &relocation.value,
                    relocation.representation,
                    invocation,
                )?;
                complete += 1;
            }
        }
        let mut snapshot = StorageSnapshotV1::new(&self.storage_accounting)?;
        let result = (|| {
            storage_reserve_v1(&mut snapshot.bytes, width, &self.storage_accounting)?;
            storage_reserve_v1(&mut snapshot.initialized, width, &self.storage_accounting)?;
            storage_reserve_v1(
                &mut snapshot.relocations,
                complete,
                &self.storage_accounting,
            )?;
            self.storage_accounting.charge(
                width
                    .checked_mul(2)
                    .and_then(|work| work.checked_add(allocation.storage.relocations.len()))
                    .ok_or(storage_violation_v1("copy snapshot work overflow"))?,
            )?;
            snapshot
                .bytes
                .extend_from_slice(&allocation.bytes[start..end]);
            snapshot
                .initialized
                .extend_from_slice(&allocation.initialized[start..end]);
            for relocation in &allocation.storage.relocations {
                if relocation.start >= start && relocation.end <= end {
                    let mut copied = relocation.clone();
                    copied.start -= start;
                    copied.end -= start;
                    snapshot.relocations.push(copied);
                }
            }
            Ok(())
        })();
        match result {
            Ok(()) => Ok(snapshot),
            Err(error) => {
                snapshot.release(&self.storage_accounting);
                Err(error)
            }
        }
    }

    pub(super) fn storage_prepare_copy_v1(
        &mut self,
        destination: &StorageAddressV1,
        snapshot: &StorageSnapshotV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        let allocation = self
            .allocations
            .get_mut(&destination.pointer.allocation)
            .ok_or(SimulationExecutionErrorKindV1::DanglingPointer {
                allocation: destination.pointer.allocation,
            })?;
        let count = allocation
            .storage
            .relocations
            .len()
            .checked_add(snapshot.relocations.len())
            .ok_or(storage_violation_v1("copy relocation count overflow"))?;
        storage_reserve_v1(
            &mut allocation.storage.relocations,
            count,
            &self.storage_accounting,
        )?;
        let mutation = allocation.storage.mutation_work()?;
        let work = snapshot
            .width()
            .checked_mul(4)
            .and_then(|work| work.checked_add(snapshot.relocations.len()))
            .and_then(|work| work.checked_add(mutation))
            .ok_or(storage_violation_v1("copy commit work overflow"))?;
        self.storage_accounting.charge(work)
    }

    pub(super) fn storage_commit_copy_v1(
        &mut self,
        destination: &StorageAddressV1,
        snapshot: &StorageSnapshotV1,
        writer: Option<u64>,
    ) {
        let start = destination.pointer.byte_offset;
        let end = start + snapshot.width();
        let allocation = self
            .allocations
            .get_mut(&destination.pointer.allocation)
            .expect("prepared copy destination remains live");
        allocation
            .storage
            .invalidate(start, end, &mut allocation.initialized);
        allocation.bytes[start..end].copy_from_slice(&snapshot.bytes);
        allocation.initialized[start..end].copy_from_slice(&snapshot.initialized);
        if let Some(writer) = writer {
            allocation.workgroup_published[start..end].fill(false);
            allocation.workgroup_writer[start..end].fill(writer);
        }
        for relocation in &snapshot.relocations {
            let mut copied = relocation.clone();
            copied.start += start;
            copied.end += start;
            allocation.storage.relocation_bytes += copied.end - copied.start;
            allocation.storage.relocations.push(copied);
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn storage_copy_object_v1(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    source: &StorageAddressV1,
    destination: &StorageAddressV1,
    source_access: MemoryAccess,
    destination_access: MemoryAccess,
    overlap: fe2o3_kernel_ir::StorageCopyOverlapV1,
    invocation: SimulationInvocationV1,
    site: &CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    if source.layout != destination.layout {
        return Err(engine.at(
            *site,
            storage_violation_v1("copy changes current-module layout ID"),
        ));
    }
    let row = storage_row_v1(engine, source.layout, site)?;
    let width = storage_extent_v1(row).map_err(|kind| engine.at(*site, kind))?;
    engine
        .memory
        .storage_validate_v1(source, source_access, width, false, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    engine
        .memory
        .storage_validate_v1(destination, destination_access, width, true, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    engine
        .memory
        .storage_publication_v1(source, width, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    let source_end = source
        .pointer
        .byte_offset
        .checked_add(width)
        .ok_or_else(|| engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow))?;
    let destination_end = destination
        .pointer
        .byte_offset
        .checked_add(width)
        .ok_or_else(|| engine.at(*site, SimulationExecutionErrorKindV1::PointerOffsetOverflow))?;
    if overlap == fe2o3_kernel_ir::StorageCopyOverlapV1::NonOverlapping
        && source.pointer.allocation == destination.pointer.allocation
        && storage_overlap_v1(
            source.pointer.byte_offset,
            source_end,
            destination.pointer.byte_offset,
            destination_end,
        )
    {
        return Err(engine.at(
            *site,
            SimulationExecutionErrorKindV1::CopyRangesOverlap {
                allocation: source.pointer.allocation,
                source_offset: source.pointer.byte_offset,
                destination_offset: destination.pointer.byte_offset,
                bytes: width,
            },
        ));
    }
    let writer = if destination.pointer.address_space == AddressSpace::Workgroup {
        Some(
            invocation_local_ordinal(invocation)
                .and_then(|value| value.checked_add(1))
                .ok_or_else(|| {
                    engine.at(
                        *site,
                        storage_violation_v1("storage workgroup writer ordinal overflow"),
                    )
                })?,
        )
    } else {
        None
    };
    let snapshot = engine
        .memory
        .storage_snapshot_v1(source, width, invocation)
        .map_err(|kind| engine.at(*site, kind))?;
    let result = (|| {
        engine
            .memory
            .storage_prepare_copy_v1(destination, &snapshot)
            .map_err(|kind| engine.at(*site, kind))?;
        storage_observe_v1(engine, source, width, false, site)?;
        storage_observe_v1(engine, destination, width, true, site)?;
        engine
            .memory
            .storage_commit_copy_v1(destination, &snapshot, writer);
        Ok(())
    })();
    snapshot.release(&engine.memory.storage_accounting);
    result
}
