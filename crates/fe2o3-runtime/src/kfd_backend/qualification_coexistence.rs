//! Qualification-only observation. These digests never grant runtime authority.
use super::*;
use sha2::{Digest, Sha256};

/// Address-free qualification data, not a publication or retirement capability.
/// Native digests bind retained receipts; membership digests bind their current
/// runtime owners. Neither establishes unfinished GPU work or physical overlap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KfdR66RetainedCustodyObservationV1 {
    pub compute: Option<[u8; 32]>,
    pub compute_membership: Option<[u8; 32]>,
    pub copy: Option<[u8; 32]>,
    pub copy_membership: Option<[u8; 32]>,
    pub copy_packets: usize,
}

fn membership(kind: &[u8], native: [u8; 32], coordinates: &[u64]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"fe2o3.r66.runtime-retained-membership.v1\0");
    hash.update(kind);
    hash.update(native);
    for coordinate in coordinates {
        hash.update(coordinate.to_le_bytes());
    }
    hash.finalize().into()
}

impl KfdRuntimeBackendV1 {
    fn r66_runtime_roster_shape_v1(&self) -> bool {
        if self.terminal
            || self.queue_retired
            || self.terminal_memory.is_some()
            || self.terminal_sdma_custody.is_some()
            || !matches!(
                self.launch_gate,
                KfdRuntimeLaunchGateV1::ExactGfx942InplaceTransform(_)
            )
            || !self.pending_compute.is_empty()
            || !self.compute_pipeline.is_empty()
            || self
                .auxiliary_compute_lanes
                .iter()
                .any(|lane| lane.active.is_some() || !lane.pipeline.is_empty())
            || self.active_sdma.len() > 1
            || self.published_sdma_submissions.len() != self.active_sdma.len()
        {
            return false;
        }
        for (&id, record) in &self.submissions {
            if matches!(record.status, BackendPollV1::Pending)
                && self.active.as_ref().is_none_or(|active| active.id != id)
                && !self.active_sdma.contains_key(&id)
            {
                return false;
            }
        }
        for (&allocation, record) in &self.allocations {
            let accounted = match record.sdma_storage {
                KfdRuntimeSdmaStorageV1::ComputeInFlight(id) => {
                    self.active.as_ref().is_some_and(|active| {
                        active.id == id && active.allocations.contains(&allocation)
                    })
                }
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(id)) => {
                    self.active_sdma.get(&id).is_some_and(|copy| {
                        copy.source == allocation || copy.destination == allocation
                    })
                }
                KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous) => false,
                _ => true,
            };
            if !accounted {
                return false;
            }
        }
        true
    }

    /// Observes one R26 compute and at most one directional copy without polling.
    /// A retained published receipt may already have completed on the GPU.
    pub fn observe_r66_retained_custody_v1(&self) -> Option<KfdR66RetainedCustodyObservationV1> {
        if !self.r66_runtime_roster_shape_v1() {
            return None;
        }
        let queue = self.queue.as_ref()?;
        let native_counts = queue.observe_r66_retained_counts_v1()?;
        if native_counts != (usize::from(self.active.is_some()), self.active_sdma.len()) {
            return None;
        }
        let mut observed = KfdR66RetainedCustodyObservationV1 {
            compute: None,
            compute_membership: None,
            copy: None,
            copy_membership: None,
            copy_packets: 0,
        };
        if let Some(active) = &self.active {
            let Some(ActiveComputeExecutionV1::Persistent {
                allocation,
                dispatch,
                ..
            }) = &active.execution
            else {
                return None;
            };
            let record = self.submissions.get(&active.id)?;
            let owner = RuntimeAllocationCustodyOwnerV1 {
                submission: active.id,
                stream: active.stream,
                kind: RuntimeAllocationCustodyKindV1::Compute,
            };
            if record.stream != active.stream
                || !matches!(record.status, BackendPollV1::Pending)
                || active.allocations.len() != 1
                || !active.allocations.contains(allocation)
                || !self.allocation_retains_exact_owner_v1(*allocation, owner)
                || !matches!(self.allocations.get(allocation)?.sdma_storage, KfdRuntimeSdmaStorageV1::ComputeInFlight(id) if id == active.id)
            {
                return None;
            }
            let identity = queue.observe_r66_retained_compute_v1(dispatch)?;
            observed.compute = Some(identity);
            observed.compute_membership = Some(membership(
                b"compute\0",
                identity,
                &[active.id, active.stream, active.kernel, *allocation],
            ));
        }
        if let Some((&id, copy)) = self.active_sdma.iter().next() {
            let record = self.submissions.get(&id)?;
            let owner = RuntimeAllocationCustodyOwnerV1 {
                submission: id,
                stream: copy.stream,
                kind: RuntimeAllocationCustodyKindV1::Sdma,
            };
            if id != copy.id || record.stream != copy.stream || !matches!(record.status, BackendPollV1::Pending)
                || self.published_sdma_submissions.as_slice() != [id]
                || copy.completed_bytes != 0 || copy.window_bytes != copy.byte_len
                || [copy.source, copy.destination].iter().any(|allocation|
                    !self.allocation_retains_exact_owner_v1(*allocation, owner)
                        || !self.allocations.get(allocation).is_some_and(|record|
                            matches!(record.sdma_storage, KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(owner_id)) if owner_id == id))
                        || self.active.as_ref().is_some_and(|active| active.allocations.contains(allocation)))
            { return None; }
            let ActiveSdmaPhaseV1::DirectionalPublished(native) = &copy.phase else {
                return None;
            };
            let (identity, packets, direction, host_offset, device_offset, bytes) =
                match native.as_ref() {
                    DirectionalSdmaSubmissionOwnerV1::NativeSingle {
                        submission,
                        host_offset,
                        device_offset,
                    } => {
                        if *host_offset != submission.host_offset()
                            || *device_offset != submission.device_offset()
                        {
                            return None;
                        }
                        (
                            queue.observe_r66_retained_single_copy_v1(submission)?,
                            1,
                            submission.direction(),
                            *host_offset,
                            *device_offset,
                            submission.copy_bytes(),
                        )
                    }
                    DirectionalSdmaSubmissionOwnerV1::NativeWindow { submission } => (
                        queue.observe_r66_retained_window_copy_v1(submission)?,
                        submission.packet_count(),
                        submission.direction(),
                        submission.host_offset(),
                        submission.device_offset(),
                        submission.copy_bytes(),
                    ),
                    #[cfg(test)]
                    _ => return None,
                };
            let (source_offset, destination_offset) = match direction {
                Gfx942PersistentSdmaDirectionV1::HostToDevice => (host_offset, device_offset),
                Gfx942PersistentSdmaDirectionV1::DeviceToHost => (device_offset, host_offset),
            };
            if copy.source_offset != source_offset
                || copy.destination_offset != destination_offset
                || copy.byte_len != u64::from(bytes)
            {
                return None;
            }
            observed.copy = Some(identity);
            observed.copy_packets = packets;
            observed.copy_membership = Some(membership(
                b"copy\0",
                identity,
                &[
                    id,
                    copy.stream,
                    copy.source,
                    copy.destination,
                    copy.source_offset,
                    copy.destination_offset,
                    copy.byte_len,
                ],
            ));
        }
        Some(observed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qualified_without_native_queue() -> KfdRuntimeBackendV1 {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.launch_gate = KfdRuntimeLaunchGateV1::ExactGfx942InplaceTransform(
            crate::qualification_gfx942_inplace_transform_v1::admit_gfx942_inplace_transform_qualification_v1().unwrap());
        backend
    }

    #[test]
    fn qualified_runtime_shape_never_substitutes_for_native_authority() {
        let backend = qualified_without_native_queue();
        assert!(backend.r66_runtime_roster_shape_v1());
        assert!(backend.observe_r66_retained_custody_v1().is_none());
        assert!(backend.queue.is_none());
        assert!(!backend.terminal);
    }

    #[test]
    fn runtime_shape_rejects_terminal_retired_unsupported_and_incomplete_rosters() {
        let mut backend = qualified_without_native_queue();
        backend.terminal = true;
        assert!(!backend.r66_runtime_roster_shape_v1());
        backend.terminal = false;
        backend.queue_retired = true;
        assert!(!backend.r66_runtime_roster_shape_v1());
        backend.queue_retired = false;
        backend.published_sdma_submissions.push(17);
        assert!(!backend.r66_runtime_roster_shape_v1());
        backend.published_sdma_submissions.clear();
        assert!(backend.r66_runtime_roster_shape_v1());
        backend.submissions.insert(
            17,
            SubmissionRecordV1 {
                stream: 19,
                status: BackendPollV1::Pending,
                profile_dispatch_published: false,
            },
        );
        assert!(!backend.r66_runtime_roster_shape_v1());
        backend.submissions.clear();
        backend.launch_gate = KfdRuntimeLaunchGateV1::Production(Box::new(TestAuthorityV1));
        assert!(!backend.r66_runtime_roster_shape_v1());
        assert!(backend.observe_r66_retained_custody_v1().is_none());
    }

    #[test]
    fn unaccounted_native_storage_markers_are_not_an_empty_observation() {
        let mut backend = qualified_without_native_queue();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap();
        for storage in [
            KfdRuntimeSdmaStorageV1::ComputeInFlight(17),
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Async(17)),
            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::Synchronous),
        ] {
            backend
                .allocations
                .get_mut(&allocation)
                .unwrap()
                .sdma_storage = storage;
            assert!(!backend.r66_runtime_roster_shape_v1());
            assert!(backend.observe_r66_retained_custody_v1().is_none());
        }
        backend
            .allocations
            .get_mut(&allocation)
            .unwrap()
            .sdma_storage = KfdRuntimeSdmaStorageV1::Synthetic;
        backend.release_allocation_v1(allocation).unwrap();
        assert!(backend.r66_runtime_roster_shape_v1());
    }
    #[test]
    fn membership_binds_every_coordinate_and_native_occurrence() {
        let baseline = membership(b"copy\0", [1; 32], &[7, 8, 9, 10, 11, 12, 13]);
        for index in 0..7 {
            let mut values = [7, 8, 9, 10, 11, 12, 13];
            values[index] += 1;
            assert_ne!(baseline, membership(b"copy\0", [1; 32], &values));
        }
        assert_ne!(
            baseline,
            membership(b"compute\0", [1; 32], &[7, 8, 9, 10, 11, 12, 13])
        );
        assert_ne!(
            baseline,
            membership(b"copy\0", [2; 32], &[7, 8, 9, 10, 11, 12, 13])
        );
    }
}
