use super::*;
use crate::{AccessMode, AddressSpace, Function, Signature, Type};

const WORK_PREFIX: usize = 11;
const STORAGE_PREFIX: usize = 7;

#[test]
fn direct_empty_admission_has_independently_derived_exact_and_one_under_limits() {
    let bytes = encode_module_v12(&Module::new("m")).unwrap();
    assert_eq!(bytes.len(), 37);
    // Decode101 = wire37 + UTF8/copy2 + extent10 + streaming comparison52.
    // Fresh semantic verification5, canonical copy37, and identity hash90.
    const COMPLETE_WORK: usize = 233;
    const BEFORE_COPY: usize = 106;
    const BEFORE_HASH: usize = 143;
    let retained = std::mem::size_of::<VerifiedCanonicalKernelIrModuleV12>() + 1 + 37;
    for case in 0..3 {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(
            WORK_PREFIX + COMPLETE_WORK - usize::from(case == 1),
        );
        work.charge_work(WORK_PREFIX).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            STORAGE_PREFIX + retained - usize::from(case == 2),
        );
        budget.reserve_storage(STORAGE_PREFIX).unwrap();
        let result =
            VerifiedCanonicalKernelIrModuleV12::from_canonical_bytes_with_verification_budget_v12(
                &bytes,
                &mut budget,
            );
        assert_eq!(budget.storage(), STORAGE_PREFIX);
        match case {
            0 => {
                let (owner, storage) = result.unwrap();
                budget.reserve_storage(storage.retained_storage()).unwrap();
                assert_eq!(storage.retained_storage(), retained);
                assert_eq!(budget.work(), WORK_PREFIX + COMPLETE_WORK);
                assert_eq!(budget.peak_storage(), STORAGE_PREFIX + retained);
                assert_eq!(owner.canonical().canonical_bytes(), bytes);
                assert_eq!(owner.module().id.as_str(), "m");
                drop(owner);
                budget.release_storage(retained).unwrap();
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(AdmissionError::Canonical(
                        MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(_)
                    ))
                ));
                assert_eq!(budget.work(), WORK_PREFIX + BEFORE_HASH);
                assert_eq!(budget.peak_storage(), STORAGE_PREFIX + retained);
                assert_eq!(work.failed_work(), Some(WORK_PREFIX + COMPLETE_WORK));
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(AdmissionError::Resource(
                        CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
                    ))
                ));
                assert_eq!(budget.work(), WORK_PREFIX + BEFORE_COPY);
                assert_eq!(budget.peak_storage(), STORAGE_PREFIX + retained - 37);
                assert_eq!(budget.failed_storage(), Some(STORAGE_PREFIX + retained));
            }
        }
    }
}

#[test]
fn direct_admission_retains_nested_owner_after_receipt_bytes_drop() {
    let mut source = Module::new("native-receipt");
    source.functions.push(Function::declaration(
        "f",
        Signature::new(
            vec![Type::pointer(
                Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            )],
            vec![],
        ),
    ));
    let bytes = encode_module_v12(&source).unwrap();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    budget.reserve_storage(STORAGE_PREFIX).unwrap();
    let (owner, retained) =
        VerifiedCanonicalKernelIrModuleV12::from_canonical_bytes_with_verification_budget_v12(
            &bytes,
            &mut budget,
        )
        .unwrap();
    budget.reserve_storage(retained.retained_storage()).unwrap();
    assert_eq!(owner.module(), &source);
    assert_eq!(owner.canonical().canonical_bytes(), bytes);
    assert_ne!(owner.canonical().canonical_bytes().as_ptr(), bytes.as_ptr());
    let (from_module, module_storage) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            &source,
            &mut budget,
        )
        .unwrap();
    budget
        .reserve_storage(module_storage.retained_storage())
        .unwrap();
    assert_eq!(owner, from_module);
    assert_eq!(
        retained.retained_storage(),
        module_storage.retained_storage()
    );
    drop(from_module);
    budget
        .release_storage(module_storage.retained_storage())
        .unwrap();
    drop(bytes);
    drop(source);
    assert_eq!(owner.module().functions.len(), 1);
    drop(owner);
    budget.release_storage(retained.retained_storage()).unwrap();
    assert_eq!(budget.storage(), STORAGE_PREFIX);
}

#[test]
fn direct_admission_rejects_wrong_version_truncation_trailing_and_invalid_semantics() {
    let valid = encode_module_v12(&Module::new("m")).unwrap();
    let mut trailing = valid.clone();
    trailing.push(0);
    let old = crate::encode_module_v11(&Module::new("m")).unwrap();
    let invalid = encode_module_v12(&Module::new("")).unwrap();
    for bytes in [
        &valid[..valid.len() - 1],
        trailing.as_slice(),
        old.as_slice(),
        invalid.as_slice(),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        work.charge_work(WORK_PREFIX).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(STORAGE_PREFIX).unwrap();
        assert!(budget.reserve_storage(usize::MAX).is_err());
        assert!(budget.charge_work(usize::MAX).is_err());
        let result =
            VerifiedCanonicalKernelIrModuleV12::from_canonical_bytes_with_verification_budget_v12(
                bytes,
                &mut budget,
            );
        assert!(result.is_err());
        assert_eq!(budget.storage(), STORAGE_PREFIX);
        assert_eq!(budget.failed_storage(), Some(usize::MAX));
        assert_eq!(work.failed_work(), Some(usize::MAX));
    }
}

fn admission_assert_borrowed_source<T: std::error::Error + 'static>(
    parent: &dyn std::error::Error,
    child: &T,
) {
    let actual = parent.source().unwrap().downcast_ref::<T>().unwrap();
    assert!(std::ptr::eq(actual, child));
}

#[test]
fn admission_error_sources_borrow_all_resource_variants_without_flattening() {
    type Resource = CanonicalKernelIrVerificationResourceErrorV1;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(3);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 5);
    let denied_work = budget.charge_work(4).unwrap_err();
    let denied_storage = budget.reserve_storage(6).unwrap_err();
    let accounting = budget.release_storage(1).unwrap_err();
    assert_eq!(accounting, Resource::Accounting);
    assert_eq!(budget.work(), 0);
    assert_eq!(budget.storage(), 0);
    assert_eq!(budget.peak_storage(), 0);
    assert_eq!(budget.failed_storage(), Some(6));
    drop(budget);
    assert_eq!(work.failed_work(), Some(4));
    // The marker cases test typed diagnostics, not host allocation failure injection.
    for resource in [
        denied_work,
        denied_storage,
        accounting,
        Resource::Allocation,
        Resource::Arithmetic,
    ] {
        let error = AdmissionError::Resource(resource);
        let AdmissionError::Resource(child) = &error else {
            unreachable!()
        };
        admission_assert_borrowed_source(&error, child);
        match child {
            Resource::Work(leaf) => {
                admission_assert_borrowed_source(child, leaf);
                assert_eq!((leaf.actual(), leaf.limit()), (4, 3));
            }
            Resource::Storage(leaf) => {
                admission_assert_borrowed_source(child, leaf);
                assert_eq!((leaf.actual(), leaf.limit()), (6, 5));
            }
            Resource::Allocation | Resource::Accounting | Resource::Arithmetic => {
                assert!(std::error::Error::source(child).is_none());
            }
        }
    }
}

#[test]
fn admission_error_sources_preserve_all_nonresource_children_and_marker() {
    let error = AdmissionError::Decode(KernelIrDecodeError::Truncated);
    let AdmissionError::Decode(child) = &error else {
        unreachable!()
    };
    admission_assert_borrowed_source(&error, child);
    let error = AdmissionError::Encode(KernelIrEncodeError::NonCanonical { field: "fixture" });
    let AdmissionError::Encode(child) = &error else {
        unreachable!()
    };
    admission_assert_borrowed_source(&error, child);
    let rejected = verify_module(&Module::new("")).unwrap_err();
    assert!(!rejected.diagnostics().is_empty());
    let error = AdmissionError::Verification(rejected);
    let AdmissionError::Verification(child) = &error else {
        unreachable!()
    };
    admission_assert_borrowed_source(&error, child);
    let error = AdmissionError::Canonical(MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(
        VerifiedCanonicalKernelIrErrorV12::IdentityMismatch,
    ));
    let AdmissionError::Canonical(child) = &error else {
        unreachable!()
    };
    admission_assert_borrowed_source(&error, child);
    let MeteredVerifiedCanonicalKernelIrErrorV12::Canonical(leaf) = child else {
        unreachable!()
    };
    admission_assert_borrowed_source(child, leaf);
    assert!(std::error::Error::source(leaf).is_none());
    assert!(std::error::Error::source(&AdmissionError::CanonicalMismatch).is_none());
}

#[test]
fn reached_admission_work_and_storage_causes_preserve_exact_prefix_and_floor() {
    let bytes = encode_module_v12(&Module::new("m")).unwrap();
    assert_eq!(bytes.len(), 37);
    // Same independently derived schedule as the existing admission boundary test.
    const COMPLETE_WORK: usize = 233;
    const BEFORE_COPY: usize = 106;
    const BEFORE_HASH: usize = 143;
    let retained = std::mem::size_of::<VerifiedCanonicalKernelIrModuleV12>() + 1 + 37;
    for deny_storage in [false, true] {
        let work_limit = WORK_PREFIX + COMPLETE_WORK - usize::from(!deny_storage);
        let storage_limit = STORAGE_PREFIX + retained - usize::from(deny_storage);
        let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
        work.charge_work(WORK_PREFIX).unwrap();
        let mut budget =
            CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, storage_limit);
        budget.reserve_storage(STORAGE_PREFIX).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let error =
            VerifiedCanonicalKernelIrModuleV12::from_canonical_bytes_with_verification_budget_v12(
                &bytes,
                &mut budget,
            )
            .err()
            .expect("the independently selected admission boundary must reject");
        if deny_storage {
            let AdmissionError::Resource(child) = &error else {
                panic!("{error:?}")
            };
            admission_assert_borrowed_source(&error, child);
            let CanonicalKernelIrVerificationResourceErrorV1::Storage(leaf) = child else {
                panic!("{child:?}")
            };
            admission_assert_borrowed_source(child, leaf);
            assert_eq!(
                (leaf.actual(), leaf.limit()),
                (STORAGE_PREFIX + retained, storage_limit)
            );
            assert_eq!(budget.work(), WORK_PREFIX + BEFORE_COPY);
            assert_eq!(budget.peak_storage(), STORAGE_PREFIX + retained - 37);
            assert_eq!(budget.failed_storage(), Some(STORAGE_PREFIX + retained));
        } else {
            let AdmissionError::Canonical(child) = &error else {
                panic!("{error:?}")
            };
            admission_assert_borrowed_source(&error, child);
            let MeteredVerifiedCanonicalKernelIrErrorV12::WorkLimit(leaf) = child else {
                panic!("{child:?}")
            };
            admission_assert_borrowed_source(child, leaf);
            assert_eq!(
                (leaf.actual(), leaf.limit()),
                (WORK_PREFIX + COMPLETE_WORK, work_limit)
            );
            assert_eq!(budget.work(), WORK_PREFIX + BEFORE_HASH);
            assert_eq!(budget.peak_storage(), STORAGE_PREFIX + retained);
            assert_eq!(budget.failed_storage(), None);
        }
        assert_eq!(budget.storage(), STORAGE_PREFIX);
        assert!(budget.work_ledger_identity_v1() == ledger);
        drop(budget);
        assert_eq!(
            work.failed_work(),
            if deny_storage {
                None
            } else {
                Some(WORK_PREFIX + COMPLETE_WORK)
            }
        );
    }
}

#[test]
fn reached_admission_decode_and_verification_causes_borrow_the_actual_rejections() {
    let valid = encode_module_v12(&Module::new("m")).unwrap();
    let invalid = encode_module_v12(&Module::new("")).unwrap();
    for (bytes, semantic) in [
        (&valid[..valid.len() - 1], false),
        (invalid.as_slice(), true),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        work.charge_work(WORK_PREFIX).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        budget.reserve_storage(STORAGE_PREFIX).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let error =
            VerifiedCanonicalKernelIrModuleV12::from_canonical_bytes_with_verification_budget_v12(
                bytes,
                &mut budget,
            )
            .err()
            .expect("invalid bytes or invalid actual semantics must reject");
        if semantic {
            let AdmissionError::Verification(child) = &error else {
                panic!("{error:?}")
            };
            admission_assert_borrowed_source(&error, child);
            assert!(!child.diagnostics().is_empty());
        } else {
            let AdmissionError::Decode(child) = &error else {
                panic!("{error:?}")
            };
            admission_assert_borrowed_source(&error, child);
        }
        assert_eq!(budget.storage(), STORAGE_PREFIX);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!(budget.failed_storage(), None);
        drop(budget);
        assert_eq!(work.failed_work(), None);
    }
}
