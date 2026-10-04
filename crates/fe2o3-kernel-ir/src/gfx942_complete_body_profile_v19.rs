//! Closed structural profile for the allocated KIR V19 complete-body graph.
//!
//! This checks the single real SSA/CFG, not a packed program or a second
//! interpreter. Origin digests are completeness/refusal predicates only;
//! authentic current-source custody belongs to the source importer.
use crate::{
    AccessMode, AddressSpace, BasicBlock, BlockId,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, ComparePredicate, Constant, Function,
    FunctionRole, Gfx942CompleteBodyDeclarationVNext, Gfx942ProgramInstructionV1 as Instruction,
    Gfx942ProgramRoleV1 as Role, IntrinsicOperation, Module, Operation, OperationKind, ScalarType,
    Terminator, Type, ValueId, WorkgroupSize,
};

/// Prepaid logical work: at most 25 operations, 48 definition slots and four
/// edges; bounded strings, fixed metadata and <= 48*48 duplicate comparisons.
/// No allocation or storage release occurs; the caller's ledger floor remains.
pub(crate) const COMPLETE_BODY_PROFILE_WORK_V19: usize = 16_384;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CompleteBodyProfileErrorV19 {
    Resource(Resource),
    Invalid(&'static str),
}
impl From<Resource> for CompleteBodyProfileErrorV19 {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
type Error = CompleteBodyProfileErrorV19;
type Result<T> = std::result::Result<T, Error>;
type Values = [Option<ValueId>; 5];

fn require(condition: bool, message: &'static str) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(Error::Invalid(message))
    }
}
fn u32_type(ty: &Type) -> bool {
    matches!(ty, Type::Scalar(ScalarType::U32))
}
fn output_slice(ty: &Type) -> bool {
    matches!(ty, Type::Slice(slice) if slice.address_space == AddressSpace::Global
        && slice.access == AccessMode::ReadWrite && u32_type(&slice.element))
}
fn output_pointer(ty: &Type) -> bool {
    matches!(ty, Type::Pointer(pointer) if pointer.address_space == AddressSpace::Global
        && pointer.access == AccessMode::ReadWrite && u32_type(&pointer.pointee))
}
fn one_result(operation: &Operation, predicate: fn(&Type) -> bool) -> Result<ValueId> {
    let [result] = operation.results.as_slice() else {
        return Err(Error::Invalid("complete-body operation result arity"));
    };
    require(predicate(&result.ty), "complete-body operation result type")?;
    Ok(result.id)
}
fn scalar_result(operation: &Operation) -> Result<ValueId> {
    one_result(operation, u32_type)
}
fn bool_type(ty: &Type) -> bool {
    matches!(ty, Type::Scalar(ScalarType::Bool))
}
fn index_type(ty: &Type) -> bool {
    matches!(ty, Type::Scalar(ScalarType::Index))
}

struct Definitions {
    ids: [ValueId; 48],
    count: usize,
}
impl Definitions {
    fn insert(&mut self, id: ValueId) -> Result<()> {
        require(
            self.count < self.ids.len(),
            "complete-body definition count",
        )?;
        require(
            !self.ids[..self.count].contains(&id),
            "complete-body duplicate SSA definition",
        )?;
        self.ids[self.count] = id;
        self.count += 1;
        Ok(())
    }
}

/// Additional whole-function profile check. Ordinary general verification
/// still checks SSA dominance, uses, types, capabilities and memory contracts.
pub(crate) fn check_gfx942_complete_body_function_v19(
    module: &Module,
    function: &Function,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(COMPLETE_BODY_PROFILE_WORK_V19)?;
    let [actual_function] = module.functions.as_slice() else {
        return Err(Error::Invalid("complete-body single function"));
    };
    require(
        std::ptr::eq(actual_function, function),
        "complete-body actual function membership",
    )?;
    let [kernel] = module.kernels.as_slice() else {
        return Err(Error::Invalid("complete-body single kernel"));
    };
    require(
        function.id.as_str().len() <= 256 && kernel.entry.as_str().len() <= 256,
        "complete-body symbol bound",
    )?;
    require(
        !function.id.as_str().is_empty()
            && function
                .id
                .as_str()
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "complete-body symbol",
    )?;
    require(
        function.role == FunctionRole::KernelEntry && kernel.entry == function.id,
        "complete-body entry binding",
    )?;
    require(
        kernel.domain.rank() == 1 && kernel.workgroup_size == Some(WorkgroupSize::new(64, 1, 1)),
        "complete-body launch profile",
    )?;
    let [output, a, b, c, selector] = function.signature.parameters.as_slice() else {
        return Err(Error::Invalid("complete-body parameter arity"));
    };
    require(
        output_slice(output)
            && [a, b, c, selector].into_iter().all(u32_type)
            && function.signature.results.is_empty(),
        "complete-body signature",
    )?;
    let body = function
        .body
        .as_ref()
        .ok_or(Error::Invalid("complete-body missing body"))?;
    let parameters: [ValueId; 5] = body
        .parameters
        .as_slice()
        .try_into()
        .map_err(|_| Error::Invalid("complete-body parameter identities"))?;
    require(
        matches!(body.blocks.len(), 1 | 4),
        "complete-body one block or diamond",
    )?;
    let first = body.blocks[0]
        .operations
        .first()
        .ok_or(Error::Invalid("complete-body missing declaration"))?;
    let OperationKind::Gfx942CompleteBodyDeclaration(declaration) = &first.kind else {
        return Err(Error::Invalid("complete-body entry declaration"));
    };
    require(
        first.results.is_empty(),
        "complete-body declaration results",
    )?;
    check_declaration(declaration, parameters, body.blocks.len())?;

    let mut definitions = Definitions {
        ids: [ValueId(0); 48],
        count: 0,
    };
    for value in parameters {
        definitions.insert(value)?;
    }
    let mut operations = 0_usize;
    for (ordinal, block) in body.blocks.iter().enumerate() {
        require(
            block.id == BlockId(ordinal as u32),
            "complete-body ordinal block identity",
        )?;
        require(
            block.parameters.len() <= 2 && block.operations.len() <= 25,
            "complete-body block bounds",
        )?;
        operations += block.operations.len();
        require(operations <= 25, "complete-body operation count")?;
        for parameter in &block.parameters {
            require(
                u32_type(&parameter.ty),
                "complete-body block parameter type",
            )?;
            definitions.insert(parameter.id)?;
        }
        for operation in &block.operations {
            require(operation.results.len() <= 1, "complete-body result bound")?;
            for result in &operation.results {
                definitions.insert(result.id)?;
            }
        }
    }

    let mut incoming = [[None; 5]; 4];
    let mut outgoing = [[None; 5]; 4];
    let mut authored = 0_usize;
    for (ordinal, block) in body.blocks.iter().enumerate() {
        let live = match ordinal {
            0 => [false; 2],
            1 | 2 => [outgoing[0][3].is_some(), outgoing[0][4].is_some()],
            3 => [
                outgoing[1][3].is_some() && outgoing[2][3].is_some(),
                outgoing[1][4].is_some() && outgoing[2][4].is_some(),
            ],
            _ => return Err(Error::Invalid("complete-body block count")),
        };
        let mut values = [
            Some(parameters[1]),
            Some(parameters[2]),
            Some(parameters[3]),
            None,
            None,
        ];
        require(
            block.parameters.len() == live.into_iter().filter(|value| *value).count(),
            "complete-body exact merge parameter roster",
        )?;
        let mut parameter_index = 0;
        for (offset, defined) in live.into_iter().enumerate() {
            if defined {
                values[offset + 3] = Some(block.parameters[parameter_index].id);
                parameter_index += 1;
            }
        }
        incoming[ordinal] = values;
        let mut index = usize::from(ordinal == 0);
        while let Some(operation) = block.operations.get(index) {
            let OperationKind::Gfx942CompleteBodyStep(step) = &operation.kind else {
                break;
            };
            require(
                authored < 16
                    && usize::from(step.authored_block) == ordinal
                    && usize::from(step.authored_instruction) == authored,
                "complete-body authored instruction position",
            )?;
            let get = |role: Role| {
                values[role as usize].ok_or(Error::Invalid("complete-body undefined role"))
            };
            let operands = match step.instruction {
                Instruction::Move { source, .. } => [Some(get(source)?), None],
                Instruction::Binary { left, right, .. } => [Some(get(left)?), Some(get(right)?)],
            };
            require(
                step.operands == operands,
                "complete-body current SSA role operands",
            )?;
            let value = scalar_result(operation)?;
            values[step.instruction.destination().role() as usize] = Some(value);
            authored += 1;
            index += 1;
        }
        outgoing[ordinal] = values;
        let remainder = &block.operations[index..];
        if body.blocks.len() == 4 && ordinal == 0 {
            check_selector(block, remainder, parameters[4])?;
        } else if body.blocks.len() == 4 && ordinal != 3 {
            require(remainder.is_empty(), "complete-body arm effects")?;
            require(
                matches!(
                    &block.terminator,
                    Some(Terminator::Branch {
                        target: BlockId(3),
                        ..
                    })
                ),
                "complete-body arm successor",
            )?;
        } else {
            let value = values[Role::Output as usize]
                .ok_or(Error::Invalid("complete-body undefined terminal output"))?;
            check_tail(block, remainder, parameters[0], value)?;
        }
    }
    require(
        authored == usize::from(declaration.instruction_count),
        "complete-body instruction census",
    )?;
    if body.blocks.len() == 4 {
        let Some(Terminator::ConditionalBranch {
            then_arguments,
            else_arguments,
            ..
        }) = &body.blocks[0].terminator
        else {
            return Err(Error::Invalid("complete-body selector edge"));
        };
        check_edge(then_arguments, outgoing[0], incoming[1])?;
        check_edge(else_arguments, outgoing[0], incoming[2])?;
        for (block, values) in body.blocks[1..3].iter().zip(&outgoing[1..3]) {
            let Some(Terminator::Branch { arguments, .. }) = &block.terminator else {
                return Err(Error::Invalid("complete-body merge edge"));
            };
            check_edge(arguments, *values, incoming[3])?;
        }
    }
    Ok(())
}

fn check_declaration(
    declaration: &Gfx942CompleteBodyDeclarationVNext,
    parameters: [ValueId; 5],
    blocks: usize,
) -> Result<()> {
    require(
        declaration.origin.is_complete(),
        "complete-body incomplete origin",
    )?;
    require(
        declaration.parameters == parameters,
        "complete-body declaration parameter binding",
    )?;
    require(
        usize::from(declaration.block_count) == blocks
            && (1..=16).contains(&declaration.instruction_count),
        "complete-body declaration counts",
    )?;
    let labels = &declaration.labels[..blocks];
    for (index, label) in labels.iter().enumerate() {
        require(
            !labels[..index].contains(label),
            "complete-body duplicate authored label",
        )?;
    }
    require(
        declaration.labels[blocks..].iter().all(|label| *label == 0),
        "complete-body unused label padding",
    )?;
    let registers = declaration.registers;
    require(
        [
            registers.scratch(),
            registers.output(),
            registers.inputs()[0],
            registers.inputs()[1],
            registers.inputs()[2],
        ]
        .into_iter()
        .all(|register| (8..64).contains(&register)),
        "complete-body reserved register",
    )?;
    Ok(())
}

fn check_selector(block: &BasicBlock, operations: &[Operation], selector: ValueId) -> Result<()> {
    let [zero, comparison] = operations else {
        return Err(Error::Invalid("complete-body selector operations"));
    };
    require(
        matches!(zero.kind, OperationKind::Constant(Constant::U32(0))),
        "complete-body selector zero",
    )?;
    let zero = scalar_result(zero)?;
    require(
        matches!(comparison.kind, OperationKind::Compare {
        predicate: ComparePredicate::Equal, lhs, rhs } if lhs == selector && rhs == zero),
        "complete-body selector comparison",
    )?;
    let condition = one_result(comparison, bool_type)?;
    require(
        matches!(&block.terminator, Some(Terminator::ConditionalBranch {
        condition: actual, then_target: BlockId(1), else_target: BlockId(2), ..
    }) if *actual == condition),
        "complete-body selector successors",
    )
}

fn check_edge(arguments: &[ValueId], values: Values, target: Values) -> Result<()> {
    let mut expected = [ValueId(0); 2];
    let mut count = 0;
    for role in 3..5 {
        if target[role].is_some() {
            expected[count] =
                values[role].ok_or(Error::Invalid("complete-body undefined edge role"))?;
            count += 1;
        }
    }
    require(
        arguments == &expected[..count],
        "complete-body exact SSA edge arguments",
    )
}

fn check_tail(
    block: &BasicBlock,
    operations: &[Operation],
    output: ValueId,
    value: ValueId,
) -> Result<()> {
    let [index, length, comparison, base, pointer, store] = operations else {
        return Err(Error::Invalid("complete-body guarded tail operation count"));
    };
    require(
        matches!(&index.kind, OperationKind::Intrinsic(intrinsic)
        if *intrinsic == IntrinsicOperation::global_id_1d()),
        "complete-body global index",
    )?;
    let index = one_result(index, index_type)?;
    require(
        matches!(length.kind, OperationKind::SliceLength { slice } if slice == output),
        "complete-body output length",
    )?;
    let length = one_result(length, index_type)?;
    require(
        matches!(comparison.kind, OperationKind::Compare {
        predicate: ComparePredicate::LessThan, lhs, rhs } if lhs == index && rhs == length),
        "complete-body bounds predicate",
    )?;
    let predicate = one_result(comparison, bool_type)?;
    require(
        matches!(base.kind, OperationKind::SliceData { slice } if slice == output),
        "complete-body output base",
    )?;
    let base = one_result(base, output_pointer)?;
    require(
        matches!(pointer.kind, OperationKind::GetElementPointer { base: actual, offset }
        if actual == base && offset == index),
        "complete-body output address",
    )?;
    let pointer = one_result(pointer, output_pointer)?;
    require(
        store.results.is_empty()
            && matches!(store.kind, OperationKind::GuardedStore {
        pointer: actual_pointer, predicate: actual_predicate, value: actual_value, access
    } if actual_pointer == pointer && actual_predicate == predicate && actual_value == value
        && access.address_space == AddressSpace::Global && access.alignment == 4 && !access.volatile),
        "complete-body actual guarded output store",
    )?;
    require(
        matches!(&block.terminator, Some(Terminator::Return { values }) if values.is_empty()),
        "complete-body terminal return",
    )
}

#[cfg(test)]
#[path = "gfx942_complete_body_profile_v19_tests.rs"]
pub(crate) mod tests;
