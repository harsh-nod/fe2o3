//! Independent exact replay admission for the fixed target-neutral V6 policy.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{Module, VerifiedCanonicalKernelIrIdentityV13};

use crate::{
    KernelIrTargetNeutralOptimizationErrorV6, KernelIrTargetNeutralOptimizationReportV6,
    optimize_production_kernel_ir_module_v6,
};

#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use = "V6 structural replay grants no semantic-refinement authority"]
pub struct KernelIrTargetNeutralStructuralReplayAdmissionV6 {
    input: VerifiedCanonicalKernelIrIdentityV13,
    output: VerifiedCanonicalKernelIrIdentityV13,
    report: KernelIrTargetNeutralOptimizationReportV6,
}

impl KernelIrTargetNeutralStructuralReplayAdmissionV6 {
    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.input
    }

    pub const fn output_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.output
    }

    pub const fn report(&self) -> &KernelIrTargetNeutralOptimizationReportV6 {
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
pub enum KernelIrTargetNeutralStructuralReplayAdmissionErrorV6 {
    NonProductionReport,
    Replay(KernelIrTargetNeutralOptimizationErrorV6),
    OutputMismatch,
    ReportMismatch,
}

impl fmt::Display for KernelIrTargetNeutralStructuralReplayAdmissionErrorV6 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonProductionReport => formatter.write_str(
                "optimization report was not produced by the fixed production V6 policy",
            ),
            Self::Replay(error) => write!(
                formatter,
                "fixed production target-neutral V6 replay failed: {error}"
            ),
            Self::OutputMismatch => formatter.write_str(
                "post-optimization Kernel IR does not match the fixed production V6 replay",
            ),
            Self::ReportMismatch => formatter.write_str(
                "optimization report does not match the independently replayed V6 transaction",
            ),
        }
    }
}

impl Error for KernelIrTargetNeutralStructuralReplayAdmissionErrorV6 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Replay(error) => Some(error),
            Self::NonProductionReport | Self::OutputMismatch | Self::ReportMismatch => None,
        }
    }
}

pub fn admit_production_kernel_ir_structural_replay_v6(
    pre_optimization: &Module,
    post_optimization: &Module,
    live_report: &KernelIrTargetNeutralOptimizationReportV6,
) -> Result<
    KernelIrTargetNeutralStructuralReplayAdmissionV6,
    KernelIrTargetNeutralStructuralReplayAdmissionErrorV6,
> {
    if !live_report.is_exact_fixed_policy_replay() {
        return Err(KernelIrTargetNeutralStructuralReplayAdmissionErrorV6::NonProductionReport);
    }
    let replayed = optimize_production_kernel_ir_module_v6(pre_optimization)
        .map_err(KernelIrTargetNeutralStructuralReplayAdmissionErrorV6::Replay)?;
    if replayed.module() != post_optimization {
        return Err(KernelIrTargetNeutralStructuralReplayAdmissionErrorV6::OutputMismatch);
    }
    if replayed.report() != live_report {
        return Err(KernelIrTargetNeutralStructuralReplayAdmissionErrorV6::ReportMismatch);
    }

    Ok(KernelIrTargetNeutralStructuralReplayAdmissionV6 {
        input: *replayed.report().input_identity(),
        output: *replayed.report().output_identity(),
        report: replayed.report().clone(),
    })
}
