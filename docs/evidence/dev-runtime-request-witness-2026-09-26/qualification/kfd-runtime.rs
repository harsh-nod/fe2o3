#[cfg(test)]
impl KfdRuntimeBackendV1 {
    pub(crate) fn qualification_install_binding_v1(
        &mut self,
        binding: crate::RuntimeAllocationDeviceAdmissionV1,
    ) {
        assert!(!self.native_available);
        assert!(self.admitted_device.is_none());
        assert!(self.allocations.is_empty());
        self.composed_request_binding = Some(binding);
        self.rooted_backing = Some(native_budget::RootedBackingV1::Composed(None));
    }

    pub(crate) fn qualification_composed_v1(
        binding: crate::RuntimeAllocationDeviceAdmissionV1,
    ) -> Self {
        let mut backend = Self::mock();
        backend.composed_request_binding = Some(binding);
        backend.rooted_backing = Some(native_budget::RootedBackingV1::Composed(None));
        assert!(!backend.native_available);
        assert!(backend.admitted_device.is_none());
        backend
    }

    pub(crate) fn qualification_policy_mismatch_v1(&mut self, policy: u8) {
        self.rooted_backing = match policy {
            0 => None,
            1 => Some(native_budget::RootedBackingV1::Host(None)),
            2 => Some(native_budget::RootedBackingV1::Native(None)),
            _ => unreachable!(),
        };
        let next = self.next_handle;
        assert!(matches!(
            self.take_rooted_backing_v1(),
            Err(RuntimeBackendFailureV1::Terminal(_))
        ));
        assert!(self.terminal);
        assert_eq!(self.next_handle, next);
        assert!(self.allocations.is_empty());
        assert!(self.queue.is_none());
        assert!(self.terminal_memory.is_none());
    }
}
