use super::*;

/// Inert exact-version source choice. It is not a replica constructor or a native
/// read lease. Submission revalidates it, then acquires the ordinary copy leases.
#[derive(Debug)]
pub struct RuntimeReplicaSourceSelectionV1 {
    origin: ReplicaStampV1,
    source: ReplicaStampV1,
    destination: ReplicaStampV1,
    stream: RuntimeStreamIdV1,
    reference: Option<RuntimeReplicaReferenceV1>,
    placement: Option<BackendPeerCopyPlacementV1>,
}

impl RuntimeReplicaSourceSelectionV1 {
    pub const fn source(&self) -> RuntimeMemoryRegionV1 {
        self.source.region
    }
    pub const fn source_device(&self) -> RuntimeDeviceIdV1 {
        self.source.record.device
    }
    pub const fn replica(&self) -> Option<RuntimeReplicaReferenceV1> {
        self.reference
    }
    pub const fn peer_placement(&self) -> Option<BackendPeerCopyPlacementV1> {
        self.placement
    }
}

impl<B: RuntimeBackendV1> RuntimeContextV1<B> {
    /// Selects only the current original or a current settled copy of it, over
    /// up to the existing 256-device Context bound. Prefer local, then eligible
    /// native peer, then bounded staging; stable original device-roster ties.
    /// No hashes or caller declarations establish replica equivalence.
    pub fn select_replica_source_v1(
        &self,
        origin: RuntimeMemoryRegionV1,
        stream: RuntimeStreamIdV1,
        destination: RuntimeMemoryRegionV1,
        candidates: &[RuntimeAllocationIdV1],
    ) -> Result<RuntimeReplicaSourceSelectionV1, RuntimeErrorV1<B::Error>> {
        self.require_graph_access(None)?;
        if origin.access != RuntimeAccessV1::Read
            || destination.access != RuntimeAccessV1::Write
            || origin.byte_len != destination.byte_len
            || candidates.is_empty()
            || candidates.len() > MAX_RUNTIME_DEVICES_V1
        {
            return Err(RuntimeValidationErrorV1::InvalidRange.into());
        }
        let origin = self.replica_stamp_v1(origin)?;
        let destination = self.replica_stamp_v1(destination)?;
        if self.unheld_stream_v1(stream)?.device != destination.record.device {
            return Err(RuntimeValidationErrorV1::WrongDevice.into());
        }
        for (index, candidate) in candidates.iter().enumerate() {
            if !self.allocations.contains_key(candidate) {
                return Err(RuntimeValidationErrorV1::UnknownAllocation.into());
            }
            if candidates[..index].contains(candidate) {
                return Err(RuntimeValidationErrorV1::InvalidRange.into());
            }
        }
        let table = self
            .replicas
            .as_ref()
            .ok_or(RuntimeValidationErrorV1::Unsupported)?;
        let mut best: Option<((u8, u64), RuntimeReplicaSourceSelectionV1)> = None;
        for device in &self.devices {
            for candidate in candidates {
                if self.allocations[candidate].device != device.id
                    || *candidate == destination.region.allocation
                {
                    continue;
                }
                let selected = if *candidate == origin.region.allocation {
                    Some((origin, None))
                } else {
                    table.slots.iter().enumerate().find_map(|(slot, entry)| {
                        let ReplicaStateV1::Settled(fact) = entry.state else {
                            return None;
                        };
                        if fact.source != origin || fact.destination.region.allocation != *candidate
                        {
                            return None;
                        }
                        let reference = RuntimeReplicaReferenceV1 {
                            context: self.context_generation,
                            slot,
                            incarnation: entry.incarnation,
                        };
                        self.current_replica_v1(reference).ok().map(|fact| {
                            let mut source = fact.destination;
                            source.region.access = RuntimeAccessV1::Read;
                            (source, Some(reference))
                        })
                    })
                };
                let Some((source, reference)) = selected else {
                    continue;
                };
                let Ok(placement) = self.replica_route_v1(stream, source, destination) else {
                    continue;
                };
                let cost = match placement {
                    None => (0, 0),
                    Some(BackendPeerCopyPlacementV1::NativeXgmiCandidate) => (1, 0),
                    Some(BackendPeerCopyPlacementV1::HostStaged { peak_bytes }) => (2, peak_bytes),
                };
                if best.as_ref().is_none_or(|(previous, _)| cost < *previous) {
                    best = Some((
                        cost,
                        RuntimeReplicaSourceSelectionV1 {
                            origin,
                            source,
                            destination,
                            stream,
                            reference,
                            placement,
                        },
                    ));
                }
            }
        }
        best.map(|(_, selection)| selection)
            .ok_or_else(|| RuntimeValidationErrorV1::Unsupported.into())
    }

    fn replica_route_v1(
        &self,
        stream: RuntimeStreamIdV1,
        source: ReplicaStampV1,
        destination: ReplicaStampV1,
    ) -> Result<Option<BackendPeerCopyPlacementV1>, RuntimeErrorV1<B::Error>> {
        if source.record.device == destination.record.device {
            self.prepare_context_copy_v1(stream, source.region, destination.region, &[], None)?;
            return Ok(None);
        }
        let prepared = self.prepare_context_peer_copy_v1(
            stream,
            source.region,
            destination.region,
            &[],
            true,
        )?;
        let quote = self
            .backend
            .observe_peer_copy_placement_v1(
                prepared.stream_record.backend_stream,
                prepared.source,
                prepared.destination,
            )
            .ok_or(RuntimeValidationErrorV1::Unsupported)?;
        if matches!(quote, BackendPeerCopyPlacementV1::HostStaged { peak_bytes } if peak_bytes < source.region.byte_len)
        {
            return Err(RuntimeValidationErrorV1::Unsupported.into());
        }
        Ok(Some(quote))
    }

    /// Revalidates the origin, selected copy and route before the original copy
    /// submission. The new fact refers to the selected physical source version,
    /// not an inferred transitive origin: replica chaining is not certified.
    pub fn submit_selected_replica_copy_v1(
        &mut self,
        selection: RuntimeReplicaSourceSelectionV1,
    ) -> Result<RuntimeTrackedReplicaCopyV1, RuntimeErrorV1<B::Error>>
    where
        B: RuntimeAsyncCopyBackendV1,
    {
        self.validate_replica_stamp_v1(selection.origin)?;
        self.validate_replica_stamp_v1(selection.source)?;
        self.validate_replica_stamp_v1(selection.destination)?;
        if let Some(reference) = selection.reference {
            let fact = self.current_replica_v1(reference)?;
            if fact.source != selection.origin
                || fact.destination.record != selection.source.record
                || fact.destination.read != selection.source.read
            {
                return Err(RuntimeValidationErrorV1::ContextReserved.into());
            }
        } else if selection.source != selection.origin {
            return Err(RuntimeValidationErrorV1::ContextReserved.into());
        }
        if self.replica_route_v1(selection.stream, selection.source, selection.destination)?
            != selection.placement
        {
            return Err(RuntimeValidationErrorV1::ContextReserved.into());
        }
        self.submit_tracked_replica_copy_v1(
            selection.stream,
            selection.source.region,
            selection.destination.region,
        )
    }
}
