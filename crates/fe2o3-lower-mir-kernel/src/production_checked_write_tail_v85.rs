// A physical postcondition shared by every checked-write mapping. This checks
// the emitted suffix, not original index/value provenance or allocation extent.
#[derive(Clone, Copy)]
struct CheckedWriteInputsV85 {
    slice: ValueId,
    index: ValueId,
    precondition: Option<ValueId>,
    value: ValueId,
    element: ScalarType,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedWriteTailV85 {
    length: ValueId,
    extent: ValueId,
    predicate: ValueId,
    zero: ValueId,
    offset: ValueId,
    data: ValueId,
    pointer: ValueId,
}

fn checked_write_tail_headers_v85() -> Result<usize, ArgumentResourceV1> {
    type Frame<'a> = (
        CheckedWriteInputsV85,
        CheckedWriteTailV85,
        &'a [Operation],
        [&'a Operation; 8],
        [&'a ValueDef; 7],
        [ValueId; 7],
        [ValueId; 4],
        [usize; 4],
        Result<CheckedWriteTailV85, ProductionSemanticKirErrorV1>,
        Result<(), ProductionSemanticKirErrorV1>,
        Option<u32>,
    );
    argument_sum_v1(&[size_of::<Frame<'_>>(), std::mem::align_of::<Frame<'_>>()])
}

// The closed emission/replay caller prepays the fixed borrowed frame above.
// No graph walk, allocation, or recursively structured type comparison occurs.
fn check_checked_write_tail_v85<O: std::borrow::Borrow<Operation>>(
    inputs: CheckedWriteInputsV85,
    operations: &[O],
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<CheckedWriteTailV85, ProductionSemanticKirErrorV1> {
    budget.charge_work(128)?;
    let extra = usize::from(inputs.precondition.is_some());
    if operations.len() != 7 + extra {
        return Err(source_issued_error_v29());
    }
    let length = operations[0].borrow();
    let extent = operations[1].borrow();
    let zero = operations[2 + extra].borrow();
    let select = operations[3 + extra].borrow();
    let data = operations[4 + extra].borrow();
    let address = operations[5 + extra].borrow();
    let store = operations[6 + extra].borrow();
    let ([length_value], [extent_value], [zero_value], [offset], [base], [pointer]) = (
        length.results.as_slice(),
        extent.results.as_slice(),
        zero.results.as_slice(),
        select.results.as_slice(),
        data.results.as_slice(),
        address.results.as_slice(),
    ) else {
        return Err(source_issued_error_v29());
    };
    let predicate = if let Some(precondition) = inputs.precondition {
        let conjunction = operations[2].borrow();
        let [result] = conjunction.results.as_slice() else {
            return Err(source_issued_error_v29());
        };
        if result.ty != Type::BOOL
            || !matches!(conjunction.kind, OperationKind::Binary { op: BinaryOp::BitAnd, lhs, rhs }
                if lhs == precondition && rhs == extent_value.id)
        {
            return Err(source_issued_error_v29());
        }
        result.id
    } else {
        extent_value.id
    };
    let pointer_type = |ty: &Type| {
        matches!(ty, Type::Pointer(pointer)
            if *pointer.pointee == Type::Scalar(inputs.element)
                && pointer.address_space == AddressSpace::Global
                && pointer.access == AccessMode::WriteOnly)
    };
    let alignment = strided_read_scalar_alignment_v1(&Type::Scalar(inputs.element))
        .ok_or_else(source_issued_error_v29)?;
    if length_value.ty != Type::INDEX
        || extent_value.ty != Type::BOOL
        || zero_value.ty != Type::INDEX
        || offset.ty != Type::INDEX
        || !pointer_type(&base.ty)
        || !pointer_type(&pointer.ty)
        || !store.results.is_empty()
        || !matches!(length.kind, OperationKind::SliceLength { slice } if slice == inputs.slice)
        || !matches!(extent.kind, OperationKind::Compare { predicate: ComparePredicate::LessThan, lhs, rhs }
            if lhs == inputs.index && rhs == length_value.id)
        || zero.kind != OperationKind::Constant(Constant::Index(0))
        || !matches!(select.kind, OperationKind::Select { condition, true_value, false_value }
            if condition == predicate && true_value == inputs.index && false_value == zero_value.id)
        || !matches!(data.kind, OperationKind::SliceData { slice } if slice == inputs.slice)
        || !matches!(address.kind, OperationKind::GetElementPointer { base: value, offset: index }
            if value == base.id && index == offset.id)
        || !matches!(store.kind, OperationKind::GuardedStore { pointer: address, predicate: condition, value, access }
            if address == pointer.id && condition == predicate && value == inputs.value
                && access == MemoryAccess::new(AddressSpace::Global, alignment))
    {
        return Err(source_issued_error_v29());
    }
    let definitions = [
        length_value.id,
        extent_value.id,
        zero_value.id,
        offset.id,
        base.id,
        pointer.id,
        predicate,
    ];
    let arguments = [
        inputs.slice,
        inputs.index,
        inputs.value,
        inputs.precondition.unwrap_or(inputs.index),
    ];
    // Without a mapping precondition the predicate is exactly the extent result.
    for (ordinal, value) in definitions[..6 + extra].iter().enumerate() {
        if definitions[..ordinal].contains(value) || arguments.contains(value) {
            return Err(source_issued_error_v29());
        }
    }
    Ok(CheckedWriteTailV85 {
        length: length_value.id,
        extent: extent_value.id,
        predicate,
        zero: zero_value.id,
        offset: offset.id,
        data: base.id,
        pointer: pointer.id,
    })
}
