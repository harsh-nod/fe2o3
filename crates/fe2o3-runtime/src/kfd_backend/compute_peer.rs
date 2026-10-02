//! Exact pending compute writers retained by the existing cooperative copy owner.

use super::*;
use fe2o3_kfd::{Gfx942ComputeXgmiCopyWindowV1, Gfx942ComputeXgmiSegmentsPlanV1};

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

pub(super) fn exact_full_writer(
    bindings: &[BackendBindingV1],
    source: u64,
    record: &AllocationRecordV1,
) -> bool {
    let mut aliases = bindings
        .iter()
        .filter(|binding| binding.region.allocation == source);
    aliases.next().is_some_and(|binding| {
        binding.region.access == RuntimeAccessV1::Write
            && compute_xgmi::full_extent(record, binding.region)
    }) && aliases.next().is_none()
}

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DestinationPredecessor {
    id: u64,
    stream: u64,
    depth: usize,
    source: RoutedHandleV1,
    source_region: BackendMemoryRegionV1,
    destination: RoutedHandleV1,
    destination_region: BackendMemoryRegionV1,
    window: Gfx942ComputeXgmiCopyWindowV1,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct DestinationFrame {
    endpoint: (u64, RoutedHandleV1, AllocationIdentity),
    window: Gfx942ComputeXgmiCopyWindowV1,
    producer: (u64, usize),
    predecessor: Option<DestinationPredecessor>,
}

impl DestinationFrame {
    pub(super) fn covers(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        region: BackendMemoryRegionV1,
    ) -> bool {
        let (global, route, identity) = self.endpoint;
        backend.allocations.get(&global) == Some(&route)
            && region.allocation == global
            && region.access == RuntimeAccessV1::Read
            && backend
                .children
                .get(route.child)
                .and_then(|child| child.allocations.get(&route.local))
                .is_some_and(|record| {
                    AllocationIdentity::of(record) == identity
                        && record.sdma_initialized
                        && compute_xgmi::checked_region(record, region)
                })
    }

    pub(super) fn is_intact(&self, backend: &KfdMultiDeviceRuntimeBackendV1, id: u64) -> bool {
        let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = backend.submissions.get(&id) else {
            return false;
        };
        if copy.directed.is_some()
            || copy.destination != self.endpoint.1
            || copy.destination_region.allocation != self.endpoint.0
            || !self.covers(
                backend,
                BackendMemoryRegionV1 {
                    allocation: self.endpoint.0,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: self.endpoint.2.bytes as u64,
                },
            )
            || copy
                .compute_xgmi
                .as_ref()
                .is_none_or(|root| !root.matches_window(self.window))
        {
            return false;
        }
        if copy.is_quiescent() {
            // Settlement clears the operational producer and may release its
            // ancestors and their source allocations. Keep only exact frame identity.
            return copy.compute_producer.is_none()
                && copy
                    .compute_xgmi
                    .as_ref()
                    .is_some_and(|root| root.is_quiescent());
        }
        copy.compute_producer.as_ref().is_some_and(|producer| {
            producer.endpoints[1] == self.endpoint
                && producer.segments().is_none()
                && producer.window() == self.window
                && (producer.id, producer.depth) == self.producer
                && producer.predecessor == self.predecessor
        }) && backend.compute_peer_chain_intact_v1(id).is_ok()
    }

    pub(super) fn orders_owner(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        id: u64,
        route: RoutedHandleV1,
        owner: u64,
    ) -> bool {
        route == self.endpoint.1
            && self.is_intact(backend, id)
            && matches!(backend.submissions.get(&id),
            Some(RoutedSubmissionV1::CooperativeCopy(copy))
                if copy.compute_producer.as_ref().is_some_and(|producer| {
                    producer.orders_destination_owner(backend, owner)
                    }))
    }

    pub(super) fn owns_occupied_child(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        id: u64,
        child: usize,
        owner: u64,
    ) -> bool {
        if self.endpoint.1.child != child || !self.orders_owner(backend, id, self.endpoint.1, owner)
        {
            return false;
        }
        matches!(backend.submissions.get(&owner),
        Some(RoutedSubmissionV1::CooperativeCopy(copy))
            if copy.status() == BackendPollV1::Pending
                && copy.compute_xgmi.as_ref().is_some_and(|root| !root.is_quiescent())
                && [copy.source, copy.destination].iter().all(|endpoint| {
                    backend.compute_xgmi_children.get(endpoint.child) == Some(&Some(owner))
                        && backend.children.get(endpoint.child)
                            .and_then(|child| child.allocations.get(&endpoint.local))
                            .is_some_and(|record| matches!(record.sdma_storage,
                                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(id))
                                    if id == owner))
                }))
    }
}

impl DestinationPredecessor {
    fn matches(&self, copy: &CooperativeCopySubmissionV1) -> bool {
        copy.stream == self.stream
            && copy.dependency_depth == self.depth
            && copy.source == self.source
            && copy.source_region == self.source_region
            && copy.destination == self.destination
            && copy.destination_region == self.destination_region
            && copy.directed.is_none()
            && copy.sdma_leaf.is_none()
            && copy.compute_xgmi.as_ref().is_some_and(|root| {
                root.matches_window(self.window) && (!copy.is_quiescent() || root.is_quiescent())
            })
    }
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

#[derive(Clone, Debug)]
pub(super) struct SegmentDestinationFrame {
    endpoint: (u64, RoutedHandleV1, AllocationIdentity),
    source: (u64, RoutedHandleV1, AllocationIdentity),
    plan: Arc<Gfx942ComputeXgmiSegmentsPlanV1>,
    producer: (u64, usize),
}

impl SegmentDestinationFrame {
    pub(super) fn covers(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        region: BackendMemoryRegionV1,
    ) -> bool {
        let (global, route, identity) = self.endpoint;
        backend.allocations.get(&global) == Some(&route)
            && region.allocation == global
            && region.access == RuntimeAccessV1::Read
            && backend.children[route.child]
                .allocations
                .get(&route.local)
                .is_some_and(|record| {
                    AllocationIdentity::of(record) == identity
                        && record.sdma_initialized
                        && compute_xgmi::checked_envelope(record, region)
                })
    }

    pub(super) fn is_intact(&self, backend: &KfdMultiDeviceRuntimeBackendV1, id: u64) -> bool {
        let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = backend.submissions.get(&id) else {
            return false;
        };
        if copy.directed.is_some()
            || copy.source != self.source.1
            || copy.source_region.allocation != self.source.0
            || copy.destination != self.endpoint.1
            || copy.destination_region.allocation != self.endpoint.0
            || !self.matches_regions(copy.source_region, copy.destination_region)
            || !copy
                .compute_xgmi
                .as_ref()
                .is_some_and(|root| root.matches_segments(&self.plan))
            || !self.covers(
                backend,
                BackendMemoryRegionV1 {
                    allocation: self.endpoint.0,
                    byte_offset: 0,
                    byte_len: self.endpoint.2.bytes as u64,
                    access: RuntimeAccessV1::Read,
                },
            )
        {
            return false;
        }
        // A final restored receipt outlives its released source and compute
        // result. It proves frame identity, not successful completion.
        if copy.is_quiescent() {
            return copy.compute_producer.is_none()
                && copy
                    .compute_xgmi
                    .as_ref()
                    .is_some_and(|root| root.is_quiescent());
        }
        copy.compute_producer.as_ref().is_some_and(|producer| {
            producer.endpoints == [self.source, self.endpoint]
                && (producer.id, producer.depth) == self.producer
                && producer.predecessor.is_none()
                && producer
                    .segments()
                    .is_some_and(|plan| Arc::ptr_eq(plan, &self.plan))
        }) && backend.compute_peer_chain_intact_v1(id).is_ok()
    }

    fn matches_regions(
        &self,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
    ) -> bool {
        segment_regions_match(
            &self.plan,
            self.source.2,
            self.endpoint.2,
            source,
            destination,
        )
    }
}

fn segment_regions_match(
    plan: &Gfx942ComputeXgmiSegmentsPlanV1,
    source_identity: AllocationIdentity,
    destination_identity: AllocationIdentity,
    source: BackendMemoryRegionV1,
    destination: BackendMemoryRegionV1,
) -> bool {
    source.access == RuntimeAccessV1::Read
        && destination.access == RuntimeAccessV1::Write
        && plan.source_logical_bytes() == source_identity.bytes as u64
        && plan.destination_logical_bytes() == destination_identity.bytes as u64
        && plan.source_offset() == source.byte_offset
        && plan.source_len() == source.byte_len
        && plan.destination_offset() == destination.byte_offset
        && plan.destination_len() == destination.byte_len
}

#[derive(Clone, Debug)]
enum TransferIdentity {
    Window(Gfx942ComputeXgmiCopyWindowV1),
    Segments(Arc<Gfx942ComputeXgmiSegmentsPlanV1>),
}

#[derive(Debug)]
pub(super) struct Producer {
    pub(super) id: u64,
    origin: Origin,
    stream: u64,
    depth: usize,
    launch: Arc<RetainedComputeLaunchV1>,
    endpoints: [(u64, RoutedHandleV1, AllocationIdentity); 2],
    transfer: TransferIdentity,
    predecessor: Option<DestinationPredecessor>,
}

impl Producer {
    pub(super) fn depth(&self) -> usize {
        self.depth
    }

    pub(super) fn window(&self) -> Gfx942ComputeXgmiCopyWindowV1 {
        match self.transfer {
            TransferIdentity::Window(window) => window,
            TransferIdentity::Segments(_) => unreachable!("scalar profile requires a window"),
        }
    }

    pub(super) fn segments(&self) -> Option<&Arc<Gfx942ComputeXgmiSegmentsPlanV1>> {
        match &self.transfer {
            TransferIdentity::Window(_) => None,
            TransferIdentity::Segments(plan) => Some(plan),
        }
    }

    fn matches_root(&self, root: &compute_xgmi::Root) -> bool {
        match &self.transfer {
            TransferIdentity::Window(window) => root.matches_window(*window),
            TransferIdentity::Segments(plan) => root.matches_segments(plan),
        }
    }

    pub(super) fn destination_frame(&self) -> DestinationFrame {
        DestinationFrame {
            endpoint: self.endpoints[1],
            window: self.window(),
            producer: (self.id, self.depth),
            predecessor: self.predecessor,
        }
    }

    pub(super) fn orders_on_stream(&self, stream: u64) -> bool {
        self.predecessor.is_none_or(|prior| prior.stream == stream)
    }

    pub(super) fn orders_destination_owner(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        owner: u64,
    ) -> bool {
        self.predecessor_orders_destination_owner(backend, self.predecessor, owner)
    }

    fn predecessor_orders_destination_owner(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        mut predecessor: Option<DestinationPredecessor>,
        owner: u64,
    ) -> bool {
        let mut depth = MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 + 1;
        let mut newer = backend.next_handle;
        for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            let Some(prior) = predecessor else {
                return false;
            };
            if prior.id >= newer || prior.depth == 0 || prior.depth >= depth {
                return false;
            }
            let Some(RoutedSubmissionV1::CooperativeCopy(copy)) =
                backend.submissions.get(&prior.id)
            else {
                return false;
            };
            if !prior.matches(copy)
                || prior.destination != self.endpoints[1].1
                || prior.destination_region.allocation != self.endpoints[1].0
            {
                return false;
            }
            if prior.id == owner {
                return true;
            }
            newer = prior.id;
            depth = prior.depth;
            predecessor = copy
                .compute_producer
                .as_ref()
                .and_then(|producer| producer.predecessor);
        }
        false
    }

    fn matches_regions(
        &self,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
    ) -> bool {
        if let Some(plan) = self.segments() {
            return segment_regions_match(
                plan,
                self.endpoints[0].2,
                self.endpoints[1].2,
                source,
                destination,
            );
        }
        source.access == RuntimeAccessV1::Read
            && destination.access == RuntimeAccessV1::Write
            && source.byte_len == destination.byte_len
            && Gfx942ComputeXgmiCopyWindowV1::new(
                self.endpoints[0].2.bytes as u64,
                self.endpoints[1].2.bytes as u64,
                source.byte_offset,
                destination.byte_offset,
                source.byte_len,
            ) == Some(self.window())
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

    fn completed_source_is_restored(&self, backend: &KfdMultiDeviceRuntimeBackendV1) -> bool {
        let source = self.endpoints[0].1;
        let child = &backend.children[source.child];
        let Some(record) = child.allocations.get(&source.local) else {
            return false;
        };
        let completed = match self.origin {
            Origin::Native(route) => {
                child
                    .submissions
                    .get(&route.local)
                    .is_some_and(|record| record.status == BackendPollV1::Succeeded)
                    && child.exact_submission_quiescent_v1(route.local)
            }
            Origin::Deferred { .. } => backend
                .deferred_compute_v1(self.id)
                .is_some_and(|root| root.status == BackendPollV1::Succeeded),
        };
        completed
            && !child.allocation_is_active(source.local)
            && !backend.allocation_retained_by_deferred_compute_v1(source)
            && record.sdma_initialized
            && record.persistent_storage_restore.is_none()
            && matches!(
                record.sdma_storage,
                KfdRuntimeSdmaStorageV1::Device(_)
                    | KfdRuntimeSdmaStorageV1::H2dReady(_)
                    | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
                    | KfdRuntimeSdmaStorageV1::InitializedStorage(_)
            )
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
        segments: Option<&Arc<Gfx942ComputeXgmiSegmentsPlanV1>>,
    ) -> Result<Option<Producer>, Failure> {
        let child = &self.children[source.child];
        let target = &self.children[destination.child];
        let source_record = &child.allocations[&source.local];
        let destination_record = &target.allocations[&destination.local];
        if source.child == destination.child
            || !self
                .compute_xgmi_routes
                .contains_key(&(source.child, destination.child))
            || !child.peer_visible_device_allocations
            || !target.peer_visible_device_allocations
            || source_region.access != RuntimeAccessV1::Read
            || destination_region.access != RuntimeAccessV1::Write
            || segments.is_none() && source_region.byte_len != destination_region.byte_len
            || !compute_xgmi::checked_envelope(source_record, source_region)
            || !compute_xgmi::checked_envelope(destination_record, destination_region)
            || !destination_record.sdma_initialized
            || !matches!(
                destination_record.sdma_storage,
                KfdRuntimeSdmaStorageV1::Device(_)
                    | KfdRuntimeSdmaStorageV1::H2dReady(_)
                    | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
                    | KfdRuntimeSdmaStorageV1::InitializedStorage(_)
            )
        {
            return Ok(None);
        }
        let transfer = if let Some(plan) = segments {
            if !segment_regions_match(
                plan,
                AllocationIdentity::of(source_record),
                AllocationIdentity::of(destination_record),
                source_region,
                destination_region,
            ) {
                return Ok(None);
            }
            TransferIdentity::Segments(Arc::clone(plan))
        } else {
            let Some(window) = Gfx942ComputeXgmiCopyWindowV1::new(
                source_record.bytes.len() as u64,
                destination_record.bytes.len() as u64,
                source_region.byte_offset,
                destination_region.byte_offset,
                source_region.byte_len,
            ) else {
                return Ok(None);
            };
            TransferIdentity::Window(window)
        };
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
            let Some((stream, depth, launch)) = self
                .pending_deferred_peer_producer_v1(*submission, source.child)
                .or_else(|| {
                    segments.and_then(|_| {
                        self.completed_deferred_segment_producer_v1(*submission, source.child)
                    })
                })
            else {
                continue;
            };
            if launch.semantic_launch != BackendSemanticLaunchV1::Ordinary
                || !exact_full_writer(&launch.bindings, source.local, source_record)
            {
                continue;
            }
            let mut producer = Producer {
                id: *submission,
                origin: Origin::Deferred {
                    child: source.child,
                },
                stream,
                depth,
                launch,
                transfer: transfer.clone(),
                predecessor: None,
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
            if !producer.intact(self)
                || !(producer.owns_source(self, source)
                    || segments.is_some() && producer.completed_source_is_restored(self))
            {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "deferred compute peer producer does not match exact source custody",
                ));
            }
            producer.predecessor = self.prepare_compute_peer_controls_v1(
                &producer,
                events,
                source.child,
                destination.child,
            )?;
            return Ok(Some(producer));
        }
        let owner = child
            .allocation_custody
            .get(&source.local)
            .and_then(|custody| {
                custody.owners.front().filter(|owner| {
                    custody.owners.len() == 1
                        && owner.kind == RuntimeAllocationCustodyKindV1::Compute
                })
            });
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
            let owns_source = owner.is_some_and(|owner| {
                owner.submission == route.local && owner.stream == launch.stream
            });
            let completed = segments.is_some()
                && child
                    .submissions
                    .get(&route.local)
                    .is_some_and(|record| record.status == BackendPollV1::Succeeded)
                && child.exact_submission_quiescent_v1(route.local);
            if launch.semantic_launch != BackendSemanticLaunchV1::Ordinary
                || !(owns_source || completed)
                || !exact_full_writer(&launch.bindings, source.local, source_record)
            {
                continue;
            }
            let Some((_, depth)) = compute_identity(child, route.local) else {
                continue;
            };
            if !child.pending_compute.contains_key(&route.local)
                && child.active_compute_submission_v1(route.local).is_none()
                && !completed
            {
                continue;
            }
            let mut producer = Producer {
                id: *submission,
                origin: Origin::Native(*route),
                stream: *stream,
                depth,
                launch: Arc::clone(launch),
                transfer: transfer.clone(),
                predecessor: None,
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
            if !producer.intact(self)
                || !(producer.owns_source(self, source)
                    || completed && producer.completed_source_is_restored(self))
            {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "compute peer producer identity does not match retained custody",
                ));
            }
            producer.predecessor = self.prepare_compute_peer_controls_v1(
                &producer,
                events,
                source.child,
                destination.child,
            )?;
            return Ok(Some(producer));
        }
        Ok(None)
    }

    fn prepare_compute_peer_controls_v1(
        &self,
        producer: &Producer,
        events: &[u64],
        source: usize,
        destination: usize,
    ) -> Result<Option<DestinationPredecessor>, Failure> {
        let mut predecessor = None;
        for event in events {
            if producer.deferred_event(self, *event) {
                continue;
            }
            let id = self.peer_dependency_submission(*event, source, destination)?;
            if id == producer.id || self.compute_peer_dependency_succeeded_v1(id) {
                continue;
            }
            let candidate = match self.submissions.get(&id) {
                Some(RoutedSubmissionV1::CooperativeCopy(copy))
                    if producer.segments().is_none()
                        && copy.status() == BackendPollV1::Pending
                        && !copy.is_quiescent()
                        && copy.destination == producer.endpoints[1].1
                        && copy.destination_region.allocation == producer.endpoints[1].0
                        && copy.directed.is_none()
                        && self.compute_peer_chain_intact_v1(id).is_ok() =>
                {
                    copy.compute_producer
                        .as_ref()
                        .filter(|prior| prior.segments().is_none())
                        .map(|prior| DestinationPredecessor {
                            id,
                            stream: copy.stream,
                            depth: copy.dependency_depth,
                            source: copy.source,
                            source_region: copy.source_region,
                            destination: copy.destination,
                            destination_region: copy.destination_region,
                            window: prior.window(),
                        })
                }
                _ => None,
            };
            if candidate.is_none() || predecessor.is_some() {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "pending compute peer requires one writer, at most one exact destination predecessor, and successful other controls",
                ));
            }
            predecessor = candidate;
        }
        if let Some(owners) = self
            .cooperative_allocation_owners
            .get(&producer.endpoints[1].1)
        {
            // A same-stream edge must not silently replace the explicit latest
            // writer event required by the ordered whole-destination contract.
            let latest = owners.iter().copied().max();
            if predecessor.map(|prior| prior.id) != latest
                || !owners.iter().all(|owner| {
                    producer.predecessor_orders_destination_owner(self, predecessor, *owner)
                })
            {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "ordered compute peer requires the exact latest destination writer",
                ));
            }
        }
        Ok(predecessor)
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

    pub(super) fn compute_peer_destination_frame_v1(
        &self,
        id: u64,
        source: RoutedHandleV1,
        region: BackendMemoryRegionV1,
    ) -> Option<&Producer> {
        let RoutedSubmissionV1::CooperativeCopy(copy) = self.submissions.get(&id)? else {
            return None;
        };
        let producer = copy.compute_producer.as_ref()?;
        if producer.segments().is_some() {
            return None;
        }
        let (global, route, identity) = producer.endpoints[1];
        let record = self
            .children
            .get(source.child)?
            .allocations
            .get(&source.local)?;
        // Each admitted native writer preserves the initialized complete owner,
        // including bytes outside its window. This is not a union-of-ranges claim.
        (copy.status() == BackendPollV1::Pending
            && self.compute_peer_chain_intact_v1(id).is_ok()
            && self
                .cooperative_allocation_owners
                .get(&source)
                .and_then(|owners| owners.iter().max())
                .copied()
                == Some(id)
            && source == route
            && region.allocation == global
            && region.access == RuntimeAccessV1::Read
            && AllocationIdentity::of(record) == identity
            && record.sdma_initialized
            && compute_xgmi::checked_region(record, region))
        .then_some(producer)
    }

    pub(super) fn compute_peer_readback_frame_v1(
        &self,
        source: RoutedHandleV1,
        source_region: BackendMemoryRegionV1,
        destination: RoutedHandleV1,
        destination_region: BackendMemoryRegionV1,
        dependencies: &[u64],
    ) -> Option<&Producer> {
        let record = self
            .children
            .get(destination.child)?
            .allocations
            .get(&destination.local)?;
        if source.child != destination.child
            || source == destination
            || destination_region.access != RuntimeAccessV1::Write
            || source_region.byte_len != destination_region.byte_len
            || record.kind != RuntimeMemoryKindV1::HostVisible
            || self.allocations.get(&destination_region.allocation) != Some(&destination)
            || destination_region
                .byte_offset
                .checked_add(destination_region.byte_len)?
                > record.bytes.len() as u64
        {
            return None;
        }
        dependencies
            .iter()
            .find_map(|id| self.compute_peer_destination_frame_v1(*id, source, source_region))
    }

    pub(super) fn compute_peer_segment_frame_v1(
        &self,
        id: u64,
        destination: RoutedHandleV1,
    ) -> Option<SegmentDestinationFrame> {
        let RoutedSubmissionV1::CooperativeCopy(copy) = self.submissions.get(&id)? else {
            return None;
        };
        let producer = copy.compute_producer.as_ref()?;
        let plan = producer.segments()?;
        if copy.status() != BackendPollV1::Pending
            || producer.predecessor.is_some()
            || producer.endpoints[1].1 != destination
            || self
                .cooperative_allocation_owners
                .get(&destination)
                .is_none_or(|owners| owners.len() != 1 || owners[0] != id)
        {
            return None;
        }
        let frame = SegmentDestinationFrame {
            source: producer.endpoints[0],
            endpoint: producer.endpoints[1],
            plan: Arc::clone(plan),
            producer: (producer.id, producer.depth),
        };
        frame.is_intact(self, id).then_some(frame)
    }

    fn compute_peer_chain_intact_v1(&self, id: u64) -> Result<(), u64> {
        let mut current = id;
        for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&current)
            else {
                return Err(current);
            };
            let Some(producer) = &copy.compute_producer else {
                return Ok(());
            };
            let intact = producer.id < current
                && producer.depth > 0
                && producer.depth < copy.dependency_depth
                && copy.dependency_depth <= MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1
                && producer.intact(self)
                && producer.matches_regions(copy.source_region, copy.destination_region)
                && copy
                    .compute_xgmi
                    .as_ref()
                    .is_some_and(|root| producer.matches_root(root))
                && copy.directed.is_none()
                && copy.source == producer.endpoints[0].1
                && copy.source_region.allocation == producer.endpoints[0].0
                && copy.destination == producer.endpoints[1].1
                && copy.destination_region.allocation == producer.endpoints[1].0
                && copy.dependencies.contains(&producer.id)
                && self.submission_retained_as_dependency(producer.id);
            if !intact {
                return Err(current);
            }
            let Some(prior) = producer.predecessor else {
                return Ok(());
            };
            if producer.segments().is_some()
                || prior.id >= current
                || prior.depth == 0
                || prior.depth >= copy.dependency_depth
                || prior.stream != copy.stream
                || prior.destination != copy.destination
                || prior.destination_region.allocation != copy.destination_region.allocation
                || !copy.dependencies.contains(&prior.id)
                || !self.submission_retained_as_dependency(prior.id)
            {
                return Err(current);
            }
            let Some(RoutedSubmissionV1::CooperativeCopy(previous)) =
                self.submissions.get(&prior.id)
            else {
                return Err(current);
            };
            if !prior.matches(previous) {
                return Err(current);
            }
            if previous.is_quiescent() {
                return Ok(());
            }
            if previous.compute_producer.is_none() {
                return Err(prior.id);
            }
            current = prior.id;
        }
        Err(current)
    }

    pub(super) fn validate_compute_peer_v1(&mut self, id: u64) -> Result<(), Failure> {
        if let Err(invalid) = self.compute_peer_chain_intact_v1(id) {
            self.terminal = true;
            for affected in [id, invalid] {
                if let Some(endpoints) = self.compute_xgmi_endpoints_v1(affected) {
                    self.poison_compute_xgmi_children_v1(endpoints);
                }
            }
            Err(RuntimeBackendFailureV1::Terminal(
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Terminal,
                    "pending compute peer lost its exact producer or allocation roots",
                ),
            ))
        } else {
            Ok(())
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
