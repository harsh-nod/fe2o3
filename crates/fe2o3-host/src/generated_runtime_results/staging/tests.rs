use super::*;
use fe2o3_runtime::*;
use std::panic::{AssertUnwindSafe, catch_unwind};

mod backend;
use backend::{Backend, Failure, State};

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod native;

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
mod graph;

struct Output<T: GeneratedDeviceScalarV1> {
    budget: GeneratedRuntimeResultBudgetV1,
    custody: ChargedOutputCustodyV1,
    observer: GeneratedRuntimeChargedResultV1<T>,
    gate: Arc<ResultReadyGateV1>,
}

fn output<T: GeneratedDeviceScalarV1>(values: Box<[T]>) -> Output<T> {
    // Real charged storage, but test-only readiness; no native completion receipt.
    let budget =
        GeneratedRuntimeResultBudgetV1::new((values.len() * size_of::<T>() * 2) as u64, 1).unwrap();
    let (custody, observer) = ChargedOutputCustodyV1::new::<T>(values.len());
    let descriptor = ResultDescriptorV1::new::<T>(
        values.len(),
        Gfx942RuntimeBufferAccessV1::WriteOnly,
        Some(&custody),
    )
    .unwrap();
    let mut preflight = ResultPreflightV1::new().unwrap();
    preflight
        .push(
            ResultDescriptorV1::new::<T>(
                values.len(),
                Gfx942RuntimeBufferAccessV1::WriteOnly,
                Some(&custody),
            )
            .unwrap(),
        )
        .unwrap();
    let mut binding = preflight.reserve(&budget).unwrap();
    custody
        .bind_seed(values, binding.take(&descriptor).unwrap())
        .unwrap();
    assert!(binding.complete());
    Output {
        budget,
        custody,
        observer,
        gate: binding.gate.clone(),
    }
}

fn context() -> (RuntimeContextV1<Backend>, Arc<Mutex<State>>) {
    let state = Arc::new(Mutex::new(State::default()));
    let mut context =
        RuntimeContextV1::open_with_version_journal_v1(Backend(state.clone()), 8, 8).unwrap();
    let devices: Vec<_> = context.devices().iter().map(|device| device.id()).collect();
    for device in devices {
        context
            .configure_allocation_admission_v1(device, 4096, 8)
            .unwrap();
    }
    (context, state)
}

fn allocation(
    context: &mut RuntimeContextV1<Backend>,
    child: usize,
    kind: RuntimeMemoryKindV1,
    bytes: u64,
) -> RuntimeAllocationIdV1 {
    context
        .allocate(context.devices()[child].id(), kind, bytes, 8)
        .unwrap()
}

fn region(
    allocation: RuntimeAllocationIdV1,
    access: RuntimeAccessV1,
    offset: u64,
    bytes: u64,
) -> RuntimeMemoryRegionV1 {
    RuntimeMemoryRegionV1 {
        allocation,
        access,
        byte_offset: offset,
        byte_len: bytes,
    }
}

fn read(
    context: &mut RuntimeContextV1<Backend>,
    allocation: RuntimeAllocationIdV1,
    bytes: usize,
) -> Vec<u8> {
    let mut output = vec![0; bytes];
    context.read_allocation(allocation, 0, &mut output).unwrap();
    output
}

#[test]
fn staging_encodes_all_ten_scalars_without_relocating_or_refunding_original_storage() {
    fn check<T: GeneratedDeviceScalarV1>(values: Box<[T]>, expected: &[u8]) {
        let mut output = output(values);
        output.custody.decode(expected, &output.gate).unwrap();
        output.gate.commit();
        let result = output.observer.try_take().unwrap().unwrap();
        let pointer = result.as_slice().as_ptr();
        let usage = output.budget.usage();
        let mut scratch = vec![0xa5; expected.len()];
        let (mut context, state) = context();
        let host = allocation(
            &mut context,
            0,
            RuntimeMemoryKindV1::HostVisible,
            expected.len() as u64,
        );
        result
            .write_staging_v1(&mut context, host, &mut scratch)
            .unwrap();
        assert_eq!(scratch, expected);
        assert_eq!(read(&mut context, host, expected.len()), expected);
        assert_eq!(state.lock().unwrap().writes, 1);
        assert_eq!(result.as_slice().as_ptr(), pointer);
        assert_eq!(output.budget.usage(), usage);
        assert!(context.cleanup().is_complete());
        context.shutdown().unwrap();
        drop(result);
        assert_eq!(output.budget.usage().reserved_peak_bytes, 0);
    }
    check(Box::new([0u8, 255]), &[0, 255]);
    check(Box::new([i8::MIN, -1]), &[128, 255]);
    check(Box::new([0x1234u16, u16::MAX]), &[0x34, 0x12, 255, 255]);
    check(Box::new([i16::MIN, -1]), &[0, 128, 255, 255]);
    check(Box::new([0x12345678u32]), &[0x78, 0x56, 0x34, 0x12]);
    check(Box::new([i32::MIN]), &[0, 0, 0, 128]);
    check(
        Box::new([0x0123456789abcdefu64]),
        &[0xef, 0xcd, 0xab, 0x89, 0x67, 0x45, 0x23, 0x01],
    );
    check(Box::new([i64::MIN]), &[0, 0, 0, 0, 0, 0, 0, 128]);
    check(
        Box::new([f32::from_bits(0x7fc01234), -0.0]),
        &[0x34, 0x12, 0xc0, 0x7f, 0, 0, 0, 128],
    );
    check(
        Box::new([f64::from_bits(0xfff8000000001234), -0.0]),
        &[0x34, 0x12, 0, 0, 0, 0, 0xf8, 0xff, 0, 0, 0, 0, 0, 0, 0, 128],
    );
}

#[test]
fn staging_authentication_contention_unavailable_and_scratch_fail_before_effects() {
    let mut output = output(Box::new([7u32, 11]));
    let (mut context, state) = context();
    let host = allocation(&mut context, 0, RuntimeMemoryKindV1::HostVisible, 8);
    let gate = output.gate.clone();
    let mut scratch = [0xa5; 8];
    for ready in [false, true] {
        if ready {
            gate.commit();
        }
        for foreign in [false, true] {
            if ready && !foreign {
                continue;
            }
            let before = output.budget.usage();
            assert!(matches!(
                output.observer.stage_completed_matching_v1(
                    |actual| !foreign && Arc::ptr_eq(actual, &gate),
                    &mut context,
                    host,
                    &mut scratch
                ),
                Err(GeneratedRuntimeStagingErrorV1::Output(
                    Error::BindingMismatch
                ))
            ));
            assert_eq!(scratch, [0xa5; 8]);
            assert_eq!(output.budget.usage(), before);
        }
    }
    let slot = output.observer.slot.clone();
    let lock = slot.state.lock().unwrap();
    assert_eq!(
        output
            .observer
            .stage_completed_matching_v1(|_| panic!("contended"), &mut context, host, &mut scratch)
            .unwrap(),
        None
    );
    drop(lock);
    for size in [0, 7, 9] {
        let mut scratch = vec![0xa5; size];
        assert!(matches!(
            output.observer.stage_completed_matching_v1(
                |actual| Arc::ptr_eq(actual, &gate),
                &mut context,
                host,
                &mut scratch
            ),
            Err(GeneratedRuntimeStagingErrorV1::Output(Error::ByteLength))
        ));
        assert_eq!(scratch, vec![0xa5; size]);
    }
    assert_eq!(state.lock().unwrap().writes, 0);
    output
        .observer
        .stage_completed_matching_v1(
            |actual| Arc::ptr_eq(actual, &gate),
            &mut context,
            host,
            &mut scratch,
        )
        .unwrap()
        .unwrap();
    assert_eq!(state.lock().unwrap().writes, 1);
    let result = output.observer.try_take().unwrap().unwrap();
    assert_eq!(result.as_slice(), &[7, 11]);
    assert!(matches!(
        output
            .observer
            .stage_completed_matching_v1(|_| true, &mut context, host, &mut scratch),
        Err(GeneratedRuntimeStagingErrorV1::Output(
            Error::OutputUnavailable
        ))
    ));
    assert_eq!(state.lock().unwrap().writes, 1);
    drop(result);
    assert_eq!(output.budget.usage().reserved_peak_bytes, 0);
    assert!(context.cleanup().is_complete());
    context.shutdown().unwrap();
}

#[test]
fn staging_wrong_and_stale_destinations_preserve_result_without_backend_writes() {
    let mut output = output(Box::new([7u32, 11]));
    output.gate.commit();
    let result = output.observer.try_take().unwrap().unwrap();
    let usage = output.budget.usage();
    let (mut context, state) = context();
    let (mut foreign, _) = self::context();
    let device = allocation(&mut context, 0, RuntimeMemoryKindV1::DeviceLocal, 8);
    let short = allocation(&mut context, 0, RuntimeMemoryKindV1::HostVisible, 4);
    let stale = allocation(&mut context, 0, RuntimeMemoryKindV1::HostVisible, 8);
    let other = allocation(&mut foreign, 0, RuntimeMemoryKindV1::HostVisible, 8);
    context.release_allocation(stale).unwrap();
    for destination in [device, short, stale, other] {
        let mut scratch = [0xa5; 8];
        assert!(matches!(
            result.write_staging_v1(&mut context, destination, &mut scratch),
            Err(GeneratedRuntimeStagingErrorV1::Context(
                RuntimeErrorV1::Validation(_)
            ))
        ));
        assert_eq!(scratch, [7, 0, 0, 0, 11, 0, 0, 0]);
        assert_eq!(result.as_slice(), &[7, 11]);
        assert_eq!(output.budget.usage(), usage);
        assert_eq!(state.lock().unwrap().writes, 0);
    }
    assert!(context.cleanup().is_complete());
    context.shutdown().unwrap();
    assert!(foreign.cleanup().is_complete());
    foreign.shutdown().unwrap();
    drop(result);
    assert_eq!(output.budget.usage().reserved_peak_bytes, 0);
}

#[test]
fn staging_empty_and_poisoned_results_never_write() {
    let (mut context, state) = context();
    let host = allocation(&mut context, 0, RuntimeMemoryKindV1::HostVisible, 8);
    let mut empty = output::<u32>(Box::new([]));
    empty.gate.commit();
    let result = empty.observer.try_take().unwrap().unwrap();
    assert!(matches!(
        result.write_staging_v1(&mut context, host, &mut []),
        Err(GeneratedRuntimeStagingErrorV1::Output(Error::ByteLength))
    ));
    let mut output = output(Box::new([7u32, 11]));
    output.gate.commit();
    let slot = output.observer.slot.clone();
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _guard = slot.state.lock().unwrap();
            panic!("poison fixture");
        }))
        .is_err()
    );
    let mut scratch = [0xa5; 8];
    assert!(matches!(
        output.observer.stage_completed_matching_v1(
            |_| panic!("poison"),
            &mut context,
            host,
            &mut scratch
        ),
        Err(GeneratedRuntimeStagingErrorV1::Output(Error::Custody))
    ));
    assert_eq!(scratch, [0xa5; 8]);
    assert_eq!(state.lock().unwrap().writes, 0);
    assert!(context.cleanup().is_complete());
    context.shutdown().unwrap();
}

#[test]
fn staging_backend_failures_and_panics_preserve_unpoisoned_result_and_charge() {
    for mode in [
        Failure::Rejected,
        Failure::Quiescent,
        Failure::Terminal,
        Failure::Panic,
    ] {
        let mut output = output(Box::new([7u32, 11]));
        output.gate.commit();
        let before = output.budget.usage();
        let (mut context, state) = context();
        let host = allocation(&mut context, 0, RuntimeMemoryKindV1::HostVisible, 8);
        state.lock().unwrap().write_failure = mode;
        let gate = output.gate.clone();
        let mut scratch = [0; 8];
        let result = catch_unwind(AssertUnwindSafe(|| {
            output.observer.stage_completed_matching_v1(
                |actual| Arc::ptr_eq(actual, &gate),
                &mut context,
                host,
                &mut scratch,
            )
        }));
        match (mode, result) {
            (
                Failure::Rejected,
                Ok(Err(GeneratedRuntimeStagingErrorV1::Context(RuntimeErrorV1::BackendRejected(
                    _,
                )))),
            )
            | (
                Failure::Quiescent,
                Ok(Err(GeneratedRuntimeStagingErrorV1::Context(RuntimeErrorV1::BackendQuiescent(
                    _,
                )))),
            )
            | (
                Failure::Terminal,
                Ok(Err(GeneratedRuntimeStagingErrorV1::Context(RuntimeErrorV1::BackendTerminal(
                    _,
                )))),
            ) => (),
            (Failure::Panic, Err(payload)) => assert_eq!(payload.downcast_ref::<u32>(), Some(&83)),
            _ => panic!("wrong failure class"),
        }
        assert_eq!(state.lock().unwrap().writes, 1);
        assert_eq!(output.budget.usage(), before);
        output
            .observer
            .encode_completed_matching_v1(|actual| Arc::ptr_eq(actual, &gate), &mut scratch)
            .unwrap()
            .unwrap();
        if mode == Failure::Rejected {
            state.lock().unwrap().write_failure = Failure::None;
            output
                .observer
                .stage_completed_matching_v1(
                    |actual| Arc::ptr_eq(actual, &gate),
                    &mut context,
                    host,
                    &mut scratch,
                )
                .unwrap()
                .unwrap();
            assert_eq!(state.lock().unwrap().writes, 2);
            assert_eq!(read(&mut context, host, 8), scratch);
            assert_eq!(output.budget.usage(), before);
        }
        let result = output.observer.try_take().unwrap().unwrap();
        assert_eq!(result.as_slice(), &[7, 11]);
        assert_eq!(output.budget.usage(), before);
        drop(result);
        assert_eq!(output.budget.usage().reserved_peak_bytes, 0);
        if matches!(mode, Failure::Panic | Failure::Terminal) {
            assert!(context.is_terminal());
            core::mem::forget(context);
        } else {
            if mode == Failure::Quiescent {
                assert!(
                    context
                        .write_host_visible_allocation_v1(host, &scratch)
                        .is_err()
                );
                assert_eq!(state.lock().unwrap().writes, 1);
            }
            assert!(context.cleanup().is_complete());
            context.shutdown().unwrap();
        }
    }
}

#[test]
fn staging_settled_upload_then_peer_preserves_readers_versions_guards_and_credits() {
    for (failed, early_drop) in [(false, false), (false, true), (true, false)] {
        let mut output = output(Box::new([0x12345678u32, 0x90abcdef]));
        output.gate.commit();
        let result = output.observer.try_take().unwrap().unwrap();
        let before = output.budget.usage();
        let (mut context, state) = context();
        let streams: Vec<_> = (0..2)
            .map(|child| {
                context
                    .create_stream(context.devices()[child].id())
                    .unwrap()
            })
            .collect();
        let host = allocation(&mut context, 0, RuntimeMemoryKindV1::HostVisible, 8);
        let source = allocation(&mut context, 0, RuntimeMemoryKindV1::DeviceLocal, 8);
        let destination = allocation(&mut context, 1, RuntimeMemoryKindV1::DeviceLocal, 16);
        context
            .write_allocation(destination, 0, &[0xa5; 16])
            .unwrap();
        let mut scratch = [0; 8];
        result
            .write_staging_v1(&mut context, host, &mut scratch)
            .unwrap();
        let result = if early_drop {
            drop(result);
            None
        } else {
            Some(result)
        };
        let retained_usage = output.budget.usage();
        if early_drop {
            assert_eq!(retained_usage.reserved_peak_bytes, 0);
        } else {
            assert_eq!(retained_usage, before);
        }
        state.lock().unwrap().copy_failure = if failed {
            Failure::Quiescent
        } else {
            Failure::None
        };
        let source_read = region(source, RuntimeAccessV1::Read, 0, 8);
        let destination_write = region(destination, RuntimeAccessV1::Write, 4, 8);
        let mut upload = context
            .copy_async(
                streams[0],
                region(host, RuntimeAccessV1::Read, 0, 8),
                region(source, RuntimeAccessV1::Write, 0, 8),
                &[],
            )
            .unwrap();
        if let Some(result) = &result {
            assert!(
                result
                    .write_staging_v1(&mut context, host, &mut scratch)
                    .is_err()
            );
        } else {
            assert!(
                context
                    .write_host_visible_allocation_v1(host, &scratch)
                    .is_err()
            );
        }
        assert!(context.release_allocation(host).is_err());
        assert!(
            context
                .peer_copy(streams[1], source_read, destination_write, &[])
                .is_err()
        );
        assert_eq!(state.lock().unwrap().copies_submitted, 1);
        context.flush_stream(streams[0]).unwrap();
        let status = context.poll(&mut upload);
        if failed {
            assert!(matches!(status, Err(RuntimeErrorV1::BackendQuiescent(_))));
            assert!(
                context
                    .peer_copy(streams[1], source_read, destination_write, &[])
                    .is_err()
            );
            assert_eq!(state.lock().unwrap().peer_submitted, 0);
        } else {
            assert_eq!(status.unwrap(), RuntimePollV1::Succeeded);
            let mut peer = context
                .peer_copy(streams[1], source_read, destination_write, &[])
                .unwrap();
            assert!(context.write_allocation(source, 0, &[0; 8]).is_err());
            context.flush_stream(streams[1]).unwrap();
            assert_eq!(context.poll(&mut peer).unwrap(), RuntimePollV1::Succeeded);
            let mut expected = vec![0xa5; 16];
            expected[4..12].copy_from_slice(&scratch);
            assert_eq!(read(&mut context, destination, 16), expected);
            assert_eq!(read(&mut context, source, 8), scratch);
            context.release_submission(peer).unwrap();
            assert_eq!(state.lock().unwrap().peer_submitted, 1);
        }
        context.release_submission(upload).unwrap();
        assert_eq!(output.budget.usage(), retained_usage);
        assert!(context.cleanup().is_complete());
        context.shutdown().unwrap();
        drop(result);
        assert_eq!(output.budget.usage().reserved_peak_bytes, 0);
    }
}
