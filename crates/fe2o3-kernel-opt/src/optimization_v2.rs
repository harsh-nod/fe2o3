//! Transactional Pliron-backed optimization of canonical Kernel IR V9.
//!
//! V2 always imports into a fresh [`PlironSession`]. A failed import, pass, or
//! extraction therefore drops the entire private candidate; neither the input
//! module nor a partially mutated graph can escape. This path is intentionally
//! separate from the frozen production V1 replay and its evidence formats. A
//! V2 report is optimization accounting, not a production replay receipt or a
//! formal semantic-preservation proof.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    KernelIrDecodeError, KernelIrEncodeError, MAX_MODULE_BYTES_V1, Module, VerificationErrors,
    VerifiedCanonicalKernelIrErrorV9, VerifiedCanonicalKernelIrV9, decode_module_v9,
    encode_module_v9, verify_module,
};
use fe2o3_pliron::{
    ContextBuildError, KirBridgeDigestV1, KirBridgeErrorV1, KirBridgeOptimizedReceiptV1, NameError,
    PlironOptimizationErrorV1, PlironOptimizationLimitsV1, PlironOptimizationPassReportV1,
    PlironOptimizationPlanErrorV1, PlironOptimizationPlanV1, PlironOptimizationReportV1,
    PlironSession, ShellLimits,
};

/// Hard byte cap inherited from canonical Kernel IR V9 encoding.
pub const MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V2: usize = MAX_MODULE_BYTES_V1;

/// V2 reports are deliberately not accepted by the frozen production replay.
pub const KERNEL_IR_PLIRON_OPTIMIZATION_V2_PRODUCTION_REPLAY_COMPATIBLE: bool = false;

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
    initial_epoch: u64,
    final_epoch: u64,
    bridge: KirBridgeOptimizedReceiptV1,
    pliron: PlironOptimizationReportV1,
    passes: Vec<KernelIrPlironOptimizationPassReportV2>,
}

impl KernelIrPlironOptimizationReportV2 {
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

    /// This report cannot satisfy frozen production replay evidence.
    pub const fn is_production_replay_compatible(&self) -> bool {
        KERNEL_IR_PLIRON_OPTIMIZATION_V2_PRODUCTION_REPLAY_COMPATIBLE
    }
}

/// Fully verified output published only after the fresh session completes.
#[derive(Debug, Eq, PartialEq)]
pub struct OptimizedKernelIrModuleV2 {
    module: Module,
    canonical: VerifiedCanonicalKernelIrV9,
    report: KernelIrPlironOptimizationReportV2,
}

impl OptimizedKernelIrModuleV2 {
    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrV9 {
        &self.canonical
    }

    pub const fn report(&self) -> &KernelIrPlironOptimizationReportV2 {
        &self.report
    }

    pub fn into_parts(
        self,
    ) -> (
        Module,
        VerifiedCanonicalKernelIrV9,
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
    InputCanonicalization(VerifiedCanonicalKernelIrErrorV9),
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
    OutputRevalidation(VerifiedCanonicalKernelIrErrorV9),
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
                write!(formatter, "Kernel IR V9 input was rejected: {error}")
            }
            Self::InputEncoding(error) => {
                write!(
                    formatter,
                    "Kernel IR V9 input could not be encoded: {error}"
                )
            }
            Self::InputByteLimitExceeded { required, limit } => write!(
                formatter,
                "canonical V9 input requires {required} bytes but the limit is {limit}"
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
                "canonical V9 output requires {required} bytes but the limit is {limit}"
            ),
            Self::OutputRevalidation(error) => {
                write!(
                    formatter,
                    "optimized canonical V9 output failed revalidation: {error}"
                )
            }
            Self::OutputDecode(error) => {
                write!(
                    formatter,
                    "optimized canonical V9 output did not decode: {error}"
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
    optimize_kernel_ir_module_at_epoch_v2(input, 0, limits)
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
    validate_byte_limit(
        KernelIrPlironOptimizationByteLimitV2::Input,
        limits.max_input_canonical_bytes,
    )?;
    validate_byte_limit(
        KernelIrPlironOptimizationByteLimitV2::Output,
        limits.max_output_canonical_bytes,
    )?;

    let input_bytes =
        encode_module_v9(input).map_err(KernelIrPlironOptimizationErrorV2::InputEncoding)?;
    if input_bytes.len() > limits.max_input_canonical_bytes {
        return Err(KernelIrPlironOptimizationErrorV2::InputByteLimitExceeded {
            required: input_bytes.len(),
            limit: limits.max_input_canonical_bytes,
        });
    }
    let (canonical, decoded_input) =
        VerifiedCanonicalKernelIrV9::from_canonical_bytes_with_module(input_bytes)
            .map_err(KernelIrPlironOptimizationErrorV2::InputCanonicalization)?;
    if &decoded_input != input {
        return Err(KernelIrPlironOptimizationErrorV2::InputCanonicalization(
            VerifiedCanonicalKernelIrErrorV9::RoundTripMismatch,
        ));
    }

    let registration = dialect_gpu::dialect_registration()
        .map_err(KernelIrPlironOptimizationErrorV2::DialectRegistration)?;
    let mut session = PlironSession::new(limits.shell, [registration])
        .map_err(KernelIrPlironOptimizationErrorV2::Session)?;
    let graph = session
        .import_canonical_kir_v9_o0(&canonical)
        .map_err(KernelIrPlironOptimizationErrorV2::Import)?;
    let standard = PlironOptimizationPlanV1::standard();
    let plan = PlironOptimizationPlanV1::new(standard.passes().to_vec(), limits.pliron)
        .map_err(KernelIrPlironOptimizationErrorV2::Plan)?;
    let pliron = session
        .execute_optimization_v1(graph.root(), &plan)
        .map_err(KernelIrPlironOptimizationErrorV2::Optimize)?;
    let (output, bridge) = session
        .extract_optimized_canonical_kir_v9_v1(&graph)
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
    let module = decode_module_v9(output.canonical_bytes())
        .map_err(KernelIrPlironOptimizationErrorV2::OutputDecode)?;
    verify_module(&module).map_err(KernelIrPlironOptimizationErrorV2::OutputVerification)?;

    let (passes, final_epoch) = epoch_reports(pliron.passes(), initial_epoch)?;
    let report = KernelIrPlironOptimizationReportV2 {
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

fn epoch_reports(
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
