use super::*;
mod inline_fixture {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/support/helper_inline_singleton_semantic_fixture_v1.rs"
    ));
}
use inline_fixture::{Fixture, KINDS};

fn expected(
    kind: SemanticGfx942InlineInstructionV30,
    left: ProductionSemanticExpressionV2,
    right: ProductionSemanticExpressionV2,
) -> ProductionSemanticExpressionV2 {
    use SemanticGfx942InlineInstructionV30 as I;
    let operation = match kind {
        I::VMovB32 => return left,
        I::VAddU32 => ProductionSemanticBinaryOpV2::Add,
        I::VSubU32 => ProductionSemanticBinaryOpV2::Subtract,
        I::VAndB32 => ProductionSemanticBinaryOpV2::BitAnd,
        I::VOrB32 => ProductionSemanticBinaryOpV2::BitOr,
        I::VXorB32 => ProductionSemanticBinaryOpV2::BitXor,
    };
    ProductionSemanticExpressionV2::Binary {
        operation,
        scalar: ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        },
        overflow: ProductionOverflowContractV2::Wrapping,
        lhs: Box::new(left),
        rhs: Box::new(right),
    }
}
fn root_expression(
    input: &AdmittedInertSemanticMirV1,
    expected: Option<&ProductionSemanticExpressionV2>,
) -> Result<(), Error> {
    let (result, floor, _, _, _) = run(1_000_000, 16 << 20, |meter, _| {
        with_source_helper_values(input, 0, meter, |context, meter| {
            let function = &input.functions()[0];
            let mut resolver = GpuSemanticExpressionResolverV2::new(input.types(), function)
                .map_err(|_| "resolver")?;
            resolver.helper_semantic = Some(input);
            resolver.helper_values = Some(context);
            resolver.helper_meter = Some(&mut *meter);
            let mut resolver = resolver
                .with_scalar_callables_v1(input.callables())
                .map_err(|_| "callables")?;
            let result = resolver.resolve_store_v2(
                function.blocks()[1].statements()[0].kind(),
                ScalarAssignmentSiteV1 {
                    block: 1,
                    statement: 0,
                },
            );
            let result = result.map(|expression| {
                if let Some(expected) = expected {
                    assert_eq!(&expression, expected);
                }
                drop(expression);
            });
            let bytes = resolver.helper_reserved;
            drop(resolver);
            meter.release(bytes)?;
            result
        })
    });
    assert_eq!(floor, 4096);
    result
}
fn helper_call(input: &Fixture) -> &SemanticDirectCallV1 {
    let SemanticTerminatorKindV1::Call(call) = input.functions[1].blocks()[0].terminator().kind()
    else {
        panic!("call")
    };
    call
}
fn replace_helper_call(input: &mut Fixture, call: SemanticDirectCallV1) {
    let helper = &input.functions[1];
    let blocks = vec![
        inline_fixture::block(61, vec![], SemanticTerminatorKindV1::Call(call)),
        helper.blocks()[1].clone(),
    ];
    input.functions[1] = inline_fixture::replace_body(helper, helper.locals().to_vec(), blocks);
}
fn replacement_call(
    original: &SemanticDirectCallV1,
    arguments: Vec<SemanticOperandV1>,
) -> SemanticDirectCallV1 {
    SemanticDirectCallV1::new_callable(
        original.callee(),
        arguments,
        original.destination().cloned(),
        original.unwind(),
    )
    .unwrap()
    .with_inline_assembly_source_v30(original.inline_assembly_source_v30().unwrap())
}
fn derive_refuses(input: &AdmittedInertSemanticMirV1) {
    let (result, floor, _, _, _) = run(1_000_000, 16 << 20, |meter, _| {
        match source::derive(input, 1, &[], meter) {
            Err(_) => Ok(()),
            Ok(template) => {
                template.destroy(meter)?;
                Err("unexpected helper recipe")
            }
        }
    });
    result.unwrap();
    assert_eq!(floor, 4096);
}

#[test]
fn all_six_singleton_helper_templates_keep_exact_ordered_wrapping_expressions() {
    for kind in KINDS {
        let input = Fixture::new(kind).admit();
        for (left, right) in [(u32::MAX, 1), (0, 1), (0xaaaaaaaa, 0x55555555), (7, 11)] {
            let (result, floor, _, _, _) = run(1_000_000, 16 << 20, |meter, _| {
                let template = source::derive(&input, 1, &[], meter)?;
                let (expression, bytes) = template
                    .instantiate(&[constant(left as u64), constant(right as u64)], meter)?;
                assert_eq!(
                    expression,
                    expected(kind, constant(left as u64), constant(right as u64))
                );
                drop(expression);
                meter.release(bytes)?;
                template.destroy(meter)
            });
            result.unwrap();
            assert_eq!(floor, 4096);
        }
    }
}

#[test]
fn actual_root_field_zero_uses_source_owned_helper_recipe_not_scalar_inline_roster() {
    let scalar = ProductionSemanticScalarTypeV2::Integer {
        signed: false,
        bits: 32,
    };
    let base = fe2o3_pliron::PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2;
    for kind in KINDS {
        let input = Fixture::new(kind).admit();
        root_expression(
            &input,
            Some(&expected(
                kind,
                ProductionSemanticExpressionV2::Symbol {
                    symbol: base,
                    scalar,
                },
                ProductionSemanticExpressionV2::Symbol {
                    symbol: base + 1,
                    scalar,
                },
            )),
        )
        .unwrap();
    }
}

#[test]
fn exact_singleton_field_write_and_whole_tuple_move_are_supported() {
    let mut input = Fixture::new(SemanticGfx942InlineInstructionV30::VOrB32);
    let helper = &input.functions[1];
    let mut locals = helper.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([65; 32]),
        inline_fixture::SINGLETON,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let statements = vec![
        inline_fixture::statement(
            inline_fixture::component(4),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(inline_fixture::word(3))),
        ),
        inline_fixture::statement(
            inline_fixture::tuple(0),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(inline_fixture::tuple(4))),
        ),
    ];
    input.functions[1] = inline_fixture::replace_body(
        helper,
        locals,
        vec![
            helper.blocks()[0].clone(),
            inline_fixture::block(62, statements, SemanticTerminatorKindV1::Return),
        ],
    );
    assert!(root_expression(&input.admit(), None).is_ok());
}

#[test]
fn edited_constant_operand_changes_actual_template_without_erasing_call_arguments() {
    let mut expressions = Vec::new();
    for bits in [256u32, 512] {
        let mut input = Fixture::new(SemanticGfx942InlineInstructionV30::VOrB32);
        let original = helper_call(&input);
        let literal = SemanticOperandV1::Constant(SemanticConstantV1::new(
            inline_fixture::WORD,
            SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(bits.into(), 4).unwrap()),
        ));
        let changed = replacement_call(original, vec![original.arguments()[0].clone(), literal]);
        replace_helper_call(&mut input, changed);
        let input = input.admit();
        let scalar = ProductionSemanticScalarTypeV2::Integer {
            signed: false,
            bits: 32,
        };
        let expression = expected(
            SemanticGfx942InlineInstructionV30::VOrB32,
            ProductionSemanticExpressionV2::Symbol {
                symbol: fe2o3_pliron::PRODUCTION_KERNEL_SCALAR_SYMBOL_BASE_V2,
                scalar,
            },
            constant(bits.into()),
        );
        root_expression(&input, Some(&expression)).unwrap();
        expressions.push(expression);
    }
    assert_ne!(expressions[0], expressions[1]);
}

#[test]
fn malformed_or_absent_source_occurrences_are_rejected_at_inert_admission() {
    for missing in [false, true] {
        let mut input = Fixture::new(SemanticGfx942InlineInstructionV30::VSubU32);
        let original = helper_call(&input);
        let mut changed = SemanticDirectCallV1::new_callable(
            original.callee(),
            original.arguments().to_vec(),
            original.destination().cloned(),
            original.unwind(),
        )
        .unwrap();
        if !missing {
            changed = changed.with_inline_assembly_source_v30(
                SemanticInlineAssemblySourceV30::new(
                    [70; 32],
                    SemanticFunctionIdentityV1::from_sha256([99; 32]),
                    [71; 32],
                    [72; 32],
                )
                .unwrap(),
            );
        }
        replace_helper_call(&mut input, changed);
        assert!(
            input
                .request()
                .admit_exact_v34(SemanticMirLimitsV1::default())
                .is_err()
        );
    }
}

#[test]
fn moved_inputs_dead_results_and_unmodeled_effects_do_not_gain_helper_recipes() {
    // Structural admission does not perform the closed helper move/lifetime analysis.
    let mut moved = Fixture::new(SemanticGfx942InlineInstructionV30::VSubU32);
    let changed = replacement_call(
        helper_call(&moved),
        vec![
            SemanticOperandV1::Move(inline_fixture::word(1)),
            SemanticOperandV1::Copy(inline_fixture::word(1)),
        ],
    );
    replace_helper_call(&mut moved, changed);
    derive_refuses(&moved.admit());
    for gap in [
        SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(3)),
        SemanticStatementKindV1::Deinitialize(inline_fixture::word(3)),
    ] {
        let mut input = Fixture::new(SemanticGfx942InlineInstructionV30::VSubU32);
        let helper = &input.functions[1];
        let mut statements = vec![SemanticStatementV1::new(
            SemanticSourceProvenanceV1::unavailable(),
            gap,
        )];
        statements.extend_from_slice(helper.blocks()[1].statements());
        input.functions[1] = inline_fixture::replace_body(
            helper,
            helper.locals().to_vec(),
            vec![
                helper.blocks()[0].clone(),
                inline_fixture::block(62, statements, SemanticTerminatorKindV1::Return),
            ],
        );
        derive_refuses(&input.admit());
    }
}

#[test]
fn singleton_move_does_not_preserve_a_second_copy() {
    let mut input = Fixture::new(SemanticGfx942InlineInstructionV30::VOrB32);
    let helper = &input.functions[1];
    let mut locals = helper.locals().to_vec();
    locals.push(SemanticLocalDeclV1::new(
        SemanticLocalIdentityV1::from_sha256([65; 32]),
        inline_fixture::SINGLETON,
        SemanticLocalRoleV1::Temporary,
        SemanticSourceProvenanceV1::unavailable(),
    ));
    let original = helper.blocks()[1].statements()[0].clone();
    let statements = vec![
        original,
        inline_fixture::statement(
            inline_fixture::tuple(4),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Move(inline_fixture::tuple(0))),
        ),
        inline_fixture::statement(
            inline_fixture::tuple(0),
            SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(inline_fixture::tuple(0))),
        ),
    ];
    input.functions[1] = inline_fixture::replace_body(
        helper,
        locals,
        vec![
            helper.blocks()[0].clone(),
            inline_fixture::block(62, statements, SemanticTerminatorKindV1::Return),
        ],
    );
    derive_refuses(&input.admit());
}

#[test]
fn exact_singleton_layout_not_generic_aggregate_or_memory_repr() {
    let mut input = Fixture::new(SemanticGfx942InlineInstructionV30::VMovB32);
    assert_eq!(
        source::singleton_u32_type(&input.types, inline_fixture::SINGLETON),
        Some(inline_fixture::WORD)
    );
    assert_eq!(
        source::singleton_u32_type(&input.types, inline_fixture::WORD),
        None
    );
    input.types[inline_fixture::SINGLETON.index() as usize] =
        inline_fixture::singleton_type(SemanticBackendReprV1::memory(true));
    assert_eq!(
        source::singleton_u32_type(&input.types, inline_fixture::SINGLETON),
        None
    );
    let (result, floor, _, _, _) = run(1_000_000, 16 << 20, |meter, _| {
        source::abi(&input.types, input.functions[1].abi(), meter)
    });
    assert!(result.is_err());
    assert_eq!(floor, 4096);
}

#[test]
fn singleton_context_still_rejects_foreign_owner_and_copied_call() {
    let input = Fixture::new(SemanticGfx942InlineInstructionV30::VSubU32).admit();
    let foreign = Fixture::new(SemanticGfx942InlineInstructionV30::VSubU32).admit();
    let (result, floor, _, _, _) = run(1_000_000, 16 << 20, |meter, _| {
        with_source_helper_values(&input, 0, meter, |context, meter| {
            assert!(
                context
                    .call(&foreign, &input.functions()[0], 0, call(&input), meter)
                    .is_err()
            );
            assert!(
                context
                    .call(
                        &input,
                        &input.functions()[0],
                        0,
                        &call(&input).clone(),
                        meter
                    )
                    .is_err()
            );
            context
                .call(&input, &input.functions()[0], 0, call(&input), meter)
                .map(|_| ())
        })
    });
    result.unwrap();
    assert_eq!(floor, 4096);
}

#[test]
fn singleton_isa_helper_uses_exact_shared_work_and_storage_cutoffs() {
    let input = Fixture::new(SemanticGfx942InlineInstructionV30::VSubU32).admit();
    let probe = |work, storage| {
        run(work, storage, |meter, _| {
            let template = source::derive(&input, 1, &[], meter)?;
            template.destroy(meter)
        })
    };
    let (result, floor, work, peak, _) = probe(1_000_000, 16 << 20);
    result.unwrap();
    assert_eq!(floor, 4096);
    let (result, floor, _, _, _) = probe(work, peak);
    result.unwrap();
    assert_eq!(floor, 4096);
    let (result, floor, _, _, _) = probe(work - 1, peak);
    assert_eq!(result, Err("work"));
    assert_eq!(floor, 4096);
    let (result, floor, _, _, _) = probe(work, peak - 1);
    assert_eq!(result, Err("storage"));
    assert_eq!(floor, 4096);
}
