//! Borrowed replay of caller-owned inert CPU records; not source authentication.

use super::signature::ReferenceLogicalSignaturePreimageV1;
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

type Error = ReferenceBindingErrorV1;

/// All records are borrowed from the caller. This is not an authenticated binding.
pub struct ReferenceReplayInputV1<'a> {
    pub signature_preimage: &'a ReferenceLogicalSignaturePreimageV1,
    pub effect_ir: &'a ReferenceEffectIrV1,
    pub effect_ir_sha256: [u8; 32],
    pub observable_output_writes: &'a [ReferenceOutputWriteV1],
}

pub struct ReplayedCpuValueV1 {
    pub block: u32,
    pub statement: u32,
    pub expression: ReferenceEffectExpressionV1,
}

pub struct ReplayedCpuEffectsV1 {
    pub writes: Vec<ReferenceOutputWriteV1>,
    pub values: Vec<ReplayedCpuValueV1>,
    pub bounds: Vec<ResolvedReferenceBoundsCheckV1>,
}

struct CallbackWorkV1<'a>(RefCell<&'a mut dyn FnMut(usize) -> Result<(), Error>>);

impl ReferenceWorkV1 for CallbackWorkV1<'_> {
    fn charge(&self, amount: usize) -> Result<(), Error> {
        (*self.0.borrow_mut())(amount)
    }
}

fn resource(error: Resource) -> Error {
    Error::new(format!("conditional CPU MIR replay resource: {error}"))
}

/// Replays the existing acyclic CPU MIR resolver and checks both retained
/// effect lists before exposing fresh, callback-scoped output expressions,
/// computations and bounds assertions from the same retained CPU body.
/// Source authentication stays with the input's caller. This
/// does not discharge CPU bounds, prove GPU equivalence or grant lowering.
/// The callback's borrow cannot outlive the owned replay scratch:
///
/// ```compile_fail
/// use fe2o3_verifier::portable_reference_v1::{
///     ReferenceReplayInputV1, ReplayedCpuEffectsV1, with_replayed_output_writes_v1,
/// };
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape<'a>(input: ReferenceReplayInputV1<'_>, budget: &mut Budget<'_>)
///     -> &'a ReplayedCpuEffectsV1
/// {
///     with_replayed_output_writes_v1(input, budget, |effects, _| effects).unwrap()
/// }
/// ```
pub fn with_replayed_output_writes_v1<R>(
    input: ReferenceReplayInputV1<'_>,
    budget: &mut Budget<'_>,
    consume: impl for<'cpu> FnOnce(&'cpu ReplayedCpuEffectsV1, &mut Budget<'_>) -> R,
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
            let meter = CallbackWorkV1(RefCell::new(&mut charge));
            checked_replay(input, &meter)?
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

fn checked_replay(
    binding: ReferenceReplayInputV1<'_>,
    meter: &impl ReferenceWorkV1,
) -> Result<ReplayedCpuEffectsV1, Error> {
    meter.charge(16)?;
    let ir = binding.effect_ir;
    if ir.blocks.is_empty()
        || ir.blocks.len() > MAX_REFERENCE_BLOCKS_V1
        || !ir.loop_summaries.is_empty()
        || ir.local_count <= ir.argument_count
    {
        return Err(Error::new(
            "conditional CPU MIR replay requires a bounded acyclic body",
        ));
    }
    let signature = binding.signature_preimage;
    meter.charge(work::add(
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
        statements = work::add(statements, block.assignments.len())?;
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
        binding.observable_output_writes,
    ] {
        meter.charge(meter.effects(&writes)?)?;
        meter.charge(meter.effects(retained)?)?;
        if writes.as_slice() != retained {
            return Err(Error::new(
                "conditional CPU MIR replay differs from retained outputs",
            ));
        }
    }
    let resolver = ReferenceExpressionResolverV1::new(meter, ir)?;
    let mut values = Vec::new();
    let mut bounds = Vec::new();
    for block in &ir.blocks {
        meter.charge(1)?;
        for assignment in &block.assignments {
            meter.grow::<ReplayedCpuValueV1>(values.len())?;
            values.push(ReplayedCpuValueV1 {
                block: block.block,
                statement: assignment.statement,
                expression: resolver.resolve_value_v1(meter, &assignment.value)?,
            });
        }
        if let ReferenceTerminatorV1::Assert {
            condition,
            expected,
            bounds_check: Some(check),
            ..
        } = &block.terminator
        {
            meter.grow::<ResolvedReferenceBoundsCheckV1>(bounds.len())?;
            let resolve = |operand| {
                resolver.resolve_operand_inner_v1(meter, operand, &mut BTreeSet::new(), &mut 0, 1)
            };
            bounds.push(ResolvedReferenceBoundsCheckV1 {
                block: block.block,
                expected: *expected,
                condition: resolve(condition)?,
                index: resolve(&check.index)?,
                length: resolve(&check.length)?,
            });
        }
    }
    Ok(ReplayedCpuEffectsV1 {
        writes,
        values,
        bounds,
    })
}
