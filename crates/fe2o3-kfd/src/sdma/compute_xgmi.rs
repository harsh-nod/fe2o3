//! Borrowed custody across publication, completion, and route checks.

use super::*;

impl Gfx942SdmaBufferV1 {
    #[cfg(test)]
    pub(crate) fn compute_xgmi_fixture_v1(
        lease: Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>,
        owner: QueueKeyV1,
        generation: u64,
        logical_bytes: u64,
        initialized_prefix: u64,
    ) -> Self {
        Self {
            storage: Gfx942SdmaBufferStorageV1::Device(lease),
            owner,
            pool_generation: generation,
            logical_bytes,
            host_content_certificate: None,
            initialized_prefix,
        }
    }

    pub(crate) fn restore_compute_xgmi_local_v1(
        local: &mut Option<Gfx942DeviceMemoryLeaseV1<Gfx942DeviceMemoryMappedV1>>,
        metadata: &mut Option<Gfx942SdmaBufferCleanupMetadataV1>,
    ) -> Result<Self, Gfx942SdmaErrorV1> {
        let lease = local.as_ref().ok_or(Gfx942SdmaErrorV1::Contract(
            "missing persistent compute-XGMI local mapping",
        ))?;
        let retained = metadata.as_ref().ok_or(Gfx942SdmaErrorV1::Contract(
            "missing persistent compute-XGMI buffer metadata",
        ))?;
        if retained.identity != Gfx942SdmaBufferStorageIdentityV1::Device(lease.storage_identity())
            || retained.physical_bytes != lease.layout().requested_bytes()
            || retained.physical_alignment != lease.layout().alignment()
            || retained.logical_bytes == 0
            || retained.logical_bytes > retained.physical_bytes
            || retained.initialized_prefix != retained.logical_bytes
            || retained.host_content_certificate.is_some()
            || lease.layout().uapi_flags()
                != fe2o3_kfd_uapi::KFD_ALLOC_MEMORY_FLAGS_DEVICE_LOCAL_PUBLIC
        {
            return Err(Gfx942SdmaErrorV1::Contract(
                "persistent compute-XGMI buffer restoration mismatch",
            ));
        }
        let retained = metadata.take().unwrap_or_else(|| std::process::abort());
        Ok(Self {
            storage: Gfx942SdmaBufferStorageV1::Device(
                local.take().unwrap_or_else(|| std::process::abort()),
            ),
            owner: retained.owner,
            pool_generation: retained.pool_generation,
            logical_bytes: retained.logical_bytes,
            host_content_certificate: None,
            initialized_prefix: retained.initialized_prefix,
        })
    }
}

#[derive(Default)]
pub(crate) struct ComputeXgmiCopyCustodyV1 {
    pub(crate) ticket: Option<Gfx942SdmaCopyTicketV1>,
    pub(crate) completed: Option<Gfx942XgmiCompletedCopyV1>,
}

impl ComputeXgmiCopyCustodyV1 {
    pub(super) fn retain_poll_then_check(
        &mut self,
        polled: Result<Gfx942XgmiCopyPollV1, Gfx942SdmaErrorV1>,
        closing_check: impl FnOnce() -> Result<(), Gfx942SdmaErrorV1>,
    ) -> Result<bool, Gfx942SdmaErrorV1> {
        let ready = match polled? {
            Gfx942XgmiCopyPollV1::Pending(ticket) => {
                if self.ticket != Some(ticket) {
                    return Err(Gfx942SdmaErrorV1::Contract(
                        "compute-XGMI pending ticket changed",
                    ));
                }
                false
            }
            Gfx942XgmiCopyPollV1::Completed(completed) => {
                self.completed = Some(completed);
                true
            }
        };
        // Completed mappings remain rooted if the closing check errors or unwinds.
        closing_check()?;
        Ok(ready)
    }

    fn retain_completion_then_check(
        &mut self,
        completed: Result<Gfx942XgmiCompletedCopyV1, Gfx942SdmaErrorV1>,
        closing_check: impl FnOnce() -> Result<(), Gfx942SdmaErrorV1>,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        self.completed = Some(completed?);
        closing_check()
    }
}

impl Gfx942NativeXgmiSdmaQueueV1 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn submit_compute_xgmi_rooted_v1(
        &mut self,
        source_session: &mut SharedGttMemorySessionV1,
        destination_session: &mut SharedGttMemorySessionV1,
        source: &mut Option<Gfx942XgmiMappedDeviceMemoryV1>,
        destination: &mut Option<Gfx942XgmiMappedDeviceMemoryV1>,
        copy_bytes: u32,
        custody: &mut ComputeXgmiCopyCustodyV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        self.require_live_queue_state_v1()?;
        if custody.ticket.is_some() || custody.completed.is_some() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "occupied compute-XGMI copy custody",
            ));
        }
        Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            XgmiRouteCurrentnessV1::Full,
        )?;
        let source_mapping = source
            .as_ref()
            .ok_or(Gfx942SdmaErrorV1::Contract("missing compute-XGMI source"))?;
        let destination_mapping = destination.as_ref().ok_or(Gfx942SdmaErrorV1::Contract(
            "missing compute-XGMI destination",
        ))?;
        if source_mapping.gpu_ids() != self.route.canonical_mapping_gpu_ids()
            || destination_mapping.gpu_ids() != self.route.canonical_mapping_gpu_ids()
        {
            return Err(Gfx942SdmaErrorV1::Contract("compute-XGMI mapping roster"));
        }
        let source_address = source_session
            .mapped_xgmi_device_memory_facts(source_mapping)?
            .checked_gpu_subrange(0, u64::from(copy_bytes), 1)
            .ok_or(Gfx942SdmaErrorV1::Contract("compute-XGMI source extent"))?;
        let destination_address = destination_session
            .mapped_xgmi_device_memory_facts(destination_mapping)?
            .checked_gpu_subrange(0, u64::from(copy_bytes), 1)
            .ok_or(Gfx942SdmaErrorV1::Contract(
                "compute-XGMI destination extent",
            ))?;
        let owner = self.owner.as_mut().ok_or(Gfx942SdmaErrorV1::Contract(
            "missing compute-XGMI queue owner",
        ))?;
        match owner.submit_xgmi(
            source_session,
            source,
            source_address,
            destination,
            destination_address,
            copy_bytes,
        ) {
            Ok(ticket) => custody.ticket = Some(ticket),
            Err(error) => {
                custody.ticket = owner.uncertain_xgmi_ticket();
                return Err(error);
            }
        }
        // The ticket and both mappings are rooted before a closing check can unwind.
        Self::validate_route_currentness(
            source_session,
            destination_session,
            self.route,
            XgmiRouteCurrentnessV1::Full,
        )
    }

    pub(crate) fn poll_compute_xgmi_rooted_v1(
        &mut self,
        source: &mut SharedGttMemorySessionV1,
        destination: &mut SharedGttMemorySessionV1,
        custody: &mut ComputeXgmiCopyCustodyV1,
    ) -> Result<bool, Gfx942SdmaErrorV1> {
        self.require_live_queue_state_v1()?;
        let ticket = custody
            .ticket
            .ok_or(Gfx942SdmaErrorV1::Contract("missing compute-XGMI ticket"))?;
        if custody.completed.is_some() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "occupied compute-XGMI completion",
            ));
        }
        Self::validate_route_currentness(
            source,
            destination,
            self.route,
            XgmiRouteCurrentnessV1::Full,
        )?;
        let polled = self
            .owner
            .as_mut()
            .ok_or(Gfx942SdmaErrorV1::Contract(
                "missing compute-XGMI queue owner",
            ))?
            .poll_xgmi_in_current_scope(source, ticket);
        custody.retain_poll_then_check(polled, || {
            Self::validate_route_currentness(
                source,
                destination,
                self.route,
                XgmiRouteCurrentnessV1::Full,
            )
        })
    }

    pub(crate) fn wait_compute_xgmi_rooted_v1(
        &mut self,
        source: &mut SharedGttMemorySessionV1,
        destination: &mut SharedGttMemorySessionV1,
        timeout: Duration,
        custody: &mut ComputeXgmiCopyCustodyV1,
    ) -> Result<(), Gfx942SdmaErrorV1> {
        self.require_live_queue_state_v1()?;
        let ticket = custody
            .ticket
            .ok_or(Gfx942SdmaErrorV1::Contract("missing compute-XGMI ticket"))?;
        if custody.completed.is_some() {
            return Err(Gfx942SdmaErrorV1::Contract(
                "occupied compute-XGMI completion",
            ));
        }
        Self::validate_route_currentness(
            source,
            destination,
            self.route,
            XgmiRouteCurrentnessV1::Full,
        )?;
        let completed = self
            .owner
            .as_mut()
            .ok_or(Gfx942SdmaErrorV1::Contract(
                "missing compute-XGMI queue owner",
            ))?
            .wait_xgmi_for_in_current_scope(
                source,
                ticket,
                XgmiSingleDeadlineV1::Relative(timeout),
            );
        custody.retain_completion_then_check(completed, || {
            Self::validate_route_currentness(
                source,
                destination,
                self.route,
                XgmiRouteCurrentnessV1::Full,
            )
        })
    }

    pub(crate) fn poison_compute_xgmi_transfer_v1(&mut self) {
        self.poison_for_abandoned_batch();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{AssertUnwindSafe, catch_unwind};

    #[test]
    fn compute_xgmi_completed_mappings_survive_closing_error_and_unwind() {
        for fault in 0..3 {
            let completed = Gfx942XgmiCompletedCopyV1 {
                source: crate::shared_memory::xgmi_mapping_for_sdma_test(11),
                destination: crate::shared_memory::xgmi_mapping_for_sdma_test(12),
                copy_bytes: 4096,
            };
            let identities = [
                completed.source.lease().storage_identity(),
                completed.destination.lease().storage_identity(),
            ];
            let mut custody = ComputeXgmiCopyCustodyV1::default();
            let result = catch_unwind(AssertUnwindSafe(|| {
                custody.retain_completion_then_check(Ok(completed), || match fault {
                    0 => Ok(()),
                    1 => Err(Gfx942SdmaErrorV1::Contract("closing currentness")),
                    _ => panic!("closing currentness"),
                })
            }));
            assert_eq!(result.is_err(), fault == 2);
            if let Ok(result) = result {
                assert_eq!(result.is_ok(), fault == 0);
            }
            let retained = custody.completed.as_ref().unwrap();
            assert_eq!(retained.source.lease().storage_identity(), identities[0]);
            assert_eq!(
                retained.destination.lease().storage_identity(),
                identities[1]
            );
            assert_eq!(retained.copy_bytes(), 4096);
        }
    }

    #[test]
    fn compute_xgmi_timeout_never_mints_completed_mapping_custody() {
        let mut custody = ComputeXgmiCopyCustodyV1::default();
        assert!(matches!(
            custody.retain_completion_then_check(Err(Gfx942SdmaErrorV1::Timeout), || panic!(
                "timeout must not publish completed mappings"
            )),
            Err(Gfx942SdmaErrorV1::Timeout)
        ));
        assert!(custody.completed.is_none());
    }
}
