use super::*;
use fe2o3_mir_model::semantic_mir_v1::*;

const LIMIT: usize = 256 * 1024 * 1024;

#[test]
fn original_integer_casts_emit_genuine_width_signedness_and_copy_move_matrix() {
    super::super::super::super::invocations::tests::run_source_transform(
        LIMIT, LIMIT,
        |types, functions| {
            let mut scalars = Vec::new();
            for (ordinal, (bits, signed)) in [8, 16, 32, 64].into_iter()
                .flat_map(|bits| [(bits, false), (bits, true)]).chain([(1, false)]).enumerate()
            {
                let ty = SemanticTypeIdV1::from_index(types.len() as u32);
                let bytes = if bits == 1 { 1 } else { bits / 8 };
                let maximum = (1u128 << bits) - 1;
                types.push(SemanticTypeDeclV1::new(
                    SemanticTypeIdentityV1::from_sha256([220 + ordinal as u8; 32]),
                    SemanticLayoutIdentityV1::from_sha256([220 + ordinal as u8; 32]),
                    SemanticTypeLayoutV1::new_with_backend_repr(Some(bytes), bytes,
                        SemanticBackendReprV1::scalar(SemanticBackendScalarV1::initialized(
                            SemanticBackendPrimitiveV1::integer(signed, if bits == 1 { 8 } else { bits as u16 }, bytes),
                            SemanticScalarValidityRangeV1::new(0, maximum))), false).unwrap(),
                    SemanticTypeShapeV1::Scalar(if bits == 1 { SemanticScalarTypeV1::Bool }
                        else { SemanticScalarTypeV1::Integer { signed, bits: bits as u16 } }),
                ));
                scalars.push((ty, bytes, maximum));
            }
            let function = functions.last_mut().unwrap();
            let source = function.source();
            let mut locals = function.locals().to_vec();
            let mut statements = function.blocks()[0].statements().to_vec();
            let place = |local, ty| SemanticPlaceV1::new(SemanticLocalIdV1::from_index(local), vec![], ty).unwrap();
            let assign = |destination: SemanticPlaceV1, value| SemanticStatementV1::new(source,
                SemanticStatementKindV1::Assign(SemanticAssignmentV1::new(destination.clone(),
                    SemanticRvalueV1::new(destination.ty(), value))));
            for &(ty, bytes, maximum) in &scalars {
                let input = locals.len() as u32;
                locals.push(SemanticLocalDeclV1::new(SemanticLocalIdentityV1::from_sha256([100 + input as u8; 32]), ty, SemanticLocalRoleV1::Temporary, source));
                statements.push(assign(place(input, ty), SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(
                    SemanticConstantV1::new(ty, SemanticConstantValueV1::Scalar(SemanticScalarValueV1::new(maximum, bytes as u8).unwrap()))))));
                for (target, &(output_type, _, _)) in scalars[..8].iter().enumerate() {
                    let output = locals.len() as u32;
                    locals.push(SemanticLocalDeclV1::new(SemanticLocalIdentityV1::from_sha256([100 + output as u8; 32]), output_type, SemanticLocalRoleV1::Temporary, source));
                    statements.push(assign(place(output, output_type), SemanticRvalueKindV1::Cast {
                        kind: SemanticCastKindV1::Integer,
                        operand: if target == 7 { SemanticOperandV1::Move(place(input, ty)) }
                            else { SemanticOperandV1::Copy(place(input, ty)) },
                    }));
                }
            }
            *function = SemanticFunctionDeclV1::new(function.identity(), function.role(),
                function.item_definition_identity(), function.monomorphization_identity(),
                function.generic_type_arguments_identity(), function.const_generic_arguments_identity(),
                source, function.abi().clone(), locals, function.entry(),
                vec![SemanticBasicBlockV1::new(function.blocks()[0].identity(), source, statements,
                    function.blocks()[0].terminator().clone()).unwrap()]).unwrap();
        },
        |plan, out| super::super::super::source_function::tests::with_slots(plan, out, |slots, out| {
            let mut program = super::super::super::source_function::SourceByteProgram::derive(plan, slots, out)?;
            program.emit(out)?;
            assert_eq!(out.text.matches("InvocationSourceByteEventV36::IntegerCast(").count(), 4 * 9 * 8);
            for input in [1, 8, 16, 32, 64] {
                for output in [8, 16, 32, 64] {
                    assert!(out.text.contains(&format!("input_bits: {input}int, input_signed: false, output_bits: {output}int")));
                    if input != 1 {
                        assert!(out.text.contains(&format!("input_bits: {input}int, input_signed: true, output_bits: {output}int")));
                    }
                }
            }
            assert!(out.text.contains("moved: true") && out.text.contains("moved: false"));
            Ok(())
        }),
    ).0.unwrap();
}

#[test]
fn original_integer_cast_value_reference_matrix_matches_rust_scalar_casts() {
    let model = |bits: u128, width: u32, signed: bool, target: u32| {
        let mathematical = if signed && bits >= (1u128 << (width - 1)) {
            bits as i128 - (1i128 << width)
        } else {
            bits as i128
        };
        mathematical.rem_euclid(1i128 << target) as u128
    };
    for value in 0..=u8::MAX {
        assert_eq!(model(value.into(), 8, false, 16), u128::from(value as u16));
        assert_eq!(
            model(value.into(), 8, true, 16),
            u128::from((value as i8) as u16)
        );
        assert_eq!(
            model(value.into(), 8, true, 64),
            u128::from((value as i8) as u64)
        );
        assert_eq!(
            model(value.into(), 8, true, 8),
            u128::from((value as i8) as u8)
        );
    }
    for value in [
        0u64,
        1,
        u32::MAX as u64,
        i64::MAX as u64,
        (i64::MAX as u64) + 1,
        u64::MAX,
    ] {
        assert_eq!(model(value.into(), 64, false, 8), u128::from(value as u8));
        assert_eq!(
            model(value.into(), 64, true, 32),
            u128::from((value as i64) as u32)
        );
        assert_eq!(
            model(value.into(), 64, true, 64),
            u128::from((value as i64) as u64)
        );
    }
    for value in [false, true] {
        assert_eq!(
            model(u128::from(value), 1, false, 64),
            u128::from(value as u64)
        );
    }
}

#[test]
fn original_integer_cast_emission_has_exact_independent_header_and_text_costs() {
    use crate::mixed_optimizer_refinement_v26::SOURCE_LIMIT;
    use fe2o3_kernel_ir::{
        CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
        CanonicalKernelIrWorkBudgetV1 as Work,
    };
    type Fields = (Destination, Value, ScalarV30, ScalarV30);
    let header = size_of::<Fields>()
        + 2 * size_of::<Result<Option<Fields>>>()
        + 2 * size_of::<ScalarV30>()
        + size_of::<SemanticCastKindV1>()
        + size_of::<Value>()
        + size_of::<Destination>()
        + 4 * size_of::<&()>();
    assert_eq!(size_of::<Cast>(), size_of::<Fields>());
    assert_eq!(headers(), header);
    let cast = Cast {
        destination: Destination::Local(2),
        value: Value::Local {
            local: 1,
            moved: true,
        },
        input: ScalarV30::Integer {
            width: 8,
            signed: true,
        },
        output: ScalarV30::Integer {
            width: 64,
            signed: false,
        },
    };
    let expected = "InvocationSourceByteEventV36::IntegerCast(InvocationSourceIntegerCastV43 { destination: InvocationSourceByteDestinationV36::Local(2int), value: InvocationSourceByteValueV36::Local { local: 1int, moved: true }, input_bits: 8int, input_signed: true, output_bits: 64int })";
    let work = 2 + expected.len();
    let storage = SOURCE_LIMIT + header;
    let run = |work_limit, storage_limit| {
        let mut work = Work::new(work_limit);
        let mut budget = Budget::new(&mut work, storage_limit);
        let result = (|| {
            budget.reserve_storage(SOURCE_LIMIT + headers())?;
            let mut out = Writer::new(&mut budget)?;
            cast.emit(&mut out)?;
            out.finish()
        })();
        (result, budget.work(), budget.peak_storage())
    };
    let exact = run(work, storage);
    assert_eq!(exact.0.unwrap(), expected);
    assert_eq!((exact.1, exact.2), (work, storage));
    assert!(
        matches!(run(work - 1, storage).0, Err(Error::Resource(Resource::Work(error))) if error.actual() == work && error.limit() == work - 1)
    );
    assert!(
        matches!(run(work, storage - 1).0, Err(Error::Resource(Resource::Storage(error))) if error.actual() == storage && error.limit() == storage - 1)
    );
}

#[test]
fn original_integer_cast_runtime_keeps_original_types_and_ordered_memory_observations() {
    let host = include_str!("original_semantic_mir_source_integer_casts_v43.rs");
    let runtime = include_str!("original_semantic_mir_source_integer_casts_v43.vrs");
    for guard in [
        "*kind != SemanticCastKindV1::Integer",
        "context.scalar(operand.ty(), out)?",
        "context.scalar(assignment.value().result_type(), out)?",
        "context.value(operand, out)?",
    ] {
        assert!(host.contains(guard));
    }
    for guard in [
        "invocation_source_byte_value_typed_v36(value, input_bits)",
        "input_bits == 1 && input_signed",
        "source_signed_v30(bits, memory_value_modulus_v30(input_bits / 8))",
        "((mathematical % modulus) + modulus) % modulus",
        "InvocationSourceByteDestinationV36::Component(place)",
        "cast.output_bits == access.width * 8",
        "writes, evaluated.source, after",
    ] {
        assert!(runtime.contains(guard), "{guard}");
    }
    assert!(super::super::SOURCE_BYTES_V36.contains(runtime));
    for effects in [
        include_str!("original_semantic_mir_observed_effects_v39.vrs"),
        include_str!("original_semantic_mir_invocation_effects_v36.rs"),
    ] {
        assert!(effects.contains("Some(InvocationSourceByteEventV36::IntegerCast(cast))"));
        assert!(effects.contains("if result.source == observation.after"));
    }
}
