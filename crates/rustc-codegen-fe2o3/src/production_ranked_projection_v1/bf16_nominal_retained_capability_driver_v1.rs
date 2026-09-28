//! Retained original-order two-pass nominal capability driver.
//! Observational DATA only; actual source-input preparation/factory remains external.
//! The entire owner must remain outside every checked-query postflight.
use super::super::{
    ProductionRankedOperationV1, ProjectedCapabilityReadEffectV1, ProjectedReadViewAccessV1,
    ProjectedTransposeWorkgroupEffectV1,
};
use super::retained_capability_fifo_v1::{CompletedFifoPassV1, RetainedCapabilityFifoPassV1};
use super::*;
type Identity = (usize, CanonicalKernelIrWorkLedgerIdentityV1);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Fresh,
    Terminal,
    Complete,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct InputsKey {
    function: usize,
    dominance: usize,
    allocations: usize,
    allocation_len: usize,
    constants: usize,
    constant_len: usize,
    call: usize,
}
fn input_key(site: &NominalCallerSiteV1<'_>, inputs: &NominalCapabilityInputsV1<'_>) -> InputsKey {
    InputsKey {
        function: inputs.function as *const SemanticFunctionDeclV1 as usize,
        dominance: inputs.enum_payload_dominance as *const SemanticEnumPayloadDominanceV1 as usize,
        allocations: inputs.local_allocations.as_ptr() as usize,
        allocation_len: inputs.local_allocations.len(),
        constants: inputs.constants.as_ptr() as usize,
        constant_len: inputs.constants.len(),
        call: site.call() as *const SemanticDirectCallV1 as usize,
    }
}
pub(in super::super) struct RetainedNominalCapabilityDriverV1 {
    phase: Phase,
    ledger: Option<Identity>,
    inputs: Option<InputsKey>,
    edges: Vec<u32>,
    indegree: Vec<u8>,
    validation_order: Vec<usize>,
    pipeline_owners: Vec<Option<usize>>,
    initial_payloads: HashMap<usize, ProjectedMfmaOperandV1>,
    pipeline_payloads: HashMap<usize, ProjectedMfmaOperandV1>,
    initial: RetainedCapabilityFifoPassV1,
    repeated: RetainedCapabilityFifoPassV1,
    replay_scratch: Vec<Slot>,
    layouts: Vec<Option<ProductionRankedOperationV1>>,
    global_reads: Vec<Option<AllocationContractV1>>,
    transpose_workgroups: Vec<Option<ProjectedTransposeWorkgroupEffectV1>>,
    read_views: Vec<Option<ProjectedReadViewAccessV1>>,
    bound_reads: Vec<Option<ProjectedCapabilityReadEffectV1>>,
    original_work: usize,
    run: NominalCapabilityRunV1,
    owner_scan_blocks: usize,
    alias_scan_locals: usize,
    payload_scan_blocks: usize,
    owner_scan_complete: bool,
    payload_scan_complete: bool,
}
pub(in super::super) struct CompletedNominalFifoDriverV1<'a> {
    pub(in super::super) initial_entries: &'a [Slot],
    pub(in super::super) initial_reached: &'a [bool],
    pub(in super::super) initial_visits: &'a [usize],
    pub(in super::super) repeated_entries: &'a [Slot],
    pub(in super::super) repeated_reached: &'a [bool],
    pub(in super::super) repeated_visits: &'a [usize],
    pub(in super::super) layouts: &'a [Option<ProductionRankedOperationV1>],
    pub(in super::super) global_reads: &'a [Option<AllocationContractV1>],
    pub(in super::super) transpose_workgroups: &'a [Option<ProjectedTransposeWorkgroupEffectV1>],
    pub(in super::super) read_views: &'a [Option<ProjectedReadViewAccessV1>],
    pub(in super::super) bound_reads: &'a [Option<ProjectedCapabilityReadEffectV1>],
    pub(in super::super) original_work: usize,
    pub(in super::super) run: NominalCapabilityRunV1,
    pub(in super::super) owner_scan_blocks: usize,
    pub(in super::super) alias_scan_locals: usize,
    pub(in super::super) payload_scan_blocks: usize,
}
impl RetainedNominalCapabilityDriverV1 {
    pub(in super::super) fn new() -> Self {
        Self {
            phase: Phase::Fresh,
            ledger: None,
            inputs: None,
            edges: Vec::new(),
            indegree: Vec::new(),
            validation_order: Vec::new(),
            pipeline_owners: Vec::new(),
            initial_payloads: HashMap::new(),
            pipeline_payloads: HashMap::new(),
            initial: RetainedCapabilityFifoPassV1::new(),
            repeated: RetainedCapabilityFifoPassV1::new(),
            replay_scratch: Vec::new(),
            layouts: Vec::new(),
            global_reads: Vec::new(),
            transpose_workgroups: Vec::new(),
            read_views: Vec::new(),
            bound_reads: Vec::new(),
            original_work: 0,
            run: NominalCapabilityRunV1::default(),
            owner_scan_blocks: 0,
            alias_scan_locals: 0,
            payload_scan_blocks: 0,
            owner_scan_complete: false,
            payload_scan_complete: false,
        }
    }
    pub(in super::super) fn prepare_into(
        &mut self,
        site: &NominalCallerSiteV1<'_>,
        inputs: &NominalCapabilityInputsV1<'_>,
        consumer: &mut dyn NominalCapabilityConsumerV1,
        owned: &mut usize,
        visit: &mut NominalCapabilityVisitV1<'_>,
    ) -> Result<()> {
        let fresh = self.phase == Phase::Fresh && self.ledger.is_none() && self.inputs.is_none();
        self.phase = Phase::Terminal;
        let ledger = consumer.ledger_v1();
        if !fresh || ledger.denied_work || ledger.denied_storage {
            return Err(Error::Resource(Resource::Accounting));
        }
        consumer.charge_work_v1(32)?;
        retain(consumer, owned, driver_frame()?)?;
        self.ledger = Some((ledger.slot, ledger.identity));
        self.inputs = Some(input_key(site, inputs));
        require(
            std::ptr::eq(site.function(), inputs.function),
            "nominal FIFO inputs belong to another source function",
        )?;
        let blocks = inputs.function.blocks().len();
        let locals = inputs.function.locals().len();
        require(
            (1..=MAX_BLOCKS).contains(&blocks) && (1..=MAX_LOCALS).contains(&locals),
            "nominal FIFO exceeds its closed block/local profile",
        )?;
        require(
            inputs.local_allocations.len() == locals && inputs.constants.len() == locals,
            "nominal FIFO source inputs differ from the actual local table",
        )?;
        require(
            (inputs.function.entry().index() as usize) < blocks,
            "nominal FIFO source entry is outside its CFG",
        )?;
        let callables = site.owner().semantic_ssa().source_semantic().callables();
        // Admission only. Its topological order is NEVER the propagation/replay order.
        self.prepare_profile(site, consumer, owned)?;
        fill_none(&mut self.pipeline_owners, locals, consumer, owned)?;
        owner_scan(
            inputs.function,
            callables,
            &mut self.pipeline_owners,
            &mut self.owner_scan_blocks,
            &mut self.alias_scan_locals,
            consumer,
        )?;
        self.owner_scan_complete = true;
        {
            let owners = &self.pipeline_owners;
            let payloads = &self.initial_payloads;
            let run = &mut self.run;
            self.initial.prepare_into(
                inputs.function,
                consumer,
                owned,
                &mut self.original_work,
                |block, scratch, consumer| {
                    run.reached_blocks[0] = add(run.reached_blocks[0], 1)?;
                    let effects = transfer_closed(
                        NominalCapabilityPassV1::Initial,
                        site,
                        inputs,
                        block,
                        scratch,
                        callables,
                        owners,
                        payloads,
                        consumer,
                        visit,
                        run,
                    )?;
                    drop(effects);
                    Ok(())
                },
            )?;
        }
        require(
            self.run.query_visits[0] > 0,
            "nominal initial FIFO did not reach its actual call",
        )?;
        let initial = self.initial.completed_for(inputs.function, consumer)?;
        payload_scan(
            inputs.function,
            callables,
            &initial,
            &self.pipeline_owners,
            &mut self.pipeline_payloads,
            &mut self.payload_scan_blocks,
            consumer,
        )?;
        self.payload_scan_complete = true;
        {
            let owners = &self.pipeline_owners;
            let payloads = &self.pipeline_payloads;
            let run = &mut self.run;
            self.repeated.prepare_into(
                inputs.function,
                consumer,
                owned,
                &mut self.original_work,
                |block, scratch, consumer| {
                    run.reached_blocks[1] = add(run.reached_blocks[1], 1)?;
                    let effects = transfer_closed(
                        NominalCapabilityPassV1::Repeated,
                        site,
                        inputs,
                        block,
                        scratch,
                        callables,
                        owners,
                        payloads,
                        consumer,
                        visit,
                        run,
                    )?;
                    drop(effects);
                    Ok(())
                },
            )?;
        }
        require(
            self.run.query_visits[1] > 0,
            "nominal repeated FIFO did not reach its actual call",
        )?;
        // Preserve original final-array source order. Both FIFO owners stay attached.
        fill_none(&mut self.layouts, blocks, consumer, owned)?;
        fill_none(&mut self.global_reads, blocks, consumer, owned)?;
        fill_none(&mut self.transpose_workgroups, blocks, consumer, owned)?;
        fill_none(&mut self.read_views, blocks, consumer, owned)?;
        fill_none(&mut self.replay_scratch, locals, consumer, owned)?;
        let repeated = self.repeated.completed_for(inputs.function, consumer)?;
        // This is source-index order, deliberately not validation_order or FIFO order.
        for block in 0..blocks {
            consumer.charge_work_v1(1)?;
            if !repeated.reached[block] {
                continue;
            }
            let start = mul(block, locals)?;
            let row = &repeated.entries[start..start + locals];
            let count = populated_actual(row, consumer)?;
            let replay_work = count
                .checked_add(inputs.function.blocks()[block].statements().len())
                .and_then(|amount| amount.checked_add(1))
                .ok_or_else(|| {
                    transfer_error(ProductionRankedProjectionErrorV1::Unsupported(
                        "capability replay work overflow",
                    ))
                })?;
            original_charge(consumer, &mut self.original_work, replay_work)?;
            consumer.charge_work_v1(locals)?;
            self.replay_scratch.copy_from_slice(row);
            self.run.reached_blocks[2] = add(self.run.reached_blocks[2], 1)?;
            let effects = transfer_closed(
                NominalCapabilityPassV1::Final,
                site,
                inputs,
                block,
                &mut self.replay_scratch,
                callables,
                &self.pipeline_owners,
                &self.pipeline_payloads,
                consumer,
                visit,
                &mut self.run,
            )?;
            self.layouts[block] = effects.layout;
            self.global_reads[block] = effects.global_read;
            self.transpose_workgroups[block] = effects.transpose_workgroup;
            self.read_views[block] = effects.read_view;
        }
        require(
            self.run.query_visits[2] == 1 && self.run.authenticated_visits[2] == 1,
            "nominal source-index replay lacks one actual authenticated call",
        )?;
        require(
            !self.run.array_destination_has_origin,
            "nominal scalar array acquired a capability origin",
        )?;
        bind_reads(
            inputs.function,
            &self.global_reads,
            &mut self.bound_reads,
            consumer,
            owned,
        )?;
        let after = consumer.ledger_v1();
        if after.denied_work
            || after.denied_storage
            || self.ledger != Some((after.slot, after.identity))
        {
            return Err(Error::Resource(Resource::Accounting));
        }
        self.phase = Phase::Complete;
        Ok(())
    }
    pub(in super::super) fn completed_for<'a>(
        &'a self,
        site: &NominalCallerSiteV1<'_>,
        inputs: &NominalCapabilityInputsV1<'_>,
        consumer: &dyn NominalCapabilityConsumerV1,
    ) -> Result<CompletedNominalFifoDriverV1<'a>> {
        let ledger = consumer.ledger_v1();
        if self.phase != Phase::Complete
            || !self.owner_scan_complete
            || !self.payload_scan_complete
            || ledger.denied_work
            || ledger.denied_storage
            || self.ledger != Some((ledger.slot, ledger.identity))
            || self.inputs != Some(input_key(site, inputs))
        {
            return Err(Error::Resource(Resource::Accounting));
        }
        let initial = self.initial.completed_for(inputs.function, consumer)?;
        let repeated = self.repeated.completed_for(inputs.function, consumer)?;
        Ok(CompletedNominalFifoDriverV1 {
            initial_entries: initial.entries,
            initial_reached: initial.reached,
            initial_visits: initial.visits,
            repeated_entries: repeated.entries,
            repeated_reached: repeated.reached,
            repeated_visits: repeated.visits,
            layouts: &self.layouts,
            global_reads: &self.global_reads,
            transpose_workgroups: &self.transpose_workgroups,
            read_views: &self.read_views,
            bound_reads: &self.bound_reads,
            original_work: self.original_work,
            run: self.run,
            owner_scan_blocks: self.owner_scan_blocks,
            alias_scan_locals: self.alias_scan_locals,
            payload_scan_blocks: self.payload_scan_blocks,
        })
    }
    fn prepare_profile(
        &mut self,
        site: &NominalCallerSiteV1<'_>,
        consumer: &mut dyn NominalCapabilityConsumerV1,
        owned: &mut usize,
    ) -> Result<()> {
        let function = site.function();
        let callables = site.owner().semantic_ssa().source_semantic().callables();
        let blocks = function.blocks().len();
        fill_copy(&mut self.edges, blocks, 0u32, consumer, owned)?;
        fill_copy(&mut self.indegree, blocks, 0u8, consumer, owned)?;
        reserve(&mut self.validation_order, blocks, consumer, owned)?;
        let mut nominal_calls = 0usize;
        let mut statements = 0usize;
        for (block_index, block) in function.blocks().iter().enumerate() {
            consumer.charge_work_v1(1)?;
            statements = add(statements, block.statements().len())?;
            require(
                statements <= MAX_STATEMENTS,
                "nominal FIFO exceeds its closed statement profile",
            )?;
            match block.terminator().kind() {
                SemanticTerminatorKindV1::TailCall(_) => {
                    return Err(Error::Unavailable(
                        "nominal FIFO profile does not support tail calls",
                    ));
                }
                SemanticTerminatorKindV1::Call(call) => {
                    consumer.charge_work_v1(2)?;
                    match callables.get(call.callee().index() as usize) {
                        Some(SemanticCallableDeclV1::Defined { .. }) => {
                            require(
                                block_index == site.block().index() as usize
                                    && std::ptr::eq(call, site.call()),
                                "nominal FIFO profile contains another Defined call",
                            )?;
                            nominal_calls = add(nominal_calls, 1)?;
                        }
                        Some(SemanticCallableDeclV1::CompilerIntrinsic { operation, .. }) => {
                            require(!matches!(operation,
                                SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { .. }
                                | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineEvent { .. }
                                | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite { .. }
                                | SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineRead { .. }),
                                "nominal FIFO profile contains a workgroup pipeline operation")?;
                        }
                        _ => {
                            return Err(Error::Unavailable(
                                "nominal FIFO profile contains an unclassified external call",
                            ));
                        }
                    }
                }
                _ => {}
            }
            block
                .terminator()
                .kind()
                .try_for_each_edge::<Error>(|edge| {
                    consumer.charge_work_v1(2)?;
                    let target = edge.target().index() as usize;
                    require(
                        target < blocks,
                        "nominal FIFO CFG edge is outside the source function",
                    )?;
                    let bit = 1u32 << target;
                    if self.edges[block_index] & bit == 0 {
                        self.edges[block_index] |= bit;
                        self.indegree[target] = self.indegree[target]
                            .checked_add(1)
                            .ok_or(Error::Resource(Resource::Arithmetic))?;
                    }
                    Ok(())
                })?;
        }
        require(
            nominal_calls == 1,
            "nominal FIFO profile lacks its sole Defined call",
        )?;
        for (block, &degree) in self.indegree.iter().enumerate() {
            consumer.charge_work_v1(1)?;
            if degree == 0 {
                self.validation_order.push(block);
            }
        }
        let mut cursor = 0usize;
        while cursor < self.validation_order.len() {
            consumer.charge_work_v1(1)?;
            let block = self.validation_order[cursor];
            cursor = add(cursor, 1)?;
            let mut edges = self.edges[block];
            while edges != 0 {
                consumer.charge_work_v1(2)?;
                let target = edges.trailing_zeros() as usize;
                edges &= edges - 1;
                self.indegree[target] = self.indegree[target]
                    .checked_sub(1)
                    .ok_or(Error::Resource(Resource::Accounting))?;
                if self.indegree[target] == 0 {
                    self.validation_order.push(target);
                }
            }
        }
        require(
            self.validation_order.len() == blocks,
            "nominal FIFO source CFG contains a cycle",
        )
    }
}
// Actual source-ordered no-pipeline branch of original owner discovery/alias pass.
// Profile refusal above occurs before this function; absent pipelines do not skip scans.
fn owner_scan(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    owners: &mut [Option<usize>],
    blocks_seen: &mut usize,
    locals_seen: &mut usize,
    consumer: &mut dyn NominalCapabilityConsumerV1,
) -> Result<()> {
    require(
        owners.len() == function.locals().len(),
        "pipeline owner storage does not match the semantic local table",
    )?;
    for block in function.blocks() {
        consumer.charge_work_v1(1)?;
        *blocks_seen = add(*blocks_seen, 1)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        consumer.charge_work_v1(1)?;
        if matches!(
            callables.get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineCreate { .. },
                ..
            })
        ) {
            return Err(Error::Unavailable(
                "closed nominal owner scan encountered a pipeline create",
            ));
        }
    }
    for owner in owners.iter() {
        consumer.charge_work_v1(1)?;
        *locals_seen = add(*locals_seen, 1)?;
        require(
            owner.is_none(),
            "closed nominal owner scan retained a pipeline owner",
        )?;
    }
    Ok(())
}
fn payload_scan(
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    initial: &CompletedFifoPassV1<'_>,
    owners: &[Option<usize>],
    payloads: &mut HashMap<usize, ProjectedMfmaOperandV1>,
    blocks_seen: &mut usize,
    consumer: &mut dyn NominalCapabilityConsumerV1,
) -> Result<()> {
    require(
        initial.reached.len() == function.blocks().len()
            && owners.len() == function.locals().len()
            && payloads.is_empty(),
        "closed nominal payload scan differs from its initial source tables",
    )?;
    for block in function.blocks() {
        consumer.charge_work_v1(1)?;
        *blocks_seen = add(*blocks_seen, 1)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        consumer.charge_work_v1(1)?;
        if matches!(
            callables.get(call.callee().index() as usize),
            Some(SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::WorkgroupPipelineWrite { .. },
                ..
            })
        ) {
            return Err(Error::Unavailable(
                "closed nominal payload scan encountered a pipeline write",
            ));
        }
    }
    Ok(())
}
fn prepay_transfer(
    function: &SemanticFunctionDeclV1,
    block: usize,
    locals: usize,
    consumer: &mut dyn NominalCapabilityConsumerV1,
) -> Result<()> {
    for statement in function.blocks()[block].statements() {
        consumer.charge_work_v1(64)?;
        if let SemanticStatementKindV1::Assign(assignment) = statement.kind() {
            assignment
                .value()
                .kind()
                .try_visit_operands(|_| consumer.charge_work_v1(8))?;
        }
    }
    let arguments = match function.blocks()[block].terminator().kind() {
        SemanticTerminatorKindV1::Call(call) => call.arguments().len(),
        SemanticTerminatorKindV1::TailCall(call) => call.arguments().len(),
        _ => 1,
    };
    consumer.charge_work_v1(add(add(locals, mul(arguments, 8)?)?, 256)?)
}
fn transfer_closed(
    pass: NominalCapabilityPassV1,
    site: &NominalCallerSiteV1<'_>,
    inputs: &NominalCapabilityInputsV1<'_>,
    block: usize,
    scratch: &mut [Slot],
    callables: &[SemanticCallableDeclV1],
    owners: &[Option<usize>],
    payloads: &HashMap<usize, ProjectedMfmaOperandV1>,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    visit: &mut NominalCapabilityVisitV1<'_>,
    run: &mut NominalCapabilityRunV1,
) -> Result<ProjectedCapabilityTerminatorEffectsV1> {
    prepay_transfer(inputs.function, block, scratch.len(), consumer)?;
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
        // Exact existing true checked-query/C1/postflight path. Never generic DATA as authority.
        transfer_nominal(pass, site, &mut state, consumer, visit, run)?
    } else {
        transfer_capability_terminator_v1(
            callables,
            inputs.function,
            block,
            &mut state,
            inputs.local_allocations,
            inputs.constants,
            owners,
            payloads,
            pass == NominalCapabilityPassV1::Final,
        )
        .map_err(transfer_error)?
    };
    state.check_bounds_v1()?;
    require(
        matches!(
            effects.layout.as_ref(),
            None | Some(ProductionRankedOperationV1::TensorLayout { .. })
        ),
        "nominal FIFO shared transfer produced an unbounded layout variant",
    )?;
    Ok(effects)
}
fn bind_reads(
    function: &SemanticFunctionDeclV1,
    reads: &[Option<AllocationContractV1>],
    bound: &mut Vec<Option<ProjectedCapabilityReadEffectV1>>,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    owned: &mut usize,
) -> Result<()> {
    require(
        reads.len() == function.blocks().len(),
        "capability read effects do not correspond one-to-one with semantic MIR blocks",
    )?;
    reserve(bound, reads.len(), consumer, owned)?;
    for (block, allocation) in function.blocks().iter().zip(reads.iter().copied()) {
        consumer.charge_work_v1(1)?;
        bound.push(
            allocation.map(|allocation| ProjectedCapabilityReadEffectV1 {
                allocation,
                source: block.terminator().source(),
            }),
        );
    }
    Ok(())
}
fn retain(
    consumer: &mut dyn NominalCapabilityConsumerV1,
    owned: &mut usize,
    bytes: usize,
) -> Result<()> {
    let next = add(*owned, bytes)?;
    consumer.reserve_storage_v1(bytes)?;
    *owned = next;
    Ok(())
}
fn reserve<T>(
    values: &mut Vec<T>,
    additional: usize,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    owned: &mut usize,
) -> Result<()> {
    let requested = add(values.len(), additional)?;
    if requested <= values.capacity() {
        return Ok(());
    }
    consumer.charge_work_v1(values.len())?;
    retain(consumer, owned, mul(requested, size_of::<T>())?)?;
    values
        .try_reserve_exact(additional)
        .map_err(|_| Error::Resource(Resource::Allocation))?;
    if size_of::<T>() != 0 && values.capacity() != requested {
        return Err(Error::Resource(Resource::Allocation));
    }
    Ok(())
}
fn fill_none<T>(
    values: &mut Vec<Option<T>>,
    count: usize,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    owned: &mut usize,
) -> Result<()> {
    consumer.charge_work_v1(count)?;
    reserve(values, count, consumer, owned)?;
    values.resize_with(count, || None);
    Ok(())
}
fn fill_copy<T: Copy>(
    values: &mut Vec<T>,
    count: usize,
    value: T,
    consumer: &mut dyn NominalCapabilityConsumerV1,
    owned: &mut usize,
) -> Result<()> {
    consumer.charge_work_v1(count)?;
    reserve(values, count, consumer, owned)?;
    values.resize(count, value);
    Ok(())
}
fn original_charge(
    consumer: &mut dyn NominalCapabilityConsumerV1,
    original_work: &mut usize,
    amount: usize,
) -> Result<()> {
    consumer.charge_work_v1(amount)?;
    charge_capability_dataflow_work_v1(original_work, amount).map_err(transfer_error)
}
fn populated_actual(
    slots: &[Slot],
    consumer: &mut dyn NominalCapabilityConsumerV1,
) -> Result<usize> {
    consumer.charge_work_v1(slots.len())?;
    let mut count = 0usize;
    for slot in slots {
        if slot.is_some() {
            count = add(count, 1)?;
        }
    }
    Ok(count)
}
// Original C2 shared-transfer policy stays separate from all new lexical rows.
// This is logical source-owned accounting, not native stack/allocator/RSS measurement.
const DRIVER_FRAME_ROWS: usize = 60;
fn reserve_frame<T>() -> usize {
    size_of::<(
        &mut Vec<T>,
        usize,
        &mut dyn NominalCapabilityConsumerV1,
        &mut usize,
        usize,
        usize,
        Result<()>,
        std::collections::TryReserveError,
        std::result::Result<(), std::collections::TryReserveError>,
        Error,
        Resource,
    )>()
}
fn none_frame<T>() -> usize {
    size_of::<(
        &mut Vec<Option<T>>,
        usize,
        &mut dyn NominalCapabilityConsumerV1,
        &mut usize,
        Option<T>,
        Result<()>,
    )>()
}
fn copy_frame<T>() -> usize {
    size_of::<(
        &mut Vec<T>,
        usize,
        T,
        &mut dyn NominalCapabilityConsumerV1,
        &mut usize,
        Result<()>,
    )>()
}
fn driver_frame_rows() -> Result<[usize; DRIVER_FRAME_ROWS]> {
    Ok([
        // inherited-unchanged-C2-transfer-envelope
        4096,
        // driver-owner
        size_of::<RetainedNominalCapabilityDriverV1>(),
        // constructor-scalars
        size_of::<(
            Phase,
            Option<Identity>,
            Option<InputsKey>,
            usize,
            usize,
            usize,
            usize,
            NominalCapabilityRunV1,
            bool,
            bool,
        )>(),
        // constructor-owned-headers
        size_of::<(
            Vec<u32>,
            Vec<u8>,
            Vec<usize>,
            Vec<Option<usize>>,
            HashMap<usize, ProjectedMfmaOperandV1>,
            HashMap<usize, ProjectedMfmaOperandV1>,
            RetainedCapabilityFifoPassV1,
            RetainedCapabilityFifoPassV1,
            Vec<Slot>,
            Vec<Option<ProductionRankedOperationV1>>,
            Vec<Option<AllocationContractV1>>,
            Vec<Option<ProjectedTransposeWorkgroupEffectV1>>,
            Vec<Option<ProjectedReadViewAccessV1>>,
            Vec<Option<ProjectedCapabilityReadEffectV1>>,
        )>(),
        // entry-arguments
        size_of::<(
            &mut RetainedNominalCapabilityDriverV1,
            &NominalCallerSiteV1<'static>,
            &NominalCapabilityInputsV1<'static>,
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            &mut NominalCapabilityVisitV1<'static>,
            Result<()>,
        )>(),
        // entry-ledger-and-shape
        size_of::<(
            bool,
            NominalCapabilityLedgerV1,
            NominalCapabilityLedgerV1,
            Identity,
            Option<Identity>,
            InputsKey,
            Option<InputsKey>,
            usize,
            usize,
            std::ops::RangeInclusive<usize>,
            std::ops::RangeInclusive<usize>,
            &[SemanticCallableDeclV1],
        )>(),
        // key-helper
        size_of::<(
            &NominalCallerSiteV1<'static>,
            &NominalCapabilityInputsV1<'static>,
            InputsKey,
            &SemanticFunctionDeclV1,
            &SemanticEnumPayloadDominanceV1,
            &[Option<AllocationContractV1>],
            &[Option<u64>],
            &SemanticDirectCallV1,
            usize,
            usize,
        )>(),
        // profile-call
        size_of::<(
            &mut RetainedNominalCapabilityDriverV1,
            &NominalCallerSiteV1<'static>,
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            Result<()>,
        )>(),
        // owner-scan-call-aliases
        size_of::<(
            &SemanticFunctionDeclV1,
            &[SemanticCallableDeclV1],
            &mut Vec<Option<usize>>,
            &mut [Option<usize>],
            &mut usize,
            &mut usize,
            &mut dyn NominalCapabilityConsumerV1,
            Result<()>,
        )>(),
        // first-pass-local-captures
        size_of::<(
            &Vec<Option<usize>>,
            &[Option<usize>],
            &HashMap<usize, ProjectedMfmaOperandV1>,
            &mut NominalCapabilityRunV1,
            &mut RetainedCapabilityFifoPassV1,
            &mut usize,
            Result<()>,
        )>(),
        // first-pass-closure-yields
        size_of::<(
            usize,
            &mut [Slot],
            &mut dyn NominalCapabilityConsumerV1,
            ProjectedCapabilityTerminatorEffectsV1,
            Result<ProjectedCapabilityTerminatorEffectsV1>,
            Result<()>,
        )>(),
        // initial-completed-loan
        size_of::<(
            CompletedFifoPassV1<'static>,
            Result<CompletedFifoPassV1<'static>>,
            &RetainedCapabilityFifoPassV1,
            &SemanticFunctionDeclV1,
            &dyn NominalCapabilityConsumerV1,
        )>(),
        // payload-scan-call-aliases
        size_of::<(
            &SemanticFunctionDeclV1,
            &[SemanticCallableDeclV1],
            &CompletedFifoPassV1<'static>,
            &Vec<Option<usize>>,
            &[Option<usize>],
            &mut HashMap<usize, ProjectedMfmaOperandV1>,
            &mut usize,
            &mut dyn NominalCapabilityConsumerV1,
            Result<()>,
        )>(),
        // repeated-pass-local-captures
        size_of::<(
            &Vec<Option<usize>>,
            &[Option<usize>],
            &HashMap<usize, ProjectedMfmaOperandV1>,
            &mut NominalCapabilityRunV1,
            &mut RetainedCapabilityFifoPassV1,
            &mut usize,
            Result<()>,
        )>(),
        // repeated-pass-closure-yields
        size_of::<(
            usize,
            &mut [Slot],
            &mut dyn NominalCapabilityConsumerV1,
            ProjectedCapabilityTerminatorEffectsV1,
            Result<ProjectedCapabilityTerminatorEffectsV1>,
            Result<()>,
        )>(),
        // repeated-completed-loan
        size_of::<(
            CompletedFifoPassV1<'static>,
            Result<CompletedFifoPassV1<'static>>,
            &RetainedCapabilityFifoPassV1,
            &SemanticFunctionDeclV1,
            &dyn NominalCapabilityConsumerV1,
        )>(),
        // source-index-replay-loop
        size_of::<(
            std::ops::Range<usize>,
            usize,
            usize,
            &[Slot],
            usize,
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            ProjectedCapabilityTerminatorEffectsV1,
            Result<ProjectedCapabilityTerminatorEffectsV1>,
            &mut Vec<Slot>,
            &mut [Slot],
        )>(),
        // read-binding-call-aliases
        size_of::<(
            &SemanticFunctionDeclV1,
            &Vec<Option<AllocationContractV1>>,
            &[Option<AllocationContractV1>],
            &mut Vec<Option<ProjectedCapabilityReadEffectV1>>,
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            Result<()>,
        )>(),
        // complete-getter
        size_of::<(
            &RetainedNominalCapabilityDriverV1,
            &NominalCallerSiteV1<'static>,
            &NominalCapabilityInputsV1<'static>,
            &dyn NominalCapabilityConsumerV1,
            NominalCapabilityLedgerV1,
            Option<Identity>,
            InputsKey,
            Option<InputsKey>,
            CompletedNominalFifoDriverV1<'static>,
            Result<CompletedNominalFifoDriverV1<'static>>,
            bool,
            CompletedFifoPassV1<'static>,
            Result<CompletedFifoPassV1<'static>>,
            CompletedFifoPassV1<'static>,
            Result<CompletedFifoPassV1<'static>>,
        )>(),
        // getter-field-aliases
        size_of::<(
            &[Slot],
            &[bool],
            &[usize],
            &[Slot],
            &[bool],
            &[usize],
            &Vec<Option<ProductionRankedOperationV1>>,
            &[Option<ProductionRankedOperationV1>],
            &Vec<Option<AllocationContractV1>>,
            &[Option<AllocationContractV1>],
            &Vec<Option<ProjectedTransposeWorkgroupEffectV1>>,
            &[Option<ProjectedTransposeWorkgroupEffectV1>],
            &Vec<Option<ProjectedReadViewAccessV1>>,
            &[Option<ProjectedReadViewAccessV1>],
            &Vec<Option<ProjectedCapabilityReadEffectV1>>,
            &[Option<ProjectedCapabilityReadEffectV1>],
        )>(),
        // profile-header
        size_of::<(
            &SemanticFunctionDeclV1,
            &[SemanticCallableDeclV1],
            usize,
            usize,
            usize,
            std::iter::Enumerate<
                std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            >,
            (
                usize,
                &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
            ),
            usize,
            &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
        )>(),
        // profile-call-selection
        size_of::<(
            &SemanticTerminatorKindV1,
            &SemanticDirectCallV1,
            Option<&SemanticCallableDeclV1>,
            &SemanticCompilerIntrinsicOperationV1,
            bool,
            usize,
            Result<usize>,
            Result<()>,
        )>(),
        // profile-edge-closure
        size_of::<(
            (
                &mut RetainedNominalCapabilityDriverV1,
                &mut Vec<u32>,
                &mut Vec<u8>,
                &mut dyn NominalCapabilityConsumerV1,
                &usize,
                &usize,
            ),
            fe2o3_mir_model::semantic_mir_v1::SemanticControlFlowEdgeV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticControlFlowEdgeV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1,
            fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1,
            u32,
            usize,
            u32,
            Option<u8>,
            Result<()>,
        )>(),
        // acyclic-validation-loop
        size_of::<(
            std::iter::Enumerate<std::slice::Iter<'static, u8>>,
            (usize, &u8),
            usize,
            u8,
            usize,
            usize,
            u32,
            usize,
            Option<u8>,
            Result<usize>,
            bool,
        )>(),
        // owner-scan-entry
        size_of::<(
            &SemanticFunctionDeclV1,
            &[SemanticCallableDeclV1],
            &mut [Option<usize>],
            &mut usize,
            &mut usize,
            &mut dyn NominalCapabilityConsumerV1,
            Result<()>,
        )>(),
        // owner-scan-body
        size_of::<(
            std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
            &SemanticTerminatorKindV1,
            &SemanticDirectCallV1,
            Option<&SemanticCallableDeclV1>,
            bool,
            std::slice::Iter<'static, Option<usize>>,
            &Option<usize>,
            Result<usize>,
        )>(),
        // payload-scan-entry
        size_of::<(
            &SemanticFunctionDeclV1,
            &[SemanticCallableDeclV1],
            &CompletedFifoPassV1<'static>,
            &[Option<usize>],
            &mut HashMap<usize, ProjectedMfmaOperandV1>,
            &mut usize,
            &mut dyn NominalCapabilityConsumerV1,
            Result<()>,
        )>(),
        // payload-scan-body
        size_of::<(
            std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
            &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
            &SemanticTerminatorKindV1,
            &SemanticDirectCallV1,
            Option<&SemanticCallableDeclV1>,
            bool,
            Result<usize>,
        )>(),
        // prepay-transfer-entry
        size_of::<(
            &SemanticFunctionDeclV1,
            usize,
            usize,
            &mut dyn NominalCapabilityConsumerV1,
            Result<()>,
        )>(),
        // prepay-transfer-body
        size_of::<(
            std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1>,
            &fe2o3_mir_model::semantic_mir_v1::SemanticStatementV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
            &mut dyn NominalCapabilityConsumerV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticOperandV1,
            &SemanticDirectCallV1,
            &fe2o3_mir_model::semantic_mir_v1::SemanticDirectTailCallV1,
            usize,
            Result<()>,
        )>(),
        // closed-transfer-entry
        size_of::<(
            NominalCapabilityPassV1,
            &NominalCallerSiteV1<'static>,
            &NominalCapabilityInputsV1<'static>,
            usize,
            &mut [Slot],
            &[SemanticCallableDeclV1],
            &[Option<usize>],
            &HashMap<usize, ProjectedMfmaOperandV1>,
            &mut dyn NominalCapabilityConsumerV1,
            &mut NominalCapabilityVisitV1<'static>,
            &mut NominalCapabilityRunV1,
            Result<ProjectedCapabilityTerminatorEffectsV1>,
        )>(),
        // closed-transfer-body
        size_of::<(
            DenseCapabilityStateV1<'static>,
            &mut DenseCapabilityStateV1<'static>,
            ProjectedCapabilityTerminatorEffectsV1,
            Result<ProjectedCapabilityTerminatorEffectsV1>,
            std::result::Result<
                ProjectedCapabilityTerminatorEffectsV1,
                ProductionRankedProjectionErrorV1,
            >,
            std::result::Result<(), ProductionRankedProjectionErrorV1>,
            Option<&ProductionRankedOperationV1>,
            bool,
        )>(),
        // unchanged-nominal-transfer-entry
        size_of::<(
            NominalCapabilityPassV1,
            &NominalCallerSiteV1<'static>,
            &mut DenseCapabilityStateV1<'static>,
            &mut dyn NominalCapabilityConsumerV1,
            &mut NominalCapabilityVisitV1<'static>,
            &mut NominalCapabilityRunV1,
            usize,
            bool,
            bool,
            Result<ProjectedCapabilityTerminatorEffectsV1>,
        )>(),
        // unchanged-query-closure
        size_of::<(
            (
                &mut bool,
                &NominalCallerSiteV1<'static>,
                &mut DenseCapabilityStateV1<'static>,
                &NominalCapabilityPassV1,
                &mut NominalCapabilityVisitV1<'static>,
                &mut bool,
            ),
            &super::super::bf16_nominal_call_projection_v1::CheckedNominalCallProjectionV1<'static>,
            &mut Budget<'static>,
            Result<()>,
            NominalTransferOutcomeV1<'static>,
            Result<NominalTransferOutcomeV1<'static>>,
            &'static str,
            AuthenticatedNominalCallerV1<'static>,
            &AuthenticatedNominalCallerV1<'static>,
        )>(),
        // unchanged-query-call
        size_of::<(
            usize,
            &SemanticDirectCallV1,
            SemanticSourceProvenanceV1,
            &mut NominalCallVisitorV1<'static>,
            Result<()>,
            usize,
            Result<usize>,
            super::super::bf16_nominal_capabilities_v1::NominalArrayDestinationV1,
        )>(),
        // binding-entry
        size_of::<(
            &SemanticFunctionDeclV1,
            &[Option<AllocationContractV1>],
            &mut Vec<Option<ProjectedCapabilityReadEffectV1>>,
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            Result<()>,
        )>(),
        // binding-loop-and-map
        size_of::<(
            std::iter::Zip<
                std::slice::Iter<'static, fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1>,
                std::iter::Copied<std::slice::Iter<'static, Option<AllocationContractV1>>>,
            >,
            (
                &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
                Option<AllocationContractV1>,
            ),
            &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
            Option<AllocationContractV1>,
            AllocationContractV1,
            ProjectedCapabilityReadEffectV1,
            Option<ProjectedCapabilityReadEffectV1>,
            SemanticSourceProvenanceV1,
        )>(),
        // retain-helper
        size_of::<(
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            usize,
            usize,
            Result<()>,
        )>(),
        // reserve-cfg-edges
        reserve_frame::<u32>(),
        // reserve-cfg-indegree
        reserve_frame::<u8>(),
        // reserve-validation-order
        reserve_frame::<usize>(),
        // reserve-pipeline-owners
        reserve_frame::<Option<usize>>(),
        // reserve-replay-scratch
        reserve_frame::<Slot>(),
        // reserve-layouts
        reserve_frame::<Option<ProductionRankedOperationV1>>(),
        // reserve-global-reads
        reserve_frame::<Option<AllocationContractV1>>(),
        // reserve-transpose
        reserve_frame::<Option<ProjectedTransposeWorkgroupEffectV1>>(),
        // reserve-read-views
        reserve_frame::<Option<ProjectedReadViewAccessV1>>(),
        // reserve-bound-reads
        reserve_frame::<Option<ProjectedCapabilityReadEffectV1>>(),
        // fill-owners
        none_frame::<usize>(),
        // fill-replay
        none_frame::<ProjectedCapabilityValueV1>(),
        // fill-layouts
        none_frame::<ProductionRankedOperationV1>(),
        // fill-global-reads
        none_frame::<AllocationContractV1>(),
        // fill-transpose
        none_frame::<ProjectedTransposeWorkgroupEffectV1>(),
        // fill-read-views
        none_frame::<ProjectedReadViewAccessV1>(),
        // fill-edges
        copy_frame::<u32>(),
        // fill-indegree
        copy_frame::<u8>(),
        // original-charge-helper
        size_of::<(
            &mut dyn NominalCapabilityConsumerV1,
            &mut usize,
            usize,
            std::result::Result<(), ProductionRankedProjectionErrorV1>,
            ProductionRankedProjectionErrorV1,
            Result<()>,
        )>(),
        // populated-helper
        size_of::<(
            &[Slot],
            &mut dyn NominalCapabilityConsumerV1,
            usize,
            std::slice::Iter<'static, Slot>,
            &Slot,
            Result<usize>,
            bool,
            Option<usize>,
        )>(),
        // checked-arithmetic-and-results
        size_of::<(
            usize,
            usize,
            Option<usize>,
            Result<usize>,
            Result<()>,
            Error,
            Resource,
            ProductionRankedProjectionErrorV1,
        )>(),
        // array-result-fold
        size_of::<(
            [usize; DRIVER_FRAME_ROWS],
            Result<[usize; DRIVER_FRAME_ROWS]>,
            std::array::IntoIter<usize, DRIVER_FRAME_ROWS>,
            usize,
            usize,
            Result<usize>,
            Option<usize>,
        )>(),
    ])
}
fn driver_frame() -> Result<usize> {
    driver_frame_rows()?
        .into_iter()
        .try_fold(0usize, |sum, row| add(sum, row))
}

#[cfg(test)]
#[path = "bf16_nominal_retained_capability_driver_v1_tests.rs"]
mod tests;
