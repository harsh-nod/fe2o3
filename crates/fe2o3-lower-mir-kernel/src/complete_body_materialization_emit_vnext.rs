//! Emits actual canonical blocks and existing memory/control operations.
use super::*;
use fe2o3_kernel_ir::{
    AccessMode, AddressSpace, ComparePredicate, Constant, Gfx942CompleteBodyDeclarationVNext,
    Gfx942CompleteBodyStepVNext, Gfx942ProgramInstructionV1 as Instruction, IntrinsicOperation,
    MemoryAccess, Operation, OperationKind, Terminator,
};

#[allow(
    clippy::too_many_arguments,
    reason = "One bounded canonical block emission context"
)]
pub(super) fn fill_block(
    input: &CompleteBodyCanonicalInputVNext<'_>,
    sources: &[SourceBlock<'_>],
    ordinal: usize,
    incoming: [[Option<ValueId>; 5]; 8],
    block: &mut BasicBlock,
    next: &mut u32,
    authored: &mut u8,
    scope: &mut Scope<'_, '_>,
) -> Result<(), Error> {
    let source = sources.get(ordinal).ok_or(Error::InternalAccounting)?;
    let extra = usize::from(ordinal == 0)
        + match source.terminator {
            End::Jump(_) => 0,
            End::BranchSelectorZero { .. } => 2,
            End::GuardedStoreOutputAndEnd => 6,
        };
    block.operations = scope.vec(source.instructions.len() + extra)?;
    if ordinal == 0 {
        let mut labels = [0_u8; 8];
        for (place, source) in labels.iter_mut().zip(sources) {
            *place = source.label.0;
        }
        block.operations.push(Operation::new(
            Vec::new(),
            OperationKind::Gfx942CompleteBodyDeclaration(Gfx942CompleteBodyDeclarationVNext {
                origin: input.origin,
                registers: input.registers,
                parameters: PARAMETERS,
                labels,
                block_count: sources.len() as u8,
                instruction_count: sources
                    .iter()
                    .map(|source| source.instructions.len())
                    .sum::<usize>() as u8,
            }),
        ));
    }
    let mut values = incoming[ordinal];
    for instruction in source.instructions.iter().copied() {
        let operand = |role: Role| values[role as usize].ok_or(Error::UndefinedRole);
        let operands = match instruction {
            Instruction::Move { source, .. } => [Some(operand(source)?), None],
            Instruction::Binary { left, right, .. } => {
                [Some(operand(left)?), Some(operand(right)?)]
            }
        };
        let result = scalar(
            block,
            next,
            Type::Scalar(ScalarType::U32),
            OperationKind::Gfx942CompleteBodyStep(Gfx942CompleteBodyStepVNext {
                authored_block: ordinal as u8,
                authored_instruction: *authored,
                instruction,
                operands,
            }),
            scope,
        )?;
        values[instruction.destination().role() as usize] = Some(result);
        *authored = authored.checked_add(1).ok_or(Error::InternalAccounting)?;
    }
    block.terminator = Some(match source.terminator {
        End::Jump(label) => {
            let target = target(sources, label)?;
            Terminator::Branch {
                target: BlockId(target as u32),
                arguments: edge_arguments(values, incoming[target], scope)?,
            }
        }
        End::BranchSelectorZero { zero, nonzero } => {
            let zero_value = scalar(
                block,
                next,
                Type::Scalar(ScalarType::U32),
                OperationKind::Constant(Constant::U32(0)),
                scope,
            )?;
            let condition = scalar(
                block,
                next,
                Type::BOOL,
                OperationKind::Compare {
                    predicate: ComparePredicate::Equal,
                    lhs: PARAMETERS[4],
                    rhs: zero_value,
                },
                scope,
            )?;
            let zero = target(sources, zero)?;
            let nonzero = target(sources, nonzero)?;
            Terminator::ConditionalBranch {
                condition,
                then_target: BlockId(zero as u32),
                then_arguments: edge_arguments(values, incoming[zero], scope)?,
                else_target: BlockId(nonzero as u32),
                else_arguments: edge_arguments(values, incoming[nonzero], scope)?,
            }
        }
        End::GuardedStoreOutputAndEnd => {
            let output = values[Role::Output as usize].ok_or(Error::UndefinedRole)?;
            tail(block, next, output, scope)?;
            Terminator::Return { values: Vec::new() }
        }
    });
    Ok(())
}

fn target(
    sources: &[SourceBlock<'_>],
    label: fe2o3_kernel_ir::Gfx942CompleteBodyLabelV1,
) -> Result<usize, Error> {
    sources
        .iter()
        .position(|source| source.label == label)
        .ok_or(Error::UnsupportedShape)
}
fn edge_arguments(
    values: [Option<ValueId>; 5],
    target: [Option<ValueId>; 5],
    scope: &mut Scope<'_, '_>,
) -> Result<Vec<ValueId>, Error> {
    let mut arguments = scope.vec(target[3..].iter().filter(|value| value.is_some()).count())?;
    for role in 3..5 {
        if target[role].is_some() {
            arguments.push(values[role].ok_or(Error::UndefinedRole)?);
        }
    }
    Ok(arguments)
}
fn scalar(
    block: &mut BasicBlock,
    next: &mut u32,
    ty: Type,
    kind: OperationKind,
    scope: &mut Scope<'_, '_>,
) -> Result<ValueId, Error> {
    let id = fresh(next)?;
    let mut results = scope.vec(1)?;
    results.push(ValueDef::new(id, ty));
    block.operations.push(Operation::new(results, kind));
    Ok(id)
}
fn pointer_type(scope: &mut Scope<'_, '_>) -> Result<Type, Error> {
    scope.reserve(std::mem::size_of::<Type>())?;
    Ok(Type::pointer(
        Type::Scalar(ScalarType::U32),
        AddressSpace::Global,
        AccessMode::ReadWrite,
    ))
}
fn tail(
    block: &mut BasicBlock,
    next: &mut u32,
    output: ValueId,
    scope: &mut Scope<'_, '_>,
) -> Result<(), Error> {
    // Ordinary existing KIR operations. Their shared analyses must independently
    // establish bounds, disjoint writes, pointer/access and target obligations.
    // No special declaration bit asserts that those proofs have succeeded.
    let index = scalar(
        block,
        next,
        Type::INDEX,
        OperationKind::Intrinsic(IntrinsicOperation::global_id_1d()),
        scope,
    )?;
    let length = scalar(
        block,
        next,
        Type::INDEX,
        OperationKind::SliceLength {
            slice: PARAMETERS[0],
        },
        scope,
    )?;
    let predicate = scalar(
        block,
        next,
        Type::BOOL,
        OperationKind::Compare {
            predicate: ComparePredicate::LessThan,
            lhs: index,
            rhs: length,
        },
        scope,
    )?;
    let pointer_ty = pointer_type(scope)?;
    let base = scalar(
        block,
        next,
        pointer_ty,
        OperationKind::SliceData {
            slice: PARAMETERS[0],
        },
        scope,
    )?;
    let pointer_ty = pointer_type(scope)?;
    let pointer = scalar(
        block,
        next,
        pointer_ty,
        OperationKind::GetElementPointer {
            base,
            offset: index,
        },
        scope,
    )?;
    block.operations.push(Operation::new(
        Vec::new(),
        OperationKind::GuardedStore {
            pointer,
            predicate,
            value: output,
            access: MemoryAccess::new(AddressSpace::Global, 4),
        },
    ));
    Ok(())
}
