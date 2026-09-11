use super::*;
use crate::RuntimeLaunchGeometryV1;
use crate::authorized_execution::tests::{TestAuthorityV1, source_authority, source_projection};

struct Fixture {
    backend: KfdRuntimeBackendV1,
    binding: GeneratedShellBindingV1,
    logical: Vec<RuntimeAllocationIdV1>,
    hsaco: Vec<u8>,
    authority: TestAuthorityV1,
    storage: crate::GeneratedGfx942PersistentStorageV1,
    roster: GeneratedHostRosterV1,
}

impl Fixture {
    fn new() -> Self {
        let mut backend = KfdRuntimeBackendV1::mock();
        let (binding, logical) =
            crate::RuntimeContextV1::generated_shell_test_binding_v1(&mut backend);
        let (hsaco, projection) = source_projection();
        let authority = source_authority(&projection);
        let roster = GeneratedHostRosterV1::from_projection(&projection).unwrap();
        Self {
            backend,
            binding,
            logical,
            hsaco,
            authority,
            storage: projection.into_generated_storage_v1(),
            roster,
        }
    }

    fn prepare(
        &mut self,
    ) -> Result<GeneratedShellPlanV1, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>> {
        self.backend
            .prepare_generated_shells_v1(self.binding, &self.roster, &self.logical)
    }

    fn install(&mut self) -> GeneratedShellPlanV1 {
        let plan = self.prepare().unwrap();
        let mut source =
            RuntimeGfx942GeneratedSourceMutV1::new(&mut self.storage, &self.hsaco, &self.authority);
        self.backend
            .commit_generated_shells_v1(plan, &mut source, &self.roster);
        plan
    }

    fn assert_no_native_effects(&self) {
        assert!(self.backend.admitted_device.is_none());
        assert!(self.backend.queue.is_none());
        assert!(self.backend.terminal_memory.is_none());
        assert!(
            self.backend
                .native_compute_lanes
                .iter()
                .all(Option::is_none)
        );
        assert_eq!(self.backend.auxiliary_compute_lanes.len(), 1);
        for lane in &self.backend.auxiliary_compute_lanes {
            assert!(lane.owner_stream.is_none());
            assert!(lane.active.is_none());
            assert!(lane.pipeline.is_empty());
            assert!(lane.resident_data.is_none());
            assert!(lane.recycled_dispatch.is_none());
        }
        assert!(self.backend.submissions.is_empty());
        assert!(self.backend.pending_compute.is_empty());
        assert!(self.backend.active.is_none());
        assert!(self.backend.active_sdma.is_empty());
        assert_eq!(self.backend.compute_completion_reservations, 0);
        assert_eq!(self.backend.sdma_completion_reservations, 0);
    }
}

fn is_busy<T>(result: Result<T, RuntimeBackendFailureV1<KfdRuntimeBackendErrorV1>>) -> bool {
    matches!(result, Err(RuntimeBackendFailureV1::Rejected(error)) if error.kind() == KfdRuntimeBackendErrorKindV1::Busy)
}

#[test]
fn shell_table_keeps_generated_slots_without_ordinary_bytes_or_native_effects() {
    let mut fixture = Fixture::new();
    let plan = fixture.install();
    assert_eq!(fixture.backend.allocations.len(), 3);
    assert!(fixture.backend.allocations.has_generated());
    assert_eq!(fixture.backend.allocations.ordinary_iter().count(), 0);
    assert_eq!(fixture.backend.staged_context_bytes, 0);
    for member in plan.members.iter().flatten() {
        assert!(fixture.backend.allocations.contains_key(&member.backend));
        assert!(fixture.backend.allocations.get(&member.backend).is_none());
        assert!(
            fixture
                .backend
                .allocations
                .get_mut(&member.backend)
                .is_none()
        );
        assert!(
            fixture
                .backend
                .allocations
                .remove(&member.backend)
                .is_none()
        );
        assert_eq!(
            fixture.backend.allocations.generated(member.backend),
            Some(member.description)
        );
    }
    fixture.assert_no_native_effects();
    assert!(!fixture.storage.control_available());
    assert_eq!(fixture.storage.buffers().len(), 3);
    fixture.backend.dispose_generated_shells_v1(&plan);
    assert!(fixture.backend.allocations.is_empty());
    assert!(fixture.backend.generated_shells.is_empty());
    assert_eq!(fixture.backend.staged_context_bytes, 0);
    fixture.assert_no_native_effects();
}

#[test]
fn shell_ordinary_apis_reject_before_native_lookup_or_destination_changes() {
    let mut fixture = Fixture::new();
    let ordinary = fixture
        .backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 16, 4)
        .unwrap();
    fixture
        .backend
        .write_allocation_v1(ordinary, 0, &[0x5a; 16])
        .unwrap();
    let plan = fixture.install();
    let next = fixture.backend.next_handle;
    let staged = fixture.backend.staged_context_bytes;
    for member in plan.members.iter().flatten() {
        let id = member.backend;
        let mut destination = [0xa5; 16];
        assert!(is_busy(fixture.backend.read_allocation_v1(
            id,
            0,
            &mut destination
        )));
        assert_eq!(destination, [0xa5; 16]);
        assert!(is_busy(
            fixture.backend.write_allocation_v1(id, 0, &[9; 16])
        ));
        assert!(is_busy(fixture.backend.release_allocation_v1(id)));
        for (source, destination) in [(id, ordinary), (ordinary, id)] {
            assert!(is_busy(fixture.backend.copy_async_v1(
                plan.binding.backend_stream,
                BackendMemoryRegionV1 {
                    allocation: source,
                    access: RuntimeAccessV1::Read,
                    byte_offset: 0,
                    byte_len: 16
                },
                BackendMemoryRegionV1 {
                    allocation: destination,
                    access: RuntimeAccessV1::Write,
                    byte_offset: 0,
                    byte_len: 16
                },
                &[]
            )));
        }
        let bindings = [BackendBindingV1 {
            region: BackendMemoryRegionV1 {
                allocation: id,
                access: RuntimeAccessV1::ReadWrite,
                byte_offset: 0,
                byte_len: 16,
            },
            kernarg_byte_offset: 0,
        }];
        assert!(is_busy(fixture.backend.submit_v1(BackendLaunchV1 {
            stream: plan.binding.backend_stream,
            kernel: 0,
            explicit_kernarg: &[0; 8],
            bindings: &bindings,
            dependencies: &[],
            geometry: RuntimeLaunchGeometryV1 {
                grid: [64, 1, 1],
                workgroup: [64, 1, 1],
                dynamic_shared_bytes: 0
            },
            semantic_launch: BackendSemanticLaunchV1::Ordinary,
        })));
    }
    assert_eq!(fixture.backend.next_handle, next);
    assert_eq!(fixture.backend.staged_context_bytes, staged);
    assert_eq!(&*fixture.backend.allocations[&ordinary].bytes, &[0x5a; 16]);
    assert!(fixture.backend.validate_generated_shell_disposal_v1(&plan));
    fixture.assert_no_native_effects();
    fixture.backend.dispose_generated_shells_v1(&plan);
    fixture.backend.release_allocation_v1(ordinary).unwrap();
    assert_eq!(fixture.backend.staged_context_bytes, 0);
}

#[test]
fn shell_corrupted_retained_rosters_reject_before_any_disposal() {
    for axis in 0..11 {
        let mut fixture = Fixture::new();
        let original = fixture.install();
        let mut changed = original;
        let mut member = changed.members[1].unwrap();
        match axis {
            0 => member = changed.members[0].unwrap(),
            1 => member.logical = changed.members[0].unwrap().logical,
            2 => member.description.ordinal = 0,
            3 => member.description.adoption += 1,
            4 => member.description.device += 1,
            5 => member.description.alignment *= 2,
            6 => member.description.kind = RuntimeMemoryKindV1::DeviceLocal,
            7 => member.description.byte_len = 0,
            8 => member.backend += 10,
            9 => changed.count -= 1,
            _ => changed.members[2] = None,
        }
        changed.members[1] = Some(member);
        // Corrupt both the retained plan and table, so descriptor equality alone
        // cannot make this a passing negative test.
        fixture.backend.allocations = AllocationTableV1::default();
        for member in changed.members.iter().flatten() {
            if !fixture.backend.allocations.contains_key(&member.backend) {
                fixture
                    .backend
                    .allocations
                    .insert_generated(member.backend, member.description);
            }
        }
        fixture
            .backend
            .generated_shells
            .get_mut(&original.key)
            .unwrap()
            .plan = changed;
        let count = fixture.backend.allocations.len();
        assert!(
            !fixture
                .backend
                .validate_generated_shell_disposal_v1(&changed),
            "axis {axis}"
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fixture
                .backend
                .dispose_generated_shells_v1(&changed)))
            .is_err(),
            "axis {axis}"
        );
        assert_eq!(fixture.backend.allocations.len(), count);
        assert!(
            fixture.backend.generated_shells[&original.key]
                .control
                .is_some()
        );
        fixture
            .backend
            .generated_shells
            .get_mut(&original.key)
            .unwrap()
            .plan = original;
        fixture.backend.allocations = AllocationTableV1::default();
        for member in original.members.iter().flatten() {
            fixture
                .backend
                .allocations
                .insert_generated(member.backend, member.description);
        }
        fixture.backend.dispose_generated_shells_v1(&original);
    }
}

#[test]
fn shell_reordered_retained_plan_and_extra_owner_member_reject() {
    let mut fixture = Fixture::new();
    let original = fixture.install();
    let mut changed = original;
    changed.members.swap(0, 1);
    fixture
        .backend
        .generated_shells
        .get_mut(&original.key)
        .unwrap()
        .plan = changed;
    assert!(
        !fixture
            .backend
            .validate_generated_shell_disposal_v1(&changed)
    );
    fixture
        .backend
        .generated_shells
        .get_mut(&original.key)
        .unwrap()
        .plan = original;
    let extra = original.members[0].unwrap().description;
    fixture
        .backend
        .allocations
        .insert_generated(original.next_handle, extra);
    assert!(
        !fixture
            .backend
            .validate_generated_shell_disposal_v1(&original)
    );
    assert!(
        fixture.backend.generated_shells[&original.key]
            .control
            .is_some()
    );
    assert!(
        fixture
            .backend
            .allocations
            .remove_generated(original.next_handle, extra)
    );
    assert!(
        fixture
            .backend
            .validate_generated_shell_disposal_v1(&original)
    );
    fixture.backend.dispose_generated_shells_v1(&original);
}

#[test]
fn shell_backend_identity_exhaustion_rejects_before_commit() {
    let mut fixture = Fixture::new();
    for next in [0, u64::MAX - 3, u64::MAX] {
        fixture.backend.next_handle = next;
        assert!(fixture.prepare().is_err());
        assert_eq!(fixture.backend.next_handle, next);
        assert!(fixture.backend.allocations.is_empty());
        assert!(fixture.backend.generated_shells.is_empty());
        assert!(fixture.storage.control_available());
    }
    fixture.backend.next_handle = u64::MAX - 4;
    let plan = fixture.install();
    assert_eq!(fixture.backend.next_handle, u64::MAX);
    fixture.backend.dispose_generated_shells_v1(&plan);
}

#[test]
fn shell_malformed_preflight_rosters_never_commit_a_prefix() {
    for axis in 0..6 {
        let mut fixture = Fixture::new();
        let next = fixture.backend.next_handle;
        match axis {
            0 => fixture.roster.count = 0,
            1 => fixture.roster.count = GFX942_MAX_FIXED_DISPATCH_DATA_V1 + 1,
            2 => fixture.roster.buffers[2] = None,
            3 => fixture.roster.buffers[2].as_mut().unwrap().ordinal = 0,
            4 => fixture.roster.buffers[2].as_mut().unwrap().bytes = 0,
            _ => fixture.roster.buffers[3] = fixture.roster.buffers[0],
        }
        assert!(fixture.prepare().is_err());
        assert_eq!(fixture.backend.next_handle, next);
        assert!(fixture.backend.allocations.is_empty());
        assert!(fixture.backend.generated_shells.is_empty());
        assert!(fixture.storage.control_available());
    }
}

#[test]
fn shell_invalid_disposal_counts_and_handle_bounds_reject_without_panicking() {
    let mut fixture = Fixture::new();
    let original = fixture.install();
    for (count, key) in [
        (0, original.key),
        (GFX942_MAX_FIXED_DISPATCH_DATA_V1 + 1, original.key),
        (original.count, 0),
        (original.count, u64::MAX),
    ] {
        let changed = GeneratedShellPlanV1 {
            count,
            key,
            ..original
        };
        fixture
            .backend
            .generated_shells
            .get_mut(&original.key)
            .unwrap()
            .plan = changed;
        assert!(
            !fixture
                .backend
                .validate_generated_shell_disposal_v1(&changed)
        );
        assert!(
            fixture.backend.generated_shells[&original.key]
                .control
                .is_some()
        );
        assert_eq!(fixture.backend.allocations.len(), original.count);
    }
    fixture
        .backend
        .generated_shells
        .get_mut(&original.key)
        .unwrap()
        .plan = original;
    fixture.backend.dispose_generated_shells_v1(&original);
}

#[test]
fn shell_late_handle_collision_preserves_existing_ordinary_allocation() {
    let mut fixture = Fixture::new();
    let ordinary = fixture
        .backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 4, 4)
        .unwrap();
    let record = fixture.backend.allocations.remove(&ordinary).unwrap();
    let collision = fixture.backend.next_handle + 3;
    fixture.backend.allocations.insert(collision, record);
    let next = fixture.backend.next_handle;
    assert!(fixture.prepare().is_err());
    assert_eq!(fixture.backend.next_handle, next);
    assert_eq!(fixture.backend.allocations.len(), 1);
    assert!(fixture.backend.allocations.get(&collision).is_some());
    assert!(fixture.backend.generated_shells.is_empty());
    assert!(fixture.storage.control_available());
    fixture.backend.release_allocation_v1(collision).unwrap();
}

#[test]
fn shell_table_duplicate_insert_and_wrong_remove_preserve_owners() {
    let mut fixture = Fixture::new();
    let ordinary = fixture
        .backend
        .allocate_v1(7, RuntimeMemoryKindV1::HostVisible, 4, 4)
        .unwrap();
    let plan = fixture.install();
    let member = plan.members[0].unwrap();
    let mut wrong = member.description;
    wrong.adoption += 1;
    assert!(
        !fixture
            .backend
            .allocations
            .remove_generated(member.backend, wrong)
    );
    assert!(
        !fixture
            .backend
            .allocations
            .remove_generated(ordinary, member.description)
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fixture
            .backend
            .allocations
            .insert_generated(ordinary, member.description)))
        .is_err()
    );
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| fixture
            .backend
            .allocations
            .insert_generated(member.backend, wrong)))
        .is_err()
    );
    assert!(fixture.backend.allocations.get(&ordinary).is_some());
    assert_eq!(
        fixture.backend.allocations.generated(member.backend),
        Some(member.description)
    );
    assert_eq!(fixture.backend.allocations.ordinary_iter().count(), 1);
    fixture.backend.dispose_generated_shells_v1(&plan);
    fixture.backend.release_allocation_v1(ordinary).unwrap();
}

#[cfg(feature = "hardware-qualification")]
#[test]
fn shell_copy_drain_qualification_rejects_unaccounted_generated_storage() {
    let mut fixture = Fixture::new();
    fixture.backend.launch_gate = KfdRuntimeLaunchGateV1::CopyOnlyQualification;
    let plan = fixture.install();
    assert!(matches!(
        fixture.backend.drain_capture_runtime_roster_v1(),
        Err(qualification_drain_capture::KfdDrainCaptureObservationFailureV1::ComputeState)
    ));
    assert!(fixture.backend.validate_generated_shell_disposal_v1(&plan));
    fixture.backend.dispose_generated_shells_v1(&plan);
}
