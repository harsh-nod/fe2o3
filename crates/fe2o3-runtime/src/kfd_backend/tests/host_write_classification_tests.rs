use super::*;
use crate::kfd_backend::tests::sdma_allocation_tests::{capacity, reject};

#[test]
fn post_effect_classification_preserves_exact_diagnostic_storage() {
    for class in 0..3 {
        let error = KfdRuntimeBackendErrorV1::new(
            KfdRuntimeBackendErrorKindV1::Capacity,
            "unique post-effect diagnostic",
        );
        let pointer = error.detail().as_ptr();
        let failure = match class {
            0 => RuntimeBackendFailureV1::Rejected(error),
            1 => RuntimeBackendFailureV1::Quiescent(error),
            _ => RuntimeBackendFailureV1::Terminal(error),
        };
        let error = match KfdRuntimeBackendV1::after_possible_host_mutation(failure) {
            RuntimeBackendFailureV1::Quiescent(error) if class < 2 => error,
            RuntimeBackendFailureV1::Terminal(error) if class == 2 => error,
            _ => panic!("post-effect failure was incorrectly classified"),
        };
        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
        assert_eq!(error.detail(), "unique post-effect diagnostic");
        assert_eq!(error.detail().as_ptr(), pointer);
    }
}

#[test]
fn rejected_host_write_range_preserves_bytes_owners_and_driver_state() {
    let (backend, _, host, device) = fixture(8, []);
    let mut backend = ManuallyDrop::new(backend);
    let before = snapshot(&backend, host, device);
    for allocation in [host, device] {
        for offset in [7, u64::MAX] {
            assert!(matches!(
                backend.write_allocation_v1(allocation, offset, &[1, 2]),
                Err(RuntimeBackendFailureV1::Rejected(_))
            ));
            assert_eq!(snapshot(&backend, host, device), before);
        }
    }
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(
        (
            driver.remaining_steps(),
            driver.live_owner_count(),
            driver.unexpected_drops()
        ),
        (0, 2, 0)
    );
    discard_scripted_fixture(backend);
}

#[test]
fn public_host_write_after_reconciliation_never_promotes_capacity_to_no_effect() {
    for reconciled in [false, true] {
        let mut steps = Vec::new();
        if reconciled {
            steps.extend(scripted_sync_copy_steps_v1(
                Gfx942PersistentSdmaDirectionV1::DeviceToHost,
                0,
                8,
                ScriptedFailureModeV1::Success,
            ));
        }
        steps.push(reject(
            RuntimeMemoryKindV1::HostVisible,
            if reconciled { 4 } else { 8 },
            false,
        ));
        let (backend, _, host, device) = fixture(8, steps);
        let mut backend = ManuallyDrop::new(backend);
        let record = backend.allocations.get_mut(&device).unwrap();
        let KfdRuntimeSdmaStorageV1::Device(owner) = &mut record.sdma_storage else {
            unreachable!()
        };
        owner.scripted_bytes_mut().unwrap().fill(0xa5);
        record.sdma_shadow_dirty = true;
        let prior_digest = Sha256::digest(&*record.bytes).into();
        record.content_sha256 = Some(prior_digest);
        record.last_full_host_write = Some((Arc::clone(&record.bytes), prior_digest));
        let before = snapshot(&backend, host, device);
        let Err(RuntimeBackendFailureV1::Quiescent(error)) =
            backend.write_allocation_v1(device, 2, &[0x5a; 4])
        else {
            panic!("post-effect capacity must be quiescent");
        };
        assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Capacity);
        assert_eq!(
            error.detail(),
            format!(
                "KFD {} staging: {}",
                if reconciled { "upload" } else { "download" },
                capacity(RuntimeMemoryKindV1::HostVisible, false).detail
            )
        );
        let after = snapshot(&backend, host, device);
        assert_eq!(after.device, before.device);
        assert_eq!(
            (
                after.host,
                after.host_shadow,
                after.host_shadow_address,
                after.host_digest
            ),
            (
                before.host,
                before.host_shadow,
                before.host_shadow_address,
                before.host_digest
            )
        );
        assert_eq!(
            (
                after.next_handle,
                after.staged_bytes,
                after.allocations,
                after.events
            ),
            (
                before.next_handle,
                before.staged_bytes,
                before.allocations,
                before.events
            )
        );
        let record = &backend.allocations[&device];
        assert_eq!(record.sdma_shadow_dirty, !reconciled);
        assert!(record.content_sha256.is_none());
        assert!(record.last_full_host_write.is_none());
        assert_eq!(
            &*record.bytes,
            if reconciled {
                &[0xa5; 8]
            } else {
                &*before.device_shadow
            }
        );
        assert!(!backend.terminal);
        assert!(backend.terminal_sdma_custody.is_none());
        let driver = backend.scripted_sdma.as_ref().unwrap();
        assert_eq!(
            (
                driver.remaining_steps(),
                driver.live_owner_count(),
                driver.unexpected_drops()
            ),
            (0, 2, 0)
        );
        discard_scripted_fixture(backend);
    }
}

#[test]
fn native_host_write_and_xgmi_post_effect_branches_cannot_report_rejected() {
    // Wiring coverage only: no native XGMI session or disposal authority is forged.
    let source = include_str!("../../kfd_backend.rs");
    let ordinary = source
        .split("impl RuntimeBackendV1 for KfdRuntimeBackendV1 {")
        .nth(1)
        .unwrap()
        .split("    fn write_allocation_v1(")
        .nth(1)
        .unwrap()
        .split("    fn read_allocation_v1(")
        .next()
        .unwrap();
    let post_effect = ordinary
        .split("self.prepare_compute_caches_for_host_write_v1")
        .nth(1)
        .unwrap();
    assert!(!post_effect.contains("Self::rejected("));
    assert_eq!(
        post_effect
            .matches(".map_err(Self::after_possible_host_mutation)")
            .count(),
        5
    );
    assert!(post_effect.contains("preflighted host-write range remains valid after upload"));
    let xgmi = source
        .split("impl RuntimeBackendV1 for KfdNativeXgmiRuntimeBackendV1 {")
        .nth(1)
        .unwrap();
    for (start, end) in [
        ("    fn write_allocation_v1(", "    fn read_allocation_v1("),
        ("    fn read_allocation_v1(", "    fn load_module_v1("),
    ] {
        let body = xgmi.split(start).nth(1).unwrap().split(end).next().unwrap();
        let post_unmap = body
            .split("self.ensure_allocation_unmapped(allocation)?;")
            .nth(1)
            .unwrap();
        assert!(!post_unmap.contains("Self::rejected("));
        assert!(post_unmap.contains("self.terminal_error("));
        assert!(
            post_unmap.contains("XGMI allocation authority unavailable after successful unmap")
        );
    }
}
