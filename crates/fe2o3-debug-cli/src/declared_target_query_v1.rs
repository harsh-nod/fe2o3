//! Exact owner/current-cursor target declaration, never a target override.
use super::*;
use fe2o3_kir_sim_cli::{AdmittedBundleTargetV1, BundleDeclaredGpuTargetV1};
use std::io::Write;

fn projection(retained: Option<AdmittedBundleTargetV1>) -> Result<DeclaredTargetObservationV1, ()> {
    let Some(retained) = retained else {
        return Ok(DeclaredTargetObservationV1::Unavailable {
            reason: DeclaredTargetUnavailableV1::RawInputHasNoDeclaredGpuTarget,
        });
    };
    let module = retained.module_identity();
    Ok(DeclaredTargetObservationV1::Declared {
        target: match retained.target() {
            BundleDeclaredGpuTargetV1::Gfx942XnackOff => DeclaredGpuTargetV1::Gfx942XnackOff,
            BundleDeclaredGpuTargetV1::Gfx950XnackOff => DeclaredGpuTargetV1::Gfx950XnackOff,
        },
        provenance: DeclaredTargetProvenanceV1::VerifiedSimulationBundle,
        envelope_version: retained.envelope_version(),
        envelope_identity: OpaqueIdentityV1::new(retained.envelope_identity()).map_err(|_| ())?,
        subject_identity: OpaqueIdentityV1::new(retained.subject_identity()).map_err(|_| ())?,
        admitted_module: DeclaredTargetModuleV1 {
            wire_version: module.wire_version(),
            sha256: OpaqueIdentityV1::new(*module.digest()).map_err(|_| ())?,
            canonical_bytes: RuntimeDecimalU64V1::new(module.canonical_length()),
        },
    })
}
impl SimulatorBackendV1 {
    pub(crate) fn handle_declared_target_v1(
        &mut self,
        request: DeclaredTargetRequestV1,
    ) -> DeclaredTargetResponseV1 {
        if request.validate(self.protocol_limits).is_err() {
            return self.declared_target_error(
                &request,
                DebugErrorCodeV1::InvalidRequest,
                "declared target request is invalid",
            );
        }
        if let Err((code, message)) = self.observed_preflight(request.expected_revision) {
            return self.declared_target_error(&request, code, message);
        }
        let Some(binding) = self.observed_binding() else {
            return self.declared_target_error(
                &request,
                DebugErrorCodeV1::InvalidState,
                "runtime observations were not enabled for this backend",
            );
        };
        if binding != request.expected_binding {
            return self.declared_target_error(
                &request,
                DebugErrorCodeV1::InvalidCursor,
                "target binding does not match the exact current owner and cursor",
            );
        }
        // The backend owns its module immutably, but compare again before exposing
        // a retained declaration. No fallback to a mutable hash or scalar profile.
        if self
            .declared_target_v1
            .is_some_and(|retained| retained.module_identity() != *self.module.identity())
        {
            return self.declared_target_error(
                &request,
                DebugErrorCodeV1::BackendFailure,
                "retained target module identity changed",
            );
        }
        let target = match projection(self.declared_target_v1) {
            Ok(target) => target,
            Err(()) => {
                return self.declared_target_error(
                    &request,
                    DebugErrorCodeV1::BackendFailure,
                    "retained target identity is invalid",
                );
            }
        };
        let response = DeclaredTargetResponseV1::Ok {
            schema: DeclaredTargetResponseSchemaV1::V1,
            operation: DeclaredTargetOperationV1::InspectDeclaredTarget,
            request_id: request.request_id,
            session: self.session_view(),
            binding,
            logical_wave_width: self.wave_width.lanes(),
            target,
        };
        if response
            .validate_for_request(&request, self.protocol_limits)
            .is_err()
        {
            return self.declared_target_error(
                &request,
                DebugErrorCodeV1::BackendFailure,
                "target projection did not satisfy its closed wire contract",
            );
        }
        // Fixed-size read-only projection consumes one existing command allowance,
        // but changes no revision, cursor, filters, replay state, or page token.
        self.command_count += 1;
        response
    }

    fn declared_target_error(
        &self,
        request: &DeclaredTargetRequestV1,
        code: DebugErrorCodeV1,
        message: &'static str,
    ) -> DeclaredTargetResponseV1 {
        DeclaredTargetResponseV1::Error {
            schema: DeclaredTargetResponseSchemaV1::V1,
            operation: DeclaredTargetOperationV1::InspectDeclaredTarget,
            request_id: request.request_id,
            session: self.session_view(),
            error: DebugErrorV1 {
                stage: DebugErrorStageV1::Session,
                code,
                message: bounded_message(message),
                state_changed: false,
            },
        }
    }
}
pub(crate) fn write_declared_target_v1<W: Write>(
    writer: &mut W,
    response: &DeclaredTargetResponseV1,
    limits: ProtocolLimitsV1,
) -> Result<(), String> {
    let bytes = encode_declared_target_response_line_v1(response, limits)
        .map_err(|error| format!("declared target response refused: {}", error.code()))?;
    writer
        .write_all(&bytes)
        .map_err(|_| "failed to write declared target response".to_owned())?;
    writer
        .flush()
        .map_err(|_| "failed to flush declared target response".to_owned())
}
