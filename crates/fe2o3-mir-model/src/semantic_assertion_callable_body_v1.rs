// Existing source/ABI/statement eligibility rules; not owner admission.
fn scalar_defined_callable_intrinsic_eligibility_v1(
    operation: SemanticCompilerIntrinsicOperationV1,
    scalar_eligible: bool,
) -> DefinedCallableTerminatorEligibilityV1 {
    match operation {
        SemanticCompilerIntrinsicOperationV1::FabsF32
        | SemanticCompilerIntrinsicOperationV1::SaturatingInteger(_) => {
            DefinedCallableTerminatorEligibilityV1::shared(scalar_eligible)
        }
        SemanticCompilerIntrinsicOperationV1::WorkgroupDimension(_)
        | SemanticCompilerIntrinsicOperationV1::GridDimension(_) => {
            DefinedCallableTerminatorEligibilityV1 {
                empty_eligible: scalar_eligible,
                deterministic_scalar_eligible: false,
            }
        }
        _ => DefinedCallableTerminatorEligibilityV1::shared(false),
    }
}

pub fn scalar_defined_callable_type_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    types.get(ty.index() as usize).is_some_and(|ty| {
        matches!(
            ty.shape(),
            SemanticTypeShapeV1::Unit
                | SemanticTypeShapeV1::Never
                | SemanticTypeShapeV1::Scalar(_)
                | SemanticTypeShapeV1::ValidityScalar(_)
        )
    })
}

pub fn checked_scalar_carrier_type_v1(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1)> {
    let SemanticTypeShapeV1::Tuple(fields) = types.get(ty.index() as usize)?.shape() else {
        return None;
    };
    let [value, overflow] = fields.fields() else {
        return None;
    };
    if !scalar_defined_callable_type_v1(types, *value)
        || !matches!(
            types.get(overflow.index() as usize).map(|ty| ty.shape()),
            Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
        )
    {
        return None;
    }
    Some((*value, *overflow))
}

fn scalar_or_checked_carrier_type_v1(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    scalar_defined_callable_type_v1(types, ty)
        || checked_scalar_carrier_type_v1(types, ty).is_some()
}

fn zero_sized_defined_callable_place_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    work: &mut CallableWork<'_, M>,
) -> Result<bool, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, sum(1, place.projections().len())?)?;
    Ok(place.projections().is_empty()
        && zero_sized_defined_callable_type_v1(types, place.ty(), work)?
        && function
            .locals()
            .get(place.local().index() as usize)
            .is_some_and(|local| local.ty() == place.ty()))
}

fn zero_sized_defined_callable_operand_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    work: &mut CallableWork<'_, M>,
) -> Result<bool, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, 1)?;
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            zero_sized_defined_callable_place_v1(types, function, place, work)
        }
        SemanticOperandV1::Constant(constant) => {
            Ok(
                zero_sized_defined_callable_type_v1(types, constant.ty(), work)?
                    && matches!(constant.value(), SemanticConstantValueV1::ZeroSized),
            )
        }
    }
}

fn zero_sized_defined_callable_statement_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    statement: &crate::semantic_mir_v1::SemanticStatementV1,
    work: &mut CallableWork<'_, M>,
) -> Result<bool, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, 1)?;
    match statement.kind() {
        SemanticStatementKindV1::Assign(assignment) => {
            if !zero_sized_defined_callable_place_v1(
                types,
                function,
                assignment.destination(),
                work,
            )? || !zero_sized_defined_callable_type_v1(
                types,
                assignment.value().result_type(),
                work,
            )? {
                return Ok(false);
            }
            match assignment.value().kind() {
                SemanticRvalueKindV1::Use(operand) => {
                    zero_sized_defined_callable_operand_v1(types, function, operand, work)
                }
                SemanticRvalueKindV1::Aggregate(aggregate) => {
                    for operand in aggregate.operands() {
                        if !zero_sized_defined_callable_operand_v1(types, function, operand, work)?
                        {
                            return Ok(false);
                        }
                    }
                    Ok(true)
                }
                SemanticRvalueKindV1::Unary { .. }
                | SemanticRvalueKindV1::Binary { .. }
                | SemanticRvalueKindV1::CheckedBinary(_)
                | SemanticRvalueKindV1::UncheckedBinary(_)
                | SemanticRvalueKindV1::Cast { .. }
                | SemanticRvalueKindV1::Borrow { .. }
                | SemanticRvalueKindV1::AddressOf { .. }
                | SemanticRvalueKindV1::Length(_)
                | SemanticRvalueKindV1::Discriminant(_)
                | SemanticRvalueKindV1::Load(_) => Ok(false),
            }
        }
        SemanticStatementKindV1::StorageLive(local)
        | SemanticStatementKindV1::StorageDead(local) => {
            let Some(local) = function.locals().get(local.index() as usize) else {
                return Ok(false);
            };
            zero_sized_defined_callable_type_v1(types, local.ty(), work)
        }
        SemanticStatementKindV1::Nop => Ok(true),
        SemanticStatementKindV1::Store(_)
        | SemanticStatementKindV1::AtomicRmw(_)
        | SemanticStatementKindV1::AtomicCompareExchange(_)
        | SemanticStatementKindV1::SetDiscriminant { .. }
        | SemanticStatementKindV1::Deinitialize(_)
        | SemanticStatementKindV1::Assume(_) => Ok(false),
    }
}

fn scalar_direct_abi_value_v1(
    types: &[SemanticTypeDeclV1],
    value: &crate::semantic_mir_v1::SemanticAbiValueV1,
) -> bool {
    scalar_defined_callable_type_v1(types, value.source_ty())
        && value.adjusted().is_none()
        && value.pointee_override().is_none()
        && matches!(
            value.mode(),
            SemanticAbiPassModeV1::Ignore | SemanticAbiPassModeV1::Direct(_)
        )
}

fn scalar_defined_callable_place_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    work: &mut CallableWork<'_, M>,
) -> Result<bool, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, sum(1, place.projections().len())?)?;
    let Some(local) = function.locals().get(place.local().index() as usize) else {
        return Ok(false);
    };
    match place.projections() {
        [] => Ok(local.ty() == place.ty() && scalar_defined_callable_type_v1(types, place.ty())),
        [projection] => {
            let SemanticProjectionKindV1::Field(field) = projection.kind() else {
                return Ok(false);
            };
            if let Some(carrier_field) = transparent_carrier(types, local.ty(), work)? {
                return Ok(field == 0
                    && projection.result_type() == carrier_field
                    && place.ty() == carrier_field);
            }
            let Some((value, overflow)) = checked_scalar_carrier_type_v1(types, local.ty()) else {
                return Ok(false);
            };
            let expected = match field {
                0 => value,
                1 => overflow,
                _ => return Ok(false),
            };
            Ok(projection.result_type() == expected && place.ty() == expected)
        }
        _ => Ok(false),
    }
}

fn scalar_or_checked_carrier_destination_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    work: &mut CallableWork<'_, M>,
) -> Result<bool, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, sum(1, place.projections().len())?)?;
    Ok(place.projections().is_empty()
        && scalar_or_checked_carrier_type_v1(types, place.ty())
        && function
            .locals()
            .get(place.local().index() as usize)
            .is_some_and(|local| local.ty() == place.ty()))
}

fn scalar_defined_callable_destination_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    place: &SemanticPlaceV1,
    work: &mut CallableWork<'_, M>,
) -> Result<bool, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, sum(1, place.projections().len())?)?;
    Ok(place.projections().is_empty()
        && scalar_defined_callable_type_v1(types, place.ty())
        && function
            .locals()
            .get(place.local().index() as usize)
            .is_some_and(|local| local.ty() == place.ty()))
}

fn scalar_defined_callable_operand_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    work: &mut CallableWork<'_, M>,
) -> Result<bool, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, 1)?;
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            scalar_defined_callable_place_v1(types, function, place, work)
        }
        SemanticOperandV1::Constant(constant) => {
            Ok(scalar_defined_callable_type_v1(types, constant.ty())
                && matches!(
                    constant.value(),
                    SemanticConstantValueV1::ZeroSized | SemanticConstantValueV1::Scalar(_)
                ))
        }
    }
}

fn scalar_defined_callable_rvalue_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    value: &SemanticRvalueV1,
    work: &mut CallableWork<'_, M>,
) -> Result<bool, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, 1)?;
    match value.kind() {
        SemanticRvalueKindV1::Use(operand) | SemanticRvalueKindV1::Unary { operand, .. } => {
            Ok(scalar_defined_callable_type_v1(types, value.result_type())
                && scalar_defined_callable_operand_v1(types, function, operand, work)?)
        }
        SemanticRvalueKindV1::Binary { left, right, .. } => {
            Ok(scalar_defined_callable_type_v1(types, value.result_type())
                && scalar_defined_callable_operand_v1(types, function, left, work)?
                && scalar_defined_callable_operand_v1(types, function, right, work)?)
        }
        SemanticRvalueKindV1::CheckedBinary(checked) => {
            Ok(
                checked_scalar_carrier_type_v1(types, value.result_type()).is_some_and(
                    |(result, _)| result == checked.left().ty() && result == checked.right().ty(),
                ) && scalar_defined_callable_operand_v1(types, function, checked.left(), work)?
                    && scalar_defined_callable_operand_v1(types, function, checked.right(), work)?,
            )
        }
        SemanticRvalueKindV1::UncheckedBinary(unchecked) => {
            Ok(scalar_defined_callable_type_v1(types, value.result_type())
                && scalar_defined_callable_operand_v1(types, function, unchecked.left(), work)?
                && scalar_defined_callable_operand_v1(types, function, unchecked.right(), work)?)
        }
        SemanticRvalueKindV1::Cast {
            kind: SemanticCastKindV1::Integer | SemanticCastKindV1::Float,
            operand,
        } => Ok(scalar_defined_callable_type_v1(types, value.result_type())
            && scalar_defined_callable_operand_v1(types, function, operand, work)?),
        SemanticRvalueKindV1::Cast { .. }
        | SemanticRvalueKindV1::Borrow { .. }
        | SemanticRvalueKindV1::AddressOf { .. }
        | SemanticRvalueKindV1::Length(_)
        | SemanticRvalueKindV1::Discriminant(_)
        | SemanticRvalueKindV1::Aggregate(_)
        | SemanticRvalueKindV1::Load(_) => Ok(false),
    }
}

fn scalar_defined_callable_statement_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    statement: &crate::semantic_mir_v1::SemanticStatementV1,
    work: &mut CallableWork<'_, M>,
) -> Result<bool, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, 1)?;
    match statement.kind() {
        SemanticStatementKindV1::Assign(assignment) => {
            Ok(scalar_or_checked_carrier_destination_v1(
                types,
                function,
                assignment.destination(),
                work,
            )? && scalar_defined_callable_rvalue_v1(types, function, assignment.value(), work)?)
        }
        SemanticStatementKindV1::StorageLive(local)
        | SemanticStatementKindV1::StorageDead(local) => {
            let Some(local) = function.locals().get(local.index() as usize) else {
                return Ok(false);
            };
            Ok(scalar_or_checked_carrier_type_v1(types, local.ty())
                || transparent_carrier(types, local.ty(), work)?.is_some())
        }
        SemanticStatementKindV1::Assume(condition) => {
            scalar_defined_callable_operand_v1(types, function, condition, work)
        }
        SemanticStatementKindV1::Nop => Ok(true),
        SemanticStatementKindV1::Store(_)
        | SemanticStatementKindV1::AtomicRmw(_)
        | SemanticStatementKindV1::AtomicCompareExchange(_)
        | SemanticStatementKindV1::SetDiscriminant { .. }
        | SemanticStatementKindV1::Deinitialize(_) => Ok(false),
    }
}

fn deterministic_scalar_defined_callable_statement_v1(
    statement: &crate::semantic_mir_v1::SemanticStatementV1,
) -> bool {
    match statement.kind() {
        SemanticStatementKindV1::Assign(assignment) => matches!(
            assignment.value().kind(),
            SemanticRvalueKindV1::Use(_)
                | SemanticRvalueKindV1::Unary { .. }
                | SemanticRvalueKindV1::Binary { .. }
                | SemanticRvalueKindV1::CheckedBinary(_)
                | SemanticRvalueKindV1::Cast {
                    kind: SemanticCastKindV1::Integer | SemanticCastKindV1::Float,
                    ..
                }
        ),
        SemanticStatementKindV1::StorageLive(_)
        | SemanticStatementKindV1::StorageDead(_)
        | SemanticStatementKindV1::Nop => true,
        SemanticStatementKindV1::Assume(_)
        | SemanticStatementKindV1::Store(_)
        | SemanticStatementKindV1::AtomicRmw(_)
        | SemanticStatementKindV1::AtomicCompareExchange(_)
        | SemanticStatementKindV1::SetDiscriminant { .. }
        | SemanticStatementKindV1::Deinitialize(_) => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn scalar_defined_callable_call_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_count: usize,
    callables: &[SemanticCallableDeclV1],
    call: &SemanticDirectCallV1,
    callees: &mut Vec<usize>,
    call_edges: &mut usize,
    work: &mut CallableWork<'_, M>,
) -> Result<DefinedCallableTerminatorEligibilityV1, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, sum(1, call.arguments().len())?)?;
    let mut eligible = call.variadic_argument_abis().is_empty()
        && matches!(
            call.unwind(),
            SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
        );
    for argument in call.arguments() {
        eligible &= scalar_defined_callable_operand_v1(types, function, argument, work)?;
    }
    if let Some(destination) = call.destination() {
        eligible &=
            scalar_defined_callable_destination_v1(types, function, destination.place(), work)?;
    }
    let Some(callable) = callables.get(call.callee().index() as usize) else {
        return Ok(DefinedCallableTerminatorEligibilityV1::shared(false));
    };
    if let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable {
        return Ok(scalar_defined_callable_intrinsic_eligibility_v1(
            *operation, eligible,
        ));
    }
    let SemanticCallableDeclV1::Defined { function: callee } = callable else {
        return Ok(DefinedCallableTerminatorEligibilityV1::shared(false));
    };
    let callee = callee.index() as usize;
    if callee >= function_count {
        return Ok(DefinedCallableTerminatorEligibilityV1::shared(false));
    }
    *call_edges = call_edges.checked_add(1).ok_or(analysis_error::<M>(
        SemanticAssertionErrorV1::Unsupported(
            "defined-callable effect-summary edge count overflowed",
        ),
    ))?;
    if *call_edges > MAX_SEMANTIC_CALLABLE_EDGES_V1 {
        return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
            "defined-callable effect-summary edge count exceeded its production limit",
        )));
    }
    work.paid().grow(callees, 1)?;
    callees.push(callee);
    Ok(DefinedCallableTerminatorEligibilityV1::shared(eligible))
}

#[allow(clippy::too_many_arguments)]
fn scalar_defined_callable_tail_call_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_count: usize,
    callables: &[SemanticCallableDeclV1],
    call: &SemanticDirectTailCallV1,
    callees: &mut Vec<usize>,
    call_edges: &mut usize,
    work: &mut CallableWork<'_, M>,
) -> Result<DefinedCallableTerminatorEligibilityV1, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, sum(1, call.arguments().len())?)?;
    let mut eligible = matches!(
        call.unwind(),
        SemanticUnwindActionV1::Continue | SemanticUnwindActionV1::Unreachable
    );
    for argument in call.arguments() {
        eligible &= scalar_defined_callable_operand_v1(types, function, argument, work)?;
    }
    let Some(callable) = callables.get(call.callee().index() as usize) else {
        return Ok(DefinedCallableTerminatorEligibilityV1::shared(false));
    };
    if let SemanticCallableDeclV1::CompilerIntrinsic { operation, .. } = callable {
        return Ok(scalar_defined_callable_intrinsic_eligibility_v1(
            *operation, eligible,
        ));
    }
    let SemanticCallableDeclV1::Defined { function: callee } = callable else {
        return Ok(DefinedCallableTerminatorEligibilityV1::shared(false));
    };
    let callee = callee.index() as usize;
    if callee >= function_count {
        return Ok(DefinedCallableTerminatorEligibilityV1::shared(false));
    }
    *call_edges = call_edges.checked_add(1).ok_or(analysis_error::<M>(
        SemanticAssertionErrorV1::Unsupported(
            "defined-callable effect-summary edge count overflowed",
        ),
    ))?;
    if *call_edges > MAX_SEMANTIC_CALLABLE_EDGES_V1 {
        return Err(analysis_error::<M>(SemanticAssertionErrorV1::Unsupported(
            "defined-callable effect-summary edge count exceeded its production limit",
        )));
    }
    work.paid().grow(callees, 1)?;
    callees.push(callee);
    Ok(DefinedCallableTerminatorEligibilityV1::shared(eligible))
}

fn scalar_defined_callable_assert_message_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    message: &SemanticAssertMessageV1,
    work: &mut CallableWork<'_, M>,
) -> Result<bool, ErrorFor<M>> {
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index }
        | SemanticAssertMessageV1::Overflow {
            left: length,
            right: index,
            ..
        }
        | SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment: length,
            found_alignment: index,
        } => Ok(
            scalar_defined_callable_operand_v1(types, function, length, work)?
                && scalar_defined_callable_operand_v1(types, function, index, work)?,
        ),
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => {
            scalar_defined_callable_operand_v1(types, function, operand, work)
        }
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => Ok(true),
    }
}

#[allow(clippy::too_many_arguments)]
fn scalar_defined_callable_terminator_v1<M: SemanticAssertionMeterV1>(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    function_count: usize,
    callables: &[SemanticCallableDeclV1],
    terminator: &SemanticTerminatorKindV1,
    callees: &mut Vec<usize>,
    call_edges: &mut usize,
    work: &mut CallableWork<'_, M>,
) -> Result<DefinedCallableTerminatorEligibilityV1, ErrorFor<M>> {
    charge_defined_callable_summary_work_v1(work, 1)?;
    match terminator {
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::Unreachable => {
            Ok(DefinedCallableTerminatorEligibilityV1::shared(true))
        }
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => {
            Ok(DefinedCallableTerminatorEligibilityV1::shared(
                scalar_defined_callable_operand_v1(types, function, discriminant, work)?,
            ))
        }
        SemanticTerminatorKindV1::Call(call) => scalar_defined_callable_call_v1(
            types,
            function,
            function_count,
            callables,
            call,
            callees,
            call_edges,
            work,
        ),
        SemanticTerminatorKindV1::TailCall(call) => scalar_defined_callable_tail_call_v1(
            types,
            function,
            function_count,
            callables,
            call,
            callees,
            call_edges,
            work,
        ),
        SemanticTerminatorKindV1::Assert {
            condition,
            message,
            target,
            unwind,
            ..
        } => Ok(DefinedCallableTerminatorEligibilityV1::shared(
            target.role() == SemanticEdgeRoleV1::AssertSuccess
                && matches!(unwind, SemanticUnwindActionV1::Unreachable)
                && scalar_defined_callable_operand_v1(types, function, condition, work)?
                && scalar_defined_callable_assert_message_v1(types, function, message, work)?,
        )),
        SemanticTerminatorKindV1::Drop { .. }
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort => {
            Ok(DefinedCallableTerminatorEligibilityV1::shared(false))
        }
    }
}
