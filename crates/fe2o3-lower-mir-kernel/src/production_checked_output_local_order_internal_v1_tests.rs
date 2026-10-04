//! Pure resolver/accounting boundaries, not production source admission.
use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CanonicalKernelIrWorkBudgetV1 as Work,
    CanonicalKirBlockCoordinateV1, CanonicalKirDefinitionCoordinateV1,
    CanonicalKirFunctionCoordinateV1, CanonicalKirOperationTransitionV1, Function, Module,
    Operation, ScalarType, Signature, Terminator, Type, ValueDef, ValueId,
};
const LIMIT: usize = 20_000_000;

fn coordinate(operation: u32) -> CanonicalKirOperationCoordinateV1 {
    CanonicalKirOperationCoordinateV1 {
        block: CanonicalKirBlockCoordinateV1 {
            function: CanonicalKirFunctionCoordinateV1(0),
            block: 0,
        },
        operation,
    }
}
fn model() -> StoreOwner {
    let mut block = BasicBlock::new(BlockId(19));
    block.operations = [
        (4, BinaryOp::BitXor, 0, 1),
        (5, BinaryOp::BitOr, 2, 3),
        (6, BinaryOp::BitAnd, 4, 5),
    ]
    .into_iter()
    .map(|(id, op, lhs, rhs)| {
        Operation::effect_free(
            ValueDef::new(ValueId(id), Type::Scalar(ScalarType::U32)),
            OperationKind::Binary {
                op,
                lhs: ValueId(lhs),
                rhs: ValueId(rhs),
            },
        )
    })
    .collect();
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(6)],
    });
    let mut module = Module::new("local-order-origin-model");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(
            vec![Type::Scalar(ScalarType::U32); 4],
            vec![Type::Scalar(ScalarType::U32)],
        ),
        (0..4).map(ValueId).collect(),
        vec![block],
    ));
    let mut work = Work::new(LIMIT);
    let mut budget = AssertOriginBudgetV1::new(&mut work, LIMIT);
    StoreOwner::from_module_ref_with_verification_budget_v12(&module, &mut budget)
        .unwrap()
        .0
}
#[test]
fn source_local_order_resolver_refuses_lost_duplicate_synthesized_and_wrong_output_rows() {
    let owner = model();
    let selected = [coordinate(0), coordinate(1), coordinate(2)];
    let mut work = Work::new(LIMIT);
    let mut budget = AssertOriginBudgetV1::new(&mut work, LIMIT);
    let (inventory, storage) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let rows = selected.map(|c| CanonicalKirOperationTransitionV1 {
        output: c,
        origin: CanonicalKirOperationOriginV1::Retained(c),
    });
    assert_eq!(
        origins::diamond(&inventory, selected, &mut budget).unwrap(),
        [0, 1, 2, 3]
    );
    assert_eq!(
        origins::retained(&inventory, &inventory, selected, &rows, &mut budget).unwrap(),
        selected
    );
    for case in 0..4 {
        let mut bad = rows.to_vec();
        match case {
            0 => {
                bad.pop();
            }
            1 => bad[1].origin = bad[0].origin,
            2 => {
                bad[0].origin = CanonicalKirOperationOriginV1::ConstantFrom(
                    CanonicalKirDefinitionCoordinateV1::Result {
                        operation: selected[0],
                        result: 0,
                    },
                )
            }
            _ => bad[1].output = bad[0].output,
        }
        assert!(origins::retained(&inventory, &inventory, selected, &bad, &mut budget).is_err());
    }
}
#[test]
fn source_local_order_scope_unwind_restores_own_storage_and_not_the_callers_floor() {
    let mut work = Work::new(LIMIT);
    let mut budget = AssertOriginBudgetV1::new(&mut work, LIMIT);
    budget.reserve_storage(17).unwrap();
    let ledger = budget.work_ledger_identity_v1();
    let result: CResult<()> = scoped(17, &mut budget, |budget, _| {
        budget.charge_work(13)?;
        budget.reserve_storage(31)?;
        panic!("closed scope test");
    });
    assert!(matches!(result, Err(CError::Panicked)));
    assert_eq!(budget.storage(), 17);
    assert_eq!(budget.work(), 13);
    assert!(budget.work_ledger_identity_v1() == ledger);
    let result: CResult<()> = scoped(18, &mut budget, |_, _| unreachable!());
    assert!(matches!(
        result,
        Err(CError::Resource(AssertOriginResourceV1::Accounting))
    ));
    assert_eq!(budget.storage(), 17);
}

#[test]
fn source_local_order_profile_only_allows_one_body_and_the_exact_assertion_declaration() {
    use fe2o3_kernel_ir::{
        AmdGpuDiagnosticOperation, FunctionRole, Kernel, LaunchDomain, LaunchExtent, WorkgroupSize,
    };
    let mut base = model().module().clone();
    base.functions[0].role = FunctionRole::KernelEntry;
    base.functions[0].signature.results.clear();
    base.functions[0].body.as_mut().unwrap().blocks[0].terminator =
        Some(Terminator::Return { values: vec![] });
    let mut kernel = Kernel::new(
        "f",
        "f",
        LaunchDomain::D1 {
            x: LaunchExtent::Static(64),
        },
    );
    kernel.workgroup_size = Some(WorkgroupSize::new(64, 1, 1));
    base.kernels.push(kernel);
    let mut work = Work::new(LIMIT);
    let mut budget = AssertOriginBudgetV1::new(&mut work, LIMIT);
    origins::module_profile(&base, &mut budget).unwrap();
    base.functions
        .push(AmdGpuDiagnosticOperation::Trap.declaration());
    origins::module_profile(&base, &mut budget).unwrap();
    for case in 0..6 {
        let mut bad = base.clone();
        match case {
            0 => bad
                .functions
                .push(AmdGpuDiagnosticOperation::Trap.declaration()),
            1 => bad.functions[1] = AmdGpuDiagnosticOperation::DebugTrap.declaration(),
            2 => bad.functions[1].body = bad.functions[0].body.clone(),
            3 => bad.functions[1].role = FunctionRole::InternalHelper,
            4 => bad.functions[1]
                .signature
                .parameters
                .push(Type::Scalar(ScalarType::U32)),
            _ => bad.functions[0].role = FunctionRole::InternalHelper,
        }
        assert!(origins::module_profile(&bad, &mut budget).is_err());
    }
}
