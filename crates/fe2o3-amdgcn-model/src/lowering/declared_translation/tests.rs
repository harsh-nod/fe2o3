use super::*;
#[path = "../v13/reusable_phase_tests/fixture.rs"]
mod fixture;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV1 as Canonical;

#[test]
fn declared_writer_replay_binds_full_source_census_and_exact_aliases() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for module in [
            fixture::memory(2),
            fixture::terminal_drops(fixture::memory(2)),
        ] {
            let expected: usize = module
                .functions
                .iter()
                .filter_map(|f| f.body.as_ref())
                .flat_map(|b| &b.blocks)
                .map(|b| b.operations.len())
                .sum();
            let owner = Canonical::from_module(module, Version::V14).unwrap();
            let launch =
                crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
            let output =
                lower_verified_canonical_kir_to_amd_llvm_ir_v1(&owner, 7, &launch, profile)
                    .unwrap();
            let record = output.declared_replay().unwrap();
            assert!(output.has_complete_operational_translation_derivation());
            assert!(output.unsupported_operational_translation().is_empty());
            let structured = record
                .structured()
                .expect("complete two-phase writer vocabulary");
            assert!(!structured.establishes_machine_refinement());
            assert_eq!(
                structured
                    .segments()
                    .iter()
                    .filter(|s| matches!(
                        s.kind,
                        DeclaredWriterSegmentKindV1::Operation(
                            DeclaredLlvmRecipeV1::PhysicalWorkgroupBarrier { .. }
                        )
                    ))
                    .count(),
                4
            );
            assert_eq!(
                structured
                    .segments()
                    .iter()
                    .filter(|s| matches!(
                        s.kind,
                        DeclaredWriterSegmentKindV1::Operation(
                            DeclaredLlvmRecipeV1::StaticLds { .. }
                        )
                    ))
                    .count(),
                1
            );
            assert!(
                structured
                    .segments()
                    .iter()
                    .any(|s| matches!(s.kind, DeclaredWriterSegmentKindV1::Terminator))
            );
            assert_eq!(record.projection().operations().len(), expected);
            assert!(
                record
                    .projection()
                    .results()
                    .iter()
                    .any(|r| matches!(r.carrier, DeclaredPhysicalCarrierV1::Erased))
            );
            assert!(
                record
                    .projection()
                    .results()
                    .iter()
                    .any(|r| matches!(r.carrier, DeclaredPhysicalCarrierV1::StaticView { .. }))
            );
            assert_eq!(record.version(), Version::V14);
            record
                .replay(&owner, 7, &launch, profile, output.llvm_ir().as_bytes())
                .unwrap();
            assert!(!record.authenticates_compiler_origin());
            assert!(!record.establishes_machine_refinement());
            assert!(!record.grants_runtime_authority());
        }
    }
}

#[test]
fn declared_writer_replay_rejects_substituted_bytes_profile_graph_and_record() {
    let owner = Canonical::from_module(fixture::memory(2), Version::V14).unwrap();
    let launch =
        crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    let mut output =
        lower_verified_canonical_kir_to_amd_llvm_ir_v1(&owner, 7, &launch, profile).unwrap();
    let record = output.declared_replay.as_mut().unwrap();
    let mut wrong_llvm = output.llvm_ir.as_bytes().to_vec();
    wrong_llvm[0] ^= 1;
    assert!(matches!(
        record.replay(&owner, 7, &launch, profile, &wrong_llvm),
        Err(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch)
    ));
    assert!(matches!(
        record.replay(
            &owner,
            7,
            &launch,
            ProductionAmdTargetProfileV1::Gfx950,
            output.llvm_ir.as_bytes()
        ),
        Err(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch)
    ));
    let other = Canonical::from_module(fixture::memory(1), Version::V14).unwrap();
    assert!(matches!(
        record.replay(&other, 7, &launch, profile, output.llvm_ir.as_bytes()),
        Err(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch)
    ));
    record.projection.operations[0].physical_count += 1;
    assert!(matches!(
        record.replay(&owner, 7, &launch, profile, output.llvm_ir.as_bytes()),
        Err(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch)
    ));
}

#[test]
fn declared_trace_original_ceilings_and_complete_census_remain_required() {
    let module = fixture::memory(2);
    for limits in [
        ReusablePhaseCheckLimitsV1 {
            work: 0,
            temporary_bytes: usize::MAX,
        },
        ReusablePhaseCheckLimitsV1 {
            work: usize::MAX,
            temporary_bytes: 0,
        },
    ] {
        let error = Builder::new(&module, limits).err().unwrap();
        assert_eq!(
            error.diagnostics()[0].code,
            LoweringDiagnosticCode::ResourceLimit
        );
    }
    let b = Builder::new(&module, ReusablePhaseCheckLimitsV1::DEFAULT).unwrap();
    assert!(
        b.remaining(ReusablePhaseCheckLimitsV1::DEFAULT).work
            < ReusablePhaseCheckLimitsV1::DEFAULT.work
    );
    assert!(
        b.remaining(ReusablePhaseCheckLimitsV1::DEFAULT)
            .temporary_bytes
            < ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes
    );
    assert_eq!(
        b.finish(&module).unwrap_err().diagnostics()[0].code,
        LoweringDiagnosticCode::ResourceLimit
    );
}

#[test]
fn declared_v13_facade_keeps_exact_complete_writer_output() {
    for mut module in fixture::legacy_modules() {
        // The physical-only catalog used hyphenated exported symbols. Both
        // full-writer facades receive the same newly admitted valid fixture.
        for (index, kernel) in module.kernels.iter_mut().enumerate() {
            kernel.id = KernelId::new(format!("legacy_lds_{index}"));
        }
        let owner = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let launch = ProductionTargetLaunchEvidenceV13::for_static_launches(&owner, 7).unwrap();
        let common_launch = crate::ProductionTargetLaunchEvidenceKirV1::from_v13(&launch);
        let old = lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
            &owner,
            7,
            &launch,
            ProductionAmdTargetProfileV1::Gfx942,
        )
        .unwrap();
        let common = lower_verified_canonical_kir_to_amd_llvm_ir_v1(
            owner.as_common(),
            7,
            &common_launch,
            ProductionAmdTargetProfileV1::Gfx942,
        )
        .unwrap();
        assert_eq!(old.llvm_ir(), common.llvm_ir());
        assert_eq!(
            old.capability_closure_identity(),
            common.capability_closure_identity()
        );
        assert!(common.declared_replay().is_none());
    }
}

#[test]
fn declared_writer_replay_holds_both_live_records_under_original_storage_ceiling() {
    let owner = Canonical::from_module(fixture::memory(2), Version::V14).unwrap();
    let launch =
        crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
    let output = lower_verified_canonical_kir_to_amd_llvm_ir_v1(
        &owner,
        7,
        &launch,
        ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    let record = output.declared_replay().unwrap();
    let limits = record
        .replay_limits(ReusablePhaseCheckLimitsV1::DEFAULT)
        .unwrap();
    assert!(limits.work < ReusablePhaseCheckLimitsV1::DEFAULT.work);
    assert!(matches!(
        record.replay_limits(ReusablePhaseCheckLimitsV1 {
            work: 0,
            ..ReusablePhaseCheckLimitsV1::DEFAULT
        }),
        Err(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch)
    ));
    let held = ReusablePhaseCheckLimitsV1::DEFAULT.temporary_bytes - limits.temporary_bytes;
    assert!(held > 0);
    assert!(matches!(
        record.replay_limits(ReusablePhaseCheckLimitsV1 {
            temporary_bytes: held - 1,
            ..ReusablePhaseCheckLimitsV1::DEFAULT
        }),
        Err(ProductionV13AmdLoweringErrorV1::DeclaredReplayMismatch)
    ));
    assert_eq!(
        record
            .replay_limits(ReusablePhaseCheckLimitsV1 {
                temporary_bytes: held,
                ..ReusablePhaseCheckLimitsV1::DEFAULT
            })
            .unwrap()
            .temporary_bytes,
        0
    );
    // Storage needed by the reconstructed trace is not allowed to reuse the
    // original record's still-live allocation.
    let result = lower_verified_canonical_kir_to_amd_llvm_ir_with_limits_v1(
        &owner,
        7,
        &launch,
        ProductionAmdTargetProfileV1::Gfx942,
        ReusablePhaseCheckLimitsV1 {
            temporary_bytes: 0,
            ..limits
        },
    );
    assert!(
        matches!(result, Err(ProductionV13AmdLoweringErrorV1::Lowering(ref e))
        if e.diagnostics()[0].code == LoweringDiagnosticCode::ResourceLimit)
    );
}

#[test]
fn declared_writer_replay_requires_same_epoch_launch_and_strict_symbol_boundary() {
    let owner = Canonical::from_module(fixture::memory(2), Version::V14).unwrap();
    let launch =
        crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
    let output = lower_verified_canonical_kir_to_amd_llvm_ir_v1(
        &owner,
        7,
        &launch,
        ProductionAmdTargetProfileV1::Gfx942,
    )
    .unwrap();
    assert!(matches!(
        output.declared_replay().unwrap().replay(
            &owner,
            8,
            &launch,
            ProductionAmdTargetProfileV1::Gfx942,
            output.llvm_ir().as_bytes()
        ),
        Err(ProductionV13AmdLoweringErrorV1::Capability(_))
    ));
    for module in fixture::legacy_modules() {
        let owner = VerifiedCanonicalKernelIrV13::from_module(module).unwrap();
        let launch = ProductionTargetLaunchEvidenceV13::for_static_launches(&owner, 7).unwrap();
        let result = lower_verified_canonical_kir_v13_to_amd_llvm_ir_v1(
            &owner,
            7,
            &launch,
            ProductionAmdTargetProfileV1::Gfx942,
        );
        assert!(
            matches!(result, Err(ProductionV13AmdLoweringErrorV1::Lowering(ref e))
            if e.diagnostics()[0].code == LoweringDiagnosticCode::UnsafeSymbolName),
            "{result:?}"
        );
    }
}
