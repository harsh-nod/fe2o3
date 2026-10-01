//! Typed actual-to-actual forwarding. Authenticated allocation recipes are
//! borrowed through both interpreters; this wrapper does not create source
//! frames or reinterpret compiler-private spill origins as source allocations.
use super::byte_function_v30::{ByteAllocationResolverV30, ByteAllocationSiteV30};
use super::store_consensus_plan_v46::{Action, MemoryPlan};
use super::*;
use fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV18 as Pair;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger,
    CanonicalKirOperationCoordinateV1 as Operation, VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use std::cell::Cell;

struct Allocations<'a, 'owner, R> {
    input: &'a Inventory<'owner>,
    output: &'a Inventory<'owner>,
    pair: &'a Pair<'owner>,
    source: &'a R,
    sites: Vec<Option<ByteAllocationSiteV30>>,
    required: usize,
    budget_slot: usize,
    ledger: Ledger,
    failure: Cell<Option<Resource>>,
}

fn mismatch() -> Error {
    Error::Statement("typed forwarding allocation owner or occurrence differs")
}

fn vector<T>(count: usize, out: &mut Writer<'_, '_>) -> Result<Vec<T>> {
    out.budget.reserve_storage(
        count
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    let mut rows = Vec::new();
    rows.try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    out.budget.reserve_storage(
        rows.capacity()
            .checked_sub(count)
            .ok_or(Resource::Accounting)?
            .checked_mul(size_of::<T>())
            .ok_or(Resource::Arithmetic)?,
    )?;
    Ok(rows)
}

impl<'a, 'owner, R: ByteAllocationResolverV30> Allocations<'a, 'owner, R> {
    fn derive(
        input: &'a Inventory<'owner>,
        output: &'a Inventory<'owner>,
        pair: &'a Pair<'owner>,
        source: &'a R,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        source.check_owner(input.owner(), out)?;
        out.budget.charge_work(4)?;
        if !input.belongs_to(pair.input())
            || !output.belongs_to(pair.output())
            || input.operations().len() != output.operations().len()
            || pair.origins().len() != input.operations().len()
        {
            return Err(mismatch());
        }
        type Key = (usize, Operation);
        out.budget.reserve_storage(
            size_of::<Self>()
                + size_of::<Result<Self>>()
                + size_of::<Vec<Key>>()
                + size_of::<Result<Vec<Key>>>(),
        )?;
        let mut sites = vector(input.operations().len(), out)?;
        let mut unique = vector::<Key>(input.operations().len(), out)?;
        for (ordinal, row) in input.operations().iter().enumerate() {
            out.budget.charge_work(4)?;
            let actual = &output.operations()[ordinal];
            let origin = &pair.origins()[ordinal];
            if row.coordinate != actual.coordinate
                || origin.input != row.coordinate
                || origin.output != actual.coordinate
            {
                return Err(mismatch());
            }
            let site = if matches!(row.operation.kind, OperationKind::Alloca { .. }) {
                if row.operation != actual.operation || origin.store.is_some() {
                    return Err(mismatch());
                }
                let site = source.site(row.coordinate, out)?;
                unique.push((site.physical_root_owner, site.original));
                Some(site)
            } else {
                None
            };
            sites.push(site);
        }
        // The same static recipe cannot silently name a selected and an
        // unrelated allocation. Reexecution uses that recipe's counter instead.
        out.budget.charge_work(
            unique
                .len()
                .checked_mul(unique.len().checked_ilog2().unwrap_or(0) as usize + 2)
                .ok_or(Resource::Arithmetic)?,
        )?;
        unique.sort_unstable();
        for rows in unique.windows(2) {
            out.budget.charge_work(1)?;
            if rows[0] == rows[1] {
                return Err(mismatch());
            }
        }
        let temporary = unique
            .capacity()
            .checked_mul(size_of::<Key>())
            .and_then(|n| n.checked_add(size_of::<Vec<Key>>() + size_of::<Result<Vec<Key>>>()))
            .ok_or(Resource::Arithmetic)?;
        drop(unique);
        out.budget.release_storage(temporary)?;
        source.check_owner(input.owner(), out)?;
        Ok(Self {
            input,
            output,
            pair,
            source,
            sites,
            required: out.budget.storage(),
            budget_slot: std::ptr::from_ref(out.budget) as usize,
            ledger: out.budget.work_ledger_identity_v1(),
            failure: Cell::new(None),
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        if let Some(error) = self.failure.get() {
            return Err(error.into());
        }
        if self.budget_slot != std::ptr::from_ref(out.budget) as usize
            || self.ledger != out.budget.work_ledger_identity_v1()
            || out.budget.storage() < self.required
        {
            self.failure.set(Some(Resource::Accounting));
            return Err(Resource::Accounting.into());
        }
        self.source.check_owner(self.input.owner(), out)
    }
}

impl<R: ByteAllocationResolverV30> ByteAllocationResolverV30 for Allocations<'_, '_, R> {
    fn check_owner(&self, owner: &Owner, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        if let Err(error) = out.budget.charge_work(4) {
            self.failure.set(Some(error));
            return Err(error.into());
        }
        if (!std::ptr::eq(owner, self.input.owner()) && !std::ptr::eq(owner, self.output.owner()))
            || !self.input.belongs_to(self.pair.input())
            || !self.output.belongs_to(self.pair.output())
        {
            return Err(mismatch());
        }
        Ok(())
    }

    fn site(
        &self,
        operation: Operation,
        out: &mut Writer<'_, '_>,
    ) -> Result<ByteAllocationSiteV30> {
        self.check(out)?;
        if let Err(error) = out
            .budget
            .charge_work(self.input.operations().len().checked_ilog2().unwrap_or(0) as usize + 2)
        {
            self.failure.set(Some(error));
            return Err(error.into());
        }
        let ordinal = self
            .input
            .operations()
            .binary_search_by_key(&operation, |row| row.coordinate)
            .map_err(|_| mismatch())?;
        self.sites
            .get(ordinal)
            .copied()
            .flatten()
            .ok_or_else(mismatch)
    }
}

fn emit_site(site: ByteAllocationSiteV30, out: &mut Writer<'_, '_>) -> Result<()> {
    emit!(
        out,
        "MemoryPrivateSiteV30 {{ owner: {}, invocation: 0, site: MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }} }}",
        site.physical_root_owner,
        site.original.block.function.0,
        site.original.block.block,
        site.original.operation
    );
    Ok(())
}

fn emit_fact(
    input: &Inventory<'_>,
    plan: &MemoryPlan,
    store: usize,
    state: &str,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    if store == NONE {
        emit!(out, "true");
        return Ok(());
    }
    out.budget.charge_work(3)?;
    let Action::Store { slot, value } = plan.actions[store] else {
        return Err(mismatch());
    };
    let OperationKind::Alloca { alignment, .. } = input.operations()[plan.slots[slot].allocation]
        .operation
        .kind
    else {
        return Err(mismatch());
    };
    emit!(
        out,
        "forwarding_store_fact_v46({state}, {}, {value}, {}, {alignment}, little_endian)",
        plan.slots[slot].pointer,
        plan.slots[slot].bits / 8
    );
    Ok(())
}

fn emit_classifiers<R: ByteAllocationResolverV30>(
    input: &Inventory<'_>,
    plan: &MemoryPlan,
    allocations: &Allocations<'_, '_, R>,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    allocations.check(out)?;
    emit!(
        out,
        "open spec fn forwarding_slot_count_v46() -> int {{ {} }}\nopen spec fn forwarding_definition_count_v46() -> int {{ {} }}\n",
        plan.slots.len(),
        input.definitions().len()
    );
    emit!(
        out,
        "open spec fn forwarding_slot_definition_v46(slot: int) -> int {{\n"
    );
    for (slot, row) in plan.slots.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(out, " if slot == {slot} {{ {} }} else", row.pointer);
    }
    emit!(
        out,
        " {{ -1 }}\n}}\nopen spec fn forwarding_selected_definition_v46(definition: int) -> bool {{ false"
    );
    for row in &plan.slots {
        out.budget.charge_work(1)?;
        emit!(out, " || definition == {}", row.pointer);
    }
    emit!(
        out,
        " }}\nopen spec fn forwarding_slot_site_v46(slot: int) -> MemoryPrivateSiteV30 {{\n"
    );
    for (slot, row) in plan.slots.iter().enumerate() {
        out.budget.charge_work(1)?;
        let site = allocations.sites[row.allocation].ok_or_else(mismatch)?;
        emit!(out, " if slot == {slot} {{ ");
        emit_site(site, out)?;
        emit!(out, " }} else");
    }
    emit!(
        out,
        " {{ MemoryPrivateSiteV30 {{ owner: -1, invocation: -1, site: MemorySourceOperationV30 {{ function: -1, block: -1, operation: -1 }} }} }}\n}}\n"
    );
    emit!(
        out,
        "open spec fn forwarding_selected_allocation_v46(allocation: MemoryAllocationV30) -> bool {{ match allocation {{ MemoryAllocationV30::Private {{ owner, invocation, site, generation }} => 0 <= generation && (exists|slot: int| 0 <= slot < forwarding_slot_count_v46() && forwarding_slot_site_v46(slot) == MemoryPrivateSiteV30 {{ owner, invocation, site }}), MemoryAllocationV30::External {{ .. }} => false }} }}\n"
    );
    emit!(
        out,
        "open spec fn forwarding_selected_object_shape_v46(allocation: MemoryAllocationV30, object: MemoryBytesV30) -> bool {{\n match allocation {{ MemoryAllocationV30::Private {{ owner, invocation, site, generation }} => 0 <= generation && (false"
    );
    for (slot, row) in plan.slots.iter().enumerate() {
        out.budget.charge_work(2)?;
        let OperationKind::Alloca { alignment, .. } =
            input.operations()[row.allocation].operation.kind
        else {
            return Err(mismatch());
        };
        emit!(
            out,
            " || (forwarding_slot_site_v46({slot}) == MemoryPrivateSiteV30 {{ owner, invocation, site }} && object.bytes.len() == {} && object.base_alignment == {alignment})",
            row.bits / 8
        );
    }
    emit!(
        out,
        ") && (forall|i: int| 0 <= i < object.bytes.len() ==> matches!(object.bytes[i], MemoryByteV37::Octet(_))) && object.relocations.dom().is_empty(), _ => false }}\n}}\n"
    );
    emit!(
        out,
        "open spec fn forwarding_removed_read_v46(observation: MemoryOperationObservationV30) -> bool {{ false"
    );
    for (ordinal, &replacement) in plan.replacements.iter().enumerate() {
        out.budget.charge_work(1)?;
        if replacement == NONE {
            continue;
        }
        let Action::Load { slot } = plan.actions[ordinal] else {
            return Err(mismatch());
        };
        let row = &input.operations()[ordinal];
        let (OperationKind::Load { access, .. }
        | OperationKind::Storage(fe2o3_kernel_ir::StorageOperationV1::ReadValue {
            access, ..
        })) = row.operation.kind
        else {
            return Err(mismatch());
        };
        let coordinate = row.coordinate;
        emit!(
            out,
            " || (observation.operation == MemorySourceOperationV30 {{ function: {}, block: {}, operation: {} }} && observation.before.values.len() == forwarding_definition_count_v46() && observation.after.values.len() == forwarding_definition_count_v46() && match observation.effect {{ MemoryOperationEffectV30::Read {{ address, width, alignment, value }} => address == observation.before.values[{}] && width == {} && alignment == {} && value == observation.after.values[{}] && match address {{ MemoryValueV30::Pointer(pointer) => forwarding_selected_allocation_v46(pointer.allocation), _ => false }}, _ => false }})",
            coordinate.block.function.0,
            coordinate.block.block,
            coordinate.operation,
            plan.slots[slot].pointer,
            plan.slots[slot].bits / 8,
            access.alignment,
            row.results.start
        );
    }
    emit!(out, " }}\n");
    for (label, rows) in [
        ("entry", &plan.entry),
        ("exit", &plan.exit),
        ("before", &plan.before),
    ] {
        emit!(
            out,
            "open spec fn forwarding_{label}_fact_v46(coordinate: int, state: MemoryStateV30, little_endian: bool) -> bool {{ !state.valid || (\n"
        );
        for (coordinate, &store) in rows.iter().enumerate() {
            out.budget.charge_work(1)?;
            emit!(out, " if coordinate == {coordinate} {{ ");
            emit_fact(input, plan, store, "state", out)?;
            emit!(out, " }} else");
        }
        emit!(out, " {{ false }}\n) }}\n");
    }
    emit!(
        out,
        "{}\n",
        include_str!("mixed_optimizer_store_consensus_typed_v46.vrs")
    );
    allocations.check(out)
}

fn emit_operation_law(
    input: &Inventory<'_>,
    plan: &MemoryPlan,
    ordinal: usize,
    input_namespace: usize,
    output_namespace: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(4)?;
    let row = &input.operations()[ordinal];
    let block = block_index(input, row.coordinate.block)?;
    let next = ordinal.checked_add(1).ok_or(Resource::Arithmetic)?;
    let (next_label, next_coordinate) = if next < input.blocks()[block].operations.end {
        ("before", next)
    } else {
        ("exit", block)
    };
    emit!(
        out,
        "proof fn forwarding_operation_{ordinal}_v46(n: MemoryStateV30, o: MemoryStateV30, little_endian: bool)\n requires forwarding_states_related_v46(n, o), n.valid ==> n.pc == {block}, forwarding_before_fact_v46({ordinal}, n, little_endian),\n ensures forwarding_states_related_v46(byte_operation_{input_namespace}_{ordinal}_v30(n, little_endian).state, byte_operation_{output_namespace}_{ordinal}_v30(o, little_endian).state),\n forwarding_{next_label}_fact_v46({next_coordinate}, byte_operation_{input_namespace}_{ordinal}_v30(n, little_endian).state, little_endian),\n forwarding_project_observation_v46(byte_operation_{input_namespace}_{ordinal}_v30(n, little_endian).observation) == forwarding_project_observation_v46(byte_operation_{output_namespace}_{ordinal}_v30(o, little_endian).observation),\n{{\n let left = byte_operation_{input_namespace}_{ordinal}_v30(n, little_endian);\n let right = byte_operation_{output_namespace}_{ordinal}_v30(o, little_endian);\n if n.valid {{\n assert(n == o);\n"
    );
    if plan.replacements[ordinal] != NONE {
        let Action::Load { slot } = plan.actions[ordinal] else {
            return Err(mismatch());
        };
        let value = plan.replacements[ordinal];
        let width = plan.slots[slot].bits;
        emit!(out, " assert(");
        emit_fact(input, plan, plan.before[ordinal], "n", out)?;
        emit!(
            out,
            ");\n match (n.values[{}], n.values[{value}]) {{\n (MemoryValueV30::Pointer(pointer), MemoryValueV30::Scalar(value)) => {{\n assert(byte_load_v30(n.memory, pointer, {}, little_endian) == value);\n assert(((value as u{width}) | (value as u{width})) == (value as u{width})) by(bit_vector);\n }}, _ => {{ assert(false); }}\n }}\n",
            plan.slots[slot].pointer,
            width / 8
        );
    }
    emit!(
        out,
        " }}\n assert(forwarding_terminal_state_v46(left.state) == forwarding_terminal_state_v46(right.state));\n assert(forwarding_state_invariant_v46(left.state));\n assert(forwarding_state_invariant_v46(right.state));\n assert(forwarding_{next_label}_fact_v46({next_coordinate}, left.state, little_endian));\n assert(forwarding_project_observation_v46(left.observation) == forwarding_project_observation_v46(right.observation));\n}}\n"
    );
    Ok(())
}

fn emit_block_law(
    input: &Inventory<'_>,
    block: usize,
    input_namespace: usize,
    output_namespace: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    out.budget.charge_work(2)?;
    let row = &input.blocks()[block];
    emit!(
        out,
        "proof fn forwarding_block_{block}_v46(n: MemoryStateV30, o: MemoryStateV30, little_endian: bool)\n requires n.valid, n.pc == {block}, forwarding_states_related_v46(n, o), forwarding_entry_fact_v46({block}, n, little_endian),\n ensures forwarding_states_related_v46(byte_block_{input_namespace}_{block}_v30(n, little_endian).state, byte_block_{output_namespace}_{block}_v30(o, little_endian).state),\n byte_block_{input_namespace}_{block}_v30(n, little_endian).state.valid && byte_block_{input_namespace}_{block}_v30(n, little_endian).state.pc >= 0 ==> forwarding_entry_fact_v46(byte_block_{input_namespace}_{block}_v30(n, little_endian).state.pc, byte_block_{input_namespace}_{block}_v30(n, little_endian).state, little_endian),\n forwarding_block_event_v46(byte_block_{input_namespace}_{block}_v30(n, little_endian)) == forwarding_block_event_v46(byte_block_{output_namespace}_{block}_v30(o, little_endian)),\n{{\n let ns = n; let os = o;\n"
    );
    for ordinal in row.operations.clone() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " forwarding_operation_{ordinal}_v46(ns, os, little_endian);\n let nr{ordinal} = byte_operation_{input_namespace}_{ordinal}_v30(ns, little_endian);\n let or{ordinal} = byte_operation_{output_namespace}_{ordinal}_v30(os, little_endian);\n let ns = nr{ordinal}.state; let os = or{ordinal}.state;\n"
        );
    }
    emit!(out, " let no = seq![");
    for ordinal in row.operations.clone() {
        out.budget.charge_work(1)?;
        emit!(out, "nr{ordinal}.observation,");
    }
    emit!(out, "];\n let oo = seq![");
    for ordinal in row.operations.clone() {
        out.budget.charge_work(1)?;
        emit!(out, "or{ordinal}.observation,");
    }
    emit!(
        out,
        "];\n assert(forwarding_project_observations_v46(no) == forwarding_project_observations_v46(oo));\n let next_n = byte_control_{input_namespace}_{block}_v30(ns, no);\n let next_o = byte_control_{output_namespace}_{block}_v30(os, oo);\n assert(forwarding_states_related_v46(next_n.state, next_o.state));\n if next_n.state.valid && next_n.state.pc >= 0 {{\n assert(forwarding_entry_fact_v46(next_n.state.pc, next_n.state, little_endian));\n }}\n assert(forwarding_block_event_v46(next_n) == forwarding_block_event_v46(next_o));\n}}\n"
    );
    Ok(())
}

fn emit_function_laws(
    input: &Inventory<'_>,
    function: usize,
    input_namespace: usize,
    output_namespace: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let row = &input.functions()[function];
    emit!(
        out,
        "open spec fn forwarding_function_pc_{function}_v46(pc: int) -> bool {{ pc == -1 || pc == -2"
    );
    for block in row.blocks.clone() {
        out.budget.charge_work(1)?;
        emit!(out, " || pc == {block}");
    }
    emit!(
        out,
        " }}\nopen spec fn forwarding_related_{function}_v46(n: MemoryStateV30, o: MemoryStateV30, little_endian: bool) -> bool {{ forwarding_states_related_v46(n, o) && (n.valid ==> forwarding_function_pc_{function}_v46(n.pc) && (n.pc >= 0 ==> forwarding_entry_fact_v46(n.pc, n, little_endian))) }}\n"
    );
    for (label, namespace) in [("input", input_namespace), ("output", output_namespace)] {
        emit!(
            out,
            "open spec fn forwarding_step_{label}_{function}_v46(state: MemoryStateV30, little_endian: bool) -> CfgStepV26<MemoryStateV30, ForwardingBlockEventV46> {{\n if !state.valid || state.pc < 0 {{ CfgStepV26 {{ state: forwarding_terminal_state_v46(state), events: seq![], halted: true }} }} else {{\n let result = byte_block_step_{namespace}_v30(state, little_endian);\n CfgStepV26 {{ state: forwarding_terminal_state_v46(result.state), events: seq![forwarding_block_event_v46(result)], halted: !result.state.valid || result.state.pc < 0 }}\n }}\n}}\n"
        );
    }
    emit!(
        out,
        "proof fn forwarding_step_refines_{function}_v46(n: MemoryStateV30, o: MemoryStateV30, little_endian: bool)\n requires forwarding_related_{function}_v46(n, o, little_endian),\n ensures forwarding_step_input_{function}_v46(n, little_endian).events == forwarding_step_output_{function}_v46(o, little_endian).events,\n forwarding_step_input_{function}_v46(n, little_endian).halted == forwarding_step_output_{function}_v46(o, little_endian).halted,\n forwarding_related_{function}_v46(forwarding_step_input_{function}_v46(n, little_endian).state, forwarding_step_output_{function}_v46(o, little_endian).state, little_endian),\n{{\n if n.valid && n.pc >= 0 {{\n"
    );
    for block in row.blocks.clone() {
        out.budget.charge_work(1)?;
        emit!(
            out,
            " if n.pc == {block} {{ forwarding_block_{block}_v46(n, o, little_endian); }} else"
        );
    }
    emit!(
        out,
        " {{ assert(false); }}\n }}\n}}\nproof fn forwarding_finite_trace_{function}_v46(n: MemoryStateV30, o: MemoryStateV30, little_endian: bool, fuel: nat)\n requires forwarding_related_{function}_v46(n, o, little_endian),\n ensures cfg_trace_v26(|s| forwarding_step_input_{function}_v46(s, little_endian), n, fuel).events == cfg_trace_v26(|s| forwarding_step_output_{function}_v46(s, little_endian), o, fuel).events,\n cfg_trace_v26(|s| forwarding_step_input_{function}_v46(s, little_endian), n, fuel).halted == cfg_trace_v26(|s| forwarding_step_output_{function}_v46(s, little_endian), o, fuel).halted,\n cfg_trace_v26(|s| forwarding_step_input_{function}_v46(s, little_endian), n, fuel).steps == cfg_trace_v26(|s| forwarding_step_output_{function}_v46(s, little_endian), o, fuel).steps,\n{{\n assert forall|a: MemoryStateV30, b: MemoryStateV30| forwarding_related_{function}_v46(a, b, little_endian) implies\n forwarding_step_input_{function}_v46(a, little_endian).events == forwarding_step_output_{function}_v46(b, little_endian).events\n && forwarding_step_input_{function}_v46(a, little_endian).halted == forwarding_step_output_{function}_v46(b, little_endian).halted\n && forwarding_related_{function}_v46(forwarding_step_input_{function}_v46(a, little_endian).state, forwarding_step_output_{function}_v46(b, little_endian).state, little_endian) by {{ forwarding_step_refines_{function}_v46(a, b, little_endian); }}\n cfg_finite_trace_refinement_v26(|s| forwarding_step_input_{function}_v46(s, little_endian), |s| forwarding_step_output_{function}_v46(s, little_endian), |a, b| forwarding_related_{function}_v46(a, b, little_endian), n, o, fuel);\n}}\n"
    );
    Ok(())
}

fn emit_initial(
    input: &Inventory<'_>,
    plan: &MemoryPlan,
    function: usize,
    width: fe2o3_kernel_ir::FormalIndexWidth,
    registry_namespace: usize,
    out: &mut Writer<'_, '_>,
) -> Result<()> {
    let row = &input.functions()[function];
    let entry = row.blocks.start;
    out.budget.charge_work(2)?;
    if row.blocks.is_empty() || plan.entry[entry] != NONE {
        return Err(Error::Statement(
            "forwarding native entry cannot assume a prior Store",
        ));
    }
    emit!(
        out,
        "open spec fn forwarding_native_inputs_{function}_v46(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, frames: MemoryFrameRuntimeV30) -> bool {{\n arguments.len() == {} && byte_memory_well_formed_v30(external) && byte_native_view_inputs_v38(external, arguments) && byte_frame_runtime_well_formed_v30(frames)\n && forwarding_heap_separate_v46(external)\n && (forall|allocation: MemoryAllocationV30| external.live.contains_key(allocation) ==> matches!(allocation, MemoryAllocationV30::External {{ .. }}))\n && (forall|i: int| 0 <= i < arguments.len() ==> forwarding_value_separate_v46(arguments[i]))",
        row.function.signature.parameters.len()
    );
    for (argument, ty) in row.function.signature.parameters.iter().enumerate() {
        out.budget.charge_work(1)?;
        emit!(out, "\n && ({{ let argument = arguments[{argument}]; ");
        byte_function_v30::emit_value_type(ty, width, "argument", out)?;
        emit!(out, " }})");
    }
    emit!(
        out,
        "\n}}\nopen spec fn forwarding_initial_{function}_v46(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, frames: MemoryFrameRuntimeV30, little_endian: bool) -> MemoryStateV30 {{\n let admitted = forwarding_native_inputs_{function}_v46(arguments, external, frames);\n let values = Seq::new({}nat, |i: int| MemoryValueV30::Undefined)",
        input.definitions().len()
    );
    let mut arguments = 0usize;
    for (ordinal, definition) in input.definitions()[row.definitions.clone()]
        .iter()
        .enumerate()
    {
        out.budget.charge_work(1)?;
        if let Definition::FunctionArgument {
            function: owner,
            argument,
        } = definition.coordinate
        {
            if owner != row.coordinate || argument as usize != arguments {
                return Err(Error::Statement(
                    "forwarding complete native argument order",
                ));
            }
            let actual = row
                .definitions
                .start
                .checked_add(ordinal)
                .ok_or(Resource::Arithmetic)?;
            emit!(
                out,
                ".update({actual}, if admitted {{ arguments[{argument}] }} else {{ MemoryValueV30::Undefined }})"
            );
            arguments = arguments.checked_add(1).ok_or(Resource::Arithmetic)?;
        }
    }
    if arguments != row.function.signature.parameters.len() {
        return Err(mismatch());
    }
    emit!(
        out,
        ";\n let memory = if admitted {{ ByteMemoryV30 {{ view_contracts: byte_target_view_contracts_{registry_namespace}_v38(little_endian), ..external }} }} else {{ external }};\n MemoryStateV30 {{ pc: {entry}, values, memory, generations: Map::empty(), frames, valid: admitted }}\n}}\nproof fn forwarding_initial_relation_{function}_v46(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, frames: MemoryFrameRuntimeV30, little_endian: bool)\n requires forwarding_native_inputs_{function}_v46(arguments, external, frames),\n ensures forwarding_initial_{function}_v46(arguments, external, frames, little_endian).valid,\n forwarding_related_{function}_v46(forwarding_initial_{function}_v46(arguments, external, frames, little_endian), forwarding_initial_{function}_v46(arguments, external, frames, little_endian), little_endian),\n{{\n let initial = forwarding_initial_{function}_v46(arguments, external, frames, little_endian);\n assert forall|slot: int| 0 <= slot < forwarding_slot_count_v46() implies forwarding_selected_slot_v46(initial, slot) by {{ assert(initial.values[forwarding_slot_definition_v46(slot)] == MemoryValueV30::Undefined); }}\n assert(forwarding_state_invariant_v46(initial));\n assert(forwarding_entry_fact_v46({entry}, initial, little_endian));\n}}\nproof fn forwarding_initial_trace_{function}_v46(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, frames: MemoryFrameRuntimeV30, little_endian: bool, fuel: nat)\n requires forwarding_native_inputs_{function}_v46(arguments, external, frames),\n ensures cfg_trace_v26(|s| forwarding_step_input_{function}_v46(s, little_endian), forwarding_initial_{function}_v46(arguments, external, frames, little_endian), fuel).events == cfg_trace_v26(|s| forwarding_step_output_{function}_v46(s, little_endian), forwarding_initial_{function}_v46(arguments, external, frames, little_endian), fuel).events,\n cfg_trace_v26(|s| forwarding_step_input_{function}_v46(s, little_endian), forwarding_initial_{function}_v46(arguments, external, frames, little_endian), fuel).halted == cfg_trace_v26(|s| forwarding_step_output_{function}_v46(s, little_endian), forwarding_initial_{function}_v46(arguments, external, frames, little_endian), fuel).halted,\n{{\n forwarding_initial_relation_{function}_v46(arguments, external, frames, little_endian);\n let initial = forwarding_initial_{function}_v46(arguments, external, frames, little_endian);\n forwarding_finite_trace_{function}_v46(initial, initial, little_endian, fuel);\n}}\n"
    );
    Ok(())
}

// Shared vocabulary, view registries and trace prelude are emitted once by the
// enclosing producer. This module contains the exact two actual interpreters.
pub(super) fn generate<'a, 'owner, R: ByteAllocationResolverV30>(
    input: &'a Inventory<'owner>,
    output: &'a Inventory<'owner>,
    pair: &'a Pair<'owner>,
    input_physical: &'a fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38<'a, 'owner>,
    output_physical: &'a fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38<'a, 'owner>,
    input_contracts: &'a target_view_contracts_v38::TargetByteViewContractsV38<'a, 'owner>,
    output_contracts: &'a target_view_contracts_v38::TargetByteViewContractsV38<'a, 'owner>,
    width: fe2o3_kernel_ir::FormalIndexWidth,
    registry_namespace: usize,
    allocation_origins: &'a R,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    allocation_origins.check_owner(input.owner(), out)?;
    input_contracts.check_owner_width_v39(input.owner(), width, out)?;
    output_contracts.check_owner_width_v39(output.owner(), width, out)?;
    let floor = out.budget.storage();
    let result = (|| {
        out.budget.reserve_storage(
            size_of::<Result<usize>>() + 8 * size_of::<&()>() + 5 * size_of::<usize>(),
        )?;
        out.budget.charge_work(3)?;
        if input.functions().len() != output.functions().len() {
            return Err(mismatch());
        }
        let input_context = byte_function_v30::ByteInterpretationContextV39::classified(
            width,
            input_contracts,
            registry_namespace,
        );
        let output_context = byte_function_v30::ByteInterpretationContextV39::classified(
            width,
            output_contracts,
            registry_namespace,
        );
        let plan = MemoryPlan::build(input, output, pair, out)?;
        let allocations = Allocations::derive(input, output, pair, allocation_origins, out)?;
        emit!(out, "mod forwarding_v46 {{\nuse super::*;\n");
        emit_classifiers(input, &plan, &allocations, out)?;
        let functions = emit_models(
            input,
            output,
            input_physical,
            output_physical,
            input_context,
            output_context,
            &plan,
            &allocations,
            width,
            registry_namespace,
            out,
        )?;
        allocations.check(out)?;
        emit!(out, "}}\n");
        drop((allocations, plan));
        Ok(functions)
    })();
    let release = out
        .budget
        .storage()
        .checked_sub(floor)
        .ok_or(Resource::Accounting)?;
    out.budget.release_storage(release)?;
    result
}

fn emit_models<R: ByteAllocationResolverV30>(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    input_physical: &fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38<'_, '_>,
    output_physical: &fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38<'_, '_>,
    input_context: byte_function_v30::ByteInterpretationContextV39<'_, '_>,
    output_context: byte_function_v30::ByteInterpretationContextV39<'_, '_>,
    plan: &MemoryPlan,
    allocations: &Allocations<'_, '_, R>,
    width: fe2o3_kernel_ir::FormalIndexWidth,
    registry_namespace: usize,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    let mut functions = 0usize;
    for (function, row) in input.functions().iter().enumerate() {
        out.budget.charge_work(2)?;
        if row.function.body.is_none() {
            continue;
        }
        let function_floor = out.budget.storage();
        let input_namespace = function.checked_mul(2).ok_or(Resource::Arithmetic)?;
        let output_namespace = input_namespace.checked_add(1).ok_or(Resource::Arithmetic)?;
        let before = byte_function_v30::ByteFunctionV30::derive(
            input,
            input_physical,
            row.coordinate,
            input_context,
            allocations,
            out,
        )?;
        let after = byte_function_v30::ByteFunctionV30::derive(
            output,
            output_physical,
            row.coordinate,
            output_context,
            allocations,
            out,
        )?;
        before.emit(input_namespace, out)?;
        after.emit(output_namespace, out)?;
        for ordinal in row.operations.clone() {
            emit_operation_law(input, plan, ordinal, input_namespace, output_namespace, out)?;
        }
        for block in row.blocks.clone() {
            emit_block_law(input, block, input_namespace, output_namespace, out)?;
        }
        emit_function_laws(input, function, input_namespace, output_namespace, out)?;
        emit_initial(input, plan, function, width, registry_namespace, out)?;
        drop((before, after));
        out.budget.release_storage(
            out.budget
                .storage()
                .checked_sub(function_floor)
                .ok_or(Resource::Accounting)?,
        )?;
        functions = functions.checked_add(1).ok_or(Resource::Arithmetic)?;
    }
    Ok(functions)
}

pub(super) fn generate_segmented<R: ByteAllocationResolverV30, S: ByteAllocationResolverV30>(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    pair: &Pair<'_>,
    input_physical: &fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38<'_, '_>,
    output_physical: &fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38<'_, '_>,
    input_contracts: &target_view_contracts_v38::TargetByteViewContractsV38<'_, '_>,
    output_contracts: &target_view_contracts_v38::TargetByteViewContractsV38<'_, '_>,
    width: fe2o3_kernel_ir::FormalIndexWidth,
    registry_namespace: usize,
    allocation_origins: &R,
    segments: &allocation_bridge_v48::PrefixSegmentsV49<'_, '_, '_, '_, S>,
    relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
    out: &mut Writer<'_, '_>,
) -> Result<usize> {
    allocation_origins.check_owner(input.owner(), out)?;
    input_contracts.check_owner_width_v39(input.owner(), width, out)?;
    output_contracts.check_owner_width_v39(output.owner(), width, out)?;
    let floor = out.budget.storage();
    let result = (|| {
        out.budget
            .reserve_storage(size_of::<([&(); 16], [usize; 8], [Result<()>; 2])>())?;
        let plan = MemoryPlan::build(input, output, pair, out)?;
        let allocations = Allocations::derive(input, output, pair, allocation_origins, out)?;
        emit!(out, "mod forwarding_v46 {{\nuse super::*;\n");
        emit_classifiers(input, &plan, &allocations, out)?;
        let functions = emit_models(
            input,
            output,
            input_physical,
            output_physical,
            byte_function_v30::ByteInterpretationContextV39::classified(
                width,
                input_contracts,
                registry_namespace,
            ),
            byte_function_v30::ByteInterpretationContextV39::classified(
                width,
                output_contracts,
                registry_namespace,
            ),
            &plan,
            &allocations,
            width,
            registry_namespace,
            out,
        )?;
        segments.emit_forwarding_composition(output, pair, &plan, relation, out)?;
        allocations.check(out)?;
        emit!(out, "}}\n");
        drop((allocations, plan));
        Ok(functions)
    })();
    out.budget.release_storage(
        out.budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?,
    )?;
    result
}

#[cfg(test)]
#[path = "mixed_optimizer_store_consensus_typed_v46_tests.rs"]
mod tests;
