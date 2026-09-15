//! Per-source-call nonobservability for one closed unsigned quotient body.
//! This proves failure impossible before expansion; it does not remove asserts.

use super::*;

pub(super) struct Certificate<'a> {
    source: &'a AdmittedInertSemanticMirV1,
    function: &'a SemanticFunctionDeclV1,
    call: &'a SemanticDirectCallV1,
    site: (SemanticFunctionIdV1, SemanticBlockIdV1),
    assertion_blocks: [usize; 3],
}

impl Certificate<'_> {
    pub(super) fn covers(
        &self,
        source: &AdmittedInertSemanticMirV1,
        function: &SemanticFunctionDeclV1,
        call: &SemanticDirectCallV1,
        site: (SemanticFunctionIdV1, SemanticBlockIdV1),
        block: usize,
    ) -> bool {
        std::ptr::eq(self.source, source)
            && std::ptr::eq(self.function, function)
            && std::ptr::eq(self.call, call)
            && self.site == site
            && self.assertion_blocks.contains(&block)
    }
}

pub(super) fn certify<'a>(
    source: &'a AdmittedInertSemanticMirV1,
    function_id: SemanticFunctionIdV1,
    function: &'a SemanticFunctionDeclV1,
    call: &'a SemanticDirectCallV1,
    site: (SemanticFunctionIdV1, SemanticBlockIdV1),
    budget: &mut Budget,
) -> Result<Option<Certificate<'a>>> {
    budget.work(1)?;
    let Some(caller) = source.functions().get(site.0.index() as usize) else {
        return Ok(None);
    };
    let Some(call_block) = caller.blocks().get(site.1.index() as usize) else {
        return Ok(None);
    };
    if !source
        .functions()
        .get(function_id.index() as usize)
        .is_some_and(|original| std::ptr::eq(original, function))
        || !matches!(call_block.terminator().kind(), SemanticTerminatorKindV1::Call(original) if std::ptr::eq(original, call))
        || !matches!(source.callables().get(call.callee().index() as usize), Some(SemanticCallableDeclV1::Defined { function: callee }) if *callee == function_id)
        || call.arguments().len() != 2
        || !call.variadic_argument_abis().is_empty()
        || call.unwind() != SemanticUnwindActionV1::Unreachable
        || function.role() != SemanticFunctionRoleV1::InternalHelper
        || function.abi().extern_abi() != SemanticExternAbiV1::Rust
        || function.abi().can_unwind()
        || function.abi().c_variadic()
        || function.locals().len() != 9
        || function.blocks().len() != 7
    {
        return Ok(None);
    }

    // Fixed-size profile: three type records, nine locals, seven terminators,
    // eight statements. Charge before inspecting them; no graph/cache allocation.
    budget.work(3 + 9 + 7 + 8)?;
    // Discover the closed graph and local roles, then verify every expression.
    // Fixed charges cover edge traversal, local-role scan, six destinations,
    // and the two complete bijections. No graph search or success cache.
    budget.work(7 + 9 + 6 + 7 + 9)?;
    let Some(roles) = Roles::derive(function) else {
        return Ok(None);
    };
    let locals = function.locals();
    let word = locals[roles.locals[1]].ty();
    let boolean = locals[roles.locals[4]].ty();
    let pair = locals[roles.locals[8]].ty();
    let Some(word_decl) = source.types().get(word.index() as usize) else {
        return Ok(None);
    };
    let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
        signed: false,
        bits,
    }) = word_decl.shape()
    else {
        return Ok(None);
    };
    if !matches!(bits, 8 | 16 | 32 | 64 | 128)
        || word_decl.layout().size_bytes() != Some(u64::from(*bits / 8))
        || !source
            .types()
            .get(boolean.index() as usize)
            .is_some_and(|ty| {
                ty.shape() == &SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool)
                    && ty.layout().size_bytes() == Some(1)
            })
        || !matches!(source.types().get(pair.index() as usize).map(|ty| ty.shape()),
            Some(SemanticTypeShapeV1::Tuple(tuple)) if tuple.fields() == [word, boolean])
        || function.abi().source_input_types() != [word, word]
        || function.abi().source_output_type() != word
        || function.abi().source_argument_ownership()
            != [SemanticSourceArgumentOwnershipV1::ByValue; 2]
        || call.arguments()[0].ty() != word
    {
        return Ok(None);
    }
    let maximum = if *bits == 128 {
        u128::MAX
    } else {
        (1u128 << bits) - 1
    };
    let Some(divisor) = scalar(&call.arguments()[1], word, (*bits / 8) as u8) else {
        return Ok(None);
    };
    if divisor < 2 || divisor > maximum
        // q <= MAX / D, so the original checked q + 1 cannot overflow.
        || maximum.checked_div(divisor).and_then(|q| q.checked_add(1)).is_none_or(|sum| sum > maximum)
    {
        return Ok(None);
    }
    let profile = Profile {
        locals: roles.locals,
        word,
        boolean,
        pair,
        bytes: (*bits / 8) as u8,
    };
    for (index, expected) in [
        word, word, word, word, boolean, word, boolean, boolean, pair,
    ]
    .into_iter()
    .enumerate()
    {
        let role = match index {
            0 => SemanticLocalRoleV1::Return,
            1 | 2 => SemanticLocalRoleV1::Argument((index - 1) as u32),
            _ => SemanticLocalRoleV1::Temporary,
        };
        if locals[roles.locals[index]].ty() != expected
            || locals[roles.locals[index]].role() != role
        {
            return Ok(None);
        }
    }
    let blocks = roles.blocks.map(|index| &function.blocks()[index]);
    if blocks
        .iter()
        .zip([1, 2, 2, 1, 1, 1, 0])
        .any(|(block, count)| block.statements().len() != count)
    {
        return Ok(None);
    }
    use ExpectedOperand::{Constant, Copy, Field, Move};
    use Expression::{Binary, CheckedAdd, Use};
    for (block, statement, destination, expected) in [
        (
            0,
            0,
            4,
            Binary(SemanticBinaryOpV1::Equal, Copy(2), Constant(0)),
        ),
        (
            1,
            0,
            3,
            Binary(SemanticBinaryOpV1::Divide, Copy(1), Copy(2)),
        ),
        (
            1,
            1,
            6,
            Binary(SemanticBinaryOpV1::Equal, Copy(2), Constant(0)),
        ),
        (
            2,
            0,
            5,
            Binary(SemanticBinaryOpV1::Remainder, Copy(1), Copy(2)),
        ),
        (
            2,
            1,
            7,
            Binary(SemanticBinaryOpV1::GreaterThan, Copy(5), Constant(0)),
        ),
        (3, 0, 8, CheckedAdd),
        (4, 0, 0, Use(Field(0))),
        (5, 0, 0, Use(Copy(3))),
    ] {
        if !profile.assignment(
            &blocks[block].statements()[statement],
            destination,
            expected,
        ) {
            return Ok(None);
        }
    }
    if !profile.assertion(
        blocks[0].terminator().kind(),
        Move(4),
        roles.blocks[1] as u32,
        0,
    ) || !profile.assertion(
        blocks[1].terminator().kind(),
        Move(6),
        roles.blocks[2] as u32,
        1,
    ) || !profile.assertion(
        blocks[3].terminator().kind(),
        Field(1),
        roles.blocks[4] as u32,
        3,
    ) || !matches!(blocks[2].terminator().kind(), SemanticTerminatorKindV1::SwitchInt { discriminant, targets }
            if profile.operand(discriminant, Move(7)) && targets.values().len() == 1
                && targets.values()[0].value() == 0
                && edge(targets.values()[0].edge(), SemanticEdgeRoleV1::SwitchValue, roles.blocks[5] as u32)
                && edge(targets.otherwise(), SemanticEdgeRoleV1::SwitchOtherwise, roles.blocks[3] as u32))
        || !matches!(blocks[4].terminator().kind(), SemanticTerminatorKindV1::Goto(target) if edge(*target, SemanticEdgeRoleV1::Goto, roles.blocks[6] as u32))
        || !matches!(blocks[5].terminator().kind(), SemanticTerminatorKindV1::Goto(target) if edge(*target, SemanticEdgeRoleV1::Goto, roles.blocks[6] as u32))
        || !matches!(
            blocks[6].terminator().kind(),
            SemanticTerminatorKindV1::Return
        )
    {
        return Ok(None);
    }
    Ok(Some(Certificate {
        source,
        function,
        call,
        site,
        assertion_blocks: [roles.blocks[0], roles.blocks[1], roles.blocks[3]],
    }))
}

// Array positions name semantic roles, never source-table ordinals.
struct Roles {
    blocks: [usize; 7],
    locals: [usize; 9],
}

impl Roles {
    fn derive(function: &SemanticFunctionDeclV1) -> Option<Self> {
        let assertion_target = |block: usize| {
            let SemanticTerminatorKindV1::Assert { target, .. } =
                function.blocks().get(block)?.terminator().kind()
            else {
                return None;
            };
            (target.role() == SemanticEdgeRoleV1::AssertSuccess)
                .then_some(target.target().index() as usize)
        };
        let guard = function.entry().index() as usize;
        let quotient = assertion_target(guard)?;
        let remainder = assertion_target(quotient)?;
        let SemanticTerminatorKindV1::SwitchInt { targets, .. } =
            function.blocks().get(remainder)?.terminator().kind()
        else {
            return None;
        };
        let [zero] = targets.values() else {
            return None;
        };
        if zero.value() != 0
            || zero.edge().role() != SemanticEdgeRoleV1::SwitchValue
            || targets.otherwise().role() != SemanticEdgeRoleV1::SwitchOtherwise
        {
            return None;
        }
        let exact = zero.edge().target().index() as usize;
        let increment = targets.otherwise().target().index() as usize;
        let rounded = assertion_target(increment)?;
        let SemanticTerminatorKindV1::Goto(target) =
            function.blocks().get(rounded)?.terminator().kind()
        else {
            return None;
        };
        if target.role() != SemanticEdgeRoleV1::Goto {
            return None;
        }
        let exit = target.target().index() as usize;
        let blocks = [guard, quotient, remainder, increment, rounded, exact, exit];
        if !bijection(&blocks) {
            return None;
        }

        let mut locals = [usize::MAX; 9];
        for (index, declaration) in function.locals().iter().enumerate() {
            let role = match declaration.role() {
                SemanticLocalRoleV1::Return => Some(0),
                SemanticLocalRoleV1::Argument(0) => Some(1),
                SemanticLocalRoleV1::Argument(1) => Some(2),
                SemanticLocalRoleV1::Temporary => None,
                _ => return None,
            };
            if let Some(role) = role {
                if locals[role] != usize::MAX {
                    return None;
                }
                locals[role] = index;
            }
        }
        for (role, block, statement) in [
            (4, guard, 0),
            (3, quotient, 0),
            (6, quotient, 1),
            (5, remainder, 0),
            (7, remainder, 1),
            (8, increment, 0),
        ] {
            let SemanticStatementKindV1::Assign(assignment) = function
                .blocks()
                .get(block)?
                .statements()
                .get(statement)?
                .kind()
            else {
                return None;
            };
            if !assignment.destination().projections().is_empty() {
                return None;
            }
            locals[role] = assignment.destination().local().index() as usize;
        }
        bijection(&locals).then_some(Self { blocks, locals })
    }
}

fn bijection<const N: usize>(indices: &[usize; N]) -> bool {
    let mut seen = [false; N];
    for &index in indices {
        let Some(slot) = seen.get_mut(index) else {
            return false;
        };
        if *slot {
            return false;
        }
        *slot = true;
    }
    true
}

fn scalar(operand: &SemanticOperandV1, ty: SemanticTypeIdV1, bytes: u8) -> Option<u128> {
    let SemanticOperandV1::Constant(value) = operand else {
        return None;
    };
    let SemanticConstantValueV1::Scalar(scalar) = value.value() else {
        return None;
    };
    (value.ty() == ty && scalar.size_bytes() == bytes).then_some(scalar.bits())
}

fn edge(edge: SemanticControlFlowEdgeV1, role: SemanticEdgeRoleV1, target: u32) -> bool {
    edge.role() == role && edge.target().index() == target
}

#[derive(Clone, Copy)]
enum ExpectedOperand {
    Copy(u32),
    Move(u32),
    Field(u32),
    Constant(u128),
}
enum Expression {
    Binary(SemanticBinaryOpV1, ExpectedOperand, ExpectedOperand),
    CheckedAdd,
    Use(ExpectedOperand),
}
struct Profile {
    locals: [usize; 9],
    word: SemanticTypeIdV1,
    boolean: SemanticTypeIdV1,
    pair: SemanticTypeIdV1,
    bytes: u8,
}

impl Profile {
    fn ty(&self, local: u32) -> SemanticTypeIdV1 {
        match local {
            4 | 6 | 7 => self.boolean,
            8 => self.pair,
            _ => self.word,
        }
    }
    fn place(&self, place: &SemanticPlaceV1, local: u32) -> bool {
        place.local().index() as usize == self.locals[local as usize]
            && place.ty() == self.ty(local)
            && place.projections().is_empty()
    }
    fn operand(&self, operand: &SemanticOperandV1, expected: ExpectedOperand) -> bool {
        match (operand, expected) {
            (SemanticOperandV1::Copy(place), ExpectedOperand::Copy(local))
            | (SemanticOperandV1::Move(place), ExpectedOperand::Move(local)) => {
                self.place(place, local)
            }
            (SemanticOperandV1::Move(place), ExpectedOperand::Field(index)) => {
                let ty = if index == 0 { self.word } else { self.boolean };
                place.local().index() as usize == self.locals[8]
                    && place.ty() == ty
                    && matches!(place.projections(), [projection] if projection.kind() == SemanticProjectionKindV1::Field(index) && projection.result_type() == ty)
            }
            (_, ExpectedOperand::Constant(bits)) => {
                scalar(operand, self.word, self.bytes) == Some(bits)
            }
            _ => false,
        }
    }
    fn assignment(
        &self,
        statement: &SemanticStatementV1,
        destination: u32,
        expected: Expression,
    ) -> bool {
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return false;
        };
        if !self.place(assignment.destination(), destination)
            || assignment.value().result_type() != self.ty(destination)
        {
            return false;
        }
        use ExpectedOperand::{Constant, Copy};
        match (assignment.value().kind(), expected) {
            (
                SemanticRvalueKindV1::Binary {
                    operation,
                    left,
                    right,
                },
                Expression::Binary(op, a, b),
            ) => *operation == op && self.operand(left, a) && self.operand(right, b),
            (SemanticRvalueKindV1::CheckedBinary(checked), Expression::CheckedAdd) => {
                checked.operation() == SemanticCheckedBinaryOpV1::Add
                    && self.operand(checked.left(), Copy(3))
                    && self.operand(checked.right(), Constant(1))
            }
            (SemanticRvalueKindV1::Use(operand), Expression::Use(expected)) => {
                self.operand(operand, expected)
            }
            _ => false,
        }
    }
    fn assertion(
        &self,
        terminator: &SemanticTerminatorKindV1,
        expected_condition: ExpectedOperand,
        successor: u32,
        block: u32,
    ) -> bool {
        let SemanticTerminatorKindV1::Assert {
            condition,
            expected: false,
            message,
            target,
            unwind: SemanticUnwindActionV1::Unreachable,
        } = terminator
        else {
            return false;
        };
        if !self.operand(condition, expected_condition)
            || !edge(*target, SemanticEdgeRoleV1::AssertSuccess, successor)
        {
            return false;
        }
        use ExpectedOperand::{Constant, Copy};
        match (block, message) {
            (0, SemanticAssertMessageV1::DivisionByZero(lhs))
            | (1, SemanticAssertMessageV1::RemainderByZero(lhs)) => self.operand(lhs, Copy(1)),
            (
                3,
                SemanticAssertMessageV1::Overflow {
                    operation: SemanticBinaryOpV1::Add,
                    left,
                    right,
                },
            ) => self.operand(left, Copy(3)) && self.operand(right, Constant(1)),
            _ => false,
        }
    }
}
