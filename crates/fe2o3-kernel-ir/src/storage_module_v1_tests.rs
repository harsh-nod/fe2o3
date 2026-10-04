use super::*;
use crate::*;
use std::mem::size_of;

fn limits() -> StorageLayoutLimitsV1 {
    StorageLayoutLimitsV1 {
        rows: 8,
        edges: 16,
        containment_depth: 8,
        object_bytes: 64,
    }
}

fn scalar(scalar: ScalarType) -> StorageLayoutV1 {
    StorageLayoutV1 {
        size: 8,
        alignment: 8,
        kind: StorageLayoutKindV1::Scalar(scalar),
    }
}

fn with_table() -> Module {
    let mut module = Module::new("x");
    module.storage_layouts.push(scalar(ScalarType::U64));
    module
}

fn expected_headers() -> usize {
    // Independent full typed-header roster, not a measured receipt or delta.
    size_of::<HeaderScope<'_, '_>>()
        + size_of::<StructurallyCheckedModuleStorageV1<'_>>()
        + 2 * size_of::<Result<StructurallyCheckedModuleStorageV1<'_>, StorageLayoutErrorV1>>()
        + 2 * size_of::<Result<StructurallyCheckedStorageLayoutsV1<'_>, StorageLayoutErrorV1>>()
        + 2 * size_of::<Result<(), ResourceError>>()
}

type Encoder = fn(&Module) -> Result<Vec<u8>, KernelIrEncodeError>;
type Decoder = fn(&[u8]) -> Result<Module, KernelIrDecodeError>;

fn codecs() -> [(u16, Encoder, Decoder); 15] {
    [
        (1, encode_module_v1, decode_module_v1),
        (2, encode_module_v2, decode_module_v2),
        (3, encode_module_v3, decode_module_v3),
        (4, encode_module_v4, decode_module_v4),
        (5, encode_module_v5, decode_module_v5),
        (6, encode_module_v6, decode_module_v6),
        (7, encode_module_v7, decode_module_v7),
        (8, encode_module_v8, decode_module_v8),
        (9, encode_module_v9, decode_module_v9),
        (10, encode_module_v10, decode_module_v10),
        (11, encode_module_v11, decode_module_v11),
        (12, encode_module_v12, decode_module_v12),
        (15, encode_module_v15, decode_module_v15),
        (16, encode_module_v16, decode_module_v16),
        (17, encode_module_v17, decode_module_v17),
    ]
}

fn unsupported(version: u16) -> KernelIrEncodeError {
    KernelIrEncodeError::UnsupportedInVersion {
        version,
        feature: "module storage layouts",
    }
}

#[test]
fn module_owned_table_defaults_empty_and_clone_preserves_independent_rows() {
    let mut module = Module::new("clone");
    assert!(module.storage_layouts.is_empty());
    module.storage_layouts = vec![
        scalar(ScalarType::U64),
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Record(
                vec![StorageFieldV1 {
                    offset: 0,
                    layout: StorageLayoutIdV1(0),
                }]
                .into_boxed_slice(),
            ),
        },
        StorageLayoutV1 {
            size: 8,
            alignment: 8,
            kind: StorageLayoutKindV1::Variants {
                encoding: StorageVariantEncodingV1::Direct {
                    tag: StorageFieldV1 {
                        offset: 0,
                        layout: StorageLayoutIdV1(0),
                    },
                },
                variants: vec![StorageVariantV1 {
                    discriminant: 9,
                    direct_tag_bits: Some(3),
                    uninhabited: false,
                    layout: StorageLayoutIdV1(1),
                }]
                .into_boxed_slice(),
            },
        },
    ];
    let mut cloned = module.clone();
    assert_eq!(cloned, module);
    let StorageLayoutKindV1::Record(fields) = &mut cloned.storage_layouts[1].kind else {
        panic!("record fixture");
    };
    fields[0].offset = 1;
    let StorageLayoutKindV1::Variants { variants, .. } = &mut cloned.storage_layouts[2].kind else {
        panic!("variant fixture");
    };
    variants[0].discriminant = 10;
    assert_ne!(cloned, module);
    let StorageLayoutKindV1::Record(fields) = &module.storage_layouts[1].kind else {
        panic!("record fixture");
    };
    assert_eq!(fields[0].offset, 0);
    let StorageLayoutKindV1::Variants { variants, .. } = &module.storage_layouts[2].kind else {
        panic!("variant fixture");
    };
    assert_eq!(variants[0].discriminant, 9);
}

#[test]
fn checked_module_view_uses_its_exact_table_not_module_name_or_row_ordinal() {
    let first = with_table();
    let mut second = with_table();
    second.storage_layouts[0] = scalar(ScalarType::I64);
    assert_eq!(first.id, second.id);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(32);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let left = check_module_storage_v1(&first, limits(), &mut budget).unwrap();
    let right = check_module_storage_v1(&second, limits(), &mut budget).unwrap();
    assert!(std::ptr::eq(left.module(), &first));
    assert!(std::ptr::eq(right.module(), &second));
    assert!(std::ptr::eq(
        left.layouts().rows(),
        first.storage_layouts.as_slice()
    ));
    assert!(std::ptr::eq(
        right.layouts().rows(),
        second.storage_layouts.as_slice()
    ));
    assert_ne!(
        left.layouts().row(StorageLayoutIdV1(0)),
        right.layouts().row(StorageLayoutIdV1(0))
    );
    assert_eq!(budget.work(), 32);
    assert_eq!(budget.storage(), 0);
}

#[test]
fn foreign_table_cannot_supply_a_missing_owned_referent() {
    let mut foreign = with_table();
    foreign.storage_layouts.push(scalar(ScalarType::I64));
    let mut owner = with_table();
    owner.storage_layouts[0].kind = StorageLayoutKindV1::Pointer(StoragePointerV1 {
        pointee: StorageLayoutIdV1(1),
        value_space: AddressSpace::Global,
        encoded_space: AddressSpace::Global,
        access: AccessMode::ReadOnly,
        stored_bits: 64,
    });
    assert!(foreign.storage_layouts.get(1).is_some());
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
    let mut budget = Budget::new(&mut work, usize::MAX);
    assert!(matches!(
        check_module_storage_v1(&owner, limits(), &mut budget),
        Err(StorageLayoutErrorV1::Invalid {
            row: 0,
            problem: StorageLayoutProblemV1::InvalidId
        })
    ));
    assert_eq!(budget.storage(), 0);
}

#[test]
fn every_legacy_encoder_counter_and_comparator_refuses_nonempty_owned_tables() {
    let module = with_table();
    for (version, encode, _) in codecs() {
        assert_eq!(encode(&module), Err(unsupported(version)));
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        for validate_roles in [false, true] {
            assert_eq!(
                crate::wire::count_module_with_work_v1(
                    &module, version, &mut work, validate_roles,
                ).unwrap_err(),
                unsupported(version),
            );
        }
        assert_eq!(
            crate::wire::encode_module_with_work_v1(&module, version, &mut work),
            Err(unsupported(version)),
        );
        assert_eq!(
            crate::wire::compare_module_encoding_v1(&module, version, &[], Some(&mut work)),
            Err(unsupported(version)),
        );
        assert_eq!(work.work(), 0);
        assert_eq!(work.failed_work(), None);
    }
}

#[test]
fn empty_tables_keep_frozen_bytes_and_all_decoders_reconstruct_empty_ownership() {
    let module = Module::new("x");
    for (version, encode, decode) in codecs() {
        // Frozen schema: 20-byte header, u32 text length + x, then three u32 counts.
        let mut expected = b"FE2O3KI\0".to_vec();
        expected.extend_from_slice(&version.to_le_bytes());
        expected.extend_from_slice(&0_u16.to_le_bytes());
        expected.extend_from_slice(&37_u32.to_le_bytes());
        expected.extend_from_slice(&0_u32.to_le_bytes());
        expected.extend_from_slice(&1_u32.to_le_bytes());
        expected.push(b'x');
        expected.extend_from_slice(&[0; 12]);
        assert_eq!(encode(&module).unwrap(), expected);
        let decoded = decode(&expected).unwrap();
        assert!(decoded.storage_layouts.is_empty());
        assert_eq!(decoded, module);
        assert!(
            crate::wire::compare_module_encoding_v1(&module, version, &expected, None).unwrap()
        );
    }
}

#[test]
fn legacy_verification_refuses_even_valid_unreferenced_owned_layouts() {
    let module = with_table();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(usize::MAX);
    let mut budget = Budget::new(&mut work, usize::MAX);
    check_module_storage_v1(&module, limits(), &mut budget).unwrap();
    let error = verify_module(&module).unwrap_err();
    assert_eq!(error.diagnostics().len(), 1);
    assert_eq!(
        error.diagnostics()[0].code,
        DiagnosticCode::InvalidSemanticOperation
    );
    let error = verify_module_ref(&module).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        DiagnosticCode::InvalidSemanticOperation
    );
    let Err(crate::MeteredKernelIrVerificationErrorV1::Verification(error)) =
        crate::verify_exact_decoded_module_with_budget_v1(&module, None, &mut budget)
    else {
        panic!("legacy shared semantic refusal");
    };
    assert_eq!(
        error.diagnostics()[0].code,
        DiagnosticCode::InvalidSemanticOperation
    );
    assert_eq!(
        VerifiedCanonicalKernelIrV12::from_module(module.clone()).unwrap_err(),
        VerifiedCanonicalKernelIrErrorV12::Encode(unsupported(12)),
    );
    let error = VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
        &module,
        &mut budget,
    )
    .unwrap_err();
    let CanonicalKernelIrReplayAdmissionErrorV12::Encode(error) = error else {
        panic!("complete legacy canonical encode refusal");
    };
    assert_eq!(error, unsupported(12));
    let mut malformed = module.clone();
    malformed.storage_layouts[0].alignment = 0;
    let error = verify_module(&malformed).unwrap_err();
    assert_eq!(error.diagnostics().len(), 1);
    assert_eq!(
        error.diagnostics()[0].code,
        DiagnosticCode::InvalidSemanticOperation
    );
}

#[test]
fn old_verified_candidate_copy_retains_the_empty_table_invariant() {
    let module = Module::new("x");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = Budget::new(&mut work, usize::MAX);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &module,
            &mut budget,
        )
        .unwrap();
    assert!(owner.module().storage_layouts.is_empty());
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    let (candidate, candidate_receipt) = owner
        .copy_module_for_transformation_v12(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), floor);
    assert!(candidate.storage_layouts.is_empty());
    assert_eq!(&candidate, owner.module());
    budget
        .reserve_storage(candidate_receipt.retained_storage())
        .unwrap();
    drop(candidate);
    budget
        .release_storage(candidate_receipt.retained_storage())
        .unwrap();
    drop(owner);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 0);
}

#[test]
fn module_wrapper_entry_refusal_never_reserves_headers() {
    let module = with_table();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = Budget::new(&mut work, 31);
    budget.reserve_storage(31).unwrap();
    let error = check_module_storage_v1(&module, limits(), &mut budget).unwrap_err();
    let StorageLayoutErrorV1::Resource(ResourceError::Work(error)) = error else {
        panic!("wrapper entry work refusal");
    };
    assert_eq!(error.actual(), 1);
    assert_eq!(error.limit(), 0);
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 31);
    assert_eq!(budget.peak_storage(), 31);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn module_wrapper_header_exact_and_one_short_are_source_derived() {
    let module = with_table();
    let floor = 17;
    let header = expected_headers();
    assert_eq!(headers().unwrap(), header);
    for storage_limit in [floor + header - 1, floor + header] {
        // Only wrapper entry succeeds; core entry's next unit refuses before
        // core headers. This isolates the complete added header envelope.
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1);
        let mut budget = Budget::new(&mut work, storage_limit);
        budget.reserve_storage(floor).unwrap();
        let error = check_module_storage_v1(&module, limits(), &mut budget).unwrap_err();
        assert_eq!(budget.work(), 1);
        assert_eq!(budget.storage(), floor);
        if storage_limit == floor + header {
            let StorageLayoutErrorV1::Resource(ResourceError::Work(error)) = error else {
                panic!("core entry work refusal");
            };
            assert_eq!(error.actual(), 2);
            assert_eq!(error.limit(), 1);
            assert_eq!(budget.peak_storage(), floor + header);
            assert_eq!(budget.failed_storage(), None);
        } else {
            let StorageLayoutErrorV1::Resource(ResourceError::Storage(error)) = error else {
                panic!("wrapper header refusal");
            };
            assert_eq!(error.actual(), floor + header);
            assert_eq!(error.limit(), storage_limit);
            assert_eq!(budget.peak_storage(), floor);
            assert_eq!(budget.failed_storage(), Some(floor + header));
        }
    }
}

#[test]
fn module_wrapper_success_and_final_work_refusal_preserve_the_caller_floor() {
    for (module, complete) in [(Module::new("x"), 8), (with_table(), 16)] {
        // Wrapper one unit plus core's empty seven / single-U64 fifteen.
        for limit in [complete - 1, complete] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = Budget::new(&mut work, usize::MAX);
            budget.reserve_storage(23).unwrap();
            let result = check_module_storage_v1(&module, limits(), &mut budget);
            assert_eq!(budget.storage(), 23);
            assert_eq!(budget.work(), limit);
            if limit == complete {
                assert!(std::ptr::eq(result.unwrap().module(), &module));
            } else {
                let StorageLayoutErrorV1::Resource(ResourceError::Work(error)) =
                    result.unwrap_err()
                else {
                    panic!("last work debit");
                };
                assert_eq!(error.actual(), complete);
                assert_eq!(error.limit(), limit);
            }
        }
    }
}

#[test]
fn module_wrapper_preserves_prior_denial_history_and_restores_invalid_input_floor() {
    let mut module = with_table();
    module.storage_layouts[0].alignment = 3;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1000);
    assert!(work.charge_work(1001).is_err());
    {
        let mut budget = Budget::new(&mut work, 10_000);
        budget.reserve_storage(19).unwrap();
        assert!(budget.reserve_storage(10_000).is_err());
        assert!(matches!(
            check_module_storage_v1(&module, limits(), &mut budget),
            Err(StorageLayoutErrorV1::Invalid {
                row: 0,
                problem: StorageLayoutProblemV1::Alignment
            })
        ));
        assert_eq!(budget.storage(), 19);
        assert_eq!(budget.failed_storage(), Some(10_019));
    }
    assert_eq!(work.failed_work(), Some(1001));
}

#[test]
fn module_header_scope_unwind_refunds_only_its_own_reservation() {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
    let mut budget = Budget::new(&mut work, usize::MAX);
    budget.reserve_storage(29).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let charged = expected_headers();
        budget.reserve_storage(charged).unwrap();
        let _scope = HeaderScope {
            budget: &mut budget,
            charged,
        };
        panic!("test-only header scope unwind");
    }));
    assert!(result.is_err());
    assert_eq!(budget.storage(), 29);
    assert_eq!(budget.peak_storage(), 29 + expected_headers());
}
