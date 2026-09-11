use super::*;

fn limits() -> Gfx942HostPoolLimitsV1 {
    Gfx942HostPoolLimitsV1::new(8192, 2).unwrap()
}

#[test]
fn host_pool_four_independent_configurations_accept_every_order_and_cannot_be_replaced() {
    let mut tested = 0;
    for a in 0..4 {
        for b in 0..4 {
            for c in 0..4 {
                for d in 0..4 {
                    let order = [a, b, c, d];
                    if order
                        .iter()
                        .enumerate()
                        .any(|(index, value)| order[..index].contains(value))
                    {
                        continue;
                    }
                    let mut backend = KfdRuntimeBackendV1::mock();
                    assert_eq!(backend.host_pool_limits_v1(), None);
                    assert_eq!(backend.host_pool_usage_v1().unwrap(), None);
                    for operation in order {
                        match operation {
                            0 => backend.configure_host_pool_limits_v1(limits()).unwrap(),
                            1 => backend
                                .configure_device_pool_limits_v1(
                                    Gfx942DevicePoolLimitsV1::new(8192, 2).unwrap(),
                                )
                                .unwrap(),
                            2 => backend
                                .configure_host_visible_backing_budget_v1(
                                    Gfx942HostVisibleBackingBudgetV1::new(16384, 4).unwrap(),
                                )
                                .unwrap(),
                            3 => backend
                                .configure_device_backing_budget_v1(
                                    Gfx942DeviceBackingBudgetV1::new(16384, 4).unwrap(),
                                )
                                .unwrap(),
                            _ => unreachable!(),
                        }
                    }
                    assert_eq!(backend.host_pool_limits_v1(), Some(limits()));
                    assert_eq!(backend.host_pool_usage_v1().unwrap(), None);
                    assert!(backend.queue.is_none());
                    for replacement in [limits(), Gfx942HostPoolLimitsV1::new(0, 0).unwrap()] {
                        assert!(
                            matches!(backend.configure_host_pool_limits_v1(replacement), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
                        );
                    }
                    assert_eq!(backend.host_pool_limits_v1(), Some(limits()));
                    tested += 1;
                }
            }
        }
    }
    assert_eq!(tested, 24);
}

#[test]
fn host_pool_configuration_rejects_logical_and_native_history_after_release() {
    for history in 0..5 {
        let mut backend = KfdRuntimeBackendV1::mock();
        match history {
            0 => {
                let stream = backend.create_stream_v1(7).unwrap();
                backend.destroy_stream_v1(stream).unwrap();
            }
            1 => {
                let allocation = backend
                    .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
                    .unwrap();
                backend.release_allocation_v1(allocation).unwrap();
            }
            2 => backend.queue_retired = true,
            3 => backend.sdma_enabled = true,
            4 => backend.next_handle = 2,
            _ => unreachable!(),
        }
        assert!(
            matches!(backend.configure_host_pool_limits_v1(limits()), Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
        );
        assert_eq!(backend.host_pool_limits_v1(), None);
    }
}

#[test]
fn host_pool_zero_limits_leave_allocation_enabled_and_terminal_usage_is_not_zero() {
    for limits in [
        Gfx942HostPoolLimitsV1::new(0, 2).unwrap(),
        Gfx942HostPoolLimitsV1::new(8192, 0).unwrap(),
    ] {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.configure_host_pool_limits_v1(limits).unwrap();
        let allocation = backend
            .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap();
        backend.release_allocation_v1(allocation).unwrap();
        assert_eq!(backend.host_pool_limits_v1(), Some(limits));
        backend.terminal = true;
        assert_eq!(
            backend.host_pool_usage_v1().unwrap_err().kind(),
            KfdRuntimeBackendErrorKindV1::Terminal
        );
        assert!(matches!(
            backend.configure_host_pool_limits_v1(limits),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        // Terminal Drop aborts; this test backend owns no native resources.
        core::mem::forget(backend);
    }
}

#[test]
fn host_pool_both_startup_paths_retain_queue_and_forward_before_any_sdma_activity() {
    for (source, start, end, sdma_first) in [
        (
            include_str!("../../kfd_backend.rs"),
            "    fn ensure_sdma_queue_v1(",
            "    fn directional_sdma_ops_v1(",
            true,
        ),
        (
            include_str!("../compute_dispatch.rs"),
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
        let (before, after) = function
            .split_once("self.configure_native_host_pool_v1()?;")
            .unwrap();
        assert!(
            before
                .trim_end()
                .ends_with("self.configure_native_device_pool_v1()?;")
        );
        assert!(before.rsplit_once("self.queue = Some(queue);").is_some());
        assert!(!after.contains("self.configure_native_host_pool_v1()?;"));
        let enable = ".enable_gfx942_directional_sdma_copy_engines()";
        assert!(!before.contains(enable));
        assert_eq!(after.contains(enable), sdma_first);
    }
}
