use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BinaryOp, BlockId, CheckedBinaryOperator, Function, Kernel, LaunchDomain,
    LaunchExtent, Module, Operation, OperationKind, ScalarType, Signature, Terminator, Type,
    ValueDef, ValueId,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticLayoutIdentityV1, SemanticTargetDataLayoutV1};

fn native_arithmetic(scalar: ScalarType, op: BinaryOp) -> Module {
    let ty = Type::Scalar(scalar);
    let mut results = vec![ValueDef::new(ValueId(2), ty.clone())];
    if matches!(op, BinaryOp::Checked(_)) {
        results.push(ValueDef::new(ValueId(3), Type::BOOL));
    }
    let mut block = BasicBlock::new(BlockId(91));
    block.operations.push(Operation {
        results,
        kind: OperationKind::Binary {
            op,
            lhs: ValueId(0),
            rhs: ValueId(1),
        },
    });
    block.terminator = Some(Terminator::Return { values: vec![] });
    let mut module = Module::new("native-arithmetic-census");
    module.functions.push(Function::kernel_entry(
        "root",
        Signature::new(vec![ty.clone(), ty], vec![]),
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

fn census_component(scalar: ScalarType, op: BinaryOp, admitted: bool) {
    let module = native_arithmetic(scalar, op);
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 16 * 1024 * 1024);
    const FLOOR: usize = 19;
    budget.reserve_storage(FLOOR).unwrap();
    let (owner, owner_storage) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&module, &mut budget).unwrap();
    budget
        .reserve_storage(owner_storage.retained_storage())
        .unwrap();
    let (inventory, inventory_storage) =
        CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget
        .reserve_storage(inventory_storage.retained_storage())
        .unwrap();
    let floor = budget.storage();
    {
        let private = private_memory::check(&inventory, 1024, &mut budget).unwrap();
        let division = unsigned_division::check(
            &inventory,
            SemanticTargetDataLayoutV1::gfx942(SemanticLayoutIdentityV1::from_sha256([250; 32])),
            &mut budget,
        )
        .unwrap();
        let helpers = scalar_helpers::check(&inventory, &mut budget).unwrap();
        let result = census::native(
            &inventory,
            &private,
            &division,
            &helpers,
            "test native grammar",
            |_, _| Ok(false),
            &mut budget,
        );
        if admitted {
            result.unwrap();
        } else {
            assert!(
                matches!(
                    result,
                    Err(E::Unsupported {
                        phase: "test native grammar",
                        detail: "closed opcode census",
                    })
                ),
                "only the exact native arithmetic grammar is under test: {result:?}"
            );
        }
    }
    // This is a grammar component, not a consuming source-origin proof. The
    // caller owns the same scratch-release boundary as the production parent.
    budget.release_storage(budget.storage() - floor).unwrap();
    assert_eq!(budget.storage(), floor);
    drop(inventory);
    budget
        .release_storage(inventory_storage.retained_storage())
        .unwrap();
    drop(owner);
    budget
        .release_storage(owner_storage.retained_storage())
        .unwrap();
    assert_eq!(budget.storage(), FLOOR);
}

#[test]
fn native_plain_integer_add_subtract_multiply_remain_outside_the_closed_census() {
    for scalar in [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::Index,
    ] {
        for op in [BinaryOp::Add, BinaryOp::Subtract, BinaryOp::Multiply] {
            census_component(scalar, op, false);
        }
    }
}

#[test]
fn native_checked_integer_pairs_remain_distinct_from_plain_integer_recipes() {
    for scalar in [
        ScalarType::I8,
        ScalarType::U8,
        ScalarType::I16,
        ScalarType::U16,
        ScalarType::I32,
        ScalarType::U32,
        ScalarType::I64,
        ScalarType::U64,
        ScalarType::Index,
    ] {
        for op in [
            CheckedBinaryOperator::Add,
            CheckedBinaryOperator::Subtract,
            CheckedBinaryOperator::Multiply,
        ] {
            census_component(scalar, BinaryOp::Checked(op), true);
        }
    }
}

#[test]
fn native_float_total_arithmetic_is_not_accidentally_closed_by_integer_refusals() {
    for scalar in [ScalarType::F32, ScalarType::F64] {
        for op in [BinaryOp::Add, BinaryOp::Subtract, BinaryOp::Multiply] {
            census_component(scalar, op, true);
        }
    }
}
