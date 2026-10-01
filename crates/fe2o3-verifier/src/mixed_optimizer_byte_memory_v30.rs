//! Shared concrete byte-memory vocabulary for selected external and private
//! source endpoints. Static source coordinates are event locators, not dynamic
//! allocation identities. This text is emitted into an exact generated request;
//! it is neither an axiom nor independently executable proof authority.

/// Generator-local symbols, never an intake of source or runtime evidence.
/// Emitters consume one state and bind all five components of its successor.
#[derive(Clone, Copy)]
pub(super) struct ByteMemoryStateNamesV30<'a> {
    pub values: &'a str,
    pub memory: &'a str,
    pub generations: &'a str,
    pub frames: &'a str,
    pub valid: &'a str,
}

#[derive(Clone, Copy)]
pub(super) struct ByteMemoryContextNamesV30<'a> {
    pub owner: &'a str,
    pub invocation: &'a str,
    pub little_endian: &'a str,
}

pub(super) const BYTE_MEMORY_V30: &str = concat!(
    r#"
struct MemorySourceOperationV30 {
    function: int,
    block: int,
    operation: int,
}

struct MemoryDynamicFrameV30 {
    owner: int,
    invocation: int,
}

struct MemoryExecutionContextV37 {
    rank: int,
    extent: Seq<int>,
    workgroup: Seq<int>,
    group: Seq<int>,
    local: Seq<int>,
    // Universally quantified exact-operator interpretation, never a supplied
    // proof premise or an integer arithmetic substitute for floating values.
    ieee_operators: spec_fn(int, int, int, int, int, int) -> int,
}

struct MemoryFrameRuntimeV30 {
    active: Seq<MemoryDynamicFrameV30>,
    next_invocation: int,
    execution: Option<MemoryExecutionContextV37>,
}

struct MemoryPrivateSiteV30 {
    owner: int,
    invocation: int,
    site: MemorySourceOperationV30,
}

enum MemoryAllocationV30 {
    External { identity: int, generation: int },
    Private { owner: int, invocation: int, site: MemorySourceOperationV30, generation: int },
}

struct MemoryPointerV30 {
    allocation: MemoryAllocationV30,
    byte_offset: int,
    view: Option<MemoryStorageViewV38>,
}

struct MemorySliceV30 {
    pointer: MemoryPointerV30,
    length: int,
}

enum MemoryValueV30 {
    Undefined,
    Unit,
    Scalar(int),
    Vector(Seq<int>),
    Pointer(MemoryPointerV30),
    Slice(MemorySliceV30),
}

enum MemoryByteV37 {
    Octet(int),
    PointerFragment { pointer: MemoryPointerV30, width: int, ordinal: int },
}

struct MemoryRelocationV37 {
    pointer: MemoryPointerV30,
    width: int,
    little_endian: bool,
}

struct MemoryBytesV30 {
    bytes: Seq<MemoryByteV37>,
    initialized: Seq<bool>,
    relocations: Map<int, MemoryRelocationV37>,
    write_clock: int,
    write_epochs: Seq<int>,
    // A checked guaranteed base-alignment multiple, not a numeric address.
    base_alignment: int,
}

// Only live allocations occur in this map. Parameter ordinals are deliberately
// absent from keys: two checked runtime parameters may name the same bytes.
struct ByteMemoryV30 {
    live: Map<MemoryAllocationV30, MemoryBytesV30>,
    view_contracts: MemoryViewContractsV38,
}

struct MemoryStateV30 {
    pc: int,
    values: Seq<MemoryValueV30>,
    memory: ByteMemoryV30,
    generations: Map<MemoryPrivateSiteV30, int>,
    frames: MemoryFrameRuntimeV30,
    valid: bool,
}

enum MemoryTagReadPurposeV39 {
    VariantValidation,
    DiscriminantRead,
}

enum MemoryOperationEffectV30 {
    Refused,
    Trap,
    Pure,
    Allocate { address: MemoryValueV30, extent: int, alignment: int },
    Read { address: MemoryValueV30, width: int, alignment: int, value: MemoryValueV30 },
    Write { address: MemoryValueV30, width: int, alignment: int, value: MemoryValueV30 },
    Copy { source: MemoryValueV30, destination: MemoryValueV30, width: int,
        source_alignment: int, destination_alignment: int, nonoverlapping: bool },
    TagRead { address: MemoryValueV30, width: int, alignment: int,
        purpose: MemoryTagReadPurposeV39, observation: Option<MemoryTagObservationV39> },
}

// Every executed operation retains its exact locator and intermediate validity.
// A rejected read cannot disappear merely because final heaps happen to agree.
struct MemoryOperationObservationV30 {
    operation: MemorySourceOperationV30,
    before: MemoryStateV30,
    after: MemoryStateV30,
    valid_before: bool,
    valid_after: bool,
    effect: MemoryOperationEffectV30,
}

open spec fn byte_observation_snapshots_valid_v39(observation: MemoryOperationObservationV30) -> bool {
    observation.valid_before == observation.before.valid
        && observation.valid_after == observation.after.valid
}

// Only the exact registered final operation may preserve a valid trap terminal.
// Plain Unreachable and arbitrary negative PCs remain invalid executions.
open spec fn byte_trap_terminal_v40(
    state: MemoryStateV30, observations: Seq<MemoryOperationObservationV30>,
    operation: MemorySourceOperationV30, block: int, count: int,
) -> bool {
    count > 0 && observations.len() == count && state.valid && state.pc == -2
        && ({
            let last = observations[count - 1];
            byte_observation_snapshots_valid_v39(last)
                && last.operation == operation && last.effect == MemoryOperationEffectV30::Trap
                && last.before.valid && last.before.pc == block && last.after == state
                && last.before.values == state.values && last.before.memory == state.memory
                && last.before.generations == state.generations && last.before.frames == state.frames
        })
}

struct MemoryOperationResultV30 {
    state: MemoryStateV30,
    observation: MemoryOperationObservationV30,
}

struct MemoryBlockResultV30 {
    state: MemoryStateV30,
    observations: Seq<MemoryOperationObservationV30>,
    returned: Seq<MemoryValueV30>,
}

struct MemoryMicroStateV30 {
    state: MemoryStateV30,
    next_operation: int,
    observations: Seq<MemoryOperationObservationV30>,
}

struct MemoryMicroResultV30 {
    next: MemoryMicroStateV30,
    observation: MemoryOperationObservationV30,
}

open spec fn byte_root_frame_v30(owner: int) -> MemoryFrameRuntimeV30 {
    MemoryFrameRuntimeV30 {
        active: seq![MemoryDynamicFrameV30 { owner, invocation: 0 }],
        next_invocation: 1,
        execution: None,
    }
}

// Coordinates are explicit inputs. The emitter separately joins the launch
// extent, target index width and original source workgroup contract.
open spec fn byte_execution_well_formed_v37(execution: MemoryExecutionContextV37) -> bool {
    1 <= execution.rank <= 3
        && execution.extent.len() == 3 && execution.workgroup.len() == 3
        && execution.group.len() == 3 && execution.local.len() == 3
        && forall|axis: int| 0 <= axis < 3 ==>
            0 < execution.extent[axis] && 0 < execution.workgroup[axis]
            && 0 <= execution.group[axis]
            && 0 <= execution.local[axis] < execution.workgroup[axis]
            && execution.group[axis] * execution.workgroup[axis] + execution.local[axis]
                < execution.extent[axis]
            && (execution.rank <= axis ==> execution.extent[axis] == 1
                && execution.workgroup[axis] == 1
                && execution.group[axis] == 0 && execution.local[axis] == 0)
}

open spec fn byte_root_frame_with_execution_v37(
    owner: int, execution: MemoryExecutionContextV37,
) -> MemoryFrameRuntimeV30 {
    MemoryFrameRuntimeV30 {
        active: seq![MemoryDynamicFrameV30 { owner, invocation: 0 }],
        next_invocation: 1,
        execution: Some(execution),
    }
}

// Hierarchy codes: global invocation 0, workgroup 1, local invocation 2.
open spec fn byte_execution_index_v37(
    execution: MemoryExecutionContextV37, hierarchy: int, axis: int,
) -> int
    recommends byte_execution_well_formed_v37(execution),
        0 <= hierarchy <= 2, 0 <= axis < execution.rank,
{
    if hierarchy == 0 {
        execution.group[axis] * execution.workgroup[axis] + execution.local[axis]
    } else if hierarchy == 1 {
        execution.group[axis]
    } else {
        execution.local[axis]
    }
}

open spec fn byte_frame_runtime_well_formed_v30(frames: MemoryFrameRuntimeV30) -> bool {
    0 < frames.next_invocation
        && (match frames.execution {
            None => true,
            Some(execution) => byte_execution_well_formed_v37(execution),
        })
        && (forall|i: int| 0 <= i < frames.active.len() ==>
            0 <= frames.active[i].invocation < frames.next_invocation)
        && (forall|i: int, j: int| 0 <= i < j < frames.active.len() ==>
            frames.active[i].invocation < frames.active[j].invocation)
}

// Every executed entry consumes a fresh dynamic identity. A static source
// call-instance or logical-local range never supplies this invocation number.
open spec fn byte_enter_frame_v30(
    frames: MemoryFrameRuntimeV30, owner: int,
) -> MemoryFrameRuntimeV30
    recommends byte_frame_runtime_well_formed_v30(frames), 0 < frames.active.len(),
{
    MemoryFrameRuntimeV30 {
        active: frames.active.push(MemoryDynamicFrameV30 {
            owner, invocation: frames.next_invocation,
        }),
        next_invocation: frames.next_invocation + 1,
        execution: frames.execution,
    }
}

open spec fn byte_pop_frame_v30(frames: MemoryFrameRuntimeV30) -> MemoryFrameRuntimeV30
    recommends byte_frame_runtime_well_formed_v30(frames), 0 < frames.active.len(),
{
    MemoryFrameRuntimeV30 {
        active: frames.active.take((frames.active.len() - 1) as int),
        next_invocation: frames.next_invocation,
        execution: frames.execution,
    }
}

open spec fn byte_allocation_in_frame_v30(
    allocation: MemoryAllocationV30, frame: MemoryDynamicFrameV30,
) -> bool {
    match allocation {
        MemoryAllocationV30::Private { owner, invocation, site: _, generation: _ } =>
            owner == frame.owner && invocation == frame.invocation,
        MemoryAllocationV30::External { identity: _, generation: _ } => false,
    }
}

// This is an explicit transform, not an inferred consequence of a static MIR
// return locator. An inlined target must independently establish the exact
// allocation/frame relation and absence of escaped pointers before using it.
open spec fn byte_end_frame_v30(
    memory: ByteMemoryV30, frame: MemoryDynamicFrameV30,
) -> ByteMemoryV30 {
    ByteMemoryV30 { live: Map::new(
        |allocation: MemoryAllocationV30| memory.live.contains_key(allocation)
            && !byte_allocation_in_frame_v30(allocation, frame),
        |allocation: MemoryAllocationV30| memory.live[allocation],
    ), view_contracts: memory.view_contracts }
}

open spec fn byte_memory_well_formed_v30(memory: ByteMemoryV30) -> bool {
    byte_view_contracts_shape_v38(memory.view_contracts)
    && forall|allocation: MemoryAllocationV30| memory.live.contains_key(allocation) ==>
        0 < memory.live[allocation].base_alignment
        && memory.live[allocation].bytes.len() == memory.live[allocation].initialized.len()
        && memory.live[allocation].write_epochs.len() == memory.live[allocation].bytes.len()
        && 0 <= memory.live[allocation].write_clock
        && (forall|i: int| 0 <= i < memory.live[allocation].write_epochs.len() ==>
            0 <= memory.live[allocation].write_epochs[i] <= memory.live[allocation].write_clock)
        && byte_object_relocations_well_formed_v37(memory.live[allocation])
}

open spec fn ordinary_memory_width_v30(width: int) -> bool {
    width == 1 || width == 2 || width == 4 || width == 8 || width == 16
}

open spec fn byte_range_live_v30(
    memory: ByteMemoryV30, pointer: MemoryPointerV30, width: int,
) -> bool {
    byte_raw_range_live_v38(memory, pointer.allocation, pointer.byte_offset, width)
        && byte_pointer_view_current_v38(memory, pointer, width)
}

// Space codes are emitted from checked canonical types: private0, global1,
// generic2. This is a representation check, not dereference or liveness proof.
open spec fn byte_pointer_type_v30(
    pointer: MemoryPointerV30, space: int, index_bytes: int,
) -> bool {
    byte_pointer_view_shape_v38(pointer)
        && 0 <= pointer.byte_offset < memory_value_modulus_v30(index_bytes)
        && match pointer.allocation {
            MemoryAllocationV30::External { identity: _, generation: _ } =>
                space == 1 || space == 2,
            MemoryAllocationV30::Private { owner: _, invocation: _, site: _, generation: _ } =>
                space == 0 || space == 2,
        }
}

open spec fn byte_range_initialized_v30(
    memory: ByteMemoryV30, pointer: MemoryPointerV30, width: int,
) -> bool {
    byte_range_live_v30(memory, pointer, width)
        && forall|i: int| pointer.byte_offset <= i < pointer.byte_offset + width ==>
            memory.live[pointer.allocation].initialized[i]
}

open spec fn byte_range_aligned_v30(
    memory: ByteMemoryV30, pointer: MemoryPointerV30, width: int, alignment: int,
) -> bool {
    byte_range_live_v30(memory, pointer, width)
        && 0 < alignment
        && memory.live[pointer.allocation].base_alignment % alignment == 0
        && pointer.byte_offset % alignment == 0
}

open spec fn byte_scale_v30(ordinal: int) -> int
    recommends 0 <= ordinal < 16,
{
    if ordinal == 0 { 1 }
    else if ordinal == 1 { 256 }
    else if ordinal == 2 { 65536 }
    else if ordinal == 3 { 16777216 }
    else if ordinal == 4 { 4294967296 }
    else if ordinal == 5 { 1099511627776 }
    else if ordinal == 6 { 281474976710656 }
    else if ordinal == 7 { 72057594037927936 }
    else if ordinal == 8 { 18446744073709551616 }
    else if ordinal == 9 { 4722366482869645213696 }
    else if ordinal == 10 { 1208925819614629174706176 }
    else if ordinal == 11 { 309485009821345068724781056 }
    else if ordinal == 12 { 79228162514264337593543950336 }
    else if ordinal == 13 { 20282409603651670423947251286016 }
    else if ordinal == 14 { 5192296858534827628530496329220096 }
    else { 1329227995784915872903807060280344576 }
}

open spec fn memory_value_modulus_v30(width: int) -> int
    recommends ordinary_memory_width_v30(width),
{
    if width == 1 { 256 }
    else if width == 2 { 65536 }
    else if width == 4 { 4294967296 }
    else if width == 8 { 18446744073709551616 }
    else { 18446744073709551616 * 18446744073709551616 }
}

open spec fn byte_load_term_v30(
    memory: ByteMemoryV30, pointer: MemoryPointerV30, width: int,
    little_endian: bool, ordinal: int,
) -> int
    recommends ordinary_memory_width_v30(width), byte_scalar_range_initialized_v37(memory, pointer, width),
        0 <= ordinal < 16,
{
    if ordinal < width {
        byte_octet_v37(memory.live[pointer.allocation].bytes[pointer.byte_offset + ordinal])
            * byte_scale_v30(if little_endian { ordinal } else { width - 1 - ordinal })
    } else { 0 }
}

open spec fn byte_load_v30(
    memory: ByteMemoryV30, pointer: MemoryPointerV30, width: int, little_endian: bool,
) -> int
    recommends ordinary_memory_width_v30(width), byte_scalar_range_initialized_v37(memory, pointer, width),
{
    byte_load_term_v30(memory, pointer, width, little_endian, 0)
        + byte_load_term_v30(memory, pointer, width, little_endian, 1)
        + byte_load_term_v30(memory, pointer, width, little_endian, 2)
        + byte_load_term_v30(memory, pointer, width, little_endian, 3)
        + byte_load_term_v30(memory, pointer, width, little_endian, 4)
        + byte_load_term_v30(memory, pointer, width, little_endian, 5)
        + byte_load_term_v30(memory, pointer, width, little_endian, 6)
        + byte_load_term_v30(memory, pointer, width, little_endian, 7)
        + byte_load_term_v30(memory, pointer, width, little_endian, 8)
        + byte_load_term_v30(memory, pointer, width, little_endian, 9)
        + byte_load_term_v30(memory, pointer, width, little_endian, 10)
        + byte_load_term_v30(memory, pointer, width, little_endian, 11)
        + byte_load_term_v30(memory, pointer, width, little_endian, 12)
        + byte_load_term_v30(memory, pointer, width, little_endian, 13)
        + byte_load_term_v30(memory, pointer, width, little_endian, 14)
        + byte_load_term_v30(memory, pointer, width, little_endian, 15)
}

open spec fn byte_store_v30(
    memory: ByteMemoryV30, pointer: MemoryPointerV30, width: int,
    value: int, little_endian: bool,
) -> ByteMemoryV30
    recommends byte_memory_well_formed_v30(memory), ordinary_memory_width_v30(width),
        byte_range_live_v30(memory, pointer, width), 0 <= value < memory_value_modulus_v30(width),
{
    let previous = memory.live[pointer.allocation];
    let bytes = Seq::new(previous.bytes.len(), |i: int|
        if pointer.byte_offset <= i < pointer.byte_offset + width {
            let ordinal = i - pointer.byte_offset;
            MemoryByteV37::Octet((value / byte_scale_v30(if little_endian { ordinal } else { width - 1 - ordinal })) % 256)
        } else { previous.bytes[i] });
    let initialized = Seq::new(previous.initialized.len(), |i: int|
        previous.initialized[i] || pointer.byte_offset <= i < pointer.byte_offset + width);
    ByteMemoryV30 { live: memory.live.insert(pointer.allocation, MemoryBytesV30 {
        bytes, initialized, base_alignment: previous.base_alignment,
        write_clock: byte_write_clock_v38(previous, width),
        write_epochs: byte_overwrite_epochs_v38(previous, pointer.byte_offset, width),
        relocations: byte_relocations_without_overlap_v37(previous.relocations, pointer.byte_offset, width),
    }), view_contracts: memory.view_contracts }
}

open spec fn vector_lane_shape_v30(lanes: int, factor: int) -> bool {
    0 < lanes && 0 < factor && factor <= lanes && lanes % factor == 0
}

open spec fn vector_physical_lane_v30(logical: int, lanes: int, factor: int) -> int
    recommends vector_lane_shape_v30(lanes, factor), 0 <= logical < lanes,
{
    (logical % factor) * (lanes / factor) + logical / factor
}

open spec fn vector_logical_lane_v30(physical: int, lanes: int, factor: int) -> int
    recommends vector_lane_shape_v30(lanes, factor), 0 <= physical < lanes,
{
    (physical % (lanes / factor)) * factor + physical / (lanes / factor)
}

// Fixed-lane vectors preserve every lane's bits and checked physical order.
// Factor 1 is contiguous; all other factors come from the exact vector type.
// The generated endpoint supplies its exact checked lane width/count/layout.
// Layout extent may exceed this payload width: these operations preserve all
// trailing padding bytes and their previous initializedness. The independent
// endpoint obligation still bounds the complete original leaf layout.
open spec fn byte_vector_load_v30(
    memory: ByteMemoryV30, pointer: MemoryPointerV30, lane_width: int,
    lanes: int, factor: int, little_endian: bool,
) -> Seq<int>
    recommends ordinary_memory_width_v30(lane_width), vector_lane_shape_v30(lanes, factor),
        byte_scalar_range_initialized_v37(memory, pointer, lane_width * lanes),
{
    Seq::new(lanes as nat, |lane: int| byte_load_v30(memory, MemoryPointerV30 {
        allocation: pointer.allocation,
        byte_offset: pointer.byte_offset + vector_physical_lane_v30(lane, lanes, factor) * lane_width,
        view: pointer.view,
    }, lane_width, little_endian))
}

open spec fn byte_vector_store_v30(
    memory: ByteMemoryV30, pointer: MemoryPointerV30, lane_width: int,
    values: Seq<int>, factor: int, little_endian: bool,
) -> ByteMemoryV30
    recommends byte_memory_well_formed_v30(memory), ordinary_memory_width_v30(lane_width),
        vector_lane_shape_v30(values.len() as int, factor),
        byte_range_live_v30(memory, pointer, lane_width * values.len()),
        forall|lane: int| 0 <= lane < values.len() ==>
            0 <= values[lane] < memory_value_modulus_v30(lane_width),
{
    let previous = memory.live[pointer.allocation];
    let bytes = Seq::new(previous.bytes.len(), |i: int|
        if pointer.byte_offset <= i < pointer.byte_offset + lane_width * values.len() {
            let relative = i - pointer.byte_offset;
            let ordinal = relative % lane_width;
            let logical = vector_logical_lane_v30(relative / lane_width, values.len() as int, factor);
            let value = values[logical];
            MemoryByteV37::Octet((value / byte_scale_v30(if little_endian { ordinal } else { lane_width - 1 - ordinal })) % 256)
        } else { previous.bytes[i] });
    let initialized = Seq::new(previous.initialized.len(), |i: int|
        previous.initialized[i] || pointer.byte_offset <= i < pointer.byte_offset + lane_width * values.len());
    ByteMemoryV30 { live: memory.live.insert(pointer.allocation, MemoryBytesV30 {
        bytes, initialized, base_alignment: previous.base_alignment,
        write_clock: byte_write_clock_v38(previous, lane_width * values.len()),
        write_epochs: byte_overwrite_epochs_v38(previous, pointer.byte_offset, lane_width * values.len()),
        relocations: byte_relocations_without_overlap_v37(previous.relocations, pointer.byte_offset, lane_width * values.len()),
    }), view_contracts: memory.view_contracts }
}

// A static Alloca site may execute repeatedly. The generated allocator step
// supplies a fresh per-site generation and advances its exact dynamic counter.
// Older generations are not removed: their lifetime is a separate event.
open spec fn byte_allocate_v30(
    memory: ByteMemoryV30, allocation: MemoryAllocationV30, extent: int, alignment: int,
) -> ByteMemoryV30
    recommends !memory.live.contains_key(allocation), 0 <= extent, 0 < alignment,
{
    ByteMemoryV30 { live: memory.live.insert(allocation, MemoryBytesV30 {
        bytes: Seq::new(extent as nat, |i: int| MemoryByteV37::Octet(0)),
        initialized: Seq::new(extent as nat, |i: int| false),
        relocations: Map::empty(),
        write_clock: 0,
        write_epochs: Seq::new(extent as nat, |i: int| 0),
        base_alignment: alignment,
    }), view_contracts: memory.view_contracts }
}

open spec fn byte_end_lifetime_v30(
    memory: ByteMemoryV30, allocation: MemoryAllocationV30,
) -> ByteMemoryV30 {
    ByteMemoryV30 { live: memory.live.remove(allocation), view_contracts: memory.view_contracts }
}

open spec fn private_generation_v30(
    generations: Map<MemoryPrivateSiteV30, int>, site: MemoryPrivateSiteV30,
) -> int {
    if generations.contains_key(site) { generations[site] } else { 0 }
}

open spec fn private_allocation_v30(
    site: MemoryPrivateSiteV30, generation: int,
) -> MemoryAllocationV30 {
    MemoryAllocationV30::Private {
        owner: site.owner, invocation: site.invocation, site: site.site, generation,
    }
}

open spec fn private_generation_counters_valid_v30(
    generations: Map<MemoryPrivateSiteV30, int>, memory: ByteMemoryV30,
) -> bool {
    (forall|site: MemoryPrivateSiteV30| generations.contains_key(site) ==>
        0 <= generations[site])
    && forall|site: MemoryPrivateSiteV30, generation: int|
        memory.live.contains_key(private_allocation_v30(site, generation)) ==>
            0 <= generation < private_generation_v30(generations, site)
}

open spec fn byte_private_frames_live_v30(
    memory: ByteMemoryV30, frames: MemoryFrameRuntimeV30,
) -> bool {
    forall|allocation: MemoryAllocationV30| memory.live.contains_key(allocation) ==>
        match allocation {
            MemoryAllocationV30::External { identity: _, generation } => 0 <= generation,
            MemoryAllocationV30::Private { owner, invocation, site, generation } =>
                0 <= owner && 0 <= invocation && 0 <= generation
                && 0 <= site.function && 0 <= site.block && 0 <= site.operation
                && exists|i: int| 0 <= i < frames.active.len()
                    && frames.active[i] == MemoryDynamicFrameV30 { owner, invocation },
        }
}

open spec fn byte_state_memory_well_formed_v30(state: MemoryStateV30) -> bool {
    byte_memory_well_formed_v30(state.memory)
        && byte_frame_runtime_well_formed_v30(state.frames)
        && byte_private_frames_live_v30(state.memory, state.frames)
        && private_generation_counters_valid_v30(state.generations, state.memory)
}
"#,
    include_str!("mixed_optimizer_byte_relocations_v37.vrs"),
    include_str!("mixed_optimizer_byte_views_v38.vrs"),
    include_str!("mixed_optimizer_float_values_v52.vrs")
);
