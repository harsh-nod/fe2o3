//! Independent structural replay admission for the closed production optimizer.
//!
//! Admission reruns production V2 from the supplied pre-optimization module and
//! requires exact agreement with both the supplied post-optimization module and
//! the complete live report. This establishes deterministic execution of the
//! closed transformation relation. It does not establish semantic refinement:
//! the current pass APIs publish aggregate accounting and endpoint identities,
//! not per-rewrite witnesses or a pre/post value correspondence.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::Module;
use fe2o3_pliron::KirBridgeDigestV1;

use crate::{
    KernelIrPlironOptimizationErrorV2, KernelIrPlironOptimizationReportV2,
    optimize_production_kernel_ir_module_v2,
};

/// A successfully replayed, exact structural relation between two KIR modules.
#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use = "structural replay admission carries no semantic-refinement authority"]
pub struct KernelIrPlironStructuralReplayAdmissionV2 {
    input: KirBridgeDigestV1,
    output: KirBridgeDigestV1,
    report: KernelIrPlironOptimizationReportV2,
}

impl KernelIrPlironStructuralReplayAdmissionV2 {
    pub const fn input_digest(&self) -> KirBridgeDigestV1 {
        self.input
    }

    pub const fn output_digest(&self) -> KirBridgeDigestV1 {
        self.output
    }

    pub const fn report(&self) -> &KernelIrPlironOptimizationReportV2 {
        &self.report
    }

    /// The post-module and report exactly match a fresh closed-policy replay.
    pub const fn establishes_exact_closed_replay(&self) -> bool {
        true
    }

    /// Both endpoints passed canonical V10 construction and KIR verification.
    pub const fn establishes_structural_well_formedness(&self) -> bool {
        true
    }

    /// Aggregate pass reports are not a proof of semantic refinement.
    pub const fn establishes_semantic_preservation(&self) -> bool {
        false
    }

    /// This token cannot authorize a compiler-refinement claim.
    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
}

/// Failure to reproduce the exact closed production transformation relation.
#[derive(Debug)]
pub enum KernelIrPlironStructuralReplayAdmissionErrorV2 {
    NonProductionReport,
    Replay(KernelIrPlironOptimizationErrorV2),
    OutputMismatch,
    ReportMismatch,
}

impl fmt::Display for KernelIrPlironStructuralReplayAdmissionErrorV2 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonProductionReport => formatter.write_str(
                "optimization report was not produced by the closed production V2 policy",
            ),
            Self::Replay(error) => write!(formatter, "closed production V2 replay failed: {error}"),
            Self::OutputMismatch => formatter.write_str(
                "post-optimization Kernel IR does not match the closed production V2 replay",
            ),
            Self::ReportMismatch => formatter.write_str(
                "optimization report does not match the independently replayed execution",
            ),
        }
    }
}

impl Error for KernelIrPlironStructuralReplayAdmissionErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Replay(error) => Some(error),
            Self::NonProductionReport | Self::OutputMismatch | Self::ReportMismatch => None,
        }
    }
}

/// Independently reruns production V2 and admits only exact post-state/report agreement.
pub fn admit_production_kernel_ir_structural_replay_v2(
    pre_optimization: &Module,
    post_optimization: &Module,
    live_report: &KernelIrPlironOptimizationReportV2,
) -> Result<KernelIrPlironStructuralReplayAdmissionV2, KernelIrPlironStructuralReplayAdmissionErrorV2>
{
    if !live_report.is_production_replay_compatible() {
        return Err(KernelIrPlironStructuralReplayAdmissionErrorV2::NonProductionReport);
    }

    let replayed = optimize_production_kernel_ir_module_v2(pre_optimization)
        .map_err(KernelIrPlironStructuralReplayAdmissionErrorV2::Replay)?;
    if replayed.module() != post_optimization {
        return Err(KernelIrPlironStructuralReplayAdmissionErrorV2::OutputMismatch);
    }
    if replayed.report() != live_report {
        return Err(KernelIrPlironStructuralReplayAdmissionErrorV2::ReportMismatch);
    }

    Ok(KernelIrPlironStructuralReplayAdmissionV2 {
        input: replayed.report().input_digest(),
        output: replayed.report().output_digest(),
        report: replayed.report().clone(),
    })
}
