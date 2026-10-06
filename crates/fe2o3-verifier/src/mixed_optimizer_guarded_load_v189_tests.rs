fn guarded_load_module_v189(space: AddressSpace, scalar: ScalarType) -> Module {
    let payload = Type::Scalar(scalar);
    let pointer = Type::pointer(payload.clone(), space, AccessMode::ReadOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        KirOperation::new(
            vec![ValueDef::new(ValueId(3), payload.clone())],
            OperationKind::GuardedLoad {
                pointer: ValueId(0),
                predicate: ValueId(1),
                fallback: ValueId(2),
                access: MemoryAccess::new(space, 1),
            },
        ),
        KirOperation::new(
            vec![ValueDef::new(ValueId(4), payload.clone())],
            OperationKind::Load {
                pointer: ValueId(0),
                access: MemoryAccess::new(space, 1),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("conditional-byte-load");
    module.functions.push(KirFunction::internal_helper(
        "load",
        Signature::new(vec![pointer, Type::BOOL, payload], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module
}

#[test]
fn guarded_byte_load_preserves_selected_operand_order_and_exact_read_effect() {
    use fe2o3_kernel_analysis::{
        CanonicalKirPrivateByteObligationV38 as Obligation,
        CanonicalKirPrivateByteOperationKindV38 as Kind,
    };
    for space in [
        AddressSpace::Global,
        AddressSpace::Generic,
        AddressSpace::Private,
    ] {
        for scalar in [
            ScalarType::Bool,
            ScalarType::U8,
            ScalarType::I16,
            ScalarType::U32,
            ScalarType::I64,
            ScalarType::U128,
            ScalarType::F32,
            ScalarType::Index,
        ] {
            for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
                with_inventory(
                    &guarded_load_module_v189(space, scalar),
                    |inventory, physical, floor| {
                        let fact = physical.operation(0).unwrap();
                        assert_eq!(fact.kind(), Kind::Unmodeled);
                        assert!(fact.requires(Obligation::Operation));
                        assert!(!fact.is_proven());
                        assert_eq!(inventory.operations()[0].operands.len(), 3);
                        assert_eq!(inventory.operations()[0].results.len(), 1);
                        assert_eq!(inventory.operations()[0].effects.len(), 1);
                        let text =
                            guarded_store_emit_v90(inventory, physical, floor, width, LIMIT, LIMIT)
                                .0
                                .unwrap();
                        let body = text
                            .split_once("spec fn byte_operation_90_0_v30(")
                            .unwrap()
                            .1
                            .split_once("spec fn byte_operation_90_1_v30(")
                            .unwrap()
                            .0;
                        let (inactive, active) = body.split_once("else if predicate == 1").unwrap();
                        assert!(inactive.contains("s.valid && match s.values[1]"));
                        assert!(inactive.contains("if predicate == 0 { match s.values[2]"));
                        assert!(!inactive.contains("byte_load_v30"));
                        assert!(!inactive.contains("byte_range_aligned_v30"));
                        assert!(!inactive.contains("s.values[0]"));
                        let bytes = scalar_bytes(scalar, width).unwrap();
                        assert!(active.contains(&format!("byte_pointer_type_v30(p, {}, {}) && byte_range_aligned_v30(s.memory, p, {bytes}, 1) && byte_scalar_range_initialized_v37(s.memory, p, {bytes})",
                        pointer_space(space).unwrap(), scalar_bytes(ScalarType::Index, width).unwrap())));
                        assert!(body.contains("let values = s.values.update(3, if valid { if s.values[1] == MemoryValueV30::Scalar(0) { s.values[2] } else"));
                        assert!(body.contains("if !valid { MemoryOperationEffectV30::Refused } else if s.values[1] == MemoryValueV30::Scalar(1) { MemoryOperationEffectV30::Read { address: s.values[0]"));
                        assert!(body.contains(
                            "value: values[3] } } else { MemoryOperationEffectV30::Pure }"
                        ));
                        assert!(body.contains("let memory = s.memory;"));
                        assert!(body.contains("let generations = s.generations;"));
                        assert!(body.contains("let frames = s.frames;"));
                    },
                );
            }
        }
    }
}

#[test]
fn guarded_byte_load_rejects_wrong_predicate_fallback_permissions_results_and_order() {
    for mutant in 0..7 {
        let mut module = guarded_load_module_v189(AddressSpace::Global, ScalarType::U32);
        let function = &mut module.functions[0];
        match mutant {
            0 => function.signature.parameters[1] = Type::Scalar(ScalarType::U32),
            1 => function.signature.parameters[2] = Type::Scalar(ScalarType::U64),
            2 => {
                function.signature.parameters[0] = Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::WriteOnly,
                )
            }
            3 => function.body.as_mut().unwrap().blocks[0].operations[0]
                .results
                .clear(),
            4 => function.body.as_mut().unwrap().blocks[0].operations[0].results[0].ty = Type::BOOL,
            5 => {
                let OperationKind::GuardedLoad {
                    predicate,
                    fallback,
                    ..
                } = &mut function.body.as_mut().unwrap().blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                std::mem::swap(predicate, fallback);
            }
            6 => {
                let OperationKind::GuardedLoad { access, .. } =
                    &mut function.body.as_mut().unwrap().blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                access.address_space = AddressSpace::Generic;
            }
            _ => unreachable!(),
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        assert!(
            Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUTS, &mut budget)
                .is_err(),
            "mutant {mutant}"
        );
    }
}

#[test]
fn guarded_byte_load_keeps_unknown_width_and_volatile_access_closed() {
    for (volatile, width, expected) in [
        (
            true,
            FormalIndexWidth::Bits64,
            "byte volatile operation is not modeled",
        ),
        (
            false,
            FormalIndexWidth::Unknown,
            "actual byte index width is unknown",
        ),
    ] {
        let mut module = guarded_load_module_v189(AddressSpace::Global, ScalarType::U32);
        let OperationKind::GuardedLoad { access, .. } =
            &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        access.volatile = volatile;
        with_inventory(&module, |inventory, physical, floor| {
            assert!(
                matches!(guarded_store_emit_v90(inventory, physical, floor, width, LIMIT, LIMIT).0,
                Err(Error::Statement(actual)) if actual == expected)
            );
        });
    }
}

#[test]
fn guarded_byte_load_replays_exact_and_one_short_resource_limits() {
    with_inventory(
        &guarded_load_module_v189(AddressSpace::Global, ScalarType::U32),
        |inventory, physical, floor| {
            let measured = guarded_store_emit_v90(
                inventory,
                physical,
                floor,
                FormalIndexWidth::Bits64,
                LIMIT,
                LIMIT,
            );
            let text = measured.0.unwrap();
            let exact = guarded_store_emit_v90(
                inventory,
                physical,
                floor,
                FormalIndexWidth::Bits64,
                measured.1,
                measured.2,
            );
            assert_eq!(exact.0.unwrap(), text);
            assert_eq!((exact.1, exact.2), (measured.1, measured.2));
            for (work, storage) in [(measured.1 - 1, measured.2), (measured.1, measured.2 - 1)] {
                assert!(matches!(
                    guarded_store_emit_v90(
                        inventory,
                        physical,
                        floor,
                        FormalIndexWidth::Bits64,
                        work,
                        storage
                    )
                    .0,
                    Err(Error::Resource(_))
                ));
            }
        },
    );
}

fn guarded_load_program_v189() -> String {
    let mut generated = None;
    with_inventory(
        &guarded_load_module_v189(AddressSpace::Global, ScalarType::U32),
        |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            generated = Some(
                run(floor, LIMIT, LIMIT, |out| {
                    emit!(
                        out,
                        "use vstd::prelude::*;\nverus! {{\n{}",
                        super::super::byte_memory_v30::BYTE_MEMORY_V30
                    );
                    ByteFunctionV30::derive(
                        inventory,
                        physical,
                        Function(0),
                        ByteContext::native(FormalIndexWidth::Bits64),
                        &allocations,
                        out,
                    )?
                    .emit(90, out)?;
                    emit!(
                        out,
                        "{}\n}}\n",
                        include_str!("mixed_optimizer_guarded_load_v189_tests.vrs")
                    );
                    Ok(())
                })
                .0
                .unwrap(),
            );
        },
    );
    generated.unwrap()
}

#[test]
fn guarded_byte_load_equations_use_actual_dispatch_and_preserve_all_failure_cases() {
    let source = guarded_load_program_v189();
    let laws = include_str!("mixed_optimizer_guarded_load_v189_tests.vrs");
    assert_eq!(laws.matches("proof fn ").count(), 7);
    assert_eq!(source.matches("spec fn byte_operation_90_").count(), 2);
    for forbidden in [
        "assume(",
        "admit(",
        "external_body",
        "new_assuming_finite",
        "verify-only",
    ] {
        assert!(!source.contains(forbidden));
    }
}

#[test]
#[ignore = "exports complete generated equations; does not execute or admit a proof"]
fn diagnostic_complete_guarded_load_model_export_v189() {
    use sha2::{Digest, Sha256};
    use std::io::Write as _;
    let source = guarded_load_program_v189();
    assert!(source.len() <= 16 * 1024 * 1024);
    let mut output = std::io::stdout().lock();
    write!(
        output,
        "{{\"kind\":\"fe2o3-guarded-load-model-v189\",\"bytes\":{},\"sha256\":\"",
        source.len()
    )
    .unwrap();
    for byte in Sha256::digest(source.as_bytes()) {
        write!(output, "{byte:02x}").unwrap();
    }
    write!(output, "\",\"model_hex\":\"").unwrap();
    for byte in source.as_bytes() {
        write!(output, "{byte:02x}").unwrap();
    }
    writeln!(output, "\"}}").unwrap();
}
