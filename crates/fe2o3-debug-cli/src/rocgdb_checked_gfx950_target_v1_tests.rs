use super::*;

#[test]
fn only_the_exact_gfx950_minus_wave64_profile_is_accepted() {
    require_profile(AmdTargetId::parse("gfx950:xnack-").unwrap(), 64).unwrap();
    for target in [
        "gfx942:xnack-",
        "gfx950",
        "gfx950:xnack+",
        "gfx950:sramecc+:xnack-",
        "gfx950:sramecc-:xnack-",
    ] {
        assert_eq!(
            require_profile(AmdTargetId::parse(target).unwrap(), 64),
            Err(RocgdbCheckedGfx950ArtifactErrorV1::ArtifactTarget)
        );
    }
    for wave in [0, 1, 32, 63, 65, u32::MAX] {
        assert_eq!(
            require_profile(AmdTargetId::parse("gfx950:xnack-").unwrap(), wave),
            Err(RocgdbCheckedGfx950ArtifactErrorV1::KernelWaveWidth)
        );
    }
}

#[test]
fn artifact_and_kernel_bounds_precede_inspection_without_allocating_max_inputs() {
    assert_eq!(
        require_input_bounds(0, "kernel"),
        Err(RocgdbCheckedGfx950ArtifactErrorV1::InputBound)
    );
    for size in [fe2o3_hsaco::MAX_HSACO_BYTES + 1, usize::MAX] {
        assert_eq!(
            require_input_bounds(size, "kernel"),
            Err(RocgdbCheckedGfx950ArtifactErrorV1::InputBound)
        );
    }
    require_input_bounds(fe2o3_hsaco::MAX_HSACO_BYTES, "kernel").unwrap();
    for name in ["", "bad\0name", "bad\nname", "bad\u{7f}name"] {
        assert_eq!(
            require_input_bounds(1, name),
            Err(RocgdbCheckedGfx950ArtifactErrorV1::InputBound)
        );
    }
    require_input_bounds(1, &"k".repeat(4096)).unwrap();
    assert_eq!(
        require_input_bounds(1, &"k".repeat(4097)),
        Err(RocgdbCheckedGfx950ArtifactErrorV1::InputBound)
    );
}

#[test]
fn arbitrary_bytes_never_become_an_inspected_code_binding() {
    for bytes in [
        b"x".as_slice(),
        b"gfx950:xnack-".as_slice(),
        b"\x7fELF".as_slice(),
    ] {
        assert_eq!(
            inspect_artifact(bytes, "kernel", 0x10000).unwrap_err(),
            RocgdbCheckedGfx950ArtifactErrorV1::ArtifactInspection
        );
    }
}

// These parsed ELF fixtures are synthetic. No checked device, target process,
// queue, real native stop or physical register is created by these tests.
#[path = "rocgdb_checked_gfx950_target_fixture_v1_tests.rs"]
mod fixture;

#[test]
fn parsed_artifact_identity_and_selected_entry_are_bound_without_a_device_owner() {
    let bytes = fixture::artifact("gfx950:xnack-");
    let value = inspect_artifact(&bytes, "vecadd", 0x10000).unwrap();
    assert_eq!(value.artifact.canonical_bytes, bytes.len() as u64);
    let expected_digest: [u8; 32] = Sha256::digest(&bytes).into();
    assert_eq!(value.artifact.digest.as_bytes(), expected_digest);
    let repeated = inspect_artifact(&bytes, "vecadd", 0x10000).unwrap();
    require_same_inspection(value.artifact, value.code, &repeated).unwrap();
    let changed_base = inspect_artifact(&bytes, "vecadd", 0x20000).unwrap();
    assert_eq!(value.artifact, changed_base.artifact);
    assert_ne!(value.code, changed_base.code);
    assert_eq!(
        require_same_inspection(value.artifact, value.code, &changed_base),
        Err(RocgdbCheckedGfx950ArtifactErrorV1::ReinspectionMismatch)
    );
    let mut changed_bytes = bytes;
    // Instruction bytes are structurally inert to the descriptor inspector,
    // but every retained byte must still affect the artifact content identity.
    changed_bytes[fixture::ENTRY_OFFSET] ^= 1;
    let changed = inspect_artifact(&changed_bytes, "vecadd", 0x10000).unwrap();
    assert_ne!(value.artifact, changed.artifact);
    assert_eq!(
        require_same_inspection(value.artifact, value.code, &changed),
        Err(RocgdbCheckedGfx950ArtifactErrorV1::ReinspectionMismatch)
    );
    // An exact entry binding alone cannot hide a substituted content length.
    // This is a synthetic inspection record, never a checked device owner.
    let altered_length = InspectedArtifact {
        artifact: LiveGpuContentIdentityV3 {
            canonical_bytes: value.artifact.canonical_bytes + 1,
            ..value.artifact
        },
        code: value.code,
    };
    assert_eq!(
        require_same_inspection(value.artifact, value.code, &altered_length),
        Err(RocgdbCheckedGfx950ArtifactErrorV1::ReinspectionMismatch)
    );
}

#[test]
fn inspected_wrong_target_missing_kernel_and_overflowing_load_base_refuse() {
    let wrong = fixture::artifact("gfx942:xnack-");
    assert_eq!(
        inspect_artifact(&wrong, "vecadd", 0x10000).unwrap_err(),
        RocgdbCheckedGfx950ArtifactErrorV1::ArtifactTarget
    );
    let bytes = fixture::artifact("gfx950:xnack-");
    assert_eq!(
        inspect_artifact(&bytes, "missing", 0x10000).unwrap_err(),
        RocgdbCheckedGfx950ArtifactErrorV1::KernelSelection
    );
    assert_eq!(
        inspect_artifact(&bytes, "vecadd", u64::MAX).unwrap_err(),
        RocgdbCheckedGfx950ArtifactErrorV1::CodeBinding
    );
}
