//! Bounded direct projections of actual V22 records, not an execution model.
use super::*;
use fe2o3_kernel_ir::AddressSpace;
use fe2o3_kir_sim::{
    PhysicalLdsExchangeDebugRecordRefV22 as Record,
    PhysicalLdsExchangeDebugSymbolicKindV22 as Symbolic, SimulationDebugBarrierActionV1 as Barrier,
    SimulationDebugCheckpointPhaseV1 as Phase, SimulationDebugMemoryAccessV1 as Access,
};
use serde::ser::{SerializeSeq, Serializer};

#[derive(Clone, Copy, Serialize)]
pub(super) struct Allocation {
    pub(super) ordinal: u64,
    generation: u64,
    pub(super) address_space: AddressSpaceV1,
    pub(super) byte_length: usize,
    first_checkpoint_sequence: u64,
}
const EMPTY_ALLOCATION: Allocation = Allocation {
    ordinal: 0,
    generation: 0,
    address_space: AddressSpaceV1::Global,
    byte_length: 0,
    first_checkpoint_sequence: 0,
};
pub(super) struct Catalog {
    entries: [Allocation; ALLOCATIONS],
    len: usize,
}
impl Catalog {
    pub(super) fn scan(session: &LdsSession) -> Result<Self, &'static str> {
        let mut catalog = Self {
            entries: [EMPTY_ALLOCATION; ALLOCATIONS],
            len: 0,
        };
        for i in 0..session.records_len() {
            let r = session.record(i).ok_or(SHAPE)?;
            if r.phase().is_none() {
                continue;
            }
            if r.memory_allocation(ALLOCATIONS).is_some() {
                return Err(LIMIT);
            }
            for slot in 0..ALLOCATIONS {
                let Some((ordinal, byte_length)) = r.memory_allocation(slot) else {
                    break;
                };
                let address_space = space(r.memory_address_space(slot).ok_or(SHAPE)?)?;
                if let Some(previous) = catalog.entries[..catalog.len]
                    .iter()
                    .find(|v| v.ordinal == ordinal)
                {
                    if (previous.address_space, previous.byte_length)
                        != (address_space, byte_length)
                    {
                        return Err(SHAPE);
                    }
                } else {
                    if catalog.len == ALLOCATIONS {
                        return Err(LIMIT);
                    }
                    catalog.entries[catalog.len] = Allocation {
                        ordinal,
                        generation: 0,
                        address_space,
                        byte_length,
                        first_checkpoint_sequence: i as u64 + 1,
                    };
                    catalog.len += 1;
                }
            }
        }
        Ok(catalog)
    }
    pub(super) fn entries(&self) -> &[Allocation] {
        &self.entries[..self.len]
    }
    fn memory(
        &self,
        ordinal: u64,
        offset: usize,
        length: usize,
        space: AddressSpaceV1,
    ) -> Result<(), &'static str> {
        let allocation = self
            .entries()
            .iter()
            .find(|a| a.ordinal == ordinal)
            .ok_or(SHAPE)?;
        if allocation.address_space != space
            || offset
                .checked_add(length)
                .is_none_or(|end| end > allocation.byte_length)
        {
            return Err(SHAPE);
        }
        Ok(())
    }
}
fn space(space: AddressSpace) -> Result<AddressSpaceV1, &'static str> {
    match space {
        AddressSpace::Global => Ok(AddressSpaceV1::Global),
        AddressSpace::Workgroup => Ok(AddressSpaceV1::Workgroup),
        _ => Err(SHAPE),
    }
}
#[derive(Serialize)]
pub(super) struct Scope {
    global: [u64; 3],
    local: [u32; 3],
    workgroup: [u64; 3],
    logical_wave: u32,
    logical_lane: u16,
    wave_width: u16,
    interpretation: &'static str,
}
impl Scope {
    fn new(record: Record<'_>) -> Result<Self, &'static str> {
        let i = record.invocation();
        if i.local[0] >= 128
            || i.local[1..] != [0, 0]
            || i.global != [u64::from(i.local[0]), 0, 0]
            || i.workgroup != [0, 0, 0]
            || i.workgroup_size != [128, 1, 1]
            || i.workgroup_count != [1, 1, 1]
            || i.launch_extent != [128, 1, 1]
        {
            return Err(SHAPE);
        }
        let (logical_wave, logical_lane) = Profile::LdsExchangeV22.logical_wave_lane(i.local[0]);
        Ok(Self {
            global: i.global,
            local: i.local,
            workgroup: i.workgroup,
            logical_wave,
            logical_lane,
            wave_width: 64,
            interpretation: "logical_visualization",
        })
    }
}
#[derive(Clone, Copy, Serialize)]
struct Pending {
    function_ordinal: u64,
    frame: u64,
    value_ordinal: u32,
    kind: &'static str,
    numeric_availability: &'static str,
}
const EMPTY_PENDING: Pending = Pending {
    function_ordinal: 0,
    frame: 1,
    value_ordinal: 0,
    kind: "",
    numeric_availability: "not_represented",
};
struct PendingSet {
    entries: [Pending; PENDING],
    len: usize,
}
impl Serialize for PendingSet {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.len))?;
        for entry in &self.entries[..self.len] {
            seq.serialize_element(entry)?;
        }
        seq.end()
    }
}
impl PendingSet {
    fn new(record: Record<'_>) -> Result<Self, &'static str> {
        if record.frame_count() != 1 {
            return Err(SHAPE);
        }
        let n = record.binding_count(0).ok_or(SHAPE)?;
        if n > BINDINGS {
            return Err(LIMIT);
        }
        let mut result = Self {
            entries: [EMPTY_PENDING; PENDING],
            len: 0,
        };
        for i in 0..n {
            let b = record.binding(0, i).ok_or(SHAPE)?;
            let kind = match b.symbolic_kind() {
                Some(Symbolic::PendingGlobalRead) => "pending_global_read",
                Some(Symbolic::PendingLdsRead) => "pending_lds_read",
                _ => continue,
            };
            if b.scalar().is_some() || result.len == PENDING {
                return Err(if result.len == PENDING { LIMIT } else { SHAPE });
            }
            result.entries[result.len] = Pending {
                function_ordinal: record.site().function_ordinal as u64,
                frame: 1,
                value_ordinal: b.value().0,
                kind,
                numeric_availability: "not_represented",
            };
            result.len += 1;
        }
        Ok(result)
    }
}
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Payload {
    Checkpoint {
        phase: &'static str,
        pending: PendingSet,
    },
    Memory {
        access: &'static str,
        allocation: u64,
        generation: u64,
        byte_offset: usize,
        byte_length: usize,
        address_space: AddressSpaceV1,
    },
    Barrier {
        action: &'static str,
        phase: u64,
        participants: u32,
    },
}
#[derive(Serialize)]
pub(super) struct Row {
    sequence: u64,
    producer_ordinal: u64,
    scope: Scope,
    site: KirSiteV1,
    payload: Payload,
}
impl Row {
    pub(super) fn new(
        index: usize,
        record: Record<'_>,
        catalog: &Catalog,
    ) -> Result<Self, &'static str> {
        let payload = match (record.phase(), record.memory_access(), record.barrier()) {
            (Some(phase), None, None) => Payload::Checkpoint {
                phase: match phase {
                    Phase::BeforeOperation => "before_operation",
                    Phase::AfterOperation => "after_operation",
                },
                pending: PendingSet::new(record)?,
            },
            (None, Some((access, allocation, offset, length, address_space)), None) => {
                let address_space = space(address_space)?;
                catalog.memory(allocation, offset, length, address_space)?;
                let access = match access {
                    Access::Read => "read",
                    Access::WriteCommitted => "write_committed",
                    _ => return Err(SHAPE),
                };
                Payload::Memory {
                    access,
                    allocation,
                    generation: 0,
                    byte_offset: offset,
                    byte_length: length,
                    address_space,
                }
            }
            (None, None, Some((action, phase, participants))) => {
                let (action, required) = match action {
                    Barrier::Arrive => ("arrive", 1),
                    Barrier::Release => ("release", 128),
                };
                if participants != required {
                    return Err(SHAPE);
                }
                Payload::Barrier {
                    action,
                    phase,
                    participants,
                }
            }
            _ => return Err(SHAPE),
        };
        let site = record.site();
        Ok(Self {
            sequence: index as u64 + 1,
            producer_ordinal: record.ordinal(),
            scope: Scope::new(record)?,
            site: KirSiteV1 {
                function_ordinal: site.function_ordinal as u64,
                block_ordinal: u64::from(site.block.0),
                point: KirSitePointV1::Operation {
                    operation_ordinal: u64::from(site.operation),
                },
            },
            payload,
        })
    }
}
