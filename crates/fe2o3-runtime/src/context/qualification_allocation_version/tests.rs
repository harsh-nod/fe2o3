//! CPU Context/journal controls, not native admission or completion evidence.
use super::*;
use crate::KfdRuntimeBackendV1;
type Context = RuntimeContextV1<KfdRuntimeBackendV1>;

fn context() -> Context {
    Context::open_with_version_journal_v1(
        KfdRuntimeBackendV1::mock_worker_v3_generated_only_v1(),
        8,
        8,
    )
    .unwrap()
}

fn region(context: &mut Context) -> RuntimeMemoryRegionV1 {
    let device = context.devices()[0].id();
    RuntimeMemoryRegionV1 {
        allocation: context
            .allocate(device, RuntimeMemoryKindV1::HostVisible, 16, 8)
            .unwrap(),
        access: RuntimeAccessV1::ReadWrite,
        byte_offset: 0,
        byte_len: 16,
    }
}

#[test]
fn snapshot_preserves_exact_identity_and_is_allocation_free() {
    let mut context = context();
    let region = region(&mut context);
    let before = context.version_journal_usage_v1();
    let (snapshot, allocations) = crate::kfd_backend::counted_allocations_for_test_v1(|| {
        context.observe_allocation_version_qualification_v1(region)
    });
    let snapshot = snapshot.unwrap();
    assert_eq!(allocations, 0);
    assert_eq!(snapshot.allocation(), region.allocation);
    assert_eq!(snapshot.device(), context.devices()[0].id());
    assert_eq!(snapshot.byte_extent(), 16);
    assert_eq!(
        (snapshot.attempt_epoch(), snapshot.content_lineage()),
        (0, 0)
    );
    assert_eq!(
        context.observe_allocation_version_qualification_v1(region),
        Ok(snapshot)
    );
    assert_eq!(context.version_journal_usage_v1(), before);
    assert!(context.cleanup().is_complete());
}

#[test]
fn snapshot_refuses_partial_zero_foreign_and_released_allocations() {
    let mut context = context();
    let original = region(&mut context);
    for (byte_offset, byte_len) in [(0, 0), (1, 15), (0, 15), (0, 17)] {
        assert_eq!(
            context.observe_allocation_version_qualification_v1(RuntimeMemoryRegionV1 {
                byte_offset,
                byte_len,
                ..original
            }),
            Err(RuntimeValidationErrorV1::InvalidRange)
        );
    }
    let mut other = self::context();
    let foreign = region(&mut other);
    assert_eq!(
        context.observe_allocation_version_qualification_v1(foreign),
        Err(RuntimeValidationErrorV1::UnknownAllocation)
    );
    context.release_allocation(original.allocation).unwrap();
    assert_eq!(
        context.observe_allocation_version_qualification_v1(original),
        Err(RuntimeValidationErrorV1::UnknownAllocation)
    );
    assert!(context.cleanup().is_complete());
    assert!(other.cleanup().is_complete());
}

#[test]
fn snapshot_refuses_pending_writer_until_actual_cpu_write_settles() {
    let mut context = context();
    let region = region(&mut context);
    let record = context.allocations[&region.allocation];
    let ticket = context
        .begin_journal_host_write_v1(region.allocation, &record)
        .unwrap()
        .unwrap();
    assert_eq!(
        context.observe_allocation_version_qualification_v1(region),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    context
        .write_with_journal_v1(ticket, &record, 0, &[7; 16])
        .unwrap();
    let snapshot = context
        .observe_allocation_version_qualification_v1(region)
        .unwrap();
    assert_eq!(
        (snapshot.attempt_epoch(), snapshot.content_lineage()),
        (1, 1)
    );
    assert!(context.cleanup().is_complete());
}

#[test]
fn snapshot_refuses_original_unknown_writer_without_settling_it() {
    let mut context = context();
    let region = region(&mut context);
    let writer = context.retain_test_writer_v1(
        &[region.allocation],
        fe2o3_runtime_model::ContextWriterKindV1::Synchronous,
    );
    let before = context
        .versions
        .as_ref()
        .unwrap()
        .journal_for_test()
        .lookup_writer(writer)
        .unwrap();
    assert_eq!(
        context.observe_allocation_version_qualification_v1(region),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    assert_eq!(
        context
            .versions
            .as_ref()
            .unwrap()
            .journal_for_test()
            .lookup_writer(writer)
            .unwrap(),
        before
    );
    // Original CPU backend disposal is still required to retire this unknown record.
    context.release_allocation(region.allocation).unwrap();
    assert!(context.cleanup().is_complete());
}

#[test]
fn snapshot_respects_graph_scope_and_terminal_gates() {
    let mut context = context();
    let region = region(&mut context);
    let graph = context.reserve_graph_v1(0).unwrap();
    assert_eq!(
        context.observe_allocation_version_qualification_v1(region),
        Err(RuntimeValidationErrorV1::ContextReserved)
    );
    context.close_graph_issue_v1(graph).unwrap();
    context.release_graph_v1(graph).unwrap();
    let mut epoch = context.scope_epoch.begin().unwrap();
    let denied = context.observe_allocation_version_qualification_v1(region);
    epoch.close();
    assert_eq!(denied, Err(RuntimeValidationErrorV1::ContextReserved));
    context.terminal = true;
    let denied = context.observe_allocation_version_qualification_v1(region);
    context.terminal = false;
    assert_eq!(denied, Err(RuntimeValidationErrorV1::ContextTerminal));
    assert!(context.cleanup().is_complete());
}

#[test]
fn snapshot_refuses_changed_original_record_and_missing_journal() {
    let mut context = context();
    let region = region(&mut context);
    let original = context.allocations[&region.allocation];
    context
        .allocations
        .get_mut(&region.allocation)
        .unwrap()
        .device
        .local += 1;
    let denied = context.observe_allocation_version_qualification_v1(region);
    *context.allocations.get_mut(&region.allocation).unwrap() = original;
    assert_eq!(denied, Err(RuntimeValidationErrorV1::ContextReserved));
    let journal = context.versions.take().unwrap();
    let denied = context.observe_allocation_version_qualification_v1(region);
    context.versions = Some(journal);
    assert_eq!(denied, Err(RuntimeValidationErrorV1::Unsupported));
    assert!(context.cleanup().is_complete());
}
