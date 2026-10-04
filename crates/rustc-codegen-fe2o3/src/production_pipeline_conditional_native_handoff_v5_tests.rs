//! Producer custody and allocation controls, not protected proof success.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1 as Work;

#[test]
fn conditional_native_rejects_extraction_custody() {
    assert!(matches!(
        invocation(&ProductionCompilerCustody::ExtractionOnly),
        Err(Error::Mismatch(
            "conditional native handoff requires original protected compiler custody"
        ))
    ));
}

#[test]
fn conditional_native_grow_pays_spare_backing_and_exact_work() {
    let mut bytes = Vec::with_capacity(128);
    bytes.extend_from_slice(b"abc");
    let capacity = bytes.capacity();
    let pointer = bytes.as_ptr();
    let mut work = Work::new(11);
    let mut budget = Budget::new(&mut work, 19 + capacity);
    budget.reserve_storage(19 + capacity).unwrap();
    let account = budget.work_ledger_identity_v1();
    grow(&mut bytes, 13, &mut budget).unwrap();
    assert_eq!(&bytes[..3], b"abc");
    assert_eq!(&bytes[3..], &[0; 10]);
    assert_eq!(bytes.as_ptr(), pointer);
    assert_eq!(budget.storage(), 19 + capacity);
    assert_eq!(budget.work(), 11);
    assert!(budget.work_ledger_identity_v1() == account);
}

#[test]
fn conditional_native_grow_short_work_keeps_terminal_reservation() {
    let mut bytes = Vec::new();
    let mut work = Work::new(16);
    let mut budget = Budget::new(&mut work, 1024);
    budget.reserve_storage(19).unwrap();
    assert!(matches!(
        grow(&mut bytes, 16, &mut budget),
        Err(Error::Resource(Resource::Work(_)))
    ));
    assert!(bytes.is_empty());
    assert!(bytes.capacity() >= 16);
    assert_eq!(budget.storage(), 19 + bytes.capacity());
    assert_eq!(budget.work(), 1);
    assert_eq!(budget.failed_work(), Some(17));
}

#[test]
fn conditional_native_grow_short_storage_precedes_allocation() {
    let mut bytes = Vec::new();
    let mut work = Work::new(100);
    let mut budget = Budget::new(&mut work, 19 + 15);
    budget.reserve_storage(19).unwrap();
    assert!(matches!(
        grow(&mut bytes, 16, &mut budget),
        Err(Error::Resource(Resource::Storage(_)))
    ));
    assert_eq!(bytes.capacity(), 0);
    assert_eq!(budget.storage(), 19);
    assert_eq!(budget.work(), 0);
}

#[test]
fn conditional_native_copy_checks_extent_and_prepays_before_writing() {
    for limit in [2, 3] {
        let mut work = Work::new(limit);
        let mut budget = Budget::new(&mut work, 19);
        budget.reserve_storage(19).unwrap();
        let mut bytes = *b"---";
        let result = copy(&mut bytes, b"abc", &mut budget);
        if limit == 2 {
            assert!(matches!(result, Err(Error::Resource(Resource::Work(_)))));
            assert_eq!(&bytes, b"---");
        } else {
            result.unwrap();
            assert_eq!(&bytes, b"abc");
        }
        assert_eq!(budget.storage(), 19);
    }
}
