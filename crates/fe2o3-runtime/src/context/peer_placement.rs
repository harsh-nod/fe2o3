//! Bounded placement over actual Context allocations, never replica certification.

use super::*;
use crate::{MAX_RUNTIME_GRAPH_PEER_SHARDS_V1, RuntimeGraphPeerShardV1};

pub const MAX_RUNTIME_PEER_GATHER_DESTINATIONS_V1: usize = 8;

/// A snapshot of eligible transport, not proof of its eventual use or speed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendPeerCopyPlacementV1 {
    /// The retained route and original endpoint storage currently meet the
    /// native profile. Queue/currentness/capacity checks still run at submission.
    NativeXgmiCandidate,
    /// Existing bounded cooperative transport, including native endpoint scratch.
    HostStaged { peak_bytes: u64 },
}

/// One fixed source shard and its requested position in the eventual output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePeerGatherSourceV1 {
    pub node: crate::completion::CompletionNodeIdV1,
    pub source: RuntimeMemoryRegionV1,
    pub destination_offset: u64,
}

/// An original destination allocation and its existing stream, not a device UID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimePeerGatherDestinationV1 {
    pub stream: RuntimeStreamIdV1,
    pub allocation: RuntimeAllocationIdV1,
}

/// A deterministic inert choice. Binding this plan does not reserve resources.
/// Actual graph admission rechecks every selected region and route observation;
/// native submission retains all existing custody and currentness requirements.
/// Source versions remain the existing graph's InitialAtAdmission inputs. No
/// hashes, replica equivalence, cross-run versions or cross-device producers are
/// accepted. Costs are staged byte counts, not measured latency/bandwidth.
#[derive(Debug)]
pub struct RuntimePeerGatherPlacementV1 {
    selected: usize,
    stream: RuntimeStreamIdV1,
    shards: Vec<RuntimeGraphPeerShardV1>,
    observations: Vec<BackendPeerCopyPlacementV1>,
    staged_bytes: u64,
    peak_staging_bytes: u64,
}

impl RuntimePeerGatherPlacementV1 {
    pub const fn selected_index(&self) -> usize {
        self.selected
    }
    pub const fn stream(&self) -> RuntimeStreamIdV1 {
        self.stream
    }
    pub fn shards(&self) -> &[RuntimeGraphPeerShardV1] {
        &self.shards
    }
    pub fn observations(&self) -> &[BackendPeerCopyPlacementV1] {
        &self.observations
    }
    pub const fn staged_bytes(&self) -> u64 {
        self.staged_bytes
    }
    pub const fn peak_staging_bytes(&self) -> u64 {
        self.peak_staging_bytes
    }

    pub(crate) fn revalidate<B: RuntimeBackendV1>(
        &self,
        context: &RuntimeContextV1<B>,
    ) -> Result<(), RuntimeErrorV1<B::Error>> {
        context.require_graph_access(None)?;
        if self.shards.is_empty()
            || self.shards.len() != self.observations.len()
            || self.shards.len() > MAX_RUNTIME_GRAPH_PEER_SHARDS_V1
        {
            return Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch.into());
        }
        for (shard, expected) in self.shards.iter().zip(&self.observations) {
            if context.observe_gather_shard_v1(self.stream, *shard)? != Some(*expected) {
                return Err(RuntimeValidationErrorV1::Unsupported.into());
            }
        }
        Ok(())
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    /// Selects one complete feasible destination for 1..=8 fixed shards among
    /// 1..=8 original candidates. Least aggregate host-staged bytes wins; equal
    /// costs retain original candidate order. A candidate is considered only
    /// after every shard has a valid original region and a bounded route quote.
    /// No native allocation, queue operation, reservation or copy is performed.
    ///
    /// This is a snapshot, not a promise of later admission. Busy/changed routes
    /// and native capacity/currentness can still refuse the eventual operation.
    /// The selected graph remains serial on its one destination stream.
    pub fn select_peer_gather_destination_v1(
        &self,
        sources: &[RuntimePeerGatherSourceV1],
        destinations: &[RuntimePeerGatherDestinationV1],
    ) -> Result<RuntimePeerGatherPlacementV1, RuntimeErrorV1<B::Error>> {
        self.require_graph_access(None)?;
        if sources.is_empty()
            || sources.len() > MAX_RUNTIME_GRAPH_PEER_SHARDS_V1
            || destinations.is_empty()
            || destinations.len() > MAX_RUNTIME_PEER_GATHER_DESTINATIONS_V1
        {
            return Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch.into());
        }
        for (index, shard) in sources.iter().enumerate() {
            let allocation = self
                .allocations
                .get(&shard.source.allocation)
                .ok_or(RuntimeValidationErrorV1::UnknownAllocation)?;
            let source_end = shard
                .source
                .byte_offset
                .checked_add(shard.source.byte_len)
                .ok_or(RuntimeValidationErrorV1::InvalidRange)?;
            let end = shard
                .destination_offset
                .checked_add(shard.source.byte_len)
                .ok_or(RuntimeValidationErrorV1::InvalidRange)?;
            if shard.source.access != RuntimeAccessV1::Read
                || shard.source.byte_len == 0
                || source_end > allocation.byte_len
            {
                return Err(RuntimeValidationErrorV1::InvalidRange.into());
            }
            for prior in &sources[..index] {
                let prior_end = prior
                    .destination_offset
                    .checked_add(prior.source.byte_len)
                    .ok_or(RuntimeValidationErrorV1::InvalidRange)?;
                if prior.node == shard.node
                    || (prior.destination_offset < end && shard.destination_offset < prior_end)
                {
                    return Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch.into());
                }
            }
        }
        for (index, candidate) in destinations.iter().enumerate() {
            if !self.allocations.contains_key(&candidate.allocation) {
                return Err(RuntimeValidationErrorV1::UnknownAllocation.into());
            }
            if !self.streams.contains_key(&candidate.stream) {
                return Err(RuntimeValidationErrorV1::UnknownStream.into());
            }
            if destinations[..index]
                .iter()
                .any(|prior| prior.allocation == candidate.allocation)
            {
                return Err(RuntimeValidationErrorV1::InvalidPeerCopyBatch.into());
            }
        }
        let mut best: Option<RuntimePeerGatherPlacementV1> = None;
        for (selected, candidate) in destinations.iter().enumerate() {
            let mut plan = RuntimePeerGatherPlacementV1 {
                selected,
                stream: candidate.stream,
                shards: Vec::new(),
                observations: Vec::new(),
                staged_bytes: 0,
                peak_staging_bytes: 0,
            };
            plan.shards
                .try_reserve_exact(sources.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            plan.observations
                .try_reserve_exact(sources.len())
                .map_err(|_| RuntimeValidationErrorV1::Capacity)?;
            for source in sources {
                let shard = RuntimeGraphPeerShardV1 {
                    node: source.node,
                    source: source.source,
                    destination: RuntimeMemoryRegionV1 {
                        allocation: candidate.allocation,
                        access: RuntimeAccessV1::Write,
                        byte_offset: source.destination_offset,
                        byte_len: source.source.byte_len,
                    },
                };
                let Ok(Some(observation)) = self.observe_gather_shard_v1(candidate.stream, shard)
                else {
                    break;
                };
                if let BackendPeerCopyPlacementV1::HostStaged { peak_bytes } = observation {
                    plan.staged_bytes = plan
                        .staged_bytes
                        .checked_add(source.source.byte_len)
                        .ok_or(RuntimeValidationErrorV1::Capacity)?;
                    plan.peak_staging_bytes = plan.peak_staging_bytes.max(peak_bytes);
                }
                plan.shards.push(shard);
                plan.observations.push(observation);
            }
            if plan.shards.len() == sources.len()
                && best
                    .as_ref()
                    .is_none_or(|prior| plan.staged_bytes < prior.staged_bytes)
            {
                best = Some(plan);
            }
        }
        best.ok_or_else(|| RuntimeValidationErrorV1::Unsupported.into())
    }

    fn observe_gather_shard_v1(
        &self,
        stream: RuntimeStreamIdV1,
        shard: RuntimeGraphPeerShardV1,
    ) -> Result<Option<BackendPeerCopyPlacementV1>, RuntimeErrorV1<B::Error>> {
        let prepared =
            self.prepare_context_peer_copy_v1(stream, shard.source, shard.destination, &[], true)?;
        let quote = self.backend.observe_peer_copy_placement_v1(
            prepared.stream_record.backend_stream,
            prepared.source,
            prepared.destination,
        );
        if matches!(quote, Some(BackendPeerCopyPlacementV1::HostStaged { peak_bytes }) if peak_bytes < shard.source.byte_len)
        {
            return Ok(None);
        }
        Ok(quote)
    }
}
