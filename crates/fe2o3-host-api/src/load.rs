//! Bounded runtime-neutral load request and result contracts.

use alloc::vec::Vec;

use crate::canonical::EncoderV1;
use crate::common::{
    ContractFieldV1, DiagnosticSeverityV1, HostRequestIdentityV1, HostResultIdentityV1,
    HostResultReferenceV1, OperationResultClassV1, check_preimage_bound, encode_diagnostics,
    validate_diagnostics,
};
use crate::{
    AdmissionAssessmentIdV1, AdmitOutcomeV1, AdmitResultIdV1, AdmitResultV1, HostContractErrorV1,
    HostDiagnosticV1, LoadRequestIdV1, LoadResultIdV1, LoadedObjectIdV1, LoaderProfileIdV1,
    OperationContextV1, PayloadIdV1, RuntimeContextIdV1,
};

/// Result disposition of one runtime-neutral load description.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoadOutcomeV1 {
    /// A load implementation reported one described loaded object.
    Loaded {
        /// Commitment to the loaded object description, not a runtime handle.
        loaded_object_identity: LoadedObjectIdV1,
        /// Nonzero load generation used to prevent stale-object confusion.
        load_generation: u64,
    },
    /// The load request was rejected.
    Rejected,
    /// The described load operation failed.
    Failed,
}

/// Complete runtime-neutral V1 load request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadRequestV1 {
    identity: LoadRequestIdV1,
    context: OperationContextV1,
    admit_result_identity: AdmitResultIdV1,
    assessment_identity: AdmissionAssessmentIdV1,
    artifact_identity: PayloadIdV1,
    loader_profile_identity: LoaderProfileIdV1,
    runtime_context_identity: RuntimeContextIdV1,
}

impl LoadRequestV1 {
    /// Creates a request bound to an accepted inert admission assessment.
    pub fn new(
        identity: LoadRequestIdV1,
        context: OperationContextV1,
        admit_result: &AdmitResultV1,
        artifact_identity: PayloadIdV1,
        loader_profile_identity: LoaderProfileIdV1,
        runtime_context_identity: RuntimeContextIdV1,
    ) -> Result<Self, HostContractErrorV1> {
        if admit_result.candidate_identity() != artifact_identity {
            return Err(HostContractErrorV1::Mismatch {
                field: ContractFieldV1::UpstreamObject,
            });
        }
        let AdmitOutcomeV1::Accepted {
            assessment_identity,
        } = admit_result.outcome()
        else {
            return Err(HostContractErrorV1::InvalidOutcome);
        };
        Ok(Self {
            identity,
            context,
            admit_result_identity: admit_result.identity(),
            assessment_identity,
            artifact_identity,
            loader_profile_identity,
            runtime_context_identity,
        })
    }

    /// Returns the caller-supplied request commitment.
    pub const fn identity(&self) -> LoadRequestIdV1 {
        self.identity
    }

    /// Returns the parallel-operation context.
    pub const fn context(&self) -> &OperationContextV1 {
        &self.context
    }

    /// Returns the exact upstream admission result commitment.
    pub const fn admit_result_identity(&self) -> AdmitResultIdV1 {
        self.admit_result_identity
    }

    /// Returns the accepted assessment commitment.
    pub const fn assessment_identity(&self) -> AdmissionAssessmentIdV1 {
        self.assessment_identity
    }

    /// Returns the artifact commitment to describe loading.
    pub const fn artifact_identity(&self) -> PayloadIdV1 {
        self.artifact_identity
    }

    /// Returns the loader profile commitment.
    pub const fn loader_profile_identity(&self) -> LoaderProfileIdV1 {
        self.loader_profile_identity
    }

    /// Returns the opaque runtime-context commitment.
    pub const fn runtime_context_identity(&self) -> RuntimeContextIdV1 {
        self.runtime_context_identity
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(LoadRequestIdV1::DOMAIN_V1);
        self.context.encode(&mut encoder);
        encoder.digest(self.admit_result_identity.digest());
        encoder.digest(self.assessment_identity.digest());
        encoder.digest(self.artifact_identity.digest());
        encoder.digest(self.loader_profile_identity.digest());
        encoder.digest(self.runtime_context_identity.digest());
        check_preimage_bound(encoder.finish())
    }
}

/// Complete inert result of one V1 load request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadResultV1 {
    identity: LoadResultIdV1,
    request_identity: LoadRequestIdV1,
    artifact_identity: PayloadIdV1,
    runtime_context_identity: RuntimeContextIdV1,
    outcome: LoadOutcomeV1,
    diagnostics: Vec<HostDiagnosticV1>,
}

impl LoadResultV1 {
    /// Creates a load result bound to the exact request.
    pub fn new(
        identity: LoadResultIdV1,
        request: &LoadRequestV1,
        outcome: LoadOutcomeV1,
        diagnostics: Vec<HostDiagnosticV1>,
    ) -> Result<Self, HostContractErrorV1> {
        let require_error = matches!(outcome, LoadOutcomeV1::Rejected | LoadOutcomeV1::Failed);
        validate_diagnostics(&diagnostics, require_error)?;
        match outcome {
            LoadOutcomeV1::Loaded {
                load_generation, ..
            } => {
                if load_generation == 0 {
                    return Err(HostContractErrorV1::Empty {
                        field: ContractFieldV1::LoadGeneration,
                    });
                }
                if diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity() == DiagnosticSeverityV1::Error)
                {
                    return Err(HostContractErrorV1::InvalidOutcome);
                }
            }
            LoadOutcomeV1::Rejected | LoadOutcomeV1::Failed => {}
        }
        Ok(Self {
            identity,
            request_identity: request.identity,
            artifact_identity: request.artifact_identity,
            runtime_context_identity: request.runtime_context_identity,
            outcome,
            diagnostics,
        })
    }

    /// Returns the caller-supplied result commitment.
    pub const fn identity(&self) -> LoadResultIdV1 {
        self.identity
    }

    /// Returns the exact load request commitment.
    pub const fn request_identity(&self) -> LoadRequestIdV1 {
        self.request_identity
    }

    /// Returns the exact artifact commitment named by the request.
    pub const fn artifact_identity(&self) -> PayloadIdV1 {
        self.artifact_identity
    }

    /// Returns the exact runtime-context commitment named by the request.
    pub const fn runtime_context_identity(&self) -> RuntimeContextIdV1 {
        self.runtime_context_identity
    }

    /// Returns the load disposition.
    pub const fn outcome(&self) -> LoadOutcomeV1 {
        self.outcome
    }

    /// Returns bounded diagnostics in producer order.
    pub fn diagnostics(&self) -> &[HostDiagnosticV1] {
        &self.diagnostics
    }

    /// Returns a flow-erased binding for terminal-state validation.
    pub const fn result_reference(&self) -> HostResultReferenceV1 {
        let class = match self.outcome {
            LoadOutcomeV1::Loaded { .. } => OperationResultClassV1::Succeeded,
            LoadOutcomeV1::Rejected => OperationResultClassV1::Rejected,
            LoadOutcomeV1::Failed => OperationResultClassV1::Failed,
        };
        HostResultReferenceV1::new(
            HostResultIdentityV1::Load(self.identity),
            HostRequestIdentityV1::Load(self.request_identity),
            class,
        )
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(LoadResultIdV1::DOMAIN_V1);
        encoder.digest(self.request_identity.digest());
        encoder.digest(self.artifact_identity.digest());
        encoder.digest(self.runtime_context_identity.digest());
        match self.outcome {
            LoadOutcomeV1::Loaded {
                loaded_object_identity,
                load_generation,
            } => {
                encoder.u8(1);
                encoder.digest(loaded_object_identity.digest());
                encoder.u64(load_generation);
            }
            LoadOutcomeV1::Rejected => encoder.u8(2),
            LoadOutcomeV1::Failed => encoder.u8(3),
        }
        encode_diagnostics(&self.diagnostics, &mut encoder);
        check_preimage_bound(encoder.finish())
    }
}
