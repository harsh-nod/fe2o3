//! One native owner root for an immutable ordered descriptor list.

use super::*;
use crate::{RuntimePeerCopySegmentV1, RuntimePeerCopySegmentsBackendV1};
use fe2o3_kfd::Gfx942ComputeXgmiSegmentsPlanErrorV1;
use fe2o3_runtime_model::OrderedPeerCopyAdmissionErrorV1;

impl RuntimePeerCopySegmentsBackendV1 for KfdMultiDeviceRuntimeBackendV1 {
    /// Native-only ordered copies of initialized PUBLIC DeviceLocal owners.
    ///
    /// Both bounding regions are allocation-relative and may differ in length.
    /// Each descriptor is checked before admission and retains its list position,
    /// including duplicates and overlapping writes. One queue, mapping pair, and
    /// logical result cover the complete list; no intermediate result is exposed.
    /// A pending source requires its exact producer-aware full-allocation Write
    /// event. Independently, either source profile may follow the exact same-stream
    /// destination-list event. Downstream reads retain the initialized frame,
    /// not its envelope.
    fn peer_copy_segments_v1(
        &mut self,
        stream: u64,
        source: BackendMemoryRegionV1,
        destination: BackendMemoryRegionV1,
        segments: &[RuntimePeerCopySegmentV1],
        dependencies: &[u64],
    ) -> Result<u64, Failure> {
        self.require_live()?;
        let source_route = Self::route(
            &self.allocations,
            source.allocation,
            "unknown segmented peer source allocation",
        )?;
        let destination_route = Self::route(
            &self.allocations,
            destination.allocation,
            "unknown segmented peer destination allocation",
        )?;
        let source_bytes = self.children[source_route.child].allocations[&source_route.local]
            .bytes
            .len() as u64;
        let destination_bytes = self.children[destination_route.child].allocations
            [&destination_route.local]
            .bytes
            .len() as u64;
        let plan = Gfx942ComputeXgmiSegmentsPlanV1::new(
            source_bytes,
            destination_bytes,
            source.byte_offset,
            source.byte_len,
            destination.byte_offset,
            destination.byte_len,
            segments,
        )
        .map_err(|error| {
            let kind = match error {
                Gfx942ComputeXgmiSegmentsPlanErrorV1::Capacity
                | Gfx942ComputeXgmiSegmentsPlanErrorV1::Segments(
                    OrderedPeerCopyAdmissionErrorV1::Count,
                ) => KfdRuntimeBackendErrorKindV1::Capacity,
                _ => KfdRuntimeBackendErrorKindV1::InvalidLaunch,
            };
            KfdRuntimeBackendV1::rejected(
                kind,
                format!("compute-XGMI segment preflight: {error:?}"),
            )
        })?;
        self.submit_cooperative_copy_transport_v1(
            stream,
            source,
            destination,
            dependencies,
            true,
            CooperativeCopyProfileV1::Segments(Arc::new(plan)),
        )
    }
}

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(in crate::kfd_backend) fn prepare_compute_xgmi_segments_v1(
        &self,
        source: RoutedHandleV1,
        source_region: BackendMemoryRegionV1,
        destination: RoutedHandleV1,
        destination_region: BackendMemoryRegionV1,
        plan: Arc<Gfx942ComputeXgmiSegmentsPlanV1>,
        custody: (
            Option<&compute_peer::Producer>,
            Option<&compute_peer::SegmentDestinationPredecessor>,
        ),
    ) -> Result<Box<Root>, Failure> {
        let (producer, predecessor) = custody;
        let unsupported = || {
            KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "segmented peer copy requires initialized PUBLIC owners, an exact source producer or settled source, and a native route",
            )
        };
        let route = self
            .compute_xgmi_routes
            .get(&(source.child, destination.child))
            .copied()
            .ok_or_else(unsupported)?;
        if producer.is_some_and(|producer| {
            producer
                .segments()
                .is_none_or(|retained| !Arc::ptr_eq(retained, &plan))
        }) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "segmented native copy lost its exact pending source plan",
            ));
        }
        for (endpoint, region) in [(source, source_region), (destination, destination_region)] {
            let child = &self.children[endpoint.child];
            let record = &child.allocations[&endpoint.local];
            let pending_source = endpoint == source
                && producer.is_some_and(|producer| producer.owns_source(self, endpoint));
            let ordered_destination = endpoint == destination
                && self.compute_xgmi_children[endpoint.child].is_some_and(|owner| {
                    predecessor
                        .is_some_and(|prior| prior.owns_occupied_child(self, endpoint.child, owner))
                });
            if !child.peer_visible_device_allocations
                || !checked_envelope(record, region)
                || !record.sdma_initialized
                || !pending_source
                    && !ordered_destination
                    && (child.allocation_is_active(endpoint.local)
                        || self.allocation_retained_by_deferred_compute_v1(endpoint)
                        || !matches!(
                            record.sdma_storage,
                            KfdRuntimeSdmaStorageV1::Device(_)
                                | KfdRuntimeSdmaStorageV1::H2dReady(_)
                                | KfdRuntimeSdmaStorageV1::PersistentReplay(_)
                                | KfdRuntimeSdmaStorageV1::InitializedStorage(_)
                        ))
            {
                return Err(unsupported());
            }
        }
        let root = Root::prepare_segments(route, plan)?;
        if !root.matches_regions(
            &self.children[source.child].allocations[&source.local],
            source_region,
            &self.children[destination.child].allocations[&destination.local],
            destination_region,
        ) {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::InvalidLaunch,
                "segmented peer envelopes changed during preflight",
            ));
        }
        Ok(root)
    }
}
