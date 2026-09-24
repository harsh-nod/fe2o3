//! Explicit Context opt-in to the directed backend profile.

use super::peer_reconciliation::DirectedPeerStateV1;
use super::*;

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    fn directed_route_v1(
        &self,
        root: &ScalarPeerCopyRootV1,
    ) -> Result<BackendDirectedPeerRouteV1, RuntimeValidationErrorV1> {
        let translate = |source: ContextReadSourceV1| BackendMemoryRegionV1 {
            allocation: source.record.backend_allocation,
            access: source.region.access,
            byte_offset: source.region.byte_offset,
            byte_len: source.region.byte_len,
        };
        Ok(BackendDirectedPeerRouteV1 {
            stream: root.backend_stream,
            source_device: self.device(root.source.record.device)?.backend_device,
            destination_device: self.device(root.destination.record.device)?.backend_device,
            source: translate(root.source),
            destination: translate(root.destination),
        })
    }
}

impl<B: RuntimeDirectedScalarPeerCopyBackendV1> RuntimeContextV1<B> {
    /// Submit a success-gated scalar peer copy with exact directed dependencies.
    ///
    /// With a version journal, one input may reserve an earlier directed copy's
    /// pending output when its exact event is supplied and its written range covers
    /// the input. Legacy/ordered/generated producers and aliased events reject.
    /// Native progress and logical producer reconciliation are separate; neither
    /// this call nor dropping the returned handle establishes completion.
    pub fn directed_peer_copy_v1(
        &mut self,
        stream: RuntimeStreamIdV1,
        source: RuntimeMemoryRegionV1,
        destination: RuntimeMemoryRegionV1,
        dependencies: &[RuntimeEventIdV1],
    ) -> Result<RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>, RuntimeErrorV1<B::Error>>
    {
        let prepared =
            self.prepare_context_peer_copy_v1(stream, source, destination, dependencies, true)?;
        let mut root =
            self.prepare_scalar_peer_custody_v1(stream, source, destination, dependencies)?;
        if root
            .dependencies
            .windows(2)
            .any(|pair| pair[0].submission == pair[1].submission)
        {
            return Err(RuntimeValidationErrorV1::DuplicateDependency.into());
        }
        let mut depth = 1;
        let mut roster = [BackendDirectedPeerDependencyV1 {
            event: 0,
            producer_submission: 0,
        }; MAX_RUNTIME_DEPENDENCIES_V1];
        for dependency in &root.dependencies {
            self.check_scalar_peer_custody_v1(dependency.submission)?;
            let producer = self
                .scalar_peer_copies
                .get(&dependency.submission)
                .and_then(|root| root.directed.as_ref())
                .ok_or(RuntimeValidationErrorV1::Unsupported)?;
            depth = depth.max(
                producer
                    .depth
                    .checked_add(1)
                    .ok_or(RuntimeValidationErrorV1::Capacity)?,
            );
            roster[dependency.ordinal] = BackendDirectedPeerDependencyV1 {
                event: dependency.backend_event,
                producer_submission: dependency.backend_submission,
            };
        }
        if depth > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(RuntimeValidationErrorV1::TooManyDependencies.into());
        }
        root.directed = Some(DirectedPeerStateV1 {
            depth,
            cursor: 0,
            terminal: None,
        });
        let route = self.directed_route_v1(&root)?;
        let roster = &roster[..root.dependencies.len()];
        self.submit_context_operation_v1(
            stream,
            prepared.stream_record,
            &[destination.allocation],
            Some(PreparedSubmissionCustodyV1::Peer(
                PreparedPeerSubmissionV1 {
                    mechanism: PeerTransferMechanismV1::DeclaredPeerCopy {
                        contract_identity: peer_copy_contract_identity(stream, source, destination),
                    },
                    scalar: Some(root),
                },
            )),
            &[prepared.journal_source],
            |backend| {
                backend.submit_directed_scalar_peer_copy_v1(BackendDirectedScalarPeerCopyV1 {
                    route,
                    dependencies: roster,
                })
            },
        )
    }

    /// Drive at most one native action, or one retained producer observation.
    ///
    /// Local reconciliation uses at most 513 cursor/traversal steps per pass and
    /// a fixed stack of 256 identities. It may yield Pending after device success.
    /// Repeat until conclusive; no thread, sleep or generic stream flush is added.
    pub fn progress_directed_peer_copy_v1(
        &mut self,
        submission: &mut RuntimeSubmissionV1<RuntimeDirectedScalarPeerCopyV1>,
    ) -> Result<RuntimePollV1, RuntimeErrorV1<B::Error>> {
        self.require_live()?;
        let record = self.submission_record(submission)?;
        self.require_retained_submission_unheld_v1(&record)?;
        self.check_scalar_peer_custody_v1(submission.id)?;
        if !record.directed_peer_copy {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        if record.status.is_terminal() {
            return Ok(submission.observe_status(record.status));
        }
        let root = &self.scalar_peer_copies[&submission.id];
        let route = self.directed_route_v1(root)?;
        let mut producers = [0; MAX_RUNTIME_DEPENDENCIES_V1];
        for dependency in &root.dependencies {
            producers[dependency.ordinal] = dependency.backend_submission;
        }
        let producers = &producers[..root.dependencies.len()];
        let status = self.observe_completion_step_v1(submission.id, |backend, id| {
            backend.progress_directed_scalar_peer_copy_v1(BackendDirectedScalarProgressV1 {
                submission: id,
                route,
                producer_submissions: producers,
            })
        })?;
        Ok(submission.observe_status(status))
    }
}
