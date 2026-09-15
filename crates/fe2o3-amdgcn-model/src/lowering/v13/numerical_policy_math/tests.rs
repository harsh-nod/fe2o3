use super::*;
use fe2o3_kernel_ir::{F32MathFunction, VerifiedCanonicalKernelIrV13};

mod fixture {
    #![allow(dead_code, unnameable_test_items)]
    include!("../../../../../fe2o3-kernel-ir/tests/execution_capability_catalog_v13.rs");

    pub(super) fn cases() -> Vec<Module> {
        catalog()
            .into_iter()
            .filter(|operation| {
                matches!(
                    operation,
                    ExecutionCapabilityOperationV1::NumericalPolicyMath(_)
                )
            })
            .map(|operation| module_for(operation, 0))
            .collect()
    }
}

fn contract(operation: &Operation) -> &ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &operation.kind else {
        panic!("execution operation");
    };
    contract
}

fn contract_mut(operation: &mut Operation) -> &mut ExecutionCapabilityOpV1 {
    let OperationKind::ExecutionCapability(contract) = &mut operation.kind else {
        panic!("execution operation");
    };
    contract
}

fn fma_module() -> Module {
    fixture::cases()
        .into_iter()
        .find(|module| {
            matches!(
                contract(
                    module.functions[0].body.as_ref().unwrap().blocks[0]
                        .operations
                        .last()
                        .unwrap()
                )
                .operation,
                ExecutionCapabilityOperationV1::NumericalPolicyMath(Math::F32 {
                    function: F32MathFunction::FusedMultiplyAdd,
                    ..
                })
            )
        })
        .unwrap()
}

fn operations_mut(module: &mut Module) -> &mut Vec<Operation> {
    &mut module.functions[0].body.as_mut().unwrap().blocks[0].operations
}

fn emit(plan: &Plan<'_>) -> (Vec<Operation>, BTreeMap<ValueId, ExecutionAliasV1>) {
    let mut output = Vec::new();
    let mut aliases = BTreeMap::new();
    let body = plan.module.functions[0].body.as_ref().unwrap();
    for block in &body.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            if let OperationKind::ExecutionCapability(contract) = &operation.kind {
                if matches!(
                    contract.operation,
                    ExecutionCapabilityOperationV1::NumericalPolicyMath(_)
                ) {
                    plan.lower(
                        plan.module,
                        0,
                        (block.id, index),
                        operation,
                        contract,
                        &mut aliases,
                        &mut output,
                    )
                    .unwrap();
                }
            }
        }
    }
    (output, aliases)
}

#[test]
fn policy_math_all_thirteen_exact_float_projections_and_unused_constructors() {
    let mut functions = BTreeSet::new();
    let mut constructors = 0;
    for module in fixture::cases() {
        VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
        let original = module.clone();
        let plan = Plan::new(&module, 0, &value_types(&module.functions[0])).unwrap();
        let expected = plan
            .entries
            .iter()
            .filter_map(|entry| {
                let OperationKind::ExecutionCapability(contract) = &entry.operation.kind else {
                    return None;
                };
                let ExecutionCapabilityOperationV1::NumericalPolicyMath(Math::F32 {
                    function, ..
                }) = contract.operation
                else {
                    return None;
                };
                functions.insert(function);
                Some(
                    FloatOperation::F32Math {
                        function,
                        implementation: function.required_implementation(),
                        arguments: contract.operands[1..].to_vec(),
                    }
                    .operation(entry.operation.results[0].id),
                )
            })
            .collect::<Vec<_>>();
        let (output, aliases) = emit(&plan);
        assert_eq!(output, expected);
        if output.is_empty() {
            constructors += 1;
        }
        assert!(
            aliases
                .values()
                .all(|alias| *alias == ExecutionAliasV1::Erased)
        );
        assert_eq!(module, original);
    }
    assert_eq!(functions.len(), 13);
    assert_eq!(constructors, 2);
}

#[test]
fn policy_math_projection_matches_function_site_and_full_original_operation() {
    let module = fma_module();
    let types = value_types(&module.functions[0]);
    let plan = Plan::new(&module, 0, &types).unwrap();
    let body = module.functions[0].body.as_ref().unwrap();
    let block = &body.blocks[0];
    let index = block.operations.len() - 1;
    let operation = &block.operations[index];
    let mut changed = operation.clone();
    contract_mut(&mut changed).operands.swap(1, 2);
    let foreign = module.clone();
    for (owner, function, site, candidate) in [
        (&foreign, 0, (block.id, index), operation),
        (&module, 1, (block.id, index), operation),
        (&module, 0, (BlockId(91), index), operation),
        (&module, 0, (block.id, index + 1), operation),
        (&module, 0, (block.id, index), &changed),
    ] {
        let mut output = vec![operation.clone()];
        let mut aliases = BTreeMap::from([(ValueId(800), ExecutionAliasV1::Erased)]);
        let before_output = output.clone();
        let before_aliases = aliases.clone();
        let error = plan
            .lower(
                owner,
                function,
                site,
                candidate,
                contract(candidate),
                &mut aliases,
                &mut output,
            )
            .unwrap_err();
        assert_eq!(
            error.diagnostics()[0].message,
            "policy-math projection does not match its original function/site/operation"
        );
        assert_eq!(output, before_output);
        assert_eq!(aliases, before_aliases);
    }
}

#[test]
fn policy_math_contract_argument_cannot_substitute_an_approved_operation() {
    let module = fma_module();
    let plan = Plan::new(&module, 0, &value_types(&module.functions[0])).unwrap();
    let entry = plan.entries.last().unwrap();
    let mut changed = contract(entry.operation).clone();
    changed.source.operation = [0xfe; 32];
    let mut output = Vec::new();
    let mut aliases = BTreeMap::new();
    assert!(
        plan.lower(
            &module,
            0,
            entry.site,
            entry.operation,
            &changed,
            &mut aliases,
            &mut output
        )
        .is_err()
    );
    assert!(output.is_empty());
    assert!(aliases.is_empty());
}

fn token_ids(module: &Module) -> Vec<ValueId> {
    module.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .iter()
        .filter_map(|op| {
            matches!(
                op.kind,
                OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                    operation: ExecutionCapabilityOperationV1::NumericalPolicyIssue { .. }
                        | ExecutionCapabilityOperationV1::NumericalPolicyMath(
                            Math::MathDerive { .. } | Math::Bind { .. }
                        ),
                    ..
                })
            )
            .then(|| op.results[0].id)
        })
        .collect()
}

#[test]
fn policy_math_unknown_call_and_memory_use_reject_each_logical_role() {
    let original = fma_module();
    for token in token_ids(&original) {
        for kind in [
            OperationKind::Call {
                callee: FunctionId::new("unknown_consumer"),
                arguments: vec![token],
            },
            OperationKind::Store {
                pointer: ValueId(3),
                value: token,
                access: MemoryAccess::new(fe2o3_kernel_ir::AddressSpace::Global, 4),
            },
        ] {
            let mut module = original.clone();
            operations_mut(&mut module).push(Operation::new(vec![], kind));
            let error = Plan::new(&module, 0, &value_types(&module.functions[0]))
                .err()
                .unwrap();
            assert_eq!(
                error.diagnostics()[0].message,
                "policy-math logical token has an unapproved operation or terminator use"
            );
        }
    }
}

#[test]
fn policy_math_terminators_cannot_transport_logical_tokens() {
    let original = fma_module();
    for token in token_ids(&original) {
        for terminator in [
            Terminator::Return {
                values: vec![token],
            },
            Terminator::Branch {
                target: BlockId(0),
                arguments: vec![token],
            },
            Terminator::ConditionalBranch {
                condition: token,
                then_target: BlockId(0),
                then_arguments: vec![],
                else_target: BlockId(0),
                else_arguments: vec![],
            },
            Terminator::Switch {
                selector: ValueId(0),
                cases: vec![fe2o3_kernel_ir::SwitchCase {
                    value: 0,
                    target: BlockId(0),
                    arguments: vec![token],
                }],
                default_target: BlockId(0),
                default_arguments: vec![],
            },
            Terminator::IntegerSwitch {
                selector: ValueId(0),
                cases: vec![],
                default_target: BlockId(0),
                default_arguments: vec![token],
            },
        ] {
            let mut module = original.clone();
            module.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(terminator);
            let error = Plan::new(&module, 0, &value_types(&module.functions[0]))
                .err()
                .unwrap();
            assert_eq!(
                error.diagnostics()[0].message,
                "policy-math logical token has an unapproved operation or terminator use"
            );
        }
    }
}

#[test]
fn policy_math_missing_wrong_and_foreign_issuer_edges_reject() {
    for mutation in 0..5 {
        let mut module = fma_module();
        let operations = operations_mut(&mut module);
        let target = operations.len() - 1;
        match mutation {
            0 => contract_mut(&mut operations[target]).operands[0] = ValueId(u32::MAX),
            1 => {
                let policy =
                    operations
                        .iter()
                        .find(|op| {
                            matches!(
                                op.kind,
                                OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                                    operation:
                                        ExecutionCapabilityOperationV1::NumericalPolicyIssue { .. },
                                    ..
                                })
                            )
                        })
                        .unwrap()
                        .results[0]
                        .id;
                contract_mut(&mut operations[target]).operands[0] = policy;
            }
            2 => contract_mut(&mut operations[target]).provenance.issuance = [0xfb; 32],
            3 => {
                let _ = contract_mut(&mut operations[target]).operands.pop();
            }
            4 => operations[target].results[0].ty = Type::Scalar(ScalarType::I32),
            _ => unreachable!(),
        }
        let error = Plan::new(&module, 0, &value_types(&module.functions[0]))
            .err()
            .unwrap();
        assert_eq!(
            error.diagnostics()[0].message,
            "policy-math exact issuer/type/custody relation changed"
        );
        assert!(VerifiedCanonicalKernelIrV13::from_module(module).is_err());
    }
}

fn cross_block(mut module: Module) -> Module {
    let body = module.functions[0].body.as_mut().unwrap();
    let mut producer = body.blocks.remove(0);
    let consumer_operation = producer.operations.pop().unwrap();
    producer.id = BlockId(2);
    producer.terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    let mut entry = BasicBlock::new(BlockId(0));
    entry.terminator = Some(Terminator::Branch {
        target: BlockId(2),
        arguments: vec![],
    });
    let mut consumer = BasicBlock::new(BlockId(1));
    consumer.operations.push(consumer_operation);
    consumer.terminator = Some(Terminator::Return { values: vec![] });
    body.blocks = vec![entry, consumer, producer];
    module
}

#[test]
fn policy_math_dominating_later_serialized_block_does_not_depend_on_erased_aliases() {
    let original = fma_module();
    let original_plan = Plan::new(&original, 0, &value_types(&original.functions[0])).unwrap();
    let expected = emit(&original_plan).0;
    let module = cross_block(original.clone());
    VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let plan = Plan::new(&module, 0, &value_types(&module.functions[0])).unwrap();
    assert_eq!(emit(&plan).0, expected);
    let mut invalid = module;
    invalid.functions[0].body.as_mut().unwrap().blocks[0].terminator = Some(Terminator::Branch {
        target: BlockId(1),
        arguments: vec![],
    });
    assert!(VerifiedCanonicalKernelIrV13::from_module(invalid).is_err());
}

#[test]
fn policy_math_multiple_binds_and_consumers_keep_the_complete_roster() {
    let mut module = fma_module();
    let operations = operations_mut(&mut module);
    let mut bind = operations
        .iter()
        .find(|op| {
            matches!(
                op.kind,
                OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                    operation: ExecutionCapabilityOperationV1::NumericalPolicyMath(
                        Math::Bind { .. }
                    ),
                    ..
                })
            )
        })
        .unwrap()
        .clone();
    bind.results[0].id = ValueId(1000);
    contract_mut(&mut bind).source.operation = [0xa1; 32];
    let mut consumer = operations.last().unwrap().clone();
    consumer.results[0].id = ValueId(1001);
    contract_mut(&mut consumer).operands[0] = ValueId(1000);
    contract_mut(&mut consumer).source.operation = [0xa2; 32];
    operations.extend([bind, consumer.clone()]);
    consumer.results[0].id = ValueId(1002);
    contract_mut(&mut consumer).source.operation = [0xa3; 32];
    operations.push(consumer);
    VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let plan = Plan::new(&module, 0, &value_types(&module.functions[0])).unwrap();
    let output = emit(&plan).0;
    assert_eq!(output.len(), 3);
    assert_eq!(output[1].results[0].id, ValueId(1001));
    assert_eq!(output[2].results[0].id, ValueId(1002));
}

#[test]
fn policy_math_key_storage_accounts_sparse_ids_and_checked_overflow() {
    let mut module = fma_module();
    let operations = operations_mut(&mut module);
    operations.last_mut().unwrap().results[0].id = ValueId(u32::MAX - 1);
    let plan = Plan::new(&module, 0, &value_types(&module.functions[0])).unwrap();
    assert_eq!(plan.entries.len(), plan.keys.len());
    assert_eq!(plan.entries.len(), 5); // Context, getter, policy, bind, consumer.
    assert_eq!(
        storage_bytes(&module, 5, MAX_MODULE_BYTES_V1).unwrap(),
        size_of::<Plan<'_>>() + 5 * (size_of::<Entry<'_>>() + size_of::<Key>())
    );
    assert!(storage_bytes(&module, usize::MAX, usize::MAX).is_err());
    assert!(storage_bytes(&module, 6, 5).is_err());
    assert_eq!(emit(&plan).0[0].results[0].id, ValueId(u32::MAX - 1));
}

#[test]
fn policy_math_source_budget_exact_boundary_and_no_partial_charge() {
    let module = fma_module();
    let types = value_types(&module.functions[0]);
    let body = module.functions[0].body.as_ref().unwrap();
    let units = body
        .blocks
        .iter()
        .map(|block| {
            1 + block
                .operations
                .iter()
                .map(|op| 1 + op.results.len() + op.operands().len())
                .sum::<usize>()
                + block
                    .terminator
                    .as_ref()
                    .map_or(0, |term| 1 + term.operands().len())
        })
        .sum::<usize>();
    assert!(Plan::with_source_limit(&module, 0, &types, units).is_ok());
    let error = Plan::with_source_limit(&module, 0, &types, units - 1)
        .err()
        .unwrap();
    assert_eq!(
        error.diagnostics()[0].code,
        LoweringDiagnosticCode::ResourceLimit
    );
    let mut used = usize::MAX - 1;
    assert!(charge(&module, &mut used, 2, usize::MAX).is_err());
    assert_eq!(used, usize::MAX - 1);
}

#[test]
fn policy_math_duplicate_definition_and_result_roster_fail_before_emission() {
    for extra_result in [false, true] {
        let mut module = fma_module();
        let operations = operations_mut(&mut module);
        let result = operations.last().unwrap().results[0].clone();
        if extra_result {
            operations.last_mut().unwrap().results.push(result);
        } else {
            let duplicate = operations.last().unwrap().clone();
            operations.push(duplicate);
        }
        let error = Plan::new(&module, 0, &value_types(&module.functions[0]))
            .err()
            .unwrap();
        assert_eq!(
            error.diagnostics()[0].message,
            if extra_result {
                "policy-math issuer/result roster changed"
            } else {
                "policy-math issuer value is duplicated"
            }
        );
    }
}

#[test]
fn policy_math_borrowed_operand_visitors_match_canonical_enumeration() {
    for module in fixture::cases() {
        for block in &module.functions[0].body.as_ref().unwrap().blocks {
            for operation in &block.operations {
                let mut actual = Vec::new();
                operation_uses(operation, |value| {
                    actual.push(value);
                    Ok(())
                })
                .unwrap();
                assert_eq!(actual, operation.operands());
            }
            let mut actual = Vec::new();
            let terminator = block.terminator.as_ref().unwrap();
            terminator_uses(terminator, |value| {
                actual.push(value);
                Ok(())
            })
            .unwrap();
            assert_eq!(actual, terminator.operands());
        }
    }
}

#[test]
fn policy_math_exact_float_declarations_deduplicate_and_reject_conflicts() {
    let module = fma_module();
    let expected = FloatOperation::F32Math {
        function: F32MathFunction::FusedMultiplyAdd,
        implementation: F32MathFunction::FusedMultiplyAdd.required_implementation(),
        arguments: Vec::new(),
    }
    .declaration();
    let mut helpers = Vec::new();
    append_declarations(&module, &mut helpers).unwrap();
    assert_eq!(helpers, [expected.clone()]);
    append_declarations(&module, &mut helpers).unwrap();
    assert_eq!(helpers, [expected.clone()]);
    let mut existing = module.clone();
    existing.functions.push(expected.clone());
    let mut helpers = Vec::new();
    append_declarations(&existing, &mut helpers).unwrap();
    assert!(helpers.is_empty());
    for duplicate in [false, true] {
        let mut bad = existing.clone();
        if duplicate {
            bad.functions.push(expected.clone());
        } else {
            bad.functions.last_mut().unwrap().signature.parameters[0] =
                Type::Scalar(ScalarType::I32);
        }
        let error = append_declarations(&bad, &mut helpers).unwrap_err();
        assert_eq!(
            error.diagnostics()[0].message,
            "policy-math reserved declaration does not match its exact float contract"
        );
        assert!(helpers.is_empty());
    }
    // Component resource boundary only; duplicate ordinary functions are not
    // claimed to be a canonical source module or a production capability owner.
    let mut full = module.clone();
    full.functions.resize(
        MAX_FUNCTIONS_V1,
        Function::declaration("component-only", Signature::new(vec![], vec![])),
    );
    let error = append_declarations(&full, &mut helpers).unwrap_err();
    assert_eq!(
        error.diagnostics()[0].code,
        LoweringDiagnosticCode::ResourceLimit
    );
    assert!(helpers.is_empty());
}

#[test]
fn policy_math_target_owner_guard_is_required_even_for_a_checked_consumer() {
    use fe2o3_amd_target::ProductionAmdCapabilityOwnerV1 as Owner;
    let module = fma_module();
    let operation = module.functions[0].body.as_ref().unwrap().blocks[0]
        .operations
        .last()
        .unwrap();
    let consumer_contract = contract(operation);
    let requirements =
        target_requirements_for_execution_operation_v1(&consumer_contract.operation).unwrap();
    assert_eq!(requirements.len(), 1);
    let numerical = *requirements.first().unwrap();
    for owner in [
        None,
        Some(Owner::WaveLowering),
        Some(Owner::ScalarMemoryLowering),
    ] {
        // Exercise the existing private owner guard, not a public certificate.
        let authority = V13LoweringAuthorityV1 {
            closure_identity: [1; 32],
            owners: owner.into_iter().map(|owner| (numerical, owner)).collect(),
        };
        let result = require_operation_closure(&module, &authority, consumer_contract);
        if owner == Some(Owner::ScalarMemoryLowering) {
            result.unwrap();
        } else {
            assert_eq!(
                result.unwrap_err().diagnostics()[0].code,
                LoweringDiagnosticCode::CapabilityClosureMismatch
            );
        }
    }
    for module in fixture::cases() {
        let operation = module.functions[0].body.as_ref().unwrap().blocks[0]
            .operations
            .last()
            .unwrap();
        let mut payload = contract(operation).operation.clone();
        let ExecutionCapabilityOperationV1::NumericalPolicyMath(math) = &mut payload else {
            unreachable!()
        };
        match math {
            Math::MathDerive { binding, .. }
            | Math::Bind { binding }
            | Math::F32 { binding, .. } => {
                binding.mode = NumericalModeV1::AllowContraction;
            }
        }
        assert!(target_requirements_for_execution_operation_v1(&payload).is_err());
    }
}

#[test]
fn policy_math_variable_width_assembly_and_switch_uses_are_not_omitted() {
    use fe2o3_kernel_ir::{
        AssemblyConstraint, AssemblyOperand, AssemblySourceIdentity, InlineAssembly,
        InlineAssemblyTarget,
    };
    let original = fma_module();
    for token in token_ids(&original) {
        let mut operands = (0..128)
            .map(|_| AssemblyOperand::input(ValueId(0), AssemblyConstraint::Sgpr32))
            .collect::<Vec<_>>();
        operands.push(AssemblyOperand {
            kind: AssemblyOperandKind::InOut {
                input: token,
                result_index: 0,
            },
            constraint: AssemblyConstraint::Sgpr32,
        });
        let operation = Operation::new(
            vec![],
            OperationKind::InlineAssembly(InlineAssembly {
                target: InlineAssemblyTarget::AmdGpuGfx942,
                source: AssemblySourceIdentity::new([1; 32], [2; 32], [3; 32], [4; 32]),
                mnemonic: "component-only-unapproved-use".into(),
                operands,
                options: BTreeSet::new(),
                declared_effects: BTreeSet::new(),
            }),
        );
        let mut observed = Vec::new();
        operation_uses(&operation, |value| {
            observed.push(value);
            Ok(())
        })
        .unwrap();
        assert_eq!(observed, operation.operands());
        let mut module = original.clone();
        operations_mut(&mut module).push(operation);
        let error = Plan::new(&module, 0, &value_types(&module.functions[0]))
            .err()
            .unwrap();
        assert_eq!(
            error.diagnostics()[0].message,
            "policy-math logical token has an unapproved operation or terminator use"
        );
        let term = Terminator::Switch {
            selector: ValueId(0),
            cases: vec![fe2o3_kernel_ir::SwitchCase {
                value: 0,
                target: BlockId(0),
                arguments: vec![ValueId(1); 128],
            }],
            default_target: BlockId(0),
            default_arguments: vec![token],
        };
        let mut observed = Vec::new();
        terminator_uses(&term, |value| {
            observed.push(value);
            Ok(())
        })
        .unwrap();
        assert_eq!(observed, term.operands());
    }
}

#[test]
fn policy_math_different_context_value_cannot_pair_equal_nominal_issuers() {
    let mut module = fma_module();
    let operations = operations_mut(&mut module);
    let mut context = operations
        .iter()
        .find(|op| matches!(op.kind, OperationKind::KernelContextIssue(_)))
        .unwrap()
        .clone();
    context.results[0].id = ValueId(1000);
    operations.insert(0, context);
    let policy = operations
        .iter_mut()
        .find(|op| {
            matches!(
                op.kind,
                OperationKind::ExecutionCapability(ExecutionCapabilityOpV1 {
                    operation: ExecutionCapabilityOperationV1::NumericalPolicyIssue { .. },
                    ..
                })
            )
        })
        .unwrap();
    contract_mut(policy).operands[0] = ValueId(1000);
    let error = Plan::new(&module, 0, &value_types(&module.functions[0]))
        .err()
        .unwrap();
    assert_eq!(
        error.diagnostics()[0].message,
        "policy-math exact issuer/type/custody relation changed"
    );
    assert!(VerifiedCanonicalKernelIrV13::from_module(module).is_err());
}

#[test]
fn policy_math_many_consumers_have_linear_index_storage_and_fixed_declarations() {
    let mut module = fma_module();
    let operations = operations_mut(&mut module);
    let original = operations.last().unwrap().clone();
    for index in 0..512u32 {
        let mut consumer = original.clone();
        consumer.results[0].id = ValueId(1000 + index);
        contract_mut(&mut consumer).source.operation[..4].copy_from_slice(&index.to_le_bytes());
        operations.push(consumer);
    }
    VerifiedCanonicalKernelIrV13::from_module(module.clone()).unwrap();
    let plan = Plan::new(&module, 0, &value_types(&module.functions[0])).unwrap();
    assert_eq!(plan.entries.len(), 517);
    assert_eq!(plan.keys.len(), 517);
    assert_eq!(
        storage_bytes(&module, plan.entries.len(), MAX_MODULE_BYTES_V1).unwrap(),
        size_of::<Plan<'_>>() + 517 * (size_of::<Entry<'_>>() + size_of::<Key>())
    );
    assert_eq!(emit(&plan).0.len(), 513);
    let mut helpers = Vec::new();
    append_declarations(&module, &mut helpers).unwrap();
    assert_eq!(helpers.len(), 1);
}
