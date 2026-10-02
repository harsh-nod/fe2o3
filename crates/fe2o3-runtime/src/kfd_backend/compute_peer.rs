//! Exact pending compute writers retained by the existing cooperative copy owner.

use super::*;

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

fn compute_identity(child: &KfdRuntimeBackendV1, id: u64) -> Option<(u64, usize)> {
    child
        .pending_compute
        .get(&id)
        .map(|pending| (pending.launch.stream, pending.dependency_depth))
        .or_else(|| {
            child
                .active_compute_submission_v1(id)
                .map(|active| (active.stream, active.dependency_depth))
        })
        .or_else(|| {
            child
                .submissions
                .get(&id)
                .map(|record| (record.stream, record.dependency_depth))
        })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationIdentity {
    device: u64,
    kind: RuntimeMemoryKindV1,
    bytes: usize,
    alignment: u64,
}

impl AllocationIdentity {
    fn of(record: &AllocationRecordV1) -> Self {
        Self {
            device: record.device,
            kind: record.kind,
            bytes: record.bytes.len(),
            alignment: record.alignment,
        }
    }
}

#[derive(Debug)]
pub(super) struct Producer {
    pub(super) id: u64,
    route: RoutedHandleV1,
    stream: u64,
    depth: usize,
    launch: Arc<RetainedComputeLaunchV1>,
    endpoints: [(u64, RoutedHandleV1, AllocationIdentity); 2],
}

impl Producer {
    pub(super) fn depth(&self) -> usize {
        self.depth
    }

    pub(super) fn owns_source(&self, child: &KfdRuntimeBackendV1, route: RoutedHandleV1) -> bool {
        route == self.endpoints[0].1
            && child
                .allocation_custody
                .get(&route.local)
                .is_some_and(|custody| {
                    custody.owners.len() == 1
                        && custody.owners.front().is_some_and(|owner| {
                            owner.kind == RuntimeAllocationCustodyKindV1::Compute
                                && owner.submission == self.route.local
                                && owner.stream == self.launch.stream
                        })
                })
    }

    fn intact(&self, backend: &KfdMultiDeviceRuntimeBackendV1) -> bool {
        let child = &backend.children[self.route.child];
        backend
            .producer_aware_native
            .get(&self.id)
            .is_some_and(|launch| Arc::ptr_eq(launch, &self.launch))
            && matches!(backend.submissions.get(&self.id),
                Some(RoutedSubmissionV1::Native { route, stream })
                    if *route == self.route && *stream == self.stream)
            && backend.streams.get(&self.stream).map_or_else(
                || child.exact_submission_quiescent_v1(self.route.local),
                |route| {
                    *route
                        == RoutedHandleV1 {
                            child: self.route.child,
                            local: self.launch.stream,
                        }
                },
            )
            && compute_identity(child, self.route.local) == Some((self.launch.stream, self.depth))
            && self.endpoints.iter().all(|(global, route, identity)| {
                backend.allocations.get(global) == Some(route)
                    && backend.children[route.child]
                        .allocations
                        .get(&route.local)
                        .is_some_and(|record| AllocationIdentity::of(record) == *identity)
            })
            && child
                .pending_compute
                .get(&self.route.local)
                .map(|pending| Arc::ptr_eq(&pending.launch, &self.launch))
                .or_else(|| {
                    child
                        .active_compute_submission_v1(self.route.local)
                        .map(|active| {
                            active.stream == self.launch.stream
                                && active.kernel == self.launch.kernel
                                && active.dependency_depth == self.depth
                                && active.allocations.contains(&self.endpoints[0].1.local)
                                && active
                                    .ordinary_recipe
                                    .as_ref()
                                    .is_none_or(|launch| Arc::ptr_eq(launch, &self.launch))
                        })
                })
                .unwrap_or_else(|| child.exact_submission_quiescent_v1(self.route.local))
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn prepare_compute_peer_v1(
        &self,
        source: RoutedHandleV1,
        source_region: BackendMemoryRegionV1,
        destination: RoutedHandleV1,
        destination_region: BackendMemoryRegionV1,
        events: &[u64],
    ) -> Result<Option<Producer>, Failure> {
        let child = &self.children[source.child];
        let target = &self.children[destination.child];
        if source.child == destination.child
            || !self
                .compute_xgmi_routes
                .contains_key(&(source.child, destination.child))
            || !child.peer_visible_device_allocations
            || !target.peer_visible_device_allocations
            || source_region.access != RuntimeAccessV1::Read
            || destination_region.access != RuntimeAccessV1::Write
            || !compute_xgmi::full_extent(&child.allocations[&source.local], source_region)
            || !compute_xgmi::full_extent(
                &target.allocations[&destination.local],
                destination_region,
            )
        {
            return Ok(None);
        }
        let Some(custody) = child.allocation_custody.get(&source.local) else {
            return Ok(None);
        };
        let Some(owner) = custody.owners.front().filter(|owner| {
            custody.owners.len() == 1 && owner.kind == RuntimeAllocationCustodyKindV1::Compute
        }) else {
            return Ok(None);
        };
        for event in events {
            let Some(RoutedEventV1::Native {
                route: event_route,
                submission,
            }) = self.events.get(event)
            else {
                continue;
            };
            let Some(RoutedSubmissionV1::Native { route, stream }) =
                self.submissions.get(submission)
            else {
                continue;
            };
            if route.child != source.child
                || route.local != owner.submission
                || event_route.child != source.child
                || child
                    .events
                    .get(&event_route.local)
                    .is_none_or(|event| event.submission != route.local)
                || !self.producer_aware_native.contains_key(submission)
            {
                continue;
            }
            let Some(launch) = self.producer_aware_native.get(submission) else {
                continue;
            };
            if launch.semantic_launch != BackendSemanticLaunchV1::Ordinary
                || launch.stream != owner.stream
                || launch
                    .bindings
                    .iter()
                    .filter(|binding| binding.region.allocation == source.local)
                    .count()
                    != 1
                || !launch.bindings.iter().any(|binding| {
                    binding.region.allocation == source.local
                        && binding.region.access == RuntimeAccessV1::Write
                        && binding.region.byte_offset == 0
                        && binding.region.byte_len == source_region.byte_len
                })
            {
                continue;
            }
            let Some((_, depth)) = compute_identity(child, route.local) else {
                continue;
            };
            if !child.pending_compute.contains_key(&route.local)
                && child.active_compute_submission_v1(route.local).is_none()
            {
                continue;
            }
            let producer = Producer {
                id: *submission,
                route: *route,
                stream: *stream,
                depth,
                launch: Arc::clone(launch),
                endpoints: [
                    (
                        source_region.allocation,
                        source,
                        AllocationIdentity::of(&child.allocations[&source.local]),
                    ),
                    (
                        destination_region.allocation,
                        destination,
                        AllocationIdentity::of(&target.allocations[&destination.local]),
                    ),
                ],
            };
            if !producer.intact(self) || !producer.owns_source(child, source) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "compute peer producer identity does not match retained custody",
                ));
            }
            for other in events {
                let id =
                    self.peer_dependency_submission(*other, source.child, destination.child)?;
                if id != producer.id && !self.compute_peer_dependency_succeeded_v1(id) {
                    return Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::Unsupported,
                        "pending compute peer requires one writer and already successful control dependencies",
                    ));
                }
            }
            return Ok(Some(producer));
        }
        Ok(None)
    }

    pub(super) fn compute_peer_dependency_depth_v1(&self, id: u64) -> Option<usize> {
        match self.submissions.get(&id)? {
            RoutedSubmissionV1::Native { route, .. } => {
                compute_identity(&self.children[route.child], route.local).map(|(_, depth)| depth)
            }
            RoutedSubmissionV1::CooperativeCopy(copy) => Some(copy.dependency_depth),
            RoutedSubmissionV1::DeferredCompute(_) => None,
        }
    }

    fn compute_peer_dependency_succeeded_v1(&self, id: u64) -> bool {
        match self.submissions.get(&id) {
            Some(RoutedSubmissionV1::Native { route, .. }) => {
                let child = &self.children[route.child];
                child
                    .submissions
                    .get(&route.local)
                    .is_some_and(|record| record.status == BackendPollV1::Succeeded)
                    && child.exact_submission_quiescent_v1(route.local)
            }
            Some(RoutedSubmissionV1::CooperativeCopy(copy)) => {
                copy.status() == BackendPollV1::Succeeded && copy.is_quiescent()
            }
            _ => false,
        }
    }

    pub(super) fn validate_compute_peer_v1(&mut self, id: u64) -> Result<(), Failure> {
        let intact = match self.submissions.get(&id) {
            Some(RoutedSubmissionV1::CooperativeCopy(copy)) => {
                copy.compute_producer.as_ref().is_none_or(|producer| {
                    producer.intact(self)
                        && copy.compute_xgmi.is_some()
                        && copy.directed.is_none()
                        && copy.source == producer.endpoints[0].1
                        && copy.source_region.allocation == producer.endpoints[0].0
                        && copy.destination == producer.endpoints[1].1
                        && copy.destination_region.allocation == producer.endpoints[1].0
                        && copy.dependencies.contains(&producer.id)
                        && self.submission_retained_as_dependency(producer.id)
                })
            }
            _ => false,
        };
        if intact {
            Ok(())
        } else {
            self.terminal = true;
            Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "pending compute peer lost its exact producer or allocation roots",
                ),
            ))
        }
    }

    pub(super) fn progress_compute_peer_dependency_v1(
        &mut self,
        id: u64,
        dependency: u64,
    ) -> Result<BackendPollV1, Failure> {
        let producer = match &self.submissions[&id] {
            RoutedSubmissionV1::CooperativeCopy(copy) => copy
                .compute_producer
                .as_ref()
                .filter(|producer| producer.id == dependency)
                .map(|producer| (producer.route, producer.launch.stream)),
            _ => unreachable!(),
        };
        let Some((route, stream)) = producer else {
            return self.observe_dependency(dependency);
        };
        self.validate_compute_peer_v1(id)?;
        if self.compute_xgmi_child_occupied_v1(route.child) {
            return Ok(BackendPollV1::Pending);
        }
        // Flush only while the exact target is still queued. Once it is active or
        // complete, another stream head is not this copy's publication authority.
        if self.children[route.child]
            .pending_compute
            .contains_key(&route.local)
        {
            if let Err(error) = self.service_native_peer_prefix_v1(route, true) {
                return self.attribute_compute_peer_progress_error_v1(id, route, error);
            }
            if self.children[route.child]
                .pending_compute
                .contains_key(&route.local)
            {
                let result = self.children[route.child].flush_stream_v1(stream);
                if let Err(error) = result {
                    return self.attribute_compute_peer_progress_error_v1(id, route, error);
                }
            }
        }
        let status = match self.observe_dependency(dependency) {
            Ok(status) => status,
            Err(error) => return self.attribute_compute_peer_progress_error_v1(id, route, error),
        };
        if status == BackendPollV1::Succeeded
            && !self.children[route.child].exact_submission_quiescent_v1(route.local)
        {
            return Ok(BackendPollV1::Pending);
        }
        Ok(status)
    }

    pub(super) fn attribute_compute_peer_progress_error_v1(
        &mut self,
        id: u64,
        route: RoutedHandleV1,
        error: Failure,
    ) -> Result<BackendPollV1, Failure> {
        match error {
            RuntimeBackendFailureV1::Rejected(error)
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy =>
            {
                Ok(BackendPollV1::Pending)
            }
            RuntimeBackendFailureV1::Quiescent(error) => {
                if !self.children[route.child].exact_submission_quiescent_v1(route.local) {
                    return Ok(BackendPollV1::Pending);
                }
                self.fail_cooperative_copy(id);
                Err(RuntimeBackendFailureV1::Quiescent(error))
            }
            error => self.latch(Err(error)),
        }
    }
}
