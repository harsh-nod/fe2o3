use super::*;

#[test]
fn device_allocator_independent_credit_limits_release_only_after_actual_disposal() {
    for byte_limit in [false, true] {
        let budget = if byte_limit {
            Gfx942DeviceBackingBudgetV1::new(12_288, 8).unwrap()
        } else {
            Gfx942DeviceBackingBudgetV1::new(16_384, 2).unwrap()
        };
        let mut fixture = Fixture::with_budget(Some(budget));
        let extra = fixture
            .memory
            .engine
            .allocate_device_memory(
                fixture.memory.device.model_key(),
                fixture.memory.vm,
                17,
                4096,
            )
            .unwrap();
        let before = counters(&fixture);
        let usage = fixture.memory.usage();
        let records: Vec<_> = fixture
            .memory
            .engine
            .device_memory
            .iter()
            .map(snapshot)
            .collect();
        let next_id = fixture.memory.engine.next_device_memory_id;
        let bytes = if byte_limit { 4097 } else { 17 };
        let mut root = Root::new();
        assert!(matches!(
            prepare(&mut fixture, &mut root, bytes, 4096),
            Err(MemorySessionError::DeviceBackingCredits(
                fe2o3_resource_accounting::ResourceCreditErrorV1::Capacity
            ))
        ));
        assert!(root.started && root.failed && !root.native_started);
        assert!(matches!(root.lease, Lease::None));
        assert_eq!(root.progress, Default::default());
        root.retain_live_failure(&mut fixture.memory.engine);
        assert_prefix(&fixture, before, [0; 5]);
        assert_eq!(fixture.memory.usage(), usage);
        assert_eq!(
            fixture
                .memory
                .engine
                .device_memory
                .iter()
                .map(snapshot)
                .collect::<Vec<_>>(),
            records
        );
        assert_eq!(fixture.memory.engine.next_device_memory_id, next_id);
        assert_eq!(
            fixture.memory.engine.phase(),
            SharedMemorySessionPhaseV1::Active
        );
        assert!(
            fixture
                .memory
                .engine
                .terminal_device_initialization
                .is_none()
        );
        fixture.assert_anchor();
        fixture.memory.engine.release_device_memory(extra).unwrap();
        assert_usage(&fixture, 4096, 1, 0);
        let mut retry = Root::new();
        prepare(&mut fixture, &mut retry, bytes, 4096).unwrap();
        let output = retry.take_complete().unwrap();
        assert_usage(&fixture, 4096 + output.layout.backing_bytes, 2, 0);
        assert_eq!(
            snapshot(&fixture.memory.engine.device_memory[0]),
            fixture.anchor_native
        );
        assert_eq!(fixture.memory.engine.backend.free_calls, 1);
        assert_eq!(fixture.memory.engine.backend.release_va_calls, 1);
        assert_eq!(fixture.memory.engine.backend.unmap_gpu_calls, 0);
    }
}
