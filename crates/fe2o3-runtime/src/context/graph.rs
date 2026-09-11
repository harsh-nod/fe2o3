//! Private graph reservation and the ordinary context's shared admission paths.

use super::*;
use fe2o3_completion::{ContextIdentityV1, DeviceIdentityV1, StreamIdentityV1};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ContextGraphReservationV1 {
    context_generation: u64,
    local: u64,
}

impl ContextGraphReservationV1 {
    pub(crate) const fn generation(self) -> u64 {
        self.local
    }
}

pub(crate) struct PreparedContextCopyV1 {
    pub(super) stream: RuntimeStreamIdV1,
    pub(super) stream_record: StreamRecordV1,
    pub(super) source: BackendMemoryRegionV1,
    pub(super) destination: BackendMemoryRegionV1,
    pub(super) backend_dependencies: Vec<u64>,
}

pub(crate) enum PreparedContextGraphActionV1 {
    Launch(PreparedContextLaunchV1),
    Copy(PreparedContextCopyV1),
}

fn identity_bytes(domain: &[u8; 16], generation: u64, local: u64) -> [u8; 32] {
    let mut bytes = [0; 32];
    bytes[..16].copy_from_slice(domain);
    bytes[16..24].copy_from_slice(&generation.to_le_bytes());
    bytes[24..].copy_from_slice(&local.to_le_bytes());
    bytes
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    /// Process-local descriptive graph identity, not device or compiler authority.
    pub fn completion_context_identity_v1(
        &self,
        device: RuntimeDeviceIdV1,
    ) -> Result<ContextIdentityV1, RuntimeValidationErrorV1> {
        self.device(device)?;
        let bytes = identity_bytes(b"fe2o3.graph.dev1", self.context_generation, device.local);
        Ok(ContextIdentityV1::new(
            DeviceIdentityV1::from_bytes(bytes),
            identity_bytes(b"fe2o3.graph.ctx1", self.context_generation, device.local),
        ))
    }

    /// Exact current stream identity for a runtime-bound, single-device graph.
    pub fn completion_stream_identity_v1(
        &self,
        stream: RuntimeStreamIdV1,
    ) -> Result<StreamIdentityV1, RuntimeValidationErrorV1> {
        let record = self
            .streams
            .get(&stream)
            .ok_or(RuntimeValidationErrorV1::UnknownStream)?;
        Ok(StreamIdentityV1::new(
            self.completion_context_identity_v1(record.device)?,
            identity_bytes(b"fe2o3.graph.str1", self.context_generation, stream.local),
        ))
    }

    pub(crate) fn reserve_graph_v1(
        &mut self,
        nodes: usize,
    ) -> Result<ContextGraphReservationV1, RuntimeValidationErrorV1> {
        self.require_live()?;
        if !fe2o3_runtime_model::r63_graph_can_acquire_v1(
            self.terminal,
            self.graph_reservation.is_some(),
            self.submissions.len(),
            self.events.len(),
        ) || self.completion_callback_count != 0
            || self.has_unpublished_holds_v1()
        {
            return Err(RuntimeValidationErrorV1::ContextReserved);
        }
        if nodes > MAX_RUNTIME_SUBMISSIONS_V1
            || self
                .next_identity
                .checked_add(nodes as u64)
                .and_then(|n| n.checked_add(1))
                .is_none()
        {
            return Err(RuntimeValidationErrorV1::Capacity);
        }
        self.submissions
            .try_reserve(nodes)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        self.backend_submissions
            .try_reserve(nodes)
            .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
        let token = ContextGraphReservationV1 {
            context_generation: self.context_generation,
            local: self.next_id()?,
        };
        self.graph_reservation = Some(token);
        self.graph_issue_closed = false;
        Ok(token)
    }

    pub(crate) fn close_graph_issue_v1(
        &mut self,
        token: ContextGraphReservationV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_graph_access(Some(token))?;
        self.graph_issue_closed = true;
        Ok(())
    }

    // Closing is permanent for this exact reservation generation.
    pub(crate) fn release_graph_v1(
        &mut self,
        token: ContextGraphReservationV1,
    ) -> Result<(), RuntimeValidationErrorV1> {
        self.require_graph_access(Some(token))?;
        if !fe2o3_runtime_model::r63_graph_can_release_v1(
            self.terminal,
            self.graph_reservation == Some(token),
            self.graph_issue_closed,
            self.submissions.len(),
            self.events.len(),
        ) {
            return Err(RuntimeValidationErrorV1::SubmissionPending);
        }
        self.graph_reservation = None;
        Ok(())
    }

    pub(crate) fn prepare_graph_launch_v1<A: RuntimeArgumentsV1>(
        &self,
        stream: RuntimeStreamIdV1,
        kernel: &TypedRuntimeKernelV1<A>,
        bytes: &[u8],
        bindings: &[RuntimeBindingV1],
        geometry: RuntimeLaunchGeometryV1,
    ) -> Result<PreparedContextGraphActionV1, RuntimeErrorV1<B::Error>> {
        self.prepare_context_launch_v1(
            ContextLaunchRequestV1 {
                stream,
                kernel,
                arguments: ContextLaunchArgumentsV1::Frozen(bytes, bindings),
                geometry,
                dependencies: &[],
                semantic_launch: BackendSemanticLaunchV1::Ordinary,
            },
            None,
        )
        .map(PreparedContextGraphActionV1::Launch)
    }

    pub(crate) fn prepare_graph_copy_v1(
        &self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
    ) -> Result<PreparedContextGraphActionV1, RuntimeErrorV1<B::Error>> {
        self.prepare_context_copy_v1(stream, source, destination, &[], None)
            .map(PreparedContextGraphActionV1::Copy)
    }

    pub(crate) fn submit_graph_action_v1(
        &mut self,
        token: ContextGraphReservationV1,
        action: PreparedContextGraphActionV1,
    ) -> Result<RuntimeSubmissionV1<()>, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        if !fe2o3_runtime_model::r63_graph_can_issue_v1(
            self.terminal,
            self.graph_reservation == Some(token),
            self.graph_issue_closed,
        ) {
            return Err(RuntimeValidationErrorV1::ContextReserved.into());
        }
        match action {
            PreparedContextGraphActionV1::Launch(launch) => {
                self.submit_prepared_launch_v1(launch, Some(token), |backend, launch| {
                    backend.submit_v1(launch)
                })
            }
            PreparedContextGraphActionV1::Copy(copy) => {
                self.submit_prepared_copy_v1(copy, Some(token))
            }
        }
    }

    pub(crate) fn release_graph_submission_v1(
        &mut self,
        token: ContextGraphReservationV1,
        submission: &RuntimeSubmissionV1<()>,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        self.release_submission_ref(submission, Some(token))
    }
}
