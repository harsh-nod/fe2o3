use std::path::PathBuf;

use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrV13, VerifiedSimulationBundleV8};
use fe2o3_wave64_collectives_v1::{
    WAVE64_LANES_V1, WAVE64_REFINEMENT_BOUNDARY_V13, Wave64SemanticOutputV1,
    bind_wave64_refinement_identities_v13, source_contributor_mask_v1,
    verify_wave64_source_model_to_kir_v13,
};

fn corpus() -> [f32; WAVE64_LANES_V1] {
    core::array::from_fn(|lane| ((lane * 37 + 11) % 127) as f32 - 63.0)
}

fn prefix_mask(end: usize) -> u64 {
    match end {
        0 => 0,
        WAVE64_LANES_V1.. => u64::MAX,
        _ => (1_u64 << end) - 1,
    }
}

fn compiler_produced_v13() -> VerifiedCanonicalKernelIrV13 {
    let path = std::env::var_os("FE2O3_M2_WAVE64_BUNDLE_V8")
        .map(PathBuf::from)
        .expect("FE2O3_M2_WAVE64_BUNDLE_V8 must name a compiler-produced Bundle V8");
    let bytes = std::fs::read(path).expect("read compiler-produced M2 Bundle V8");
    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(bytes)
        .expect("admit exact compiler-produced Bundle V8");
    bundle.revalidate().expect("revalidate Bundle V8 custody");
    let expected_target = std::env::var("FE2O3_M2_EXPECTED_TARGET")
        .expect("FE2O3_M2_EXPECTED_TARGET must name the exact bound target");
    assert_eq!(bundle.target(), expected_target);
    assert!(bundle.final_graph_epoch() > 0);
    assert!(!bundle.grants_compiler_authority());
    assert!(!bundle.grants_proof_authority());
    assert!(!bundle.grants_hardware_authority());
    VerifiedCanonicalKernelIrV13::from_canonical_bytes(bundle.canonical_kir_v13().to_vec())
        .expect("admit exact canonical KIR V13 from Bundle V8")
}

#[test]
fn symbolic_contributor_sets_cover_every_lane_and_every_mask() {
    for lane in 0..WAVE64_LANES_V1 {
        assert_eq!(
            source_contributor_mask_v1(Wave64SemanticOutputV1::Reduction, lane),
            u64::MAX
        );
        assert_eq!(
            source_contributor_mask_v1(Wave64SemanticOutputV1::Inclusive, lane),
            prefix_mask(lane + 1)
        );
        assert_eq!(
            source_contributor_mask_v1(Wave64SemanticOutputV1::Exclusive, lane),
            prefix_mask(lane)
        );
    }
    for output in [
        Wave64SemanticOutputV1::Reduction,
        Wave64SemanticOutputV1::Inclusive,
        Wave64SemanticOutputV1::Exclusive,
    ] {
        assert_eq!(source_contributor_mask_v1(output, 64), 0);
        assert_eq!(source_contributor_mask_v1(output, usize::MAX), 0);
    }
}

#[test]
fn documented_boundary_does_not_overclaim_v13_authority() {
    for boundary in [
        "verified canonical KIR V13 bytes",
        "target-neutral",
        "unresolved proof obligations are not discharged",
        "no source-to-KIR proof",
        "no compiler causality",
        "no LLVM/ISA refinement",
        "no artifact",
        "protected-execution",
        "generalized-safety",
        "parity authority",
    ] {
        assert!(WAVE64_REFINEMENT_BOUNDARY_V13.contains(boundary));
    }
}

#[test]
#[ignore = "requires a genuine compiler-produced M2 Bundle V8; never substitute a synthetic receipt"]
fn compiler_produced_v13_matches_the_source_model_corpus() {
    let canonical = compiler_produced_v13();
    let identities = bind_wave64_refinement_identities_v13(&canonical);
    let input = corpus();
    for mask in [
        0,
        1,
        1_u64 << 63,
        0xaaaa_aaaa_aaaa_aaaa,
        0x8000_0042_8000_0021,
        u64::MAX,
    ] {
        let receipt =
            verify_wave64_source_model_to_kir_v13(&input, mask, &canonical, identities).unwrap();
        assert_eq!(receipt.identities(), identities);
        assert_eq!(receipt.active_mask(), mask);
        assert_eq!(receipt.active_lanes(), mask.count_ones());
        assert_eq!(receipt.checked_symbolic_relations(), 3 * 64);
        assert!(!receipt.proves_source_to_kir_refinement());
        assert!(!receipt.proves_compiler_causality());
        assert!(!receipt.discharges_proof_obligations());
        assert!(!receipt.grants_protected_execution());
    }
}

#[test]
#[ignore = "requires a genuine compiler-produced M2 Bundle V8 identity"]
fn canonical_v13_identity_substitution_fails_closed() {
    let canonical = compiler_produced_v13();
    let mut identities = bind_wave64_refinement_identities_v13(&canonical);
    identities.canonical_kir_v13_identity[0] ^= 1;
    assert!(
        verify_wave64_source_model_to_kir_v13(&corpus(), u64::MAX, &canonical, identities).is_err()
    );
}
