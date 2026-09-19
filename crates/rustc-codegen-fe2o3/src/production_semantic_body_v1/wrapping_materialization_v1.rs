//! Consuming constructor for authenticated, total safe-core wrapping calls.

use super::*;
use crate::production_safe_core_shift_v1::{DirectionV1, SafeCoreShiftV1, locals};
use crate::rustc_semantic_adapter_v1::{
    borrowed_rustc_mir_body_sha256_v1, canonical_function_identities_v1, rustc_block_identity_v1,
    rustc_local_identity_v1,
};

#[derive(Clone, Copy)]
pub(super) struct LocalV1 {
    pub(super) local: ReceiverLocalV1,
    pub(super) ty: SemanticTypeIdV1,
    pub(super) source: SemanticSourceProvenanceV1,
}

pub(super) struct WrappingMaterializationV1 {
    local_count: usize,
    locals: Vec<Option<LocalV1>>,
    calls: Vec<Option<[SemanticLocalIdV1; 2]>>,
}

fn local_error(
    error: locals::Error<ProductionSemanticBodyErrorV1>,
) -> ProductionSemanticBodyErrorV1 {
    match error {
        locals::Error::Resource(error) => error,
        locals::Error::Allocation => allocation(SemanticMirResourceV1::Locals),
        locals::Error::Invalid => table("safe core shift local order"),
    }
}

impl WrappingMaterializationV1 {
    pub(super) fn local(&self, index: usize) -> Option<LocalV1> {
        self.locals.get(index).copied().flatten()
    }

    pub(super) fn local_count(&self) -> usize {
        self.local_count
    }

    pub(super) fn derive<'tcx>(
        input: &ProductionSemanticBodyInputV1<'_, 'tcx>,
        blocks: &[&ProductionSemanticBlockBindingV1],
        calls: &CallTablesV1<'_, 'tcx>,
        types: &HashMap<Ty<'tcx>, SemanticTypeIdV1>,
        receiver: &mut Option<ReceiverMaterializationV1<'tcx>>,
        owner: &mut ProductionSemanticBodyRequestOwnerV1<'tcx>,
    ) -> Result<Option<Self>, ProductionSemanticBodyErrorV1> {
        let count = locals::prepaid_rows_v1(input.normalized_intrinsics, |work| {
            owner.charge(SemanticMirResourceV1::ValidationWork, work)
        })?
        .filter(|call| matches!(call.operation, NormalizedCallV1::SafeCoreShift(_)))
        .count();
        if count == 0 {
            return Ok(None);
        }
        let local_count = count
            .checked_mul(2)
            .ok_or_else(|| table("safe core shift local count"))?;
        owner.charge(SemanticMirResourceV1::Locals, local_count)?;
        owner.charge(
            SemanticMirResourceV1::ValidationWork,
            input.local_bindings.len(),
        )?;
        let function = canonical_function_identities_v1(input.tcx, input.instance).function();
        if function != input.identities.identity {
            return Err(table("safe core shift function owner"));
        }
        let body_sha256 = borrowed_rustc_mir_body_sha256_v1(input.tcx, input.instance, input.body);
        for binding in input.local_bindings {
            if binding.rustc_local as usize >= input.body.local_decls.len()
                || binding.identity
                    != rustc_local_identity_v1(function, body_sha256, binding.rustc_local)
            {
                return Err(table("safe core shift local source owner"));
            }
        }
        let capacity = local_count
            .checked_add(usize::from(receiver.is_some()))
            .ok_or_else(|| table("safe core shift local count"))?;
        owner.charge(SemanticMirResourceV1::ValidationWork, capacity)?;
        let mut inserted = try_vec_v1(capacity, SemanticMirResourceV1::Locals)?;
        let mut specifications = try_vec_v1(local_count, SemanticMirResourceV1::Locals)?;
        if let Some(receiver) = receiver.as_ref() {
            inserted.push(receiver.local.identity);
        }
        for call in locals::prepaid_rows_v1(input.normalized_intrinsics, |work| {
            owner.charge(SemanticMirResourceV1::ValidationWork, work)
        })? {
            let NormalizedCallV1::SafeCoreShift(shift) = call.operation else {
                continue;
            };
            owner.charge(SemanticMirResourceV1::ValidationWork, 5)?;
            let block = blocks
                .get(call.rustc_block as usize)
                .copied()
                .ok_or_else(|| table("safe core shift source block"))?;
            if call.caller != input.function
                || block.identity
                    != rustc_block_identity_v1(function, body_sha256, call.rustc_block)
                || !calls
                    .2
                    .get(call.rustc_block as usize)
                    .copied()
                    .flatten()
                    .is_some_and(|actual| std::ptr::eq(actual, call))
                || calls.0[call.rustc_block as usize].is_some()
                || calls.1[call.rustc_block as usize].is_some()
            {
                return Err(table("safe core shift occurrence owner"));
            }
            let actual = SafeCoreShiftV1::classify(input.tcx, call.expected_callee)
                .map_err(table)?
                .ok_or_else(|| table("safe core shift classification"))?;
            if !shift.same_producers(actual) || shift.value_type() != call.expected_element_type {
                return Err(table("safe core shift actual producers"));
            }
            let source = receiver_materialization_v1::reobserve_receiver_source_v1(
                input.tcx,
                input.body.basic_blocks
                    [rustc_middle::mir::BasicBlock::from_usize(call.rustc_block as usize)]
                .terminator()
                .source_info
                .span,
                input.body.span,
                owner,
            )?;
            if source != block.terminator_source {
                return Err(table("safe core shift source provenance"));
            }
            let ids = locals::identities_v1(
                function,
                block.identity,
                canonical_function_identities_v1(input.tcx, call.expected_callee).function(),
            );
            let value_type = types
                .get(&shift.value_type())
                .copied()
                .ok_or_else(|| table("safe core shift value type"))?;
            let count_type = types
                .get(&input.tcx.types.u32)
                .copied()
                .ok_or_else(|| table("safe core shift count type"))?;
            inserted.extend_from_slice(&ids);
            specifications.extend(ids.into_iter().zip([value_type, count_type]).map(
                |(identity, ty)| LocalV1 {
                    local: ReceiverLocalV1 {
                        identity,
                        local: SemanticLocalIdV1::from_index(0),
                    },
                    ty,
                    source,
                },
            ));
        }
        let order = locals::LocalOrderV1::new(
            input.local_bindings.iter().map(|binding| binding.identity),
            &inserted,
            |work| owner.charge(SemanticMirResourceV1::ValidationWork, work),
        )
        .map_err(local_error)?;
        if let Some(receiver) = receiver.as_mut() {
            receiver.local.local = order
                .index(receiver.local.identity, |work| {
                    owner.charge(SemanticMirResourceV1::ValidationWork, work)
                })
                .map_err(local_error)?;
        }
        let total = input
            .local_bindings
            .len()
            .checked_add(inserted.len())
            .ok_or_else(|| table("safe core shift local count"))?;
        owner.charge(SemanticMirResourceV1::ValidationWork, total)?;
        let mut local_slots = try_filled_vec_v1(total, None, SemanticMirResourceV1::Locals)?;
        for mut local in specifications {
            local.local.local = order
                .index(local.local.identity, |work| {
                    owner.charge(SemanticMirResourceV1::ValidationWork, work)
                })
                .map_err(local_error)?;
            let index = local.local.local.index() as usize;
            if local_slots[index].replace(local).is_some() {
                return Err(table("safe core shift duplicate local"));
            }
        }
        owner.charge(SemanticMirResourceV1::ValidationWork, blocks.len())?;
        let mut call_slots = try_filled_vec_v1(blocks.len(), None, SemanticMirResourceV1::Blocks)?;
        for call in locals::prepaid_rows_v1(input.normalized_intrinsics, |work| {
            owner.charge(SemanticMirResourceV1::ValidationWork, work)
        })? {
            if !matches!(call.operation, NormalizedCallV1::SafeCoreShift(_)) {
                continue;
            }
            owner.charge(SemanticMirResourceV1::ValidationWork, 3)?;
            let block = blocks[call.rustc_block as usize];
            let ids = locals::identities_v1(
                function,
                block.identity,
                canonical_function_identities_v1(input.tcx, call.expected_callee).function(),
            );
            let first = order
                .index(ids[0], |work| {
                    owner.charge(SemanticMirResourceV1::ValidationWork, work)
                })
                .map_err(local_error)?;
            let second = order
                .index(ids[1], |work| {
                    owner.charge(SemanticMirResourceV1::ValidationWork, work)
                })
                .map_err(local_error)?;
            if call_slots[call.rustc_block as usize]
                .replace([first, second])
                .is_some()
            {
                return Err(table("safe core shift duplicate occurrence"));
            }
        }
        Ok(Some(Self {
            local_count,
            locals: local_slots,
            calls: call_slots,
        }))
    }
}

impl<'tcx> BodyProducerV1<'_, '_, 'tcx> {
    pub(super) fn construct_wrapping_shift(
        &mut self,
        raw_block: u32,
        terminator: &TerminatorKind<'tcx>,
        source: SemanticSourceProvenanceV1,
        statements: &mut Vec<SemanticStatementV1>,
    ) -> Result<SemanticTerminatorKindV1, ProductionSemanticBodyErrorV1> {
        let block = Some(raw_block);
        let index = raw_block as usize;
        let recipe = self
            .normalized_intrinsics_by_raw
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(|| table("safe core shift recipe"))?;
        let NormalizedCallV1::SafeCoreShift(shift) = recipe.operation else {
            return Err(table("safe core shift recipe variant"));
        };
        let TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            unwind,
            ..
        } = terminator
        else {
            return Err(table("safe core shift source call"));
        };
        if args.len() != 2 || !matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable)
        {
            return Err(unsupported(
                "safe core shift call arity or executable unwind edge",
                block,
                None,
            ));
        }
        let target = target
            .ok_or_else(|| unsupported("safe core shift without return edge", block, None))?;
        self.owner
            .charge(SemanticMirResourceV1::CallArguments, args.len())?;
        let resolved = resolve_direct_call_v1(self.tcx, self.instance, self.body, func)
            .map_err(|error| unsupported(error, block, None))?;
        let actual = SafeCoreShiftV1::classify(self.tcx, resolved)
            .map_err(table)?
            .ok_or_else(|| table("safe core shift actual callee"))?;
        if recipe.caller != self.function
            || recipe.expected_callee != resolved
            || recipe.expected_element_type != actual.value_type()
            || !shift.same_producers(actual)
            || self.consumed_normalized_intrinsics[index]
            || self.direct_calls_by_raw[index].is_some()
            || self.terminal_expansions_by_raw[index].is_some()
        {
            return Err(table("safe core shift consuming occurrence"));
        }
        for (ty, expected) in [
            (
                args[0].node.ty(&self.body.local_decls, self.tcx),
                shift.value_type(),
            ),
            (
                args[1].node.ty(&self.body.local_decls, self.tcx),
                self.tcx.types.u32,
            ),
            (
                destination.ty(&self.body.local_decls, self.tcx).ty,
                shift.value_type(),
            ),
        ] {
            self.work()?;
            if normalize_type_v1(self.tcx, self.instance, ty)
                .map_err(|_| table("safe core shift operand type"))?
                != expected
            {
                return Err(table("safe core shift operand type"));
            }
        }
        let locals = self
            .wrapping_shifts
            .as_ref()
            .and_then(|shifts| shifts.calls.get(index))
            .copied()
            .flatten()
            .ok_or_else(|| table("safe core shift local occurrence"))?;
        let value_type = self.type_id(shift.value_type(), block, None)?;
        let count_type = self.type_id(self.tcx.types.u32, block, None)?;
        let saved = SemanticPlaceV1::new(locals[0], Vec::new(), value_type)?;
        let masked = SemanticPlaceV1::new(locals[1], Vec::new(), count_type)?;
        // Preserve the original argument use order, before writing destination.
        let value = self.construct_operand(&args[0].node, block, None)?;
        let count = self.construct_operand(&args[1].node, block, None)?;
        let destination = self.construct_place(*destination, block, None)?;
        self.owner.charge(SemanticMirResourceV1::Operands, 3)?;
        let mask = SemanticOperandV1::Constant(SemanticConstantV1::new(
            count_type,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(
                u128::from(shift.width() - 1),
                4,
            )?),
        ));
        let assignment = |destination, ty, kind| {
            SemanticStatementV1::new(
                source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                    destination,
                    SemanticRvalueV1::new(ty, kind),
                )),
            )
        };
        let output = [
            assignment(saved.clone(), value_type, SemanticRvalueKindV1::Use(value)),
            assignment(
                masked.clone(),
                count_type,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::BitAnd,
                    left: count,
                    right: mask,
                },
            ),
            assignment(
                destination,
                value_type,
                SemanticRvalueKindV1::Binary {
                    operation: match shift.direction() {
                        DirectionV1::Left => SemanticBinaryOpV1::ShiftLeft,
                        DirectionV1::Right => SemanticBinaryOpV1::ShiftRight,
                    },
                    left: SemanticOperandV1::Move(saved),
                    right: SemanticOperandV1::Move(masked),
                },
            ),
        ];
        let edge = self.edge(SemanticEdgeRoleV1::Goto, target.index())?;
        self.consumed_normalized_intrinsics[index] = true;
        statements.extend(output);
        Ok(SemanticTerminatorKindV1::Goto(edge))
    }
}

#[cfg(test)]
#[path = "wrapping_materialization_v1_tests.rs"]
mod tests;
