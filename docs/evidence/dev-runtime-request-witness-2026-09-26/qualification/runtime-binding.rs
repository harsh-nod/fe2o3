// Descriptive binding only. No CheckedDevice or native admission is constructed.
#[cfg(test)]
impl RuntimeAllocationDeviceAdmissionV1 {
    pub(crate) fn qualification_witness_v1<'a>(
        &'a self,
        device: RuntimeDeviceIdV1,
        credit: &'a Gfx942RetainedRequestV1,
        bytes: u64,
    ) -> RuntimeAllocationRequestWitnessV1<'a> {
        RuntimeAllocationRequestWitnessV1::new(device, self, credit, bytes)
    }

    pub(crate) fn qualification_root_v1() -> fe2o3_kfd::Gfx942ComposedBackingRootV1 {
        use crate::{RuntimeResourceKindV1 as K, RuntimeResourceVectorV1 as V};
        fe2o3_kfd::Gfx942ComposedBackingRootV1::new(
            V::ZERO
                .with(K::ControlResidentBytes, 1 << 20)
                .with(K::RequestedAllocationBytes, 262144)
                .with(K::ResidentHostAllocationBytes, 262144)
                .with(K::ResidentDeviceAllocationBytes, 262144)
                .with(K::AllocationRecords, 128),
            4,
            64,
            128,
        )
        .unwrap()
    }

    pub(crate) fn qualification_entry_v1(
        root: &fe2o3_kfd::Gfx942ComposedBackingRootV1,
        key: u64,
    ) -> Self {
        let mut backend = crate::KfdRuntimeBackendV1::mock();
        let (binding, _) = RuntimeContextV1::generated_shell_test_binding_v1(&mut backend);
        Self::qualification_v1(
            key,
            binding.native_device,
            root.qualification_request_v1(key, binding.native_device.model_key()),
        )
    }

    pub(crate) fn qualification_v1(
        backend_device: u64,
        model: ModelDeviceAdmissionV1,
        account: Gfx942RequestAccountV1,
    ) -> Self {
        Self {
            backend_device,
            model,
            account,
        }
    }
}
