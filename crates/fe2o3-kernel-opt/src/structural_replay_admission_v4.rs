//! Independent exact replay admission for the canonical V13 optimizer.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{Module, VerifiedCanonicalKernelIrIdentityV13};

use crate::{
    KernelIrPlironOptimizationErrorV4, KernelIrPlironOptimizationReportV4,
    optimize_production_kernel_ir_module_v4,
};

#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use = "V13 structural replay grants no semantic-refinement authority"]
pub struct KernelIrPlironStructuralReplayAdmissionV4 {
    input: VerifiedCanonicalKernelIrIdentityV13,
    output: VerifiedCanonicalKernelIrIdentityV13,
    report: KernelIrPlironOptimizationReportV4,
}

impl KernelIrPlironStructuralReplayAdmissionV4 {
    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.input
    }

    pub const fn output_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.output
    }

    pub const fn report(&self) -> &KernelIrPlironOptimizationReportV4 {
        &self.report
    }

    pub const fn establishes_exact_closed_replay(&self) -> bool {
        true
    }

    pub const fn establishes_semantic_preservation(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub enum KernelIrPlironStructuralReplayAdmissionErrorV4 {
    NonProductionReport,
    Replay(KernelIrPlironOptimizationErrorV4),
    OutputMismatch,
    ReportMismatch,
}

impl fmt::Display for KernelIrPlironStructuralReplayAdmissionErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonProductionReport => formatter.write_str(
                "optimization report was not produced by the closed production V4 policy",
            ),
            Self::Replay(error) => {
                write!(formatter, "closed production V13 replay failed: {error}")
            }
            Self::OutputMismatch => formatter.write_str(
                "post-optimization Kernel IR does not match the closed production V13 replay",
            ),
            Self::ReportMismatch => formatter.write_str(
                "optimization report does not match the independently replayed V13 transaction",
            ),
        }
    }
}

impl Error for KernelIrPlironStructuralReplayAdmissionErrorV4 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Replay(error) => Some(error),
            Self::NonProductionReport | Self::OutputMismatch | Self::ReportMismatch => None,
        }
    }
}

pub fn admit_production_kernel_ir_structural_replay_v4(
    pre_optimization: &Module,
    post_optimization: &Module,
    live_report: &KernelIrPlironOptimizationReportV4,
) -> Result<KernelIrPlironStructuralReplayAdmissionV4, KernelIrPlironStructuralReplayAdmissionErrorV4>
{
    if !live_report.is_production_replay_compatible() {
        return Err(KernelIrPlironStructuralReplayAdmissionErrorV4::NonProductionReport);
    }
    let replayed = optimize_production_kernel_ir_module_v4(pre_optimization)
        .map_err(KernelIrPlironStructuralReplayAdmissionErrorV4::Replay)?;
    if replayed.module() != post_optimization {
        return Err(KernelIrPlironStructuralReplayAdmissionErrorV4::OutputMismatch);
    }
    if replayed.report() != live_report {
        return Err(KernelIrPlironStructuralReplayAdmissionErrorV4::ReportMismatch);
    }

    Ok(KernelIrPlironStructuralReplayAdmissionV4 {
        input: *replayed.report().input_identity(),
        output: *replayed.report().output_identity(),
        report: replayed.report().clone(),
    })
}
