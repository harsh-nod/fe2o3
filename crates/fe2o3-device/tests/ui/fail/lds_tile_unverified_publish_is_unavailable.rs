use fe2o3_device::{Bf16, LdsTile16x16};

fn publish_without_compiler_proof(tile: LdsTile16x16<'_, Bf16>) {
    let _ = tile.assume_init();
}

fn main() {}
