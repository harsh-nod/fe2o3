//! Typed execution cuts at checked original block segments. A merged target
//! block is advanced only to the next source cut, not executed prematurely.
use super::*;

#[path = "mixed_optimizer_typed_forwarding_segments_v49.rs"]
mod forwarding;
#[path = "mixed_optimizer_typed_source_boundaries_v49.rs"]
mod source_boundaries;
use fe2o3_kernel_analysis::CanonicalKirPrivateByteAnalysisV38 as Physical;
use fe2o3_kernel_ir::FormalIndexWidth;

#[path = "mixed_optimizer_typed_prefix_licm_v49.rs"]
mod composition;
#[path = "mixed_optimizer_typed_prefix_models_v49.rs"]
mod models;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SegmentV49 {
    pub(super) output_block: usize,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) next_original: Option<usize>,
}

pub(in crate::mixed_optimizer_refinement_v26::semantics) struct PrefixSegmentsV49<
    'b,
    'a,
    'owner,
    'rows,
    R,
> {
    bridge: &'b AllocationBridgeV48<'a, 'owner, 'rows, R>,
    segments: Vec<Option<SegmentV49>>,
    heads: Vec<usize>,
    anchors: Vec<usize>,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("typed prefix segment differs from checked original occurrence chain")
}

impl<'b, 'a, 'owner, 'rows, R: ByteAllocationResolverV30>
    PrefixSegmentsV49<'b, 'a, 'owner, 'rows, R>
{
    pub(super) fn derive(
        bridge: &'b AllocationBridgeV48<'a, 'owner, 'rows, R>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        bridge.check(out)?;
        let result = Self::derive_inner(bridge, out);
        if let Err(Error::Resource(error)) = &result {
            bridge.failure.set(Some(*error));
        }
        result
    }

    fn derive_inner(
        bridge: &'b AllocationBridgeV48<'a, 'owner, 'rows, R>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let prefix = bridge.prefix;
        let input = prefix.input();
        let output = prefix.output();
        let rows = prefix.rows();
        out.budget.reserve_storage(
            size_of::<Self>()
                + size_of::<Result<Self>>()
                + size_of::<([usize; 12], [Option<SegmentV49>; 2], [&(); 8])>(),
        )?;
        out.budget.reserve_storage(
            input
                .blocks()
                .len()
                .checked_mul(size_of::<Option<SegmentV49>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(input.blocks().len())
            .map_err(|_| Resource::Allocation)?;
        out.budget.reserve_storage(
            segments
                .capacity()
                .checked_sub(input.blocks().len())
                .ok_or(Resource::Accounting)?
                .checked_mul(size_of::<Option<SegmentV49>>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        out.budget.charge_work(input.blocks().len())?;
        segments.resize(input.blocks().len(), None);
        let mut heads = allocate(output.blocks().len(), out)?;
        let mut anchors = allocate(output.definitions().len(), out)?;
        let scratch = out.budget.storage();
        let built = (|| {
            let mut positions = allocate(input.blocks().len(), out)?;
            if rows.blocks.len() != output.blocks().len()
                || rows.operations.len() != output.operations().len()
                || rows.functions.len() != output.functions().len()
            {
                return Err(mismatch());
            }
            let mut next_segment = 0usize;
            for (output_block, block_row) in rows.blocks.iter().enumerate() {
                out.budget.charge_work(8)?;
                let actual = &output.blocks()[output_block];
                let begin = block_row.segments.start as usize;
                let end = begin
                    .checked_add(block_row.segments.len as usize)
                    .ok_or(Resource::Arithmetic)?;
                if block_row.output != actual.coordinate
                    || begin != next_segment
                    || begin == end
                    || end > rows.segments.len()
                {
                    return Err(mismatch());
                }
                next_segment = end;
                let function = rows
                    .functions
                    .get(actual.coordinate.function.0 as usize)
                    .ok_or_else(mismatch)?;
                if function.output != actual.coordinate.function {
                    return Err(mismatch());
                }
                for position in begin..end {
                    out.budget.charge_work(9)?;
                    let row = &rows.segments[position];
                    let original = block_index(input, row.input)?;
                    if row.input.function != function.input || positions[original] != NONE {
                        return Err(mismatch());
                    }
                    let next_original = if position + 1 < end {
                        let next = &rows.segments[position + 1];
                        let connector = row.connector.ok_or_else(mismatch)?;
                        let original_block = &input.blocks()[original];
                        let mut found = false;
                        for edge in original_block.edges.clone() {
                            out.budget.charge_work(3)?;
                            let edge = &input.edges()[edge];
                            if edge.coordinate == connector {
                                if found || edge.target != next.input {
                                    return Err(mismatch());
                                }
                                found = true;
                            }
                        }
                        if !found {
                            return Err(mismatch());
                        }
                        Some(block_index(input, next.input)?)
                    } else {
                        if row.connector.is_some() {
                            return Err(mismatch());
                        }
                        None
                    };
                    positions[original] = position;
                    segments[original] = Some(SegmentV49 {
                        output_block,
                        start: actual.operations.start,
                        end: actual.operations.start,
                        next_original,
                    });
                    if position == begin {
                        heads[output_block] = original;
                    }
                }

                // Synthesized constants stay in the current segment. Retained
                // operations move the cut forward by their checked source block.
                let mut position = begin;
                for operation in actual.operations.clone() {
                    out.budget.charge_work(8)?;
                    let actual_operation = &output.operations()[operation];
                    let origin = &rows.operations[operation];
                    if origin.output != actual_operation.coordinate {
                        return Err(mismatch());
                    }
                    let next = match origin.origin {
                        Origin::Retained(original) => {
                            operation_index(input, original)?;
                            positions[block_index(input, original.block)?]
                        }
                        Origin::ConstantFrom(original) => {
                            let original = &input.definitions()[definition_index(input, original)?];
                            let original_function = match original.coordinate {
                                Definition::FunctionArgument { function, .. } => function,
                                Definition::BlockArgument { block, .. } => block.function,
                                Definition::Result { operation, .. } => operation.block.function,
                            };
                            if original_function != function.input
                                || !matches!(
                                    actual_operation.operation.kind,
                                    OperationKind::Constant(_)
                                )
                                || actual_operation.results.len() != 1
                                || output.definitions()[actual_operation.results.start].ty
                                    != original.ty
                            {
                                return Err(mismatch());
                            }
                            position
                        }
                    };
                    if next < position || next >= end {
                        return Err(mismatch());
                    }
                    while position < next {
                        out.budget.charge_work(3)?;
                        let original = block_index(input, rows.segments[position].input)?;
                        segments[original].as_mut().ok_or_else(mismatch)?.end = operation;
                        position += 1;
                        let original = block_index(input, rows.segments[position].input)?;
                        segments[original].as_mut().ok_or_else(mismatch)?.start = operation;
                    }
                }
                while position < end {
                    out.budget.charge_work(3)?;
                    let original = block_index(input, rows.segments[position].input)?;
                    segments[original].as_mut().ok_or_else(mismatch)?.end = actual.operations.end;
                    position += 1;
                    if position < end {
                        let original = block_index(input, rows.segments[position].input)?;
                        segments[original].as_mut().ok_or_else(mismatch)?.start =
                            actual.operations.end;
                    }
                }
            }
            if next_segment != rows.segments.len() {
                return Err(mismatch());
            }
            for (original, definition) in input.definitions().iter().enumerate() {
                for descendant in descendants(rows, original)? {
                    out.budget.charge_work(4)?;
                    let target = definition_index(output, descendant.output)?;
                    if output.definitions()[target].ty != definition.ty {
                        return Err(mismatch());
                    }
                    if descendant.kind == Descendant::Retained {
                        if anchors[target] != NONE {
                            return Err(mismatch());
                        }
                        anchors[target] = original;
                    }
                }
            }
            for (target, anchor) in anchors.iter().enumerate() {
                out.budget.charge_work(3)?;
                if *anchor != NONE {
                    continue;
                }
                let Definition::Result {
                    operation,
                    result: 0,
                } = output.definitions()[target].coordinate
                else {
                    return Err(mismatch());
                };
                let operation = operation_index(output, operation)?;
                if !matches!(rows.operations[operation].origin, Origin::ConstantFrom(_)) {
                    return Err(mismatch());
                }
            }
            drop(positions);
            Ok(())
        })();
        out.budget.release_storage(
            out.budget
                .storage()
                .checked_sub(scratch)
                .ok_or(Resource::Accounting)?,
        )?;
        built?;
        bridge.check(out)?;
        Ok(Self {
            bridge,
            segments,
            heads,
            anchors,
            required: out.budget.storage(),
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        if let Some(error) = self.bridge.failure.get() {
            return Err(error.into());
        }
        if out.budget.storage() < self.required {
            self.bridge.failure.set(Some(Resource::Accounting));
            return Err(Resource::Accounting.into());
        }
        self.bridge.check(out)
    }

    pub(super) fn segment(
        &self,
        original: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<Option<SegmentV49>> {
        self.check(out)?;
        self.bridge.charge(1, out)?;
        self.segments.get(original).copied().ok_or_else(mismatch)
    }

    fn emit_cursors(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.check(out)?;
        emit!(
            out,
            "struct TypedPrefixCursorV49 {{ micro: MemoryMicroStateV30, segment: int }}\nstruct TypedPrefixStepV49 {{ cursor: TypedPrefixCursorV49, observations: Seq<MemoryOperationObservationV30>, returned: Seq<MemoryValueV30> }}\n"
        );
        emit!(
            out,
            "open spec fn typed_prefix_refuse_v49(cursor: TypedPrefixCursorV49) -> TypedPrefixStepV49 {{ TypedPrefixStepV49 {{ cursor: TypedPrefixCursorV49 {{ micro: MemoryMicroStateV30 {{ state: MemoryStateV30 {{ valid: false, ..cursor.micro.state }}, ..cursor.micro }}, ..cursor }}, observations: seq![], returned: seq![] }} }}\n"
        );
        self.emit_stage_cursors(AllocationSideV48::Prefix, &self.segments, out)
    }

    fn emit_stage_cursors(
        &self,
        side: AllocationSideV48,
        segments: &[Option<SegmentV49>],
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        let (stage, output, base) = match side {
            AllocationSideV48::Prefix => (
                "prefix",
                self.bridge.prefix.output(),
                self.bridge.prefix.input().functions().len(),
            ),
            AllocationSideV48::Relocated => (
                "relocated",
                self.bridge.output,
                self.bridge
                    .prefix
                    .input()
                    .functions()
                    .len()
                    .checked_add(self.bridge.prefix.output().functions().len())
                    .ok_or(Resource::Arithmetic)?,
            ),
            AllocationSideV48::Original => return Err(mismatch()),
        };
        self.emit_named_stage_cursors(stage, output, segments, base, 1, out)
    }

    pub(super) fn emit_named_stage_cursors(
        &self,
        stage: &str,
        output: &Inventory<'_>,
        segments: &[Option<SegmentV49>],
        base: usize,
        stride: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.check(out)?;
        emit!(
            out,
            "open spec fn typed_{stage}_head_v49(block: int) -> Option<int> {{\n"
        );
        for (block, head) in self.heads.iter().enumerate() {
            self.bridge.charge(1, out)?;
            emit!(out, " if block == {block} {{ Some({head}) }} else\n");
        }
        emit!(out, " {{ None }}\n}}\n");
        for (function, row) in output.functions().iter().enumerate() {
            self.bridge.charge(1, out)?;
            if row.function.body.is_none() {
                continue;
            }
            let namespace = function
                .checked_mul(stride)
                .and_then(|offset| base.checked_add(offset))
                .ok_or(Resource::Arithmetic)?;
            emit!(
                out,
                "open spec fn typed_{stage}_observation_{function}_v49(block: int, index: int, observation: MemoryOperationObservationV30, little_endian: bool) -> bool {{\n observation.before.valid && observation.after.valid && observation.before.pc == block && (\n"
            );
            for block in row.blocks.clone() {
                for (index, operation) in output.blocks()[block].operations.clone().enumerate() {
                    self.bridge.charge(2, out)?;
                    emit!(
                        out,
                        " if block == {block} && index == {index} {{ byte_operation_{namespace}_{operation}_v30(observation.before, little_endian).observation == observation }} else\n"
                    );
                }
            }
            emit!(out, " {{ false }})\n}}\n");
            emit!(
                out,
                "open spec fn typed_{stage}_begin_{function}_v49(state: MemoryStateV30) -> TypedPrefixCursorV49 {{\n if !state.valid || state.pc < 0 {{ TypedPrefixCursorV49 {{ micro: MemoryMicroStateV30 {{ state, observations: seq![], next_operation: -1 }}, segment: state.pc }} }} else {{ match typed_{stage}_head_v49(state.pc) {{ Some(segment) => TypedPrefixCursorV49 {{ micro: byte_micro_begin_{namespace}_v30(state), segment }}, None => TypedPrefixCursorV49 {{ micro: MemoryMicroStateV30 {{ state: MemoryStateV30 {{ valid: false, ..state }}, observations: seq![], next_operation: -1 }}, segment: -3 }} }} }}\n}}\n"
            );
            self.emit_cursor_valid(stage, output, segments, function, out)?;
            emit!(
                out,
                "open spec fn typed_{stage}_step_{function}_v49(cursor: TypedPrefixCursorV49, little_endian: bool) -> TypedPrefixStepV49 {{\n if !cursor.micro.state.valid || cursor.micro.state.pc < 0 {{ TypedPrefixStepV49 {{ cursor, observations: seq![], returned: seq![] }} }} else if !typed_{stage}_cursor_valid_{function}_v49(cursor, little_endian) {{ typed_prefix_refuse_v49(cursor) }} else {{\n"
            );
            for (original, segment) in segments.iter().enumerate() {
                let Some(segment) = segment else {
                    continue;
                };
                if !row.blocks.contains(&segment.output_block) {
                    continue;
                }
                self.bridge.charge(3, out)?;
                emit!(
                    out,
                    " if cursor.segment == {original} {{\n let initial_count = cursor.micro.observations.len();\n let m = cursor.micro;\n"
                );
                for _ in segment.start..segment.end {
                    self.bridge.charge(1, out)?;
                    emit!(
                        out,
                        " let m = byte_micro_step_{namespace}_v30(m, little_endian).next;\n"
                    );
                }
                match segment.next_original {
                    Some(next) => emit!(
                        out,
                        " TypedPrefixStepV49 {{ cursor: TypedPrefixCursorV49 {{ micro: m, segment: {next} }}, observations: m.observations.subrange(initial_count as int, m.observations.len() as int), returned: seq![] }}\n"
                    ),
                    None => emit!(
                        out,
                        " let result = byte_micro_finish_{namespace}_v30(m);\n TypedPrefixStepV49 {{ cursor: typed_{stage}_begin_{function}_v49(result.state), observations: result.observations.subrange(initial_count as int, result.observations.len() as int), returned: result.returned }}\n"
                    ),
                }
                emit!(out, " }} else\n");
            }
            emit!(out, " {{ typed_prefix_refuse_v49(cursor) }} }}\n}}\n");
        }
        self.check(out)
    }

    fn emit_cursor_valid(
        &self,
        stage: &str,
        output: &Inventory<'_>,
        segments: &[Option<SegmentV49>],
        function: usize,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        let row = &output.functions()[function];
        emit!(
            out,
            "open spec fn typed_{stage}_cursor_valid_{function}_v49(cursor: TypedPrefixCursorV49, little_endian: bool) -> bool {{\n let m = cursor.micro;\n m.state.valid && m.state.values.len() == {} && byte_state_memory_well_formed_v30(m.state) && (if m.state.pc < 0 {{ (m.state.pc == -1 || m.state.pc == -2) && cursor.segment == m.state.pc && m.next_operation == -1 && m.observations.len() == 0 }} else {{\n",
            output.definitions().len()
        );
        for (original, segment) in segments.iter().enumerate() {
            let Some(segment) = segment else {
                continue;
            };
            if !row.blocks.contains(&segment.output_block) {
                continue;
            }
            self.bridge.charge(4, out)?;
            let block = &output.blocks()[segment.output_block];
            let count = segment
                .start
                .checked_sub(block.operations.start)
                .ok_or(Resource::Arithmetic)?;
            emit!(
                out,
                " if cursor.segment == {original} {{ m.state.pc == {} && m.next_operation == ",
                segment.output_block
            );
            if segment.start == block.operations.end {
                emit!(out, "-1");
            } else {
                emit!(out, "{}", segment.start);
            }
            emit!(
                out,
                " && m.observations.len() == {count}\n && (forall|i: int| 0 <= i < m.observations.len() ==> typed_{stage}_observation_{function}_v49(m.state.pc, i, m.observations[i], little_endian) && (i == 0 || m.observations[i - 1].after == m.observations[i].before))"
            );
            if count > 0 {
                emit!(out, "\n && m.observations[{}].after == m.state", count - 1);
            }
            emit!(out, " }} else\n");
        }
        emit!(out, " {{ false }} }})\n}}\n");
        self.check(out)
    }
}
