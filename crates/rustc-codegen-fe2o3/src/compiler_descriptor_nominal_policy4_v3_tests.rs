//! Genuine P4 owners; these tests do not claim rustc capture or native execution.
use super::super::nominal_policy4_v3::{self as nominal, OwnerRef};
use super::*;
use crate::compiler_descriptor::nominal_v3::{self as codec, NominalDescriptorErrorV3 as E};
use crate::production_ranked_projection_v1::{
    with_backend_checked_output_policy4_v1, with_backend_erased_bound_v1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};
use std::mem::size_of;

const PROFILES: [ProductionAmdTargetProfileV1; 2] = [
    ProductionAmdTargetProfileV1::Gfx942,
    ProductionAmdTargetProfileV1::Gfx950,
];

fn scalar64_roots(
    source: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
    kind: DescriptorArgumentKindV1,
    signed: bool,
) -> Vec<TypedDescriptorRootV1> {
    let mut roots = typed_roots_for_source(source);
    for root in &mut roots {
        let mut args = root.arguments.as_slice().to_vec();
        args[0].kind = kind;
        args[0].source_size = 8;
        args[0].source_alignment = 8;
        args[0].layout = if matches!(kind, DescriptorArgumentKindV1::Scalar(_)) {
            let scalar = if signed {
                RustScalarElementTypeV1::I64
            } else {
                RustScalarElementTypeV1::U64
            };
            Some(
                RustLayoutEvidenceV1::new(
                    RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(scalar)),
                    RustcAbiClassV1::Scalar,
                    PointerWidth::Bits64,
                    8,
                    8,
                    vec![
                        RustPhysicalComponentV1::new(
                            0,
                            8,
                            8,
                            RustPhysicalComponentKindV1::Scalar { scalar },
                        )
                        .unwrap(),
                    ],
                )
                .unwrap(),
            )
        } else {
            None
        };
        root.arguments = TypedArgumentListV1::new(args).unwrap();
        root.explicit_argument_bytes = 8;
    }
    roots
}

fn scalar32_roots(
    source: &fe2o3_lower_mir_kernel::ProductionSemanticKirOwnerV1,
) -> Vec<TypedDescriptorRootV1> {
    let mut roots = typed_roots_for_source(source);
    // V3 requires the exact argument-derived alignment, not V1's over-alignment.
    for root in &mut roots {
        root.kernarg_alignment_bytes = 4;
    }
    roots
}

#[test]
fn nominal_p4_real_nominal_and_fixed_scalar_sources_cannot_substitute_each_other() {
    use fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1 as Kind;
    for profile in PROFILES {
        for (source_kind, signed, typed, wrong) in [
            (
                Kind::Usize,
                false,
                DescriptorArgumentKindV1::CompilerLaidOutUsize,
                DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U64),
            ),
            (
                Kind::Isize,
                true,
                DescriptorArgumentKindV1::CompilerLaidOutIsize,
                DescriptorArgumentKindV1::Scalar(ScalarTypeV1::I64),
            ),
            (
                Kind::Ordinary,
                false,
                DescriptorArgumentKindV1::Scalar(ScalarTypeV1::U64),
                DescriptorArgumentKindV1::CompilerLaidOutUsize,
            ),
            (
                Kind::Ordinary,
                true,
                DescriptorArgumentKindV1::Scalar(ScalarTypeV1::I64),
                DescriptorArgumentKindV1::CompilerLaidOutIsize,
            ),
        ] {
            crate::production_ranked_projection_v1::with_backend_nominal_policy4_owned_v3(
                profile,
                source_kind,
                signed,
                |owner, budget| {
                    let roots = scalar64_roots(owner.source_semantic_kir(), typed, signed);
                    exercise(
                        OwnerRef::Direct(&owner),
                        &roots,
                        profile,
                        owner.output().canonical().canonical_bytes(),
                        owner
                            .source_semantic_kir()
                            .semantic()
                            .semantic()
                            .canonical_encoding(),
                        budget,
                    );
                    let floor = budget.storage();
                    let wrong = scalar64_roots(owner.source_semantic_kir(), wrong, signed);
                    assert!(matches!(
                        nominal::produce_for_profile(
                            OwnerRef::Direct(&owner),
                            &wrong,
                            profile,
                            64,
                            budget
                        ),
                        Err(E::Mismatch("actual rustc nominal kind"))
                    ));
                    assert_eq!(budget.storage(), floor);
                },
            );
        }
    }
}

#[test]
fn nominal_p4_erased_rejects_e_and_c_evidence_before_callback() {
    for profile in PROFILES {
        with_backend_erased_bound_v1(true, 2, profile, |source, bound, budget| {
            let roots = erased_native_handoff_tests::erased_typed_roots(&source);
            let checked =
                fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(&bound, budget)
                    .unwrap();
            let retained = checked.retained_storage();
            budget.reserve_storage(retained).unwrap();
            let owner = fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1
                ::try_admit_v1(source, bound, checked, budget).unwrap();
            let floor = budget.storage();
            for prior in [
                owner.erased(),
                owner.checked_output().intermediate_policy3().owner(),
            ] {
                assert_ne!(
                    prior.canonical().canonical_bytes(),
                    owner.output().canonical().canonical_bytes()
                );
                let bytes = codec::encode_subject(
                    &roots,
                    owner.original_source().semantic_ssa().source_semantic(),
                    prior.module(),
                    prior.canonical().canonical_bytes(),
                    owner.kernels().len(),
                    profile,
                    64,
                    nominal::OUTPUT_DOMAIN,
                    "inert-nominal-policy4-output-v3",
                    budget,
                )
                .unwrap();
                let prepaid = fe2o3_compiler_ffi::compiler_descriptor_source_validation_storage_v3(
                    bytes.capacity(),
                )
                .unwrap();
                budget.reserve_storage(prepaid).unwrap();
                let descriptor =
                    fe2o3_compiler_ffi::CompilerDescriptorSourceV3::from_owned_canonical_bytes(
                        bytes,
                        prepaid,
                        &mut |w| budget.charge_work(w),
                    )
                    .unwrap();
                let result = nominal::check_for_profile(
                    OwnerRef::Erased(&owner),
                    &roots,
                    profile,
                    64,
                    &descriptor,
                    budget,
                    |_, _| -> Result<(), E> { panic!("stale Erased subject reached callback") },
                );
                assert!(matches!(
                    result,
                    Err(E::Mismatch("actual P4 output-derived V3 bytes"))
                ));
                drop(descriptor);
                budget.release_storage(prepaid).unwrap();
                assert_eq!(budget.storage(), floor);
            }
            drop(owner);
            budget.release_storage(retained).unwrap();
        });
    }
}

fn digest(domain: &[u8], binding: &[u8; 32], subject: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(domain);
    hash.update(2u64.to_le_bytes());
    hash.update(32u64.to_le_bytes());
    hash.update(binding);
    hash.update((subject.len() as u64).to_le_bytes());
    hash.update(subject);
    hash.finalize().into()
}

fn exercise(
    owner: OwnerRef<'_>,
    roots: &[TypedDescriptorRootV1],
    profile: ProductionAmdTargetProfileV1,
    output: &[u8],
    source: &[u8],
    budget: &mut Budget<'_>,
) {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let (descriptor, storage) =
        nominal::produce_for_profile(owner, roots, profile, 64, budget).unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(storage.retained_storage()).unwrap();
    assert!(!descriptor.authenticates_compiler_origin());
    assert!(!descriptor.grants_link_authority());
    assert!(!descriptor.grants_load_authority());
    assert!(!descriptor.grants_launch_authority());
    let retained = budget.storage();
    nominal::check_for_profile(
        owner,
        roots,
        profile,
        64,
        &descriptor,
        budget,
        |table, budget| {
            codec::scoped(budget, |budget| {
                budget.reserve_storage(fe2o3_kernel_descriptor::DESCRIPTOR_QUERY_STORAGE_V3
                + size_of::<fe2o3_kernel_descriptor::KernelDescriptorRefV3<'static, 'static>>())?;
                assert_eq!(table.kernel_count(), roots.len());
                let mut last = None;
                for index in 0..table.kernel_count() {
                    let kernel = table
                        .kernel(index, &mut |w| budget.charge_work(w))
                        .map_err(E::Wire)?;
                    let id = kernel.kernel_id();
                    let binding = id.as_bytes();
                    assert!(last.is_none_or(|previous| previous < *binding));
                    last = Some(*binding);
                    assert!(
                        roots
                            .iter()
                            .any(|root| &root.kernel_binding_bytes() == binding)
                    );
                    let expected = digest(nominal::OUTPUT_DOMAIN, binding, output);
                    assert_eq!(
                        kernel.executable_ir_evidence(),
                        BuildEvidenceV1::new(
                            EvidenceIdentity::from_opaque_bytes(expected),
                            EvidenceDigest::from_sha256_bytes(expected),
                        )
                    );
                    let expected = digest(b"FE2O3/NOMINAL-SOURCE-ABI/V3\0", binding, source);
                    assert_eq!(
                        kernel.source_evidence(),
                        BuildEvidenceV1::new(
                            EvidenceIdentity::from_opaque_bytes(expected),
                            EvidenceDigest::from_sha256_bytes(expected),
                        )
                    );
                }
                Ok(())
            })
        },
    )
    .unwrap();
    assert_eq!(budget.storage(), retained);
    let (again, additional) =
        nominal::produce_for_profile(owner, roots, profile, 64, budget).unwrap();
    budget
        .reserve_storage(additional.retained_storage())
        .unwrap();
    assert_eq!(descriptor.canonical_bytes(), again.canonical_bytes());
    drop(again);
    budget
        .release_storage(additional.retained_storage())
        .unwrap();
    drop(descriptor);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(budget.work_ledger_identity_v1() == ledger);
}

#[test]
fn nominal_p4_direct_actual_o_evidence_and_complete_canonical_roster() {
    for profile in PROFILES {
        with_backend_checked_output_policy4_v1(profile, |owner, budget| {
            let roots = scalar32_roots(owner.source_semantic_kir());
            assert_eq!(roots.len(), 2);
            assert!(roots[0].kernel_binding_bytes() > roots[1].kernel_binding_bytes());
            assert!(std::ptr::eq(owner.output(), owner.checked_output().owner()));
            exercise(
                OwnerRef::Direct(owner),
                &roots,
                profile,
                owner.output().canonical().canonical_bytes(),
                owner
                    .source_semantic_kir()
                    .semantic()
                    .semantic()
                    .canonical_encoding(),
                budget,
            );
        });
    }
}

#[test]
fn nominal_p4_erased_preserves_original_source_and_distinct_e_and_o() {
    for profile in PROFILES {
        for count in [1, 2] {
            with_backend_erased_bound_v1(true, count, profile, |source, bound, budget| {
                let roots = erased_native_handoff_tests::erased_typed_roots(&source);
                let checked = fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_policy4_v1(
                    &bound, budget,
                )
                .unwrap();
                let retained = checked.retained_storage();
                budget.reserve_storage(retained).unwrap();
                let owner = fe2o3_lower_mir_kernel::ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1
                    ::try_admit_v1(source, bound, checked, budget).unwrap();
                assert_ne!(
                    owner
                        .original_source()
                        .executable()
                        .canonical()
                        .canonical_bytes(),
                    owner.erased().canonical().canonical_bytes()
                );
                assert_ne!(
                    owner
                        .checked_output()
                        .intermediate_policy3()
                        .owner()
                        .canonical()
                        .canonical_bytes(),
                    owner.output().canonical().canonical_bytes()
                );
                exercise(
                    OwnerRef::Erased(&owner),
                    &roots,
                    profile,
                    owner.output().canonical().canonical_bytes(),
                    owner
                        .original_source()
                        .semantic_ssa()
                        .source_semantic()
                        .canonical_encoding(),
                    budget,
                );
                drop(owner);
                budget.release_storage(retained).unwrap();
            });
        }
    }
}

#[test]
fn nominal_p4_rejects_changed_roots_abi_launch_target_and_width() {
    for profile in PROFILES {
        with_backend_checked_output_policy4_v1(profile, |owner, budget| {
            let original = scalar32_roots(owner.source_semantic_kir());
            let floor = budget.storage();
            for mutation in 0..13 {
                let mut roots = original.clone();
                match mutation {
                    0 => roots.clear(),
                    1 => {
                        roots.pop();
                    }
                    2 => roots.swap(0, 1),
                    3 => roots[1].kernel_binding = roots[0].kernel_binding,
                    4 => roots[1].export_name = roots[0].export_name.clone(),
                    5 => roots[0].source_launch = None,
                    6 => roots[0].explicit_argument_bytes += 8,
                    7 => roots[0].kernarg_alignment_bytes = 1,
                    _ => {
                        let mut args = roots[0].arguments.as_slice().to_vec();
                        match mutation {
                            8 => {
                                args[0].semantic_type_identity =
                                    SemanticTypeIdentityV1::from_sha256([4; 32])
                            }
                            9 => args[0].source_size = 8,
                            10 => args[0].source_alignment = 8,
                            11 => args[0].offset = 8,
                            _ => args[0].rustc_abi_class = RustcAbiClassV1::ScalarPair,
                        }
                        roots[0].arguments = TypedArgumentListV1::new(args).unwrap();
                    }
                }
                assert!(
                    nominal::produce_for_profile(
                        OwnerRef::Direct(owner),
                        &roots,
                        profile,
                        64,
                        budget
                    )
                    .is_err(),
                    "mutation {mutation}"
                );
                assert_eq!(budget.storage(), floor);
            }
            let wrong = PROFILES.into_iter().find(|p| *p != profile).unwrap();
            assert!(
                nominal::produce_for_profile(OwnerRef::Direct(owner), &original, wrong, 64, budget)
                    .is_err()
            );
            assert!(matches!(
                nominal::produce_for_profile(
                    OwnerRef::Direct(owner),
                    &original,
                    profile,
                    32,
                    budget
                ),
                Err(E::Mismatch("retained 64-bit rustc target"))
            ));
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn nominal_p4_reproduces_evidence_before_callback_and_restores_callback_failures() {
    for profile in PROFILES {
        with_backend_checked_output_policy4_v1(profile, |owner, budget| {
            let roots = scalar32_roots(owner.source_semantic_kir());
            let floor = budget.storage();
            let source = owner.source_semantic_kir();
            let original = source.pre_ranked_executable().unwrap();
            let bytes = codec::encode_subject(
                &roots,
                source.semantic().semantic(),
                original.module(),
                original.canonical().canonical_bytes(),
                owner.kernels().len(),
                profile,
                64,
                nominal::OUTPUT_DOMAIN,
                "inert-nominal-policy4-output-v3",
                budget,
            )
            .unwrap();
            let prepaid = fe2o3_compiler_ffi::compiler_descriptor_source_validation_storage_v3(
                bytes.capacity(),
            )
            .unwrap();
            budget.reserve_storage(prepaid).unwrap();
            let stale = fe2o3_compiler_ffi::CompilerDescriptorSourceV3::from_owned_canonical_bytes(
                bytes,
                prepaid,
                &mut |w| budget.charge_work(w),
            )
            .unwrap();
            let result = nominal::check_for_profile(
                OwnerRef::Direct(owner),
                &roots,
                profile,
                64,
                &stale,
                budget,
                |_, _| -> Result<(), E> { panic!("stale subject reached callback") },
            );
            assert!(matches!(
                result,
                Err(E::Mismatch("actual P4 output-derived V3 bytes"))
            ));
            drop(stale);
            budget.release_storage(prepaid).unwrap();
            let (descriptor, storage) =
                nominal::produce_for_profile(OwnerRef::Direct(owner), &roots, profile, 64, budget)
                    .unwrap();
            budget.reserve_storage(storage.retained_storage()).unwrap();
            let retained = budget.storage();
            let result = nominal::check_for_profile(
                OwnerRef::Direct(owner),
                &roots,
                profile,
                64,
                &descriptor,
                budget,
                |_, budget| {
                    budget.reserve_storage(1)?;
                    Ok(())
                },
            );
            assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
            assert_eq!(budget.storage(), retained);
            let result = nominal::check_for_profile(
                OwnerRef::Direct(owner),
                &roots,
                profile,
                64,
                &descriptor,
                budget,
                |_, _| -> Result<(), E> { panic!("callback unwind") },
            );
            assert!(matches!(result, Err(E::Panicked)));
            assert_eq!(budget.storage(), retained);
            for unwind in [false, true] {
                let result = nominal::check_for_profile(
                    OwnerRef::Direct(owner),
                    &roots,
                    profile,
                    64,
                    &descriptor,
                    budget,
                    |_, budget| {
                        budget.release_storage(1)?;
                        if unwind {
                            panic!("release live table credit then unwind");
                        }
                        Ok(())
                    },
                );
                assert!(matches!(result, Err(E::Resource(Resource::Accounting))));
                assert_eq!(budget.storage(), retained);
            }
            drop(descriptor);
            budget.release_storage(storage.retained_storage()).unwrap();
            assert_eq!(budget.storage(), floor);
        });
    }
}

#[test]
fn nominal_p4_first_denials_are_independent_of_successful_run_measurements() {
    with_backend_checked_output_policy4_v1(PROFILES[0], |owner, original| {
        let roots = scalar32_roots(owner.source_semantic_kir());
        let floor = original.storage();
        let header = size_of::<OwnerRef<'static>>()
            + size_of::<CheckedDescriptorViewV1<'static>>()
            + size_of::<fe2o3_lower_mir_kernel::CanonicalOutputFormalSourceAnchorV1<'static>>()
            + size_of::<crate::production_geometry_v1::ProductionGeometryV1>();
        let mut work = Work::new(usize::MAX);
        let mut budget = Budget::new(&mut work, floor + header - 1);
        budget.reserve_storage(floor).unwrap();
        let result = nominal::produce_for_profile(
            OwnerRef::Direct(owner),
            &roots,
            PROFILES[0],
            64,
            &mut budget,
        );
        assert!(matches!(result, Err(E::Resource(Resource::Storage(_)))));
        assert_eq!(budget.storage(), floor);
        assert_eq!(budget.failed_storage(), Some(floor + header));
        assert_eq!(budget.work(), 0);
        let mut work = Work::new(7);
        let mut budget = Budget::new(&mut work, usize::MAX);
        budget.reserve_storage(floor).unwrap();
        assert!(
            nominal::produce_for_profile(
                OwnerRef::Direct(owner),
                &roots,
                PROFILES[0],
                64,
                &mut budget
            )
            .is_err()
        );
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.storage(), floor);
    });
}

#[test]
fn nominal_p4_physical_matcher_does_not_change_legacy_nominal_refusal() {
    for (kind, scalar) in [
        (
            DescriptorArgumentKindV1::CompilerLaidOutUsize,
            fe2o3_kernel_ir::ScalarType::U64,
        ),
        (
            DescriptorArgumentKindV1::CompilerLaidOutIsize,
            fe2o3_kernel_ir::ScalarType::I64,
        ),
    ] {
        let ty = fe2o3_kernel_ir::Type::Scalar(scalar);
        assert!(!production_descriptor_argument_matches_kernel_type_v1(
            kind,
            AccessMode::ByValue,
            &ty
        ));
        assert!(nominal::physical_matches(kind, AccessMode::ByValue, &ty));
        assert!(!nominal::physical_matches(kind, AccessMode::ReadOnly, &ty));
    }
}
