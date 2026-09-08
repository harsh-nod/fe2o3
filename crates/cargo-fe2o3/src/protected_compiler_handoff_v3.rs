use std::error::Error;
use std::fmt;
use std::path::Path;

use fe2o3_artifact_transaction::{
    BuildAttempt, CompilerCapabilityHandoffErrorV5, CompilerExecutionReceiptTransportErrorV1,
    CompilerExecutionSubjectErrorV1, CompilerModuleHandoffErrorV3, CompilerModuleHandoffReceiptV3,
    ConsumedCompilerCapabilityHandoffV5, ConsumedCompilerModuleHandoffV3,
    InertCompilerExecutionSubjectV1, ProducerIdentity,
    acquire_compiler_module_handoff_currentness_lease_v3, consume_compiler_capability_handoff_v5,
    consume_compiler_module_handoff_with_currentness_v3, recover_compiler_capability_handoff_v5,
    recover_compiler_execution_receipt_transport_for_capability_v5,
    recover_compiler_execution_receipt_transport_with_currentness_v1,
    recover_compiler_module_handoff_receipt_v3,
};
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::RustcInvocationCapabilityV1;
use fe2o3_compiler_execution_protocol::CompilerExecutionReceiptCarriageV1;
use fe2o3_compiler_ffi::InertSemanticCompilerModuleHandoffV3;
use fe2o3_hsaco_finalize::ProtectedFirstBuildWorkerV3Error;
use fe2o3_rustc_invocation::RustcInvocationDescriptorV3;

use crate::compiler_execution_boundary::{
    CompilerExecutionBoundaryErrorV1, ParentCompilerExecutionReadinessCustodyV1,
};
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
pub(crate) struct ParentConsumedProductionHandoff {
    receipt: CompilerModuleHandoffReceiptV3,
    consumed: ConsumedCompilerModuleHandoffV3,
    compiler_closure: CompilerClosureV2,
    compiler_execution: CompilerExecutionReceiptCarriageV1,
    capability_v5: Option<ConsumedCompilerCapabilityHandoffV5>,
}

impl ParentConsumedProductionHandoff {
    pub(crate) fn into_parts(
        self,
    ) -> (
        CompilerModuleHandoffReceiptV3,
        ConsumedCompilerModuleHandoffV3,
        CompilerClosureV2,
        CompilerExecutionReceiptCarriageV1,
        Option<ConsumedCompilerCapabilityHandoffV5>,
    ) {
        (
            self.receipt,
            self.consumed,
            self.compiler_closure,
            self.compiler_execution,
            self.capability_v5,
        )
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

    pub(crate) fn consume_after_preflight<T>(
        &self,
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        parent_custody: &ParentRustcInvocationCustody,
        compiler_execution_readiness: &ParentCompilerExecutionReadinessCustodyV1,
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
        match recover_compiler_capability_handoff_v5(output_dir, producer, attempt) {
            Ok(recovered) => Self::consume_capability_v5(
                output_dir,
                producer,
                attempt,
                parent_custody,
                compiler_execution_readiness,
                recovered,
                preflight,
            ),
            Err(CompilerCapabilityHandoffErrorV5::NotPublished) => Self::consume_legacy_v3(
                output_dir,
                producer,
                attempt,
                parent_custody,
                compiler_execution_readiness,
                preflight,
            ),
            Err(error) => {
                Err(ProductionCompilerModuleHandoffIntakeError::CapabilityTransport(error))
            }
        }
    }

    fn consume_capability_v5<T>(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        parent_custody: &ParentRustcInvocationCustody,
        compiler_execution_readiness: &ParentCompilerExecutionReadinessCustodyV1,
        recovered: fe2o3_artifact_transaction::RecoveredCompilerCapabilityHandoffV5,
        preflight: impl FnOnce(
            &InertSemanticCompilerModuleHandoffV3,
            CompilerModuleHandoffReceiptV3,
            CompilerClosureV2,
        ) -> Result<T, ProtectedFirstBuildWorkerV3Error>,
    ) -> Result<(ParentConsumedProductionHandoff, T), ProductionCompilerModuleHandoffIntakeError>
    {
        let capability_receipt = recovered.receipt();
        if capability_receipt.attempt() != attempt {
            return Err(
                ProductionCompilerModuleHandoffIntakeError::CapabilityTransportBindingMismatch,
            );
        }
        if recovered.handoff().legacy_handoff().capsule().invocation()
            != parent_custody.descriptor()
        {
            return Err(ProductionCompilerModuleHandoffIntakeError::InvocationMismatch);
        }
        let subject = InertCompilerExecutionSubjectV1::from_capability_publication_v5(
            capability_receipt,
            recovered.handoff(),
        )
        .map_err(ProductionCompilerModuleHandoffIntakeError::CompilerExecutionSubject)?;
        let receipt_transport = recover_compiler_execution_receipt_transport_for_capability_v5(
            output_dir,
            producer,
            capability_receipt,
            &subject,
        )
        .map_err(ProductionCompilerModuleHandoffIntakeError::CompilerExecutionTransport)?;
        let compiler_execution = compiler_execution_readiness
            .admit_receipt_transport(&subject, receipt_transport)
            .map_err(ProductionCompilerModuleHandoffIntakeError::CompilerExecutionReadiness)?;
        let (worker_receipt, worker_handoff) = recovered
            .worker_v3_preflight_compatibility()
            .map_err(ProductionCompilerModuleHandoffIntakeError::CapabilityTransport)?;
        let compiler_closure = *parent_custody.descriptor().compiler_closure();
        let prepared = preflight(&worker_handoff, worker_receipt, compiler_closure)
            .map_err(ProductionCompilerModuleHandoffIntakeError::WorkerPreflight)?;
        parent_custody
            .revalidate()
            .map_err(ProductionCompilerModuleHandoffIntakeError::ParentCustody)?;
        compiler_execution_readiness
            .revalidate()
            .map_err(ProductionCompilerModuleHandoffIntakeError::CompilerExecutionReadiness)?;
        let expected_capability_attempt = capability_receipt.attempt();
        let expected_capability_slot = capability_receipt.slot();
        let expected_capability_transaction = capability_receipt.transaction_identity();
        let mut capability_v5 =
            consume_compiler_capability_handoff_v5(output_dir, producer, recovered.into_receipt())
                .map_err(ProductionCompilerModuleHandoffIntakeError::CapabilityTransport)?;
        if capability_v5.attempt() != expected_capability_attempt
            || capability_v5.slot() != expected_capability_slot
            || capability_v5.transaction_identity() != expected_capability_transaction
            || InertCompilerExecutionSubjectV1::from_consumed_capability_v5(&capability_v5)
                .map_err(ProductionCompilerModuleHandoffIntakeError::CompilerExecutionSubject)?
                != subject
        {
            return Err(
                ProductionCompilerModuleHandoffIntakeError::CapabilityTransportBindingMismatch,
            );
        }
        let (consumed_receipt, consumed) = capability_v5
            .take_authenticated_worker_v3_compatibility()
            .map_err(ProductionCompilerModuleHandoffIntakeError::CapabilityTransport)?;
        if consumed_receipt != worker_receipt
            || consumed.handoff().canonical_bytes() != worker_handoff.canonical_bytes()
            || compiler_execution.request().subject() != &subject
        {
            return Err(
                ProductionCompilerModuleHandoffIntakeError::CompilerExecutionBindingMismatch,
            );
        }
        Ok((
            ParentConsumedProductionHandoff {
                receipt: consumed_receipt,
                consumed,
                compiler_closure,
                compiler_execution,
                capability_v5: Some(capability_v5),
            },
            prepared,
        ))
    }

    fn consume_legacy_v3<T>(
        output_dir: &Path,
        producer: &ProducerIdentity,
        attempt: BuildAttempt,
        parent_custody: &ParentRustcInvocationCustody,
        compiler_execution_readiness: &ParentCompilerExecutionReadinessCustodyV1,
        preflight: impl FnOnce(
            &InertSemanticCompilerModuleHandoffV3,
            CompilerModuleHandoffReceiptV3,
            CompilerClosureV2,
        ) -> Result<T, ProtectedFirstBuildWorkerV3Error>,
    ) -> Result<(ParentConsumedProductionHandoff, T), ProductionCompilerModuleHandoffIntakeError>
    {
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
        if handoff_contains_native_v13(token.handoff()) {
            return Err(ProductionCompilerModuleHandoffIntakeError::MissingCapabilityV5);
        }
        if token.handoff().capsule().invocation() != parent_custody.descriptor() {
            return Err(ProductionCompilerModuleHandoffIntakeError::InvocationMismatch);
        }
        let subject =
            InertCompilerExecutionSubjectV1::from_publication(receipt, token.handoff())
                .map_err(ProductionCompilerModuleHandoffIntakeError::CompilerExecutionSubject)?;
        let receipt_transport = recover_compiler_execution_receipt_transport_with_currentness_v1(
            &lease, &token, &subject,
        )
        .map_err(ProductionCompilerModuleHandoffIntakeError::CompilerExecutionTransport)?;
        let compiler_execution = compiler_execution_readiness
            .admit_receipt_transport(&subject, receipt_transport)
            .map_err(ProductionCompilerModuleHandoffIntakeError::CompilerExecutionReadiness)?;
        let compiler_closure = *parent_custody.descriptor().compiler_closure();
        let prepared = preflight(token.handoff(), receipt, compiler_closure)
            .map_err(ProductionCompilerModuleHandoffIntakeError::WorkerPreflight)?;
        parent_custody
            .revalidate()
            .map_err(ProductionCompilerModuleHandoffIntakeError::ParentCustody)?;
        compiler_execution_readiness
            .revalidate()
            .map_err(ProductionCompilerModuleHandoffIntakeError::CompilerExecutionReadiness)?;
        let consumed = consume_compiler_module_handoff_with_currentness_v3(&lease, token)
            .map_err(ProductionCompilerModuleHandoffIntakeError::Transport)?;
        if consumed.attempt() != receipt.attempt()
            || consumed.slot() != receipt.slot()
            || consumed.transaction_identity() != receipt.transaction_identity()
            || consumed.handoff_identity() != receipt.handoff_identity()
        {
            return Err(ProductionCompilerModuleHandoffIntakeError::TransportBindingMismatch);
        }
        if InertCompilerExecutionSubjectV1::from_consumed(&consumed)
            .map_err(ProductionCompilerModuleHandoffIntakeError::CompilerExecutionSubject)?
            != subject
            || compiler_execution.request().subject() != &subject
        {
            return Err(
                ProductionCompilerModuleHandoffIntakeError::CompilerExecutionBindingMismatch,
            );
        }
        debug_assert!(!consumed.grants_compiler_authority());
        let consumed = ParentConsumedProductionHandoff {
            receipt,
            consumed,
            compiler_closure,
            compiler_execution,
            capability_v5: None,
        };
        Ok((consumed, prepared))
    }
}

pub(crate) fn handoff_contains_native_v13(handoff: &InertSemanticCompilerModuleHandoffV3) -> bool {
    let kir = handoff
        .capsule()
        .receipts()
        .kernel_ir()
        .canonical_preimage();
    const VERSION_OFFSET: usize = 8;
    kir.starts_with(&fe2o3_kernel_ir::KERNEL_IR_MAGIC_V1)
        && kir
            .get(VERSION_OFFSET..VERSION_OFFSET + 2)
            .is_some_and(|version| {
                u16::from_le_bytes([version[0], version[1]])
                    == fe2o3_kernel_ir::KERNEL_IR_VERSION_V13
            })
}

#[derive(Debug)]
pub(crate) enum ProductionCompilerModuleHandoffIntakeError {
    ParentCustody(ParentRustcInvocationCustodyError),
    CompilerExecutionReadiness(CompilerExecutionBoundaryErrorV1),
    Transport(CompilerModuleHandoffErrorV3),
    CapabilityTransport(CompilerCapabilityHandoffErrorV5),
    CompilerExecutionTransport(CompilerExecutionReceiptTransportErrorV1),
    CompilerExecutionSubject(CompilerExecutionSubjectErrorV1),
    WorkerPreflight(ProtectedFirstBuildWorkerV3Error),
    TransportBindingMismatch,
    CapabilityTransportBindingMismatch,
    MissingCapabilityV5,
    CompilerExecutionBindingMismatch,
    InvocationMismatch,
}

impl fmt::Display for ProductionCompilerModuleHandoffIntakeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParentCustody(error) => error.fmt(formatter),
            Self::CompilerExecutionReadiness(error) => error.fmt(formatter),
            Self::Transport(error) => error.fmt(formatter),
            Self::CapabilityTransport(error) => error.fmt(formatter),
            Self::CompilerExecutionTransport(error) => error.fmt(formatter),
            Self::CompilerExecutionSubject(error) => error.fmt(formatter),
            Self::WorkerPreflight(error) => write!(formatter, "protected V3 worker preflight failed before handoff consumption: {error}"),
            Self::TransportBindingMismatch => formatter.write_str(
                "consumed V3 compiler-module handoff changed its exact transaction binding",
            ),
            Self::CapabilityTransportBindingMismatch => formatter.write_str(
                "consumed V5 capability handoff changed its exact transaction binding",
            ),
            Self::MissingCapabilityV5 => formatter.write_str(
                "native canonical KIR V13 requires the exact V5 capability handoff; V3 downgrade is forbidden",
            ),
            Self::CompilerExecutionBindingMismatch => formatter.write_str(
                "consumed V3 compiler-module handoff changed its exact compiler-execution receipt binding",
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
            Self::CompilerExecutionReadiness(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::CapabilityTransport(error) => Some(error),
            Self::CompilerExecutionTransport(error) => Some(error),
            Self::CompilerExecutionSubject(error) => Some(error),
            Self::WorkerPreflight(error) => Some(error),
            Self::TransportBindingMismatch
            | Self::CapabilityTransportBindingMismatch
            | Self::MissingCapabilityV5
            | Self::CompilerExecutionBindingMismatch
            | Self::InvocationMismatch => None,
        }
    }
}
