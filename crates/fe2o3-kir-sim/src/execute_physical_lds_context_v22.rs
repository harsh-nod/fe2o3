//! One profile-owned frame and publication context, not an executable shadow graph.
use super::*;
use fe2o3_kernel_ir::{BarrierSemantics, Convergence, Gfx942PhysicalLdsExchangeOpcodeV1 as Opcode};

pub(super) struct Context {
    pub(super) declaration: CompactSite,
    pub(super) memory: WorkgroupMemory,
    pub(super) barrier: WorkgroupBarrier,
}
#[derive(Clone)]
struct PendingWrite {
    pointer: PointerValue,
    bits: ScalarBitsV1,
    generation: CompactSite,
}
#[derive(Clone, Default)]
struct Slot {
    pending: Option<PendingWrite>,
    completed: bool,
}
pub(super) struct State {
    slots: Vec<Slot>,
    published: bool,
}

pub(super) fn resident_bytes() -> Option<usize> {
    let mut ledger = ResidentLedger::new(size_of::<Option<Context>>());
    ledger.add_btree_set::<AddressSpace>(1)?;
    ledger.add_bytes(reserved_vec_bytes::<Slot>(128)?)?;
    ledger.add_bytes(size_of::<PendingWrite>())?;
    Some(ledger.bytes())
}
impl Context {
    pub(super) fn new(module: &Module) -> Result<Self, SimulationExecutionErrorV1> {
        let invalid = || {
            top_level_error(SimulationExecutionErrorKindV1::InternalInvariant(
                "V22 context exact declaration",
            ))
        };
        let function = module.functions.first().ok_or_else(invalid)?;
        let block = function
            .body
            .as_ref()
            .and_then(|b| b.blocks.first())
            .ok_or_else(invalid)?;
        let operation = block.operations.first().ok_or_else(invalid)?;
        let OperationKind::Gfx942PhysicalLdsExchangeDeclaration(declaration) = &operation.kind
        else {
            return Err(invalid());
        };
        if declaration.validate_shape().is_err() {
            return Err(invalid());
        }
        Ok(Self {
            declaration: CompactSite {
                function: 0,
                block: block.id,
                operation: Some(0),
            },
            memory: WorkgroupMemory {
                element: Type::Scalar(ScalarType::U32),
                extent: WorkgroupMemoryExtent::Static(128),
                alignment: declaration.lds_frame.alignment,
            },
            barrier: WorkgroupBarrier {
                memory_scope: SynchronizationScope::Workgroup,
                semantics: BarrierSemantics::new(
                    MemoryOrdering::AcquireRelease,
                    [AddressSpace::Workgroup],
                ),
                convergence: Convergence::uniform(SynchronizationScope::Workgroup),
            },
        })
    }
}
impl State {
    pub(super) fn new() -> Result<Self, SimulationExecutionErrorV1> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(128)
            .map_err(|_| top_level_error(SimulationExecutionErrorKindV1::AllocationFailure))?;
        slots.resize_with(128, Slot::default);
        Ok(Self {
            slots,
            published: false,
        })
    }
}
fn failure(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    site: CompactSite,
    message: &'static str,
) -> SimulationExecutionErrorV1 {
    engine.at(
        site,
        SimulationExecutionErrorKindV1::InternalInvariant(message),
    )
}
fn lane(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    site: CompactSite,
) -> Result<usize, SimulationExecutionErrorV1> {
    let invocation = engine
        .invocation
        .ok_or_else(|| failure(engine, site, "V22 invocation"))?;
    if invocation.workgroup_size != [128, 1, 1]
        || invocation.local[0] >= 128
        || invocation.local[1..] != [0, 0]
    {
        return Err(failure(engine, site, "V22 exact two-wave participant"));
    }
    Ok(invocation.local[0] as usize)
}
pub(super) fn frame_pointer(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    site: CompactSite,
) -> Result<PointerValue, SimulationExecutionErrorV1> {
    let context = engine
        .physical_lds_context
        .ok_or_else(|| failure(engine, site, "V22 retained frame context"))?;
    engine.workgroup_pointer(context.declaration, &context.memory)
}
pub(super) fn issue_write(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    site: CompactSite,
    offset: u32,
    bits: ScalarBitsV1,
) -> Result<(), SimulationExecutionErrorV1> {
    let lane = lane(engine, site)?;
    let state = engine
        .physical_lds_state
        .as_ref()
        .ok_or_else(|| failure(engine, site, "V22 pending storage"))?;
    if offset as usize != lane * 4
        || bits.ty() != ScalarType::U32
        || state.published
        || state.slots[lane].pending.is_some()
        || state.slots[lane].completed
    {
        return Err(failure(engine, site, "V22 unique local LDS write"));
    }
    let base = frame_pointer(engine, site)?;
    let pointer = pointer_at_byte(engine, &base, offset as usize, ScalarType::U32, &site)?;
    // No memory write at issue: exact wait performs the real engine store.
    let Some(state) = engine.physical_lds_state.as_mut() else {
        return Err(failure(engine, site, "V22 retained pending storage"));
    };
    state.slots[lane].pending = Some(PendingWrite {
        pointer,
        bits,
        generation: site,
    });
    Ok(())
}
pub(super) fn complete_write(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    site: CompactSite,
    source: CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    let lane = lane(engine, site)?;
    let state = engine
        .physical_lds_state
        .as_ref()
        .ok_or_else(|| failure(engine, site, "V22 pending storage"))?;
    let pending = state.slots[lane]
        .pending
        .as_ref()
        .ok_or_else(|| failure(engine, site, "V22 missing pending write"))?;
    if state.published || state.slots[lane].completed || pending.generation != source {
        return Err(failure(engine, site, "V22 write completion generation"));
    }
    let pending = pending.clone();
    // The authored write site, not the wait, owns the actual memory effect/event.
    execute_pointer_store(
        engine,
        &pending.pointer,
        pending.bits,
        MemoryAccess::new(AddressSpace::Workgroup, 4),
        &source,
    )?;
    let Some(state) = engine.physical_lds_state.as_mut() else {
        return Err(failure(engine, site, "V22 retained completion storage"));
    };
    let slot = &mut state.slots[lane];
    slot.pending = None;
    slot.completed = true;
    Ok(())
}
pub(super) fn read(
    engine: &mut Engine<'_, impl SimulationEventSinkV1>,
    site: CompactSite,
    offset: u32,
) -> Result<ScalarBitsV1, SimulationExecutionErrorV1> {
    let lane = lane(engine, site)?;
    let state = engine
        .physical_lds_state
        .as_ref()
        .ok_or_else(|| failure(engine, site, "V22 publication state"))?;
    if !state.published || offset as usize != (lane ^ 64) * 4 {
        return Err(failure(
            engine,
            site,
            "V22 peer read before workgroup publication",
        ));
    }
    let base = frame_pointer(engine, site)?;
    let pointer = pointer_at_byte(engine, &base, offset as usize, ScalarType::U32, &site)?;
    execute_pointer_load(
        engine,
        &pointer,
        MemoryAccess::new(AddressSpace::Workgroup, 4),
        &site,
    )
}
pub(super) fn barrier<'a>(
    engine: &Engine<'a, impl SimulationEventSinkV1>,
    site: CompactSite,
) -> Result<&'a WorkgroupBarrier, SimulationExecutionErrorV1> {
    let lane = lane(engine, site)?;
    let state = engine
        .physical_lds_state
        .as_ref()
        .ok_or_else(|| failure(engine, site, "V22 barrier state"))?;
    if state.published || state.slots[lane].pending.is_some() || !state.slots[lane].completed {
        return Err(failure(
            engine,
            site,
            "V22 barrier does not complete pending LDS",
        ));
    }
    Ok(&engine
        .physical_lds_context
        .ok_or_else(|| failure(engine, site, "V22 barrier lifetime"))?
        .barrier)
}
/// Called only by the existing WG scheduler after it joins every actual arrival.
pub(super) fn validate_release(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    site: CompactSite,
    participants: u32,
) -> Result<bool, SimulationExecutionErrorV1> {
    if engine.physical_lds_context.is_none() {
        return Ok(false);
    }
    let (operation, _) = physical_global_copy_pending_v21::prior_operation(
        engine,
        site,
        site.operation
            .ok_or_else(|| failure(engine, site, "V22 barrier operation"))?,
    )?;
    let valid = matches!(&operation.kind, OperationKind::Gfx942PhysicalLdsExchangeStep(step) if step.instruction.opcode == Opcode::WorkgroupPublishBarrier);
    let state = engine
        .physical_lds_state
        .as_ref()
        .ok_or_else(|| failure(engine, site, "V22 release state"))?;
    if !valid
        || participants != 128
        || state.published
        || state.slots.len() != 128
        || state
            .slots
            .iter()
            .any(|slot| !slot.completed || slot.pending.is_some())
    {
        return Err(failure(
            engine,
            site,
            "V22 full workgroup completed-write publication",
        ));
    }
    Ok(true)
}
pub(super) fn mark_published(engine: &mut Engine<'_, impl SimulationEventSinkV1>) {
    if let Some(state) = &mut engine.physical_lds_state {
        state.published = true;
    }
}

pub(super) fn require_published(
    engine: &Engine<'_, impl SimulationEventSinkV1>,
    site: CompactSite,
) -> Result<(), SimulationExecutionErrorV1> {
    if !engine
        .physical_lds_state
        .as_ref()
        .is_some_and(|state| state.published)
    {
        return Err(failure(
            engine,
            site,
            "V22 LGKM read requires retained published epoch",
        ));
    }
    Ok(())
}
