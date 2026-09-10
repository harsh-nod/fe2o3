use super::*;

fn host_budget() -> Gfx942HostVisibleBackingBudgetV1 {
    Gfx942HostVisibleBackingBudgetV1::new(64 * 1024, 4).unwrap()
}

fn device_budget() -> Gfx942DeviceBackingBudgetV1 {
    Gfx942DeviceBackingBudgetV1::new(128 * 1024, 8).unwrap()
}

fn pool_limits() -> Gfx942DevicePoolLimitsV1 {
    Gfx942DevicePoolLimitsV1::new(8192, 2).unwrap()
}

fn assert_busy(result: Result<(), RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>) {
    assert!(matches!(result,
        Err(RuntimeBackendFailureV1::Rejected(error))
            if error.kind() == KfdRuntimeBackendErrorKindV1::Busy));
}

#[test]
fn host_backing_default_configuration_does_not_imply_native_account_or_residency() {
    let backend = KfdRuntimeBackendV1::mock();
    assert_eq!(backend.host_visible_backing_budget_v1(), None);
    assert_eq!(backend.host_visible_backing_usage_v1(), None);
    assert_eq!(backend.device_backing_budget_v1(), None);
    assert_eq!(backend.device_backing_usage_v1(), None);
    assert_eq!(backend.device_pool_limits_v1(), None);
    assert_eq!(backend.device_pool_usage_v1().unwrap(), None);
    assert!(backend.queue.is_none());
    assert!(!backend.native_available);
}

#[test]
fn host_backing_configuration_is_immutable_and_preserves_exact_capabilities() {
    for mut backend in [
        KfdRuntimeBackendV1::mock(),
        KfdRuntimeBackendV1::mock_with_semantic_authority_v1(),
    ] {
        let capabilities = backend.description.capabilities;
        backend
            .configure_host_visible_backing_budget_v1(host_budget())
            .unwrap();
        assert_eq!(
            backend.host_visible_backing_budget_v1(),
            Some(host_budget())
        );
        assert_eq!(backend.host_visible_backing_usage_v1(), None);
        assert_eq!(backend.device_backing_budget_v1(), None);
        assert_eq!(backend.device_pool_limits_v1(), None);
        assert_eq!(backend.description.capabilities, capabilities);
        assert_eq!(backend.next_handle, 1);
        assert!(backend.queue.is_none());
        assert!(!backend.native_available);
        assert_busy(backend.configure_host_visible_backing_budget_v1(host_budget()));
        assert_busy(backend.configure_host_visible_backing_budget_v1(
            Gfx942HostVisibleBackingBudgetV1::new(128 * 1024, 8).unwrap(),
        ));
        assert_eq!(
            backend.host_visible_backing_budget_v1(),
            Some(host_budget())
        );
        assert_eq!(backend.description.capabilities, capabilities);
    }
}

#[derive(Clone, Copy, Debug)]
enum Configuration {
    Host,
    Device,
    Pool,
}

#[test]
fn host_device_and_pool_limits_are_independent_in_all_six_configuration_orders() {
    use Configuration::{Device, Host, Pool};
    for order in [
        [Host, Device, Pool],
        [Host, Pool, Device],
        [Device, Host, Pool],
        [Device, Pool, Host],
        [Pool, Host, Device],
        [Pool, Device, Host],
    ] {
        let mut backend = KfdRuntimeBackendV1::mock();
        let capabilities = backend.description.capabilities;
        let mut configured = [false; 3];
        for configuration in order {
            match configuration {
                Host => {
                    backend
                        .configure_host_visible_backing_budget_v1(host_budget())
                        .unwrap();
                    configured[0] = true;
                }
                Device => {
                    backend
                        .configure_device_backing_budget_v1(device_budget())
                        .unwrap();
                    configured[1] = true;
                }
                Pool => {
                    backend
                        .configure_device_pool_limits_v1(pool_limits())
                        .unwrap();
                    configured[2] = true;
                }
            }
            assert_eq!(
                backend.host_visible_backing_budget_v1(),
                configured[0].then(host_budget),
                "{order:?}",
            );
            assert_eq!(
                backend.device_backing_budget_v1(),
                configured[1].then(device_budget),
                "{order:?}",
            );
            assert_eq!(
                backend.device_pool_limits_v1(),
                configured[2].then(pool_limits),
                "{order:?}",
            );
            assert_eq!(backend.host_visible_backing_usage_v1(), None);
            assert_eq!(backend.device_backing_usage_v1(), None);
            assert_eq!(backend.device_pool_usage_v1().unwrap(), None);
            assert_eq!(backend.description.capabilities, capabilities);
            assert_eq!(backend.next_handle, 1);
            assert!(backend.queue.is_none());
        }
        assert_busy(backend.configure_host_visible_backing_budget_v1(host_budget()));
        assert_busy(backend.configure_device_backing_budget_v1(device_budget()));
        assert_busy(backend.configure_device_pool_limits_v1(pool_limits()));
        assert_eq!(
            backend.host_visible_backing_budget_v1(),
            Some(host_budget())
        );
        assert_eq!(backend.device_backing_budget_v1(), Some(device_budget()));
        assert_eq!(backend.device_pool_limits_v1(), Some(pool_limits()));
    }
}

#[test]
fn host_backing_configuration_rejects_live_and_released_logical_resource_history() {
    let mut backend = KfdRuntimeBackendV1::mock();
    let stream = backend.create_stream_v1(7).unwrap();
    assert_busy(backend.configure_host_visible_backing_budget_v1(host_budget()));
    backend.destroy_stream_v1(stream).unwrap();
    assert_busy(backend.configure_host_visible_backing_budget_v1(host_budget()));
    assert_eq!(backend.host_visible_backing_budget_v1(), None);

    for kind in [
        RuntimeMemoryKindV1::HostVisible,
        RuntimeMemoryKindV1::DeviceLocal,
    ] {
        let mut backend = KfdRuntimeBackendV1::mock();
        let allocation = backend.allocate_v1(7, kind, 32, 8).unwrap();
        assert_busy(backend.configure_host_visible_backing_budget_v1(host_budget()));
        backend.release_allocation_v1(allocation).unwrap();
        assert_busy(backend.configure_host_visible_backing_budget_v1(host_budget()));
        assert_eq!(backend.host_visible_backing_budget_v1(), None);
        assert_eq!(backend.host_visible_backing_usage_v1(), None);
    }
}

#[test]
fn host_backing_configuration_rejects_native_lifecycle_history() {
    let mut retired = KfdRuntimeBackendV1::mock();
    retired.queue_retired = true;
    assert_busy(retired.configure_host_visible_backing_budget_v1(host_budget()));
    assert_eq!(retired.host_visible_backing_budget_v1(), None);

    let mut enabled = KfdRuntimeBackendV1::mock();
    enabled.sdma_enabled = true;
    assert_busy(enabled.configure_host_visible_backing_budget_v1(host_budget()));
    assert_eq!(enabled.host_visible_backing_budget_v1(), None);
    enabled.sdma_enabled = false;
}

#[test]
fn terminal_host_backing_configuration_cannot_restore_or_replace_limits() {
    for configured in [false, true] {
        let mut backend = KfdRuntimeBackendV1::mock();
        if configured {
            backend
                .configure_host_visible_backing_budget_v1(host_budget())
                .unwrap();
        }
        backend.terminal = true;
        assert!(matches!(
            backend.configure_host_visible_backing_budget_v1(
                Gfx942HostVisibleBackingBudgetV1::new(128 * 1024, 8).unwrap(),
            ),
            Err(RuntimeBackendFailureV1::Terminal(_)),
        ));
        assert_eq!(
            backend.host_visible_backing_budget_v1(),
            configured.then(host_budget)
        );
        assert_eq!(backend.host_visible_backing_usage_v1(), None);
        // Terminal Drop deliberately aborts; retain the resource-free mock.
        core::mem::forget(backend);
    }
}

#[test]
fn zero_device_cache_policy_does_not_disable_host_backing_configuration() {
    for limits in [
        Gfx942DevicePoolLimitsV1::new(0, 2).unwrap(),
        Gfx942DevicePoolLimitsV1::new(8192, 0).unwrap(),
        Gfx942DevicePoolLimitsV1::new(0, 0).unwrap(),
    ] {
        let mut backend = KfdRuntimeBackendV1::mock();
        backend.configure_device_pool_limits_v1(limits).unwrap();
        backend
            .configure_host_visible_backing_budget_v1(host_budget())
            .unwrap();
        assert_eq!(backend.device_pool_limits_v1(), Some(limits));
        assert_eq!(
            backend.host_visible_backing_budget_v1(),
            Some(host_budget())
        );
        assert_eq!(backend.host_visible_backing_usage_v1(), None);
        assert_eq!(backend.device_backing_budget_v1(), None);
    }
}

#[test]
fn host_backing_limits_reach_both_combined_native_startup_paths() {
    // This checks forwarding only. KFD's fake-backend tests independently
    // exercise actual host charges, coherent bootstrap signals and queue loans.
    let source = include_str!("../../kfd_backend.rs");
    let sdma = source
        .split("    fn ensure_sdma_queue_v1(")
        .nth(1)
        .unwrap()
        .split("    fn directional_sdma_ops_v1(")
        .next()
        .unwrap();
    let create = sdma
        .find(".create_compute_aql_queue_with_backing_budgets_v1(")
        .unwrap();
    let arguments = sdma[create..].split(')').next().unwrap();
    assert!(arguments.contains("self.device_backing_budget,"));
    assert!(arguments.contains("self.host_visible_backing_budget,"));
    assert!(!sdma.contains(".create_compute_aql_queue_with_device_backing_budget_v1("));
    assert!(!sdma.contains(".create_compute_aql_queue("));

    let compute = include_str!("../compute_dispatch.rs");
    let acquire = compute
        .find(".acquire_shared_gtt_memory_session_with_backing_budgets_v1(")
        .unwrap();
    let materialize = acquire
        + compute[acquire..]
            .find("materialize_initial_data_v1(")
            .unwrap();
    let arguments = compute[acquire..materialize].split(')').next().unwrap();
    assert!(arguments.contains("self.device_backing_budget,"));
    assert!(arguments.contains("self.host_visible_backing_budget,"));
    assert!(!compute.contains(".acquire_shared_gtt_memory_session_with_device_backing_budget_v1("));
    assert!(!compute.contains(".acquire_shared_gtt_memory_session()"));
}
