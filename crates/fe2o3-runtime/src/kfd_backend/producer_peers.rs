//! Router-owned predecessor custody, independent of child completion storage.

use super::*;

#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub(super) struct PeerLaunchRetainsV1 {
    consumers: HashMap<u64, PeerLaunchConsumerV1>,
    producers: HashMap<u64, usize>,
    ancestries: HashMap<u64, PeerLaunchAncestryV1>,
    streams: HashMap<u64, Vec<u64>>,
}

#[derive(Debug)]
struct PeerLaunchConsumerV1 {
    producers: Vec<u64>,
    stream: Option<u64>,
    route: Option<RoutedHandleV1>,
}

impl PeerLaunchRetainsV1 {
    pub(super) fn is_empty(&self) -> bool {
        self.consumers.is_empty()
            && self.producers.is_empty()
            && self.ancestries.is_empty()
            && self.streams.is_empty()
    }

    pub(super) fn retains(&self, producer: u64) -> bool {
        self.producers.contains_key(&producer)
    }

    pub(super) fn prepare(
        &mut self,
        producers: &[u64],
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if producers.is_empty() {
            return Ok(());
        }
        if producers
            .iter()
            .any(|id| self.producers.get(id) == Some(&usize::MAX))
        {
            return Err(KfdRuntimeBackendV1::capacity(
                "peer launch retain count overflow",
            ));
        }
        self.consumers.try_reserve(1).map_err(|_| {
            KfdRuntimeBackendV1::capacity("peer launch consumer custody growth failed")
        })?;
        self.producers.try_reserve(producers.len()).map_err(|_| {
            KfdRuntimeBackendV1::capacity("peer launch producer custody growth failed")
        })
    }

    fn acquire(&mut self, consumer: u64, producers: Vec<u64>) {
        if producers.is_empty() {
            return;
        }
        assert!(!self.consumers.contains_key(&consumer));
        self.consumers.insert(
            consumer,
            PeerLaunchConsumerV1 {
                producers,
                stream: None,
                route: None,
            },
        );
        for producer in &self.consumers[&consumer].producers {
            *self.producers.entry(*producer).or_insert(0) += 1;
        }
    }

    fn prepare_stream(
        &mut self,
        stream: u64,
    ) -> Result<Vec<u64>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if let Some(consumers) = self.streams.get_mut(&stream) {
            consumers
                .try_reserve(1)
                .map_err(|_| KfdRuntimeBackendV1::capacity("peer stream roster growth failed"))?;
            return Ok(Vec::new());
        }
        self.streams
            .try_reserve(1)
            .map_err(|_| KfdRuntimeBackendV1::capacity("peer stream index growth failed"))?;
        let mut consumers = Vec::new();
        consumers
            .try_reserve(1)
            .map_err(|_| KfdRuntimeBackendV1::capacity("peer stream roster allocation failed"))?;
        Ok(consumers)
    }

    fn acquire_stream(&mut self, consumer: u64, stream: u64, prepared: Vec<u64>) {
        self.consumers
            .get_mut(&consumer)
            .expect("peer consumer acquired before indexing")
            .stream = Some(stream);
        self.streams
            .entry(stream)
            .or_insert(prepared)
            .push(consumer);
    }

    pub(super) fn release(&mut self, consumer: u64) {
        self.ancestries.remove(&consumer);
        if let Some(entry) = self.consumers.remove(&consumer) {
            if let Some(stream) = entry.stream {
                let ids = self
                    .streams
                    .get_mut(&stream)
                    .expect("peer stream retains consumer");
                ids.retain(|id| *id != consumer);
                if ids.is_empty() {
                    self.streams.remove(&stream);
                }
            }
            for producer in entry.producers {
                KfdMultiDeviceRuntimeBackendV1::decrement_indexed_count(
                    &mut self.producers,
                    producer,
                    "peer launch producer remains retained",
                );
            }
        }
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    /// Walk only the retained native prefix. FIFO edges may be deeper than the
    /// explicit dependency limit, so bound the traversal by the submission cap.
    pub(super) fn service_native_peer_prefix_v1(
        &mut self,
        route: RoutedHandleV1,
        execute: bool,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.peer_launch_retains.is_empty() {
            return Ok(());
        }
        let mut visited = HashSet::new();
        let mut frontier = Vec::new();
        let mut order = Vec::new();
        let queue = |id, visited: &mut HashSet<u64>, frontier: &mut Vec<u64>| {
            if visited.contains(&id) {
                return Ok(());
            }
            if visited.len() >= MAX_RUNTIME_SUBMISSIONS_V1 {
                return Err(KfdRuntimeBackendV1::capacity(
                    "native peer prefix capacity exceeded",
                ));
            }
            visited.try_reserve(1).map_err(|_| {
                KfdRuntimeBackendV1::capacity("native peer prefix index allocation failed")
            })?;
            frontier.try_reserve(1).map_err(|_| {
                KfdRuntimeBackendV1::capacity("native peer prefix frontier allocation failed")
            })?;
            visited.insert(id);
            frontier.push(id);
            Ok(())
        };
        queue(route.local, &mut visited, &mut frontier)?;
        while let Some(id) = frontier.pop() {
            let child = &self.children[route.child];
            let (stream, dependencies, prior) =
                if let Some(pending) = child.pending_compute.get(&id) {
                    (
                        pending.launch.stream,
                        pending.explicit_success_dependencies.as_ref(),
                        pending.ordered_predecessor,
                    )
                } else if let Some(active) = child.active_sdma.get(&id) {
                    (
                        active.stream,
                        active.dependencies.as_slice(),
                        active.prior_stream_submission,
                    )
                } else {
                    continue;
                };
            order.try_reserve(1).map_err(|_| {
                KfdRuntimeBackendV1::capacity("native peer prefix schedule allocation failed")
            })?;
            order.push((id, stream));
            for predecessor in dependencies.iter().copied().chain(prior) {
                if predecessor == 0 || predecessor >= id {
                    return Err(self.directed_corruption_v1());
                }
                queue(predecessor, &mut visited, &mut frontier)?;
            }
        }
        order.sort_unstable_by_key(|(id, _)| *id);
        for (id, stream) in order {
            let gate = self.children[route.child]
                .pending_compute
                .get(&id)
                .and_then(|pending| pending.peer_gate);
            if let Some(gate) = gate {
                let owner = gate.router_owner();
                if !matches!(self.submissions.get(&owner), Some(RoutedSubmissionV1::Native { route: actual, .. }) if *actual == RoutedHandleV1 { child: route.child, local: id })
                {
                    return Err(self.directed_corruption_v1());
                }
                if execute {
                    self.progress_peer_launch_once_v1(owner)?;
                } else {
                    self.refresh_peer_launch_gate_v1(owner)?;
                }
            }
            if execute && !Self::child_launch_is_quiescent_v1(&self.children[route.child], id) {
                match self.children[route.child].poll_v1(id) {
                    Ok(BackendPollV1::Failed { .. }) if id == route.local => {
                        return Err(KfdRuntimeBackendV1::quiescent_error(
                            KfdRuntimeBackendErrorKindV1::Native,
                            "native peer-prefix target failed before publication",
                        ));
                    }
                    Ok(_) => {}
                    Err(RuntimeBackendFailureV1::Quiescent(_))
                        if id != route.local
                            && Self::child_launch_is_quiescent_v1(
                                &self.children[route.child],
                                id,
                            ) => {}
                    Err(error) => return self.latch(Err(error)),
                }
                let child = &self.children[route.child];
                let compute = child
                    .pending_compute_streams
                    .get(&stream)
                    .and_then(|ids| ids.front())
                    .copied();
                let sdma = child
                    .active_sdma_streams
                    .get(&stream)
                    .and_then(|ids| ids.front())
                    .copied()
                    .filter(|id| {
                        child
                            .active_sdma
                            .get(id)
                            .is_some_and(|copy| matches!(copy.phase, ActiveSdmaPhaseV1::Ready))
                    });
                // A prior observation may have published/settled this entry.
                // Never flush a later stream head on its behalf.
                if compute.into_iter().chain(sdma).min() != Some(id) {
                    continue;
                }
                if child.pending_compute.get(&id).is_some_and(|pending| {
                    child
                        .compute_stream_head_publication_blocker_v1(pending)
                        .is_some()
                }) {
                    continue;
                }
                let result = self.children[route.child].flush_stream_v1(stream);
                match result {
                    Ok(()) => {}
                    Err(RuntimeBackendFailureV1::Quiescent(_))
                        if id != route.local
                            && Self::child_launch_is_quiescent_v1(
                                &self.children[route.child],
                                id,
                            ) => {}
                    Err(error) => return self.latch(Err(error)),
                }
            }
        }
        Ok(())
    }

    pub(super) fn with_peer_launch_ancestry_v1(
        &mut self,
        id: u64,
        ancestry: Option<PeerLaunchAncestryV1>,
        submit: impl FnOnce(&mut Self) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(ancestry) = ancestry else {
            return self.with_peer_launch_custody_v1(id, Vec::new(), submit);
        };
        if ancestry.owner() != id || self.peer_launch_retains.ancestries.contains_key(&id) {
            return Err(self.directed_corruption_v1());
        }
        self.validate_peer_launch_ancestry_v1(&ancestry)?;
        let ids = ancestry.retained_ids()?;
        if ids.is_empty() {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "peer ancestry custody requires a predecessor",
            ));
        }
        self.peer_launch_retains.prepare(&ids)?;
        self.peer_launch_retains
            .ancestries
            .try_reserve(1)
            .map_err(|_| {
                KfdRuntimeBackendV1::capacity("peer launch ancestry custody growth failed")
            })?;
        let stream = ancestry.stream();
        let prepared = self.peer_launch_retains.prepare_stream(stream)?;
        let child = self.streams[&stream].child;
        let route = RoutedHandleV1 {
            child,
            local: self.children[child].next_handle,
        };
        self.with_peer_launch_custody_v1(id, ids, |backend| {
            backend
                .peer_launch_retains
                .acquire_stream(id, stream, prepared);
            backend
                .peer_launch_retains
                .consumers
                .get_mut(&id)
                .expect("acquired consumer")
                .route = Some(route);
            backend.peer_launch_retains.ancestries.insert(id, ancestry);
            submit(backend)
        })
    }

    pub(super) fn with_peer_launch_custody_v1(
        &mut self,
        id: u64,
        producers: Vec<u64>,
        submit: impl FnOnce(&mut Self) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
    ) -> Result<u64, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if producers.is_empty() {
            let result = submit(self);
            return self.latch(result);
        }
        // Custody precedes child entry, including publication followed by unwind.
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.peer_launch_retains.acquire(id, producers);
            submit(self)
        }));
        match result {
            Ok(result) => {
                if matches!(
                    result,
                    Err(RuntimeBackendFailureV1::Rejected(_)
                        | RuntimeBackendFailureV1::Quiescent(_))
                ) {
                    self.peer_launch_retains.release(id);
                }
                self.latch(result)
            }
            Err(payload) => {
                self.terminal = true;
                std::panic::resume_unwind(payload)
            }
        }
    }

    pub(super) fn observe_peer_launch_result_v1<T>(
        &mut self,
        id: u64,
        result: Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>,
        quiescent: impl FnOnce(&T) -> bool,
    ) -> Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.peer_launch_retains.consumers.contains_key(&id)
            && match &result {
                Ok(value) => quiescent(value),
                Err(RuntimeBackendFailureV1::Quiescent(_)) => self.peer_launch_is_quiescent_v1(id),
                _ => false,
            }
        {
            self.peer_launch_retains.release(id);
        }
        self.latch(result)
    }

    fn peer_launch_is_quiescent_v1(&self, id: u64) -> bool {
        let Some(RoutedSubmissionV1::Native { route, .. }) = self.submissions.get(&id) else {
            return false;
        };
        Self::child_launch_is_quiescent_v1(&self.children[route.child], route.local)
    }

    fn child_launch_is_quiescent_v1(child: &KfdRuntimeBackendV1, id: u64) -> bool {
        child
            .submissions
            .get(&id)
            .is_some_and(|record| record.status != BackendPollV1::Pending)
            && !child.pending_compute.contains_key(&id)
            && !child.active_sdma.contains_key(&id)
            && child.active_compute_lane_v1(id).is_none()
    }

    pub(super) fn retire_flushed_peer_launches_v1(&mut self, stream: u64) {
        let PeerLaunchRetainsV1 {
            consumers,
            producers,
            ancestries,
            streams,
        } = &mut self.peer_launch_retains;
        let Some(ids) = streams.get_mut(&stream) else {
            return;
        };
        ids.retain(|id| {
            let settled = matches!(self.submissions.get(id), Some(RoutedSubmissionV1::Native { route, stream: owner })
                if *owner == stream && Self::child_launch_is_quiescent_v1(&self.children[route.child], route.local));
            if settled {
                ancestries.remove(id);
                let entry = consumers.remove(id).expect("stream index retains consumer");
                for producer in entry.producers {
                    Self::decrement_indexed_count(producers, producer, "flushed peer launch retains producer");
                }
            }
            !settled
        });
        if ids.is_empty() {
            streams.remove(&stream);
        }
    }

    pub(super) fn refresh_peer_launch_gate_v1(
        &mut self,
        id: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(ancestry) = self.peer_launch_retains.ancestries.get(&id) else {
            return Ok(());
        };
        if ancestry.owner() != id || !self.peer_launch_ancestry_is_intact_v1(ancestry) {
            return Err(self.directed_corruption_v1());
        }
        let Some(&RoutedSubmissionV1::Native { route, stream }) = self.submissions.get(&id) else {
            return Err(self.directed_corruption_v1());
        };
        if stream != ancestry.stream()
            || self
                .peer_launch_retains
                .consumers
                .get(&id)
                .is_none_or(|entry| entry.route != Some(route) || entry.stream != Some(stream))
            || self
                .streams
                .get(&stream)
                .is_none_or(|owner| owner.child != route.child)
        {
            return Err(self.directed_corruption_v1());
        }
        let (result, ordered) = ancestry.state(self);
        let Some(pending) = self.children[route.child]
            .pending_compute
            .get_mut(&route.local)
        else {
            if (result == PeerComputeResultV1::Succeeded
                && ordered
                && self.children[route.child]
                    .active_compute_lane_v1(route.local)
                    .is_some())
                || Self::child_launch_is_quiescent_v1(&self.children[route.child], route.local)
            {
                return Ok(());
            }
            return Err(self.directed_corruption_v1());
        };
        let resolved = pending
            .peer_gate
            .and_then(|gate| gate.resolve(id, route.local, result, ordered).ok());
        let Some(gate) = resolved else {
            return Err(self.directed_corruption_v1());
        };
        pending.peer_gate = Some(gate);
        Ok(())
    }

    pub(super) fn progress_peer_launch_once_v1(
        &mut self,
        id: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.refresh_peer_launch_gate_v1(id)?;
        let Some(ancestry) = self.peer_launch_retains.ancestries.get(&id) else {
            return Ok(());
        };
        let (result, ordered) = ancestry.state(self);
        if result != PeerComputeResultV1::Pending && ordered {
            return Ok(());
        }
        let selected = ancestry.roots().find(|root| matches!(self.submissions.get(root), Some(RoutedSubmissionV1::CooperativeCopy(copy)) if !copy.is_quiescent()));
        if let Some(root) = selected {
            match self.progress_retained_directed_peer_v1(root) {
                Ok(_) => {}
                Err(RuntimeBackendFailureV1::Quiescent(_)) if matches!(self.submissions.get(&root), Some(RoutedSubmissionV1::CooperativeCopy(copy)) if copy.is_quiescent()) =>
                    {}
                Err(error) => return self.latch(Err(error)),
            }
        }
        self.refresh_peer_launch_gate_v1(id)
    }

    pub(super) fn flush_peer_launch_roots_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let mut index = 0;
        while let Some(id) = self
            .peer_launch_retains
            .streams
            .get(&stream)
            .and_then(|ids| ids.get(index))
            .copied()
        {
            loop {
                let before = self.cooperative_progress_generation;
                self.progress_peer_launch_once_v1(id)?;
                if before == self.cooperative_progress_generation {
                    break;
                }
            }
            index += 1;
        }
        Ok(())
    }
}
