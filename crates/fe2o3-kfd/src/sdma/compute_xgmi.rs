//! Borrowed custody across publication, completion, and route checks.

use super::*;

#[derive(Default)]
pub(crate) struct ComputeXgmiCopyCustodyV1 {
    pub(crate) ticket: Option<Gfx942SdmaCopyTicketV1>,
    pub(crate) completed: Option<Gfx942XgmiCompletedCopyV1>,
}

impl ComputeXgmiCopyCustodyV1 {
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
