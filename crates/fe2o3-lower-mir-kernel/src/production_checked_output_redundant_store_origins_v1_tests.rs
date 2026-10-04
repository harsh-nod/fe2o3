// Inert transport controls only; source-owned acceptance is tested separately.
use super::*;
use fe2o3_kernel_ir::{
    BasicBlock, BlockId, Function, Module, Operation, OperationKind, ScalarType, Signature,
    Terminator, Type, UnaryOp, ValueDef, ValueId,
};

fn module() -> Module {
    let ty = Type::Scalar(ScalarType::U32);
    let mut block = BasicBlock::new(BlockId(9));
    block.operations = vec![
        Operation::effect_free(
            ValueDef::new(ValueId(1), ty.clone()),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(0),
            },
        ),
        Operation::effect_free(
            ValueDef::new(ValueId(2), ty.clone()),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand: ValueId(1),
            },
        ),
    ];
    block.terminator = Some(Terminator::Return {
        values: vec![ValueId(2)],
    });
    let mut module = Module::new("store-source-sites");
    module.functions.push(Function::internal_helper(
        "f",
        Signature::new(vec![ty.clone()], vec![ty]),
        vec![ValueId(0)],
        vec![block],
    ));
    module
}

#[test]
fn redundant_store_origin_transport_is_total_positional_and_does_not_source_synthetic_constants() {
    let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(10_000_000);
    let mut budget = AssertOriginBudgetV1::new(&mut work, 16 << 20);
    let (owner, storage) =
        StoreOwner::from_module_ref_with_verification_budget_v12(&module(), &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let (inventory, storage) = CanonicalKirInventoryV1::derive(&owner, &mut budget).unwrap();
    budget.reserve_storage(storage.retained_storage()).unwrap();
    let sites = [
        Some((
            SemanticFunctionIdV1::from_index(0),
            SemanticBlockIdV1::from_index(0),
            3,
        )),
        Some((
            SemanticFunctionIdV1::from_index(0),
            SemanticBlockIdV1::from_index(0),
            9,
        )),
    ];
    let mut rows = inventory
        .operations()
        .iter()
        .map(|op| CanonicalKirOperationTransitionV1 {
            output: op.coordinate,
            origin: CanonicalKirOperationOriginV1::Retained(op.coordinate),
        })
        .collect::<Vec<_>>();
    assert_eq!(
        retained_sites(&inventory, &inventory, &sites, &rows, &mut budget).unwrap(),
        sites
    );
    rows[0].origin = CanonicalKirOperationOriginV1::Retained(inventory.operations()[1].coordinate);
    assert_eq!(
        retained_sites(&inventory, &inventory, &sites, &rows, &mut budget).unwrap()[0],
        sites[1]
    );
    rows[0].origin =
        CanonicalKirOperationOriginV1::ConstantFrom(inventory.definitions()[0].coordinate);
    assert_eq!(
        retained_sites(&inventory, &inventory, &sites, &rows, &mut budget).unwrap()[0],
        None
    );
    assert!(retained_sites(&inventory, &inventory, &sites[..1], &rows, &mut budget).is_err());
    assert!(retained_sites(&inventory, &inventory, &sites, &rows[..1], &mut budget).is_err());
    rows[1].output = rows[0].output;
    assert!(matches!(
        retained_sites(&inventory, &inventory, &sites, &rows, &mut budget),
        Err(E::Unsupported {
            phase: "redundant Store source",
            detail: "exact retained operation coordinate"
        })
    ));
}
