//! Transactional Pliron-backed optimization of canonical Kernel IR V13.
//!
//! V4 imports and exports the one V13 executable graph. Compiler-issued
//! context provenance and portable execution requirements remain live graph
//! facts throughout every pass. Before publication, an epoch-bound affected
//! analysis rejects deletion, duplication, movement, substitution, or
//! weakening of those facts.

use std::{error::Error, fmt};

use fe2o3_kernel_analysis::{
    KernelCapabilityPreservationErrorV1, KernelCapabilityPreservationReplayV1,
    analyze_kernel_capability_preservation_v1,
};
use fe2o3_kernel_ir::{
    KernelIrDecodeError, KernelIrEncodeError, MAX_MODULE_BYTES_V1, Module, VerificationErrors,
    VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrIdentityV13,
    VerifiedCanonicalKernelIrV13, decode_module_v13, encode_module_v13, verify_module,
};
use fe2o3_pliron::{
    ContextBuildError, KirBridgeErrorV1, NameError, PlironOptimizationErrorV1,
    PlironOptimizationPassV1, PlironOptimizationPlanErrorV1, PlironOptimizationPlanV1,
    PlironSession,
};

use crate::{
    CheckedTransformationPreservationRecordV1,
    KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2, KernelIrPlironOptimizationByteLimitV2,
    KernelIrPlironOptimizationErrorV2, KernelIrPlironOptimizationLimitsV2,
    KernelIrPlironOptimizationPassReportV2, KernelIrPlironOptimizationPolicyV2,
    KernelIrPlironOptimizationReportV2, TransformationPreservationErrorV1,
    check_pliron_transformation_preservation_v1, epoch_reports,
    production_kernel_ir_pliron_optimization_limits_v2,
    production_policy_has_only_checked_transformations_v1,
};

pub const MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V4: usize = MAX_MODULE_BYTES_V1;
pub const KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_POLICY_VERSION_V4: u16 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelIrPlironOptimizationPolicyV4 {
    Configurable,
    ProductionV4,
}

/// Affected-analysis replay for one exact optimizer pass boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
#[must_use = "per-pass capability replay is not semantic-preservation authority"]
pub struct KernelCapabilityPassReplayV4 {
    pass: PlironOptimizationPassV1,
    preservation: CheckedTransformationPreservationRecordV1,
}

impl KernelCapabilityPassReplayV4 {
    pub const fn pass(&self) -> PlironOptimizationPassV1 {
        self.pass
    }

    pub const fn replay(&self) -> &KernelCapabilityPreservationReplayV1 {
        self.preservation.capability_replay()
    }

    pub const fn preservation(&self) -> &CheckedTransformationPreservationRecordV1 {
        &self.preservation
    }

    pub const fn grants_semantic_preservation_authority(&self) -> bool {
        false
    }
}

/// Accounting for one exact V13 transaction.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelIrPlironOptimizationReportV4 {
    policy: KernelIrPlironOptimizationPolicyV4,
    limits: KernelIrPlironOptimizationLimitsV2,
    initial_epoch: u64,
    final_epoch: u64,
    input_identity: VerifiedCanonicalKernelIrIdentityV13,
    output_identity: VerifiedCanonicalKernelIrIdentityV13,
    optimizer: KernelIrPlironOptimizationReportV2,
    capability_pass_replays: Vec<KernelCapabilityPassReplayV4>,
    capability_replay: KernelCapabilityPreservationReplayV1,
}

impl KernelIrPlironOptimizationReportV4 {
    pub const fn policy(&self) -> KernelIrPlironOptimizationPolicyV4 {
        self.policy
    }

    pub const fn limits(&self) -> KernelIrPlironOptimizationLimitsV2 {
        self.limits
    }

    pub const fn initial_epoch(&self) -> u64 {
        self.initial_epoch
    }

    pub const fn final_epoch(&self) -> u64 {
        self.final_epoch
    }

    pub const fn input_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.input_identity
    }

    pub const fn output_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.output_identity
    }

    pub const fn optimizer(&self) -> &KernelIrPlironOptimizationReportV2 {
        &self.optimizer
    }

    pub const fn capability_replay(&self) -> &KernelCapabilityPreservationReplayV1 {
        &self.capability_replay
    }

    pub fn capability_pass_replays(&self) -> &[KernelCapabilityPassReplayV4] {
        &self.capability_pass_replays
    }

    pub fn changed(&self) -> bool {
        self.input_identity != self.output_identity
    }

    pub fn is_production_replay_compatible(&self) -> bool {
        self.policy == KernelIrPlironOptimizationPolicyV4::ProductionV4
            && self.limits == production_kernel_ir_pliron_optimization_limits_v2()
            && self.initial_epoch == 0
            && self.optimizer.is_production_replay_compatible()
            && self.capability_replay.input_epoch() == self.initial_epoch
            && self.capability_replay.output_epoch() == self.final_epoch
            && capability_pass_replay_chain_is_exact(self)
    }

    /// Checked structural and affected-analysis replay is not a proof of
    /// general transformation semantics, target support, or safe launch.
    pub const fn grants_semantic_preservation_authority(&self) -> bool {
        false
    }
}

/// Fully verified V13 output published only after all passes and affected
/// capability analysis complete.
#[derive(Debug, Eq, PartialEq)]
pub struct OptimizedKernelIrModuleV4 {
    module: Module,
    canonical: VerifiedCanonicalKernelIrV13,
    report: KernelIrPlironOptimizationReportV4,
}

impl OptimizedKernelIrModuleV4 {
    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrV13 {
        &self.canonical
    }

    pub const fn report(&self) -> &KernelIrPlironOptimizationReportV4 {
        &self.report
    }

    pub fn into_parts(
        self,
    ) -> (
        Module,
        VerifiedCanonicalKernelIrV13,
        KernelIrPlironOptimizationReportV4,
    ) {
        (self.module, self.canonical, self.report)
    }
}

#[derive(Debug)]
pub enum KernelIrPlironOptimizationErrorV4 {
    InvalidByteLimit {
        limit: KernelIrPlironOptimizationByteLimitV2,
        requested: usize,
        hard_maximum: usize,
    },
    InputEncoding(KernelIrEncodeError),
    InputCanonicalization(VerifiedCanonicalKernelIrErrorV13),
    InputByteLimitExceeded {
        required: usize,
        limit: usize,
    },
    CapabilityAnalysis(VerifiedCanonicalKernelIrErrorV13),
    DialectRegistration(NameError),
    Session(ContextBuildError),
    Import(KirBridgeErrorV1),
    Plan(PlironOptimizationPlanErrorV1),
    Optimize(PlironOptimizationErrorV1),
    Export(KirBridgeErrorV1),
    OutputByteLimitExceeded {
        required: usize,
        limit: usize,
    },
    OutputRevalidation(VerifiedCanonicalKernelIrErrorV13),
    OutputDecode(KernelIrDecodeError),
    OutputVerification(VerificationErrors),
    EpochOverflow,
    CapabilityReplay(KernelCapabilityPreservationErrorV1),
    TransformationPreservation(TransformationPreservationErrorV1),
    CapabilityPassAccountingMismatch {
        position: usize,
    },
    MutationAccountingMismatch {
        position: usize,
        reported_changed: bool,
        canonical_changed: bool,
    },
    CapabilityPassOutputMismatch,
    ProductionPolicyContainsUnavailableTransformation,
}

impl fmt::Display for KernelIrPlironOptimizationErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidByteLimit {
                limit,
                requested,
                hard_maximum,
            } => write!(
                formatter,
                "{limit:?} canonical-byte limit {requested} is outside 1..={hard_maximum}"
            ),
            Self::InputEncoding(error) => {
                write!(
                    formatter,
                    "Kernel IR V13 input could not be encoded: {error}"
                )
            }
            Self::InputCanonicalization(error) => {
                write!(formatter, "Kernel IR V13 input was rejected: {error}")
            }
            Self::InputByteLimitExceeded { required, limit } => write!(
                formatter,
                "canonical V13 input requires {required} bytes but the limit is {limit}"
            ),
            Self::CapabilityAnalysis(error) => {
                write!(formatter, "input capability analysis failed: {error}")
            }
            Self::DialectRegistration(error) => {
                write!(
                    formatter,
                    "GPU dialect registration was rejected: {error:?}"
                )
            }
            Self::Session(error) => write!(formatter, "fresh Pliron session failed: {error:?}"),
            Self::Import(error) => write!(formatter, "typed Kernel IR V13 import failed: {error}"),
            Self::Plan(error) => write!(formatter, "closed Pliron plan failed: {error}"),
            Self::Optimize(error) => {
                write!(formatter, "closed Pliron optimization failed: {error}")
            }
            Self::Export(error) => {
                write!(
                    formatter,
                    "optimized Kernel IR V13 extraction failed: {error}"
                )
            }
            Self::OutputByteLimitExceeded { required, limit } => write!(
                formatter,
                "canonical V13 output requires {required} bytes but the limit is {limit}"
            ),
            Self::OutputRevalidation(error) => write!(
                formatter,
                "optimized canonical V13 output failed revalidation: {error}"
            ),
            Self::OutputDecode(error) => {
                write!(
                    formatter,
                    "optimized canonical V13 output did not decode: {error}"
                )
            }
            Self::OutputVerification(error) => {
                write!(
                    formatter,
                    "optimized Kernel IR V13 failed verification: {error}"
                )
            }
            Self::EpochOverflow => {
                formatter.write_str("Pliron optimization mutation epoch overflowed")
            }
            Self::CapabilityReplay(error) => {
                write!(
                    formatter,
                    "post-optimization capability replay failed: {error}"
                )
            }
            Self::TransformationPreservation(error) => {
                write!(formatter, "pass-boundary preservation failed: {error}")
            }
            Self::CapabilityPassAccountingMismatch { position } => write!(
                formatter,
                "independent capability replay diverged from optimizer pass accounting at position {position}",
            ),
            Self::MutationAccountingMismatch {
                position,
                reported_changed,
                canonical_changed,
            } => write!(
                formatter,
                "optimizer pass at position {position} reported changed={reported_changed}, but exact canonical identities observed changed={canonical_changed}",
            ),
            Self::CapabilityPassOutputMismatch => formatter.write_str(
                "independent per-pass capability replay produced a different terminal V13 graph",
            ),
            Self::ProductionPolicyContainsUnavailableTransformation => formatter.write_str(
                "closed production optimizer policy selected an unavailable transformation",
            ),
        }
    }
}

impl Error for KernelIrPlironOptimizationErrorV4 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InputEncoding(error) => Some(error),
            Self::InputCanonicalization(error)
            | Self::CapabilityAnalysis(error)
            | Self::OutputRevalidation(error) => Some(error),
            Self::Import(error) | Self::Export(error) => Some(error),
            Self::Plan(error) => Some(error),
            Self::Optimize(error) => Some(error),
            Self::OutputDecode(error) => Some(error),
            Self::OutputVerification(error) => Some(error),
            Self::CapabilityReplay(error) => Some(error),
            Self::TransformationPreservation(error) => Some(error),
            Self::InvalidByteLimit { .. }
            | Self::InputByteLimitExceeded { .. }
            | Self::DialectRegistration(_)
            | Self::Session(_)
            | Self::OutputByteLimitExceeded { .. }
            | Self::EpochOverflow
            | Self::CapabilityPassAccountingMismatch { .. }
            | Self::MutationAccountingMismatch { .. }
            | Self::CapabilityPassOutputMismatch
            | Self::ProductionPolicyContainsUnavailableTransformation => None,
        }
    }
}

pub fn optimize_kernel_ir_module_v4(
    input: &Module,
    limits: KernelIrPlironOptimizationLimitsV2,
) -> Result<OptimizedKernelIrModuleV4, KernelIrPlironOptimizationErrorV4> {
    optimize_kernel_ir_module_with_policy_at_epoch_v4(
        input,
        0,
        limits,
        KernelIrPlironOptimizationPolicyV4::Configurable,
    )
}

pub fn optimize_kernel_ir_module_at_epoch_v4(
    input: &Module,
    initial_epoch: u64,
    limits: KernelIrPlironOptimizationLimitsV2,
) -> Result<OptimizedKernelIrModuleV4, KernelIrPlironOptimizationErrorV4> {
    optimize_kernel_ir_module_with_policy_at_epoch_v4(
        input,
        initial_epoch,
        limits,
        KernelIrPlironOptimizationPolicyV4::Configurable,
    )
}

pub fn optimize_production_kernel_ir_module_v4(
    input: &Module,
) -> Result<OptimizedKernelIrModuleV4, KernelIrPlironOptimizationErrorV4> {
    optimize_kernel_ir_module_with_policy_at_epoch_v4(
        input,
        0,
        production_kernel_ir_pliron_optimization_limits_v2(),
        KernelIrPlironOptimizationPolicyV4::ProductionV4,
    )
}

fn optimize_kernel_ir_module_with_policy_at_epoch_v4(
    input: &Module,
    initial_epoch: u64,
    limits: KernelIrPlironOptimizationLimitsV2,
    policy: KernelIrPlironOptimizationPolicyV4,
) -> Result<OptimizedKernelIrModuleV4, KernelIrPlironOptimizationErrorV4> {
    if policy == KernelIrPlironOptimizationPolicyV4::ProductionV4
        && !production_policy_has_only_checked_transformations_v1()
    {
        return Err(
            KernelIrPlironOptimizationErrorV4::ProductionPolicyContainsUnavailableTransformation,
        );
    }
    validate_byte_limit(
        KernelIrPlironOptimizationByteLimitV2::Input,
        limits.max_input_canonical_bytes(),
    )?;
    validate_byte_limit(
        KernelIrPlironOptimizationByteLimitV2::Output,
        limits.max_output_canonical_bytes(),
    )?;

    let input_bytes =
        encode_module_v13(input).map_err(KernelIrPlironOptimizationErrorV4::InputEncoding)?;
    if input_bytes.len() > limits.max_input_canonical_bytes() {
        return Err(KernelIrPlironOptimizationErrorV4::InputByteLimitExceeded {
            required: input_bytes.len(),
            limit: limits.max_input_canonical_bytes(),
        });
    }
    let (canonical, decoded_input) =
        VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(input_bytes)
            .map_err(KernelIrPlironOptimizationErrorV4::InputCanonicalization)?;
    if &decoded_input != input {
        return Err(KernelIrPlironOptimizationErrorV4::InputCanonicalization(
            VerifiedCanonicalKernelIrErrorV13::RoundTripMismatch,
        ));
    }
    let capability_analysis =
        analyze_kernel_capability_preservation_v1(&decoded_input, initial_epoch)
            .map_err(KernelIrPlironOptimizationErrorV4::CapabilityAnalysis)?;

    let registration = dialect_gpu::dialect_registration()
        .map_err(KernelIrPlironOptimizationErrorV4::DialectRegistration)?;
    let mut session = PlironSession::new(limits.shell(), [registration])
        .map_err(KernelIrPlironOptimizationErrorV4::Session)?;
    let graph = session
        .import_canonical_kir_v13_o0(&canonical)
        .map_err(KernelIrPlironOptimizationErrorV4::Import)?;
    let optimizer_policy = match policy {
        KernelIrPlironOptimizationPolicyV4::Configurable => {
            KernelIrPlironOptimizationPolicyV2::Configurable
        }
        KernelIrPlironOptimizationPolicyV4::ProductionV4 => {
            KernelIrPlironOptimizationPolicyV2::ProductionV2
        }
    };
    let passes = match policy {
        KernelIrPlironOptimizationPolicyV4::Configurable => {
            PlironOptimizationPlanV1::standard().passes().to_vec()
        }
        KernelIrPlironOptimizationPolicyV4::ProductionV4 => {
            KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2
                .into_iter()
                .map(|pass| pass.pliron())
                .collect()
        }
    };
    let plan = PlironOptimizationPlanV1::new(passes, limits.pliron())
        .map_err(KernelIrPlironOptimizationErrorV4::Plan)?;
    let pliron = session
        .execute_optimization_v1(graph.root(), &plan)
        .map_err(KernelIrPlironOptimizationErrorV4::Optimize)?;
    let (output, bridge) = session
        .extract_optimized_canonical_kir_v13_v1(&graph)
        .map_err(KernelIrPlironOptimizationErrorV4::Export)?;
    if output.canonical_bytes().len() > limits.max_output_canonical_bytes() {
        return Err(KernelIrPlironOptimizationErrorV4::OutputByteLimitExceeded {
            required: output.canonical_bytes().len(),
            limit: limits.max_output_canonical_bytes(),
        });
    }
    output
        .revalidate()
        .map_err(KernelIrPlironOptimizationErrorV4::OutputRevalidation)?;
    let module = decode_module_v13(output.canonical_bytes())
        .map_err(KernelIrPlironOptimizationErrorV4::OutputDecode)?;
    verify_module(&module).map_err(KernelIrPlironOptimizationErrorV4::OutputVerification)?;

    let (passes, reported_final_epoch) =
        epoch_reports(pliron.passes(), initial_epoch).map_err(|error| {
            debug_assert!(matches!(
                error,
                KernelIrPlironOptimizationErrorV2::EpochOverflow
            ));
            KernelIrPlironOptimizationErrorV4::EpochOverflow
        })?;
    let capability_pass_replays = replay_capabilities_after_each_pass(
        &canonical,
        &decoded_input,
        initial_epoch,
        &passes,
        limits,
        output.identity(),
    )?;
    let final_epoch = capability_pass_replays
        .last()
        .map_or(initial_epoch, |pass| pass.preservation().output_epoch());
    if final_epoch != reported_final_epoch {
        return Err(
            KernelIrPlironOptimizationErrorV4::CapabilityPassAccountingMismatch {
                position: passes.len(),
            },
        );
    }
    let committed_mutations = u64::try_from(
        capability_pass_replays
            .iter()
            .filter(|pass| pass.preservation().changed())
            .count(),
    )
    .map_err(|_| KernelIrPlironOptimizationErrorV4::EpochOverflow)?;
    let capability_replay = capability_analysis
        .replay_candidate(initial_epoch, final_epoch, committed_mutations, &module)
        .map_err(KernelIrPlironOptimizationErrorV4::CapabilityReplay)?;
    let input_identity = *capability_replay.input_identity();
    let output_identity = *capability_replay.output_identity();
    let optimizer = KernelIrPlironOptimizationReportV2::from_parts(
        optimizer_policy,
        limits,
        initial_epoch,
        final_epoch,
        bridge,
        pliron,
        passes,
    );

    Ok(OptimizedKernelIrModuleV4 {
        module,
        canonical: output,
        report: KernelIrPlironOptimizationReportV4 {
            policy,
            limits,
            initial_epoch,
            final_epoch,
            input_identity,
            output_identity,
            optimizer,
            capability_pass_replays,
            capability_replay,
        },
    })
}

fn replay_capabilities_after_each_pass(
    input: &VerifiedCanonicalKernelIrV13,
    input_module: &Module,
    initial_epoch: u64,
    expected_passes: &[KernelIrPlironOptimizationPassReportV2],
    limits: KernelIrPlironOptimizationLimitsV2,
    expected_output: &VerifiedCanonicalKernelIrIdentityV13,
) -> Result<Vec<KernelCapabilityPassReplayV4>, KernelIrPlironOptimizationErrorV4> {
    let registration = dialect_gpu::dialect_registration()
        .map_err(KernelIrPlironOptimizationErrorV4::DialectRegistration)?;
    let mut session = PlironSession::new(limits.shell(), [registration])
        .map_err(KernelIrPlironOptimizationErrorV4::Session)?;
    let graph = session
        .import_canonical_kir_v13_o0(input)
        .map_err(KernelIrPlironOptimizationErrorV4::Import)?;
    let mut epoch = initial_epoch;
    let mut replays = Vec::with_capacity(expected_passes.len());
    let mut terminal_identity = *input.identity();
    let mut previous_module = input_module.clone();

    for (position, expected) in expected_passes.iter().copied().enumerate() {
        let pass = expected.pliron().pass();
        let plan = PlironOptimizationPlanV1::new(vec![pass], limits.pliron())
            .map_err(KernelIrPlironOptimizationErrorV4::Plan)?;
        let report = session
            .execute_optimization_v1(graph.root(), &plan)
            .map_err(KernelIrPlironOptimizationErrorV4::Optimize)?;
        let [observed] = report.passes() else {
            return Err(
                KernelIrPlironOptimizationErrorV4::CapabilityPassAccountingMismatch { position },
            );
        };
        if observed.pass() != pass || expected.input_epoch() != epoch {
            return Err(
                KernelIrPlironOptimizationErrorV4::CapabilityPassAccountingMismatch { position },
            );
        }

        let (canonical, _) = session
            .extract_optimized_canonical_kir_v13_v1(&graph)
            .map_err(KernelIrPlironOptimizationErrorV4::Export)?;
        let candidate = decode_module_v13(canonical.canonical_bytes())
            .map_err(KernelIrPlironOptimizationErrorV4::OutputDecode)?;
        let canonical_changed = canonical.identity() != &terminal_identity;
        if observed.changed() != canonical_changed {
            return Err(
                KernelIrPlironOptimizationErrorV4::MutationAccountingMismatch {
                    position,
                    reported_changed: observed.changed(),
                    canonical_changed,
                },
            );
        }
        if *observed != expected.pliron() {
            return Err(
                KernelIrPlironOptimizationErrorV4::CapabilityPassAccountingMismatch { position },
            );
        }
        let next_epoch = if canonical_changed {
            epoch
                .checked_add(1)
                .ok_or(KernelIrPlironOptimizationErrorV4::EpochOverflow)?
        } else {
            epoch
        };
        if expected.output_epoch() != next_epoch {
            return Err(
                KernelIrPlironOptimizationErrorV4::CapabilityPassAccountingMismatch { position },
            );
        }
        let preservation = check_pliron_transformation_preservation_v1(
            pass,
            &previous_module,
            &candidate,
            epoch,
            next_epoch,
        )
        .map_err(KernelIrPlironOptimizationErrorV4::TransformationPreservation)?;
        replays.push(KernelCapabilityPassReplayV4 { pass, preservation });
        terminal_identity = *canonical.identity();
        previous_module = candidate;
        epoch = next_epoch;
    }

    if &terminal_identity != expected_output {
        return Err(KernelIrPlironOptimizationErrorV4::CapabilityPassOutputMismatch);
    }
    Ok(replays)
}

fn capability_pass_replay_chain_is_exact(report: &KernelIrPlironOptimizationReportV4) -> bool {
    let expected = report.optimizer.passes();
    if report.capability_pass_replays.len() != expected.len() {
        return false;
    }
    let mut epoch = report.initial_epoch;
    let mut identity = report.input_identity;
    for (replay, expected) in report
        .capability_pass_replays
        .iter()
        .zip(expected.iter().copied())
    {
        let checked = replay.replay();
        let preservation = replay.preservation();
        if replay.pass() != expected.pliron().pass()
            || checked.input_epoch() != epoch
            || checked.output_epoch() != expected.output_epoch()
            || checked.input_identity() != &identity
            || preservation.input_epoch() != checked.input_epoch()
            || preservation.output_epoch() != checked.output_epoch()
            || preservation.input_identity() != checked.input_identity()
            || preservation.output_identity() != checked.output_identity()
            || preservation.fresh_output_analysis().graph_epoch() != checked.output_epoch()
            || preservation.fresh_output_analysis().graph_identity() != checked.output_identity()
        {
            return false;
        }
        epoch = checked.output_epoch();
        identity = *checked.output_identity();
    }
    epoch == report.final_epoch && identity == report.output_identity
}

fn validate_byte_limit(
    limit: KernelIrPlironOptimizationByteLimitV2,
    value: usize,
) -> Result<(), KernelIrPlironOptimizationErrorV4> {
    if value == 0 || value > MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V4 {
        return Err(KernelIrPlironOptimizationErrorV4::InvalidByteLimit {
            limit,
            requested: value,
            hard_maximum: MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V4,
        });
    }
    Ok(())
}
