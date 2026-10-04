use super::*;
use fe2o3_lower_mir_kernel::ProductionSourceOwnedViewErrorV18 as SourceError;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 100_000_000;

fn message(kind: usize, word: SemanticTypeIdV1) -> SemanticAssertMessageV1 {
    let place =
        |local| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], word).unwrap();
    let left = SemanticOperandV1::Move(place(1));
    let right = SemanticOperandV1::Copy(place(2));
    match kind {
        0 => SemanticAssertMessageV1::BoundsCheck {
            length: left,
            index: right,
        },
        1 => SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Add,
            left,
            right,
        },
        2 => SemanticAssertMessageV1::DivisionByZero(left),
        3 => SemanticAssertMessageV1::RemainderByZero(left),
        4 => SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment: left,
            found_alignment: right,
        },
        5 => SemanticAssertMessageV1::NullPointerDereference,
        6 => SemanticAssertMessageV1::ResumedAfterReturn,
        7 => SemanticAssertMessageV1::ResumedAfterPanic,
        _ => unreachable!(),
    }
}

fn transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    expected: bool,
    kind: usize,
) {
    let word = SemanticTypeIdV1::from_index(0);
    let boolean = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([210; 32]),
        SemanticLayoutIdentityV1::from_sha256([211; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(1),
            1,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::integer(false, 8, 1),
                SemanticScalarValidityRangeV1::new(0, 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool),
    ));
    let helper = functions.last_mut().unwrap();
    let source = helper.source();
    let mut locals = helper.locals().to_vec();
    assert_eq!(locals.len(), 4);
    let condition_local = locals.len() as u32;
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([212; 32]),
        boolean,
        SemanticLocalRoleV1::Temporary,
        source,
    ));
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let mut statements = helper.blocks()[0].statements().to_vec();
    statements.push(SemanticStatementV1::new(
        source,
        SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
            place(condition_local, boolean),
            SemanticRvalueV1::new(
                boolean,
                SemanticRvalueKindV1::Binary {
                    operation: SemanticBinaryOpV1::Equal,
                    left: SemanticOperandV1::Copy(place(1, word)),
                    right: SemanticOperandV1::Copy(place(2, word)),
                },
            ),
        )),
    ));
    let blocks = vec![
        SemanticBasicBlockV1::new(
            helper.blocks()[0].identity(),
            source,
            statements,
            SemanticTerminatorV1::new(
                source,
                SemanticTerminatorKindV1::Assert {
                    condition: SemanticOperandV1::Move(place(condition_local, boolean)),
                    expected,
                    message: message(kind, word),
                    target: SemanticControlFlowEdgeV1::new(
                        SemanticEdgeRoleV1::AssertSuccess,
                        SemanticBlockIdV1::from_index(1),
                    ),
                    unwind: SemanticUnwindActionV1::Unreachable,
                },
            ),
        )
        .unwrap(),
        SemanticBasicBlockV1::new(
            SemanticBlockIdentityV1::from_sha256([213; 32]),
            source,
            vec![],
            SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
        )
        .unwrap(),
    ];
    *helper = SemanticFunctionDeclV1::new(
        helper.identity(),
        helper.role(),
        helper.item_definition_identity(),
        helper.monomorphization_identity(),
        helper.generic_type_arguments_identity(),
        helper.const_generic_arguments_identity(),
        source,
        helper.abi().clone(),
        locals,
        helper.entry(),
        blocks,
    )
    .unwrap();
}

fn run(
    expected: bool,
    kind: usize,
    work: usize,
    storage: usize,
    examine: impl FnOnce(&mut SourceByteProgram<'_, '_, '_>, &mut Writer<'_, '_>) -> Result<()>,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| transform(types, functions, expected, kind),
        |plan, out| {
            super::super::tests::with_slots(plan, out, |slots, out| {
                let mut program = SourceByteProgram::derive(plan, slots, out)?;
                examine(&mut program, out)
            })
        },
    )
}

#[test]
fn original_mir_assert_executes_both_polarities_and_failure_only_ordered_moves() {
    for expected in [false, true] {
        run(expected, 1, LIMIT, LIMIT, |program, out| {
            assert_eq!(
                program.functions.iter().filter(|row| row.is_some()).count(),
                6
            );
            for function in program
                .functions
                .iter()
                .flatten()
                .filter(|row| row.instance != 0)
            {
                let End::Assert(assertion) = &function.control[0].end else {
                    panic!("missing original assertion");
                };
                assert_eq!(assertion.expected, expected);
                assert_eq!(assertion.condition.scalar(), Some(ScalarV30::Bool));
                assert_eq!(assertion.failure.iter().flatten().count(), 2);
                let start = out.text.len();
                assertion.emit(function.root, function.instance, 0, out)?;
                let text = &out.text[start..];
                assert!(text.contains("if !assertion_condition.source.machine.valid || !(assertion_condition.value == MemoryValueV30::Scalar(0) || assertion_condition.value == MemoryValueV30::Scalar(1))"));
                assert!(!text.contains("matches!(assertion_condition.value"));
                assert_eq!(
                    text.matches(
                        "let assertion_condition = invocation_source_operand_evaluate_v36"
                    )
                    .count(),
                    1
                );
                let success = text
                    .find(&format!(
                        "else if assertion_condition.value == MemoryValueV30::Scalar({})",
                        u8::from(expected)
                    ))
                    .unwrap();
                let failed = text[success..].find("} else {").unwrap() + success;
                let first = text
                    .find("let assertion_failure_0 = invocation_source_value_evaluate_v42(source,")
                    .unwrap();
                let second = text
                    .find("let assertion_failure_1 = invocation_source_value_evaluate_v42(source,")
                    .unwrap();
                let trap = text
                    .find("source: invocation_source_byte_trap_v40(source)")
                    .unwrap();
                assert!(success < failed && failed < first && first < second && second < trap);
                assert!(!text[success..failed].contains("assertion_failure"));
                assert!(text.contains("let source = assertion_failure_0.source;"));
                assert!(text.contains("role: InvocationSourceOperandRoleV36::AssertMessage(0)"));
                assert!(
                    text.contains("before: assertion_before_1, after: assertion_failure_1.source")
                );
                assert!(!text.contains("assume("));
            }
            program.emit(out)
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_mir_assert_censuses_every_original_failure_message_without_extra_operands() {
    for kind in 0..8 {
        let count = match kind {
            0 | 1 | 4 => 2,
            2 | 3 => 1,
            _ => 0,
        };
        run(true, kind, LIMIT, LIMIT, |program, out| {
            for function in program
                .functions
                .iter()
                .flatten()
                .filter(|row| row.instance != 0)
            {
                let End::Assert(assertion) = &function.control[0].end else {
                    panic!("missing original assertion");
                };
                assert_eq!(assertion.failure.iter().flatten().count(), count);
                let start = out.text.len();
                assertion.emit(function.root, function.instance, 0, out)?;
                assert_eq!(
                    out.text[start..]
                        .matches("role: InvocationSourceOperandRoleV36::AssertMessage(")
                        .count(),
                    count
                );
            }
            Ok(())
        })
        .0
        .unwrap();
    }
}

#[test]
fn original_mir_assert_rejects_equal_cloned_original_and_wrong_block() {
    run(true, 1, LIMIT, LIMIT, |program, out| {
        let function = program
            .functions
            .iter()
            .flatten()
            .find(|row| row.instance == 1)
            .unwrap();
        let source = function
            .slots
            .correspondence(out)?
            .source(out.budget)?
            .source_semantic(out.budget)?;
        let original = source.functions().last().unwrap().blocks()[0]
            .terminator()
            .kind();
        let End::Assert(assertion) = &function.control[0].end else {
            panic!("missing original assertion");
        };
        assert!(
            SourceAssertControlV40::derive(&function.body, 0, original, assertion.success, out)
                .is_ok()
        );
        let equal_but_foreign = original.clone();
        assert!(
            SourceAssertControlV40::derive(
                &function.body,
                0,
                &equal_but_foreign,
                assertion.success,
                out
            )
            .is_err()
        );
        assert!(
            SourceAssertControlV40::derive(&function.body, 1, original, assertion.success, out)
                .is_err()
        );
        Ok(())
    })
    .0
    .unwrap();
}

#[test]
fn original_mir_assert_has_exact_and_one_short_whole_resources() {
    let execute = |work, storage| run(false, 1, work, storage, |program, out| program.emit(out));
    let (result, work, _, storage) = execute(LIMIT, LIMIT);
    result.unwrap();
    let exact = execute(work, storage);
    exact.0.unwrap();
    assert_eq!((exact.1, exact.3), (work, storage));
    for (w, s, is_work) in [(work - 1, storage, true), (work, storage - 1, false)] {
        let failure = execute(w, s);
        assert!(
            matches!((is_work, &failure.0),
            (true, Err(Error::Source(SourceError::Resource(Resource::Work(error))))) if error.limit() == w && error.actual() == work)
                || matches!((is_work, &failure.0),
                (false, Err(Error::Source(SourceError::Resource(Resource::Storage(error))))) if error.limit() == s && error.actual() == storage),
            "{:?}",
            failure.0
        );
        assert!(failure.1 <= w && failure.3 <= s);
    }
}

#[test]
fn original_mir_assert_headers_are_independent() {
    type Loop<'a> =
        std::iter::Enumerate<std::iter::Flatten<std::slice::Iter<'a, Option<TypedOperand>>>>;
    type Fields = (TypedOperand, [Option<TypedOperand>; 2], bool, usize);
    assert_eq!(size_of::<SourceAssertControlV40>(), size_of::<Fields>());
    let expected = size_of::<Fields>()
        + 2 * size_of::<Result<SourceAssertControlV40>>()
        + size_of::<(TypedOperand, [Option<TypedOperand>; 2])>()
        + 2 * size_of::<Result<(TypedOperand, [Option<TypedOperand>; 2])>>()
        + size_of::<Loop<'_>>()
        + size_of::<(
            &SourceByteBody<'_, '_, '_>,
            &Terminator,
            &mut Writer<'_, '_>,
            [usize; 7],
            bool,
        )>();
    assert_eq!(headers(), expected);
}

#[test]
fn original_mir_assert_trap_refusal_and_normal_return_remain_distinct() {
    let helper = SOURCE_FUNCTION_V36
        .split_once("spec fn invocation_source_byte_trap_v40")
        .unwrap()
        .1
        .split_once("spec fn ")
        .unwrap()
        .0;
    assert!(helper.contains("!source.machine.valid || source.machine.pc < 0"));
    assert!(helper.contains("invocation_source_byte_refused_v36(source)"));
    assert!(helper.contains("pc: -2"));
    assert!(!helper.contains("valid: true"));
    let observations = include_str!("original_semantic_mir_observed_effects_v39.vrs");
    assert!(observations.contains("result.source == invocation_source_byte_trap_v40(before)"));
    assert!(
        observations.contains("source.after == invocation_source_byte_trap_v40(source.before)")
    );
    assert!(
        observations.contains("MemoryOperationEffectV30::Trap, MemoryOperationEffectV30::Trap")
    );
    assert!(observations.contains("target.after == (MemoryStateV30 { pc: -2, ..target.before })"));
    let pair = include_str!("original_semantic_mir_invocation_paired_generate_v36.rs");
    assert!(pair.contains("source.machine.pc == -1 && target.pc == -1"));
    assert!(pair.contains("source.machine.pc == -2 && target.pc == -2"));
    assert!(!pair.contains("source.machine.pc < 0 {{ target.pc == -1"));
}
