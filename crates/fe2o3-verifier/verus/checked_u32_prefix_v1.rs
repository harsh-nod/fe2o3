//! Shared argument-basis initialization and fold denotation.
//! No ABI discovery, MIR/KIR step normalization, machine-entry or authority proof.
use vstd::prelude::*;

include!("../src/gfx942_local_checked_u32_add_v1/source_prefix/fold_body.rs");
include!("../src/gfx942_local_checked_u32_add_v1/source_prefix/basis_body.rs");
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

}
