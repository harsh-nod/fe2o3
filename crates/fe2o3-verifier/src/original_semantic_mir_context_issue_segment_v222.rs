//! Replay a real source terminator and one real target micro-operation.
//! This does not establish the statements before this boundary, arrival at the
//! next source cut, or preservation of other live values and dynamic leases.
//! The caller must establish the actual target byte-input admission predicate.
use super::super::source_function::{ContextIssueSiteV222, SourceByteProgram};
use super::*;

pub(super) fn headers() -> usize {
    size_of::<ContextIssueSiteV222>()
        + size_of::<Site>()
        + size_of::<Owner>()
        + size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + size_of::<Result<()>>()
        + 12 * size_of::<usize>()
        + 12 * size_of::<&()>()
}

impl ExpandedExecutionBindingsV199<'_, '_, '_, '_, '_> {
    pub(in super::super) fn emit_context_issue_segment_v222(
        &self,
        program: &SourceByteProgram<'_, '_, '_>,
        root: usize,
        instance: usize,
        block: usize,
        value: Value,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            out.budget.charge_work(1)?;
            if !std::ptr::eq(self.slots, program.source_slots(out)?) { return Err(mismatch()); }
            let call = program.context_issue_site_v222(root, instance, block, out)?.ok_or_else(mismatch)?;
            let endpoint = self.slots.correspondence(out)?.ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
            let owner = endpoint.execution_owner_v199(out.budget)?.ok_or_else(mismatch)?;
            let row = self.plan.instance(root, instance, out)?;
            let local = endpoint.source_local(out.budget)?;
            out.budget.charge_work(10)?;
            if owner.role != SourceRole::KernelContext || owner.identity.instance != instance
                || owner.identity.block.index() as usize != block
                || owner.identity.destination != local || owner.identity.source_type != call.source_type
                || endpoint.source_type(out.budget)? != call.source_type
                || endpoint.source_function(out.budget)? != row.function
                || endpoint.execution_borrow_v163(out.budget)?.is_some()
                || row.locals.start.checked_add(local.index() as usize) != Some(call.destination)
            { return Err(mismatch()); }
            let site = self.site(root, owner.identity, KirRole::Context, out)?;
            let inventory = self.target.inventory(out)?;
            out.budget.charge_work(6)?;
            let operation = operation_index(inventory, site.operation)?;
            let actual_block = crate::mixed_optimizer_refinement_v26::semantics::block_index(inventory, site.operation.block)?;
            let operations = &inventory.blocks()[actual_block].operations;
            if !operations.contains(&operation) { return Err(mismatch()); }
            let prefix = operation.checked_sub(operations.start).ok_or(Resource::Arithmetic)?;
            let next = operation.checked_add(1).ok_or(Resource::Arithmetic)?;
            let next = if next == operations.end { -1i128 } else { next as i128 };
            let (pc, statements, destination, source_type, continuation) = (
                call.pc, call.statements, call.destination, call.source_type.index(), call.continuation);
            let definition = site.definition;
            let (function, target_block, target_operation) = (site.operation.block.function.0, site.operation.block.block, site.operation.operation);
            write!(out, r#"
// Exact endpoint replay only. No whole-cut or lifetime-preservation claim.
proof fn invocation_context_issue_segment_{root}_{instance}_{block}_v222(
    source: InvocationSourceMicroStateV36, target: MemoryMicroStateV30,
    execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>,
    little_endian: bool,
)
    requires invocation_source_active_{root}_{instance}_v36(source.source),
        source.source.machine.pc == {pc}, source.next_statement == {statements},
        source.observations.len() == {statements},
        target.state.pc == {actual_block}, target.next_operation == {operation},
        target.observations.len() == {prefix},
        byte_inputs_{root}_v55(target.state, little_endian),
    ensures ({{
        let issue = InvocationSourceContextIssueV161 {{ destination: {destination}, source_type: {source_type} }};
        let site = MemorySourceOperationV30 {{ function: {function}, block: {target_block}, operation: {target_operation} }};
        let coupled = invocation_context_issue_coupled_v211(source.source, target.state, execution_map, issue, site, {definition});
        let original = invocation_source_micro_finish_{root}_{instance}_v36(source, little_endian);
        let actual = byte_micro_step_{root}_v30(target, little_endian);
        original.source == invocation_source_byte_pc_v36(coupled.source.source, {continuation})
            && original.before_control == source.source
            && original.observations == source.observations.push(InvocationSourceStatementObservationV36 {{
                root: {root}, instance: {instance}, block: {block}, statement: {statements},
                event: Some(InvocationSourceByteEventV36::ContextIssue(issue)),
                before: source.source, after: coupled.source.source }})
            && original.operands == Seq::empty() && original.returned == None
            && actual.next.state == coupled.target && actual.next.next_operation == {next}
            && actual.next.observations == target.observations.push(actual.observation)
            && actual.observation == (MemoryOperationObservationV30 {{
                operation: site, before: target.state, after: coupled.target,
                valid_before: target.state.valid, valid_after: coupled.target.valid,
                effect: MemoryOperationEffectV30::Pure }})
    }}),
{{ }}
"#).map_err(|_| out.error())?;
            out.budget.charge_work(1)?;
            write!(out, r#"
// Fresh issuance from an empty witness only, not live-map preservation.
proof fn invocation_context_issue_initial_map_segment_{root}_{instance}_{block}_v232(
    source: InvocationSourceMicroStateV36, target: MemoryMicroStateV30,
    little_endian: bool,
)
    requires invocation_source_active_{root}_{instance}_v36(source.source),
        source.source.machine.pc == {pc}, source.next_statement == {statements},
        source.observations.len() == {statements},
        target.state.pc == {actual_block}, target.next_operation == {operation},
        target.observations.len() == {prefix},
        byte_inputs_{root}_v55(target.state, little_endian),
        invocation_context_issue_fresh_enabled_v211(source.source, target.state,
            InvocationSourceContextIssueV161 {{ destination: {destination}, source_type: {source_type} }},
            MemorySourceOperationV30 {{ function: {function}, block: {target_block}, operation: {target_operation} }},
            {definition}),
    ensures ({{
        let issue = InvocationSourceContextIssueV161 {{ destination: {destination}, source_type: {source_type} }};
        let site = MemorySourceOperationV30 {{ function: {function}, block: {target_block}, operation: {target_operation} }};
        let coupled = invocation_context_issue_coupled_v211(source.source, target.state, Map::empty(), issue, site, {definition});
        let original = invocation_source_micro_finish_{root}_{instance}_v36(source, little_endian);
        let actual = byte_micro_step_{root}_v30(target, little_endian);
        coupled.updated && invocation_execution_map_current_v205(
            original.source, actual.next.state, coupled.execution_map)
    }}),
{{
    let issue = InvocationSourceContextIssueV161 {{ destination: {destination}, source_type: {source_type} }};
    let site = MemorySourceOperationV30 {{ function: {function}, block: {target_block}, operation: {target_operation} }};
    invocation_context_issue_segment_{root}_{instance}_{block}_v222(
        source, target, Map::empty(), little_endian);
    invocation_context_issue_fresh_has_exact_updates_v211(
        source.source, target.state, Map::empty(), issue, site, {definition});
    invocation_context_issue_initializes_current_map_v211(
        source.source, target.state, issue, site, {definition});
    let coupled = invocation_context_issue_coupled_v211(
        source.source, target.state, Map::empty(), issue, site, {definition});
    let identity = MemoryExecutionReferenceV178 {{
        definition: {definition}, epoch: 0, frame: target.state.frames.active.last() }};
    assert(coupled.execution_map.contains_key(identity));
    assert(invocation_execution_map_entry_v205(coupled.source.source, coupled.target,
        identity, coupled.execution_map[identity]));
    assert(invocation_source_byte_state_well_formed_v36(coupled.source.source));
    assert(coupled.source.source.machine.pc == {pc});
    let continued = invocation_source_byte_pc_v36(coupled.source.source, {continuation});
    assert(continued.machine == (MemoryStateV30 {{ pc: {continuation}, ..coupled.source.source.machine }}));
    assert(continued.logical == coupled.source.source.logical
        && continued.slots == coupled.source.source.slots
        && continued.objects == coupled.source.source.objects);
    assert forall|key: MemoryExecutionReferenceV178|
        #![trigger coupled.execution_map[key]]
        coupled.execution_map.contains_key(key) implies
            invocation_execution_map_entry_v205(continued, coupled.target,
                key, coupled.execution_map[key]) by {{
        assert(invocation_execution_map_entry_v205(coupled.source.source, coupled.target,
            key, coupled.execution_map[key]));
    }}
}}
"#).map_err(|_| out.error())?;
            out.budget.charge_work(1)?;
            write!(out, r#"
// Fresh issuance with a current input witness, not reissue or general history.
proof fn invocation_context_issue_current_map_segment_{root}_{instance}_{block}_v240(
    source: InvocationSourceMicroStateV36, target: MemoryMicroStateV30,
    execution_map: Map<MemoryExecutionReferenceV178, InvocationExecutionOriginV205>,
    little_endian: bool,
)
    requires invocation_source_active_{root}_{instance}_v36(source.source),
        source.source.machine.pc == {pc}, source.next_statement == {statements},
        source.observations.len() == {statements},
        target.state.pc == {actual_block}, target.next_operation == {operation},
        target.observations.len() == {prefix},
        byte_inputs_{root}_v55(target.state, little_endian),
        invocation_context_issue_fresh_enabled_v211(source.source, target.state,
            InvocationSourceContextIssueV161 {{ destination: {destination}, source_type: {source_type} }},
            MemorySourceOperationV30 {{ function: {function}, block: {target_block}, operation: {target_operation} }},
            {definition}),
        invocation_execution_map_current_v205(source.source, target.state, execution_map),
    ensures ({{
        let issue = InvocationSourceContextIssueV161 {{ destination: {destination}, source_type: {source_type} }};
        let site = MemorySourceOperationV30 {{ function: {function}, block: {target_block}, operation: {target_operation} }};
        let coupled = invocation_context_issue_coupled_v211(source.source, target.state, execution_map, issue, site, {definition});
        let original = invocation_source_micro_finish_{root}_{instance}_v36(source, little_endian);
        let actual = byte_micro_step_{root}_v30(target, little_endian);
        coupled.updated && invocation_execution_map_current_v205(
            original.source, actual.next.state, coupled.execution_map)
    }}),
{{
    let issue = InvocationSourceContextIssueV161 {{ destination: {destination}, source_type: {source_type} }};
    let site = MemorySourceOperationV30 {{ function: {function}, block: {target_block}, operation: {target_operation} }};
    invocation_context_issue_segment_{root}_{instance}_{block}_v222(
        source, target, execution_map, little_endian);
    invocation_context_issue_fresh_has_exact_updates_v211(
        source.source, target.state, execution_map, issue, site, {definition});
    invocation_context_issue_fresh_preserves_frame_v259(
        source.source, target.state, execution_map, issue, site, {definition});
    let coupled = invocation_context_issue_coupled_v211(
        source.source, target.state, execution_map, issue, site, {definition});
    let identity = MemoryExecutionReferenceV178 {{
        definition: {definition}, epoch: 0, frame: target.state.frames.active.last() }};
    assert(coupled.execution_map.contains_key(identity));
    assert(invocation_execution_map_entry_v205(coupled.source.source, coupled.target,
        identity, coupled.execution_map[identity]));
    assert(invocation_source_byte_state_well_formed_v36(coupled.source.source));
    assert(coupled.source.source.machine == (MemoryStateV30 {{
        values: source.source.machine.values.update({destination}, MemoryValueV30::Undefined),
        ..source.source.machine }}));
    assert(coupled.source.source.machine.pc == {pc});
    let continued = invocation_source_byte_pc_v36(coupled.source.source, {continuation});
    assert(continued.machine == (MemoryStateV30 {{ pc: {continuation}, ..coupled.source.source.machine }}));
    assert(continued.logical == coupled.source.source.logical
        && continued.slots == coupled.source.source.slots
        && continued.objects == coupled.source.source.objects);
    assert forall|key: MemoryExecutionReferenceV178|
        #![trigger coupled.execution_map[key]]
        coupled.execution_map.contains_key(key) implies
            invocation_execution_map_entry_v205(continued, coupled.target,
                key, coupled.execution_map[key]) by {{
        assert(invocation_execution_map_entry_v205(coupled.source.source, coupled.target,
            key, coupled.execution_map[key]));
    }}
}}
"#).map_err(|_| out.error())?;
            self.check(out)
        })
    }
}
