use fe2o3_amd_target::{
    AmdTargetFeature, AmdTargetId, AsyncCopyInstructionSet, AtomicOrdering, AtomicScope,
    AtomicWidth, CapabilityDerivationError, CapabilitySupport, DeviceDiagnosticFeature, Fp8Format,
    KNOWN_PROCESSORS, LaunchBoundsField, MatrixInstructionSet, MfmaFamily, MxFormat,
    ParseAmdTargetIdError, WavefrontWidth, WorkgroupAxis,
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
fn gfx942_advanced_capabilities_are_exact_and_conservative() {
    let gfx942 = capabilities("gfx942");
    let limits = gfx942.workgroup_limits().unwrap();
    assert_eq!(limits.max_workitems(), 1024);
    for axis in [WorkgroupAxis::X, WorkgroupAxis::Y, WorkgroupAxis::Z] {
        assert_eq!(limits.max_extent(axis), 1024);
    }
    assert!(limits.supports_dimensions(1024, 1, 1));
    assert!(limits.supports_dimensions(32, 8, 4));
    assert!(!limits.supports_dimensions(0, 1, 1));
    assert!(!limits.supports_dimensions(1024, 2, 1));
    assert!(!limits.supports_dimensions(u32::MAX, u32::MAX, 1));
    assert_eq!(
        gfx942.max_wavefronts_per_workgroup(WavefrontWidth::Wave64),
        Some(16)
    );
    assert_eq!(
        gfx942.max_wavefronts_per_workgroup(WavefrontWidth::Wave32),
        None
    );

    for width in [AtomicWidth::Bits32, AtomicWidth::Bits64] {
        assert!(gfx942.standard_atomic_widths().contains(width));
    }
    for width in [
        AtomicWidth::Bits8,
        AtomicWidth::Bits16,
        AtomicWidth::Bits128,
    ] {
        assert!(!gfx942.standard_atomic_widths().contains(width));
    }
    for scope in [
        AtomicScope::Workgroup,
        AtomicScope::Device,
        AtomicScope::System,
    ] {
        assert!(gfx942.standard_atomic_scopes().contains(scope));
    }
    for ordering in [
        AtomicOrdering::Relaxed,
        AtomicOrdering::Acquire,
        AtomicOrdering::Release,
        AtomicOrdering::AcquireRelease,
        AtomicOrdering::SequentiallyConsistent,
    ] {
        assert!(gfx942.standard_atomic_orderings().contains(ordering));
    }
    assert_eq!(
        gfx942.native_split_barriers(),
        CapabilitySupport::Unsupported
    );

    for format in [Fp8Format::E4M3Fnuz, Fp8Format::E5M2Fnuz] {
        assert!(gfx942.fp8_formats().contains(format));
    }
    for format in [Fp8Format::E4M3Ocp, Fp8Format::E5M2Ocp] {
        assert!(!gfx942.fp8_formats().contains(format));
    }
    for format in [MxFormat::Fp8, MxFormat::Bf8] {
        assert!(!gfx942.mx_formats().contains(format));
    }

    for family in [
        MfmaFamily::F32FromF16,
        MfmaFamily::F32FromBf16,
        MfmaFamily::F32FromFp8Fnuz,
        MfmaFamily::F32FromBf8Fnuz,
    ] {
        assert!(gfx942.mfma_families().contains(family));
    }
    for family in [MfmaFamily::F64FromF64, MfmaFamily::I32FromI8] {
        assert!(!gfx942.mfma_families().contains(family));
    }

    assert_eq!(
        gfx942.device_diagnostic_support(DeviceDiagnosticFeature::Printf),
        CapabilitySupport::RequiresRuntimeEvidence
    );
    assert_eq!(
        gfx942.device_diagnostic_support(DeviceDiagnosticFeature::Trap),
        CapabilitySupport::Supported
    );
    assert_eq!(
        gfx942.device_diagnostic_support(DeviceDiagnosticFeature::DebugTrap),
        CapabilitySupport::RequiresRuntimeEvidence
    );
    assert_eq!(
        gfx942.device_diagnostic_support(DeviceDiagnosticFeature::ClockCounter),
        CapabilitySupport::Supported
    );
    assert_eq!(
        gfx942.device_diagnostic_support(DeviceDiagnosticFeature::ProfilingMarker),
        CapabilitySupport::Unsupported
    );
    assert_eq!(
        gfx942.launch_bounds_support(LaunchBoundsField::MaxWorkgroupSize),
        CapabilitySupport::Supported
    );
    assert_eq!(
        gfx942.launch_bounds_support(LaunchBoundsField::MinWorkgroupsPerComputeUnit),
        CapabilitySupport::Unsupported
    );
}

#[test]
fn unreviewed_advanced_capability_profiles_fail_closed() {
    for &processor in KNOWN_PROCESSORS {
        if processor == "gfx942" {
            continue;
        }
        let capabilities = capabilities(processor);
        assert!(capabilities.workgroup_limits().is_none(), "{processor}");
        assert!(
            capabilities.standard_atomic_widths().is_empty(),
            "{processor}"
        );
        assert!(
            capabilities.standard_atomic_scopes().is_empty(),
            "{processor}"
        );
        assert!(
            capabilities.standard_atomic_orderings().is_empty(),
            "{processor}"
        );
        assert_eq!(
            capabilities.native_split_barriers(),
            CapabilitySupport::Unsupported,
            "{processor}"
        );
        assert!(capabilities.fp8_formats().is_empty(), "{processor}");
        assert!(capabilities.mx_formats().is_empty(), "{processor}");
        assert!(capabilities.mfma_families().is_empty(), "{processor}");
        for feature in [
            DeviceDiagnosticFeature::Printf,
            DeviceDiagnosticFeature::Trap,
            DeviceDiagnosticFeature::DebugTrap,
            DeviceDiagnosticFeature::ClockCounter,
            DeviceDiagnosticFeature::ProfilingMarker,
        ] {
            assert_eq!(
                capabilities.device_diagnostic_support(feature),
                CapabilitySupport::Unsupported,
                "{processor}: {feature:?}"
            );
        }
        for field in [
            LaunchBoundsField::MaxWorkgroupSize,
            LaunchBoundsField::MinWorkgroupsPerComputeUnit,
        ] {
            assert_eq!(
                capabilities.launch_bounds_support(field),
                CapabilitySupport::Unsupported,
                "{processor}: {field:?}"
            );
        }
    }
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
