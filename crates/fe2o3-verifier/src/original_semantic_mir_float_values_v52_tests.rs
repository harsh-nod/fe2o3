use super::*;

fn transform(
    types: &mut Vec<SemanticTypeDeclV1>,
    functions: &mut Vec<SemanticFunctionDeclV1>,
    bits: u16,
) {
    assert!(matches!(bits, 32 | 64));
    let ty = SemanticTypeIdV1::from_index(0);
    let bytes = u64::from(bits / 8);
    let old_type = &types[0];
    types[0] = SemanticTypeDeclV1::new(
        old_type.identity(),
        old_type.layout_identity(),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(bytes),
            bytes,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::float(bits, bytes),
                SemanticScalarValidityRangeV1::new(0, (1u128 << bits) - 1),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Float { bits }),
    );
    let raw = SemanticTypeIdV1::from_index(types.len() as u32);
    types.push(SemanticTypeDeclV1::new(
        SemanticTypeIdentityV1::from_sha256([243; 32]),
        SemanticLayoutIdentityV1::from_sha256([244; 32]),
        SemanticTypeLayoutV1::new_with_backend_repr(
            Some(8),
            8,
            SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                SemanticBackendPrimitiveV1::pointer(0, 8, 8),
                SemanticScalarValidityRangeV1::new(0, u64::MAX.into()),
            )),
            false,
        )
        .unwrap(),
        SemanticTypeShapeV1::Pointer(
            SemanticPointerTypeV1::new_with_kind(
                ty,
                SemanticPointerKindV1::Raw,
                SemanticMutabilityV1::Mutable,
                0,
                64,
                SemanticPointerMetadataV1::None,
            )
            .unwrap(),
        ),
    ));
    let helper = functions.last_mut().unwrap();
    assert_eq!(helper.locals().len(), 4);
    let source = helper.source();
    let mut locals = helper.locals().to_vec();
    for (ordinal, local_type) in [ty, ty, raw, raw].into_iter().enumerate() {
        locals.push(SemanticLocalDeclV1::new(
            SemanticLocalIdentityV1::from_sha256([244 + ordinal as u8; 32]),
            local_type,
            SemanticLocalRoleV1::Temporary,
            source,
        ));
    }
    let place =
        |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
    let assign = |local, local_type, rhs| {
        SemanticStatementV1::new(
            source,
            SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(
                place(local, local_type),
                SemanticRvalueV1::new(local_type, rhs),
            )),
        )
    };
    let copy = |local| SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place(local, ty)));
    let marker = |local, live| {
        SemanticStatementV1::new(
            source,
            if live {
                SemanticStatementKindV1::StorageLive(SemanticLocalIdV1::from_index(local))
            } else {
                SemanticStatementKindV1::StorageDead(SemanticLocalIdV1::from_index(local))
            },
        )
    };
    let statements = vec![
        marker(4, true),
        marker(5, true),
        marker(6, true),
        marker(7, true),
        assign(4, ty, copy(1)),
        assign(5, ty, copy(2)),
        assign(
            6,
            raw,
            SemanticRvalueKindV1::AddressOf {
                place: place(4, ty),
                mutability: SemanticMutabilityV1::Mutable,
            },
        ),
        assign(
            7,
            raw,
            SemanticRvalueKindV1::AddressOf {
                place: place(5, ty),
                mutability: SemanticMutabilityV1::Mutable,
            },
        ),
        assign(
            3,
            ty,
            SemanticRvalueKindV1::Binary {
                operation: SemanticBinaryOpV1::Add,
                left: SemanticOperandV1::Copy(place(4, ty)),
                right: SemanticOperandV1::Copy(place(5, ty)),
            },
        ),
        assign(4, ty, copy(3)),
        assign(0, ty, copy(4)),
        marker(7, false),
        marker(6, false),
        marker(5, false),
        marker(4, false),
    ];
    let block = SemanticBasicBlockV1::new(
        helper.blocks()[0].identity(),
        source,
        statements,
        SemanticTerminatorV1::new(source, SemanticTerminatorKindV1::Return),
    )
    .unwrap();
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
        vec![block],
    )
    .unwrap();
}

fn f32_transform(types: &mut Vec<SemanticTypeDeclV1>, functions: &mut Vec<SemanticFunctionDeclV1>) {
    transform(types, functions, 32);
}
fn f64_transform(types: &mut Vec<SemanticTypeDeclV1>, functions: &mut Vec<SemanticFunctionDeclV1>) {
    transform(types, functions, 64);
}

#[test]
fn original_float_object_events_reach_the_mandatory_paired_source_consumer() {
    for bits in [32, 64] {
        let mut reached = false;
        let result = super::super::super::invocations::tests::run_source_transform(
            LIMIT,
            LIMIT,
            |types, functions| transform(types, functions, bits),
            |plan, out| {
                source_function::tests::with_slots(plan, out, |slots, out| {
                    for root in 0..2 {
                        for instance in 1..=2 {
                            for local in [4, 5] {
                                assert!(slots.has_original_object(root, instance, local, out)?);
                            }
                        }
                    }
                    let mut source = source_function::SourceByteProgram::derive(plan, slots, out)?;
                    let bindings = byte_bindings::SourceByteBindings::derive(slots, out)?;
                    let paired = paired::PairedInvocations::derive(
                        plan,
                        &source,
                        FormalIndexWidth::Bits64,
                        out,
                    )?;
                    slots.emit(out)?;
                    source.emit(out)?;
                    bindings.emit(out)?;
                    paired.emit(out)?;
                    assert_eq!(out.text.matches(&format!("operation: 11int, input_bits: {bits}int, input_signed: false, output_bits: {bits}int")).count(), 4);
                    assert!(out.text.contains("InvocationSourceByteValueV36::Read"));
                    assert!(
                        out.text
                            .contains("InvocationSourceByteBaseV36::ObjectLocal(")
                    );
                    reached = true;
                    Ok(())
                })
            },
        );
        assert!(
            reached,
            "paired F{bits} source consumer not reached: {:?}",
            result.0
        );
        result.0.unwrap();
    }
}

#[test]
fn original_float_load_add_store_reaches_the_complete_typed_production_generator() {
    type Transform = fn(&mut Vec<SemanticTypeDeclV1>, &mut Vec<SemanticFunctionDeclV1>);
    for (bits, transform) in [
        (32, f32_transform as Transform),
        (64, f64_transform as Transform),
    ] {
        let mut completed = false;
        let result = run_transform(LIMIT, LIMIT, false, false, Some(transform), |text| {
            for root in 0..2 {
                assert!(text.contains(&format!("proof fn typed_final_source_step_{root}_v49")));
                assert!(text.contains(&format!(
                    "invocation_paired_native_inputs_{root}_v38(arguments, external, execution)"
                )));
            }
            assert!(text.contains(&format!("operation: 11int, input_bits: {bits}int, input_signed: false, output_bits: {bits}int")));
            assert!(text.contains(&format!(
                "byte_float_value_v52(s.frames, 11int, {bits}int, {bits}int"
            )));
            assert!(text.contains("ieee_operators: spec_fn(int, int, int, int, int, int) -> int"));
            assert!(text.contains("InvocationSourceByteValueV36::Read"));
            assert!(!text.contains("assume("));
            assert!(!text.contains("external_body"));
            completed = true;
        });
        assert!(
            completed,
            "genuine F{bits} production chain did not finish: {:?}",
            result.0
        );
        result.0.unwrap();
    }
}

#[test]
fn original_float_complete_generation_has_exact_and_one_short_resources() {
    let measured = run_transform(LIMIT, LIMIT, false, false, Some(f32_transform), |_| {});
    measured.0.unwrap();
    let exact = run_transform(
        measured.1,
        measured.3,
        false,
        false,
        Some(f32_transform),
        |_| {},
    );
    exact.0.unwrap();
    assert_eq!(
        (exact.1, exact.2, exact.3),
        (measured.1, measured.2, measured.3)
    );
    for work_short in [false, true] {
        let work = measured.1 - usize::from(work_short);
        let storage = measured.3 - usize::from(!work_short);
        let denied = run_transform(work, storage, false, false, Some(f32_transform), |_| {});
        let error = denied
            .0
            .expect_err("one-short float generation must refuse");
        let mut chain: &(dyn std::error::Error + 'static) = &error;
        let resource = loop {
            if let Some(resource) = chain.downcast_ref::<Resource>() {
                break *resource;
            }
            chain = chain
                .source()
                .unwrap_or_else(|| panic!("missing resource: {error:?}"));
        };
        match resource {
            Resource::Work(limit) if work_short => {
                assert_eq!(limit.limit(), work);
                assert_eq!(limit.actual(), measured.1);
            }
            Resource::Storage(limit) if !work_short => {
                assert_eq!(limit.limit(), storage);
                assert_eq!(limit.actual(), measured.3);
            }
            other => panic!("wrong resource: {other:?}"),
        }
    }
}
