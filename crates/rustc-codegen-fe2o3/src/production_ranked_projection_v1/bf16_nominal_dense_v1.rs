//! C2 observational dense propagation. Actual source-preparation inputs are
//! borrowed, not authority. No normal/ranked receipt or HashMap storage proof.
use super::bf16_nominal_call_routing_v1::NominalCallVisitorV1;
use super::bf16_nominal_capabilities_v1::{
    AuthenticatedNominalCallerV1, NominalCallerSiteV1, NominalTransferOutcomeV1,
    authenticate_nominal_call_v1,
};
use super::capability_state_access_v1::{CapabilityStateAccessV1, DenseCapabilityStateV1};
use super::{
    AllocationContractV1, ProductionRankedProjectionErrorV1,
    ProjectedCapabilityTerminatorEffectsV1, ProjectedCapabilityValueV1, ProjectedMfmaOperandV1,
    SemanticCallableDeclV1, SemanticCompilerIntrinsicOperationV1, SemanticDirectCallV1,
    SemanticEnumPayloadDominanceV1, SemanticFunctionDeclV1, SemanticSourceProvenanceV1,
    SemanticStatementKindV1, SemanticTerminatorKindV1, charge_capability_dataflow_work_v1,
    checked_capability_stored_entries_v1, consume_capability_operands_v1,
    merge_capability_values_v1, transfer_capability_statements_v1,
    transfer_capability_terminator_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1,
};
use fe2o3_lower_mir_kernel::Bf16NominalCallQueryErrorV1 as Error;
use std::collections::HashMap;
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

type Result<T> = std::result::Result<T, Error>;
type Slot = Option<ProjectedCapabilityValueV1>;
const MAX_BLOCKS: usize = 32;
const MAX_LOCALS: usize = 4096;
const MAX_STATEMENTS: usize = 4096;

/// Observational inputs ONLY. Their actual same-source factory and retained
/// original-ledger ownership must enclose this driver. This constructor makes no
/// claim that arbitrary supplied dominance/allocation/constant data is genuine.
pub(super) struct NominalCapabilityInputsV1<'a> {
    function: &'a SemanticFunctionDeclV1,
    enum_payload_dominance: &'a SemanticEnumPayloadDominanceV1,
    local_allocations: &'a [Option<AllocationContractV1>],
    constants: &'a [Option<u64>],
}
impl<'a> NominalCapabilityInputsV1<'a> {
    pub(super) fn from_borrowed_source_v1(
        function: &'a SemanticFunctionDeclV1,
        enum_payload_dominance: &'a SemanticEnumPayloadDominanceV1,
        local_allocations: &'a [Option<AllocationContractV1>],
        constants: &'a [Option<u64>],
    ) -> Self {
        Self {
            function,
            enum_payload_dominance,
            local_allocations,
            constants,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum NominalCapabilityPassV1 {
    Initial,
    Repeated,
    Final,
}
impl NominalCapabilityPassV1 {
    const fn index(self) -> usize {
        match self {
            Self::Initial => 0,
            Self::Repeated => 1,
            Self::Final => 2,
        }
    }
}
pub(super) type NominalCapabilityVisitV1<'v> = dyn for<'a, 'w> FnMut(
        NominalCapabilityPassV1,
        &AuthenticatedNominalCallerV1<'a>,
        &mut Budget<'w>,
    ) -> Result<()>
    + 'v;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct NominalCapabilityRunV1 {
    pub(super) query_visits: [usize; 3],
    pub(super) authenticated_visits: [usize; 3],
    pub(super) reached_blocks: [usize; 3],
    pub(super) array_destination_has_origin: bool,
}
#[derive(Clone, Copy)]
pub(super) struct NominalCapabilityLedgerV1 {
    pub(super) slot: usize,
    pub(super) identity: CanonicalKernelIrWorkLedgerIdentityV1,
    pub(super) work: usize,
    pub(super) storage: usize,
    pub(super) peak: usize,
    pub(super) denied_work: bool,
    pub(super) denied_storage: bool,
}

/// Only the private real-facts child implements this in production. No fresh
/// Budget, unchecked receipt, generic facts fallback or ranked constructor.
pub(super) trait NominalCapabilityConsumerV1 {
    fn charge_work_v1(&mut self, amount: usize) -> Result<()>;
    fn reserve_storage_v1(&mut self, amount: usize) -> Result<()>;
    fn release_storage_v1(&mut self, amount: usize) -> Result<()>;
    fn ledger_v1(&self) -> NominalCapabilityLedgerV1;
    fn with_nominal_call_v1(
        &mut self,
        block: usize,
        call: &SemanticDirectCallV1,
        source: SemanticSourceProvenanceV1,
        visit: &mut NominalCallVisitorV1<'_>,
    ) -> Result<()>;
}
fn require(condition: bool, reason: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Unavailable(reason))
    }
}
fn add(a: usize, b: usize) -> Result<usize> {
    a.checked_add(b)
        .ok_or(Error::Resource(Resource::Arithmetic))
}
fn mul(a: usize, b: usize) -> Result<usize> {
    a.checked_mul(b)
        .ok_or(Error::Resource(Resource::Arithmetic))
}
fn transfer_error(error: ProductionRankedProjectionErrorV1) -> Error {
    // The extracted shared transfer helpers have only these two static variants;
    // they neither query the canonical ledger nor allocate on the dense backing.
    match error {
        ProductionRankedProjectionErrorV1::Incomplete(reason)
        | ProductionRankedProjectionErrorV1::Unsupported(reason) => Error::Unavailable(reason),
        _ => Error::Unavailable("nominal shared capability transfer contract changed"),
    }
}
fn charge(
    consumer: &mut dyn NominalCapabilityConsumerV1,
    legacy_work: &mut usize,
    amount: usize,
) -> Result<()> {
    consumer.charge_work_v1(amount)?;
    charge_capability_dataflow_work_v1(legacy_work, amount).map_err(transfer_error)
}
fn bytes<T>(count: usize) -> Result<usize> {
    mul(count, size_of::<T>())
}

struct Frames {
    initial: Option<Vec<Slot>>,
    repeated: Vec<Slot>,
    initial_reached: Option<Vec<bool>>,
    repeated_reached: Vec<bool>,
    scratch: Vec<Slot>,
    edges: Vec<u32>,
    indegree: Vec<u8>,
    order: Vec<usize>,
    effects: Vec<ProjectedCapabilityTerminatorEffectsV1>,
    pipeline_owners: Vec<Option<usize>>,
    pipeline_payloads: HashMap<usize, ProjectedMfmaOperandV1>,
}
#[derive(Clone, Copy)]
struct Shape {
    blocks: usize,
    locals: usize,
    slots: usize,
    first_payload: usize,
    total: usize,
}
fn shape<R>(blocks: usize, locals: usize) -> Result<Shape> {
    require(
        (1..=MAX_BLOCKS).contains(&blocks) && (1..=MAX_LOCALS).contains(&locals),
        "nominal dense source exceeds its closed block/local profile",
    )?;
    let slots = mul(blocks, locals)?;
    let first_payload = add(bytes::<Slot>(slots)?, bytes::<bool>(blocks)?)?;
    let mut total = mul(first_payload, 2)?;
    for amount in [
        bytes::<Slot>(locals)?,
        bytes::<u32>(blocks)?,
        bytes::<u8>(blocks)?,
        bytes::<usize>(blocks)?,
        bytes::<ProjectedCapabilityTerminatorEffectsV1>(blocks)?,
        bytes::<Option<usize>>(locals)?,
        size_of::<Frames>(),
        size_of::<Shape>(),
        size_of::<NominalCapabilityInputsV1<'static>>(),
        size_of::<NominalCallerSiteV1<'static>>(),
        size_of::<DenseCapabilityStateV1<'static>>(),
        size_of::<NominalCapabilityRunV1>(),
        mul(size_of::<Result<(NominalCapabilityRunV1, R)>>(), 2)?,
        // Fixed traversal/closure/header envelope, not allocator/RSS authority.
        4096,
    ] {
        total = add(total, amount)?;
    }
    Ok(Shape {
        blocks,
        locals,
        slots,
        first_payload,
        total,
    })
}
fn initialized<T>(
    count: usize,
    mut value: impl FnMut() -> T,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    work: &mut usize,
) -> Result<Vec<T>> {
    charge(consumer, work, add(count, 1)?)?;
    let mut result = Vec::new();
    result
        .try_reserve_exact(count)
        .map_err(|_| Error::Resource(Resource::Allocation))?;
    // Logical accounting is for exact vector capacities, not hidden allocator
    // buckets. If a platform grants a different capacity, drop it and refuse.
    require_capacity(result.capacity(), count)?;
    result.resize_with(count, &mut value);
    Ok(result)
}
fn require_capacity(actual: usize, expected: usize) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err(Error::Resource(Resource::Allocation))
    }
}
impl Frames {
    fn new(
        s: Shape,
        consumer: &mut dyn NominalCapabilityConsumerV1,
        work: &mut usize,
    ) -> Result<Self> {
        let mut order = initialized(s.blocks, || 0usize, consumer, work)?;
        order.clear(); // capacity remains the exact prepaid B.
        Ok(Self {
            initial: Some(initialized(s.slots, || None, consumer, work)?),
            repeated: initialized(s.slots, || None, consumer, work)?,
            initial_reached: Some(initialized(s.blocks, || false, consumer, work)?),
            repeated_reached: initialized(s.blocks, || false, consumer, work)?,
            scratch: initialized(s.locals, || None, consumer, work)?,
            edges: initialized(s.blocks, || 0, consumer, work)?,
            indegree: initialized(s.blocks, || 0, consumer, work)?,
            order,
            effects: initialized(
                s.blocks,
                ProjectedCapabilityTerminatorEffectsV1::default,
                consumer,
                work,
            )?,
            pipeline_owners: initialized(s.locals, || None, consumer, work)?,
            // Empty, immutable and never reserved/inserted in this closed profile.
            pipeline_payloads: HashMap::new(),
        })
    }
}

/// Reserve ALL owned tables before their first allocation; real source inputs
/// remain separately paid by their factory. Returned values are observational.
pub(super) fn run_nominal_capability_dataflow_v1<R: Copy + 'static>(
    site: &NominalCallerSiteV1<'_>,
    inputs: NominalCapabilityInputsV1<'_>,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    visit: &mut NominalCapabilityVisitV1<'_>,
    inspect: impl FnOnce(
        &[ProjectedCapabilityTerminatorEffectsV1],
        NominalCapabilityRunV1,
        &mut dyn NominalCapabilityConsumerV1,
    ) -> Result<R>,
) -> Result<(NominalCapabilityRunV1, R)> {
    let before = consumer.ledger_v1();
    if before.denied_work || before.denied_storage {
        return Err(Error::Resource(Resource::Accounting));
    }
    let mut work = 0;
    charge(consumer, &mut work, 8)?;
    require(
        std::ptr::eq(site.function(), inputs.function),
        "nominal capability inputs belong to another source function",
    )?;
    let s = shape::<R>(
        inputs.function.blocks().len(),
        inputs.function.locals().len(),
    )?;
    require(
        inputs.local_allocations.len() == s.locals && inputs.constants.len() == s.locals,
        "nominal source capability inputs differ from the actual local table",
    )?;
    require(
        (inputs.function.entry().index() as usize) < s.blocks,
        "nominal dense source entry is outside its CFG",
    )?;
    let mut statements = 0;
    for block in inputs.function.blocks() {
        charge(consumer, &mut work, 1)?;
        statements = add(statements, block.statements().len())?;
        require(
            statements <= MAX_STATEMENTS,
            "nominal dense source exceeds its closed statement profile",
        )?;
    }
    with_owned_storage(consumer, s.total, |consumer, owned| {
        let mut frames = Frames::new(s, consumer, &mut work)?;
        prepare_cfg(site, s, &mut frames, consumer, &mut work)?;
        let mut run = NominalCapabilityRunV1::default();
        let callables = site.owner().semantic_ssa().source_semantic().callables();
        propagate(
            NominalCapabilityPassV1::Initial,
            site,
            &inputs,
            s,
            callables,
            frames.initial.as_mut().expect("owned initial table"),
            frames
                .initial_reached
                .as_mut()
                .expect("owned initial reachability"),
            &mut frames.scratch,
            &frames.edges,
            &frames.order,
            &frames.pipeline_owners,
            &frames.pipeline_payloads,
            consumer,
            &mut work,
            visit,
            &mut run,
        )?;
        // Closed roster has no pipeline operation. Preserve the second real
        // propagation rather than using its absence to skip a required pass.
        propagate(
            NominalCapabilityPassV1::Repeated,
            site,
            &inputs,
            s,
            callables,
            &mut frames.repeated,
            &mut frames.repeated_reached,
            &mut frames.scratch,
            &frames.edges,
            &frames.order,
            &frames.pipeline_owners,
            &frames.pipeline_payloads,
            consumer,
            &mut work,
            visit,
            &mut run,
        )?;
        drop(frames.initial.take());
        drop(frames.initial_reached.take());
        // Only first-pass allocation payloads are gone; all frame/vector headers
        // remain included in the outer owned charge until the entire scope ends.
        consumer.release_storage_v1(s.first_payload)?;
        *owned = owned
            .checked_sub(s.first_payload)
            .ok_or(Error::Resource(Resource::Accounting))?;
        replay(
            site,
            &inputs,
            s,
            callables,
            &frames.repeated,
            &frames.repeated_reached,
            &mut frames.scratch,
            &frames.order,
            &frames.pipeline_owners,
            &frames.pipeline_payloads,
            &mut frames.effects,
            consumer,
            &mut work,
            visit,
            &mut run,
        )?;
        require(
            run.query_visits == [1, 1, 1] && run.authenticated_visits[2] == 1,
            "nominal three-pass replay did not visit its sole actual call exactly once per pass",
        )?;
        require(
            !run.array_destination_has_origin,
            "nominal scalar array acquired a capability origin",
        )?;
        let observation = inspect(&frames.effects, run, consumer)?;
        // Every owned vector/effect and any borrowed query product is dropped
        // here before the wrapper releases its remaining owned reservation.
        drop(frames);
        Ok((run, observation))
    })
}

fn with_owned_storage<R: Copy + 'static>(
    consumer: &mut dyn NominalCapabilityConsumerV1,
    storage: usize,
    body: impl FnOnce(&mut dyn NominalCapabilityConsumerV1, &mut usize) -> Result<R>,
) -> Result<R> {
    let before = consumer.ledger_v1();
    if before.denied_work || before.denied_storage {
        return Err(Error::Resource(Resource::Accounting));
    }
    consumer.reserve_storage_v1(storage)?;
    let mut owned = storage;
    let outcome = catch_unwind(AssertUnwindSafe(|| body(consumer, &mut owned)));
    // The body must destroy its complete owned graph before returning, including
    // unwind paths. Only the private first-pass drop may reduce owned.
    let after = consumer.ledger_v1();
    let protected = add(before.storage, owned)?;
    if before.slot != after.slot
        || before.identity != after.identity
        || after.storage < protected
        || after.work < before.work
        || after.peak < before.peak
    {
        drop(outcome);
        return Err(Error::Resource(Resource::Accounting));
    }
    let denied = after.denied_work || after.denied_storage;
    let result = match outcome {
        Ok(Ok(_)) if denied => Err(Error::Resource(Resource::Accounting)),
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(Error::CallbackPanicked)
        }
    };
    consumer.release_storage_v1(owned)?;
    result
}

fn prepare_cfg(
    site: &NominalCallerSiteV1<'_>,
    s: Shape,
    frames: &mut Frames,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    work: &mut usize,
) -> Result<()> {
    let function = site.function();
    let callables = site.owner().semantic_ssa().source_semantic().callables();
    let mut nominal_calls = 0;
    for (block_index, block) in function.blocks().iter().enumerate() {
        charge(consumer, work, 1)?;
        match block.terminator().kind() {
            SemanticTerminatorKindV1::TailCall(_) => {
                return Err(Error::Unavailable(
                    "nominal dense profile does not support tail calls",
                ));
            }
            SemanticTerminatorKindV1::Call(call) => {
                charge(consumer, work, 2)?;
                match callables.get(call.callee().index() as usize) {
                    Some(SemanticCallableDeclV1::Defined { .. }) => {
                        require(
                            block_index == site.block().index() as usize
                                && std::ptr::eq(call, site.call()),
                            "nominal dense profile contains another Defined call",
                        )?;
                        nominal_calls += 1;
                    }
                    Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) => {
                        require(
                            !matches!(
                                operation,
                                SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { .. }
                                    | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent { .. }
                                    | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite { .. }
                                    | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead { .. }
                            ),
                            "nominal dense profile contains a workgroup pipeline operation",
                        )?;
                    }
                    _ => {
                        return Err(Error::Unavailable(
                            "nominal dense profile contains an unclassified external call",
                        ));
                    }
                }
            }
            _ => {}
        }
        block
            .terminator()
            .kind()
            .try_for_each_edge(|edge| -> Result<()> {
                charge(consumer, work, 2)?;
                let target = edge.target().index() as usize;
                require(
                    target < s.blocks,
                    "nominal dense CFG edge is outside the source function",
                )?;
                let bit = 1u32 << target;
                if frames.edges[block_index] & bit == 0 {
                    frames.edges[block_index] |= bit;
                    frames.indegree[target] = frames.indegree[target]
                        .checked_add(1)
                        .ok_or(Error::Resource(Resource::Arithmetic))?;
                }
                Ok(())
            })?;
    }
    require(
        nominal_calls == 1,
        "nominal dense profile lacks its sole Defined call",
    )?;
    order_cfg(
        &frames.edges,
        &mut frames.indegree,
        &mut frames.order,
        consumer,
        work,
    )
}

fn order_cfg(
    edges_by_source: &[u32],
    indegree: &mut [u8],
    order: &mut Vec<usize>,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    work: &mut usize,
) -> Result<()> {
    let blocks = edges_by_source.len();
    require(
        blocks <= MAX_BLOCKS
            && indegree.len() == blocks
            && order.capacity() == blocks
            && order.is_empty(),
        "nominal topological tables differ from their prepaid shape",
    )?;
    for (block, &degree) in indegree.iter().enumerate() {
        charge(consumer, work, 1)?;
        if degree == 0 {
            order.push(block);
        }
    }
    let mut cursor = 0;
    while cursor < order.len() {
        charge(consumer, work, 1)?;
        let block = order[cursor];
        cursor += 1;
        let mut edges = edges_by_source[block];
        while edges != 0 {
            charge(consumer, work, 2)?;
            let target = edges.trailing_zeros() as usize;
            edges &= edges - 1;
            indegree[target] = indegree[target]
                .checked_sub(1)
                .ok_or(Error::Resource(Resource::Accounting))?;
            if indegree[target] == 0 {
                order.push(target);
            }
        }
    }
    require(
        order.len() == blocks,
        "nominal dense source CFG contains a cycle",
    )
}

fn prepay_block(
    function: &SemanticFunctionDeclV1,
    block: usize,
    locals: usize,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    work: &mut usize,
) -> Result<()> {
    // Fixed statement/contract checks include bounded enum binary-search work.
    // Each dynamic aggregate operand is charged before the actual shared scan.
    for statement in function.blocks()[block].statements() {
        charge(consumer, work, 64)?;
        if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
            assignment
                .value()
                .kind()
                .try_visit_operands(|_| charge(consumer, work, 8))?;
        }
    }
    let arguments = match function.blocks()[block].terminator().kind() {
        SemanticTerminatorKindV1::Call(call) => call.arguments().len(),
        SemanticTerminatorKindV1::TailCall(call) => call.arguments().len(),
        _ => 1,
    };
    // Prepay full dense iteration, even though the closed BF16 roster does not
    // use the gfx950 zero-sized-token fallback scan in the shared core.
    charge(consumer, work, add(add(locals, mul(arguments, 8)?)?, 256)?)
}
fn populated(
    slots: &[Slot],
    consumer: &mut dyn NominalCapabilityConsumerV1,
    work: &mut usize,
) -> Result<usize> {
    charge(consumer, work, slots.len())?;
    Ok(slots.iter().filter(|slot| slot.is_some()).count())
}
fn merge_row(
    existing: &mut [Slot],
    incoming: &[Slot],
    consumer: &mut dyn NominalCapabilityConsumerV1,
    work: &mut usize,
) -> Result<usize> {
    require(
        existing.len() == incoming.len(),
        "nominal dense merge row lengths differ",
    )?;
    charge(consumer, work, mul(existing.len(), 3)?)?;
    let mut additional = 0;
    for (current, next) in existing.iter_mut().zip(incoming.iter().copied()) {
        let merged = match (*current, next) {
            (Some(a), Some(b)) => Some(merge_capability_values_v1(a, b)),
            (Some(_), None) | (None, Some(_)) => Some(ProjectedCapabilityValueV1::Invalid),
            (None, None) => None,
        };
        additional += usize::from(current.is_none() && merged.is_some());
        *current = merged;
    }
    Ok(additional)
}
fn transfer_nominal(
    pass: NominalCapabilityPassV1,
    site: &NominalCallerSiteV1<'_>,
    state: &mut DenseCapabilityStateV1<'_>,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    visit: &mut NominalCapabilityVisitV1<'_>,
    run: &mut NominalCapabilityRunV1,
) -> Result<ProjectedCapabilityTerminatorEffectsV1> {
    state.check_bounds_v1()?;
    let index = pass.index();
    let mut invoked = false;
    let mut authenticated = false;
    {
        let mut visitor =
            |candidate: &super::bf16_nominal_call_projection_v1::CheckedNominalCallProjectionV1<
                '_,
            >,
             budget: &mut Budget<'_>|
             -> Result<()> {
                require(
                    !invoked,
                    "nominal capability query invoked its visitor more than once",
                )?;
                invoked = true;
                match authenticate_nominal_call_v1(candidate, site, state, budget)? {
                    NominalTransferOutcomeV1::Invalid(reason) => {
                        if pass == NominalCapabilityPassV1::Final {
                            return Err(Error::Unavailable(reason));
                        }
                    }
                    NominalTransferOutcomeV1::Authenticated(value) => {
                        visit(pass, &value, budget)?;
                        authenticated = true;
                    }
                }
                Ok(())
            };
        consumer.with_nominal_call_v1(
            site.block().index() as usize,
            site.call(),
            site.source(),
            &mut visitor,
        )?;
    }
    require(
        invoked,
        "nominal capability query did not invoke its visitor",
    )?;
    state.check_bounds_v1()?;
    run.query_visits[index] = add(run.query_visits[index], 1)?;
    run.authenticated_visits[index] =
        add(run.authenticated_visits[index], usize::from(authenticated))?;
    // No mutation occurs until the complete canonical query, callback and its
    // custody checks have succeeded. Consume the actual caller arguments ONCE.
    consume_capability_operands_v1(state, site.call().arguments());
    let destination = site.destination().local().index() as usize;
    state.remove(&destination);
    run.array_destination_has_origin |= state.contains_key(&destination);
    state.check_bounds_v1()?;
    Ok(ProjectedCapabilityTerminatorEffectsV1::default())
}

#[allow(clippy::too_many_arguments)]
fn block_transfer(
    pass: NominalCapabilityPassV1,
    site: &NominalCallerSiteV1<'_>,
    inputs: &NominalCapabilityInputsV1<'_>,
    block: usize,
    scratch: &mut [Slot],
    callables: &[SemanticCallableDeclV1],
    pipeline_owners: &[Option<usize>],
    pipeline_payloads: &HashMap<usize, ProjectedMfmaOperandV1>,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    work: &mut usize,
    visit: &mut NominalCapabilityVisitV1<'_>,
    run: &mut NominalCapabilityRunV1,
) -> Result<ProjectedCapabilityTerminatorEffectsV1> {
    prepay_block(inputs.function, block, scratch.len(), consumer, work)?;
    let mut state = DenseCapabilityStateV1::new(scratch);
    transfer_capability_statements_v1(
        inputs.function,
        block,
        &mut state,
        inputs.enum_payload_dominance,
    )
    .map_err(transfer_error)?;
    state.check_bounds_v1()?;
    let effects = if block == site.block().index() as usize {
        transfer_nominal(pass, site, &mut state, consumer, visit, run)?
    } else {
        transfer_capability_terminator_v1(
            callables,
            inputs.function,
            block,
            &mut state,
            inputs.local_allocations,
            inputs.constants,
            pipeline_owners,
            pipeline_payloads,
            pass == NominalCapabilityPassV1::Final,
        )
        .map_err(transfer_error)?
    };
    state.check_bounds_v1()?;
    Ok(effects)
}

#[allow(clippy::too_many_arguments)]
fn propagate(
    pass: NominalCapabilityPassV1,
    site: &NominalCallerSiteV1<'_>,
    inputs: &NominalCapabilityInputsV1<'_>,
    s: Shape,
    callables: &[SemanticCallableDeclV1],
    entries: &mut [Slot],
    reached: &mut [bool],
    scratch: &mut [Slot],
    edges: &[u32],
    order: &[usize],
    pipeline_owners: &[Option<usize>],
    pipeline_payloads: &HashMap<usize, ProjectedMfmaOperandV1>,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    work: &mut usize,
    visit: &mut NominalCapabilityVisitV1<'_>,
    run: &mut NominalCapabilityRunV1,
) -> Result<()> {
    let entry = inputs.function.entry().index() as usize;
    reached[entry] = true;
    let mut stored = 0;
    for &block in order {
        charge(consumer, work, 1)?;
        if !reached[block] {
            continue;
        }
        run.reached_blocks[pass.index()] = add(run.reached_blocks[pass.index()], 1)?;
        let start = mul(block, s.locals)?;
        charge(consumer, work, s.locals)?;
        scratch.copy_from_slice(&entries[start..start + s.locals]);
        let effects = block_transfer(
            pass,
            site,
            inputs,
            block,
            scratch,
            callables,
            pipeline_owners,
            pipeline_payloads,
            consumer,
            work,
            visit,
            run,
        )?;
        drop(effects);
        let count = populated(scratch, consumer, work)?;
        checked_capability_stored_entries_v1(0, count).map_err(transfer_error)?;
        let mut successors = edges[block];
        while successors != 0 {
            charge(consumer, work, 1)?;
            let target = successors.trailing_zeros() as usize;
            successors &= successors - 1;
            let start = mul(target, s.locals)?;
            let row = &mut entries[start..start + s.locals];
            let additional = if reached[target] {
                merge_row(row, scratch, consumer, work)?
            } else {
                charge(consumer, work, s.locals)?;
                row.copy_from_slice(scratch);
                reached[target] = true;
                count
            };
            stored =
                checked_capability_stored_entries_v1(stored, additional).map_err(transfer_error)?;
        }
    }
    require(
        run.query_visits[pass.index()] == 1,
        "nominal propagation did not reach its sole source call",
    )
}

#[allow(clippy::too_many_arguments)]
fn replay(
    site: &NominalCallerSiteV1<'_>,
    inputs: &NominalCapabilityInputsV1<'_>,
    s: Shape,
    callables: &[SemanticCallableDeclV1],
    entries: &[Slot],
    reached: &[bool],
    scratch: &mut [Slot],
    order: &[usize],
    pipeline_owners: &[Option<usize>],
    pipeline_payloads: &HashMap<usize, ProjectedMfmaOperandV1>,
    effects: &mut [ProjectedCapabilityTerminatorEffectsV1],
    consumer: &mut dyn NominalCapabilityConsumerV1,
    work: &mut usize,
    visit: &mut NominalCapabilityVisitV1<'_>,
    run: &mut NominalCapabilityRunV1,
) -> Result<()> {
    for &block in order {
        charge(consumer, work, 1)?;
        if !reached[block] {
            continue;
        }
        run.reached_blocks[2] = add(run.reached_blocks[2], 1)?;
        let start = mul(block, s.locals)?;
        charge(consumer, work, s.locals)?;
        scratch.copy_from_slice(&entries[start..start + s.locals]);
        effects[block] = block_transfer(
            NominalCapabilityPassV1::Final,
            site,
            inputs,
            block,
            scratch,
            callables,
            pipeline_owners,
            pipeline_payloads,
            consumer,
            work,
            visit,
            run,
        )?;
        let count = populated(scratch, consumer, work)?;
        checked_capability_stored_entries_v1(0, count).map_err(transfer_error)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "bf16_nominal_dense_v1_tests.rs"]
mod tests;
