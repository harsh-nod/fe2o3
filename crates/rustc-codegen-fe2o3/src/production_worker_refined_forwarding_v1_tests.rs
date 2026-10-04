//! Unsigned component controls, not signed source or protected worker evidence.
use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::refined_forwarding_v1::producer_version_v1;

#[test]
fn final_worker_family_labels_are_closed_and_not_numbered_policies() {
    assert_eq!(
        producer_version_v1(Profile::Gfx942),
        "production-refined-forwarding-checked-gfx942-cov6-v1"
    );
    assert_eq!(
        producer_version_v1(Profile::Gfx950),
        "production-refined-forwarding-checked-gfx950-cov6-v1"
    );
    for profile in [Profile::Gfx942, Profile::Gfx950] {
        assert_ne!(
            producer_version_v1(profile),
            crate::compiler_descriptor::checked_output_policy3_v1::policy8::producer_version(
                profile
            )
        );
    }
}

#[test]
fn final_worker_assembly_preserves_typed_error_sources() {
    let value = FinalWorkerAssemblyErrorV1::Handoff(Box::new(
        ProductionWorkerHandoffError::MissingProductionBindings,
    ));
    assert!(value.source().unwrap().is::<ProductionWorkerHandoffError>());
    let value = FinalWorkerAssemblyErrorV1::Descriptor(Box::new(
        RefinedForwardingDescriptorErrorV1::Resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Accounting,
        ),
    ));
    let source = value.source().unwrap();
    assert!(source.is::<RefinedForwardingDescriptorErrorV1>());
    assert!(
        source
            .source()
            .unwrap()
            .is::<fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1>()
    );
}

pub(crate) fn assert_bound_output(
    owner: FinalOwnerV1<'_>,
    catalog: &Catalog,
    profile: Profile,
    prepared: &PreparedProductionWorkerHandoff,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    replay_v1(owner, catalog, profile, prepared, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    assert_eq!(
        prepared
            .compiler_descriptor_source
            .table()
            .producer()
            .version()
            .as_str(),
        producer_version_v1(profile)
    );
    assert_eq!(
        prepared
            .compiler_descriptor_source
            .table()
            .producer()
            .name()
            .as_str(),
        fe2o3_kernel_descriptor::RUSTC_CODEGEN_FE2O3_PRODUCTION_V3_PRODUCER_NAME_V1
    );
    assert_eq!(
        prepared.compiler_descriptor_source.table().kernels().len(),
        owner.output().module().kernels.len()
    );
    assert_eq!(prepared.handoff.kind(), CompilerModuleKindV1::LlvmTextIr);
    assert_eq!(
        prepared.handoff.code_object_version(),
        CodeObjectVersion::V6
    );
    assert_eq!(
        prepared.handoff.target(),
        DeviceTargetV1::parse(profile.device_target()).unwrap()
    );
    assert_eq!(
        Sha256::digest(prepared.handoff.module_bytes()).as_slice(),
        prepared.llvm_ir_sha256
    );
}

pub(crate) fn assert_changed_digest_and_profile_refuse(
    owner: FinalOwnerV1<'_>,
    catalog: &Catalog,
    profile: Profile,
    prepared: &mut PreparedProductionWorkerHandoff,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    prepared.llvm_ir_sha256[0] ^= 1;
    let result = replay_v1(owner, catalog, profile, prepared, budget);
    prepared.llvm_ir_sha256[0] ^= 1;
    let error = result.unwrap_err();
    assert!(matches!(error, FinalWorkerAssemblyErrorV1::Handoff(error)
        if matches!(*error, ProductionWorkerHandoffError::MissingProductionBindings)));
    let other = match profile {
        Profile::Gfx942 => Profile::Gfx950,
        Profile::Gfx950 => Profile::Gfx942,
    };
    let error = replay_v1(owner, catalog, other, prepared, budget).unwrap_err();
    assert!(matches!(error, FinalWorkerAssemblyErrorV1::Handoff(error)
        if matches!(*error, ProductionWorkerHandoffError::MissingProductionBindings)));
    assert_eq!(budget.storage(), floor);
    assert_bound_output(owner, catalog, profile, prepared, budget);
}

pub(crate) fn assert_changed_text_refuses(
    owner: FinalOwnerV1<'_>,
    catalog: &Catalog,
    profile: Profile,
    prepared: &mut PreparedProductionWorkerHandoff,
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let mut changed = prepared.handoff.module_bytes().to_vec();
    changed.extend_from_slice(b"\n; not the independently reproduced final module\n");
    let replacement = CompilerModuleHandoffV2::new(
        prepared.handoff.kind(),
        prepared.handoff.target(),
        prepared.handoff.code_object_version(),
        prepared.handoff.envelope().clone(),
        prepared.handoff.symbol_manifest().clone(),
        &changed,
    )
    .unwrap();
    let old = std::mem::replace(&mut prepared.handoff, replacement);
    let digest = prepared.llvm_ir_sha256;
    prepared.llvm_ir_sha256 = Sha256::digest(&changed).into();
    let result = replay_v1(owner, catalog, profile, prepared, budget);
    prepared.handoff = old;
    prepared.llvm_ir_sha256 = digest;
    let error = result.unwrap_err();
    assert!(matches!(error, FinalWorkerAssemblyErrorV1::Handoff(error)
        if matches!(*error, ProductionWorkerHandoffError::NativeOutputReplay(_))));
    assert_eq!(budget.storage(), floor);
    assert_bound_output(owner, catalog, profile, prepared, budget);
}
