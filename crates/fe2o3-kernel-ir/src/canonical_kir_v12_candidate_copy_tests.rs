use super::*;
use crate::{AccessMode, AddressSpace, Function, Signature, Type};

const WORK_PREFIX: usize = 11;
const STORAGE_PREFIX: usize = 7;

fn owner(module: &Module) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    let (owner, receipt) =
        VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
            module,
            &mut budget,
        )
        .unwrap();
    assert_eq!(budget.storage(), 0);
    (owner, receipt.retained_storage())
}

#[test]
fn empty_candidate_copy_has_exact_independent_work_and_storage_boundaries() {
    let (owner, source_storage) = owner(&Module::new("m"));
    // Exact V12 decode costs 101, complete structural equality costs wire37.
    const COPY_WORK: usize = 138;
    const BEFORE_COMPARISON: usize = 101;
    let copied_storage = std::mem::size_of::<Module>() + 1;
    let floor = STORAGE_PREFIX + source_storage;
    for case in 0..3 {
        let mut work =
            CanonicalKernelIrWorkBudgetV1::new(WORK_PREFIX + COPY_WORK - usize::from(case == 1));
        work.charge_work(WORK_PREFIX).unwrap();
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(
            &mut work,
            floor + copied_storage - usize::from(case == 2),
        );
        budget.reserve_storage(floor).unwrap();
        let result = owner.copy_module_for_transformation_v12(&mut budget);
        assert_eq!(budget.storage(), floor);
        match case {
            0 => {
                let (candidate, receipt) = result.unwrap();
                assert_eq!(receipt.retained_storage(), copied_storage);
                assert_eq!(&candidate, owner.module());
                assert_eq!(budget.work(), WORK_PREFIX + COPY_WORK);
                assert_eq!(budget.peak_storage(), floor + copied_storage);
                budget.reserve_storage(receipt.retained_storage()).unwrap();
                drop(candidate);
                budget.release_storage(receipt.retained_storage()).unwrap();
                assert_eq!(budget.storage(), floor);
            }
            1 => {
                assert!(matches!(
                    result,
                    Err(AdmissionError::Resource(
                        CanonicalKernelIrVerificationResourceErrorV1::Work(_)
                    ))
                ));
                assert_eq!(budget.work(), WORK_PREFIX + BEFORE_COMPARISON);
                assert_eq!(budget.peak_storage(), floor + copied_storage);
                assert_eq!(work.failed_work(), Some(WORK_PREFIX + COPY_WORK));
            }
            _ => {
                assert!(matches!(
                    result,
                    Err(AdmissionError::Decode(KernelIrDecodeError::Resource(
                        CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
                    )))
                ));
                assert_eq!(budget.peak_storage(), floor + std::mem::size_of::<Module>());
                assert_eq!(budget.failed_storage(), Some(floor + copied_storage));
            }
        }
    }
}

#[test]
fn copied_nested_candidate_is_independent_and_does_not_modify_source_custody() {
    let mut module = Module::new("nested-copy");
    module.functions.push(Function::declaration(
        "declaration",
        Signature::new(
            vec![Type::pointer(
                Type::slice(Type::F32, AddressSpace::Global, AccessMode::ReadOnly),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            )],
            vec![],
        ),
    ));
    let (owner, source_storage) = owner(&module);
    let source_identity = *owner.canonical().identity();
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    let floor = STORAGE_PREFIX + source_storage;
    budget.reserve_storage(floor).unwrap();
    let (mut candidate, receipt) = owner
        .copy_module_for_transformation_v12(&mut budget)
        .unwrap();
    assert_eq!(budget.storage(), floor);
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    assert_eq!(&candidate, owner.module());
    assert_ne!(
        candidate.id.as_str().as_ptr(),
        owner.module().id.as_str().as_ptr()
    );
    assert_ne!(
        candidate.functions.as_ptr(),
        owner.module().functions.as_ptr()
    );
    assert!(receipt.retained_storage() > std::mem::size_of::<Module>());
    candidate.functions.clear();
    assert_eq!(owner.module().functions.len(), 1);
    assert_eq!(owner.canonical().identity(), &source_identity);
    assert_ne!(&candidate, owner.module());
    drop(candidate);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), floor);
}

#[test]
fn copy_refuses_an_internal_endpoint_mismatch_without_leaking_storage() {
    let (mut owner, source_storage) = owner(&Module::new("m"));
    // Only this module's tests can violate the private immutable owner invariant.
    owner.module = Module::new("substituted");
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    let floor = STORAGE_PREFIX + source_storage;
    budget.reserve_storage(floor).unwrap();
    assert!(budget.reserve_storage(usize::MAX).is_err());
    assert!(budget.charge_work(usize::MAX).is_err());
    assert!(matches!(
        owner.copy_module_for_transformation_v12(&mut budget),
        Err(AdmissionError::CanonicalMismatch)
    ));
    assert_eq!(budget.storage(), floor);
    assert_eq!(budget.failed_storage(), Some(usize::MAX));
    assert_eq!(work.failed_work(), Some(usize::MAX));
}

#[test]
fn candidate_transfer_does_not_retain_or_borrow_the_source_owner() {
    let (owner, source_storage) = owner(&Module::new("m"));
    let mut work = CanonicalKernelIrWorkBudgetV1::new(1_000_000);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
    budget
        .reserve_storage(STORAGE_PREFIX + source_storage)
        .unwrap();
    let (candidate, receipt) = owner
        .copy_module_for_transformation_v12(&mut budget)
        .unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(source_storage).unwrap();
    assert_eq!(candidate.id.as_str(), "m");
    assert_eq!(
        budget.storage(),
        STORAGE_PREFIX + receipt.retained_storage()
    );
    drop(candidate);
    budget.release_storage(receipt.retained_storage()).unwrap();
    assert_eq!(budget.storage(), STORAGE_PREFIX);
}
