//! Direct typed KIR evaluation. Rejected payloads are explicitly erased.
use super::*;
use std::collections::BTreeMap;

include!("../src/gfx942_local_checked_u32_add_v1/source_prefix/kernel_body.rs");

verus! {

struct Ignored {}
enum ScalarType { Bool, I8, I16, I32, I64, I128, U8, U16, U32, U64, U128, Index, F16, Bf16, F32, F64 }
enum Type { Unit, Scalar(ScalarType), Pointer(Ignored), Slice(Ignored) }
struct ValueDef { pub id: ValueId, pub ty: Type }
struct Operation { pub results: Vec<ValueDef>, pub kind: OperationKind }
enum Constant {
    Bool(bool), I8(i8), I16(i16), I32(i32), I64(i64), U8(u8), U16(u16), U32(u32), U64(u64),
    Index(u64), F16Bits(u16), Bf16Bits(u16), F32Bits(u32), F64Bits(u64),
}
enum BinaryOp { Add, Subtract, Multiply, Divide, Remainder, BitAnd, BitOr, BitXor, ShiftLeft, ShiftRight, Checked(CheckedBinaryOperator) }
enum CheckedBinaryOperator { Add, Subtract, Multiply }
#[allow(inconsistent_fields)]
enum OperationKind {
    Constant(Constant), Intrinsic(Ignored), MemoryIntrinsic(Ignored),
    Unary { op: Ignored, operand: ValueId },
    Binary { op: BinaryOp, lhs: ValueId, rhs: ValueId },
    Compare { predicate: Ignored, lhs: ValueId, rhs: ValueId },
    Cast { kind: Ignored, value: ValueId, to: Type },
    Select { condition: ValueId, true_value: ValueId, false_value: ValueId },
    Call { callee: Ignored, arguments: Vec<ValueId> },
    Alloca { element: Type, count: Option<ValueId>, address_space: Ignored, alignment: u32 },
    SliceLength { slice: ValueId }, SliceData { slice: ValueId },
    GetElementPointer { base: ValueId, offset: ValueId },
    Load { pointer: ValueId, access: Ignored },
    GuardedLoad { pointer: ValueId, predicate: ValueId, fallback: ValueId, access: Ignored },
    GuardedStore { pointer: ValueId, predicate: ValueId, value: ValueId, access: Ignored },
    Store { pointer: ValueId, value: ValueId, access: Ignored },
    Barrier(Ignored), Atomic(Ignored), Fence(Ignored), WorkgroupBarrier(Ignored),
    WorkgroupMemory(Ignored), Matrix(Ignored), Gfx950LdsTranspose(Ignored), Wave(Ignored), InlineAssembly(Ignored),
}
enum CheckedU32PrefixErrorV1 { Kernel, ValueMismatch }

spec fn constant(operation: Operation) -> Option<(u32, u32)> {
    match operation.kind {
        OperationKind::Constant(Constant::U32(value)) => {
            if operation.results@.len() == 1 && operation.results@[0].ty == Type::Scalar(ScalarType::U32) {
                Some((operation.results@[0].id.0, value))
            } else { None }
        },
        _ => None,
    }
}

fn constant_binding(operation: &Operation) -> (result: Option<(u32, u32)>)
    ensures result == constant(*operation),
{
    checked_u32_prefix_kernel_constant_body_v1!(verus_exec_expr, operation)
}

spec fn origin_result(result: Result<Origin, CheckedU32PrefixErrorV1>) -> Option<Origin> {
    match result { Ok(origin) => Some(origin), Err(_) => None }
}

spec fn terminal_result(
    operation: Operation, previous: Operation, origins: Map<u32, Origin>,
    operand: u32, value: u32, overflow: u32, literal: u32,
) -> Option<Origin> {
    match operation.kind {
        OperationKind::Binary { op: BinaryOp::Checked(CheckedBinaryOperator::Add), lhs, rhs } => {
            if operation.results@.len() == 2
                && lhs.0 == operand && operation.results@[0].id.0 == value && operation.results@[1].id.0 == overflow
                && value != overflow && !origins.contains_key(value) && !origins.contains_key(overflow)
                && operation.results@[0].ty == Type::Scalar(ScalarType::U32)
                && operation.results@[1].ty == Type::Scalar(ScalarType::Bool)
                && constant(previous) == Some((rhs.0, literal))
                && origins.contains_key(rhs.0) && origins[rhs.0] == Origin::Constant(literal)
                && origins.contains_key(lhs.0)
            { Some(origins[lhs.0]) } else { None }
        },
        _ => None,
    }
}

fn terminal_origin(
    terminal: &Operation, previous: &Operation, origins: &BTreeMap<u32, Origin>,
    operand: u32, value: u32, overflow: u32, literal: u32,
) -> (result: Result<Origin, CheckedU32PrefixErrorV1>)
    ensures origin_result(result) == terminal_result(*terminal, *previous, origins@, operand, value, overflow, literal),
{
    broadcast use vstd::std_specs::btree::group_btree_axioms;
    checked_u32_prefix_kernel_terminal_body_v1!(verus_exec_expr, terminal, previous, origins, operand, value, overflow, literal)
}

spec fn arguments_prefix(rows: Seq<CheckedU32PrefixArgumentV1>, count: nat) -> Option<Map<u32, Origin>>
    decreases count,
{
    if count == 0 { Some(Map::empty()) } else {
        match arguments_prefix(rows, (count - 1) as nat) {
            None => None,
            Some(before) => if rows[count - 1].argument == count - 1 && !before.contains_key(rows[count - 1].kernel_ir_value.0) {
                Some(before.insert(rows[count - 1].kernel_ir_value.0, Origin::Argument((count - 1) as usize)))
            } else { None },
        }
    }
}

proof fn arguments_success_prefix(rows: Seq<CheckedU32PrefixArgumentV1>, count: nat, end: nat)
    requires count <= end, arguments_prefix(rows, end).is_some(),
    ensures arguments_prefix(rows, count).is_some(),
    decreases end - count,
{
    if count < end { arguments_success_prefix(rows, count, (end - 1) as nat); }
}

spec fn constants_prefix(operations: Seq<Operation>, initial: Map<u32, Origin>, count: nat) -> Option<Map<u32, Origin>>
    decreases count,
{
    if count == 0 { Some(initial) } else {
        match constants_prefix(operations, initial, (count - 1) as nat) {
            None => None,
            Some(before) => match constant(operations[count - 1]) {
                Some((id, value)) => if before.contains_key(id) { None } else { Some(before.insert(id, Origin::Constant(value))) },
                None => None,
            },
        }
    }
}

proof fn constants_success_prefix(operations: Seq<Operation>, initial: Map<u32, Origin>, count: nat, end: nat)
    requires count <= end, constants_prefix(operations, initial, end).is_some(),
    ensures constants_prefix(operations, initial, count).is_some(),
    decreases end - count,
{
    if count < end { constants_success_prefix(operations, initial, count, (end - 1) as nat); }
}

spec fn assembly(rows: Seq<CheckedU32PrefixArgumentV1>, operations: Seq<Operation>,
    operand: u32, value: u32, overflow: u32, literal: u32) -> Option<Origin>
{
    if rows.len() > 128 || operations.len() < 2 || operations.len() > 257 { None }
    else {
        match arguments_prefix(rows, rows.len()) {
            None => None,
            Some(initial) => match constants_prefix(operations, initial, (operations.len() - 1) as nat) {
                None => None,
                Some(origins) => terminal_result(operations.last(), operations[operations.len() - 2], origins, operand, value, overflow, literal),
            },
        }
    }
}

fn assemble_kernel(arguments: &[CheckedU32PrefixArgumentV1], operations: &[Operation],
    operand: u32, value: u32, overflow: u32, literal: u32) -> (result: Result<Origin, CheckedU32PrefixErrorV1>)
    ensures origin_result(result) == assembly(arguments@, operations@, operand, value, overflow, literal),
{
    broadcast use vstd::std_specs::btree::group_btree_axioms;
    checked_u32_prefix_kernel_assemble_body_v1!(verus_exec_expr, arguments, operations, operand, value, overflow, literal,
        origins, argument, [
            invariant
                argument <= arguments.len(), arguments.len() <= 128, 2 <= operations.len() <= 257,
                arguments_prefix(arguments@, argument as nat) == Some(origins@),
            decreases arguments.len() - argument,
        ], [
            proof {
                if arguments_prefix(arguments@, arguments@.len()).is_some() {
                    arguments_success_prefix(arguments@, (argument + 1) as nat, arguments@.len());
                }
            }
        ], index, [
            invariant
                index < operations.len(), arguments.len() <= 128, 2 <= operations.len() <= 257,
                arguments_prefix(arguments@, arguments@.len()).is_some(),
                constants_prefix(operations@, arguments_prefix(arguments@, arguments@.len()).unwrap(), index as nat) == Some(origins@),
            decreases operations.len() - 1 - index,
        ], [
            proof {
                let initial = arguments_prefix(arguments@, arguments@.len()).unwrap();
                if constants_prefix(operations@, initial, (operations@.len() - 1) as nat).is_some() {
                    constants_success_prefix(operations@, initial, (index + 1) as nat, (operations@.len() - 1) as nat);
                }
            }
        ])
}

spec fn denote_map(origins: Map<u32, Origin>, values: Seq<u32>) -> Map<u32, Option<u32>> {
    Map::new(origins.dom(), |id: u32| denote_origin(origins[id], values))
}

spec fn concrete_arguments(rows: Seq<CheckedU32PrefixArgumentV1>, values: Seq<u32>, count: nat) -> Option<Map<u32, Option<u32>>>
    decreases count,
{
    if count == 0 { Some(Map::empty()) } else {
        match concrete_arguments(rows, values, (count - 1) as nat) {
            None => None,
            Some(before) => if rows[count - 1].argument == count - 1 && !before.contains_key(rows[count - 1].kernel_ir_value.0) {
                Some(before.insert(rows[count - 1].kernel_ir_value.0, Some(values[count - 1])))
            } else { None },
        }
    }
}

// Executes the actual typed constant instruction, independent of its normalizer.
spec fn execute_constants(operations: Seq<Operation>, initial: Map<u32, Option<u32>>, count: nat) -> Option<Map<u32, Option<u32>>>
    decreases count,
{
    if count == 0 { Some(initial) } else {
        match execute_constants(operations, initial, (count - 1) as nat) {
            None => None,
            Some(before) => match operations[count - 1].kind {
                OperationKind::Constant(Constant::U32(value)) => {
                    let results = operations[count - 1].results@;
                    if results.len() == 1 && results[0].ty == Type::Scalar(ScalarType::U32) && !before.contains_key(results[0].id.0) {
                        Some(before.insert(results[0].id.0, Some(value)))
                    } else { None }
                },
                _ => None,
            },
        }
    }
}

proof fn arguments_denotation(rows: Seq<CheckedU32PrefixArgumentV1>, values: Seq<u32>, count: nat)
    requires count <= rows.len(), values.len() == rows.len(), arguments_prefix(rows, count).is_some(),
    ensures concrete_arguments(rows, values, count) == Some(denote_map(arguments_prefix(rows, count).unwrap(), values)),
        forall|id: u32| arguments_prefix(rows, count).unwrap().contains_key(id) ==>
            match #[trigger] arguments_prefix(rows, count).unwrap()[id] {
                Origin::Argument(index) => index < count, _ => false,
            },
    decreases count,
{
    if count == 0 {
        assert(denote_map(Map::empty(), values) =~= Map::<u32, Option<u32>>::empty());
    } else {
        arguments_denotation(rows, values, (count - 1) as nat);
        let before = arguments_prefix(rows, (count - 1) as nat).unwrap();
        let id = rows[count - 1].kernel_ir_value.0;
        assert(denote_map(before.insert(id, Origin::Argument((count - 1) as usize)), values)
            =~= denote_map(before, values).insert(id, Some(values[count - 1])));
    }
}

proof fn constants_denotation(operations: Seq<Operation>, initial: Map<u32, Origin>, values: Seq<u32>, count: nat)
    requires count <= operations.len(), constants_prefix(operations, initial, count).is_some(),
        forall|id: u32| initial.contains_key(id) ==> match #[trigger] initial[id] {
            Origin::Uninitialized => false, Origin::Argument(index) => index < values.len(), _ => true,
        },
    ensures execute_constants(operations, denote_map(initial, values), count)
        == Some(denote_map(constants_prefix(operations, initial, count).unwrap(), values)),
        forall|id: u32| constants_prefix(operations, initial, count).unwrap().contains_key(id) ==>
            match #[trigger] constants_prefix(operations, initial, count).unwrap()[id] {
                Origin::Uninitialized => false, Origin::Argument(index) => index < values.len(), _ => true,
            },
    decreases count,
{
    if count > 0 {
        constants_denotation(operations, initial, values, (count - 1) as nat);
        let before = constants_prefix(operations, initial, (count - 1) as nat).unwrap();
        let (id, value) = constant(operations[count - 1]).unwrap();
        assert(denote_map(before.insert(id, Origin::Constant(value)), values)
            =~= denote_map(before, values).insert(id, Some(value)));
    }
}

proof fn assembled_kernel_denotation(rows: Seq<CheckedU32PrefixArgumentV1>, operations: Seq<Operation>,
    values: Seq<u32>, operand: u32, value: u32, overflow: u32, literal: u32)
    requires values.len() == rows.len(), assembly(rows, operations, operand, value, overflow, literal).is_some(),
    ensures
        concrete_arguments(rows, values, rows.len()).is_some(),
        execute_constants(operations, concrete_arguments(rows, values, rows.len()).unwrap(), (operations.len() - 1) as nat).is_some(),
        execute_constants(operations, concrete_arguments(rows, values, rows.len()).unwrap(), (operations.len() - 1) as nat).unwrap().contains_key(operand),
        execute_constants(operations, concrete_arguments(rows, values, rows.len()).unwrap(), (operations.len() - 1) as nat).unwrap()[operand]
            == denote_origin(assembly(rows, operations, operand, value, overflow, literal).unwrap(), values),
        match assembly(rows, operations, operand, value, overflow, literal).unwrap() {
            Origin::Uninitialized => false, Origin::Argument(index) => index < values.len(), _ => true,
        },
{
    arguments_denotation(rows, values, rows.len());
    let initial = arguments_prefix(rows, rows.len()).unwrap();
    constants_denotation(operations, initial, values, (operations.len() - 1) as nat);
}

spec fn execute_checked_add(operation: Operation, state: Map<u32, Option<u32>>) -> Option<(u32, bool)> {
    match operation.kind {
        OperationKind::Binary { op: BinaryOp::Checked(CheckedBinaryOperator::Add), lhs, rhs } => {
            if operation.results@.len() == 2
                && operation.results@[0].ty == Type::Scalar(ScalarType::U32)
                && operation.results@[1].ty == Type::Scalar(ScalarType::Bool)
                && state.contains_key(lhs.0) && state.contains_key(rhs.0)
                && state[lhs.0].is_some() && state[rhs.0].is_some()
            {
                let sum = state[lhs.0].unwrap() as int + state[rhs.0].unwrap() as int;
                Some(((sum % 0x1_0000_0000) as u32, sum >= 0x1_0000_0000))
            } else { None }
        },
        _ => None,
    }
}

proof fn source_fold_kernel_add_denotation(rows: Seq<CheckedU32PrefixArgumentV1>, operations: Seq<Operation>,
    source: Seq<Origin>, steps: Seq<PrefixStep>, slot: int, values: Seq<u32>,
    operand: u32, value: u32, overflow: u32, literal: u32)
    requires values.len() == rows.len(), valid_origins(source, values), 0 <= slot < source.len(),
        assembly(rows, operations, operand, value, overflow, literal).is_some(),
        symbolic_after(source, steps, steps.len()).is_some(),
        symbolic_after(source, steps, steps.len()).unwrap()[slot] == assembly(rows, operations, operand, value, overflow, literal).unwrap(),
    ensures
        concrete_after(denote(source, values), steps, steps.len()).is_some(),
        concrete_after(denote(source, values), steps, steps.len()).unwrap()[slot].is_some(),
        execute_checked_add(operations.last(), execute_constants(operations, concrete_arguments(rows, values, rows.len()).unwrap(), (operations.len() - 1) as nat).unwrap())
            == Some((((concrete_after(denote(source, values), steps, steps.len()).unwrap()[slot].unwrap() as int + literal as int) % 0x1_0000_0000) as u32,
                concrete_after(denote(source, values), steps, steps.len()).unwrap()[slot].unwrap() as int + literal as int >= 0x1_0000_0000)),
{
    assembled_kernel_denotation(rows, operations, values, operand, value, overflow, literal);
    prefix_denotation(source, steps, steps.len(), values);
}

}
