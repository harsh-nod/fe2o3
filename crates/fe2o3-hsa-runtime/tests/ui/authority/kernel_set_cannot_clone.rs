use fe2o3_hsa_runtime::ReviewedHsaKernelSetV1;

fn require_clone<T: Clone>() {}

fn assert_linear<'executable>() {
    require_clone::<ReviewedHsaKernelSetV1<'executable, 2>>();
}

fn main() {}
