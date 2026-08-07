use alpha_zeta_adapter_fixture::alpha_gpu;

fn duplicate(arguments: alpha_gpu::Arguments<'_>) {
    let _copy = arguments.clone();
}

fn main() {
    let _ = duplicate;
}
