use super::*;

#[test]
fn n1_actual_foundation_loans_and_pool_retags_keep_exact_charge_in_both_orders() {
    use crate::resource_domains::{composed_tests, native_tests};
    use fe2o3_resource_accounting::ResourceKindV1 as K;

    for (compute_first, profile) in [
        (false, 0),
        (true, 0),
        (false, 1),
        (true, 1),
        (false, 2),
        (true, 2),
        (false, 3),
        (true, 3),
    ] {
        let mut fixture = BackingConstructorFixture::new(None);
        let root = (profile == 1).then(|| crate::resource_domains::tests::root(16384, 4));
        let native_root = (profile == 2).then(|| native_tests::root(65536, 65536, 8));
        let composed_root = (profile == 3).then(composed_tests::root);
        let mut request = None;
        if let Some(root) = &root {
            let admission = crate::resource_domains::tests::admission(
                root,
                fixture.device.model_key(),
                budget(16384, 4),
                budget(16384, 4),
            );
            fixture
                .ownership
                .configure_rooted_host_backing(
                    &mut fixture.engine,
                    fixture.device,
                    fixture.vm,
                    admission,
                )
                .unwrap();
        } else if let Some(root) = &native_root {
            let admission = native_tests::admission(
                root,
                fixture.device.model_key(),
                native_tests::device_budget(8),
                native_tests::budget(4, 4, 8),
            );
            fixture
                .ownership
                .configure_native_backing(
                    &mut fixture.engine,
                    fixture.device,
                    fixture.vm,
                    admission,
                )
                .unwrap();
        } else if let Some(root) = &composed_root {
            let admission = composed_tests::admission_for_device(
                root,
                fixture.device.model_key(),
                composed_tests::device_budget(),
                composed_tests::budget(8),
            );
            request = Some(
                admission
                    .request_account_v1()
                    .reserve_v1(4100)
                    .unwrap()
                    .retain(),
            );
            fixture
                .ownership
                .configure_composed_backing(
                    &mut fixture.engine,
                    fixture.device,
                    fixture.vm,
                    admission,
                )
                .unwrap();
        } else {
            configure_fixture(&mut fixture, true);
        }
        let root_usage = || {
            root.as_ref()
                .map(|root| root.usage_v1())
                .or_else(|| native_root.as_ref().map(|root| root.usage_v1()))
                .or_else(|| composed_root.as_ref().map(|root| root.usage_v1()))
        };
        let baseline = root_usage();
        let mut compute = compute_first.then(|| fixture.mapped_device());
        let host_before = (!compute_first).then(|| projected_host(&mut fixture));
        let authorities = compute.as_ref().into_iter().collect::<Vec<_>>();
        let mut queue = fixture.transfer(&authorities).unwrap();
        assert!(fixture.engine.host_backing_configuration_closed);
        let loan = fixture
            .ownership
            .loan_foundation(
                fixture.engine.session_id,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
            )
            .unwrap();
        if !compute_first && profile >= 2 {
            compute = Some(fixture.mapped_device());
        }
        let token = host_before.unwrap_or_else(|| projected_host(&mut fixture));
        let identity = token.storage_identity();
        let before = usage(&fixture.engine);
        assert_eq!(before.used_backing_bytes, 8192);
        assert_eq!(before.used_allocation_records, 1);
        let root_before = root_usage();
        if let Some(usage) = root_before {
            assert_eq!(
                usage
                    .used
                    .get(fe2o3_resource_accounting::ResourceKindV1::ResidentHostAllocationBytes),
                8192
            );
            assert_eq!(
                usage
                    .used
                    .get(fe2o3_resource_accounting::ResourceKindV1::AllocationRecords),
                1 + u64::from(profile >= 2) + u64::from(request.is_some())
            );
            assert_eq!(
                usage.used.get(K::ResidentDeviceAllocationBytes),
                if profile >= 2 { 4096 } else { 0 }
            );
            assert_eq!(
                usage.used.get(K::RequestedAllocationBytes),
                if request.is_some() { 4100 } else { 0 }
            );
        }
        let owner = QueueKeyV1 {
            vm: fixture.vm,
            id: QueueInstanceIdV1(33),
            generation: QueueGenerationV1(1),
        };
        let mut buffer = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Host(token),
            owner,
            1,
            4100,
        );
        let limits = crate::sdma::Gfx942HostPoolLimitsV1::new(8192, 1).unwrap();
        assert_eq!(
            crate::sdma::host_pool_policy::host_pool_recycle_decision_with_v1(
                owner,
                limits,
                &[],
                &buffer,
                &mut |token| fixture.engine.host_pool_backing_bytes_v1(
                    token,
                    fixture.device.model_key(),
                    fixture.vm
                )
            ),
            Ok(crate::sdma::HostPoolDispositionV1::Cache)
        );
        // Exercise the production move-only buffer generation/logical-extent
        // transitions; the fake fixture is not the Linux queue pool facade.
        buffer.advance_pool_generation().unwrap();
        buffer.set_logical_bytes(1024);
        let mut cached = vec![buffer];
        assert_eq!(
            crate::sdma::host_pool_policy::host_pool_usage_with_v1(
                owner,
                limits,
                &cached,
                &mut |token| fixture.engine.host_pool_backing_bytes_v1(
                    token,
                    fixture.device.model_key(),
                    fixture.vm
                )
            )
            .unwrap()
            .cached_backing_bytes,
            8192
        );
        fixture
            .ownership
            .reclaim_foundation(
                fixture.engine.session_id,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
                loan,
            )
            .unwrap();
        assert_eq!(usage(&fixture.engine), before);
        let loan = fixture
            .ownership
            .loan_foundation(
                fixture.engine.session_id,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
            )
            .unwrap();
        let mut buffer = cached.pop().unwrap();
        assert_eq!(
            crate::sdma::host_pool_policy::host_pool_usage_with_v1(
                owner,
                limits,
                &cached,
                &mut |token| fixture.engine.host_pool_backing_bytes_v1(
                    token,
                    fixture.device.model_key(),
                    fixture.vm
                )
            )
            .unwrap()
            .cached_backing_bytes,
            0
        );
        buffer.advance_pool_generation().unwrap();
        buffer.set_logical_bytes(4100);
        let (Gfx942SdmaBufferStorageV1::Host(mut token), _, _, _) = buffer.into_bridge_parts()
        else {
            panic!("host fixture");
        };
        assert_eq!(token.storage_identity(), identity);
        fixture
            .engine
            .overwrite_mapped_host_visible_subrange(&mut token, 0, &[7])
            .unwrap();
        assert_eq!(usage(&fixture.engine), before);
        assert_eq!(root_usage(), root_before);
        projected_dispose(&mut fixture, token);
        debit(&fixture.engine, 0, 0);
        if profile >= 2 {
            let remaining = root_usage().unwrap();
            assert_eq!(remaining.used.get(K::ResidentHostAllocationBytes), 0);
            assert_eq!(remaining.used.get(K::ResidentDeviceAllocationBytes), 4096);
            assert_eq!(
                remaining.used.get(K::AllocationRecords),
                1 + u64::from(request.is_some())
            );
            assert_eq!(fixture.usage().unwrap().used_allocation_records, 1);
        } else {
            assert_eq!(root_usage(), baseline);
        }
        fixture
            .ownership
            .reclaim_foundation(
                fixture.engine.session_id,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
                loan,
            )
            .unwrap();
        fixture
            .ownership
            .restore_foundation(
                &mut fixture.engine,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
            )
            .unwrap();
        debit(&fixture.engine, 0, 0);
        if let Some(compute) = compute {
            let unmapped = fixture.engine.unmap_device_memory(compute.lease).unwrap();
            fixture.engine.release_device_memory(unmapped).unwrap();
        }
        assert_eq!(root_usage(), baseline);
        let before_calls = calls(&fixture.engine);
        assert!(
            fixture
                .ownership
                .configure_optional_host_visible_backing_budget(
                    &mut fixture.engine,
                    fixture.device,
                    fixture.vm,
                    Some(budget(8192, 2))
                )
                .is_err()
        );
        assert_eq!(calls(&fixture.engine), before_calls);
        if let Some(request) = request {
            request.release_after_disposal().unwrap();
            assert_eq!(root_usage().unwrap().used.get(K::AllocationRecords), 0);
        }
    }
}

#[test]
fn n1_unconfigured_transfer_cannot_reopen_budget_after_restore_or_live_loan() {
    let mut fixture = BackingConstructorFixture::new(None);
    let before = calls(&fixture.engine);
    configure_fixture(&mut fixture, false);
    assert_eq!(calls(&fixture.engine), before);
    let mut queue = fixture.transfer(&[]).unwrap();
    let loan = fixture
        .ownership
        .loan_foundation(
            fixture.engine.session_id,
            &mut fixture.foundation,
            &mut queue,
            fixture.device,
            fixture.vm,
        )
        .unwrap();
    assert!(
        fixture
            .ownership
            .configure_optional_host_visible_backing_budget(
                &mut fixture.engine,
                fixture.device,
                fixture.vm,
                Some(budget(8192, 2))
            )
            .is_err()
    );
    fixture
        .ownership
        .reclaim_foundation(
            fixture.engine.session_id,
            &mut fixture.foundation,
            &mut queue,
            fixture.device,
            fixture.vm,
            loan,
        )
        .unwrap();
    fixture
        .ownership
        .restore_foundation(
            &mut fixture.engine,
            &mut fixture.foundation,
            &mut queue,
            fixture.device,
            fixture.vm,
        )
        .unwrap();
    assert!(
        fixture
            .ownership
            .configure_optional_host_visible_backing_budget(
                &mut fixture.engine,
                fixture.device,
                fixture.vm,
                Some(budget(8192, 2))
            )
            .is_err()
    );
    assert!(fixture.engine.host_backing_account.is_none());
}

#[test]
#[allow(clippy::drop_non_drop)] // Explicitly relinquish token authority before observing retained custody.
fn n1_failed_retake_retains_live_debit_but_does_not_resurrect_disposed_backing() {
    for dispose in [false, true] {
        let mut fixture = BackingConstructorFixture::new(None);
        configure_fixture(&mut fixture, true);
        let token = projected_host(&mut fixture);
        let mut queue = fixture.transfer(&[]).unwrap();
        let loan = fixture
            .ownership
            .loan_foundation(
                fixture.engine.session_id,
                &mut fixture.foundation,
                &mut queue,
                fixture.device,
                fixture.vm,
            )
            .unwrap();
        let owner = QueueKeyV1 {
            vm: fixture.vm,
            id: QueueInstanceIdV1(33),
            generation: QueueGenerationV1(1),
        };
        let buffer = Gfx942SdmaBufferV1::from_bridge_parts(
            Gfx942SdmaBufferStorageV1::Host(token),
            owner,
            1,
            4100,
        );
        assert_eq!(
            crate::sdma::host_pool_policy::host_pool_recycle_decision_with_v1(
                owner,
                crate::sdma::Gfx942HostPoolLimitsV1::new(0, 0).unwrap(),
                &[],
                &buffer,
                &mut |token| fixture.engine.host_pool_backing_bytes_v1(
                    token,
                    fixture.device.model_key(),
                    fixture.vm
                )
            ),
            Ok(crate::sdma::HostPoolDispositionV1::Dispose)
        );
        let (Gfx942SdmaBufferStorageV1::Host(token), _, _, _) = buffer.into_bridge_parts() else {
            unreachable!();
        };
        if dispose {
            projected_dispose(&mut fixture, token);
        } else {
            drop(token);
        }
        let expected = if dispose { 0 } else { 8192 };
        debit(&fixture.engine, expected, u64::from(!dispose));
        let wrong_vm = VmKeyV1 {
            id: VmIdV1(fixture.vm.id.0 + 1),
            ..fixture.vm
        };
        assert!(
            fixture
                .ownership
                .reclaim_foundation(
                    fixture.engine.session_id,
                    &mut fixture.foundation,
                    &mut queue,
                    fixture.device,
                    wrong_vm,
                    loan
                )
                .is_err()
        );
        // The enclosing live-owner guard quarantines on retake rejection. Its
        // existing queue regression tests cover that Linux facade wiring.
        assert!(
            fixture
                .engine
                .quarantine::<()>(MemorySessionError::Model("test retake rejection"))
                .is_err()
        );
        debit(&fixture.engine, expected, u64::from(!dispose));
        closed(&mut fixture.engine);
    }
}
