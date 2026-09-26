use super::*;
use crate::resource_domains::tests::{admission, observe_lifetime, root};
use fe2o3_resource_accounting::{ResourceCreditErrorV1, ResourceKindV1};

fn configured_root(
    root: &crate::Gfx942HostBackingRootV1,
    parent: Gfx942HostVisibleBackingBudgetV1,
    leaf: Gfx942HostVisibleBackingBudgetV1,
) -> SharedMemoryEngine<FakeBackend> {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    let admission = admission(root, device, parent, leaf);
    engine
        .configure_host_visible_backing_admission_v1(device, vm, leaf, Some(admission))
        .unwrap();
    engine
}

#[test]
fn rooted_n1_native_allocation_obeys_all_ancestor_bytes_and_records_before_effects() {
    for limit in 0..6 {
        let root = root(
            if limit == 0 { 8192 } else { 32768 },
            if limit == 3 { 1 } else { 4 },
        );
        let parent = budget(
            if limit == 1 { 8192 } else { 32768 },
            if limit == 4 { 1 } else { 4 },
        );
        let leaf = budget(
            if limit == 2 { 8192 } else { 32768 },
            if limit == 5 { 1 } else { 4 },
        );
        let mut a = configured_root(&root, parent, leaf);
        let mut b = configured_root(&root, parent, leaf);
        let token = a.allocate::<HostVisibleCoherentGttV1>(4100).unwrap();
        let before = root.usage_v1();
        assert_eq!(
            before.used.get(ResourceKindV1::ResidentHostAllocationBytes),
            8192
        );
        assert_eq!(before.used.get(ResourceKindV1::AllocationRecords), 1);
        let rejected = if limit == 2 || limit == 5 {
            &mut a
        } else {
            &mut b
        };
        let before_calls = calls(rejected);
        let expected = if limit >= 4 {
            ResourceCreditErrorV1::RecordCapacity
        } else {
            ResourceCreditErrorV1::Capacity
        };
        assert!(
            matches!(rejected.allocate::<HostVisibleCoherentGttV1>(1), Err(MemorySessionError::HostVisibleBackingCredits(error)) if error == expected)
        );
        assert_eq!(rejected.phase(), SharedMemorySessionPhaseV1::Active);
        assert_eq!(calls(rejected), before_calls);
        assert_eq!(root.usage_v1(), before);
        a.release(token, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        let token = b.allocate::<HostVisibleCoherentGttV1>(4100).unwrap();
        b.release(token, SharedAllocationPhaseV1::CpuWritable)
            .unwrap();
        assert_eq!(
            root.usage_v1()
                .used
                .get(ResourceKindV1::ResidentHostAllocationBytes),
            0
        );
    }
}

#[test]
fn rooted_n1_native_wrong_generation_and_replacement_reject_without_effects() {
    let root = root(8192, 2);
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    let token = admission(&root, device_vm(2).0, budget(8192, 2), budget(8192, 2));
    let before = calls(&engine);
    let root_before = root.usage_v1();
    assert!(
        engine
            .configure_host_visible_backing_admission_v1(device, vm, budget(8192, 2), Some(token))
            .is_err()
    );
    assert!(engine.host_backing_account.is_none());
    assert_eq!(calls(&engine), before);
    assert_eq!(root.usage_v1(), root_before);
    let token = admission(&root, device, budget(8192, 2), budget(8192, 2));
    engine
        .configure_host_visible_backing_admission_v1(device, vm, budget(8192, 2), Some(token))
        .unwrap();
    let before = calls(&engine);
    assert!(
        engine
            .configure_host_visible_backing_budget_v1(device, vm, budget(16384, 4))
            .is_err()
    );
    assert_eq!(calls(&engine), before);
}

#[test]
fn rooted_n1_native_disposal_faults_preserve_ancestors_across_session_drop() {
    for panic in [false, true] {
        for boundary in 0..7 {
            let root = root(4096, 1);
            let mut engine = configured_root(&root, budget(4096, 1), budget(4096, 1));
            let token = engine.allocate::<HostVisibleCoherentGttV1>(17).unwrap();
            let before = root.usage_v1().used;
            let operation = match boundary {
                1 => "unmap_cpu",
                3 => "free",
                5 => "release_va_reservation",
                _ => "currentness",
            };
            if boundary % 2 == 0 {
                let at = engine.backend.currentness_calls + boundary / 2 + 1;
                if panic {
                    engine.backend.panic_currentness_at = Some(at);
                } else {
                    engine.backend.fail_currentness_at = Some(at);
                }
            } else if panic {
                engine.backend.panic_operation = Some(operation);
            } else {
                engine.backend.fail_operation = Some(operation);
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                engine.release(token, SharedAllocationPhaseV1::CpuWritable)
            }));
            if panic {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(root.usage_v1().used, before);
            drop(engine);
            assert_eq!(root.usage_v1().quarantined_records, 1);
            assert_eq!(root.usage_v1().used, before);
            let mut replacement = configured_root(&root, budget(4096, 1), budget(4096, 1));
            let calls_before = calls(&replacement);
            assert!(replacement.allocate::<HostVisibleCoherentGttV1>(1).is_err());
            assert_eq!(calls(&replacement), calls_before);
        }
    }
}

#[test]
fn rooted_n1_native_owner_retains_registry_and_quarantine_anchors_it() {
    for dispose in [false, true] {
        let root = root(8192, 2);
        let observe = observe_lifetime(&root);
        let mut engine = configured_root(&root, budget(8192, 2), budget(8192, 2));
        let token = engine.allocate::<HostVisibleCoherentGttV1>(4100).unwrap();
        drop(root);
        assert_eq!(
            observe()
                .unwrap()
                .used
                .get(ResourceKindV1::ResidentHostAllocationBytes),
            8192
        );
        if dispose {
            engine
                .release(token, SharedAllocationPhaseV1::CpuWritable)
                .unwrap();
        }
        drop(engine);
        if dispose {
            assert!(observe().is_none());
        } else {
            let usage = observe().unwrap();
            assert_eq!(usage.quarantined_records, 1);
            assert_eq!(usage.retained_records, 1); // Canonical registry payload.
            assert_eq!(
                usage.used.get(ResourceKindV1::ResidentHostAllocationBytes),
                8192
            );
        }
    }
}
