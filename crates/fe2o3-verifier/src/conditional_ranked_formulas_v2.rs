//! CPU-content-bound conditional statements; never source-origin or native authority.
use super::*;
use crate::conditional_reference_v1::{
    ConditionalReferenceErrorV1, ConditionalReferenceInputV1,
    with_source_bound_cpu_correspondence_v1,
};
use crate::functional_refinement_receipt_v2::{
    InertFunctionalRefinementReceiptSignatureV2, import_and_retain_functional_refinement_receipt_v2,
};
use crate::portable_reference_v1::{
    ReferenceReplayInputV1,
    codec::{
        DecodedNativeCpuInputV1, NativeCpuCodecErrorV1, NativeCpuInputV1,
        with_encoded_native_cpu_input_v1,
    },
};

type Request<'a> = ProductionSourceBoundConditionalAggregateRequestV1<'a>;
type Error = ProductionConditionalFormulaErrorV2;
const DOMAIN: &[u8] = b"FE2O3/CONDITIONAL-MEMORY-TRANSITION/V2/LE/SHARED-IEEE/CPU-SOURCE\0";
const RETAINED_STORAGE: usize = std::mem::size_of::<RetainedProductionConditionalFormulaV2>();

/// Version-specific refusal; none of these errors carries proof authority.
#[derive(Debug)]
pub enum ProductionConditionalFormulaErrorV2 {
    Formula(ProductionConditionalFormulaErrorV1),
    Codec(NativeCpuCodecErrorV1),
    Correspondence(ConditionalReferenceErrorV1),
    Subject(&'static str),
}
impl From<ProductionConditionalFormulaErrorV1> for Error {
    fn from(value: ProductionConditionalFormulaErrorV1) -> Self {
        Self::Formula(value)
    }
}
impl From<Resource> for Error {
    fn from(value: Resource) -> Self {
        Self::Formula(value.into())
    }
}
impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "conditional CPU-bound formula V2: {self:?}")
    }
}
impl std::error::Error for Error {}

/// Copyable diagnostic identities, not a proof or a V1 report conversion.
///
/// ```compile_fail
/// use fe2o3_verifier::{ProductionConditionalFormulaReportV2, RetainedProductionConditionalFormulaV2};
/// fn install(report: ProductionConditionalFormulaReportV2) -> RetainedProductionConditionalFormulaV2 { report.into() }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionConditionalFormulaReportV2 {
    formula: ProductionConditionalFormulaReportV1,
    cpu_input: DigestV1,
}
impl ProductionConditionalFormulaReportV2 {
    pub const fn statement_identity(self) -> DigestV1 {
        self.formula.statement_identity()
    }
    pub const fn generated_source_identity(self) -> DigestV1 {
        self.formula.generated_source_identity()
    }
    pub const fn binding(self) -> FunctionalRefinementBindingV2 {
        self.formula.binding()
    }
    pub const fn execution_identity(self) -> DigestV1 {
        self.formula.execution_identity()
    }
    pub const fn receipt_identity(self) -> DigestV1 {
        self.formula.receipt_identity()
    }
    pub const fn cpu_input_commitment(self) -> DigestV1 {
        self.cpu_input
    }
}

/// Lent only after fresh CPU/source/statement replay and strict receipt import.
pub struct ProductionConditionalFormulaExecutionV2 {
    report: ProductionConditionalFormulaReportV2,
    retained: RetainedImportedFunctionalRefinementReceiptV2,
}
impl ProductionConditionalFormulaExecutionV2 {
    pub const fn report(&self) -> ProductionConditionalFormulaReportV2 {
        self.report
    }
    pub const fn signed_receipt_wire(&self) -> &[u8] {
        self.retained.wire()
    }
    pub const fn receipt_verifying_key(&self) -> &[u8; 32] {
        self.retained.verifying_key()
    }
}

/// Move-only imported receipt custody for one exact source/graph and CPU input.
/// The caller retains its original accounting phase and source authenticity.
/// Dropping this owner does not release that phase's storage reservation.
///
/// ```compile_fail
/// use fe2o3_verifier::RetainedProductionConditionalFormulaV2;
/// fn duplicate(p: RetainedProductionConditionalFormulaV2) { let _ = p.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::{RetainedProductionConditionalFormulaV1, RetainedProductionConditionalFormulaV2};
/// fn upgrade(p: RetainedProductionConditionalFormulaV1) -> RetainedProductionConditionalFormulaV2 { p.into() }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::{RetainedProductionConditionalFormulaV1, RetainedProductionConditionalFormulaV2};
/// fn downgrade(p: RetainedProductionConditionalFormulaV2) -> RetainedProductionConditionalFormulaV1 { p.into() }
/// ```
/// ```compile_fail
/// use fe2o3_verifier::RetainedProductionConditionalFormulaV2;
/// use fe2o3_lower_mir_kernel::ProductionFormalMemoryOwnerV1;
/// fn promote(p: RetainedProductionConditionalFormulaV2) -> ProductionFormalMemoryOwnerV1 { p.into() }
/// ```
#[must_use = "retain on the original account or drop before releasing its charge"]
pub struct RetainedProductionConditionalFormulaV2 {
    execution: ProductionConditionalFormulaExecutionV2,
    policy: FunctionalRefinementImportPolicyV2,
    subject: retention::RetainedSubjectV1,
}

/// Executes a fresh V2 statement over the same borrowed input encoded and joined.
/// No raw commitment, generated proof text or V1 execution is accepted.
pub fn execute_and_retain_conditional_ranked_formula_v2(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    request: &Request<'_>,
    input: NativeCpuInputV1<'_>,
    budget: &mut Budget<'_>,
    timeout_seconds: u32,
) -> Result<RetainedProductionConditionalFormulaV2, Error> {
    retention::retain_reservation_using(budget, RETAINED_STORAGE, |budget| {
        with_live_cpu(request, input, budget, |request, cpu_input, budget| {
            with_scratch_using(budget, retention::PREPARATION_STORAGE, |budget| {
                let prepared = prepare_v2(request, cpu_input, budget)?;
                let (formula, retained, policy) =
                    execute_prepared(runtime, request, prepared, budget, timeout_seconds)?;
                require_policy(request, &policy, budget)?;
                Ok(retain(request, formula, cpu_input, retained, policy))
            })
        })
    })
}

/// Reprepares from this same decoded CPU owner and imports actual signed bytes.
/// Accepted policy is external; the signature's embedded key cannot select it.
/// This runs no protected execution and constructs no source-origin authority.
/// The enclosing B1 decode/lower callbacks must also finish before installation.
///
/// ```compile_fail
/// use fe2o3_verifier::{import_and_retain_conditional_ranked_formula_v2 as import, InertFunctionalRefinementReceiptSignatureV2 as Signature};
/// use fe2o3_functional_proof::FunctionalRefinementImportPolicyV2 as Policy;
/// use fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1 as Request;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn raw_digest(r: &Request<'_>, hash: &[u8; 32], s: &Signature, p: &Policy, b: &mut Budget<'_>) {
///     let _ = import(r, hash, s, p, b);
/// }
/// ```
pub fn import_and_retain_conditional_ranked_formula_v2(
    request: &Request<'_>,
    input: &DecodedNativeCpuInputV1,
    signature: &InertFunctionalRefinementReceiptSignatureV2,
    accepted: &FunctionalRefinementImportPolicyV2,
    budget: &mut Budget<'_>,
) -> Result<RetainedProductionConditionalFormulaV2, Error> {
    retention::retain_reservation_using(budget, RETAINED_STORAGE, |budget| {
        with_decoded_cpu(request, input, budget, |request, cpu_input, budget| {
            with_scratch_using(budget, retention::PREPARATION_STORAGE, |budget| {
                let prepared = prepare_v2(request, cpu_input, budget)?;
                require_policy(request, accepted, budget)?;
                budget.charge_work(signature.wire().len())?;
                let retained = import_and_retain_functional_refinement_receipt_v2(
                    prepared.binding,
                    signature,
                    accepted,
                )
                .map_err(|_| Error::Subject("conditional V2 signature/policy"))?;
                let proof = retained.proof();
                let formula = report_for_proof(prepared.binding, prepared.generated_source, proof);
                retention::require_imported_identity(formula, accepted, proof)?;
                current(request.pliron_input(), budget)?;
                Ok(retain(
                    request,
                    formula,
                    cpu_input,
                    retained,
                    accepted.clone(),
                ))
            })
        })
    })
}

impl RetainedProductionConditionalFormulaV2 {
    pub const fn report(&self) -> ProductionConditionalFormulaReportV2 {
        self.execution.report
    }
    pub const fn retained_storage_v2(&self) -> usize {
        RETAINED_STORAGE
    }
    /// Policy transport is inert; an independent caller must accept its provenance.
    pub const fn import_policy_v2(&self) -> &FunctionalRefinementImportPolicyV2 {
        &self.policy
    }

    /// Re-encodes and rejoins the same whole input before lending the V2 execution.
    ///
    /// ```compile_fail
    /// use fe2o3_verifier::{RetainedProductionConditionalFormulaV2, ProductionConditionalFormulaExecutionV2};
    /// use fe2o3_verifier::portable_reference_v1::codec::NativeCpuInputV1;
    /// use fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1 as Request;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape<'a>(p: &RetainedProductionConditionalFormulaV2, r: &Request<'_>,
    ///     input: NativeCpuInputV1<'_>, b: &mut Budget<'_>) -> &'a ProductionConditionalFormulaExecutionV2 {
    ///     p.with_replayed_request_v2(r, input, b, |proof, _| Ok(proof)).unwrap()
    /// }
    /// ```
    pub fn with_replayed_request_v2<R>(
        &self,
        request: &Request<'_>,
        input: NativeCpuInputV1<'_>,
        budget: &mut Budget<'_>,
        consume: impl for<'proof> FnOnce(
            &'proof ProductionConditionalFormulaExecutionV2,
            &mut Budget<'_>,
        ) -> Result<R, Error>,
    ) -> Result<R, Error> {
        self.require_reservation(budget)?;
        with_live_cpu(request, input, budget, |request, cpu_input, budget| {
            self.replay_checked(request, cpu_input, budget, consume)
        })
    }

    /// Uses this decoded owner's input and commitment together, without a clone.
    ///
    /// ```compile_fail
    /// use fe2o3_verifier::{RetainedProductionConditionalFormulaV2, ProductionConditionalFormulaExecutionV2};
    /// use fe2o3_verifier::portable_reference_v1::codec::DecodedNativeCpuInputV1;
    /// use fe2o3_lower_mir_kernel::ProductionSourceBoundConditionalAggregateRequestV1 as Request;
    /// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
    /// fn escape<'a>(p: &RetainedProductionConditionalFormulaV2, r: &Request<'_>,
    ///     input: &DecodedNativeCpuInputV1, b: &mut Budget<'_>) -> &'a ProductionConditionalFormulaExecutionV2 {
    ///     p.with_replayed_decoded_request_v2(r, input, b, |proof, _| Ok(proof)).unwrap()
    /// }
    /// ```
    pub fn with_replayed_decoded_request_v2<R>(
        &self,
        request: &Request<'_>,
        input: &DecodedNativeCpuInputV1,
        budget: &mut Budget<'_>,
        consume: impl for<'proof> FnOnce(
            &'proof ProductionConditionalFormulaExecutionV2,
            &mut Budget<'_>,
        ) -> Result<R, Error>,
    ) -> Result<R, Error> {
        self.require_reservation(budget)?;
        with_decoded_cpu(request, input, budget, |request, cpu_input, budget| {
            self.replay_checked(request, cpu_input, budget, consume)
        })
    }

    fn require_reservation(&self, budget: &mut Budget<'_>) -> Result<(), Error> {
        budget.charge_work(1)?;
        if budget.storage() < RETAINED_STORAGE {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }

    fn replay_checked<R>(
        &self,
        request: &Request<'_>,
        cpu_input: DigestV1,
        budget: &mut Budget<'_>,
        consume: impl for<'proof> FnOnce(
            &'proof ProductionConditionalFormulaExecutionV2,
            &mut Budget<'_>,
        ) -> Result<R, Error>,
    ) -> Result<R, Error> {
        if self.subject != retention::RetainedSubjectV1::from_request(request) {
            return Err(Error::Subject(
                "conditional V2 source/graph/root substitution",
            ));
        }
        require_commitment(self.report().cpu_input, cpu_input)?;
        with_scratch_using(budget, retention::PREPARATION_STORAGE, |budget| {
            let expected = prepare_v2(request, cpu_input, budget)?;
            require_policy(request, &self.policy, budget)?;
            retention::reimport(
                self.execution.report.formula,
                &self.policy,
                self.execution.signed_receipt_wire(),
                &expected,
                budget,
            )?;
            current(request.pliron_input(), budget)?;
            let result = consume(&self.execution, budget);
            current(request.pliron_input(), budget)?;
            result
        })
    }
}

fn retain(
    request: &Request<'_>,
    formula: ProductionConditionalFormulaReportV1,
    cpu_input: DigestV1,
    retained: RetainedImportedFunctionalRefinementReceiptV2,
    policy: FunctionalRefinementImportPolicyV2,
) -> RetainedProductionConditionalFormulaV2 {
    RetainedProductionConditionalFormulaV2 {
        execution: ProductionConditionalFormulaExecutionV2 {
            report: ProductionConditionalFormulaReportV2 { formula, cpu_input },
            retained,
        },
        policy,
        subject: retention::RetainedSubjectV1::from_request(request),
    }
}

fn require_policy(
    request: &Request<'_>,
    accepted: &FunctionalRefinementImportPolicyV2,
    budget: &mut Budget<'_>,
) -> Result<(), Error> {
    budget.charge_work(161)?;
    let [staging] = request
        .pliron_input()
        .retained_policy_checked_refinement_staging()
    else {
        return Err(Error::Subject("conditional V2 staging policy roster"));
    };
    if accepted.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron
        || accepted.toolchain() != staging.toolchain()
    {
        return Err(Error::Subject("conditional V2 accepted boundary/toolchain"));
    }
    Ok(())
}

fn reborrow<'a>(input: &NativeCpuInputV1<'a>) -> NativeCpuInputV1<'a> {
    NativeCpuInputV1 {
        association: input.association,
        kernel: input.kernel,
        reference: input.reference,
        replay: ReferenceReplayInputV1 {
            signature_preimage: input.replay.signature_preimage,
            effect_ir: input.replay.effect_ir,
            effect_ir_sha256: input.replay.effect_ir_sha256,
            observable_output_writes: input.replay.observable_output_writes,
        },
    }
}

fn with_live_cpu<R>(
    request: &Request<'_>,
    input: NativeCpuInputV1<'_>,
    budget: &mut Budget<'_>,
    run: impl FnOnce(&Request<'_>, DigestV1, &mut Budget<'_>) -> Result<R, Error>,
) -> Result<R, Error> {
    with_encoded_native_cpu_input_v1(reborrow(&input), budget, |_, commitment, budget| {
        with_cpu(request, input, commitment, budget, run)
    })
    .map_err(Error::Codec)?
}

fn with_decoded_cpu<R>(
    request: &Request<'_>,
    input: &DecodedNativeCpuInputV1,
    budget: &mut Budget<'_>,
    run: impl FnOnce(&Request<'_>, DigestV1, &mut Budget<'_>) -> Result<R, Error>,
) -> Result<R, Error> {
    with_cpu(
        request,
        input.input_v1(),
        input.commitment_v1(),
        budget,
        run,
    )
}

// This raw commitment is PRIVATE and supplied only by the paired codec paths.
fn with_cpu<R>(
    request: &Request<'_>,
    input: NativeCpuInputV1<'_>,
    commitment: [u8; 32],
    budget: &mut Budget<'_>,
    run: impl FnOnce(&Request<'_>, DigestV1, &mut Budget<'_>) -> Result<R, Error>,
) -> Result<R, Error> {
    current(request.pliron_input(), budget)?;
    let semantic = request.source().semantic_ssa().source_semantic();
    budget.charge_work(64)?;
    if input.association.semantic_mir_sha256 != *semantic.semantic_sha256().as_bytes()
        || input.association.semantic_mir_sha256
            != *request.pliron_input().source_semantic_identity().as_bytes()
    {
        return Err(Error::Subject("conditional V2 CPU source association"));
    }
    let mut root_found = false;
    for root in semantic.roots() {
        budget.charge_work(1)?;
        root_found |= root.index() == input.association.semantic_root;
    }
    if !root_found {
        return Err(Error::Subject("conditional V2 CPU root association"));
    }
    with_source_bound_cpu_correspondence_v1(
        request,
        ConditionalReferenceInputV1 {
            kernel: input.kernel,
            reference: input.reference,
            replay: input.replay,
        },
        input.association.semantic_root,
        budget,
        |cpu, budget| {
            cpu.require_subjects(budget)
                .map_err(Error::Correspondence)?;
            let result = run(
                cpu.request(),
                DigestV1::from_untrusted_bytes(commitment),
                budget,
            );
            current(cpu.request().pliron_input(), budget)?;
            result
        },
    )
    .map_err(Error::Correspondence)?
}

fn require_commitment(expected: DigestV1, actual: DigestV1) -> Result<(), Error> {
    if expected != actual {
        return Err(Error::Subject("conditional V2 CPU input substitution"));
    }
    Ok(())
}

fn prepare_v2(
    request: &Request<'_>,
    cpu_input: DigestV1,
    budget: &mut Budget<'_>,
) -> Result<Prepared, Error> {
    let mut prepared = prepare(request, budget)?;
    budget.charge_work(DOMAIN.len() + 7 * 32)?;
    let obligation = obligation_identity_v2(
        obligation_commitments(request.pliron_input(), prepared.generated_source),
        cpu_input,
    );
    prepared.binding = FunctionalRefinementBindingV2::from_subjects(
        request.pliron_input().reference_subjects(),
        obligation,
    )
    .map_err(|_| Error::Subject("conditional V2 formula binding"))?;
    Ok(prepared)
}

fn obligation_identity_v2(commitments: [DigestV1; 6], cpu_input: DigestV1) -> DigestV1 {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    for identity in commitments.into_iter().chain([cpu_input]) {
        hash.update(identity.as_bytes());
    }
    DigestV1::from_untrusted_bytes(hash.finalize().into())
}

#[cfg(test)]
#[path = "conditional_ranked_formula_cpu_v2_tests.rs"]
mod cpu_tests;
#[cfg(test)]
#[path = "conditional_ranked_formula_fixture_v2_tests.rs"]
mod fixtures;
#[cfg(test)]
#[path = "conditional_ranked_formula_import_v2_tests.rs"]
mod import_tests;
#[cfg(test)]
#[path = "conditional_ranked_formula_resources_v2_tests.rs"]
mod resource_tests;
