use vstd::prelude::*;

verus! {

pub struct AllocationV3 {
    pub parameter: int,
    pub provenance: int,
    pub base: int,
    pub bytes: Seq<int>,
    pub mutable: bool,
}

pub struct MemoryV3 {
    pub first: AllocationV3,
    pub second: AllocationV3,
    pub output: AllocationV3,
}

pub struct AddressV3 {
    pub parameter: int,
    pub provenance: int,
    pub offset: int,
}

pub struct EventV3 {
    // 1 = read, 2 = write.
    pub kind: int,
    pub parameter: int,
    pub provenance: int,
    pub start: int,
    pub end: int,
    pub absolute: int,
    pub previous: int,
    pub value: int,
}

pub struct ObservationV3 {
    pub memory: MemoryV3,
    pub has_result: bool,
    pub result: int,
    pub trace: Seq<EventV3>,
}

pub open spec fn pow2_v3(exponent: nat) -> nat
    decreases exponent,
{
    if exponent == 0 { 1 } else { 2 * pow2_v3((exponent - 1) as nat) }
}

pub open spec fn norm_u32_v3(value: int) -> int { value % 4294967296 }

pub open spec fn bit_v3(value: int, bit: nat) -> int {
    (norm_u32_v3(value) / (pow2_v3(bit) as int)) % 2
}

pub open spec fn xor_v3(left: int, right: int, width: nat) -> int
    decreases width,
{
    if width == 0 { 0 } else {
        let bit = (width - 1) as nat;
        xor_v3(left, right, bit)
            + if bit_v3(left, bit) != bit_v3(right, bit) {
                pow2_v3(bit) as int
            } else {
                0
            }
    }
}

pub open spec fn allocation_valid_v3(allocation: AllocationV3) -> bool {
    &&& allocation.provenance != 0
    &&& allocation.base >= 0
    &&& allocation.base % 4 == 0
    &&& allocation.base + allocation.bytes.len() <= 0xffff_ffff_ffff_ffff
    &&& forall|index: int| 0 <= index < allocation.bytes.len()
        ==> 0 <= #[trigger] allocation.bytes[index] < 256
}

pub open spec fn disjoint_v3(left: AllocationV3, right: AllocationV3) -> bool {
    &&& left.parameter != right.parameter
    &&& left.provenance != right.provenance
    &&& (left.base + left.bytes.len() <= right.base
        || right.base + right.bytes.len() <= left.base)
}

pub open spec fn memory_valid_v3(memory: MemoryV3) -> bool {
    &&& allocation_valid_v3(memory.first)
    &&& allocation_valid_v3(memory.second)
    &&& allocation_valid_v3(memory.output)
    &&& !memory.first.mutable
    &&& !memory.second.mutable
    &&& memory.output.mutable
    &&& disjoint_v3(memory.first, memory.second)
    &&& disjoint_v3(memory.first, memory.output)
    &&& disjoint_v3(memory.second, memory.output)
}

pub open spec fn access_ok_v3(allocation: AllocationV3, address: AddressV3) -> bool {
    &&& address.parameter == allocation.parameter
    &&& address.provenance == allocation.provenance
    &&& 0 <= address.offset
    &&& address.offset % 4 == 0
    &&& address.offset + 4 <= allocation.bytes.len()
    &&& allocation.base + address.offset <= 0xffff_ffff_ffff_ffff - 4
}

pub open spec fn read_u32_v3(allocation: AllocationV3, offset: int) -> int {
    allocation.bytes[offset]
        + 256 * allocation.bytes[offset + 1]
        + 65536 * allocation.bytes[offset + 2]
        + 16777216 * allocation.bytes[offset + 3]
}

pub open spec fn write_u32_v3(
    allocation: AllocationV3,
    offset: int,
    value: int,
) -> AllocationV3 {
    let normalized = norm_u32_v3(value);
    AllocationV3 {
        bytes: allocation.bytes
            .update(offset, normalized % 256)
            .update(offset + 1, (normalized / 256) % 256)
            .update(offset + 2, (normalized / 65536) % 256)
            .update(offset + 3, (normalized / 16777216) % 256),
        ..allocation
    }
}

pub open spec fn read_event_v3(
    allocation: AllocationV3,
    offset: int,
    value: int,
) -> EventV3 {
    EventV3 {
        kind: 1,
        parameter: allocation.parameter,
        provenance: allocation.provenance,
        start: offset,
        end: offset + 4,
        absolute: allocation.base + offset,
        previous: value,
        value,
    }
}

pub open spec fn write_event_v3(
    allocation: AllocationV3,
    offset: int,
    previous: int,
    value: int,
) -> EventV3 {
    EventV3 {
        kind: 2,
        parameter: allocation.parameter,
        provenance: allocation.provenance,
        start: offset,
        end: offset + 4,
        absolute: allocation.base + offset,
        previous,
        value: norm_u32_v3(value),
    }
}

pub open spec fn source_helper_v3(
    source_opcode: int,
    left: int,
    right: int,
    fallback: int,
) -> int {
    if source_opcode == 17 {
        let combined = xor_v3(left, right, 32);
        if combined == 0 { combined } else { norm_u32_v3(fallback) }
    } else {
        norm_u32_v3(left + right)
    }
}

pub open spec fn mir_helper_v3(
    mir_opcode: int,
    left: int,
    right: int,
    fallback: int,
) -> int {
    if mir_opcode == 6 {
        let xor_local = xor_v3(left, right, 32);
        if xor_local == 0 { xor_local } else { norm_u32_v3(fallback) }
    } else {
        norm_u32_v3(left + right)
    }
}

pub open spec fn kir_helper_v3(
    kir_opcode: int,
    left: int,
    right: int,
    fallback: int,
) -> int {
    if kir_opcode == 106 {
        let xor_ssa = xor_v3(left, right, 32);
        let join_ssa = if xor_ssa == 0 { xor_ssa } else { norm_u32_v3(fallback) };
        join_ssa
    } else {
        norm_u32_v3(left + right)
    }
}

pub open spec fn opcode_relation_v3(source: int, mir: int, kir: int) -> bool {
    source == 17 && mir == 6 && kir == 106
}

pub open spec fn addresses_for_gid_v3(memory: MemoryV3, gid: int) -> (AddressV3, AddressV3, AddressV3) {
    (
        AddressV3 {
            parameter: memory.first.parameter,
            provenance: memory.first.provenance,
            offset: 4 * gid,
        },
        AddressV3 {
            parameter: memory.second.parameter,
            provenance: memory.second.provenance,
            offset: 4 * gid,
        },
        AddressV3 {
            parameter: memory.output.parameter,
            provenance: memory.output.provenance,
            offset: 4 * gid,
        },
    )
}

pub open spec fn guard_for_gid_v3(memory: MemoryV3, gid: int) -> bool {
    &&& gid >= 0
    &&& 4 * gid + 4 <= memory.first.bytes.len()
    &&& 4 * gid + 4 <= memory.second.bytes.len()
    &&& 4 * gid + 4 <= memory.output.bytes.len()
}

pub open spec fn selected_result_v3(memory: MemoryV3, gid: int, fallback: int) -> int {
    let addresses = addresses_for_gid_v3(memory, gid);
    let combined = xor_v3(
        read_u32_v3(memory.first, addresses.0.offset),
        read_u32_v3(memory.second, addresses.1.offset),
        32,
    );
    if combined == 0 { combined } else { norm_u32_v3(fallback) }
}

pub open spec fn source_step_v3(
    memory: MemoryV3,
    guard: bool,
    gid: int,
    opcode: int,
    fallback: int,
) -> ObservationV3 {
    if !guard {
        ObservationV3 { memory, has_result: false, result: 0, trace: Seq::empty() }
    } else {
        let addresses = addresses_for_gid_v3(memory, gid);
        let left = read_u32_v3(memory.first, addresses.0.offset);
        let right = read_u32_v3(memory.second, addresses.1.offset);
        let result = source_helper_v3(opcode, left, right, fallback);
        let previous = read_u32_v3(memory.output, addresses.2.offset);
        ObservationV3 {
            memory: MemoryV3 {
                output: write_u32_v3(memory.output, addresses.2.offset, result),
                ..memory
            },
            has_result: true,
            result,
            trace: seq![
                read_event_v3(memory.first, addresses.0.offset, left),
                read_event_v3(memory.second, addresses.1.offset, right),
                write_event_v3(memory.output, addresses.2.offset, previous, result),
            ],
        }
    }
}

pub open spec fn mir_step_v3(
    memory: MemoryV3,
    guard: bool,
    gid: int,
    opcode: int,
    fallback: int,
) -> ObservationV3 {
    if !guard {
        ObservationV3 { memory, has_result: false, result: 0, trace: Seq::empty() }
    } else {
        let addresses = addresses_for_gid_v3(memory, gid);
        let first_local = read_u32_v3(memory.first, addresses.0.offset);
        let second_local = read_u32_v3(memory.second, addresses.1.offset);
        let call_destination = mir_helper_v3(opcode, first_local, second_local, fallback);
        let old_output = read_u32_v3(memory.output, addresses.2.offset);
        ObservationV3 {
            memory: MemoryV3 {
                output: write_u32_v3(memory.output, addresses.2.offset, call_destination),
                ..memory
            },
            has_result: true,
            result: call_destination,
            trace: seq![
                read_event_v3(memory.first, addresses.0.offset, first_local),
                read_event_v3(memory.second, addresses.1.offset, second_local),
                write_event_v3(memory.output, addresses.2.offset, old_output, call_destination),
            ],
        }
    }
}

pub open spec fn kir_step_v3(
    memory: MemoryV3,
    guard: bool,
    gid: int,
    opcode: int,
    fallback: int,
) -> ObservationV3 {
    if !guard {
        ObservationV3 { memory, has_result: false, result: 0, trace: Seq::empty() }
    } else {
        let addresses = addresses_for_gid_v3(memory, gid);
        let load_ssa_1 = read_u32_v3(memory.first, addresses.0.offset);
        let load_ssa_2 = read_u32_v3(memory.second, addresses.1.offset);
        let call_result_ssa = kir_helper_v3(opcode, load_ssa_1, load_ssa_2, fallback);
        let prior_ssa = read_u32_v3(memory.output, addresses.2.offset);
        ObservationV3 {
            memory: MemoryV3 {
                output: write_u32_v3(memory.output, addresses.2.offset, call_result_ssa),
                ..memory
            },
            has_result: true,
            result: call_result_ssa,
            trace: seq![
                read_event_v3(memory.first, addresses.0.offset, load_ssa_1),
                read_event_v3(memory.second, addresses.1.offset, load_ssa_2),
                write_event_v3(memory.output, addresses.2.offset, prior_ssa, call_result_ssa),
            ],
        }
    }
}

pub open spec fn environments_related_v3(
    source: MemoryV3,
    mir: MemoryV3,
    kir: MemoryV3,
) -> bool {
    source == mir && mir == kir
}

/// Universal conditional refinement for exactly one guarded lane. Identity
/// integers represent premises discharged by the executable checker; they are
/// not cryptographic reasoning inside Verus. Runtime provenance, allocation
/// disjointness, range, and alignment are explicit relational assumptions.
pub proof fn fe2o3_guarded_two_load_xor_store_refines_v3(
    source_memory: MemoryV3,
    mir_memory: MemoryV3,
    kir_memory: MemoryV3,
    guard: bool,
    gid: int,
    source_opcode: int,
    mir_opcode: int,
    kir_opcode: int,
    fallback: int,
    source_identity: int,
    semantic_mir_identity: int,
    kir_identity: int,
    model_identity: int,
)
    requires
        source_identity != 0,
        semantic_mir_identity != 0,
        kir_identity != 0,
        model_identity != 0,
        opcode_relation_v3(source_opcode, mir_opcode, kir_opcode),
        environments_related_v3(source_memory, mir_memory, kir_memory),
        memory_valid_v3(source_memory),
        gid >= 0,
        0 <= fallback < 4294967296,
        guard == guard_for_gid_v3(source_memory, gid),
        guard ==> {
            let addresses = addresses_for_gid_v3(source_memory, gid);
            access_ok_v3(source_memory.first, addresses.0)
                && access_ok_v3(source_memory.second, addresses.1)
                && access_ok_v3(source_memory.output, addresses.2)
        },
    ensures
        source_step_v3(source_memory, guard, gid, source_opcode, fallback)
            == mir_step_v3(mir_memory, guard, gid, mir_opcode, fallback),
        mir_step_v3(mir_memory, guard, gid, mir_opcode, fallback)
            == kir_step_v3(kir_memory, guard, gid, kir_opcode, fallback),
        !guard ==> source_step_v3(source_memory, guard, gid, source_opcode, fallback).trace.len() == 0,
        !guard ==> source_step_v3(source_memory, guard, gid, source_opcode, fallback).memory == source_memory,
        guard ==> source_step_v3(source_memory, guard, gid, source_opcode, fallback).trace.len() == 3,
        guard ==> source_step_v3(source_memory, guard, gid, source_opcode, fallback).result
            == selected_result_v3(source_memory, gid, fallback),
        guard ==> source_step_v3(source_memory, guard, gid, source_opcode, fallback).memory.output
            == write_u32_v3(
                source_memory.output,
                4 * gid,
                selected_result_v3(source_memory, gid, fallback),
            ),
{
}

}
