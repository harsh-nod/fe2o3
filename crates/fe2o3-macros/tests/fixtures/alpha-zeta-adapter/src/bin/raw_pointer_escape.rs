use alpha_zeta_adapter_fixture::alpha_gpu;

fn escape(arguments: alpha_gpu::Arguments<'_>) -> *const () {
    arguments.input.device_pointer()
}

fn main() {
    let _ = escape;
}
