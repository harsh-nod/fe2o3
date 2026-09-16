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

#[test]
fn pointer_storability_charges_every_descendant_before_read() {
    for depth in [0, 1, 64] {
        for (leaf, expected) in [
            (Type::INDEX, true),
            (Type::Execution(crate::ExecutionRoleV15::Context), false),
        ] {
            let ty = Type::pointer(
                nested(depth, leaf),
                AddressSpace::Private,
                AccessMode::ReadWrite,
            );
            let exact = depth + 1;
            for limit in [exact - 1, exact] {
                let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
                let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
                let result = verification_type_is_storable_v15(&ty, &mut budget);
                if limit == exact {
                    assert_eq!(result.unwrap(), expected);
                } else {
                    assert!(matches!(
                        result,
                        Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                            if error.actual() == exact && error.limit() == limit
                    ));
                }
                assert_eq!(budget.work(), limit);
                assert_eq!(budget.peak_storage(), 0);
            }
        }
    }
}

#[test]
fn nonpointer_storability_keeps_the_callers_fixed_work_charge() {
    for (ty, expected) in [
        (Type::INDEX, true),
        (Type::Unit, false),
        (Type::Execution(crate::ExecutionRoleV15::Context), false),
        (
            Type::slice(
                nested(64, Type::INDEX),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            false,
        ),
    ] {
        let mut work = CanonicalKernelIrWorkBudgetV1::new(0);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
        assert_eq!(
            verification_type_is_storable_v15(&ty, &mut budget).unwrap(),
            expected
        );
        assert_eq!(budget.work(), 0);
        assert_eq!(budget.peak_storage(), 0);
    }
}

#[test]
fn shared_type_facts_reuse_the_exact_workgroup_node_census() {
    let context = Type::Execution(crate::ExecutionRoleV15::Context);
    for (ty, nodes, storable, contains_execution) in [
        (Type::INDEX, 1, true, false),
        (context.clone(), 1, false, true),
        (nested(3, Type::INDEX), 4, true, false),
        (nested(4, Type::INDEX), 5, false, false),
        (nested(3, context), 4, false, true),
        (nested(3, Type::Unit), 4, true, false),
    ] {
        for limit in [nodes - 1, nodes] {
            let mut work = CanonicalKernelIrWorkBudgetV1::new(limit);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 0);
            let result = verification_type_facts_v15(&ty, &mut budget);
            if limit == nodes {
                let facts = result.unwrap();
                assert_eq!(facts.storable, storable);
                assert_eq!(facts.contains_execution_role, contains_execution);
                assert!(facts.vector_error.is_none());
            } else {
                assert!(matches!(
                    result,
                    Err(CanonicalKernelIrVerificationResourceErrorV1::Work(error))
                        if error.actual() == nodes && error.limit() == limit
                ));
            }
            assert_eq!(budget.work(), limit);
            assert_eq!(budget.peak_storage(), 0);
        }
    }
}
