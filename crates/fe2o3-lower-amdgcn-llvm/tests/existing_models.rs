//! Compatibility tests against existing typed gfx942 producers.

use fe2o3_amdgcn_pliron_llvm::{ScalarKernelModuleV1, lower_scalar_kernel_v2};
use fe2o3_llvm_handoff::{IdentityV1, StageIdentitiesV1};
use fe2o3_lower_amdgcn_llvm::lower_amdgcn_to_pliron_llvm_v1;

#[test]
fn consumes_the_existing_typed_gfx942_scalar_handoff() {
    let source = ScalarKernelModuleV1::canonical(
        "existing_scalar_module",
        "existing_scalar_kernel",
        IdentityV1::new([0x31; 32]).unwrap(),
        StageIdentitiesV1::new([0x41; 32], [0x42; 32], [0x43; 32]).unwrap(),
    );
    let handoff = lower_scalar_kernel_v2(&source).expect("existing typed scalar handoff");
    let lowered = lower_amdgcn_to_pliron_llvm_v1(&handoff)
        .expect("existing scalar handoff enters bounded #145 lane");

    assert_eq!(lowered.source_identity(), handoff.identity());
    assert_eq!(
        lowered.source_handoff().encode_canonical(),
        handoff.encode_canonical()
    );
    assert!(lowered.inspect_live_graph().unwrap().strict_float());
}
