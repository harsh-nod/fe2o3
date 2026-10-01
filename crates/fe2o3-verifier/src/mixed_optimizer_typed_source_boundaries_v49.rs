//! Original source cuts composed with concrete checked-prefix/LICM cursors.
//! The source and middle states are proof witnesses, not inputs to execution.
use super::*;

const BOUNDARY: &str = r#"
struct TypedSourceBoundaryV49 {
    cursor: TypedPrefixCursorV49,
    observations: Seq<MemoryOperationObservationV30>,
    returned: Seq<MemoryValueV30>,
    blocks: Seq<int>,
}
open spec fn typed_source_boundary_empty_v49(cursor: TypedPrefixCursorV49) -> TypedSourceBoundaryV49 {
    TypedSourceBoundaryV49 { cursor, observations: seq![], returned: seq![], blocks: seq![] }
}
open spec fn typed_source_boundary_join_v49(block: int, head: TypedPrefixStepV49,
    tail: TypedSourceBoundaryV49,
) -> TypedSourceBoundaryV49 {
    TypedSourceBoundaryV49 { cursor: tail.cursor,
        observations: head.observations + tail.observations,
        returned: if head.cursor.micro.state.pc < 0 { head.returned } else { tail.returned },
        blocks: seq![block] + tail.blocks }
}
open spec fn typed_source_observations_valid_v49(observations: Seq<MemoryOperationObservationV30>) -> bool {
    forall|i: int| 0 <= i < observations.len() ==>
        byte_observation_snapshots_valid_v39(observations[i])
        && observations[i].before.valid && observations[i].after.valid
        && (match observations[i].effect {
            MemoryOperationEffectV30::Trap => observations[i].before.pc >= 0
                && observations[i].after == (MemoryStateV30 { pc: -2, ..observations[i].before }),
            _ => true,
        })
}
// Only the input to source map/effect predicates is projected. These records
// are never passed to a ByteFunction, a source interpreter, or a cut step.
open spec fn typed_source_mapped_observation_v49(observation: MemoryOperationObservationV30)
    -> MemoryOperationObservationV30 {
    MemoryOperationObservationV30 { before: typed_original_map_view_2_v48(observation.before),
        after: typed_original_map_view_2_v48(observation.after), ..observation }
}
open spec fn typed_source_mapped_observations_v49(observations: Seq<MemoryOperationObservationV30>)
    -> Seq<MemoryOperationObservationV30> {
    let observable = invocation_actual_observations_v39(observations);
    Seq::new(observable.len(), |i: int| typed_source_mapped_observation_v49(observable[i]))
}
"#;

impl<'b, 'a, 'owner, 'rows, R: ByteAllocationResolverV30>
    PrefixSegmentsV49<'b, 'a, 'owner, 'rows, R>
{
    pub(super) fn emit_source_composition(
        &self,
        relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let inventory = relation.inventory(out.budget)?;
        if !std::ptr::eq(inventory.owner(), self.bridge.prefix.input().owner()) {
            return Err(mismatch());
        }
        let source = relation.source(out.budget)?;
        let roots = source.root_count(out.budget)?;
        let floor = out.budget.storage();
        let result = (|| {
            out.budget
                .reserve_storage(size_of::<([usize; 8], [&(); 4])>())?;
            let mut root_by_function = allocate(inventory.functions().len(), out)?;
            for root in 0..roots {
                let (_, function) = source.root(root, out.budget)?;
                self.bridge.charge(3, out)?;
                let slot = root_by_function.get_mut(function).ok_or_else(mismatch)?;
                if *slot != NONE || inventory.functions()[function].function.body.is_none() {
                    return Err(mismatch());
                }
                *slot = root;
            }
            emit!(out, "{BOUNDARY}\n");
            let mut emitted = 0usize;
            for (function, association) in self.bridge.prefix.rows().functions.iter().enumerate() {
                self.bridge.charge(3, out)?;
                let original = association.input.0 as usize;
                let root = root_by_function.get_mut(original).ok_or_else(mismatch)?;
                if *root == NONE {
                    continue;
                }
                self.emit_source_root(*root, original, function, out)?;
                *root = NONE;
                emitted = emitted.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
            self.bridge.charge(root_by_function.len(), out)?;
            if emitted != roots || root_by_function.iter().any(|root| *root != NONE) {
                return Err(mismatch());
            }
            drop(root_by_function);
            Ok(())
        })();
        out.budget.release_storage(
            out.budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?,
        )?;
        if let Err(Error::Resource(error)) = &result {
            self.bridge.failure.set(Some(*error));
        }
        result?;
        self.check(out)
    }

    fn emit_source_root(
        &self,
        root: usize,
        original: usize,
        function: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let range = self.bridge.prefix.input().functions()[original]
            .blocks
            .clone();
        let start = range.start;
        let end = range.end;
        let bound = range.len();
        self.bridge.charge(12, out)?;
        self.emit_observation_transport(root, out)?;
        // Both independently emitted functions consume the identical actual owner,
        // physical analysis, source allocation resolver and registry namespace.
        emit!(
            out,
            "proof fn typed_source_original_block_{root}_v49(s: MemoryStateV30)\n ensures super::byte_block_step_{root}_v30(s, invocation_runtime_little_endian_v36()) == byte_block_step_{original}_v30(s, invocation_runtime_little_endian_v36()),\n{{ }}\n"
        );
        for stage in ["prefix", "relocated"] {
            self.bridge.charge(4, out)?;
            emit!(
                out,
                r#"open spec fn typed_source_{stage}_follow_{root}_v49(cursor: TypedPrefixCursorV49, fuel: nat) -> TypedSourceBoundaryV49
 decreases fuel
{{ if !cursor.micro.state.valid || cursor.micro.state.pc < 0 || invocation_byte_cut_{root}_v36(cursor.segment) {{ typed_source_boundary_empty_v49(cursor) }}
 else if fuel == 0 || cursor.segment < {start} || {end} <= cursor.segment {{ typed_source_boundary_empty_v49(typed_prefix_refuse_v49(cursor).cursor) }}
 else {{ let head = typed_{stage}_step_{function}_v49(cursor, invocation_runtime_little_endian_v36());
 let tail = typed_source_{stage}_follow_{root}_v49(head.cursor, (fuel - 1) as nat);
 typed_source_boundary_join_v49(cursor.segment, head, tail) }} }}
open spec fn typed_source_{stage}_boundary_{root}_v49(cursor: TypedPrefixCursorV49) -> TypedSourceBoundaryV49 {{
 if cursor.micro.state.pc < 0 {{ typed_source_boundary_empty_v49(cursor) }}
 else if !cursor.micro.state.valid || !invocation_byte_cut_{root}_v36(cursor.segment) {{ typed_source_boundary_empty_v49(typed_prefix_refuse_v49(cursor).cursor) }}
 else {{ let head = typed_{stage}_step_{function}_v49(cursor, invocation_runtime_little_endian_v36());
 let tail = typed_source_{stage}_follow_{root}_v49(head.cursor, {bound}nat);
 typed_source_boundary_join_v49(cursor.segment, head, tail) }} }}
open spec fn typed_source_{stage}_cut_step_{root}_v49(cursor: TypedPrefixCursorV49) -> CfgStepV26<TypedPrefixCursorV49, ()> {{
 let next = typed_source_{stage}_boundary_{root}_v49(cursor);
 CfgStepV26 {{ state: next.cursor, events: seq![], halted: next.cursor.micro.state.pc < 0 }} }}
"#
            );
        }
        emit!(
            out,
            r#"proof fn typed_source_follow_{root}_v49(original: MemoryStateV30, middle: TypedPrefixCursorV49, actual: TypedPrefixCursorV49, fuel: nat)
 requires typed_composed_related_{function}_v49(original, middle, actual, invocation_runtime_little_endian_v36()),
 invocation_byte_follow_{root}_v36(original, fuel).state.valid,
 ensures typed_composed_related_{function}_v49(invocation_byte_follow_{root}_v36(original, fuel).state, typed_source_prefix_follow_{root}_v49(middle, fuel).cursor, typed_source_relocated_follow_{root}_v49(actual, fuel).cursor, invocation_runtime_little_endian_v36()),
 typed_observations_0_v48(invocation_byte_follow_{root}_v36(original, fuel).observations) == typed_observations_2_v48(typed_source_relocated_follow_{root}_v49(actual, fuel).observations),
 typed_observations_0_v48(invocation_byte_follow_{root}_v36(original, fuel).observations).is_some(),
 typed_source_observations_valid_v49(invocation_byte_follow_{root}_v36(original, fuel).observations),
 typed_source_observations_valid_v49(typed_source_relocated_follow_{root}_v49(actual, fuel).observations),
 invocation_byte_follow_{root}_v36(original, fuel).returned == typed_source_relocated_follow_{root}_v49(actual, fuel).returned,
 invocation_byte_follow_{root}_v36(original, fuel).blocks == typed_source_relocated_follow_{root}_v49(actual, fuel).blocks,
 decreases fuel,
{{ if original.pc >= 0 && !invocation_byte_cut_{root}_v36(original.pc) {{
 assert(fuel > 0 && {start} <= original.pc < {end});
 typed_source_original_block_{root}_v49(original);
 let head = super::byte_block_step_{root}_v30(original, invocation_runtime_little_endian_v36());
 assert(head.state.valid);
 typed_composed_step_{function}_v49(original, middle, actual, invocation_runtime_little_endian_v36());
 let center = typed_prefix_step_{function}_v49(middle, invocation_runtime_little_endian_v36());
 let right = typed_relocated_step_{function}_v49(actual, invocation_runtime_little_endian_v36());
 typed_source_follow_{root}_v49(head.state, center.cursor, right.cursor, (fuel - 1) as nat);
}} }}
proof fn typed_source_boundary_{root}_v49(original: MemoryStateV30, middle: TypedPrefixCursorV49, actual: TypedPrefixCursorV49)
 requires typed_composed_related_{function}_v49(original, middle, actual, invocation_runtime_little_endian_v36()), invocation_byte_boundary_{root}_v36(original).state.valid,
 ensures typed_composed_related_{function}_v49(invocation_byte_boundary_{root}_v36(original).state, typed_source_prefix_boundary_{root}_v49(middle).cursor, typed_source_relocated_boundary_{root}_v49(actual).cursor, invocation_runtime_little_endian_v36()),
 typed_observations_0_v48(invocation_byte_boundary_{root}_v36(original).observations) == typed_observations_2_v48(typed_source_relocated_boundary_{root}_v49(actual).observations),
 typed_observations_0_v48(invocation_byte_boundary_{root}_v36(original).observations).is_some(),
 typed_source_observations_valid_v49(invocation_byte_boundary_{root}_v36(original).observations),
 typed_source_observations_valid_v49(typed_source_relocated_boundary_{root}_v49(actual).observations),
 invocation_byte_boundary_{root}_v36(original).returned == typed_source_relocated_boundary_{root}_v49(actual).returned,
 invocation_byte_boundary_{root}_v36(original).blocks == typed_source_relocated_boundary_{root}_v49(actual).blocks,
{{ if original.pc >= 0 {{
 assert(invocation_byte_cut_{root}_v36(original.pc));
 typed_source_original_block_{root}_v49(original);
 let head = super::byte_block_step_{root}_v30(original, invocation_runtime_little_endian_v36());
 assert(head.state.valid);
 typed_composed_step_{function}_v49(original, middle, actual, invocation_runtime_little_endian_v36());
 typed_source_follow_{root}_v49(head.state, typed_prefix_step_{function}_v49(middle, invocation_runtime_little_endian_v36()).cursor, typed_relocated_step_{function}_v49(actual, invocation_runtime_little_endian_v36()).cursor, {bound}nat);
}} }}
open spec fn typed_source_related_{root}_v49(source: InvocationSourceByteStateV36, original: MemoryStateV30, middle: TypedPrefixCursorV49, actual: TypedPrefixCursorV49) -> bool {{
 invocation_paired_related_{root}_v36(source, original) && typed_composed_related_{function}_v49(original, middle, actual, invocation_runtime_little_endian_v36()) }}
proof fn typed_source_step_{root}_v49(source: InvocationSourceByteStateV36, original: MemoryStateV30, middle: TypedPrefixCursorV49, actual: TypedPrefixCursorV49)
 requires typed_source_related_{root}_v49(source, original, middle, actual), invocation_paired_source_defined_{root}_v36(source, 1),
 ensures typed_source_related_{root}_v49(invocation_paired_source_step_{root}_v36(source).state, invocation_byte_boundary_{root}_v36(original).state, typed_source_prefix_boundary_{root}_v49(middle).cursor, typed_source_relocated_boundary_{root}_v49(actual).cursor),
 invocation_paired_source_step_{root}_v36(source).halted == (typed_source_relocated_boundary_{root}_v49(actual).cursor.micro.state.pc < 0),
 typed_observations_0_v48(invocation_byte_boundary_{root}_v36(original).observations) == typed_observations_2_v48(typed_source_relocated_boundary_{root}_v49(actual).observations),
 invocation_paired_observations_related_{root}_v39(invocation_paired_source_step_{root}_v36(source).events, typed_source_mapped_observations_v49(typed_source_relocated_boundary_{root}_v49(actual).observations)),
 invocation_byte_boundary_{root}_v36(original).returned == typed_source_relocated_boundary_{root}_v49(actual).returned,
{{ invocation_paired_step_{root}_v36(source, original); typed_source_boundary_{root}_v49(original, middle, actual);
 typed_source_observation_transport_{root}_v49(invocation_byte_boundary_{root}_v36(original).observations, typed_source_relocated_boundary_{root}_v49(actual).observations);
 invocation_source_observations_extensionality_{root}_v49(invocation_paired_source_step_{root}_v36(source).events, invocation_actual_observations_v39(invocation_byte_boundary_{root}_v36(original).observations), typed_source_mapped_observations_v49(typed_source_relocated_boundary_{root}_v49(actual).observations)); }}
open spec fn typed_source_middle_ready_{root}_v49(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37) -> TypedSourceBoundaryV49 {{
 typed_source_prefix_follow_{root}_v49(typed_prefix_entry_{function}_v49(invocation_paired_raw_initial_{root}_v36(arguments, external, execution)), {bound}nat) }}
open spec fn typed_source_actual_ready_{root}_v49(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37) -> TypedSourceBoundaryV49 {{
 let original = invocation_paired_raw_initial_{root}_v36(arguments, external, execution);
 let middle = typed_prefix_entry_{function}_v49(original);
 typed_source_relocated_follow_{root}_v49(typed_relocated_entry_{function}_v49(middle.micro.state), {bound}nat) }}
proof fn typed_source_initial_{root}_v49(arguments: Seq<MemoryValueV30>, external: ByteMemoryV30, execution: MemoryExecutionContextV37)
 requires invocation_paired_native_inputs_{root}_v38(arguments, external, execution),
 ensures typed_source_related_{root}_v49(invocation_source_initial_runtime_{root}_v36(arguments, external, execution), invocation_paired_ready_{root}_v36(arguments, external, execution).state, typed_source_middle_ready_{root}_v49(arguments, external, execution).cursor, typed_source_actual_ready_{root}_v49(arguments, external, execution).cursor),
 typed_observations_0_v48(invocation_paired_ready_{root}_v36(arguments, external, execution).observations) == typed_observations_2_v48(typed_source_actual_ready_{root}_v49(arguments, external, execution).observations),
 typed_observations_0_v48(invocation_paired_ready_{root}_v36(arguments, external, execution).observations).is_some(),
 typed_source_observations_valid_v49(invocation_paired_ready_{root}_v36(arguments, external, execution).observations),
 typed_source_observations_valid_v49(typed_source_actual_ready_{root}_v49(arguments, external, execution).observations),
{{ invocation_paired_initial_{root}_v36(arguments, external, execution);
 let original = invocation_paired_raw_initial_{root}_v36(arguments, external, execution);
 typed_composed_entry_relation_{function}_v49(original, invocation_runtime_little_endian_v36());
 let middle = typed_prefix_entry_{function}_v49(original);
 let actual = typed_relocated_entry_{function}_v49(middle.micro.state);
 typed_source_follow_{root}_v49(original, middle, actual, {bound}nat);
}}
"#
        );
        self.check(out)
    }

    fn emit_observation_transport(&self, root: usize, out: &mut Writer<'_, '_>) -> Result<()> {
        self.bridge.charge(6, out)?;
        emit!(
            out,
            r#"proof fn typed_source_observation_pair_{root}_v49(a: MemoryOperationObservationV30, b: MemoryOperationObservationV30)
 requires typed_source_observations_valid_v49(seq![a]), typed_source_observations_valid_v49(seq![b]),
 typed_observation_0_v48(a).is_some(), typed_observation_0_v48(a) == typed_observation_2_v48(b), typed_observation_0_v48(a).unwrap().len() == 1,
 ensures invocation_source_observation_carriers_equal_{root}_v49(a, typed_source_mapped_observation_v49(b)),
{{ typed_original_map_view_exact_2_v48(b.before); typed_original_map_view_exact_2_v48(b.after);
 typed_original_allocation_registers_0_v48(a.before, typed_original_map_view_2_v48(b.before));
 typed_original_allocation_registers_0_v48(a.after, typed_original_map_view_2_v48(b.after)); }}
proof fn typed_source_observation_transport_{root}_v49(a: Seq<MemoryOperationObservationV30>, b: Seq<MemoryOperationObservationV30>)
 requires typed_source_observations_valid_v49(a), typed_source_observations_valid_v49(b),
 typed_observations_0_v48(a).is_some(), typed_observations_0_v48(a) == typed_observations_2_v48(b),
 ensures invocation_actual_observations_v39(a).len() == typed_source_mapped_observations_v49(b).len(),
 forall|i: int| 0 <= i < invocation_actual_observations_v39(a).len() ==> invocation_source_observation_carriers_equal_{root}_v49(invocation_actual_observations_v39(a)[i], typed_source_mapped_observations_v49(b)[i]),
 decreases a.len() + b.len(),
{{ if a.len() > 0 && typed_observation_0_v48(a[0]) == Some(seq![]) {{
 assert(invocation_project_effect_v39(a[0].effect).is_none());
 typed_source_observation_transport_{root}_v49(a.drop_first(), b);
}} else if b.len() > 0 && typed_observation_2_v48(b[0]) == Some(seq![]) {{
 assert(invocation_project_effect_v39(b[0].effect).is_none());
 typed_source_observation_transport_{root}_v49(a, b.drop_first());
}} else if a.len() > 0 || b.len() > 0 {{
 assert(a.len() > 0 && b.len() > 0);
 assert(typed_observation_0_v48(a[0]) == typed_observation_2_v48(b[0]));
 typed_source_observation_pair_{root}_v49(a[0], b[0]);
 typed_source_observation_transport_{root}_v49(a.drop_first(), b.drop_first());
}} }}
"#
        );
        self.check(out)
    }
}
