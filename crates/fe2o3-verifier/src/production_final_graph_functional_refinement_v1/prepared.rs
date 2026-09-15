use fe2o3_kernel_ir::{Kernel, VerifiedCanonicalKernelIrV13};
use fe2o3_pliron::ProductionFinalGraphTargetContractV1;

use super::theorem::{CanonicalFinalKirSubjectsV1, ExecutedFinalKirTheoremV1};
use super::*;

type Failure = ProductionFinalGraphFunctionalRefinementErrorV1;

/// Exact canonical subject and generated theorem, before the mandatory live
/// graph schedule. Construction neither acquires a runtime nor executes Verus.
/// The target records are checked metadata, not proof of target closure.
///
/// ```compile_fail
/// use fe2o3_verifier::ProductionPreparedFinalGraphFunctionalSubjectV1;
/// fn needs_clone<T: Clone>() {}
/// needs_clone::<ProductionPreparedFinalGraphFunctionalSubjectV1>();
/// ```
#[must_use = "dropping this subject abandons its prepared functional theorem"]
pub struct ProductionPreparedFinalGraphFunctionalSubjectV1 {
    theorem: PreparedFinalKirTheoremV1,
    target: ProductionFinalGraphTargetContractV1,
}

impl fmt::Debug for ProductionPreparedFinalGraphFunctionalSubjectV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionPreparedFinalGraphFunctionalSubjectV1")
            .field("subjects", &self.theorem.subjects)
            .field("generated_source", &self.generated_source())
            .finish_non_exhaustive()
    }
}

impl ProductionPreparedFinalGraphFunctionalSubjectV1 {
    pub fn try_new(
        source: &VerifiedCanonicalKernelIrV13,
        final_graph: &VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
        target: &ProductionFinalGraphTargetContractV1,
    ) -> Result<Self, Failure> {
        require_target_subject(source, final_graph, final_epoch, target)?;
        // Reconstruct through the checked constructor; retain every decision,
        // not just the caller's closure identity.
        let target = ProductionFinalGraphTargetContractV1::try_new(
            final_graph,
            final_epoch,
            *target.neutral_graph(),
            target.neutral_epoch(),
            target.closure_identity(),
            target.model(),
            target.decisions().iter().cloned(),
        )
        .map_err(Failure::TargetContract)?;
        let theorem = PreparedFinalKirTheoremV1::try_new(source, final_graph, final_epoch)?;
        Ok(Self { theorem, target })
    }

    pub fn source_canonical(&self) -> &VerifiedCanonicalKernelIrV13 {
        self.theorem.subjects.source()
    }

    pub fn final_canonical(&self) -> &VerifiedCanonicalKernelIrV13 {
        self.theorem.subjects.final_graph()
    }

    pub fn final_epoch(&self) -> u64 {
        self.theorem.subjects.final_epoch()
    }

    pub fn target_contract(&self) -> &ProductionFinalGraphTargetContractV1 {
        &self.target
    }

    pub fn source_kernels(&self) -> &[Kernel] {
        self.theorem.subjects.source_kernels()
    }

    pub fn final_kernels(&self) -> &[Kernel] {
        self.theorem.subjects.final_kernels()
    }

    /// Generated content identity only, not an executed proof receipt.
    pub fn generated_source(&self) -> DigestV1 {
        self.theorem.generated_source()
    }

    pub fn require_exact_subject(
        &self,
        source: &VerifiedCanonicalKernelIrV13,
        final_graph: &VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
        target: &ProductionFinalGraphTargetContractV1,
    ) -> Result<(), Failure> {
        self.theorem
            .subjects
            .require_exact(source, final_graph, final_epoch)?;
        require_exact_target(&self.target, target)
    }

    /// Consumes the source proof as well as this prepared theorem. The source
    /// owner is replayed and joined before the sole runtime execution call.
    /// This does not establish completeness of the caller's source-proof roster.
    pub fn execute_effect_ir_derived(
        self,
        runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
        semantic_kir: &ProductionSemanticKirOwnerV1,
        verified: ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3,
        timeout_seconds: u32,
    ) -> Result<ProductionPreparedFinalGraphFunctionalExecutionV1, Failure> {
        require_target_subject(
            self.source_canonical(),
            self.final_canonical(),
            self.final_epoch(),
            &self.target,
        )?;
        let execution = self.theorem.execute(
            runtime,
            semantic_kir,
            verified.derivation(),
            verified.per_compilation_verus_execution(),
            self.target.closure_identity(),
            timeout_seconds,
        )?;
        Ok(ProductionPreparedFinalGraphFunctionalExecutionV1 {
            execution,
            target: self.target,
            verified,
        })
    }

    pub const fn authenticates_verus_execution(&self) -> bool {
        false
    }

    pub const fn grants_final_graph_verification_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Retains both executed theorems and the exact prepared canonical/target
/// subject. It is deliberately not a final-graph functional custody token.
/// Completion requires an independently verified live graph; this type cannot
/// construct one or bypass its mandatory schedule.
///
/// ```compile_fail
/// use fe2o3_verifier::ProductionPreparedFinalGraphFunctionalExecutionV1;
/// fn needs_clone<T: Clone>() {}
/// needs_clone::<ProductionPreparedFinalGraphFunctionalExecutionV1>();
/// ```
#[must_use = "retain this execution through the later exact final-graph schedule and roster join"]
pub struct ProductionPreparedFinalGraphFunctionalExecutionV1 {
    execution: ExecutedFinalKirTheoremV1,
    target: ProductionFinalGraphTargetContractV1,
    verified: ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3,
}

impl fmt::Debug for ProductionPreparedFinalGraphFunctionalExecutionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionPreparedFinalGraphFunctionalExecutionV1")
            .field("subjects", &self.execution.subjects)
            .field("obligation", &self.obligation())
            .finish_non_exhaustive()
    }
}

impl ProductionPreparedFinalGraphFunctionalExecutionV1 {
    /// Transfers the existing receipts into final functional custody only
    /// after the borrowed owner has passed its actual mandatory schedule.
    /// The source semantic owner was replayed before execution; its immutable
    /// canonical snapshot and source proof are revalidated here. No theorem
    /// is regenerated or executed, and the graph remains owned by the caller.
    /// This per-root conversion does not establish the caller's module roster.
    ///
    /// An unexecuted preparation cannot take this transition:
    /// ```compile_fail
    /// use fe2o3_pliron::ProductionVerifiedFinalGraphV1;
    /// use fe2o3_verifier::ProductionPreparedFinalGraphFunctionalSubjectV1;
    /// fn incomplete(subject: ProductionPreparedFinalGraphFunctionalSubjectV1,
    ///               graph: &mut ProductionVerifiedFinalGraphV1) {
    ///     let _ = subject.complete_with_verified_graph(graph);
    /// }
    /// ```
    pub fn complete_with_verified_graph(
        self,
        final_graph: &mut ProductionVerifiedFinalGraphV1,
    ) -> Result<ProductionFinalGraphFunctionalRefinementExecutionV2, Failure> {
        require_verified_graph_subject(&self.execution.subjects, &self.target, final_graph)?;
        self.execution.require_exact_proof(
            self.verified.derivation(),
            self.verified.per_compilation_verus_execution(),
            final_graph.target_contract().closure_identity(),
        )?;
        final_graph.revalidate_live().map_err(Failure::FinalGraph)?;
        Ok(ProductionFinalGraphFunctionalRefinementExecutionV2 {
            verified: self.verified,
            final_execution: self.execution.final_execution,
            _staging_policy: self.execution.staging_policy,
            report: self.execution.report,
        })
    }

    pub fn source_canonical(&self) -> &VerifiedCanonicalKernelIrV13 {
        self.execution.subjects.source()
    }

    pub fn final_canonical(&self) -> &VerifiedCanonicalKernelIrV13 {
        self.execution.subjects.final_graph()
    }

    pub fn final_epoch(&self) -> u64 {
        self.execution.subjects.final_epoch()
    }

    pub fn target_contract(&self) -> &ProductionFinalGraphTargetContractV1 {
        &self.target
    }

    pub fn source_kernels(&self) -> &[Kernel] {
        self.execution.subjects.source_kernels()
    }

    pub fn final_kernels(&self) -> &[Kernel] {
        self.execution.subjects.final_kernels()
    }

    pub fn verified(&self) -> &ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3 {
        &self.verified
    }

    pub fn obligation(&self) -> DigestV1 {
        self.execution.report.obligation()
    }

    pub fn binding(&self) -> FunctionalRefinementBindingV2 {
        self.execution.report.binding()
    }

    pub fn final_signed_receipt_wire(&self) -> &[u8] {
        self.execution.final_execution.wire()
    }

    pub fn final_receipt_verifying_key(&self) -> &[u8; 32] {
        self.execution.final_execution.verifying_key()
    }

    pub fn retains_strictly_imported_final_graph_receipt(&self) -> bool {
        self.execution
            .final_execution
            .proof()
            .signature_and_policy_verified()
    }

    /// Checks only the immutable prepared subject. The caller must separately
    /// complete mandatory live verification, exact target custody, and its
    /// authenticated source-root roster before any final handoff.
    pub fn require_exact_subject(
        &self,
        source: &VerifiedCanonicalKernelIrV13,
        final_graph: &VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
        target: &ProductionFinalGraphTargetContractV1,
    ) -> Result<(), Failure> {
        self.execution
            .subjects
            .require_exact(source, final_graph, final_epoch)?;
        require_exact_target(&self.target, target)
    }

    pub const fn grants_final_graph_verification_authority(&self) -> bool {
        false
    }

    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

// This comparison grants no proof authority by itself. Keeping it separate
// permits real-schedule owner tests without manufacturing runtime receipts.
pub(super) fn require_verified_graph_subject(
    subjects: &CanonicalFinalKirSubjectsV1,
    target: &ProductionFinalGraphTargetContractV1,
    final_graph: &mut ProductionVerifiedFinalGraphV1,
) -> Result<(), Failure> {
    final_graph.revalidate_live().map_err(Failure::FinalGraph)?;
    subjects.require_exact(
        subjects.source(),
        final_graph.canonical(),
        final_graph.final_epoch(),
    )?;
    require_target_subject(
        subjects.source(),
        final_graph.canonical(),
        final_graph.final_epoch(),
        final_graph.target_contract(),
    )?;
    require_exact_target(target, final_graph.target_contract())?;
    let report = final_graph.report();
    if report.final_graph() != subjects.final_graph().identity()
        || report.final_epoch() != subjects.final_epoch()
        || report.resources().final_graph() != subjects.final_graph().identity()
        || report.resources().final_epoch() != subjects.final_epoch()
        || report.resources().closure_identity() != target.closure_identity()
    {
        return Err(Failure::FinalGraphSubjectMismatch);
    }
    Ok(())
}

fn require_target_subject(
    source: &VerifiedCanonicalKernelIrV13,
    final_graph: &VerifiedCanonicalKernelIrV13,
    final_epoch: u64,
    target: &ProductionFinalGraphTargetContractV1,
) -> Result<(), Failure> {
    if target.neutral_graph() != source.identity()
        || target.final_graph() != final_graph.identity()
        || target.final_epoch() != final_epoch
    {
        return Err(Failure::TargetContractSubjectMismatch);
    }
    if target.closure_identity() == [0; 32] {
        return Err(Failure::TargetClosureMissing);
    }
    Ok(())
}

fn require_exact_target(
    retained: &ProductionFinalGraphTargetContractV1,
    candidate: &ProductionFinalGraphTargetContractV1,
) -> Result<(), Failure> {
    if retained.final_graph() != candidate.final_graph()
        || retained.final_epoch() != candidate.final_epoch()
        || retained.neutral_graph() != candidate.neutral_graph()
        || retained.neutral_epoch() != candidate.neutral_epoch()
        || retained.closure_identity() != candidate.closure_identity()
        || retained.model() != candidate.model()
        || retained.decisions() != candidate.decisions()
    {
        return Err(Failure::TargetContractSubjectMismatch);
    }
    Ok(())
}
