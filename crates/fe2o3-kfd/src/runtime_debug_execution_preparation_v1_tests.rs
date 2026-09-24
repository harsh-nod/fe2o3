//! Pure controls only. No actual cold owner, native resource or runtime evidence
//! is constructed. The ignored image check reads an explicitly supplied artifact.
use super::*;
use Gfx950DebugQueueLifecycleEventV1 as Event;
use Gfx950DebugQueueLifecyclePhaseV1 as Phase;
use profile::{KernelFactsV1, check_kernel, geometry};

fn kernel() -> KernelFactsV1 {
    KernelFactsV1 {
        wave: 64,
        private: 0,
        group: 0,
        sgprs: 32,
        vgprs: 4,
        agprs: Some(0),
        sgpr_spills: Some(0),
        vgpr_spills: Some(0),
        max_workgroup: 64,
        required_workgroup: Some([64, 1, 1]),
        max_workgroups: [Some(1); 3],
        cluster: None,
        kernarg_bytes: 16,
        kernarg_alignment: 8,
    }
}
fn empty_queue() -> Vec<Event> {
    vec![
        Event::TrapInstalled,
        Event::RuntimeEnableBegan,
        Event::RuntimeEnableAcknowledged,
        Event::EventCreated,
        Event::QueueCreated,
        Event::QueueDestroyed,
        Event::EventDestroyed,
        Event::RuntimeDisableBegan,
        Event::RuntimeDisableAcknowledged,
        Event::TrapClearAcknowledged,
        Event::DoorbellReleased,
        Event::BackingReleased,
    ]
}
#[test]
fn exact_static_profile_is_inert_and_closed() {
    assert_eq!(check_kernel(kernel()), Ok(()));
    let mut k = kernel();
    k.required_workgroup = None;
    k.max_workgroups = [None; 3];
    k.agprs = None;
    k.sgpr_spills = None;
    k.vgpr_spills = None;
    assert_eq!(check_kernel(k), Ok(()));
}
#[test]
fn wrong_wave_scratch_and_lds_are_precise() {
    let mut k = kernel();
    k.wave = 32;
    assert_eq!(check_kernel(k), Err(E::Wave));
    let mut k = kernel();
    k.private = 4;
    assert_eq!(check_kernel(k), Err(E::Scratch));
    let mut k = kernel();
    k.group = 512;
    assert_eq!(check_kernel(k), Err(E::GroupMemory));
}
#[test]
fn register_and_spill_variants_refuse() {
    for count in [0, 103, u16::MAX] {
        let mut k = kernel();
        k.sgprs = count;
        assert_eq!(check_kernel(k), Err(E::RegisterResources));
    }
    for count in [0, 257, u16::MAX] {
        let mut k = kernel();
        k.vgprs = count;
        assert_eq!(check_kernel(k), Err(E::RegisterResources));
    }
    for which in 0..3 {
        let mut k = kernel();
        match which {
            0 => k.agprs = Some(1),
            1 => k.sgpr_spills = Some(1),
            _ => k.vgpr_spills = Some(1),
        }
        assert_eq!(check_kernel(k), Err(E::RegisterResources));
    }
}
#[test]
fn workgroup_grid_and_cluster_constraints_refuse() {
    for count in [0, 63, 1025, u32::MAX] {
        let mut k = kernel();
        k.max_workgroup = count;
        assert_eq!(check_kernel(k), Err(E::Workgroup));
    }
    let mut k = kernel();
    k.required_workgroup = Some([128, 1, 1]);
    assert_eq!(check_kernel(k), Err(E::Workgroup));
    let mut k = kernel();
    k.max_workgroups = [None, Some(0), None];
    assert_eq!(check_kernel(k), Err(E::Workgroup));
    let mut k = kernel();
    k.cluster = Some([1; 3]);
    assert_eq!(check_kernel(k), Err(E::Cluster));
}
#[test]
fn exact_kernarg_capacity_and_alignment_are_bounded() {
    let mut k = kernel();
    k.kernarg_bytes = 65536;
    k.kernarg_alignment = 4096;
    assert_eq!(check_kernel(k), Ok(()));
    k.kernarg_bytes = 65537;
    assert_eq!(check_kernel(k), Err(E::Kernarg));
    for alignment in [0, 3, 8192, u64::MAX] {
        let mut k = kernel();
        k.kernarg_alignment = alignment;
        assert_eq!(check_kernel(k), Err(E::Kernarg));
    }
}
#[test]
fn actual_eight_xcc_geometry_fits_without_smaller_ring() {
    let g = geometry(1024 * 1024 + 4096, 1024 * 1024, 4 * 1024 * 1024).unwrap();
    assert_eq!(g.ring_bytes(), 8_388_608);
    assert_eq!(g.cwsr_bytes(), 181_829_632);
    assert_eq!(g.queue_backing_bytes(), 190_296_064);
    assert_eq!(g.projected_native_bytes(), 191_352_832);
    assert_eq!(g.projected_logical_bytes(), 196_595_712);
    assert_eq!(g.output_logical_bytes(), 272);
    assert_eq!(g.grid(), [64, 1, 1]);
    assert_eq!(g.workgroup(), [64, 1, 1]);
    assert_eq!(g.packet_limit(), 1);
    assert_eq!(
        g.contract(),
        Gfx950DebugExecutionContractV1::NoSamplingRuntime3TtmpCwsr8XccMetadata11
    );
}
#[test]
fn logical_bound_exact_one_over_and_overflow_are_checked() {
    let queue = geometry(0, 1, 1).unwrap().queue_backing_bytes();
    let exact = profile::MAX_LOGICAL_BYTES - queue - 4096 - 2;
    assert_eq!(
        geometry(exact, 1, 1).unwrap().projected_logical_bytes(),
        profile::MAX_LOGICAL_BYTES
    );
    assert_eq!(geometry(exact + 1, 1, 1), Err(E::LogicalStorageBound));
    assert_eq!(geometry(u64::MAX, 1, 1), Err(E::LogicalStorageBound));
}
#[test]
fn artifact_and_metadata_bounds_are_not_queue_budget_bypasses() {
    for n in [0, profile::MAX_OBJECT_BYTES + 1, usize::MAX] {
        assert_eq!(geometry(0, n, 1), Err(E::ObjectBound));
    }
    for n in [0, profile::MAX_METADATA_BYTES + 1, usize::MAX] {
        assert_eq!(geometry(0, 1, n), Err(E::MetadataIdentity));
    }
}
#[test]
fn empty_queue_model_orders_every_release() {
    assert_eq!(
        validate_gfx950_debug_queue_lifecycle_v1(&[]),
        Ok(Phase::Prepared)
    );
    assert_eq!(
        validate_gfx950_debug_queue_lifecycle_v1(&empty_queue()),
        Ok(Phase::ModelTeardownComplete)
    );
}
#[test]
fn stopped_dispatch_model_requires_resume_and_completion() {
    let mut rows = empty_queue();
    rows.splice(
        5..5,
        [
            Event::PacketPublished,
            Event::WaveStopped,
            Event::WaveResumed,
            Event::PacketCompleted,
        ],
    );
    assert_eq!(
        validate_gfx950_debug_queue_lifecycle_v1(&rows),
        Ok(Phase::ModelTeardownComplete)
    );
    for index in 5..9 {
        let mut invalid = rows.clone();
        invalid.remove(index);
        assert!(validate_gfx950_debug_queue_lifecycle_v1(&invalid).is_err());
    }
}
#[test]
fn every_missing_required_ack_refuses_complete_model() {
    let rows = empty_queue();
    for index in 0..rows.len() - 1 {
        let mut invalid = rows.clone();
        invalid.remove(index);
        assert!(
            validate_gfx950_debug_queue_lifecycle_v1(&invalid).is_err(),
            "missing {index}"
        );
    }
    assert_eq!(
        validate_gfx950_debug_queue_lifecycle_v1(&rows[..rows.len() - 1]),
        Ok(Phase::DoorbellReleased)
    );
}
#[test]
fn every_duplicate_event_refuses() {
    let rows = empty_queue();
    for (index, event) in rows.iter().copied().enumerate() {
        let mut invalid = rows.clone();
        invalid.insert(index, event);
        assert!(validate_gfx950_debug_queue_lifecycle_v1(&invalid).is_err());
    }
}
#[test]
fn swapped_runtime_and_event_cleanup_refuses() {
    let mut rows = empty_queue();
    rows.swap(6, 8);
    assert!(validate_gfx950_debug_queue_lifecycle_v1(&rows).is_err());
    let mut rows = empty_queue();
    rows.swap(9, 10);
    assert!(validate_gfx950_debug_queue_lifecycle_v1(&rows).is_err());
}
#[test]
fn uncertainty_is_sticky_and_cannot_become_acknowledged_cleanup() {
    let rows = empty_queue();
    for end in 0..rows.len() {
        let mut prefix = rows[..end].to_vec();
        prefix.push(Event::NativeOutcomeUncertain);
        assert_eq!(
            validate_gfx950_debug_queue_lifecycle_v1(&prefix),
            Ok(Phase::RetainUntilProcessExit)
        );
        prefix.push(Event::BackingReleased);
        assert!(validate_gfx950_debug_queue_lifecycle_v1(&prefix).is_err());
    }
}
#[test]
fn model_cap_refuses_before_interpreting_rows() {
    assert_eq!(
        validate_gfx950_debug_queue_lifecycle_v1(&[Event::TrapInstalled; 33]),
        Err(Gfx950DebugQueueLifecycleErrorV1::EventBound)
    );
    assert!(matches!(
        validate_gfx950_debug_queue_lifecycle_v1(&[Event::QueueCreated]),
        Err(Gfx950DebugQueueLifecycleErrorV1::Transition {
            ordinal: 0,
            phase: Phase::Prepared,
            event: Event::QueueCreated
        })
    ));
}
#[test]
fn native_boundary_has_no_success_and_lists_all_missing_evidence() {
    let closed = Gfx950DebugNativeUnavailableV1::PREPARATION_ONLY;
    assert_eq!(closed.missing_requirements().len(), 9);
    let rows = closed.missing_requirements();
    for (i, row) in rows.iter().enumerate() {
        assert!(!rows[..i].contains(row));
    }
    assert!(
        rows.contains(
            &Gfx950DebugNativeRequirementV1::LifetimeExclusiveNoSamplingAndNoForeignRuntime
        )
    );
    assert!(rows.contains(&Gfx950DebugNativeRequirementV1::ActualTrapRegistrationAndTtmpSetup));
    assert!(
        closed
            .reason()
            .contains("native queue lifecycle unavailable")
    );
}
#[test]
fn zero_region_refuses_missing_nonzero_and_truncation() {
    assert!(all_zero(Some(&[])));
    assert!(all_zero(Some(&[0; 64])));
    assert!(!all_zero(None));
    assert!(!all_zero(Some(&[0, 1, 0])));
}
#[test]
#[ignore = "requires explicitly selected real FE2O3_TEST_GFX950_COV6; CPU bytes only, no owner or GPU"]
fn exact_real_loader_image_and_selected_region_corruptions_are_rejected() {
    use std::io::Read;
    let path = std::env::var("FE2O3_TEST_GFX950_COV6").expect("set artifact path");
    let file = std::fs::File::open(path).unwrap();
    assert!(file.metadata().unwrap().is_file());
    assert!(file.metadata().unwrap().len() <= profile::MAX_OBJECT_BYTES as u64);
    let mut bytes = Vec::new();
    file.take((profile::MAX_OBJECT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(!bytes.is_empty() && bytes.len() <= profile::MAX_OBJECT_BYTES);
    let envelope =
        fe2o3_amdhsa_loader::validate(&bytes, AdmittedProfile::Gfx950XnackOffCov6).unwrap();
    let plan = envelope.materialization();
    let n = usize::try_from(plan.image_len()).unwrap();
    assert!(n > 0 && n <= profile::MAX_IMAGE_BYTES);
    let mut image = vec![0; n];
    envelope.materialize_into(&mut image).unwrap();
    assert!(image_matches(&image, plan));
    assert!(!image_matches(&image[..n - 1], plan));
    let mut positions = std::collections::BTreeSet::from([0, n - 1]);
    for ordinal in [
        SegmentOrdinal::First,
        SegmentOrdinal::Second,
        SegmentOrdinal::Third,
    ] {
        let zeros = plan.zero_phase().segment(ordinal);
        for r in [
            plan.copy_phase().segment(ordinal).destination(),
            zeros.mapping_prefix().destination(),
            zeros.memory_suffix().destination(),
            zeros.mapping_tail().destination(),
        ] {
            if r.byte_len() != 0 {
                positions.insert(r.offset_from_image_start() as usize);
                positions.insert((r.offset_from_image_start() + r.byte_len() - 1) as usize);
            }
        }
    }
    for ordinal in [
        InterSegmentGapOrdinal::FirstToSecond,
        InterSegmentGapOrdinal::SecondToThird,
    ] {
        let r = plan.zero_phase().inter_segment_gap(ordinal).destination();
        if r.byte_len() != 0 {
            positions.insert(r.offset_from_image_start() as usize);
            positions.insert((r.offset_from_image_start() + r.byte_len() - 1) as usize);
        }
    }
    assert!(positions.len() <= 32);
    for index in positions {
        image[index] ^= 1;
        assert!(!image_matches(&image, plan), "mutated byte {index}");
        image[index] ^= 1;
    }
    image.push(0);
    assert!(image_matches(&image, plan));
    image[n] = 1;
    assert!(!image_matches(&image, plan));
    assert!(fe2o3_amdhsa_loader::validate(&bytes, AdmittedProfile::Gfx942XnackOffCov6).is_err());
}
