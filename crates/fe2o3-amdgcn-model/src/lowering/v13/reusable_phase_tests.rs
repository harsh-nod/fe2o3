use super::*;
use fe2o3_kernel_ir::{CanonicalKernelIrVersionV1 as Version, ReusablePhaseCheckLimitsV1 as Limits};
#[path = "reusable_phase_tests/fixture.rs"]
mod fixtures;

struct Physical {
    operations: Vec<Operation>,
    aliases: BTreeMap<ValueId, ExecutionAliasV1>,
}

// Exercise the actual dispatchers at their original coordinates. Target
// closure legalization and LLVM emission are deliberately outside this test.
fn physical(module: &Module, version: Version, limits: Limits) -> Result<Physical, LoweringErrors> {
    reusable_phase::check_declared_version(module, version)?;
    let original = &module.functions[0];
    reject_execution_capability_parameters(module, original)?;
    let mut phase = reusable_phase::prepare(module, original, version, limits)?;
    let types = value_types(original);
    let elements = infer_element_types(module, original, &types)?;
    let policy = numerical_policy_math::Plan::new(module, 0, &types)?;
    let mut lowered = types.clone();
    let mut aliases = BTreeMap::new();
    let mut output = Vec::new();
    let mut helpers = Vec::new();
    let mut next = next_value_id(module, original)?;
    let mut wave = None;
    let body = original.body.as_ref().unwrap();
    let used = body.blocks.iter().flat_map(|b| b.operations.iter().flat_map(Operation::operands)
        .chain(b.terminator.iter().flat_map(Terminator::operands))).collect();
    for block in &body.blocks {
        for (index, operation) in block.operations.iter().enumerate() {
            match &operation.kind {
                OperationKind::ReusablePhase(_) => reusable_phase::lower(
                    module, version, phase.as_mut().expect("prepared"), original,
                    block.id, index, operation, &elements, &mut lowered, &mut aliases,
                )?,
                OperationKind::ExecutionCapability(contract) => lower_operation(
                    module, operation, contract, &types, &used, Some(&policy),
                    &elements, &mut lowered, &mut aliases, &mut next, &mut wave,
                    &mut output, &mut helpers, 0, block.id, index,
                )?,
                _ => {
                    reject_unlowered_execution_operand(module, operation, &aliases)?;
                    output.push(operation.clone());
                }
            }
        }
    }
    if let Some(phase) = phase {
        phase.finish().map_err(|e| incomplete(module, format!("phase physical census incomplete: {e:?}")))?;
    }
    assert!(helpers.is_empty());
    Ok(Physical { operations: output, aliases })
}

fn fixture(n: usize) -> Module { fixtures::memory(n) }
fn message(error: &LoweringErrors) -> &str {
    assert_eq!(error.diagnostics().len(), 1);
    &error.diagnostics()[0].message
}

#[test]
fn phase_backend_real_dispatch_keeps_one_allocation_and_exact_two_memory_generations() {
    for n in [1, 2] {
        let module = fixture(n);
        fe2o3_kernel_ir::verify_module(&module).unwrap();
        let bytes = fe2o3_kernel_ir::encode_module_v14(&module).unwrap();
        let p = physical(&module, Version::V14, Limits::DEFAULT).unwrap();
        let allocation = p.operations.iter().filter(|o| matches!(o.kind, OperationKind::WorkgroupMemory(_))).collect::<Vec<_>>();
        assert_eq!(allocation.len(), 1);
        assert_eq!(allocation[0].results[0].id, ValueId(80));
        assert!(matches!(&allocation[0].kind, OperationKind::WorkgroupMemory(m)
            if m.element == Type::F32 && m.extent == WorkgroupMemoryExtent::Static(64) && m.alignment == 4));
        assert_eq!(p.operations.iter().filter(|o| matches!(o.kind, OperationKind::Store { .. })).count(), n);
        assert_eq!(p.operations.iter().filter(|o| matches!(o.kind, OperationKind::Load { .. } | OperationKind::GuardedLoad { .. })).count(), n);
        assert_eq!(p.operations.iter().filter(|o| matches!(o.kind, OperationKind::WorkgroupBarrier(_))).count(), n * 2);
        assert!(p.operations.iter().all(|o| !matches!(o.kind,
            OperationKind::ReusablePhase(_) | OperationKind::ExecutionCapability(_))));
        for operation in &module.functions[0].body.as_ref().unwrap().blocks[0].operations {
            for result in &operation.results {
                if reusable_phase::is_phase_type(&result.ty)
                    || matches!(&result.ty, Type::ExecutionCapability(c) if c.role == ExecutionCapabilityRoleV1::Workgroup)
                {
                    assert!(matches!(p.aliases.get(&result.id), Some(ExecutionAliasV1::Erased)), "{result:?}");
                }
                if matches!(&operation.kind, OperationKind::ReusablePhase(_))
                    && matches!(&result.ty, Type::ExecutionCapability(c) if matches!(c.role,
                        ExecutionCapabilityRoleV1::Lds { .. } | ExecutionCapabilityRoleV1::ReusableLds { .. }))
                {
                    assert!(matches!(p.aliases.get(&result.id), Some(ExecutionAliasV1::Physical(p))
                        if p.value == ValueId(80) && p.extent == Some(PhysicalExtentV1::Static(64))));
                }
            }
        }
        assert_eq!(fe2o3_kernel_ir::encode_module_v14(&module).unwrap(), bytes);
    }
}

#[test]
fn phase_backend_v13_boundary_rejects_the_new_family_before_dispatch() {
    let module = fixture(2);
    let error = physical(&module, Version::V13, Limits::DEFAULT).err().unwrap();
    assert_eq!(message(&error), "V13 physical lowering cannot consume V14 phase custody");
}

#[test]
fn phase_backend_dispatch_requires_existing_physical_element_witness() {
    let module = fixtures::without_memory();
    fe2o3_kernel_ir::verify_module(&module).unwrap();
    let error = physical(&module, Version::V14, Limits::DEFAULT).err().unwrap();
    assert_eq!(message(&error), "V13 LDS element identity has no structural physical KIR type witness");
}

#[test]
fn phase_backend_dispatch_rejects_incomplete_end_and_original_budget_limits() {
    let mut module = fixture(2);
    module.functions[0].body.as_mut().unwrap().blocks[0].operations.retain(|o|
        !matches!(&o.kind, OperationKind::ReusablePhase(p)
            if matches!(p.operation, fe2o3_kernel_ir::ReusablePhaseOperationV1::End { .. })));
    let error = physical(&module, Version::V14, Limits::DEFAULT).err().unwrap();
    assert!(message(&error).starts_with("phase physical projection rejected:"), "{error}");
    for limits in [Limits {work: 0, ..Limits::DEFAULT}, Limits {temporary_bytes: 1, ..Limits::DEFAULT}] {
        let error = physical(&fixture(2), Version::V14, limits).err().unwrap();
        assert!(message(&error).starts_with("phase physical projection rejected:"), "{error}");
    }
}

#[test]
fn phase_backend_legacy_erase_and_alias_helpers_cannot_materialize_phase_authority() {
    let module = fixture(1);
    let operation = module.functions[0].body.as_ref().unwrap().blocks[0].operations.iter()
        .find(|o| matches!(o.kind, OperationKind::ReusablePhase(_))).unwrap();
    for alias in [false, true] {
        let mut aliases = BTreeMap::new();
        let error = if alias {
            alias_capability_results(&module, operation, PhysicalValueV1::scalar(ValueId(80)), &mut aliases)
        } else {
            erase_capability_results(&module, operation, &mut aliases)
        }.unwrap_err();
        assert_eq!(message(&error), "phase result lacks its exact lifecycle adapter");
        assert!(aliases.is_empty());
    }
}

#[test]
fn phase_backend_new_tokens_do_not_cross_unimplemented_physical_phi_or_function_abi() {
    for position in 0..3 {
        let mut module = fixture(1);
        let f = &mut module.functions[0];
        let token = f.body.as_ref().unwrap().blocks[0].operations.iter()
            .flat_map(|o| &o.results).find(|r| matches!(r.ty, Type::ReusablePhaseToken(_))).unwrap().ty.clone();
        match position {
            0 => f.signature.parameters.push(token),
            1 => f.signature.results.push(token),
            2 => f.body.as_mut().unwrap().blocks[0].parameters.push(ValueDef::new(ValueId(9000), token)),
            _ => unreachable!(),
        }
        let error = reject_execution_capability_parameters(&module, &module.functions[0]).unwrap_err();
        assert_eq!(message(&error), "V13 execution capabilities crossing function or block boundaries lack a physical ABI/phi lowering");
    }
}

#[test]
fn phase_backend_numerical_policy_visitor_retains_all_phase_operands_in_order() {
    let module = fixture(2);
    for operation in &module.functions[0].body.as_ref().unwrap().blocks[0].operations {
        if let OperationKind::ReusablePhase(contract) = &operation.kind {
            let mut actual = Vec::new();
            numerical_policy_math::test_phase_operands(operation, &mut actual).unwrap();
            assert_eq!(actual, contract.operands);
        }
    }
}

#[test]
fn phase_backend_nonphase_v13_lds_and_memory_view_keep_identical_physical_output() {
    for module in fixtures::legacy_modules() {
        fe2o3_kernel_ir::verify_module(&module).unwrap();
        let bytes = fe2o3_kernel_ir::encode_module_v13(&module).unwrap();
        let old = physical(&module, Version::V13, Limits::DEFAULT).unwrap();
        let declared = physical(&module, Version::V14, Limits::DEFAULT).unwrap();
        assert_eq!(old.operations, declared.operations);
        assert_eq!(old.aliases, declared.aliases);
        assert_eq!(old.operations.iter().filter(|o| matches!(o.kind, OperationKind::WorkgroupMemory(_))).count(), 1);
        assert!(old.operations.iter().any(|o| matches!(o.kind, OperationKind::WorkgroupBarrier(_))));
        assert_eq!(fe2o3_kernel_ir::encode_module_v13(&module).unwrap(), bytes);
    }
}

#[test]
fn phase_backend_module_work_does_not_restart_per_function() {
    let module = fixture(1);
    let f = &module.functions[0];
    let plan = reusable_phase::prepare(&module, f, Version::V14, Limits::DEFAULT).unwrap().unwrap();
    let used = plan.usage();
    assert!(used.work > 0);
    let mut budget = reusable_phase::ModuleBudget::new(Limits {work: used.work, ..Limits::DEFAULT});
    budget.consume(&module, used).unwrap();
    assert_eq!(budget.remaining().work, 0);
    let error = reusable_phase::prepare(&module, f, Version::V14, budget.remaining()).unwrap_err();
    assert!(message(&error).contains("WorkLimit"), "{error}");
    let error = budget.consume(&module, used).unwrap_err();
    assert_eq!(message(&error), "phase module physical work ceiling exceeded");
    let capped = reusable_phase::ModuleBudget::new(Limits {work: usize::MAX, temporary_bytes: usize::MAX});
    assert_eq!(capped.remaining(), Limits::DEFAULT);
}
