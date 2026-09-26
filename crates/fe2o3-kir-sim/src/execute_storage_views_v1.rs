//! Allocation-local views. IDs never stand for numerical machine addresses.

use super::*;
use fe2o3_kernel_ir::{StorageLayoutIdV1, StorageLayoutKindV1, StorageLayoutV1};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct StorageAddressV1 {
    pub(super) pointer: PointerValue,
    pub(super) layout: StorageLayoutIdV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum StorageScopeV1 {
    Unscoped,
    Invocation(Option<[u64; 3]>),
    Workgroup(Option<[u64; 3]>),
}

impl StorageScopeV1 {
    pub(super) fn new(space: AddressSpace, invocation: Option<SimulationInvocationV1>) -> Self {
        match space {
            AddressSpace::Private => Self::Invocation(invocation.map(|value| value.global)),
            AddressSpace::Workgroup => Self::Workgroup(invocation.map(|value| value.workgroup)),
            _ => Self::Unscoped,
        }
    }

    pub(super) fn validate(
        self,
        invocation: SimulationInvocationV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        let valid = match self {
            Self::Unscoped => true,
            Self::Invocation(owner) => owner == Some(invocation.global),
            Self::Workgroup(owner) => owner == Some(invocation.workgroup),
        };
        if valid {
            Ok(())
        } else {
            Err(storage_violation_v1(
                "pointer escaped its invocation or workgroup",
            ))
        }
    }
}

pub(super) struct StorageGuardV1 {
    start: usize,
    end: usize,
    parent: Option<usize>,
    valid: bool,
}

pub(super) struct StorageAllocationV1 {
    pub(super) scope: StorageScopeV1,
    pub(super) guards: Vec<StorageGuardV1>,
    pub(super) relocations: Vec<StorageRelocationV1>,
    pub(super) relocation_bytes: usize,
}

impl StorageAllocationV1 {
    pub(super) fn new(space: AddressSpace, invocation: Option<SimulationInvocationV1>) -> Self {
        Self {
            scope: StorageScopeV1::new(space, invocation),
            guards: Vec::new(),
            relocations: Vec::new(),
            relocation_bytes: 0,
        }
    }

    pub(super) fn mutation_work(&self) -> Result<usize, SimulationExecutionErrorKindV1> {
        self.relocations
            .len()
            .checked_mul(2)
            .and_then(|work| work.checked_add(self.guards.len()))
            .and_then(|work| work.checked_add(self.relocation_bytes))
            .ok_or(storage_violation_v1("storage mutation scan overflow"))
    }

    pub(super) fn invalidate(&mut self, start: usize, end: usize, initialized: &mut [bool]) {
        for guard in &mut self.guards {
            if storage_overlap_v1(start, end, guard.start, guard.end) {
                guard.valid = false;
            }
        }
        for relocation in &self.relocations {
            if storage_overlap_v1(start, end, relocation.start, relocation.end) {
                // A partial byte write destroys the symbolic representation;
                // untouched placeholder bytes do not thereby become real bits.
                initialized[relocation.start..relocation.end].fill(false);
                self.relocation_bytes -= relocation.end - relocation.start;
            }
        }
        self.relocations
            .retain(|relocation| !storage_overlap_v1(start, end, relocation.start, relocation.end));
    }

    pub(super) fn guard(
        &self,
        mut current: Option<usize>,
        accounting: &StorageAccountingV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        while let Some(index) = current {
            accounting.charge(1)?;
            let guard = self.guards.get(index).ok_or(storage_violation_v1(
                "variant guard is absent from this allocation",
            ))?;
            if !guard.valid || guard.parent.is_some_and(|parent| parent >= index) {
                return Err(storage_violation_v1("variant view was invalidated"));
            }
            current = guard.parent;
        }
        Ok(())
    }

    pub(super) fn raw_read(
        &self,
        start: usize,
        end: usize,
        accounting: &StorageAccountingV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        accounting.charge(self.relocations.len())?;
        if self
            .relocations
            .iter()
            .any(|relocation| storage_overlap_v1(start, end, relocation.start, relocation.end))
        {
            return Err(storage_violation_v1(
                "symbolic pointer bytes have no concrete target encoding",
            ));
        }
        Ok(())
    }

    pub(super) fn backing_bytes(&self) -> Result<usize, SimulationExecutionErrorKindV1> {
        storage_bytes_v1::<StorageGuardV1>(self.guards.capacity())?
            .checked_add(storage_bytes_v1::<StorageRelocationV1>(
                self.relocations.capacity(),
            )?)
            .ok_or(storage_violation_v1(
                "storage allocation accounting overflow",
            ))
    }
}

pub(super) fn storage_violation_v1(reason: &'static str) -> SimulationExecutionErrorKindV1 {
    SimulationExecutionErrorKindV1::StorageViolation { reason }
}

pub(super) fn storage_overlap_v1(a: usize, b: usize, c: usize, d: usize) -> bool {
    a < d && c < b
}

pub(super) fn storage_extent_v1(
    row: &StorageLayoutV1,
) -> Result<usize, SimulationExecutionErrorKindV1> {
    let width = usize::try_from(row.size)
        .map_err(|_| SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
    if width == 0 {
        return Err(storage_violation_v1(
            "zero-sized storage access needs a separate validity proof",
        ));
    }
    Ok(width)
}

impl Memory {
    pub(super) fn storage_publication_v1(
        &self,
        address: &StorageAddressV1,
        width: usize,
        invocation: SimulationInvocationV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        if address.pointer.address_space != AddressSpace::Workgroup {
            return Ok(());
        }
        self.storage_accounting.charge(width)?;
        let allocation = self.allocation(&address.pointer)?;
        let start = address.pointer.byte_offset;
        let end = start
            .checked_add(width)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        let writer = invocation_local_ordinal(invocation)
            .and_then(|value| value.checked_add(1))
            .ok_or(storage_violation_v1(
                "storage workgroup writer ordinal overflow",
            ))?;
        for offset in start..end {
            if allocation.initialized[offset]
                && !allocation.workgroup_published[offset]
                && allocation.workgroup_writer[offset] != writer
            {
                return Err(SimulationExecutionErrorKindV1::WorkgroupUseBeforePublish {
                    allocation: address.pointer.allocation,
                    offset: start,
                    bytes: width,
                });
            }
        }
        Ok(())
    }

    pub(super) fn storage_value_ready_v1(
        &self,
        address: &StorageAddressV1,
        access: MemoryAccess,
        width: usize,
        invocation: SimulationInvocationV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        self.storage_validate_v1(address, access, width, false, invocation)?;
        self.storage_accounting.charge(width)?;
        let allocation = self.allocation(&address.pointer)?;
        let start = address.pointer.byte_offset;
        let end = start
            .checked_add(width)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        if allocation.initialized[start..end]
            .iter()
            .any(|value| !value)
        {
            return Err(SimulationExecutionErrorKindV1::UninitializedRead {
                allocation: address.pointer.allocation,
                offset: start,
                bytes: width,
            });
        }
        self.storage_publication_v1(address, width, invocation)
    }

    pub(super) fn storage_validate_v1(
        &self,
        address: &StorageAddressV1,
        access: MemoryAccess,
        width: usize,
        write: bool,
        invocation: SimulationInvocationV1,
    ) -> Result<(), SimulationExecutionErrorKindV1> {
        self.storage_accounting.charge(1)?;
        let allocation = self.allocation(&address.pointer)?;
        allocation.storage.scope.validate(invocation)?;
        allocation
            .storage
            .guard(address.pointer.storage_guard, &self.storage_accounting)?;
        validate_access(allocation, &address.pointer, access, width, write)
    }

    pub(super) fn storage_register_guard_v1(
        &mut self,
        address: &StorageAddressV1,
        start: usize,
        end: usize,
    ) -> Result<usize, SimulationExecutionErrorKindV1> {
        self.storage_accounting.charge(1)?;
        let allocation = self
            .allocations
            .get_mut(&address.pointer.allocation)
            .ok_or(SimulationExecutionErrorKindV1::DanglingPointer {
                allocation: address.pointer.allocation,
            })?;
        if start >= end || end > allocation.bytes.len() {
            return Err(storage_violation_v1(
                "variant tag guard is outside its allocation",
            ));
        }
        allocation
            .storage
            .guard(address.pointer.storage_guard, &self.storage_accounting)?;
        let index = allocation.storage.guards.len();
        let count = index
            .checked_add(1)
            .ok_or(storage_violation_v1("variant guard ID exhausted"))?;
        storage_reserve_v1(
            &mut allocation.storage.guards,
            count,
            &self.storage_accounting,
        )?;
        allocation.storage.guards.push(StorageGuardV1 {
            start,
            end,
            parent: address.pointer.storage_guard,
            valid: true,
        });
        Ok(index)
    }

    pub(super) fn storage_child_v1(
        &self,
        base: &StorageAddressV1,
        offset: u64,
        layout: StorageLayoutIdV1,
        row: &StorageLayoutV1,
        access: AccessMode,
    ) -> Result<StorageAddressV1, SimulationExecutionErrorKindV1> {
        self.storage_accounting.charge(1)?;
        if base.pointer.access != AccessMode::ReadWrite && base.pointer.access != access {
            return Err(storage_violation_v1(
                "projection strengthens pointer rights",
            ));
        }
        let relative = usize::try_from(offset)
            .map_err(|_| SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        let start = base
            .pointer
            .byte_offset
            .checked_add(relative)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        let width = storage_extent_v1(row)?;
        let end = start
            .checked_add(width)
            .ok_or(SimulationExecutionErrorKindV1::PointerOffsetOverflow)?;
        if start < base.pointer.lower_bound || end > base.pointer.upper_bound {
            return Err(storage_violation_v1("projection exceeds its parent view"));
        }
        let mut pointer = base.pointer.clone();
        pointer.byte_offset = start;
        pointer.lower_bound = start;
        pointer.upper_bound = end;
        pointer.access = access;
        Ok(StorageAddressV1 { pointer, layout })
    }
}

pub(super) fn storage_field_v1(
    row: &StorageLayoutV1,
    index: u32,
) -> Result<(u64, StorageLayoutIdV1), SimulationExecutionErrorKindV1> {
    let field = match &row.kind {
        StorageLayoutKindV1::Record(fields) | StorageLayoutKindV1::Union(fields) => {
            fields.get(index as usize)
        }
        StorageLayoutKindV1::Slice { data, length, .. } => match index {
            0 => Some(data),
            1 => Some(length),
            _ => None,
        },
        _ => None,
    }
    .ok_or(storage_violation_v1(
        "projection does not name a storage field",
    ))?;
    Ok((field.offset, field.layout))
}
