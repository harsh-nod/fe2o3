//! Genuine semantic source plus the normal projector and actual native replay.
//! No collected-Rust, signed-source-proof, protected or runtime admission claim.
#[path = "production_pipeline_erased_progress_neutrality_v1_tests.rs"]
mod progress_neutrality;
use super::*;
use crate::production_pipeline::erased_checked_output_policy4_v1::prepare_erased_checked_output_artifacts_v1;
use crate::production_ranked_projection_v1::with_backend_erased_bound_v1;
use dialect_amdgcn::{
    NativeV12TextDescriptorReplayErrorV1 as ReplayError,
    check_native_v12_text_descriptor_relation_v1 as replay,
};
use fe2o3_artifacts::RustPointerMutabilityV1;
use fe2o3_kernel_ir::{
    AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrWorkBudgetV1 as Work, Constant, OperationKind,
    VerifiedCanonicalKernelIrModuleV12 as Verified,
};
use fe2o3_lower_mir_kernel::{
    ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1 as Admitted,
    ProductionUnitLocalErasedSourceOwnerV1 as Erased,
};

pub(super) fn erased_typed_roots(source: &Erased) -> Vec<TypedDescriptorRootV1> {
    let semantic = source.original_source().semantic_ssa().source_semantic();
    semantic
        .roots()
        .iter()
        .enumerate()
        .map(|(ordinal, id)| {
            let function = &semantic.functions()[id.index() as usize];
            let entry = function.kernel_entry().unwrap();
            let ty = function.abi().source_input_types()[0];
            let layout = RustLayoutEvidenceV1::new(
                RustTypeEvidenceV1::new(RustSourceTypeShapeV1::global_mut_pointer(
                    RustScalarElementTypeV1::U32,
                )),
                RustcAbiClassV1::Scalar,
                PointerWidth::Bits64,
                8,
                8,
                vec![
                    RustPhysicalComponentV1::new(
                        0,
                        8,
                        8,
                        RustPhysicalComponentKindV1::Pointer {
                            mutability: RustPointerMutabilityV1::Mut,
                            pointee: RustScalarElementTypeV1::U32,
                        },
                    )
                    .unwrap(),
                ],
            )
            .unwrap();
            TypedDescriptorRootV1 {
                logical_name: if ordinal == 0 {
                    "canonical_assertion_root"
                } else {
                    "erased_second_root"
                }
                .to_owned(),
                export_name: String::from_utf8(entry.export_symbol().as_bytes().to_vec()).unwrap(),
                kernel_binding: KernelBindingIdV1::from_bytes(
                    *entry.kernel_binding_identity().as_bytes(),
                ),
                arguments: TypedArgumentListV1::new(vec![TypedDescriptorArgumentV1 {
                    name: "output".to_owned(),
                    kind: DescriptorArgumentKindV1::GlobalMutPointer(ScalarTypeV1::U32),
                    access: AccessMode::ReadWrite,
                    offset: 0,
                    layout: Some(layout),
                    source_size: 8,
                    source_alignment: 8,
                    rustc_abi_class: RustcAbiClassV1::Scalar,
                    semantic_type_identity: semantic.types()[ty.index() as usize].identity(),
                }])
                .unwrap(),
                explicit_argument_bytes: 8,
                kernarg_alignment_bytes: 8,
                source_launch: Some(
                    LaunchContract::new(
                        1,
                        BlockSize::Exact(Dimensions::new(1, 1, 1).unwrap()),
                        Dimensions::new(1, 1, 1).unwrap(),
                        0,
                        0,
                    )
                    .unwrap(),
                ),
            }
        })
        .collect()
}

fn erased_memory_counts(owner: &Verified) -> (usize, usize, usize) {
    let mut private_stores = 0;
    let mut private_loads = 0;
    let mut global_stores = 0;
    for operation in owner
        .module()
        .functions
        .iter()
        .filter_map(|f| f.body.as_ref())
        .flat_map(|body| &body.blocks)
        .flat_map(|block| &block.operations)
    {
        match &operation.kind {
            OperationKind::Store { access, .. }
                if access.address_space == AddressSpace::Private =>
            {
                private_stores += 1
            }
            OperationKind::Load { access, .. } if access.address_space == AddressSpace::Private => {
                private_loads += 1
            }
            OperationKind::Store { access, .. } if access.address_space == AddressSpace::Global => {
                global_stores += 1
            }
            _ => {}
        }
    }
    (private_stores, private_loads, global_stores)
}

#[test]
fn erased_backend_real_projector_reaches_policy4_actual_o_and_native_replay() {
    for profile in [
        ProductionAmdTargetProfileV1::Gfx942,
        ProductionAmdTargetProfileV1::Gfx950,
    ] {
        for expected in [false, true] {
            for root_count in [1, 2] {
                with_backend_erased_bound_v1(
                    expected,
                    root_count,
                    profile,
                    |source, bound, budget| {
                        assert_eq!(
                            erased_memory_counts(source.original_source().executable()),
                            (9 + 2 * root_count, root_count, root_count)
                        );
                        assert_eq!(
                            erased_memory_counts(source.erased()),
                            (2 * root_count, root_count, root_count)
                        );
                        let branches = source
                            .erased()
                            .module()
                            .functions
                            .iter()
                            .filter_map(|f| f.body.as_ref())
                            .flat_map(|body| &body.blocks)
                            .filter(|block| {
                                matches!(
                                    block.terminator,
                                    Some(fe2o3_kernel_ir::Terminator::ConditionalBranch { .. })
                                )
                            })
                            .count();
                        assert_eq!(branches, root_count);
                        let typed = erased_typed_roots(&source);
                        let original_identity =
                            *source.original_source().executable().canonical().identity();
                        let erased_identity = *source.erased().canonical().identity();
                        let checked =
                            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(
                                &bound, budget,
                            )
                            .unwrap();
                        let checked_storage = checked.retained_storage();
                        budget.reserve_storage(checked_storage).unwrap();
                        assert_eq!(checked.intermediate_policy3().report().passes().len(), 8);
                        assert_eq!(checked.execution().policy_version(), 4);
                        assert_eq!(checked.forwarding_rows().len(), root_count);
                        assert_eq!(
                            erased_memory_counts(checked.owner()),
                            (2 * root_count, 0, root_count)
                        );
                        let admitted =
                            Admitted::try_admit_v1(source, bound, checked, budget).unwrap();
                        admitted.verify_equivalence(budget).unwrap();
                        let output_identity = *admitted.output().canonical().identity();
                        assert_eq!(admitted.kernels().len(), root_count);
                        let floor = budget.storage();
                        let (artifacts, storage) = prepare_erased_checked_output_artifacts_v1(
                            admitted, profile, &typed, None, budget,
                        )
                        .unwrap();
                        assert_eq!(budget.storage(), floor);
                        budget.reserve_storage(storage.retained_storage()).unwrap();
                        let owner = artifacts.admitted();
                        assert_eq!(
                            *owner.original_source().executable().canonical().identity(),
                            original_identity
                        );
                        assert_eq!(*owner.erased().canonical().identity(), erased_identity);
                        assert_eq!(*owner.output().canonical().identity(), output_identity);
                        assert!(std::ptr::eq(owner.output(), owner.checked_output().owner()));
                        assert_eq!(
                            artifacts.descriptor_source().table().kernels().len(),
                            root_count
                        );
                        assert_eq!(artifacts.workgroup_sizes().len(), root_count);
                        assert!(artifacts.catalog().definitions().is_empty());
                        assert!(artifacts.catalog().bindings().is_empty());
                        assert_eq!(
                            artifacts.catalog().semantic_source(),
                            owner
                                .original_source()
                                .semantic_ssa()
                                .source_semantic()
                                .semantic_sha256()
                                .as_bytes()
                        );
                        assert!(!artifacts.grants_artifact_or_launch_authority());
                        assert!(!artifacts.descriptor_source().grants_launch_authority());
                        {
                            let live = replay(
                                owner.output(),
                                artifacts.catalog(),
                                owner.output().canonical().canonical_bytes(),
                                profile,
                                artifacts.descriptor_source().table(),
                                artifacts.llvm_ir(),
                                budget,
                            )
                            .unwrap();
                            assert!(std::ptr::eq(live.output(), owner.output()));
                            assert_eq!(live.final_llvm(), artifacts.llvm_ir());
                            assert!(!live.grants_authority());
                        }
                        drop(artifacts);
                        budget.release_storage(storage.retained_storage()).unwrap();
                        assert_eq!(budget.storage(), floor);
                        budget.release_storage(checked_storage).unwrap();
                    },
                );
            }
        }
    }
}

#[test]
fn erased_backend_rejects_a_verified_and_independently_optimized_changed_b() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_erased_bound_v1(true, 1, profile, |source, bound, budget| {
        let entry = budget.storage();
        let (mut candidate, copy_storage) =
            bound.copy_module_for_transformation_v12(budget).unwrap();
        budget
            .reserve_storage(copy_storage.retained_storage())
            .unwrap();
        let operation = candidate
            .functions
            .iter_mut()
            .filter_map(|f| f.body.as_mut())
            .flat_map(|body| &mut body.blocks)
            .flat_map(|block| &mut block.operations)
            .find(|op| matches!(op.kind, OperationKind::Constant(Constant::U32(7))))
            .unwrap();
        operation.kind = OperationKind::Constant(Constant::U32(6));
        let (changed, changed_storage) =
            Verified::from_module_ref_with_verification_budget_v12(&candidate, budget).unwrap();
        budget
            .reserve_storage(changed_storage.retained_storage())
            .unwrap();
        drop(candidate);
        budget
            .release_storage(copy_storage.retained_storage())
            .unwrap();
        assert_ne!(
            changed.canonical().canonical_bytes(),
            bound.canonical().canonical_bytes()
        );
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&changed, budget)
                .unwrap();
        let checked_storage = checked.retained_storage();
        budget.reserve_storage(checked_storage).unwrap();
        let live = budget.storage();
        assert!(Admitted::try_admit_v1(source, changed, checked, budget).is_err());
        assert_eq!(budget.storage(), live);
        budget
            .release_storage(checked_storage + changed_storage.retained_storage())
            .unwrap();
        drop(bound);
        assert_eq!(budget.storage(), entry);
    });
}

#[test]
fn erased_backend_native_replay_rejects_output_bytes_profile_and_changed_llvm() {
    let profile = ProductionAmdTargetProfileV1::Gfx942;
    with_backend_erased_bound_v1(false, 2, profile, |source, bound, budget| {
        let typed = erased_typed_roots(&source);
        let checked =
            fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, budget)
                .unwrap();
        let checked_storage = checked.retained_storage();
        budget.reserve_storage(checked_storage).unwrap();
        let admitted = Admitted::try_admit_v1(source, bound, checked, budget).unwrap();
        let floor = budget.storage();
        let (artifacts, storage) =
            prepare_erased_checked_output_artifacts_v1(admitted, profile, &typed, None, budget)
                .unwrap();
        budget.reserve_storage(storage.retained_storage()).unwrap();
        let live = budget.storage();
        let output = artifacts.admitted().output();
        let bytes = output.canonical().canonical_bytes();
        assert!(matches!(
            replay(
                output,
                artifacts.catalog(),
                &bytes[..bytes.len() - 1],
                profile,
                artifacts.descriptor_source().table(),
                artifacts.llvm_ir(),
                budget
            ),
            Err(ReplayError::OutputBytes)
        ));
        assert_eq!(budget.storage(), live);
        assert!(
            replay(
                output,
                artifacts.catalog(),
                bytes,
                ProductionAmdTargetProfileV1::Gfx950,
                artifacts.descriptor_source().table(),
                artifacts.llvm_ir(),
                budget
            )
            .is_err()
        );
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
                artifacts.descriptor_source().table(),
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
        let mut work = Work::new(0);
        let mut empty = Budget::new(&mut work, live);
        empty.reserve_storage(live).unwrap();
        assert!(
            replay(
                output,
                artifacts.catalog(),
                bytes,
                profile,
                artifacts.descriptor_source().table(),
                artifacts.llvm_ir(),
                &mut empty
            )
            .is_err()
        );
        assert_eq!(empty.storage(), live);
        assert!(work.failed_work().is_some());
        drop(artifacts);
        budget.release_storage(storage.retained_storage()).unwrap();
        assert_eq!(budget.storage(), floor);
        budget.release_storage(checked_storage).unwrap();
    });
}
