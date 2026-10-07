use super::*;
use crate::shared_memory::pair_operational;
use std::panic::{AssertUnwindSafe, catch_unwind};

fn pair() -> (
    SharedMemoryEngine<FakeBackend>,
    SharedMemoryEngine<FakeBackend>,
) {
    (
        SharedMemoryEngine::acquire(FakeBackend::good()).unwrap(),
        SharedMemoryEngine::acquire(FakeBackend::good()).unwrap(),
    )
}

#[test]
fn retained_pair_operational_success_checks_both_without_full_discovery() {
    let (mut source, mut peer) = pair();
    let before = (
        source.backend.currentness_calls,
        peer.backend.currentness_calls,
    );
    for _ in 0..4 {
        pair_operational::check(&mut source, &mut peer).unwrap();
    }
    assert_eq!(
        (
            source.backend.currentness_calls,
            peer.backend.currentness_calls
        ),
        before
    );
    assert_eq!(
        (
            source.backend.operational_currentness_calls,
            peer.backend.operational_currentness_calls
        ),
        (4, 4)
    );
    assert_eq!(source.phase(), SharedMemorySessionPhaseV1::Active);
    assert_eq!(peer.phase(), SharedMemorySessionPhaseV1::Active);
}

#[test]
fn retained_pair_operational_each_endpoint_error_is_terminal_and_not_repaired() {
    for fault_at in [1, 2] {
        for peer_fails in [false, true] {
            let (mut source, mut peer) = pair();
            if fault_at == 2 {
                pair_operational::check(&mut source, &mut peer).unwrap();
            }
            let selected = if peer_fails { &mut peer } else { &mut source };
            selected.backend.fail_operational_currentness_at = Some(fault_at);
            assert!(matches!(
                pair_operational::check(&mut source, &mut peer),
                Err(MemorySessionError::Injected("operational_currentness"))
            ));
            assert_eq!(source.phase(), SharedMemorySessionPhaseV1::Quarantined);
            assert_eq!(peer.phase(), SharedMemorySessionPhaseV1::Quarantined);
            let calls = (
                source.backend.operational_currentness_calls,
                peer.backend.operational_currentness_calls,
            );
            assert_eq!(calls, (fault_at, fault_at - 1 + usize::from(peer_fails)));
            source.backend.fail_operational_currentness_at = None;
            peer.backend.fail_operational_currentness_at = None;
            assert!(matches!(
                pair_operational::check(&mut source, &mut peer),
                Err(MemorySessionError::SharedSessionQuarantined)
            ));
            assert_eq!(
                (
                    source.backend.operational_currentness_calls,
                    peer.backend.operational_currentness_calls
                ),
                calls
            );
        }
    }
}

#[test]
fn retained_pair_operational_each_endpoint_panic_keeps_both_phases_terminal() {
    for fault_at in [1, 2] {
        for peer_panics in [false, true] {
            let (mut source, mut peer) = pair();
            if fault_at == 2 {
                pair_operational::check(&mut source, &mut peer).unwrap();
            }
            let selected = if peer_panics { &mut peer } else { &mut source };
            selected.backend.panic_operational_currentness_at = Some(fault_at);
            let panic = catch_unwind(AssertUnwindSafe(|| {
                pair_operational::check(&mut source, &mut peer)
            }))
            .unwrap_err();
            assert_eq!(
                panic.downcast_ref::<(&str, &str)>(),
                Some(&("N1 native panic", "operational_currentness"))
            );
            assert_eq!(source.phase(), SharedMemorySessionPhaseV1::Quarantined);
            assert_eq!(peer.phase(), SharedMemorySessionPhaseV1::Quarantined);
            assert_eq!(
                (
                    source.backend.operational_currentness_calls,
                    peer.backend.operational_currentness_calls
                ),
                (fault_at, fault_at - 1 + usize::from(peer_panics))
            );
        }
    }
}

#[test]
fn retained_pair_operational_wrong_opener_and_terminal_entry_make_no_later_calls() {
    for peer_is_wrong in [false, true] {
        let (mut source, mut peer) = pair();
        let selected = if peer_is_wrong {
            &mut peer
        } else {
            &mut source
        };
        selected.backend.opener_pid_override = Some(std::process::id().wrapping_add(1));
        assert!(matches!(
            pair_operational::check(&mut source, &mut peer),
            Err(MemorySessionError::ProcessChanged)
        ));
        assert_eq!(
            (
                source.backend.operational_currentness_calls,
                peer.backend.operational_currentness_calls
            ),
            (usize::from(peer_is_wrong), 0)
        );
        assert_eq!(source.phase(), SharedMemorySessionPhaseV1::Quarantined);
        assert_eq!(peer.phase(), SharedMemorySessionPhaseV1::Quarantined);
    }
    for peer_is_terminal in [false, true] {
        let (mut source, mut peer) = pair();
        let selected = if peer_is_terminal {
            &mut peer
        } else {
            &mut source
        };
        selected.phase = SharedMemorySessionPhaseV1::Quarantined;
        assert!(matches!(
            pair_operational::check(&mut source, &mut peer),
            Err(MemorySessionError::SharedSessionQuarantined)
        ));
        assert_eq!(
            (
                source.backend.operational_currentness_calls,
                peer.backend.operational_currentness_calls
            ),
            (0, 0)
        );
    }
}

#[test]
fn retained_pair_direction_requires_exact_source_vm_occurrence_and_order() {
    let route = crate::topology::tests::admitted_xgmi_routes()[0];
    let fixture = preparation::PreparationMemoryFixtureV1::new(false);
    let queue = QueueKeyV1 {
        vm: fixture.fixture.vm,
        id: QueueInstanceIdV1(11),
        generation: QueueGenerationV1(13),
    };
    let check = |vm, source, peer, queue| {
        pair_operational::validate_direction(vm, source, peer, route, queue)
    };
    assert!(
        check(
            queue.vm,
            route.source_gpu_id(),
            route.destination_gpu_id(),
            queue
        )
        .is_ok()
    );
    for field in 0..5 {
        let mut wrong = queue;
        match field {
            0 => wrong.vm.id.0 += 1,
            1 => wrong.vm.device.physical.0 += 1,
            2 => wrong.vm.device.generation.0 += 1,
            3 => wrong.id.0 = 0,
            _ => wrong.generation.0 = 0,
        }
        assert!(
            check(
                queue.vm,
                route.source_gpu_id(),
                route.destination_gpu_id(),
                wrong
            )
            .is_err()
        );
    }
    assert!(
        check(
            queue.vm,
            route.destination_gpu_id(),
            route.source_gpu_id(),
            queue
        )
        .is_err()
    );
    assert!(
        check(
            queue.vm,
            route.source_gpu_id(),
            route.source_gpu_id(),
            queue
        )
        .is_err()
    );
}

#[test]
fn retained_pair_resource_rechecks_genuine_token_and_all_cached_facts() {
    let mut fixture = preparation::PreparationMemoryFixtureV1::new(false);
    let token = fixture.allocate::<HostVisibleCoherentGttV1>(4096).unwrap();
    let token = fixture.map(token).unwrap();
    let vm = fixture.fixture.vm;
    let mut authority: SharedGttQueueResourceAuthorityV1<
        AqlControlResourceRoleV1,
        HostVisibleCoherentGttV1,
        GttGpuAccessibleMutableV1,
    > = crate::shared_memory::transitions::retain_v1(&mut fixture.fixture.engine, vm, token)
        .unwrap();
    let original_facts = authority.facts;
    let original_session = authority.token.session_id;
    let original_id = authority.token.id;
    for field in 0..8 {
        match field {
            0 => authority.facts.gpu_va += 4096,
            1 => authority.facts.logical_bytes += 1,
            2 => authority.facts.cpu_mapping_bytes += 4096,
            3 => authority.facts.gpu_va_bytes += 4096,
            4 => authority.facts.mapping.id.0 += 1,
            5 => authority.facts.publication.id.0 += 1,
            6 => authority.token.session_id += 1,
            _ => authority.token.id += 1,
        }
        assert!(
            pair_operational::validate_resource(&fixture.fixture.engine, vm, &authority).is_err()
        );
        authority.facts = original_facts;
        authority.token.session_id = original_session;
        authority.token.id = original_id;
        assert!(
            pair_operational::validate_resource(&fixture.fixture.engine, vm, &authority).is_ok()
        );
    }
    let other = SharedMemoryEngine::acquire(FakeBackend::good()).unwrap();
    assert!(pair_operational::validate_resource(&other, vm, &authority).is_err());
    assert_eq!(
        fixture.fixture.engine.phase(),
        SharedMemorySessionPhaseV1::Active
    );
    assert_eq!(
        fixture.fixture.engine.backend.operational_currentness_calls,
        0
    );
}
