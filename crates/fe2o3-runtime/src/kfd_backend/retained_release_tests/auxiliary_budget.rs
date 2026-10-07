use super::*;

#[cfg(feature = "hardware-qualification")]
#[test]
#[ignore = "requires an isolated MI300X process and FE2O3_TEST_NATIVE_UNIQUE_ID; terminal retention until process exit"]
fn native_runtime_auxiliary_budget_failure_retains_initialized_prefix() {
    use crate::qualification_gfx942_vecadd_v1::{
        GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1 as ALIGNMENT,
        GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as BYTES, Gfx942VecaddQualificationArgumentsV1,
        admit_gfx942_vecadd_qualification_v1,
    };
    use crate::{RuntimeContextV1, RuntimeErrorV1};

    let prefix: usize = std::env::var("FE2O3_TEST_NATIVE_INITIALIZED_PREFIX")
        .unwrap_or_else(|_| "2".to_owned())
        .parse()
        .unwrap();
    assert!(prefix <= 2);
    let admitted = admit_gfx942_vecadd_qualification_v1().unwrap();
    let buffers = admitted.host_buffers().unwrap();
    let mut backend =
        KfdRuntimeBackendV1::open_gfx942_vecadd_qualification_v1(native_device()).unwrap();
    backend
        .configure_host_visible_backing_budget_v1(
            Gfx942HostVisibleBackingBudgetV1::new((37 + 4 * prefix as u64) * 1024 * 1024, 128)
                .unwrap(),
        )
        .unwrap();
    backend
        .enable_profiler_v1(KfdRuntimeProfilerConfigV1::new([0x78; 32], 128).unwrap())
        .unwrap();
    // Terminal retention intentionally lasts until this isolated test process exits.
    // Do not run ordinary shutdown or Drop on the terminal context, even on assertion failure.
    let mut context = core::mem::ManuallyDrop::new(RuntimeContextV1::open(backend).unwrap());
    let device = context.devices()[0].id();
    let module = context.load_module(device, admitted.hsaco()).unwrap();
    let kernel = context
        .resolve_kernel::<Gfx942VecaddQualificationArgumentsV1>(module, admitted.kernel_name())
        .unwrap();
    let mut prepared = Vec::with_capacity(2);
    for _ in 0..2 {
        let stream = context.create_stream(device).unwrap();
        let allocations = [buffers.left(), buffers.right(), buffers.output()].map(|bytes| {
            let allocation = context
                .allocate(
                    device,
                    RuntimeMemoryKindV1::HostVisible,
                    BYTES as u64,
                    ALIGNMENT,
                )
                .unwrap();
            context.write_allocation(allocation, 0, bytes).unwrap();
            allocation
        });
        prepared.push((
            stream,
            Gfx942VecaddQualificationArgumentsV1::new(
                allocations[0],
                allocations[1],
                allocations[2],
            )
            .unwrap(),
        ));
    }
    let (first_stream, first_args) = &prepared[0];
    let _first = context
        .launch(*first_stream, &kernel, first_args, admitted.geometry(), &[])
        .unwrap();
    context.flush_stream(*first_stream).unwrap();
    let before = context.backend().host_visible_backing_usage_v1().unwrap();
    let primary = context.backend().native_compute_lanes[0].unwrap();
    assert_eq!(primary.ordinal(), 0);
    assert!(context.backend().native_compute_lanes[1].is_none());
    let active = context
        .backend()
        .active
        .iter()
        .chain(context.backend().compute_pipeline.iter())
        .next()
        .unwrap();
    let first_identity = (active.id, active.stream, active.allocations.clone());
    let second_backend_stream = *context
        .backend()
        .streams
        .keys()
        .find(|stream| **stream != active.stream)
        .unwrap();
    let publications_before = context
        .backend()
        .profiler
        .as_ref()
        .unwrap()
        .recorded_events_for_test_v1()
        .iter()
        .filter(|event| {
            matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { .. }
                    | KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        (before.used_backing_bytes, before.used_allocation_records),
        (38_281_216, 12)
    );
    assert_eq!(
        (
            before.reserved_records,
            before.retained_records,
            before.quarantined_records,
            before.poisoned
        ),
        (0, 12, 0, false)
    );
    assert!(
        before.used_backing_bytes + prefix as u64 * BYTES as u64
            <= before.budget.max_backing_bytes()
    );
    assert!(
        before.used_backing_bytes + (prefix as u64 + 1) * BYTES as u64
            > before.budget.max_backing_bytes()
    );

    MATERIALIZATION_RETENTION.with(|slot| slot.set(Some(MaterializationRetention::default())));
    // Leave primary work logically pending so this launch must select AUX, not rebind lane 0.
    let (second_stream, second_args) = &prepared[1];
    let result = context
        .launch(
            *second_stream,
            &kernel,
            second_args,
            admitted.geometry(),
            &[],
        )
        .and_then(|_| context.flush_stream(*second_stream));
    let error = match result {
        Err(RuntimeErrorV1::BackendTerminal(error)) => error,
        other => panic!("expected terminal native budget rejection, got {other:?}"),
    };
    assert_eq!(
        error.detail(),
        "KFD host-visible initialization: host-visible backing resource credits: runtime resource credit error: Capacity"
    );
    assert!(context.is_terminal() && context.backend().terminal);
    assert_eq!(context.backend().native_compute_lanes[0], Some(primary));
    assert!(context.backend().native_compute_lanes[1].is_none());
    assert_eq!(
        context
            .backend()
            .queue
            .as_ref()
            .unwrap()
            .auxiliary_compute_lane_count_v1(),
        0
    );
    assert_eq!(context.backend().allocations.len(), 6);
    assert_eq!(context.backend().pending_compute.len(), 1);
    assert_eq!(
        context
            .backend()
            .pending_compute
            .values()
            .next()
            .unwrap()
            .launch
            .stream,
        second_backend_stream
    );
    let active = context
        .backend()
        .active
        .iter()
        .chain(context.backend().compute_pipeline.iter())
        .find(|active| active.id == first_identity.0)
        .unwrap();
    assert_eq!(
        (active.id, active.stream, &active.allocations),
        (first_identity.0, first_identity.1, &first_identity.2)
    );
    assert!(active.execution.is_some());
    let observation = MATERIALIZATION_RETENTION.with(Cell::get).unwrap();
    assert_eq!(
        (
            observation.calls,
            observation.spec_count,
            observation.retained_count
        ),
        (1, 3, prefix)
    );
    for layout in &observation.layouts[..prefix] {
        let layout = layout.unwrap();
        assert_eq!(
            layout.kind(),
            fe2o3_kfd::Gfx942FixedDispatchDataKindV1::HostVisibleCoherent
        );
        assert_eq!(layout.requested_bytes(), BYTES as u64);
        assert_eq!(layout.alignment(), 4096);
    }
    assert!(observation.layouts[prefix..].iter().all(Option::is_none));
    let failure_usage = observation.failure_usage.unwrap();
    assert_eq!(failure_usage.budget, before.budget);
    assert_eq!(
        failure_usage.used_backing_bytes,
        before.used_backing_bytes + prefix as u64 * BYTES as u64
    );
    assert_eq!(
        failure_usage.used_allocation_records,
        before.used_allocation_records + prefix as u64
    );
    assert_eq!(
        failure_usage.retained_records,
        before.retained_records + prefix
    );
    assert_eq!(
        (
            failure_usage.reserved_records,
            failure_usage.quarantined_records
        ),
        (before.reserved_records, before.quarantined_records)
    );
    // This is an inert pre-retake snapshot; the lower constructor subsequently retains its parent.
    assert!(!failure_usage.poisoned);
    let events = context
        .backend()
        .profiler
        .as_ref()
        .unwrap()
        .recorded_events_for_test_v1();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { .. }
            ))
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
            ))
            .count(),
        1
    );
    let event_count = events.len();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event.event,
                KfdRuntimeProfileEventKindV1::NativeQueueCreated { .. }
                    | KfdRuntimeProfileEventKindV1::DispatchPublished { .. }
            ))
            .cloned()
            .collect::<Vec<_>>(),
        publications_before
    );
    assert!(context.flush_stream(*second_stream).is_err());
    assert_eq!(MATERIALIZATION_RETENTION.with(Cell::get), Some(observation));
    assert_eq!(
        context
            .backend()
            .profiler
            .as_ref()
            .unwrap()
            .recorded_events_for_test_v1()
            .len(),
        event_count
    );
    MATERIALIZATION_RETENTION.with(|slot| slot.set(None));
    println!(
        "before_auxiliary={before:?} retained_materialization={observation:?} terminal_error={error:?}"
    );
    println!(
        "native_auxiliary_budget_failure=retained initialized_prefix={prefix} unpublished_auxiliary=confirmed retry=inert reclamation=process_exit_only"
    );
}
