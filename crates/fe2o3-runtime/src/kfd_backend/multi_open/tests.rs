use super::*;
use crate::kfd_backend::kfd_backend_sdma_seam::{
    ScriptedBufferKindV1, ScriptedRecycleOutcomeV1, ScriptedSdmaStepV1,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

#[derive(Debug)]
struct DeniedAuthority {
    calls: Rc<Cell<usize>>,
    panic: bool,
}

// This test authority never authorizes a launch or advertises a profile.
unsafe impl KfdRuntimeLaunchAuthorityV1 for DeniedAuthority {
    fn authorize_launch_v1(&self, _: KfdRuntimeAuthorityRequestV1<'_>) -> bool {
        self.calls.set(self.calls.get() + 1);
        assert!(!self.panic, "test authority denied by panic");
        false
    }
}

unsafe impl KfdRuntimeSemanticLaunchAuthorityV1 for DeniedAuthority {
    fn atomic_profiles_v1(&self) -> &[KfdRuntimeAtomicExecutionProfileV1] {
        &[]
    }

    fn collective_profiles_v1(&self) -> &[KfdRuntimeCollectiveExecutionProfileV1] {
        &[]
    }
}

fn authority() -> DeniedAuthority {
    DeniedAuthority {
        calls: Rc::new(Cell::new(0)),
        panic: false,
    }
}

fn gates(ids: &[u64]) -> Vec<(u64, KfdRuntimeLaunchGateV1)> {
    ids.iter()
        .map(|uid| {
            (
                *uid,
                KfdRuntimeLaunchGateV1::Production(Box::new(authority())),
            )
        })
        .collect()
}

fn child(uid: u64, gate: KfdRuntimeLaunchGateV1) -> KfdRuntimeBackendV1 {
    let mut child = KfdRuntimeBackendV1::mock();
    child.description.backend_device = uid;
    child.launch_gate = gate;
    child
}

fn scripted_route() -> compute_xgmi::Route {
    compute_xgmi::Route::Scripted {
        failure: None,
        unwind: false,
        pending_samples: 0,
    }
}

fn opened(native: bool) -> KfdMultiDeviceRuntimeBackendV1 {
    open_with_v1(gates(&[30, 10, 20]), native, Ok, child, |_, _| {
        Ok(scripted_route())
    })
    .unwrap()
}

#[test]
fn multi_open_all_public_constructors_reject_invalid_rosters_before_native_admission() {
    for ids in [
        vec![],
        vec![1],
        (1..=crate::MAX_RUNTIME_DEVICES_V1 as u64 + 1).collect(),
        vec![0, 2],
        vec![1, 0],
        vec![1, 1],
        vec![1, 2, 3, 0],
        vec![1, 2, 3, 1],
    ] {
        let production = || {
            ids.iter()
                .map(|uid| {
                    (
                        *uid,
                        Box::new(authority()) as Box<dyn KfdRuntimeLaunchAuthorityV1>,
                    )
                })
                .collect()
        };
        let semantic = || {
            ids.iter()
                .map(|uid| {
                    (
                        *uid,
                        Box::new(authority()) as Box<dyn KfdRuntimeSemanticLaunchAuthorityV1>,
                    )
                })
                .collect()
        };
        for result in [
            KfdMultiDeviceRuntimeBackendV1::open_default(production()),
            KfdMultiDeviceRuntimeBackendV1::open_default_with_native_peer_copy_v1(production()),
            KfdMultiDeviceRuntimeBackendV1::open_default_with_semantic_authorities_v1(semantic()),
            KfdMultiDeviceRuntimeBackendV1::open_default_with_semantic_authorities_and_native_peer_copy_v1(semantic()),
        ] {
            assert_eq!(result.unwrap_err().kind(), KfdRuntimeBackendErrorKindV1::InvalidLaunch);
        }
        let result = open_with_v1(
            gates(&ids),
            true,
            |_| -> Result<u64, KfdRuntimeBackendErrorV1> {
                panic!("invalid roster opened a device")
            },
            |_, _| panic!("invalid roster built a child"),
            |_, _| panic!("invalid roster admitted a route"),
        );
        assert_eq!(
            result.unwrap_err().kind(),
            KfdRuntimeBackendErrorKindV1::InvalidLaunch
        );
    }
}

#[test]
fn multi_open_orders_all_admissions_before_children_and_commits_peer_policy_last() {
    for native in [false, true] {
        let trace = RefCell::new(Vec::new());
        let mut backend = open_with_v1(
            gates(&[30, 10, 20]),
            native,
            |uid| {
                trace.borrow_mut().push((0, uid, 0));
                Ok(uid)
            },
            |uid, gate| {
                trace.borrow_mut().push((1, uid, 0));
                child(uid, gate)
            },
            |source, destination| {
                assert!(!source.peer_visible_device_allocations);
                assert!(!destination.peer_visible_device_allocations);
                assert!(source.queue.is_none() && destination.queue.is_none());
                assert!(source.allocations.is_empty() && destination.allocations.is_empty());
                trace.borrow_mut().push((
                    2,
                    source.description.backend_device,
                    destination.description.backend_device,
                ));
                Ok(scripted_route())
            },
        )
        .unwrap();
        let mut expected = vec![
            (0, 30, 0),
            (0, 10, 0),
            (0, 20, 0),
            (1, 30, 0),
            (1, 10, 0),
            (1, 20, 0),
        ];
        if native {
            expected.extend([
                (2, 30, 10),
                (2, 30, 20),
                (2, 10, 30),
                (2, 10, 20),
                (2, 20, 30),
                (2, 20, 10),
            ]);
        }
        assert_eq!(*trace.borrow(), expected);
        assert_eq!(
            backend.compute_xgmi_routes.len(),
            if native { 6 } else { 0 }
        );
        assert_eq!(
            backend.request_policy,
            multi_admission::MultiRequestPolicyV1::Legacy
        );
        for (index, uid) in [30, 10, 20].into_iter().enumerate() {
            assert_eq!(backend.device_children[&uid], index);
            assert_eq!(
                backend.children[index].peer_visible_device_allocations,
                native
            );
            assert!(matches!(
                backend.children[index].launch_gate,
                KfdRuntimeLaunchGateV1::Production(_)
            ));
        }
        assert!(backend.compute_xgmi_children.iter().all(Option::is_none));
        assert_eq!(backend.completed_compute_xgmi_copies, 0);
        backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn multi_open_native_and_route_failures_stop_without_returning_partial_peer_policy() {
    let admissions = Cell::new(0);
    let result = open_with_v1(
        gates(&[30, 10, 20]),
        true,
        |uid| {
            admissions.set(admissions.get() + 1);
            if uid == 10 {
                Err(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Native,
                    "test device admission",
                ))
            } else {
                Ok(uid)
            }
        },
        |_, _| panic!("child constructed before complete device admission"),
        |_, _| panic!("route admitted before complete device admission"),
    );
    assert_eq!(
        result.unwrap_err().kind(),
        KfdRuntimeBackendErrorKindV1::Native
    );
    assert_eq!(admissions.get(), 2);

    let routes = Cell::new(0);
    let result = open_with_v1(
        gates(&[30, 10, 20]),
        true,
        Ok,
        child,
        |source, destination| {
            assert!(
                !source.peer_visible_device_allocations
                    && !destination.peer_visible_device_allocations
            );
            routes.set(routes.get() + 1);
            if routes.get() == 3 {
                Err(KfdRuntimeBackendErrorV1::new(
                    KfdRuntimeBackendErrorKindV1::Unsupported,
                    "test missing directed route",
                ))
            } else {
                Ok(scripted_route())
            }
        },
    );
    let error = result.unwrap_err();
    assert_eq!(error.kind(), KfdRuntimeBackendErrorKindV1::Unsupported);
    assert_eq!(error.detail(), "test missing directed route");
    assert_eq!(routes.get(), 3);

    let source = child(
        30,
        KfdRuntimeLaunchGateV1::Production(Box::new(authority())),
    );
    let destination = child(
        10,
        KfdRuntimeLaunchGateV1::Production(Box::new(authority())),
    );
    assert_eq!(
        compute_xgmi::admit_native_route_v1(&source, &destination)
            .unwrap_err()
            .kind(),
        KfdRuntimeBackendErrorKindV1::InvalidLaunch
    );
}

#[test]
fn multi_open_retains_exact_denied_and_panicking_production_and_semantic_gates() {
    for semantic in [false, true] {
        for panic in [false, true] {
            let calls = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
            let devices = [30, 10]
                .into_iter()
                .zip(&calls)
                .map(|(uid, calls)| {
                    let authority = DeniedAuthority {
                        calls: Rc::clone(calls),
                        panic,
                    };
                    let gate = if semantic {
                        KfdRuntimeLaunchGateV1::Semantic(Box::new(authority))
                    } else {
                        KfdRuntimeLaunchGateV1::Production(Box::new(authority))
                    };
                    (uid, gate)
                })
                .collect();
            let mut backend =
                open_with_v1(devices, true, Ok, child, |_, _| Ok(scripted_route())).unwrap();
            assert!(calls.iter().all(|count| count.get() == 0));
            let request = KfdRuntimeAuthorityRequestV1 {
                module_image: &[],
                module_sha256: [0; 32],
                kernel_name: "denied",
                signature: [0; 32],
                explicit_kernarg: &[],
                complete_kernarg_template: &[],
                bindings: &[],
                dispatch_abi: &[],
                allocations: &[],
                geometry: crate::RuntimeLaunchGeometryV1 {
                    grid: [1; 3],
                    workgroup: [1; 3],
                    dynamic_shared_bytes: 0,
                },
                semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
            };
            for (index, child) in backend.children.iter().enumerate() {
                assert_eq!(
                    matches!(child.launch_gate, KfdRuntimeLaunchGateV1::Semantic(_)),
                    semantic
                );
                assert!(!child.launch_gate.authorize_launch_v1(request));
                assert_eq!(calls[index].get(), 1);
                assert!(!child.launch_gate.advertises_atomics_v1());
                assert!(!child.launch_gate.advertises_collectives_v1());
                assert!(child.queue.is_none() && child.allocations.is_empty());
                assert!(!child.terminal);
            }
            assert!(backend.submissions.is_empty() && !backend.terminal);
            backend.shutdown_native_v1().unwrap();
        }
    }
}

#[test]
fn multi_open_peer_policy_selects_public_device_factory_without_changing_host_factory() {
    for native in [false, true] {
        let mut backend = opened(native);
        for child in &mut backend.children {
            child.scripted_sdma = Some(ScriptedSdmaDriverV1::new([
                ScriptedSdmaStepV1::Allocate {
                    kind: if native {
                        ScriptedBufferKindV1::PublicDevice
                    } else {
                        ScriptedBufferKindV1::Device
                    },
                    byte_len: 8,
                },
                ScriptedSdmaStepV1::Allocate {
                    kind: ScriptedBufferKindV1::Host,
                    byte_len: 8,
                },
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
                ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
            ]));
            let device = child
                .allocate_sdma_owner_v1(
                    RuntimeMemoryKindV1::DeviceLocal,
                    8,
                    8,
                    true,
                    "test device factory",
                )
                .unwrap();
            let host = child
                .allocate_sdma_owner_v1(
                    RuntimeMemoryKindV1::HostVisible,
                    8,
                    8,
                    true,
                    "test host factory",
                )
                .unwrap();
            assert!(child.directional_sdma_ops_v1().recycle(device).is_ok());
            assert!(child.directional_sdma_ops_v1().recycle(host).is_ok());
            let driver = child.scripted_sdma.as_ref().unwrap();
            assert!(driver.is_exhausted());
            assert_eq!(driver.live_owner_count(), 0);
            assert_eq!(driver.unexpected_drops(), 0);
        }
        backend.shutdown_native_v1().unwrap();
    }
}

#[test]
fn multi_open_host_visible_peer_copy_keeps_staged_fallback_and_exact_child_routing() {
    for native in [false, true] {
        let mut backend = opened(native);
        let stream = backend.create_stream_v1(10).unwrap();
        let source = backend
            .allocate_v1(30, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        let destination = backend
            .allocate_v1(10, RuntimeMemoryKindV1::HostVisible, 8, 8)
            .unwrap();
        backend.write_allocation_v1(source, 0, &[0x6b; 8]).unwrap();
        assert_eq!(backend.allocations[&source].child, 0);
        assert_eq!(backend.allocations[&destination].child, 1);
        assert_eq!(backend.streams[&stream].child, 1);
        let region = |allocation, access| BackendMemoryRegionV1 {
            allocation,
            access,
            byte_offset: 0,
            byte_len: 8,
        };
        let copy = backend
            .peer_copy_v1(
                stream,
                region(source, RuntimeAccessV1::Read),
                region(destination, RuntimeAccessV1::Write),
                &[],
            )
            .unwrap();
        let RoutedSubmissionV1::CooperativeCopy(record) = &backend.submissions[&copy] else {
            panic!("cooperative copy required")
        };
        assert!(record.compute_xgmi.is_none());
        assert_eq!(backend.cooperative_staging_bytes, 8);
        assert_eq!(backend.poll_v1(copy).unwrap(), BackendPollV1::Pending);
        assert_eq!(
            backend
                .drain_v1(copy, Instant::now() + Duration::from_secs(1))
                .unwrap(),
            BackendPollV1::Succeeded
        );
        let mut output = [0; 8];
        backend
            .read_allocation_v1(destination, 0, &mut output)
            .unwrap();
        assert_eq!(output, [0x6b; 8]);
        assert_eq!(backend.completed_compute_xgmi_copies, 0);
        assert!(backend.children[2].allocations.is_empty());
        backend.release_submission_v1(copy).unwrap();
        backend.release_allocation_v1(source).unwrap();
        backend.release_allocation_v1(destination).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
    }
}
