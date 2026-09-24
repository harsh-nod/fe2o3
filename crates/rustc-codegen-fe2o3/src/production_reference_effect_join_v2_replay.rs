//! Borrowed replay of retained CPU MIR, not authentication of a new binding.

use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

type Error = ReferenceBindingErrorV1;

fn resource(error: Resource) -> Error {
    Error::new(format!("conditional CPU MIR replay resource: {error}"))
}

impl AuthenticatedReferenceEffectBindingV1 {
    /// Replays the existing acyclic CPU MIR resolver and checks both retained
    /// effect lists before exposing fresh, callback-scoped output expressions.
    /// Source authentication stays with this original binding's caller. This
    /// does not discharge CPU bounds, prove GPU equivalence or grant lowering.
    pub(crate) fn with_replayed_output_writes_v1<R>(
        &self,
        budget: &mut Budget<'_>,
        consume: impl for<'cpu> FnOnce(&'cpu [ReferenceOutputWriteV1], &mut Budget<'_>) -> R,
    ) -> Result<R, Error> {
        let account = budget.work_ledger_identity_v1();
        let floor = budget.storage();
        let mut reserved = 0usize;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let writes = {
                // The existing resolver prepays requested payload/clone bytes as
                // work. Reserving its cumulative debit is a conservative scratch
                // envelope, not allocator/RSS accounting or a second work meter.
                let mut charge = |amount: usize| {
                    budget.charge_work(amount).map_err(resource)?;
                    let next = reserved
                        .checked_add(amount)
                        .ok_or_else(|| resource(Resource::Arithmetic))?;
                    budget.reserve_storage(amount).map_err(resource)?;
                    reserved = next;
                    Ok(())
                };
                let meter = ReferenceExtractionWorkV1::canonical(&mut charge);
                checked_replay(self, &meter)?
            };
            let protected = budget.storage();
            let result = consume(&writes, budget);
            drop(writes);
            if budget.work_ledger_identity_v1() != account || budget.storage() < protected {
                return Err(resource(Resource::Accounting));
            }
            Ok(result)
        }));
        let protected = floor
            .checked_add(reserved)
            .ok_or_else(|| resource(Resource::Arithmetic));
        let cleanup = match protected {
            Ok(protected)
                if budget.work_ledger_identity_v1() == account && budget.storage() >= protected =>
            {
                budget.release_storage(reserved).map_err(resource)
            }
            _ => Err(resource(Resource::Accounting)),
        };
        match result {
            Ok(result) => {
                cleanup?;
                result
            }
            Err(panic) => resume_unwind(panic),
        }
    }
}

fn checked_replay(
    binding: &AuthenticatedReferenceEffectBindingV1,
    meter: &ReferenceExtractionWorkV1<'_>,
) -> Result<Vec<ReferenceOutputWriteV1>, Error> {
    meter.charge(16)?;
    let ir = &binding.effect_ir;
    if ir.blocks.is_empty()
        || ir.blocks.len() > MAX_REFERENCE_BLOCKS_V1
        || !ir.loop_summaries.is_empty()
        || ir.local_count <= ir.argument_count
    {
        return Err(Error::new(
            "conditional CPU MIR replay requires a bounded acyclic body",
        ));
    }
    let signature = &binding.signature_preimage;
    meter.charge(reference_extraction_work_v1::add(
        signature.kernel_inputs().len(),
        signature.reference_inputs().len(),
    )?)?;
    let derived = signature
        .derive_relations_v1()
        .map_err(|error| Error::new(error.to_string()))?;
    if derived.len() != ir.relations.len() || derived.len() != ir.argument_count as usize {
        return Err(Error::new("conditional CPU MIR signature arity changed"));
    }
    for (raw, relation) in ir.relations.iter().enumerate() {
        meter.charge(8)?;
        let raw = u32::try_from(raw).map_err(|_| resource(Resource::Arithmetic))?;
        if derived.relation_at_raw_argument_v1(raw) != Some(*relation) {
            return Err(Error::new(
                "conditional CPU MIR raw argument relation changed",
            ));
        }
    }
    let mut statements = 0usize;
    for (index, block) in ir.blocks.iter().enumerate() {
        meter.charge(4)?;
        statements = reference_extraction_work_v1::add(statements, block.assignments.len())?;
        if block.block as usize != index || statements > MAX_REFERENCE_STATEMENTS_V1 {
            return Err(Error::new(
                "conditional CPU MIR block or statement roster changed",
            ));
        }
        let mut previous = None;
        for assignment in &block.assignments {
            meter.charge(4)?;
            if assignment.destination.local >= ir.local_count
                || previous.is_some_and(|statement| statement >= assignment.statement)
            {
                return Err(Error::new(
                    "conditional CPU MIR assignment occurrence changed",
                ));
            }
            previous = Some(assignment.statement);
        }
    }
    // The existing debit checks recursive depth/nodes before hashing or Eq.
    meter.ir_hash(ir)?;
    if ir.canonical_sha256_v1() != binding.effect_ir_sha256 {
        return Err(Error::new("conditional CPU MIR effect digest changed"));
    }
    // This is the same resolver called by live rustc extraction. Its CFG walk
    // rejects cycles, including a forged cycle with an empty summary list.
    let writes = ir.observable_output_writes_v1(meter)?;
    for retained in [
        ir.observable_output_effects.as_ref(),
        binding.observable_output_writes.as_ref(),
    ] {
        meter.charge(meter.effects(&writes)?)?;
        meter.charge(meter.effects(retained)?)?;
        if writes.as_slice() != retained {
            return Err(Error::new(
                "conditional CPU MIR replay differs from retained outputs",
            ));
        }
    }
    meter.charge(1)?;
    Ok(writes)
}

#[cfg(test)]
#[path = "production_reference_effect_join_v2_replay_tests.rs"]
mod tests;
