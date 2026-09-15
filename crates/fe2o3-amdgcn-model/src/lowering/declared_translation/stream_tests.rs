use super::*;
#[path = "../v13/reusable_phase_tests/fixture.rs"]
mod fixture;

#[test]
fn replay_digest_matches_original_physical_wire_identity_both_targets() {
    for profile in [ProductionAmdTargetProfileV1::Gfx942, ProductionAmdTargetProfileV1::Gfx950] {
        let owner = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV1::from_module(fixture::memory(2), Version::V14).unwrap();
        let launch = crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
        let output = lower_verified_canonical_kir_to_amd_llvm_ir_v1(&owner, 7, &launch, profile).unwrap();
        let module = fe2o3_kernel_ir::decode_module_v14(owner.canonical_bytes()).unwrap();
        let authority = V13LoweringAuthorityV1::from_declared_closure(&module, output.capability_closure(), &owner).unwrap();
        let physical = v13::lower_declared_execution_capabilities_recorded_v1(
            &module, profile, &authority, Version::V14, ReusablePhaseCheckLimitsV1::DEFAULT, None,
        ).unwrap();
        let encoded = fe2o3_kernel_ir::encode_module_v13(&physical).unwrap();
        let record = output.declared_replay().unwrap();
        assert_eq!(record.physical_digest, <[u8; 32]>::from(Sha256::digest(&encoded)));
        assert_eq!(record.physical_length, encoded.len() as u64);
        record.replay(&owner, 7, &launch, profile, output.llvm_ir().as_bytes()).unwrap();
    }
}

#[test]
fn replay_digest_fixed_workspace_exact_boundary_and_short_preserve_work() {
    let physical = fixture::legacy_modules().into_iter().next().unwrap();
    let held = 137;
    let boundary = held + fe2o3_kernel_ir::KERNEL_IR_DIGEST_WORKSPACE_BYTES_V1;
    let mut work = ReusablePhaseCheckLimitsV1::DEFAULT.work;
    let id = physical_identity(&physical, held, boundary, &mut work).unwrap();
    assert_eq!(id.canonical_length(), fe2o3_kernel_ir::encode_module_v13(&physical).unwrap().len() as u64);
    let mut untouched = ReusablePhaseCheckLimitsV1::DEFAULT.work;
    let error = physical_identity(&physical, held, boundary - 1, &mut untouched).unwrap_err();
    assert_eq!(error.diagnostics()[0].code, LoweringDiagnosticCode::ResourceLimit);
    assert_eq!(error.diagnostics()[0].message, "declared replay digest workspace and retained trace exceed ceiling");
    assert_eq!(untouched, ReusablePhaseCheckLimitsV1::DEFAULT.work);
    let mut zero = 0;
    let error = physical_identity(&physical, held, boundary, &mut zero).unwrap_err();
    assert_eq!(error.diagnostics()[0].message, "declared replay canonical digest work ceiling");
    assert_eq!(zero, 0);
}

#[test]
fn replay_digest_residual_budget_is_cumulative_through_writer() {
    let owner = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV1::from_module(fixture::memory(2), Version::V14).unwrap();
    let launch = crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
    // Locate the exact whole-pipeline work boundary, without resetting any stage.
    let mut lo = 0;
    let mut hi = ReusablePhaseCheckLimitsV1::DEFAULT.work;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if lower_verified_canonical_kir_to_amd_llvm_ir_with_limits_v1(&owner, 7, &launch,
            ProductionAmdTargetProfileV1::Gfx942, ReusablePhaseCheckLimitsV1 {
                work: mid, ..ReusablePhaseCheckLimitsV1::DEFAULT
            }).is_ok() { hi = mid; } else { lo = mid + 1; }
    }
    let limits = ReusablePhaseCheckLimitsV1 { work: lo, ..ReusablePhaseCheckLimitsV1::DEFAULT };
    assert!(lower_verified_canonical_kir_to_amd_llvm_ir_with_limits_v1(&owner, 7, &launch,
        ProductionAmdTargetProfileV1::Gfx942, limits).is_ok());
    let error = lower_verified_canonical_kir_to_amd_llvm_ir_with_limits_v1(&owner, 7, &launch,
        ProductionAmdTargetProfileV1::Gfx942, ReusablePhaseCheckLimitsV1 { work: lo - 1, ..limits }).unwrap_err();
    let ProductionV13AmdLoweringErrorV1::Lowering(error) = error else { panic!("expected resource rejection") };
    assert_eq!(error.diagnostics()[0].code, LoweringDiagnosticCode::ResourceLimit);
    assert_eq!(error.diagnostics()[0].message, "declared replay LLVM digest work ceiling");
}

#[test]
fn replay_digest_never_relabels_logical_phase_as_physical_v13() {
    let mut remaining = ReusablePhaseCheckLimitsV1::DEFAULT.work;
    let error = physical_identity(&fixture::memory(2), 0, usize::MAX, &mut remaining).unwrap_err();
    assert_eq!(error.diagnostics()[0].message, "declared physical replay is not representable as exact physical V13");
    assert!(remaining < ReusablePhaseCheckLimitsV1::DEFAULT.work);
}

#[test]
fn replay_digest_unsupported_structured_recipe_preserves_residual_work() {
    let mut module = fixture::memory(2);
    let body = module.functions[0].body.as_mut().unwrap();
    let first = body.parameters.iter().copied().chain(body.blocks.iter().flat_map(|block| {
        block.parameters.iter().chain(block.operations.iter().flat_map(|op| op.results.iter()))
    }).map(|value| value.id)).map(|id| id.0).max().unwrap().checked_add(1).unwrap();
    let second = first.checked_add(1).unwrap();
    // Integer Not has real LLVM lowering but deliberately no structured recipe.
    body.blocks[0].operations.push(Operation::effect_free(
        fe2o3_kernel_ir::ValueDef::new(ValueId(first), Type::Scalar(ScalarType::U32)),
        OperationKind::Constant(Constant::U32(3)),
    ));
    body.blocks[0].operations.push(Operation::effect_free(
        fe2o3_kernel_ir::ValueDef::new(ValueId(second), Type::Scalar(ScalarType::U32)),
        OperationKind::Unary { op: UnaryOp::Not, operand: ValueId(first) },
    ));
    let owner = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV1::from_module(module, Version::V14).unwrap();
    let launch = crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
    let run = |work| lower_verified_canonical_kir_to_amd_llvm_ir_with_limits_v1(
        &owner, 7, &launch, ProductionAmdTargetProfileV1::Gfx942,
        ReusablePhaseCheckLimitsV1 { work, ..ReusablePhaseCheckLimitsV1::DEFAULT },
    );
    let output = run(ReusablePhaseCheckLimitsV1::DEFAULT.work).unwrap();
    assert!(output.declared_replay().unwrap().structured().is_none());
    assert!(!output.has_complete_operational_translation_derivation());
    assert!(!output.unsupported_operational_translation().is_empty());

    let mut lo = 0;
    let mut hi = ReusablePhaseCheckLimitsV1::DEFAULT.work;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if run(mid).is_ok() { hi = mid; } else { lo = mid + 1; }
    }
    assert!(lo > 0);
    let boundary = run(lo).unwrap();
    assert!(boundary.declared_replay().unwrap().structured().is_none());
    assert_eq!(boundary.llvm_ir(), output.llvm_ir());
    let error = run(lo - 1).unwrap_err();
    let ProductionV13AmdLoweringErrorV1::Lowering(error) = error else { panic!("expected resource rejection") };
    assert_eq!(error.diagnostics()[0].code, LoweringDiagnosticCode::ResourceLimit);
    // A budget reset when finish() returns None would instead move this
    // boundary back into the writer, and fail this exact stage assertion.
    assert_eq!(error.diagnostics()[0].message, "declared replay LLVM digest work ceiling");
}
