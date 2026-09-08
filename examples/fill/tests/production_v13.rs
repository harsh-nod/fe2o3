use std::path::PathBuf;

use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrV13, VerifiedSimulationBundleV8};

#[test]
#[ignore = "requires a genuine compiler-produced fill Bundle V8"]
fn compiler_bundle_is_exact_final_graph_v13() {
    let path = std::env::var_os("FE2O3_FILL_BUNDLE_V8")
        .map(PathBuf::from)
        .expect("FE2O3_FILL_BUNDLE_V8 must name compiler-produced bytes");
    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(std::fs::read(path).unwrap())
        .expect("admit Bundle V8");
    bundle.revalidate().expect("revalidate exact custody");
    assert!(bundle.final_graph_epoch() > 0);
    assert!(!bundle.grants_compiler_authority());
    assert!(!bundle.grants_hardware_authority());
    let (_, module) = VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
        bundle.canonical_kir_v13().to_vec(),
    )
    .expect("decode canonical KIR V13");
    assert!(
        module
            .kernels
            .iter()
            .any(|kernel| kernel.id.as_str() == "fill")
    );
}
