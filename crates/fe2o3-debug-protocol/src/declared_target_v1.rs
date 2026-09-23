//! Additive, explicitly requested bundle-declared GPU target metadata.
//! This is a CPU owner's admitted content declaration, never a detected device,
//! a native instruction transaction, or a compiler/hardware authentication claim.
use crate::runtime_observations_v1::validate_observed_session;
use crate::{
    DebugErrorV1, OpaqueIdentityV1, ProtocolLimitsV1, ProtocolValidationErrorV1,
    RuntimeDecimalU64V1, RuntimeObservationBindingV1, SessionViewV1,
};
use serde::{Deserialize, Serialize};

pub const DECLARED_TARGET_REQUEST_SCHEMA_V1: &str = "fe2o3-debug-target-request-v1";
pub const DECLARED_TARGET_RESPONSE_SCHEMA_V1: &str = "fe2o3-debug-target-response-v1";
/// Whole NDJSON line, including the terminal LF. Independent of old family caps.
pub const MAX_DECLARED_TARGET_LINE_BYTES_V1: usize = 4096;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DeclaredTargetRequestSchemaV1 {
    #[serde(rename = "fe2o3-debug-target-request-v1")]
    V1,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DeclaredTargetResponseSchemaV1 {
    #[serde(rename = "fe2o3-debug-target-response-v1")]
    V1,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredTargetOperationV1 {
    InspectDeclaredTarget,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredTargetRequestV1 {
    pub schema: DeclaredTargetRequestSchemaV1,
    pub operation: DeclaredTargetOperationV1,
    pub request_id: u64,
    pub expected_revision: u64,
    /// Obtained from an accepted runtime query on this exact backend transport.
    /// No owner discovery, target override, or partial cursor is accepted here.
    pub expected_binding: RuntimeObservationBindingV1,
}
impl DeclaredTargetRequestV1 {
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        if self.request_id == 0 {
            return Err(ProtocolValidationErrorV1::ZeroRequestId);
        }
        self.expected_binding.validate()?;
        if self.expected_revision != self.expected_binding.cursor.state_revision {
            return Err(ProtocolValidationErrorV1::RevisionMismatch);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum DeclaredGpuTargetV1 {
    #[serde(rename = "gfx942:xnack-")]
    Gfx942XnackOff,
    #[serde(rename = "gfx950:xnack-")]
    Gfx950XnackOff,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredTargetProvenanceV1 {
    VerifiedSimulationBundle,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredTargetUnavailableV1 {
    RawInputHasNoDeclaredGpuTarget,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredTargetModuleV1 {
    pub wire_version: u16,
    pub sha256: OpaqueIdentityV1,
    pub canonical_bytes: RuntimeDecimalU64V1,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "availability", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeclaredTargetObservationV1 {
    Declared {
        target: DeclaredGpuTargetV1,
        provenance: DeclaredTargetProvenanceV1,
        envelope_version: u16,
        envelope_identity: OpaqueIdentityV1,
        subject_identity: OpaqueIdentityV1,
        admitted_module: DeclaredTargetModuleV1,
    },
    Unavailable {
        reason: DeclaredTargetUnavailableV1,
    },
}
impl DeclaredTargetObservationV1 {
    pub fn validate(self) -> Result<(), ProtocolValidationErrorV1> {
        if let Self::Declared {
            envelope_version,
            admitted_module,
            ..
        } = self
        {
            let expected_version = match envelope_version {
                1..=4 => 7,
                5 => 10,
                6 => 11,
                _ => {
                    return Err(ProtocolValidationErrorV1::InvalidRange(
                        "target envelope version",
                    ));
                }
            };
            if admitted_module.wire_version != expected_version {
                return Err(ProtocolValidationErrorV1::IdentityMismatch(
                    "target admitted KIR version",
                ));
            }
            if admitted_module.canonical_bytes.get() == 0 {
                return Err(ProtocolValidationErrorV1::InvalidRange(
                    "target admitted KIR length",
                ));
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum DeclaredTargetResponseV1 {
    Ok {
        schema: DeclaredTargetResponseSchemaV1,
        operation: DeclaredTargetOperationV1,
        request_id: u64,
        session: SessionViewV1,
        binding: RuntimeObservationBindingV1,
        /// Logical CPU debugger grouping only; not a hardware wave observation.
        logical_wave_width: u16,
        target: DeclaredTargetObservationV1,
    },
    Error {
        schema: DeclaredTargetResponseSchemaV1,
        operation: DeclaredTargetOperationV1,
        request_id: u64,
        session: SessionViewV1,
        error: DebugErrorV1,
    },
}
impl DeclaredTargetResponseV1 {
    pub const fn request_id(&self) -> u64 {
        match self {
            Self::Ok { request_id, .. } | Self::Error { request_id, .. } => *request_id,
        }
    }
    pub const fn session(&self) -> SessionViewV1 {
        match self {
            Self::Ok { session, .. } | Self::Error { session, .. } => *session,
        }
    }
    pub fn validate(&self, limits: ProtocolLimitsV1) -> Result<(), ProtocolValidationErrorV1> {
        limits.validate()?;
        if self.request_id() == 0 {
            return Err(ProtocolValidationErrorV1::ZeroRequestId);
        }
        validate_observed_session(self.session())?;
        match self {
            Self::Ok {
                binding,
                logical_wave_width,
                target,
                ..
            } => {
                binding.validate()?;
                if binding.cursor != self.session().cursor {
                    return Err(ProtocolValidationErrorV1::IdentityMismatch(
                        "target session cursor",
                    ));
                }
                if !matches!(logical_wave_width, 32 | 64) {
                    return Err(ProtocolValidationErrorV1::InvalidRange(
                        "logical wave width",
                    ));
                }
                target.validate()?;
            }
            Self::Error { error, .. } => error.validate()?,
        }
        Ok(())
    }
    pub fn validate_for_request(
        &self,
        request: &DeclaredTargetRequestV1,
        limits: ProtocolLimitsV1,
    ) -> Result<(), ProtocolValidationErrorV1> {
        request.validate(limits)?;
        self.validate(limits)?;
        if self.request_id() != request.request_id {
            return Err(ProtocolValidationErrorV1::OperationResultMismatch);
        }
        if let Self::Ok { binding, .. } = self {
            request.expected_binding.matches(*binding)?;
            if self.session().revision != request.expected_revision {
                return Err(ProtocolValidationErrorV1::RevisionMismatch);
            }
        }
        Ok(())
    }
}
