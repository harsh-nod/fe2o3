//! Expanded fixed target-neutral production optimization over canonical KIR V13.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    Module, VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrIdentityV13,
    VerifiedCanonicalKernelIrV13,
};

use crate::{
    CheckedCanonicalTransformationErrorV1, CheckedTransformationPreservationRecordV1,
    ProductionTransformationV1, execute_checked_canonical_transformation_v13_v1,
    execute_checked_scalar_cleanup_replay_v1,
};

pub const KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_POLICY_VERSION_V5: u16 = 5;

/// Non-configurable phase order for the first expanded target-neutral policy.
pub const KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_PHASE_ORDER_V5: [ProductionTransformationV1; 4] = [
    ProductionTransformationV1::ScalarReplacementOfAggregates,
    ProductionTransformationV1::SecondarySsaPromotion,
    ProductionTransformationV1::HelperInlining,
    ProductionTransformationV1::InterproceduralCleanup,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelIrTargetNeutralOptimizationReportV5 {
    policy_version: u16,
    input_identity: VerifiedCanonicalKernelIrIdentityV13,
    output_identity: VerifiedCanonicalKernelIrIdentityV13,
    initial_epoch: u64,
    final_epoch: u64,
    transformations: Vec<CheckedTransformationPreservationRecordV1>,
}

impl KernelIrTargetNeutralOptimizationReportV5 {
    pub const fn policy_version(&self) -> u16 {
        self.policy_version
    }

    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.input_identity
    }

    pub const fn output_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.output_identity
    }

    pub const fn initial_epoch(&self) -> u64 {
        self.initial_epoch
    }

    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }

    pub fn transformations(&self) -> &[CheckedTransformationPreservationRecordV1] {
        &self.transformations
    }

    pub fn changed(&self) -> bool {
        self.input_identity != self.output_identity
    }

    pub fn is_exact_fixed_policy_replay(&self) -> bool {
        self.policy_version == KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_POLICY_VERSION_V5
            && self.initial_epoch == 0
            && exact_record_chain(self)
    }

    pub const fn grants_semantic_preservation_authority(&self) -> bool {
        false
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct OptimizedKernelIrModuleV5 {
    module: Module,
    canonical: VerifiedCanonicalKernelIrV13,
    report: KernelIrTargetNeutralOptimizationReportV5,
}

impl OptimizedKernelIrModuleV5 {
    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrV13 {
        &self.canonical
    }

    pub const fn report(&self) -> &KernelIrTargetNeutralOptimizationReportV5 {
        &self.report
    }

    pub fn into_parts(
        self,
    ) -> (
        Module,
        VerifiedCanonicalKernelIrV13,
        KernelIrTargetNeutralOptimizationReportV5,
    ) {
        (self.module, self.canonical, self.report)
    }
}

#[derive(Debug)]
pub enum KernelIrTargetNeutralOptimizationErrorV5 {
    Input(VerifiedCanonicalKernelIrErrorV13),
    Transform(CheckedCanonicalTransformationErrorV1),
    Output(VerifiedCanonicalKernelIrErrorV13),
    RecordChainMismatch,
}

impl fmt::Display for KernelIrTargetNeutralOptimizationErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Input(error) => write!(formatter, "expanded optimizer input was rejected: {error}"),
            Self::Transform(error) => error.fmt(formatter),
            Self::Output(error) => write!(formatter, "expanded optimizer output was rejected: {error}"),
            Self::RecordChainMismatch => formatter.write_str(
                "expanded optimizer transformation records do not form the exact identity/epoch chain",
            ),
        }
    }
}

impl Error for KernelIrTargetNeutralOptimizationErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) | Self::Output(error) => Some(error),
            Self::Transform(error) => Some(error),
            Self::RecordChainMismatch => None,
        }
    }
}

pub fn optimize_production_kernel_ir_module_v5(
    input: &Module,
) -> Result<OptimizedKernelIrModuleV5, KernelIrTargetNeutralOptimizationErrorV5> {
    let input_canonical = VerifiedCanonicalKernelIrV13::from_module(input.clone())
        .map_err(KernelIrTargetNeutralOptimizationErrorV5::Input)?;
    let mut module = input.clone();
    let mut epoch = 0_u64;
    let mut transformations = Vec::new();

    run_scalar_cleanup(&mut module, &mut epoch, &mut transformations)?;
    for transformation in KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_PHASE_ORDER_V5 {
        let output =
            execute_checked_canonical_transformation_v13_v1(transformation, &module, epoch)
                .map_err(KernelIrTargetNeutralOptimizationErrorV5::Transform)?;
        epoch = output.preservation().output_epoch();
        transformations.push(output.preservation().clone());
        module = output.module().clone();
        if matches!(
            transformation,
            ProductionTransformationV1::SecondarySsaPromotion
                | ProductionTransformationV1::InterproceduralCleanup
        ) {
            run_scalar_cleanup(&mut module, &mut epoch, &mut transformations)?;
        }
    }

    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone())
        .map_err(KernelIrTargetNeutralOptimizationErrorV5::Output)?;
    let report = KernelIrTargetNeutralOptimizationReportV5 {
        policy_version: KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_POLICY_VERSION_V5,
        input_identity: *input_canonical.identity(),
        output_identity: *canonical.identity(),
        initial_epoch: 0,
        final_epoch: epoch,
        transformations,
    };
    if !exact_record_chain(&report) {
        return Err(KernelIrTargetNeutralOptimizationErrorV5::RecordChainMismatch);
    }
    Ok(OptimizedKernelIrModuleV5 {
        module,
        canonical,
        report,
    })
}

fn run_scalar_cleanup(
    module: &mut Module,
    epoch: &mut u64,
    transformations: &mut Vec<CheckedTransformationPreservationRecordV1>,
) -> Result<(), KernelIrTargetNeutralOptimizationErrorV5> {
    let cleanup =
        execute_checked_scalar_cleanup_replay_v1(module, *epoch).map_err(|error| match error {
            crate::CheckedScalarCleanupReplayErrorV1::Transform(error) => {
                KernelIrTargetNeutralOptimizationErrorV5::Transform(error)
            }
            crate::CheckedScalarCleanupReplayErrorV1::IterationLimitExceeded { .. } => {
                KernelIrTargetNeutralOptimizationErrorV5::RecordChainMismatch
            }
        })?;
    *module = cleanup.module().clone();
    *epoch = cleanup.final_epoch();
    transformations.extend(cleanup.pass_records().iter().cloned());
    Ok(())
}

fn exact_record_chain(report: &KernelIrTargetNeutralOptimizationReportV5) -> bool {
    let mut identity = &report.input_identity;
    let mut epoch = report.initial_epoch;
    for record in &report.transformations {
        if record.input_identity() != identity || record.input_epoch() != epoch {
            return false;
        }
        identity = record.output_identity();
        epoch = record.output_epoch();
    }
    identity == &report.output_identity && epoch == report.final_epoch
}
