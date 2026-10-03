use super::*;
use crate::authorized_execution::tests::{
    TestAuthorityV1, source_authority_for_device, source_projection,
};

struct Fixture {
    binding: GeneratedShellBindingV1,
    logical: Vec<crate::RuntimeAllocationIdV1>,
    hsaco: Vec<u8>,
    authority: TestAuthorityV1,
    storage: crate::GeneratedGfx942PersistentStorageV1,
    roster: GeneratedHostRosterV1,
}

impl Fixture {
    fn new(backend: &mut KfdMultiDeviceRuntimeBackendV1, device: u64) -> Self {
        let stream = backend.create_stream_v1(device).unwrap();
        let (hsaco, projection) = source_projection();
        let authority = source_authority_for_device(&projection, device);
        let roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
        let (binding, logical) =
            crate::RuntimeContextV1::generated_route_ids_for_test_v1(device, stream, roster.count);
        Self {
            binding,
            logical,
            hsaco,
            authority,
            storage: projection.into_generated_storage_v1(),
            roster,
        }
    }

    fn prepare(&self, backend: &mut KfdMultiDeviceRuntimeBackendV1) -> MultiGeneratedShellPlanV1 {
        backend
            .prepare_generated_shells_v1(self.binding, &self.roster, &self.logical)
            .unwrap()
    }

    fn install(&mut self, backend: &mut KfdMultiDeviceRuntimeBackendV1) -> GeneratedShellPlanV1 {
        let pending = self.prepare(backend);
        let global = *pending.plan();
        let bound = backend
            .bind_generated_shell_requests_v1(pending, core::array::from_fn(|_| None))
            .unwrap();
        let mut source =
            RuntimeGfx942GeneratedSourceMutV1::new(&mut self.storage, &self.hsaco, &self.authority);
        backend.commit_generated_shells_v1(bound, &mut source, &self.roster);
        assert!(!self.storage.control_available());
        global
    }
}

fn dispose(backend: &mut KfdMultiDeviceRuntimeBackendV1, plan: &GeneratedShellPlanV1) {
    backend.retire_generated_data_v1(plan).unwrap();
    assert!(backend.validate_generated_shell_disposal_v1(plan));
    backend.dispose_generated_shells_v1(plan);
    backend
        .destroy_stream_v1(plan.binding.backend_stream)
        .unwrap();
}

#[test]
fn generated_multi_shells_keep_colliding_child_ids_in_separate_private_routes() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let mut first = Fixture::new(&mut backend, 7);
    let mut second = Fixture::new(&mut backend, 8);
    let first = first.install(&mut backend);
    let second = second.install(&mut backend);
    let a = backend.generated_shells[&first.key];
    let b = backend.generated_shells[&second.key];
    assert_eq!(a.local.key, b.local.key);
    assert_eq!(
        a.local.binding.backend_stream,
        b.local.binding.backend_stream
    );
    assert_ne!(first.key, second.key);
    assert_eq!(backend.generated_allocations.len(), 6);
    assert!(backend.allocations.is_empty());
    assert!(backend.validate_generated_shell_records_v1(&first));
    assert!(backend.validate_generated_shell_records_v1(&second));
    assert!(backend.shutdown_native_v1().is_err());
    dispose(&mut backend, &second);
    assert_eq!(backend.generated_allocations.len(), 3);
    assert!(backend.validate_generated_shell_records_v1(&first));
    assert!(backend.children[1].allocations.is_empty());
    assert!(backend.children[2].allocations.is_empty());
    dispose(&mut backend, &first);
    assert!(backend.generated_allocations.is_empty());
    assert!(backend.generated_shells.is_empty());
    assert!(
        backend
            .children
            .iter()
            .all(|child| child.queue.is_none() && child.generated_submissions.is_empty())
    );
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn generated_multi_pending_plan_rejects_route_identity_and_handle_drift_before_transfer() {
    for mode in 0..7 {
        let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
        let fixture = Fixture::new(&mut backend, 8);
        let mut pending = fixture.prepare(&mut backend);
        match mode {
            0 => pending.scope.child = Some(0),
            1 => {
                pending.global.members[0]
                    .as_mut()
                    .unwrap()
                    .description
                    .ordinal = 1
            }
            2 => pending.global.binding.backend_device = 7,
            3 => pending.local.key += 1,
            4 => backend.next_handle += 1,
            5 => backend.children[1].next_handle += 1,
            _ => backend.compute_xgmi_children[1] = Some(1),
        }
        assert!(
            backend
                .bind_generated_shell_requests_v1(pending, core::array::from_fn(|_| None))
                .is_err()
        );
        assert!(fixture.storage.control_available());
        assert!(backend.generated_allocations.is_empty());
        assert!(
            backend
                .children
                .iter()
                .all(|child| child.generated_shells.is_empty())
        );
        backend.compute_xgmi_children.fill(None);
        backend
            .destroy_stream_v1(fixture.binding.backend_stream)
            .unwrap();
    }
}

#[test]
fn generated_multi_prepare_rejects_overflow_collision_wrong_stream_and_oversized_roster() {
    for mode in 0..6 {
        let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
        let mut fixture = Fixture::new(&mut backend, 8);
        match mode {
            0 => backend.next_handle = u64::MAX - 1,
            1 => backend.next_handle = fixture.binding.backend_stream,
            2 => fixture.binding.backend_device = 7,
            3 => fixture.roster.count = GFX942_MAX_FIXED_DISPATCH_DATA_V1 + 1,
            4 => {
                for id in 100..100 + crate::MAX_RUNTIME_ALLOCATIONS_V1 as u64 {
                    backend.generated_allocations.insert(
                        id,
                        RoutedHandleV1 {
                            child: 0,
                            local: id,
                        },
                    );
                }
            }
            _ => backend.compute_xgmi_children[1] = Some(1),
        }
        let next = backend.next_handle;
        assert!(
            backend
                .prepare_generated_shells_v1(fixture.binding, &fixture.roster, &fixture.logical)
                .is_err()
        );
        assert_eq!(backend.next_handle, next);
        assert!(fixture.storage.control_available());
        assert!(backend.generated_shells.is_empty());
        assert!(
            backend
                .children
                .iter()
                .all(|child| child.generated_shells.is_empty())
        );
        backend.generated_allocations.clear();
        backend.compute_xgmi_children.fill(None);
        backend
            .destroy_stream_v1(fixture.binding.backend_stream)
            .unwrap();
    }
}

#[test]
fn generated_multi_retained_route_corruption_terminalizes_without_discard() {
    for mode in 0..6 {
        let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
        let mut fixture = Fixture::new(&mut backend, 8);
        let plan = fixture.install(&mut backend);
        let member = plan.members[0].unwrap().backend;
        match mode {
            0 => {
                backend
                    .generated_allocations
                    .get_mut(&member)
                    .unwrap()
                    .child = 0
            }
            1 => {
                backend
                    .generated_allocations
                    .get_mut(&member)
                    .unwrap()
                    .local += 1
            }
            2 => {
                backend
                    .generated_shells
                    .get_mut(&plan.key)
                    .unwrap()
                    .local
                    .members[0]
                    .as_mut()
                    .unwrap()
                    .description
                    .ordinal = 2
            }
            3 => {
                backend.generated_allocations.remove(&member);
            }
            4 => {
                let route = backend.generated_allocations[&member];
                backend.generated_allocations.insert(u64::MAX, route);
            }
            _ => {
                let route = backend.generated_allocations[&member];
                backend.allocations.insert(u64::MAX, route);
            }
        }
        assert!(!backend.validate_generated_shell_records_v1(&plan));
        assert!(matches!(
            backend.retire_generated_data_v1(&plan),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(backend.terminal);
        assert!(backend.children[1].terminal);
        assert!(!backend.children[0].terminal);
        assert_eq!(backend.children[1].generated_shells.len(), 1);
        core::mem::forget(backend);
    }
}

#[test]
fn generated_multi_scope_quarantine_does_not_reresolve_mutated_stream_routes() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let stream = backend.create_stream_v1(8).unwrap();
    let scope = backend.generated_adoption_scope_v1(8, stream).unwrap();
    backend.streams.get_mut(&stream).unwrap().child = 0;
    let payload = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        backend.resume_generated_scope_panic_v1(scope, Box::new(73u32));
    }))
    .unwrap_err();
    assert_eq!(payload.downcast_ref::<u32>(), Some(&73));
    assert!(backend.terminal && backend.children[1].terminal);
    assert!(!backend.children[0].terminal && !backend.children[2].terminal);
    core::mem::forget(backend);
}

#[test]
fn generated_multi_ordinary_handles_cannot_alias_private_generated_routes() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let mut fixture = Fixture::new(&mut backend, 8);
    let plan = fixture.install(&mut backend);
    backend.next_handle = plan.members[0].unwrap().backend;
    assert!(backend.next_id().is_err());
    assert!(matches!(
        backend.route_allocation_v1(0, |_| panic!("colliding allocation callback")),
        crate::RuntimeRequestAllocationResultV1::Outcome(Err(RuntimeBackendFailureV1::Terminal(_)))
    ));
    assert_eq!(backend.generated_allocations.len(), 3);
    assert!(backend.allocations.is_empty());
    core::mem::forget(backend);
}

#[test]
fn generated_multi_public_memory_paths_reject_both_generated_endpoints() {
    use crate::{RuntimePeerCopySegmentV1, RuntimePeerCopySegmentsBackendV1};
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let mut fixture = Fixture::new(&mut backend, 8);
    let ordinary_stream = backend.create_stream_v1(7).unwrap();
    let ordinary = backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 32, 8)
        .unwrap();
    backend.write_allocation_v1(ordinary, 0, &[91; 32]).unwrap();
    let plan = fixture.install(&mut backend);
    let generated = plan.members[0].unwrap().backend;
    let region = |allocation, access| BackendMemoryRegionV1 {
        allocation,
        access,
        byte_offset: 0,
        byte_len: 8,
    };
    for (source, destination) in [(generated, ordinary), (ordinary, generated)] {
        let stream = if destination == ordinary {
            ordinary_stream
        } else {
            fixture.binding.backend_stream
        };
        let source = region(source, RuntimeAccessV1::Read);
        let destination = region(destination, RuntimeAccessV1::Write);
        assert!(
            backend
                .peer_copy_v1(stream, source, destination, &[])
                .is_err()
        );
        assert!(
            backend
                .peer_copy_segments_v1(
                    stream,
                    source,
                    destination,
                    &[RuntimePeerCopySegmentV1 {
                        source_offset: 0,
                        destination_offset: 0,
                        byte_len: 8,
                    }],
                    &[],
                )
                .is_err()
        );
        assert!(
            backend
                .copy_async_v1(stream, source, destination, &[])
                .is_err()
        );
    }
    assert!(backend.write_allocation_v1(generated, 0, &[0; 8]).is_err());
    assert!(
        backend
            .read_allocation_v1(generated, 0, &mut [0; 8])
            .is_err()
    );
    assert!(backend.release_allocation_v1(generated).is_err());
    assert!(backend.submissions.is_empty());
    assert!(backend.validate_generated_shell_records_v1(&plan));
    let mut ordinary_bytes = [0; 32];
    backend
        .read_allocation_v1(ordinary, 0, &mut ordinary_bytes)
        .unwrap();
    assert_eq!(ordinary_bytes, [91; 32]);
    dispose(&mut backend, &plan);
    backend.release_allocation_v1(ordinary).unwrap();
    backend.destroy_stream_v1(ordinary_stream).unwrap();
}

#[test]
fn generated_multi_prepare_latches_terminal_child_before_sibling_can_continue() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let fixture = Fixture::new(&mut backend, 8);
    backend.children[1].terminal = true;
    assert!(matches!(
        backend.prepare_generated_shells_v1(fixture.binding, &fixture.roster, &fixture.logical),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(backend.terminal);
    assert!(matches!(
        backend.create_stream_v1(7),
        Err(RuntimeBackendFailureV1::Terminal(_))
    ));
    assert!(!backend.children[0].terminal && !backend.children[2].terminal);
    assert!(fixture.storage.control_available());
    assert!(backend.generated_shells.is_empty());
    core::mem::forget(backend);
}

#[test]
fn generated_multi_commit_route_drift_quarantines_original_child_before_transfer() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let mut fixture = Fixture::new(&mut backend, 8);
    let pending = fixture.prepare(&mut backend);
    let bound = backend
        .bind_generated_shell_requests_v1(pending, core::array::from_fn(|_| None))
        .unwrap();
    backend
        .streams
        .get_mut(&fixture.binding.backend_stream)
        .unwrap()
        .child = 0;
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut source = RuntimeGfx942GeneratedSourceMutV1::new(
            &mut fixture.storage,
            &fixture.hsaco,
            &fixture.authority,
        );
        backend.commit_generated_shells_v1(bound, &mut source, &fixture.roster);
    }));
    assert!(failure.is_err());
    assert!(backend.terminal && backend.children[1].terminal);
    assert!(!backend.children[0].terminal && !backend.children[2].terminal);
    assert!(fixture.storage.control_available());
    assert!(backend.generated_shells.is_empty() && backend.generated_allocations.is_empty());
    assert!(
        backend
            .children
            .iter()
            .all(|child| child.generated_shells.is_empty())
    );
    core::mem::forget(backend);
}

#[test]
fn generated_multi_commit_source_mismatch_retains_rooted_routes_after_child_failure() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let fixture = Fixture::new(&mut backend, 8);
    let pending = fixture.prepare(&mut backend);
    let plan = pending.global;
    let bound = backend
        .bind_generated_shell_requests_v1(pending, core::array::from_fn(|_| None))
        .unwrap();
    let (hsaco, projection) = source_projection();
    let authority = source_authority_for_device(&projection, 8);
    let mut storage = projection.into_generated_storage_v1();
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut source = RuntimeGfx942GeneratedSourceMutV1::new(&mut storage, &hsaco, &authority);
        backend.commit_generated_shells_v1(bound, &mut source, &fixture.roster);
    }))
    .unwrap_err();
    assert!(
        failure
            .downcast_ref::<&str>()
            .is_some_and(|message| message.contains("same reserved source"))
    );
    assert!(backend.terminal && backend.children[1].terminal);
    assert!(!backend.children[0].terminal && !backend.children[2].terminal);
    assert!(fixture.storage.control_available() && storage.control_available());
    assert_eq!(backend.generated_shells[&plan.key], pending);
    assert_eq!(backend.generated_allocations.len(), plan.count);
    assert!(
        backend
            .children
            .iter()
            .all(|child| child.generated_shells.is_empty())
    );
    core::mem::forget(backend);
}

#[test]
fn generated_multi_disposal_corruption_keeps_routes_and_original_child_custody() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let mut fixture = Fixture::new(&mut backend, 8);
    let plan = fixture.install(&mut backend);
    let route = backend.generated_shells[&plan.key];
    let member = route.local.members[0].unwrap();
    assert!(
        backend.children[1]
            .allocations
            .remove_generated(member.backend, member.description)
    );
    let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        backend.dispose_generated_shells_v1(&plan);
    }));
    assert!(failure.is_err());
    assert!(backend.terminal && backend.children[1].terminal);
    assert!(!backend.children[0].terminal && !backend.children[2].terminal);
    assert_eq!(backend.generated_shells[&plan.key], route);
    assert_eq!(backend.generated_allocations.len(), plan.count);
    assert_eq!(backend.children[1].generated_shells.len(), 1);
    assert!(
        backend.children[1].generated_shells[&route.local.key]
            .control
            .is_some()
    );
    core::mem::forget(backend);
}

#[test]
fn generated_multi_copied_plan_and_disposed_replay_reject_without_poisoning_siblings() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let mut first = Fixture::new(&mut backend, 7);
    let mut second = Fixture::new(&mut backend, 8);
    let first = first.install(&mut backend);
    let second = second.install(&mut backend);
    for mode in 0..3 {
        let mut invalid = first;
        match mode {
            0 => invalid.key = u64::MAX,
            1 => invalid.binding.backend_device = 8,
            _ => invalid.members[0].as_mut().unwrap().description.ordinal = 1,
        }
        assert!(matches!(
            backend.retire_generated_data_v1(&invalid),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
        assert!(!backend.terminal && backend.children.iter().all(|child| !child.terminal));
        assert!(backend.validate_generated_shell_records_v1(&first));
        assert!(backend.validate_generated_shell_records_v1(&second));
    }
    dispose(&mut backend, &first);
    assert!(matches!(
        backend.retire_generated_data_v1(&first),
        Err(RuntimeBackendFailureV1::Rejected(_))
    ));
    assert!(!backend.terminal && backend.children.iter().all(|child| !child.terminal));
    assert!(backend.validate_generated_shell_records_v1(&second));
    dispose(&mut backend, &second);
    backend.shutdown_native_v1().unwrap();
}

#[test]
fn generated_multi_readiness_uses_exact_device_and_stream_without_advancing() {
    let mut backend = KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1();
    let first = backend.create_stream_v1(7).unwrap();
    let second = backend.create_stream_v1(8).unwrap();
    assert!(
        backend
            .generated_lane_ready_for_stream_v1(8, second)
            .unwrap()
    );
    assert!(
        backend
            .generated_lane_ready_for_stream_v1(7, second)
            .is_err()
    );
    backend.compute_xgmi_children[0] = Some(1);
    assert!(
        !backend
            .generated_lane_ready_for_stream_v1(7, first)
            .unwrap()
    );
    assert!(
        backend
            .generated_lane_ready_for_stream_v1(8, second)
            .unwrap()
    );
    assert!(backend.generated_allocations.is_empty());
    assert!(backend.generated_shells.is_empty());
    assert!(backend.children.iter().all(|child| child.queue.is_none()));
    backend.compute_xgmi_children[0] = None;
    backend.destroy_stream_v1(first).unwrap();
    backend.destroy_stream_v1(second).unwrap();
}
