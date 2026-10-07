//! Actual native records and model ownership with a fake backend, not Linux/GPU evidence.
#[path = "host_backing/borrowed_initialization.rs"]
mod borrowed_initialization;
#[path = "host_backing/host_pool.rs"]
mod host_pool;
#[path = "host_backing/rooted.rs"]
mod rooted;

use super::*;
use crate::sdma::{Gfx942SdmaBufferStorageV1, Gfx942SdmaBufferV1};

type HostCpu = SharedGttAllocationV1<HostVisibleCoherentGttV1, GttCpuWritableV1>;
type HostMapped = SharedGttAllocationV1<HostVisibleCoherentGttV1, GttGpuAccessibleMutableV1>;

fn budget(bytes: u64, records: usize) -> Gfx942HostVisibleBackingBudgetV1 {
    Gfx942HostVisibleBackingBudgetV1::new(bytes, records).unwrap()
}

fn configured(bytes: u64, records: usize) -> SharedMemoryEngine<FakeBackend> {
    let mut engine = acquired();
    let (device, vm) = device_vm(1);
    engine
        .configure_host_visible_backing_budget_v1(device, vm, budget(bytes, records))
        .unwrap();
    engine
}

fn usage(engine: &SharedMemoryEngine<FakeBackend>) -> Gfx942HostVisibleBackingUsageV1 {
    engine.host_backing_account.as_ref().unwrap().usage()
}

fn debit(engine: &SharedMemoryEngine<FakeBackend>, bytes: u64, records: u64) {
    let usage = usage(engine);
    assert_eq!(usage.used_backing_bytes, bytes);
    assert_eq!(usage.used_allocation_records, records);
    assert_eq!(usage.reserved_records, 0);
}

fn calls(engine: &SharedMemoryEngine<FakeBackend>) -> [usize; 9] {
    let backend = &engine.backend;
    [
        backend.currentness_calls,
        backend.operational_currentness_calls,
        backend.reserve_va_calls,
        backend.alloc_calls,
        backend.map_cpu_calls,
        backend.map_gpu_calls,
        backend.unmap_gpu_calls,
        backend.free_calls,
        backend.release_va_calls,
    ]
}

fn closed(engine: &mut SharedMemoryEngine<FakeBackend>) {
    assert_eq!(engine.phase(), SharedMemorySessionPhaseV1::Quarantined);
    let before = calls(engine);
    assert!(matches!(
        engine.allocate::<HostVisibleCoherentGttV1>(1),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    let token = HostCpu {
        session_id: engine.session_id,
        id: 1,
        generation: 1,
        layout: profile_layout::<HostVisibleCoherentGttV1>(1).unwrap(),
        marker: PhantomData,
    };
    assert!(matches!(
        engine.with_bytes(&token, SharedAllocationPhaseV1::CpuWritable, |_| ()),
        Err(MemorySessionError::SharedSessionQuarantined)
    ));
    assert_eq!(calls(engine), before);
}

fn mapped(engine: &mut SharedMemoryEngine<FakeBackend>, bytes: usize) -> HostMapped {
    let token = engine.allocate::<HostVisibleCoherentGttV1>(bytes).unwrap();
    engine.map_mutable(token).unwrap()
}

fn panic_payload(
    payload: &(dyn core::any::Any + Send),
    prefix: &'static str,
    operation: &'static str,
) {
    assert_eq!(
        payload.downcast_ref::<(&'static str, &'static str)>(),
        Some(&(prefix, operation))
    );
}

fn configure_fixture(fixture: &mut BackingConstructorFixture, configured: bool) {
    fixture
        .ownership
        .configure_optional_host_visible_backing_budget(
            &mut fixture.engine,
            fixture.device,
            fixture.vm,
            configured.then(|| budget(16384, 4)),
        )
        .unwrap();
}

fn projected_host(fixture: &mut BackingConstructorFixture) -> HostMapped {
    let token = fixture
        .engine
        .allocate::<HostVisibleCoherentGttV1>(4100)
        .unwrap();
    let (id, generation, layout, base, handle) = fixture.engine.evidence(&token).unwrap();
    let (reservation, allocation, mapping) = model_keys(fixture.vm, id, generation);
    let projected = project_allocation(
        fixture.foundation.memory(),
        reservation,
        allocation,
        base,
        layout,
        handle,
        MemoryKindV1::HostVisibleCoherent,
    )
    .unwrap();
    fixture
        .foundation
        .replace_memory_after_sealed_transition(projected)
        .unwrap();
    let token = fixture.engine.map_mutable(token).unwrap();
    let projected = project_map(fixture.foundation.memory(), mapping, fixture.device).unwrap();
    fixture
        .foundation
        .replace_memory_after_sealed_transition(projected)
        .unwrap();
    token
}

fn projected_dispose(fixture: &mut BackingConstructorFixture, token: HostMapped) {
    let (reservation, allocation, mapping) = model_keys(fixture.vm, token.id, token.generation);
    let token = fixture.engine.unmap_mutable(token).unwrap();
    let projected = project_unmap(fixture.foundation.memory(), mapping).unwrap();
    fixture
        .foundation
        .replace_memory_after_sealed_transition(projected)
        .unwrap();
    let projected = project_release(
        fixture.foundation.memory(),
        reservation,
        allocation,
        mapping,
    )
    .unwrap();
    fixture
        .engine
        .release(token, SharedAllocationPhaseV1::CpuWritable)
        .unwrap();
    fixture
        .foundation
        .replace_memory_after_sealed_transition(projected)
        .unwrap();
}

#[path = "host_backing/admission_tests.rs"]
mod admission_tests;
#[path = "host_backing/loan_tests.rs"]
mod loan_tests;
