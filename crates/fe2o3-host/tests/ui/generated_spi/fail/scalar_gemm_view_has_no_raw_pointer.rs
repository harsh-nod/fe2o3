use fe2o3_host::GeneratedScalarGemmV1ReadWriteDeviceSlice;

fn rejected(capability: GeneratedScalarGemmV1ReadWriteDeviceSlice<'_>) {
    let _raw = capability.as_device_ptr();
}

fn main() {}
