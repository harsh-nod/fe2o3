//! Move-only receipt custody. Production source and ledger custody stay in the
//! backend owner; a receipt, policy or report alone cannot establish either.
use super::*;
use fe2o3_functional_proof::{
    FunctionalRefinementImportExpectationV2, FunctionalRefinementImportPolicyV2,
    FunctionalRefinementReceiptImporterV2,
};
use fe2o3_pliron::{
    OperationGraphSnapshotV1, ProductionExactGraphIdentityV1, ProductionRefinementStagingPolicyV2,
};

pub(super) const PREPARATION_STORAGE: usize = 3 * SOURCE_LIMIT
    + crate::functional_refinement_receipt_v2::MAX_FUNCTIONAL_REFINEMENT_FORMULA_NODES_V2
        * std::mem::size_of::<u32>();
pub(super) const RETAINED_STORAGE: usize =
    std::mem::size_of::<RetainedProductionConditionalFormulaV1>();

/// Actual signed receipt and accepted import policy for one retained graph.
///
/// This is not CPU-source, original-ledger, lowering or launch authority. The
/// private production owner retains those dependencies and lends its original
/// account for every consumption. No address token is saved across callbacks.
/// Dropping this value destroys its receipt but does not debit a caller's budget;
/// its enclosing accounting owner releases the reservation after destruction.
///
/// ```no_run
/// use fe2o3_verifier::RetainedProductionConditionalFormulaV1;
/// fn inspect(proof: &RetainedProductionConditionalFormulaV1) {
///     let _ = proof.report();
///     let _ = proof.retained_storage_v1();
/// }
/// ```
/// ```compile_fail,E0599
/// use fe2o3_verifier::RetainedProductionConditionalFormulaV1;
/// fn duplicate(proof: RetainedProductionConditionalFormulaV1) { let _ = proof.clone(); }
/// ```
/// ```compile_fail,E0277
/// use fe2o3_verifier::RetainedProductionConditionalFormulaV1;
/// use fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1;
/// fn ordinary(proof: RetainedProductionConditionalFormulaV1) -> ProductionFormalMemoryOwnerV1 {
///     proof.into()
/// }
/// ```
#[must_use = "retain with the original production account or destroy before releasing its charge"]
pub struct RetainedProductionConditionalFormulaV1 {
    pub(super) execution: ProductionConditionalFormulaExecutionV1,
    policy: FunctionalRefinementImportPolicyV2,
    subject: RetainedSubjectV1,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub(super) struct RetainedSubjectV1 {
    aggregate: DigestV1,
    source: DigestV1,
    graph: ProductionExactGraphIdentityV1,
    snapshot: OperationGraphSnapshotV1,
}
impl RetainedSubjectV1 {
    pub(super) fn from_request(
        request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
    ) -> Self {
        let input = request.pliron_input();
        Self {
            aggregate: input.identity(),
            source: input.source_semantic_identity(),
            graph: input.exact_graph_identity(),
            snapshot: input.graph_snapshot(),
        }
    }
}

pub(super) fn accepted_policy(
    retained: &RetainedImportedFunctionalRefinementReceiptV2,
    accepted: &ProductionRefinementStagingPolicyV2,
) -> Result<FunctionalRefinementImportPolicyV2, Error> {
    let policy = FunctionalRefinementImportPolicyV2::new(
        *retained.verifying_key(),
        accepted.toolchain(),
        FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron,
    )
    .map_err(|_| Error::Subject("conditional retained import policy"))?;
    if !accepted.accepts_signer(policy.signer_identity())
        || policy.signer_identity() != retained.proof().signer_identity()
        || policy.toolchain() != retained.proof().toolchain()
    {
        return Err(Error::Subject("conditional accepted policy substitution"));
    }
    Ok(policy)
}

pub(super) fn retain_checked(
    execution: ProductionConditionalFormulaExecutionV1,
    policy: FunctionalRefinementImportPolicyV2,
    request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
) -> RetainedProductionConditionalFormulaV1 {
    RetainedProductionConditionalFormulaV1 {
        execution,
        policy,
        subject: RetainedSubjectV1::from_request(request),
    }
}

/// Executes the same preparation/import path as the borrowed wrapper, retaining
/// its fixed-size receipt and policy reservation on the caller's original phase.
/// This function cannot authenticate that phase's custody: the backend owns it.
pub fn execute_and_retain_conditional_ranked_formula_v1(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
    budget: &mut Budget<'_>,
    timeout_seconds: u32,
) -> Result<RetainedProductionConditionalFormulaV1, Error> {
    retain_reservation(budget, RETAINED_STORAGE, |budget| {
        execute_formula(runtime, request, budget, timeout_seconds)
    })
}

impl RetainedProductionConditionalFormulaV1 {
    pub const fn report(&self) -> ProductionConditionalFormulaReportV1 {
        self.execution.report()
    }
    pub const fn retained_storage_v1(&self) -> usize {
        RETAINED_STORAGE
    }

    /// Independently prepares the same statement and reimports its exact signed
    /// receipt under the retained policy. CPU-MIR replay and original-account
    /// custody must surround this call in the private backend consumer.
    ///
    /// ```compile_fail
    /// use fe2o3_verifier::RetainedProductionConditionalFormulaV1;
    /// use fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1;
    /// fn escape(proof: &RetainedProductionConditionalFormulaV1,
    ///     request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
    ///     budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
    ///     let escaped = proof.with_replayed_request_v1(request, budget, |execution, _| execution).unwrap();
    ///     let _ = escaped.signed_receipt_wire();
    /// }
    /// ```
    pub fn with_replayed_request_v1<R>(
        &self,
        request: &ProductionSourceBoundConditionalAggregateRequestV1<'_>,
        budget: &mut Budget<'_>,
        consume: impl for<'proof> FnOnce(
            &'proof ProductionConditionalFormulaExecutionV1,
            &mut Budget<'_>,
        ) -> R,
    ) -> Result<R, Error> {
        budget.charge_work(1)?;
        if budget.storage() < RETAINED_STORAGE {
            return Err(Resource::Accounting.into());
        }
        current(request.pliron_input(), budget)?;
        if self.subject != RetainedSubjectV1::from_request(request) {
            return Err(Error::Subject(
                "conditional retained source/graph/root substitution",
            ));
        }
        with_scratch(budget, PREPARATION_STORAGE, |budget| {
            let expected = prepare(request, budget)?;
            self.reimport(&expected, budget)?;
            current(request.pliron_input(), budget)?;
            let result = consume(&self.execution, budget);
            current(request.pliron_input(), budget)?;
            Ok(result)
        })
    }

    fn reimport(&self, expected: &Prepared, budget: &mut Budget<'_>) -> Result<(), Error> {
        reimport(
            self.report(),
            &self.policy,
            self.execution.signed_receipt_wire(),
            expected,
            budget,
        )
    }
}

pub(super) fn reimport(
    report: ProductionConditionalFormulaReportV1,
    policy: &FunctionalRefinementImportPolicyV2,
    wire: &[u8],
    expected: &Prepared,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(wire.len())?;
    if report.binding != expected.binding
        || report.generated_source != expected.generated_source
        || report.statement != expected.binding.normalized_obligation_effect_ir_hash()
    {
        return Err(Error::Subject("conditional retained theorem substitution"));
    }
    let proof = import_expected(expected.binding, policy, wire)?;
    require_imported_identity(report, policy, &proof)
}

pub(super) fn import_expected(
    binding: FunctionalRefinementBindingV2,
    policy: &FunctionalRefinementImportPolicyV2,
    wire: &[u8],
) -> Result<ImportedFunctionalRefinementProofV2, Error> {
    // The existing strict importer bounds this singleton's internal storage.
    let mut importer = FunctionalRefinementReceiptImporterV2::new(policy.clone(), 1)
        .map_err(|_| Error::Subject("conditional retained import policy"))?;
    importer
        .import(FunctionalRefinementImportExpectationV2::new(binding), wire)
        .map_err(|_| Error::Subject("conditional retained signature/policy"))
}

pub(super) fn require_imported_identity(
    report: ProductionConditionalFormulaReportV1,
    policy: &FunctionalRefinementImportPolicyV2,
    proof: &ImportedFunctionalRefinementProofV2,
) -> Result<(), Error> {
    if proof.execution_identity() != report.execution
        || proof.receipt_identity().digest() != report.receipt
        || proof.signer_identity() != policy.signer_identity()
        || proof.toolchain() != policy.toolchain()
        || proof.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron
        || !proof.signature_and_policy_verified()
    {
        return Err(Error::Subject("conditional retained execution identity"));
    }
    Ok(())
}

// Reservation transfer is local to one active borrow. It never saves an
// address-based ledger identity as persistent receipt or account authority.
fn retain_reservation<T>(
    budget: &mut Budget<'_>,
    bytes: usize,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    retain_reservation_using(budget, bytes, run)
}

pub(super) fn retain_reservation_using<T, E: From<Error>>(
    budget: &mut Budget<'_>,
    bytes: usize,
    run: impl FnOnce(&mut Budget<'_>) -> Result<T, E>,
) -> Result<T, E> {
    use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
    budget.charge_work(1).map_err(Error::from)?;
    let account = budget.work_ledger_identity_v1();
    budget.reserve_storage(bytes).map_err(Error::from)?;
    let protected = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| run(budget)));
    let valid = budget.work_ledger_identity_v1() == account && budget.storage() >= protected;
    match outcome {
        Ok(Ok(value)) if valid => Ok(value),
        Ok(result) => {
            let result = result.map(|value| {
                drop(value);
            });
            if !valid {
                return Err(Error::from(Resource::Accounting).into());
            }
            budget.release_storage(bytes).map_err(Error::from)?;
            result.and(Err(Error::from(Resource::Accounting).into()))
        }
        Err(panic) => {
            if valid {
                let _ = budget.release_storage(bytes);
            }
            resume_unwind(panic)
        }
    }
}

#[cfg(test)]
#[path = "conditional_ranked_formula_retention_v1_tests.rs"]
mod tests;
