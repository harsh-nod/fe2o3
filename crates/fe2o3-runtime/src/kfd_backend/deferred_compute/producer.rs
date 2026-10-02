//! Stable admission identity for an independently retained downstream peer.

use super::*;

#[derive(Debug)]
pub(super) struct DeferredComputeIdentityV1 {
    launch: std::sync::Weak<RetainedComputeLaunchV1>,
    pub(super) completed_peer_launch: Option<Arc<RetainedComputeLaunchV1>>,
    pub(super) depth: usize,
    pub(super) route: Option<RoutedHandleV1>,
}

impl DeferredComputeIdentityV1 {
    pub(super) fn new(
        launch: &Arc<RetainedComputeLaunchV1>,
        depth: usize,
        route: Option<RoutedHandleV1>,
    ) -> Self {
        Self {
            launch: Arc::downgrade(launch),
            completed_peer_launch: None,
            depth,
            route,
        }
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(in super::super) fn pending_deferred_peer_producer_v1(
        &self,
        id: u64,
        child: usize,
    ) -> Option<(u64, usize, Arc<RetainedComputeLaunchV1>)> {
        let root = self.deferred_compute_v1(id)?;
        let launch = root.launch.as_ref()?;
        (root.child == child
            && root.status == BackendPollV1::Pending
            && self.deferred_peer_producer_intact_v1(
                id,
                child,
                root.stream,
                root.identity.depth,
                launch,
            ))
        .then(|| (root.stream, root.identity.depth, Arc::clone(launch)))
    }

    pub(in super::super) fn deferred_peer_depth_v1(&self, id: u64) -> Option<usize> {
        let depth = self.deferred_compute_v1(id)?.identity.depth;
        (1..=MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1)
            .contains(&depth)
            .then_some(depth)
    }

    pub(in super::super) fn deferred_peer_source_reserved_v1(
        &self,
        id: u64,
        source: RoutedHandleV1,
    ) -> bool {
        let Some(root) = self.deferred_compute_v1(id) else {
            return false;
        };
        root.child == source.child
            && root.status == BackendPollV1::Pending
            && root.allocations.iter().any(|(_, route)| *route == source)
            && self.deferred_compute_retains.streams.get(&root.stream) == Some(&id)
            && self.deferred_compute_retains.allocations.get(&source) == Some(&1)
            && root.route.map_or_else(
                || !self.children[source.child].allocation_is_active(source.local),
                |route| {
                    self.children[source.child]
                        .allocation_custody
                        .get(&source.local)
                        .is_some_and(|custody| {
                            custody.owners.len() == 1
                                && custody.owners.front().is_some_and(|owner| {
                                    owner.kind == RuntimeAllocationCustodyKindV1::Compute
                                        && owner.submission == route.local
                                        && root
                                            .launch
                                            .as_ref()
                                            .is_some_and(|launch| owner.stream == launch.stream)
                                })
                        })
                },
            )
    }

    pub(in super::super) fn deferred_peer_producer_intact_v1(
        &self,
        id: u64,
        child: usize,
        stream: u64,
        depth: usize,
        launch: &Arc<RetainedComputeLaunchV1>,
    ) -> bool {
        let Some(root) = self.deferred_compute_v1(id) else {
            return false;
        };
        if root.child != child
            || root.stream != stream
            || root.identity.depth != depth
            || root.route != root.identity.route
            || !(1..=MAX_DIRECT_SDMA_COPY_DEPENDENCY_DEPTH_V1).contains(&depth)
            || !root.identity.launch.ptr_eq(&Arc::downgrade(launch))
            || root.kernel.1.local != launch.kernel
            || root.kernel.1.child != child
            || root.route.is_some_and(|route| route.child != child)
        {
            return false;
        }
        let endpoint = &self.children[child];
        match root.status {
            BackendPollV1::Pending => {
                root.quiescent.is_none()
                    && root.completed.is_none()
                    && root
                        .launch
                        .as_ref()
                        .is_some_and(|held| Arc::ptr_eq(held, launch))
                    && self.streams.get(&stream)
                        == Some(&RoutedHandleV1 {
                            child,
                            local: launch.stream,
                        })
                    && self.kernels.get(&root.kernel.0) == Some(&root.kernel.1)
                    && self.modules.get(&root.module.0) == Some(&root.module.1)
                    && self.kernel_modules.get(&root.kernel.0) == Some(&root.module.0)
                    && self.deferred_compute_retains.streams.get(&stream) == Some(&id)
                    && self
                        .deferred_compute_retains
                        .modules
                        .get(&root.module.1)
                        .is_some_and(|count| *count > 0)
                    && root.allocations.iter().all(|(global, route)| {
                        self.allocations.get(global) == Some(route)
                            && self
                                .deferred_compute_retains
                                .allocations
                                .get(route)
                                .is_some_and(|count| *count > 0)
                    })
                    && root.route.map_or_else(
                        || {
                            root.collected.as_ref().is_some_and(|collected| {
                                collected.minimum_dependency_depth == depth
                                    && endpoint
                                        .next_dependency_depth_v1(
                                            collected.ordered_predecessor,
                                            &collected.explicit_success_dependencies,
                                        )
                                        .ok()
                                        .is_some_and(|actual| actual.max(depth) == depth)
                            })
                        },
                        |route| {
                            compute_peer::retained_child_launch_intact_v1(
                                endpoint,
                                route.local,
                                depth,
                                launch,
                            )
                        },
                    )
            }
            BackendPollV1::Succeeded => {
                root.quiescent.is_none()
                    && root.launch.is_none()
                    && root.collected.is_none()
                    && root.completed.is_some_and(|receipt| {
                        root.route == Some(receipt.route)
                            && receipt.stream == stream
                            && receipt.local_stream == launch.stream
                            && receipt.dependency_depth == depth
                            && endpoint.submissions.get(&receipt.route.local).is_some_and(
                                |record| {
                                    record.status == BackendPollV1::Succeeded
                                        && record.stream == launch.stream
                                        && record.dependency_depth == depth
                                },
                            )
                            && endpoint.exact_submission_quiescent_v1(receipt.route.local)
                    })
            }
            BackendPollV1::Failed { .. } => {
                root.launch.is_none()
                    && root.collected.is_none()
                    && root.completed.is_none()
                    && root.route.is_none_or(|route| {
                        endpoint.exact_submission_quiescent_v1(route.local)
                            && endpoint
                                .submissions
                                .get(&route.local)
                                .is_some_and(|record| {
                                    matches!(record.status, BackendPollV1::Failed { .. })
                                        && record.stream == launch.stream
                                        && record.dependency_depth == depth
                                })
                    })
            }
        }
    }
}
