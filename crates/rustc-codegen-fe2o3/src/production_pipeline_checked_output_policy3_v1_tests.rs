//! Constructed semantic fixtures, genuine backend-ranked admission and actual
//! native replay. These tests do not authenticate rustc or protected publication.
use super::*;
use crate::production_pipeline::ProductionPipelineError;
use crate::production_pipeline::checked_output_policy3_v1::{
    CheckedOutputStageErrorV1, PreparedCheckedOutputArtifactsV1,
    prepare_checked_output_artifacts_v1,
};
use crate::production_ranked_projection_v1::with_backend_checked_output_policy3_owned_v1;
use dialect_amdgcn::{
    NativeV12TextDescriptorReplayErrorV1 as ReplayError,
    check_native_v12_text_descriptor_relation_v1 as replay,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

#[test]
fn checked_native_handoff_owns_real_two_root_output_and_replays_both_targets() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        with_backend_checked_output_policy3_owned_v1(profile, |owner, budget| {
            let roots = typed_roots(&owner);
            let identity = *owner.output().canonical().identity().digest();
            let floor = budget.storage();
            let (artifacts, receipt) =
                prepare_checked_output_artifacts_v1(owner, profile, &roots, None, budget).unwrap();
            assert_eq!(budget.storage(), floor);
            budget.reserve_storage(receipt.retained_storage()).unwrap();
            assert_eq!(
                artifacts
                    .admitted()
                    .output()
                    .canonical()
                    .identity()
                    .digest(),
                &identity
            );
            assert!(std::ptr::eq(
                artifacts.admitted().output(),
                artifacts.admitted().checked_output().owner()
            ));
            assert_eq!(artifacts.descriptor_source().table().kernels().len(), 2);
            assert_eq!(artifacts.workgroup_sizes().len(), 2);
            for ((name, size), kernel) in artifacts
                .workgroup_sizes()
                .iter()
                .zip(&artifacts.admitted().output().module().kernels)
            {
                assert_eq!(name, kernel.id.as_str());
                assert_eq!(Some(*size), kernel.workgroup_size);
            }
            assert!(artifacts.catalog().definitions().is_empty());
            assert!(artifacts.catalog().bindings().is_empty());
            assert_eq!(
                artifacts.catalog().semantic_source(),
                artifacts
                    .admitted()
                    .source_semantic_kir()
                    .semantic()
                    .semantic()
                    .semantic_sha256()
                    .as_bytes()
            );
            assert!(!artifacts.grants_artifact_or_launch_authority());
            assert!(!artifacts.descriptor_source().grants_launch_authority());
            let relation = replay(
                artifacts.admitted().output(),
                artifacts.catalog(),
                artifacts.admitted().output().canonical().canonical_bytes(),
                profile,
                artifacts.descriptor_source().table(),
                artifacts.llvm_ir(),
                budget,
            )
            .unwrap();
            assert!(std::ptr::eq(
                relation.output(),
                artifacts.admitted().output()
            ));
            assert_eq!(relation.final_llvm(), artifacts.llvm_ir());
            assert!(!relation.grants_authority());
            // The borrowed header is discarded before another allocation.
            let _ = relation;
            drop(artifacts);
            budget.release_storage(receipt.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn checked_native_handoff_replay_refuses_changed_llvm_descriptor_profile_and_bytes() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_checked_output_policy3_owned_v1(profile, |owner, budget| {
        let roots = typed_roots(&owner);
        let floor = budget.storage();
        let (artifacts, receipt) =
            prepare_checked_output_artifacts_v1(owner, profile, &roots, None, budget).unwrap();
        budget.reserve_storage(receipt.retained_storage()).unwrap();
        let live = budget.storage();
        let output = artifacts.admitted().output();
        let bytes = output.canonical().canonical_bytes();
        let table = artifacts.descriptor_source().table();
        assert!(matches!(
            replay(
                output,
                artifacts.catalog(),
                bytes,
                ProductionAmdTargetProfileV1::Gfx950,
                table,
                artifacts.llvm_ir(),
                budget
            ),
            Err(ReplayError::Invalid("descriptor profile/COV6/zero digest"))
        ));
        assert_eq!(budget.storage(), live);
        assert!(matches!(
            replay(
                output,
                artifacts.catalog(),
                &bytes[..bytes.len() - 1],
                profile,
                table,
                artifacts.llvm_ir(),
                budget
            ),
            Err(ReplayError::OutputBytes)
        ));
        assert_eq!(budget.storage(), live);

        budget
            .reserve_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
            .unwrap();
        let mut changed = artifacts.llvm_ir().to_owned();
        changed.push('\n');
        assert!(matches!(
            replay(
                output,
                artifacts.catalog(),
                bytes,
                profile,
                table,
                &changed,
                budget
            ),
            Err(ReplayError::Invalid("complete native LLVM length"))
        ));
        drop(changed);
        budget
            .release_storage(dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES)
            .unwrap();
        assert_eq!(budget.storage(), live);

        budget
            .reserve_storage(fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES)
            .unwrap();
        let substituted = fe2o3_kernel_descriptor::DeviceDescriptorTableV1::new(
            table.canonical_code_object_digest(),
            table.code_object_version(),
            table.compiler().clone(),
            fe2o3_kernel_descriptor::ProducerIdentityV1::new(
                fe2o3_kernel_descriptor::Text::new("changed producer").unwrap(),
                fe2o3_kernel_descriptor::Text::new("changed version").unwrap(),
            ),
            table.device_target(),
            table.type_records().to_vec(),
            table.layout_records().to_vec(),
            table.kernels().to_vec(),
        )
        .unwrap();
        assert!(matches!(
            replay(
                output,
                artifacts.catalog(),
                bytes,
                profile,
                &substituted,
                artifacts.llvm_ir(),
                budget
            ),
            Err(ReplayError::Invalid("complete native LLVM length"))
                | Err(ReplayError::Invalid("exact native LLVM/descriptor text"))
        ));
        drop(substituted);
        budget
            .release_storage(fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES)
            .unwrap();
        assert_eq!(budget.storage(), live);
        drop(artifacts);
        budget.release_storage(receipt.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn checked_native_handoff_refuses_actual_target_owner_and_root_substitution() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_checked_output_policy3_v1(profile, |owner, budget| {
        let roots = typed_roots(owner);
        let floor = budget.storage();
        let (catalog, storage) =
            fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1::from_rows_with_budget(
                *owner
                    .source_semantic_kir()
                    .semantic()
                    .semantic()
                    .semantic_sha256()
                    .as_bytes(),
                &[],
                &[],
                budget,
            )
            .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        // The wrong target is checked against the actual owner's module before
        // LLVM or descriptor inspection; no foreign target metadata is injected.
        assert!(matches!(crate::production_worker_handoff::prepare_checked_output_policy3_worker_handoff(
            owner, &catalog, DeviceTargetV1::parse("gfx950:xnack-").unwrap(), String::new(), &roots, None, budget),
            Err(crate::production_worker_handoff::ProductionWorkerHandoffError::TargetBindingMismatch { .. })));
        drop(catalog);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
    });
    with_backend_checked_output_policy3_owned_v1(profile, |owner, budget| {
        let mut roots = typed_roots(&owner);
        roots[1].kernel_binding = roots[0].kernel_binding;
        let floor = budget.storage();
        assert!(matches!(
            prepare_checked_output_artifacts_v1(owner, profile, &roots, None, budget),
            Err(ProductionPipelineError::WorkerHandoff(
                crate::production_worker_handoff::ProductionWorkerHandoffError::CompilerDescriptor(
                    CompilerDescriptorError::ProductionDescriptorMismatch(
                        "ordered typed/source/output/formal root identity"
                    )
                )
            ))
        ));
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn checked_native_handoff_entry_work_and_input_floor_are_exact() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_checked_output_policy3_owned_v1(profile, |owner, original| {
        let roots = typed_roots(&owner);
        let floor = original.storage();
        let mut work = Work::new(3);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        let error = prepare_checked_output_artifacts_v1(owner, profile, &roots, None, &mut budget)
            .err()
            .unwrap();
        let ProductionPipelineError::CheckedOutputStage(CheckedOutputStageErrorV1::Resource(
            Resource::Work(error),
        )) = error
        else {
            panic!("expected exact entry work refusal");
        };
        assert_eq!(error.actual(), 4);
        assert_eq!(error.limit(), 3);
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
    });
    with_backend_checked_output_policy3_owned_v1(profile, |owner, _| {
        let roots = typed_roots(&owner);
        let floor = owner.retained_input_storage_floor_v1().unwrap() - 1;
        let mut work = Work::new(4);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            prepare_checked_output_artifacts_v1(owner, profile, &roots, None, &mut budget),
            Err(ProductionPipelineError::CheckedOutputStage(
                CheckedOutputStageErrorV1::Resource(Resource::Accounting)
            ))
        ));
        assert_eq!(budget.work(), 4);
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.peak_storage(), floor);
    });
}

#[test]
fn checked_native_handoff_engine_prepayment_one_short_restores_input_floor() {
    with_backend_checked_output_policy3_owned_v1(
        ProductionAmdTargetProfileV1::Gfx942,
        |owner, original| {
            let roots = typed_roots(&owner);
            let floor = original.storage();
            // Empty catalog framing: magic8 + versions4 + length4 + source32 + counts8.
            const CATALOG_WIRE: usize = 56;
            let catalog = std::mem::size_of::<
                fe2o3_kernel_ir::InertCanonicalKernelIrContractCatalogV1,
            >() + CATALOG_WIRE;
            let prepaid = 3 * dialect_amdgcn::MAX_COMPILER_MODULE_TEXT_BYTES
                + 2 * fe2o3_kernel_descriptor::MAX_DESCRIPTOR_TABLE_BYTES
                + std::mem::size_of::<PreparedCheckedOutputArtifactsV1>();
            let limit = floor + catalog + prepaid - 1;
            let mut work = Work::new(usize::MAX);
            let mut budget = Budget::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            let error = prepare_checked_output_artifacts_v1(
                owner,
                ProductionAmdTargetProfileV1::Gfx942,
                &roots,
                None,
                &mut budget,
            )
            .err()
            .unwrap();
            let ProductionPipelineError::CheckedOutputStage(CheckedOutputStageErrorV1::Resource(
                Resource::Storage(error),
            )) = error
            else {
                panic!("expected prepaid engine payload refusal");
            };
            assert_eq!(error.actual(), limit + 1);
            assert_eq!(error.limit(), limit);
            assert_eq!(budget.storage(), floor);
            assert_eq!(budget.peak_storage(), floor + catalog);
            assert_eq!(budget.failed_storage(), Some(limit + 1));
        },
    );
}
