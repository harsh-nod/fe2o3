const GUARDED_STORE_EQUATIONS_V90: &str =
    include_str!("mixed_optimizer_guarded_store_v90_tests.vrs");

fn guarded_store_module_v90(space: AddressSpace, scalar: ScalarType) -> Module {
    let payload = Type::Scalar(scalar);
    let pointer = Type::pointer(payload.clone(), space, AccessMode::WriteOnly);
    let mut block = BasicBlock::new(BlockId(0));
    block.operations = vec![
        KirOperation::new(
            vec![],
            OperationKind::GuardedStore {
                pointer: ValueId(0),
                predicate: ValueId(1),
                value: ValueId(2),
                access: MemoryAccess::new(space, 1),
            },
        ),
        KirOperation::new(
            vec![],
            OperationKind::Store {
                pointer: ValueId(0),
                value: ValueId(2),
                access: MemoryAccess::new(space, 1),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("exact-conditional-byte-store");
    module.functions.push(KirFunction::internal_helper(
        "store",
        Signature::new(vec![pointer, Type::BOOL, payload], vec![]),
        vec![ValueId(0), ValueId(1), ValueId(2)],
        vec![block],
    ));
    module
}

fn guarded_store_emit_v90(
    inventory: &Inventory<'_>,
    physical: &Physical<'_, '_>,
    floor: usize,
    width: FormalIndexWidth,
    work: usize,
    storage: usize,
) -> (Result<String>, usize, usize) {
    let allocations = NoAllocations(inventory.owner());
    run(floor, work, storage, |out| {
        ByteFunctionV30::derive(
            inventory,
            physical,
            Function(0),
            ByteContext::native(width),
            &allocations,
            out,
        )?
        .emit(90, out)
    })
}

#[test]
fn guarded_byte_store_retains_exact_conditional_effect_and_conservative_physical_fact() {
    use fe2o3_kernel_analysis::{
        CanonicalKirPrivateByteObligationV38 as Obligation,
        CanonicalKirPrivateByteOperationKindV38 as Kind,
    };
    for space in [AddressSpace::Global, AddressSpace::Generic] {
        for scalar in [
            ScalarType::Bool,
            ScalarType::U8,
            ScalarType::U16,
            ScalarType::U32,
            ScalarType::U64,
            ScalarType::U128,
            ScalarType::Index,
        ] {
            for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
                with_inventory(
                    &guarded_store_module_v90(space, scalar),
                    |inventory, physical, floor| {
                        let fact = physical.operation(0).unwrap();
                        assert_eq!(fact.kind(), Kind::Unmodeled);
                        assert!(!fact.is_proven());
                        assert_eq!(fact.payload_initialized(), None);
                        assert!(fact.requires(Obligation::Operation));
                        assert_eq!(inventory.operations()[0].operands.len(), 3);
                        assert!(inventory.operations()[0].results.is_empty());
                        assert_eq!(inventory.operations()[0].effects.len(), 1);
                        let source =
                            guarded_store_emit_v90(inventory, physical, floor, width, LIMIT, LIMIT)
                                .0
                                .unwrap();
                        let space = pointer_space(space).unwrap();
                        let bytes = scalar_bytes(scalar, width).unwrap();
                        let index = scalar_bytes(ScalarType::Index, width).unwrap();
                        assert!(source.contains("s.valid && match s.values[1] { MemoryValueV30::Scalar(0) => true, MemoryValueV30::Scalar(1) => match (s.values[0], s.values[2])"));
                        assert!(source.contains(&format!("byte_pointer_type_v30(p, {space}, {index}) && byte_range_aligned_v30(s.memory, p, {bytes}, 1)")));
                        assert!(source.contains("if !valid { MemoryOperationEffectV30::Refused } else if s.values[1] == MemoryValueV30::Scalar(1)"));
                        assert!(source.contains("} else { MemoryOperationEffectV30::Pure }"));
                        assert!(source.contains("let values = s.values;"));
                        assert!(source.contains("let generations = s.generations;"));
                        assert!(source.contains("let frames = s.frames;"));
                        assert_eq!(source.matches("spec fn byte_operation_90_").count(), 2);
                    },
                );
            }
        }
    }
}

#[test]
fn guarded_byte_store_does_not_discharge_pointer_formation_or_prior_value_evaluation() {
    let mut module = guarded_store_module_v90(AddressSpace::Global, ScalarType::U32);
    let function = &mut module.functions[0];
    function.signature.parameters.push(Type::INDEX);
    function.body.as_mut().unwrap().parameters.push(ValueId(3));
    let pointer = function.signature.parameters[0].clone();
    let block = &mut function.body.as_mut().unwrap().blocks[0];
    block.operations.insert(
        0,
        KirOperation::effect_free(
            ValueDef::new(ValueId(4), pointer),
            OperationKind::GetElementPointer {
                base: ValueId(0),
                offset: ValueId(3),
            },
        ),
    );
    let OperationKind::GuardedStore { pointer, .. } = &mut block.operations[1].kind else {
        unreachable!()
    };
    *pointer = ValueId(4);
    with_inventory(&module, |inventory, physical, floor| {
        let source = guarded_store_emit_v90(
            inventory,
            physical,
            floor,
            FormalIndexWidth::Bits64,
            LIMIT,
            LIMIT,
        )
        .0
        .unwrap();
        assert!(source.contains("byte_range_live_v30(s.memory, p, 0)"));
        assert!(source.contains("p.byte_offset + i * 4 < memory_value_modulus_v30(8)"));
        assert!(source.contains("let step0 = byte_operation_90_0_v30(current, little_endian);\n let current = step0.state;"));
        assert!(source.contains("let step1 = byte_operation_90_1_v30(current, little_endian);"));
        assert!(source.contains("s.valid && match s.values[1]"));
    });
}

#[test]
fn guarded_byte_store_refuses_unmodeled_spaces_volatile_and_unknown_index_width() {
    for (space, volatile, width, message) in [
        (
            AddressSpace::Private,
            false,
            FormalIndexWidth::Bits64,
            "byte guarded store requires external memory",
        ),
        (
            AddressSpace::Global,
            true,
            FormalIndexWidth::Bits64,
            "byte volatile operation is not modeled",
        ),
        (
            AddressSpace::Global,
            false,
            FormalIndexWidth::Unknown,
            "actual byte index width is unknown",
        ),
    ] {
        let mut module = guarded_store_module_v90(space, ScalarType::U32);
        let OperationKind::GuardedStore { access, .. } =
            &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations[0].kind
        else {
            unreachable!()
        };
        access.volatile = volatile;
        with_inventory(&module, |inventory, physical, floor| {
            let result = guarded_store_emit_v90(inventory, physical, floor, width, LIMIT, LIMIT);
            assert!(matches!(result.0, Err(Error::Statement(actual)) if actual == message));
        });
    }
}

#[test]
fn guarded_byte_store_rejects_wrong_predicate_payload_permissions_results_and_operand_order() {
    for mutant in 0..6 {
        let mut module = guarded_store_module_v90(AddressSpace::Global, ScalarType::U32);
        let function = &mut module.functions[0];
        match mutant {
            0 => function.signature.parameters[1] = Type::Scalar(ScalarType::U32),
            1 => function.signature.parameters[2] = Type::Scalar(ScalarType::U64),
            2 => {
                function.signature.parameters[0] = Type::pointer(
                    Type::Scalar(ScalarType::U32),
                    AddressSpace::Global,
                    AccessMode::ReadOnly,
                )
            }
            3 => function.body.as_mut().unwrap().blocks[0].operations[0]
                .results
                .push(ValueDef::new(ValueId(3), Type::BOOL)),
            4 => {
                let OperationKind::GuardedStore {
                    predicate, value, ..
                } = &mut function.body.as_mut().unwrap().blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                std::mem::swap(predicate, value);
            }
            5 => {
                let OperationKind::GuardedStore { access, .. } =
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
            "canonical owner must refuse mutant {mutant}"
        );
    }
}

#[test]
fn guarded_byte_store_keeps_exact_short_and_sticky_owner_accounts() {
    let module = guarded_store_module_v90(AddressSpace::Global, ScalarType::U32);
    with_inventory(&module, |inventory, physical, floor| {
        let measured = guarded_store_emit_v90(
            inventory,
            physical,
            floor,
            FormalIndexWidth::Bits64,
            LIMIT,
            LIMIT,
        );
        let source = measured.0.unwrap();
        let exact = guarded_store_emit_v90(
            inventory,
            physical,
            floor,
            FormalIndexWidth::Bits64,
            measured.1,
            measured.2,
        );
        assert_eq!(exact.0.unwrap(), source);
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
        let allocations = NoAllocations(inventory.owner());
        run(floor, LIMIT, LIMIT, |out| {
            let model = ByteFunctionV30::derive(
                inventory,
                physical,
                Function(0),
                ByteContext::native(FormalIndexWidth::Bits64),
                &allocations,
                out,
            )?;
            out.budget.release_storage(1)?;
            let work = out.budget.work();
            for _ in 0..2 {
                assert!(matches!(
                    model.emit(90, out),
                    Err(Error::Resource(Resource::Accounting))
                ));
                assert_eq!(out.budget.work(), work);
                assert!(out.text.is_empty());
            }
            Ok(())
        })
        .0
        .unwrap();
        with_inventory(&module, |foreign, _, _| {
            let allocations = NoAllocations(foreign.owner());
            let result = run(floor, LIMIT, LIMIT, |out| {
                ByteFunctionV30::derive(
                    foreign,
                    physical,
                    Function(0),
                    ByteContext::native(FormalIndexWidth::Bits64),
                    &allocations,
                    out,
                )
                .map(|_| ())
            });
            assert!(matches!(
                result.0,
                Err(Error::Statement(
                    "actual byte function owner or occurrence differs"
                ))
            ));
        });
    });
}

fn guarded_store_program_v90() -> String {
    let mut generated = None;
    with_inventory(
        &guarded_store_module_v90(AddressSpace::Global, ScalarType::U32),
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
                    emit!(out, "{GUARDED_STORE_EQUATIONS_V90}\n}}\n");
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
fn guarded_byte_store_equations_keep_real_operations_and_all_refusal_cases() {
    let program = guarded_store_program_v90();
    assert!(program.contains(super::super::byte_memory_v30::BYTE_MEMORY_V30));
    assert!(program.contains("spec fn byte_operation_90_0_v30("));
    assert!(program.contains("spec fn byte_operation_90_1_v30("));
    assert_eq!(GUARDED_STORE_EQUATIONS_V90.matches("proof fn ").count(), 5);
    for forbidden in [
        "assume(",
        "admit(",
        "external_body",
        "assume_specification",
        "verify-only",
    ] {
        assert!(!program.contains(forbidden));
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the installed root-owned pinned functional-refinement runtime"]
fn protected_guarded_byte_store_preserves_cfg_write_and_false_refusal_equations() {
    use crate::{CanonicalGeneratedVerusProofInputV3, FunctionalRefinementVerusRuntimeLeaseV1};
    use std::time::{Duration, Instant};
    let source = CanonicalGeneratedVerusProofInputV3::new(guarded_store_program_v90().into_bytes())
        .expect("complete actual guarded/ordinary byte model");
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open_pinned_contexts_v3(
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
    )
    .expect("actual pinned runtime");
    runtime.revalidate().expect("before guarded-store proof");
    let mut attempt = runtime.begin_attempt().expect("guarded-store attempt");
    let output = runtime
        .execute_generated_rust_verify(
            &mut attempt,
            &source,
            Instant::now() + Duration::from_secs(120),
            16 * 1024,
        )
        .expect("prove all actual guarded-store equations");
    runtime.revalidate().expect("after guarded-store proof");
    attempt.complete().expect("complete guarded-store attempt");
    crate::functional_refinement_receipt_v2::validate_proved_output(&output)
        .expect("all equations genuinely verified");
}
