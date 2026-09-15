//! Move-only binding of an executed functional theorem to exact final KIR.
//!
//! This is the publication boundary for the supported optimized subset. The
//! final graph is not required to equal source-derived KIR. Instead, its exact
//! output expressions and writes are independently extracted and proved equal
//! to the source-derived KIR output transformer in retained Verus.

use std::{error::Error, fmt};

use fe2o3_functional_proof::{
    FunctionalRefinementBindingV2, FunctionalRefinementBoundaryV2,
    FunctionalRefinementImportErrorV2,
};
use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrIdentityV13};
use fe2o3_lower_mir_kernel::{ProductionSemanticKirErrorV1, ProductionSemanticKirOwnerV1};
use fe2o3_pliron::{
    ProductionFinalGraphTargetContractErrorV1, ProductionFinalGraphVerificationErrorV1,
    ProductionRefinementStagingPolicyV2, ProductionVerifiedFinalGraphV1,
};
use fe2o3_proof_contracts::DigestV1;
use sha2::{Digest as _, Sha256};

use crate::final_kir_output_equivalence_v1::FinalKirNumericalModelV1;
use crate::functional_refinement_receipt_v2::{
    RetainedImportedFunctionalRefinementReceiptV2,
    execute_and_import_generated_mir_pliron_composition_locally_v1,
};
use crate::{
    FinalKirOutputEquivalenceErrorV1, FunctionalRefinementVerusExecutionErrorV2,
    FunctionalRefinementVerusRuntimeLeaseV1,
    ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3,
    ProductionIrDerivedVerusVerifiedMirPlironKernelV2,
};

const FINAL_GRAPH_FUNCTIONAL_OBLIGATION_DOMAIN_V1: &[u8] =
    b"FE2O3/PRODUCTION/FINAL-KIR-DIRECT-FUNCTIONAL-OBLIGATION/V1\0";

mod prepared;
mod theorem;

pub use prepared::{
    ProductionPreparedFinalGraphFunctionalExecutionV1,
    ProductionPreparedFinalGraphFunctionalSubjectV1,
};
use theorem::PreparedFinalKirTheoremV1;

#[cfg(test)]
mod tests;

/// Exact subjects retained at the final-graph functional boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionFinalGraphFunctionalReportV1 {
    safe_reference_mir: DigestV1,
    kernel_mir: DigestV1,
    ranked_kernel: DigestV1,
    parallel_contract: DigestV1,
    output_expression_product: DigestV1,
    source_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_graph: VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    target_closure: [u8; 32],
    generated_source: DigestV1,
    obligation: DigestV1,
    binding: FunctionalRefinementBindingV2,
    output_writes: u64,
    numerical_model: FinalKirNumericalModelV1,
}

impl ProductionFinalGraphFunctionalReportV1 {
    pub const fn safe_reference_mir(self) -> DigestV1 {
        self.safe_reference_mir
    }

    pub const fn kernel_mir(self) -> DigestV1 {
        self.kernel_mir
    }

    pub const fn ranked_kernel(self) -> DigestV1 {
        self.ranked_kernel
    }

    pub const fn parallel_contract(self) -> DigestV1 {
        self.parallel_contract
    }

    pub const fn output_expression_product(self) -> DigestV1 {
        self.output_expression_product
    }

    pub const fn source_graph(self) -> VerifiedCanonicalKernelIrIdentityV13 {
        self.source_graph
    }

    pub const fn final_graph(self) -> VerifiedCanonicalKernelIrIdentityV13 {
        self.final_graph
    }

    pub const fn final_epoch(self) -> u64 {
        self.final_epoch
    }

    pub const fn target_closure(self) -> [u8; 32] {
        self.target_closure
    }

    pub const fn generated_source(self) -> DigestV1 {
        self.generated_source
    }

    pub const fn obligation(self) -> DigestV1 {
        self.obligation
    }

    pub const fn binding(self) -> FunctionalRefinementBindingV2 {
        self.binding
    }

    pub const fn output_writes(self) -> u64 {
        self.output_writes
    }

    pub const fn numerical_model(self) -> FinalKirNumericalModelV1 {
        self.numerical_model
    }

    pub const fn final_graph_binds_launch_and_target_contracts(self) -> bool {
        true
    }

    pub const fn proves_exact_final_graph_output_transformer(self) -> bool {
        true
    }
}

/// Move-only final result that retains both executed theorems and the exact
/// optimized live graph to which the second theorem applies.
#[must_use = "dropping this value abandons final-graph functional refinement custody"]
pub struct ProductionFinalGraphFunctionalRefinementV1 {
    verified: ProductionIrDerivedVerusVerifiedMirPlironKernelV2,
    final_graph: ProductionVerifiedFinalGraphV1,
    final_execution: RetainedImportedFunctionalRefinementReceiptV2,
    _staging_policy: ProductionRefinementStagingPolicyV2,
    report: ProductionFinalGraphFunctionalReportV1,
}

/// Move-only functional result for one ranked root while a module-wide final
/// graph remains borrowed by the production compiler's single owner.
#[must_use = "dropping this value abandons final-graph functional refinement custody"]
pub struct ProductionFinalGraphFunctionalRefinementExecutionV2 {
    verified: ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3,
    final_execution: RetainedImportedFunctionalRefinementReceiptV2,
    _staging_policy: ProductionRefinementStagingPolicyV2,
    report: ProductionFinalGraphFunctionalReportV1,
}

impl fmt::Debug for ProductionFinalGraphFunctionalRefinementExecutionV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionFinalGraphFunctionalRefinementExecutionV2")
            .field("report", &self.report)
            .finish_non_exhaustive()
    }
}

impl ProductionFinalGraphFunctionalRefinementExecutionV2 {
    pub const fn report(&self) -> ProductionFinalGraphFunctionalReportV1 {
        self.report
    }

    pub const fn verified(&self) -> &ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3 {
        &self.verified
    }

    pub const fn final_signed_receipt_wire(&self) -> &[u8] {
        self.final_execution.wire()
    }

    pub const fn final_receipt_verifying_key(&self) -> &[u8; 32] {
        self.final_execution.verifying_key()
    }

    pub const fn retains_strictly_imported_final_graph_receipt(&self) -> bool {
        self.final_execution.proof().signature_and_policy_verified()
    }

    pub const fn grants_llvm_or_later_authority(&self) -> bool {
        false
    }
}

impl fmt::Debug for ProductionFinalGraphFunctionalRefinementV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProductionFinalGraphFunctionalRefinementV1")
            .field("report", &self.report)
            .finish_non_exhaustive()
    }
}

impl ProductionFinalGraphFunctionalRefinementV1 {
    pub const fn report(&self) -> ProductionFinalGraphFunctionalReportV1 {
        self.report
    }

    pub const fn verified(&self) -> &ProductionIrDerivedVerusVerifiedMirPlironKernelV2 {
        &self.verified
    }

    pub const fn final_graph(&self) -> &ProductionVerifiedFinalGraphV1 {
        &self.final_graph
    }

    pub const fn final_signed_receipt_wire(&self) -> &[u8] {
        self.final_execution.wire()
    }

    pub const fn final_receipt_verifying_key(&self) -> &[u8; 32] {
        self.final_execution.verifying_key()
    }

    pub const fn retains_strictly_imported_final_graph_receipt(&self) -> bool {
        self.final_execution.proof().signature_and_policy_verified()
    }

    pub const fn accepts_caller_authored_semantic_truth(&self) -> bool {
        false
    }

    pub const fn permits_unproved_semantics_changing_optimization(&self) -> bool {
        false
    }

    pub const fn requires_source_and_final_graph_byte_equality(&self) -> bool {
        false
    }

    pub const fn grants_llvm_or_later_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum ProductionFinalGraphFunctionalRefinementErrorV1 {
    Canonical(VerifiedCanonicalKernelIrErrorV13),
    TargetContract(ProductionFinalGraphTargetContractErrorV1),
    TargetContractSubjectMismatch,
    SourceGraphSubjectMismatch,
    SourceProofSubjectMismatch,
    SemanticKir(ProductionSemanticKirErrorV1),
    FinalGraph(ProductionFinalGraphVerificationErrorV1),
    SemanticProof(FinalKirOutputEquivalenceErrorV1),
    Binding(FunctionalRefinementImportErrorV2),
    Execution(FunctionalRefinementVerusExecutionErrorV2),
    KernelMirSubjectMismatch,
    FinalGraphSubjectMismatch,
    ImportedProofSubjectMismatch,
    TargetClosureMissing,
}

impl fmt::Display for ProductionFinalGraphFunctionalRefinementErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Canonical(error) => write!(formatter, "canonical KIR replay failed: {error}"),
            Self::TargetContract(error) => write!(formatter, "final target contract failed: {error}"),
            Self::TargetContractSubjectMismatch => formatter.write_str(
                "prepared functional subject and checked target contract differ",
            ),
            Self::SourceGraphSubjectMismatch => formatter.write_str(
                "prepared functional subject and semantic owner name different source KIR",
            ),
            Self::SourceProofSubjectMismatch => formatter.write_str(
                "source functional derivation and retained execution have different subjects",
            ),
            Self::SemanticKir(error) => write!(formatter, "semantic KIR replay failed: {error}"),
            Self::FinalGraph(error) => write!(formatter, "final live KIR replay failed: {error}"),
            Self::SemanticProof(error) => error.fmt(formatter),
            Self::Binding(error) => write!(formatter, "final-KIR proof binding failed: {error}"),
            Self::Execution(error) => write!(formatter, "final-KIR Verus execution failed: {error}"),
            Self::KernelMirSubjectMismatch => formatter.write_str(
                "functional theorem and source-derived KIR name different authenticated kernel MIR",
            ),
            Self::FinalGraphSubjectMismatch => formatter.write_str(
                "retained final-graph report does not name its exact canonical graph and epoch",
            ),
            Self::ImportedProofSubjectMismatch => formatter.write_str(
                "executed final-KIR receipt does not bind the exact generated theorem and final graph",
            ),
            Self::TargetClosureMissing => {
                formatter.write_str("final KIR has no bound target-capability closure")
            }
        }
    }
}

impl Error for ProductionFinalGraphFunctionalRefinementErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Canonical(error) => Some(error),
            Self::TargetContract(error) => Some(error),
            Self::SemanticKir(error) => Some(error),
            Self::FinalGraph(error) => Some(error),
            Self::SemanticProof(error) => Some(error),
            Self::Binding(error) => Some(error),
            Self::Execution(error) => Some(error),
            _ => None,
        }
    }
}

/// Consumes the exact optimized graph at the production publication boundary.
///
/// Volta's integration site is
/// `rustc-codegen-fe2o3/src/production_pipeline.rs::TargetLoweredProductionCompilation::prepare_worker_handoff`,
/// after destructuring `target_verification` and before the current
/// `drop(reference_effect_bindings)`. The caller must carry the returned owner
/// into publication instead of publishing the final graph and functional
/// theorem independently.
pub fn bind_ir_derived_functional_refinement_to_final_graph_v1(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    semantic_kir: &ProductionSemanticKirOwnerV1,
    mut final_graph: ProductionVerifiedFinalGraphV1,
    verified: ProductionIrDerivedVerusVerifiedMirPlironKernelV2,
    timeout_seconds: u32,
) -> Result<
    ProductionFinalGraphFunctionalRefinementV1,
    ProductionFinalGraphFunctionalRefinementErrorV1,
> {
    let theorem = prepare_for_verified_graph(semantic_kir, &mut final_graph)?;
    let execution = theorem.execute(
        runtime,
        semantic_kir,
        verified.derivation(),
        verified.verified().per_compilation_verus_execution(),
        final_graph.report().resources().closure_identity(),
        timeout_seconds,
    )?;
    final_graph
        .revalidate_live()
        .map_err(ProductionFinalGraphFunctionalRefinementErrorV1::FinalGraph)?;
    Ok(ProductionFinalGraphFunctionalRefinementV1 {
        verified,
        final_graph,
        final_execution: execution.final_execution,
        _staging_policy: execution.staging_policy,
        report: execution.report,
    })
}

/// Executes the final-graph theorem for one exact rustc reference-effect owner
/// while retaining the sole live graph in the caller. This is the module-roster
/// integration entrypoint: callers must execute it once for every exact ranked
/// root and retain all returned move-only results beside the borrowed graph.
pub fn bind_effect_ir_derived_functional_refinement_to_borrowed_final_graph_v2(
    runtime: &FunctionalRefinementVerusRuntimeLeaseV1,
    semantic_kir: &ProductionSemanticKirOwnerV1,
    final_graph: &mut ProductionVerifiedFinalGraphV1,
    verified: ProductionEffectIrDerivedVerusVerifiedMirPlironKernelV3,
    timeout_seconds: u32,
) -> Result<
    ProductionFinalGraphFunctionalRefinementExecutionV2,
    ProductionFinalGraphFunctionalRefinementErrorV1,
> {
    let theorem = prepare_for_verified_graph(semantic_kir, final_graph)?;
    let execution = theorem.execute(
        runtime,
        semantic_kir,
        verified.derivation(),
        verified.per_compilation_verus_execution(),
        final_graph.report().resources().closure_identity(),
        timeout_seconds,
    )?;
    final_graph
        .revalidate_live()
        .map_err(ProductionFinalGraphFunctionalRefinementErrorV1::FinalGraph)?;
    Ok(ProductionFinalGraphFunctionalRefinementExecutionV2 {
        verified,
        final_execution: execution.final_execution,
        _staging_policy: execution.staging_policy,
        report: execution.report,
    })
}

fn prepare_for_verified_graph(
    semantic_kir: &ProductionSemanticKirOwnerV1,
    final_graph: &mut ProductionVerifiedFinalGraphV1,
) -> Result<PreparedFinalKirTheoremV1, ProductionFinalGraphFunctionalRefinementErrorV1> {
    final_graph
        .revalidate_live()
        .map_err(ProductionFinalGraphFunctionalRefinementErrorV1::FinalGraph)?;
    if final_graph.report().final_graph() != final_graph.canonical().identity()
        || final_graph.report().final_epoch() != final_graph.final_epoch()
    {
        return Err(ProductionFinalGraphFunctionalRefinementErrorV1::FinalGraphSubjectMismatch);
    }
    if final_graph.report().resources().closure_identity() == [0; 32] {
        return Err(ProductionFinalGraphFunctionalRefinementErrorV1::TargetClosureMissing);
    }
    let source = semantic_kir
        .canonical_kernel_ir_v13()
        .ok_or(ProductionFinalGraphFunctionalRefinementErrorV1::SourceGraphSubjectMismatch)?;
    PreparedFinalKirTheoremV1::try_new(source, final_graph.canonical(), final_graph.final_epoch())
}

fn final_graph_obligation_identity_v1(
    source_graph: &VerifiedCanonicalKernelIrIdentityV13,
    final_graph: &VerifiedCanonicalKernelIrIdentityV13,
    final_epoch: u64,
    output_expression_product: DigestV1,
    generated_source: DigestV1,
    output_writes: u64,
    numerical_model: FinalKirNumericalModelV1,
) -> DigestV1 {
    let mut digest = Sha256::new();
    digest.update((FINAL_GRAPH_FUNCTIONAL_OBLIGATION_DOMAIN_V1.len() as u64).to_le_bytes());
    digest.update(FINAL_GRAPH_FUNCTIONAL_OBLIGATION_DOMAIN_V1);
    digest.update(source_graph.digest());
    digest.update(source_graph.canonical_length().to_le_bytes());
    digest.update(final_graph.digest());
    digest.update(final_graph.canonical_length().to_le_bytes());
    digest.update(final_epoch.to_le_bytes());
    digest.update(output_expression_product.as_bytes());
    digest.update(generated_source.as_bytes());
    digest.update(output_writes.to_le_bytes());
    digest.update([match numerical_model {
        FinalKirNumericalModelV1::ExactBitVector => 1,
        FinalKirNumericalModelV1::StrictFloatOperatorCongruence => 2,
    }]);
    DigestV1::from_untrusted_bytes(digest.finalize().into())
}
