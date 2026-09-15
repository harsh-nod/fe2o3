use super::*;
#[path = "../../lowering/v13/reusable_phase_tests/fixture.rs"]
mod fixture;

#[test]
fn declared_phase_target_closure_queries_real_memory_and_barrier_requirements() {
    for profile in [ProductionAmdTargetProfileV1::Gfx942, ProductionAmdTargetProfileV1::Gfx950] {
        for phases in [1, 2] {
            for module in [fixture::memory(phases), fixture::terminal_drops(fixture::memory(phases))] {
            let canonical = VerifiedCanonicalKernelIrV1::from_module(module.clone(), Version::V14).unwrap();
            let launch = ProductionTargetLaunchEvidenceKirV1::for_static_launches(&canonical, 7).unwrap();
            let closure = legalize_production_target_capabilities_kir_v1(&canonical, 7, &launch, profile).unwrap();
            assert_eq!(closure.subject().version(), ProductionCanonicalGraphVersionV1::V14);
            assert_eq!(closure.subject().digest(), *canonical.identity().digest());
            assert_eq!(closure.subject().canonical_length(), canonical.identity().canonical_length());
            assert_eq!(closure.subject().epoch(), 7);
            assert_eq!(closure.launch_evidence(), &launch);
            let requirements = production_target_requirements_for_module_v1(&module).unwrap();
            assert!(requirements.iter().any(|r| matches!(r, TargetCapabilityRequirementV1::Barrier(_))));
            assert!(requirements.iter().any(|r| matches!(r, TargetCapabilityRequirementV1::Resource(_))));
            assert!(requirements.iter().all(|r| closure.decisions().iter().any(|d| d.requirement() == *r)));
            assert!(closure.decisions().iter().all(|d| d.outcome() == TargetCapabilityDecisionOutcomeV1::Supported));
            assert_eq!(ProductionTargetCapabilityClosureKirV1::decode_canonical(closure.canonical_bytes(), &launch).unwrap(), closure);
            assert!(!closure.grants_load_authority() && !closure.grants_launch_authority() && !closure.grants_publication_authority());
            }
        }
    }
}

#[test]
fn declared_phase_target_closure_rejects_epoch_graph_and_version_substitutions() {
    let module = fixture::memory(1);
    let canonical = VerifiedCanonicalKernelIrV1::from_module(module.clone(), Version::V14).unwrap();
    let launch = ProductionTargetLaunchEvidenceKirV1::for_static_launches(&canonical, 7).unwrap();
    let closure = legalize_production_target_capabilities_kir_v1(&canonical, 7, &launch, ProductionAmdTargetProfileV1::Gfx942).unwrap();
    for mutated in [
        ProductionTargetLaunchEvidenceKirV1::for_static_launches(&canonical, 8).unwrap(),
        ProductionTargetLaunchEvidenceKirV1::for_static_launches(&VerifiedCanonicalKernelIrV1::from_module(fixture::memory(2), Version::V14).unwrap(), 7).unwrap(),
        ProductionTargetLaunchEvidenceKirV1::for_static_launches(&VerifiedCanonicalKernelIrV1::from_module(fixture::legacy_modules()[0].clone(), Version::V13).unwrap(), 7).unwrap(),
    ] {
        assert!(matches!(legalize_production_target_capabilities_kir_v1(&canonical, 7, &mutated, ProductionAmdTargetProfileV1::Gfx942),
            Err(ProductionTargetCapabilityErrorV1::LaunchEvidenceSubjectMismatch)));
        assert!(matches!(ProductionTargetCapabilityClosureKirV1::decode_canonical(closure.canonical_bytes(), &mutated),
            Err(ProductionTargetCapabilityCanonicalErrorV1::LaunchEvidenceMismatch)));
    }
    assert!(matches!(closure.into_v13(), Err(ProductionTargetCapabilityErrorV1::DeclaredVersionMismatch)));
}

#[test]
fn declared_phase_old_v13_launch_closure_and_canonical_bytes_remain_exact() {
    for module in fixture::legacy_modules() {
        for profile in [ProductionAmdTargetProfileV1::Gfx942, ProductionAmdTargetProfileV1::Gfx950] {
            let old = VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
            let old_launch = ProductionTargetLaunchEvidenceV13::for_static_launches(&old, 4).unwrap();
            let old_closure = legalize_production_target_capabilities_v13(&old, 4, &old_launch, profile).unwrap();
            let common_launch = ProductionTargetLaunchEvidenceKirV1::for_static_launches(old.as_common(), 4).unwrap();
            assert_eq!(old_launch.identity(), common_launch.identity());
            assert_eq!(old_launch.subject(), common_launch.subject());
            let common_closure = legalize_production_target_capabilities_kir_v1(old.as_common(), 4, &common_launch, profile).unwrap();
            assert_eq!(old_closure.identity(), common_closure.identity());
            assert_eq!(old_closure.canonical_bytes(), common_closure.canonical_bytes());
            assert_eq!(ProductionTargetCapabilityClosureV13::decode_canonical(common_closure.canonical_bytes(), &old_launch).unwrap(), old_closure);
            assert_eq!(common_closure.into_v13().unwrap(), old_closure);
        }
    }
}

#[test]
fn declared_phase_closure_bytes_do_not_enter_strict_v13_or_ignore_trailing_bytes() {
    let canonical = VerifiedCanonicalKernelIrV1::from_module(fixture::memory(2), Version::V14).unwrap();
    let launch = ProductionTargetLaunchEvidenceKirV1::for_static_launches(&canonical, 7).unwrap();
    let closure = legalize_production_target_capabilities_kir_v1(&canonical, 7, &launch, ProductionAmdTargetProfileV1::Gfx942).unwrap();
    let legacy = VerifiedCanonicalKernelIrV13::from_module(fixture::legacy_modules()[0].clone()).unwrap();
    let old_launch = ProductionTargetLaunchEvidenceV13::for_static_launches(&legacy, 7).unwrap();
    assert!(matches!(ProductionTargetCapabilityClosureV13::decode_canonical(closure.canonical_bytes(), &old_launch),
        Err(ProductionTargetCapabilityCanonicalErrorV1::GraphVersionMismatch)));
    let mut bytes = closure.canonical_bytes().to_vec();
    bytes.push(0);
    assert!(ProductionTargetCapabilityClosureKirV1::decode_canonical(&bytes, &launch).is_err());
}
