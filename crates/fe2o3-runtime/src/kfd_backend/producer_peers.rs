//! Router-owned predecessor custody, independent of child completion storage.

use super::*;

#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub(super) struct PeerLaunchRetainsV1 {
    consumers: HashMap<u64, Vec<u64>>,
    producers: HashMap<u64, usize>,
    ancestries: HashMap<u64, PeerLaunchAncestryV1>,
}

impl PeerLaunchRetainsV1 {
    pub(super) fn is_empty(&self) -> bool {
        self.consumers.is_empty() && self.producers.is_empty() && self.ancestries.is_empty()
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
        self.consumers.insert(consumer, producers);
        for producer in &self.consumers[&consumer] {
            *self.producers.entry(*producer).or_insert(0) += 1;
        }
    }

    pub(super) fn release(&mut self, consumer: u64) {
        self.ancestries.remove(&consumer);
        if let Some(producers) = self.consumers.remove(&consumer) {
            for producer in producers {
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
        self.with_peer_launch_custody_v1(id, ids, |backend| {
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
        let producers = &mut self.peer_launch_retains.producers;
        let ancestries = &mut self.peer_launch_retains.ancestries;
        self.peer_launch_retains
            .consumers
            .retain(|id, dependencies| {
                let settled = match self.submissions.get(id) {
                    Some(RoutedSubmissionV1::Native {
                        route,
                        stream: owner,
                    }) if *owner == stream => {
                        Self::child_launch_is_quiescent_v1(&self.children[route.child], route.local)
                    }
                    _ => false,
                };
                if settled {
                    ancestries.remove(id);
                    for producer in dependencies {
                        Self::decrement_indexed_count(
                            producers,
                            *producer,
                            "flushed peer launch retains producer",
                        );
                    }
                }
                !settled
            });
    }
}
