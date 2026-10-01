fn trap_header_oracle_v40() -> usize {
    use fe2o3_kernel_ir::AmdGpuDiagnosticOperation as Diagnostic;
    assert_eq!(size_of::<TrapByteOperationV40>(), 0);
    2 * size_of::<Result<Option<TrapByteOperationV40>>>()
        + 2 * size_of::<ByteMemoryStateNamesV30<'_>>()
        + size_of::<(Option<Diagnostic>, [&(); 5], [usize; 5], Result<usize>)>()
        + size_of::<([&str; 3], [usize; 4], Option<usize>, Result<()>)>()
}

fn trap_module_v40(count: usize) -> Module {
    use fe2o3_kernel_ir::{AmdGpuDiagnosticOperation as Diagnostic, SwitchCase};
    assert!(count > 0);
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Switch {
        selector: ValueId(0),
        cases: (0..count - 1)
            .map(|index| SwitchCase {
                value: index as u64,
                target: BlockId(index as u32 + 1),
                arguments: vec![],
            })
            .collect(),
        default_target: BlockId(count as u32),
        default_arguments: vec![],
    });
    let mut blocks = vec![entry];
    for index in 1..=count {
        let mut block = BasicBlock::new(BlockId(index as u32));
        block.operations.push(Diagnostic::Trap.operation(None));
        block.terminator = Some(Terminator::Unreachable);
        blocks.push(block);
    }
    let mut function = KirFunction::internal_helper(
        "terminal_traps",
        Signature::new(vec![Type::Scalar(ScalarType::U32)], vec![]),
        vec![ValueId(0)],
        blocks,
    );
    function.required_capabilities = Diagnostic::Trap.required_capabilities();
    let mut module = Module::new("general-actual-terminal-traps");
    module.functions = vec![function, Diagnostic::Trap.declaration()];
    module
}

#[test]
fn byte_trap_dispatch_preserves_exact_terminal_observation_and_machine_data() {
    for width in [FormalIndexWidth::Bits32, FormalIndexWidth::Bits64] {
        with_inventory(&trap_module_v40(3), |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let text = run(floor, LIMIT, LIMIT, |out| {
                let model = ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(width),
                    &allocations,
                    out,
                )?;
                assert_eq!(model.operations.len(), 3);
                assert!(
                    model
                        .operations
                        .iter()
                        .all(|plan| matches!(plan, ByteOperationV30::Trap(_)))
                );
                model.emit(143, out)
            })
            .0
            .unwrap();
            assert_eq!(text.matches("pc: if valid { -2 } else { s.pc }").count(), 3);
            assert_eq!(text.matches("if valid { MemoryOperationEffectV30::Trap } else { MemoryOperationEffectV30::Refused }").count(), 3);
            assert_eq!(text.matches("let values = s.values; let memory = s.memory; let generations = s.generations; let frames = s.frames; let valid = s.valid;").count(), 3);
            for block in 1..=3 {
                let operation = block - 1;
                let locator = format!(
                    "MemorySourceOperationV30 {{ function: 0, block: {block}, operation: 0 }}"
                );
                assert!(text.contains(&format!(
                    "byte_trap_terminal_v40(done, observations, {locator}, {block}, 1)"
                )));
                assert!(text.contains(&format!(
                    "byte_trap_terminal_v40(m.state, m.observations, {locator}, {block}, 1)"
                )));
                assert!(text.contains(&format!("byte_operation_143_{operation}_v30")));
            }
            assert!(text.contains("if trapped { MemoryBlockResultV30 { state: done, observations, returned: Seq::empty() } }"));
            assert!(text.contains("let trapped = false;"));
            assert_eq!(
                text.matches("byte_result_v55(s, state, operation, effect)")
                    .count(),
                3
            );
            assert!(
                include_str!("mixed_optimizer_byte_results_v55.vrs")
                    .contains("valid_before: before.valid, valid_after: after.valid, effect")
            );
            assert!(!text.contains("byte_pop_frame_v30("));
            assert!(!text.contains("byte_end_lifetime_v30("));
        });
    }
}

#[test]
fn byte_trap_terminal_guard_requires_exact_effect_locator_and_unchanged_snapshots() {
    let source = super::super::byte_memory_v30::BYTE_MEMORY_V30;
    let start = source.find("open spec fn byte_trap_terminal_v40(").unwrap();
    let body = source[start..].split_once("\n}\n").unwrap().0;
    for check in [
        "count > 0 && observations.len() == count && state.valid && state.pc == -2",
        "byte_observation_snapshots_valid_v39(last)",
        "last.operation == operation && last.effect == MemoryOperationEffectV30::Trap",
        "last.before.valid && last.before.pc == block && last.after == state",
        "last.before.values == state.values && last.before.memory == state.memory",
        "last.before.generations == state.generations && last.before.frames == state.frames",
    ] {
        assert!(body.contains(check), "missing trap terminal check: {check}");
    }
    assert!(!body.contains("pc < 0"));
}

#[test]
fn byte_trap_derivation_has_independent_linear_exact_and_one_short_budgets() {
    use fe2o3_kernel_ir::AmdGpuDiagnosticOperation as Diagnostic;
    for count in [1, 4, 16] {
        with_inventory(&trap_module_v40(count), |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let derive = |out: &mut Writer<'_, '_>| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(FormalIndexWidth::Bits64),
                    &allocations,
                    out,
                )
                .map(|_| ())
            };
            // Owner1 + body4 + definition1 + selector control8. Per trap:
            // dispatch4, Alloca6, Storage6, pointer26, INDEX2, trap9+lookup,
            // physical19, entry edge4 and final Unreachable control5.
            let work = 14
                + count * (81 + 8 * (Diagnostic::Trap.intrinsic_function_id().as_str().len() + 2));
            let storage = floor
                + super::super::super::SOURCE_LIMIT
                + headers::<NoAllocations<'_>>()
                + count * size_of::<ByteOperationV30<'_, '_>>();
            let exact = run(floor, work, storage, derive);
            assert!(exact.0.unwrap().is_empty());
            assert_eq!((exact.1, exact.2), (work, storage));
            assert!(matches!(run(floor, work - 1, storage, derive).0,
                Err(Error::Resource(Resource::Work(error))) if error.limit() == work - 1 && error.actual() == work));
            assert!(matches!(run(floor, work, storage - 1, derive).0,
                Err(Error::Resource(Resource::Storage(error))) if error.limit() == storage - 1 && error.actual() == storage));
            let emit = |out: &mut Writer<'_, '_>| {
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(FormalIndexWidth::Bits64),
                    &allocations,
                    out,
                )?
                .emit(144, out)
            };
            let measured = run(floor, LIMIT, LIMIT, emit);
            let text = measured.0.unwrap();
            assert_eq!(run(floor, measured.1, measured.2, emit).0.unwrap(), text);
            assert!(matches!(
                run(floor, measured.1 - 1, measured.2, emit).0,
                Err(Error::Resource(Resource::Work(_)))
            ));
            assert!(matches!(
                run(floor, measured.1, measured.2 - 1, emit).0,
                Err(Error::Resource(Resource::Storage(_)))
            ));
        });
    }
}

#[test]
fn byte_trap_adapter_refuses_other_diagnostics_and_unknown_calls() {
    use fe2o3_kernel_ir::{AmdGpuDiagnosticOperation as Diagnostic, FunctionId};
    for diagnostic in [Some(Diagnostic::DebugTrap), Some(Diagnostic::Clock32), None] {
        let mut module = trap_module_v40(1);
        let (operation, declaration) = match diagnostic {
            Some(ref diagnostic) => (
                diagnostic.operation(diagnostic.result_type().map(|_| ValueId(1))),
                diagnostic.declaration(),
            ),
            None => {
                let callee = FunctionId::new("unregistered_ordinary_call");
                (
                    KirOperation::new(
                        vec![],
                        OperationKind::Call {
                            callee: callee.clone(),
                            arguments: vec![],
                        },
                    ),
                    KirFunction::external_import(callee, Signature::new(vec![], vec![])),
                )
            }
        };
        module.functions[0].body.as_mut().unwrap().blocks[1].operations = vec![operation];
        module.functions[0].body.as_mut().unwrap().blocks[1].terminator =
            Some(Terminator::Return { values: vec![] });
        module.functions[1] = declaration;
        with_inventory(&module, |inventory, physical, floor| {
            let allocations = NoAllocations(inventory.owner());
            let result = run(floor, LIMIT, LIMIT, |out| {
                assert!(TrapByteOperationV40::derive(inventory, 0, out)?.is_none());
                ByteFunctionV30::derive(
                    inventory,
                    physical,
                    Function(0),
                    ByteContext::native(FormalIndexWidth::Bits64),
                    &allocations,
                    out,
                )?
                .emit(145, out)
            })
            .0;
            assert!(matches!(result, Err(Error::Statement(_))));
        });
    }
}

#[test]
fn byte_trap_malformed_reserved_declarations_or_continuations_never_enter_dispatch() {
    for fault in 0..7 {
        let mut module = trap_module_v40(1);
        match fault {
            0 => module.functions[1].required_capabilities.clear(),
            1 => module.functions[1].signature.parameters.push(Type::INDEX),
            2 => module.functions[1].role = fe2o3_kernel_ir::FunctionRole::InternalHelper,
            3 => {
                module.functions[0].body.as_mut().unwrap().blocks[1].terminator =
                    Some(Terminator::Return { values: vec![] })
            }
            4 => {
                let OperationKind::Call { arguments, .. } =
                    &mut module.functions[0].body.as_mut().unwrap().blocks[1].operations[0].kind
                else {
                    unreachable!()
                };
                arguments.push(ValueId(0));
            }
            5 => module.functions[0].body.as_mut().unwrap().blocks[1].operations[0]
                .results
                .push(ValueDef::new(ValueId(1), Type::INDEX)),
            6 => module.functions[0].body.as_mut().unwrap().blocks[1]
                .operations
                .push(KirOperation::effect_free(
                    ValueDef::new(ValueId(1), Type::Scalar(ScalarType::U32)),
                    OperationKind::Constant(fe2o3_kernel_ir::Constant::U32(0)),
                )),
            _ => unreachable!(),
        }
        let mut work = Work::new(LIMIT);
        let mut budget = Budget::new(&mut work, LIMIT);
        assert!(
            Owner::from_module_ref_with_verification_budget_v18(&module, LAYOUTS, &mut budget)
                .is_err(),
            "fault={fault}"
        );
        assert_eq!(budget.storage(), 0);
    }
}
