//! Transactional Pliron-backed optimization of canonical Kernel IR V10.
//!
//! V2 always imports into a fresh [`PlironSession`]. A failed import, pass, or
//! extraction therefore drops the entire private candidate; neither the input
//! module nor a partially mutated graph can escape. Production V4 replay
//! evidence embeds and independently validates V2 accounting. A V2 report is
//! not by itself a replay receipt or a formal semantic-preservation proof.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    KernelIrDecodeError, KernelIrEncodeError, MAX_MODULE_BYTES_V1, Module, VerificationErrors,
    VerifiedCanonicalKernelIrErrorV10, VerifiedCanonicalKernelIrV10, decode_module_v10,
    encode_module_v10, verify_module,
};
use fe2o3_pliron::{
    ContextBuildError, KirBridgeDigestV1, KirBridgeErrorV1, KirBridgeOptimizedReceiptV1, NameError,
    PlironOptimizationErrorV1, PlironOptimizationLimitsV1, PlironOptimizationPassReportV1,
    PlironOptimizationPlanErrorV1, PlironOptimizationPlanV1, PlironOptimizationReportV1,
    PlironSession, ShellLimits,
};

/// Hard byte cap inherited from canonical Kernel IR V10 encoding.
pub const MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2: usize = MAX_MODULE_BYTES_V1;

/// Version of the closed optimizer policy used for new production compilations.
pub const KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_POLICY_VERSION_V2: u16 = 2;

/// Stable pass identity owned by the V2 production optimizer policy.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum KernelIrPlironProductionPassV2 {
    SparseConditionalConstantPropagation,
    SimplifyControlFlow,
    SelectSameValueCanonicalization,
    DeadCodeElimination,
    LocalPureCommonSubexpressionElimination,
}

impl KernelIrPlironProductionPassV2 {
    pub const fn name(self) -> &'static str {
        self.pliron().name()
    }

    pub(crate) const fn pliron(self) -> fe2o3_pliron::PlironOptimizationPassV1 {
        use fe2o3_pliron::PlironOptimizationPassV1;

        match self {
            Self::SparseConditionalConstantPropagation => {
                PlironOptimizationPassV1::SparseConditionalConstantPropagation
            }
            Self::SimplifyControlFlow => PlironOptimizationPassV1::SimplifyControlFlow,
            Self::SelectSameValueCanonicalization => {
                PlironOptimizationPassV1::SelectSameValueCanonicalization
            }
            Self::DeadCodeElimination => PlironOptimizationPassV1::DeadCodeElimination,
            Self::LocalPureCommonSubexpressionElimination => {
                PlironOptimizationPassV1::LocalPureCommonSubexpressionElimination
            }
        }
    }
}

/// Exact, ordered pass roster shared by production optimizer policy versions 1 and 2.
pub const KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2: [KernelIrPlironProductionPassV2;
    7] = [
    KernelIrPlironProductionPassV2::SparseConditionalConstantPropagation,
    KernelIrPlironProductionPassV2::SimplifyControlFlow,
    KernelIrPlironProductionPassV2::SelectSameValueCanonicalization,
    KernelIrPlironProductionPassV2::DeadCodeElimination,
    KernelIrPlironProductionPassV2::LocalPureCommonSubexpressionElimination,
    KernelIrPlironProductionPassV2::DeadCodeElimination,
    KernelIrPlironProductionPassV2::SimplifyControlFlow,
];

/// Production-policy V2 reports can be accepted by production V4 replay evidence.
///
/// This constant advertises format support only. Call
/// [`KernelIrPlironOptimizationReportV2::is_production_replay_compatible`] to
/// admit an individual report.
pub const KERNEL_IR_PLIRON_OPTIMIZATION_V2_PRODUCTION_REPLAY_COMPATIBLE: bool = true;

/// Provenance of the policy that produced a V2 optimization report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelIrPlironOptimizationPolicyV2 {
    /// Caller-selected limits or mutation lineage.
    Configurable,
    /// Historical closed production policy version 1.
    ProductionV1,
    /// Current closed production policy version 2 with its fixed limits and epoch.
    ProductionV2,
}

/// Which canonical-byte admission limit was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KernelIrPlironOptimizationByteLimitV2 {
    Input,
    Output,
}

/// Fixed and configurable limits for one fresh-session optimization.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrPlironOptimizationLimitsV2 {
    max_input_canonical_bytes: usize,
    max_output_canonical_bytes: usize,
    shell: ShellLimits,
    pliron: PlironOptimizationLimitsV1,
}

impl KernelIrPlironOptimizationLimitsV2 {
    /// Creates limits bounded by the canonical encoder and Pliron hard caps.
    ///
    /// The byte limits are admission checks on already materialized,
    /// fixed-hard-bounded encodings. They do not claim to prevent the canonical
    /// encoder or optimized extractor from allocating up to their hard caps.
    pub fn new(
        max_input_canonical_bytes: usize,
        max_output_canonical_bytes: usize,
        shell: ShellLimits,
        pliron: PlironOptimizationLimitsV1,
    ) -> Result<Self, KernelIrPlironOptimizationErrorV2> {
        validate_byte_limit(
            KernelIrPlironOptimizationByteLimitV2::Input,
            max_input_canonical_bytes,
        )?;
        validate_byte_limit(
            KernelIrPlironOptimizationByteLimitV2::Output,
            max_output_canonical_bytes,
        )?;
        Ok(Self {
            max_input_canonical_bytes,
            max_output_canonical_bytes,
            shell,
            pliron,
        })
    }

    pub const fn max_input_canonical_bytes(self) -> usize {
        self.max_input_canonical_bytes
    }

    pub const fn max_output_canonical_bytes(self) -> usize {
        self.max_output_canonical_bytes
    }

    pub const fn shell(self) -> ShellLimits {
        self.shell
    }

    pub const fn pliron(self) -> PlironOptimizationLimitsV1 {
        self.pliron
    }
}

impl Default for KernelIrPlironOptimizationLimitsV2 {
    fn default() -> Self {
        Self {
            max_input_canonical_bytes: MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2,
            max_output_canonical_bytes: MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2,
            shell: ShellLimits::default(),
            pliron: PlironOptimizationLimitsV1::default(),
        }
    }
}

/// Returns the frozen limits for production optimizer policy version 2.
///
/// Literal values keep the policy independent of mutable `Default`
/// implementations. Construction will fail loudly if an underlying hard cap
/// is ever reduced below the versioned policy.
pub fn production_kernel_ir_pliron_optimization_limits_v2() -> KernelIrPlironOptimizationLimitsV2 {
    let shell = ShellLimits::new(32, 64, 512)
        .expect("production optimizer V2 shell limits must remain supported");
    let pliron = PlironOptimizationLimitsV1::new(256, 32_768, 25_268_224)
        .expect("production optimizer V2 execution limits must remain supported");
    KernelIrPlironOptimizationLimitsV2::new(16_777_216, 16_777_216, shell, pliron)
        .expect("production optimizer V2 byte limits must remain supported")
}

fn validate_byte_limit(
    limit: KernelIrPlironOptimizationByteLimitV2,
    value: usize,
) -> Result<(), KernelIrPlironOptimizationErrorV2> {
    if value == 0 || value > MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2 {
        return Err(KernelIrPlironOptimizationErrorV2::InvalidByteLimit {
            limit,
            requested: value,
            hard_maximum: MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2,
        });
    }
    Ok(())
}

/// One upstream pass report bound to deterministic mutation epochs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KernelIrPlironOptimizationPassReportV2 {
    input_epoch: u64,
    output_epoch: u64,
    pliron: PlironOptimizationPassReportV1,
}

impl KernelIrPlironOptimizationPassReportV2 {
    pub const fn input_epoch(self) -> u64 {
        self.input_epoch
    }

    pub const fn output_epoch(self) -> u64 {
        self.output_epoch
    }

    pub const fn pliron(self) -> PlironOptimizationPassReportV1 {
        self.pliron
    }
}

/// Immutable accounting for one successfully extracted V2 candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelIrPlironOptimizationReportV2 {
    policy: KernelIrPlironOptimizationPolicyV2,
    limits: KernelIrPlironOptimizationLimitsV2,
    initial_epoch: u64,
    final_epoch: u64,
    bridge: KirBridgeOptimizedReceiptV1,
    pliron: PlironOptimizationReportV1,
    passes: Vec<KernelIrPlironOptimizationPassReportV2>,
}

impl KernelIrPlironOptimizationReportV2 {
    pub(crate) fn from_parts(
        policy: KernelIrPlironOptimizationPolicyV2,
        limits: KernelIrPlironOptimizationLimitsV2,
        initial_epoch: u64,
        final_epoch: u64,
        bridge: KirBridgeOptimizedReceiptV1,
        pliron: PlironOptimizationReportV1,
        passes: Vec<KernelIrPlironOptimizationPassReportV2>,
    ) -> Self {
        Self {
            policy,
            limits,
            initial_epoch,
            final_epoch,
            bridge,
            pliron,
            passes,
        }
    }

    pub const fn policy(&self) -> KernelIrPlironOptimizationPolicyV2 {
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

    pub const fn input_digest(&self) -> KirBridgeDigestV1 {
        self.bridge.input()
    }

    pub const fn output_digest(&self) -> KirBridgeDigestV1 {
        self.bridge.output()
    }

    pub fn changed(&self) -> bool {
        self.bridge.changed()
    }

    pub const fn bridge(&self) -> &KirBridgeOptimizedReceiptV1 {
        &self.bridge
    }

    pub const fn pliron(&self) -> &PlironOptimizationReportV1 {
        &self.pliron
    }

    pub fn passes(&self) -> &[KernelIrPlironOptimizationPassReportV2] {
        &self.passes
    }

    /// Whether this report came from the exact policy admitted by production replay.
    pub fn is_production_replay_compatible(&self) -> bool {
        KERNEL_IR_PLIRON_OPTIMIZATION_V2_PRODUCTION_REPLAY_COMPATIBLE
            && self.policy == KernelIrPlironOptimizationPolicyV2::ProductionV2
            && self.limits == production_kernel_ir_pliron_optimization_limits_v2()
            && self.initial_epoch == 0
    }
}

/// Fully verified output published only after the fresh session completes.
#[derive(Debug, Eq, PartialEq)]
pub struct OptimizedKernelIrModuleV2 {
    module: Module,
    canonical: VerifiedCanonicalKernelIrV10,
    report: KernelIrPlironOptimizationReportV2,
}

impl OptimizedKernelIrModuleV2 {
    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrV10 {
        &self.canonical
    }

    pub const fn report(&self) -> &KernelIrPlironOptimizationReportV2 {
        &self.report
    }

    pub fn into_parts(
        self,
    ) -> (
        Module,
        VerifiedCanonicalKernelIrV10,
        KernelIrPlironOptimizationReportV2,
    ) {
        (self.module, self.canonical, self.report)
    }
}

/// Fail-closed V2 failure. No variant contains a candidate module or session.
#[derive(Debug)]
pub enum KernelIrPlironOptimizationErrorV2 {
    InvalidByteLimit {
        limit: KernelIrPlironOptimizationByteLimitV2,
        requested: usize,
        hard_maximum: usize,
    },
    InputEncoding(KernelIrEncodeError),
    InputCanonicalization(VerifiedCanonicalKernelIrErrorV10),
    InputByteLimitExceeded {
        required: usize,
        limit: usize,
    },
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
    OutputRevalidation(VerifiedCanonicalKernelIrErrorV10),
    OutputDecode(KernelIrDecodeError),
    OutputVerification(VerificationErrors),
    EpochOverflow,
}

impl fmt::Display for KernelIrPlironOptimizationErrorV2 {
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
            Self::InputCanonicalization(error) => {
                write!(formatter, "Kernel IR V10 input was rejected: {error}")
            }
            Self::InputEncoding(error) => {
                write!(
                    formatter,
                    "Kernel IR V10 input could not be encoded: {error}"
                )
            }
            Self::InputByteLimitExceeded { required, limit } => write!(
                formatter,
                "canonical V10 input requires {required} bytes but the limit is {limit}"
            ),
            Self::DialectRegistration(error) => {
                write!(
                    formatter,
                    "GPU dialect registration was rejected: {error:?}"
                )
            }
            Self::Session(error) => write!(formatter, "fresh Pliron session failed: {error:?}"),
            Self::Import(error) => write!(formatter, "typed Kernel IR import failed: {error}"),
            Self::Plan(error) => write!(formatter, "closed Pliron plan failed: {error}"),
            Self::Optimize(error) => {
                write!(formatter, "closed Pliron optimization failed: {error}")
            }
            Self::Export(error) => {
                write!(formatter, "optimized Kernel IR extraction failed: {error}")
            }
            Self::OutputByteLimitExceeded { required, limit } => write!(
                formatter,
                "canonical V10 output requires {required} bytes but the limit is {limit}"
            ),
            Self::OutputRevalidation(error) => {
                write!(
                    formatter,
                    "optimized canonical V10 output failed revalidation: {error}"
                )
            }
            Self::OutputDecode(error) => {
                write!(
                    formatter,
                    "optimized canonical V10 output did not decode: {error}"
                )
            }
            Self::OutputVerification(error) => {
                write!(
                    formatter,
                    "optimized Kernel IR failed final verification: {error}"
                )
            }
            Self::EpochOverflow => {
                formatter.write_str("Pliron optimization mutation epoch overflowed")
            }
        }
    }
}

impl Error for KernelIrPlironOptimizationErrorV2 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::InputEncoding(error) => Some(error),
            Self::InputCanonicalization(error) => Some(error),
            Self::Import(error) | Self::Export(error) => Some(error),
            Self::Plan(error) => Some(error),
            Self::Optimize(error) => Some(error),
            Self::OutputRevalidation(error) => Some(error),
            Self::OutputDecode(error) => Some(error),
            Self::OutputVerification(error) => Some(error),
            Self::InvalidByteLimit { .. }
            | Self::InputByteLimitExceeded { .. }
            | Self::DialectRegistration(_)
            | Self::Session(_)
            | Self::OutputByteLimitExceeded { .. }
            | Self::EpochOverflow => None,
        }
    }
}

/// Runs the fixed V2 Pliron optimizer in a fresh, disposable session.
///
/// This function does not mutate `input`. On every error the private session and
/// its candidate graph are dropped without publishing a module or report.
pub fn optimize_kernel_ir_module_v2(
    input: &Module,
    limits: KernelIrPlironOptimizationLimitsV2,
) -> Result<OptimizedKernelIrModuleV2, KernelIrPlironOptimizationErrorV2> {
    optimize_kernel_ir_module_with_policy_at_epoch_v2(
        input,
        0,
        limits,
        KernelIrPlironOptimizationPolicyV2::Configurable,
    )
}

/// Runs the only optimizer admitted for new production compilations.
///
/// This entry point deliberately exposes neither an optimizer selector nor
/// caller-controlled limits. Any V2 admission, optimization, or extraction
/// failure is returned to the production transaction; there is no legacy or
/// unoptimized fallback path.
pub fn optimize_production_kernel_ir_module_v2(
    input: &Module,
) -> Result<OptimizedKernelIrModuleV2, KernelIrPlironOptimizationErrorV2> {
    optimize_kernel_ir_module_with_policy_at_epoch_v2(
        input,
        0,
        production_kernel_ir_pliron_optimization_limits_v2(),
        KernelIrPlironOptimizationPolicyV2::ProductionV2,
    )
}

/// Runs V2 using `initial_epoch` as the caller's mutation lineage.
///
/// The epoch advances once for each Pliron pass reporting a mutation. Epoch
/// overflow rejects the private candidate after extraction and publishes no
/// output.
pub fn optimize_kernel_ir_module_at_epoch_v2(
    input: &Module,
    initial_epoch: u64,
    limits: KernelIrPlironOptimizationLimitsV2,
) -> Result<OptimizedKernelIrModuleV2, KernelIrPlironOptimizationErrorV2> {
    optimize_kernel_ir_module_with_policy_at_epoch_v2(
        input,
        initial_epoch,
        limits,
        KernelIrPlironOptimizationPolicyV2::Configurable,
    )
}

fn optimize_kernel_ir_module_with_policy_at_epoch_v2(
    input: &Module,
    initial_epoch: u64,
    limits: KernelIrPlironOptimizationLimitsV2,
    policy: KernelIrPlironOptimizationPolicyV2,
) -> Result<OptimizedKernelIrModuleV2, KernelIrPlironOptimizationErrorV2> {
    validate_byte_limit(
        KernelIrPlironOptimizationByteLimitV2::Input,
        limits.max_input_canonical_bytes,
    )?;
    validate_byte_limit(
        KernelIrPlironOptimizationByteLimitV2::Output,
        limits.max_output_canonical_bytes,
    )?;

    let input_bytes =
        encode_module_v10(input).map_err(KernelIrPlironOptimizationErrorV2::InputEncoding)?;
    if input_bytes.len() > limits.max_input_canonical_bytes {
        return Err(KernelIrPlironOptimizationErrorV2::InputByteLimitExceeded {
            required: input_bytes.len(),
            limit: limits.max_input_canonical_bytes,
        });
    }
    let (canonical, decoded_input) =
        VerifiedCanonicalKernelIrV10::from_canonical_bytes_with_module(input_bytes)
            .map_err(KernelIrPlironOptimizationErrorV2::InputCanonicalization)?;
    if &decoded_input != input {
        return Err(KernelIrPlironOptimizationErrorV2::InputCanonicalization(
            VerifiedCanonicalKernelIrErrorV10::RoundTripMismatch,
        ));
    }

    let registration = dialect_gpu::dialect_registration()
        .map_err(KernelIrPlironOptimizationErrorV2::DialectRegistration)?;
    let mut session = PlironSession::new(limits.shell, [registration])
        .map_err(KernelIrPlironOptimizationErrorV2::Session)?;
    let graph = session
        .import_canonical_kir_v10_o0(&canonical)
        .map_err(KernelIrPlironOptimizationErrorV2::Import)?;
    let passes = match policy {
        KernelIrPlironOptimizationPolicyV2::Configurable => {
            PlironOptimizationPlanV1::standard().passes().to_vec()
        }
        KernelIrPlironOptimizationPolicyV2::ProductionV1
        | KernelIrPlironOptimizationPolicyV2::ProductionV2 => {
            KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2
                .into_iter()
                .map(KernelIrPlironProductionPassV2::pliron)
                .collect()
        }
    };
    let plan = PlironOptimizationPlanV1::new(passes, limits.pliron)
        .map_err(KernelIrPlironOptimizationErrorV2::Plan)?;
    let pliron = session
        .execute_optimization_v1(graph.root(), &plan)
        .map_err(KernelIrPlironOptimizationErrorV2::Optimize)?;
    let (output, bridge) = session
        .extract_optimized_canonical_kir_v10_v1(&graph)
        .map_err(KernelIrPlironOptimizationErrorV2::Export)?;
    if output.canonical_bytes().len() > limits.max_output_canonical_bytes {
        return Err(KernelIrPlironOptimizationErrorV2::OutputByteLimitExceeded {
            required: output.canonical_bytes().len(),
            limit: limits.max_output_canonical_bytes,
        });
    }
    output
        .revalidate()
        .map_err(KernelIrPlironOptimizationErrorV2::OutputRevalidation)?;
    let module = decode_module_v10(output.canonical_bytes())
        .map_err(KernelIrPlironOptimizationErrorV2::OutputDecode)?;
    verify_module(&module).map_err(KernelIrPlironOptimizationErrorV2::OutputVerification)?;

    let (passes, final_epoch) = epoch_reports(pliron.passes(), initial_epoch)?;
    let report = KernelIrPlironOptimizationReportV2 {
        policy,
        limits,
        initial_epoch,
        final_epoch,
        bridge,
        pliron,
        passes,
    };
    Ok(OptimizedKernelIrModuleV2 {
        module,
        canonical: output,
        report,
    })
}

pub(crate) fn epoch_reports(
    reports: &[PlironOptimizationPassReportV1],
    initial_epoch: u64,
) -> Result<(Vec<KernelIrPlironOptimizationPassReportV2>, u64), KernelIrPlironOptimizationErrorV2> {
    let mut epoch = initial_epoch;
    let mut output = Vec::with_capacity(reports.len());
    for report in reports.iter().copied() {
        let input_epoch = epoch;
        if report.changed() {
            epoch = epoch
                .checked_add(1)
                .ok_or(KernelIrPlironOptimizationErrorV2::EpochOverflow)?;
        }
        output.push(KernelIrPlironOptimizationPassReportV2 {
            input_epoch,
            output_epoch: epoch,
            pliron: report,
        });
    }
    Ok((output, epoch))
}
