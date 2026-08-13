fn escape(
    input: gpu_host::__generated::GeneratedScalarGemmV1ReadDeviceSlice<'_>,
) -> *const () {
    input.device_pointer()
}

fn main() {
    let _ = escape;
}
