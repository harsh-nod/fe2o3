//! Bounded immutable predecessor closure; membership is not input readiness.

use super::*;
use crate::BackendDirectedPeerDependencyV1;

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

pub(super) const MAX_PEER_LAUNCH_ANCESTORS_V1: usize =
    fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 * MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1;
const MAX_PEER_LAUNCH_EDGES_V1: usize = MAX_PEER_LAUNCH_ANCESTORS_V1 * 8;

#[derive(Debug)]
struct PeerAncestorV1 {
    id: u64,
    stream: u64,
    source: RoutedHandleV1,
    destination: RoutedHandleV1,
    source_region: BackendMemoryRegionV1,
    destination_region: BackendMemoryRegionV1,
    depth: usize,
    prior: Option<u64>,
    directed_extents: Option<[u64; 2]>,
    terminal: Option<BackendPollV1>,
    dependencies: Range<usize>,
}

#[derive(Debug)]
pub(super) struct PeerLaunchAncestryV1 {
    owner: u64,
    stream: u64,
    success_roots: Box<[u64]>,
    ordered: Option<u64>,
    nodes: Vec<PeerAncestorV1>,
    dependencies: Vec<BackendDirectedPeerDependencyV1>,
    depth: usize,
}

impl PeerLaunchAncestryV1 {
    pub(super) fn stream(&self) -> u64 {
        self.stream
    }

    pub(super) fn roots(&self) -> impl Iterator<Item = u64> + '_ {
        self.ordered
            .into_iter()
            .chain(self.success_roots.iter().copied())
    }

    pub(super) fn state(
        &self,
        backend: &KfdMultiDeviceRuntimeBackendV1,
    ) -> (PeerComputeResultV1, bool) {
        let status = |id| match &backend.submissions[&id] {
            RoutedSubmissionV1::CooperativeCopy(copy) => copy.status(),
            _ => unreachable!("authenticated peer ancestry names cooperative copies"),
        };
        let result = if self
            .success_roots
            .iter()
            .any(|id| matches!(status(*id), BackendPollV1::Failed { .. }))
        {
            PeerComputeResultV1::Failed
        } else if self
            .success_roots
            .iter()
            .all(|id| status(*id) == BackendPollV1::Succeeded)
        {
            PeerComputeResultV1::Succeeded
        } else {
            PeerComputeResultV1::Pending
        };
        (
            result,
            self.ordered
                .is_none_or(|id| status(id) != BackendPollV1::Pending),
        )
    }

    pub(super) fn owner(&self) -> u64 {
        self.owner
    }
    pub(super) fn depth(&self) -> usize {
        self.depth
    }

    pub(super) fn producers(&self) -> impl Iterator<Item = u64> + '_ {
        self.nodes.iter().map(|node| node.id)
    }

    pub(super) fn contains(&self, id: u64) -> bool {
        self.nodes.binary_search_by_key(&id, |node| node.id).is_ok()
    }

    pub(super) fn retained_ids(&self) -> Result<Vec<u64>, Failure> {
        let mut ids = Vec::new();
        ids.try_reserve_exact(self.nodes.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("peer ancestry retain allocation failed"))?;
        ids.extend(self.producers());
        Ok(ids)
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn capture_peer_launch_ancestry_v1(
        &mut self,
        owner: u64,
        stream: u64,
        success_roots: &[u64],
    ) -> Result<PeerLaunchAncestryV1, Failure> {
        self.capture_peer_launch_ancestry_with_limits_v1(
            owner,
            stream,
            success_roots,
            MAX_PEER_LAUNCH_ANCESTORS_V1,
            MAX_PEER_LAUNCH_EDGES_V1,
        )
    }

    pub(super) fn capture_peer_launch_ancestry_with_limits_v1(
        &mut self,
        owner: u64,
        stream: u64,
        success_roots: &[u64],
        node_limit: usize,
        edge_limit: usize,
    ) -> Result<PeerLaunchAncestryV1, Failure> {
        self.require_live()?;
        Self::route(&self.streams, stream, "unknown peer launch stream")?;
        if owner == 0 || success_roots.len() > MAX_RUNTIME_DEPENDENCIES_V1 {
            return Err(KfdRuntimeBackendV1::capacity(
                "peer ancestry root capacity exceeded",
            ));
        }
        let mut sorted_roots = [0; MAX_RUNTIME_DEPENDENCIES_V1];
        sorted_roots[..success_roots.len()].copy_from_slice(success_roots);
        let sorted_roots = &mut sorted_roots[..success_roots.len()];
        sorted_roots.sort_unstable();
        if sorted_roots.iter().any(|id| *id == 0 || *id >= owner)
            || sorted_roots.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "peer ancestry requires distinct earlier producers",
            ));
        }
        let ordered = self.cooperative_stream_tails.get(&stream).copied();
        if ordered.is_some_and(|id| id == 0 || id >= owner
            || !matches!(self.submissions.get(&id), Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.stream == stream)) {
            return Err(self.directed_corruption_v1());
        }
        let node_limit = node_limit.min(MAX_PEER_LAUNCH_ANCESTORS_V1);
        let edge_limit = edge_limit.min(MAX_PEER_LAUNCH_EDGES_V1);
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(success_roots.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("peer ancestry roots allocation failed"))?;
        roots.extend_from_slice(success_roots);
        let roots = roots.into_boxed_slice();
        let mut frontier = Vec::new();
        let mut visited = HashSet::new();
        let mut nodes = Vec::new();
        let mut dependencies = Vec::new();
        let mut edge_work = 0_usize;
        let mut depth = 1_usize;
        for id in success_roots.iter().copied().chain(ordered) {
            if visited.contains(&id) {
                continue;
            }
            Self::queue_peer_ancestor_v1(id, node_limit, &mut visited, &mut frontier)?;
        }
        while let Some(id) = frontier.pop() {
            self.check_directed_if_present_v1(id)?;
            let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&id) else {
                return Err(self.directed_corruption_v1());
            };
            if copy.directed.is_none() && !copy.is_quiescent() {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "pending peer ancestry requires directed provenance",
                ));
            }
            if !(1..=MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1).contains(&copy.dependency_depth)
                || copy.is_quiescent()
                    && (!copy.dependencies.is_empty() || !copy.staging.is_empty())
            {
                return Err(self.directed_corruption_v1());
            }
            // Preserve original event identity even after the public event or
            // terminal grandparents have been released. Only live edges recurse.
            let original = copy
                .directed
                .as_ref()
                .map_or(&[][..], |root| root.dependencies());
            let implicit = copy.prior_stream_submission.filter(|prior| {
                !original
                    .iter()
                    .any(|entry| entry.producer_submission == *prior)
            });
            edge_work = edge_work
                .checked_add(original.len())
                .and_then(|count| count.checked_add(usize::from(implicit.is_some())))
                .filter(|count| *count <= edge_limit)
                .ok_or_else(|| {
                    KfdRuntimeBackendV1::capacity("peer ancestry edge capacity exceeded")
                })?;
            let start = dependencies.len();
            dependencies.try_reserve(original.len()).map_err(|_| {
                KfdRuntimeBackendV1::capacity("peer ancestry edge allocation failed")
            })?;
            dependencies.extend_from_slice(original);
            if !copy.is_quiescent() {
                let mut parent_depth = 0;
                for parent in &copy.dependencies {
                    if *parent == 0 || *parent >= id {
                        return Err(self.directed_corruption_v1());
                    }
                    let Some(RoutedSubmissionV1::CooperativeCopy(parent_copy)) =
                        self.submissions.get(parent)
                    else {
                        return Err(self.directed_corruption_v1());
                    };
                    if Some(*parent) == copy.prior_stream_submission
                        && parent_copy.stream != copy.stream
                    {
                        return Err(self.directed_corruption_v1());
                    }
                    parent_depth = parent_depth.max(parent_copy.dependency_depth);
                    if !visited.contains(parent) {
                        Self::queue_peer_ancestor_v1(
                            *parent,
                            node_limit,
                            &mut visited,
                            &mut frontier,
                        )?;
                    }
                }
                if parent_depth.checked_add(1) != Some(copy.dependency_depth) {
                    return Err(self.directed_corruption_v1());
                }
            }
            if sorted_roots.binary_search(&id).is_ok() {
                let producer_depth = if copy.directed.is_some() {
                    copy.dependency_depth
                } else {
                    1
                };
                depth =
                    depth.max(producer_depth.checked_add(1).ok_or_else(|| {
                        KfdRuntimeBackendV1::capacity("peer launch depth overflow")
                    })?);
            }
            nodes.try_reserve(1).map_err(|_| {
                KfdRuntimeBackendV1::capacity("peer ancestry node allocation failed")
            })?;
            nodes.push(PeerAncestorV1 {
                id,
                stream: copy.stream,
                source: copy.source,
                destination: copy.destination,
                source_region: copy.source_region,
                destination_region: copy.destination_region,
                depth: copy.dependency_depth,
                prior: copy.prior_stream_submission,
                directed_extents: copy.directed.as_ref().map(|root| root.extents()),
                terminal: copy.is_quiescent().then(|| copy.status()),
                dependencies: start..dependencies.len(),
            });
        }
        if depth > MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1 {
            return Err(KfdRuntimeBackendV1::capacity(
                "peer launch dependency depth exceeded",
            ));
        }
        nodes.sort_unstable_by_key(|node| node.id);
        Ok(PeerLaunchAncestryV1 {
            owner,
            stream,
            success_roots: roots,
            ordered,
            nodes,
            dependencies,
            depth,
        })
    }

    fn queue_peer_ancestor_v1(
        id: u64,
        limit: usize,
        visited: &mut HashSet<u64>,
        frontier: &mut Vec<u64>,
    ) -> Result<(), Failure> {
        if visited.len() == limit {
            return Err(KfdRuntimeBackendV1::capacity(
                "peer ancestry node capacity exceeded",
            ));
        }
        visited.try_reserve(1).map_err(|_| {
            KfdRuntimeBackendV1::capacity("peer ancestry visited allocation failed")
        })?;
        frontier.try_reserve(1).map_err(|_| {
            KfdRuntimeBackendV1::capacity("peer ancestry frontier allocation failed")
        })?;
        visited.insert(id);
        frontier.push(id);
        Ok(())
    }

    pub(super) fn validate_peer_launch_ancestry_v1(
        &mut self,
        ancestry: &PeerLaunchAncestryV1,
    ) -> Result<(), Failure> {
        if self.peer_launch_ancestry_is_intact_v1(ancestry) {
            Ok(())
        } else {
            Err(self.directed_corruption_v1())
        }
    }

    pub(super) fn peer_launch_ancestry_is_intact_v1(
        &self,
        ancestry: &PeerLaunchAncestryV1,
    ) -> bool {
        if ancestry.owner == 0
            || !self.streams.contains_key(&ancestry.stream)
            || ancestry.nodes.len() > MAX_PEER_LAUNCH_ANCESTORS_V1
            || ancestry.dependencies.len() > MAX_PEER_LAUNCH_EDGES_V1
            || ancestry.success_roots.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || ancestry
                .success_roots
                .iter()
                .copied()
                .chain(ancestry.ordered)
                .any(|id| !ancestry.contains(id))
            || !ancestry
                .nodes
                .windows(2)
                .all(|pair| pair[0].id < pair[1].id)
        {
            return false;
        }
        let mut sorted_roots = [0; MAX_RUNTIME_DEPENDENCIES_V1];
        sorted_roots[..ancestry.success_roots.len()].copy_from_slice(&ancestry.success_roots);
        let sorted_roots = &mut sorted_roots[..ancestry.success_roots.len()];
        sorted_roots.sort_unstable();
        if sorted_roots.windows(2).any(|pair| pair[0] == pair[1])
            || ancestry.ordered.is_some_and(|id| {
                !matches!(self.submissions.get(&id),
                Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.stream == ancestry.stream)
            })
        {
            return false;
        }
        let mut edge_work = 0_usize;
        let mut depth = 1_usize;
        for node in &ancestry.nodes {
            let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&node.id)
            else {
                return false;
            };
            if copy.directed.is_some() && !self.directed_identity_is_intact_v1(node.id) {
                return false;
            }
            let Some(original) = ancestry.dependencies.get(node.dependencies.clone()) else {
                return false;
            };
            let implicit = node.prior.filter(|prior| {
                !original
                    .iter()
                    .any(|entry| entry.producer_submission == *prior)
            });
            let Some(next_edge_work) = edge_work
                .checked_add(original.len())
                .and_then(|count| count.checked_add(usize::from(implicit.is_some())))
                .filter(|count| *count <= MAX_PEER_LAUNCH_EDGES_V1)
            else {
                return false;
            };
            edge_work = next_edge_work;
            if node.id == 0
                || node.id >= ancestry.owner
                || node.stream != copy.stream
                || node.source != copy.source
                || node.destination != copy.destination
                || node.source_region != copy.source_region
                || node.destination_region != copy.destination_region
                || node.depth != copy.dependency_depth
                || node.prior != copy.prior_stream_submission
                || node.directed_extents != copy.directed.as_ref().map(|root| root.extents())
                || original
                    != copy
                        .directed
                        .as_ref()
                        .map_or(&[][..], |root| root.dependencies())
                || node
                    .terminal
                    .is_some_and(|status| !copy.is_quiescent() || copy.status() != status)
                || !copy.is_quiescent()
                    && !original
                        .iter()
                        .map(|entry| entry.producer_submission)
                        .chain(implicit)
                        .eq(copy.dependencies.iter().copied())
            {
                return false;
            }
            if sorted_roots.binary_search(&node.id).is_ok() {
                let producer_depth = if node.directed_extents.is_some() {
                    node.depth
                } else {
                    1
                };
                let Some(next) = producer_depth.checked_add(1) else {
                    return false;
                };
                depth = depth.max(next);
            }
        }
        depth == ancestry.depth && depth <= MAX_COOPERATIVE_COPY_DEPENDENCY_DEPTH_V1
    }

    pub(super) fn admit_peer_ancestry_bindings_v1(
        &mut self,
        ancestry: &PeerLaunchAncestryV1,
        child: usize,
        bindings: &[BackendBindingV1],
    ) -> Result<(), Failure> {
        self.validate_peer_launch_ancestry_v1(ancestry)?;
        for binding in bindings {
            let endpoint = RoutedHandleV1 {
                child,
                local: binding.region.allocation,
            };
            let Some(owners) = self.cooperative_allocation_owners.get(&endpoint) else {
                continue;
            };
            if owners.len() > MAX_RUNTIME_ALLOCATION_CUSTODY_OWNERS_V1 {
                return Err(KfdRuntimeBackendV1::capacity(
                    "peer binding owner capacity exceeded",
                ));
            }
            if !self.directed_owner_roster_is_intact_v1(endpoint) {
                return Err(self.directed_corruption_v1());
            }
            if owners.iter().any(|id| !ancestry.contains(*id)) {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::Busy,
                    "peer launch binding has an unrelated cooperative owner",
                ));
            }
        }
        Ok(())
    }
}
