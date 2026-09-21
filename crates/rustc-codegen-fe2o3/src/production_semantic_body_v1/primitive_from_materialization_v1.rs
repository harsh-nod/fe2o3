//! One checked From call becomes one exact integer cast and its original edge.
use super::*;
use crate::production_primitive_from_v1::{
    PrimitiveFromErrorV1, PrimitiveFromStageV1, check_primitive_from_v1,
};

impl<'tcx> BodyProducerV1<'_, '_, 'tcx> {
    pub(super) fn construct_primitive_from(
        &mut self,
        raw_block: u32,
        terminator: &TerminatorKind<'tcx>,
    ) -> Result<(SemanticStatementKindV1, SemanticTerminatorKindV1), ProductionSemanticBodyErrorV1>
    {
        let block = Some(raw_block);
        let index = raw_block as usize;
        let recipe = self
            .normalized_intrinsics_by_raw
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(|| table("checked primitive From recipe"))?;
        let NormalizedCallV1::CheckedPrimitiveFrom(expected) = recipe.operation else {
            return Err(table("checked primitive From recipe variant"));
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
            return Err(table("checked primitive From source call"));
        };
        if args.len() != 1 || !matches!(unwind, UnwindAction::Continue | UnwindAction::Unreachable)
        {
            return Err(unsupported(
                "checked primitive From arity or unwind edge",
                block,
                None,
            ));
        }
        let target = target.ok_or_else(|| {
            unsupported("checked primitive From without return edge", block, None)
        })?;
        self.owner
            .charge(SemanticMirResourceV1::CallArguments, args.len())?;
        let resolved = resolve_direct_call_v1(self.tcx, self.instance, self.body, func)
            .map_err(|error| unsupported(error, block, None))?;
        let actual = check_primitive_from_v1(
            self.tcx,
            resolved,
            PrimitiveFromStageV1::BodyReplay,
            self.owner.limits,
            &mut |amount| {
                self.owner.totals.charge(
                    SemanticMirResourceV1::ValidationWork,
                    amount,
                    self.owner.limits,
                )
            },
        )
        .map_err(|error| match error {
            PrimitiveFromErrorV1::Work { source, .. } => source,
            other => unsupported(other.to_string(), block, None),
        })?
        .ok_or_else(|| table("checked primitive From actual callee"))?;
        if recipe.caller != self.function
            || recipe.expected_callee != resolved
            || recipe.expected_element_type != actual.output_type()
            || !expected.same_producers(actual)
            || self.consumed_normalized_intrinsics[index]
            || self.direct_calls_by_raw[index].is_some()
            || self.terminal_expansions_by_raw[index].is_some()
        {
            return Err(table("checked primitive From consuming occurrence"));
        }
        for (raw, expected) in [
            (
                args[0].node.ty(&self.body.local_decls, self.tcx),
                actual.input_type(),
            ),
            (
                destination.ty(&self.body.local_decls, self.tcx).ty,
                actual.output_type(),
            ),
        ] {
            self.work()?;
            if normalize_type_v1(self.tcx, self.instance, raw)
                .map_err(|_| table("checked primitive From operand type"))?
                != expected
            {
                return Err(table("checked primitive From operand type"));
            }
        }
        let result_type = self.type_id(actual.output_type(), block, None)?;
        let operand = self.construct_operand(&args[0].node, block, None)?;
        let destination = self.construct_place(*destination, block, None)?;
        let statement = SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            destination,
            SemanticRvalueV1::new(
                result_type,
                SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer,
                    operand,
                },
            ),
        ));
        let edge = self.edge(SemanticEdgeRoleV1::Goto, target.index())?;
        self.consumed_normalized_intrinsics[index] = true;
        Ok((statement, SemanticTerminatorKindV1::Goto(edge)))
    }
}
