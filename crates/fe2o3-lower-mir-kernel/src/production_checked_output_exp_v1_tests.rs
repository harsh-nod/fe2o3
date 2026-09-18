use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, FunctionId, Kernel, LaunchDomain,
    LaunchExtent, Module, Signature, Terminator, ValueId,
    VerifiedCanonicalKernelIrModuleV12 as Owner, VerifiedCanonicalKernelIrV12,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticLayoutIdentityV1, SemanticTargetDataLayoutV1};

fn math(function: F32MathFunction) -> FloatOperation {
    FloatOperation::F32Math {
        function,
        implementation: function.required_implementation(),
        arguments: vec![ValueId(0)],
    }
}

fn module(function: F32MathFunction, helper: bool) -> Module {
    let operation = math(function);
    let mut block = BasicBlock::new(BlockId(7));
    block.operations.push(operation.operation(ValueId(2)));
    block.terminator = Some(Terminator::Return {
        values: if helper { vec![ValueId(2)] } else { vec![] },
    });
    let mut module = Module::new("exact-exp-census");
    module.functions.push(operation.declaration());
    if helper {
        module.functions.push(Function::internal_helper(
            "helper",
            Signature::new(vec![Type::F32, Type::F32], vec![Type::F32]),
            vec![ValueId(0), ValueId(1)],
            vec![block],
        ));
        let mut entry = BasicBlock::new(BlockId(9));
        entry.operations.push(Operation::effect_free(
            fe2o3_kernel_ir::ValueDef::new(ValueId(2), Type::F32),
            OperationKind::Call {
                callee: FunctionId::new("helper"),
                arguments: vec![ValueId(0), ValueId(1)],
            },
        ));
        entry.terminator = Some(Terminator::Return { values: vec![] });
        block = entry;
    }
    module.functions.push(Function::kernel_entry(
        "root",
        Signature::new(vec![Type::F32, Type::F32], vec![]),
        vec![ValueId(0), ValueId(1)],
        vec![block],
    ));
    module.kernels.push(Kernel::new(
        "root",
        "root",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(1),
        },
    ));
    module
}

fn with_inventory(
    module: Module,
    next: impl FnOnce(&CanonicalKirInventoryV1<'_>, &mut AssertOriginBudgetV1<'_>),
) {
    let mut work = Work::new(10_000_000);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 16 << 20);
    budget.reserve_storage(19).unwrap();
    let (owner, storage) =
        Owner::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (inventory, receipt) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(receipt.retained_storage()).unwrap();
    let floor = budget.storage();
    next(&inventory, &mut budget);
    budget.release_storage(budget.storage() - floor).unwrap();
    drop(inventory);
    budget.release_storage(receipt.retained_storage()).unwrap();
    drop(owner);
    budget.release_storage(storage.retained_storage()).unwrap();
    assert_eq!(budget.storage(), 19);
}

#[test]
fn verified_exp_calls_pass_the_actual_root_and_helper_censuses() {
    for helper in [false, true] {
        with_inventory(module(F32MathFunction::Exp, helper), |inventory, budget| {
            let private = private_memory::check(inventory, 1024, budget).unwrap();
            let division = unsigned_division::check(
                inventory,
                SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256(
                    [250; 32],
                )),
                budget,
            )
            .unwrap();
            let helpers = scalar_helpers::check(inventory, budget).unwrap();
            census::native(
                inventory,
                &private,
                &division,
                &helpers,
                "exp test",
                |_, _| Ok(false),
                budget,
            )
            .unwrap();
            let external = inventory
                .functions()
                .iter()
                .find(|row| row.function.role == FunctionRole::ExternalImport)
                .unwrap();
            assert!(declaration(external.function, budget).unwrap());
            let row = inventory.operations().iter().find(|row| {
                matches!(&row.operation.kind, OperationKind::Call { callee, .. } if callee == &math(F32MathFunction::Exp).intrinsic_function_id())
            }).unwrap();
            assert!(call(row.operation, budget).unwrap());
            assert!(row.effects.is_empty() && row.compiler_ordering().is_empty());
        });
    }
}

#[test]
fn exact_descriptor_and_call_queries_are_prepaid_allocation_free_and_bounded() {
    let operation = math(F32MathFunction::Exp);
    let callee = operation.intrinsic_function_id();
    // The existing closed descriptor roster has 27 rows. This literal schedule
    // changes only if its independently reviewed roster or this query changes.
    let descriptor_work = 1 + 27 * (callee.as_str().len() + 2);
    for (which, overhead) in [(0, 0), (1, 4), (2, 2)] {
        let required = descriptor_work + overhead;
        for limit in [required, required - 1] {
            let mut work = Work::new(limit);
            let mut budget = AssertOriginBudgetV1::new(&mut work, 19);
            budget.reserve_storage(19).unwrap();
            let result = match which {
                0 => FloatOperation::f32_math_descriptor_with_budget_v1(&callee, &mut budget)
                    .map(|value| {
                        value == Some((F32MathFunction::Exp, F32MathImplementation::OcmlAbiV1))
                    })
                    .map_err(E::Resource),
                1 => declaration(&operation.declaration(), &mut budget),
                _ => call(&operation.operation(ValueId(2)), &mut budget),
            };
            assert_eq!(budget.storage(), 19);
            assert_eq!(budget.peak_storage(), 19);
            if limit == required {
                assert!(result.unwrap());
                assert_eq!(budget.work(), required);
            } else {
                assert!(matches!(
                    result,
                    Err(E::Resource(AssertOriginResourceV1::Work(_)))
                ));
                assert_eq!(budget.work(), overhead + 1);
            }
        }
    }
}

#[test]
fn other_valid_reserved_math_does_not_acquire_exp_admission() {
    for function in [
        F32MathFunction::Exp2,
        F32MathFunction::Sqrt,
        F32MathFunction::Abs,
    ] {
        with_inventory(module(function, false), |inventory, budget| {
            assert!(!declaration(inventory.functions()[0].function, budget).unwrap());
            assert!(!call(inventory.operations()[0].operation, budget).unwrap());
        });
    }
    let mut work = Work::new(100_000);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 0);
    for name in ["__ocml_exp_f32", "exp", "__fe2o3_ir_float_v1_exp_f32_extra"] {
        assert_eq!(
            FloatOperation::f32_math_descriptor_with_budget_v1(&FunctionId::new(name), &mut budget)
                .unwrap(),
            None
        );
    }
}

#[test]
fn malformed_signature_operand_contract_and_capabilities_fail_v12_first() {
    for mutation in 0..6 {
        let mut candidate = module(F32MathFunction::Exp, false);
        match mutation {
            0 => candidate.functions[0].signature.parameters[0] = Type::F64,
            1 => {
                candidate.functions[0].signature.results[0] =
                    Type::Scalar(fe2o3_kernel_ir::ScalarType::U32)
            }
            2 => {
                candidate.functions[0]
                    .required_capabilities
                    .insert(fe2o3_kernel_ir::TargetCapability::Float16);
            }
            3 => {
                let OperationKind::Call { arguments, .. } =
                    &mut candidate.functions[1].body.as_mut().unwrap().blocks[0].operations[0].kind
                else {
                    unreachable!()
                };
                arguments.clear();
            }
            4 => {
                candidate.functions[1].body.as_mut().unwrap().blocks[0].operations[0].results[0]
                    .ty = Type::F64
            }
            _ => {
                let bad = FloatOperation::F32Math {
                    function: F32MathFunction::Exp,
                    implementation: F32MathImplementation::ConstrainedLlvm,
                    arguments: vec![ValueId(0)],
                };
                candidate.functions[0] = bad.declaration();
                candidate.functions[1].body.as_mut().unwrap().blocks[0].operations[0] =
                    bad.operation(ValueId(2));
            }
        }
        assert!(
            VerifiedCanonicalKernelIrV12::from_module(candidate).is_err(),
            "mutation {mutation}"
        );
    }
}
