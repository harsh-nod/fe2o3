use super::*;

fn run(
    owner: &ProductionSemanticKirOwnerV1,
    spans: &[SemanticKirStatementOperationSpanV1],
    replacement: Option<&[SemanticStatementV1]>,
) -> Result<(Vec<PrefixStep>, u32), CheckedU32PrefixErrorV1> {
    let capture = owner.checked_u32_add_capture_v1().unwrap();
    let source = owner.semantic().semantic();
    let function = &source.functions()[capture.request().function().index() as usize];
    let block = &function.blocks()[capture.request().block().index() as usize];
    let prefix =
        replacement.unwrap_or(&block.statements()[..=capture.request().statement() as usize]);
    assemble::source_prefix(source, function, prefix, spans, capture, capture.block())
}

#[test]
fn actual_span_roster_permutations_preserve_exact_source_order() {
    let owner = owner(
        vec![
            assign(3, copy(1)),
            nop(),
            assign(4, constant(29)),
            assign(3, copy(4)),
        ],
        3,
        17,
        2,
        6,
    );
    let spans = owner.correspondence().statement_operation_spans();
    let expected = run(&owner, spans, None).unwrap();
    assert_eq!(expected.1, 3);
    assert_eq!(
        expected.0,
        [
            PrefixStep {
                destination: 3,
                input: PrefixInput::Cell(1)
            },
            PrefixStep {
                destination: 4,
                input: PrefixInput::Constant(29)
            },
            PrefixStep {
                destination: 3,
                input: PrefixInput::Cell(4)
            },
        ]
    );
    for shift in 0..spans.len() {
        let mut changed = spans.to_vec();
        changed.rotate_left(shift);
        assert_eq!(run(&owner, &changed, None).unwrap(), expected);
        changed.reverse();
        assert_eq!(run(&owner, &changed, None).unwrap(), expected);
    }
}

#[test]
fn duplicate_and_missing_original_rows_reject_at_every_ordinal() {
    let owner = owner(
        vec![nop(), assign(2, copy(1)), assign(3, constant(9))],
        3,
        17,
        1,
        5,
    );
    let spans = owner.correspondence().statement_operation_spans();
    for index in 0..spans.len() {
        let mut missing = spans.to_vec();
        missing.remove(index);
        assert_eq!(
            run(&owner, &missing, None),
            Err(CheckedU32PrefixErrorV1::Span)
        );
        let mut duplicate = spans.to_vec();
        duplicate.push(spans[index]);
        assert_eq!(
            run(&owner, &duplicate, None),
            Err(CheckedU32PrefixErrorV1::Span)
        );
    }
}

#[test]
fn later_rows_are_excluded_and_cross_profile_operation_counts_are_checked() {
    let copies = owner(vec![assign(2, copy(1))], 2, 17, 1, 4);
    let constants = owner(vec![assign(2, constant(9))], 2, 17, 1, 4);
    let later = owner(vec![nop(), nop(), nop()], 1, 17, 1, 3);
    let spans = copies.correspondence().statement_operation_spans();
    let expected = run(&copies, spans, None).unwrap();
    let mut extra = spans.to_vec();
    extra.push(
        *later
            .correspondence()
            .statement_operation_spans()
            .last()
            .unwrap(),
    );
    assert_eq!(run(&copies, &extra, None).unwrap(), expected);
    assert_eq!(
        run(
            &copies,
            constants.correspondence().statement_operation_spans(),
            None
        ),
        Err(CheckedU32PrefixErrorV1::Source)
    );
    let mut gap = spans.to_vec();
    gap[1] = constants.correspondence().statement_operation_spans()[1];
    assert_eq!(run(&copies, &gap, None), Err(CheckedU32PrefixErrorV1::Span));
}

#[test]
fn assembly_capacity_and_terminal_ast_validation_are_distinct_boundaries() {
    let owner = owner(vec![nop(); MAX_STATEMENTS - 1], 1, 17, 1, 3);
    let spans = owner.correspondence().statement_operation_spans();
    assert_eq!(run(&owner, spans, None).unwrap(), (vec![], 2));
    assert_eq!(
        run(&owner, spans, Some(&[])),
        Err(CheckedU32PrefixErrorV1::Capacity)
    );
    let over = vec![nop(); MAX_STATEMENTS + 1];
    assert_eq!(
        run(&owner, spans, Some(&over)),
        Err(CheckedU32PrefixErrorV1::Capacity)
    );
    // This helper checks the terminal span, not the terminal AST expression.
    let changed_terminal = vec![nop(); MAX_STATEMENTS];
    assert_eq!(
        run(&owner, spans, Some(&changed_terminal)).unwrap(),
        (vec![], 2)
    );
}
