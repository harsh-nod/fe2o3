fn descriptor_resource_assertions_completed(require_observer: bool) {
    assert!(
        DESCRIPTOR_RUN_COMPLETED.get(),
        "caller-floor assertions must complete outside production catches"
    );
    assert_eq!(
        DESCRIPTOR_ASSERTIONS_STARTED.get(),
        DESCRIPTOR_ASSERTIONS_COMPLETED.get(),
        "every entered postflight assertion group must complete without a caught panic"
    );
    if require_observer {
        assert!(DESCRIPTOR_ASSERTIONS_COMPLETED.get() > 0);
    }
}

fn descriptor_resource_error(error: ScopedModuleErrorV29, is_work: bool) {
    let resource = match error {
        ScopedModuleErrorV29::Source(
            ProductionSemanticKirErrorV1::ArgumentCorrespondenceResource(error),
        )
        | ScopedModuleErrorV29::Source(ProductionSemanticKirErrorV1::AssertOrigin(
            SemanticKirAssertOriginErrorV1::Resource(error),
        ))
        | ScopedModuleErrorV29::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Resource(error),
        )
        | ScopedModuleErrorV29::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Decode(
                fe2o3_kernel_ir::KernelIrDecodeError::Resource(error),
            ),
        )
        | ScopedModuleErrorV29::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Layout(
                fe2o3_kernel_ir::StorageLayoutErrorV1::Resource(error),
            ),
        )
        | ScopedModuleErrorV29::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Verification(
                fe2o3_kernel_ir::BorrowedKernelIrVerificationErrorV1::Resource(error),
            ),
        ) => error,
        ScopedModuleErrorV29::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Encode(
                fe2o3_kernel_ir::KernelIrEncodeError::WorkLimit(limit),
            ),
        )
        | ScopedModuleErrorV29::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Decode(
                fe2o3_kernel_ir::KernelIrDecodeError::WorkLimit(limit),
            ),
        )
        | ScopedModuleErrorV29::Canonical(
            fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Decode(
                fe2o3_kernel_ir::KernelIrDecodeError::Encode(
                    fe2o3_kernel_ir::KernelIrEncodeError::WorkLimit(limit),
                ),
            ),
        ) => ArgumentResourceV1::Work(limit),
        other => panic!("expected a typed descriptor resource refusal: {other:?}"),
    };
    assert!(
        matches!(
            (&resource, is_work),
            (ArgumentResourceV1::Work(_), true) | (ArgumentResourceV1::Storage(_), false)
        ),
        "wrong descriptor resource boundary: {resource:?}, work={is_work}"
    );
}

#[test]
fn descriptor_module_and_replay_preserve_exact_and_one_short_work_storage() {
    for case in [
        DescriptorCase::READ,
        DescriptorCase {
            write: true,
            explicit: true,
            ..DescriptorCase::READ
        },
    ] {
        let (result, work, peak) =
            run_descriptor_module(case, DescriptorFault::None, MODULE_LIMIT, MODULE_LIMIT);
        result.unwrap();
        descriptor_resource_assertions_completed(true);
        assert!(work > 0 && peak > MODULE_FLOOR);
        run_descriptor_module(case, DescriptorFault::None, work, peak)
            .0
            .unwrap();
        descriptor_resource_assertions_completed(true);
        let work_error = run_descriptor_module(case, DescriptorFault::None, work - 1, peak)
            .0
            .unwrap_err();
        descriptor_resource_assertions_completed(false);
        descriptor_resource_error(work_error, true);
        let storage_error = run_descriptor_module(case, DescriptorFault::None, work, peak - 1)
            .0
            .unwrap_err();
        descriptor_resource_assertions_completed(false);
        descriptor_resource_error(storage_error, false);
    }
}
