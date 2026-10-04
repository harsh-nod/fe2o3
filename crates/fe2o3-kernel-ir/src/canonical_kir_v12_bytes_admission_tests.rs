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
