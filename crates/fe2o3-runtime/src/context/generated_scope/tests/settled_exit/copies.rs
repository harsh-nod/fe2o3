#![cfg(test)]
use super::*;

#[test]
fn early_async_callback_result_waits_for_actual_copy_release_and_healthy_decoder() {
    let mut context = RuntimeContextV1::open_with_version_journal_v1(
        KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1(),
        16,
        16,
    )
    .unwrap();
    let devices = [context.devices()[0].id(), context.devices()[1].id()];
    let copy_stream = context.create_stream(devices[1]).unwrap();
    let compute_stream = context.create_stream(devices[0]).unwrap();
    let source = context
        .allocate(devices[0], RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    let destination = context
        .allocate(devices[1], RuntimeMemoryKindV1::HostVisible, 96, 8)
        .unwrap();
    context.write_allocation(source, 0, &[0x57; 64]).unwrap();
    context
        .write_allocation(destination, 0, &[0xa3; 96])
        .unwrap();
    let decoded = Cell::new(0);
    let dropped = Cell::new(0);
    let carrier = borrowed(&decoded, &dropped);
    carrier.ticks.set(100);
    let prepared = context.bound_multi_preparation_for_test_v1(devices[0], carrier);
    let result = futures_executor::LocalPool::new().run_until(
        context.with_generated_gfx942_scope_settled_async_v1(
            2,
            Instant::now() + Duration::from_secs(30),
            |_| std::future::ready(()),
            async |scope| {
                scope.hooks = hooks();
                scope.admit(prepared, compute_stream).unwrap();
                scope
                    .peer_copy_v1(
                        copy_stream,
                        RuntimeMemoryRegionV1 {
                            allocation: source,
                            access: RuntimeAccessV1::Read,
                            byte_offset: 0,
                            byte_len: 64,
                        },
                        RuntimeMemoryRegionV1 {
                            allocation: destination,
                            access: RuntimeAccessV1::Write,
                            byte_offset: 16,
                            byte_len: 64,
                        },
                    )
                    .unwrap();
                assert_eq!(scope.pending_v1(), 2);
                assert_eq!((decoded.get(), dropped.get()), (0, 0));
                (source, destination)
            },
        ),
    );
    assert_eq!(
        result.unwrap().into_parts_v1(),
        ((source, destination), Ok(()))
    );
    assert!(!context.scope_epoch.active());
    assert_eq!((decoded.get(), dropped.get()), (1, 1));
    assert!(context.submissions.is_empty());
    let mut bytes = [0; 96];
    context.read_allocation(destination, 0, &mut bytes).unwrap();
    let mut expected = [0xa3; 96];
    expected[16..80].fill(0x57);
    assert_eq!(bytes, expected);
    assert!(context.cleanup().is_complete());
}
