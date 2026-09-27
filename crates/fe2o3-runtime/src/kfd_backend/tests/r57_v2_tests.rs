use super::*;
use crate::qualification_gfx942_r57_n3_v1::*;

#[test]
fn r57_v2_mixed_memory_rejection_precedes_authority_and_preserves_custody() {
    let byte_len = GFX942_R57_N3_QUALIFICATION_BUFFER_BYTES_V1 as u64;
    let host_steps = [
        ScriptedSdmaStepV1::Allocate {
            kind: ScriptedBufferKindV1::Host,
            byte_len: byte_len as usize,
        },
        ScriptedSdmaStepV1::Write {
            offset: 0,
            byte_len: byte_len as usize,
        },
        ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
    ];
    let device_release_steps = (0..3).flat_map(|_| {
        [
            ScriptedSdmaStepV1::Demote(ScriptedFailureModeV1::Success),
            ScriptedSdmaStepV1::Recycle(ScriptedRecycleOutcomeV1::Success),
        ]
    });
    let (context, stream, allocations, backend_allocations) =
        scripted_three_binding_context_with_steps_v1(
            byte_len,
            host_steps.into_iter().chain(device_release_steps),
        );
    let mut context = core::mem::ManuallyDrop::new(context);
    let admitted = admit_gfx942_r57_n3_qualification_v2().unwrap();
    let observation = admitted.observation_v1();
    context.backend_mut_for_test_v1().launch_gate =
        KfdRuntimeLaunchGateV1::ExactGfx942R57N3V2(admitted);
    let device = context.devices()[0].id();
    let module = context
        .load_module(device, gfx942_r57_n3_qualification_hsaco_v1())
        .unwrap();
    let kernel = context
        .resolve_kernel::<Gfx942R57N3QualificationArgumentsV2>(
            module,
            GFX942_R57_N3_QUALIFICATION_KERNEL_V1,
        )
        .unwrap();
    let upload = context
        .allocate(device, RuntimeMemoryKindV1::HostVisible, byte_len, 8)
        .unwrap();
    let arguments =
        Gfx942R57N3QualificationArgumentsV2::new(allocations[0], allocations[1], upload).unwrap();
    let before = three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations);
    let next = context.backend().next_handle;
    let steps = context
        .backend()
        .scripted_sdma
        .as_ref()
        .unwrap()
        .remaining_steps();
    for _ in 0..2 {
        let result = context.launch(
            stream,
            &kernel,
            &arguments,
            GFX942_R57_N3_QUALIFICATION_GEOMETRY_V1,
            &[],
        );
        assert!(
            matches!(&result,
            Err(crate::RuntimeErrorV1::BackendRejected(error))
                if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
                    && error.detail() == "three-binding persistent-compute candidate failed exact R/R/W admission"),
            "error: {:?}",
            result.as_ref().err()
        );
        assert_eq!(observation.authorization_calls_v1(), 0);
        assert_eq!(context.backend().next_handle, next);
        assert_eq!(
            three_binding_prelaunch_snapshot_v1(context.backend(), backend_allocations),
            before
        );
        assert_runtime_compute_pipeline_empty_v1(context.backend());
        assert!(context.backend().allocation_custody.is_empty());
        assert_eq!(context.backend().compute_completion_reservations, 0);
        let driver = context.backend().scripted_sdma.as_ref().unwrap();
        assert_eq!(driver.remaining_steps(), steps);
        assert_eq!(driver.live_owner_count(), 4);
        assert_eq!(driver.unexpected_drops(), 0);
    }
    context.release_allocation(upload).unwrap();
    context.unload_module(module).unwrap();
    for record in context.backend_mut_for_test_v1().allocations.values_mut() {
        record.sdma_backed = false;
    }
    for allocation in allocations {
        context.release_allocation(allocation).unwrap();
    }
    context.destroy_stream(stream).unwrap();
    let mut backend = core::mem::ManuallyDrop::into_inner(context)
        .shutdown()
        .unwrap();
    let driver = backend.scripted_sdma.as_ref().unwrap();
    assert_eq!(driver.live_owner_count(), 0);
    assert_eq!(driver.unexpected_drops(), 0);
    assert!(driver.is_exhausted());
    backend.shutdown_native_v1().unwrap();
}
