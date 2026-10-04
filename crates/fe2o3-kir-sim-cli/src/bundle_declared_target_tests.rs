//! Private invariant controls use explicitly synthetic bundle metadata.
//! Public verified-bundle admission is covered separately by command fixture tests.
use super::*;
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV7;
use fe2o3_kir_sim::AdmittedSimulationModuleV1;

fn raw() -> AdmittedSimulationInputV1 {
    crate::load_debug_simulation_input_bytes_v1(
        include_bytes!("../tutorial/fill-v1/kernel.kir"),
        include_bytes!("../tutorial/fill-v1/request.json"),
    )
    .unwrap()
}
fn fixture() -> AdmittedSimulationInputV1 {
    let mut input = raw();
    input.simulation_bundle_identity = Some([1; 32]);
    input.simulation_bundle_subject = Some([2; 32]);
    input.simulation_bundle_evidence = Some(AdmittedSimulationBundleEvidenceV1 {
        envelope_version: 2,
        envelope_identity: [3; 32],
        subject_identity: [2; 32],
        production_kir_version: 8,
        production_kir_sha256: [4; 32],
        production_kir_bytes: 5,
        kernel_abi_identity: [6; 32],
        identity_inventory_receipt_sha256: [7; 32],
        identity_inventory_receipt_bytes: 8,
        preflight_plan_receipt_sha256: [9; 32],
        preflight_plan_receipt_bytes: 10,
    });
    input.retain_bundle_target_v1("gfx942:xnack-").unwrap();
    input
}
#[test]
fn raw_profile_never_infers_target() {
    let input = raw();
    assert_eq!(
        input.simulation_target(),
        fe2o3_kir_sim::SimulationTargetV1::amdgpu_64()
    );
    assert_eq!(input.retained_bundle_target_v1().unwrap(), None);
}
#[test]
fn exact_outer_and_inner_bindings_are_distinct() {
    let input = fixture();
    let retained = input.retained_bundle_target_v1().unwrap().unwrap();
    assert_eq!(retained.envelope_version(), 2);
    assert_eq!(retained.envelope_identity(), [3; 32]);
    assert_eq!(input.simulation_bundle_identity(), Some([1; 32]));
    assert_eq!(retained.subject_identity(), [2; 32]);
    assert_eq!(retained.module_identity(), *input.module.identity());
    assert_eq!(retained.target().as_str(), "gfx942:xnack-");
}
#[test]
fn same_version_module_substitution_refuses_even_with_matching_public_hash() {
    let mut input = fixture();
    let original = *input.module.identity();
    let mut module = input.module.module().clone();
    module.id = "different-same-version-target-fixture".into();
    input.module = AdmittedSimulationModuleV1::admit(
        VerifiedCanonicalKernelIrV7::from_module(module).unwrap(),
        input.simulation_limits,
    )
    .unwrap();
    assert_eq!(
        input.module.identity().wire_version(),
        original.wire_version()
    );
    assert_ne!(*input.module.identity(), original);
    input.kir_sha256 = *input.module.identity().digest();
    assert_eq!(
        input.retained_bundle_target_v1().unwrap_err().code,
        "bundle_target_binding_changed"
    );
}
#[test]
fn every_retained_envelope_subject_and_public_hash_substitution_refuses() {
    for change in 0..6 {
        let mut input = fixture();
        match change {
            0 => {
                input
                    .simulation_bundle_evidence
                    .as_mut()
                    .unwrap()
                    .envelope_identity = [11; 32]
            }
            1 => {
                input
                    .simulation_bundle_evidence
                    .as_mut()
                    .unwrap()
                    .subject_identity = [12; 32]
            }
            2 => input.simulation_bundle_subject = Some([13; 32]),
            3 => input.simulation_bundle_identity = Some([14; 32]),
            4 => input.kir_sha256 = [15; 32],
            5 => {
                input
                    .simulation_bundle_evidence
                    .as_mut()
                    .unwrap()
                    .envelope_version = 3
            }
            _ => unreachable!(),
        }
        assert!(
            input.retained_bundle_target_v1().is_err(),
            "change {change}"
        );
    }
}
#[test]
fn partial_metadata_unknown_target_and_rebinding_refuse() {
    let mut input = raw();
    input.simulation_bundle_subject = Some([2; 32]);
    assert!(input.retained_bundle_target_v1().is_err());
    assert!(input.retain_bundle_target_v1("gfx950:xnack-").is_err());
    let mut input = fixture();
    assert!(input.retain_bundle_target_v1("gfx950:xnack-").is_err());
    assert!(input.retain_bundle_target_v1("gfx942").is_err());
    assert_eq!(
        input
            .retained_bundle_target_v1()
            .unwrap()
            .unwrap()
            .target()
            .as_str(),
        "gfx942:xnack-"
    );
}
