//! Native dependency reachability and independently retained buffer ordering.

use super::*;

type Failure = RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>;

impl KfdRuntimeBackendV1 {
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
        let mut candidates = Vec::new();
        let mut fifo_suffix = None;
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
                if owner.kind != RuntimeAllocationCustodyKindV1::Compute
                    || Some(owner.submission) == ordered
                    || explicit.contains(&owner.submission)
                {
                    continue;
                }
                let authentic = if let Some(pending) = self.pending_compute.get(&owner.submission) {
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
                if owner.stream == launch.stream {
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
