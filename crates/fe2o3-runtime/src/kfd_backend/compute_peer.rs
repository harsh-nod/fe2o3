//! Exact pending compute writers retained by the existing cooperative copy owner.

use super::*;

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

pub(super) fn compute_identity(child: &KfdRuntimeBackendV1, id: u64) -> Option<(u64, usize)> {
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

pub(super) fn retained_child_launch_intact_v1(
    child: &KfdRuntimeBackendV1,
    id: u64,
    depth: usize,
    launch: &Arc<RetainedComputeLaunchV1>,
) -> bool {
    compute_identity(child, id) == Some((launch.stream, depth))
        && child
            .pending_compute
            .get(&id)
            .map(|pending| Arc::ptr_eq(&pending.launch, launch))
            .or_else(|| {
                child.active_compute_submission_v1(id).map(|active| {
                    active.stream == launch.stream
                        && active.kernel == launch.kernel
                        && active.dependency_depth == depth
                        && launch
                            .bindings
                            .iter()
                            .all(|binding| active.allocations.contains(&binding.region.allocation))
                        && active
                            .ordinary_recipe
                            .as_ref()
                            .is_none_or(|recipe| Arc::ptr_eq(recipe, launch))
                })
            })
            .unwrap_or_else(|| child.exact_submission_quiescent_v1(id))
}

#[derive(Clone, Copy, Debug)]
enum Origin {
    Native(RoutedHandleV1),
    Deferred { child: usize },
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
    origin: Origin,
    stream: u64,
    depth: usize,
    launch: Arc<RetainedComputeLaunchV1>,
    endpoints: [(u64, RoutedHandleV1, AllocationIdentity); 2],
}

impl Producer {
    pub(super) fn depth(&self) -> usize {
        self.depth
    }

    pub(super) fn reserves_deferred_source(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        route: RoutedHandleV1,
    ) -> bool {
        matches!(self.origin, Origin::Deferred { .. }) && self.owns_source(backend, route)
    }

    pub(super) fn owns_source(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        route: RoutedHandleV1,
    ) -> bool {
        let Origin::Native(producer_route) = self.origin else {
            return route == self.endpoints[0].1
                && backend.deferred_peer_source_reserved_v1(self.id, route);
        };
        let child = &backend.children[route.child];
        route == self.endpoints[0].1
            && child
                .allocation_custody
                .get(&route.local)
                .is_some_and(|custody| {
                    custody.owners.len() == 1
                        && custody.owners.front().is_some_and(|owner| {
                            owner.kind == RuntimeAllocationCustodyKindV1::Compute
                                && owner.submission == producer_route.local
                                && owner.stream == self.launch.stream
                        })
                })
    }

    fn intact(&self, backend: &KfdMultiDeviceRuntimeBackendV1) -> bool {
        if !self.endpoints.iter().all(|(global, route, identity)| {
            backend.allocations.get(global) == Some(route)
                && backend.children[route.child]
                    .allocations
                    .get(&route.local)
                    .is_some_and(|record| AllocationIdentity::of(record) == *identity)
        }) {
            return false;
        }
        let Origin::Native(producer_route) = self.origin else {
            let Origin::Deferred { child } = self.origin else {
                unreachable!()
            };
            return backend.deferred_peer_producer_intact_v1(
                self.id,
                child,
                self.stream,
                self.depth,
                &self.launch,
            );
        };
        let child = &backend.children[producer_route.child];
        backend
            .producer_aware_native
            .get(&self.id)
            .is_some_and(|launch| Arc::ptr_eq(launch, &self.launch))
            && matches!(backend.submissions.get(&self.id),
                Some(RoutedSubmissionV1::Native { route, stream })
                    if *route == producer_route && *stream == self.stream)
            && backend.streams.get(&self.stream).map_or_else(
                || child.exact_submission_quiescent_v1(producer_route.local),
                |route| {
                    *route
                        == RoutedHandleV1 {
                            child: producer_route.child,
                            local: self.launch.stream,
                        }
                },
            )
            && compute_identity(child, producer_route.local)
                == Some((self.launch.stream, self.depth))
            && child
                .pending_compute
                .get(&producer_route.local)
                .map(|pending| Arc::ptr_eq(&pending.launch, &self.launch))
                .or_else(|| {
                    child
                        .active_compute_submission_v1(producer_route.local)
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
                .unwrap_or_else(|| child.exact_submission_quiescent_v1(producer_route.local))
    }

    pub(super) fn deferred_event(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        event: u64,
    ) -> bool {
        matches!(self.origin, Origin::Deferred { child }
            if matches!(backend.events.get(&event),
                Some(RoutedEventV1::DeferredCompute { submission, child: event_child })
                    if *submission == self.id && *event_child == child))
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
        for event in events {
            let Some(RoutedEventV1::DeferredCompute {
                submission,
                child: event_child,
            }) = self.events.get(event)
            else {
                continue;
            };
            if *event_child != source.child {
                continue;
            }
            let Some((stream, depth, launch)) =
                self.pending_deferred_peer_producer_v1(*submission, source.child)
            else {
                continue;
            };
            if launch.semantic_launch != BackendSemanticLaunchV1::Ordinary
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
            let producer = Producer {
                id: *submission,
                origin: Origin::Deferred {
                    child: source.child,
                },
                stream,
                depth,
                launch,
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
            if !producer.intact(self) || !producer.owns_source(self, source) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "deferred compute peer producer does not match exact source custody",
                ));
            }
            self.require_compute_peer_controls_v1(
                &producer,
                events,
                source.child,
                destination.child,
            )?;
            return Ok(Some(producer));
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
                origin: Origin::Native(*route),
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
            if !producer.intact(self) || !producer.owns_source(self, source) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "compute peer producer identity does not match retained custody",
                ));
            }
            self.require_compute_peer_controls_v1(
                &producer,
                events,
                source.child,
                destination.child,
            )?;
            return Ok(Some(producer));
        }
        Ok(None)
    }

    fn require_compute_peer_controls_v1(
        &self,
        producer: &Producer,
        events: &[u64],
        source: usize,
        destination: usize,
    ) -> Result<(), Failure> {
        for event in events {
            if producer.deferred_event(self, *event) {
                continue;
            }
            let id = self.peer_dependency_submission(*event, source, destination)?;
            if id != producer.id && !self.compute_peer_dependency_succeeded_v1(id) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "pending compute peer requires one writer and already successful control dependencies",
                ));
            }
        }
        Ok(())
    }

    pub(super) fn compute_peer_dependency_depth_v1(&self, id: u64) -> Option<usize> {
        match self.submissions.get(&id)? {
            RoutedSubmissionV1::Native { route, .. } => {
                compute_identity(&self.children[route.child], route.local).map(|(_, depth)| depth)
            }
            RoutedSubmissionV1::CooperativeCopy(copy) => Some(copy.dependency_depth),
            RoutedSubmissionV1::DeferredCompute(_) => self.deferred_peer_depth_v1(id),
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
                    producer.id < id
                        && producer.depth > 0
                        && producer.depth < copy.dependency_depth
                        && copy.dependency_depth <= MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1
                        && producer.intact(self)
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
                .map(|producer| (producer.origin, producer.launch.stream)),
            _ => unreachable!(),
        };
        let Some((origin, stream)) = producer else {
            return self.observe_dependency(dependency);
        };
        self.validate_compute_peer_v1(id)?;
        let Origin::Native(route) = origin else {
            let result = self.progress_deferred_compute_v1(dependency);
            if matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))) {
                return result;
            }
            self.validate_compute_peer_v1(id)?;
            return match result {
                Err(RuntimeBackendFailureV1::Rejected(error))
                    if error.kind() == KfdRuntimeBackendErrorKindV1::Busy =>
                {
                    Ok(BackendPollV1::Pending)
                }
                Err(RuntimeBackendFailureV1::Quiescent(error)) => {
                    let root = self
                        .deferred_compute_v1(dependency)
                        .expect("retained deferred producer");
                    if root.status == BackendPollV1::Pending {
                        Ok(BackendPollV1::Pending)
                    } else {
                        self.fail_cooperative_copy(id);
                        Err(RuntimeBackendFailureV1::Quiescent(error))
                    }
                }
                result => result,
            };
        };
        // Flush only while the exact target is still queued. Once it is active or
        // complete, another stream head is not this copy's publication authority.
        if self.children[route.child]
            .pending_compute
            .contains_key(&route.local)
        {
            if let Err(error) = self.service_native_peer_prefix_v1(route, true) {
                return self.attribute_compute_peer_progress_error_v1(id, route, error);
            }
            // Its retained peer prefix may own this child. Drive that prefix
            // above, but never enter child I/O until the peer restores both owners.
            if self.compute_xgmi_child_occupied_v1(route.child) {
                return Ok(BackendPollV1::Pending);
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
        if self.compute_xgmi_child_occupied_v1(route.child) {
            return Ok(BackendPollV1::Pending);
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
