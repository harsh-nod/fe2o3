//! Exact pending native-peer inputs for the existing cooperative D2H machine.

use super::*;

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(super) fn pending_native_peer_readback_v1(
        &self,
        stream: RoutedHandleV1,
        source: BackendMemoryRegionV1,
        source_route: RoutedHandleV1,
        destination: BackendMemoryRegionV1,
        destination_route: RoutedHandleV1,
        dependencies: &[u64],
    ) -> bool {
        if dependencies.len() > MAX_RUNTIME_DEPENDENCIES_V1
            || source_route == destination_route
            || source_route.child != destination_route.child
            || destination_route.child != stream.child
            || source.access != RuntimeAccessV1::Read
            || destination.access != RuntimeAccessV1::Write
            || source.byte_len == 0
            || source.byte_len != destination.byte_len
            || self.allocations.get(&source.allocation) != Some(&source_route)
            || self.allocations.get(&destination.allocation) != Some(&destination_route)
        {
            return false;
        }
        let Some(source_end) = source.byte_offset.checked_add(source.byte_len) else {
            return false;
        };
        let Some(destination_end) = destination.byte_offset.checked_add(destination.byte_len)
        else {
            return false;
        };
        let Some(child) = self.children.get(stream.child) else {
            return false;
        };
        // These records retain immutable identity/extent metadata even while the
        // peer root, not the child storage slot, owns the physical allocations.
        let Some(source_record) = child.allocations.get(&source_route.local) else {
            return false;
        };
        let Some(destination_record) = child.allocations.get(&destination_route.local) else {
            return false;
        };
        if source_record.kind != RuntimeMemoryKindV1::DeviceLocal
            || destination_record.kind != RuntimeMemoryKindV1::HostVisible
            || source_record.device != child.description.backend_device
            || destination_record.device != child.description.backend_device
            || source_end > source_record.bytes.len() as u64
            || destination_end > destination_record.bytes.len() as u64
        {
            return false;
        }
        dependencies.iter().any(|event| {
            let Some(RoutedEventV1::CooperativeCopy {
                submission,
                child: event_child,
            }) = self.events.get(event)
            else {
                return false;
            };
            let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(submission)
            else {
                return false;
            };
            *event_child == stream.child
                && copy.status() == BackendPollV1::Pending
                && copy.directed.is_none()
                && copy.compute_xgmi.is_some()
                && copy.source.child != copy.destination.child
                && copy.destination == source_route
                && copy.destination_region.allocation == source.allocation
                && self.allocations.get(&copy.source_region.allocation) == Some(&copy.source)
                && self.allocations.get(&copy.destination_region.allocation)
                    == Some(&copy.destination)
                && self
                    .streams
                    .get(&copy.stream)
                    .is_some_and(|route| route.child == *event_child)
                && copy.destination_region.byte_offset <= source.byte_offset
                && copy
                    .destination_region
                    .byte_offset
                    .checked_add(copy.destination_region.byte_len)
                    .is_some_and(|end| source_end <= end)
        })
    }
}
