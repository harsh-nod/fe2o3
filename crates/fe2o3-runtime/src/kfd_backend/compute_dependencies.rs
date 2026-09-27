//! Native dependency reachability and independently retained buffer ordering.

use super::*;

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

impl KfdRuntimeBackendV1 {
    fn compute_custody_index_intact_v1(&self, allocation: u64) -> bool {
        let Some(custody) = self.allocation_custody.get(&allocation) else {
            return false;
        };
        let Some(first) = custody.owners.front() else {
            return false;
        };
        if custody.owners.len() > self.dispatch_capacity.custody_limit() {
            return false;
        }
        let mut counts = [0; 2];
        let mut previous = 0;
        let mut sole_stream = Some(first.stream);
        for entry in &custody.owners {
            if entry.submission <= previous {
                return false;
            }
            previous = entry.submission;
            counts[entry.kind.index()] += 1;
            if entry.stream != first.stream {
                sole_stream = None;
            }
        }
        custody.owner_counts == counts && custody.sole_stream == sole_stream
    }

    fn compute_sdma_stream_index_intact_v1(&self, stream: u64) -> bool {
        self.active_sdma_streams.get(&stream).is_some_and(|queue| {
            !queue.is_empty()
                && queue.len() <= MAX_RUNTIME_SUBMISSIONS_V1
                && queue.front().is_some_and(|id| *id != 0)
                && queue.iter().zip(queue.iter().skip(1)).all(|(a, b)| a < b)
        })
    }

    pub(super) fn native_sdma_binding_owner_intact_v1(
        &self,
        allocation: u64,
        owner: RuntimeAllocationCustodyOwnerV1,
    ) -> bool {
        self.native_sdma_binding_owner_with_v1(
            allocation,
            owner,
            |id| self.compute_custody_index_intact_v1(id),
            |stream| self.compute_sdma_stream_index_intact_v1(stream),
        )
    }

    fn native_sdma_binding_owner_with_v1(
        &self,
        allocation: u64,
        owner: RuntimeAllocationCustodyOwnerV1,
        mut custody_intact: impl FnMut(u64) -> bool,
        mut stream_intact: impl FnMut(u64) -> bool,
    ) -> bool {
        let Some(active) = self.active_sdma.get(&owner.submission) else {
            return false;
        };
        let Some(&device) = self.streams.get(&owner.stream) else {
            return false;
        };
        if owner.kind != RuntimeAllocationCustodyKindV1::Sdma
            || active.peer_access.is_some()
            || active.id == 0
            || active.id != owner.submission
            || active.stream != owner.stream
            || active.source == active.destination
            || ![active.source, active.destination].contains(&allocation)
            || active.byte_len == 0
            || active.completed_bytes >= active.byte_len
            || active.dependency_cursor > active.dependencies.len()
            || !stream_intact(owner.stream)
            || self.active_sdma_streams[&owner.stream]
                .binary_search(&active.id)
                .is_err()
        {
            return false;
        }
        let published = self
            .published_sdma_submissions
            .iter()
            .filter(|id| **id == active.id)
            .count();
        let is_published = !matches!(active.phase, ActiveSdmaPhaseV1::Ready);
        if published != usize::from(is_published)
            || if is_published {
                active.window_bytes == 0
                    || active.window_requests.is_none()
                    || active.window_bytes > active.byte_len - active.completed_bytes
            } else {
                active.window_bytes != 0 || active.window_requests.is_some()
            }
        {
            return false;
        }
        let endpoints = [
            (active.source, active.source_offset),
            (active.destination, active.destination_offset),
        ];
        if !endpoints.into_iter().all(|(id, offset)| {
            custody_intact(id)
                && self.allocation_custody[&id].owners
                    .binary_search_by_key(&owner.submission, |entry| entry.submission)
                    .is_ok_and(|index| self.allocation_custody[&id].owners[index] == owner)
                && self.allocations.get(&id).is_some_and(|record| {
                    record.device == device
                        && record.sdma_backed
                        && record.sdma_initialized
                        && !matches!(record.sdma_storage,
                            KfdRuntimeSdmaStorageV1::ComputeInFlight(actual) if actual == active.id)
                        && offset.checked_add(active.byte_len).is_some_and(|end| {
                            end <= record.bytes.len() as u64
                        })
                        && (matches!(record.sdma_storage,
                            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual))
                                if actual == active.id) == is_published)
                })
        }) {
            return false;
        }
        let kinds = (
            self.allocations[&active.source].kind,
            self.allocations[&active.destination].kind,
        );
        let directional = matches!(
            kinds,
            (
                RuntimeMemoryKindV1::HostVisible,
                RuntimeMemoryKindV1::DeviceLocal
            ) | (
                RuntimeMemoryKindV1::DeviceLocal,
                RuntimeMemoryKindV1::HostVisible
            )
        );
        let same_device = kinds
            == (
                RuntimeMemoryKindV1::DeviceLocal,
                RuntimeMemoryKindV1::DeviceLocal,
            );
        match active.phase {
            // A queued copy may still share storage with an earlier producer.
            ActiveSdmaPhaseV1::Ready => directional || same_device,
            ActiveSdmaPhaseV1::DirectionalPublished(_) => directional,
            ActiveSdmaPhaseV1::SameDevicePublished(_) => same_device,
        }
    }

    fn compute_quiescence_limit_v1(&self) -> usize {
        fe2o3_host_api::MAX_DISPATCH_BINDINGS_V1 * self.dispatch_capacity.custody_limit()
    }

    fn native_dependency_stream_v1(&self, id: u64) -> Option<u64> {
        self.pending_compute
            .get(&id)
            .map(|pending| pending.launch.stream)
            .or_else(|| self.active_sdma.get(&id).map(|active| active.stream))
            .or_else(|| {
                self.active_compute_submission_v1(id)
                    .map(|active| active.stream)
            })
            .or_else(|| self.submissions.get(&id).map(|record| record.stream))
    }

    pub(super) fn native_dependency_prefix_v1(
        &mut self,
        roots: impl Iterator<Item = u64>,
    ) -> Result<Vec<(u64, u64)>, Failure> {
        let mut visited = HashSet::new();
        let mut frontier = Vec::new();
        let mut order = Vec::new();
        let quiescence_limit = self.compute_quiescence_limit_v1();
        let queue = |id, visited: &mut HashSet<u64>, frontier: &mut Vec<u64>| {
            if visited.contains(&id) {
                return Ok(());
            }
            if visited.len() >= MAX_RUNTIME_SUBMISSIONS_V1 {
                return Err(Self::capacity("native dependency prefix capacity exceeded"));
            }
            visited
                .try_reserve(1)
                .map_err(|_| Self::capacity("native prefix index allocation failed"))?;
            frontier
                .try_reserve(1)
                .map_err(|_| Self::capacity("native prefix frontier allocation failed"))?;
            visited.insert(id);
            frontier.push(id);
            Ok(())
        };
        for id in roots {
            queue(id, &mut visited, &mut frontier)?;
        }
        while let Some(id) = frontier.pop() {
            let (stream, dependencies, prior, quiescence, retains) = if let Some(pending) =
                self.pending_compute.get(&id)
            {
                if pending.id != id {
                    return Err(self.terminal_error("native prefix lost pending compute identity"));
                }
                (
                    pending.launch.stream,
                    pending.explicit_success_dependencies.as_ref(),
                    pending.ordered_predecessor,
                    pending.quiescence_dependencies.as_ref(),
                    &self.compute_dependency_retain_counts,
                )
            } else if let Some(active) = self.active_sdma.get(&id) {
                if active.id != id {
                    return Err(self.terminal_error("native prefix lost SDMA identity"));
                }
                (
                    active.stream,
                    active.dependencies.as_slice(),
                    active.prior_stream_submission,
                    &[][..],
                    &self.sdma_dependency_retain_counts,
                )
            } else if let Some(active) = self.active_compute_submission_v1(id) {
                (
                    active.stream,
                    &[][..],
                    active
                        .ordered_predecessor
                        .filter(|_| active.deferred_ordered_predecessor_retain),
                    &[][..],
                    &self.compute_dependency_retain_counts,
                )
            } else if let Some(record) = self.submissions.get(&id) {
                (
                    record.stream,
                    &[][..],
                    None,
                    &[][..],
                    &self.compute_dependency_retain_counts,
                )
            } else {
                return Err(self.terminal_error("native prefix lost a retained submission"));
            };
            if id == 0
                || !self.streams.contains_key(&stream)
                || dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
                || dependencies
                    .iter()
                    .enumerate()
                    .any(|(index, id)| dependencies[..index].contains(id))
                || quiescence.len() > quiescence_limit
                || !quiescence.windows(2).all(|pair| pair[0] < pair[1])
                || quiescence
                    .iter()
                    .any(|id| dependencies.contains(id) || prior == Some(*id))
                || prior
                    .is_some_and(|prior| self.native_dependency_stream_v1(prior) != Some(stream))
            {
                return Err(
                    self.terminal_error("native prefix lost its immutable predecessor roster")
                );
            }
            order
                .try_reserve(1)
                .map_err(|_| Self::capacity("native prefix schedule allocation failed"))?;
            order.push((id, stream));
            for predecessor in dependencies
                .iter()
                .copied()
                .chain(prior)
                .chain(quiescence.iter().copied())
            {
                if predecessor == 0
                    || predecessor >= id
                    || retains.get(&predecessor).is_none_or(|count| *count == 0)
                {
                    return Err(
                        self.terminal_error("native prefix lost earlier predecessor custody")
                    );
                }
                queue(predecessor, &mut visited, &mut frontier)?;
            }
        }
        order.sort_unstable_by_key(|(id, _)| *id);
        Ok(order)
    }

    pub(super) fn capture_compute_quiescence_v1(
        &mut self,
        launch: &BackendLaunchV1<'_>,
        explicit: &[u64],
        ordered: Option<u64>,
    ) -> Result<Box<[u64]>, Failure> {
        if ordered
            .and_then(|id| self.native_dependency_stream_v1(id))
            .is_some_and(|stream| stream != launch.stream)
        {
            return Err(self.terminal_error("compute admission lost its exact FIFO stream"));
        }
        let mut candidates = Vec::new();
        let mut fifo_suffix = None;
        let mut checked_custody = HashSet::new();
        let mut checked_streams = HashSet::new();
        let quiescence_limit = self.compute_quiescence_limit_v1();
        for binding in launch.bindings {
            let allocation = binding.region.allocation;
            let Some(custody) = self.allocation_custody.get(&allocation) else {
                continue;
            };
            if custody.owners.len() > self.dispatch_capacity.custody_limit() {
                return Err(self.terminal_error("compute binding custody exceeded its owner bound"));
            }
            for owner in &custody.owners {
                if owner.kind == RuntimeAllocationCustodyKindV1::Sdma {
                    let Some(active) = self.active_sdma.get(&owner.submission) else {
                        return Err(self.terminal_error("compute binding lost its SDMA owner"));
                    };
                    // Routed peer DMA has separate private authority, not a native DAG edge.
                    if active.peer_access.is_some() {
                        continue;
                    }
                    // Shared indexes are validated once; exact membership then uses
                    // binary search instead of rescanning long queued-copy rosters.
                    checked_custody.try_reserve(2).map_err(|_| {
                        Self::capacity("SDMA custody validation index allocation failed")
                    })?;
                    checked_streams.try_reserve(1).map_err(|_| {
                        Self::capacity("SDMA stream validation index allocation failed")
                    })?;
                    if !self.native_sdma_binding_owner_with_v1(
                        allocation,
                        *owner,
                        |id| {
                            !checked_custody.insert(id) || self.compute_custody_index_intact_v1(id)
                        },
                        |stream| {
                            !checked_streams.insert(stream)
                                || self.compute_sdma_stream_index_intact_v1(stream)
                        },
                    ) {
                        return Err(self.terminal_error("compute binding lost exact SDMA custody"));
                    }
                }
                if Some(owner.submission) == ordered || explicit.contains(&owner.submission) {
                    continue;
                }
                let authentic = if owner.kind == RuntimeAllocationCustodyKindV1::Sdma {
                    true
                } else if let Some(pending) = self.pending_compute.get(&owner.submission) {
                    pending.id == owner.submission
                        && pending.launch.stream == owner.stream
                        && pending.retained_allocations.contains(&allocation)
                        && pending
                            .launch
                            .bindings
                            .iter()
                            .any(|binding| binding.region.allocation == allocation)
                } else {
                    self.active_compute_submission_v1(owner.submission)
                        .is_some_and(|active| {
                            active.id == owner.submission
                                && active.stream == owner.stream
                                && active.allocations.contains(&allocation)
                        })
                };
                if !authentic {
                    return Err(
                        self.terminal_error("transitive compute binding lost its exact owner")
                    );
                }
                if owner.kind == RuntimeAllocationCustodyKindV1::Compute
                    && owner.stream == launch.stream
                {
                    if self.pending_compute.contains_key(&owner.submission) {
                        let start = match fifo_suffix {
                            Some(start) => start,
                            None => {
                                let start = match self.pending_compute_fifo_suffix_v1(launch.stream, ordered) {
                                    Ok(start) => start,
                                    Err(detail) => return Err(self.terminal_error(detail)),
                                };
                                fifo_suffix = Some(start);
                                start
                            }
                        };
                        if self.pending_compute_streams[&launch.stream]
                            .binary_search(&owner.submission)
                            .is_ok_and(|index| index >= start)
                        {
                            continue;
                        }
                    } else if !self.allocations.get(&allocation).is_some_and(|record| {
                        matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(actual)
                            if actual == owner.submission)
                    }) {
                        continue;
                    }
                }
                if candidates.len() == quiescence_limit {
                    return Err(Self::capacity("compute quiescence owner capacity exceeded"));
                }
                candidates
                    .try_reserve(1)
                    .map_err(|_| Self::capacity("compute quiescence roster allocation failed"))?;
                candidates.push(owner.submission);
            }
        }
        // The ordinary/direct-dependency path neither allocates nor walks a DAG.
        if candidates.is_empty() {
            return Ok(Box::new([]));
        }
        candidates.sort_unstable();
        candidates.dedup();
        let prefix = self.native_dependency_prefix_v1(explicit.iter().copied().chain(ordered))?;
        if ordered.is_some_and(|id| {
            prefix
                .binary_search_by_key(&id, |(id, _)| *id)
                .ok()
                .is_none_or(|index| prefix[index].1 != launch.stream)
        }) {
            return Err(
                self.terminal_error("compute quiescence prefix names a foreign FIFO stream")
            );
        }
        if candidates
            .iter()
            .any(|id| prefix.binary_search_by_key(id, |(id, _)| *id).is_err())
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "overlapping compute has no retained native predecessor path",
            ));
        }
        Ok(candidates.into_boxed_slice())
    }

    fn pending_compute_fifo_suffix_v1(
        &self,
        stream: u64,
        mut tail: Option<u64>,
    ) -> Result<usize, &'static str> {
        let queue = self
            .pending_compute_streams
            .get(&stream)
            .ok_or("compute custody lost its pending stream queue")?;
        let mut start = queue.len();
        // Only uninterrupted pending-compute FIFO links prevent interior cancellation.
        // An SDMA or active/settled intermediate is a separate custody boundary.
        for (index, &id) in queue.iter().enumerate().rev() {
            if tail != Some(id) {
                break;
            }
            let pending = self
                .pending_compute
                .get(&id)
                .ok_or("compute FIFO suffix lost its pending owner")?;
            if id == 0
                || pending.id != id
                || pending.launch.stream != stream
                || pending.ordered_predecessor.is_some_and(|prior| {
                    prior == 0
                        || prior >= id
                        || self
                            .compute_dependency_retain_counts
                            .get(&prior)
                            .is_none_or(|count| *count == 0)
                })
            {
                return Err("compute FIFO suffix lost its exact predecessor custody");
            }
            start = index;
            tail = pending.ordered_predecessor;
        }
        Ok(start)
    }

    pub(super) fn observe_compute_quiescence_v1(
        &mut self,
        pending: PendingComputeSubmissionV1,
    ) -> Result<Option<PendingComputeSubmissionV1>, Failure> {
        self.observe_compute_quiescence_with_v1(pending, Self::poll_v1)
    }

    pub(super) fn observe_compute_quiescence_with_v1(
        &mut self,
        mut pending: PendingComputeSubmissionV1,
        mut poll: impl FnMut(&mut Self, u64) -> Result<BackendPollV1, Failure>,
    ) -> Result<Option<PendingComputeSubmissionV1>, Failure> {
        let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut polled = false;
            while let Some(&id) = pending
                .quiescence_dependencies
                .get(pending.quiescence_cursor)
            {
                if !self.exact_submission_quiescent_v1(id) {
                    if polled {
                        return Ok(false);
                    }
                    polled = true;
                    match poll(self, id) {
                        Ok(_) | Err(RuntimeBackendFailureV1::Quiescent(_)) => {}
                        Err(RuntimeBackendFailureV1::Rejected(error)) => {
                            return Err(self.terminal_error(format!(
                                "compute quiescence retained a rejected predecessor: {error}"
                            )));
                        }
                        Err(failure @ RuntimeBackendFailureV1::Terminal(_)) => return Err(failure),
                    }
                    if !self.exact_submission_quiescent_v1(id) {
                        return Ok(false);
                    }
                }
                pending.quiescence_cursor += 1;
            }
            Ok(true)
        }));
        match observed {
            Ok(Ok(true)) => Ok(Some(pending)),
            Ok(result) => {
                self.pending_compute.insert(pending.id, pending);
                result.map(|_| None)
            }
            Err(payload) => {
                self.pending_compute.insert(pending.id, pending);
                self.poison_terminal_v1();
                std::panic::resume_unwind(payload)
            }
        }
    }
}
