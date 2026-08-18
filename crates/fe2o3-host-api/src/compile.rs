//! Bounded compile request and result contracts.

use alloc::vec::Vec;

use crate::canonical::EncoderV1;
use crate::common::{
    ContractFieldV1, DiagnosticSeverityV1, HostRequestIdentityV1, HostResultIdentityV1,
    HostResultReferenceV1, OperationResultClassV1, check_preimage_bound, encode_diagnostics,
    validate_diagnostics,
};
use crate::{
    CompileConfigurationIdV1, CompileRequestIdV1, CompileResultIdV1, CompilerProfileIdV1,
    HostContractErrorV1, HostDiagnosticV1, MAX_PAYLOAD_BYTES_V1, OperationContextV1,
    PayloadDescriptorV1, TargetProfileIdV1,
};

/// Result disposition of one compile request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompileOutcomeV1 {
    /// A bounded opaque candidate was described.
    Candidate(PayloadDescriptorV1),
    /// The compile request was rejected before producing a candidate.
    Rejected,
    /// The described compiler operation failed without producing a candidate.
    Failed,
}

/// Complete target-neutral V1 compile request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileRequestV1 {
    identity: CompileRequestIdV1,
    context: OperationContextV1,
    input: PayloadDescriptorV1,
    compiler_profile_identity: CompilerProfileIdV1,
    target_profile_identity: TargetProfileIdV1,
    configuration_identity: CompileConfigurationIdV1,
    maximum_output_bytes: u64,
}

impl CompileRequestV1 {
    /// Creates a bounded compile description.
    pub fn new(
        identity: CompileRequestIdV1,
        context: OperationContextV1,
        input: PayloadDescriptorV1,
        compiler_profile_identity: CompilerProfileIdV1,
        target_profile_identity: TargetProfileIdV1,
        configuration_identity: CompileConfigurationIdV1,
        maximum_output_bytes: u64,
    ) -> Result<Self, HostContractErrorV1> {
        if maximum_output_bytes == 0 {
            return Err(HostContractErrorV1::Empty {
                field: ContractFieldV1::CompileOutputBytes,
            });
        }
        if maximum_output_bytes > MAX_PAYLOAD_BYTES_V1 {
            return Err(HostContractErrorV1::LimitExceeded {
                field: ContractFieldV1::CompileOutputBytes,
                actual: maximum_output_bytes,
                maximum: MAX_PAYLOAD_BYTES_V1,
            });
        }
        Ok(Self {
            identity,
            context,
            input,
            compiler_profile_identity,
            target_profile_identity,
            configuration_identity,
            maximum_output_bytes,
        })
    }

    /// Returns the caller-supplied request commitment.
    pub const fn identity(&self) -> CompileRequestIdV1 {
        self.identity
    }

    /// Returns the parallel-operation context.
    pub const fn context(&self) -> &OperationContextV1 {
        &self.context
    }

    /// Returns the opaque compile input description.
    pub const fn input(&self) -> PayloadDescriptorV1 {
        self.input
    }

    /// Returns the compiler/frontend profile commitment.
    pub const fn compiler_profile_identity(&self) -> CompilerProfileIdV1 {
        self.compiler_profile_identity
    }

    /// Returns the target-neutral target profile commitment.
    pub const fn target_profile_identity(&self) -> TargetProfileIdV1 {
        self.target_profile_identity
    }

    /// Returns the complete compile configuration commitment.
    pub const fn configuration_identity(&self) -> CompileConfigurationIdV1 {
        self.configuration_identity
    }

    /// Returns the caller-selected output byte ceiling.
    pub const fn maximum_output_bytes(&self) -> u64 {
        self.maximum_output_bytes
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(CompileRequestIdV1::DOMAIN_V1);
        self.context.encode(&mut encoder);
        self.input.encode(&mut encoder);
        encoder.digest(self.compiler_profile_identity.digest());
        encoder.digest(self.target_profile_identity.digest());
        encoder.digest(self.configuration_identity.digest());
        encoder.u64(self.maximum_output_bytes);
        check_preimage_bound(encoder.finish())
    }
}

/// Complete inert result of one V1 compile request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileResultV1 {
    identity: CompileResultIdV1,
    request_identity: CompileRequestIdV1,
    outcome: CompileOutcomeV1,
    diagnostics: Vec<HostDiagnosticV1>,
}

impl CompileResultV1 {
    /// Creates a result bound to the exact request and its output limit.
    pub fn new(
        identity: CompileResultIdV1,
        request: &CompileRequestV1,
        outcome: CompileOutcomeV1,
        diagnostics: Vec<HostDiagnosticV1>,
    ) -> Result<Self, HostContractErrorV1> {
        let require_error = matches!(
            outcome,
            CompileOutcomeV1::Rejected | CompileOutcomeV1::Failed
        );
        validate_diagnostics(&diagnostics, require_error)?;
        match outcome {
            CompileOutcomeV1::Candidate(candidate) => {
                if candidate.byte_len() > request.maximum_output_bytes {
                    return Err(HostContractErrorV1::LimitExceeded {
                        field: ContractFieldV1::CompileOutputBytes,
                        actual: candidate.byte_len(),
                        maximum: request.maximum_output_bytes,
                    });
                }
                if diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity() == DiagnosticSeverityV1::Error)
                {
                    return Err(HostContractErrorV1::InvalidOutcome);
                }
            }
            CompileOutcomeV1::Rejected | CompileOutcomeV1::Failed => {}
        }
        Ok(Self {
            identity,
            request_identity: request.identity,
            outcome,
            diagnostics,
        })
    }

    /// Returns the caller-supplied result commitment.
    pub const fn identity(&self) -> CompileResultIdV1 {
        self.identity
    }

    /// Returns the exact compile request commitment.
    pub const fn request_identity(&self) -> CompileRequestIdV1 {
        self.request_identity
    }

    /// Returns the compile disposition.
    pub const fn outcome(&self) -> &CompileOutcomeV1 {
        &self.outcome
    }

    /// Returns bounded diagnostics in producer order.
    pub fn diagnostics(&self) -> &[HostDiagnosticV1] {
        &self.diagnostics
    }

    /// Returns a flow-erased binding for terminal-state validation.
    pub const fn result_reference(&self) -> HostResultReferenceV1 {
        let class = match self.outcome {
            CompileOutcomeV1::Candidate(_) => OperationResultClassV1::Succeeded,
            CompileOutcomeV1::Rejected => OperationResultClassV1::Rejected,
            CompileOutcomeV1::Failed => OperationResultClassV1::Failed,
        };
        HostResultReferenceV1::new(
            HostResultIdentityV1::Compile(self.identity),
            HostRequestIdentityV1::Compile(self.request_identity),
            class,
        )
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(CompileResultIdV1::DOMAIN_V1);
        encoder.digest(self.request_identity.digest());
        match self.outcome {
            CompileOutcomeV1::Candidate(candidate) => {
                encoder.u8(1);
                candidate.encode(&mut encoder);
            }
            CompileOutcomeV1::Rejected => encoder.u8(2),
            CompileOutcomeV1::Failed => encoder.u8(3),
        }
        encode_diagnostics(&self.diagnostics, &mut encoder);
        check_preimage_bound(encoder.finish())
    }
}
