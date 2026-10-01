//! Independent literal/typed controls only; these are not live rustc/HIR owners.
use super::super::{
    flat_source_matches, render, require_flat_source, require_supported_source, splice,
};
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrWorkBudgetV1 as Work, Gfx942OrderedProgramRegistersV1 as Registers,
    Gfx942ProgramDestinationV1 as Destination, Gfx942ProgramRoleV1 as Role,
};

const NAME: &str = "__fe2o3_region_0123456789abcdef";
const HEADER: &str = "fe2o3_device::amdgpu_ordered_program! { gfx942_xnack_off_wave64; scratch(8); out(9); in(10) = a; in(11) = b; in(12) = c;";
const INITIAL: Step = Step::Move {
    destination: Destination::Output,
    source: Role::Input0,
};
const ADD: Step = Step::Binary {
    opcode: Op::Add,
    destination: Destination::Output,
    left: Role::Output,
    right: Role::Input1,
};

fn actual(steps: &[Step]) -> OrderedCompositionTypedEditV1 {
    OrderedCompositionTypedEditV1 {
        program: Program::from_instructions(steps).unwrap(),
        registers: Registers::new(8, 9, [10, 11, 12]).unwrap(),
    }
}
fn repeated(repetitions: usize) -> OrderedCompositionTypedEditV1 {
    assert!((1..=15).contains(&repetitions));
    let mut steps = [ADD; 16];
    steps[0] = INITIAL;
    actual(&steps[..repetitions + 1])
}
fn source(body: &str) -> String {
    format!("{HEADER} {body} }}")
}
fn repeated_source(repetitions: usize) -> String {
    source(&format!(
        "init {{ mov(out,input0); }} repeat({repetitions}) {{ add(out,out,input1); }}"
    ))
}
fn embedded(selected: &str) -> (String, Coordinates) {
    let prefix = "fn root(a: u32, b: u32, c: u32) { let value = ";
    let original = format!("{prefix}{selected}; consume(value); }}\n");
    let args = ["a:", "b:", "c:"].map(|name| {
        let start = original.find(name).unwrap();
        start..start + 1
    });
    let coordinates = Coordinates {
        insertion: original.find('{').unwrap() + 1,
        selected: prefix.len()..prefix.len() + selected.len(),
        arguments: args,
    };
    (original, coordinates)
}

#[test]
fn independent_literal_one_two_fifteen_expansions_match_all_descriptors() {
    for repetitions in [1, 2, 15] {
        let selected = repeated_source(repetitions);
        let edit = repeated(repetitions);
        assert!(recognize(&selected, ["a", "b", "c"], edit).unwrap());
        let mut words = [0_u16; 16];
        // Independent known source descriptor words: mov(out,input0)=8,
        // add(out,out,input1)=1+8+(4<<4)+(1<<7)=201.
        words[0] = 8;
        words[1..=repetitions].fill(201);
        assert_eq!(*edit.program.descriptors(), words);
        assert_eq!(edit.program.count(), (repetitions + 1) as u8);
    }
}

#[test]
fn same_count_but_changed_late_descriptor_or_register_is_refused() {
    let source = repeated_source(2);
    let wrong = actual(&[
        INITIAL,
        ADD,
        Step::Move {
            destination: Destination::Output,
            source: Role::Input2,
        },
    ]);
    assert!(!recognize(&source, ["a", "b", "c"], wrong).unwrap());
    let mut wrong = repeated(2);
    wrong.registers = Registers::new(7, 9, [10, 11, 12]).unwrap();
    assert!(!recognize(&source, ["a", "b", "c"], wrong).unwrap());
    assert!(!recognize(&source, ["b", "a", "c"], repeated(2)).unwrap());
}

#[test]
fn changed_count_or_prefix_never_matches_only_a_final_value() {
    assert!(!recognize(&repeated_source(1), ["a", "b", "c"], repeated(2)).unwrap());
    let wrong_prefix = repeated_source(2).replace("mov(out,input0)", "mov(out,input2)");
    assert!(!recognize(&wrong_prefix, ["a", "b", "c"], repeated(2)).unwrap());
    // All of these moves return the same logical input but have distinct counts.
    let source = source("init { mov(out,input0); } repeat(2) { mov(out,input0); }");
    assert!(!recognize(&source, ["a", "b", "c"], actual(&[INITIAL, INITIAL])).unwrap());
    assert!(recognize(&source, ["a", "b", "c"], actual(&[INITIAL; 3])).unwrap());
}

#[test]
fn nonperiodic_decompositions_do_not_abort_a_later_valid_match() {
    let scratch = Step::Move {
        destination: Destination::Scratch,
        source: Role::Input0,
    };
    let out = Step::Move {
        destination: Destination::Output,
        source: Role::Scratch,
    };
    let edit = actual(&[scratch, scratch, out, scratch, out]);
    let source = source(
        "init { mov(scratch,input0); } repeat(2) { mov(scratch,input0); mov(out,scratch); }",
    );
    assert!(recognize(&source, ["a", "b", "c"], edit).unwrap());
    assert_eq!(expanded(&edit.program, 1, 1, 4).unwrap(), None);
}

#[test]
fn source_grammar_remains_canonical_literal_single_repeat_only() {
    let good = repeated_source(2);
    for count in [
        "0",
        "16",
        "02",
        "2usize",
        "2_usize",
        "0x2",
        "2+0",
        "COUNT",
        "usize::MAX",
        "-2",
    ] {
        let changed = good.replace("repeat(2)", &format!("repeat({count})"));
        assert!(
            !recognize(&changed, ["a", "b", "c"], repeated(2)).unwrap(),
            "{count}"
        );
    }
    for body in [
        "init {} repeat(2) { add(out,out,input1); }",
        "init { mov(out,input0); } repeat(2) {}",
        "init { mov(out,input0); } repeat(2) { repeat(1) { add(out,out,input1); } }",
        "init { mov(out,input0); } repeat(1) { add(out,out,input1); } repeat(1) { add(out,out,input1); }",
        "const_if(true) { mov(out,input0); add(out,out,input1); add(out,out,input1); } else { mov(out,input0); }",
        "init { mov(out,input0); } repeat(2) { add(out,out,input1); /* comment */ }",
        "init { mov(out,input0); } repeat(2) { add(out,out,input1); };",
    ] {
        assert!(
            !recognize(&source(body), ["a", "b", "c"], repeated(2)).unwrap(),
            "{body}"
        );
    }
}

#[test]
fn every_admitted_opcode_and_dead_self_write_is_preserved() {
    let steps = [
        INITIAL,
        Step::Binary {
            opcode: Op::Add,
            destination: Destination::Scratch,
            left: Role::Output,
            right: Role::Input1,
        },
        Step::Binary {
            opcode: Op::Subtract,
            destination: Destination::Output,
            left: Role::Scratch,
            right: Role::Input2,
        },
        Step::Binary {
            opcode: Op::And,
            destination: Destination::Scratch,
            left: Role::Output,
            right: Role::Input0,
        },
        Step::Binary {
            opcode: Op::Or,
            destination: Destination::Output,
            left: Role::Scratch,
            right: Role::Input1,
        },
        Step::Binary {
            opcode: Op::Xor,
            destination: Destination::Output,
            left: Role::Output,
            right: Role::Input2,
        },
        Step::Move {
            destination: Destination::Output,
            source: Role::Output,
        },
    ];
    let body = "init { mov(out,input0); } repeat(1) { add(scratch,out,input1); sub(out,scratch,input2); and(scratch,out,input0); or(out,scratch,input1); xor(out,out,input2); mov(out,out); }";
    assert!(recognize(&source(body), ["a", "b", "c"], actual(&steps)).unwrap());
}

#[test]
fn ascii_whitespace_allowed_but_aliases_wrappers_and_unicode_are_not() {
    let source = repeated_source(2);
    assert!(
        recognize(
            &source.replace("repeat(2)", "repeat \n ( 2 )"),
            ["a", "b", "c"],
            repeated(2)
        )
        .unwrap()
    );
    for changed in [
        source.replace("fe2o3_device::", ""),
        source.replace("fe2o3_device::", "alias::"),
        source.replace("repeat(2)", "repeat(\u{a0}2)"),
        format!("wrapper!{{{source}}}"),
        format!("{source}{}", repeated_source(2)),
    ] {
        assert!(!recognize(&changed, ["a", "b", "c"], repeated(2)).unwrap());
    }
}

#[test]
fn fixed_source_and_compact_bounds_refuse_before_truncating() {
    let mut bytes = [0; HELPER_CAP];
    assert_eq!(
        compact(&"x".repeat(HELPER_CAP), &mut bytes).unwrap(),
        HELPER_CAP
    );
    assert!(compact(&"x".repeat(HELPER_CAP + 1), &mut bytes).is_err());
    let source = repeated_source(2);
    let exact = format!("{}{}", " ".repeat(SOURCE_CAP - source.len()), source);
    assert!(recognize(&exact, ["a", "b", "c"], repeated(2)).unwrap());
    assert!(recognize(&format!(" {exact}"), ["a", "b", "c"], repeated(2)).is_err());
    assert!(WORK_BOUND <= MATCH_WORK && SCRATCH_BOUND <= MATCH_SCRATCH);
}

#[test]
fn decomposition_bound_is_exact_and_one_step_has_no_repeat_form() {
    let mut maximum = 0;
    for count in 2..=16 {
        let candidates: usize = (1..=15).map(|r| (count - 1) / r).sum();
        assert!(candidates <= MAX_CANDIDATES);
        maximum = maximum.max(candidates);
    }
    assert_eq!(maximum, 45);
    assert!(!recognize(&repeated_source(1), ["a", "b", "c"], actual(&[INITIAL])).unwrap());
    for (initial, block_length, repetitions) in [
        (0, 1, 2),
        (1, 0, 2),
        (1, 1, 0),
        (1, 1, 16),
        (usize::MAX, 1, 1),
    ] {
        assert!(expanded(&repeated(2).program, initial, block_length, repetitions).is_err());
    }
}

#[test]
fn flat_path_preserves_zero_extra_work_and_scratch() {
    let (source, coordinates) = embedded(&source("mov(out,input0);"));
    let mut work = Work::new(0);
    let mut budget = Budget::new(&mut work, 17);
    budget.reserve_storage(17).unwrap();
    assert!(flat_source_matches(&source, &coordinates, actual(&[INITIAL])).unwrap());
    require_flat_source(&source, &coordinates, actual(&[INITIAL])).unwrap();
    require_supported_source(&source, &coordinates, actual(&[INITIAL]), &mut budget).unwrap();
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, 17, 17)
    );
}

#[test]
fn exact_same_ledger_work_and_scratch_succeed_then_release_only_scratch() {
    let (source, coordinates) = embedded(&repeated_source(2));
    let mut work = Work::new(MATCH_WORK + 7);
    let mut budget = Budget::new(&mut work, MATCH_SCRATCH + 19);
    budget.charge_work(7).unwrap();
    budget.reserve_storage(19).unwrap();
    let identity = budget.work_ledger_identity_v1();
    require_supported_source(&source, &coordinates, repeated(2), &mut budget).unwrap();
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (MATCH_WORK + 7, 19, MATCH_SCRATCH + 19)
    );
    assert!(budget.work_ledger_identity_v1() == identity);
    assert_eq!(budget.failed_work(), None);
    assert_eq!(budget.failed_storage(), None);
}

#[test]
fn one_short_work_refuses_before_repeat_scratch_or_content_refusal() {
    // Deliberately malformed range would fail inside the matcher if prepayment
    // were moved too late. Call the same production metered repeat entry.
    let (_, mut coordinates) = embedded(&repeated_source(2));
    coordinates.selected = 0..usize::MAX;
    let mut work = Work::new(MATCH_WORK - 1);
    let mut budget = Budget::new(&mut work, MATCH_SCRATCH + 23);
    budget.reserve_storage(23).unwrap();
    let error = require("", &coordinates, repeated(2), &mut budget).unwrap_err();
    assert!(matches!(
        error.reason,
        super::super::super::PublishReasonV1::Resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Work(_)
        )
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (0, 23, 23)
    );
    assert_eq!(budget.failed_work(), Some(MATCH_WORK));
}

#[test]
fn one_short_scratch_refuses_before_repeat_content_and_keeps_work() {
    let (_, mut coordinates) = embedded(&repeated_source(2));
    coordinates.selected = 0..usize::MAX;
    let mut work = Work::new(MATCH_WORK);
    let mut budget = Budget::new(&mut work, MATCH_SCRATCH + 29 - 1);
    budget.reserve_storage(29).unwrap();
    let error = require("", &coordinates, repeated(2), &mut budget).unwrap_err();
    assert!(matches!(
        error.reason,
        super::super::super::PublishReasonV1::Resource(
            fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceErrorV1::Storage(_)
        )
    ));
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (MATCH_WORK, 29, 29)
    );
    assert_eq!(budget.failed_storage(), Some(MATCH_SCRATCH + 29));
}

#[test]
fn semantic_refusal_retains_work_peak_and_original_floor() {
    let (source, coordinates) = embedded(&repeated_source(2));
    let mut work = Work::new(MATCH_WORK);
    let mut budget = Budget::new(&mut work, MATCH_SCRATCH + 31);
    budget.reserve_storage(31).unwrap();
    assert!(require_supported_source(&source, &coordinates, repeated(1), &mut budget).is_err());
    assert_eq!(
        (budget.work(), budget.storage(), budget.peak_storage()),
        (MATCH_WORK, 31, MATCH_SCRATCH + 31)
    );
}

#[test]
fn supported_repeat_materializes_to_flat_helper_and_preserves_other_bytes() {
    let (source, coordinates) = embedded(&repeated_source(2));
    let chosen = repeated(2);
    let mut work = Work::new(MATCH_WORK);
    let mut budget = Budget::new(&mut work, MATCH_SCRATCH);
    require_supported_source(&source, &coordinates, chosen, &mut budget).unwrap();
    let helper = render(NAME, chosen).unwrap();
    assert!(!helper.contains("repeat(") && !helper.contains("init {"));
    assert_eq!(helper.matches("mov(out, input0);").count(), 1);
    assert_eq!(helper.matches("add(out, out, input1);").count(), 2);
    let candidate = splice(&source, &coordinates, NAME, &helper).unwrap();
    assert_eq!(
        candidate,
        format!(
            "{}{}{}{}(a, b, c){}",
            &source[..coordinates.insertion],
            helper,
            &source[coordinates.insertion..coordinates.selected.start],
            NAME,
            &source[coordinates.selected.end..]
        )
    );
    assert!(!candidate.contains("repeat("));
}
