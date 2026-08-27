use std::error::Error;
use std::fmt;
use std::path::Path;

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerModuleHandoffErrorV3, CompilerModuleHandoffReceiptV3,
    ConsumedCompilerModuleHandoffV3, ProducerIdentity,
    acquire_compiler_module_handoff_currentness_lease_v3,
    consume_compiler_module_handoff_with_currentness_v3,
    recover_compiler_module_handoff_receipt_v3,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::RustcInvocationCapabilityV1;
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use fe2o3_hsaco_finalize::ProtectedFirstBuildWorkerV3Error;
use fe2o3_rustc_invocation::RustcInvocationDescriptorV3;

use crate::inert_rustc_invocation_capture::{
    InertPreparedRustcInvocationCapture, InertRustcInvocationCaptureV3,
};

/// Move-only parent custody of the exact protected invocation prepared for one
/// production rustc child.
///
/// Qualification V2 captures are observations only and are not retained as
/// production custody. This value grants no compiler, link, load, or launch
/// authority.
pub(crate) struct ParentRustcInvocationCustody {
    invocation: Box<InertRustcInvocationCaptureV3>,
    capability: RustcInvocationCapabilityV1,
}

impl ParentRustcInvocationCustody {
    pub(crate) fn retain(
        capture: Option<InertPreparedRustcInvocationCapture>,
        capability: Option<RustcInvocationCapabilityV1>,
    ) -> Result<Option<Self>, ParentRustcInvocationCustodyError> {
        match (capture, capability) {
            (Some(InertPreparedRustcInvocationCapture::V3(invocation)), Some(capability)) => {
                let custody = Self {
                    invocation,
                    capability,
                };
                custody.revalidate()?;
                Ok(Some(custody))
            }
            (Some(InertPreparedRustcInvocationCapture::V2(_)), None) => Ok(None),
            (None, None) => Ok(None),
            (Some(InertPreparedRustcInvocationCapture::V3(_)), None) => {
                Err(ParentRustcInvocationCustodyError::MissingCapability)
            }
            (Some(InertPreparedRustcInvocationCapture::V2(_)), Some(_)) => {
                Err(ParentRustcInvocationCustodyError::CapabilityForV2)
            }
            (None, Some(_)) => Err(ParentRustcInvocationCustodyError::MissingCapture),
        }
    }

    pub(crate) fn revalidate(&self) -> Result<(), ParentRustcInvocationCustodyError> {
        self.capability
            .revalidate()
            .map_err(ParentRustcInvocationCustodyError::Capability)?;
        if self.invocation.descriptor() != self.capability.descriptor() {
            return Err(ParentRustcInvocationCustodyError::DescriptorMismatch);
        }
        Ok(())
    }

    const fn descriptor(&self) -> &RustcInvocationDescriptorV3 {
        self.invocation.descriptor()
    }

    /// Runs one operation while the exact selected parent custody remains live.
    pub(crate) fn retain_through<T>(self, operation: impl FnOnce(&Self) -> T) -> T {
        operation(&self)
    }

    pub(crate) const fn grants_compiler_authority(&self) -> bool {
        false
    }
}

#[derive(Debug)]
pub(crate) enum ParentRustcInvocationCustodyError {
    MissingCapture,
    MissingCapability,
    CapabilityForV2,
    DescriptorMismatch,
    Capability(String),
}

impl fmt::Display for ParentRustcInvocationCustodyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCapture => formatter.write_str(
                "protected rustc invocation capability has no exact parent invocation capture",
            ),
            Self::MissingCapability => formatter.write_str(
                "protected parent invocation capture has no retained sealed capability",
            ),
            Self::CapabilityForV2 => formatter
                .write_str("unprotected V2 invocation capture unexpectedly has a V3 capability"),
            Self::DescriptorMismatch => formatter.write_str(
                "parent invocation capture and retained sealed capability describe different rustc invocations",
            ),
            Self::Capability(error) => write!(formatter, "retained rustc invocation capability is invalid: {error}"),
        }
    }
}

impl Error for ParentRustcInvocationCustodyError {}

/// Move-only result of parent-authorized current V3 consumption.
///
/// The exact recovered receipt remains paired with the consumed transaction so
/// downstream worker execution never reconstructs or drops its transaction
/// identity. This remains inert and grants no compiler or runtime authority.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct ParentConsumedProductionHandoff {
    receipt: CompilerModuleHandoffReceiptV3,
    consumed: ConsumedCompilerModuleHandoffV3,
    compiler_closure: CompilerClosureV2,
}

#[cfg_attr(not(test), allow(dead_code))]
impl ParentConsumedProductionHandoff {
    pub(crate) fn into_parts(
        self,
    ) -> (
        CompilerModuleHandoffReceiptV3,
        ConsumedCompilerModuleHandoffV3,
        CompilerClosureV2,
    ) {
        (self.receipt, self.consumed, self.compiler_closure)
    }
}

/// Production-only intake for the current protected compiler-module wire.
///
/// It derives the expected terminal identity from the exact durable V3 receipt
/// under the cooperative lock and authenticates no compiler authorship.
pub(crate) struct ProductionCompilerModuleHandoffIntake;

impl ProductionCompilerModuleHandoffIntake {
    pub(crate) const fn new() -> Self {
        Self
    }

    #[cfg_attr(not(test), allow(dead_code))]
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn consume_after_preflight<T>(
        &self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        parent_custody: &ParentRustcInvocationCustody,
        preflight: impl FnOnce(
            &InertSemanticCompilerModuleHandoffV3,
            CompilerModuleHandoffReceiptV3,
            CompilerClosureV2,
        ) -> Result<T, ProtectedFirstBuildWorkerV3Error>,
    ) -> Result<(ParentConsumedProductionHandoff, T), ProductionCompilerModuleHandoffIntakeError>
    {
        parent_custody
            .revalidate()
            .map_err(ProductionCompilerModuleHandoffIntakeError::ParentCustody)?;
        let receipt = recover_compiler_module_handoff_receipt_v3(output_dir, producer, attempt)
            .map_err(ProductionCompilerModuleHandoffIntakeError::Transport)?;
        if receipt.attempt() != attempt || receipt.grants_compiler_authority() {
            return Err(ProductionCompilerModuleHandoffIntakeError::TransportBindingMismatch);
        }
        let lease =
            acquire_compiler_module_handoff_currentness_lease_v3(output_dir, producer, receipt)
                .map_err(ProductionCompilerModuleHandoffIntakeError::Transport)?;
        if lease.receipt() != receipt {
            return Err(ProductionCompilerModuleHandoffIntakeError::TransportBindingMismatch);
        }
        parent_custody
            .revalidate()
            .map_err(ProductionCompilerModuleHandoffIntakeError::ParentCustody)?;
        let token = lease
            .acquire_current_token()
            .map_err(ProductionCompilerModuleHandoffIntakeError::Transport)?;
        if token.handoff().capsule().invocation() != parent_custody.descriptor() {
            return Err(ProductionCompilerModuleHandoffIntakeError::InvocationMismatch);
        }
        let compiler_closure = *parent_custody.descriptor().compiler_closure();
        let prepared = preflight(token.handoff(), receipt, compiler_closure)
            .map_err(ProductionCompilerModuleHandoffIntakeError::WorkerPreflight)?;
        parent_custody
            .revalidate()
            .map_err(ProductionCompilerModuleHandoffIntakeError::ParentCustody)?;
        let consumed = consume_compiler_module_handoff_with_currentness_v3(&lease, token)
            .map_err(ProductionCompilerModuleHandoffIntakeError::Transport)?;
        if consumed.attempt() != receipt.attempt()
            || consumed.slot() != receipt.slot()
            || consumed.transaction_identity() != receipt.transaction_identity()
            || consumed.handoff_identity() != receipt.handoff_identity()
        {
            return Err(ProductionCompilerModuleHandoffIntakeError::TransportBindingMismatch);
        }
        debug_assert!(!consumed.grants_compiler_authority());
        let consumed = ParentConsumedProductionHandoff {
            receipt,
            consumed,
            compiler_closure,
        };
        Ok((consumed, prepared))
    }
}

#[derive(Debug)]
pub(crate) enum ProductionCompilerModuleHandoffIntakeError {
    ParentCustody(ParentRustcInvocationCustodyError),
    Transport(CompilerModuleHandoffErrorV3),
    WorkerPreflight(ProtectedFirstBuildWorkerV3Error),
    TransportBindingMismatch,
    InvocationMismatch,
}

impl fmt::Display for ProductionCompilerModuleHandoffIntakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParentCustody(error) => error.fmt(formatter),
            Self::Transport(error) => error.fmt(formatter),
            Self::WorkerPreflight(error) => write!(formatter, "protected V3 worker preflight failed before handoff consumption: {error}"),
            Self::TransportBindingMismatch => formatter.write_str(
                "consumed V3 compiler-module handoff changed its exact transaction binding",
            ),
            Self::InvocationMismatch => formatter.write_str(
                "consumed V3 compiler-module handoff does not retain the exact parent-prepared rustc invocation",
            ),
        }
    }
}

impl Error for ProductionCompilerModuleHandoffIntakeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ParentCustody(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::WorkerPreflight(error) => Some(error),
            Self::TransportBindingMismatch | Self::InvocationMismatch => None,
        }
    }
}
