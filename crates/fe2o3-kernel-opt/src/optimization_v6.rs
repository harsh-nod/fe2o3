//! Fixed target-neutral loop and memory production policy over canonical KIR V13.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    Module, VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrIdentityV13,
    VerifiedCanonicalKernelIrV13,
};

use crate::{
    CheckedCanonicalTransformationErrorV1, CheckedScalarCleanupReplayErrorV1,
    CheckedTransformationPreservationRecordV1, KernelIrTargetNeutralOptimizationErrorV5,
    ProductionTransformationV1, execute_checked_canonical_transformation_v13_v1,
    execute_checked_scalar_cleanup_replay_v1, optimize_production_kernel_ir_module_v5,
};

pub const KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_POLICY_VERSION_V6: u16 = 6;

/// The loop/memory phase sequence always follows the complete V5 policy and is never
/// assembled from caller-selected passes.
pub const KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_PHASE_ORDER_V6: [ProductionTransformationV1; 7] = [
    ProductionTransformationV1::LoopCanonicalization,
    ProductionTransformationV1::InductionVariableSimplification,
    ProductionTransformationV1::LoopInvariantCodeMotion,
    ProductionTransformationV1::MemoryEffectVersioning,
    ProductionTransformationV1::MemorySimplification,
    ProductionTransformationV1::FullLoopUnrolling,
    ProductionTransformationV1::PartialLoopUnrolling,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelIrTargetNeutralOptimizationReportV6 {
    policy_version: u16,
    input_identity: VerifiedCanonicalKernelIrIdentityV13,
    output_identity: VerifiedCanonicalKernelIrIdentityV13,
    initial_epoch: u64,
    final_epoch: u64,
    transformations: Vec<CheckedTransformationPreservationRecordV1>,
}

impl KernelIrTargetNeutralOptimizationReportV6 {
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
        self.policy_version == KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_POLICY_VERSION_V6
            && self.initial_epoch == 0
            && exact_record_chain(self)
    }

    pub const fn grants_semantic_preservation_authority(&self) -> bool {
        false
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct OptimizedKernelIrModuleV6 {
    module: Module,
    canonical: VerifiedCanonicalKernelIrV13,
    report: KernelIrTargetNeutralOptimizationReportV6,
}

impl OptimizedKernelIrModuleV6 {
    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrV13 {
        &self.canonical
    }

    pub const fn report(&self) -> &KernelIrTargetNeutralOptimizationReportV6 {
        &self.report
    }

    pub fn into_parts(
        self,
    ) -> (
        Module,
        VerifiedCanonicalKernelIrV13,
        KernelIrTargetNeutralOptimizationReportV6,
    ) {
        (self.module, self.canonical, self.report)
    }
}

#[derive(Debug)]
pub enum KernelIrTargetNeutralOptimizationErrorV6 {
    V5(KernelIrTargetNeutralOptimizationErrorV5),
    Transform(CheckedCanonicalTransformationErrorV1),
    ScalarCleanup(CheckedScalarCleanupReplayErrorV1),
    Output(VerifiedCanonicalKernelIrErrorV13),
    RecordChainMismatch,
}

impl fmt::Display for KernelIrTargetNeutralOptimizationErrorV6 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::V5(error) => error.fmt(formatter),
            Self::Transform(error) => error.fmt(formatter),
            Self::ScalarCleanup(error) => error.fmt(formatter),
            Self::Output(error) => write!(formatter, "V6 optimizer output was rejected: {error}"),
            Self::RecordChainMismatch => formatter.write_str(
                "V6 optimizer records do not form the exact canonical identity/epoch chain",
            ),
        }
    }
}

impl Error for KernelIrTargetNeutralOptimizationErrorV6 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::V5(error) => Some(error),
            Self::Transform(error) => Some(error),
            Self::ScalarCleanup(error) => Some(error),
            Self::Output(error) => Some(error),
            Self::RecordChainMismatch => None,
        }
    }
}

pub fn optimize_production_kernel_ir_module_v6(
    input: &Module,
) -> Result<OptimizedKernelIrModuleV6, KernelIrTargetNeutralOptimizationErrorV6> {
    let v5 = optimize_production_kernel_ir_module_v5(input)
        .map_err(KernelIrTargetNeutralOptimizationErrorV6::V5)?;
    let (mut module, _, v5_report) = v5.into_parts();
    let mut epoch = v5_report.final_epoch();
    let mut transformations = v5_report.transformations().to_vec();

    for transformation in KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_PHASE_ORDER_V6 {
        let output =
            execute_checked_canonical_transformation_v13_v1(transformation, &module, epoch)
                .map_err(KernelIrTargetNeutralOptimizationErrorV6::Transform)?;
        epoch = output.preservation().output_epoch();
        transformations.push(output.preservation().clone());
        module = output.module().clone();
    }

    let cleanup = execute_checked_scalar_cleanup_replay_v1(&module, epoch)
        .map_err(KernelIrTargetNeutralOptimizationErrorV6::ScalarCleanup)?;
    module = cleanup.module().clone();
    epoch = cleanup.final_epoch();
    transformations.extend(cleanup.pass_records().iter().cloned());

    let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone())
        .map_err(KernelIrTargetNeutralOptimizationErrorV6::Output)?;
    let report = KernelIrTargetNeutralOptimizationReportV6 {
        policy_version: KERNEL_IR_TARGET_NEUTRAL_PRODUCTION_POLICY_VERSION_V6,
        input_identity: *v5_report.input_identity(),
        output_identity: *canonical.identity(),
        initial_epoch: v5_report.initial_epoch(),
        final_epoch: epoch,
        transformations,
    };
    if !exact_record_chain(&report) {
        return Err(KernelIrTargetNeutralOptimizationErrorV6::RecordChainMismatch);
    }
    Ok(OptimizedKernelIrModuleV6 {
        module,
        canonical,
        report,
    })
}

fn exact_record_chain(report: &KernelIrTargetNeutralOptimizationReportV6) -> bool {
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
