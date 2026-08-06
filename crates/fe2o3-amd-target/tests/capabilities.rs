use fe2o3_amd_target::{
    AmdTargetFeature, AmdTargetId, AsyncCopyInstructionSet, AtomicScope, CapabilityDerivationError,
    CapabilitySupport, KNOWN_PROCESSORS, MatrixInstructionSet, ParseAmdTargetIdError,
    WavefrontWidth,
};

#[test]
fn every_accepted_processor_has_a_canonical_capability_profile() {
    for &processor in KNOWN_PROCESSORS {
        let target = AmdTargetId::parse(processor).unwrap();
        let capabilities = target.capabilities().unwrap();
        assert_eq!(capabilities.target(), target, "{processor}");
        assert!(
            capabilities
                .wavefront_widths()
                .contains(capabilities.default_wavefront_width()),
            "{processor}"
        );
        assert!(
            capabilities.max_lds_bytes_per_workgroup() > 0,
            "{processor}"
        );
        for scope in [
            AtomicScope::Workgroup,
            AtomicScope::Device,
            AtomicScope::System,
        ] {
            assert!(capabilities.atomic_scopes().contains(scope), "{processor}");
        }
        assert_eq!(
            capabilities.cooperative_launch(),
            CapabilitySupport::RequiresRuntimeEvidence,
            "{processor}"
        );
    }
}

#[test]
fn representative_wave_and_lds_profiles_match_llvm_subtarget_facts() {
    let gfx600 = capabilities("gfx600");
    assert_eq!(gfx600.default_wavefront_width(), WavefrontWidth::Wave64);
    assert!(!gfx600.wavefront_widths().contains(WavefrontWidth::Wave32));
    assert!(gfx600.wavefront_widths().contains(WavefrontWidth::Wave64));
    assert_eq!(gfx600.max_lds_bytes_per_workgroup(), 32 * 1024);

    let gfx942 = capabilities("gfx942");
    assert_eq!(gfx942.default_wavefront_width(), WavefrontWidth::Wave64);
    assert!(!gfx942.wavefront_widths().contains(WavefrontWidth::Wave32));
    assert_eq!(gfx942.max_lds_bytes_per_workgroup(), 64 * 1024);

    let gfx1151 = capabilities("gfx1151");
    assert_eq!(gfx1151.default_wavefront_width(), WavefrontWidth::Wave32);
    assert!(gfx1151.wavefront_widths().contains(WavefrontWidth::Wave32));
    assert!(gfx1151.wavefront_widths().contains(WavefrontWidth::Wave64));
    assert_eq!(gfx1151.max_lds_bytes_per_workgroup(), 64 * 1024);

    let gfx950 = capabilities("gfx950");
    assert_eq!(gfx950.max_lds_bytes_per_workgroup(), 160 * 1024);

    let gfx1250 = capabilities("gfx1250");
    assert_eq!(gfx1250.default_wavefront_width(), WavefrontWidth::Wave32);
    assert!(gfx1250.wavefront_widths().contains(WavefrontWidth::Wave32));
    assert!(!gfx1250.wavefront_widths().contains(WavefrontWidth::Wave64));
    assert_eq!(gfx1250.max_lds_bytes_per_workgroup(), 320 * 1024);

    let gfx1310 = capabilities("gfx1310");
    assert!(gfx1310.wavefront_widths().contains(WavefrontWidth::Wave32));
    assert!(gfx1310.wavefront_widths().contains(WavefrontWidth::Wave64));
}

#[test]
fn representative_matrix_families_are_exact_and_fail_closed() {
    let gfx942 = capabilities("gfx942").matrix_instruction_sets();
    assert!(gfx942.contains(MatrixInstructionSet::Mfma));
    assert!(!gfx942.contains(MatrixInstructionSet::Wmma128));

    let gfx1151 = capabilities("gfx1151").matrix_instruction_sets();
    assert!(gfx1151.contains(MatrixInstructionSet::Wmma256));
    assert!(!gfx1151.contains(MatrixInstructionSet::Mfma));

    let gfx1170 = capabilities("gfx1170").matrix_instruction_sets();
    assert!(gfx1170.contains(MatrixInstructionSet::Wmma128));
    assert!(gfx1170.contains(MatrixInstructionSet::Swmma));
    assert!(!gfx1170.contains(MatrixInstructionSet::Wmma256));

    let gfx1251 = capabilities("gfx1251").matrix_instruction_sets();
    assert!(gfx1251.contains(MatrixInstructionSet::Swmma));
    assert!(!gfx1251.contains(MatrixInstructionSet::Wmma128));

    assert!(capabilities("gfx906").matrix_instruction_sets().is_empty());
    assert!(capabilities("gfx1310").matrix_instruction_sets().is_empty());
}

#[test]
fn representative_async_families_are_exact_and_fail_closed() {
    let gfx942 = capabilities("gfx942").async_copy_instruction_sets();
    assert!(gfx942.contains(AsyncCopyInstructionSet::VmemToLds));
    assert!(!gfx942.contains(AsyncCopyInstructionSet::AsyncLoadToLds));

    assert!(
        capabilities("gfx1151")
            .async_copy_instruction_sets()
            .is_empty()
    );

    let gfx1250 = capabilities("gfx1250").async_copy_instruction_sets();
    assert!(gfx1250.contains(AsyncCopyInstructionSet::AsyncLoadToLds));
    assert!(gfx1250.contains(AsyncCopyInstructionSet::AsyncStoreFromLds));
    assert!(!gfx1250.contains(AsyncCopyInstructionSet::VmemToLds));

    let gfx1310 = capabilities("gfx1310").async_copy_instruction_sets();
    assert!(gfx1310.contains(AsyncCopyInstructionSet::AsyncLoadToLds));
    assert!(!gfx1310.contains(AsyncCopyInstructionSet::AsyncStoreFromLds));
}

#[test]
fn explicit_target_id_features_are_bound_into_the_canonical_encoding() {
    let target = AmdTargetId::parse("gfx942:xnack-:sramecc+").unwrap();
    let capabilities = target.capabilities().unwrap();
    let expected = "amd-target-capabilities-v1{target=gfx942:sramecc+:xnack-;default-wave=wave64;waves=[wave64];max-lds-per-workgroup=65536;atomic-scopes=[workgroup,device,system];cooperative-launch=runtime-evidence;matrix=[mfma];async-copy=[vmem-to-lds]}";
    assert_eq!(capabilities.to_string(), expected);

    let mut encoded = String::new();
    capabilities.encode_canonical(&mut encoded).unwrap();
    assert_eq!(encoded, expected);
}

#[test]
fn unknown_unsupported_and_contradictory_features_never_reach_derivation() {
    assert_eq!(
        AmdTargetId::parse("gfx999"),
        Err(ParseAmdTargetIdError::UnknownProcessor)
    );
    assert_eq!(
        AmdTargetId::parse("gfx1151:xnack+"),
        Err(ParseAmdTargetIdError::UnsupportedFeature(
            AmdTargetFeature::Xnack
        ))
    );
    assert_eq!(
        AmdTargetId::parse("gfx942:xnack+:xnack-"),
        Err(ParseAmdTargetIdError::DuplicateFeature(
            AmdTargetFeature::Xnack
        ))
    );
    assert_eq!(
        AmdTargetId::parse("gfx942:wavefrontsize32+"),
        Err(ParseAmdTargetIdError::UnknownFeature)
    );
}

#[test]
fn capability_derivation_error_text_is_stable() {
    assert_eq!(
        CapabilityDerivationError::UnknownProcessor.to_string(),
        "no canonical capabilities exist for this processor"
    );
    assert_eq!(
        CapabilityDerivationError::ContradictoryWavefrontProfile.to_string(),
        "processor has a contradictory wavefront profile"
    );
}

fn capabilities(target: &str) -> fe2o3_amd_target::AmdTargetCapabilities {
    AmdTargetId::parse(target).unwrap().capabilities().unwrap()
}
