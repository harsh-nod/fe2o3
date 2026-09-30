//! Exact, paid congruence classes for the closed total-operation grammar.
//! Operands remain arguments of the shared interpretation, not part of a key.

use super::*;
use std::cmp::Ordering;

fn type_tag(ty: &Type) -> u8 {
    match ty {
        Type::Unit => 0,
        Type::Scalar(_) => 1,
        Type::StorageObject(_) => 2,
        Type::Execution(_) => 3,
        Type::Vector(_) => 4,
        Type::Pointer(_) => 5,
        Type::Slice(_) => 6,
    }
}

// Pointer/slice nesting is linear. Do not use derived recursive Type::cmp at
// this resource boundary, including when the first unequal node is very deep.
pub(super) fn compare_type(
    mut a: &Type,
    mut b: &Type,
    out: &mut Writer<'_, '_>,
) -> Result<Ordering> {
    loop {
        out.budget.charge_work(3)?;
        let tag = type_tag(a).cmp(&type_tag(b));
        if tag != Ordering::Equal {
            return Ok(tag);
        }
        let (order, next) = match (a, b) {
            (Type::Unit, Type::Unit) => (Ordering::Equal, None),
            (Type::Scalar(a), Type::Scalar(b)) => (a.cmp(b), None),
            (Type::StorageObject(a), Type::StorageObject(b)) => (a.cmp(b), None),
            (Type::Execution(a), Type::Execution(b)) => (a.cmp(b), None),
            (Type::Vector(a), Type::Vector(b)) => (a.cmp(b), None),
            (Type::Pointer(a), Type::Pointer(b)) => (
                (a.address_space, a.access).cmp(&(b.address_space, b.access)),
                Some((&*a.pointee, &*b.pointee)),
            ),
            (Type::Slice(a), Type::Slice(b)) => (
                (a.address_space, a.access).cmp(&(b.address_space, b.access)),
                Some((&*a.element, &*b.element)),
            ),
            _ => return Err(Error::Statement("total operator type tag")),
        };
        if order != Ordering::Equal {
            return Ok(order);
        }
        match next {
            Some((left, right)) => (a, b) = (left, right),
            None => return Ok(Ordering::Equal),
        }
    }
}

fn operation_tag(kind: &OperationKind) -> Result<u8> {
    if !opaque_total(kind) {
        return Err(Error::Statement(
            "ordered operation has no total congruence key",
        ));
    }
    Ok(match kind {
        OperationKind::Constant(_) => 0,
        OperationKind::Unary { .. } => 1,
        OperationKind::Binary { .. } => 2,
        OperationKind::Compare { .. } => 3,
        OperationKind::Cast { .. } => 4,
        OperationKind::SliceLength { .. } => 6,
        OperationKind::SliceData { .. } => 7,
        _ => return Err(Error::Statement("closed total operator key")),
    })
}

fn compare_kind(
    a: &OperationKind,
    b: &OperationKind,
    out: &mut Writer<'_, '_>,
) -> Result<Ordering> {
    out.budget.charge_work(4)?;
    let tag = operation_tag(a)?.cmp(&operation_tag(b)?);
    if tag != Ordering::Equal {
        return Ok(tag);
    }
    Ok(match (a, b) {
        (OperationKind::Constant(a), OperationKind::Constant(b)) => a.cmp(b),
        (OperationKind::Unary { op: a, .. }, OperationKind::Unary { op: b, .. }) => a.cmp(b),
        (OperationKind::Binary { op: a, .. }, OperationKind::Binary { op: b, .. }) => a.cmp(b),
        (
            OperationKind::Compare { predicate: a, .. },
            OperationKind::Compare { predicate: b, .. },
        ) => a.cmp(b),
        (
            OperationKind::Cast {
                kind: a, to: at, ..
            },
            OperationKind::Cast {
                kind: b, to: bt, ..
            },
        ) => {
            let kind = a.cmp(b);
            if kind == Ordering::Equal {
                compare_type(at, bt, out)?
            } else {
                kind
            }
        }
        (OperationKind::SliceLength { .. }, OperationKind::SliceLength { .. })
        | (OperationKind::SliceData { .. }, OperationKind::SliceData { .. }) => Ordering::Equal,
        _ => return Err(Error::Statement("total operator attribute tag")),
    })
}

fn compare(
    left: &Inventory<'_>,
    a: usize,
    right: &Inventory<'_>,
    b: usize,
    out: &mut Writer<'_, '_>,
) -> Result<Ordering> {
    out.budget.charge_work(5)?;
    let a = left
        .operations()
        .get(a)
        .ok_or(Error::Statement("left total operator coordinate"))?;
    let b = right
        .operations()
        .get(b)
        .ok_or(Error::Statement("right total operator coordinate"))?;
    let kind = compare_kind(&a.operation.kind, &b.operation.kind, out)?;
    if kind != Ordering::Equal {
        return Ok(kind);
    }
    let arity = (a.results.len(), a.operands.len()).cmp(&(b.results.len(), b.operands.len()));
    if arity != Ordering::Equal {
        return Ok(arity);
    }
    for (a, b) in a.results.clone().zip(b.results.clone()) {
        out.budget.charge_work(2)?;
        let types = compare_type(left.definitions()[a].ty, right.definitions()[b].ty, out)?;
        if types != Ordering::Equal {
            return Ok(types);
        }
    }
    for (a, b) in a.operands.clone().zip(b.operands.clone()) {
        out.budget.charge_work(4)?;
        let a = left.uses()[a].definition;
        let b = right.uses()[b].definition;
        let types = compare_type(left.definitions()[a].ty, right.definitions()[b].ty, out)?;
        if types != Ordering::Equal {
            return Ok(types);
        }
    }
    Ok(Ordering::Equal)
}

pub(super) fn build(
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    output_origins: &[usize],
    out: &mut Writer<'_, '_>,
) -> Result<Vec<usize>> {
    let count = input.operations().len();
    let mut classes = allocate(count, out)?;
    let scratch_floor = out.budget.storage();
    let result = (|| {
        let mut order = allocate(count, out)?;
        let mut scratch = allocate(count, out)?;
        let mut end = 0;
        for (ordinal, row) in input.operations().iter().enumerate() {
            out.budget.charge_work(2)?;
            if opaque_total(&row.operation.kind) {
                order[end] = ordinal;
                end += 1;
            }
        }
        // A fallible bottom-up merge sort bounds comparisons by O(n log n).
        // Every type node and output write is debited; no comparator allocation.
        let mut width = 1usize;
        while width < end {
            let mut start = 0;
            while start < end {
                out.budget.charge_work(3)?;
                let middle = start.saturating_add(width).min(end);
                let finish = middle.saturating_add(width).min(end);
                let (mut a, mut b) = (start, middle);
                for target in start..finish {
                    out.budget.charge_work(3)?;
                    let take_left = b == finish
                        || (a < middle
                            && compare(input, order[a], input, order[b], out)?
                                != Ordering::Greater);
                    scratch[target] = if take_left {
                        let value = order[a];
                        a += 1;
                        value
                    } else {
                        let value = order[b];
                        b += 1;
                        value
                    };
                }
                start = finish;
            }
            std::mem::swap(&mut order, &mut scratch);
            width = width.checked_mul(2).ok_or(Resource::Accounting)?;
        }
        let mut representative = NONE;
        let mut previous = None;
        for &ordinal in &order[..end] {
            out.budget.charge_work(3)?;
            if match previous {
                Some(previous) => compare(input, previous, input, ordinal, out)? != Ordering::Equal,
                None => true,
            } {
                // Keep total keys disjoint from every ordered occurrence key.
                representative = count.checked_add(ordinal).ok_or(Resource::Accounting)?;
            }
            classes[ordinal] = representative;
            previous = Some(ordinal);
        }
        if output_origins.len() != output.operations().len() {
            return Err(Error::Statement("total output origin census"));
        }
        for (ordinal, row) in output.operations().iter().enumerate() {
            out.budget.charge_work(2)?;
            if opaque_total(&row.operation.kind)
                && !matches!(row.operation.kind, OperationKind::Constant(_))
            {
                let origin = output_origins[ordinal];
                if classes.get(origin).is_none_or(|class| *class == NONE)
                    || compare(input, origin, output, ordinal, out)? != Ordering::Equal
                {
                    return Err(Error::Statement(
                        "retained total operator changed its exact shape",
                    ));
                }
            }
        }
        Ok(())
    })();
    // Both sorting buffers have dropped; only the exact class allocation lives.
    let released = out
        .budget
        .storage()
        .checked_sub(scratch_floor)
        .ok_or(Resource::Accounting)?;
    out.budget.release_storage(released)?;
    result?;
    Ok(classes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        AccessMode, AddressSpace, CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work, CastKind, ValueId,
    };

    #[test]
    fn total_operator_type_walk_has_independent_exact_and_short_work() {
        for depth in [0, 1, 7, 127] {
            let mut ty = Type::Scalar(ScalarType::U32);
            for level in 0..depth {
                ty = if level % 2 == 0 {
                    Type::pointer(ty, AddressSpace::Global, AccessMode::ReadOnly)
                } else {
                    Type::slice(ty, AddressSpace::Generic, AccessMode::ReadWrite)
                };
            }
            let exact = 3 * (depth + 1);
            for limit in [exact, exact - 1] {
                let mut work = Work::new(limit);
                let mut budget = Budget::new(&mut work, 17);
                budget.reserve_storage(17).unwrap();
                let mut out = Writer {
                    text: String::new(),
                    budget: &mut budget,
                    failure: None,
                };
                let result = compare_type(&ty, &ty, &mut out);
                if limit == exact {
                    assert_eq!(result.unwrap(), Ordering::Equal);
                    assert_eq!(budget.work(), exact);
                } else {
                    assert!(
                        matches!(result,Err(Error::Resource(Resource::Work(error))) if error.limit()==limit&&error.actual()==exact)
                    );
                    assert_eq!(budget.work(), exact - 3);
                }
                assert_eq!(budget.storage(), 17);
            }
        }
    }

    #[test]
    fn total_operator_attributes_and_nested_capability_types_are_exact() {
        let mut work = Work::new(10_000);
        let mut budget = Budget::new(&mut work, 17);
        budget.reserve_storage(17).unwrap();
        let mut out = Writer {
            text: String::new(),
            budget: &mut budget,
            failure: None,
        };
        let original = Type::slice(
            Type::pointer(
                Type::Scalar(ScalarType::U32),
                AddressSpace::Global,
                AccessMode::ReadOnly,
            ),
            AddressSpace::Generic,
            AccessMode::ReadWrite,
        );
        for changed in [
            Type::slice(
                Type::pointer(
                    Type::Scalar(ScalarType::I32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                AddressSpace::Generic,
                AccessMode::ReadWrite,
            ),
            Type::slice(
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Private,
                    AccessMode::ReadOnly,
                ),
                AddressSpace::Generic,
                AccessMode::ReadWrite,
            ),
            Type::slice(
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadWrite,
                ),
                AddressSpace::Generic,
                AccessMode::ReadWrite,
            ),
            Type::pointer(
                Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                ),
                AddressSpace::Generic,
                AccessMode::ReadWrite,
            ),
        ] {
            assert_ne!(
                compare_type(&original, &changed, &mut out).unwrap(),
                Ordering::Equal
            );
        }
        let cast = |to| OperationKind::Cast {
            kind: CastKind::Truncate,
            value: ValueId(0),
            to,
        };
        assert_ne!(
            compare_kind(
                &cast(Type::Scalar(ScalarType::U16)),
                &cast(Type::Scalar(ScalarType::U8)),
                &mut out
            )
            .unwrap(),
            Ordering::Equal
        );
        for operator in [
            BinaryOp::Add,
            BinaryOp::Subtract,
            BinaryOp::Multiply,
            BinaryOp::Divide,
            BinaryOp::Remainder,
        ] {
            let kind = OperationKind::Binary {
                op: operator,
                lhs: ValueId(0),
                rhs: ValueId(1),
            };
            assert!(matches!(
                compare_kind(&kind, &kind, &mut out),
                Err(Error::Statement(
                    "ordered operation has no total congruence key"
                ))
            ));
        }
        assert_eq!(out.budget.storage(), 17);
    }
}
