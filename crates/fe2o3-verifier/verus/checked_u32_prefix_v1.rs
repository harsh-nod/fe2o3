//! Shared argument-basis, typed source-statement normalization and fold denotation.
//! Includes actual source-span and KIR assembly; terminal source AST, ABI and authority remain separate.
use vstd::prelude::*;

#[path = "checked_u32_kernel_v1.rs"]
mod kernel_assembly;

include!("../src/gfx942_local_checked_u32_add_v1/source_prefix/fold_body.rs");
include!("../src/gfx942_local_checked_u32_add_v1/source_prefix/basis_body.rs");
include!("../src/gfx942_local_checked_u32_add_v1/source_prefix/normalize_body.rs");
include!("../src/gfx942_local_checked_u32_add_v1/source_prefix/assemble_body.rs");
include!("../../fe2o3-kernel-analysis/src/gfx942_integer_semantics_v1/add_u32_body.rs");

verus! {

#[derive(Clone, Copy)]
enum Origin {
    Uninitialized,
    Argument(usize),
    Constant(u32),
}

#[derive(Clone, Copy)]
enum PrefixInput {
    Cell(usize),
    Constant(u32),
}

#[derive(Clone, Copy)]
struct PrefixStep {
    destination: usize,
    input: PrefixInput,
}

#[derive(Clone, Copy)]
struct ValueId(u32);

#[derive(Clone, Copy)]
struct CheckedU32PrefixArgumentV1 {
    argument: usize,
    semantic_local: u32,
    kernel_ir_value: ValueId,
}

struct Gfx942U32AddResultV1 {
    value: u32,
    scc: bool,
}

spec fn symbolic_step(state: Seq<Origin>, step: PrefixStep) -> Option<Seq<Origin>> {
    if step.destination >= state.len() {
        None
    } else {
        let value = match step.input {
            PrefixInput::Constant(value) => Some(Origin::Constant(value)),
            PrefixInput::Cell(source) => if source < state.len() { Some(state[source as int]) } else { None },
        };
        match value {
            Some(Origin::Uninitialized) | None => None,
            Some(origin) => Some(state.update(step.destination as int, origin)),
        }
    }
}

spec fn symbolic_after(initial: Seq<Origin>, steps: Seq<PrefixStep>, count: nat) -> Option<Seq<Origin>>
    decreases count,
{
    if count == 0 { Some(initial) }
    else {
        match symbolic_after(initial, steps, (count - 1) as nat) {
            None => None,
            Some(before) => symbolic_step(before, steps[count - 1]),
        }
    }
}

fn fold(state: &mut [Origin], steps: &[PrefixStep]) -> (accepted: bool)
    ensures accepted ==> symbolic_after(old(state)@, steps@, steps@.len()) == Some(final(state)@),
{
    let ghost before = state@;
    checked_u32_prefix_fold_body_v1!(verus_exec_expr, state, steps, index, [
        invariant
            index <= steps.len(),
            state@.len() == before.len(),
            symbolic_after(before, steps@, index as nat) == Some(state@),
        decreases steps.len() - index,
    ])
}

spec fn valid_basis_rows(rows: Seq<CheckedU32PrefixArgumentV1>, locals: nat, count: nat) -> bool {
    count <= rows.len()
    && (forall|i: int| 0 <= i < count ==> (#[trigger] rows[i]).argument == i
        && rows[i].semantic_local < locals)
    && (forall|i: int, j: int| 0 <= i < j < count ==>
        (#[trigger] rows[i]).semantic_local != (#[trigger] rows[j]).semantic_local)
}

spec fn paired_basis_prefix(
    rows: Seq<CheckedU32PrefixArgumentV1>, source: Seq<Origin>, kernel: Seq<Origin>, count: nat,
) -> bool {
    count <= rows.len() && kernel.len() == rows.len()
    && (forall|i: int| 0 <= i < count ==>
        source[(#[trigger] rows[i]).semantic_local as int] == Origin::Argument(i as usize))
    && (forall|i: int| 0 <= i < count ==> #[trigger] kernel[i] == Origin::Argument(i as usize))
    && (forall|slot: int| 0 <= slot < source.len()
        && !(exists|i: int| 0 <= i < count && #[trigger] rows[i].semantic_local == slot)
        ==> #[trigger] source[slot] == Origin::Uninitialized)
}

fn initialize_argument_basis(
    arguments: &[CheckedU32PrefixArgumentV1], source: &mut [Origin], kernel: &mut [Origin],
) -> (accepted: bool)
    ensures
        accepted == (old(kernel)@.len() == arguments@.len()
            && valid_basis_rows(arguments@, old(source)@.len(), arguments@.len())),
        accepted ==> paired_basis_prefix(arguments@, final(source)@, final(kernel)@, arguments@.len()),
        final(source)@.len() == old(source)@.len(),
        final(kernel)@.len() == old(kernel)@.len(),
{
    checked_u32_prefix_basis_body_v1!(verus_exec_expr, arguments, source, kernel,
        clear, [
            invariant
                clear <= source.len(), source@.len() == old(source)@.len(),
                kernel@.len() == old(kernel)@.len(), kernel@.len() == arguments@.len(),
                forall|slot: int| 0 <= slot < clear ==> #[trigger] source@[slot] == Origin::Uninitialized,
            decreases source.len() - clear,
        ], index, [
            invariant
                index <= arguments.len(), source@.len() == old(source)@.len(),
                kernel@.len() == old(kernel)@.len(), kernel@.len() == arguments@.len(),
                valid_basis_rows(arguments@, old(source)@.len(), index as nat),
                paired_basis_prefix(arguments@, source@, kernel@, index as nat),
            decreases arguments.len() - index,
        ])
}

spec fn valid_origins(state: Seq<Origin>, arguments: Seq<u32>) -> bool {
    forall|i: int| 0 <= i < state.len() ==> match #[trigger] state[i] {
        Origin::Argument(argument) => argument < arguments.len(),
        _ => true,
    }
}

spec fn denote_origin(origin: Origin, arguments: Seq<u32>) -> Option<u32> {
    match origin {
        Origin::Uninitialized => None,
        Origin::Argument(argument) => Some(arguments[argument as int]),
        Origin::Constant(value) => Some(value),
    }
}

spec fn denote(state: Seq<Origin>, arguments: Seq<u32>) -> Seq<Option<u32>> {
    Seq::new(state.len(), |i: int| denote_origin(state[i], arguments))
}

proof fn initialized_basis_denotation(
    rows: Seq<CheckedU32PrefixArgumentV1>, source: Seq<Origin>, kernel: Seq<Origin>, values: Seq<u32>,
)
    requires
        values.len() == rows.len(),
        valid_basis_rows(rows, source.len(), rows.len()),
        paired_basis_prefix(rows, source, kernel, rows.len()),
    ensures
        valid_origins(source, values), valid_origins(kernel, values),
        forall|i: int| 0 <= i < rows.len() ==>
            #[trigger] denote(source, values)[rows[i].semantic_local as int] == Some(values[i])
            && #[trigger] denote(kernel, values)[i] == Some(values[i]),
{
    assert forall|slot: int| 0 <= slot < source.len() implies
        match #[trigger] source[slot] {
            Origin::Argument(argument) => argument < values.len(),
            _ => true,
        } by {
        if exists|i: int| 0 <= i < rows.len() && #[trigger] rows[i].semantic_local == slot {
            let i = choose|i: int| 0 <= i < rows.len() && #[trigger] rows[i].semantic_local == slot;
            assert(source[slot] == Origin::Argument(i as usize));
        }
    }
}

spec fn concrete_step(state: Seq<Option<u32>>, step: PrefixStep) -> Option<Seq<Option<u32>>> {
    if step.destination >= state.len() { None }
    else {
        let value = match step.input {
            PrefixInput::Constant(value) => Some(value),
            PrefixInput::Cell(source) => if source < state.len() { state[source as int] } else { None },
        };
        match value {
            None => None,
            Some(value) => Some(state.update(step.destination as int, Some(value))),
        }
    }
}

spec fn concrete_after(initial: Seq<Option<u32>>, steps: Seq<PrefixStep>, count: nat)
    -> Option<Seq<Option<u32>>>
    decreases count,
{
    if count == 0 { Some(initial) }
    else {
        match concrete_after(initial, steps, (count - 1) as nat) {
            None => None,
            Some(before) => concrete_step(before, steps[count - 1]),
        }
    }
}

proof fn step_denotation(state: Seq<Origin>, step: PrefixStep, arguments: Seq<u32>)
    requires valid_origins(state, arguments), symbolic_step(state, step).is_some(),
    ensures
        valid_origins(symbolic_step(state, step).unwrap(), arguments),
        symbolic_step(state, step).unwrap().len() == state.len(),
        concrete_step(denote(state, arguments), step)
            == Some(denote(symbolic_step(state, step).unwrap(), arguments)),
{
    let origin = match step.input {
        PrefixInput::Constant(value) => Origin::Constant(value),
        PrefixInput::Cell(source) => state[source as int],
    };
    let after = state.update(step.destination as int, origin);
    assert forall|i: int| 0 <= i < after.len() implies
        match #[trigger] after[i] {
            Origin::Argument(argument) => argument < arguments.len(),
            _ => true,
        } by {
        if i != step.destination { assert(after[i] == state[i]); }
    }
    assert(denote(after, arguments) =~= denote(state, arguments).update(
        step.destination as int, denote_origin(origin, arguments)));
}

proof fn prefix_denotation(initial: Seq<Origin>, steps: Seq<PrefixStep>, count: nat, arguments: Seq<u32>)
    requires count <= steps.len(), valid_origins(initial, arguments),
        symbolic_after(initial, steps, count).is_some(),
    ensures
        valid_origins(symbolic_after(initial, steps, count).unwrap(), arguments),
        symbolic_after(initial, steps, count).unwrap().len() == initial.len(),
        concrete_after(denote(initial, arguments), steps, count)
            == Some(denote(symbolic_after(initial, steps, count).unwrap(), arguments)),
    decreases count,
{
    if count > 0 {
        prefix_denotation(initial, steps, (count - 1) as nat, arguments);
        step_denotation(symbolic_after(initial, steps, (count - 1) as nat).unwrap(),
                        steps[count - 1], arguments);
    }
}

proof fn equal_terminal_values(
    source: Seq<Origin>, source_steps: Seq<PrefixStep>, source_slot: int,
    kernel: Seq<Origin>, kernel_steps: Seq<PrefixStep>, kernel_slot: int,
    arguments: Seq<u32>,
)
    requires
        valid_origins(source, arguments), valid_origins(kernel, arguments),
        symbolic_after(source, source_steps, source_steps.len()).is_some(),
        symbolic_after(kernel, kernel_steps, kernel_steps.len()).is_some(),
        0 <= source_slot < source.len(), 0 <= kernel_slot < kernel.len(),
        symbolic_after(source, source_steps, source_steps.len()).unwrap()[source_slot]
            == symbolic_after(kernel, kernel_steps, kernel_steps.len()).unwrap()[kernel_slot],
        symbolic_after(source, source_steps, source_steps.len()).unwrap()[source_slot] != Origin::Uninitialized,
    ensures
        concrete_after(denote(source, arguments), source_steps, source_steps.len()).is_some(),
        concrete_after(denote(kernel, arguments), kernel_steps, kernel_steps.len()).is_some(),
        concrete_after(denote(source, arguments), source_steps, source_steps.len()).unwrap()[source_slot].is_some(),
        concrete_after(denote(source, arguments), source_steps, source_steps.len()).unwrap()[source_slot]
            == concrete_after(denote(kernel, arguments), kernel_steps, kernel_steps.len()).unwrap()[kernel_slot],
{
    prefix_denotation(source, source_steps, source_steps.len(), arguments);
    prefix_denotation(kernel, kernel_steps, kernel_steps.len(), arguments);
}

spec fn padded_kernel_basis(kernel: Seq<Origin>, padding: nat) -> Seq<Origin> {
    kernel + Seq::new(padding, |_slot: int| Origin::Uninitialized)
}

proof fn initialized_terminal_values(
    rows: Seq<CheckedU32PrefixArgumentV1>,
    source: Seq<Origin>, source_steps: Seq<PrefixStep>, source_slot: int,
    kernel: Seq<Origin>, padding: nat, kernel_steps: Seq<PrefixStep>, kernel_slot: int,
    arguments: Seq<u32>,
)
    requires
        arguments.len() == rows.len(),
        valid_basis_rows(rows, source.len(), rows.len()),
        paired_basis_prefix(rows, source, kernel, rows.len()),
        symbolic_after(source, source_steps, source_steps.len()).is_some(),
        symbolic_after(padded_kernel_basis(kernel, padding), kernel_steps, kernel_steps.len()).is_some(),
        0 <= source_slot < source.len(),
        0 <= kernel_slot < padded_kernel_basis(kernel, padding).len(),
        symbolic_after(source, source_steps, source_steps.len()).unwrap()[source_slot]
            == symbolic_after(padded_kernel_basis(kernel, padding), kernel_steps, kernel_steps.len()).unwrap()[kernel_slot],
        symbolic_after(source, source_steps, source_steps.len()).unwrap()[source_slot] != Origin::Uninitialized,
    ensures
        denote(padded_kernel_basis(kernel, padding), arguments)
            == denote(kernel, arguments) + Seq::new(padding, |_slot: int| None::<u32>),
        concrete_after(denote(source, arguments), source_steps, source_steps.len()).is_some(),
        concrete_after(denote(padded_kernel_basis(kernel, padding), arguments), kernel_steps, kernel_steps.len()).is_some(),
        concrete_after(denote(source, arguments), source_steps, source_steps.len()).unwrap()[source_slot].is_some(),
        concrete_after(denote(source, arguments), source_steps, source_steps.len()).unwrap()[source_slot]
            == concrete_after(denote(padded_kernel_basis(kernel, padding), arguments), kernel_steps, kernel_steps.len()).unwrap()[kernel_slot],
{
    initialized_basis_denotation(rows, source, kernel, arguments);
    let padded = padded_kernel_basis(kernel, padding);
    assert(denote(padded, arguments) =~= denote(kernel, arguments)
        + Seq::new(padding, |_slot: int| None::<u32>));
    assert forall|slot: int| 0 <= slot < padded.len() implies
        match #[trigger] padded[slot] {
            Origin::Argument(argument) => argument < arguments.len(),
            _ => true,
        } by {
        if slot < kernel.len() { assert(padded[slot] == kernel[slot]); }
        else { assert(padded[slot] == Origin::Uninitialized); }
    }
    equal_terminal_values(source, source_steps, source_slot, padded, kernel_steps, kernel_slot, arguments);
}

fn checked_add_value(lhs: u32, literal: u32) -> (result: Gfx942U32AddResultV1)
    ensures
        result.value as int == (lhs as int + literal as int) % 0x1_0000_0000,
        result.scc == (lhs as int + literal as int >= 0x1_0000_0000),
{
    gfx942_add_u32_body_v1!(lhs, literal)
}

mod normalization {
use super::*;

// Inspected fields and all discriminants match the retained semantic AST.
// The source control checks the explicit erasure of uninspected payload types.
struct Ignored;
#[derive(Clone, Copy)]
struct SemanticTypeIdV1(u32);
#[derive(Clone, Copy)]
struct SemanticLocalIdV1(u32);
#[derive(Clone, Copy)]
enum SemanticScalarTypeV1 {
    Bool, Char, Integer { signed: bool, bits: u16 }, Float { bits: u16 },
}
enum SemanticTypeShapeV1 {
    Unit, Never, Scalar(SemanticScalarTypeV1), ValidityScalar(Ignored), Pointer(Ignored),
    Array { element: SemanticTypeIdV1, length: u64 },
    Slice { element: SemanticTypeIdV1 },
    Tuple(Ignored), Aggregate(Ignored), Union(Ignored),
    Enum { discriminant: SemanticTypeIdV1, variants: Vec<Ignored> },
    FunctionPointer {
        safety: Ignored, extern_abi: Ignored, c_variadic: bool,
        arguments: Ignored, return_type: SemanticTypeIdV1,
    },
    Opaque,
}
struct SemanticTypeDeclV1 {
    identity: Ignored, layout_identity: Ignored, layout: Ignored,
    shape: SemanticTypeShapeV1, abi_properties: Ignored, rust_type_kind: Ignored,
}
struct SemanticLocalDeclV1 { identity: Ignored, ty: SemanticTypeIdV1, role: Ignored, source: Ignored }
struct SemanticPlaceV1 { local: SemanticLocalIdV1, projections: Vec<Ignored>, ty: SemanticTypeIdV1 }
#[derive(Clone, Copy)]
struct SemanticScalarValueV1 { bits: u128, size_bytes: u8 }
enum SemanticConstantValueV1 {
    ZeroSized, Scalar(SemanticScalarValueV1), Bytes(Ignored), Pointer(Ignored), Callable(Ignored),
}
struct SemanticConstantV1 { ty: SemanticTypeIdV1, value: SemanticConstantValueV1 }
enum SemanticOperandV1 { Copy(SemanticPlaceV1), Move(SemanticPlaceV1), Constant(SemanticConstantV1) }
enum SemanticRvalueKindV1 {
    Use(SemanticOperandV1),
    Unary { operation: Ignored, operand: SemanticOperandV1 },
    Binary { operation: Ignored, left: SemanticOperandV1, right: SemanticOperandV1 },
    CheckedBinary(Ignored), UncheckedBinary(Ignored),
    Cast { kind: Ignored, operand: SemanticOperandV1 },
    Borrow { kind: Ignored, place: SemanticPlaceV1 },
    AddressOf { mutability: Ignored, place: SemanticPlaceV1 },
    Length(SemanticPlaceV1), Discriminant(SemanticPlaceV1), Aggregate(Ignored), Load(Ignored),
}
struct SemanticRvalueV1 { result_type: SemanticTypeIdV1, kind: SemanticRvalueKindV1 }
struct SemanticAssignmentV1 { destination: SemanticPlaceV1, value: SemanticRvalueV1 }
enum SemanticStatementKindV1 {
    Assign(SemanticAssignmentV1), Store(Ignored), AtomicRmw(Ignored), AtomicCompareExchange(Ignored),
    SetDiscriminant { place: SemanticPlaceV1, variant_index: u32 },
    Deinitialize(SemanticPlaceV1), StorageLive(SemanticLocalIdV1), StorageDead(SemanticLocalIdV1),
    Assume(SemanticOperandV1), Nop,
}
struct SemanticStatementV1 { source: Ignored, kind: SemanticStatementKindV1 }
#[derive(Clone, Copy, Debug)]
enum CheckedU32PrefixErrorV1 {
    Version, Capacity, Entry, Arguments, Source, Span, Kernel, Uninitialized, ValueMismatch,
}

impl SemanticTypeIdV1 {
    fn index(self) -> (value: u32) ensures value == self.0 { self.0 }
}
impl SemanticLocalIdV1 {
    fn index(self) -> (value: u32) ensures value == self.0 { self.0 }
}
impl SemanticTypeDeclV1 {
    fn shape(&self) -> (value: &SemanticTypeShapeV1) ensures *value == self.shape { &self.shape }
}
impl SemanticLocalDeclV1 {
    fn ty(&self) -> (value: SemanticTypeIdV1) ensures value == self.ty { self.ty }
}
impl SemanticPlaceV1 {
    fn local(&self) -> (value: SemanticLocalIdV1) ensures value == self.local { self.local }
    fn projections(&self) -> (value: &[Ignored]) ensures value@ == self.projections@ { &self.projections }
    fn ty(&self) -> (value: SemanticTypeIdV1) ensures value == self.ty { self.ty }
}
impl SemanticScalarValueV1 {
    fn bits(self) -> (value: u128) ensures value == self.bits { self.bits }
    fn size_bytes(self) -> (value: u8) ensures value == self.size_bytes { self.size_bytes }
}
impl SemanticConstantV1 {
    fn ty(&self) -> (value: SemanticTypeIdV1) ensures value == self.ty { self.ty }
    fn value(&self) -> (value: &SemanticConstantValueV1) ensures *value == self.value { &self.value }
}
impl SemanticRvalueV1 {
    fn result_type(&self) -> (value: SemanticTypeIdV1) ensures value == self.result_type { self.result_type }
    fn kind(&self) -> (value: &SemanticRvalueKindV1) ensures *value == self.kind { &self.kind }
}
impl SemanticAssignmentV1 {
    fn destination(&self) -> (value: &SemanticPlaceV1) ensures *value == self.destination { &self.destination }
    fn value(&self) -> (value: &SemanticRvalueV1) ensures *value == self.value { &self.value }
}
impl SemanticStatementV1 {
    fn kind(&self) -> (value: &SemanticStatementKindV1) ensures *value == self.kind { &self.kind }
}

spec fn u32_type(types: Seq<SemanticTypeDeclV1>, ty: SemanticTypeIdV1) -> bool {
    ty.0 < types.len() && match types[ty.0 as int].shape {
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }) => !signed && bits == 32,
        _ => false,
    }
}
fn is_u32(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> (accepted: bool)
    ensures accepted == u32_type(types@, ty),
{
    checked_u32_prefix_type_body_v1!(verus_exec_expr, types, ty)
}

spec fn valid_place(types: Seq<SemanticTypeDeclV1>, locals: Seq<SemanticLocalDeclV1>, place: SemanticPlaceV1) -> bool {
    place.projections@.len() == 0 && u32_type(types, place.ty)
        && place.local.0 < locals.len() && locals[place.local.0 as int].ty.0 == place.ty.0
}
fn scalar_local(types: &[SemanticTypeDeclV1], locals: &[SemanticLocalDeclV1], place: &SemanticPlaceV1)
    -> (result: Result<usize, CheckedU32PrefixErrorV1>)
    ensures result == if valid_place(types@, locals@, *place) { Ok(place.local.0 as usize) }
        else { Err(CheckedU32PrefixErrorV1::Source) },
{
    checked_u32_prefix_local_body_v1!(verus_exec_expr, types, locals, place)
}

spec fn constant_value(types: Seq<SemanticTypeDeclV1>, operand: SemanticOperandV1) -> Option<u32> {
    match operand {
        SemanticOperandV1::Constant(constant) => match constant.value {
            SemanticConstantValueV1::Scalar(value) =>
                if u32_type(types, constant.ty) && value.size_bytes == 4 && value.bits <= u32::MAX {
                    Some(value.bits as u32)
                } else { None },
            _ => None,
        },
        _ => None,
    }
}
fn scalar_constant(types: &[SemanticTypeDeclV1], operand: &SemanticOperandV1) -> (result: Option<u32>)
    ensures result == constant_value(types@, *operand),
{
    checked_u32_prefix_constant_body_v1!(verus_exec_expr, types, operand)
}

spec fn typed_step(
    types: Seq<SemanticTypeDeclV1>, locals: Seq<SemanticLocalDeclV1>, statement: SemanticStatementV1, operations: u32,
) -> Result<Option<PrefixStep>, CheckedU32PrefixErrorV1> {
    match statement.kind {
        SemanticStatementKindV1::Nop => if operations == 0 { Ok(None) } else { Err(CheckedU32PrefixErrorV1::Source) },
        SemanticStatementKindV1::Assign(assign) => {
            if !valid_place(types, locals, assign.destination)
                || assign.value.result_type.0 != assign.destination.ty.0 {
                Err(CheckedU32PrefixErrorV1::Source)
            } else {
                match assign.value.kind {
                    SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) => {
                        if operations == 0 && valid_place(types, locals, place) {
                            Ok(Some(PrefixStep { destination: assign.destination.local.0 as usize,
                                input: PrefixInput::Cell(place.local.0 as usize) }))
                        } else { Err(CheckedU32PrefixErrorV1::Source) }
                    },
                    SemanticRvalueKindV1::Use(operand) => match constant_value(types, operand) {
                        Some(value) => if operations == 1 {
                            Ok(Some(PrefixStep { destination: assign.destination.local.0 as usize,
                                input: PrefixInput::Constant(value) }))
                        } else { Err(CheckedU32PrefixErrorV1::Source) },
                        None => Err(CheckedU32PrefixErrorV1::Source),
                    },
                    _ => Err(CheckedU32PrefixErrorV1::Source),
                }
            }
        },
        _ => Err(CheckedU32PrefixErrorV1::Source),
    }
}
fn source_step(types: &[SemanticTypeDeclV1], locals: &[SemanticLocalDeclV1], statement: &SemanticStatementV1, operations: u32)
    -> (result: Result<Option<PrefixStep>, CheckedU32PrefixErrorV1>)
    ensures result == typed_step(types@, locals@, *statement, operations),
{
    checked_u32_prefix_source_step_body_v1!(verus_exec_expr, types, locals, statement, operations)
}

// Independent concrete AST evaluation does not consult normalized rows or origins.
spec fn execute_statement(statement: SemanticStatementV1, state: Seq<Option<u32>>) -> Option<Seq<Option<u32>>> {
    match statement.kind {
        SemanticStatementKindV1::Nop => Some(state),
        SemanticStatementKindV1::Assign(assign) => {
            let destination = assign.destination.local.0 as int;
            let value = match assign.value.kind {
                SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) =>
                    if place.local.0 < state.len() { state[place.local.0 as int] } else { None },
                SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(constant)) => match constant.value {
                    SemanticConstantValueV1::Scalar(value) =>
                        if value.size_bytes == 4 && value.bits <= u32::MAX { Some(value.bits as u32) } else { None },
                    _ => None,
                },
                _ => None,
            };
            if destination < state.len() && value.is_some() { Some(state.update(destination, value)) } else { None }
        },
        _ => None,
    }
}

proof fn source_statement_denotation(
    types: Seq<SemanticTypeDeclV1>, locals: Seq<SemanticLocalDeclV1>, statement: SemanticStatementV1,
    operations: u32, state: Seq<Option<u32>>,
)
    requires state.len() == locals.len(), typed_step(types, locals, statement, operations).is_ok(),
    ensures execute_statement(statement, state) == match typed_step(types, locals, statement, operations).unwrap() {
        None => Some(state),
        Some(step) => concrete_step(state, step),
    },
{
    match statement.kind {
        SemanticStatementKindV1::Assign(assign) => match assign.value.kind {
            SemanticRvalueKindV1::Use(operand) => match operand {
                SemanticOperandV1::Copy(_) => {},
                SemanticOperandV1::Constant(constant) => match constant.value {
                    SemanticConstantValueV1::Scalar(_) => {},
                    _ => {},
                },
                _ => {},
            },
            _ => {},
        },
        _ => {},
    }
}

#[derive(Clone, Copy)]
struct SemanticFunctionIdV1(u32);
#[derive(Clone, Copy)]
struct SemanticBlockIdV1(u32);
#[derive(Clone, Copy)]
struct BlockId(u32);
impl SemanticFunctionIdV1 {
    fn index(self) -> (value: u32) ensures value == self.0 { self.0 }
}
impl SemanticBlockIdV1 {
    fn index(self) -> (value: u32) ensures value == self.0 { self.0 }
}
#[derive(Clone, Copy)]
struct SemanticKirStatementOperationSpanV1 {
    correspondence_owner: SemanticFunctionIdV1, semantic_function: SemanticFunctionIdV1,
    semantic_block: SemanticBlockIdV1, statement_ordinal: u32, kernel_ir_block: BlockId,
    first_operation_ordinal: u32, operation_count: u32,
}
impl SemanticKirStatementOperationSpanV1 {
    fn correspondence_owner(self) -> (value: SemanticFunctionIdV1) ensures value == self.correspondence_owner { self.correspondence_owner }
    fn semantic_function(self) -> (value: SemanticFunctionIdV1) ensures value == self.semantic_function { self.semantic_function }
    fn semantic_block(self) -> (value: SemanticBlockIdV1) ensures value == self.semantic_block { self.semantic_block }
    fn statement_ordinal(self) -> (value: u32) ensures value == self.statement_ordinal { self.statement_ordinal }
    fn kernel_ir_block(self) -> (value: BlockId) ensures value == self.kernel_ir_block { self.kernel_ir_block }
    fn first_operation_ordinal(self) -> (value: u32) ensures value == self.first_operation_ordinal { self.first_operation_ordinal }
    fn operation_count(self) -> (value: u32) ensures value == self.operation_count { self.operation_count }
}

spec fn selects(span: SemanticKirStatementOperationSpanV1, root: u32, function: u32, block: u32, length: nat) -> bool {
    span.correspondence_owner.0 == root && span.semantic_function.0 == function
        && span.semantic_block.0 == block && span.statement_ordinal < length
}

spec fn select_spans(spans: Seq<SemanticKirStatementOperationSpanV1>, root: u32, function: u32,
                     block: u32, length: nat, count: nat) -> Option<Seq<Option<usize>>>
    decreases count,
{
    if count > spans.len() || count > usize::MAX { None }
    else if count == 0 { Some(Seq::new(length, |_i: int| None)) }
    else {
        match select_spans(spans, root, function, block, length, (count - 1) as nat) {
            None => None,
            Some(before) => {
                let span = spans[count - 1];
                if selects(span, root, function, block, length) {
                    if before[span.statement_ordinal as int].is_some() { None }
                    else { Some(before.update(span.statement_ordinal as int, Some((count - 1) as usize))) }
                } else { Some(before) }
            },
        }
    }
}

proof fn selection_success_prefix(spans: Seq<SemanticKirStatementOperationSpanV1>, root: u32,
    function: u32, block: u32, length: nat, total: nat, count: nat)
    requires count <= total <= spans.len(), select_spans(spans, root, function, block, length, total).is_some(),
    ensures select_spans(spans, root, function, block, length, count).is_some(),
    decreases total - count,
{
    if count < total { selection_success_prefix(spans, root, function, block, length, (total - 1) as nat, count); }
}

spec fn walk_spans(types: Seq<SemanticTypeDeclV1>, locals: Seq<SemanticLocalDeclV1>,
    prefix: Seq<SemanticStatementV1>, spans: Seq<SemanticKirStatementOperationSpanV1>,
    selected: Seq<Option<usize>>, kernel_block: u32, operation: u32, count: nat)
    -> Option<(Seq<PrefixStep>, u32)>
    decreases count,
{
    if count == 0 { Some((Seq::empty(), 0u32)) }
    else {
        match walk_spans(types, locals, prefix, spans, selected, kernel_block, operation, (count - 1) as nat) {
            None => None,
            Some((steps, next)) => match selected[count - 1] {
                None => None,
                Some(index) => {
                    let span = spans[index as int];
                    let end = next as int + span.operation_count as int;
                    if span.kernel_ir_block.0 != kernel_block || span.first_operation_ordinal != next || end > u32::MAX {
                        None
                    } else if count < prefix.len() {
                        match typed_step(types, locals, prefix[count - 1], span.operation_count) {
                            Ok(None) => Some((steps, end as u32)),
                            Ok(Some(step)) => Some((steps.push(step), end as u32)),
                            Err(_) => None,
                        }
                    } else if span.operation_count != 2 || span.first_operation_ordinal as int + 1 != operation {
                        None
                    } else { Some((steps, end as u32)) }
                },
            },
        }
    }
}

proof fn walk_success_prefix(types: Seq<SemanticTypeDeclV1>, locals: Seq<SemanticLocalDeclV1>,
    prefix: Seq<SemanticStatementV1>, spans: Seq<SemanticKirStatementOperationSpanV1>, selected: Seq<Option<usize>>,
    kernel_block: u32, operation: u32, total: nat, count: nat)
    requires count <= total <= prefix.len(), selected.len() == prefix.len(),
        walk_spans(types, locals, prefix, spans, selected, kernel_block, operation, total).is_some(),
    ensures walk_spans(types, locals, prefix, spans, selected, kernel_block, operation, count).is_some(),
    decreases total - count,
{
    if count < total {
        walk_success_prefix(types, locals, prefix, spans, selected, kernel_block, operation, (total - 1) as nat, count);
    }
}

spec fn source_assembly(types: Seq<SemanticTypeDeclV1>, locals: Seq<SemanticLocalDeclV1>,
    prefix: Seq<SemanticStatementV1>, spans: Seq<SemanticKirStatementOperationSpanV1>,
    root: u32, function: u32, block: u32, kernel_block: u32, operation: u32) -> Option<(Seq<PrefixStep>, u32)>
{
    if prefix.len() == 0 || prefix.len() > 256 { None }
    else {
        match select_spans(spans, root, function, block, prefix.len(), spans.len()) {
            None => None,
            Some(selected) => walk_spans(types, locals, prefix, spans, selected, kernel_block, operation, prefix.len()),
        }
    }
}

spec fn assembly_result(result: Result<(Vec<PrefixStep>, u32), CheckedU32PrefixErrorV1>) -> Option<(Seq<PrefixStep>, u32)> {
    match result { Ok((steps, end)) => Some((steps@, end)), Err(_) => None }
}

fn assemble_source(types: &[SemanticTypeDeclV1], locals: &[SemanticLocalDeclV1],
    prefix: &[SemanticStatementV1], spans: &[SemanticKirStatementOperationSpanV1],
    root: u32, function: u32, block: u32, kernel_block: u32, operation: u32)
    -> (result: Result<(Vec<PrefixStep>, u32), CheckedU32PrefixErrorV1>)
    ensures assembly_result(result) == source_assembly(types@, locals@, prefix@, spans@, root, function, block, kernel_block, operation),
{
    checked_u32_prefix_assemble_body_v1!(verus_exec_expr, types, locals, prefix, spans, root, function, block,
        kernel_block, operation, selected, index, [
            invariant
                0 < prefix.len() <= 256, index <= spans.len(), selected.len() == prefix.len(),
                select_spans(spans@, root, function, block, prefix@.len(), index as nat) == Some(selected@),
                forall|i: int| 0 <= i < selected.len() && (#[trigger] selected@[i]).is_some()
                    ==> selected@[i].unwrap() < index,
            decreases spans.len() - index,
        ], [proof {
            if select_spans(spans@, root, function, block, prefix@.len(), spans@.len()).is_some() {
                selection_success_prefix(spans@, root, function, block, prefix@.len(), spans@.len(), (index + 1) as nat);
            }
        }], steps, next_operation, ordinal, [
            invariant
                0 < prefix.len() <= 256, ordinal <= prefix.len(), selected.len() == prefix.len(),
                select_spans(spans@, root, function, block, prefix@.len(), spans@.len()) == Some(selected@),
                forall|i: int| 0 <= i < selected.len() && (#[trigger] selected@[i]).is_some()
                    ==> selected@[i].unwrap() < spans.len(),
                walk_spans(types@, locals@, prefix@, spans@, selected@, kernel_block, operation, ordinal as nat)
                    == Some((steps@, next_operation)),
            decreases prefix.len() - ordinal,
        ], [proof {
            if walk_spans(types@, locals@, prefix@, spans@, selected@, kernel_block, operation, prefix@.len()).is_some() {
                walk_success_prefix(types@, locals@, prefix@, spans@, selected@, kernel_block, operation, prefix@.len(), (ordinal + 1) as nat);
            }
        }])
}

proof fn selected_original_spans(spans: Seq<SemanticKirStatementOperationSpanV1>, root: u32,
    function: u32, block: u32, length: nat, count: nat)
    requires count <= spans.len(), select_spans(spans, root, function, block, length, count).is_some(),
    ensures
        select_spans(spans, root, function, block, length, count).unwrap().len() == length,
        forall|i: int| 0 <= i < count && selects(#[trigger] spans[i], root, function, block, length)
            ==> select_spans(spans, root, function, block, length, count).unwrap()[spans[i].statement_ordinal as int] == Some(i as usize),
        forall|ordinal: int| 0 <= ordinal < length
            && (#[trigger] select_spans(spans, root, function, block, length, count).unwrap()[ordinal]).is_some()
            ==> {
                let index = select_spans(spans, root, function, block, length, count).unwrap()[ordinal].unwrap();
                index < count && selects(spans[index as int], root, function, block, length)
                    && spans[index as int].statement_ordinal == ordinal
            },
    decreases count,
{
    if count > 0 {
        selected_original_spans(spans, root, function, block, length, (count - 1) as nat);
        let before = select_spans(spans, root, function, block, length, (count - 1) as nat).unwrap();
        let span = spans[count - 1];
        if selects(span, root, function, block, length) {
            let after = before.update(span.statement_ordinal as int, Some((count - 1) as usize));
            assert forall|i: int| 0 <= i < count && selects(#[trigger] spans[i], root, function, block, length)
                implies after[spans[i].statement_ordinal as int] == Some(i as usize) by {
                if i < count - 1 {
                    assert(before[spans[i].statement_ordinal as int] == Some(i as usize));
                }
            }
            assert forall|ordinal: int| 0 <= ordinal < length && (#[trigger] after[ordinal]).is_some()
                implies {
                    let index = after[ordinal].unwrap();
                    index < count && selects(spans[index as int], root, function, block, length)
                        && spans[index as int].statement_ordinal == ordinal
                } by {
                if ordinal != span.statement_ordinal { assert(after[ordinal] == before[ordinal]); }
            }
        }
    }
}

// The terminal checked-add AST node is validated separately by check_parts.
// This evaluates only its preceding statements, preserving the terminal boundary.
spec fn execute_prefix(prefix: Seq<SemanticStatementV1>, initial: Seq<Option<u32>>, count: nat)
    -> Option<Seq<Option<u32>>>
    decreases count,
{
    if count == 0 { Some(initial) }
    else {
        match execute_prefix(prefix, initial, (count - 1) as nat) {
            None => None,
            Some(before) => if count < prefix.len() { execute_statement(prefix[count - 1], before) }
                else { Some(before) },
        }
    }
}

proof fn concrete_appended_prefix(initial: Seq<Option<u32>>, steps: Seq<PrefixStep>, step: PrefixStep, count: nat)
    requires count <= steps.len(),
    ensures concrete_after(initial, steps.push(step), count) == concrete_after(initial, steps, count),
    decreases count,
{
    if count > 0 { concrete_appended_prefix(initial, steps, step, (count - 1) as nat); }
}

proof fn concrete_length(initial: Seq<Option<u32>>, steps: Seq<PrefixStep>, count: nat)
    requires count <= steps.len(), concrete_after(initial, steps, count).is_some(),
    ensures concrete_after(initial, steps, count).unwrap().len() == initial.len(),
    decreases count,
{
    if count > 0 { concrete_length(initial, steps, (count - 1) as nat); }
}

proof fn walk_source_denotation(types: Seq<SemanticTypeDeclV1>, locals: Seq<SemanticLocalDeclV1>,
    prefix: Seq<SemanticStatementV1>, spans: Seq<SemanticKirStatementOperationSpanV1>, selected: Seq<Option<usize>>,
    kernel_block: u32, operation: u32, count: nat, initial: Seq<Option<u32>>)
    requires count <= prefix.len(), selected.len() == prefix.len(), initial.len() == locals.len(),
        walk_spans(types, locals, prefix, spans, selected, kernel_block, operation, count).is_some(),
    ensures {
        let steps = walk_spans(types, locals, prefix, spans, selected, kernel_block, operation, count).unwrap().0;
        execute_prefix(prefix, initial, count) == concrete_after(initial, steps, steps.len())
    },
    decreases count,
{
    if count > 0 {
        walk_source_denotation(types, locals, prefix, spans, selected, kernel_block, operation, (count - 1) as nat, initial);
        let before = walk_spans(types, locals, prefix, spans, selected, kernel_block, operation, (count - 1) as nat).unwrap();
        if count < prefix.len() {
            let span = spans[selected[count - 1].unwrap() as int];
            let normalized = typed_step(types, locals, prefix[count - 1], span.operation_count).unwrap();
            if let Some(step) = normalized {
                concrete_appended_prefix(initial, before.0, step, before.0.len());
            }
            if concrete_after(initial, before.0, before.0.len()).is_some() {
                concrete_length(initial, before.0, before.0.len());
                source_statement_denotation(types, locals, prefix[count - 1], span.operation_count,
                    concrete_after(initial, before.0, before.0.len()).unwrap());
            }
        }
    }
}

proof fn assembled_source_symbolic_denotation(types: Seq<SemanticTypeDeclV1>, locals: Seq<SemanticLocalDeclV1>,
    prefix: Seq<SemanticStatementV1>, spans: Seq<SemanticKirStatementOperationSpanV1>,
    root: u32, function: u32, block: u32, kernel_block: u32, operation: u32,
    steps: Seq<PrefixStep>, end: u32, initial: Seq<Origin>, arguments: Seq<u32>)
    requires
        source_assembly(types, locals, prefix, spans, root, function, block, kernel_block, operation) == Some((steps, end)),
        initial.len() == locals.len(), valid_origins(initial, arguments),
        symbolic_after(initial, steps, steps.len()).is_some(),
    ensures execute_prefix(prefix, denote(initial, arguments), prefix.len())
        == Some(denote(symbolic_after(initial, steps, steps.len()).unwrap(), arguments)),
{
    let selected = select_spans(spans, root, function, block, prefix.len(), spans.len()).unwrap();
    selected_original_spans(spans, root, function, block, prefix.len(), spans.len());
    walk_source_denotation(types, locals, prefix, spans, selected, kernel_block, operation, prefix.len(), denote(initial, arguments));
    prefix_denotation(initial, steps, steps.len(), arguments);
}

proof fn source_statement_symbolic_composition(
    types: Seq<SemanticTypeDeclV1>, locals: Seq<SemanticLocalDeclV1>, statement: SemanticStatementV1,
    operations: u32, state: Seq<Origin>, arguments: Seq<u32>, step: PrefixStep,
)
    requires state.len() == locals.len(), valid_origins(state, arguments),
        typed_step(types, locals, statement, operations) == Ok(Some(step)),
        symbolic_step(state, step).is_some(),
    ensures execute_statement(statement, denote(state, arguments))
        == Some(denote(symbolic_step(state, step).unwrap(), arguments)),
{
    source_statement_denotation(types, locals, statement, operations, denote(state, arguments));
    step_denotation(state, step, arguments);
}
}

}
