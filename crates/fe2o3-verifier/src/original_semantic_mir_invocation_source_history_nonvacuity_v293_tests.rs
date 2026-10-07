//! Unexecuted test-only emission of constructed semantic witnesses.
//! The generated Verus obligations, not these Rust assertions, establish behavior.
use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBinaryOpV1 as Binary, SemanticOperandV1 as Operand, SemanticRvalueKindV1 as Rvalue,
    SemanticStatementKindV1 as Statement,
};
use std::fmt::Write as _;

const LIMIT: usize = 512 * 1024 * 1024;
const TEMPLATE: &str = include_str!("nonvacuity.vrs.in");

fn scalar_declaration(
    text: &str,
    root: usize,
    instance: usize,
    block: usize,
    statement: usize,
) -> &str {
    let marker =
        format!("spec fn invocation_source_scalar_{root}_{instance}_{block}_{statement}_v36(");
    assert_eq!(text.matches(&marker).count(), 1);
    let rest = &text[text.find(&marker).unwrap()..];
    let end = ["\nspec fn ", "\nproof fn "]
        .into_iter()
        .filter_map(|next| rest.find(next))
        .min()
        .unwrap_or(rest.len());
    &rest[..end]
}

struct Witness {
    root: usize,
    instance: usize,
    block: usize,
    pc: usize,
    locals_end: usize,
    owners: Vec<u32>,
    inputs: Vec<[usize; 2]>,
    destinations: Vec<usize>,
}

// This is independent test scaffolding, not a production admission route.
// Its Vec/String storage is not claimed as a new metered production helper.
fn witnesses(
    plan: &InvocationPlan<'_, '_>,
    program: &SourceByteProgram<'_, '_, '_>,
    out: &mut Writer<'_, '_>,
) -> Result<Vec<Witness>> {
    let source = plan.source(out)?;
    assert!(std::ptr::eq(
        source,
        program.slots.correspondence(out)?.source(out.budget)?
    ));
    let semantic = source.source_semantic(out.budget)?;
    let mut selected = Vec::new();
    for function in program.functions.iter().flatten() {
        let row = plan.instance(function.root, function.instance, out)?;
        assert!(row.active);
        assert_eq!(row.blocks, function.blocks);
        let original = &semantic.functions()[row.function.index() as usize];
        assert_eq!(original.blocks().len(), function.control.len());
        assert_eq!(original.locals().len(), row.locals.len());
        for (block, control) in function.control.iter().enumerate() {
            if control.statements == 0 || matches!(control.end, End::Unreachable) {
                continue;
            }
            let statements = original.blocks()[block].statements();
            assert_eq!(statements.len(), control.statements);
            let mut inputs = Vec::new();
            let mut destinations = Vec::new();
            for (ordinal, statement) in statements.iter().enumerate() {
                let Statement::Assign(assignment) = statement.kind() else {
                    break;
                };
                let Rvalue::Binary {
                    operation: Binary::BitXor | Binary::BitOr,
                    left,
                    right,
                } = assignment.value().kind()
                else {
                    break;
                };
                let (Operand::Copy(left), Operand::Copy(right)) = (left, right) else {
                    break;
                };
                if function.body.event_at(block, ordinal, out)? != Event::Scalar {
                    break;
                }
                let places = [left, right, assignment.destination()];
                if places.iter().any(|place| !place.projections().is_empty()) {
                    break;
                }
                let mut globals = [0; 3];
                for (index, place) in places.into_iter().enumerate() {
                    let local = place.local().index() as usize;
                    assert_eq!(original.locals()[local].ty(), place.ty());
                    assert_eq!(
                        ScalarV30::from_source(semantic.types(), place.ty())?,
                        ScalarV30::Integer {
                            width: 32,
                            signed: false
                        }
                    );
                    assert_eq!(
                        semantic.types()[place.ty().index() as usize]
                            .layout()
                            .size_bytes(),
                        Some(4)
                    );
                    globals[index] = row.locals.start.checked_add(local).unwrap();
                    assert!(globals[index] < row.locals.end);
                    assert!(
                        program
                            .slots
                            .legacy_descriptor_by_source(
                                function.root,
                                function.instance,
                                place.local().index(),
                                "history-nonvacuity-scalar",
                                out
                            )?
                            .is_none()
                    );
                }
                inputs.push([globals[0], globals[1]]);
                destinations.push(globals[2]);
            }
            if inputs.len() != statements.len() {
                continue;
            }
            assert!(!inputs.is_empty());
            let mut owners = Vec::new();
            let mut current = function.instance;
            loop {
                let ancestor = plan.instance(function.root, current, out)?;
                assert!(ancestor.active);
                owners.push(ancestor.function.index());
                match ancestor.incoming {
                    Some((parent, _)) => {
                        assert!(parent < current);
                        current = parent;
                    }
                    None => {
                        assert_eq!(current, 0);
                        break;
                    }
                }
            }
            owners.reverse();
            selected.push(Witness {
                root: function.root,
                instance: function.instance,
                block,
                pc: row.blocks.start.checked_add(block).unwrap(),
                locals_end: row.locals.end,
                owners,
                inputs,
                destinations,
            });
        }
    }
    assert!(
        !selected.is_empty(),
        "the actual fixture must retain a scalar witness"
    );
    for root in 0..source.root_count(out.budget)? {
        assert!(selected.iter().any(|row| row.root == root));
    }
    assert!(
        selected
            .iter()
            .any(|row| row.owners.len() > 1 && row.pc > row.block)
    );
    Ok(selected)
}

fn render(row: &Witness) -> String {
    let (r, i, b, n) = (row.root, row.instance, row.block, row.locals_end);
    let count = row.inputs.len();
    assert_eq!(count, row.destinations.len());
    let mut frames = format!(
        "    let frames = byte_root_frame_with_execution_v37({}, execution);",
        row.owners[0]
    );
    for owner in &row.owners[1..] {
        writeln!(
            frames,
            "\n    let frames = byte_enter_frame_v30(frames, {owner});"
        )
        .unwrap();
    }
    let mut facts = format!(
        "    assert(source.machine.frames.active.len() == {});\n",
        row.owners.len()
    );
    for (depth, owner) in row.owners.iter().enumerate() {
        writeln!(
            facts,
            "    assert(source.machine.frames.active[{depth}].owner == {owner});"
        )
        .unwrap();
        writeln!(
            facts,
            "    assert(source.machine.frames.active[{depth}].invocation == {depth});"
        )
        .unwrap();
    }
    let mut steps = String::new();
    for (statement, [left, right]) in row.inputs.iter().copied().enumerate() {
        let next = statement + 1;
        let destination = row.destinations[statement];
        writeln!(
            steps,
            "    assert(invocation_source_active_{r}_{i}_v36(c{statement}.source));"
        )
        .unwrap();
        for local in [left, right] {
            writeln!(steps, "    assert(c{statement}.source.machine.values[{local}] == MemoryValueV30::Scalar(0));").unwrap();
            writeln!(steps, "    assert(invocation_source_byte_value_typed_v36(c{statement}.source.machine.values[{local}], 32));").unwrap();
        }
        writeln!(
            steps,
            "    reveal(invocation_source_scalar_{r}_{i}_{b}_{statement}_v36);"
        )
        .unwrap();
        writeln!(
            steps,
            "    let c{next} = invocation_source_micro_step_{r}_{i}_v36(c{statement}, true);"
        )
        .unwrap();
        writeln!(steps, "    assert(c{next}.source.machine.valid);").unwrap();
        writeln!(
            steps,
            "    assert(c{next}.source.machine.pc == {});",
            row.pc
        )
        .unwrap();
        writeln!(steps, "    assert(c{next}.source.machine.values =~= Seq::new({n}, |local: int| MemoryValueV30::Scalar(0)));").unwrap();
        writeln!(
            steps,
            "    assert(c{next}.source.machine.values[{destination}] == MemoryValueV30::Scalar(0));"
        )
        .unwrap();
        writeln!(
            steps,
            "    assert(invocation_source_byte_state_well_formed_v36(c{next}.source));"
        )
        .unwrap();
        writeln!(
            steps,
            "    assert(invocation_source_active_{r}_{i}_v36(c{next}.source));"
        )
        .unwrap();
        writeln!(
            steps,
            "    assert(c{next}.next_statement == {next} && c{next}.observations.len() == {next});"
        )
        .unwrap();
        writeln!(
            steps,
            "    assert(c{next}.observations[{statement}].before == c{statement}.source);"
        )
        .unwrap();
        writeln!(
            steps,
            "    assert(c{next}.observations[{statement}].after == c{next}.source);"
        )
        .unwrap();
        writeln!(
            steps,
            "    invocation_source_micro_run_composes_{r}_{i}_v292(start, {statement}, 1, true);"
        )
        .unwrap();
        writeln!(
            steps,
            "    reveal_with_fuel(invocation_source_micro_run_{r}_{i}_v36, 2);"
        )
        .unwrap();
        writeln!(
            steps,
            "    assert(invocation_source_micro_run_{r}_{i}_v36(start, {next}, true) == c{next});"
        )
        .unwrap();
    }
    let mut text = TEMPLATE.to_owned();
    for (key, value) in [
        ("R", r.to_string()),
        ("I", i.to_string()),
        ("B", b.to_string()),
        ("PC", row.pc.to_string()),
        ("N", n.to_string()),
        ("COUNT", count.to_string()),
        ("LAST", (count - 1).to_string()),
        ("COUNT_PLUS_ONE", (count + 1).to_string()),
        ("FIRST_INPUT", row.inputs[0][0].to_string()),
        ("FRAME_BINDINGS", frames),
        ("FRAME_FACTS", facts),
        ("ACTUAL_PREFIX_STEPS", steps),
    ] {
        let marker = format!("@@{key}@@");
        assert!(text.contains(&marker));
        text = text.replace(&marker, &value);
    }
    assert!(!text.contains("@@"));
    for prohibited in [
        "requires",
        "assume(",
        "admit(",
        "external_body",
        "unimplemented",
        "axiom",
    ] {
        assert!(!text.contains(prohibited), "{prohibited}");
    }
    text
}

fn emitted(examine: impl FnOnce(&[Witness], &str, &str)) {
    super::super::super::invocations::tests::run_variant(LIMIT, LIMIT, false, |plan, out| {
        super::tests::with_slots(plan, out, |slots, out| {
            let mut program = SourceByteProgram::derive(plan, slots, out)?;
            let coordinates = witnesses(plan, &program, out)?;
            program.emit(out)?;
            let original = out.text.clone();
            for row in &coordinates {
                assert!(
                    original.contains(&format!(
                        "proof fn invocation_source_micro_run_history_{}_{}_v293(",
                        row.root, row.instance
                    )),
                    "requires the separate history candidate, not the current production emitter"
                );
                write!(out, "{}", render(row)).map_err(|_| out.error())?;
            }
            examine(&coordinates, &original, &out.text);
            super::super::support_closure::retain_referenced(out)?;
            for row in &coordinates {
                assert!(out.text.contains(&render(row)));
            }
            Ok(())
        })
    })
    .0
    .unwrap();
}

#[test]
fn source_micro_history_witnesses_use_authenticated_scalar_coordinates_and_owner_chains() {
    emitted(|rows, original, combined| {
        assert!(
            combined.starts_with(original),
            "no original interpreter text is rewritten"
        );
        for row in rows {
            let (r, i, b) = (row.root, row.instance, row.block);
            for statement in 0..row.inputs.len() {
                assert!(original.contains(&format!(
                    "spec fn invocation_source_scalar_{r}_{i}_{b}_{statement}_v36("
                )));
                assert!(original.contains(&format!(
                    "if cursor.source.machine.pc == {} && cursor.next_statement == {statement}",
                    row.pc
                )));
                let scalar = scalar_declaration(original, r, i, b, statement);
                for global in row.inputs[statement] {
                    assert!(scalar.contains(&format!(
                        "!invocation_source_byte_value_typed_v36(n.machine.values[{global}], 32)"
                    )));
                }
            }
            assert!(combined.contains(&format!(
                "proof fn history_witness_initial_active_{r}_{i}_{b}_v293("
            )));
        }
    });
}

#[test]
fn source_micro_history_witnesses_separate_recorded_scalar_failure_from_unrecorded_refusal() {
    emitted(|rows, _, combined| {
        for row in rows {
            let text = render(row);
            assert_eq!(text.matches("proof fn history_witness_").count(), 4);
            assert!(text.contains("one.observations.len() == 1"));
            assert!(text.contains("one != invocation_source_micro_refused_v36(start)"));
            assert!(text.contains("refused.observations == end.observations"));
            assert!(text.contains(&format!(
                "refused.observations[{}].after != refused.source",
                row.inputs.len() - 1
            )));
            assert!(combined.contains(&text));
        }
    });
}

#[path = "original_semantic_mir_invocation_source_history_export_v293_tests.rs"]
mod complete_export_tests;
