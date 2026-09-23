//! Actual verified synthetic bundle admission, not ordinary-source or hardware qualification.
use super::*;
use fe2o3_kir_sim_cli::load_debug_simulation_bundle_v1;

const REQUEST: &[u8] = br#"{"schema":"fe2o3-simulation-request-v1","kernel":"kernel","grid":[2,1,1],"workgroup":[1,1,1],"arguments":[{"kind":"buffer","element":"u8","access":"read_only","alignment":1,"bytes":"0x2a"}]}"#;

#[test]
fn both_admitted_target_strings_survive_flattening_with_exact_bindings() {
    for target in ["gfx942:xnack-", "gfx950:xnack-"] {
        let directory = TestDirectory::new();
        let bundle_path = directory.path().join("declared.fe2sim");
        let request_path = directory.path().join("request.json");
        fs::write(&bundle_path, simulation_bundle(target)).unwrap();
        fs::write(&request_path, REQUEST).unwrap();
        let admitted = load_debug_simulation_bundle_v1(&bundle_path, &request_path).unwrap();
        let target_before = admitted
            .input()
            .retained_bundle_target_v1()
            .unwrap()
            .unwrap();
        assert_eq!(target_before.target().as_str(), admitted.bundle().target());
        assert_eq!(
            target_before.envelope_identity(),
            *admitted.bundle().identity().as_bytes()
        );
        assert_eq!(
            target_before.subject_identity(),
            *admitted.bundle().subject_identity()
        );
        let (mut input, bundle) = admitted.into_parts();
        assert_eq!(
            input.retained_bundle_target_v1().unwrap(),
            Some(target_before)
        );
        assert_eq!(
            target_before.module_identity().canonical_length(),
            bundle.canonical_kir_v7().len() as u64
        );
        let replacement = fe2o3_kir_sim_cli::load_debug_simulation_input_bytes_v1(
            &canonical_noop_with_buffer_variant(),
            REQUEST,
        )
        .unwrap();
        input.module = replacement.module;
        input.kir_sha256 = *input.module.identity().digest();
        assert!(input.retained_bundle_target_v1().is_err());
    }
}
#[test]
fn v6_keeps_its_outer_envelope_and_v11_module_identity() {
    let directory = TestDirectory::new();
    let bundle_path = directory.path().join("declared-v6.fe2sim");
    let request_path = directory.path().join("request.json");
    let bundle = simulation_bundle_v6();
    fs::write(&bundle_path, bundle.canonical_bytes()).unwrap();
    fs::write(&request_path, br#"{"schema":"fe2o3-simulation-request-v1","kernel":"bundle_v6_kernel","grid":[2,1,1],"workgroup":[1,1,1],"arguments":[]}"#).unwrap();
    let admitted =
        fe2o3_kir_sim_cli::load_debug_simulation_bundle_v6(&bundle_path, &request_path).unwrap();
    let retained = admitted
        .input()
        .retained_bundle_target_v1()
        .unwrap()
        .unwrap();
    assert_eq!(retained.target().as_str(), "gfx942:xnack-");
    assert_eq!(retained.envelope_version(), 6);
    assert_eq!(retained.envelope_identity(), *bundle.identity().as_bytes());
    assert_eq!(retained.subject_identity(), *bundle.subject_identity());
    assert_eq!(retained.module_identity().wire_version(), 11);
    assert_eq!(
        retained.module_identity().digest(),
        bundle.canonical_kir_v11_digest()
    );
    assert_eq!(
        retained.module_identity().canonical_length(),
        bundle.canonical_kir_v11_length()
    );
}
