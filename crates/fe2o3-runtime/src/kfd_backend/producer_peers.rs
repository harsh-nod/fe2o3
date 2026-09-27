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
    routes: HashMap<RoutedHandleV1, u64>,
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
            && self.routes.is_empty()
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

    fn release_links_are_intact(&self, consumer: u64) -> bool {
        let Some(entry) = self.consumers.get(&consumer) else {
            return !self.ancestries.contains_key(&consumer);
        };
        entry
            .producers
            .iter()
            .all(|id| self.producers.get(id).is_some_and(|count| *count != 0))
            && match entry.route {
                Some(route) => {
                    self.routes.get(&route) == Some(&consumer)
                        && self.ancestries.get(&consumer).is_some_and(|ancestry| {
                            ancestry.owner() == consumer
                                && Some(ancestry.stream()) == entry.stream
                                && entry.producers.iter().copied().eq(ancestry.producers())
                        })
                }
                None => !self.ancestries.contains_key(&consumer),
            }
    }

    pub(super) fn can_release(&self, consumer: u64) -> bool {
        self.release_links_are_intact(consumer)
            && self
                .consumers
                .get(&consumer)
                .and_then(|entry| entry.stream)
                .is_none_or(|stream| {
                    self.streams
                        .get(&stream)
                        .is_some_and(|ids| ids.iter().filter(|id| **id == consumer).count() == 1)
                })
    }

    pub(super) fn matches_native_route(
        &self,
        consumer: u64,
        route: RoutedHandleV1,
        stream: u64,
    ) -> bool {
        match self.consumers.get(&consumer) {
            Some(entry) => {
                entry.route.is_none_or(|actual| actual == route)
                    && entry.stream.is_none_or(|actual| actual == stream)
            }
            None => {
                !self.routes.contains_key(&route)
                    && self
                        .streams
                        .get(&stream)
                        .is_none_or(|ids| !ids.contains(&consumer))
                    && !self.ancestries.contains_key(&consumer)
            }
        }
    }

    pub(super) fn release(&mut self, consumer: u64) -> bool {
        if !self.can_release(consumer) {
            return false;
        }
        self.ancestries.remove(&consumer);
        if let Some(entry) = self.consumers.remove(&consumer) {
            if let Some(route) = entry.route {
                self.routes.remove(&route);
            }
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
        true
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    fn native_peer_prefix_v1(
        &mut self,
        child_index: usize,
        roots: impl Iterator<Item = u64>,
    ) -> Result<Vec<(u64, u64)>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let result = self.children[child_index].native_dependency_prefix_v1(roots);
        self.latch(result)
    }

    pub(super) fn inherited_peer_launch_roots_v1(
        &mut self,
        stream: RoutedHandleV1,
        collected: &CollectedComputeDependenciesV1,
    ) -> Result<Vec<u64>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self.peer_launch_retains.is_empty()
            && !self.children[stream.child].has_admitted_peer_gate
        {
            if collected
                .explicit_success_dependencies
                .iter()
                .copied()
                .chain(collected.ordered_predecessor)
                .any(|id| {
                    self.children[stream.child]
                        .pending_compute
                        .get(&id)
                        .is_some_and(|pending| pending.peer_gate.is_some())
                })
            {
                return Err(self.directed_corruption_v1());
            }
            return Ok(Vec::new());
        }
        let prefix = self.native_peer_prefix_v1(
            stream.child,
            collected
                .explicit_success_dependencies
                .iter()
                .copied()
                .chain(collected.ordered_predecessor),
        )?;
        let mut roots = HashSet::new();
        let mut work = 0_usize;
        for (id, local_stream) in prefix {
            if Some(id) == collected.ordered_predecessor && local_stream != stream.local {
                return Err(self.directed_corruption_v1());
            }
            let route = RoutedHandleV1 {
                child: stream.child,
                local: id,
            };
            let gate = self.children[stream.child]
                .pending_compute
                .get(&id)
                .and_then(|pending| pending.peer_gate);
            let Some(&owner) = self.peer_launch_retains.routes.get(&route) else {
                if gate.is_some() {
                    return Err(self.directed_corruption_v1());
                }
                continue;
            };
            let Some(ancestry) = self.peer_launch_retains.ancestries.get(&owner) else {
                return Err(self.directed_corruption_v1());
            };
            // Bound cumulative snapshot work, including repeated shared closures.
            work = work
                .checked_add(ancestry.import_work())
                .filter(|work| *work <= peer_ancestry::MAX_PEER_LAUNCH_EDGES_V1)
                .ok_or_else(|| {
                    KfdRuntimeBackendV1::capacity("inherited peer snapshot work capacity exceeded")
                })?;
            if ancestry.owner() != owner
                || !self.peer_launch_ancestry_is_intact_v1(ancestry)
                || !matches!(self.submissions.get(&owner), Some(RoutedSubmissionV1::Native { route: actual, stream }) if *actual == route && *stream == ancestry.stream())
                || self.streams.get(&ancestry.stream())
                    != Some(&RoutedHandleV1 {
                        child: stream.child,
                        local: local_stream,
                    })
                || self
                    .peer_launch_retains
                    .consumers
                    .get(&owner)
                    .is_none_or(|entry| {
                        entry.route != Some(route)
                            || entry.stream != Some(ancestry.stream())
                            || !entry.producers.iter().copied().eq(ancestry.producers())
                    })
                || gate.is_some_and(|gate| !gate.owns(owner, id))
                || ancestry
                    .producers()
                    .any(|producer| !self.peer_launch_retains.retains(producer))
                || self.children[stream.child]
                    .pending_compute
                    .contains_key(&id)
                    && gate.is_none()
                || gate.is_none()
                    && !Self::child_launch_is_quiescent_v1(&self.children[stream.child], id)
                    && !(self.children[stream.child]
                        .active_compute_lane_v1(id)
                        .is_some()
                        && ancestry.state(self) == (PeerComputeResultV1::Succeeded, true))
            {
                return Err(self.directed_corruption_v1());
            }
            for producer in ancestry.producers() {
                if !roots.contains(&producer) {
                    if roots.len() == peer_ancestry::MAX_PEER_LAUNCH_ANCESTORS_V1 {
                        return Err(KfdRuntimeBackendV1::capacity(
                            "inherited peer root capacity exceeded",
                        ));
                    }
                    roots.try_reserve(1).map_err(|_| {
                        KfdRuntimeBackendV1::capacity("inherited peer root index allocation failed")
                    })?;
                    roots.insert(producer);
                }
            }
        }
        let mut sorted = Vec::new();
        sorted
            .try_reserve_exact(roots.len())
            .map_err(|_| KfdRuntimeBackendV1::capacity("inherited peer root allocation failed"))?;
        sorted.extend(roots);
        sorted.sort_unstable();
        Ok(sorted)
    }

    /// Walk only the retained native prefix. FIFO edges may be deeper than the
    /// explicit dependency limit, so bound the traversal by the submission cap.
    pub(super) fn service_native_peer_prefix_v1(
        &mut self,
        route: RoutedHandleV1,
        execute: bool,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let no_peer_custody = self.peer_launch_retains.is_empty();
        if no_peer_custody && !self.children[route.child].has_admitted_peer_gate {
            if self.children[route.child]
                .pending_compute
                .get(&route.local)
                .is_some_and(|pending| pending.peer_gate.is_some())
            {
                return Err(self.directed_corruption_v1());
            }
            return Ok(());
        }
        let order = self.native_peer_prefix_v1(route.child, core::iter::once(route.local))?;
        if no_peer_custody {
            // A retired peer history must not change native-only progress policy.
            if order.iter().any(|(id, _)| {
                self.children[route.child]
                    .pending_compute
                    .get(id)
                    .is_some_and(|pending| pending.peer_gate.is_some())
            }) {
                return Err(self.directed_corruption_v1());
            }
            return Ok(());
        }
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
        if self.peer_launch_retains.routes.contains_key(&route) {
            return Err(self.directed_corruption_v1());
        }
        self.peer_launch_retains
            .routes
            .try_reserve(1)
            .map_err(|_| KfdRuntimeBackendV1::capacity("peer launch route index growth failed"))?;
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
            backend.peer_launch_retains.routes.insert(route, id);
            let local = submit(backend)?;
            if local != route.local {
                return Err(backend.directed_corruption_v1());
            }
            Ok(local)
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
                ) && !self.peer_launch_retains.release(id)
                {
                    return Err(self.directed_corruption_v1());
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
        if !self.peer_launch_release_route_is_intact_v1(id) {
            return Err(self.directed_corruption_v1());
        }
        if self.peer_launch_retains.consumers.contains_key(&id)
            && match &result {
                Ok(value) => quiescent(value),
                Err(RuntimeBackendFailureV1::Quiescent(_)) => self.peer_launch_is_quiescent_v1(id),
                _ => false,
            }
            && !self.peer_launch_retains.release(id)
        {
            return Err(self.directed_corruption_v1());
        }
        self.latch(result)
    }

    fn peer_launch_release_route_is_intact_v1(&self, id: u64) -> bool {
        match self.submissions.get(&id) {
            Some(RoutedSubmissionV1::Native { route, stream }) => self
                .peer_launch_retains
                .matches_native_route(id, *route, *stream),
            _ => true,
        }
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

    pub(super) fn retire_flushed_peer_launches_v1(
        &mut self,
        stream: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        if self
            .peer_launch_retains
            .streams
            .get(&stream)
            .is_some_and(|ids| {
                !ids.windows(2).all(|pair| pair[0] < pair[1])
                    || ids.iter().any(|id| {
                        !self.peer_launch_retains.release_links_are_intact(*id)
                            || !self.peer_launch_release_route_is_intact_v1(*id)
                            || self
                                .peer_launch_retains
                                .consumers
                                .get(id)
                                .is_none_or(|entry| entry.stream != Some(stream))
                    })
            })
        {
            return Err(self.directed_corruption_v1());
        }
        let PeerLaunchRetainsV1 {
            consumers,
            producers,
            ancestries,
            streams,
            routes,
        } = &mut self.peer_launch_retains;
        let Some(ids) = streams.get_mut(&stream) else {
            return Ok(());
        };
        ids.retain(|id| {
            let settled = matches!(self.submissions.get(id), Some(RoutedSubmissionV1::Native { route, stream: owner })
                if *owner == stream && Self::child_launch_is_quiescent_v1(&self.children[route.child], route.local));
            if settled {
                ancestries.remove(id);
                let entry = consumers.remove(id).expect("stream index retains consumer");
                if let Some(route) = entry.route {
                    routes.remove(&route);
                }
                for producer in entry.producers {
                    Self::decrement_indexed_count(producers, producer, "flushed peer launch retains producer");
                }
            }
            !settled
        });
        if ids.is_empty() {
            streams.remove(&stream);
        }
        Ok(())
    }

    pub(super) fn refresh_peer_launch_gate_v1(
        &mut self,
        id: u64,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(ancestry) = self.peer_launch_retains.ancestries.get(&id) else {
            if let Some(RoutedSubmissionV1::Native { route, .. }) = self.submissions.get(&id)
                && (self.peer_launch_retains.routes.contains_key(route)
                    || self.children.get(route.child).is_none_or(|child| {
                        child
                            .pending_compute
                            .get(&route.local)
                            .is_some_and(|pending| pending.peer_gate.is_some())
                    }))
            {
                return Err(self.directed_corruption_v1());
            }
            return Ok(());
        };
        if ancestry.owner() != id || !self.peer_launch_ancestry_is_intact_v1(ancestry) {
            return Err(self.directed_corruption_v1());
        }
        let Some(&RoutedSubmissionV1::Native { route, stream }) = self.submissions.get(&id) else {
            return Err(self.directed_corruption_v1());
        };
        if stream != ancestry.stream()
            || self.peer_launch_retains.routes.get(&route) != Some(&id)
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
