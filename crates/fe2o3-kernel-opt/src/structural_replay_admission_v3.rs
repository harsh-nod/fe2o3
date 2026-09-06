//! Independent structural replay admission for V11 optimization transport.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::Module;
use fe2o3_pliron::KirBridgeDigestV1;

use crate::{
    KernelIrPlironOptimizationErrorV3, KernelIrPlironOptimizationReportV2,
    optimize_production_kernel_ir_module_v3,
};

/// Exact replay of the closed V2 policy between two canonical V11 modules.
#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use = "structural replay admission carries no semantic-refinement authority"]
pub struct KernelIrPlironStructuralReplayAdmissionV3 {
    input: KirBridgeDigestV1,
    output: KirBridgeDigestV1,
    report: KernelIrPlironOptimizationReportV2,
}

impl KernelIrPlironStructuralReplayAdmissionV3 {
    pub const fn input_digest(&self) -> KirBridgeDigestV1 {
        self.input
    }

    pub const fn output_digest(&self) -> KirBridgeDigestV1 {
        self.output
    }

    pub const fn report(&self) -> &KernelIrPlironOptimizationReportV2 {
        &self.report
    }

    pub const fn establishes_exact_closed_replay(&self) -> bool {
        true
    }

    pub const fn establishes_structural_well_formedness(&self) -> bool {
        true
    }

    pub const fn establishes_semantic_preservation(&self) -> bool {
        false
    }

    pub const fn grants_compiler_refinement_authority(&self) -> bool {
        false
    }
}

/// Failure to reproduce the exact closed policy through the V11 endpoint.
#[derive(Debug)]
pub enum KernelIrPlironStructuralReplayAdmissionErrorV3 {
    NonProductionReport,
    Replay(KernelIrPlironOptimizationErrorV3),
    OutputMismatch,
    ReportMismatch,
}

impl fmt::Display for KernelIrPlironStructuralReplayAdmissionErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NonProductionReport => formatter.write_str(
                "optimization report was not produced by the closed production V2 policy",
            ),
            Self::Replay(error) => {
                write!(formatter, "closed production V11 replay failed: {error}")
            }
            Self::OutputMismatch => formatter.write_str(
                "post-optimization Kernel IR does not match the closed production V11 replay",
            ),
            Self::ReportMismatch => formatter.write_str(
                "optimization report does not match the independently replayed V11 execution",
            ),
        }
    }
}

impl Error for KernelIrPlironStructuralReplayAdmissionErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Replay(error) => Some(error),
            Self::NonProductionReport | Self::OutputMismatch | Self::ReportMismatch => None,
        }
    }
}

/// Independently reruns production V3 and admits exact post-state/report agreement.
pub fn admit_production_kernel_ir_structural_replay_v3(
    pre_optimization: &Module,
    post_optimization: &Module,
    live_report: &KernelIrPlironOptimizationReportV2,
) -> Result<KernelIrPlironStructuralReplayAdmissionV3, KernelIrPlironStructuralReplayAdmissionErrorV3>
{
    if !live_report.is_production_replay_compatible() {
        return Err(KernelIrPlironStructuralReplayAdmissionErrorV3::NonProductionReport);
    }

    let replayed = optimize_production_kernel_ir_module_v3(pre_optimization)
        .map_err(KernelIrPlironStructuralReplayAdmissionErrorV3::Replay)?;
    if replayed.module() != post_optimization {
        return Err(KernelIrPlironStructuralReplayAdmissionErrorV3::OutputMismatch);
    }
    if replayed.report() != live_report {
        return Err(KernelIrPlironStructuralReplayAdmissionErrorV3::ReportMismatch);
    }

    Ok(KernelIrPlironStructuralReplayAdmissionV3 {
        input: replayed.report().input_digest(),
        output: replayed.report().output_digest(),
        report: replayed.report().clone(),
    })
}
