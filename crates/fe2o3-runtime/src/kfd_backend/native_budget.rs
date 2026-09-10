//! Immutable session-local backing and device-cache limits, not aggregate accounting.

use super::*;
use fe2o3_kfd::{
    Gfx942DeviceBackingUsageV1, Gfx942DevicePoolUsageV1, Gfx942HostVisibleBackingUsageV1,
};

#[cfg(test)]
mod host_backing_tests;

impl KfdRuntimeBackendV1 {
    /// Selects immutable ordinary coherent GTT backing limits before resource use.
    ///
    /// Charges actual page-padded backing and one native record, including ordinary
    /// coherent bootstrap allocations. Userptr, executable/AQL profiles, complete
    /// bootstrap, metadata, other sessions and aggregate budgets remain separate.
    pub fn configure_host_visible_backing_budget_v1(
        &mut self,
        budget: Gfx942HostVisibleBackingBudgetV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_pristine_native_resource_configuration_v1()?;
        if self.host_visible_backing_budget.is_some() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "host-visible backing limits must be configured once before resource creation",
            ));
        }
        self.host_visible_backing_budget = Some(budget);
        Ok(())
    }

    /// Reports selected limits, not evidence that a native account exists.
    pub const fn host_visible_backing_budget_v1(&self) -> Option<Gfx942HostVisibleBackingBudgetV1> {
        self.host_visible_backing_budget
    }

    /// Observes the retained native account without progress or disposal authority.
    /// `None` means unavailable or unconfigured, never proof of a refund.
    pub fn host_visible_backing_usage_v1(&self) -> Option<Gfx942HostVisibleBackingUsageV1> {
        self.queue
            .as_ref()
            .and_then(ComputeAqlQueueSessionV1::host_visible_backing_usage_v1)
            .or_else(|| {
                self.terminal_memory
                    .as_ref()
                    .and_then(SharedGttMemorySessionV1::host_visible_backing_usage_v1)
            })
    }

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
        self.require_pristine_native_resource_configuration_v1()?;
        if self.device_backing_budget.is_some() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "native backing limits must be configured once before resource creation",
            ));
        }
        self.device_backing_budget = Some(budget);
        Ok(())
    }

    fn require_pristine_native_resource_configuration_v1(
        &self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_live()?;
        if self.next_handle != 1
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
                "native resource limits must precede resource creation",
            ));
        }
        Ok(())
    }

    /// Bounds cached-free device backing in the existing native SDMA pool.
    ///
    /// Configure once before any logical/native resource history. Either zero
    /// limit disables device caching, not device allocation. Cache overflow
    /// disposes the returned idle buffer through the existing native release
    /// path; uncertain disposal never refunds its N2 backing charge. Host pools,
    /// checked-out backing, metadata and aggregate budgets are separate.
    pub fn configure_device_pool_limits_v1(
        &mut self,
        limits: Gfx942DevicePoolLimitsV1,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.require_pristine_native_resource_configuration_v1()?;
        if self.device_pool_limits.is_some() {
            return Err(Self::rejected(
                KfdRuntimeBackendErrorKindV1::Busy,
                "device pool limits must be configured once before resource creation",
            ));
        }
        self.device_pool_limits = Some(limits);
        Ok(())
    }

    pub const fn device_pool_limits_v1(&self) -> Option<Gfx942DevicePoolLimitsV1> {
        self.device_pool_limits
    }

    /// Observes cached-free device backing, not total resident or disposed bytes.
    /// `None` means unavailable or unconfigured, never a zero-usage certificate.
    pub fn device_pool_usage_v1(
        &self,
    ) -> Result<Option<Gfx942DevicePoolUsageV1>, KfdRuntimeBackendErrorV1> {
        if self.terminal {
            return Err(KfdRuntimeBackendErrorV1::new(
                KfdRuntimeBackendErrorKindV1::Terminal,
                "KFD backend is terminal",
            ));
        }
        self.queue.as_ref().map_or(Ok(None), |queue| {
            queue.sdma_device_pool_usage_v1().map_err(|error| {
                KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Native,
                    format!("KFD device pool observation: {error}"),
                )
            })
        })
    }

    pub(super) fn configure_native_device_pool_v1(
        &mut self,
    ) -> Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        let Some(limits) = self.device_pool_limits else {
            return Ok(());
        };
        self.queue
            .as_mut()
            .expect("new native queue remains retained before pool configuration")
            .configure_sdma_device_pool_v1(limits)
            .map_err(|error| self.terminal_error(format!("KFD device pool configuration: {error}")))
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
        assert!(sdma.contains(".create_compute_aql_queue_with_backing_budgets_v1("));
        assert!(sdma.contains("self.device_backing_budget,"));
        assert!(!sdma.contains(".create_compute_aql_queue("));

        let compute = include_str!("compute_dispatch.rs");
        let acquire = compute
            .find(".acquire_shared_gtt_memory_session_with_backing_budgets_v1(")
            .unwrap();
        let materialize = compute[acquire..]
            .find("materialize_initial_data_v1(")
            .unwrap();
        assert!(compute[acquire..acquire + materialize].contains("self.device_backing_budget,"));
        assert!(!compute.contains(".acquire_shared_gtt_memory_session()"));
    }

    fn pool_limits() -> Gfx942DevicePoolLimitsV1 {
        Gfx942DevicePoolLimitsV1::new(8192, 2).unwrap()
    }

    #[test]
    fn device_pool_defaults_and_configuration_do_not_imply_native_residency() {
        let mut backend = KfdRuntimeBackendV1::mock();
        assert_eq!(backend.device_pool_limits_v1(), None);
        assert_eq!(backend.device_pool_usage_v1().unwrap(), None);
        let capabilities = backend.description.capabilities;
        backend
            .configure_device_pool_limits_v1(pool_limits())
            .unwrap();
        assert_eq!(backend.device_pool_limits_v1(), Some(pool_limits()));
        assert_eq!(backend.device_pool_usage_v1().unwrap(), None);
        assert_eq!(backend.device_backing_budget_v1(), None);
        assert_eq!(backend.description.capabilities, capabilities);
        assert_eq!(backend.next_handle, 1);
        assert_busy(backend.configure_device_pool_limits_v1(pool_limits()));
        assert_busy(
            backend.configure_device_pool_limits_v1(Gfx942DevicePoolLimitsV1::new(0, 0).unwrap()),
        );
        assert_eq!(backend.device_pool_limits_v1(), Some(pool_limits()));
    }

    #[test]
    fn device_pool_and_backing_limits_are_independent_in_both_configuration_orders() {
        for backing_first in [false, true] {
            let mut backend = KfdRuntimeBackendV1::mock();
            if backing_first {
                backend
                    .configure_device_backing_budget_v1(budget())
                    .unwrap();
            }
            backend
                .configure_device_pool_limits_v1(pool_limits())
                .unwrap();
            if !backing_first {
                backend
                    .configure_device_backing_budget_v1(budget())
                    .unwrap();
            }
            assert_eq!(backend.device_backing_budget_v1(), Some(budget()));
            assert_eq!(backend.device_pool_limits_v1(), Some(pool_limits()));
            assert_busy(backend.configure_device_backing_budget_v1(budget()));
            assert_busy(backend.configure_device_pool_limits_v1(pool_limits()));
            assert!(backend.queue.is_none());
        }
    }

    #[test]
    fn device_pool_limits_reject_live_released_and_native_resource_history() {
        let mut backend = KfdRuntimeBackendV1::mock();
        let stream = backend.create_stream_v1(7).unwrap();
        assert_busy(backend.configure_device_pool_limits_v1(pool_limits()));
        backend.destroy_stream_v1(stream).unwrap();
        assert_busy(backend.configure_device_pool_limits_v1(pool_limits()));
        assert_eq!(backend.device_pool_limits_v1(), None);
        let mut allocated = KfdRuntimeBackendV1::mock();
        let allocation = allocated
            .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
            .unwrap();
        allocated.release_allocation_v1(allocation).unwrap();
        assert_busy(allocated.configure_device_pool_limits_v1(pool_limits()));
        let mut retired = KfdRuntimeBackendV1::mock();
        retired.queue_retired = true;
        assert_busy(retired.configure_device_pool_limits_v1(pool_limits()));
    }

    #[test]
    fn zero_device_cache_limits_do_not_disable_or_replace_backing_admission() {
        for limits in [
            Gfx942DevicePoolLimitsV1::new(0, 2).unwrap(),
            Gfx942DevicePoolLimitsV1::new(8192, 0).unwrap(),
        ] {
            let mut backend = KfdRuntimeBackendV1::mock();
            backend
                .configure_device_backing_budget_v1(budget())
                .unwrap();
            backend.configure_device_pool_limits_v1(limits).unwrap();
            assert_eq!(backend.device_pool_limits_v1(), Some(limits));
            assert_eq!(backend.device_backing_budget_v1(), Some(budget()));
            let allocation = backend
                .allocate_v1(7, RuntimeMemoryKindV1::DeviceLocal, 16, 8)
                .unwrap();
            backend.release_allocation_v1(allocation).unwrap();
        }
    }

    #[test]
    fn terminal_device_pool_observation_does_not_report_empty_or_reopen_configuration() {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.terminal = true;
        assert!(matches!(
            backend.configure_device_pool_limits_v1(pool_limits()),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert_eq!(
            backend.device_pool_usage_v1().unwrap_err().kind(),
            KfdRuntimeBackendErrorKindV1::Terminal
        );
        assert_eq!(backend.device_pool_limits_v1(), None);
        // Terminal Drop deliberately aborts; this mock has no native resources.
        core::mem::forget(backend);
    }

    #[test]
    fn device_pool_forwarding_retains_queue_before_configuration_in_both_startup_paths() {
        // Source wiring complements the native fake-record/ownership tests.
        for (source, start, end, sdma_first) in [
            (
                include_str!("../kfd_backend.rs"),
                "    fn ensure_sdma_queue_v1(",
                "    fn directional_sdma_ops_v1(",
                true,
            ),
            (
                include_str!("compute_dispatch.rs"),
                "    pub(super) fn publish(",
                "    pub(super) fn observe_materialized_dispatch_published_v1(",
                false,
            ),
        ] {
            let function = source
                .split_once(start)
                .unwrap()
                .1
                .split_once(end)
                .unwrap()
                .0;
            assert_eq!(
                function
                    .matches("self.configure_native_device_pool_v1()?;")
                    .count(),
                1
            );
            let (before, after) = function
                .split_once("self.configure_native_device_pool_v1()?;")
                .unwrap();
            assert!(before.trim_end().ends_with("self.queue = Some(queue);"));
            let enable = ".enable_gfx942_directional_sdma_copy_engines()";
            assert!(!before.contains(enable));
            if sdma_first {
                assert_eq!(after.matches(enable).count(), 1);
            } else {
                assert!(!after.contains(enable));
                assert!(after.contains(
                    "self.native_compute_lanes[self.selected_compute_lane] = Some(primary_lane);"
                ));
            }
        }
    }
}
