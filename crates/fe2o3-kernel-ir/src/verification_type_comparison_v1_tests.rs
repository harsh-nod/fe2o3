use super::*;
use crate::{
    AccessMode, AddressSpace, CanonicalKernelIrVerificationResourceErrorV1,
    CanonicalKernelIrWorkBudgetV1,
};

fn nested(depth: usize, leaf: Type) -> Type {
    (0..depth).fold(leaf, |ty, ordinal| {
        if ordinal % 2 == 0 {
            Type::pointer(ty, AddressSpace::Global, AccessMode::ReadOnly)
        } else {
            Type::slice(ty, AddressSpace::Constant, AccessMode::ReadOnly)
        }
    })
}

#[test]
fn recursive_comparison_charges_each_reached_node_before_read() {
    let left = nested(64, Type::INDEX);
    let right = nested(64, Type::INDEX);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(64);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert!(matches!(
        verification_types_equal_v1(&left, &right, &mut budget),
        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
            if error.actual() == 65 && error.limit() == 64
    ));
    assert_eq!(budget.work(), 64);

    let mut work = CanonicalKernelIrWorkBudgetV1::new(65);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert!(verification_types_equal_v1(&left, &right, &mut budget).unwrap());
    assert_eq!(budget.work(), 65);
    assert_eq!(budget.peak_storage(), 0);
}

#[test]
fn message_bound_uses_the_same_exact_node_census() {
    let ty = nested(3, Type::F32);
    let mut work = CanonicalKernelIrWorkBudgetV1::new(4);
    let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
    assert_eq!(
        verification_type_message_work_upper_v1(&ty, &mut budget).unwrap(),
        512 + 4 * 128
    );
    assert_eq!(budget.work(), 4);
}
