//! Private predecessor access; public allocation operations never receive an origin.

use super::*;

const MAX_PEER_COMPUTE_PERMITS_V1: usize = 2 * peer_ancestry::MAX_PEER_LAUNCH_ANCESTORS_V1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum PeerCopyLegV1 {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum PeerAccessPurposeV1 {
    Copy,
    Reconcile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PeerCopyRegionV1 {
    producer: u64,
    endpoint: RoutedHandleV1,
    device: u64,
    leg: PeerCopyLegV1,
    offset: u64,
    bytes: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct PeerCopyOriginV1 {
    region: PeerCopyRegionV1,
    scratch: Option<u64>,
    stream: Option<u64>,
}

impl PeerCopyOriginV1 {
    pub(super) fn leg(self) -> PeerCopyLegV1 {
        self.region.leg
    }

    pub(super) fn with_scratch(mut self, scratch: Option<u64>) -> Self {
        self.scratch = scratch;
        self
    }

    pub(super) fn with_stream(mut self, stream: Option<u64>) -> Self {
        self.stream = stream;
        self
    }

    pub(super) fn matches_leaf(
        self,
        base: Self,
        endpoint: RoutedHandleV1,
        scratch: Option<u64>,
        stream: Option<u64>,
    ) -> bool {
        self.region == base.region
            && self.region.endpoint == endpoint
            && self.scratch == scratch
            && self.stream == stream
    }

    pub(super) fn matches_host(
        self,
        device: u64,
        allocation: u64,
        leg: PeerCopyLegV1,
        offset: u64,
        bytes: u64,
    ) -> bool {
        self.region.device == device
            && self.region.endpoint.local == allocation
            && self.region.leg == leg
            && bytes != 0
            && offset >= self.region.offset
            && offset
                .checked_add(bytes)
                .zip(self.region.offset.checked_add(self.region.bytes))
                .is_some_and(|(end, limit)| end <= limit)
    }

    pub(super) fn matches_preparation(self, device: u64, allocation: u64, scratch: u64) -> bool {
        self.region.device == device
            && self.region.endpoint.local == allocation
            && self.scratch == Some(scratch)
            && self.stream.is_some_and(|stream| stream != 0)
            && scratch != allocation
    }

    pub(super) fn matches_dma(
        self,
        device: u64,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
    ) -> bool {
        let (endpoint, scratch) = if self.region.leg == PeerCopyLegV1::Read {
            (source, destination)
        } else {
            (destination, source)
        };
        self.stream == Some(stream)
            && self.scratch == Some(scratch.allocation)
            && scratch.allocation != endpoint.allocation
            && scratch.byte_offset == 0
            && source.access == RuntimeAccessV1::Read
            && destination.access == RuntimeAccessV1::Write
            && scratch.byte_len == endpoint.byte_len
            && self.matches_host(
                device,
                endpoint.allocation,
                self.region.leg,
                endpoint.byte_offset,
                endpoint.byte_len,
            )
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PeerCopyAccessV1 {
    origin: PeerCopyOriginV1,
    stream: u64,
    source: BackendMemoryRegionV1,
    destination: BackendMemoryRegionV1,
}

impl PeerCopyAccessV1 {
    pub(super) fn origin(self) -> PeerCopyOriginV1 {
        self.origin
    }

    pub(super) fn capture(
        origin: PeerCopyOriginV1,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
    ) -> Self {
        Self {
            origin,
            stream,
            source,
            destination,
        }
    }
}

#[derive(Debug)]
struct PeerComputePermitV1 {
    owner: u64,
    consumer: u64,
    region: PeerCopyRegionV1,
    purpose: PeerAccessPurposeV1,
}

impl PeerComputePermitV1 {
    fn key(&self) -> (u64, usize, u64, PeerCopyLegV1, PeerAccessPurposeV1) {
        (
            self.region.producer,
            self.region.endpoint.child,
            self.region.endpoint.local,
            self.region.leg,
            self.purpose,
        )
    }
}

#[derive(Debug, Default)]
pub(super) struct PeerComputePermitsV1(Box<[PeerComputePermitV1]>);

#[derive(Default)]
pub(super) struct PeerDmaAdmissionsV1(Vec<(u64, u64)>);

impl PeerDmaAdmissionsV1 {
    pub(super) fn authorizes(&self, allocation: u64, submission: u64) -> bool {
        self.0.contains(&(allocation, submission))
    }
}

impl PeerComputePermitsV1 {
    pub(super) fn valid_for(
        &self,
        gate: Option<PeerComputeGateV1>,
        consumer: u64,
        bindings: &[BackendBindingV1],
    ) -> bool {
        self.0.len() <= MAX_PEER_COMPUTE_PERMITS_V1
            && self.0.windows(2).all(|pair| pair[0].key() < pair[1].key())
            && self.0.iter().all(|permit| {
                permit.consumer == consumer
                    && gate.is_some_and(|gate| gate.owns(permit.owner, consumer))
                    && bindings
                        .iter()
                        .any(|binding| binding.region.allocation == permit.region.endpoint.local)
            })
    }

    fn authorizes(
        &self,
        gate: PeerComputeGateV1,
        consumer: u64,
        origin: PeerCopyOriginV1,
        purpose: PeerAccessPurposeV1,
    ) -> bool {
        let key = (
            origin.region.producer,
            origin.region.endpoint.child,
            origin.region.endpoint.local,
            origin.region.leg,
            purpose,
        );
        self.0
            .binary_search_by_key(&key, PeerComputePermitV1::key)
            .ok()
            .is_some_and(|index| {
                let permit = &self.0[index];
                permit.consumer == consumer
                    && permit.region == origin.region
                    && gate.permits_predecessor_access(permit.owner, consumer)
            })
    }
}

impl KfdRuntimeBackendV1 {
    pub(super) fn admit_peer_dma_owners_v1(
        &mut self,
        bindings: &[BackendBindingV1],
        gate: Option<PeerComputeGateV1>,
        permits: &PeerComputePermitsV1,
    ) -> Result<PeerDmaAdmissionsV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let mut admitted = PeerDmaAdmissionsV1::default();
        let Some(gate) = gate else {
            return Ok(admitted);
        };
        for binding in bindings {
            let allocation = binding.region.allocation;
            let Some(custody) = self.allocation_custody.get(&allocation) else {
                continue;
            };
            for owner in &custody.owners {
                if owner.kind != RuntimeAllocationCustodyKindV1::Sdma {
                    continue;
                }
                let Some(active) = self.active_sdma.get(&owner.submission) else {
                    return Err(self.terminal_error("peer admission lost a retained DMA owner"));
                };
                let Some(access) = active.peer_access else {
                    continue;
                };
                let published = self
                    .published_sdma_submissions
                    .iter()
                    .filter(|id| **id == owner.submission)
                    .count();
                let phase_intact = match active.phase {
                    ActiveSdmaPhaseV1::Ready => published == 0 && [active.source, active.destination].into_iter().all(|id| {
                        self.allocations.get(&id).is_some_and(|record| !matches!(record.sdma_storage,
                            KfdRuntimeSdmaStorageV1::InFlight(_) | KfdRuntimeSdmaStorageV1::ComputeInFlight(_)))
                    }),
                    ActiveSdmaPhaseV1::DirectionalPublished(_) => published == 1 && [active.source, active.destination].into_iter().all(|id| {
                        self.allocations.get(&id).is_some_and(|record| matches!(record.sdma_storage,
                            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(actual)) if actual == active.id))
                    }),
                    ActiveSdmaPhaseV1::SameDevicePublished(_) | ActiveSdmaPhaseV1::Quarantined => false,
                };
                if active.id != owner.submission
                    || active.stream != owner.stream
                    || !active.dependencies.is_empty()
                    || !phase_intact
                    || !self.peer_dma_access_is_intact_v1(active)
                    || ![active.source, active.destination].into_iter().all(|id| {
                        self.allocation_retains_exact_owner_v1(id, *owner)
                            && self.allocation_custody[&id].owner_counts
                                [RuntimeAllocationCustodyKindV1::Sdma.index()]
                                == 1
                            && self.allocation_custody[&id]
                                .owners
                                .iter()
                                .filter(|entry| entry.kind == RuntimeAllocationCustodyKindV1::Sdma)
                                .eq(std::iter::once(owner))
                    })
                {
                    return Err(self
                        .terminal_error("peer admission DMA identity or custody is inconsistent"));
                }
                if access.origin.region.endpoint.local == allocation
                    && permits.authorizes(
                        gate,
                        self.next_handle,
                        access.origin,
                        PeerAccessPurposeV1::Copy,
                    )
                    && !admitted.authorizes(allocation, owner.submission)
                {
                    admitted.0.try_reserve(1).map_err(|_| {
                        Self::capacity("peer DMA admission roster allocation failed")
                    })?;
                    admitted.0.push((allocation, owner.submission));
                }
            }
        }
        Ok(admitted)
    }

    pub(super) fn peer_dma_endpoints_are_clean_v1(&self, source: u64, destination: u64) -> bool {
        [source, destination].into_iter().all(|allocation| {
            self.allocations
                .get(&allocation)
                .is_some_and(|record| record.native_dirty.is_empty())
                && !self.native_reconciliation_holds_v1(allocation)
        })
    }

    pub(super) fn peer_access_authorizes_owner_v1(
        &self,
        allocation: u64,
        owner: RuntimeAllocationCustodyOwnerV1,
        origin: Option<PeerCopyOriginV1>,
        purpose: PeerAccessPurposeV1,
    ) -> bool {
        let Some(origin) = origin else {
            return false;
        };
        if owner.kind != RuntimeAllocationCustodyKindV1::Compute
            || origin.region.endpoint.local != allocation
            || origin.region.device != self.description.backend_device
            || self.active_compute_lane_v1(owner.submission).is_some()
            || self.submissions.contains_key(&owner.submission)
        {
            return false;
        }
        self.pending_compute
            .get(&owner.submission)
            .is_some_and(|pending| {
                pending.id == owner.submission
                    && pending.launch.stream == owner.stream
                    && pending.retained_allocations.contains(&allocation)
                    && pending.peer_gate.is_some_and(|gate| {
                        pending
                            .peer_access
                            .authorizes(gate, pending.id, origin, purpose)
                    })
            })
    }

    pub(super) fn peer_access_has_conflict_v1(
        &self,
        allocation: u64,
        origin: Option<PeerCopyOriginV1>,
        purpose: PeerAccessPurposeV1,
    ) -> bool {
        self.allocation_custody
            .get(&allocation)
            .is_some_and(|custody| {
                custody.owners.iter().any(|owner| {
                    !self.peer_access_authorizes_owner_v1(allocation, *owner, origin, purpose)
                })
            })
    }

    pub(super) fn peer_dma_access_is_intact_v1(&self, copy: &ActiveSdmaCopyV1) -> bool {
        let Some(access) = copy.peer_access else {
            return true;
        };
        let origin = access.origin;
        let source = BackendMemoryRegionV1 {
            allocation: copy.source,
            access: RuntimeAccessV1::Read,
            byte_offset: copy.source_offset,
            byte_len: copy.byte_len,
        };
        let destination = BackendMemoryRegionV1 {
            allocation: copy.destination,
            access: RuntimeAccessV1::Write,
            byte_offset: copy.destination_offset,
            byte_len: copy.byte_len,
        };
        access.stream == copy.stream
            && access.source == source
            && access.destination == destination
            && self.peer_dma_endpoints_are_clean_v1(copy.source, copy.destination)
            && origin.matches_dma(
                self.description.backend_device,
                copy.stream,
                source,
                destination,
            )
            && [copy.source, copy.destination]
                .into_iter()
                .all(|allocation| {
                    self.allocation_custody
                        .get(&allocation)
                        .is_none_or(|custody| {
                            custody
                                .owners
                                .iter()
                                .filter(|owner| {
                                    owner.kind == RuntimeAllocationCustodyKindV1::Compute
                                })
                                .all(|owner| {
                                    self.peer_access_authorizes_owner_v1(
                                        allocation,
                                        *owner,
                                        Some(origin),
                                        PeerAccessPurposeV1::Copy,
                                    )
                                })
                        })
                })
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn peer_copy_origin_v1(
        &mut self,
        producer: u64,
    ) -> Result<Option<PeerCopyOriginV1>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.check_directed_if_present_v1(producer)?;
        let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&producer)
        else {
            return Err(self.directed_corruption_v1());
        };
        if copy.directed.is_none() {
            return Ok(None);
        }
        let leg = match copy.phase {
            CooperativeCopyPhaseV1::Read => PeerCopyLegV1::Read,
            CooperativeCopyPhaseV1::Write => PeerCopyLegV1::Write,
            _ => return Err(self.directed_corruption_v1()),
        };
        self.peer_copy_origin_for_leg_v1(producer, leg)
    }

    pub(super) fn peer_copy_origin_for_leg_v1(
        &mut self,
        producer: u64,
        leg: PeerCopyLegV1,
    ) -> Result<Option<PeerCopyOriginV1>, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.check_directed_if_present_v1(producer)?;
        let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&producer)
        else {
            return Err(self.directed_corruption_v1());
        };
        if copy.directed.is_none() {
            return Ok(None);
        }
        let (endpoint, region) = match leg {
            PeerCopyLegV1::Read => (copy.source, copy.source_region),
            PeerCopyLegV1::Write => (copy.destination, copy.destination_region),
        };
        Ok(Some(PeerCopyOriginV1 {
            region: PeerCopyRegionV1 {
                producer,
                endpoint,
                device: self.children[endpoint.child].description.backend_device,
                leg,
                offset: region.byte_offset,
                bytes: region.byte_len,
            },
            scratch: None,
            stream: None,
        }))
    }

    pub(super) fn prepare_peer_compute_access_v1(
        &mut self,
        consumer: RoutedHandleV1,
        ancestry: &PeerLaunchAncestryV1,
        bindings: &[BackendBindingV1],
    ) -> Result<PeerComputePermitsV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let mut permits = Vec::new();
        let owner = ancestry.owner();
        if owner == 0 || consumer.local == 0 || self.children.get(consumer.child).is_none() {
            return Err(KfdRuntimeBackendV1::capacity(
                "invalid peer compute permit roster",
            ));
        }
        self.admit_peer_ancestry_bindings_v1(ancestry, consumer.child, bindings)?;
        for producer in ancestry.producers() {
            let read_origin = self.peer_copy_origin_for_leg_v1(producer, PeerCopyLegV1::Read)?;
            let write_origin = self.peer_copy_origin_for_leg_v1(producer, PeerCopyLegV1::Write)?;
            let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&producer)
            else {
                return Err(self.directed_corruption_v1());
            };
            if copy.is_quiescent() {
                continue;
            }
            if !copy.sdma_leaf.as_ref().is_none_or(|leaf| {
                leaf.authenticates_compute_predecessor_v1(
                    copy,
                    &self.children[leaf.child()],
                    read_origin,
                    write_origin,
                )
            }) {
                return Err(self.directed_corruption_v1());
            }
            if copy.directed.is_none() || producer >= owner {
                return Err(KfdRuntimeBackendV1::rejected(
                    KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                    "peer permit requires an earlier directed producer",
                ));
            }
            for (endpoint, region, leg) in [
                (copy.source, copy.source_region, PeerCopyLegV1::Read),
                (
                    copy.destination,
                    copy.destination_region,
                    PeerCopyLegV1::Write,
                ),
            ] {
                if endpoint.child != consumer.child
                    || !bindings
                        .iter()
                        .any(|binding| binding.region.allocation == endpoint.local)
                {
                    continue;
                }
                if permits.len() + 2 > MAX_PEER_COMPUTE_PERMITS_V1 {
                    return Err(KfdRuntimeBackendV1::capacity(
                        "peer compute permit capacity exceeded",
                    ));
                }
                permits.try_reserve(2).map_err(|_| {
                    KfdRuntimeBackendV1::capacity("peer compute permit allocation failed")
                })?;
                for purpose in [PeerAccessPurposeV1::Copy, PeerAccessPurposeV1::Reconcile] {
                    permits.push(PeerComputePermitV1 {
                        owner,
                        consumer: consumer.local,
                        purpose,
                        region: PeerCopyRegionV1 {
                            producer,
                            endpoint,
                            device: self.children[endpoint.child].description.backend_device,
                            leg,
                            offset: region.byte_offset,
                            bytes: region.byte_len,
                        },
                    });
                }
            }
        }
        permits.sort_unstable_by_key(PeerComputePermitV1::key);
        if permits
            .windows(2)
            .any(|pair| pair[0].key() == pair[1].key())
        {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "duplicate peer compute permit",
            ));
        }
        Ok(PeerComputePermitsV1(permits.into_boxed_slice()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origin() -> PeerCopyOriginV1 {
        PeerCopyOriginV1 {
            region: PeerCopyRegionV1 {
                producer: 1,
                endpoint: RoutedHandleV1 { child: 0, local: 3 },
                device: 7,
                leg: PeerCopyLegV1::Write,
                offset: 2,
                bytes: 4,
            },
            scratch: Some(4),
            stream: Some(5),
        }
    }

    #[test]
    fn peer_compute_access_origin_bounds_identity_and_overflow() {
        let origin = origin();
        for offset in 0..8 {
            for bytes in 0..8 {
                assert_eq!(
                    origin.matches_host(7, 3, PeerCopyLegV1::Write, offset, bytes),
                    bytes != 0 && offset >= 2 && offset + bytes <= 6
                );
            }
        }
        assert!(!origin.matches_host(8, 3, PeerCopyLegV1::Write, 2, 4));
        assert!(!origin.matches_host(7, 4, PeerCopyLegV1::Write, 2, 4));
        assert!(!origin.matches_host(7, 3, PeerCopyLegV1::Read, 2, 4));
        assert!(!origin.matches_host(7, 3, PeerCopyLegV1::Write, u64::MAX, 2));
        let malformed = PeerCopyOriginV1 {
            region: PeerCopyRegionV1 {
                offset: u64::MAX - 1,
                bytes: 3,
                ..origin.region
            },
            ..origin
        };
        assert!(!malformed.matches_host(7, 3, PeerCopyLegV1::Write, u64::MAX - 1, 1));
        let source = BackendMemoryRegionV1 {
            allocation: 4,
            access: RuntimeAccessV1::Read,
            byte_offset: 0,
            byte_len: 4,
        };
        let destination = BackendMemoryRegionV1 {
            allocation: 3,
            access: RuntimeAccessV1::Write,
            byte_offset: 2,
            byte_len: 4,
        };
        assert!(origin.matches_dma(7, 5, source, destination));
        assert!(!origin.matches_dma(7, 6, source, destination));
        assert!(!origin.matches_dma(
            7,
            5,
            BackendMemoryRegionV1 {
                byte_offset: 1,
                ..source
            },
            destination
        ));
        assert!(!origin.matches_dma(
            7,
            5,
            BackendMemoryRegionV1 {
                allocation: 6,
                ..source
            },
            destination
        ));
        assert!(!origin.matches_dma(
            7,
            5,
            source,
            BackendMemoryRegionV1 {
                byte_offset: 3,
                ..destination
            }
        ));
    }

    #[test]
    fn peer_compute_access_permits_separate_copy_preparation_and_exact_producer() {
        let origin = origin();
        let gate = PeerComputeGateV1::waiting(10, 20, true);
        for purpose in [PeerAccessPurposeV1::Copy, PeerAccessPurposeV1::Reconcile] {
            let permits = PeerComputePermitsV1(
                vec![PeerComputePermitV1 {
                    owner: 10,
                    consumer: 20,
                    region: origin.region,
                    purpose,
                }]
                .into_boxed_slice(),
            );
            assert!(permits.authorizes(gate, 20, origin, purpose));
            let other = if purpose == PeerAccessPurposeV1::Copy {
                PeerAccessPurposeV1::Reconcile
            } else {
                PeerAccessPurposeV1::Copy
            };
            assert!(!permits.authorizes(gate, 20, origin, other));
            assert!(!permits.authorizes(gate, 21, origin, purpose));
            assert!(!permits.authorizes(
                PeerComputeGateV1::waiting(11, 20, true),
                20,
                origin,
                purpose
            ));
            for region in [
                PeerCopyRegionV1 {
                    producer: 2,
                    ..origin.region
                },
                PeerCopyRegionV1 {
                    leg: PeerCopyLegV1::Read,
                    ..origin.region
                },
                PeerCopyRegionV1 {
                    endpoint: RoutedHandleV1 { child: 1, local: 3 },
                    ..origin.region
                },
                PeerCopyRegionV1 {
                    offset: 3,
                    ..origin.region
                },
            ] {
                assert!(!permits.authorizes(
                    gate,
                    20,
                    PeerCopyOriginV1 { region, ..origin },
                    purpose
                ));
            }
        }
    }
}
