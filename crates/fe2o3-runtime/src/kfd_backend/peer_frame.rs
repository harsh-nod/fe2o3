//! Native windows/lists whose input is an exact retained segmented frame.

use super::*;
use compute_peer::{AllocationIdentity, SegmentDestinationFrame, TransferIdentity};
use fe2o3_kfd::{Gfx942ComputeXgmiCopyWindowV1, Gfx942ComputeXgmiSegmentsPlanV1};

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

#[derive(Debug)]
pub(super) struct Source {
    id: u64,
    frame: Arc<SegmentDestinationFrame>,
    endpoints: [(RoutedHandleV1, BackendMemoryRegionV1, AllocationIdentity); 2],
    transfer: TransferIdentity,
}

impl Source {
    pub(super) fn retained_poison_roots_v1(&self) -> ([usize; 2], Arc<SegmentDestinationFrame>) {
        (
            self.endpoints.map(|(route, _, _)| route.child),
            Arc::clone(&self.frame),
        )
    }

    pub(super) fn id(&self) -> u64 {
        self.id
    }

    pub(super) fn depth(&self) -> usize {
        self.frame.depth()
    }

    pub(super) fn window(&self) -> Gfx942ComputeXgmiCopyWindowV1 {
        match self.transfer {
            TransferIdentity::Window(window) => window,
            TransferIdentity::Segments(_) => unreachable!("scalar frame profile requires a window"),
        }
    }

    pub(super) fn segments(&self) -> Option<&Arc<Gfx942ComputeXgmiSegmentsPlanV1>> {
        match &self.transfer {
            TransferIdentity::Window(_) => None,
            TransferIdentity::Segments(plan) => Some(plan),
        }
    }

    pub(super) fn ancestor(&self) -> (u64, Arc<SegmentDestinationFrame>) {
        (self.id, Arc::clone(&self.frame))
    }

    pub(super) fn matches_segment_endpoints(
        &self,
        endpoints: [(RoutedHandleV1, BackendMemoryRegionV1); 2],
        plan: &Arc<Gfx942ComputeXgmiSegmentsPlanV1>,
    ) -> bool {
        self.endpoints.map(|(route, region, _)| (route, region)) == endpoints
            && self.segments().is_some_and(|held| Arc::ptr_eq(held, plan))
    }

    pub(super) fn matches_segment_frame(
        &self,
        ancestor: (u64, &Arc<SegmentDestinationFrame>),
        endpoints: [(u64, RoutedHandleV1, AllocationIdentity); 2],
        plan: &Arc<Gfx942ComputeXgmiSegmentsPlanV1>,
    ) -> bool {
        self.id == ancestor.0
            && Arc::ptr_eq(&self.frame, ancestor.1)
            && self
                .endpoints
                .map(|(route, region, identity)| (region.allocation, route, identity))
                == endpoints
            && self.segments().is_some_and(|held| Arc::ptr_eq(held, plan))
    }

    pub(super) fn orders_owner(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        route: RoutedHandleV1,
        owner: u64,
    ) -> bool {
        route == self.endpoints[0].0 && self.frame.orders_owner(backend, self.id, route, owner)
    }

    pub(super) fn controls_succeeded(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        dependencies: &[u64],
    ) -> bool {
        dependencies
            .iter()
            .all(|id| *id == self.id || backend.compute_peer_dependency_succeeded_v1(*id))
    }

    fn source_restorable(&self, backend: &KfdMultiDeviceRuntimeBackendV1) -> bool {
        let route = self.endpoints[0].0;
        let record = &backend.children[route.child].allocations[&route.local];
        let occupied = |owner| {
            self.frame
                .owns_occupied_child(backend, self.id, route.child, owner)
        };
        if backend.compute_xgmi_children[route.child].is_some_and(|owner| {
            !self
                .frame
                .owns_occupied_ancestor_child(backend, self.id, route.child, owner)
        }) {
            return false;
        }
        if backend
            .cooperative_allocation_owners
            .get(&route)
            .is_some_and(|owners| {
                owners.iter().any(|owner| {
                    matches!(backend.submissions.get(owner),
                    Some(RoutedSubmissionV1::CooperativeCopy(copy))
                        if *owner <= self.id && copy.compute_xgmi.as_ref()
                            .is_some_and(|root| !root.is_quiescent()) && !occupied(*owner))
                })
            })
        {
            return false;
        }
        match record.sdma_storage {
            KfdRuntimeSdmaStorageV1::Device(_)
            | KfdRuntimeSdmaStorageV1::H2dReady(_)
            | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
            | KfdRuntimeSdmaStorageV1::InitializedStorage(_) => true,
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(owner)) => {
                occupied(owner)
            }
            _ => false,
        }
    }

    pub(super) fn is_intact(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        id: u64,
        copy: &CooperativeCopySubmissionV1,
    ) -> bool {
        self.local_is_intact(backend, id, copy)
            && self.frame.is_intact(backend, self.id)
            && (copy.phase != CooperativeCopyPhaseV1::Dependencies
                || self.source_restorable(backend))
    }

    pub(super) fn local_is_intact(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
        id: u64,
        copy: &CooperativeCopySubmissionV1,
    ) -> bool {
        // Check the rank and older ID before the bounded parent frame walk.
        if self.id >= id
            || self.depth() == 0
            || self.depth() >= copy.dependency_depth
            || copy.dependency_depth > MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1
            || copy.directed.is_some()
            || copy.compute_producer.is_some()
            || !copy.dependencies.contains(&self.id)
            || !backend.submission_retained_as_dependency(self.id)
            || !copy
                .compute_xgmi
                .as_ref()
                .is_some_and(|root| match &self.transfer {
                    TransferIdentity::Window(window) => root.matches_window(*window),
                    TransferIdentity::Segments(plan) => root.matches_segments(plan),
                })
            || [
                (copy.source, copy.source_region),
                (copy.destination, copy.destination_region),
            ] != self.endpoints.map(|(route, region, _)| (route, region))
            || !self.endpoints.iter().all(|(route, region, identity)| {
                backend.allocations.get(&region.allocation) == Some(route)
                    && backend
                        .children
                        .get(route.child)
                        .and_then(|child| child.allocations.get(&route.local))
                        .is_some_and(|record| {
                            AllocationIdentity::of(record) == *identity && record.sdma_initialized
                        })
            })
        {
            return false;
        }
        let Some(RoutedSubmissionV1::CooperativeCopy(parent)) = backend.submissions.get(&self.id)
        else {
            return false;
        };
        if !parent
            .compute_xgmi
            .as_ref()
            .and_then(|root| root.segment_frame_v1())
            .is_some_and(|frame| Arc::ptr_eq(frame, &self.frame))
            || !self.frame.covers(backend, self.endpoints[0].1)
        {
            return false;
        }
        // Failure is a dependency disposition, never successful frame readiness.
        // Native owner extraction may begin only after success AND restoration.
        if copy.phase != CooperativeCopyPhaseV1::Dependencies {
            return parent.status() == BackendPollV1::Succeeded && parent.is_quiescent();
        }
        true
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn prepare_peer_frame_source_v1(
        &self,
        endpoints: [(RoutedHandleV1, BackendMemoryRegionV1); 2],
        events: &[u64],
        segments: Option<&Arc<Gfx942ComputeXgmiSegmentsPlanV1>>,
    ) -> Result<Option<Source>, Failure> {
        let [(source, source_region), (destination, destination_region)] = endpoints;
        if source.child == destination.child {
            return Ok(None);
        }
        let selected = events.iter().find_map(|event| {
            let RoutedEventV1::CooperativeCopy { submission, child } = self.events.get(event)?
            else {
                return None;
            };
            let RoutedSubmissionV1::CooperativeCopy(parent) = self.submissions.get(submission)?
            else {
                return None;
            };
            (*child == source.child
                && parent.destination == source
                && !(segments.is_some()
                    && parent.status() == BackendPollV1::Succeeded
                    && parent.is_quiescent())
                && parent
                    .compute_xgmi
                    .as_ref()
                    .is_some_and(|root| root.is_segmented()))
            .then_some(*submission)
        });
        let Some(id) = selected else {
            return Ok(None);
        };
        let reject = || {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "native frame peer requires an exact current frame, checked transfer and fresh initialized destination",
            )
        };
        let RoutedSubmissionV1::CooperativeCopy(parent) = &self.submissions[&id] else {
            unreachable!()
        };
        let frame = Arc::clone(
            parent
                .compute_xgmi
                .as_ref()
                .and_then(|root| root.segment_frame_v1())
                .ok_or_else(reject)?,
        );
        let source_record = &self.children[source.child].allocations[&source.local];
        let destination_record = &self.children[destination.child].allocations[&destination.local];
        let transfer = match segments {
            Some(plan)
                if compute_peer::segment_regions_match(
                    plan,
                    AllocationIdentity::of(source_record),
                    AllocationIdentity::of(destination_record),
                    source_region,
                    destination_region,
                ) =>
            {
                TransferIdentity::Segments(Arc::clone(plan))
            }
            Some(_) => return Err(reject()),
            None => {
                if source_region.byte_len != destination_region.byte_len {
                    return Err(reject());
                }
                TransferIdentity::Window(
                    Gfx942ComputeXgmiCopyWindowV1::new(
                        source_record.bytes.len() as u64,
                        destination_record.bytes.len() as u64,
                        source_region.byte_offset,
                        destination_region.byte_offset,
                        source_region.byte_len,
                    )
                    .ok_or_else(reject)?,
                )
            }
        };
        if source_region.access != RuntimeAccessV1::Read
            || destination_region.access != RuntimeAccessV1::Write
            || !self
                .compute_xgmi_routes
                .contains_key(&(source.child, destination.child))
            || ![source.child, destination.child]
                .into_iter()
                .all(|child| self.children[child].peer_visible_device_allocations)
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
            || self
                .cooperative_allocation_owners
                .contains_key(&destination)
            || !frame.is_intact(self, id)
            || !frame.covers(self, source_region)
        {
            return Err(reject());
        }
        // A physically completed receipt can be used while Context still holds
        // its pending writer/read lease. Context supplies content currentness;
        // this raw backend receipt does not assert a historical content epoch.
        let RoutedSubmissionV1::CooperativeCopy(parent) = &self.submissions[&id] else {
            unreachable!()
        };
        if !matches!(
            parent.status(),
            BackendPollV1::Pending | BackendPollV1::Succeeded
        ) || (parent.status() == BackendPollV1::Succeeded && !parent.is_quiescent())
            || self
                .cooperative_allocation_owners
                .get(&source)
                .is_some_and(|owners| {
                    owners.iter().max().copied() != Some(id)
                        || owners
                            .iter()
                            .any(|owner| !frame.orders_owner(self, id, source, *owner))
                })
        {
            return Err(reject());
        }
        for child in [source.child, destination.child] {
            if let Some(owner) = self.compute_xgmi_children[child]
                && (!frame.owns_occupied_ancestor_child(self, id, child, owner)
                    || !matches!(self.submissions.get(&owner),
                        Some(RoutedSubmissionV1::CooperativeCopy(copy))
                            if [copy.source.child, copy.destination.child].contains(&child)))
            {
                return Err(reject());
            }
        }
        let retained = Source {
            id,
            frame,
            endpoints: endpoints.map(|(route, region)| {
                (
                    route,
                    region,
                    AllocationIdentity::of(&self.children[route.child].allocations[&route.local]),
                )
            }),
            transfer,
        };
        let mut controls = Vec::new();
        controls
            .try_reserve_exact(events.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("frame peer control allocation failed"))?;
        for event in events {
            controls.push(self.peer_dependency_submission(
                *event,
                source.child,
                destination.child,
            )?);
        }
        if !retained.controls_succeeded(self, &controls) || !retained.source_restorable(self) {
            return Err(reject());
        }
        Ok(Some(retained))
    }

    pub(super) fn peer_frame_destination_covers_v1(
        &self,
        id: u64,
        region: BackendMemoryRegionV1,
    ) -> bool {
        let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&id) else {
            return false;
        };
        let Some(source) = &copy.frame_source else {
            return false;
        };
        let (route, original, identity) = source.endpoints[1];
        copy.status() == BackendPollV1::Pending
            && region.access == RuntimeAccessV1::Read
            && region.allocation == original.allocation
            && self.allocations.get(&region.allocation) == Some(&route)
            && self
                .cooperative_allocation_owners
                .get(&route)
                .and_then(|owners| owners.iter().max())
                .copied()
                == Some(id)
            && self.children[route.child]
                .allocations
                .get(&route.local)
                .is_some_and(|record| {
                    AllocationIdentity::of(record) == identity
                        && record.sdma_initialized
                        && compute_xgmi::checked_region(record, region)
                })
            && source.is_intact(self, id, copy)
    }
}
