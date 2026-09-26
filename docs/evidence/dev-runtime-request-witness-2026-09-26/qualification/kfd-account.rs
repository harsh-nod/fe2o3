// Accounting-only fixture installed in an isolated source copy, never shipped.
impl Gfx942ComposedBackingRootV1 {
    pub fn qualification_observer_v1(&self) -> impl Fn() -> Option<ResourceCreditUsageV1> + use<> {
        let weak = Arc::downgrade(&self.0.0);
        move || {
            weak.upgrade()
                .map(|inner| Gfx942HostBackingRootV1(inner).usage_v1())
        }
    }

    pub fn qualification_request_v1(
        &self,
        uid: u64,
        generation: DeviceKeyV1,
    ) -> Gfx942RequestAccountV1 {
        self.admit(
            Identity {
                unique_id: uid,
                pci: PciAddressV1 {
                    domain: 0,
                    bus: uid as u8,
                    device: 0,
                    function: 0,
                },
            },
            generation,
            Gfx942ComposedBackingDeviceBudgetV1::new(65536, 65536, 65536, 32).unwrap(),
            Gfx942ComposedBackingSessionBudgetV1::new(
                Gfx942AllocationRequestBudgetV1::new(32768, 8).unwrap(),
                Gfx942HostVisibleBackingBudgetV1::new(32768, 8).unwrap(),
                Gfx942DeviceBackingBudgetV1::new(32768, 8).unwrap(),
                24,
            )
            .unwrap(),
        )
        .unwrap()
        .request_account_v1()
        .clone()
    }
}
