use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrVersionV1 as Version, VerifiedCanonicalKernelIrV1 as Canonical, ReusablePhaseCheckLimitsV1};
#[path = "v13/reusable_phase_tests/fixture.rs"]
mod fixture;

#[test]
fn declared_phase_complete_writer_keeps_exact_lds_barriers_and_target_identity() {
    for profile in [ProductionAmdTargetProfileV1::Gfx942, ProductionAmdTargetProfileV1::Gfx950] {
        for module in [fixture::memory(2), fixture::terminal_drops(fixture::memory(2))] {
            let owner = Canonical::from_module(module, Version::V14).unwrap();
            let launch = crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&owner, 7).unwrap();
            let output = lower_verified_canonical_kir_to_amd_llvm_ir_v1(&owner, 7, &launch, profile).unwrap();
            assert!(output.llvm_ir().contains("addrspace(3)"));
            assert_eq!(output.llvm_ir().matches("call void asm sideeffect \"s_barrier\", \"\"()").count(), 4);
            assert!(output.llvm_ir().contains("store float"));
            assert!(output.llvm_ir().contains("load float"));
            assert_eq!(output.capability_closure().subject(), launch.subject());
            assert!(!output.grants_load_authority());
            assert!(!output.grants_launch_authority());
            let again = lower_verified_canonical_kir_to_amd_llvm_ir_v1(&owner, 7, &launch, profile).unwrap();
            assert_eq!(output.llvm_ir(), again.llvm_ir());
        }
    }
}

#[test]
fn declared_phase_physical_entry_consumes_real_exact_target_closure() {
    for profile in [ProductionAmdTargetProfileV1::Gfx942, ProductionAmdTargetProfileV1::Gfx950] {
        for phases in [1, 2] {
            for module in [fixture::memory(phases), fixture::terminal_drops(fixture::memory(phases))] {
            let canonical = Canonical::from_module(module.clone(), Version::V14).unwrap();
            let before = canonical.canonical_bytes().to_vec();
            let launch = crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&canonical, 7).unwrap();
            let closure = crate::legalize_production_target_capabilities_kir_v1(&canonical, 7, &launch, profile).unwrap();
            let authority = V13LoweringAuthorityV1::from_declared_closure(&module, &closure, &canonical).unwrap();
            let physical = v13::lower_declared_execution_capabilities_v1(&module, profile, &authority, Version::V14, ReusablePhaseCheckLimitsV1::DEFAULT).unwrap();
            fe2o3_kernel_ir::verify_module(&physical).unwrap();
            let ops = &physical.functions[0].body.as_ref().unwrap().blocks[0].operations;
            assert_eq!(ops.iter().filter(|o| matches!(o.kind, OperationKind::WorkgroupMemory(_))).count(), 1);
            assert_eq!(ops.iter().filter(|o| matches!(o.kind, OperationKind::WorkgroupBarrier(_))).count(), 2 * phases);
            assert_eq!(ops.iter().filter(|o| matches!(o.kind, OperationKind::Store { .. })).count(), phases);
            assert_eq!(ops.iter().filter(|o| matches!(o.kind, OperationKind::GuardedLoad { .. })).count(), phases);
            assert!(ops.iter().all(|o| !matches!(o.kind, OperationKind::ReusablePhase(_) | OperationKind::ExecutionCapability(_))));
            assert_eq!(canonical.canonical_bytes(), before);
            assert_eq!(authority.identity(), closure.identity());
            }
        }
    }
}

#[test]
fn declared_phase_physical_entry_keeps_old_strict_v13_and_new_graph_identity_separate() {
    let module = fixture::memory(1);
    let canonical = Canonical::from_module(module.clone(), Version::V14).unwrap();
    let launch = crate::ProductionTargetLaunchEvidenceKirV1::for_static_launches(&canonical, 7).unwrap();
    let closure = crate::legalize_production_target_capabilities_kir_v1(&canonical, 7, &launch, ProductionAmdTargetProfileV1::Gfx942).unwrap();
    let authority = V13LoweringAuthorityV1::from_declared_closure(&module, &closure, &canonical).unwrap();
    assert!(v13::lower_execution_capabilities_v1(&module, ProductionAmdTargetProfileV1::Gfx942, &authority).is_err());
    let different = Canonical::from_module(fixture::memory(2), Version::V14).unwrap();
    let error = V13LoweringAuthorityV1::from_declared_closure(&module, &closure, &different).err().unwrap();
    assert_eq!(error.diagnostics()[0].code, LoweringDiagnosticCode::CapabilityClosureMismatch);
}

#[test]
fn declared_phase_physical_v13_legacy_graphs_match_old_facade_exactly() {
    for module in fixture::legacy_modules() {
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let launch = ProductionTargetLaunchEvidenceV13::for_static_launches(&canonical, 3).unwrap();
        let closure = crate::legalize_production_target_capabilities_v13(&canonical, 3, &launch, ProductionAmdTargetProfileV1::Gfx942).unwrap();
        let old_authority = V13LoweringAuthorityV1::from_closure(&module, &closure).unwrap();
        let common_launch = crate::ProductionTargetLaunchEvidenceKirV1::from_v13(&launch);
        let common_closure = crate::legalize_production_target_capabilities_kir_v1(canonical.as_common(), 3, &common_launch, ProductionAmdTargetProfileV1::Gfx942).unwrap();
        let authority = V13LoweringAuthorityV1::from_declared_closure(&module, &common_closure, canonical.as_common()).unwrap();
        let old = v13::lower_execution_capabilities_v1(&module, ProductionAmdTargetProfileV1::Gfx942, &old_authority).unwrap();
        let common = v13::lower_declared_execution_capabilities_v1(&module, ProductionAmdTargetProfileV1::Gfx942, &authority, Version::V13, ReusablePhaseCheckLimitsV1::DEFAULT).unwrap();
        assert_eq!(old, common);
        assert_eq!(old_authority.identity(), authority.identity());
    }
}
