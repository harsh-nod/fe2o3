//! Exact directed provenance around the existing cooperative transfer owner.

#![forbid(unsafe_code)]

use super::*;
use crate::{
    BackendDirectedPeerDependencyV1, BackendDirectedPeerRouteV1, BackendDirectedScalarPeerCopyV1,
    BackendDirectedScalarProgressV1, RuntimeDirectedScalarPeerCopyBackendV1,
};

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

#[derive(Debug)]
pub(super) struct Root {
    pub(super) submission: u64,
    pub(super) depth: usize,
    pub(super) prior_stream_submission: Option<u64>,
    route: BackendDirectedPeerRouteV1,
    source: RoutedHandleV1,
    destination: RoutedHandleV1,
    extents: [u64; 2],
    dependencies: Vec<BackendDirectedPeerDependencyV1>,
}

impl Root {
    pub(super) fn extents(&self) -> [u64; 2] {
        self.extents
    }

    pub(super) fn dependencies(&self) -> &[BackendDirectedPeerDependencyV1] {
        &self.dependencies
    }

    #[cfg(test)]
    pub(super) fn dependencies_mut(&mut self) -> &mut [BackendDirectedPeerDependencyV1] {
        &mut self.dependencies
    }

    pub(super) fn shares_read_source(
        &self,
        other: &CooperativeCopySubmissionV1,
        endpoint: RoutedHandleV1,
    ) -> bool {
        self.source == endpoint
            && self.route.source.access == RuntimeAccessV1::Read
            && other.source == endpoint
            && other.source_region.access == RuntimeAccessV1::Read
            && other.directed.is_some()
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn directed_corruption_v1(&mut self) -> Failure {
        self.terminal = true;
        RuntimeBackendFailureV1::Terminal(KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Terminal,
            "directed cooperative copy lost retained provenance",
        ))
    }

    fn directed_copy_v1(&self, id: u64) -> Result<&CooperativeCopySubmissionV1, Failure> {
        match self.submissions.get(&id) {
            Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.directed.is_some() => Ok(copy),
            Some(_) => Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "directed copy requires a producer admitted through the same profile",
            )),
            None => Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown directed cooperative submission",
            )),
        }
    }

    pub(super) fn directed_identity_is_intact_v1(&self, id: u64) -> bool {
        let Ok(copy) = self.directed_copy_v1(id) else {
            return false;
        };
        let root = copy.directed.as_ref().unwrap();
        let route = root.route;
        if root.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return false;
        }
        let mut sorted = [0; MAX_RUNTIME_DEPENDENCIES_V1];
        for (slot, entry) in sorted.iter_mut().zip(&root.dependencies) {
            *slot = entry.producer_submission;
        }
        let sorted = &mut sorted[..root.dependencies.len()];
        sorted.sort_unstable();
        if sorted.windows(2).any(|pair| pair[0] == pair[1]) {
            return false;
        }
        let expected = root
            .dependencies
            .iter()
            .map(|entry| entry.producer_submission)
            .chain(root.prior_stream_submission.filter(|tail| {
                !root
                    .dependencies
                    .iter()
                    .any(|entry| entry.producer_submission == *tail)
            }));
        if id == 0
            || root.submission != id
            || root.depth != copy.dependency_depth
            || !(1..=MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1).contains(&root.depth)
            || root.prior_stream_submission != copy.prior_stream_submission
            || root
                .prior_stream_submission
                .is_some_and(|prior| prior == 0 || prior >= id)
            || root.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || root.dependencies.iter().any(|entry| {
                entry.event == 0
                    || entry.producer_submission == 0
                    || entry.producer_submission >= id
            })
            || route.stream != copy.stream
            || route.source != copy.source_region
            || route.destination != copy.destination_region
            || root.source != copy.source
            || root.destination != copy.destination
            || root.source.child == root.destination.child
            || self
                .children
                .get(root.source.child)
                .is_none_or(|child| child.description.backend_device != route.source_device)
            || self
                .children
                .get(root.destination.child)
                .is_none_or(|child| child.description.backend_device != route.destination_device)
            || route.source.byte_len == 0
            || route.source.byte_len != route.destination.byte_len
            || !matches!(
                route.source.access,
                RuntimeAccessV1::Read | RuntimeAccessV1::ReadWrite
            )
            || !matches!(
                route.destination.access,
                RuntimeAccessV1::Write | RuntimeAccessV1::ReadWrite
            )
            || [route.source, route.destination]
                .into_iter()
                .zip(root.extents)
                .any(|(region, extent)| {
                    region
                        .byte_offset
                        .checked_add(region.byte_len)
                        .is_none_or(|end| end > extent)
                })
        {
            return false;
        }
        if copy.is_quiescent() {
            return copy.dependencies.is_empty() && copy.staging.is_empty();
        }
        if !copy.dependencies.iter().copied().eq(expected)
            || copy.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || copy.dependency_cursor > copy.dependencies.len()
            || copy.byte_cursor > copy.staging.len()
            || copy.staging.len() as u64 != route.source.byte_len
            || self
                .streams
                .get(&route.stream)
                .is_none_or(|stream| stream.child != root.destination.child)
            || self
                .cooperative_stream_pending_counts
                .get(&route.stream)
                .is_none_or(|count| *count == 0)
        {
            return false;
        }
        match copy.phase {
            CooperativeCopyPhaseV1::Dependencies => {
                if copy.byte_cursor != 0 || copy.sdma_leaf.is_some() {
                    return false;
                }
            }
            CooperativeCopyPhaseV1::Read | CooperativeCopyPhaseV1::Write => {
                if copy.dependency_cursor != copy.dependencies.len() {
                    return false;
                }
            }
            _ => return false,
        }
        // Consumed producers retain conclusive local success and their original
        // identity. Their already-consumed histories need not be re-walked.
        if copy.dependencies[..copy.dependency_cursor]
            .iter()
            .any(|parent| !self.directed_consumed_success_v1(*parent))
        {
            return false;
        }
        for ((allocation, endpoint), extent) in [
            (route.source.allocation, root.source),
            (route.destination.allocation, root.destination),
        ]
        .into_iter()
        .zip(root.extents)
        {
            if self.allocations.get(&allocation) != Some(&endpoint)
                || !self.directed_owner_roster_is_intact_v1(endpoint)
                || self.children[endpoint.child]
                    .allocations
                    .get(&endpoint.local)
                    .is_none_or(|record| record.bytes.len() as u64 != extent)
                || self
                    .cooperative_allocation_owners
                    .get(&endpoint)
                    .is_none_or(|owners| !owners.contains(&id))
            {
                return false;
            }
        }
        copy.dependencies.iter().all(|dependency| {
            self.cooperative_dependency_retain_counts
                .get(dependency)
                .is_some_and(|count| *count != 0)
                && self.directed_copy_v1(*dependency).is_ok()
        })
    }

    fn directed_consumed_success_v1(&self, id: u64) -> bool {
        let Ok(copy) = self.directed_copy_v1(id) else {
            return false;
        };
        let root = copy.directed.as_ref().unwrap();
        copy.status() == BackendPollV1::Succeeded
            && copy.dependencies.is_empty()
            && copy.staging.is_empty()
            && id != 0
            && root.submission == id
            && root.depth == copy.dependency_depth
            && (1..=MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1).contains(&root.depth)
            && root.prior_stream_submission == copy.prior_stream_submission
            && root.route.stream == copy.stream
            && root.route.source == copy.source_region
            && root.route.destination == copy.destination_region
            && root.source == copy.source
            && root.destination == copy.destination
    }

    pub(super) fn check_directed_identity_v1(&mut self, id: u64) -> Result<(), Failure> {
        if self.directed_identity_is_intact_v1(id) {
            Ok(())
        } else {
            Err(self.directed_corruption_v1())
        }
    }

    pub(super) fn check_directed_if_present_v1(&mut self, id: u64) -> Result<(), Failure> {
        if matches!(self.submissions.get(&id), Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.directed.is_some())
        {
            self.check_directed_identity_v1(id)?;
        }
        Ok(())
    }

    pub(super) fn directed_owner_roster_is_intact_v1(&self, endpoint: RoutedHandleV1) -> bool {
        let Some(owners) = self.cooperative_allocation_owners.get(&endpoint) else {
            return true;
        };
        if owners.is_empty() || owners.len() > MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 {
            return false;
        }
        let mut sorted = [0; MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1];
        sorted[..owners.len()].copy_from_slice(owners);
        let sorted = &mut sorted[..owners.len()];
        sorted.sort_unstable();
        sorted[0] != 0
            && !sorted.windows(2).any(|pair| pair[0] == pair[1])
            && owners.iter().all(|id| {
                matches!(self.submissions.get(id), Some(RoutedSubmissionV1::CooperativeCopy(copy))
                    if !copy.is_quiescent() && [copy.source, copy.destination].contains(&endpoint))
            })
    }

    pub(super) fn admit_directed_owner_capacity_v1(
        &mut self,
        endpoints: [RoutedHandleV1; 2],
        incoming_directed: bool,
    ) -> Result<(), Failure> {
        for endpoint in endpoints {
            let Some(owners) = self.cooperative_allocation_owners.get(&endpoint) else {
                continue;
            };
            let has_directed = owners.iter().any(|id| {
                matches!(self.submissions.get(id), Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.directed.is_some())
            });
            if !incoming_directed && !has_directed {
                continue;
            }
            // A legacy-only roster may predate this bounded profile. Reject
            // joining it without treating the existing legacy state as corrupt.
            if owners.len() > MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 && !has_directed {
                return Err(KfdRuntimeBackendV1::capacity(
                    "directed allocation owner capacity exceeded",
                ));
            }
            if !self.directed_owner_roster_is_intact_v1(endpoint)
                || owners.iter().any(|id| {
                    matches!(self.submissions.get(id), Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.directed.is_some())
                        && !self.directed_identity_is_intact_v1(*id)
                })
            {
                return Err(self.directed_corruption_v1());
            }
            if owners.len() == MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 {
                return Err(KfdRuntimeBackendV1::capacity(
                    "directed allocation owner capacity exceeded",
                ));
            }
        }
        Ok(())
    }

    fn prepare_directed_copy_v1(
        &mut self,
        request: BackendDirectedScalarPeerCopyV1<'_>,
    ) -> Result<Root, Failure> {
        self.require_live()?;
        if request.dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(KfdRuntimeBackendV1::capacity(
                "directed cooperative dependency capacity exceeded",
            ));
        }
        let route = request.route;
        let stream = Self::route(&self.streams, route.stream, "unknown directed copy stream")?;
        let source = Self::route(
            &self.allocations,
            route.source.allocation,
            "unknown directed source",
        )?;
        let destination = Self::route(
            &self.allocations,
            route.destination.allocation,
            "unknown directed destination",
        )?;
        if source.child == destination.child
            || stream.child != destination.child
            || self.children[source.child].description.backend_device != route.source_device
            || self.children[destination.child].description.backend_device
                != route.destination_device
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::WrongDevice,
                "directed copy devices do not match its retained allocation route",
            ));
        }
        let extents = [source, destination].map(|endpoint| {
            self.children[endpoint.child]
                .allocations
                .get(&endpoint.local)
                .map(|record| record.bytes.len() as u64)
        });
        let [Some(source_extent), Some(destination_extent)] = extents else {
            return Err(self.directed_corruption_v1());
        };
        self.admit_directed_owner_capacity_v1([source, destination], true)?;
        for (index, dependency) in request.dependencies.iter().enumerate() {
            let event = self.events.get(&dependency.event).copied().ok_or_else(|| {
                KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::UnknownHandle,
                    "unknown directed dependency event",
                )
            })?;
            let (actual, child, cooperative) = match event {
                RoutedEventV1::CooperativeCopy { submission, child } => (submission, child, true),
                RoutedEventV1::Native { submission, route } => (submission, route.child, false),
            };
            let intact = match self.submissions.get(&actual) {
                Some(RoutedSubmissionV1::CooperativeCopy(copy)) => {
                    cooperative && child == copy.destination.child
                }
                Some(RoutedSubmissionV1::Native { route, .. }) => {
                    !cooperative && child == route.child
                }
                None => false,
            };
            if !intact {
                return Err(self.directed_corruption_v1());
            }
            let actual =
                self.peer_dependency_submission(dependency.event, source.child, destination.child)?;
            if actual != dependency.producer_submission
                || request.dependencies[..index]
                    .iter()
                    .any(|prior| prior.producer_submission == actual)
            {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "directed dependency requires a distinct exact event/producer binding",
                ));
            }
            self.directed_copy_v1(actual)?;
            self.check_directed_identity_v1(actual)?;
        }
        if let Some(tail) = self.cooperative_stream_tails.get(&route.stream).copied() {
            if !matches!(self.submissions.get(&tail), Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.stream == route.stream)
            {
                return Err(self.directed_corruption_v1());
            }
            self.directed_copy_v1(tail)?;
            self.check_directed_identity_v1(tail)?;
        }
        let mut dependencies = Vec::new();
        dependencies
            .try_reserve_exact(request.dependencies.len())
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("directed cooperative provenance allocation failed")
            })?;
        dependencies.extend_from_slice(request.dependencies);
        Ok(Root {
            submission: 0,
            depth: 0,
            prior_stream_submission: None,
            route,
            source,
            destination,
            extents: [source_extent, destination_extent],
            dependencies,
        })
    }
}

impl RuntimeDirectedScalarPeerCopyBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    fn submit_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarPeerCopyV1<'_>,
    ) -> Result<u64, Failure> {
        let root = self.prepare_directed_copy_v1(request)?;
        let mut events = [0; MAX_RUNTIME_DEPENDENCIES_V1];
        for (event, dependency) in events.iter_mut().zip(request.dependencies) {
            *event = dependency.event;
        }
        let route = request.route;
        // All fallible metadata reservations precede the shared admission's
        // ownership commit. No provisional native event or completion exists.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.submit_cooperative_copy_profile_v1(
                route.stream,
                route.source,
                route.destination,
                &events[..request.dependencies.len()],
                true,
                Some(root),
            )
        }));
        match outcome {
            Ok(result) => result,
            Err(payload) => {
                self.terminal = true;
                std::panic::resume_unwind(payload)
            }
        }
    }

    /// Iterative selection visits at most 256 roots of at most 256 dependencies.
    /// One selected step observes/publishes at most one private DMA window,
    /// copies at most 64 KiB, disposes one owner, or settles one failed copy.
    /// Allocator, driver and profiling calls have no hard wall-clock bound.
    fn progress_directed_scalar_peer_copy_v1(
        &mut self,
        request: BackendDirectedScalarProgressV1<'_>,
    ) -> Result<BackendPollV1, Failure> {
        self.require_live()?;
        self.directed_copy_v1(request.submission)?;
        self.check_directed_identity_v1(request.submission)?;
        let copy = self.directed_copy_v1(request.submission)?;
        let root = copy.directed.as_ref().unwrap();
        if root.route != request.route
            || !root
                .dependencies
                .iter()
                .map(|entry| entry.producer_submission)
                .eq(request.producer_submissions.iter().copied())
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "directed progress does not match retained route and ordered producers",
            ));
        }
        self.progress_retained_directed_peer_v1(request.submission)
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn progress_retained_directed_peer_v1(
        &mut self,
        submission: u64,
    ) -> Result<BackendPollV1, Failure> {
        self.require_live()?;
        self.check_directed_identity_v1(submission)?;
        let copy = self.directed_copy_v1(submission)?;
        if copy.is_quiescent() {
            return Ok(copy.status());
        }
        let mut selected = submission;
        let mut found = false;
        for _ in 0..MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            self.check_directed_identity_v1(selected)?;
            let copy = self.directed_copy_v1(selected)?;
            let predecessor = (copy.phase == CooperativeCopyPhaseV1::Dependencies)
                .then(|| copy.dependencies.get(copy.dependency_cursor).copied())
                .flatten();
            if let Some(predecessor) = predecessor {
                self.check_directed_identity_v1(predecessor)?;
                if !self.directed_copy_v1(predecessor)?.is_quiescent() {
                    selected = predecessor;
                    continue;
                }
            }
            found = true;
            break;
        }
        if !found {
            return Err(self.directed_corruption_v1());
        }
        if let Some(blocker) = self.directed_private_blocker_v1(selected)? {
            selected = blocker;
        }
        match self.progress_cooperative_copy_step_v1(selected) {
            Ok(_) => {}
            Err(failure @ RuntimeBackendFailureV1::Quiescent(_)) => {
                self.check_directed_identity_v1(selected)?;
                if !self.directed_copy_v1(selected)?.is_quiescent() {
                    return Err(self.directed_corruption_v1());
                }
                if selected == submission {
                    return Err(failure);
                }
                // Do not misattribute another owner's result. A failed success
                // dependency will be observed later; a resource sibling is not
                // a success dependency of the requested operation.
            }
            Err(failure) => return Err(failure),
        }
        self.check_directed_identity_v1(submission)?;
        Ok(self.directed_copy_v1(submission)?.status())
    }
}
