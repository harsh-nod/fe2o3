#[path = "../../../tests/support/compiler_proof_inputs_v3.rs"]
mod compiler_proof_inputs_v3;

use compiler_proof_inputs_v3::{
    CanonicalCompilerProofInputsV3, ProductionSourceIsaKernelFamilyV1,
    canonical_compiler_proof_inputs_v4, canonical_compiler_proof_inputs_v4_with_sourceful_family,
};
use sha2::{Digest, Sha256};
use std::{fs, io::Write, path::Path};

fn capture(root: &Path, name: &str, inputs: CanonicalCompilerProofInputsV3) {
    let directory = root.join(name);
    fs::create_dir(&directory).unwrap();
    for (kind, bytes) in [
        ("semantic-mir", inputs.semantic_mir()),
        ("middle-end", inputs.middle_end()),
        ("kernel-ir", inputs.kernel_ir()),
        ("correspondence", inputs.correspondence()),
        ("formal-memory", inputs.formal_memory()),
    ] {
        let relative = format!("{name}/{kind}.bin");
        let mut file = fs::File::create_new(root.join(&relative)).unwrap();
        file.write_all(bytes).unwrap();
        let digest = Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        println!("{digest}  {relative}");
    }
}

#[test]
fn capture_historical_v4_fixtures() {
    let root = std::env::var_os("FE2O3_FROZEN_CAPTURE_DIRECTORY").unwrap();
    let root = Path::new(&root);
    fs::create_dir(root).unwrap();
    for seed in [0x20, 0x40] {
        capture(
            root,
            &format!("noop-{seed:02x}"),
            canonical_compiler_proof_inputs_v4(seed),
        );
    }
    for (name, seed, family) in [
        (
            "elementwise-20",
            0x20,
            ProductionSourceIsaKernelFamilyV1::Elementwise,
        ),
        (
            "elementwise-21",
            0x21,
            ProductionSourceIsaKernelFamilyV1::Elementwise,
        ),
        (
            "elementwise-40",
            0x40,
            ProductionSourceIsaKernelFamilyV1::Elementwise,
        ),
        (
            "collective-20",
            0x20,
            ProductionSourceIsaKernelFamilyV1::WorkgroupCollective,
        ),
        ("tiled-20", 0x20, ProductionSourceIsaKernelFamilyV1::Tiled),
    ] {
        capture(
            root,
            name,
            canonical_compiler_proof_inputs_v4_with_sourceful_family(seed, family),
        );
    }
}
