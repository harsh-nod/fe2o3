//! Exhaustive, shallow MIR remapping. No expression or call-graph recursion.

use super::*;

pub(super) fn local(
    local: SemanticLocalIdV1,
    instance: &SemanticCallInstanceV1,
) -> SemanticLocalIdV1 {
    SemanticLocalIdV1::from_index(instance.local_start + local.index())
}
pub(super) fn block(
    block: SemanticBlockIdV1,
    instance: &SemanticCallInstanceV1,
) -> SemanticBlockIdV1 {
    SemanticBlockIdV1::from_index(instance.block_start + block.index())
}
fn edge(
    edge: SemanticControlFlowEdgeV1,
    instance: &SemanticCallInstanceV1,
) -> SemanticControlFlowEdgeV1 {
    SemanticControlFlowEdgeV1::new(edge.role(), block(edge.target(), instance))
}
fn place(
    place: &SemanticPlaceV1,
    instance: &SemanticCallInstanceV1,
    budget: &mut Budget,
) -> Result<SemanticPlaceV1> {
    budget.work(1 + place.projections().len())?;
    let projections = place
        .projections()
        .iter()
        .map(|projection| {
            SemanticProjectionV1::new(
                match projection.kind() {
                    SemanticProjectionKindV1::Index(index) => {
                        SemanticProjectionKindV1::Index(local(index, instance))
                    }
                    kind => kind,
                },
                projection.result_type(),
            )
        })
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(SemanticPlaceV1::new(
        local(place.local(), instance),
        projections,
        place.ty(),
    )?)
}
pub(super) fn operand(
    operand: &SemanticOperandV1,
    instance: &SemanticCallInstanceV1,
    budget: &mut Budget,
) -> Result<SemanticOperandV1> {
    budget.work(1)?;
    Ok(match operand {
        SemanticOperandV1::Copy(value) => SemanticOperandV1::Copy(place(value, instance, budget)?),
        SemanticOperandV1::Move(value) => SemanticOperandV1::Move(place(value, instance, budget)?),
        SemanticOperandV1::Constant(value) => {
            if let SemanticConstantValueV1::Bytes(bytes) = value.value() {
                budget.work(bytes.as_bytes().len())?;
            }
            SemanticOperandV1::Constant(value.clone())
        }
    })
}
fn rvalue(
    value: &SemanticRvalueV1,
    instance: &SemanticCallInstanceV1,
    budget: &mut Budget,
) -> Result<SemanticRvalueV1> {
    budget.work(1)?;
    let kind = match value.kind() {
        SemanticRvalueKindV1::Use(value) => {
            SemanticRvalueKindV1::Use(operand(value, instance, budget)?)
        }
        SemanticRvalueKindV1::Unary {
            operation,
            operand: value,
        } => SemanticRvalueKindV1::Unary {
            operation: *operation,
            operand: operand(value, instance, budget)?,
        },
        SemanticRvalueKindV1::Binary {
            operation,
            left,
            right,
        } => SemanticRvalueKindV1::Binary {
            operation: *operation,
            left: operand(left, instance, budget)?,
            right: operand(right, instance, budget)?,
        },
        SemanticRvalueKindV1::CheckedBinary(value) => {
            SemanticRvalueKindV1::CheckedBinary(SemanticCheckedBinaryRvalueV1::new(
                value.operation(),
                operand(value.left(), instance, budget)?,
                operand(value.right(), instance, budget)?,
            ))
        }
        SemanticRvalueKindV1::UncheckedBinary(value) => {
            SemanticRvalueKindV1::UncheckedBinary(SemanticUncheckedBinaryRvalueV1::new(
                value.operation(),
                operand(value.left(), instance, budget)?,
                operand(value.right(), instance, budget)?,
            ))
        }
        SemanticRvalueKindV1::Cast {
            kind,
            operand: value,
        } => SemanticRvalueKindV1::Cast {
            kind: *kind,
            operand: operand(value, instance, budget)?,
        },
        SemanticRvalueKindV1::Borrow { kind, place: value } => SemanticRvalueKindV1::Borrow {
            kind: *kind,
            place: place(value, instance, budget)?,
        },
        SemanticRvalueKindV1::AddressOf {
            mutability,
            place: value,
        } => SemanticRvalueKindV1::AddressOf {
            mutability: *mutability,
            place: place(value, instance, budget)?,
        },
        SemanticRvalueKindV1::Length(value) => {
            SemanticRvalueKindV1::Length(place(value, instance, budget)?)
        }
        SemanticRvalueKindV1::Discriminant(value) => {
            SemanticRvalueKindV1::Discriminant(place(value, instance, budget)?)
        }
        SemanticRvalueKindV1::Aggregate(value) => {
            SemanticRvalueKindV1::Aggregate(SemanticAggregateRvalueV1::new(
                value.kind().clone(),
                value
                    .operands()
                    .iter()
                    .map(|value| operand(value, instance, budget))
                    .collect::<Result<Vec<_>>>()?,
            )?)
        }
        SemanticRvalueKindV1::Load(value) => SemanticRvalueKindV1::Load(SemanticMemoryLoadV1::new(
            place(value.source(), instance, budget)?,
            value.volatility(),
            value.atomic(),
        )),
    };
    Ok(SemanticRvalueV1::new(value.result_type(), kind))
}
pub(super) fn statement(
    statement: &SemanticStatementV1,
    instance: &SemanticCallInstanceV1,
    budget: &mut Budget,
) -> Result<SemanticStatementV1> {
    budget.work(1)?;
    let kind = match statement.kind() {
        SemanticStatementKindV1::Assign(value) => {
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(value.destination(), instance, budget)?,
                rvalue(value.value(), instance, budget)?,
            ))
        }
        SemanticStatementKindV1::Store(value) => {
            SemanticStatementKindV1::Store(SemanticMemoryStoreV1::new(
                place(value.destination(), instance, budget)?,
                operand(value.value(), instance, budget)?,
                value.volatility(),
                value.atomic(),
            ))
        }
        SemanticStatementKindV1::AtomicRmw(value) => {
            SemanticStatementKindV1::AtomicRmw(SemanticAtomicRmwV1::new(
                place(value.destination(), instance, budget)?,
                place(value.address(), instance, budget)?,
                operand(value.value(), instance, budget)?,
                value.operation(),
                value.access(),
            ))
        }
        SemanticStatementKindV1::AtomicCompareExchange(value) => {
            SemanticStatementKindV1::AtomicCompareExchange(SemanticAtomicCompareExchangeV1::new(
                place(value.destination(), instance, budget)?,
                place(value.address(), instance, budget)?,
                operand(value.expected(), instance, budget)?,
                operand(value.replacement(), instance, budget)?,
                value.success(),
                value.failure_ordering(),
                value.is_weak(),
            ))
        }
        SemanticStatementKindV1::SetDiscriminant {
            place: value,
            variant_index,
        } => SemanticStatementKindV1::SetDiscriminant {
            place: place(value, instance, budget)?,
            variant_index: *variant_index,
        },
        SemanticStatementKindV1::Deinitialize(value) => {
            SemanticStatementKindV1::Deinitialize(place(value, instance, budget)?)
        }
        SemanticStatementKindV1::StorageLive(value) => {
            SemanticStatementKindV1::StorageLive(local(*value, instance))
        }
        SemanticStatementKindV1::StorageDead(value) => {
            SemanticStatementKindV1::StorageDead(local(*value, instance))
        }
        SemanticStatementKindV1::Assume(value) => {
            SemanticStatementKindV1::Assume(operand(value, instance, budget)?)
        }
        SemanticStatementKindV1::Nop => SemanticStatementKindV1::Nop,
    };
    Ok(SemanticStatementV1::new(statement.source(), kind))
}
pub(super) fn destination(
    destination: &SemanticCallDestinationV1,
    instance: &SemanticCallInstanceV1,
    budget: &mut Budget,
) -> Result<SemanticCallDestinationV1> {
    Ok(SemanticCallDestinationV1::new(
        place(destination.place(), instance, budget)?,
        edge(destination.edge(), instance),
    ))
}
fn assert_message(
    message: &SemanticAssertMessageV1,
    instance: &SemanticCallInstanceV1,
    budget: &mut Budget,
) -> Result<SemanticAssertMessageV1> {
    Ok(match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => {
            SemanticAssertMessageV1::BoundsCheck {
                length: operand(length, instance, budget)?,
                index: operand(index, instance, budget)?,
            }
        }
        SemanticAssertMessageV1::Overflow {
            operation,
            left,
            right,
        } => SemanticAssertMessageV1::Overflow {
            operation: *operation,
            left: operand(left, instance, budget)?,
            right: operand(right, instance, budget)?,
        },
        SemanticAssertMessageV1::DivisionByZero(value) => {
            SemanticAssertMessageV1::DivisionByZero(operand(value, instance, budget)?)
        }
        SemanticAssertMessageV1::RemainderByZero(value) => {
            SemanticAssertMessageV1::RemainderByZero(operand(value, instance, budget)?)
        }
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment: operand(required_alignment, instance, budget)?,
            found_alignment: operand(found_alignment, instance, budget)?,
        },
        SemanticAssertMessageV1::NullPointerDereference => {
            SemanticAssertMessageV1::NullPointerDereference
        }
        SemanticAssertMessageV1::ResumedAfterReturn => SemanticAssertMessageV1::ResumedAfterReturn,
        SemanticAssertMessageV1::ResumedAfterPanic => SemanticAssertMessageV1::ResumedAfterPanic,
    })
}
pub(super) fn terminator(
    kind: &SemanticTerminatorKindV1,
    instance: &SemanticCallInstanceV1,
    source_block: SemanticBlockIdV1,
    budget: &mut Budget,
) -> Result<SemanticTerminatorKindV1> {
    budget.work(1)?;
    let reject = |reason| unsupported(instance.function, Some(source_block), reason);
    Ok(match kind {
        SemanticTerminatorKindV1::Goto(target) => {
            SemanticTerminatorKindV1::Goto(edge(*target, instance))
        }
        SemanticTerminatorKindV1::SwitchInt {
            discriminant,
            targets,
        } => {
            budget.work(targets.values().len())?;
            SemanticTerminatorKindV1::SwitchInt {
                discriminant: operand(discriminant, instance, budget)?,
                targets: SemanticSwitchTargetsV1::new(
                    targets
                        .values()
                        .iter()
                        .map(|target| {
                            SemanticSwitchTargetV1::new(
                                target.value(),
                                edge(target.edge(), instance),
                            )
                        })
                        .collect(),
                    edge(targets.otherwise(), instance),
                )?,
            }
        }
        SemanticTerminatorKindV1::Call(call) => {
            if call.unwind() != SemanticUnwindActionV1::Unreachable {
                return Err(reject("call unwind is not unreachable"));
            }
            if !call.variadic_argument_abis().is_empty() {
                return Err(reject("variadic call"));
            }
            SemanticTerminatorKindV1::Call(SemanticDirectCallV1::new_callable(
                call.callee(),
                call.arguments()
                    .iter()
                    .map(|value| operand(value, instance, budget))
                    .collect::<Result<Vec<_>>>()?,
                call.destination()
                    .map(|value| destination(value, instance, budget))
                    .transpose()?,
                call.unwind(),
            )?)
        }
        SemanticTerminatorKindV1::Assert {
            condition,
            expected,
            message,
            target,
            unwind,
        } => {
            if *unwind != SemanticUnwindActionV1::Unreachable {
                return Err(reject("assert unwind is not unreachable"));
            }
            SemanticTerminatorKindV1::Assert {
                condition: operand(condition, instance, budget)?,
                expected: *expected,
                message: assert_message(message, instance, budget)?,
                target: edge(*target, instance),
                unwind: *unwind,
            }
        }
        SemanticTerminatorKindV1::FalseEdge {
            real_target,
            imaginary_target,
        } => SemanticTerminatorKindV1::FalseEdge {
            real_target: edge(*real_target, instance),
            imaginary_target: edge(*imaginary_target, instance),
        },
        SemanticTerminatorKindV1::Return => SemanticTerminatorKindV1::Return,
        SemanticTerminatorKindV1::Abort => SemanticTerminatorKindV1::Abort,
        SemanticTerminatorKindV1::Unreachable => SemanticTerminatorKindV1::Unreachable,
        SemanticTerminatorKindV1::TailCall(_) => {
            return Err(reject("tail call expansion is unsupported"));
        }
        SemanticTerminatorKindV1::Drop { .. } => {
            return Err(reject("drop-glue expansion is unsupported"));
        }
        SemanticTerminatorKindV1::UnwindResume | SemanticTerminatorKindV1::UnwindTerminate => {
            return Err(reject("unwind terminator is unsupported"));
        }
    })
}
