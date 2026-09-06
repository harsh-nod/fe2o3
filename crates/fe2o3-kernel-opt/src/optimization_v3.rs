//! Transactional Pliron-backed optimization of canonical Kernel IR V11.
//!
//! V3 changes only the canonical transport boundary. It imports and exports
//! exact V11 while retaining the closed V2 pass policy, limits, accounting,
//! and report schema. V2 remains the frozen V10 endpoint.

use std::{error::Error, fmt};

use fe2o3_kernel_ir::{
    KernelIrDecodeError, KernelIrEncodeError, MAX_MODULE_BYTES_V1, Module, VerificationErrors,
    VerifiedCanonicalKernelIrErrorV11, VerifiedCanonicalKernelIrV11, decode_module_v11,
    encode_module_v11, verify_module,
};
use fe2o3_pliron::{
    ContextBuildError, KirBridgeErrorV1, NameError, PlironOptimizationErrorV1,
    PlironOptimizationPlanErrorV1, PlironOptimizationPlanV1, PlironSession,
};

use crate::{
    KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2, KernelIrPlironOptimizationByteLimitV2,
    KernelIrPlironOptimizationLimitsV2, KernelIrPlironOptimizationPolicyV2,
    KernelIrPlironOptimizationReportV2, epoch_reports,
    production_kernel_ir_pliron_optimization_limits_v2,
};

/// Hard byte cap inherited from canonical Kernel IR V11 encoding.
pub const MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V3: usize = MAX_MODULE_BYTES_V1;

/// Fully verified V11 output published only after the fresh session completes.
#[derive(Debug, Eq, PartialEq)]
pub struct OptimizedKernelIrModuleV3 {
    module: Module,
    canonical: VerifiedCanonicalKernelIrV11,
    report: KernelIrPlironOptimizationReportV2,
}

impl OptimizedKernelIrModuleV3 {
    pub const fn module(&self) -> &Module {
        &self.module
    }

    pub const fn canonical(&self) -> &VerifiedCanonicalKernelIrV11 {
        &self.canonical
    }

    /// Returns accounting for the unchanged closed V2 optimization policy.
    pub const fn report(&self) -> &KernelIrPlironOptimizationReportV2 {
        &self.report
    }

    pub fn into_parts(
        self,
    ) -> (
        Module,
        VerifiedCanonicalKernelIrV11,
        KernelIrPlironOptimizationReportV2,
    ) {
        (self.module, self.canonical, self.report)
    }
}

/// Fail-closed V3 transport failure. No variant contains a candidate module.
#[derive(Debug)]
pub enum KernelIrPlironOptimizationErrorV3 {
    InvalidByteLimit {
        limit: KernelIrPlironOptimizationByteLimitV2,
        requested: usize,
        hard_maximum: usize,
    },
    InputEncoding(KernelIrEncodeError),
    InputCanonicalization(VerifiedCanonicalKernelIrErrorV11),
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
    OutputRevalidation(VerifiedCanonicalKernelIrErrorV11),
    OutputDecode(KernelIrDecodeError),
    OutputVerification(VerificationErrors),
    EpochOverflow,
}

impl fmt::Display for KernelIrPlironOptimizationErrorV3 {
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
                    "Kernel IR V11 input could not be encoded: {error}"
                )
            }
            Self::InputCanonicalization(error) => {
                write!(formatter, "Kernel IR V11 input was rejected: {error}")
            }
            Self::InputByteLimitExceeded { required, limit } => write!(
                formatter,
                "canonical V11 input requires {required} bytes but the limit is {limit}"
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
                "canonical V11 output requires {required} bytes but the limit is {limit}"
            ),
            Self::OutputRevalidation(error) => write!(
                formatter,
                "optimized canonical V11 output failed revalidation: {error}"
            ),
            Self::OutputDecode(error) => {
                write!(
                    formatter,
                    "optimized canonical V11 output did not decode: {error}"
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

impl Error for KernelIrPlironOptimizationErrorV3 {
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

/// Runs the configurable V2 pass plan over an exact canonical V11 boundary.
pub fn optimize_kernel_ir_module_v3(
    input: &Module,
    limits: KernelIrPlironOptimizationLimitsV2,
) -> Result<OptimizedKernelIrModuleV3, KernelIrPlironOptimizationErrorV3> {
    optimize_kernel_ir_module_with_policy_at_epoch_v3(
        input,
        0,
        limits,
        KernelIrPlironOptimizationPolicyV2::Configurable,
    )
}

/// Runs the closed production V2 pass policy over exact canonical V11.
pub fn optimize_production_kernel_ir_module_v3(
    input: &Module,
) -> Result<OptimizedKernelIrModuleV3, KernelIrPlironOptimizationErrorV3> {
    optimize_kernel_ir_module_with_policy_at_epoch_v3(
        input,
        0,
        production_kernel_ir_pliron_optimization_limits_v2(),
        KernelIrPlironOptimizationPolicyV2::ProductionV2,
    )
}

/// Runs V3 using `initial_epoch` as the caller's mutation lineage.
pub fn optimize_kernel_ir_module_at_epoch_v3(
    input: &Module,
    initial_epoch: u64,
    limits: KernelIrPlironOptimizationLimitsV2,
) -> Result<OptimizedKernelIrModuleV3, KernelIrPlironOptimizationErrorV3> {
    optimize_kernel_ir_module_with_policy_at_epoch_v3(
        input,
        initial_epoch,
        limits,
        KernelIrPlironOptimizationPolicyV2::Configurable,
    )
}

fn validate_byte_limit_v3(
    limit: KernelIrPlironOptimizationByteLimitV2,
    value: usize,
) -> Result<(), KernelIrPlironOptimizationErrorV3> {
    if value == 0 || value > MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V3 {
        return Err(KernelIrPlironOptimizationErrorV3::InvalidByteLimit {
            limit,
            requested: value,
            hard_maximum: MAX_KERNEL_IR_PLIRON_OPTIMIZATION_MODULE_BYTES_V3,
        });
    }
    Ok(())
}

fn optimize_kernel_ir_module_with_policy_at_epoch_v3(
    input: &Module,
    initial_epoch: u64,
    limits: KernelIrPlironOptimizationLimitsV2,
    policy: KernelIrPlironOptimizationPolicyV2,
) -> Result<OptimizedKernelIrModuleV3, KernelIrPlironOptimizationErrorV3> {
    validate_byte_limit_v3(
        KernelIrPlironOptimizationByteLimitV2::Input,
        limits.max_input_canonical_bytes(),
    )?;
    validate_byte_limit_v3(
        KernelIrPlironOptimizationByteLimitV2::Output,
        limits.max_output_canonical_bytes(),
    )?;

    let input_bytes =
        encode_module_v11(input).map_err(KernelIrPlironOptimizationErrorV3::InputEncoding)?;
    if input_bytes.len() > limits.max_input_canonical_bytes() {
        return Err(KernelIrPlironOptimizationErrorV3::InputByteLimitExceeded {
            required: input_bytes.len(),
            limit: limits.max_input_canonical_bytes(),
        });
    }
    let (canonical, decoded_input) =
        VerifiedCanonicalKernelIrV11::from_canonical_bytes_with_module(input_bytes)
            .map_err(KernelIrPlironOptimizationErrorV3::InputCanonicalization)?;
    if &decoded_input != input {
        return Err(KernelIrPlironOptimizationErrorV3::InputCanonicalization(
            VerifiedCanonicalKernelIrErrorV11::RoundTripMismatch,
        ));
    }

    let registration = dialect_gpu::dialect_registration()
        .map_err(KernelIrPlironOptimizationErrorV3::DialectRegistration)?;
    let mut session = PlironSession::new(limits.shell(), [registration])
        .map_err(KernelIrPlironOptimizationErrorV3::Session)?;
    let graph = session
        .import_canonical_kir_v11_o0(&canonical)
        .map_err(KernelIrPlironOptimizationErrorV3::Import)?;
    let passes = match policy {
        KernelIrPlironOptimizationPolicyV2::Configurable => {
            PlironOptimizationPlanV1::standard().passes().to_vec()
        }
        KernelIrPlironOptimizationPolicyV2::ProductionV1
        | KernelIrPlironOptimizationPolicyV2::ProductionV2 => {
            KERNEL_IR_PLIRON_OPTIMIZATION_PRODUCTION_PASS_ORDER_V2
                .into_iter()
                .map(|pass| pass.pliron())
                .collect()
        }
    };
    let plan = PlironOptimizationPlanV1::new(passes, limits.pliron())
        .map_err(KernelIrPlironOptimizationErrorV3::Plan)?;
    let pliron = session
        .execute_optimization_v1(graph.root(), &plan)
        .map_err(KernelIrPlironOptimizationErrorV3::Optimize)?;
    let (output, bridge) = session
        .extract_optimized_canonical_kir_v11_v1(&graph)
        .map_err(KernelIrPlironOptimizationErrorV3::Export)?;
    if output.canonical_bytes().len() > limits.max_output_canonical_bytes() {
        return Err(KernelIrPlironOptimizationErrorV3::OutputByteLimitExceeded {
            required: output.canonical_bytes().len(),
            limit: limits.max_output_canonical_bytes(),
        });
    }
    output
        .revalidate()
        .map_err(KernelIrPlironOptimizationErrorV3::OutputRevalidation)?;
    let module = decode_module_v11(output.canonical_bytes())
        .map_err(KernelIrPlironOptimizationErrorV3::OutputDecode)?;
    verify_module(&module).map_err(KernelIrPlironOptimizationErrorV3::OutputVerification)?;

    let (passes, final_epoch) = epoch_reports(pliron.passes(), initial_epoch)
        .map_err(|_| KernelIrPlironOptimizationErrorV3::EpochOverflow)?;
    let report = KernelIrPlironOptimizationReportV2::from_parts(
        policy,
        limits,
        initial_epoch,
        final_epoch,
        bridge,
        pliron,
        passes,
    );
    Ok(OptimizedKernelIrModuleV3 {
        module,
        canonical: output,
        report,
    })
}
