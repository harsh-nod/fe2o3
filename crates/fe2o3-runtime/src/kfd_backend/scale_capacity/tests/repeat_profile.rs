//! Actual preparation gate integration without a native queue or receipt.

use super::*;
use crate::qualification_gfx942_vecadd_repeat_v1::{
    GFX942_VECADD_REPEAT_QUALIFICATION_SIGNATURE_V1, admit_gfx942_vecadd_repeat_qualification_v1,
};
use crate::qualification_gfx942_vecadd_v1::{
    GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1, GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1,
    GFX942_VECADD_QUALIFICATION_SIGNATURE_V1, admit_gfx942_vecadd_qualification_v1,
    gfx942_vecadd_qualification_bindings_v1,
};

#[test]
fn repeated_output_preparation_requires_its_own_gate_signature_and_exact_inputs() {
    for truthful_completed_output in [false, true] {
        let account = account(64 * 1024 * 1024, 32);
        let mut backend = backend(account.clone());
        let admitted = admit_gfx942_vecadd_repeat_qualification_v1().unwrap();
        let buffers = admitted.host_buffers().unwrap();
        let stream = backend.create_stream_v1(7).unwrap();
        let module = backend.load_module_v1(7, admitted.hsaco()).unwrap();
        let repeat_kernel = backend
            .resolve_kernel_v1(
                module,
                admitted.kernel_name(),
                GFX942_VECADD_REPEAT_QUALIFICATION_SIGNATURE_V1,
            )
            .unwrap();
        let original_kernel = backend
            .resolve_kernel_v1(
                module,
                admitted.kernel_name(),
                GFX942_VECADD_QUALIFICATION_SIGNATURE_V1,
            )
            .unwrap();
        let allocations = [buffers.left(), buffers.right(), buffers.output()].map(|bytes| {
            let allocation = backend
                .allocate_v1(
                    7,
                    RuntimeMemoryKindV1::HostVisible,
                    GFX942_VECADD_QUALIFICATION_BUFFER_BYTES_V1 as u64,
                    GFX942_VECADD_QUALIFICATION_BUFFER_ALIGNMENT_V1,
                )
                .unwrap();
            backend.write_allocation_v1(allocation, 0, bytes).unwrap();
            allocation
        });
        let mut launch = OwnedComputeLaunchV1 {
            stream,
            kernel: repeat_kernel,
            explicit_kernarg: admitted.explicit_kernarg().into(),
            bindings: gfx942_vecadd_qualification_bindings_v1(allocations)
                .unwrap()
                .into(),
            geometry: admitted.geometry(),
            semantic_launch: KfdRuntimeSemanticLaunchV1::Ordinary,
        };
        let denied = |backend: &mut KfdRuntimeBackendV1, launch: &OwnedComputeLaunchV1| {
            for reuse_bound_recipe in [false, true] {
                assert!(matches!(
                    backend.prepare_launch(launch.borrowed(), false, reuse_bound_recipe),
                    Err(RuntimeBackendFailureV1::Rejected(error))
                        if error.kind() == KfdRuntimeBackendErrorKindV1::Unsupported
                            && error.detail() == "direct KFD launch authority denied the exact invocation"
                ));
            }
        };
        // Even sentinel output cannot cross the original gate's signature.
        denied(&mut backend, &launch);
        backend.launch_gate = KfdRuntimeLaunchGateV1::ExactGfx942VecaddRepeat(admitted);
        for reuse_bound_recipe in [false, true] {
            backend
                .prepare_launch(launch.borrowed(), false, reuse_bound_recipe)
                .unwrap();
        }
        if truthful_completed_output {
            backend
                .write_allocation_v1(allocations[2], 0, buffers.expected_output())
                .unwrap();
        } else {
            // Model only logical writeback's metadata invalidation. There was
            // no native submission, publication, completion or DATA authority.
            backend
                .allocations
                .get_mut(&allocations[2])
                .unwrap()
                .content_sha256 = None;
        }
        let before = account.usage();
        for reuse_bound_recipe in [false, true] {
            backend
                .prepare_launch(launch.borrowed(), false, reuse_bound_recipe)
                .unwrap();
        }
        launch.kernel = original_kernel;
        denied(&mut backend, &launch);
        launch.kernel = repeat_kernel;
        for allocation in &allocations[..2] {
            let original = backend.allocations[allocation].content_sha256;
            for changed in [None, Some([0; 32])] {
                backend
                    .allocations
                    .get_mut(allocation)
                    .unwrap()
                    .content_sha256 = changed;
                denied(&mut backend, &launch);
            }
            backend
                .allocations
                .get_mut(allocation)
                .unwrap()
                .content_sha256 = original;
        }
        backend.launch_gate = KfdRuntimeLaunchGateV1::ExactGfx942Vecadd(
            admit_gfx942_vecadd_qualification_v1().unwrap(),
        );
        launch.kernel = original_kernel;
        denied(&mut backend, &launch);
        assert_eq!(account.usage(), before);
        assert!(!backend.native_available && !backend.terminal);
        assert!(
            backend.queue.is_none()
                && backend.admitted_device.is_none()
                && backend.terminal_memory.is_none()
        );
        assert!(backend.native_compute_lanes.iter().all(Option::is_none));
        assert!(backend.submissions.is_empty() && backend.pending_compute.is_empty());
        assert!(backend.active.is_none() && backend.compute_pipeline.is_empty());
        assert!(
            backend.allocation_custody.is_empty()
                && backend.compute_module_retain_counts.is_empty()
        );
        assert_eq!(backend.compute_completion_reservations, 0);
        assert!(allocations.iter().all(|allocation| matches!(
            backend.allocations[allocation].sdma_storage,
            KfdRuntimeSdmaStorageV1::Synthetic
        )));
        for allocation in allocations {
            backend.release_allocation_v1(allocation).unwrap();
        }
        backend.unload_module_v1(module).unwrap();
        backend.destroy_stream_v1(stream).unwrap();
        backend.shutdown_native_v1().unwrap();
        drop(backend);
        assert_eq!(account.usage().used, ResourceVectorV1::ZERO);
        assert_eq!(account.usage().retained_records, 0);
    }
}

#[test]
fn repeat_constructor_rejects_zero_identity_or_bad_capacity_before_kfd_open() {
    assert!(matches!(
        KfdRuntimeBackendV1::open_gfx942_vecadd_repeat_scale_qualification_v1(0, u64::MAX, 32),
        Err(error) if error.kind() == KfdRuntimeBackendErrorKindV1::InvalidLaunch
    ));
    for (bytes, records) in [(0, 2), (u64::MAX, 0), (pipeline_bytes(), 2)] {
        assert!(matches!(
            KfdRuntimeBackendV1::open_gfx942_vecadd_repeat_scale_qualification_v1(1, bytes, records),
            Err(error) if error.kind() == KfdRuntimeBackendErrorKindV1::Capacity
        ));
    }
}
