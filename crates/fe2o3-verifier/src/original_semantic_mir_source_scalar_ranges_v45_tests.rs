use super::*;

fn range_transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    failure_move: bool,
) {
    retained_transform_v44(types, functions, SemanticCheckedBinaryOpV1::Add);
    let old = functions.last_mut().unwrap();
    let pair = old.locals()[4].ty();
    let Shape::Tuple(fields) = types[pair.index() as usize].shape() else {
        panic!("original checked pair");
    };
    let field = |index: u32| {
        let ty = fields.fields()[index as usize];
        SemanticPlaceV1::new(
            SemanticLocalIdV1::from_index(4),
            vec![SemanticProjectionV1::new(SemanticProjectionKindV1::Field(index), ty).unwrap()],
            ty,
        )
        .unwrap()
    };
    let mut blocks = old.blocks().to_vec();
    let SemanticTerminatorKindV1::Assert {
        expected,
        message,
        target,
        unwind,
        ..
    } = blocks[1].terminator().kind()
    else {
        panic!("original overflow assertion")
    };
    let message = if failure_move {
        SemanticAssertMessageV1::Overflow {
            operation: SemanticBinaryOpV1::Add,
            left: SemanticOperandV1::Move(field(0)),
            right: SemanticOperandV1::Copy(
                SemanticPlaceV1::new(SemanticLocalIdV1::from_index(1), vec![], fields.fields()[0])
                    .unwrap(),
            ),
        }
    } else {
        message.clone()
    };
    blocks[1] = SemanticBasicBlockV1::new(
        blocks[1].identity(),
        blocks[1].source(),
        blocks[1].statements().to_vec(),
        SemanticTerminatorV1::new(
            blocks[1].terminator().source(),
            SemanticTerminatorKindV1::Assert {
                condition: SemanticOperandV1::Move(field(1)),
                expected: *expected,
                message,
                target: *target,
                unwind: *unwind,
            },
        ),
    )
    .unwrap();
    *old = SemanticFunctionDeclV1::new(
        old.identity(),
        old.role(),
        old.item_definition_identity(),
        old.monomorphization_identity(),
        old.generic_type_arguments_identity(),
        old.const_generic_arguments_identity(),
        old.source(),
        old.abi().clone(),
        old.locals().to_vec(),
        old.entry(),
        blocks,
    )
    .unwrap();
}

fn emitted_function<'a>(text: &'a str, name: &str) -> &'a str {
    let marker = format!("spec fn {name}(");
    assert_eq!(text.matches(&marker).count(), 1);
    text.split_once(&marker)
        .unwrap()
        .1
        .split("spec fn ")
        .next()
        .unwrap()
}

fn run_range_program(
    failure_move: bool,
    work: usize,
    storage: usize,
) -> (Result<()>, usize, usize, usize) {
    super::super::super::super::super::invocations::tests::run_source_transform(
        work,
        storage,
        |types, functions| range_transform(types, functions, failure_move),
        |plan, out| {
            super::super::super::super::source_function::tests::with_slots(
                plan,
                out,
                |slots, out| {
                    let mut program =
                        super::super::super::super::source_function::SourceByteProgram::derive(
                            plan, slots, out,
                        )?;
                    let paired = super::super::super::super::paired::PairedInvocations::derive(
                        plan,
                        &program,
                        fe2o3_kernel_ir::FormalIndexWidth::Bits64,
                        out,
                    )?;
                    slots.emit(out)?;
                    program.emit(out)?;
                    paired.emit(out)?;
                    for root in 0..2 {
                        for instance in 1..=2 {
                            assert!(slots.has_original_object(root, instance, 4, out)?);
                            let local = plan.instance(root, instance, out)?.locals.start + 4;
                            let condition = format!(
                                "InvocationSourceByteValueV36::Read {{ access: InvocationSourceByteAccessV36 {{ base: InvocationSourceByteBaseV36::ObjectLocal({local}int), offset: 4int, width: 1int, alignment: 1int }}, moved: true }}"
                            );
                            let payload = format!(
                                "InvocationSourceByteValueV36::Read {{ access: InvocationSourceByteAccessV36 {{ base: InvocationSourceByteBaseV36::ObjectLocal({local}int), offset: 0int, width: 4int, alignment: 4int }}, moved: true }}"
                            );
                            let finish = emitted_function(
                                &out.text,
                                &format!("invocation_source_micro_finish_{root}_{instance}_v36"),
                            );
                            assert_eq!(
                                finish.matches(&condition).count(),
                                2,
                                "evaluation plus retained original observation"
                            );
                            assert_eq!(
                                finish.matches(&payload).count(),
                                if failure_move { 2 } else { 0 }
                            );
                            if failure_move {
                                let branch = finish
                                    .find("let source = assertion_condition.source;")
                                    .unwrap();
                                assert!(finish.find(&payload).unwrap() > branch);
                                assert!(finish[..branch].contains(
                                    "invocation_source_byte_pc_v36(assertion_condition.source,"
                                ));
                            }
                            let events = emitted_function(
                                &out.text,
                                &format!("invocation_source_byte_event_{root}_{instance}_v36"),
                            );
                            let sibling = payload.replace("moved: true", "moved: false");
                            assert!(
                                events.contains(&sibling),
                                "the success path still reads the value sibling"
                            );
                        }
                    }
                    assert!(out.text.contains("invocation_observed_effect_related_v39"));
                    assert!(!out.text.contains("assume("));
                    Ok(())
                },
            )
        },
    )
}

#[test]
fn original_retained_scalar_moves_bind_source_reads_failure_order_and_actual_cuts() {
    for failure_move in [false, true] {
        run_range_program(failure_move, LIMIT, LIMIT).0.unwrap();
    }
}

#[test]
fn original_retained_scalar_range_program_has_exact_and_one_short_resources() {
    let measured = run_range_program(true, LIMIT, LIMIT);
    measured.0.unwrap();
    let exact = run_range_program(true, measured.1, measured.3);
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for (work, storage, work_short) in [
        (measured.1 - 1, measured.3, true),
        (measured.1, measured.3 - 1, false),
    ] {
        let refused = run_range_program(true, work, storage).0;
        let mut error: &(dyn std::error::Error + 'static) = refused.as_ref().unwrap_err();
        loop {
            if let Some(resource) = error.downcast_ref::<Resource>() {
                match resource {
                    Resource::Work(limit) if work_short => {
                        assert_eq!((limit.limit(), limit.actual()), (work, measured.1))
                    }
                    Resource::Storage(limit) if !work_short => {
                        assert_eq!((limit.limit(), limit.actual()), (storage, measured.3))
                    }
                    other => panic!("wrong range resource: {other:?}"),
                }
                break;
            }
            error = error
                .source()
                .unwrap_or_else(|| panic!("missing range resource: {refused:?}"));
        }
    }
}
