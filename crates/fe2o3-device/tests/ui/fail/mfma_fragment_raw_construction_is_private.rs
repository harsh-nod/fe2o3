use fe2o3_device::{Bf16MfmaAFragment, MfmaRowMajor};

fn main() {
    let _ = Bf16MfmaAFragment::<MfmaRowMajor>::from_bits([0_u16; 4]);
}
