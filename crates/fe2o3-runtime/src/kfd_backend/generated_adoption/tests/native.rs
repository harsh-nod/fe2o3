//! Exact-fixture DATA mechanics only, not Worker authority or publication.

use super::*;
use crate::RuntimeContextV1;
use crate::qualification_gfx942_vecadd_v1::*;
use fe2o3_amdhsa_loader::{AdmittedProfile, KernelGlobalBufferAbiV1, validate};
use fe2o3_hsaco::ArgumentAccess;

fn install(backend: &mut KfdRuntimeBackendV1, stream: u64) -> GeneratedShellPlanV1 {
    install_with_roster(backend, stream).0
}

fn install_with_roster(
    backend: &mut KfdRuntimeBackendV1,
    stream: u64,
) -> (GeneratedShellPlanV1, GeneratedHostRosterV1) {
    let admitted = admit_gfx942_vecadd_qualification_v1().unwrap();
    let uid = backend.description.backend_device;
    let model = backend
        .with_retained_preparation_device_v1(uid, |device| device.model_admission())
        .unwrap();
    let (binding, logical) = RuntimeContextV1::generated_native_test_ids_v1(uid, stream, model);
    let (left, right, output, _expected) = admitted.host_buffers().unwrap().into_parts();
    let buffers: Vec<_> = [left, right, output]
        .into_iter()
        .map(|bytes| crate::Gfx942KfdDispatchBufferV1::new(bytes).unwrap())
        .collect();
    let roster = GeneratedHostRosterV1 {
        source_identity: Arc::new(()),
        buffers: core::array::from_fn(|index| {
            (index < 3).then_some(crate::generated_source::GeneratedBufferSlotV1 {
                ordinal: index,
                bytes: GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                access: if index < 2 {
                    crate::Gfx942RuntimeBufferAccessV1::ReadOnly
                } else {
                    crate::Gfx942RuntimeBufferAccessV1::WriteOnly
                },
            })
        }),
        count: 3,
        readback_bytes: 3 * GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
        fixup_count: 3,
        dispatch_contract_sha256: admitted.signature(),
    };
    let rows: Vec<_> = admitted
        .arguments()
        .iter()
        .map(|argument| {
            KernelGlobalBufferAbiV1::new(
                argument.explicit_argument_index,
                argument.name,
                u64::from(argument.pointer_offset),
                argument.reconciled_pointee_alignment,
                if argument.access == RuntimeAccessV1::Read {
                    ArgumentAccess::ReadOnly
                } else {
                    ArgumentAccess::WriteOnly
                },
            )
        })
        .collect();
    let program = validate(admitted.hsaco(), AdmittedProfile::Gfx942XnackOffCov6)
        .unwrap()
        .bind_kernel(admitted.kernel_name())
        .unwrap()
        .reconcile_dispatch_abi(admitted.signature(), &rows)
        .unwrap();
    let packet = Gfx942FixedDispatchPacketV1::new(
        0,
        AqlDispatchGeometryV1::new(admitted.geometry().grid, admitted.geometry().workgroup)
            .unwrap(),
        0,
        admitted.explicit_kernarg().to_vec().into_boxed_slice(),
        admitted
            .arguments()
            .iter()
            .enumerate()
            .map(|(index, argument)| {
                Gfx942DispatchBufferBindingV1::new(
                    argument.explicit_argument_index,
                    index,
                    0,
                    GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let plan = backend
        .prepare_generated_shells_v1(binding, &roster, &logical)
        .unwrap();
    // This test bypasses only carrier registration, never native device or loader
    // admission. No TestAuthority is used for these native calls.
    backend.generated_shells.insert(
        plan.key,
        generated_shells::GeneratedShellRecordV1 {
            plan,
            source_identity: Arc::clone(&roster.source_identity),
            control: Some(packet),
            native: None,
        },
    );
    backend.next_handle = plan.key + 1 + plan.count as u64;
    for member in plan.members.iter().flatten() {
        backend
            .allocations
            .insert_generated(member.backend, member.description);
    }
    backend
        .adopt_generated_data_v1(&plan, &roster, program, &buffers)
        .unwrap();
    (plan, roster)
}

fn retire(backend: &mut KfdRuntimeBackendV1, plan: &GeneratedShellPlanV1) {
    backend.retire_generated_data_v1(plan).unwrap();
    let submission = backend.generated_shells[&plan.key]
        .native
        .as_ref()
        .unwrap()
        .submission
        .as_ref()
        .map(|submission| submission.id);
    if let Some(submission) = submission {
        backend.release_submission_v1(submission).unwrap();
    }
    assert!(backend.validate_generated_shell_disposal_v1(plan));
    assert_eq!(
        backend.generated_shells[&plan.key]
            .native
            .as_ref()
            .unwrap()
            .returned
            .completed,
        3
    );
    backend.dispose_generated_shells_v1(plan);
}

fn run(bootstrap: bool) {
    assert_eq!(
        std::env::var("FE2O3_TEST_NATIVE_ISOLATED").as_deref(),
        Ok("1"),
        "explicit isolated qualification window required"
    );
    let raw = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_ID").expect("explicit physical device UID");
    let uid = u64::from_str_radix(raw.strip_prefix("0x").unwrap_or(&raw), 16).unwrap();
    let mut backend = KfdRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(uid).unwrap();
    if bootstrap {
        let allocation = backend
            .allocate_v1(uid, RuntimeMemoryKindV1::HostVisible, 4096, 4096)
            .unwrap();
        backend.release_allocation_v1(allocation).unwrap();
        assert!(backend.queue.is_some());
        assert!(backend.native_compute_lanes.iter().all(Option::is_none));
    }
    let first_stream = backend.create_stream_v1(uid).unwrap();
    let second_stream = backend.create_stream_v1(uid).unwrap();
    let first = install(&mut backend, first_stream);
    let primary = backend.native_compute_lanes[0].unwrap();
    assert_eq!(
        backend.generated_shells[&first.key]
            .native
            .as_ref()
            .unwrap()
            .phase,
        PhaseV1::Adopted
    );
    assert_eq!(backend.free_compute_lane_v1(), Some(1));
    let second = install(&mut backend, second_stream);
    let auxiliary = backend.native_compute_lanes[1].unwrap();
    assert_ne!(primary, auxiliary);
    assert!(backend.free_compute_lane_v1().is_none());
    retire(&mut backend, &first);
    assert_eq!(backend.native_compute_lanes[0], Some(primary));
    let rebound = install(&mut backend, first_stream);
    assert_eq!(backend.native_compute_lanes[0], Some(primary));
    assert_eq!(backend.native_compute_lanes[1], Some(auxiliary));
    let unpublished = backend.submissions.is_empty()
        && backend.pending_compute.is_empty()
        && !backend.any_compute_active_v1()
        && backend.compute_completion_reservations == 0
        && backend.sdma_completion_reservations == 0;
    retire(&mut backend, &rebound);
    retire(&mut backend, &second);
    assert!(backend.stream_compute_lanes.is_empty());
    assert!(backend.allocations.is_empty());
    backend.destroy_stream_v1(first_stream).unwrap();
    backend.destroy_stream_v1(second_stream).unwrap();
    backend.shutdown_native_v1().unwrap();
    assert!(unpublished);
    assert!(backend.queue.is_none());
    eprintln!(
        "N5 DATA native mechanics: bootstrap={bootstrap}, primary/AUX/rebound, 9 DATA releases, zero publication, shutdown complete"
    );
}

#[test]
#[ignore = "requires isolated MI300X, FE2O3_TEST_NATIVE_ISOLATED=1 and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn generated_native_cold_primary_auxiliary_rebound_abort_and_shutdown() {
    run(false);
}

#[test]
#[ignore = "requires isolated MI300X, FE2O3_TEST_NATIVE_ISOLATED=1 and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn generated_native_bootstrap_primary_auxiliary_rebound_abort_and_shutdown() {
    run(true);
}

fn run_issue(bootstrap: bool) {
    assert_eq!(
        std::env::var("FE2O3_TEST_NATIVE_ISOLATED").as_deref(),
        Ok("1")
    );
    let raw = std::env::var("FE2O3_TEST_NATIVE_UNIQUE_ID").expect("explicit device UID");
    let uid = u64::from_str_radix(raw.strip_prefix("0x").unwrap_or(&raw), 16).unwrap();
    let mut backend = KfdRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(uid).unwrap();
    if bootstrap {
        let allocation = backend
            .allocate_v1(uid, RuntimeMemoryKindV1::HostVisible, 4096, 4096)
            .unwrap();
        backend.release_allocation_v1(allocation).unwrap();
        assert!(backend.queue.is_some());
        assert!(backend.native_compute_lanes.iter().all(Option::is_none));
    } else {
        assert!(backend.queue.is_none());
    }
    let first = backend.create_stream_v1(uid).unwrap();
    let second = backend.create_stream_v1(uid).unwrap();
    let (first_plan, first_roster) = install_with_roster(&mut backend, first);
    let primary = backend.native_compute_lanes[0].unwrap();
    assert_eq!(
        backend.generated_shells[&first_plan.key]
            .native
            .as_ref()
            .unwrap()
            .native_lane,
        Some(primary)
    );
    let (second_plan, second_roster) = install_with_roster(&mut backend, second);
    let auxiliary = backend.native_compute_lanes[1].unwrap();
    assert_ne!(primary, auxiliary);
    assert_eq!(
        backend.generated_shells[&second_plan.key]
            .native
            .as_ref()
            .unwrap()
            .native_lane,
        Some(auxiliary)
    );
    let (_, _, _, expected) = admit_gfx942_vecadd_qualification_v1()
        .unwrap()
        .host_buffers()
        .unwrap()
        .into_parts();
    let first_id = backend
        .prepare_generated_issue_v1(&first_plan, &first_roster)
        .unwrap();
    let second_id = backend
        .prepare_generated_issue_v1(&second_plan, &second_roster)
        .unwrap();
    assert_ne!(first_id, second_id);
    for (plan, id) in [(&first_plan, first_id), (&second_plan, second_id)] {
        assert_eq!(backend.poll_v1(id).unwrap(), BackendPollV1::Pending);
        assert!(matches!(
            backend.generated_shells[&plan.key]
                .native
                .as_ref()
                .unwrap()
                .submission
                .as_ref()
                .unwrap()
                .receipt,
            ReceiptV1::Ready
        ));
        assert!(matches!(
            backend.release_submission_v1(id),
            Err(RuntimeBackendFailureV1::Rejected(_))
        ));
    }
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut complete = [false; 2];
    while !complete.iter().all(|done| *done) {
        for (index, (plan, id)) in [(&first_plan, first_id), (&second_plan, second_id)]
            .into_iter()
            .enumerate()
        {
            if !complete[index] {
                complete[index] = backend.advance_generated_issue_v1(plan, id).unwrap();
            }
        }
        assert!(
            Instant::now() < deadline,
            "generated fixture completion deadline"
        );
        std::thread::yield_now();
    }
    for (plan, id) in [(&first_plan, first_id), (&second_plan, second_id)] {
        assert!(matches!(
            backend.poll_v1(id),
            Err(RuntimeBackendFailureV1::Quiescent(_))
        ));
        let handle = backend.generated_shells[&plan.key]
            .native
            .as_ref()
            .unwrap()
            .native_lane
            .unwrap();
        let mut output = vec![0; expected.len()];
        backend
            .queue
            .as_mut()
            .unwrap()
            .with_compute_lane_v1(handle, |lane| {
                let generation = lane.recycled_fixed_dispatch_generation()?;
                lane.read_recycled_fixed_dispatch_data_into(
                    fe2o3_kfd::Gfx942CompletedDispatchReadRequestV1::new(
                        generation,
                        2,
                        0,
                        output.len() as u64,
                    ),
                    &mut output,
                )
            })
            .unwrap()
            .unwrap();
        assert_eq!(output, expected);
        retire(&mut backend, plan);
    }
    // A fresh generation on the same original primary, not cross-run input reuse.
    let (rebound, roster) = install_with_roster(&mut backend, first);
    assert_eq!(backend.native_compute_lanes[0], Some(primary));
    assert_eq!(backend.native_compute_lanes[1], Some(auxiliary));
    assert_eq!(
        backend.generated_shells[&rebound.key]
            .native
            .as_ref()
            .unwrap()
            .native_lane,
        Some(primary)
    );
    let id = backend
        .prepare_generated_issue_v1(&rebound, &roster)
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !backend.advance_generated_issue_v1(&rebound, id).unwrap() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    retire(&mut backend, &rebound);
    assert!(backend.generated_submissions.is_empty());
    assert!(backend.generated_shells.is_empty());
    assert!(backend.allocations.is_empty());
    assert!(backend.submissions.is_empty());
    assert!(backend.stream_compute_lanes.is_empty());
    assert!(!backend.any_compute_active_v1());
    backend.destroy_stream_v1(first).unwrap();
    backend.destroy_stream_v1(second).unwrap();
    backend.shutdown_native_v1().unwrap();
    assert!(backend.queue.is_none());
    eprintln!(
        "I2 native fixture: bootstrap={bootstrap}, primary/AUX exact output, rebound completion, 9 DATA disposals, shutdown complete; no protected Worker/carrier or typed reply claim"
    );
}

#[test]
#[ignore = "requires isolated MI300X, FE2O3_TEST_NATIVE_ISOLATED=1 and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn generated_native_cold_issue_complete_readback_retire() {
    run_issue(false);
}

#[test]
#[ignore = "requires isolated MI300X, FE2O3_TEST_NATIVE_ISOLATED=1 and FE2O3_TEST_NATIVE_UNIQUE_ID"]
fn generated_native_bootstrap_issue_complete_readback_retire() {
    run_issue(true);
}
