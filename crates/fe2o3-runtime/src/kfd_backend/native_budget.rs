//! Immutable forwarding of session-local N2 limits; not aggregate accounting.

use super::*;
use fe2o3_kfd::Gfx942DeviceBackingUsageV1;

impl KfdRuntimeBackendV1 {
    /// Selects immutable N2 backing limits before logical or native resource use.
    ///
    /// The selected limits are installed in the actual native memory session
    /// before either startup path can allocate device backing or certify a queue.
    /// They measure padded N2 bytes and backing records, not logical allocation
    /// bytes, GTT, queue/control storage, other sessions or a global budget.
    /// Leaving this unconfigured preserves the existing default behavior.
    pub fn configure_device_backing_budget_v1(
        &mut self,
        budget: Gfx942DeviceBackingBudgetV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.device_backing_budget.is_some()
            || self.next_handle != 1
            || self.queue.is_some()
            || self.queue_retired
            || self.terminal_memory.is_some()
            || self.terminal_sdma_custody.is_some()
            || self.sdma_enabled
            || !self.streams.is_empty()
            || !self.allocations.is_empty()
            || !self.modules.is_empty()
            || !self.kernels.is_empty()
            || !self.submissions.is_empty()
            || !self.events.is_empty()
            || self.any_compute_active_v1()
        {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native backing limits must be configured once before resource creation",
            ));
        }
        self.device_backing_budget = Some(budget);
        Ok(())
    }

    /// Reports the configured limits, not evidence of native account creation.
    pub const fn device_backing_budget_v1(&self) -> Option<Gfx942DeviceBackingBudgetV1> {
        self.device_backing_budget
    }

    /// Observes the retained native session's N2 account when available.
    ///
    /// `None` means no observation is available, not zero usage or disposal.
    /// In particular, merely configuring limits does not create a native account,
    /// and lost access to quarantined custody does not establish a refund.
    pub fn device_backing_usage_v1(&self) -> Option<Gfx942DeviceBackingUsageV1> {
        self.queue
            .as_ref()
            .and_then(ComputeAqlQueueSessionV1::device_backing_usage_v1)
            .or_else(|| {
                self.terminal_memory
                    .as_ref()
                    .and_then(SharedGttMemorySessionV1::device_backing_usage_v1)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn budget() -> Gfx942DeviceBackingBudgetV1 {
        Gfx942DeviceBackingBudgetV1::new(64 * 1024, 4).unwrap()
    }

    fn assert_busy(result: Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>) {
        assert!(matches!(result,
            Err(RuntimeBackendFailureV1::Rejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::Busy));
    }

    #[test]
    fn native_budget_default_is_unconfigured_and_no_account_is_implied() {
        let backend = KfdRuntimeBackendV1::mock();
        assert_eq!(backend.device_backing_budget_v1(), None);
        assert_eq!(backend.device_backing_usage_v1(), None);
    }

    #[test]
    fn native_budget_configuration_preserves_launch_capabilities_and_is_immutable() {
        for mut backend in [
            KfdRuntimeBackendV1::mock(),
            KfdRuntimeBackendV1::mock_with_semantic_authority_v1(),
        ] {
            let capabilities = backend.description.capabilities;
            backend
                .configure_device_backing_budget_v1(budget())
                .unwrap();
            assert_eq!(backend.device_backing_budget_v1(), Some(budget()));
            assert_eq!(backend.description.capabilities, capabilities);
            assert_eq!(backend.device_backing_usage_v1(), None);
            assert_eq!(backend.next_handle, 1);
            assert!(!backend.native_available);
            assert!(backend.queue.is_none());
            assert_busy(backend.configure_device_backing_budget_v1(budget()));
            assert_busy(backend.configure_device_backing_budget_v1(
                Gfx942DeviceBackingBudgetV1::new(128 * 1024, 8).unwrap(),
            ));
            assert_eq!(backend.device_backing_budget_v1(), Some(budget()));
        }
    }

    #[test]
    fn native_budget_configuration_rejects_live_and_released_logical_resources() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        assert_busy(backend.configure_device_backing_budget_v1(budget()));
        backend.destroy_stream_v1(stream).unwrap();
        assert_busy(backend.configure_device_backing_budget_v1(budget()));
        assert_eq!(backend.device_backing_budget_v1(), None);

        for kind in [
            RuntimeMemoryKindV1::HostVisible,
            RuntimeMemoryKindV1::DeviceLocal,
        ] {
            let mut backend = KfdRuntimeBackendV1::mock();
            let allocation = backend.allocate_v1(7, kind, 32, 8).unwrap();
            assert_busy(backend.configure_device_backing_budget_v1(budget()));
            backend.release_allocation_v1(allocation).unwrap();
            assert_busy(backend.configure_device_backing_budget_v1(budget()));
            assert_eq!(backend.device_backing_budget_v1(), None);
        }
    }

    #[test]
    fn native_budget_configuration_rejects_native_lifecycle_history() {
        let mut retired = KfdRuntimeBackendV1::mock();
        retired.queue_retired = true;
        assert_busy(retired.configure_device_backing_budget_v1(budget()));

        let mut enabled = KfdRuntimeBackendV1::mock();
        enabled.sdma_enabled = true;
        assert_busy(enabled.configure_device_backing_budget_v1(budget()));
        assert_eq!(enabled.device_backing_budget_v1(), None);
        enabled.sdma_enabled = false;
    }

    #[test]
    fn native_budget_configuration_cannot_recover_a_terminal_backend() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.terminal = true;
        assert!(matches!(
            backend.configure_device_backing_budget_v1(budget()),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(backend.device_backing_budget_v1(), None);
        assert_eq!(backend.device_backing_usage_v1(), None);
        // Terminal Drop deliberately aborts; retain this resource-free mock.
        core::mem::forget(backend);
    }

    #[test]
    fn native_budget_forwarding_is_wired_into_both_initial_startup_paths() {
        // This source-wiring check complements KFD's fake-backend ownership
        // round trips; it does not invoke a native constructor on hardware.
        let source = include_str!("../kfd_backend.rs");
        let sdma = source
            .split("    fn ensure_sdma_queue_v1(")
            .nth(1)
            .unwrap()
            .split("    fn directional_sdma_ops_v1(")
            .next()
            .unwrap();
        assert!(sdma.contains(".create_compute_aql_queue_with_device_backing_budget_v1("));
        assert!(sdma.contains("self.device_backing_budget,"));
        assert!(!sdma.contains(".create_compute_aql_queue("));

        let compute = include_str!("compute_dispatch.rs");
        let acquire = compute
            .find(".acquire_shared_gtt_memory_session_with_device_backing_budget_v1(")
            .unwrap();
        let materialize = compute[acquire..]
            .find("materialize_initial_data_v1(")
            .unwrap();
        assert!(compute[acquire..acquire + materialize].contains("self.device_backing_budget,"));
        assert!(!compute.contains(".acquire_shared_gtt_memory_session()"));
    }
}
