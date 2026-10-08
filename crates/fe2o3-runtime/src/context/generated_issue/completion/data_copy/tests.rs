//! Real Context journal/order tests with a synthetic lower copy, not DATA admission.

use super::*;
use fe2o3_runtime_model::ContextAllocationStateV1;

type Context = RuntimeContextV1<KfdMultiDeviceRuntimeBackendV1>;

struct Fixture {
    context: Context,
    stream: RuntimeStreamIdV1,
    source: RuntimeMemoryRegionV1,
    destination: RuntimeMemoryRegionV1,
}

fn fixture() -> Fixture {
    let mut context = Context::open_with_version_journal_v1(
        KfdMultiDeviceRuntimeBackendV1::mock_preparation_v1(),
        16,
        16,
    )
    .unwrap();
    let device = context.devices()[0].id();
    let source = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    let destination = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, 64, 8)
        .unwrap();
    context.write_allocation(source, 0, &[0x37; 64]).unwrap();
    Fixture {
        stream: context.create_stream(device).unwrap(),
        context,
        source: RuntimeMemoryRegionV1 {
            allocation: source,
            byte_offset: 0,
            byte_len: 64,
            access: RuntimeAccessV1::Read,
        },
        destination: RuntimeMemoryRegionV1 {
            allocation: destination,
            byte_offset: 0,
            byte_len: 64,
            access: RuntimeAccessV1::Write,
        },
    }
}

fn state(context: &Context, allocation: RuntimeAllocationIdV1) -> ContextAllocationStateV1 {
    context
        .versions
        .as_ref()
        .unwrap()
        .journal_for_test()
        .lookup_allocation(context.allocations[&allocation].journal.unwrap())
        .unwrap()
}

fn admit(
    f: &mut Fixture,
    copy: &mut Option<RuntimeSubmissionV1<RuntimeCopyV1>>,
) -> Result<bool, RuntimeErrorV1<KfdRuntimeBackendErrorV1>> {
    let (stream, source, destination) = (f.stream, f.source, f.destination);
    advance_destination_after_source(&mut f.context, copy, |context, copy| {
        let prepared = context.prepare_context_copy_v1(stream, source, destination, &[], None)?;
        *copy = Some(context.submit_context_operation_v1(
            stream,
            prepared.stream_record,
            &[destination.allocation],
            None,
            &[],
            |backend| {
                backend.copy_async_v1(
                    prepared.stream_record.backend_stream,
                    prepared.source,
                    prepared.destination,
                    &[],
                )
            },
        )?);
        Ok(false)
    })
}

fn finish_mock_backend(f: &mut Fixture, copy: &RuntimeSubmissionV1<RuntimeCopyV1>) {
    let stream = f.context.streams[&f.stream].backend_stream;
    for _ in 0..8 {
        f.context.backend.progress_stream_v1(stream).unwrap();
        if f.context.backend.poll_v1(copy.backend_submission).unwrap() == BackendPollV1::Succeeded {
            return;
        }
    }
    panic!("bounded mock copy did not settle");
}

#[test]
fn destination_only_writer_stays_pending_until_closing_step_then_releases_once() {
    let mut f = fixture();
    let source_before = state(&f.context, f.source.allocation);
    let destination_before = state(&f.context, f.destination.allocation);
    let mut copy = None;
    assert!(!admit(&mut f, &mut copy).unwrap());
    let original = copy.as_ref().unwrap();
    let id = original.id;
    let pending = state(&f.context, f.destination.allocation);
    let writer = pending.pending_writer.unwrap();
    assert_eq!(writer.key.context_generation, id.context_generation);
    assert_eq!(writer.key.local, id.local);
    assert_ne!(writer.key.local, original.backend_submission);
    assert_eq!(pending.content_lineage, destination_before.content_lineage);
    assert_eq!(pending.attempt_epoch, destination_before.attempt_epoch + 1);
    assert_eq!(state(&f.context, f.source.allocation), source_before);
    finish_mock_backend(&mut f, copy.as_ref().unwrap());
    assert!(
        !advance_destination_after_source(&mut f.context, &mut copy, |_, _| Ok(false)).unwrap()
    );
    assert_eq!(state(&f.context, f.destination.allocation), pending);
    assert!(f.context.submissions.contains_key(&id));
    assert!(
        advance_destination_after_source(&mut f.context, &mut copy, |context, _| {
            assert_eq!(state(context, f.destination.allocation), pending);
            Ok(true)
        })
        .unwrap()
    );
    let settled = state(&f.context, f.destination.allocation);
    assert_eq!(
        settled.content_lineage,
        destination_before.content_lineage + 1
    );
    assert!(settled.pending_writer.is_none());
    assert_eq!(settled.attempt_epoch, pending.attempt_epoch);
    assert!(!f.context.submissions.contains_key(&id));
    assert!(advance_destination_after_source(&mut f.context, &mut copy, |_, _| Ok(true)).is_err());
    assert_eq!(state(&f.context, f.destination.allocation), settled);
    assert_eq!(state(&f.context, f.source.allocation), source_before);
    assert!(f.context.cleanup().is_complete());
}

#[test]
fn closing_failure_or_unwind_after_backend_success_keeps_exact_pending_writer() {
    for unwind in [false, true] {
        let mut f = fixture();
        let mut copy = None;
        assert!(!admit(&mut f, &mut copy).unwrap());
        let id = copy.as_ref().unwrap().id;
        finish_mock_backend(&mut f, copy.as_ref().unwrap());
        let pending = state(&f.context, f.destination.allocation);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            advance_destination_after_source(&mut f.context, &mut copy, |_, _| {
                assert!(!unwind, "scripted original source closing unwind");
                Err(RuntimeValidationErrorV1::InvalidBackendDescription.into())
            })
        }));
        assert!(if unwind {
            result.is_err()
        } else {
            matches!(result, Ok(Err(_)))
        });
        assert_eq!(copy.as_ref().unwrap().id, id);
        assert_eq!(state(&f.context, f.destination.allocation), pending);
        assert!(pending.pending_writer.is_some());
        assert!(f.context.submissions.contains_key(&id));
        // The concrete outer generated caller quarantines this uncertainty.
        // This isolated hook test retains its synthetic Context instead of retrying.
        core::mem::forget(f);
    }
}

#[test]
fn destination_reservation_refusal_keeps_prior_writer_and_returns_no_new_token() {
    let mut f = fixture();
    let mut first = None;
    assert!(!admit(&mut f, &mut first).unwrap());
    let original = first.as_ref().unwrap().id;
    let pending = state(&f.context, f.destination.allocation);
    let mut refused = None;
    assert!(admit(&mut f, &mut refused).is_err());
    assert!(refused.is_none());
    assert_eq!(state(&f.context, f.destination.allocation), pending);
    assert!(f.context.submissions.contains_key(&original));
    assert_eq!(f.context.submissions.len(), 1);
    finish_mock_backend(&mut f, first.as_ref().unwrap());
    assert!(advance_destination_after_source(&mut f.context, &mut first, |_, _| Ok(true)).unwrap());
    assert!(f.context.cleanup().is_complete());
}
