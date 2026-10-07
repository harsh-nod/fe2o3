//! Retained, one-operation Checked source/target segments. Coverage is partial;
//! a locator is not initialization, a complete frame relation, or authority.
use super::super::{
    paired::ExpandedScalarBindingsV196,
    source_frame_plan::{FramePlan, prefix::Prefix},
    tile_target::TileTargetV176,
};
use super::*;
use crate::mixed_optimizer_refinement_v26::semantics::{
    block_index, byte_function_v30::CheckedByteOperationV48, operation_index,
};
use fe2o3_kernel_ir::{
    CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as Descendant,
    CanonicalKirOperationCoordinateV1 as Operation, CheckedBinaryOperator, FormalIndexWidth,
};
use fe2o3_lower_mir_kernel::{
    ProductionOptimizedSourceOperationV18 as NeutralOperation,
    ProductionSourceOperationV18 as SourceOperation, ProductionSourceSsaCarrierShapeV37 as Carrier,
};
use fe2o3_mir_model::semantic_mir_v1::SemanticBlockIdV1 as SourceBlock;

#[path = "original_semantic_mir_checked_target_segment_emit_v298.rs"]
mod emission;

#[cfg(test)]
#[path = "original_semantic_mir_checked_target_segment_v298_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Unsupported {
    SourceSpan,
    Removed,
    Rewritten,
    ExpandedSpan,
    Effectful,
    UnavailableSsa,
    UnretainedValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum NeutralSelection {
    Retained(Operation),
    Unsupported(Unsupported),
}

enum Selection<'a, 'slots, 'view, 'source> {
    Ready(CheckedTargetSegment<'a, 'slots, 'view, 'source>),
    Unsupported(Unsupported),
}

struct CheckedTargetSegment<'a, 'slots, 'view, 'source> {
    program: &'a SourceByteProgram<'slots, 'view, 'source>,
    target: &'a TileTargetV176<'slots, 'view, 'source>,
    root: usize,
    instance: usize,
    block: usize,
    statement: usize,
    source_pc: usize,
    destination: usize,
    source_operands: [usize; 2],
    target_block: usize,
    target_operation: usize,
    target_prefix: usize,
    target_next: Option<usize>,
    checked: CheckedByteOperationV48,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement(
        "checked target segment differs from its exact source statement or target owner",
    )
}

fn headers() -> usize {
    2 * size_of::<CheckedTargetSegment<'_, '_, '_, '_>>()
        + 2 * size_of::<Selection<'_, '_, '_, '_>>()
        + 2 * size_of::<Result<Selection<'_, '_, '_, '_>>>()
        + size_of::<FramePlan<'_, '_, '_, '_>>()
        + size_of::<Result<FramePlan<'_, '_, '_, '_>>>()
        + 2 * size_of::<Prefix<'_, '_, '_, '_, '_>>()
        + 2 * size_of::<Result<Prefix<'_, '_, '_, '_, '_>>>()
        + size_of::<ExpandedScalarBindingsV196<'_, '_, '_, '_>>()
        + size_of::<Result<ExpandedScalarBindingsV196<'_, '_, '_, '_>>>()
        + 4 * size_of::<CheckedByteOperationV48>()
        + 4 * size_of::<Result<CheckedByteOperationV48>>()
        + 4 * size_of::<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>()
        + 4 * size_of::<Result<fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>>>()
        + 4 * size_of::<
            std::result::Result<
                fe2o3_lower_mir_kernel::ProductionSourceSsaEndpointV36<'_, '_>,
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18,
            >,
        >()
        + 4 * size_of::<
            std::result::Result<
                Option<usize>,
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18,
            >,
        >()
        + 2 * size_of::<
            std::result::Result<
                Option<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>,
                fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18,
            >,
        >()
        + 2 * size_of::<
            std::result::Result<(), fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18>,
        >()
        + 2 * size_of::<SourceOperation>()
        + 2 * size_of::<Option<Operation>>()
        + 2 * size_of::<NeutralOperation>()
        + 2 * size_of::<NeutralSelection>()
        + 2 * size_of::<Result<NeutralSelection>>()
        + 2 * size_of::<Result<Option<Operation>>>()
        + 2 * size_of::<fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159>()
        + 2 * size_of::<Definition>()
        + 2 * size_of::<Carrier>()
        + 4 * size_of::<Event>()
        + 4 * size_of::<Result<Event>>()
        + 4 * size_of::<Range<usize>>()
        + 4 * size_of::<Option<usize>>()
        + 2 * size_of::<Option<(usize, usize, usize)>>()
        + 4 * size_of::<Result<()>>()
        + 2 * size_of::<std::ops::Range<usize>>()
        + 2 * size_of::<std::array::IntoIter<usize, 4>>()
        + 2 * size_of::<
            std::iter::Enumerate<std::array::IntoIter<(fe2o3_mir_model::SsaValueV1, usize), 2>>,
        >()
        + 64 * size_of::<usize>()
        + 32 * size_of::<&()>()
}

fn is_u32_add(checked: CheckedByteOperationV48) -> bool {
    checked.bits == 32 && !checked.signed && checked.operator == CheckedBinaryOperator::Add
}

fn neutral_selection(original: Operation, row: NeutralOperation) -> Result<NeutralSelection> {
    match row {
        NeutralOperation::RemovedUnreachable { input } if input == original => {
            Ok(NeutralSelection::Unsupported(Unsupported::Removed))
        }
        NeutralOperation::Rewritten { input } if input == original => {
            Ok(NeutralSelection::Unsupported(Unsupported::Rewritten))
        }
        NeutralOperation::Retained { input, output } if input == original => {
            Ok(NeutralSelection::Retained(output))
        }
        _ => Err(mismatch()),
    }
}

fn singleton_span(
    original: Operation,
    neutral: Operation,
    span: fe2o3_lower_mir_kernel::ProductionSourceTileOperationSpanV159,
) -> Result<Option<Operation>> {
    if span.original != original || span.expansion.input != neutral {
        return Err(mismatch());
    }
    let count = span
        .expansion
        .end
        .checked_sub(span.expansion.first)
        .ok_or_else(mismatch)?;
    Ok((count == 1).then_some(Operation {
        block: span.expansion.input.block,
        operation: span.expansion.first,
    }))
}

/// Preclassify unsupported transformations before querying the strict retained
/// locator. Never recover a latched source/resource error as optional coverage.
fn retained(
    slots: &SourceSlots<'_, '_>,
    definition: usize,
    out: &mut Writer<'_, '_>,
) -> Result<bool> {
    out.budget.charge_work(10)?;
    let input = slots.correspondence(out)?.inventory(out.budget)?;
    let coordinate = input
        .definitions()
        .get(definition)
        .ok_or_else(mismatch)?
        .coordinate;
    let tile = slots.tile_owner_v176(out)?;
    let neutral = tile.neutral_source_v162(out.budget)?;
    let descendants = neutral.definition_descendants(coordinate, out.budget)?;
    let [row] = descendants else { return Ok(false) };
    if row.kind != Descendant::Retained {
        return Ok(false);
    }
    if let Definition::Result { operation, .. } = coordinate {
        let NeutralSelection::Retained(output) =
            neutral_selection(operation, neutral.operation(operation, out.budget)?)?
        else {
            return Ok(false);
        };
        let span = tile
            .operation_span(operation, out.budget)?
            .ok_or_else(mismatch)?;
        if singleton_span(operation, output, span)?.is_none() {
            return Ok(false);
        }
    }
    Ok(true)
}

impl<'a, 'slots, 'view, 'source> CheckedTargetSegment<'a, 'slots, 'view, 'source> {
    #[allow(clippy::too_many_arguments)]
    fn derive(
        program: &'a SourceByteProgram<'slots, 'view, 'source>,
        target: &'a TileTargetV176<'slots, 'view, 'source>,
        plan: &InvocationPlan<'view, 'source>,
        frames: &FramePlan<'_, 'slots, 'view, 'source>,
        scalar: &ExpandedScalarBindingsV196<'_, 'slots, 'view, 'source>,
        before: &Prefix<'_, '_, 'slots, 'view, 'source>,
        after: &Prefix<'_, '_, 'slots, 'view, 'source>,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<Selection<'a, 'slots, 'view, 'source>> {
        frames.check(plan, program.source_slots(out)?, out)?;
        before.check(frames, out)?;
        after.check(frames, out)?;
        scalar.check_owner(program.slots, target, out)?;
        // The query is independently metered even when used outside the scan.
        out.budget.reserve_storage(headers())?;
        out.budget.charge_work(28)?;
        if !std::ptr::eq(program.slots, target.source_slots(out)?)
            || before.frame != after.frame
            || before.block != after.block
            || before.pc != after.pc
            || before.statement.checked_add(1) != Some(after.statement)
        {
            return Err(mismatch());
        }
        let frame = frames.frames.get(before.frame).ok_or_else(mismatch)?;
        let (root, instance) = (frame.root, frame.instance);
        let range = &program.roots.get(root).ok_or_else(mismatch)?.0;
        if instance >= range.len() {
            return Err(mismatch());
        }
        let function = program
            .functions
            .get(range.start + instance)
            .and_then(Option::as_ref)
            .ok_or_else(mismatch)?;
        if !frame.active
            || function.root != root
            || function.instance != instance
            || function.checked_micro_pc_v293(
                root,
                instance,
                before.block,
                before.statement,
                out,
            )? != Some(before.pc)
        {
            return Err(mismatch());
        }
        function
            .body
            .check_prefix_owner_v296(plan, root, instance, out)?;
        let event = function
            .body
            .event_at(before.block, before.statement, out)?;
        let (destination, left, right) = event.checked_prefix_site_v296().ok_or_else(mismatch)?;
        if !frame.locals.contains(&destination)
            || !frame.locals.contains(&left)
            || !frame.locals.contains(&right)
        {
            return Err(mismatch());
        }
        let relation = program.slots.correspondence(out)?;
        let mut count = 0usize;
        let mut original = None;
        let visit = |row, budget: &mut fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1<'_>| {
            budget.charge_work(2)?;
            count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
            if let SourceOperation::Operation(operation) = row { original = Some(operation); }
            Ok(())
        };
        out.budget
            .reserve_storage(2 * std::mem::size_of_val(&visit) + std::mem::align_of_val(&visit))?;
        relation.visit_source_operations(
            root,
            instance,
            SourceBlock::from_index(u32::try_from(before.block).map_err(|_| Resource::Arithmetic)?),
            Some(u32::try_from(before.statement).map_err(|_| Resource::Arithmetic)?),
            out.budget,
            visit,
        )?;
        let Some(original) = original.filter(|_| count == 1) else {
            return Ok(Selection::Unsupported(Unsupported::SourceSpan));
        };
        let tile = program.slots.tile_owner_v176(out)?;
        let neutral_output = match neutral_selection(
            original,
            tile.neutral_source_v162(out.budget)?
                .operation(original, out.budget)?,
        )? {
            NeutralSelection::Unsupported(reason) => return Ok(Selection::Unsupported(reason)),
            NeutralSelection::Retained(output) => output,
        };
        let span = tile
            .operation_span(original, out.budget)?
            .ok_or_else(mismatch)?;
        let Some(coordinate) = singleton_span(original, neutral_output, span)? else {
            return Ok(Selection::Unsupported(Unsupported::ExpandedSpan));
        };
        let input = relation.inventory(out.budget)?;
        let input_operation = operation_index(input, original)?;
        if !input.operations()[input_operation].effects.is_empty() {
            return Ok(Selection::Unsupported(Unsupported::Effectful));
        }
        let original_checked = CheckedByteOperationV48::derive(input, input_operation, width, out)?;
        if !is_u32_add(original_checked) {
            return Err(mismatch());
        }
        let before_left = before
            .locals
            .get(left - frame.locals.start)
            .ok_or_else(mismatch)?;
        let before_right = before
            .locals
            .get(right - frame.locals.start)
            .ok_or_else(mismatch)?;
        let after_result = after
            .locals
            .get(destination - frame.locals.start)
            .ok_or_else(mismatch)?;
        let (Some(left_ssa), Some(right_ssa), Some(result_ssa)) = (
            before_left.current,
            before_right.current,
            after_result.current,
        ) else {
            return Ok(Selection::Unsupported(Unsupported::UnavailableSsa));
        };
        let mut source_operands = [0; 2];
        for (ordinal, (value, local)) in [(left_ssa, left), (right_ssa, right)]
            .into_iter()
            .enumerate()
        {
            out.budget.charge_work(4)?;
            let endpoint = relation.ssa_typed_endpoint_v36(root, instance, value, out.budget)?;
            if endpoint.source_function(out.budget)? != frame.function
                || endpoint.source_local(out.budget)?.index() as usize != local - frame.locals.start
                || endpoint.carrier_shape(out.budget)? != Carrier::Value
            {
                return Err(mismatch());
            }
            source_operands[ordinal] = endpoint
                .original_definition(out.budget)?
                .ok_or_else(mismatch)?;
        }
        let endpoint = relation.ssa_typed_endpoint_v36(root, instance, result_ssa, out.budget)?;
        if endpoint.source_function(out.budget)? != frame.function
            || endpoint.source_local(out.budget)?.index() as usize
                != destination - frame.locals.start
            || endpoint.source_type(out.budget)? != after_result.ty
            || endpoint.carrier_shape(out.budget)? != (Carrier::Aggregate { components: 2 })
        {
            return Err(mismatch());
        }
        let mut source_results = [0; 2];
        for ordinal in 0..2 {
            out.budget.charge_work(2)?;
            source_results[ordinal] = endpoint
                .component(ordinal, out.budget)?
                .original_definition(out.budget)?
                .ok_or_else(mismatch)?;
        }
        if original_checked.operands != source_operands
            || original_checked.results != source_results
        {
            return Err(mismatch());
        }
        for definition in [
            source_operands[0],
            source_operands[1],
            source_results[0],
            source_results[1],
        ] {
            if !retained(program.slots, definition, out)? {
                return Ok(Selection::Unsupported(Unsupported::UnretainedValue));
            }
        }
        let actual = target.inventory(out)?;
        out.budget.charge_work(16)?;
        if coordinate.block.function != target.root_function(root, out)? {
            return Err(mismatch());
        }
        let target_block = block_index(actual, coordinate.block)?;
        let target_operation = operation_index(actual, coordinate)?;
        if !actual.operations()[target_operation].effects.is_empty() {
            return Ok(Selection::Unsupported(Unsupported::Effectful));
        }
        let checked = CheckedByteOperationV48::derive(actual, target_operation, width, out)?;
        let expected_operands = [
            scalar.definition(source_operands[0], out)?,
            scalar.definition(source_operands[1], out)?,
        ];
        let expected_results = [
            scalar.definition(source_results[0], out)?,
            scalar.definition(source_results[1], out)?,
        ];
        if !is_u32_add(checked)
            || checked.operands != expected_operands
            || checked.results != expected_results
        {
            return Err(mismatch());
        }
        let target_row = actual.blocks().get(target_block).ok_or_else(mismatch)?;
        let target_prefix = target_operation
            .checked_sub(target_row.operations.start)
            .ok_or_else(mismatch)?;
        let next = target_operation
            .checked_add(1)
            .ok_or(Resource::Arithmetic)?;
        let target_next = (next < target_row.operations.end).then_some(next);
        Ok(Selection::Ready(Self {
            program,
            target,
            root,
            instance,
            block: before.block,
            statement: before.statement,
            source_pc: before.pc,
            destination,
            source_operands: [left, right],
            target_block,
            target_operation,
            target_prefix,
            target_next,
            checked,
            required: out.budget.storage(),
        }))
    }

    fn check(
        &self,
        program: &SourceByteProgram<'_, '_, '_>,
        target: &TileTargetV176<'_, '_, '_>,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        self.program
            .slots
            .check_query_storage_floor(self.required, out.budget)?;
        self.program.source_slots(out)?;
        self.target.inventory(out)?;
        out.budget.charge_work(3)?;
        if !std::ptr::eq(self.program, program)
            || !std::ptr::eq(self.target, target)
            || !std::ptr::eq(self.program.slots, self.target.source_slots(out)?)
        {
            return Err(mismatch());
        }
        Ok(())
    }
}

impl<'slots, 'view, 'source> SourceByteProgram<'slots, 'view, 'source> {
    pub(in super::super) fn emit_checked_target_segments_v298(
        &self,
        plan: &InvocationPlan<'view, 'source>,
        target: &TileTargetV176<'slots, 'view, 'source>,
        width: FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<usize> {
        let emit = |out: &mut Writer<'_, '_>| {
            self.source_slots(out)?;
            out.budget.reserve_storage(headers())?;
            let frames = FramePlan::derive(plan, self.slots, out)?;
            let scalar = ExpandedScalarBindingsV196::derive(self.slots, target, out)?;
            let mut count = 0usize;
            for root in 0..self.roots.len() {
                out.budget.charge_work(1)?;
                let range = &self.roots[root].0;
                for instance in 0..range.len() {
                    out.budget.charge_work(1)?;
                    let Some(function) = &self.functions[range.start + instance] else {
                        continue;
                    };
                    for block in 0..function.control.len() {
                        out.budget.charge_work(1)?;
                        for statement in 0..function.control[block].statements {
                            out.budget.charge_work(3)?;
                            if function
                                .checked_micro_pc_v293(root, instance, block, statement, out)?
                                .is_none()
                                || function
                                    .body
                                    .event_at(block, statement, out)?
                                    .checked_prefix_site_v296()
                                    .is_none()
                            {
                                continue;
                            }
                            let before = frames
                                .partial_prefix_v296(root, instance, block, statement, out)?;
                            let after = frames.partial_prefix_v296(
                                root,
                                instance,
                                block,
                                statement.checked_add(1).ok_or(Resource::Arithmetic)?,
                                out,
                            )?;
                            let selection = CheckedTargetSegment::derive(
                                self, target, plan, &frames, &scalar, &before, &after, width, out,
                            )?;
                            match selection {
                                Selection::Ready(segment) => {
                                    segment.emit(out)?;
                                    count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
                                }
                                Selection::Unsupported(reason) => {
                                    writeln!(out, "// Checked target segment {root}/{instance}/{block}/{statement} unsupported: {reason:?}; no relational claim.")
                                        .map_err(|_| out.error())?;
                                }
                            }
                            after.discard(out)?;
                            before.discard(out)?;
                        }
                    }
                }
            }
            frames.check(plan, self.slots, out)?;
            scalar.check_owner(self.slots, target, out)?;
            Ok(count)
        };
        out.budget
            .reserve_storage(2 * std::mem::size_of_val(&emit) + std::mem::align_of_val(&emit))?;
        self.slots.with_source_query_v42(out, emit)
    }
}
