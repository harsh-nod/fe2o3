//! Conditional source agreement through the actual independently checked final F.
//! Borrowed semantic composition is not authenticated producer/native authority.
use super::{
    Budget, Coordinates, Facts, Graph, Prefix, ProductionSourceBoundConditionalAggregateRequestV1,
    Resource, facts, occurrences, scoped_resource, source_kernel, tail,
};
use fe2o3_kernel_analysis::{
    CanonicalKirInductionRefinementOriginV1 as Refinement,
    CanonicalKirPrivateCellOriginKindV1 as Promotion,
};
use fe2o3_kernel_ir::{
    CanonicalKirOperationCoordinateV1 as Site, CanonicalKirOperationOriginV1 as Origin,
    ConditionalTotalViewReadV1 as Read, KernelId,
};
use fe2o3_kernel_opt::{
    CanonicalRefinedForwardingHistoryErrorV1 as HistoryError,
    CanonicalRefinedForwardingHistoryInputsV1 as Inputs,
    CanonicalRefinedForwardingHistoryLimitsV1 as Limits,
    CheckedCanonicalRefinedForwardingHistoryV1 as History,
    check_canonical_refined_forwarding_history_v1,
};
use fe2o3_pliron::ProductionConditionalRuntimePremiseV1 as Premise;
use std::{
    fmt,
    mem::{size_of, size_of_val},
};

/// Refusal of original conditional source agreement with the supplied final F.
#[derive(Debug)]
pub enum ProductionConditionalCheckedFinalErrorV1 {
    /// An original account refused work, storage, arithmetic, or valid cleanup.
    Resource(Resource),
    /// Genuine N-to-I source, occurrence, coverage, or premise agreement failed.
    Prefix(super::ProductionConditionalCheckedOutputErrorV1),
    /// Complete independent B/C/S/O/I/J/K/P/H/L/R/F replay failed.
    History(HistoryError),
    /// A memory coordinate could not be interpreted in its actual graph roster.
    Coordinate(tail::ProductionConditionalCheckedTailErrorV1),
    /// Expected producer limits differ from the supplied semantic history limits.
    LimitsMismatch,
    /// An actual endpoint, complete occurrence association, or account differed.
    Mismatch(&'static str),
}
type Error = ProductionConditionalCheckedFinalErrorV1;
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
        write!(out, "conditional source-through-F agreement: {self:?}")
    }
}
impl std::error::Error for Error {}

const SCRATCH: usize =
    4 * size_of::<Facts<'static>>() + size_of::<Inputs<'static>>() + size_of::<Error>() + 1024;

fn scoped<T>(budget: &mut Budget<'_>, run: impl FnOnce(&mut Budget<'_>) -> Result<T>) -> Result<T> {
    scoped_resource(budget, run)
}

impl ProductionSourceBoundConditionalAggregateRequestV1<'_> {
    /// Checks this genuine request once through N-to-I, then through final F.
    /// Every B/C/S/O/I borrow must be the supplied coordinate/policy6 owner's
    /// actual endpoint. All later pairs and complete rows are independently
    /// replayed by the existing fixed-history checker, without running a pass.
    /// Final conditional coverage and premises are freshly derived; memory
    /// occurrences follow checked lineage, not an assumption of stable SSA IDs.
    /// Unsupported final coverage or lost occurrences fail closed.
    /// The complete ORIGINAL premise roster remains mandatory, not a minimized
    /// projection of optimized uses. Current coverage admits only nonvolatile
    /// global readonly-slice reads; none of these fixed tail engines eliminates
    /// such reads. Private-load elimination, loops/block arguments in the
    /// conditional entry, and other unsupported coverage domains are NOT admitted
    /// by this API. Bitwise value sharing and changed operation coordinates are.
    ///
    /// Producer custody is BACKEND-owned: `history` is an untrusted semantic
    /// view, not proof of execution. The consumer must borrow its once-built
    /// owners, supply `expected_limits` from the original R/F owners, and check
    /// the complete simultaneously retained owner/row-capacity floor on the
    /// original target account BEFORE this call. The checks here include all
    /// borrowed wire/row lengths, not hidden decoded capacities or allocator RSS.
    /// Genuine receipt reimport, CPU-source replay and exact contract projection
    /// remain mandatory in the original source-ledger guarded callback. Neither
    /// a numeric floor nor this API authenticates a fresh replacement account.
    ///
    /// Both distinct original accounts retain their entry floors, work and
    /// first-denial history. Added scratch and the borrowed replay receipt are
    /// dropped before same-ledger cleanup on return or unwind. No executable
    /// graph is copied or constructed. Success is only unit: no ordinary formal
    /// receipt, native source recovery, publication, load or launch authority.
    /// The separate conditional-finalizer refusal remains required; this is not
    /// source-to-native completion or issue #272 closure.
    ///
    /// ```compile_fail,E0308
    /// use fe2o3_lower_mir_kernel::{ProductionSourceBoundConditionalAggregateRequestV1 as Request,
    ///     ProductionFormalMemoryOwnerV1};
    /// use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1 as Coordinates;
    /// use fe2o3_kernel_opt::{CheckedCanonicalKernelIrOwnerPolicy6V1 as Prefix,
    ///     CanonicalRefinedForwardingHistoryInputsV1 as Inputs,
    ///     CanonicalRefinedForwardingHistoryLimitsV1 as Limits};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn promote(r: &Request<'_>, c: &Coordinates<'_, '_>, i: &Prefix,
    ///     h: Inputs<'_>, limits: Limits, target: &mut Budget<'_>, source: &mut Budget<'_>)
    ///     -> ProductionFormalMemoryOwnerV1 {
    ///     r.check_refined_forwarding_output_v1(c, i, h, limits, target, source).unwrap()
    /// }
    /// ```
    /// ```compile_fail,E0308
    /// use fe2o3_lower_mir_kernel::{ProductionSourceBoundConditionalAggregateRequestV1 as Request,
    ///     ProductionPreRankedKirOwnerV1};
    /// use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1 as Coordinates;
    /// use fe2o3_kernel_opt::{CheckedCanonicalKernelIrOwnerPolicy6V1 as Prefix,
    ///     CanonicalRefinedForwardingHistoryInputsV1 as Inputs,
    ///     CanonicalRefinedForwardingHistoryLimitsV1 as Limits};
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn source(r: &Request<'_>, c: &Coordinates<'_, '_>, i: &Prefix,
    ///     h: Inputs<'_>, limits: Limits, target: &mut Budget<'_>, source: &mut Budget<'_>)
    ///     -> ProductionPreRankedKirOwnerV1 {
    ///     r.check_refined_forwarding_output_v1(c, i, h, limits, target, source).unwrap()
    /// }
    /// ```
    #[allow(clippy::result_large_err)]
    pub fn check_refined_forwarding_output_v1(
        &self,
        coordinates: &Coordinates<'_, '_>,
        checked: &Prefix,
        history: Inputs<'_>,
        expected_limits: Limits,
        target: &mut Budget<'_>,
        source: &mut Budget<'_>,
    ) -> Result<()> {
        scoped(source, |source| {
            scoped(target, |target| {
                require_subjects(
                    coordinates.output(),
                    checked,
                    history,
                    expected_limits,
                    target,
                )?;
                self.check_policy6_output_v1(coordinates, checked, target, source)?;
                let kernel = source_kernel(self, source)?;
                check_final(
                    checked,
                    history,
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

fn require_subjects(
    bound: &Graph,
    checked: &Prefix,
    history: Inputs<'_>,
    expected_limits: Limits,
    target: &mut Budget<'_>,
) -> Result<()> {
    target.charge_work(size_of::<Limits>() + 12)?;
    if history.limits != expected_limits {
        return Err(Error::LimitsMismatch);
    }
    let p6 = history.prefix.prefix.prefix;
    let p5 = checked.intermediate_policy5();
    let p4 = p5.intermediate_policy4();
    for (actual, expected) in [
        (p6.prefix.input, bound),
        (p6.prefix.intermediate, p4.intermediate_policy3().owner()),
        (p6.prefix.stored, p4.owner()),
        (p6.prefix.output, p5.owner()),
        (p6.output, checked.owner()),
    ] {
        if !std::ptr::eq(actual, expected) {
            return Err(Error::Mismatch("actual bound B/C/S/O/I endpoints"));
        }
    }
    // Prefix backing already belongs to checked; reject replacement load rows.
    if !std::ptr::eq(p6.prefix.load_rows, p5.load_forwarding_rows()) {
        return Err(Error::Mismatch("actual policy5 load rows"));
    }
    let records: [(&[u8], &[u8]); 3] = [
        (p6.prefix.policy5_record, p5.execution().canonical_bytes()),
        (
            p6.continuation.composition_record,
            checked.execution().canonical_bytes(),
        ),
        (
            p6.continuation.integer_record,
            checked.continuation().execution().canonical_bytes(),
        ),
    ];
    for (actual, expected) in records {
        if !std::ptr::eq(actual, expected) {
            return Err(Error::Mismatch("actual policy5/policy6 records"));
        }
    }
    let required = backing_floor(bound, checked, history, target)?;
    if target.storage() < required {
        return Err(Resource::Accounting.into());
    }
    Ok(())
}

// Minimum visible backing only. Full decoded/owner/capacity custody stays with
// the backend's existing original-account guard, never inferred from these views.
fn backing_floor(
    bound: &Graph,
    checked: &Prefix,
    h: Inputs<'_>,
    budget: &mut Budget<'_>,
) -> Result<usize> {
    budget.charge_work(40)?;
    let p7 = h.prefix.prefix;
    let p6 = p7.prefix;
    let c = h.prefix.continuation.occurrences;
    let mut total = checked.retained_storage();
    for graph in [
        bound,
        p7.output,
        h.prefix.output,
        h.promoted,
        h.preheaders,
        h.licm,
        h.refined,
        h.output,
    ] {
        total = total
            .checked_add(graph.canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?;
    }
    for bytes in [
        p6.prefix.policy4_wire.len(),
        p6.continuation.transition_wire.len(),
        p7.continuation.execution_record.len(),
        h.prefix.continuation.pass_name.len(),
        size_of_val(p7.continuation.deletion_rows),
        size_of_val(p7.continuation.retained_operations),
        size_of_val(c.functions),
        size_of_val(c.blocks),
        size_of_val(c.segments),
        size_of_val(c.operations),
        size_of_val(c.definitions),
        size_of_val(c.definition_outputs),
        size_of_val(c.uses),
        size_of_val(c.edges),
        size_of_val(c.edge_arguments),
        size_of_val(h.selected_allocations),
        size_of_val(h.promotion_origins),
        size_of_val(h.preheader_rows),
        size_of_val(h.licm_origins),
        size_of_val(h.refinement_origins),
        size_of_val(h.forwarding_origins),
    ] {
        total = total.checked_add(bytes).ok_or(Resource::Arithmetic)?;
    }
    Ok(total)
}

fn check_final(
    checked: &Prefix,
    inputs: Inputs<'_>,
    kernel: &KernelId,
    premises: &[Premise],
    target: &mut Budget<'_>,
    source: &mut Budget<'_>,
) -> Result<()> {
    source.charge_work(1)?;
    if source.work_ledger_identity_v1() == target.work_ledger_identity_v1() {
        return Err(Error::Mismatch(
            "distinct original source and target accounts",
        ));
    }
    source.reserve_storage(SCRATCH)?;
    target.reserve_storage(SCRATCH)?;
    let history =
        check_canonical_refined_forwarding_history_v1(inputs, target).map_err(Error::History)?;
    target.reserve_storage(history.storage().retained_storage())?;
    let before = facts(checked.owner(), kernel, target)?;
    let after = facts(history.output(), kernel, target)?;
    let mut ordered = occurrences::reads(&before, source)?;
    let output = occurrences::reads(&after, source)?;
    // Never discard an original proof/runtime assumption because later scalar
    // uses share a value. The source request and its premise roster stay intact.
    occurrences::premises(&before, &ordered, premises, source)?;
    coverage(
        checked,
        &history,
        &before,
        &after,
        &mut ordered,
        &output,
        source,
    )?;
    drop(history);
    Ok(())
}

fn coordinate(
    facts: &Facts<'_>,
    location: fe2o3_kernel_ir::FunctionOperationLocation,
    budget: &mut Budget<'_>,
) -> Result<Site> {
    tail::site(occurrences::coordinate(facts, location, budget)?).map_err(Error::Coordinate)
}

fn coverage(
    checked: &Prefix,
    history: &History<'_>,
    before: &Facts<'_>,
    after: &Facts<'_>,
    ordered: &mut [Read],
    output: &[Read],
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(
        checked
            .owner()
            .canonical()
            .canonical_bytes()
            .len()
            .checked_add(history.output().canonical().canonical_bytes().len())
            .ok_or(Resource::Arithmetic)?,
    )?;
    if !std::ptr::eq(before.module(), checked.owner().module())
        || !std::ptr::eq(after.module(), history.output().module())
        || before.function_ordinal() != after.function_ordinal()
    {
        return Err(Error::Mismatch("actual I/F conditional subjects"));
    }
    // The admitted global readonly loads cannot be deleted by these fixed
    // engines (K deletes only scalar bitwise definitions; P/F copy only private
    // loads). Check complete cardinality AND lineage, not raw read SSA equality.
    occurrences::coverage_subjects(before, after, ordered, output)?;
    if follow(
        history,
        coordinate(before, before.store_location(), budget)?,
        budget,
    )? != coordinate(after, after.store_location(), budget)?
    {
        return Err(Error::Mismatch("final conditional store occurrence"));
    }
    for index in 0..ordered.len() {
        let original = ordered[index];
        let site = follow(
            history,
            coordinate(before, original.location(), budget)?,
            budget,
        )?;
        let mut found = None;
        for read in output {
            budget.charge_work(2)?;
            if coordinate(after, read.location(), budget)? == site && found.replace(*read).is_some()
            {
                return Err(Error::Mismatch("unique final read occurrence"));
            }
        }
        let read = found.ok_or(Error::Mismatch("missing final read occurrence"))?;
        for previous in &ordered[..index] {
            budget.charge_work(2)?;
            if previous.location() == read.location() {
                return Err(Error::Mismatch("complete one-to-one final reads"));
            }
        }
        occurrences::read_premises(original, read, budget)?;
        ordered[index] = read;
    }
    Ok(())
}

fn mapped<T>(
    input: Site,
    rows: &[T],
    project: impl Fn(&T) -> (Option<Site>, Option<Site>),
    budget: &mut Budget<'_>,
) -> Result<Site> {
    let mut found = None;
    for row in rows {
        budget.charge_work(4)?;
        let (from, to) = project(row);
        if from != Some(input) {
            continue;
        }
        let output = to.ok_or(Error::Mismatch("conditional memory operation rewritten"))?;
        if found.replace(output).is_some() {
            return Err(Error::Mismatch("unique surviving memory occurrence"));
        }
    }
    found.ok_or(Error::Mismatch("missing surviving memory occurrence"))
}

// Only a sealed complete-history receipt reaches this function. The independent
// engines establish operation semantics, including K's operand substitutions.
fn follow(history: &History<'_>, input: Site, budget: &mut Budget<'_>) -> Result<Site> {
    let p8 = history.prefix();
    let j = mapped(
        input,
        p8.policy7_relation()
            .continuation()
            .relation()
            .retained_operations(),
        |r| (Some(r.input), Some(r.output)),
        budget,
    )?;
    let k = mapped(
        j,
        p8.continuation().claims().occurrences.operations,
        |r| {
            (
                match r.origin {
                    Origin::Retained(input) => Some(input),
                    _ => None,
                },
                Some(r.output),
            )
        },
        budget,
    )?;
    let p = mapped(
        k,
        history.promotion().origins(),
        |r| {
            (
                Some(r.input),
                (r.kind == Promotion::Retained).then_some(r.output),
            )
        },
        budget,
    )?;
    // Checked P/H subdivision appends empty blocks and preserves every original
    // block's operations and roster coordinate; no synthetic memory is possible.
    budget.charge_work(1)?;
    let l = mapped(
        p,
        history.licm().origins(),
        |r| (Some(r.input), r.hoist.is_none().then_some(r.output)),
        budget,
    )?;
    let r = mapped(
        l,
        history.refinement().origins(),
        |r| {
            (
                Some(r.input()),
                match *r {
                    Refinement::Unchanged { output, .. } => Some(output),
                    _ => None,
                },
            )
        },
        budget,
    )?;
    mapped(
        r,
        history.forwarding().origins(),
        |r| (Some(r.input), r.store.is_none().then_some(r.output)),
        budget,
    )
}

#[cfg(test)]
#[path = "production_conditional_checked_final_v1_tests.rs"]
mod tests;
