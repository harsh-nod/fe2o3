//! Ordinary scalar KIR for an explicitly scheduled masked tile load.

use crate::{
    AccessMode, AddressSpace, Axis, BinaryOp, CheckedBinaryOperator, ComparePredicate, Constant,
    ExecutionTileLayoutV1, ExecutionTileScheduleV1, IndexKind, IntrinsicKind, IntrinsicOperation,
    MemoryAccess, Operation, OperationKind, ScalarType, Type, UnaryOp, ValueDef, ValueId,
};

/// An inert, constant-space emission recipe. This is not a source or launch
/// proof: its integrator must bind the slice/base operands, reserve fresh SSA
/// definitions and establish a one-dimensional workgroup of `schedule.lanes()`.
/// Every emitted operation replaces the original load at its effect position.
/// In particular, unused components still perform their active reads there.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExecutionTileScalarLoweringV1 {
    schedule: ExecutionTileScheduleV1,
    input: ValueId,
    base: ValueId,
    first: ValueId,
    next: ValueId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionTileScalarLoweringErrorV1 {
    ValueRange,
    OperationCount,
    Operation { position: usize },
}

impl ExecutionTileScalarLoweringV1 {
    /// Rejects arithmetic overflow and overlap with the two external operands.
    /// Freshness against the containing graph remains an integration obligation.
    pub fn new(
        schedule: ExecutionTileScheduleV1,
        input: ValueId,
        base: ValueId,
        first: ValueId,
    ) -> Result<Self, ExecutionTileScalarLoweringErrorV1> {
        let next = first
            .0
            .checked_add(5 + 12 * u32::from(schedule.elements()))
            .ok_or(ExecutionTileScalarLoweringErrorV1::ValueRange)?;
        if input == base
            || [input, base]
                .iter()
                .any(|id| (first.0..next).contains(&id.0))
        {
            return Err(ExecutionTileScalarLoweringErrorV1::ValueRange);
        }
        Ok(Self {
            schedule,
            input,
            base,
            first,
            next: ValueId(next),
        })
    }

    pub fn operation_count(self) -> usize {
        5 + 11 * usize::from(self.schedule.elements())
    }

    pub fn next_value(self) -> ValueId {
        self.next
    }

    pub fn component(self, element: u16) -> Option<(ValueId, ValueId)> {
        (element < self.schedule.elements()).then(|| {
            let start = 5 + 12 * u32::from(element);
            (self.value(start + 11), self.value(start + 8))
        })
    }

    fn value(self, offset: u32) -> ValueId {
        ValueId(self.first.0 + offset)
    }

    /// Emits one ordinary KIR operation with at most two result definitions.
    /// The caller owns the returned allocation and any enclosing output vector.
    /// This permits metered streaming into an already-reserved candidate graph.
    pub fn emit_operation(self, position: usize) -> Option<Operation> {
        if position >= self.operation_count() {
            return None;
        }
        let v = |offset| self.value(offset);
        let (result, ty, kind) = match position {
            0 => (
                v(0),
                Type::INDEX,
                OperationKind::Intrinsic(IntrinsicOperation::new(
                    IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Local,
                        axis: Axis::X,
                    },
                    Type::INDEX,
                )),
            ),
            1 => (
                v(1),
                Type::INDEX,
                OperationKind::SliceLength { slice: self.input },
            ),
            2 => (
                v(2),
                pointer_type(),
                OperationKind::SliceData { slice: self.input },
            ),
            3 => (
                v(3),
                Type::Scalar(ScalarType::U32),
                OperationKind::Constant(Constant::U32(0)),
            ),
            4 => (
                v(4),
                Type::INDEX,
                OperationKind::Constant(Constant::Index(0)),
            ),
            _ => {
                let element = (position - 5) / 11;
                let step = (position - 5) % 11;
                let start = 5 + 12 * element as u32;
                let c = |offset| v(start + offset);
                let (scale, mul_lhs, add_rhs) = match self.schedule.layout() {
                    ExecutionTileLayoutV1::Blocked => (self.schedule.elements(), v(0), c(1)),
                    ExecutionTileLayoutV1::Striped => (self.schedule.lanes(), c(1), v(0)),
                };
                match step {
                    0 => (
                        c(0),
                        Type::INDEX,
                        OperationKind::Constant(Constant::Index(u64::from(scale))),
                    ),
                    1 => (
                        c(1),
                        Type::INDEX,
                        OperationKind::Constant(Constant::Index(element as u64)),
                    ),
                    2 => (
                        c(2),
                        Type::INDEX,
                        OperationKind::Binary {
                            op: BinaryOp::Multiply,
                            lhs: mul_lhs,
                            rhs: c(0),
                        },
                    ),
                    3 => (
                        c(3),
                        Type::INDEX,
                        OperationKind::Binary {
                            op: BinaryOp::Add,
                            lhs: c(2),
                            rhs: add_rhs,
                        },
                    ),
                    4 => {
                        return Some(Operation::checked_binary(
                            ValueDef::new(c(4), Type::INDEX),
                            ValueDef::new(c(5), Type::BOOL),
                            CheckedBinaryOperator::Add,
                            self.base,
                            c(3),
                        ));
                    }
                    5 => (
                        c(6),
                        Type::BOOL,
                        OperationKind::Unary {
                            op: UnaryOp::Not,
                            operand: c(5),
                        },
                    ),
                    6 => (
                        c(7),
                        Type::BOOL,
                        OperationKind::Compare {
                            predicate: ComparePredicate::LessThan,
                            lhs: c(4),
                            rhs: v(1),
                        },
                    ),
                    7 => (
                        c(8),
                        Type::BOOL,
                        OperationKind::Binary {
                            op: BinaryOp::BitAnd,
                            lhs: c(6),
                            rhs: c(7),
                        },
                    ),
                    8 => (
                        c(9),
                        Type::INDEX,
                        OperationKind::Select {
                            condition: c(8),
                            true_value: c(4),
                            false_value: v(4),
                        },
                    ),
                    9 => (
                        c(10),
                        pointer_type(),
                        OperationKind::GetElementPointer {
                            base: v(2),
                            offset: c(9),
                        },
                    ),
                    10 => (
                        c(11),
                        Type::Scalar(ScalarType::U32),
                        OperationKind::GuardedLoad {
                            pointer: c(10),
                            predicate: c(8),
                            fallback: v(3),
                            access: MemoryAccess::new(AddressSpace::Global, 4),
                        },
                    ),
                    _ => unreachable!(),
                }
            }
        };
        Some(Operation::effect_free(ValueDef::new(result, ty), kind))
    }

    /// Independently checks a complete replacement without calling the emitter
    /// or constructing an expected replacement graph. This structural refinement
    /// check grants no source, memory-safety, target or formal-proof authority.
    pub fn check_replacement(
        self,
        operations: &[Operation],
    ) -> Result<(), ExecutionTileScalarLoweringErrorV1> {
        if operations.len() != self.operation_count() {
            return Err(ExecutionTileScalarLoweringErrorV1::OperationCount);
        }
        for (position, operation) in operations.iter().enumerate() {
            self.check_operation(position, operation)?;
        }
        Ok(())
    }

    pub fn check_operation(
        self,
        position: usize,
        operation: &Operation,
    ) -> Result<(), ExecutionTileScalarLoweringErrorV1> {
        let fail = || ExecutionTileScalarLoweringErrorV1::Operation { position };
        if position >= self.operation_count() {
            return Err(fail());
        }
        let id = |offset| ValueId(self.first.0 + offset);
        let result = |offset, ty: &Type| {
            operation.results.len() == 1
                && operation.results[0].id == id(offset)
                && &operation.results[0].ty == ty
        };
        let pointer = |offset| {
            operation.results.len() == 1
                && operation.results[0].id == id(offset)
                && is_pointer(&operation.results[0].ty)
        };
        let valid = match position {
            0 => {
                result(0, &Type::INDEX)
                    && matches!(
                        &operation.kind,
                        OperationKind::Intrinsic(IntrinsicOperation {
                            kind: IntrinsicKind::InvocationIndex {
                                kind: IndexKind::Local,
                                axis: Axis::X
                            },
                            result_type: Type::Scalar(ScalarType::Index)
                        })
                    )
            }
            1 => {
                result(1, &Type::INDEX)
                    && matches!(operation.kind, OperationKind::SliceLength { slice } if slice == self.input)
            }
            2 => {
                pointer(2)
                    && matches!(operation.kind, OperationKind::SliceData { slice } if slice == self.input)
            }
            3 => {
                result(3, &Type::Scalar(ScalarType::U32))
                    && matches!(operation.kind, OperationKind::Constant(Constant::U32(0)))
            }
            4 => {
                result(4, &Type::INDEX)
                    && matches!(operation.kind, OperationKind::Constant(Constant::Index(0)))
            }
            _ => {
                let element = (position - 5) / 11;
                let start = 5 + 12 * element as u32;
                let c = |offset| id(start + offset);
                let typed = |offset, ty: &Type| result(start + offset, ty);
                match (position - 5) % 11 {
                    0 => {
                        typed(0, &Type::INDEX)
                            && matches!(operation.kind,
                            OperationKind::Constant(Constant::Index(scale)) if scale == match self.schedule.layout() {
                                ExecutionTileLayoutV1::Blocked => u64::from(self.schedule.elements()),
                                ExecutionTileLayoutV1::Striped => u64::from(self.schedule.lanes()),
                            })
                    }
                    1 => {
                        typed(1, &Type::INDEX)
                            && matches!(operation.kind, OperationKind::Constant(Constant::Index(value)) if value == element as u64)
                    }
                    2 => {
                        typed(2, &Type::INDEX)
                            && matches!(operation.kind, OperationKind::Binary { op: BinaryOp::Multiply, lhs, rhs }
                        if rhs == c(0) && lhs == match self.schedule.layout() { ExecutionTileLayoutV1::Blocked => id(0), ExecutionTileLayoutV1::Striped => c(1) })
                    }
                    3 => {
                        typed(3, &Type::INDEX)
                            && matches!(operation.kind, OperationKind::Binary { op: BinaryOp::Add, lhs, rhs }
                        if lhs == c(2) && rhs == match self.schedule.layout() { ExecutionTileLayoutV1::Blocked => c(1), ExecutionTileLayoutV1::Striped => id(0) })
                    }
                    4 => {
                        operation.results.len() == 2
                            && operation.results[0].id == c(4)
                            && operation.results[0].ty == Type::INDEX
                            && operation.results[1].id == c(5)
                            && operation.results[1].ty == Type::BOOL
                            && matches!(operation.kind, OperationKind::Binary { op: BinaryOp::Checked(CheckedBinaryOperator::Add), lhs, rhs } if lhs == self.base && rhs == c(3))
                    }
                    5 => {
                        typed(6, &Type::BOOL)
                            && matches!(operation.kind, OperationKind::Unary { op: UnaryOp::Not, operand } if operand == c(5))
                    }
                    6 => {
                        typed(7, &Type::BOOL)
                            && matches!(operation.kind, OperationKind::Compare { predicate: ComparePredicate::LessThan, lhs, rhs } if lhs == c(4) && rhs == id(1))
                    }
                    7 => {
                        typed(8, &Type::BOOL)
                            && matches!(operation.kind, OperationKind::Binary { op: BinaryOp::BitAnd, lhs, rhs } if lhs == c(6) && rhs == c(7))
                    }
                    8 => {
                        typed(9, &Type::INDEX)
                            && matches!(operation.kind, OperationKind::Select { condition, true_value, false_value } if condition == c(8) && true_value == c(4) && false_value == id(4))
                    }
                    9 => {
                        pointer(start + 10)
                            && matches!(operation.kind, OperationKind::GetElementPointer { base, offset } if base == id(2) && offset == c(9))
                    }
                    10 => {
                        typed(11, &Type::Scalar(ScalarType::U32))
                            && matches!(operation.kind, OperationKind::GuardedLoad { pointer, predicate, fallback, access } if pointer == c(10) && predicate == c(8) && fallback == id(3) && access == MemoryAccess::new(AddressSpace::Global, 4))
                    }
                    _ => false,
                }
            }
        };
        if valid { Ok(()) } else { Err(fail()) }
    }
}

fn pointer_type() -> Type {
    Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadOnly,
    )
}

fn is_pointer(ty: &Type) -> bool {
    matches!(ty, Type::Pointer(pointer) if pointer.pointee.as_ref() == &Type::Scalar(ScalarType::U32)
        && pointer.address_space == AddressSpace::Global && pointer.access == AccessMode::ReadOnly)
}

#[cfg(test)]
#[path = "execution_tile_scalar_lowering_v1_tests.rs"]
mod tests;
