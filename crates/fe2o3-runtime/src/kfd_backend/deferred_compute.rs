//! Router custody for a compute consumer waiting for native peer restoration.

use super::*;

mod producer;
use producer::DeferredComputeIdentityV1;

#[cfg(test)]
mod tests;

#[derive(Debug)]
pub(super) struct DeferredComputeV1 {
    pub(super) stream: u64,
    pub(super) child: usize,
    pub(super) route: Option<RoutedHandleV1>,
    pub(super) status: BackendPollV1,
    quiescent: Option<KfdRuntimeBackendErrorV1>,
    completed: Option<CompletedDeferredResultV1>,
    identity: DeferredComputeIdentityV1,
    launch: Option<Arc<RetainedComputeLaunchV1>>,
    collected: Option<CollectedComputeDependenciesV1>,
    kernel: (u64, RoutedHandleV1),
    module: (u64, RoutedHandleV1),
    allocations: Vec<(u64, RoutedHandleV1)>,
    peers: Vec<NativePeerProducerV1>,
}

#[derive(Clone, Copy, Debug)]
struct CompletedDeferredResultV1 {
    route: RoutedHandleV1,
    stream: u64,
    local_stream: u64,
    dependency_depth: usize,
}

#[derive(Debug)]
struct NativePeerProducerV1 {
    id: u64,
    stream: u64,
    dependency_depth: usize,
    directed: bool,
    source: RoutedHandleV1,
    destination: RoutedHandleV1,
    source_region: BackendMemoryRegionV1,
    destination_region: BackendMemoryRegionV1,
    frame: Option<compute_peer::DestinationFrame>,
    segment_frame: Option<Arc<compute_peer::SegmentDestinationFrame>>,
}

#[derive(Debug, Default)]
pub(super) struct DeferredComputeRetainsV1 {
    streams: HashMap<u64, u64>,
    allocations: HashMap<RoutedHandleV1, usize>,
    pub(super) modules: HashMap<RoutedHandleV1, usize>,
}

impl DeferredComputeRetainsV1 {
    pub(super) fn is_empty(&self) -> bool {
        self.streams.is_empty() && self.allocations.is_empty() && self.modules.is_empty()
    }

    fn prepare(
        &mut self,
        stream: u64,
        allocations: &[(u64, RoutedHandleV1)],
        module: RoutedHandleV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.streams.contains_key(&stream)
            || self.modules.get(&module) == Some(&usize::MAX)
            || allocations
                .iter()
                .any(|(_, route)| self.allocations.get(route) == Some(&usize::MAX))
        {
            return Err(KfdRuntimeBackendV1::capacity(
                "deferred compute retain capacity exceeded",
            ));
        }
        self.streams.try_reserve(1).map_err(|_| {
            KfdRuntimeBackendV1::capacity("deferred stream index allocation failed")
        })?;
        self.allocations
            .try_reserve(allocations.len())
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("deferred allocation index allocation failed")
            })?;
        self.modules.try_reserve(1).map_err(|_| {
            KfdRuntimeBackendV1::capacity("deferred module index allocation failed")
        })?;
        Ok(())
    }

    fn acquire(&mut self, id: u64, root: &DeferredComputeV1) {
        assert!(self.streams.insert(root.stream, id).is_none());
        for (_, route) in &root.allocations {
            *self.allocations.entry(*route).or_insert(0) += 1;
        }
        *self.modules.entry(root.module.1).or_insert(0) += 1;
    }

    fn release(&mut self, id: u64, root: &DeferredComputeV1) -> bool {
        if self.streams.get(&root.stream) != Some(&id)
            || self
                .modules
                .get(&root.module.1)
                .is_none_or(|count| *count == 0)
            || root
                .allocations
                .iter()
                .any(|(_, route)| self.allocations.get(route).is_none_or(|count| *count == 0))
        {
            return false;
        }
        self.streams.remove(&root.stream);
        for (_, route) in &root.allocations {
            let count = self
                .allocations
                .get_mut(route)
                .expect("deferred allocation remains indexed");
            *count -= 1;
            if *count == 0 {
                self.allocations.remove(route);
            }
        }
        let count = self
            .modules
            .get_mut(&root.module.1)
            .expect("deferred module remains indexed");
        *count -= 1;
        if *count == 0 {
            self.modules.remove(&root.module.1);
        }
        true
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    fn deferred_peer_identity_is_intact_v1(&self, peer: &NativePeerProducerV1) -> bool {
        matches!(self.submissions.get(&peer.id),
            Some(RoutedSubmissionV1::CooperativeCopy(copy))
                if copy.stream == peer.stream
                    && copy.dependency_depth == peer.dependency_depth
                    && copy.directed.is_some() == peer.directed
                    && copy.source == peer.source
                    && copy.destination == peer.destination
                    && copy.source_region == peer.source_region
                    && copy.destination_region == peer.destination_region
                    && copy.compute_xgmi.as_ref().is_some_and(|root|
                        root.is_segmented() == peer.segment_frame.is_some()))
            && (!peer.directed || self.directed_identity_is_intact_v1(peer.id))
            && peer
                .frame
                .as_ref()
                .is_none_or(|frame| frame.is_intact(self, peer.id))
            && peer
                .segment_frame
                .as_ref()
                .is_none_or(|frame| frame.is_intact(self, peer.id))
    }

    #[cfg(test)]
    pub(super) fn assert_deferred_compute_indexes_consistent_v1(&self) {
        let mut expected = DeferredComputeRetainsV1::default();
        for (id, submission) in &self.submissions {
            if let RoutedSubmissionV1::DeferredCompute(root) = submission
                && root.status == BackendPollV1::Pending
            {
                expected.acquire(*id, root);
            }
        }
        assert_eq!(self.deferred_compute_retains.streams, expected.streams);
        assert_eq!(
            self.deferred_compute_retains.allocations,
            expected.allocations
        );
        assert_eq!(self.deferred_compute_retains.modules, expected.modules);
    }

    pub(super) fn deferred_compute_v1(&self, id: u64) -> Option<&DeferredComputeV1> {
        match self.submissions.get(&id) {
            Some(RoutedSubmissionV1::DeferredCompute(root)) => Some(root),
            _ => None,
        }
    }

    pub(super) fn completed_deferred_dependency_depth_v1(
        &self,
        dependency: BackendLaunchProducerV1,
        child: usize,
    ) -> Result<usize, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(RoutedEventV1::DeferredCompute {
            submission,
            child: event_child,
        }) = self.events.get(&dependency.event).copied()
        else {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "completed deferred dependency requires its exact event",
            ));
        };
        if submission != dependency.producer_submission {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "deferred event does not name the exact producer",
            ));
        }
        let root = self.deferred_compute_v1(submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "deferred event lost its exact producer",
            )
        })?;
        if event_child != child || root.child != child {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "deferred producer belongs to another device",
            ));
        }
        if root.status == BackendPollV1::Pending {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "exact producer-aware launch does not admit pending deferred compute events",
            ));
        }
        let receipt = root.completed.filter(|receipt| {
            root.route == Some(receipt.route)
                && receipt.route.child == child
                && root.stream == receipt.stream
        });
        let record = receipt.and_then(|receipt| {
            let endpoint = self.children.get(child)?;
            endpoint
                .submissions
                .get(&receipt.route.local)
                .filter(|record| {
                    record.status == BackendPollV1::Succeeded
                        && record.stream == receipt.local_stream
                        && record.dependency_depth == receipt.dependency_depth
                        && endpoint.exact_submission_quiescent_v1(receipt.route.local)
                })
        });
        let record = record
            .filter(|_| {
                root.status == BackendPollV1::Succeeded
                    && root.quiescent.is_none()
                    && root.launch.is_none()
                    && root.collected.is_none()
            })
            .ok_or_else(|| {
                KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "deferred producer lacks exact successful child completion",
                )
            })?;
        if record.dependency_depth == 0 {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "completed deferred producer has no dependency depth",
            ));
        }
        record
            .dependency_depth
            .checked_add(1)
            .filter(|depth| *depth <= MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1)
            .ok_or_else(|| {
                KfdRuntimeBackendV1::capacity("completed deferred dependency depth exceeded")
            })
    }

    pub(super) fn completed_deferred_segment_producer_v1(
        &self,
        id: u64,
        child: usize,
    ) -> Option<(u64, usize, Arc<RetainedComputeLaunchV1>)> {
        let root = self.deferred_compute_v1(id)?;
        let launch = root.identity.completed_peer_launch.as_ref()?;
        (root.status == BackendPollV1::Succeeded
            && self.deferred_peer_producer_intact_v1(
                id,
                child,
                root.stream,
                root.identity.depth,
                launch,
            ))
        .then(|| (root.stream, root.identity.depth, Arc::clone(launch)))
    }

    fn deferred_compute_mut_v1(&mut self, id: u64) -> &mut DeferredComputeV1 {
        match self.submissions.get_mut(&id) {
            Some(RoutedSubmissionV1::DeferredCompute(root)) => root,
            _ => unreachable!("retained deferred compute root"),
        }
    }

    pub(super) fn deferred_stream_head_v1(&self, stream: u64) -> Option<u64> {
        self.deferred_compute_retains.streams.get(&stream).copied()
    }

    pub(super) fn require_no_deferred_stream_v1(
        &self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.deferred_stream_head_v1(stream).is_some() {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "stream retains a deferred native-peer compute consumer",
            ));
        }
        Ok(())
    }

    pub(super) fn allocation_retained_by_deferred_compute_v1(&self, route: RoutedHandleV1) -> bool {
        self.deferred_compute_retains
            .allocations
            .contains_key(&route)
    }

    pub(super) fn allocation_retained_by_router_v1(&self, route: RoutedHandleV1) -> bool {
        self.allocation_retained_by_cooperative_copy(route)
            || self.allocation_retained_by_deferred_compute_v1(route)
    }

    pub(super) fn deferred_event_dependency_v1(
        &self,
        submission: u64,
        event_child: usize,
        child: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let root = self.deferred_compute_v1(submission).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "deferred event lost its compute root",
            )
        })?;
        if root.child != event_child || event_child != child {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "deferred compute event belongs to another device",
            ));
        }
        match root.status {
            BackendPollV1::Succeeded => Ok(()),
            BackendPollV1::Pending => Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "deferred compute dependency is pending",
            )),
            BackendPollV1::Failed { .. } => Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "deferred compute dependency failed",
            )),
        }
    }

    fn settle_deferred_compute_v1(
        &mut self,
        id: u64,
        status: BackendPollV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if status != BackendPollV1::Pending {
            let completed = if status == BackendPollV1::Succeeded {
                let root = self
                    .deferred_compute_v1(id)
                    .expect("known deferred completion");
                let receipt = root.route.and_then(|route| {
                    let child = self.children.get(route.child)?;
                    let record = child.submissions.get(&route.local)?;
                    (route.child == root.child
                        && record.status == BackendPollV1::Succeeded
                        && record.dependency_depth == root.identity.depth
                        && child.exact_submission_quiescent_v1(route.local)
                        && root
                            .launch
                            .as_ref()
                            .is_some_and(|launch| launch.stream == record.stream)
                        && self.streams.get(&root.stream)
                            == Some(&RoutedHandleV1 {
                                child: route.child,
                                local: record.stream,
                            }))
                    .then_some(CompletedDeferredResultV1 {
                        route,
                        stream: root.stream,
                        local_stream: record.stream,
                        dependency_depth: record.dependency_depth,
                    })
                });
                Some(receipt.ok_or_else(|| self.directed_corruption_v1())?)
            } else {
                None
            };
            if !self.peer_launch_retains.release(id) {
                return Err(self.directed_corruption_v1());
            }
            let Some(RoutedSubmissionV1::DeferredCompute(root)) = self.submissions.get(&id) else {
                unreachable!()
            };
            if root.status == BackendPollV1::Pending
                && !self.deferred_compute_retains.release(id, root)
            {
                return Err(self.directed_corruption_v1());
            }
            let root = self.deferred_compute_mut_v1(id);
            root.status = status;
            root.completed = completed;
            // Keep the original, once-charged writable launch until this result
            // is released, even when Context has not observed its completion yet.
            // Historical content currentness remains the Context journal's duty;
            // this receipt authenticates the operation and restored child result.
            root.identity.completed_peer_launch = root.launch.take().filter(|launch| {
                status == BackendPollV1::Succeeded
                    && launch
                        .bindings
                        .iter()
                        .any(|binding| binding.region.access == RuntimeAccessV1::Write)
            });
            root.collected = None;
        }
        Ok(status)
    }

    fn quiesce_deferred_compute_v1(
        &mut self,
        id: u64,
        error: KfdRuntimeBackendErrorV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.deferred_compute_mut_v1(id).quiescent = Some(error);
        self.settle_deferred_compute_v1(
            id,
            BackendPollV1::Failed {
                code: COOPERATIVE_COPY_FAILURE_CODE_V1,
            },
        )?;
        Err(RuntimeBackendFailureV1::Quiescent(
            self.deferred_compute_v1(id)
                .expect("retained quiescent root")
                .quiescent
                .as_ref()
                .expect("retained diagnostic")
                .clone(),
        ))
    }

    pub(super) fn observe_deferred_compute_v1(
        &mut self,
        id: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.with_deferred_root_v1(id, |backend| backend.observe_deferred_compute_inner_v1(id))
    }

    fn observe_deferred_compute_inner_v1(
        &mut self,
        id: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let root = self.deferred_compute_v1(id).expect("known deferred root");
        if let Some(error) = &root.quiescent {
            return Err(RuntimeBackendFailureV1::Quiescent(error.clone()));
        }
        if root.status != BackendPollV1::Pending {
            return Ok(root.status);
        }
        let Some(route) = root.route else {
            return Ok(BackendPollV1::Pending);
        };
        if self.compute_xgmi_child_occupied_v1(route.child) {
            return Ok(BackendPollV1::Pending);
        }
        let result = self.children[route.child].poll_v1(route.local);
        match result {
            Ok(status) => self.settle_deferred_compute_v1(id, status),
            Err(RuntimeBackendFailureV1::Quiescent(error)) => {
                self.attribute_deferred_child_quiescence_v1(id, route, error)
            }
            Err(error) => self.latch(Err(error)),
        }
    }

    fn attribute_deferred_child_quiescence_v1(
        &mut self,
        id: u64,
        route: RoutedHandleV1,
        error: KfdRuntimeBackendErrorV1,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        // A stream flush can report its predecessor's quiescence, not this
        // consumer's. Its journal and custody remain live until the exact target settles.
        if self.children[route.child].exact_submission_quiescent_v1(route.local) {
            self.quiesce_deferred_compute_v1(id, error)
        } else {
            Ok(BackendPollV1::Pending)
        }
    }

    pub(super) fn progress_deferred_compute_v1(
        &mut self,
        id: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.with_deferred_root_v1(id, |backend| backend.progress_deferred_compute_inner_v1(id))
    }

    fn with_deferred_root_v1<T>(
        &mut self,
        id: u64,
        operation: impl FnOnce(
            &mut Self,
        ) -> Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let root = self.deferred_compute_v1(id).expect("known deferred root");
        let child = root.child;
        // Rank and ID checks precede cross-router recursive progress. Settled
        // results no longer retain their own parents and need no traversal.
        if root.status == BackendPollV1::Pending
            && (!(1..=MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1).contains(&root.identity.depth)
                || root.peers.iter().any(|peer| {
                    peer.id == 0
                        || peer.id >= id
                        || peer.dependency_depth == 0
                        || peer.dependency_depth >= root.identity.depth
                        || !self.deferred_peer_identity_is_intact_v1(peer)
                }))
        {
            self.children[child].terminal = true;
            return Err(self.directed_corruption_v1());
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| operation(self)));
        match result {
            Ok(result) => {
                if matches!(result, Err(RuntimeBackendFailureV1::Terminal(_))) {
                    self.children[child].terminal = true;
                }
                self.latch(result)
            }
            Err(payload) => {
                self.terminal = true;
                self.children[child].terminal = true;
                std::panic::resume_unwind(payload)
            }
        }
    }

    fn progress_deferred_compute_inner_v1(
        &mut self,
        id: u64,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let root = self.deferred_compute_v1(id).expect("known deferred root");
        if let Some(error) = &root.quiescent {
            return Err(RuntimeBackendFailureV1::Quiescent(error.clone()));
        }
        if root.status != BackendPollV1::Pending {
            return Ok(root.status);
        }
        let child = root.child;
        if let Some(route) = root.route {
            if self.compute_xgmi_child_occupied_v1(child) {
                self.progress_deferred_directed_blocker_v1(child)?;
                return Ok(BackendPollV1::Pending);
            }
            let stream = self.streams[&root.stream].local;
            let result = self.children[child].flush_stream_v1(stream);
            match result {
                Ok(()) => {}
                Err(RuntimeBackendFailureV1::Quiescent(error)) => {
                    return self.attribute_deferred_child_quiescence_v1(id, route, error);
                }
                Err(error) => return Err(error),
            }
            return self.observe_deferred_compute_v1(id);
        }
        let mut selected = None;
        for peer in &root.peers {
            let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&peer.id)
            else {
                return Err(self.directed_corruption_v1());
            };
            if !self.deferred_peer_identity_is_intact_v1(peer) {
                return Err(self.directed_corruption_v1());
            }
            match copy.status() {
                BackendPollV1::Failed { .. } => {
                    return self.settle_deferred_compute_v1(
                        id,
                        BackendPollV1::Failed {
                            code: COOPERATIVE_COPY_FAILURE_CODE_V1,
                        },
                    );
                }
                BackendPollV1::Pending => {
                    let ancestor = self.compute_xgmi_children[child].filter(|owner| {
                        peer.frame.as_ref().is_some_and(|frame| {
                            frame.owns_occupied_child(self, peer.id, child, *owner)
                        }) || peer.segment_frame.as_ref().is_some_and(|frame| {
                            frame.owns_occupied_child(self, peer.id, child, *owner)
                        })
                    });
                    selected = Some((peer.id, peer.directed, ancestor));
                    break;
                }
                BackendPollV1::Succeeded => {
                    if !copy
                        .compute_xgmi
                        .as_ref()
                        .is_some_and(|root| root.is_quiescent())
                        || [copy.source.child, copy.destination.child]
                            .into_iter()
                            .any(|child| self.compute_xgmi_children[child] == Some(peer.id))
                    {
                        return Err(self.directed_corruption_v1());
                    }
                }
            }
        }
        if let Some((peer, directed, ancestor)) = selected {
            // A published gather ancestor can block the selected tail's compute
            // producer before the dependency cursor reaches that ancestor. Restore
            // only this authenticated, already-started owner before child I/O.
            let result = if let Some(ancestor) = ancestor {
                self.progress_cooperative_copy_step_v1(ancestor)
            } else if directed {
                self.progress_retained_directed_peer_v1(peer)
            } else {
                self.progress_cooperative_copy(peer)
            };
            match result {
                Ok(_) => return Ok(BackendPollV1::Pending),
                Err(RuntimeBackendFailureV1::Quiescent(error)) => {
                    return self.quiesce_deferred_compute_v1(id, error);
                }
                Err(error) => return Err(error),
            }
        }
        if self.compute_xgmi_child_occupied_v1(child) {
            self.progress_deferred_directed_blocker_v1(child)?;
            return Ok(BackendPollV1::Pending);
        }
        match self.children[child].require_submission_capacity_v1() {
            Ok(()) => {}
            Err(
                RuntimeBackendFailureV1::Rejected(error)
                | RuntimeBackendFailureV1::Quiescent(error),
            ) => return self.quiesce_deferred_compute_v1(id, error),
            Err(error) => return Err(error),
        }
        let root = self.deferred_compute_v1(id).expect("known deferred root");
        if self
            .streams
            .get(&root.stream)
            .is_none_or(|route| route.child != child)
            || self.kernels.get(&root.kernel.0) != Some(&root.kernel.1)
            || self.modules.get(&root.module.0) != Some(&root.module.1)
            || self.kernel_modules.get(&root.kernel.0) != Some(&root.module.0)
            || root
                .allocations
                .iter()
                .any(|(global, route)| self.allocations.get(global) != Some(route))
        {
            return Err(self.directed_corruption_v1());
        }
        let payload = Arc::clone(
            root.launch
                .as_ref()
                .expect("waiting deferred launch payload"),
        );
        let retained = root
            .collected
            .as_ref()
            .expect("waiting deferred dependencies");
        if self.children[child]
            .next_dependency_depth_v1(
                retained.ordered_predecessor,
                &retained.explicit_success_dependencies,
            )
            .ok()
            .map(|depth| depth.max(retained.minimum_dependency_depth))
            != Some(root.identity.depth)
        {
            return Err(self.directed_corruption_v1());
        }
        let mut dependencies = Vec::new();
        if dependencies
            .try_reserve_exact(retained.explicit_success_dependencies.len())
            .is_err()
        {
            return self.quiesce_deferred_compute_v1(
                id,
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Capacity,
                    "deferred child dependency handoff allocation failed",
                ),
            );
        }
        dependencies.extend_from_slice(&retained.explicit_success_dependencies);
        let collected = CollectedComputeDependenciesV1 {
            minimum_dependency_depth: retained.minimum_dependency_depth,
            ordered_predecessor: retained.ordered_predecessor,
            explicit_success_dependencies: dependencies.into_boxed_slice(),
            input_admission: retained.input_admission,
            peer_gate: None,
            peer_access: PeerComputePermitsV1::default(),
        };
        let expected_local = self.children[child].next_handle;
        let result = self.children[child].submit_collected_compute_with_payload_v1(
            payload.borrowed(),
            collected,
            Some(Arc::clone(&payload)),
        );
        match result {
            Ok(local) => {
                let root = self.deferred_compute_mut_v1(id);
                root.route = Some(RoutedHandleV1 { child, local });
                root.identity.route = root.route;
                Ok(BackendPollV1::Pending)
            }
            Err(
                RuntimeBackendFailureV1::Rejected(error)
                | RuntimeBackendFailureV1::Quiescent(error),
            ) => {
                if self.children[child].next_handle != expected_local {
                    let root = self.deferred_compute_mut_v1(id);
                    root.route = Some(RoutedHandleV1 {
                        child,
                        local: expected_local,
                    });
                    root.identity.route = root.route;
                    if !self.children[child].exact_submission_quiescent_v1(expected_local) {
                        self.children[child].terminal = true;
                        return Err(self.directed_corruption_v1());
                    }
                }
                self.quiesce_deferred_compute_v1(id, error)
            }
            Err(error) => Err(error),
        }
    }

    fn progress_deferred_directed_blocker_v1(
        &mut self,
        child: usize,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(owner) = self.compute_xgmi_children[child] else {
            return Ok(());
        };
        let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&owner) else {
            return Err(self.directed_corruption_v1());
        };
        if copy.directed.is_none() {
            return Ok(());
        }
        if ![copy.source.child, copy.destination.child].contains(&child)
            || copy.is_quiescent()
            || copy
                .compute_xgmi
                .as_ref()
                .is_none_or(|root| root.is_quiescent())
            || !self.directed_identity_is_intact_v1(owner)
        {
            return Err(self.directed_corruption_v1());
        }
        // The occupied endpoint is a resource blocker, not a success dependency.
        // One step may release it; child compute still waits until a later call.
        match self.progress_retained_directed_peer_v1(owner) {
            Ok(_) => Ok(()),
            Err(RuntimeBackendFailureV1::Quiescent(_))
                if matches!(self.submissions.get(&owner),
                    Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.is_quiescent()) =>
            {
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    pub(super) fn cancel_deferred_compute_v1(
        &mut self,
        id: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        self.with_deferred_root_v1(id, |backend| backend.cancel_deferred_compute_inner_v1(id))
    }

    fn cancel_deferred_compute_inner_v1(
        &mut self,
        id: u64,
    ) -> Result<crate::BackendCancellationV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>
    {
        let root = self.deferred_compute_v1(id).expect("known deferred root");
        if root.status != BackendPollV1::Pending {
            return Ok(crate::BackendCancellationV1::TooLate);
        }
        if let Some(route) = root.route {
            self.require_compute_xgmi_child_available_v1(route.child)?;
            let result = self.children[route.child].cancel_v1(route.local);
            let status = self.latch(result)?;
            if status != crate::BackendCancellationV1::Cancelled {
                return Ok(status);
            }
        }
        self.settle_deferred_compute_v1(id, BackendPollV1::Failed { code: -2 })?;
        Ok(crate::BackendCancellationV1::Cancelled)
    }

    pub(super) fn drain_deferred_compute_v1(
        &mut self,
        id: u64,
        deadline: Instant,
        execute: bool,
    ) -> Result<BackendPollV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let mut attempts = 0_u32;
        let mut sleep = WAIT_INITIAL_SLEEP_V1;
        loop {
            let status = if execute && Instant::now() < deadline {
                self.progress_deferred_compute_v1(id)?
            } else {
                self.observe_deferred_compute_v1(id)?
            };
            if status != BackendPollV1::Pending
                || !apply_wait_backoff_v1(attempts, &mut sleep, deadline)
            {
                return Ok(status);
            }
            attempts = attempts.saturating_add(1);
        }
    }

    pub(super) fn record_deferred_compute_event_v1(
        &mut self,
        submission: u64,
        child: usize,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        Self::reserve_route(&mut self.events, "deferred compute event allocation failed")?;
        if self.event_submission_retain_counts.get(&submission) == Some(&usize::MAX) {
            return Err(KfdRuntimeBackendV1::capacity(
                "deferred event retain count overflow",
            ));
        }
        self.event_submission_retain_counts
            .try_reserve(1)
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("deferred event retain allocation failed")
            })?;
        let id = self.next_id()?;
        self.events
            .insert(id, RoutedEventV1::DeferredCompute { submission, child });
        *self
            .event_submission_retain_counts
            .entry(submission)
            .or_insert(0) += 1;
        Ok(id)
    }

    pub(super) fn release_deferred_compute_v1(
        &mut self,
        id: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let root = self.deferred_compute_v1(id).expect("known deferred root");
        if root.status == BackendPollV1::Pending
            || self.event_submission_retain_counts.contains_key(&id)
            || self.submission_retained_as_dependency(id)
            || self.peer_launch_retains.retains(id)
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "deferred compute is pending or retained",
            ));
        }
        let stream = root.stream;
        if let Some(route) = root.route {
            self.require_compute_xgmi_child_available_v1(route.child)?;
            let result = self.children[route.child].release_submission_v1(route.local);
            self.latch(result)?;
        }
        if !self.peer_launch_retains.release(id) {
            return Err(self.directed_corruption_v1());
        }
        self.release_native_stream_submission_v1(stream);
        self.submissions.remove(&id);
        Ok(())
    }
}

mod submission;
