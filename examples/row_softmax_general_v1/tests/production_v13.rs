use std::path::PathBuf;

use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrV13, VerifiedSimulationBundleV8};

#[test]
#[ignore = "requires a genuine compiler-produced row-softmax Bundle V8"]
fn compiler_bundle_is_exact_final_graph_v13() {
    let path = std::env::var_os("FE2O3_ROW_SOFTMAX_BUNDLE_V8")
        .map(PathBuf::from)
        .expect("FE2O3_ROW_SOFTMAX_BUNDLE_V8 must name compiler-produced bytes");
    let bundle = VerifiedSimulationBundleV8::from_canonical_bytes(std::fs::read(path).unwrap())
        .expect("admit Bundle V8");
    bundle.revalidate().expect("revalidate exact custody");
    assert!(bundle.final_graph_epoch() > 0);
    assert!(!bundle.grants_compiler_authority());
    assert!(!bundle.grants_proof_authority());
    let (_, module) = VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
        bundle.canonical_kir_v13().to_vec(),
    )
    .expect("decode canonical KIR V13");
    let kernel = module
        .kernels
        .iter()
        .find(|kernel| kernel.id.as_str() == "row_softmax_general_v1")
        .expect("exact kernel root");
    assert_eq!(
        kernel.workgroup_size.map(|size| (size.x, size.y, size.z)),
        Some((64, 1, 1))
    );
}
