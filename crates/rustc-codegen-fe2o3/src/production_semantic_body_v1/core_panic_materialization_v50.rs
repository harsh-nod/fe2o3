//! Consume the exact original pure-literal panic recipe once, retaining Abort.
use super::*;
use crate::production_core_panic_v50::{CorePanicErrorV50, observe};

impl BodyProducerV1<'_, '_, '_> {
    pub(super) fn construct_core_panic_v50(
        &mut self,
        raw_block: u32,
    ) -> Result<SemanticTerminatorKindV1, ProductionSemanticBodyErrorV1> {
        let index = raw_block as usize;
        let recipe = self
            .normalized_intrinsics_by_raw
            .get(index)
            .copied()
            .flatten()
            .ok_or_else(|| table("core panic normalized recipe"))?;
        let NormalizedCallV1::CorePanic(expected) = recipe.operation else {
            return Err(table("core panic normalized recipe kind"));
        };
        self.owner.charge(SemanticMirResourceV1::CallArguments, 1)?;
        self.owner.charge(SemanticMirResourceV1::Operands, 2)?;
        self.owner
            .charge(SemanticMirResourceV1::ConstantBytes, expected.bytes().len())?;
        let actual = observe(
            self.tcx,
            self.instance,
            self.body,
            rustc_middle::mir::BasicBlock::from_usize(index),
            &mut |amount| {
                self.owner.totals.charge(
                    SemanticMirResourceV1::ValidationWork,
                    amount,
                    self.owner.limits,
                )
            },
        )
        .map_err(|error| match error {
            CorePanicErrorV50::Work(error) => error,
            CorePanicErrorV50::Refused(reason) => unsupported(reason, Some(raw_block), None),
        })?
        .ok_or_else(|| table("core panic original call"))?;
        if recipe.caller != self.function
            || recipe.expected_callee != actual.instance()
            || recipe.expected_element_type != self.tcx.types.never
            || !expected.same_producers(actual)
            || self.consumed_normalized_intrinsics[index]
            || self.direct_calls_by_raw[index].is_some()
            || self.terminal_expansions_by_raw[index].is_some()
        {
            return Err(table("core panic consuming occurrence"));
        }
        self.consumed_normalized_intrinsics[index] = true;
        Ok(SemanticTerminatorKindV1::Abort)
    }
}
