use fe2o3_host::GeneratedDeviceScalarV1;

fn require_generated_scalar<T: GeneratedDeviceScalarV1>() {}

fn rejected() {
    require_generated_scalar::<bool>();
    require_generated_scalar::<usize>();
    require_generated_scalar::<[u32; 2]>();
}

fn main() {}
