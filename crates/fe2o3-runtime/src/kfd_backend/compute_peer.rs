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
pub(super) struct AllocationIdentity {
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
    pub(super) fn of(record: &AllocationRecordV1) -> Self {
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
    stream: u64,
    transfer_depth: usize,
    depth: usize,
    endpoint: (u64, RoutedHandleV1, AllocationIdentity),
    source: (u64, RoutedHandleV1, AllocationIdentity),
    plan: Arc<Gfx942ComputeXgmiSegmentsPlanV1>,
    origin: SegmentFrameOrigin,
    predecessor: Option<SegmentDestinationPredecessor>,
}

#[derive(Clone, Debug)]
pub(super) struct SegmentDestinationPredecessor {
    id: u64,
    frame: Arc<SegmentDestinationFrame>,
}

impl SegmentDestinationPredecessor {
    pub(super) fn orders_owner(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        route: RoutedHandleV1,
        owner: u64,
    ) -> bool {
        self.frame.orders_owner(backend, self.id, route, owner)
    }

    pub(super) fn owns_occupied_child(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        child: usize,
        owner: u64,
    ) -> bool {
        self.frame
            .owns_occupied_child(backend, self.id, child, owner)
    }
}

#[derive(Clone, Debug)]
enum SegmentFrameOrigin {
    Settled,
    Compute {
        id: u64,
        depth: usize,
    },
    Frame {
        id: u64,
        frame: Arc<SegmentDestinationFrame>,
    },
}

impl SegmentDestinationFrame {
    pub(super) fn poison_retained_children_v1(&self, backend: &mut KfdMultiDeviceRuntimeBackendV1) {
        // Failure containment uses the immutable held chain, never corrupted
        // occupancy markers. This does not authorize progress or restoration.
        let mut frame = self;
        for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            backend.poison_compute_xgmi_children_v1([frame.source.1.child, frame.endpoint.1.child]);
            if let SegmentFrameOrigin::Frame { frame: source, .. } = &frame.origin {
                if source.depth == 0 || source.depth >= frame.depth {
                    return;
                }
                frame = source;
                continue;
            }
            let Some(prior) = &frame.predecessor else {
                return;
            };
            if prior.frame.depth == 0 || prior.frame.depth >= frame.depth {
                return;
            }
            frame = &prior.frame;
        }
    }

    #[cfg(test)]
    pub(super) fn is_settled_source_for_test_v1(&self) -> bool {
        matches!(self.origin, SegmentFrameOrigin::Settled)
    }

    pub(super) fn depth(&self) -> usize {
        self.depth
    }

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
        let mut frame = self;
        let mut current = id;
        for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            if !frame.node_is_intact(backend, current) {
                return false;
            }
            let RoutedSubmissionV1::CooperativeCopy(copy) = &backend.submissions[&current] else {
                return false;
            };
            // A settled receipt no longer retains operational ancestor sources.
            if copy.is_quiescent() {
                return true;
            }
            // Frame-source nodes have no destination predecessor. Following
            // their one immutable edge keeps validation linear in chain depth.
            if let SegmentFrameOrigin::Frame {
                id: parent,
                frame: source,
            } = &frame.origin
            {
                if frame.predecessor.is_some()
                    || *parent >= current
                    || source.depth == 0
                    || source.depth >= frame.depth
                    || !copy.dependencies.contains(parent)
                    || !backend.submission_retained_as_dependency(*parent)
                {
                    return false;
                }
                current = *parent;
                frame = source;
                continue;
            }
            let Some(prior) = &frame.predecessor else {
                return true;
            };
            if prior.id >= current
                || prior.frame.depth == 0
                || prior.frame.depth >= frame.depth
                || prior.frame.stream != frame.stream
                || prior.frame.endpoint != frame.endpoint
                || !copy.dependencies.contains(&prior.id)
                || !backend.submission_retained_as_dependency(prior.id)
            {
                return false;
            }
            current = prior.id;
            frame = &prior.frame;
        }
        false
    }

    pub(super) fn orders_owner(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        id: u64,
        route: RoutedHandleV1,
        owner: u64,
    ) -> bool {
        if route != self.endpoint.1 || !self.is_intact(backend, id) {
            return false;
        }
        let mut current = id;
        let mut frame = self;
        for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            if current == owner {
                return true;
            }
            let Some(prior) = &frame.predecessor else {
                return false;
            };
            if prior.id >= current || prior.frame.depth >= frame.depth {
                return false;
            }
            current = prior.id;
            frame = &prior.frame;
        }
        false
    }

    pub(super) fn owns_occupied_child(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        id: u64,
        child: usize,
        owner: u64,
    ) -> bool {
        child == self.endpoint.1.child
            && self.orders_owner(backend, id, self.endpoint.1, owner)
            && matches!(backend.submissions.get(&owner),
            Some(RoutedSubmissionV1::CooperativeCopy(copy))
                if copy.status() == BackendPollV1::Pending
                    && copy.destination == self.endpoint.1
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

    pub(super) fn owns_occupied_ancestor_child(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        id: u64,
        child: usize,
        owner: u64,
    ) -> bool {
        if !self.is_intact(backend, id) {
            return false;
        }
        // Resource occupancy may follow source ancestry. It never establishes
        // writer ordering for the current destination allocation.
        let mut frame = self;
        let mut current = id;
        for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = backend.submissions.get(&current)
            else {
                return false;
            };
            if copy.is_quiescent() {
                return false;
            }
            if current == owner {
                return copy.status() == BackendPollV1::Pending
                    && [frame.source.1.child, frame.endpoint.1.child].contains(&child)
                    && copy.compute_xgmi.as_ref().is_some_and(|root| !root.is_quiescent())
                    && [frame.source.1, frame.endpoint.1].iter().all(|endpoint| {
                        backend.compute_xgmi_children.get(endpoint.child) == Some(&Some(owner))
                            && backend.children.get(endpoint.child)
                                .and_then(|child| child.allocations.get(&endpoint.local))
                                .is_some_and(|record| matches!(record.sdma_storage,
                                    KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(id))
                                        if id == owner))
                    });
            }
            let (parent, ancestor) = match &frame.origin {
                SegmentFrameOrigin::Frame { id, frame } => (*id, frame.as_ref()),
                _ => match &frame.predecessor {
                    Some(prior) => (prior.id, prior.frame.as_ref()),
                    None => return false,
                },
            };
            if parent >= current || ancestor.depth == 0 || ancestor.depth >= frame.depth {
                return false;
            }
            current = parent;
            frame = ancestor;
        }
        false
    }

    fn node_is_intact(&self, backend: &KfdMultiDeviceRuntimeBackendV1, id: u64) -> bool {
        let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = backend.submissions.get(&id) else {
            return false;
        };
        if copy.stream != self.stream
            || copy.dependency_depth != self.transfer_depth
            || self.transfer_depth == 0
            || self.transfer_depth > self.depth
            || self.depth > MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1
            || copy.directed.is_some()
            || copy.source != self.source.1
            || copy.source_region.allocation != self.source.0
            || copy.destination != self.endpoint.1
            || copy.destination_region.allocation != self.endpoint.0
            || !self.matches_regions(copy.source_region, copy.destination_region)
            || !copy.compute_xgmi.as_ref().is_some_and(|root| {
                root.matches_segments(&self.plan)
                    && root
                        .segment_frame_v1()
                        .is_some_and(|frame| std::ptr::eq(frame.as_ref(), self))
            })
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
                && copy.frame_source.is_none()
                && copy
                    .compute_xgmi
                    .as_ref()
                    .is_some_and(|root| root.is_quiescent());
        }
        if !copy.dependencies.iter().all(|dependency| {
            *dependency < id
                && backend.submission_retained_as_dependency(*dependency)
                && backend
                    .segment_frame_dependency_depth_v1(*dependency)
                    .is_some_and(|depth| depth > 0 && depth < self.depth)
        }) {
            return false;
        }
        match &self.origin {
            SegmentFrameOrigin::Settled => {
                copy.compute_producer.is_none()
                    && copy.frame_source.is_none()
                    && backend.allocations.get(&self.source.0) == Some(&self.source.1)
                    && backend.children[self.source.1.child]
                        .allocations
                        .get(&self.source.1.local)
                        .is_some_and(|record| {
                            AllocationIdentity::of(record) == self.source.2
                                && record.sdma_initialized
                        })
            }
            SegmentFrameOrigin::Compute {
                id: producer_id,
                depth,
            } => {
                copy.frame_source.is_none()
                    && copy.compute_producer.as_ref().is_some_and(|producer| {
                        producer.endpoints == [self.source, self.endpoint]
                            && (producer.id, producer.depth) == (*producer_id, *depth)
                            && producer.predecessor.is_none()
                            && producer
                                .segments()
                                .is_some_and(|plan| Arc::ptr_eq(plan, &self.plan))
                    })
                    && copy.compute_producer.as_ref().is_some_and(|producer| {
                        backend.compute_peer_producer_intact_v1(id, copy, producer)
                    })
            }
            SegmentFrameOrigin::Frame { id: parent, frame } => {
                self.predecessor.is_none()
                    && copy.compute_producer.is_none()
                    && copy.frame_source.as_ref().is_some_and(|source| {
                        source.matches_segment_frame(
                            (*parent, frame),
                            [self.source, self.endpoint],
                            &self.plan,
                        ) && source.local_is_intact(backend, id, copy)
                    })
            }
        }
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

pub(super) fn segment_regions_match(
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
pub(super) enum TransferIdentity {
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

mod admission;
