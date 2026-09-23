//! Read-only observed-runtime queries over one sealed CPU capture owner.
use crate::{
    MAX_SESSION_COMMANDS_V1, SimulatorBackendV1, bounded_message, transcript_truncation_reason,
};
use fe2o3_debug_protocol::*;
use fe2o3_kir_debugger::DebugTranscriptCompletenessV1;

#[path = "declared_target_query_v1.rs"]
mod declared_target;
pub(crate) use declared_target::write_declared_target_v1;

#[path = "runtime_queries_map.rs"]
mod mapping;
#[path = "runtime_queries_resources.rs"]
mod resources;
#[path = "runtime_queries_state.rs"]
mod state;
use mapping::number;
pub(crate) use state::RuntimeQueryStateV1;

#[derive(Clone, Copy, Debug)]
enum QueryFailure {
    Unavailable(RuntimeObservationUnavailableV1),
    Refused(DebugErrorCodeV1, &'static str),
}
impl QueryFailure {
    fn invalid(message: &'static str) -> Self {
        Self::Refused(DebugErrorCodeV1::InvalidCursor, message)
    }
    fn work() -> Self {
        Self::Unavailable(RuntimeObservationUnavailableV1::WorkLimit)
    }
}
impl SimulatorBackendV1 {
    fn observed_binding(&self) -> Option<RuntimeObservationBindingV1> {
        let owner = self.session.observed()?;
        Some(RuntimeObservationBindingV1 {
            owner: self.runtime_queries.owner(owner.capture_instance()),
            cursor: self.session_view().cursor,
        })
    }
    fn observed_completeness(&self) -> CaptureCompletenessV1 {
        match self.session.transcript().completeness() {
            DebugTranscriptCompletenessV1::Complete => CaptureCompletenessV1::Complete,
            DebugTranscriptCompletenessV1::Truncated(reason) => CaptureCompletenessV1::Truncated {
                reason: transcript_truncation_reason(reason),
                emitted_events: self.session.transcript().records().len() as u64,
                dropped_events: None,
            },
        }
    }
    fn observed_preflight(&self, revision: u64) -> Result<(), (DebugErrorCodeV1, &'static str)> {
        if revision != self.revision {
            return Err((
                DebugErrorCodeV1::StaleRevision,
                "expected_revision does not match the current session",
            ));
        }
        if self.terminated {
            return Err((
                DebugErrorCodeV1::InvalidState,
                "debug session is terminated",
            ));
        }
        if self.command_count >= MAX_SESSION_COMMANDS_V1 {
            return Err((
                DebugErrorCodeV1::ResourceLimit,
                "debug session command budget exhausted",
            ));
        }
        Ok(())
    }
    pub(crate) fn handle_runtime_observation_v1(
        &mut self,
        request: RuntimeObservationRequestV1,
    ) -> RuntimeObservationResponseV1 {
        let binding = self.observed_binding();
        self.runtime_queries.refresh(binding);
        if request.validate(self.protocol_limits).is_err() {
            return self.runtime_error(
                &request,
                DebugErrorCodeV1::InvalidRequest,
                "runtime observation request is invalid",
            );
        }
        if let Err((code, message)) = self.observed_preflight(request.expected_revision()) {
            return self.runtime_error(&request, code, message);
        }
        if request.expected_cursor() != self.session_view().cursor {
            return self.runtime_error(
                &request,
                DebugErrorCodeV1::InvalidCursor,
                "expected_cursor does not match the complete current cursor",
            );
        }
        if request
            .expected_owner()
            .is_some_and(|expected| binding.is_none_or(|actual| actual.owner != expected))
        {
            return self.runtime_error(
                &request,
                DebugErrorCodeV1::InvalidCursor,
                "runtime owner belongs to another backend lifetime or capture",
            );
        }
        self.command_count += 1;
        let Some(binding) = binding else {
            return RuntimeObservationResponseV1::Unavailable {
                schema: RuntimeObservationResponseSchemaV1::V1,
                request_id: request.request_id(),
                session: self.session_view(),
                binding: None,
                completeness: self.observed_completeness(),
                reason: RuntimeObservationUnavailableV1::NotRequested,
            };
        };
        let Some(observed) = self.session.observed() else {
            return self.runtime_error(
                &request,
                DebugErrorCodeV1::BackendFailure,
                "observed owner disappeared",
            );
        };
        let Some(record) = observed.legacy().current() else {
            return RuntimeObservationResponseV1::Unavailable {
                schema: RuntimeObservationResponseSchemaV1::V1,
                request_id: request.request_id(),
                session: self.session_view(),
                binding: Some(binding),
                completeness: self.observed_completeness(),
                reason: RuntimeObservationUnavailableV1::NoSelectedRecord,
            };
        };
        let mut work = match self.runtime_queries.work() {
            Ok(work) => work,
            Err(message) => {
                return self.runtime_error(&request, DebugErrorCodeV1::ResourceLimit, message);
            }
        };
        let before = work.remaining();
        // Prepay the complete bounded roster projection. No replay or semantic
        // execution occurs here, and a failed charge never emits a partial stack.
        let frame_count = observed.current_frames().map_or(0, |frames| frames.len());
        if work.charge(frame_count.saturating_add(4)).is_err() {
            return self.runtime_error(
                &request,
                DebugErrorCodeV1::ResourceLimit,
                "runtime metadata projection work limit",
            );
        }
        let response = RuntimeObservationResponseV1::Ok {
            schema: RuntimeObservationResponseSchemaV1::V1,
            request_id: request.request_id(),
            session: self.session_view(),
            binding,
            invocation: Box::new(mapping::invocation(record.invocation)),
            origin: mapping::origin(observed.current_origin()),
            frames: mapping::frames(
                observed.current_frames(),
                self.protocol_limits.max_response_items,
            ),
            allocation_watermark: match observed.current_allocations() {
                Ok(value) => RuntimeAllocationWatermarkV1::Available {
                    through_sequence: number(value.through_sequence()),
                },
                Err(error) => RuntimeAllocationWatermarkV1::Unavailable {
                    reason: mapping::allocation_missing(error),
                },
            },
            completeness: self.observed_completeness(),
            origin_coverage: mapping::coverage(
                observed.origin_coverage(),
                observed.origin_metadata_usage().retained_rows,
            ),
            frame_coverage: mapping::coverage(
                observed.frame_coverage(),
                observed.frame_metadata_usage().retained_records,
            ),
            lifecycle_coverage: mapping::coverage(
                observed.allocation_coverage(),
                observed.allocation_metadata_usage().retained_records,
            ),
        };
        self.runtime_queries.settle_work(before, work.remaining());
        if response
            .validate_for_request(&request, self.protocol_limits)
            .is_err()
        {
            return self.runtime_error(
                &request,
                DebugErrorCodeV1::BackendFailure,
                "runtime producer projection did not satisfy its closed wire contract",
            );
        }
        response
    }
    pub(crate) fn handle_resource_queries_v2(
        &mut self,
        request: ResourceRequestV2,
    ) -> ResourceResponseV2 {
        let binding = self.observed_binding();
        self.runtime_queries.refresh(binding);
        if request.validate(self.protocol_limits).is_err() {
            return self.observed_resource_error(
                &request,
                DebugErrorCodeV1::InvalidRequest,
                "observed resource request is invalid",
            );
        }
        if let Err((code, message)) = self.observed_preflight(request.expected_revision()) {
            return self.observed_resource_error(&request, code, message);
        }
        let Some(binding) = binding else {
            return self.observed_resource_error(
                &request,
                DebugErrorCodeV1::InvalidState,
                "runtime observations were not enabled for this backend",
            );
        };
        if request.expected_binding() != binding {
            return self.observed_resource_error(
                &request,
                DebugErrorCodeV1::InvalidCursor,
                "resource binding does not match the exact current owner and cursor",
            );
        }
        self.command_count += 1;
        let mut work = match self.runtime_queries.work() {
            Ok(work) => work,
            Err(message) => {
                return self.observed_resource_error(
                    &request,
                    DebugErrorCodeV1::ResourceLimit,
                    message,
                );
            }
        };
        let before = work.remaining();
        let result = match self.runtime_queries.take_page(&request, &mut work) {
            Ok(start) => self.project_observed_resource(&request, start, &mut work),
            Err(message) => Err(QueryFailure::invalid(message)),
        };
        self.runtime_queries.settle_work(before, work.remaining());
        let projected = match result {
            Ok(projected) => projected,
            Err(QueryFailure::Unavailable(reason)) => {
                return ResourceResponseV2::Unavailable {
                    schema: ResourceResponseSchemaV2::V2,
                    request_id: request.request_id(),
                    operation: request.operation(),
                    session: self.session_view(),
                    binding,
                    completeness: self.observed_completeness(),
                    reason,
                };
            }
            Err(QueryFailure::Refused(code, message)) => {
                return self.observed_resource_error(&request, code, message);
            }
        };
        let next_token = match self.runtime_queries.retain_page(&request, projected.next) {
            Ok(token) => token,
            Err(message) => {
                return self.observed_resource_error(
                    &request,
                    DebugErrorCodeV1::ResourceLimit,
                    message,
                );
            }
        };
        let response = ResourceResponseV2::Ok {
            schema: ResourceResponseSchemaV2::V2,
            request_id: request.request_id(),
            operation: request.operation(),
            session: self.session_view(),
            binding,
            through_sequence: number(projected.through),
            completeness: self.observed_completeness(),
            page: projected
                .page
                .map(|(source_count, source_start, scanned)| ResourcePageInfoV2 {
                    source_count: number(source_count),
                    source_start: number(source_start),
                    scanned,
                    next_token,
                }),
            result: projected.result,
        };
        if response
            .validate_for_request(&request, self.protocol_limits)
            .is_err()
        {
            // Drop any just-created token together with the invalid projection.
            self.runtime_queries.refresh(None);
            return self.observed_resource_error(
                &request,
                DebugErrorCodeV1::BackendFailure,
                "resource producer projection did not satisfy its closed wire contract",
            );
        }
        response
    }
    fn runtime_error(
        &self,
        request: &RuntimeObservationRequestV1,
        code: DebugErrorCodeV1,
        message: &str,
    ) -> RuntimeObservationResponseV1 {
        RuntimeObservationResponseV1::Error {
            schema: RuntimeObservationResponseSchemaV1::V1,
            request_id: request.request_id(),
            session: self.session_view(),
            error: query_error(code, message),
        }
    }
    fn observed_resource_error(
        &self,
        request: &ResourceRequestV2,
        code: DebugErrorCodeV1,
        message: &str,
    ) -> ResourceResponseV2 {
        ResourceResponseV2::Error {
            schema: ResourceResponseSchemaV2::V2,
            request_id: request.request_id(),
            operation: request.operation(),
            session: self.session_view(),
            error: query_error(code, message),
        }
    }
}
fn query_error(code: DebugErrorCodeV1, message: &str) -> DebugErrorV1 {
    DebugErrorV1 {
        stage: DebugErrorStageV1::Session,
        code,
        message: bounded_message(message),
        state_changed: false,
    }
}

#[cfg(test)]
#[path = "runtime_queries_tests.rs"]
mod tests;
