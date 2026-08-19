use std::{env, fs};

use fe2o3_hsaco_finalize::inspect_pliron_scalar_add_v1_hsaco;

#[test]
#[ignore = "requires a measured pinned-LLVM scalar HSACO path"]
fn measured_pinned_llvm_scalar_hsaco_matches_the_exact_elf_closure() {
    let path = env::var("FE2O3_TEST_PLIRON_SCALAR_ADD_HSACO")
        .expect("set FE2O3_TEST_PLIRON_SCALAR_ADD_HSACO to the measured worker output");
    let bytes = fs::read(path).expect("read measured scalar HSACO");
    let observation = inspect_pliron_scalar_add_v1_hsaco(&bytes)
        .expect("measured pinned-LLVM scalar HSACO must match the exact closure");
    assert!(!observation.grants_publication_authority());
    assert!(!observation.grants_load_authority());
    assert!(!observation.grants_launch_authority());
}
