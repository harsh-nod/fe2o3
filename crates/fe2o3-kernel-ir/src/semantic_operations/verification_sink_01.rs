use super::*;

pub(crate) trait SemanticOperationIssueSinkV1 {
    type Error;

    fn charge_work(&mut self, amount: usize) -> Result<(), Self::Error>;

    fn emit(
        &mut self,
        kind: SemanticOperationIssueKind,
        message_work_upper: usize,
        arguments: fmt::Arguments<'_>,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy)]
pub(crate) struct SemanticOperationBorrowedVerificationContextV1<'a> {
    pub(crate) operands: &'a [ValueId],
    pub(crate) results: &'a [ValueDef],
    pub(crate) operand_types: &'a [Option<&'a Type>],
}

pub(super) trait SemanticOperandTypeViewV1 {
    fn len(&self) -> usize;
    fn get_type(&self, index: usize) -> Option<Option<&Type>>;
}

impl SemanticOperandTypeViewV1 for [Option<Type>] {
    fn len(&self) -> usize {
        <[Option<Type>]>::len(self)
    }

    fn get_type(&self, index: usize) -> Option<Option<&Type>> {
        self.get(index).map(Option::as_ref)
    }
}

impl SemanticOperandTypeViewV1 for [Option<&Type>] {
    fn len(&self) -> usize {
        <[Option<&Type>]>::len(self)
    }

    fn get_type(&self, index: usize) -> Option<Option<&Type>> {
        self.get(index).copied()
    }
}

pub(super) struct LegacySemanticOperationIssueSinkV1<'a> {
    pub(super) issues: &'a mut Vec<SemanticOperationIssue>,
}

impl SemanticOperationIssueSinkV1 for LegacySemanticOperationIssueSinkV1<'_> {
    type Error = std::convert::Infallible;

    fn charge_work(&mut self, _amount: usize) -> Result<(), Self::Error> {
        Ok(())
    }

    fn emit(
        &mut self,
        kind: SemanticOperationIssueKind,
        _message_work_upper: usize,
        arguments: fmt::Arguments<'_>,
    ) -> Result<(), Self::Error> {
        self.issues
            .push(SemanticOperationIssue::new(kind, arguments.to_string()));
        Ok(())
    }
}

pub(super) const SEMANTIC_FIXED_MESSAGE_WORK_UPPER_V1: usize = 512;
const SEMANTIC_TYPE_NODE_MESSAGE_WORK_UPPER_V1: usize = 128;

pub(super) fn semantic_type_message_work_upper_v1<S: SemanticOperationIssueSinkV1>(
    mut ty: &Type,
    sink: &mut S,
) -> Result<usize, S::Error> {
    let mut nodes = 0_usize;
    loop {
        sink.charge_work(1)?;
        nodes = nodes.saturating_add(1);
        ty = match ty {
            Type::Pointer(pointer) => pointer.pointee.as_ref(),
            Type::Slice(slice) => slice.element.as_ref(),
            Type::Unit | Type::Scalar(_) | Type::Vector(_) | Type::Execution(_) => break,
        };
    }
    Ok(SEMANTIC_FIXED_MESSAGE_WORK_UPPER_V1
        .saturating_add(nodes.saturating_mul(SEMANTIC_TYPE_NODE_MESSAGE_WORK_UPPER_V1)))
}

pub(super) fn semantic_types_equal_v1<S: SemanticOperationIssueSinkV1>(
    mut actual: &Type,
    mut expected: &Type,
    sink: &mut S,
) -> Result<bool, S::Error> {
    loop {
        sink.charge_work(1)?;
        match (actual, expected) {
            (Type::Unit, Type::Unit) => return Ok(true),
            (Type::Scalar(actual), Type::Scalar(expected)) => return Ok(actual == expected),
            (Type::Vector(actual), Type::Vector(expected)) => return Ok(actual == expected),
            (Type::Execution(actual), Type::Execution(expected)) => return Ok(actual == expected),
            (Type::Pointer(actual_pointer), Type::Pointer(expected_pointer)) => {
                if actual_pointer.address_space != expected_pointer.address_space
                    || actual_pointer.access != expected_pointer.access
                {
                    return Ok(false);
                }
                actual = actual_pointer.pointee.as_ref();
                expected = expected_pointer.pointee.as_ref();
            }
            (Type::Slice(actual_slice), Type::Slice(expected_slice)) => {
                if actual_slice.address_space != expected_slice.address_space
                    || actual_slice.access != expected_slice.access
                {
                    return Ok(false);
                }
                actual = actual_slice.element.as_ref();
                expected = expected_slice.element.as_ref();
            }
            _ => return Ok(false),
        }
    }
}

pub(super) fn emit_semantic_fixed_v1<S: SemanticOperationIssueSinkV1>(
    sink: &mut S,
    kind: SemanticOperationIssueKind,
    message: &'static str,
) -> Result<(), S::Error> {
    sink.emit(kind, message.len(), format_args!("{message}"))
}

fn try_verify_semantic_shape_with_sink_v1<
    T: SemanticOperandTypeViewV1 + ?Sized,
    S: SemanticOperationIssueSinkV1,
>(
    operands: &[ValueId],
    results: &[ValueDef],
    operand_types: &T,
    expected_operand_count: usize,
    expected_result_types: &[Type],
    sink: &mut S,
) -> Result<(), S::Error> {
    sink.charge_work(1)?;
    if operand_types.len() != operands.len() {
        sink.emit(
            SemanticOperationIssueKind::InvalidStructure,
            SEMANTIC_FIXED_MESSAGE_WORK_UPPER_V1,
            format_args!(
                "semantic verifier received {} operands but {} operand types",
                operands.len(),
                operand_types.len()
            ),
        )?;
    }
    sink.charge_work(1)?;
    if operands.len() != expected_operand_count {
        sink.emit(
            SemanticOperationIssueKind::InvalidStructure,
            SEMANTIC_FIXED_MESSAGE_WORK_UPPER_V1,
            format_args!(
                "operation contains {} operands but schema requires {expected_operand_count}",
                operands.len()
            ),
        )?;
    }
    sink.charge_work(1)?;
    if results.len() != expected_result_types.len() {
        sink.emit(
            SemanticOperationIssueKind::ResultArity,
            SEMANTIC_FIXED_MESSAGE_WORK_UPPER_V1,
            format_args!(
                "operation defines {} results but {} are required",
                results.len(),
                expected_result_types.len()
            ),
        )?;
    }
    let compared_results = results.len().min(expected_result_types.len());
    sink.charge_work(compared_results)?;
    for (result, expected_ty) in results.iter().zip(expected_result_types) {
        if !semantic_types_equal_v1(&result.ty, expected_ty, sink)? {
            let actual_upper = semantic_type_message_work_upper_v1(&result.ty, sink)?;
            let expected_upper = semantic_type_message_work_upper_v1(expected_ty, sink)?;
            sink.emit(
                SemanticOperationIssueKind::TypeMismatch,
                SEMANTIC_FIXED_MESSAGE_WORK_UPPER_V1
                    .saturating_add(actual_upper)
                    .saturating_add(expected_upper),
                format_args!(
                    "result {} has type {:?}, expected {expected_ty:?}",
                    result.id, result.ty
                ),
            )?;
        }
    }
    Ok(())
}

fn try_verify_memory_intrinsic_with_type_view_v1<
    T: SemanticOperandTypeViewV1 + ?Sized,
    S: SemanticOperationIssueSinkV1,
>(
    operation: &MemoryIntrinsicOperation,
    operands: &[ValueId],
    results: &[ValueDef],
    operand_types: &T,
    sink: &mut S,
) -> Result<(), S::Error> {
    let (operand_count, result_type) = match *operation {
        MemoryIntrinsicOperation::PointerDistance { kind, .. } => (
            2,
            Some(match kind {
                PointerDistanceKind::Signed => Type::Scalar(ScalarType::I64),
                PointerDistanceKind::Unsigned => Type::INDEX,
            }),
        ),
        MemoryIntrinsicOperation::VolatileLoad { element, .. } => (
            1,
            (element != MemoryElementType::Unit).then(|| element.ir_type()),
        ),
        MemoryIntrinsicOperation::VolatileStore { .. } => (2, None),
        MemoryIntrinsicOperation::CopyNonOverlapping { .. } => (3, None),
    };
    let result_types = result_type.as_slice();
    try_verify_semantic_shape_with_sink_v1(
        operands,
        results,
        operand_types,
        operand_count,
        result_types,
        sink,
    )?;
    try_verify_memory_intrinsic_additional_with_sink_v1(operation, operand_types, sink)
}

pub(crate) fn try_verify_memory_intrinsic_with_sink_v1<S: SemanticOperationIssueSinkV1>(
    operation: &MemoryIntrinsicOperation,
    context: SemanticOperationBorrowedVerificationContextV1<'_>,
    sink: &mut S,
) -> Result<(), S::Error> {
    try_verify_memory_intrinsic_with_type_view_v1(
        operation,
        context.operands,
        context.results,
        context.operand_types,
        sink,
    )
}

pub(super) fn try_verify_memory_intrinsic_legacy_with_sink_v1<S: SemanticOperationIssueSinkV1>(
    operation: &MemoryIntrinsicOperation,
    context: SemanticOperationVerificationContext<'_>,
    sink: &mut S,
) -> Result<(), S::Error> {
    try_verify_memory_intrinsic_with_type_view_v1(
        operation,
        context.operands,
        context.results,
        context.operand_types,
        sink,
    )
}

fn try_verify_intrinsic_with_type_view_v1<
    T: SemanticOperandTypeViewV1 + ?Sized,
    S: SemanticOperationIssueSinkV1,
>(
    operation: &IntrinsicOperation,
    operands: &[ValueId],
    results: &[ValueDef],
    operand_types: &T,
    sink: &mut S,
) -> Result<(), S::Error> {
    let expected = Type::INDEX;
    try_verify_semantic_shape_with_sink_v1(
        operands,
        results,
        operand_types,
        0,
        std::slice::from_ref(&expected),
        sink,
    )?;
    try_verify_intrinsic_additional_with_sink_v1(operation, sink)
}

pub(crate) fn try_verify_intrinsic_with_sink_v1<S: SemanticOperationIssueSinkV1>(
    operation: &IntrinsicOperation,
    context: SemanticOperationBorrowedVerificationContextV1<'_>,
    sink: &mut S,
) -> Result<(), S::Error> {
    try_verify_intrinsic_with_type_view_v1(
        operation,
        context.operands,
        context.results,
        context.operand_types,
        sink,
    )
}

pub(super) fn try_verify_intrinsic_legacy_with_sink_v1<S: SemanticOperationIssueSinkV1>(
    operation: &IntrinsicOperation,
    context: SemanticOperationVerificationContext<'_>,
    sink: &mut S,
) -> Result<(), S::Error> {
    try_verify_intrinsic_with_type_view_v1(
        operation,
        context.operands,
        context.results,
        context.operand_types,
        sink,
    )
}

pub(super) fn try_verify_intrinsic_additional_with_sink_v1<S: SemanticOperationIssueSinkV1>(
    operation: &IntrinsicOperation,
    sink: &mut S,
) -> Result<(), S::Error> {
    let expected = Type::INDEX;
    sink.charge_work(1)?;
    if !semantic_types_equal_v1(&operation.result_type, &expected, sink)? {
        let actual_upper = semantic_type_message_work_upper_v1(&operation.result_type, sink)?;
        let expected_upper = semantic_type_message_work_upper_v1(&expected, sink)?;
        sink.emit(
            SemanticOperationIssueKind::TypeMismatch,
            SEMANTIC_FIXED_MESSAGE_WORK_UPPER_V1
                .saturating_add(actual_upper)
                .saturating_add(expected_upper),
            format_args!(
                "intrinsic declares result type {:?}, expected {:?}",
                operation.result_type, expected
            ),
        )?;
    }
    Ok(())
}

pub(crate) fn try_verify_semantic_operation_with_sink_v1<S: SemanticOperationIssueSinkV1>(
    operation: &OperationKind,
    context: SemanticOperationBorrowedVerificationContextV1<'_>,
    sink: &mut S,
) -> Result<bool, S::Error> {
    match operation {
        OperationKind::Execution(execution) => {
            try_verify_execution_operation_with_sink_v3(execution, context, sink)?;
            Ok(true)
        }
        OperationKind::Intrinsic(intrinsic) => {
            try_verify_intrinsic_with_sink_v1(intrinsic, context, sink)?;
            Ok(true)
        }
        OperationKind::MemoryIntrinsic(intrinsic) => {
            try_verify_memory_intrinsic_with_sink_v1(intrinsic, context, sink)?;
            Ok(true)
        }
        _ => Ok(false),
    }
}
