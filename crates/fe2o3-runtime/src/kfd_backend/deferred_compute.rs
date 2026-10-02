//! Router custody for a compute consumer waiting for native peer restoration.

use super::*;

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
    source: RoutedHandleV1,
    destination: RoutedHandleV1,
    source_region: BackendMemoryRegionV1,
    destination_region: BackendMemoryRegionV1,
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

    /// `None` leaves the existing ordinary/directed admission paths unchanged.
    pub(super) fn try_submit_deferred_compute_v1(
        &mut self,
        request: BackendProducerAwareLaunchV1<'_>,
    ) -> Result<Option<u64>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if request.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || request.bindings.len() > fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1
        {
            return Err(KfdRuntimeBackendV1::capacity(
                "deferred compute admission capacity exceeded",
            ));
        }
        let has_native_peer = request.dependencies.iter().any(|dependency| {
            matches!(
            self.submissions.get(&dependency.producer_submission),
            Some(RoutedSubmissionV1::CooperativeCopy(copy))
                if copy.directed.is_none() && copy.compute_xgmi.is_some() && !copy.is_quiescent())
        });
        if !has_native_peer {
            return Ok(None);
        }
        self.require_no_deferred_stream_v1(request.stream)?;
        let stream = Self::route(
            &self.streams,
            request.stream,
            "unknown deferred compute stream",
        )?;
        let kernel = Self::route(
            &self.kernels,
            request.kernel,
            "unknown deferred compute kernel",
        )?;
        let module_id = *self.kernel_modules.get(&request.kernel).ok_or_else(|| {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "deferred compute kernel lost its module",
            )
        })?;
        let module = Self::route(&self.modules, module_id, "unknown deferred compute module")?;
        if kernel.child != stream.child || module.child != stream.child {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "deferred compute kernel belongs to another device",
            ));
        }
        let mut bindings = Vec::new();
        let mut allocations = Vec::new();
        bindings
            .try_reserve_exact(request.bindings.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("deferred bindings allocation failed"))?;
        allocations
            .try_reserve_exact(request.bindings.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("deferred allocation roster failed"))?;
        for binding in request.bindings {
            let allocation = Self::route(
                &self.allocations,
                binding.region.allocation,
                "unknown deferred compute allocation",
            )?;
            if allocation.child != stream.child {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::WrongDevice,
                    "deferred compute binding belongs to another device",
                ));
            }
            if self.allocation_retained_by_deferred_compute_v1(allocation) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "binding belongs to another deferred compute consumer",
                ));
            }
            if !allocations.iter().any(|(_, route)| *route == allocation) {
                allocations.push((binding.region.allocation, allocation));
            }
            bindings.push(BackendBindingV1 {
                region: BackendMemoryRegionV1 {
                    allocation: allocation.local,
                    ..binding.region
                },
                kernarg_byte_offset: binding.kernarg_byte_offset,
            });
        }
        let mut retained = Vec::new();
        let mut peers = Vec::new();
        let mut native = Vec::new();
        retained
            .try_reserve_exact(request.dependencies.len() + 1)
            .map_err(|_| KfdRuntimeBackendV1::capacity("deferred producer roster failed"))?;
        peers
            .try_reserve_exact(request.dependencies.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("deferred peer roster failed"))?;
        native
            .try_reserve_exact(request.dependencies.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("deferred native roster failed"))?;
        let mut depth = 1;
        for dependency in request.dependencies {
            if retained.contains(&dependency.producer_submission) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "deferred compute producers must be distinct",
                ));
            }
            match self.events.get(&dependency.event).copied() {
                Some(RoutedEventV1::CooperativeCopy { submission, child })
                    if submission == dependency.producer_submission && child == stream.child =>
                {
                    let Some(RoutedSubmissionV1::CooperativeCopy(copy)) =
                        self.submissions.get(&submission)
                    else {
                        return Err(self.directed_corruption_v1());
                    };
                    if copy.directed.is_some()
                        || copy.compute_xgmi.is_none()
                        || copy.destination.child != stream.child
                        || matches!(copy.status(), BackendPollV1::Failed { .. })
                        || request
                            .bindings
                            .iter()
                            .filter(|binding| {
                                binding.region.allocation == copy.destination_region.allocation
                            })
                            .any(|binding| {
                                binding.region.access != RuntimeAccessV1::Read
                                    || binding.region.byte_len == 0
                                    || binding.region.byte_offset
                                        < copy.destination_region.byte_offset
                                    || binding
                                        .region
                                        .byte_offset
                                        .checked_add(binding.region.byte_len)
                                        .zip(
                                            copy.destination_region
                                                .byte_offset
                                                .checked_add(copy.destination_region.byte_len),
                                        )
                                        .is_none_or(|(end, producer_end)| end > producer_end)
                            })
                    {
                        return Err(KfdRuntimeBackendV1::rejected(
                            KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                            "deferred peer destination aliases require covered read ranges",
                        ));
                    }
                    depth = depth.max(copy.dependency_depth.checked_add(1).ok_or_else(|| {
                        KfdRuntimeBackendV1::capacity("deferred dependency depth overflow")
                    })?);
                    peers.push(NativePeerProducerV1 {
                        id: submission,
                        source: copy.source,
                        destination: copy.destination,
                        source_region: copy.source_region,
                        destination_region: copy.destination_region,
                    });
                }
                Some(RoutedEventV1::Native { .. }) => {
                    let local = self
                        .exact_launch_dependency_for_child(*dependency, stream.child)?
                        .expect("native event returns an exact local producer");
                    native.push(local);
                }
                Some(RoutedEventV1::DeferredCompute { .. }) => {
                    depth = depth.max(
                        self.completed_deferred_dependency_depth_v1(*dependency, stream.child)?,
                    );
                }
                _ => {
                    return Err(KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                        "deferred producer event does not name its exact native producer",
                    ));
                }
            }
            retained.push(dependency.producer_submission);
        }
        if depth > MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1 {
            return Err(KfdRuntimeBackendV1::capacity(
                "deferred dependency depth exceeds its bound",
            ));
        }
        if self
            .cooperative_stream_tails
            .get(&request.stream)
            .is_some_and(|tail| !retained.contains(tail))
            || allocations.iter().any(|(_, route)| {
                self.cooperative_allocation_owners
                    .get(route)
                    .is_some_and(|owners| owners.iter().any(|owner| !retained.contains(owner)))
            })
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "deferred consumer has an unrelated cooperative predecessor",
            ));
        }
        // A busy child is admissible only as a metadata-only wait for that exact copy.
        if self.compute_xgmi_children[stream.child]
            .is_some_and(|owner| !peers.iter().any(|peer| peer.id == owner))
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "deferred consumer child belongs to another native copy",
            ));
        }
        let launch = BackendLaunchV1 {
            stream: stream.local,
            kernel: kernel.local,
            explicit_kernarg: request.explicit_kernarg,
            bindings: &bindings,
            dependencies: &[],
            geometry: request.geometry,
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        };
        let result = self.children[stream.child]
            .preflight_compute_v1(launch, ComputeDependencyRosterV1::Exact(&native));
        let mut collected = self.latch(result)?;
        let result = self.children[stream.child].validate_compute_launch_base_v1(&launch);
        self.latch(result)?;
        collected.minimum_dependency_depth = depth;
        if let Some(prior) = collected.ordered_predecessor {
            let owner = self
                .submissions
                .iter()
                .find_map(|(id, entry)| match entry {
                    RoutedSubmissionV1::Native { route, .. }
                        if *route
                            == (RoutedHandleV1 {
                                child: stream.child,
                                local: prior,
                            }) =>
                    {
                        Some(*id)
                    }
                    RoutedSubmissionV1::DeferredCompute(root)
                        if root.route
                            == Some(RoutedHandleV1 {
                                child: stream.child,
                                local: prior,
                            })
                            && root.status == BackendPollV1::Succeeded
                            && retained.contains(id) =>
                    {
                        Some(*id)
                    }
                    _ => None,
                })
                .ok_or_else(|| {
                    KfdRuntimeBackendV1::rejected(
                        KfdRuntimeBackendErrorKindV1::Busy,
                        "deferred consumer stream has an unsupported predecessor",
                    )
                })?;
            if !retained.contains(&owner) {
                retained.push(owner);
            }
        }
        let payload = RetainedComputeLaunchV1::copy_from(
            launch,
            self.children[stream.child].launch_payload_account.as_ref(),
        )?;
        self.peer_launch_retains.prepare(&retained)?;
        self.deferred_compute_retains
            .prepare(request.stream, &allocations, module)?;
        self.reserve_native_stream_submission_v1(request.stream)?;
        Self::reserve_route(
            &mut self.submissions,
            "deferred compute route allocation failed",
        )?;
        let shell = try_uninit_box_v1().map_err(|()| {
            KfdRuntimeBackendV1::capacity("deferred compute root allocation failed")
        })?;
        let id = self.next_id()?;
        self.with_peer_launch_custody_v1(id, retained, |backend| {
            let root = Box::write(
                shell,
                DeferredComputeV1 {
                    stream: request.stream,
                    child: stream.child,
                    route: None,
                    status: BackendPollV1::Pending,
                    quiescent: None,
                    completed: None,
                    launch: Some(payload),
                    collected: Some(collected),
                    kernel: (request.kernel, kernel),
                    module: (module_id, module),
                    allocations,
                    peers,
                },
            );
            backend.deferred_compute_retains.acquire(id, &root);
            backend
                .submissions
                .insert(id, RoutedSubmissionV1::DeferredCompute(root));
            backend.retain_native_stream_submission_v1(request.stream);
            Ok(id)
        })?;
        Ok(Some(id))
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
            root.launch = None;
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
        let child = self
            .deferred_compute_v1(id)
            .expect("known deferred root")
            .child;
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
            if copy.source != peer.source
                || copy.destination != peer.destination
                || copy.source_region != peer.source_region
                || copy.destination_region != peer.destination_region
                || copy.directed.is_some()
                || copy.compute_xgmi.is_none()
            {
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
                    selected = Some(peer.id);
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
        if let Some(peer) = selected {
            match self.progress_cooperative_copy(peer) {
                Ok(_) => return Ok(BackendPollV1::Pending),
                Err(RuntimeBackendFailureV1::Quiescent(error)) => {
                    return self.quiesce_deferred_compute_v1(id, error);
                }
                Err(error) => return Err(error),
            }
        }
        if self.compute_xgmi_child_occupied_v1(child) {
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
                self.deferred_compute_mut_v1(id).route = Some(RoutedHandleV1 { child, local });
                Ok(BackendPollV1::Pending)
            }
            Err(
                RuntimeBackendFailureV1::Rejected(error)
                | RuntimeBackendFailureV1::Quiescent(error),
            ) => {
                if self.children[child].next_handle != expected_local {
                    self.deferred_compute_mut_v1(id).route = Some(RoutedHandleV1 {
                        child,
                        local: expected_local,
                    });
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
