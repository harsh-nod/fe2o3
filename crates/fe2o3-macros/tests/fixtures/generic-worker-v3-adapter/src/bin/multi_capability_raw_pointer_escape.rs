fn escape(
    input: gpu_host::__generated::GeneratedKfdReadSlice<'_, f32>,
) -> *const () {
    input.device_pointer()
}

fn main() {
    let _ = escape;
}
