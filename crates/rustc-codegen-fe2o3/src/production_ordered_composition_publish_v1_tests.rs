//! Pure contract controls only. They do not manufacture a rustc/HIR source owner.
use super::text::{Coordinates, helper_name, render, splice};
use super::*;
use fe2o3_kernel_ir::{
    Gfx942ProgramBinaryOpcodeV1 as Op, Gfx942ProgramDestinationV1 as Dest,
    Gfx942ProgramInstructionV1 as Step, Gfx942ProgramRoleV1 as Role,
};

const NAME: &str = "__fe2o3_region_0123456789abcdef";
fn edit(steps: &[Step]) -> OrderedCompositionTypedEditV1 {
    OrderedCompositionTypedEditV1 {
        program: Gfx942U32ProgramV1::from_instructions(steps).unwrap(),
        registers: Gfx942OrderedProgramRegistersV1::new(8, 9, [10, 11, 12]).unwrap(),
    }
}
fn copied() -> OrderedCompositionTypedEditV1 {
    edit(&[Step::Move {
        destination: Dest::Output,
        source: Role::Input0,
    }])
}
fn source() -> (&'static str, Coordinates) {
    let source = "fn root(a: u32, b: u32, c: u32) {\n    let y = fe2o3_device::amdgpu_ordered_program! { gfx942_xnack_off_wave64; scratch(8); out(9); in(10) = a; in(11) = b; in(12) = c; mov(out,input0); };\n    consume(y);\n}\n";
    let start = source.find("fe2o3_device").unwrap();
    let end = source[start..].find("};").unwrap() + start + 1;
    (
        source,
        Coordinates {
            insertion: source.find('{').unwrap() + 1,
            selected: start..end,
            arguments: [
                source.find("a:").unwrap()..source.find("a:").unwrap() + 1,
                source.find("b:").unwrap()..source.find("b:").unwrap() + 1,
                source.find("c:").unwrap()..source.find("c:").unwrap() + 1,
            ],
        },
    )
}

#[test]
fn generated_name_grammar_is_closed_and_still_not_collision_authority() {
    helper_name(NAME).unwrap();
    for name in [
        "",
        "helper",
        "fn",
        "__fe2o3_region_0123456789abcde",
        "__fe2o3_region_0123456789abcdef0",
        "__fe2o3_region_0123456789abcdeF",
        "__fe2o3_region_0123456789abcd;f",
        "__fe2o3_region_0123456789abcd\nf",
    ] {
        assert!(helper_name(name).is_err(), "{name:?}");
    }
}

#[test]
fn every_typed_opcode_is_rendered_in_authored_order() {
    let chosen = edit(&[
        Step::Move {
            destination: Dest::Scratch,
            source: Role::Input0,
        },
        Step::Binary {
            opcode: Op::Add,
            destination: Dest::Output,
            left: Role::Scratch,
            right: Role::Input1,
        },
        Step::Binary {
            opcode: Op::Subtract,
            destination: Dest::Output,
            left: Role::Output,
            right: Role::Input2,
        },
        Step::Binary {
            opcode: Op::And,
            destination: Dest::Scratch,
            left: Role::Output,
            right: Role::Input0,
        },
        Step::Binary {
            opcode: Op::Or,
            destination: Dest::Output,
            left: Role::Scratch,
            right: Role::Input1,
        },
        Step::Binary {
            opcode: Op::Xor,
            destination: Dest::Output,
            left: Role::Output,
            right: Role::Input2,
        },
        Step::Move {
            destination: Dest::Output,
            source: Role::Output,
        },
    ]);
    let rendered = render(NAME, chosen).unwrap();
    let lines: Vec<_> = rendered
        .lines()
        .filter(|line| {
            ["mov(", "add(", "sub(", "and(", "or(", "xor("]
                .iter()
                .any(|p| line.trim_start().starts_with(p))
        })
        .map(str::trim)
        .collect();
    assert_eq!(
        lines,
        [
            "mov(scratch, input0);",
            "add(out, scratch, input1);",
            "sub(out, out, input2);",
            "and(scratch, out, input0);",
            "or(out, scratch, input1);",
            "xor(out, out, input2);",
            "mov(out, out);",
        ]
    );
    assert!(rendered.contains("scratch(8); out(9);"));
    assert!(rendered.contains("in(10) = a; in(11) = b; in(12) = c;"));
}

#[test]
fn maximum_program_and_repeated_dead_writes_are_not_folded() {
    let steps = [Step::Move {
        destination: Dest::Output,
        source: Role::Input0,
    }; 16];
    let rendered = render(NAME, edit(&steps)).unwrap();
    assert_eq!(rendered.matches("mov(out, input0);").count(), 16);
    assert!(rendered.len() < HELPER_CAP);
    assert_eq!(rendered.matches("#[inline(never)]").count(), 1);
}

#[test]
fn insertion_and_replacement_preserve_all_other_original_bytes() {
    let (original, coordinates) = source();
    let helper = render(NAME, copied()).unwrap();
    let candidate = splice(original, &coordinates, NAME, &helper).unwrap();
    let expected = format!(
        "{}{}{}{}(a, b, c){}",
        &original[..coordinates.insertion],
        helper,
        &original[coordinates.insertion..coordinates.selected.start],
        NAME,
        &original[coordinates.selected.end..]
    );
    assert_eq!(candidate, expected);
    assert_eq!(candidate.matches("consume(y)").count(), 1);
    assert_eq!(
        candidate
            .matches("fe2o3_device::amdgpu_ordered_program!")
            .count(),
        1
    );
}

#[test]
fn splice_refuses_unbound_or_non_body_coordinates() {
    for kind in 0..7 {
        let (original, mut c) = source();
        match kind {
            0 => c.insertion = 0,
            1 => c.insertion -= 1,
            2 => c.insertion = c.selected.start + 1,
            3 => c.selected.end = original.len() + 1,
            4 => c.selected.start = c.selected.end,
            5 => c.arguments[0] = 0..3,
            6 => c.selected.start += 1,
            _ => unreachable!(),
        }
        assert!(
            splice(original, &c, NAME, &render(NAME, copied()).unwrap()).is_err(),
            "{kind}"
        );
    }
}

#[test]
fn source_and_helper_byte_bounds_are_refusals_not_truncations() {
    let (original, c) = source();
    let oversized = format!("{original}{}", " ".repeat(SOURCE_CAP));
    assert!(splice(&oversized, &c, NAME, &render(NAME, copied()).unwrap()).is_err());
    assert!(splice(original, &c, NAME, &" ".repeat(HELPER_CAP + 1)).is_err());
}

#[test]
fn helper_edits_do_not_claim_equivalence_to_selected_program() {
    let original = render(NAME, copied()).unwrap();
    let edited = render(
        NAME,
        edit(&[Step::Move {
            destination: Dest::Output,
            source: Role::Input2,
        }]),
    )
    .unwrap();
    assert_ne!(original, edited);
    assert!(edited.contains("mov(out, input2);"));
    // Fresh source admission and checked continuation, not this string test,
    // decides whether a published candidate can proceed.
}

#[test]
fn post_attempt_failures_never_revert_to_not_attempted() {
    let early = Error::refused("source mismatch").after_attempt(false);
    assert_eq!(
        early.effect,
        OrderedCompositionSourcePublishEffectV1::NotAttempted
    );
    let after = Error::refused("directory durability").after_attempt(true);
    assert_eq!(
        after.effect,
        OrderedCompositionSourcePublishEffectV1::MayHaveCreatedCandidate
    );
    let accounting = Error::from(Resource::Accounting).after_attempt(true);
    assert_eq!(
        accounting.effect,
        OrderedCompositionSourcePublishEffectV1::MayHaveCreatedCandidate
    );
}

#[test]
fn receipt_is_inline_and_never_a_compiler_owner() {
    assert!(std::mem::size_of::<PublishedOrderedCompositionSourceV1>() <= 256);
}

#[test]
fn flat_literal_source_matches_the_original_typed_program_only() {
    let (original, coordinates) = source();
    text::require_flat_source(original, &coordinates, copied()).unwrap();
    let edited = edit(&[Step::Move {
        destination: Dest::Output,
        source: Role::Input2,
    }]);
    assert!(text::require_flat_source(original, &coordinates, edited).is_err());
}

#[test]
fn flat_publisher_refuses_compile_time_control_and_unreviewed_token_spelling() {
    let (original, _) = source();
    for replacement in [
        "init { mov(out,input0); } repeat(1) { mov(out,input0); }",
        "const_if(true) { mov(out,input0); } else { mov(out,input0); }",
        "mov(out,input0); /* comment */",
    ] {
        let changed = original.replace("mov(out,input0);", replacement);
        let start = changed.find("fe2o3_device").unwrap();
        let end = changed.rfind("};").unwrap() + 1;
        let (_, mut coordinates) = source();
        coordinates.selected = start..end;
        assert!(text::require_flat_source(&changed, &coordinates, copied()).is_err());
    }
}

#[test]
fn flat_token_comparison_permits_only_whitespace_changes() {
    let (original, mut coordinates) = source();
    let changed = original.replace("mov(out,input0);", "mov ( out , input0 ) ;");
    coordinates.selected.end += "mov ( out , input0 ) ;".len() - "mov(out,input0);".len();
    text::require_flat_source(&changed, &coordinates, copied()).unwrap();
}
