//! Direct policy6 N-to-I agreement, not final-F or ordinary formal evidence.
use super::{
    ProductionConditionalContinuationErrorV1, ProductionSourceBoundConditionalAggregateRequestV1,
};
use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1 as Coordinates;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, ConditionalTotalViewErrorV1,
    ConditionalTotalViewFactsV1 as Facts, VerifiedCanonicalKernelIrModuleV12 as Graph,
    derive_conditional_total_view_from_verified_v1,
};
use fe2o3_kernel_opt::{
    CanonicalPolicy6OptimizationErrorV1, CheckedCanonicalKernelIrOwnerPolicy6V1 as Prefix,
};
use fe2o3_pliron::ProductionConditionalAggregateErrorV1;
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
};

/// Refusal while joining a live conditional source request to policy6 output.
#[derive(Debug)]
pub enum ProductionConditionalCheckedOutputErrorV1 {
    /// An original phase account could not pay for the check or its scratch.
    Resource(Resource),
    /// The sealed optimization prefix did not replay against the bound input.
    Prefix(CanonicalPolicy6OptimizationErrorV1),
    /// Conditional coverage could not be derived from the actual graph.
    Coverage(ConditionalTotalViewErrorV1),
    /// Source arguments or output correspondence did not match the request.
    Source(ProductionConditionalContinuationErrorV1),
    /// The borrowed ranked graph no longer matched its original owner.
    Graph(ProductionConditionalAggregateErrorV1),
    /// A required identity, occurrence or runtime premise differed.
    Mismatch(&'static str),
}
type Error = ProductionConditionalCheckedOutputErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional N-to-I agreement: {self:?}")
    }
}
impl std::error::Error for Error {}

const SCRATCH: usize = 4 * size_of::<Facts<'static>>() + size_of::<Error>() + 1024;

#[path = "production_conditional_checked_tail_v1.rs"]
mod tail;
pub use tail::ProductionConditionalCheckedTailErrorV1;

#[path = "production_conditional_checked_final_v1.rs"]
mod final_output;
pub use final_output::ProductionConditionalCheckedFinalErrorV1;

impl ProductionSourceBoundConditionalAggregateRequestV1<'_> {
    /// Checks the actual sealed B/C/S/O/I against this live source request.
    /// The coordinate receipt must borrow this exact N. The backend separately
    /// checks its exact target metadata, genuine proof and retained contract.
    /// Source/arena/proof remain paid on their ORIGINAL source account; B and
    /// the entire prefix remain paid on their distinct ORIGINAL target account.
    /// Neither account is reconstructed or transferred by this call. Added
    /// scratch is released on return/unwind, never work or refusal history.
    ///
    /// No graph is copied and no evidence owner or authority is returned. I is
    /// not native history F; the ordinary conditional-finalizer gate stays shut.
    ///
    /// ```compile_fail
    /// use fe2o3_lower_mir_kernel::{ProductionSourceBoundConditionalAggregateRequestV1 as Request,
    ///     ProductionFormalMemoryOwnerV1};
    /// use fe2o3_kernel_analysis::CheckedCanonicalKirCoordinatePreservationV1 as Coordinates;
    /// use fe2o3_kernel_opt::CheckedCanonicalKernelIrOwnerPolicy6V1 as Prefix;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn promote(request: &Request<'_>, coordinates: &Coordinates<'_, '_>, prefix: &Prefix,
    ///     target: &mut Budget<'_>, source: &mut Budget<'_>) -> ProductionFormalMemoryOwnerV1 {
    ///     request.check_policy6_output_v1(coordinates, prefix, target, source).unwrap()
    /// }
    /// ```
    #[allow(clippy::result_large_err)]
    pub fn check_policy6_output_v1(
        &self,
        coordinates: &Coordinates<'_, '_>,
        checked: &Prefix,
        target: &mut Budget<'_>,
        source: &mut Budget<'_>,
    ) -> Result<()> {
        scoped(source, |source| {
            scoped(target, |target| {
                source.charge_work(4)?;
                if source.work_ledger_identity_v1() == target.work_ledger_identity_v1()
                    || !std::ptr::eq(self.source().executable(), coordinates.input())
                {
                    return Err(Error::Mismatch(
                        "original source and distinct phase accounts",
                    ));
                }
                if source.storage() < self.source().retained_analysis_storage_v1()
                    || target.storage()
                        < checked
                            .retained_storage()
                            .checked_add(coordinates.output().canonical().canonical_bytes().len())
                            .ok_or(Resource::Arithmetic)?
                {
                    return Err(Resource::Accounting.into());
                }
                source.reserve_storage(SCRATCH)?;
                target.reserve_storage(SCRATCH)?;
                self.pliron_input()
                    .require_current_graph_v1(source)
                    .map_err(Error::Graph)?;
                checked
                    .replay(coordinates.output(), target)
                    .map_err(Error::Prefix)?;
                let kernel = source_kernel(self, source)?;
                let original = facts(coordinates.input(), kernel, source)?;
                let binding = self
                    .source()
                    .bind_conditional_output_v1(original, source)
                    .map_err(|e| {
                        Error::Source(ProductionConditionalContinuationErrorV1::Binding(e))
                    })?;
                let output = facts(checked.owner(), kernel, target)?;
                super::require_conditional_argument_rows_v1(
                    self.arguments(),
                    self.pliron_input(),
                    source,
                )
                .map_err(Error::Source)?;
                occurrences::check(self, binding.coverage(), &output, checked, source)?;
                self.pliron_input()
                    .require_current_graph_v1(source)
                    .map_err(Error::Graph)
            })
        })
    }
}

fn source_kernel<'a>(
    request: &ProductionSourceBoundConditionalAggregateRequestV1<'a>,
    budget: &mut Budget<'_>,
) -> Result<&'a fe2o3_kernel_ir::KernelId> {
    let source = request.source();
    let semantic = source.semantic_ssa.source_semantic();
    let name = request.pliron_input().kernel().function_name();
    let mut selected = None;
    for row in source.correspondence.lowered_functions() {
        budget.charge_work(3)?;
        if row.role() != super::SemanticKirFunctionRoleV1::KernelEntry {
            continue;
        }
        let declaration = semantic
            .functions()
            .get(row.correspondence_owner().index() as usize)
            .and_then(|function| function.kernel_entry())
            .ok_or(Error::Mismatch("source kernel declaration"))?;
        let symbol = declaration.export_symbol().as_bytes();
        budget.charge_work(
            symbol
                .len()
                .checked_add(name.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if symbol == name.as_bytes() && selected.replace(row).is_some() {
            return Err(Error::Mismatch("unique source kernel correspondence"));
        }
    }
    let selected = selected.ok_or(Error::Mismatch("source kernel correspondence"))?;
    let mut kernel = None;
    for row in &source.executable().module().kernels {
        budget.charge_work(
            row.entry
                .as_str()
                .len()
                .checked_add(selected.kernel_ir_function().as_str().len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if &row.entry == selected.kernel_ir_function() && kernel.replace(&row.id).is_some() {
            return Err(Error::Mismatch("unique canonical source kernel"));
        }
    }
    kernel.ok_or(Error::Mismatch("canonical source kernel"))
}

fn facts<'a>(
    owner: &'a Graph,
    kernel: &fe2o3_kernel_ir::KernelId,
    budget: &mut Budget<'_>,
) -> Result<Facts<'a>> {
    match derive_conditional_total_view_from_verified_v1(
        owner.verified_module_ref_v1(),
        kernel,
        budget,
    )
    .map_err(Error::Coverage)?
    {
        fe2o3_kernel_ir::ConditionalTotalViewAnalysisV1::Established(facts) => Ok(facts),
        fe2o3_kernel_ir::ConditionalTotalViewAnalysisV1::Unsupported(_) => {
            Err(Error::Mismatch("conditional canonical coverage"))
        }
    }
}

fn scoped<T>(budget: &mut Budget<'_>, run: impl FnOnce(&mut Budget<'_>) -> Result<T>) -> Result<T> {
    scoped_resource(budget, run)
}

fn scoped_resource<'w, T, E: From<Resource>>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> std::result::Result<T, E>,
) -> std::result::Result<T, E> {
    let floor = budget.storage();
    let account = budget.work_ledger_identity_v1();
    let address = std::ptr::from_mut(budget);
    let result = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let cleanup =
        if std::ptr::from_mut(budget) != address || budget.work_ledger_identity_v1() != account {
            Err(Resource::Accounting)
        } else {
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)
                .and_then(|delta| budget.release_storage(delta))
        };
    match result {
        Ok(value) => {
            cleanup?;
            value
        }
        Err(payload) => {
            let _ = cleanup;
            resume_unwind(payload)
        }
    }
}

#[path = "production_conditional_checked_relations_v1.rs"]
mod occurrences;

#[cfg(test)]
#[path = "production_conditional_checked_output_v1_tests.rs"]
mod tests;
