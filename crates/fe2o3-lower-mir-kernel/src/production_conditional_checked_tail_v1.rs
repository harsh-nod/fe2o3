//! Conditional I-to-J agreement on the existing owned redundant-store history.
//! This neither constructs J nor admits ordinary source/formal/native evidence.
use super::{
    Budget, Coordinates, Facts, Graph, Prefix, ProductionSourceBoundConditionalAggregateRequestV1,
    Resource, facts, occurrences, scoped_resource, source_kernel,
};
use fe2o3_kernel_analysis::CanonicalKirRedundantStoreRetainedOperationV1 as Retained;
use fe2o3_kernel_ir::{
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirFunctionCoordinateV1 as Function,
    CanonicalKirOperationCoordinateV1 as Site, ConditionalTotalViewReadV1 as Read,
    FunctionOperationLocation as Location, KernelId,
};
use fe2o3_kernel_opt::{CheckedRedundantStoreErrorV1, OwnedRedundantStoreContinuationV1 as Tail};
use fe2o3_pliron::{
    KirBridgeCoordinateV1 as Bridge, ProductionConditionalRuntimePremiseV1 as Premise,
};
use std::{fmt, mem::size_of};

/// A refusal of the existing source/policy6 prefix or its actual I-to-J tail.
#[derive(Debug)]
pub enum ProductionConditionalCheckedTailErrorV1 {
    /// The original source or target account refused work, storage, or cleanup.
    Resource(Resource),
    /// Source, policy6, coverage, occurrence, or explicit premise checks failed.
    Prefix(super::ProductionConditionalCheckedOutputErrorV1),
    /// The owned J history did not independently replay against actual I.
    Tail(CheckedRedundantStoreErrorV1),
    /// The selected actual subjects or complete retained occurrences differed.
    Mismatch(&'static str),
}
type Error = ProductionConditionalCheckedTailErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<super::ProductionConditionalCheckedOutputErrorV1> for Error {
    fn from(value: super::ProductionConditionalCheckedOutputErrorV1) -> Self {
        Self::Prefix(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional N-through-J agreement: {self:?}")
    }
}
impl std::error::Error for Error {}

const SCRATCH: usize = 4 * size_of::<Facts<'static>>() + size_of::<Error>() + 1024;

fn scoped<T>(budget: &mut Budget<'_>, run: impl FnOnce(&mut Budget<'_>) -> Result<T>) -> Result<T> {
    scoped_resource(budget, run)
}

impl ProductionSourceBoundConditionalAggregateRequestV1<'_> {
    /// Checks this original source request through the supplied actual I and J.
    /// N-to-I is checked once with the existing method; J is independently
    /// replayed and analyzed, never cloned, reconstructed or optimized here.
    /// The backend must supply its already target-checked N/B coordinates and
    /// retain the actual single prefix/tail construction and both original
    /// accounts. This check does not authenticate that producer's execution.
    ///
    /// B, the whole policy6 prefix and J must already be simultaneously reserved
    /// on `target`; source/arena/proof custody stays on distinct `source`.
    /// Graph replay/analysis uses target, while source association, occurrence
    /// and premise comparison uses source. Added scratch is released only after
    /// its owners die, including refusal/unwind; spent work is never refunded.
    /// Genuine receipt reimport, CPU-source replay and exact contract projection
    /// must surround this method in the existing backend replay callback.
    ///
    /// Success is unit, not a clean ranked/formal receipt, final F, native V4
    /// admission, publication or launch authority. Unsupported coverage fails
    /// closed. The existing conditional-finalizer refusal remains mandatory.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::{ProductionSourceBoundConditionalAggregateRequestV1 as Request,
    ///     ProductionFormalMemoryOwnerV1};
    /// use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1 as Coordinates;
    /// use fe2o3_kernel_opt::{CheckedCanonicalKernelIrOwnerPolicy6V1 as Prefix,
    ///     OwnedRedundantStoreContinuationV1 as Tail};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn promote(r: &Request<'_>, c: &Coordinates<'_, '_>, i: &Prefix, j: &Tail,
    ///     target: &mut Budget<'_>, source: &mut Budget<'_>) -> ProductionFormalMemoryOwnerV1 {
    ///     r.check_policy6_redundant_store_output_v1(c, i, j, target, source).unwrap()
    /// }
    /// ```
    #[allow(clippy::result_large_err)]
    pub fn check_policy6_redundant_store_output_v1(
        &self,
        coordinates: &Coordinates<'_, '_>,
        checked: &Prefix,
        tail: &Tail,
        target: &mut Budget<'_>,
        source: &mut Budget<'_>,
    ) -> Result<()> {
        scoped(source, |source| {
            scoped(target, |target| {
                require_target_floor(coordinates.output(), checked, tail, target)?;
                self.check_policy6_output_v1(coordinates, checked, target, source)?;
                let kernel = source_kernel(self, source)?;
                check_tail(
                    checked,
                    tail,
                    kernel,
                    self.pliron_input().premises(),
                    target,
                    source,
                )?;
                self.pliron_input()
                    .require_current_graph_v1(source)
                    .map_err(super::Error::Graph)?;
                Ok(())
            })
        })
    }
}

fn require_target_floor(
    bound: &Graph,
    prefix: &Prefix,
    tail: &Tail,
    target: &mut Budget<'_>,
) -> Result<()> {
    target.charge_work(4)?;
    let required = prefix
        .retained_storage()
        .checked_add(bound.canonical().canonical_bytes().len())
        .and_then(|n| n.checked_add(tail.retained_storage()))
        .ok_or(Resource::Arithmetic)?;
    if target.storage() < required {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}

fn check_tail(
    prefix: &Prefix,
    tail: &Tail,
    kernel: &KernelId,
    premises: &[Premise],
    target: &mut Budget<'_>,
    source: &mut Budget<'_>,
) -> Result<()> {
    source.charge_work(1)?;
    if source.work_ledger_identity_v1() == target.work_ledger_identity_v1() {
        return Err(Error::Mismatch("distinct original phase accounts"));
    }
    source.reserve_storage(SCRATCH)?;
    target.reserve_storage(SCRATCH)?;
    let (relation, storage) = tail
        .replay_against(prefix.owner(), target)
        .map_err(Error::Tail)?;
    target.reserve_storage(storage.retained_storage())?;
    if !std::ptr::eq(relation.input(), prefix.owner())
        || !std::ptr::eq(relation.output(), tail.output())
    {
        return Err(Error::Mismatch("actual I/J relation endpoints"));
    }
    let before = facts(prefix.owner(), kernel, target)?;
    let after = facts(tail.output(), kernel, target)?;
    let original = occurrences::reads(&before, source)?;
    let output = occurrences::reads(&after, source)?;
    coverage(prefix, tail, &before, &after, &original, &output, source)?;
    occurrences::premises(&after, &output, premises, source)?;
    drop(relation);
    Ok(())
}

fn coverage(
    prefix: &Prefix,
    tail: &Tail,
    before: &Facts<'_>,
    after: &Facts<'_>,
    original: &[Read],
    output: &[Read],
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(
        prefix
            .owner()
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(tail.output().canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    if !std::ptr::eq(before.module(), prefix.owner().module())
        || !std::ptr::eq(after.module(), tail.output().module())
        || before.function_ordinal() != after.function_ordinal()
    {
        return Err(Error::Mismatch("actual I/J conditional coverage"));
    }
    occurrences::coverage_subjects(before, after, original, output)?;
    same_occurrence(
        before,
        before.store_location(),
        after,
        after.store_location(),
        tail.retained_operations(),
        budget,
    )?;
    for (a, b) in original.iter().zip(output) {
        occurrences::read_premises(*a, *b, budget)?;
        budget.charge_work(2 * size_of::<Read>())?;
        if a.slice() != b.slice()
            || a.pointer() != b.pointer()
            || a.index() != b.index()
            || a.value() != b.value()
        {
            return Err(Error::Mismatch("complete ordered I/J read agreement"));
        }
        same_occurrence(
            before,
            a.location(),
            after,
            b.location(),
            tail.retained_operations(),
            budget,
        )?;
    }
    Ok(())
}

pub(super) fn site(coordinate: Bridge) -> Result<Site> {
    let Bridge::Operation {
        function,
        block,
        operation,
    } = coordinate
    else {
        return Err(Error::Mismatch("conditional operation coordinate"));
    };
    Ok(Site {
        block: Block {
            function: Function(function),
            block,
        },
        operation,
    })
}

fn same_occurrence(
    before: &Facts<'_>,
    a: Location,
    after: &Facts<'_>,
    b: Location,
    rows: &[Retained],
    budget: &mut Budget<'_>,
) -> Result<()> {
    // Both coordinate APIs use roster ordinals, never raw BlockId values.
    let a = occurrences::coordinate(before, a, budget)?;
    let b = occurrences::coordinate(after, b, budget)?;
    require_mapping(site(a)?, site(b)?, rows, budget)?;
    budget.charge_work(2)?;
    // J deletes stores but does not rewrite any surviving operation or SSA id.
    // The full graph comparison allowance was prepaid by coverage above.
    if occurrences::operation(before.module(), a)? != occurrences::operation(after.module(), b)? {
        return Err(Error::Mismatch("surviving I/J memory operation changed"));
    }
    Ok(())
}

fn require_mapping(
    input: Site,
    output: Site,
    rows: &[Retained],
    budget: &mut Budget<'_>,
) -> Result<()> {
    let mut found = false;
    for row in rows {
        budget.charge_work(3)?;
        if row.input == input || row.output == output {
            if found || row.input != input || row.output != output {
                return Err(Error::Mismatch("unique retained I/J memory occurrence"));
            }
            found = true;
        }
    }
    if !found {
        return Err(Error::Mismatch("missing retained I/J memory occurrence"));
    }
    Ok(())
}

#[cfg(test)]
#[path = "production_conditional_checked_tail_v1_tests.rs"]
mod tests;
