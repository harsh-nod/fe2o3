//! CPU custody tests; the fake backend is not native execution evidence.
use super::*;
use crate::shared_memory::device_allocation::{
    AllocationLeaseV1 as Lease, DeviceAllocationCustodyV1 as Root,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct AllocationSnapshot {
    lease: Option<(LeaseIdentity, bool)>,
    flags: u32,
    started: bool,
    failed: bool,
    native_started: bool,
    progress: crate::shared_memory::transitions::NativeTransitionProgressV1,
}

pub(in crate::shared_memory) fn allocation_snapshot(root: &Root) -> AllocationSnapshot {
    let lease = match &root.lease {
        Lease::None => None,
        Lease::Unmapped(lease) => Some((
            (
                lease.id,
                lease.generation,
                lease.device,
                lease.vm,
                lease.layout,
            ),
            false,
        )),
        Lease::Mapped(lease) => Some((
            (
                lease.id,
                lease.generation,
                lease.device,
                lease.vm,
                lease.layout,
            ),
            true,
        )),
    };
    AllocationSnapshot {
        lease,
        flags: root.flags.bits(),
        started: root.started,
        failed: root.failed,
        native_started: root.native_started,
        progress: root.progress,
    }
}

impl AllocationSnapshot {
    pub(crate) fn with_failure_for_test(&self) -> Self {
        let mut result = self.clone();
        result.failed = true;
        result
    }

    pub(crate) fn started(&self) -> bool {
        self.started
    }
    pub(crate) fn failed(&self) -> bool {
        self.failed
    }
    pub(crate) fn native_started(&self) -> bool {
        self.native_started
    }
    pub(crate) fn progress(&self) -> (bool, Option<bool>, Option<u32>) {
        (
            self.progress.attempted,
            self.progress.returned_success,
            self.progress.returned_map_prefix,
        )
    }
    pub(crate) fn lease(
        &self,
    ) -> Option<(
        Gfx942DeviceMemoryIdentityV1,
        Gfx942DeviceMemoryLayoutV1,
        bool,
    )> {
        self.lease
            .map(|((id, generation, device, vm, layout), mapped)| {
                (
                    Gfx942DeviceMemoryIdentityV1 {
                        id,
                        generation,
                        device,
                        vm,
                    },
                    layout,
                    mapped,
                )
            })
    }
}

fn prepare(
    fixture: &mut Fixture,
    root: &mut Root,
    bytes: u64,
    alignment: u64,
) -> Result<(), MemorySessionError> {
    root.prepare_in_place(
        &mut fixture.memory.engine,
        fixture.memory.device.model_key(),
        fixture.memory.vm,
        bytes,
        alignment,
    )
}

fn counters(fixture: &Fixture) -> [usize; 5] {
    let (currentness, reserve, alloc, cpu, gpu, _) = fixture.calls();
    [currentness, reserve, alloc, cpu, gpu]
}

fn assert_prefix(fixture: &Fixture, before: [usize; 5], delta: [usize; 5]) {
    assert_eq!(
        counters(fixture),
        std::array::from_fn(|i| before[i] + delta[i])
    );
    let backend = &fixture.memory.engine.backend;
    assert_eq!(backend.map_cpu_inputs, []);
    assert_eq!(backend.last_unmapped_bytes, None);
    assert_eq!(backend.last_unmapped_readback_calls, 0);
    assert!(
        fixture
            .memory
            .engine
            .device_memory
            .iter()
            .all(|r| r.mapping.is_none())
    );
    assert!(backend.operations.iter().all(|op| *op == "map_gpu"));
}

fn assert_usage(fixture: &Fixture, bytes: u64, records: u64, quarantined: usize) {
    fixture.assert_usage(bytes, records, quarantined);
    if fixture.account.is_none() {
        assert_eq!(fixture.memory.usage(), None);
        assert!(fixture.memory.engine.device_backing_account.is_none());
        assert!(
            fixture
                .memory
                .engine
                .device_memory
                .iter()
                .all(|r| r.backing_charge.is_none())
        );
    }
}

fn assert_actual_lease(fixture: &Fixture, root: &Root, mapped: bool, bytes: u64) {
    let state = allocation_snapshot(root);
    let expected = (
        fixture.next_id,
        1,
        fixture.memory.device.model_key(),
        fixture.memory.vm,
        device_memory_layout(bytes, 4096, root.flags).unwrap(),
    );
    assert_eq!(state.lease, Some((expected, mapped)));
    assert_eq!(
        snapshot(&fixture.memory.engine.device_memory[1]).identity,
        expected
    );
}

fn clear_faults(fixture: &mut Fixture) {
    let backend = &mut fixture.memory.engine.backend;
    backend.fail_operation = None;
    backend.panic_operation = None;
    backend.fail_currentness_at = None;
    backend.panic_currentness_at = None;
    backend.map_errno = false;
    backend.map_progress = 1;
    backend.alloc_oom = false;
    backend.corrupt_flags = false;
    backend.allocation_output_mutator = None;
    backend.fixed_va = None;
}

fn no_retry(fixture: &mut Fixture) {
    let engine = &fixture.memory.engine;
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    let records: Vec<_> = engine.device_memory.iter().map(snapshot).collect();
    let calls = fixture.calls();
    let usage = fixture.memory.usage();
    let next_id = engine.next_device_memory_id;
    let initialized = engine
        .terminal_device_initialization
        .as_ref()
        .map(root_snapshot);
    let allocation = engine
        .terminal_device_initialization
        .allocation_as_ref()
        .map(allocation_snapshot);
    clear_faults(fixture);
    let mut retry = Root::new();
    assert!(matches!(
        prepare(fixture, &mut retry, 17, 4096),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert!(!retry.requires_retention());
    retry.retain_live_failure(&mut fixture.memory.engine);
    assert_eq!(fixture.calls(), calls);
    assert_eq!(fixture.memory.usage(), usage);
    let engine = &fixture.memory.engine;
    assert_eq!(engine.next_device_memory_id, next_id);
    assert_eq!(
        engine
            .device_memory
            .iter()
            .map(snapshot)
            .collect::<Vec<_>>(),
        records
    );
    assert_eq!(
        engine
            .terminal_device_initialization
            .as_ref()
            .map(root_snapshot),
        initialized
    );
    assert_eq!(
        engine
            .terminal_device_initialization
            .allocation_as_ref()
            .map(allocation_snapshot),
        allocation
    );
    fixture.assert_anchor();
}

fn retain_failure(fixture: &mut Fixture, mut root: Root) {
    let before = allocation_snapshot(&root);
    // Retention must not repair missing quarantine at the preparation boundary.
    no_retry(fixture);
    assert_eq!(allocation_snapshot(&root), before);
    assert!(root.completed().is_err());
    assert!(root.take_complete().is_err());
    let calls = fixture.calls();
    assert!(matches!(
        prepare(fixture, &mut root, 17, 4096),
        Err(MemorySessionError::InvalidDeviceMemoryAuthority)
    ));
    assert_eq!(allocation_snapshot(&root), before);
    assert_eq!(fixture.calls(), calls);
    assert!(root.requires_retention());
    root.retain_live_failure(&mut fixture.memory.engine);
    let terminal = fixture
        .memory
        .engine
        .terminal_device_initialization
        .allocation_as_ref()
        .unwrap();
    assert_eq!(allocation_snapshot(terminal), before);
    assert!(terminal.completed().is_err());
    assert!(
        fixture
            .memory
            .engine
            .terminal_device_initialization
            .as_ref()
            .is_none()
    );
    no_retry(fixture);
}

#[path = "device_allocation/allocation_tests.rs"]
mod allocation_tests;
#[path = "device_allocation/credit_tests.rs"]
mod credit_tests;
