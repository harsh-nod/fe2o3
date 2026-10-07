use super::*;

impl KfdMultiDeviceRuntimeBackendV1 {
    #[cfg(all(test, feature = "hardware-qualification"))]
    pub(crate) fn qualification_xgmi_test_backend_v1() -> Self {
        let source = KfdRuntimeBackendV1::mock();
        let mut destination = KfdRuntimeBackendV1::mock();
        destination.description.backend_device = source.description.backend_device + 1;
        Self::from_backends(vec![source, destination]).unwrap()
    }

    #[cfg(feature = "hardware-qualification")]
    pub(super) fn qualification_xgmi_slots_intact_v1(
        &self,
        copy: &CooperativeCopySubmissionV1,
        restored: bool,
    ) -> bool {
        let Some(root) = copy.compute_xgmi.as_ref() else {
            return false;
        };
        if copy.source.child == copy.destination.child
            || self.allocations.get(&copy.source_region.allocation) != Some(&copy.source)
            || self.allocations.get(&copy.destination_region.allocation) != Some(&copy.destination)
        {
            return false;
        }
        // This qualification-only validation never normalizes or extracts an owner.
        for endpoint in [copy.source, copy.destination] {
            let Some(child) = self.children.get(endpoint.child) else {
                return false;
            };
            let Some(record) = child.allocations.get(&endpoint.local) else {
                return false;
            };
            if child.terminal
                || !child.peer_visible_device_allocations
                || self.compute_xgmi_children.get(endpoint.child) != Some(&None)
                || !record.sdma_initialized
                || record.persistent_storage_restore.is_some()
            {
                return false;
            }
            let native_slot = match &record.sdma_storage {
                KfdRuntimeSdmaStorageV1::Device(owner) => {
                    matches!(&**owner, DirectionalSdmaDeviceOwnerV1::Native(_))
                }
                KfdRuntimeSdmaStorageV1::H2dReady(ready) if !restored => {
                    matches!(&ready.owner, PersistentComputeReadyOwnerV1::Native(_))
                }
                KfdRuntimeSdmaStorageV1::InitializedStorage(owner) if !restored => {
                    matches!(&**owner, InitializedStorageOwnerV1::Native(_))
                }
                KfdRuntimeSdmaStorageV1::PersistentReplay(_) if !restored => true,
                _ => false,
            };
            if !native_slot {
                return false;
            }
        }
        let records = [
            &self.children[copy.source.child].allocations[&copy.source.local],
            &self.children[copy.destination.child].allocations[&copy.destination.local],
        ];
        root.accepts_region(records[0], copy.source_region)
            && root.accepts_region(records[1], copy.destination_region)
            && root.matches_regions(
                records[0],
                copy.source_region,
                records[1],
                copy.destination_region,
            )
    }

    /// Arms only this accepted, unstarted native operation. No progress occurs here.
    #[cfg(feature = "hardware-qualification")]
    pub(crate) fn reject_native_xgmi_host_preparation_once_for_qualification_v1(
        &mut self,
        submission: u64,
    ) -> Result<(), Failure> {
        self.require_live()?;
        let Some(record) = self.submissions.get(&submission) else {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown qualification submission",
            ));
        };
        let accepted = matches!(record, RoutedSubmissionV1::CooperativeCopy(copy)
            if copy.directed.is_none()
                && copy.status() == BackendPollV1::Pending
                && copy.compute_xgmi.as_ref().is_some_and(|root| {
                    matches!(root.route, Route::Native(_))
                        && root.phase == Phase::Prepared
                        && root.qualification_host_preparation == QualificationHostPreparationV1::Unrequested
                        && root.is_quiescent()
                        && root.shells.iter().all(Option::is_some)
                        && root.no_effect_error.is_none()
                })
                && self.streams.get(&copy.stream).is_some_and(|stream| stream.child == copy.destination.child)
                && self.qualification_xgmi_slots_intact_v1(copy, false));
        if !accepted {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::Unsupported,
                "qualification denial requires an unarmed prepared native peer with original owners",
            ));
        }
        let Some(RoutedSubmissionV1::CooperativeCopy(copy)) = self.submissions.get_mut(&submission)
        else {
            unreachable!("validated qualification submission remains indexed")
        };
        copy.compute_xgmi
            .as_mut()
            .unwrap()
            .qualification_host_preparation = QualificationHostPreparationV1::Armed;
        Ok(())
    }

    /// Historical outcome for this retained result, not permission to reuse memory.
    #[cfg(feature = "hardware-qualification")]
    pub(crate) fn native_xgmi_host_preparation_rejected_for_qualification_v1(
        &self,
        submission: u64,
    ) -> Result<bool, Failure> {
        self.require_live()?;
        let Some(record) = self.submissions.get(&submission) else {
            return Err(KfdRuntimeBackendV1::rejected(
                KfdRuntimeBackendErrorKindV1::UnknownHandle,
                "unknown qualification submission",
            ));
        };
        Ok(matches!(record, RoutedSubmissionV1::CooperativeCopy(copy)
            if copy.directed.is_none()
                && copy.phase == CooperativeCopyPhaseV1::Failed
                && copy.is_quiescent()
                && copy.compute_xgmi.as_ref().is_some_and(|root| {
                    matches!(root.route, Route::Native(_))
                        && root.qualification_host_preparation == QualificationHostPreparationV1::Certified
                        && root.phase == Phase::Retired
                        && root.no_effect_error.is_some()
                        && root.is_quiescent()
                        && root.shells.iter().all(Option::is_none)
                })
                && self.qualification_xgmi_slots_intact_v1(copy, true)))
    }
}
