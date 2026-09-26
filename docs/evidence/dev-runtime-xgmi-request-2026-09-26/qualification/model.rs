impl crate::RuntimeAllocationDeviceAdmissionV1 {
    // Reuse the existing model-only fixture. This grants no native authority.
    pub(crate) fn qualification_model_v1(
        generation: u64,
    ) -> fe2o3_runtime_model::ModelDeviceAdmissionV1 {
        super::admission(1, generation).1
    }
}
