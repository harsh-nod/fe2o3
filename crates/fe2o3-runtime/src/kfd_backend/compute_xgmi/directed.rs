//! Native transport preserves the directed graph's provenance and bounded drive.

use super::*;

impl KfdMultiDeviceRuntimeBackendV1 {
    pub(in super::super) fn directed_native_transport_intact_v1(
        &self,
        id: u64,
        copy: &CooperativeCopySubmissionV1,
    ) -> bool {
        let Some(root) = &copy.compute_xgmi else {
            return true;
        };
        let endpoints = [copy.source, copy.destination];
        if !copy.staging.is_empty()
            || copy.scratch_byte_len != 0
            || copy.byte_cursor != 0
            || copy.sdma_leaf.is_some()
            || copy.compute_producer.is_some()
        {
            return false;
        }
        let reserved = endpoints
            .map(|endpoint| self.compute_xgmi_children.get(endpoint.child) == Some(&Some(id)));
        if copy.is_quiescent() {
            return root.is_quiescent()
                && matches!(root.phase, Phase::Prepared | Phase::Retired)
                && reserved == [false; 2];
        }
        if !endpoints
            .into_iter()
            .zip([copy.source_region, copy.destination_region])
            .all(|(endpoint, region)| {
                self.children.get(endpoint.child).is_some_and(|child| {
                    child.peer_visible_device_allocations
                        && child
                            .allocations
                            .get(&endpoint.local)
                            .is_some_and(|record| {
                                full_extent(record, region) && record.sdma_initialized
                            })
                })
            })
        {
            return false;
        }
        match (copy.phase, root.phase) {
            (CooperativeCopyPhaseV1::Dependencies | CooperativeCopyPhaseV1::Read, Phase::Prepared) => {
                root.is_quiescent() && reserved == [false; 2]
            }
            (CooperativeCopyPhaseV1::Read, Phase::Published | Phase::Ready) => {
                !root.is_quiescent()
                    && reserved == [true; 2]
                    && endpoints.into_iter().all(|endpoint| {
                        matches!(self.children[endpoint.child].allocations[&endpoint.local].sdma_storage,
                            KfdRuntimeSdmaStorageV1::InFlight(KfdRuntimeSdmaInFlightV1::ComputeXgmi(owner)) if owner == id)
                    })
            }
            _ => false,
        }
    }

    /// One already-started owner can block both native endpoints or a staged leg.
    /// Advancing it is resource progress, not a success dependency of the caller.
    pub(in super::super) fn directed_native_blocker_v1(
        &mut self,
        selected: u64,
    ) -> Result<Option<u64>, Failure> {
        let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get(&selected)
        else {
            return Ok(None);
        };
        let endpoints = match copy.phase {
            CooperativeCopyPhaseV1::Read if copy.compute_xgmi.is_some() => {
                [Some(copy.source.child), Some(copy.destination.child)]
            }
            CooperativeCopyPhaseV1::Read => [Some(copy.source.child), None],
            CooperativeCopyPhaseV1::Write => [Some(copy.destination.child), None],
            _ => return Ok(None),
        };
        let mut blocker = None;
        for child in endpoints.into_iter().flatten() {
            let Some(owner) = self.compute_xgmi_children[child] else {
                continue;
            };
            if owner == selected {
                continue;
            }
            let Some(RoutedSubmissionV1::CooperativeCopy(other)) = self.submissions.get(&owner)
            else {
                return Err(self.directed_corruption_v1());
            };
            if other.directed.is_none() {
                continue;
            }
            if other.phase != CooperativeCopyPhaseV1::Read
                || ![other.source.child, other.destination.child].contains(&child)
                || other
                    .compute_xgmi
                    .as_ref()
                    .is_none_or(|root| root.is_quiescent())
                || !self.directed_identity_is_intact_v1(owner)
            {
                return Err(self.directed_corruption_v1());
            }
            blocker = Some(blocker.map_or(owner, |prior: u64| prior.min(owner)));
        }
        Ok(blocker)
    }
}
