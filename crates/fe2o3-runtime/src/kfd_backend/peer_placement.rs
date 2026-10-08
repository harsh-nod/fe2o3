//! Read-only transport observations over retained routes and exact allocations.

use super::*;
use crate::BackendPeerCopyPlacementV1;

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn observe_multi_peer_placement_v1(
        &self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
    ) -> Option<BackendPeerCopyPlacementV1> {
        self.require_live().ok()?;
        let stream_route = *self.streams.get(&stream)?;
        let source_route = *self.allocations.get(&source.allocation)?;
        let destination_route = *self.allocations.get(&destination.allocation)?;
        if source_route.child == destination_route.child
            || stream_route.child != destination_route.child
            || source.access != RuntimeAccessV1::Read
            || destination.access != RuntimeAccessV1::Write
            || source.byte_len == 0
            || source.byte_len != destination.byte_len
            || !self.routed_region_fits(source_route, source)
            || !self.routed_region_fits(destination_route, destination)
            || self.stream_has_native_submission_v1(stream)
            || self.stream_has_pending_cooperative_copy_v1(stream)
        {
            return None;
        }
        for endpoint in [source_route, destination_route] {
            self.children.get(endpoint.child)?.require_live().ok()?;
            if self.compute_xgmi_child_occupied_v1(endpoint.child)
                || self.cooperative_allocation_owners.contains_key(&endpoint)
                || matches!(
                    self.children[endpoint.child]
                        .allocations
                        .get(&endpoint.local)?
                        .sdma_storage,
                    KfdRuntimeSdmaStorageV1::InFlight(_)
                )
            {
                return None;
            }
        }
        let local_source = BackendMemoryRegionV1 {
            allocation: source_route.local,
            ..source
        };
        let local_destination = BackendMemoryRegionV1 {
            allocation: destination_route.local,
            ..destination
        };
        if self
            .observe_unowned_compute_xgmi_v1(
                source_route,
                local_source,
                destination_route,
                local_destination,
            )
            .is_some()
        {
            return Some(BackendPeerCopyPlacementV1::NativeXgmiCandidate);
        }
        let scratch = if [source_route, destination_route]
            .into_iter()
            .any(|endpoint| self.children[endpoint.child].native_available)
        {
            source.byte_len.min(COOPERATIVE_COPY_CHUNK_BYTES_V1 as u64)
        } else {
            0
        };
        let peak_bytes = source.byte_len.checked_add(scratch)?;
        if self.cooperative_staging_bytes.checked_add(peak_bytes)?
            > self.cooperative_staging_limit_bytes
        {
            return None;
        }
        Some(BackendPeerCopyPlacementV1::HostStaged { peak_bytes })
    }
}

impl KfdNativeXgmiRuntimeBackendV1 {
    pub(super) fn observe_native_peer_placement_v1(
        &self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
    ) -> Option<BackendPeerCopyPlacementV1> {
        self.require_live().ok()?;
        let source_record = self.allocations.get(&source.allocation)?;
        let destination_record = self.allocations.get(&destination.allocation)?;
        let direction = admit_xgmi_peer_copy_v1(XgmiPeerCopyAdmissionV1 {
            stream_device: *self.streams.get(&stream)?,
            source_device: source_record.device,
            destination_device: destination_record.device,
            source_offset: source.byte_offset,
            source_len: source.byte_len,
            source_allocation_len: source_record.byte_len,
            source_access: source.access,
            destination_offset: destination.byte_offset,
            destination_len: destination.byte_len,
            destination_allocation_len: destination_record.byte_len,
            destination_access: destination.access,
        })
        .ok()?;
        if self.sequence_by_direction[direction].is_some()
            || self.active_stream_owners.contains_key(&stream)
            || [source.allocation, destination.allocation]
                .iter()
                .any(|id| self.active_allocation_owners.contains_key(id))
            || self
                .submissions
                .len()
                .checked_add(self.completion_reservations)?
                >= MAX_RUNTIME_SUBMISSIONS_V1
        {
            return None;
        }
        Some(BackendPeerCopyPlacementV1::NativeXgmiCandidate)
    }
}
