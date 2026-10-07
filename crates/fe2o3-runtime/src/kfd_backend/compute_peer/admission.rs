use super::*;

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(in crate::kfd_backend) fn prepare_compute_peer_v1(
        &self,
        source: RoutedHandleV1,
        source_region: BackendMemoryRegionV1,
        destination: RoutedHandleV1,
        destination_region: BackendMemoryRegionV1,
        events: &[u64],
        profile: (
            Option<&Arc<Gfx942ComputeXgmiSegmentsPlanV1>>,
            Option<&SegmentDestinationPredecessor>,
        ),
    ) -> Result<Option<Producer>, Failure> {
        let (segments, segment_predecessor) = profile;
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
            || !(matches!(
                destination_record.sdma_storage,
                KfdRuntimeSdmaStorageV1::Device(_)
                    | KfdRuntimeSdmaStorageV1::H2dReady(_)
                    | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
                    | KfdRuntimeSdmaStorageV1::InitializedStorage(_)
            ) || segments.is_some()
                && self.compute_xgmi_children[destination.child].is_some_and(|owner| {
                    segment_predecessor.is_some_and(|prior| {
                        prior.owns_occupied_child(self, destination.child, owner)
                    })
                }))
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
                segment_predecessor,
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
                segment_predecessor,
            )?;
            return Ok(Some(producer));
        }
        Ok(None)
    }

    pub(super) fn prepare_compute_peer_controls_v1(
        &self,
        producer: &Producer,
        events: &[u64],
        source: usize,
        destination: usize,
        segment_predecessor: Option<&SegmentDestinationPredecessor>,
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
            if producer.segments().is_some()
                && segment_predecessor.is_some_and(|prior| prior.id == id)
            {
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
            let ordered = if producer.segments().is_some() {
                segment_predecessor.map(|prior| prior.id) == latest
                    && owners.iter().all(|owner| {
                        segment_predecessor.is_some_and(|prior| {
                            prior.orders_owner(self, producer.endpoints[1].1, *owner)
                        })
                    })
            } else {
                predecessor.map(|prior| prior.id) == latest
                    && owners.iter().all(|owner| {
                        producer.predecessor_orders_destination_owner(self, predecessor, *owner)
                    })
            };
            if !ordered {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "ordered compute peer requires the exact latest destination writer",
                ));
            }
        }
        Ok(predecessor)
    }

    pub(in crate::kfd_backend) fn compute_peer_dependency_depth_v1(
        &self,
        id: u64,
    ) -> Option<usize> {
        match self.submissions.get(&id)? {
            RoutedSubmissionV1::Native { route, .. } => {
                compute_identity(&self.children[route.child], route.local).map(|(_, depth)| depth)
            }
            RoutedSubmissionV1::CooperativeCopy(copy) => Some(copy.dependency_depth),
            RoutedSubmissionV1::DeferredCompute(_) => self.deferred_peer_depth_v1(id),
        }
    }

    pub(in crate::kfd_backend) fn segment_frame_dependency_depth_v1(
        &self,
        id: u64,
    ) -> Option<usize> {
        // Metadata-only rank projection; custody is validated separately after
        // checking older IDs, so this lookup cannot recursively walk a cycle.
        if let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&id)
            && let Some(frame) = copy
                .compute_xgmi
                .as_ref()
                .and_then(|root| root.segment_frame_v1())
        {
            return Some(frame.depth());
        }
        self.compute_peer_dependency_depth_v1(id)
    }

    pub(in crate::kfd_backend) fn compute_peer_dependency_succeeded_v1(&self, id: u64) -> bool {
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

    pub(in crate::kfd_backend) fn compute_peer_destination_frame_v1(
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

    pub(in crate::kfd_backend) fn compute_peer_readback_frame_v1(
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

    pub(in crate::kfd_backend) fn compute_peer_segment_frame_v1(
        &self,
        id: u64,
        destination: RoutedHandleV1,
    ) -> Option<Arc<SegmentDestinationFrame>> {
        let RoutedSubmissionV1::CooperativeCopy(copy) = self.submissions.get(&id)? else {
            return None;
        };
        let frame = copy.compute_xgmi.as_ref()?.segment_frame_v1()?;
        if copy.status() != BackendPollV1::Pending
            || frame.endpoint.1 != destination
            || self
                .cooperative_allocation_owners
                .get(&destination)
                .is_none_or(|owners| {
                    owners.iter().max().copied() != Some(id)
                        || owners
                            .iter()
                            .any(|owner| !frame.orders_owner(self, id, destination, *owner))
                })
        {
            return None;
        }
        frame.is_intact(self, id).then(|| Arc::clone(frame))
    }

    pub(in crate::kfd_backend) fn prepare_segment_destination_predecessor_v1(
        &self,
        stream: u64,
        destination: RoutedHandleV1,
        events: &[u64],
    ) -> Option<SegmentDestinationPredecessor> {
        let owners = self.cooperative_allocation_owners.get(&destination)?;
        let id = owners.iter().max().copied()?;
        if !events.iter().any(|event| {
            matches!(self.events.get(event),
                Some(RoutedEventV1::CooperativeCopy { submission, child })
                    if *submission == id && *child == destination.child)
        }) {
            return None;
        }
        let frame = self.compute_peer_segment_frame_v1(id, destination)?;
        (frame.stream == stream && !matches!(frame.origin, SegmentFrameOrigin::Frame { .. }))
            .then_some(SegmentDestinationPredecessor { id, frame })
    }

    pub(in crate::kfd_backend) fn prepare_segment_destination_frame_v1(
        &self,
        root: &compute_xgmi::Root,
        submission: (u64, usize, usize),
        endpoints: [(RoutedHandleV1, BackendMemoryRegionV1); 2],
        producer: Option<&Producer>,
        predecessor: Option<SegmentDestinationPredecessor>,
        source_frame: Option<&peer_frame::Source>,
    ) -> Option<Arc<SegmentDestinationFrame>> {
        let plan = root.segment_plan_v1()?;
        if endpoints[0].1.access != RuntimeAccessV1::Read
            || endpoints[1].1.access != RuntimeAccessV1::Write
        {
            return None;
        }
        let origin = match (producer, source_frame) {
            (Some(_), Some(_)) => return None,
            (None, Some(source)) => {
                if predecessor.is_some()
                    || source
                        .segments()
                        .is_none_or(|held| !Arc::ptr_eq(held, plan))
                {
                    return None;
                }
                let (id, frame) = source.ancestor();
                SegmentFrameOrigin::Frame { id, frame }
            }
            (Some(producer), None) => SegmentFrameOrigin::Compute {
                id: producer.id,
                depth: producer.depth,
            },
            (None, None) => {
                // Unrelated pending controls are still success-gated by the
                // existing copy scheduler. Pending endpoint writers are not a
                // stable-source frame and retain only legacy transfer behavior.
                if self
                    .cooperative_allocation_owners
                    .get(&endpoints[1].0)
                    .is_some_and(|owners| {
                        owners.iter().any(|owner| {
                            !predecessor.as_ref().is_some_and(|prior| {
                                prior.orders_owner(self, endpoints[1].0, *owner)
                            })
                        })
                    })
                    || self
                        .cooperative_allocation_owners
                        .get(&endpoints[0].0)
                        .is_some_and(|owners| {
                            owners.iter().any(|id| {
                                !matches!(self.submissions.get(id),
                                Some(RoutedSubmissionV1::CooperativeCopy(copy))
                                    if copy.source == endpoints[0].0
                                        && copy.source_region.access == RuntimeAccessV1::Read
                                        && copy.destination != endpoints[0].0)
                            })
                        })
                {
                    return None;
                }
                SegmentFrameOrigin::Settled
            }
        };
        let [source, endpoint] = endpoints.map(|(route, region)| {
            (
                region.allocation,
                route,
                AllocationIdentity::of(&self.children[route.child].allocations[&route.local]),
            )
        });
        Some(Arc::new(SegmentDestinationFrame {
            stream: submission.0,
            transfer_depth: submission.1,
            depth: submission.2,
            source,
            endpoint,
            plan: Arc::clone(plan),
            origin,
            predecessor,
        }))
    }

    pub(super) fn compute_peer_producer_intact_v1(
        &self,
        id: u64,
        copy: &CooperativeCopySubmissionV1,
        producer: &Producer,
    ) -> bool {
        producer.id < id
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
            && self.submission_retained_as_dependency(producer.id)
    }

    pub(super) fn compute_peer_chain_intact_v1(&self, id: u64) -> Result<(), u64> {
        let mut current = id;
        for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&current)
            else {
                return Err(current);
            };
            if let Some(source) = &copy.frame_source {
                return (source.is_intact(self, current, copy)
                    && copy
                        .compute_xgmi
                        .as_ref()
                        .and_then(|root| root.segment_frame_v1())
                        .map_or_else(
                            || source.segments().is_none(),
                            |frame| frame.is_intact(self, current),
                        ))
                .then_some(())
                .ok_or(current);
            }
            if let Some(frame) = copy
                .compute_xgmi
                .as_ref()
                .and_then(|root| root.segment_frame_v1())
            {
                return frame.is_intact(self, current).then_some(()).ok_or(current);
            }
            let Some(producer) = &copy.compute_producer else {
                return if copy
                    .compute_xgmi
                    .as_ref()
                    .and_then(|root| root.segment_frame_v1())
                    .is_none_or(|frame| frame.is_intact(self, current))
                {
                    Ok(())
                } else {
                    Err(current)
                };
            };
            let intact = producer.segments().is_none()
                && self.compute_peer_producer_intact_v1(current, copy, producer);
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

    pub(in crate::kfd_backend) fn validate_compute_peer_v1(
        &mut self,
        id: u64,
    ) -> Result<(), Failure> {
        if let Err(invalid) = self.compute_peer_chain_intact_v1(id) {
            let retained = match self.submissions.get(&id) {
                Some(RoutedSubmissionV1::CooperativeCopy(copy)) => copy
                    .frame_source
                    .as_ref()
                    .map(|source| source.retained_poison_roots_v1()),
                _ => None,
            };
            self.terminal = true;
            if let Some((endpoints, frame)) = retained {
                // Mutable routes may be the corruption just detected. The held
                // frame profile quarantines only its original admitted pairs.
                self.poison_compute_xgmi_children_v1(endpoints);
                frame.poison_retained_children_v1(self);
            } else {
                for affected in [id, invalid] {
                    if let Some(endpoints) = self.compute_xgmi_endpoints_v1(affected) {
                        self.poison_compute_xgmi_children_v1(endpoints);
                    }
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

    pub(in crate::kfd_backend) fn progress_compute_peer_dependency_v1(
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
        let source_child = match origin {
            Origin::Native(route) => route.child,
            Origin::Deferred { child } => child,
        };
        // A published destination ancestor can hold the source producer's child
        // while that producer precedes the ancestor event in this list's roster.
        // Restore only that exact retained owner before entering any child I/O.
        let ancestor = match &self.submissions[&id] {
            RoutedSubmissionV1::CooperativeCopy(copy) => copy
                .compute_xgmi
                .as_ref()
                .and_then(|root| root.segment_frame_v1())
                .and_then(|frame| {
                    self.compute_xgmi_children[copy.destination.child]
                        .filter(|owner| *owner != id)
                        .filter(|owner| self.compute_xgmi_children[source_child] == Some(*owner))
                        .filter(|owner| {
                            frame.owns_occupied_child(self, id, copy.destination.child, *owner)
                        })
                }),
            _ => None,
        };
        if let Some(ancestor) = ancestor {
            return match self.progress_cooperative_copy_step_v1(ancestor) {
                Ok(_) | Err(RuntimeBackendFailureV1::Quiescent(_)) => Ok(BackendPollV1::Pending),
                Err(error) => Err(error),
            };
        }
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

    pub(in crate::kfd_backend) fn attribute_compute_peer_progress_error_v1(
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
