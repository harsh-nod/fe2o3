//! Bounded artifact-admission request and assessment contracts.

use alloc::vec::Vec;

use crate::canonical::EncoderV1;
use crate::common::{
    ContractFieldV1, DiagnosticSeverityV1, HostRequestIdentityV1, HostResultIdentityV1,
    HostResultReferenceV1, OperationResultClassV1, check_preimage_bound, encode_diagnostics,
    validate_diagnostics, validate_strictly_ordered,
};
use crate::{
    AdmissionAssessmentIdV1, AdmissionPolicyIdV1, AdmitRequestIdV1, AdmitResultIdV1, ClaimIdV1,
    CompileOutcomeV1, CompileResultIdV1, CompileResultV1, HostContractErrorV1, HostDiagnosticV1,
    OperationContextV1, PayloadDescriptorV1,
};

/// Hard maximum required-claim count in one admission request.
pub const MAX_ADMISSION_CLAIMS_V1: usize = 128;

/// Result disposition of one admission assessment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmitOutcomeV1 {
    /// The inert assessment accepted the candidate under the named policy.
    Accepted {
        /// Commitment to the complete assessment, not an authority token.
        assessment_identity: AdmissionAssessmentIdV1,
    },
    /// The policy assessment rejected the candidate.
    Rejected,
    /// The described assessment operation failed.
    Failed,
}

/// Complete V1 admission-assessment request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitRequestV1 {
    identity: AdmitRequestIdV1,
    context: OperationContextV1,
    compile_result_identity: CompileResultIdV1,
    candidate: PayloadDescriptorV1,
    policy_identity: AdmissionPolicyIdV1,
    required_claims: Vec<ClaimIdV1>,
}

impl AdmitRequestV1 {
    /// Creates a bounded request bound to a candidate-producing compile result.
    pub fn new(
        identity: AdmitRequestIdV1,
        context: OperationContextV1,
        compile_result: &CompileResultV1,
        candidate: PayloadDescriptorV1,
        policy_identity: AdmissionPolicyIdV1,
        required_claims: Vec<ClaimIdV1>,
    ) -> Result<Self, HostContractErrorV1> {
        validate_strictly_ordered(
            &required_claims,
            MAX_ADMISSION_CLAIMS_V1,
            ContractFieldV1::AdmissionClaims,
        )?;
        match compile_result.outcome() {
            CompileOutcomeV1::Candidate(compiled) if *compiled == candidate => {}
            CompileOutcomeV1::Candidate(_) => {
                return Err(HostContractErrorV1::Mismatch {
                    field: ContractFieldV1::UpstreamObject,
                });
            }
            CompileOutcomeV1::Rejected | CompileOutcomeV1::Failed => {
                return Err(HostContractErrorV1::InvalidOutcome);
            }
        }
        Ok(Self {
            identity,
            context,
            compile_result_identity: compile_result.identity(),
            candidate,
            policy_identity,
            required_claims,
        })
    }

    /// Returns the caller-supplied request commitment.
    pub const fn identity(&self) -> AdmitRequestIdV1 {
        self.identity
    }

    /// Returns the parallel-operation context.
    pub const fn context(&self) -> &OperationContextV1 {
        &self.context
    }

    /// Returns the exact upstream compile result commitment.
    pub const fn compile_result_identity(&self) -> CompileResultIdV1 {
        self.compile_result_identity
    }

    /// Returns the candidate description under assessment.
    pub const fn candidate(&self) -> PayloadDescriptorV1 {
        self.candidate
    }

    /// Returns the admission-policy commitment.
    pub const fn policy_identity(&self) -> AdmissionPolicyIdV1 {
        self.policy_identity
    }

    /// Returns the canonical set of required claim commitments.
    pub fn required_claims(&self) -> &[ClaimIdV1] {
        &self.required_claims
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(AdmitRequestIdV1::DOMAIN_V1);
        self.context.encode(&mut encoder);
        encoder.digest(self.compile_result_identity.digest());
        self.candidate.encode(&mut encoder);
        encoder.digest(self.policy_identity.digest());
        encoder.usize_as_u16(self.required_claims.len());
        for claim in &self.required_claims {
            encoder.digest(claim.digest());
        }
        check_preimage_bound(encoder.finish())
    }
}

/// Complete inert result of one V1 admission assessment.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdmitResultV1 {
    identity: AdmitResultIdV1,
    request_identity: AdmitRequestIdV1,
    candidate_identity: crate::PayloadIdV1,
    outcome: AdmitOutcomeV1,
    diagnostics: Vec<HostDiagnosticV1>,
}

impl AdmitResultV1 {
    /// Creates an assessment result bound to the exact request and candidate.
    pub fn new(
        identity: AdmitResultIdV1,
        request: &AdmitRequestV1,
        outcome: AdmitOutcomeV1,
        diagnostics: Vec<HostDiagnosticV1>,
    ) -> Result<Self, HostContractErrorV1> {
        let require_error = matches!(outcome, AdmitOutcomeV1::Rejected | AdmitOutcomeV1::Failed);
        validate_diagnostics(&diagnostics, require_error)?;
        if matches!(outcome, AdmitOutcomeV1::Accepted { .. })
            && diagnostics
                .iter()
                .any(|diagnostic| diagnostic.severity() == DiagnosticSeverityV1::Error)
        {
            return Err(HostContractErrorV1::InvalidOutcome);
        }
        Ok(Self {
            identity,
            request_identity: request.identity,
            candidate_identity: request.candidate.identity(),
            outcome,
            diagnostics,
        })
    }

    /// Returns the caller-supplied result commitment.
    pub const fn identity(&self) -> AdmitResultIdV1 {
        self.identity
    }

    /// Returns the exact admission request commitment.
    pub const fn request_identity(&self) -> AdmitRequestIdV1 {
        self.request_identity
    }

    /// Returns the exact candidate commitment assessed by this result.
    pub const fn candidate_identity(&self) -> crate::PayloadIdV1 {
        self.candidate_identity
    }

    /// Returns the assessment disposition.
    pub const fn outcome(&self) -> AdmitOutcomeV1 {
        self.outcome
    }

    /// Returns bounded diagnostics in producer order.
    pub fn diagnostics(&self) -> &[HostDiagnosticV1] {
        &self.diagnostics
    }

    /// Returns a flow-erased binding for terminal-state validation.
    pub const fn result_reference(&self) -> HostResultReferenceV1 {
        let class = match self.outcome {
            AdmitOutcomeV1::Accepted { .. } => OperationResultClassV1::Succeeded,
            AdmitOutcomeV1::Rejected => OperationResultClassV1::Rejected,
            AdmitOutcomeV1::Failed => OperationResultClassV1::Failed,
        };
        HostResultReferenceV1::new(
            HostResultIdentityV1::Admit(self.identity),
            HostRequestIdentityV1::Admit(self.request_identity),
            class,
        )
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(AdmitResultIdV1::DOMAIN_V1);
        encoder.digest(self.request_identity.digest());
        encoder.digest(self.candidate_identity.digest());
        match self.outcome {
            AdmitOutcomeV1::Accepted {
                assessment_identity,
            } => {
                encoder.u8(1);
                encoder.digest(assessment_identity.digest());
            }
            AdmitOutcomeV1::Rejected => encoder.u8(2),
            AdmitOutcomeV1::Failed => encoder.u8(3),
        }
        encode_diagnostics(&self.diagnostics, &mut encoder);
        check_preimage_bound(encoder.finish())
    }
}
