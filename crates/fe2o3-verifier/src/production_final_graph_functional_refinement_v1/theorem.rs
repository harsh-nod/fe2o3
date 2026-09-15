use fe2o3_kernel_ir::{Kernel, Module, VerifiedCanonicalKernelIrV13};

use super::*;
use crate::final_kir_output_equivalence_v1::generate_final_kir_output_equivalence_v1;
use crate::{
    CanonicalGeneratedVerusProofInputV3, ProductionIrDerivedFunctionalSemanticsV1,
    ProductionMirPlironPerCompilationVerusExecutionV1,
};

type Failure = ProductionFinalGraphFunctionalRefinementErrorV1;

/// Immutable bytes and decoded rosters, never a live-graph verification token.
#[derive(Debug)]
pub(super) struct CanonicalFinalKirSubjectsV1 {
    source: VerifiedCanonicalKernelIrV13,
    final_graph: VerifiedCanonicalKernelIrV13,
    final_epoch: u64,
    source_module: Module,
    final_module: Module,
}

impl CanonicalFinalKirSubjectsV1 {
    pub(super) fn try_new(
        source: &VerifiedCanonicalKernelIrV13,
        final_graph: &VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
    ) -> Result<Self, Failure> {
        source.revalidate().map_err(Failure::Canonical)?;
        final_graph.revalidate().map_err(Failure::Canonical)?;
        let (source, source_module) =
            VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
                source.canonical_bytes().to_vec(),
            )
            .map_err(Failure::Canonical)?;
        let (final_graph, final_module) =
            VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
                final_graph.canonical_bytes().to_vec(),
            )
            .map_err(Failure::Canonical)?;
        Ok(Self {
            source,
            final_graph,
            final_epoch,
            source_module,
            final_module,
        })
    }

    pub(super) fn source(&self) -> &VerifiedCanonicalKernelIrV13 {
        &self.source
    }

    pub(super) fn final_graph(&self) -> &VerifiedCanonicalKernelIrV13 {
        &self.final_graph
    }

    pub(super) const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }

    pub(super) fn source_kernels(&self) -> &[Kernel] {
        &self.source_module.kernels
    }

    pub(super) fn final_kernels(&self) -> &[Kernel] {
        &self.final_module.kernels
    }

    pub(super) fn require_exact(
        &self,
        source: &VerifiedCanonicalKernelIrV13,
        final_graph: &VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
    ) -> Result<(), Failure> {
        source.revalidate().map_err(Failure::Canonical)?;
        final_graph.revalidate().map_err(Failure::Canonical)?;
        if source != &self.source {
            return Err(Failure::SourceGraphSubjectMismatch);
        }
        if final_graph != &self.final_graph || final_epoch != self.final_epoch {
            return Err(Failure::FinalGraphSubjectMismatch);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(super) struct PreparedFinalKirTheoremV1 {
    pub(super) subjects: CanonicalFinalKirSubjectsV1,
    source: CanonicalGeneratedVerusProofInputV3,
    output_writes: u64,
    numerical_model: FinalKirNumericalModelV1,
}

pub(super) struct ExecutedFinalKirTheoremV1 {
    pub(super) subjects: CanonicalFinalKirSubjectsV1,
    pub(super) final_execution: RetainedImportedFunctionalRefinementReceiptV2,
    pub(super) staging_policy: ProductionRefinementStagingPolicyV2,
    pub(super) report: ProductionFinalGraphFunctionalReportV1,
}

impl PreparedFinalKirTheoremV1 {
    pub(super) fn try_new(
        source: &VerifiedCanonicalKernelIrV13,
        final_graph: &VerifiedCanonicalKernelIrV13,
        final_epoch: u64,
    ) -> Result<Self, Failure> {
        let subjects = CanonicalFinalKirSubjectsV1::try_new(source, final_graph, final_epoch)?;
        let proof = generate_final_kir_output_equivalence_v1(
            &subjects.source_module,
            &subjects.final_module,
        )
        .map_err(Failure::SemanticProof)?;
        let (source_text, output_writes, numerical_model) = proof.into_parts();
        Ok(Self {
            subjects,
            source: source_text,
            output_writes,
            numerical_model,
        })
    }

    pub(super) fn generated_source(&self) -> DigestV1 {
        DigestV1::from_untrusted_bytes(self.source.identity().as_bytes())
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn execute(
        self,
        runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
        semantic_kir: &ProductionSemanticKirOwnerV1,
        derivation: &ProductionIrDerivedFunctionalSemanticsV1,
        source_execution: &ProductionMirPlironPerCompilationVerusExecutionV1,
        target_closure: [u8; 32],
        timeout_seconds: u32,
    ) -> Result<ExecutedFinalKirTheoremV1, Failure> {
        semantic_kir
            .verify_equivalence()
            .map_err(Failure::SemanticKir)?;
        let source_graph = semantic_kir
            .canonical_kernel_ir_v13()
            .ok_or(Failure::SourceGraphSubjectMismatch)?;
        self.subjects.require_exact(
            source_graph,
            &self.subjects.final_graph,
            self.subjects.final_epoch,
        )?;
        let kernel_mir = DigestV1::from_untrusted_bytes(
            *semantic_kir
                .semantic()
                .semantic()
                .semantic_sha256()
                .as_bytes(),
        );
        if kernel_mir != derivation.kernel_mir() {
            return Err(Failure::KernelMirSubjectMismatch);
        }
        let prior_binding = require_source_proof(derivation, source_execution)?;
        if target_closure == [0; 32] {
            return Err(Failure::TargetClosureMissing);
        }
        let generated_source = self.generated_source();
        let obligation = final_graph_obligation_identity_v1(
            self.subjects.source.identity(),
            self.subjects.final_graph.identity(),
            self.subjects.final_epoch,
            derivation.output_expression_product(),
            generated_source,
            self.output_writes,
            self.numerical_model,
        );
        let binding = FunctionalRefinementBindingV2::new(
            prior_binding.safe_reference_kind(),
            prior_binding.safe_reference_identity(),
            prior_binding.safe_reference_source_hash(),
            prior_binding.safe_reference_mir_hash(),
            DigestV1::from_untrusted_bytes(*self.subjects.final_graph.identity().digest()),
            kernel_mir,
            obligation,
        )
        .map_err(Failure::Binding)?;
        let (final_execution, staging_policy) =
            execute_and_import_generated_mir_pliron_composition_locally_v1(
                runtime,
                self.source,
                binding,
                timeout_seconds,
            )
            .map_err(Failure::Execution)?;
        let report = ProductionFinalGraphFunctionalReportV1 {
            safe_reference_mir: derivation.safe_reference_mir(),
            kernel_mir,
            ranked_kernel: derivation.ranked_kernel(),
            parallel_contract: derivation.parallel_contract(),
            output_expression_product: derivation.output_expression_product(),
            source_graph: *self.subjects.source.identity(),
            final_graph: *self.subjects.final_graph.identity(),
            final_epoch: self.subjects.final_epoch,
            target_closure,
            generated_source,
            obligation,
            binding,
            output_writes: self.output_writes,
            numerical_model: self.numerical_model,
        };
        let execution = ExecutedFinalKirTheoremV1 {
            subjects: self.subjects,
            final_execution,
            staging_policy,
            report,
        };
        execution.require_exact_proof(derivation, source_execution, target_closure)?;
        Ok(execution)
    }
}

impl ExecutedFinalKirTheoremV1 {
    /// Rechecks retained custody without generating source or invoking Verus.
    pub(super) fn require_exact_proof(
        &self,
        derivation: &ProductionIrDerivedFunctionalSemanticsV1,
        source_execution: &ProductionMirPlironPerCompilationVerusExecutionV1,
        target_closure: [u8; 32],
    ) -> Result<(), Failure> {
        self.subjects.require_exact(
            self.subjects.source(),
            self.subjects.final_graph(),
            self.subjects.final_epoch(),
        )?;
        let prior_binding = require_source_proof(derivation, source_execution)?;
        let report = &self.report;
        if target_closure == [0; 32] {
            return Err(Failure::TargetClosureMissing);
        }
        if report.safe_reference_mir != derivation.safe_reference_mir()
            || report.kernel_mir != derivation.kernel_mir()
            || report.ranked_kernel != derivation.ranked_kernel()
            || report.parallel_contract != derivation.parallel_contract()
            || report.output_expression_product != derivation.output_expression_product()
            || report.source_graph != *self.subjects.source().identity()
            || report.final_graph != *self.subjects.final_graph().identity()
            || report.final_epoch != self.subjects.final_epoch()
            || report.target_closure != target_closure
        {
            return Err(Failure::ImportedProofSubjectMismatch);
        }
        let obligation = final_graph_obligation_identity_v1(
            self.subjects.source().identity(),
            self.subjects.final_graph().identity(),
            self.subjects.final_epoch(),
            derivation.output_expression_product(),
            report.generated_source,
            report.output_writes,
            report.numerical_model,
        );
        let expected_binding = FunctionalRefinementBindingV2::new(
            prior_binding.safe_reference_kind(),
            prior_binding.safe_reference_identity(),
            prior_binding.safe_reference_source_hash(),
            prior_binding.safe_reference_mir_hash(),
            DigestV1::from_untrusted_bytes(*self.subjects.final_graph().identity().digest()),
            derivation.kernel_mir(),
            obligation,
        )
        .map_err(Failure::Binding)?;
        if report.obligation != obligation || report.binding != expected_binding {
            return Err(Failure::ImportedProofSubjectMismatch);
        }
        let imported = self.final_execution.proof();
        if imported.binding() != expected_binding
            || imported.boundary() != FunctionalRefinementBoundaryV2::SafeReferenceMirToLivePliron
            || !self
                .staging_policy
                .accepts_signer(imported.signer_identity())
            || self.staging_policy.toolchain() != imported.toolchain()
            || !imported.signature_and_policy_verified()
        {
            return Err(Failure::ImportedProofSubjectMismatch);
        }
        Ok(())
    }
}

fn require_source_proof(
    derivation: &ProductionIrDerivedFunctionalSemanticsV1,
    source_execution: &ProductionMirPlironPerCompilationVerusExecutionV1,
) -> Result<FunctionalRefinementBindingV2, Failure> {
    let report = source_execution.report();
    let binding = report.binding();
    if !source_execution.retains_strictly_imported_signed_receipt()
        || binding.kernel_mir_hash() != derivation.kernel_mir()
        || binding.safe_reference_mir_hash() != derivation.safe_reference_mir()
        || binding.normalized_obligation_effect_ir_hash() != report.obligation_identity()
        || report.contract_identity() != derivation.semantic_contract()
        || report.parallel_contract_identity() != derivation.parallel_contract()
    {
        return Err(Failure::SourceProofSubjectMismatch);
    }
    Ok(binding)
}
